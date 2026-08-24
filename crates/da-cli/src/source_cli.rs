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
    let dir = std::path::Path::new("data/processed").join(paper_id);
    let type_detectors_yaml = std::fs::read_to_string("data/ontology/structural_type_detectors.yaml")
        .map_err(|e| anyhow::anyhow!("structural_type_detectors.yaml: {e}"))?;
    // CrossLayerInputs: aggregate from existing layer files
    let mut cross_layer = da_domain::navigation::CrossLayerInputs::default();
    if let Ok(raw) = std::fs::read_to_string(dir.join("extracted/entities.json"))
        && let Ok(ents) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(arr) = ents.as_array()
    {
        for (ordinal, e) in arr.iter().enumerate() {
            let label = e.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string();
            cross_layer.entities_by_ordinal.entry(ordinal).or_default().push(label);
        }
    }
    if let Ok(raw) = std::fs::read_to_string(dir.join("parsed/article.json")) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            for t in parsed.get("tables").and_then(|t| t.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                let path_id = t.get("section_path_id").and_then(|p| p.as_str()).unwrap_or("").to_string();
                let id = t.get("table_id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                cross_layer.tables_by_path.entry(path_id).or_default().push(id);
            }
            for f in parsed.get("formulas").and_then(|f| f.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                let path_id = f.get("section_path_id").and_then(|p| p.as_str()).unwrap_or("").to_string();
                let id = f.get("formula_id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                cross_layer.equations_by_path.entry(path_id).or_default().push(id);
            }
            for c in parsed.get("citations").and_then(|c| c.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                for a in c.get("anchors").and_then(|a| a.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                    let path_id = a.get("section_path_id").and_then(|p| p.as_str()).unwrap_or("").to_string();
                    *cross_layer.citation_anchors_by_path.entry(path_id).or_insert(0) += 1;
                }
            }
        }
    }
    // Section texts for text_preview (from parsed/article.json)
    if let Ok(raw) = std::fs::read_to_string(dir.join("parsed/article.json")) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(secs) = parsed.get("sections").and_then(|s| s.as_array()) {
                for (i, s) in secs.iter().enumerate() {
                    let text = s.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    cross_layer.section_texts.insert(i, text);
                }
            }
        }
    }

    let figures_file = dir.join("figures/index.json");
    if let Ok(figs_raw) = std::fs::read_to_string(&figures_file) {
        if let Ok(figs) = serde_json::from_str::<serde_json::Value>(&figs_raw) {
            for f in figs.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                let page = f.get("page").and_then(|p| p.as_u64()).unwrap_or(0);
                let fig_id = f.get("figure_id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                cross_layer.figures_by_path.entry(format!("{page}")).or_default().push(fig_id);
            }
        }
    }

    let out = build_navigation_stage(&store, paper_id, &detectors_yaml, &type_detectors_yaml, &policy_yaml, &cross_layer)?;

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

/// `da source build-structural <paper_id>` — structural-elements transition.
pub fn run_build_structural(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_application::structural_elements_stage::build_structural_elements;
    use da_ports::path_config::PathConfig;

    let detectors_yaml = std::fs::read_to_string("data/ontology/structural_elements_detectors.yaml")
        .map_err(|e| anyhow::anyhow!("structural_elements_detectors.yaml: {e}"))?;
    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let out = build_structural_elements(&store, paper_id, &detectors_yaml)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::write(dir.join("context/structural_elements.json"), &out.elements_json)?;

    // manifest: context.structural_elements entry (append-only: refresh stats only)
    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let by_kind: serde_yaml::Mapping = out
        .by_kind
        .iter()
        .map(|(k, v)| (serde_yaml::Value::from(k.as_str()), serde_yaml::Value::from(*v as u64)))
        .collect();
    let entry = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("context/structural_elements.json")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.elements_json.as_bytes()))),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-structural-stage")),
        (serde_yaml::Value::from("stats"), serde_yaml::Value::Mapping(
            serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("count"), serde_yaml::Value::from(out.elements.len() as u64)),
                (serde_yaml::Value::from("definition_sources"), serde_yaml::Value::from(out.definition_sources as u64)),
                (serde_yaml::Value::from("by_kind"), serde_yaml::Value::Mapping(by_kind)),
            ]),
        )),
    ]);
    if let Some(ctx) = m
        .get_mut("artifacts")
        .and_then(|a| a.get_mut("context"))
        .and_then(|c| c.as_mapping_mut())
    {
        ctx.insert("structural_elements".into(), serde_yaml::Value::Mapping(entry));
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;

    println!(
        "{paper_id}: structural elements — count={} definition_sources={} kinds={:?}",
        out.elements.len(),
        out.definition_sources,
        out.by_kind
    );
    Ok(())
}

/// `da source build-figures <paper_id>` — figures transition: Rust
/// analytic core over the observation cache (reference exporter).
pub fn run_build_figures(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_application::figures_stage::build_figures_stage;
    use da_ports::path_config::PathConfig;

    let obs_path = std::path::Path::new("artifacts/pipeline_cache/pdf_observations")
        .join(format!("{paper_id}.json"));
    let obs_json = std::fs::read_to_string(&obs_path)
        .map_err(|e| anyhow::anyhow!("observations {} (run scripts/m451_pdf_observations.py): {e}", obs_path.display()))?;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let out = build_figures_stage(&store, paper_id, &obs_json)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::write(dir.join("figures/index.json"), &out.index_json)?;

    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let stats_yaml = serde_yaml::to_value(&out.stats)?;
    let entry = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("figures/index.json")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.index_json.as_bytes()))),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-figures-stage")),
        (serde_yaml::Value::from("stats"), stats_yaml),
    ]);
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        arts.insert("figures".into(), serde_yaml::Value::Mapping(entry));
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;

    println!(
        "{paper_id}: figures — indexed={} with_files={} captions={} unassigned_images={}",
        out.stats.figures_indexed, out.stats.with_image_file,
        out.stats.captions_in_text, out.stats.unassigned_images
    );
    Ok(())
}

/// `da source build-figures-tei <paper_id>` — TEI figures transition
/// (arxiv_pdf_dot grammar: untyped <figure> from parsed tei_xml).
pub fn run_build_figures_tei(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_application::tei_figures_stage::build_tei_figures_stage;
    use da_ports::path_config::PathConfig;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let out = build_tei_figures_stage(&store, paper_id)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("figures"))?;
    std::fs::write(dir.join("figures/index.json"), &out.index_json)?;

    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let stats_yaml = serde_yaml::to_value(&out.stats)?;
    let entry = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("figures/index.json")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.index_json.as_bytes()))),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-tei-figures-stage")),
        (serde_yaml::Value::from("stats"), stats_yaml),
    ]);
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        arts.insert("figures".into(), serde_yaml::Value::Mapping(entry));
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
    println!(
        "{paper_id}: tei figures — count={} with_caption={} with_graphic={} refs={}",
        out.stats.count, out.stats.with_caption, out.stats.with_graphic, out.stats.total_in_text_refs
    );
    Ok(())
}

/// `da source parse-html <object_id> --html <path>` — html_headings
/// grammar: create parsed + projection + navigation + manifest (new or
/// idempotent rebuild from source truth).
pub fn run_parse_html(paper_id: &str, html_path: Option<&str>) -> anyhow::Result<()> {
    use da_application::html_parsed_stage::build_html_parsed;

    let Some(path) = html_path else {
        anyhow::bail!("parse-html requires --html <path>");
    };
    let html = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {path}: {e}"))?;
    let out = build_html_parsed(&html, paper_id)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("parsed"))?;
    std::fs::create_dir_all(dir.join("context"))?;
    std::fs::write(dir.join("parsed/article.json"), &out.parsed_json)?;
    std::fs::write(dir.join("context/projection.json"), &out.projection_json)?;

    // draft manifest (idempotent: rebuilt from parsed truth)
    let now = chrono_now();
    let sha_p = sha256_hex(out.parsed_json.as_bytes());
    let mut m = serde_yaml::Mapping::new();
    m.insert("schema_version".into(), "article-manifest.v1".into());
    m.insert("paper_id".into(), paper_id.into());
    m.insert("grammar_profile".into(), "html_headings".into());
    let mut src = serde_yaml::Mapping::new();
    src.insert("code".into(), "textbook".into());
    m.insert("source".into(), src.into());
    m.insert("lifecycle".into(), "parsed".into());
    let ev_raw = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("stage"), serde_yaml::Value::from("raw")),
        (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(now.clone())),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("html-chapter-source")),
    ]);
    let ev_parsed = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("stage"), serde_yaml::Value::from("parsed")),
        (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(now)),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-html-parsed-stage")),
        (serde_yaml::Value::from("evidence_sha256"), serde_yaml::Value::String(sha_p.clone())),
    ]);
    m.insert("lifecycle_events".into(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Mapping(ev_raw), serde_yaml::Value::Mapping(ev_parsed)]));
    let mut arts = serde_yaml::Mapping::new();
    let miss = |kind: &str, reason: &str| {
        serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("present"), serde_yaml::Value::from(false)),
            (serde_yaml::Value::from(kind), serde_yaml::Value::from(reason)),
            (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
        ])
    };
    let nr = || serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("not_reached"), serde_yaml::Value::from(true)),
    ]);
    for (k, v) in [
        ("extracted", miss("missingness", "extraction_not_run")),
        ("citations_resolved", miss("missingness", "no_citation_elements")),
        ("equations", miss("missingness", "no_formula_elements")),
        ("tables", miss("missingness", "no_table_elements")),
        ("figures", miss("missingness", "no_figure_elements")),
        ("external_resources", miss("missingness", "no_external_resources")),
        ("enriched", nr()),
        ("summaries", nr()),
        ("quality_signals", nr()),
    ] {
        arts.insert(k.into(), serde_yaml::Value::Mapping(v));
    }
    arts.insert("parsed".into(), serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("parsed/article.json")),
        (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("source")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha_p)),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-html-parsed-stage")),
        (serde_yaml::Value::from("stats"), serde_yaml::Value::Mapping(
            serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("sections"), serde_yaml::Value::from(out.sections as u64)),
            ]))),
    ]).into());
    m.insert("artifacts".into(), arts.into());
    std::fs::write(dir.join("manifest.yaml"), serde_yaml::to_string(&serde_yaml::Value::Mapping(m))?)?;

    // reuse the rust navigation stage for structure
    run_build_navigation(paper_id)?;
    println!("{paper_id}: html parsed — sections={}", out.sections);
    Ok(())
}

/// `da source parse-pdf-toc <object_id>` — page_outline grammar parsed
/// stage over the toc observation cache (reference extractor).
pub fn run_parse_pdf_toc(paper_id: &str) -> anyhow::Result<()> {
    use da_application::pdf_toc_stage::build_pdf_toc_stage;

    let obs_path = std::path::Path::new("artifacts/pipeline_cache/pdf_toc_observations")
        .join(format!("{paper_id}.json"));
    let obs_json = std::fs::read_to_string(&obs_path)
        .map_err(|e| anyhow::anyhow!("observations {} (run scripts/m451_pdf_toc_observations.py): {e}", obs_path.display()))?;

    let out = build_pdf_toc_stage(paper_id, &obs_json)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("parsed"))?;
    std::fs::create_dir_all(dir.join("context"))?;
    std::fs::write(dir.join("parsed/article.json"), &out.parsed_json)?;
    std::fs::write(dir.join("context/projection.json"), &out.projection_json)?;

    // manifest: parsed event (idempotent full draft, like parse-html)
    let now = chrono_now();
    let sha_p = sha256_hex(out.parsed_json.as_bytes());
    let miss = |reason: &str| serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("present"), serde_yaml::Value::from(false)),
        (serde_yaml::Value::from("missingness"), serde_yaml::Value::from(reason)),
        (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
    ]);
    let nr = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("not_reached"), serde_yaml::Value::from(true)),
    ]);
    let mut m = serde_yaml::Mapping::new();
    m.insert("schema_version".into(), "article-manifest.v1".into());
    m.insert("paper_id".into(), paper_id.into());
    m.insert("grammar_profile".into(), "page_outline".into());
    let mut src = serde_yaml::Mapping::new();
    src.insert("code".into(), "textbook".into());
    m.insert("source".into(), src.into());
    m.insert("lifecycle".into(), "parsed".into());
    let ev = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("stage"), serde_yaml::Value::from("raw")),
        (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(now.clone())),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("monograph-source")),
    ]);
    let ev2 = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("stage"), serde_yaml::Value::from("parsed")),
        (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(now)),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-pdf-toc-stage")),
        (serde_yaml::Value::from("evidence_sha256"), serde_yaml::Value::String(sha_p.clone())),
    ]);
    m.insert("lifecycle_events".into(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Mapping(ev), serde_yaml::Value::Mapping(ev2)]));
    let mut arts = serde_yaml::Mapping::new();
    for (k, v) in [
        ("extracted", miss("extraction_not_run")),
        ("citations_resolved", miss("no_citation_elements")),
        ("equations", miss("no_formula_elements")),
        ("tables", miss("no_table_elements")),
        ("figures", miss("no_figure_elements")),
        ("external_resources", miss("no_external_resources")),
        ("enriched", nr.clone()),
        ("summaries", nr.clone()),
        ("quality_signals", nr),
    ] {
        arts.insert(k.into(), serde_yaml::Value::Mapping(v));
    }
    arts.insert("parsed".into(), serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("file"), serde_yaml::Value::from("parsed/article.json")),
        (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("source")),
        (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha_p)),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-pdf-toc-stage")),
        (serde_yaml::Value::from("stats"), serde_yaml::Value::Mapping(
            serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("sections"), serde_yaml::Value::from(out.sections as u64)),
                (serde_yaml::Value::from("furniture"), serde_yaml::Value::from(out.furniture as u64)),
            ]))),
    ]).into());
    m.insert("artifacts".into(), arts.into());
    std::fs::write(dir.join("manifest.yaml"), serde_yaml::to_string(&serde_yaml::Value::Mapping(m))?)?;

    // structure via the rust navigation stage
    run_build_navigation(paper_id)?;
    println!("{paper_id}: pdf-toc parsed — sections={} furniture={}", out.sections, out.furniture);
    Ok(())
}

/// `da source enrich-layer <paper_id>` — enriched layer (OpenAlex work
/// anchor + positional author binding) over the reference fetch cache.
pub fn run_enrich_layer(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_application::enriched_layer_stage::build_enriched_layer;
    use da_ports::path_config::PathConfig;

    let obs_path = std::path::Path::new("artifacts/pipeline_cache/openalex_works")
        .join(format!("{paper_id}.json"));
    let obs = std::fs::read_to_string(&obs_path)
        .map_err(|e| anyhow::anyhow!("work cache {} (run scripts/m451_enrich_openalex.py): {e}", obs_path.display()))?;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let out = build_enriched_layer(&store, paper_id, &obs)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;

    if out.not_found {
        if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
            arts.insert("enriched".into(), serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("present"), serde_yaml::Value::from(false)),
                (serde_yaml::Value::from("missingness"), serde_yaml::Value::from("openalex_not_found")),
                (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
            ]).into());
        }
        // typed event: enriched attempted, not found — FSM stage reached
        let has_enriched = m
            .get("lifecycle_events")
            .and_then(|e| e.as_sequence())
            .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("enriched")))
            .unwrap_or(false);
        if !has_enriched {
            let events = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut());
            if let Some(events) = events {
                events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                    (serde_yaml::Value::from("stage"), serde_yaml::Value::from("enriched")),
                    (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
                    (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-enriched-layer")),
                    (serde_yaml::Value::from("evidence_sha256"), serde_yaml::Value::String(
                        sha256_hex(b"openalex_not_found"))),
                ])));
            }
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("enriched");
        }
        std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
        println!("{paper_id}: enriched (honest pending: openalex_not_found)");
        return Ok(());
    }

    std::fs::create_dir_all(dir.join("enriched"))?;
    std::fs::write(dir.join("enriched/authors.json"), &out.authors_json)?;
    std::fs::write(dir.join("enriched/venue.json"), &out.venue_json)?;
    std::fs::write(dir.join("enriched/topics.json"), &out.topics_json)?;
    std::fs::write(dir.join("enriched/provenance.json"), &out.provenance_json)?;

    // append-only event + artifact stats (skip if already enriched)
    let already = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("enriched")))
        .unwrap_or(false);
    if !already {
        if let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut()) {
            events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("stage"), serde_yaml::Value::from("enriched")),
                (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
                (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-enriched-layer")),
                (serde_yaml::Value::from("evidence_sha256"),
                 serde_yaml::Value::String(sha256_hex(out.provenance_json.as_bytes()))),
            ])));
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("enriched");
        }
    }
    let stats: serde_yaml::Value = serde_yaml::from_str(&serde_json::to_string(&out.stats).unwrap())?;
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        arts.insert("enriched".into(), serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("file"), serde_yaml::Value::from("enriched/authors.json")),
            (serde_yaml::Value::from("files"), serde_yaml::Value::from("enriched/{authors,venue,topics,provenance}.json")),
            (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
            (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.provenance_json.as_bytes()))),
            (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-enriched-layer")),
            (serde_yaml::Value::from("stats"), stats),
        ]).into());
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
    println!("{paper_id}: enriched — {}", serde_json::to_string(&out.stats)?);
    Ok(())
}

/// `da source build-summary <paper_id>` — run the summarizer AGENT
/// (manifest-driven: binding from models.yaml, prompt file, CVF memory)
/// and persist summaries/abstract.json + lifecycle event.
pub fn run_build_summary(paper_id: &str) -> anyhow::Result<()> {
    use da_adapters::agent_memory::CvfAgentMemory;
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_adapters::model_registry::load_model_registry;
    use da_adapters::openai_compatible_llm::OpenAiCompatibleLlmClient;
    use da_application::summaries_agent_stage::run_summaries_agent;
    use da_domain::agent_manifest::AgentManifest;
    use da_ports::path_config::PathConfig;

    // agent manifest (DATA) — summarizer for now; agent-id arg later
    let agent_yaml = std::fs::read_to_string("data/agents/summarizer/manifest.yaml")
        .map_err(|e| anyhow::anyhow!("agent manifest: {e}"))?;
    let manifest = AgentManifest::from_yaml_str(&agent_yaml)
        .map_err(|e| anyhow::anyhow!("agent manifest: {e}"))?;
    manifest.check_locks().map_err(|e| anyhow::anyhow!("agent locks: {e}"))?;

    let system_prompt = std::fs::read_to_string(&manifest.prompt.system_file)
        .map_err(|e| anyhow::anyhow!("system prompt {}: {e}", manifest.prompt.system_file))?;
    let injection_yaml = std::fs::read_to_string("data/ontology/prompt_injection_policy.yaml")?;

    // resolve binding -> model/endpoint (registry DATA)
    let registry = load_model_registry(std::path::Path::new("models.yaml"))
        .map_err(|e| anyhow::anyhow!("models.yaml: {e:?}"))?;
    let binding = registry
        .resolve(&manifest.model.generate_binding, None)
        .map_err(|e| anyhow::anyhow!("binding {}: {e}", manifest.model.generate_binding))?;
    let key = std::env::var("NINE_ROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty());
    let llm = OpenAiCompatibleLlmClient::new(binding.clone(), key, 240)
        .map_err(|e| anyhow::anyhow!("llm client: {e}"))?;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;

    // CVF memory: per-agent namespace, persist after the run
    let ns = manifest.memory.namespace.clone().unwrap_or_else(|| manifest.agent_id.clone());
    let mem_dir = std::path::Path::new("data/agent_memory").join(&ns);
    let mut memory = CvfAgentMemory::open(mem_dir)
        .map_err(|e| anyhow::anyhow!("cvf memory: {e}"))?;

    let rt = tokio::runtime::Runtime::new()?;
    let out = rt.block_on(run_summaries_agent(
        da_application::summaries_agent_stage::AgentRunContext {
            store: &store,
            memory: &mut memory,
            llm: &llm,
            manifest: &manifest,
            model_name: &binding.model_name,
            system_prompt: &system_prompt,
            injection_policy_yaml: &injection_yaml,
            paper_id,
        },
    ))?;
    use da_ports::agent_memory::AgentMemory as _;
    memory.persist().ok();

    if !out.ok {
        anyhow::bail!("agent produced unparseable output for {paper_id}");
    }
    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("summaries"))?;
    std::fs::write(dir.join("summaries/abstract.json"), &out.summary_node_json)?;

    // FSM guard: enriched must be reached before summarized (lifecycle order)
    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let reached_enriched = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("enriched")))
        .unwrap_or(false);
    if !reached_enriched {
        anyhow::bail!(
            "build-summary: enriched stage not reached for {paper_id} —              run 'da source enrich-layer {paper_id}' first (FSM lifecycle order)"
        );
    }
    let already = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("summarized")))
        .unwrap_or(false);
    if !already {
        if let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut()) {
            events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("stage"), serde_yaml::Value::from("summarized")),
                (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
                (serde_yaml::Value::from("actor"), serde_yaml::Value::from(format!("agent:{}", manifest.agent_id))),
                (serde_yaml::Value::from("evidence_sha256"),
                 serde_yaml::Value::String(sha256_hex(out.summary_node_json.as_bytes()))),
            ])));
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("summarized");
        }
    }
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        let mut sm = serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("file"), serde_yaml::Value::from("summaries/abstract.json")),
            (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
            (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha256_hex(out.summary_node_json.as_bytes()))),
            (serde_yaml::Value::from("actor"), serde_yaml::Value::from(format!("agent:{}", manifest.agent_id))),
        ]);
        sm.insert("stats".into(), serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("prompt_sha256"), serde_yaml::Value::String(out.prompt_sha256.clone())),
            (serde_yaml::Value::from("injection_scrubbed"), serde_yaml::Value::from(out.scrubbed as u64)),
            (serde_yaml::Value::from("latency_ms"), serde_yaml::Value::from(out.latency_ms)),
            (serde_yaml::Value::from("memory_epoch"), serde_yaml::Value::from(out.epoch.unwrap_or(0))),
        ]).into());
        arts.insert("summaries".into(), sm.into());
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;

    println!(
        "{paper_id}: summarized by agent {} — epoch={:?} latency={}ms scrubbed={}",
        manifest.agent_id, out.epoch, out.latency_ms, out.scrubbed
    );
    Ok(())
}


/// `da source eval-frozen` — run the frozen 6-article benchmark for
/// EVERY model in the frozen eval manifest, writing ALL trajectories to
/// CVF (comparative decision base; prod binding untouched).
pub fn run_eval_frozen() -> anyhow::Result<()> {
    use da_adapters::agent_memory::CvfAgentMemory;
    use da_adapters::fs_schema_objects::FsSchemaObjectStore;
    use da_adapters::model_registry::load_model_registry;
    use da_adapters::openai_compatible_llm::OpenAiCompatibleLlmClient;
    use da_application::summaries_agent_stage::{run_summaries_agent, AgentRunContext};
    use da_domain::agent_manifest::AgentManifest;
    use da_ports::path_config::PathConfig;

    let fixture_path = std::path::Path::new(
        "governor/fixtures/prompt_eval/article-summary.eval.v1.json");
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixture_path)?)?;
    let articles: Vec<String> = fx["frozen_articles"]
        .as_array()
        .expect("frozen_articles")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let baseline_model = fx["baseline"]["model"].as_str().unwrap_or("unknown");

    let agent_yaml = std::fs::read_to_string("data/agents/summarizer/manifest.yaml")?;
    let manifest = AgentManifest::from_yaml_str(&agent_yaml)
        .map_err(|e| anyhow::anyhow!("agent manifest: {e}"))?;
    let system_prompt =
        std::fs::read_to_string("data/prompts/summarizer/system.v1.md")?;
    let injection_yaml =
        std::fs::read_to_string("data/ontology/prompt_injection_policy.yaml")?;

    // candidate models from the benchmark policy (DATA), baseline included;
    // each gets its own CVF namespace -> comparative but non-mixing
    let policy = std::fs::read_to_string("data/ontology/summary_eval_policy.yaml")
        .map_err(|e| anyhow::anyhow!("summary_eval_policy: {e}"))?;
    let pol: serde_yaml::Value = serde_yaml::from_str(&policy)?;
    let mut models: Vec<String> = Vec::new();
    for m in pol.get("models").and_then(|m| m.as_sequence()).map(|s| s.as_slice()).unwrap_or(&[]) {
        match m {
            serde_yaml::Value::String(s) => models.push(s.clone()),
            serde_yaml::Value::Mapping(mm) => {
                if let Some(serde_yaml::Value::String(n)) = mm.get("name") {
                    models.push(n.clone());
                }
            }
            _ => {}
        }
    }
    models.sort();
    models.dedup();

    let cfg = PathConfig::load();
    use da_ports::agent_memory::AgentMemory as _;
    let key: Option<String> = std::env::var("NINE_ROUTER_API_KEY").ok();
    let registry = load_model_registry(std::path::Path::new("models.yaml"))
        .map_err(|e| anyhow::anyhow!("models.yaml: {}", e))?;
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let rt = tokio::runtime::Runtime::new()?;
    let injection_policy = compile_injection(injection_yaml);

    println!(
        "frozen eval: {} articles × {} models (baseline {baseline_model})",
        articles.len(),
        models.len()
    );
    for model in &models {
        // namespace per MODEL: comparative trajectories, no mixing
        let mem_dir = std::path::Path::new("data/agent_memory/eval").join(model.replace('/', "-"));
        let mut memory = CvfAgentMemory::open(mem_dir)
            .map_err(|e| anyhow::anyhow!("cvf memory: {e}"))?;
        // resolve by model name (the agent contract rejects model !=
        // binding — so the client itself must match the candidate)
        let resolved =
            da_adapters::model_registry::resolve_model_record(&registry, model)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        let llm = OpenAiCompatibleLlmClient::new(resolved.clone(), key.clone(), 240)
            .map_err(|e| anyhow::anyhow!("llm client: {e}"))?;

        for pid in &articles {
            let ctx = AgentRunContext {
                store: &store,
                memory: &mut memory,
                llm: &llm,
                manifest: &manifest,
                model_name: model,
                system_prompt: &system_prompt,
                injection_policy_yaml: &injection_policy,
                paper_id: pid,
            };
            let result = rt.block_on(run_summaries_agent(ctx));
            match result {
                Ok(out) => {
                    memory.persist().ok();
                    println!(
                        "  {model} × {pid}: ok={} grounding={:.3} lat={}ms",
                        out.ok, out.grounding, out.latency_ms
                    );
                }
                Err(e) => println!("  {model} × {pid}: FAIL {e}"),
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    println!(
        "\ntrajectories written to data/agent_memory/eval/<model>/ — \
         comparative base for routing decisions (SONA W3 reads these)"
    );
    Ok(())
}

fn compile_injection(yaml_text: String) -> String {
    yaml_text
}

/// `da source parse-pdf <paper_id> --pdf <path>` — GROBID structural ->
/// parsed/article.json + context/projection.json + manifest draft
/// (idempotent; the article FSM then continues via build-* commands).
pub fn run_parse_pdf(paper_id: &str, pdf_path: Option<&str>) -> anyhow::Result<()> {
    use da_adapters::GrobidParser;
    use da_ports::parser::ParserPort;

    let Some(pdf) = pdf_path else {
        anyhow::bail!("parse-pdf requires --pdf <path>");
    };
    let rt = tokio::runtime::Runtime::new()?;
    let parsed = rt.block_on(async {
        let parser = GrobidParser::from_env();
        parser.parse_pdf(pdf, paper_id).await.map_err(|e| anyhow::anyhow!("grobid: {e}"))
    })?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("parsed"))?;
    std::fs::create_dir_all(dir.join("context"))?;

    let parsed_json = serde_json::to_string_pretty(&parsed)?;
    std::fs::write(dir.join("parsed/article.json"), &parsed_json)?;

    // projection from sections
    let proj = serde_json::json!({
        "paper_id": paper_id,
        "sections": parsed.sections.iter().enumerate().map(|(i, s)| {
            serde_json::json!({
                "section_id": format!("{paper_id}:{i}"),
                "path_id": s.path_id,
                "order": i,
                "level": s.level,
                "title": s.title,
                "char_count": s.text.chars().count(),
            })
        }).collect::<Vec<_>>(),
    });
    std::fs::write(dir.join("context/projection.json"), serde_json::to_string_pretty(&proj)?)?;

    // manifest draft (raw+parsed events), idempotent rebuild
    let now = chrono_now();
    let miss = |reason: &str| serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("present"), serde_yaml::Value::from(false)),
        (serde_yaml::Value::from("missingness"), serde_yaml::Value::from(reason)),
        (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
    ]);
    let nr = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("not_reached"), serde_yaml::Value::from(true)),
    ]);
    let mut m = serde_yaml::Mapping::new();
    m.insert("schema_version".into(), "article-manifest.v1".into());
    m.insert("paper_id".into(), paper_id.into());
    m.insert("grammar_profile".into(), "arxiv_pdf_dot".into());
    m.insert("lifecycle".into(), "parsed".into());
    let ev_raw = serde_yaml::Mapping::from_iter([
        ("stage".into(), "raw".into()),
        ("recorded_at".into(), now.clone().into()),
        ("actor".into(), "source-acquire".into()),
    ]);
    let ev_parsed = serde_yaml::Mapping::from_iter([
        (serde_yaml::Value::from("stage"), serde_yaml::Value::from("parsed")),
        (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(now.clone())),
        (serde_yaml::Value::from("actor"), serde_yaml::Value::from("grobid-structural")),
        (serde_yaml::Value::from("evidence_sha256"),
         serde_yaml::Value::String(sha256_hex(parsed_json.as_bytes()))),
    ]);
    m.insert("lifecycle_events".into(), vec![
        serde_yaml::Value::Mapping(ev_raw), serde_yaml::Value::Mapping(ev_parsed),
    ].into());
    let mut arts = serde_yaml::Mapping::new();
    for (k, v) in [
        ("extracted", miss("extraction_not_run")),
        ("citations_resolved", miss("no_citation_elements")),
        ("enriched", nr.clone()),
        ("summaries", nr.clone()),
        ("quality_signals", nr.clone()),
    ] {
        arts.insert(k.into(), serde_yaml::Value::Mapping(v));
    }
    arts.insert("parsed".into(), serde_yaml::Mapping::from_iter([
        ("file".into(), "parsed/article.json".into()),
        ("derived_from".into(), "source".into()),
        ("sha256".into(), sha256_hex(parsed_json.as_bytes()).into()),
        ("actor".into(), "grobid-structural".into()),
        ("stats".into(), serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
            ("sections".into(), parsed.sections.len().into()),
            ("citations".into(), parsed.citations.len().into()),
            ("tables".into(), parsed.tables.len().into()),
            ("formulas".into(), parsed.formulas.len().into()),
        ]))),
    ]).into());
    m.insert("artifacts".into(), arts.into());
    std::fs::write(dir.join("manifest.yaml"), serde_yaml::to_string(&m)?)?;
    println!(
        "{paper_id}: parsed — sections={} citations={} tables={} formulas={}",
        parsed.sections.len(), parsed.citations.len(), parsed.tables.len(), parsed.formulas.len()
    );
    Ok(())
}

/// `da source build-extracted <paper_id>` — run rule-based extraction
/// and persist extracted/entities.json (article layer; graph writes stay
/// with `da extract`).
pub fn run_build_extracted(paper_id: &str) -> anyhow::Result<()> {
    // parsed/article.json is the input; rule-based extractor over section texts
    let parsed_path = std::path::Path::new("data/processed")
        .join(paper_id)
        .join("parsed/article.json");
    let raw = std::fs::read_to_string(&parsed_path)
        .map_err(|e| anyhow::anyhow!("parsed unreadable {}: {e}", parsed_path.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parsed parse: {e}"))?;

    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(sections) = parsed.get("sections").and_then(|s| s.as_array()) {
        for sec in sections {
            pairs.push((
                sec.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                sec.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            ));
        }
    }

    let rt = tokio::runtime::Runtime::new()?;
    let entities = rt.block_on(async {
        use da_ports::Extractor as _;
        let ex = da_adapters::RuleBasedExtractor::new();
        ex.extract(&pairs).await
            .map_err(|e| anyhow::anyhow!("extraction: {e}"))
    })?;

    let mut records: Vec<serde_json::Value> = Vec::new();
    for e in &entities {
        // section ordinal from the source span's section (extractor keeps it)
        records.push(serde_json::json!({
            "label": e.label,
            "entity_type": format!("{:?}", e.entity_type),
            "section_ordinal": e.section_ordinal,
        }));
    }
    let json = serde_json::to_string_pretty(&records)?;

    let dir = std::path::Path::new("data/processed").join(paper_id);
    std::fs::create_dir_all(dir.join("extracted"))?;
    std::fs::write(dir.join("extracted/entities.json"), &json)?;

    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    let sha = sha256_hex(json.as_bytes());
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        arts.insert("extracted".into(), serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("file"), serde_yaml::Value::from("extracted/entities.json")),
            (serde_yaml::Value::from("derived_from"), serde_yaml::Value::from("parsed")),
            (serde_yaml::Value::from("sha256"), serde_yaml::Value::String(sha.clone())),
            (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rule-based-extractor")),
            (serde_yaml::Value::from("stats"), serde_yaml::Value::Mapping(
                serde_yaml::Mapping::from_iter([
                    (serde_yaml::Value::from("entities"), serde_yaml::Value::from(records.len() as u64)),
                ]))),
        ]).into());
        let already = m.get("lifecycle_events").and_then(|e| e.as_sequence())
            .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("extracted")))
            .unwrap_or(false);
        if !already
            && let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut())
        {
            events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("stage"), serde_yaml::Value::from("extracted")),
                (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
                (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rule-based-extractor")),
                (serde_yaml::Value::from("evidence_sha256"), serde_yaml::Value::String(sha)),
            ])));
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("extracted");
        }
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
    println!("{paper_id}: extracted — entities={}", records.len());
    Ok(())
}



/// `da source run <paper_id>` — full FSM orchestration: reads the plan,
/// executes every remaining stage via CLI-registered tools, stops on
/// first failure with an honest report. The operator gives one command;
/// the pipeline drives itself.
pub fn run_source_run(paper_id: &str) -> anyhow::Result<()> {
    use da_domain::source_import_fsm::{ImportFsmState, Toolchain};

    // load toolchain + FSM state
    let tc_text = std::fs::read_to_string("data/ontology/source_import_tools.yaml")
        .map_err(|e| anyhow::anyhow!("toolchain: {e}"))?;
    let tc = Toolchain::from_yaml_str(&tc_text)
        .map_err(|e| anyhow::anyhow!("toolchain: {e}"))?;
    let state = load_fsm_state(paper_id)?;

    println!("═══ FSM run: {paper_id} ═══");
    println!("stage: {} | grammar: {} ({})", state.current_stage(), state.grammar_profile, state.object_class());
    println!();

    let plan = state.plan(&tc);
    if plan.is_empty() {
        println!("✓ pipeline complete — nothing to do");
        return Ok(());
    }
    // ensure manifest exists for stages that write to it
    let mp = std::path::Path::new("data/processed").join(paper_id).join("manifest.yaml");
    if !mp.exists() {
        std::fs::create_dir_all(mp.parent().unwrap())?;
        let mut draft = serde_yaml::Mapping::new();
        draft.insert("schema_version".into(), "article-manifest.v1".into());
        draft.insert("paper_id".into(), serde_yaml::Value::from(paper_id));
        draft.insert("grammar_profile".into(), "arxiv_pdf_dot".into());
        draft.insert("lifecycle".into(), "raw".into());
        let raw_event = serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("stage"), serde_yaml::Value::from("raw")),
            (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
            (serde_yaml::Value::from("actor"), serde_yaml::Value::from("source-acquire")),
        ]);
        draft.insert("lifecycle_events".into(), vec![serde_yaml::Value::Mapping(raw_event)].into());
        draft.insert("artifacts".into(), serde_yaml::Mapping::new().into());
        std::fs::write(&mp, serde_yaml::to_string(&draft)?)?;
        println!("manifest: created (fresh)");
    }

    println!("plan: {} stages", plan.len());
    for (stage, required) in &plan {
        println!("  {} {stage}", if *required { "*" } else { " " });
    }
    println!();

    // stage → CLI action mapping (Rust tools only; reference tools are
    // reported honestly and the stage is marked blocked)
    // stage -> CLI subcommands; validate-objects has a different form
    let actions: std::collections::BTreeMap<&str, Vec<&str>> = [
        ("parsed", vec!["parse-pdf"]),
        ("structured", vec!["build-navigation", "build-figures-tei"]),
        ("extracted", vec!["build-extracted"]),
        ("enriched", vec!["enrich-layer"]),
        ("summarized", vec!["build-summary"]),
        ("validated", vec!["validate-objects"]),
    ].into_iter().collect::<std::collections::BTreeMap<&str, Vec<&str>>>();

    let mut completed = 0;
    let mut skipped_reference = 0;
    let mut failed: Option<String> = None;

    for (stage, required) in &plan {
        println!("── {stage} {}", if *required { "(required)" } else { "(optional)" });
        let Some(action_list) = actions.get(*stage) else {
            println!("   no rust tool for this stage; reference tools:");
            if let Some(spec) = tc.stage(stage) {
                for t in &spec.tools {
                    if t.r#impl == "reference" {
                        println!("     [ref] {} — {}", t.id, t.cli);
                        skipped_reference += 1;
                    }
                }
            }
            if *required {
                failed = Some(format!("stage {stage}: no rust tool available"));
                break;
            }
            println!("   (optional, skipping)");
            continue;
        };

        // special: parsed needs the PDF path — auto-find from catalog
        let pdf = if *stage == "parsed" {
            let mut hits = Vec::new();
            collect_pdfs("data/sources", paper_id, &mut hits);
            hits.first().cloned()
        } else {
            None
        };
        if *stage == "parsed" && pdf.is_none() {
            println!("   ✗ parse-pdf: no source PDF found for {paper_id}");
            failed = Some(format!("parsed: no PDF under data/sources for {paper_id}"));
            break;
        }

        // pre-stage hook: enriched needs work cache (reference fetcher)
        if *stage == "enriched" {
            let cache = std::path::Path::new("artifacts/pipeline_cache/openalex_works")
                .join(format!("{paper_id}.json"));
            if !cache.exists() {
                println!("   [pre] work cache missing — running reference fetcher…");
                let st = std::process::Command::new("python3")
                    .args(["scripts/m451_enrich_openalex.py", paper_id])
                    .output();
                match st {
                    Ok(o) if o.status.success() => println!("   [pre] cache built"),
                    Ok(o) => {
                        failed = Some(format!("enriched: fetcher failed: {}", String::from_utf8_lossy(&o.stderr)));
                        break;
                    }
                    Err(e) => {
                        failed = Some(format!("enriched: fetcher spawn: {e}"));
                        break;
                    }
                }
            }
        }

        let mut stage_ok = true;
        for action in action_list {
            let mut cmd = std::process::Command::new(std::env::current_exe()?);
            if *action == "validate-objects" {
                cmd.args(["validate-objects", "--class", "article", paper_id]);
            } else {
                cmd.args(["source", action, paper_id]);
            }
            if let Some(pdf_path) = &pdf {
                cmd.arg("--pdf").arg(pdf_path);
            }
            let result = cmd.output();
            match result {
                Ok(o) if o.status.success() => {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let line = stdout
                        .lines()
                        .filter(|l| !l.contains("Logging") && !l.trim().is_empty())
                        .last()
                        .unwrap_or("ok");
                    println!("   ✓ {action}: {line}");
                    completed += 1;
                    // validated stage: write the lifecycle event
                    if *action == "validate-objects" {
                        stamp_validated_event(paper_id)?;
                    }
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    let err = stderr
                        .lines()
                        .filter(|l| !l.contains("Logging") && !l.contains("INFO"))
                        .last()
                        .unwrap_or("unknown");
                    println!("   ✗ {action}: {err}");
                    stage_ok = false;
                    failed = Some(format!("{stage}/{action}: {err}"));
                    break;
                }
                Err(e) => {
                    println!("   ✗ {action}: spawn {e}");
                    stage_ok = false;
                    failed = Some(format!("{stage}/{action}: {e}"));
                    break;
                }
            }
        }
        if !stage_ok {
            break;
        }
    }

    println!();
    if let Some(f) = &failed {
        println!("■ STOPPED: {f}");
        println!("  completed={completed} reference-skipped={skipped_reference}");
        std::process::exit(1);
    }
    println!("✓ run complete — completed={completed} reference-skipped={skipped_reference}");

    // final status
    let state2 = load_fsm_state(paper_id)?;
    println!();
    println!("final: stage={}", state2.current_stage());
    if let Some((next, required)) = state2.next_stage(&tc) {
        println!("next:  {next} ({})", if required { "REQUIRED" } else { "optional" });
    } else {
        println!("next:  pipeline complete");
    }
    Ok(())
}

fn load_fsm_state(paper_id: &str) -> anyhow::Result<ImportFsmState> {
    let mp = std::path::Path::new("data/processed").join(paper_id).join("manifest.yaml");
    if !mp.exists() {
        // no manifest = fresh acquisition, FSM starts at raw
        return Ok(ImportFsmState::from_events("arxiv_pdf_dot", &["raw".to_string()]));
    }
    let m: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(&mp)?,
    )?;
    let profile = m
        .get("grammar_profile")
        .and_then(|v| v.as_str())
        .unwrap_or("arxiv_pdf_dot")
        .to_string();
    let mut events = Vec::new();
    if let Some(seq) = m.get("lifecycle_events").and_then(|v| v.as_sequence()) {
        for e in seq {
            if let Some(stage) = e.get("stage").and_then(|s| s.as_str()) {
                events.push(stage.to_string());
            }
        }
    }
    Ok(ImportFsmState::from_events(&profile, &events))
}

fn collect_pdfs(root: &str, paper_id: &str, out: &mut Vec<String>) {
    use std::fs;
    let Ok(rd) = fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().map(|n| n == paper_id).unwrap_or(false) {
                if let Ok(inner) = fs::read_dir(&p) {
                    for f in inner.flatten() {
                        if f.path().extension().map(|x| x == "pdf").unwrap_or(false) {
                            out.push(f.path().to_string_lossy().to_string());
                        }
                    }
                }
            } else {
                collect_pdfs(&p.to_string_lossy(), paper_id, out);
            }
        }
    }
}


fn stamp_validated_event(paper_id: &str) -> anyhow::Result<()> {
    use std::path::Path;
    let mp = Path::new("data/processed").join(paper_id).join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(&mp)
            .map_err(|e| anyhow::anyhow!("{}: {e}", mp.display()))?,
    )?;
    let already = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("validated")))
        .unwrap_or(false);
    if already {
        return Ok(());
    }
    if let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut()) {
        events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("stage"), serde_yaml::Value::from("validated")),
            (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
            (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-source-run")),
        ])));
    }
    if let Some(l) = m.get_mut("lifecycle") {
        *l = serde_yaml::Value::from("validated");
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
    Ok(())
}
