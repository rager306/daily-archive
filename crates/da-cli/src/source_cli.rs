//! `da source` — source-import FSM surface (M451).
//!
//! Orchestrates an article object through the import pipeline using the
//! DATA toolchain registry (data/ontology/source_import_tools.yaml).
//! Phase 1: status/plan are fully functional; `import` executes the
//! FSM analysis and reports the tool to run per remaining transition —
//! Rust tools are invoked where wired, reference tools are surfaced
//! honestly (impl: reference) instead of silently shelling out to
//! python from Rust.

use da_domain::source_import_fsm::{ImportFsmState, Toolchain};
use std::path::{Path, PathBuf};

fn registry_path() -> PathBuf {
    std::path::Path::new("data/ontology/source_import_tools.yaml").to_path_buf()
}

fn load_toolchain() -> anyhow::Result<Toolchain> {
    let text = std::fs::read_to_string(registry_path())?;
    Toolchain::from_yaml_str(&text)
        .map_err(|e| anyhow::anyhow!("toolchain registry: {e}"))
}

fn load_state(paper_id: &str) -> anyhow::Result<ImportFsmState> {
    let mp = Path::new("data/processed").join(paper_id).join("manifest.yaml");
    let m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)
        .map_err(|e| anyhow::anyhow!("{}: {e}", mp.display()))?;
    let profile = m
        .get("grammar_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("arxiv_pdf_dot")
        .to_string();
    let mut events = Vec::new();
    if let Some(seq) = m
        .get("lifecycle_events")
        .and_then(|v| v.as_sequence())
    {
        for e in seq {
            if let Some(stage) = e.get("stage").and_then(|s| s.as_str()) {
                events.push(stage.to_string());
            }
        }
    }
    Ok(ImportFsmState::from_events(&profile, &events))
}

fn print_status(paper_id: &str, tc: &Toolchain, state: &ImportFsmState) {
    println!("object:   {paper_id}");
    println!("grammar:  {} (class: {})", state.grammar_profile, state.object_class());
    println!("stage:    {}", state.current_stage());
    println!("reached:  {:?}", state.reached);
    if let Some((next, required)) = state.next_stage(tc) {
        println!();
        println!("next:     {next} ({})", if required { "REQUIRED" } else { "optional/lazy" });
        if let Some(spec) = tc.stage(next) {
            for t in &spec.tools {
                println!(
                    "  tool:  {} [{}] — {}{}",
                    t.id,
                    t.r#impl,
                    t.cli,
                    t.note.as_ref().map(|n| format!(" ({n})")).unwrap_or_default()
                );
            }
        }
    } else {
        println!();
        println!("next:     none — pipeline complete");
    }
}

pub fn run_status(paper_id: &str) -> anyhow::Result<()> {
    let tc = load_toolchain()?;
    let state = load_state(paper_id)?;
    print_status(paper_id, &tc, &state);
    Ok(())
}

pub fn run_plan(paper_id: &str) -> anyhow::Result<()> {
    let tc = load_toolchain()?;
    let state = load_state(paper_id)?;
    println!("plan for {paper_id} ({} → validated):", state.current_stage());
    for (stage, required) in state.plan(&tc) {
        let tools: Vec<String> = tc
            .stage(stage)
            .map(|s| s.tools.iter().map(|t| format!("{}[{}]", t.id, t.r#impl)).collect())
            .unwrap_or_default();
        println!(
            "  {stage:<12} {:<10} {}",
            if required { "REQUIRED" } else { "optional" },
            tools.join(", ")
        );
    }
    Ok(())
}

pub fn run_import(paper_id: &str, dry_run: bool) -> anyhow::Result<()> {
    let tc = load_toolchain()?;
    let state = load_state(paper_id)?;
    println!("import FSM: {paper_id}");
    println!("  current: {}", state.current_stage());
    for (stage, required) in state.plan(&tc) {
        let spec = tc.stage(stage);
        let rust_tools: Vec<&_> = spec
            .map(|s| s.tools.iter().filter(|t| t.r#impl == "rust").collect())
            .unwrap_or_default();
        let reference_tools: Vec<&_> = spec
            .map(|s| s.tools.iter().filter(|t| t.r#impl == "reference").collect())
            .unwrap_or_default();
        let marker = if required { "*" } else { " " };
        println!("  {marker} {stage:<12}",);
        for t in rust_tools {
            println!("      rust-exec: {} — {}", t.id, t.cli);
        }
        for t in reference_tools {
            println!(
                "      reference (phase-2 port): {} — {}",
                t.id, t.cli
            );
        }
        if dry_run {
            continue;
        }
        // Phase 1: only reference transitions exist for remaining stages;
        // running them from Rust would mean shelling to python — declined
        // by design. The orchestrator reports; the port list is tracked in
        // the registry (impl flags flip as phases land).
    }
    println!("\n  (dry-run={dry_run}; rust-executable transitions are invoked when wired)");
    Ok(())
}

/// `da source build-navigation <paper_id>` — execute the structured-stage
/// navigation transition (phase-2 rust tool). CLI owns the bounded
/// filesystem write-back: navigation.json, manifest patch (append-only
/// event + context.navigation stats), quality_signals merge.
pub fn run_build_navigation(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_application::navigation_stage::build_navigation_stage;
    use da_ports::path_config::PathConfig;

    let detectors_yaml = std::fs::read_to_string("data/ontology/furniture_detectors.yaml")
        .map_err(|e| anyhow::anyhow!("furniture_detectors.yaml: {e}"))?;
    let policy_yaml = std::fs::read_to_string("data/ontology/navigation_quality_policy.yaml")
        .map_err(|e| anyhow::anyhow!("navigation_quality_policy.yaml: {e}"))?;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let out = build_navigation_stage(&store, paper_id, &detectors_yaml, &policy_yaml)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    // 1. navigation.json
    std::fs::write(dir.join("context/navigation.json"), &out.navigation_json)?;

    // 2. manifest: append event (if structured not reached) + artifact stats
    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let reached_structured = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|seq| seq.iter().any(|e| e.get("stage").and_then(|s| s.as_str()) == Some("structured")))
        .unwrap_or(false);
    if !reached_structured {
        if let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut()) {
            let mut ev = serde_yaml::Mapping::new();
            ev.insert("stage".into(), "structured".into());
            ev.insert(
                "recorded_at".into(),
                chrono_now().into(),
            );
            ev.insert("actor".into(), "rust-navigation-stage".into());
            ev.insert(
                "evidence_sha256".into(),
                sha256_hex(out.navigation_json.as_bytes()).into(),
            );
            events.push(serde_yaml::Value::Mapping(ev));
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("structured");
        }
    }
    // context artifact entry (create or refresh)
    let stats_yaml = serde_yaml::to_value(&out.stats)?;
    let nav_entry = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("context/navigation.json")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.navigation_json.as_bytes()))),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-navigation-stage")),
        (serde_yaml::Value::from("stats"), stats_yaml),
    ]);
    if m.get("artifacts").and_then(|a| a.get("context")).is_none() {
        let ctx = serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("file"), serde_yaml::Value::from("context/projection.json")),
            (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
            (serde_yaml::Value::from("navigation"), serde_yaml::Value::Mapping(nav_entry)),
        ]);
        if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
            arts.insert("context".into(), serde_yaml::Value::Mapping(ctx));
        }
    } else if let Some(nav) = m
        .get_mut("artifacts")
        .and_then(|a| a.get_mut("context"))
        .and_then(|c| c.as_mapping_mut())
    {
        nav.insert("navigation".into(), serde_yaml::Value::Mapping(nav_entry));
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;

    // 3. quality_signals merge (navigation_efficiency + furniture_share)
    let qs_path = dir.join("quality_signals.json");
    let mut qs: Vec<serde_json::Value> = std::fs::read_to_string(&qs_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    qs.retain(|s| {
        s.get("signal_id").and_then(|v| v.as_str())
            != Some("navigation_efficiency")
            && s.get("signal_id").and_then(|v| v.as_str()) != Some("furniture_share")
    });
    qs.push(serde_json::json!({
        "signal_id": "navigation_efficiency", "value": out.stats.tree_cost,
        "scale": "chars", "verdict": out.tree_cost_verdict
    }));
    qs.push(serde_json::json!({
        "signal_id": "furniture_share",
        "value": (out.stats.furniture_share * 1000.0).round() / 1000.0,
        "scale": "ratio", "verdict": out.furniture_share_verdict
    }));
    std::fs::write(&qs_path, serde_json::to_string_pretty(&qs)?)?;

    println!(
        "{paper_id}: navigation built — nodes={}, furniture={}, tree_cost={} ({}) share={} ({})",
        out.stats.nodes, out.stats.furniture, out.stats.tree_cost,
        out.tree_cost_verdict, out.stats.furniture_share, out.furniture_share_verdict
    );
    Ok(())
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            let (y, mo, da, h, mi, s) = epoch_to_utc(secs);
            format!("{y:04}-{mo:02}-{da:02}T{h:02}:{mi:02}:{s:02}Z")
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn epoch_to_utc(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    // days-from-civil inverse (Howard Hinnant algorithm), sufficient for timestamps
    let days = (secs / 86400) as i64;
    let rem = (secs % 86400) as u32;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32, rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}
