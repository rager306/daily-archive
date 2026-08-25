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
    let cfg = da_ports::path_config::PathConfig::load();
    let domains = resolve_domain(paper_id, &cfg);
    println!("object:   {paper_id}");
    println!("domains:  {domains:?}");
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
    let state = load_fsm_state(paper_id)?;
    print_status(paper_id, &tc, &state);
    Ok(())
}

pub fn run_plan(paper_id: &str) -> anyhow::Result<()> {
    let tc = load_toolchain()?;
    let state = load_fsm_state(paper_id)?;
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
        .map_err(|e| anyhow::anyhow!("furniture_detectors: {e}"))?;
    let type_detectors_yaml = std::fs::read_to_string("data/ontology/structural_type_detectors.yaml")
        .map_err(|e| anyhow::anyhow!("structural_type_detectors: {e}"))?;
    let policy_yaml = std::fs::read_to_string("data/ontology/navigation_quality_policy.yaml")
        .map_err(|e| anyhow::anyhow!("navigation_quality_policy: {e}"))?;

    let cfg = PathConfig::load();
    let store = FsSchemaObjectStore::from_path_config(&cfg, "article")?;
    let dir = std::path::Path::new("data/processed").join(paper_id);
    let mut cross_layer = da_domain::navigation::CrossLayerInputs::default();

    if let Ok(raw) = std::fs::read_to_string(dir.join("parsed/article.json"))
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(secs) = parsed.get("sections").and_then(|s| s.as_array())
    {
        for (i, s) in secs.iter().enumerate() {
            let text = s.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            cross_layer.section_texts.insert(i, text);
        }
        for t in parsed.get("tables").and_then(|t| t.as_array()).into_iter().flatten() {
            let pid = t.get("section_path_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let id = t.get("table_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            cross_layer.tables_by_path.entry(pid).or_default().push(id);
        }
        for f in parsed.get("formulas").and_then(|f| f.as_array()).into_iter().flatten() {
            let pid = f.get("section_path_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let id = f.get("formula_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            cross_layer.equations_by_path.entry(pid).or_default().push(id);
        }
        for c in parsed.get("citations").and_then(|c| c.as_array()).into_iter().flatten() {
            for anc in c.get("anchors").and_then(|a| a.as_array()).into_iter().flatten() {
                let spid = anc.get("section_path_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                *cross_layer.citation_anchors_by_path.entry(spid).or_insert(0) += 1;
            }
        }
    }

    if let Ok(figs_raw) = std::fs::read_to_string(dir.join("figures/index.json"))
        && let Ok(figs) = serde_json::from_str::<serde_json::Value>(&figs_raw)
    {
        for f in figs.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let page = f.get("page").and_then(|x| x.as_u64()).unwrap_or(0).to_string();
            let fid = f.get("figure_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            cross_layer.figures_by_path.entry(page).or_default().push(fid);
        }
    }

    let out = build_navigation_stage(
        &store, paper_id, &detectors_yaml, &type_detectors_yaml,
        &policy_yaml, &cross_layer,
    )?;
    std::fs::write(dir.join("context/navigation.json"), &out.navigation_json)?;

    // Manifest patch
    let mp = dir.join("manifest.yaml");
    let mut m: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&mp).map_err(|e| anyhow::anyhow!("{}: {e}", mp.display()))?)?;
    let reached = m
        .get("lifecycle_events")
        .and_then(|e| e.as_sequence())
        .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("structured")))
        .unwrap_or(false);
    if !reached {
        if let Some(events) = m.get_mut("lifecycle_events").and_then(|v| v.as_sequence_mut()) {
            events.push(serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                (serde_yaml::Value::from("stage"), serde_yaml::Value::from("structured")),
                (serde_yaml::Value::from("recorded_at"), serde_yaml::Value::from(chrono_now())),
                (serde_yaml::Value::from("actor"), serde_yaml::Value::from("rust-navigation-stage")),
            ])));
        }
        if let Some(l) = m.get_mut("lifecycle") {
            *l = serde_yaml::Value::from("structured");
        }
    }

    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        let nav_entry = serde_yaml::Mapping::from_iter([
            (serde_yaml::Value::from("file"), serde_yaml::Value::from("context/navigation.json")),
            ("actor".into(), serde_yaml::Value::from("rust-navigation-stage")),
            ("stats".into(), serde_yaml::to_value(&out.stats).unwrap_or_default()),
        ]);
        if let Some(ctx) = arts.get_mut("context").and_then(|c| c.as_mapping_mut()) {
            ctx.insert(serde_yaml::Value::from("navigation"), serde_yaml::Value::Mapping(nav_entry));
        } else {
            arts.insert(serde_yaml::Value::from("context"), serde_yaml::Value::Mapping(
                serde_yaml::Mapping::from_iter([
                    ("navigation".into(), serde_yaml::Value::Mapping(nav_entry)),
                ])));
        }
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
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
        .map_err(|e| anyhow::anyhow!("work cache {} (run `da source fetch-openalex {paper_id}`): {e}", obs_path.display()))?;

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
    for m in pol.get("models").and_then(|m| m.as_sequence()).into_iter().flatten() {
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

/// `da source audit-fidelity <paper_id>` — dogfood-гейт качества парсинга
/// книги: сверка parsed-секций с реконструкцией оригинала (obs pages_text,
/// body-зона уже вычищена экспортёром). Проверяет: якоря заголовков на
/// TOC-страницах, возврат граничных хвостов, покрытие слов по секциям,
/// прикреплённость маргиналий. Порог: покрытие ≥98%, потерянных границ = 0.
pub fn run_audit_fidelity(paper_id: &str) -> anyhow::Result<()> {
    use da_domain::pdf_toc::find_heading_anchor;

    let obs_path = std::path::Path::new("artifacts/pipeline_cache/pdf_toc_observations")
        .join(format!("{paper_id}.json"));
    let obs: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&obs_path)?)
        .map_err(|e| anyhow::anyhow!("observations {}: {e}", obs_path.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        std::path::Path::new("data/processed").join(paper_id).join("parsed/article.json"),
    )?)?;

    let toc = obs.get("toc").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let pages: Vec<(u32, String)> = obs
        .get("pages_text")
        .and_then(|p| p.as_object())
        .map(|m| {
            let mut v: Vec<(u32, String)> = m
                .iter()
                .filter_map(|(k, val)| Some((k.parse().ok()?, val.as_str()?.to_string())))
                .collect();
            v.sort_by_key(|(p, _)| *p);
            v
        })
        .unwrap_or_default();
    let page_text = |p: u32| -> String {
        pages.iter().find(|(n, _)| *n == p).map(|(_, t)| t.clone()).unwrap_or_default()
    };
    let sections = parsed.get("sections").and_then(|s| s.as_array()).cloned().unwrap_or_default();

    let word_set = |s: &str| -> std::collections::HashSet<String> {
        s.split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| w.len() >= 4)
            .map(str::to_lowercase)
            .collect()
    };

    let mut anchor_miss = 0u32;
    let mut tail_lost = 0u32;
    let mut tail_total = 0u32;
    let mut cov_sum = 0.0f64;
    let mut cov_n = 0u32;
    let mut worst: Vec<(f64, String)> = Vec::new();
    let mut margin_missing = 0u32;
    let margins = obs.get("pages_margin_notes").and_then(|m| m.as_object());

    for (i, t) in toc.iter().enumerate() {
        let level = t.get("level").and_then(|l| l.as_u64()).unwrap_or(9) as u8;
        if level < 2 {
            continue;
        }
        let title = t.get("title").and_then(|x| x.as_str()).unwrap_or("");
        let page = t.get("page").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let own = page_text(page);
        if find_heading_anchor(&own, title).is_none() {
            anchor_miss += 1;
        }

        let next = toc.get(i + 1);
        let next_page = next.map(|n| n.get("page").and_then(|x| x.as_u64()).unwrap_or(0) as u32);
        // Expected = семантика anchor-splitting:
        //   [свой якорь .. конец своей страницы] + промежуточные +
        //   [начало общей страницы .. якорь следующей]
        let own_off = find_heading_anchor(&own, title);
        let mut expected = match own_off {
            Some((o, _)) => own[o..].to_string(),
            None => own.clone(),
        };
        if let (Some(np), Some(nt)) = (next_page, next.and_then(|n| n.get("title").and_then(|x| x.as_str())))
            && np > page
        {
            for p in (page + 1)..np {
                expected.push('\n');
                expected.push_str(&page_text(p));
            }
            let shared = page_text(np);
            if let Some((off, _)) = find_heading_anchor(&shared, nt) {
                expected.push('\n');
                expected.push_str(&shared[..off]);
                tail_total += 1;
                let tail_probe: String = shared[..off].trim().chars().take(30).collect();
                if !tail_probe.is_empty() {
                    let in_cur = sections.get(i)
                        .and_then(|s| s.get("text"))
                        .and_then(|x| x.as_str())
                        .map(|st| st.contains(&tail_probe))
                        .unwrap_or(false);
                    let in_next = sections.get(i + 1)
                        .and_then(|s| s.get("text"))
                        .and_then(|x| x.as_str())
                        .map(|st| st.contains(&tail_probe))
                        .unwrap_or(false);
                    if !in_cur && !in_next {
                        tail_lost += 1;
                    }
                }
            }
        }
        let ew = word_set(&expected);
        if ew.len() >= 20
            && let Some(sec) = sections.get(i)
        {
            let st = sec.get("text").and_then(|x| x.as_str()).unwrap_or("");
            let miss = ew.difference(&word_set(st)).count();
            let cov = 1.0 - miss as f64 / ew.len() as f64;
            cov_sum += cov;
            cov_n += 1;
            if cov < 0.90 {
                worst.push((cov, title.chars().take(40).collect()));
            }
        }
        if let Some(m) = margins {
            let np = next_page.unwrap_or(page + 1);
            let has_margin = (page..np).any(|p| {
                m.get(&p.to_string())
                    .and_then(|v| v.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
            });
            if has_margin
                && sections
                    .get(i)
                    .and_then(|s| s.get("margin_notes"))
                    .and_then(|x| x.as_str())
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            {
                margin_missing += 1;
            }
        }
    }

    worst.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let avg_cov = if cov_n > 0 { cov_sum / cov_n as f64 } else { 1.0 };
    println!("== audit-fidelity: {paper_id} ==");
    println!("  anchor misses:   {anchor_miss}");
    println!("  boundary tails:  {}/{} recovered (lost {tail_lost})", tail_total - tail_lost, tail_total);
    println!("  word coverage:   avg {:.1}% ({cov_n} sections)", avg_cov * 100.0);
    println!("  margin attached: {margin_missing} sections missing");
    if !worst.is_empty() {
        println!("  worst coverage:");
        for (c, t) in worst.iter().take(5) {
            println!("    {:.0}% - {t}", c * 100.0);
        }
    }
    let pass = anchor_miss == 0 && tail_lost == 0 && avg_cov >= 0.98 && margin_missing == 0;
    println!("  verdict:         {}", if pass { "PASS" } else { "NEEDS-ATTENTION" });
    if !pass {
        std::process::exit(1);
    }
    Ok(())
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
        // D204: structural profile for corpus parsing — consolidation flags
        // off, coordinates on. Rich mode requires GROBID outbound to Crossref
        // which is not guaranteed in the corpus environment (2512.00107
        // regression: rich profile hung 120s until client timeout when the
        // container had no crossref route). Bibliography enrichment runs via
        // the separately cached OpenAlex/Crossref enrich stage from the host.
        let parser = GrobidParser::from_env().with_profile(da_adapters::GrobidProfile::structural());
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

/// Exact-quote gate (ADR-069 §2.1): the entity span must resolve inside the
/// canonical text — the surface equals the slice at [char_start..char_end],
/// or at minimum the label occurs in the canonical text. Verified entities
/// pass the mechanical gate; unverified keep reduced trust for the critic.
fn verify_evidence_span(
    e: &da_ports::extractor::ExtractedEntity,
    canonical: &str,
) -> (&'static str, String) {
    if canonical.is_empty() {
        return ("unverified_no_canonical", String::new());
    }
    let start = e.char_start.min(canonical.len());
    let end = e.char_end.min(canonical.len());
    if start <= end && canonical.is_char_boundary(start) && canonical.is_char_boundary(end) {
        let slice = &canonical[start..end];
        if slice == e.surface {
            return ("verified", e.surface.clone());
        }
    }
    // fallback: label occurrence anywhere in canonical text
    if !e.label.is_empty()
        && canonical.to_lowercase().contains(&e.label.to_lowercase())
    {
        return (
            "verified_label_occurrence",
            canonical
                .get(..e.label.len())
                .filter(|s| s.eq_ignore_ascii_case(&e.label))
                .unwrap_or("")
                .to_string(),
        );
    }
    ("unverified_span_mismatch", e.surface.clone())
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
    // Table-cells as extraction surface (ADR-071): dataset/method names often
    // live ONLY in table cells (specimen 2512.00107 Table 2: senate-bills (SB),
    // email-eu (EEU) — 11 hypergraph datasets absent from body whitelist).
    // Header row containing "Dataset" promotes the pseudo-section so the
    // Dataset section arm + acronym gate type entities correctly.
    if let Some(tables) = parsed.get("tables").and_then(|t| t.as_array()) {
        for (ti, tab) in tables.iter().enumerate() {
            let Some(cells) = tab.get("cells").and_then(|c| c.as_array()) else { continue };
            let header: Vec<String> = cells
                .iter()
                .filter(|c| c.get("row").and_then(|r| r.as_u64()) == Some(0))
                .map(|c| c.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
                .collect();
            let is_dataset_table = header
                .iter()
                .any(|h| h.to_lowercase().contains("dataset"));
            let title = if is_dataset_table {
                format!("Datasets (Table {})", ti + 1)
            } else {
                format!("Table {}", ti + 1)
            };
            let body: Vec<String> = cells
                .iter()
                .map(|c| c.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
                .filter(|t| !t.trim().is_empty())
                .collect();
            if !body.is_empty() {
                pairs.push((title, body.join(" . ")));
            }
        }
    }

    let rt = tokio::runtime::Runtime::new()?;
    let entities = rt.block_on(async {
        use da_ports::Extractor as _;
        let ex = da_adapters::RuleBasedExtractor::new();
        ex.extract(&pairs).await
            .map_err(|e| anyhow::anyhow!("extraction: {e}"))
    })?;

    // Cross-layer (2608.20845 lesson): GROBID text may lose acronym
    // definitions ("(ISC)" absent from parsed sections), while the grounded
    // agent summary keeps them ("Proposes ingest-time semantic compilation
    // (ISC) as a paradigm"). Extract structural entities from the summary
    // layer when present and merge (dedup by lowercase label).
    let summary_path = std::path::Path::new("data/processed")
        .join(paper_id)
        .join("summaries/abstract.json");
    let mut merged: Vec<da_ports::extractor::ExtractedEntity> = entities;
    let mut summary_canonical: Option<String> = None;
    if let Ok(sraw) = std::fs::read_to_string(&summary_path)
        && let Ok(summary) = serde_json::from_str::<serde_json::Value>(&sraw)
    {
        let mut summary_text = String::new();
        for key in ["tldr", "contributions", "methods", "results"] {
            match summary.get(key) {
                Some(serde_json::Value::String(s)) => {
                    summary_text.push_str(s);
                    summary_text.push(' ');
                }
                Some(serde_json::Value::Array(items)) => {
                    for it in items {
                        if let Some(s) = it.as_str() {
                            summary_text.push_str(s);
                            summary_text.push(' ');
                        }
                    }
                }
                _ => {}
            }
        }
        if !summary_text.is_empty() {
            summary_canonical = Some(format!("Summary {summary_text}"));
            let summary_pairs = vec![("Summary".to_string(), summary_text)];
            let summary_entities = rt.block_on(async {
                use da_ports::Extractor as _;
                let ex = da_adapters::RuleBasedExtractor::new();
                ex.extract(&summary_pairs).await
                    .map_err(|e| anyhow::anyhow!("summary extraction: {e}"))
            })?;
            let mut seen: std::collections::HashSet<String> =
                merged.iter().map(|e| e.label.to_lowercase()).collect();
            for e in summary_entities {
                if seen.insert(e.label.to_lowercase()) {
                    merged.push(e);
                }
            }
        }
    }

    // Exact-quote evidence gate (ADR-069 §2.1) + lineage (§2.2):
    // every entity carries verbatim_quote + span, verified by string
    // comparison against the canonical section text BEFORE any LLM critic.
    // content_hash makes recompilation idempotent; pipeline_run_id stamps
    // extractor lineage (ExtractionCacheIdentity fingerprints).
    let extractor = da_adapters::RuleBasedExtractor::new();
    let cache_id = extractor.cache_identity();
    let run_id = format!(
        "{}:{}:{}",
        cache_id.implementation_fingerprint, cache_id.configuration_fingerprint,
        cache_id.output_schema_fingerprint
    );

    let mut records: Vec<serde_json::Value> = Vec::new();
    for e in &merged {
        // canonical text for span verification: global passes scan
        // title+space+text, section passes scan text only; the summary
        // layer scans the concatenated summary text.
        let (canonical, origin) = if e.section_title == "Summary" {
            (summary_canonical.clone().unwrap_or_default(), "summary")
        } else {
            (
                e.section_ordinal
                    .and_then(|i| pairs.get(i).map(|(t, x)| format!("{t} {x}")))
                    .unwrap_or_default(),
                "parsed",
            )
        };
        let (evidence_state, verbatim) = verify_evidence_span(e, &canonical);
        let etype = format!("{:?}", e.entity_type);
        let content_hash = sha256_hex(format!("{paper_id}:{}:{}", e.label, etype).as_bytes());
        records.push(serde_json::json!({
            "label": e.label,
            "entity_type": format!("{:?}", e.entity_type),
            "section_ordinal": e.section_ordinal,
            "evidence": {
                "verbatim_quote": verbatim,
                "char_start": e.char_start,
                "char_end": e.char_end,
                "origin": origin,
                "evidence_state": evidence_state,
            },
            "content_hash": content_hash,
            "pipeline_run_id": run_id,
        }));
    }
    let json = serde_json::to_string_pretty(&records)?;

    // ADR-069: refresh domain MANIFEST.yaml (derived cache) for every
    // domain this paper belongs to — coverage stats + gaps after each
    // processed article.
    let cfg = da_ports::path_config::PathConfig::load();
    let domains = resolve_domain(paper_id, &cfg);
    update_domain_manifests(paper_id, &domains);

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
        // lifecycle = LAST event stage, never a hardcoded stage: a standalone
        // re-run of build-extracted on an already-validated paper must not
        // regress the derived state (derivedstatemismatch regression,
        // caught by validate-objects on 2608.20845/2608.19799).
        let last_stage = m
            .get("lifecycle_events")
            .and_then(|v| v.as_sequence())
            .and_then(|s| s.last())
            .and_then(|e| e.get("stage"))
            .cloned()
            .unwrap_or(serde_yaml::Value::from("extracted"));
        if let Some(l) = m.get_mut("lifecycle") {
            *l = last_stage;
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
    let outcome = run_pipeline_for(paper_id)?;
    if !outcome.success {
        std::process::exit(1);
    }
    Ok(())
}

/// `da source fetch-openalex <pid...|--pending> [--no-purge]` —
/// ADR-073 citracer trio in Rust: purge stale negative cache → batch-fetch
/// pending works (50 DOIs per request, circuit breaker) → write per-pid
/// cache files (slim shape consumed by enrich-layer).
pub fn run_fetch_openalex(args: &[String]) -> anyhow::Result<()> {
    use da_adapters::openalex_adapter::{CircuitBreaker, OpenAlexHttpAdapter};

    let no_purge = args.iter().any(|a| a == "--no-purge");
    let mut ids: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .filter(|a| !matches!(a.as_str(), "pending" | "auto" | "all"))
        .cloned()
        .collect();
    if args.iter().any(|a| matches!(a.as_str(), "--pending" | "pending" | "auto" | "all"))
        || ids.is_empty()
    {
        ids = openalex_pending_ids()?;
    }
    if ids.is_empty() {
        println!("fetch-openalex: cache clean, nothing to do");
        return Ok(());
    }

    let cache_dir = std::path::Path::new("artifacts/pipeline_cache/openalex_works");
    std::fs::create_dir_all(cache_dir)?;

    // шаг 1: purge терминальных негативов (404/422) — устаревают со временем
    let mut purged = 0usize;
    if !no_purge {
        for e in std::fs::read_dir(cache_dir)? {
            let e = e?;
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            let Ok(rec) = serde_json::from_str::<serde_json::Value>(&text) else {
                let _ = std::fs::remove_file(e.path());
                purged += 1;
                continue;
            };
            let status = rec.get("http_status").and_then(|s| s.as_u64()).unwrap_or(0);
            let terminal = rec.get("terminal").and_then(|t| t.as_bool()).unwrap_or(false);
            if terminal && (status == 404 || status == 422) {
                let _ = std::fs::remove_file(e.path());
                purged += 1;
            }
        }
        if purged > 0 {
            println!("purged {purged} stale negative cache entries");
        }
    }

    // шаг 2: batch-fetch pending (после purge список пересчитывается)
    let pending: Vec<String> = ids
        .iter()
        .filter(|pid| openalex_needs_fetch(cache_dir, pid))
        .cloned()
        .collect();
    if pending.is_empty() {
        println!("fetch-openalex: no pending works");
        return Ok(());
    }

    let dois: Vec<String> = pending
        .iter()
        .map(|pid| format!("10.48550/arXiv.{pid}"))
        .collect();

    let adapter = OpenAlexHttpAdapter::default();
    let breaker = CircuitBreaker::default();
    let now = chrono_now();

    let rt = tokio::runtime::Runtime::new()?;
    let works = rt.block_on(adapter.fetch_works_concurrent(&dois, &breaker, 8));
    let mut ok = 0usize;
    let mut breaker_open = false;
    for (pid, w) in pending.iter().zip(works) {
        let rec = match w {
            Some(w) => {
                ok += 1;
                serde_json::json!({"http_status": 200, "fetched_at": now, "work": w})
            }
            None if !breaker_open => serde_json::json!({
                "http_status": 404, "fetched_at": now, "terminal": true, "work": null,
            }),
            None => {
                breaker_open = true;
                serde_json::json!({
                    "http_status": 0, "fetched_at": now, "terminal": false,
                    "error": "circuit_open_or_upstream_error", "work": null,
                })
            }
        };
        std::fs::write(
            cache_dir.join(format!("{pid}.json")),
            serde_json::to_string(&rec)?,
        )?;
    }
    println!(
        "fetch: {ok}/{} works (concurrent path-form, breaker={})",
        pending.len(),
        if breaker_open { "tripped" } else { "ok" }
    );
    Ok(())
}

/// Cache absent, corrupt, or non-terminal failure → needs a fetch.
fn openalex_needs_fetch(cache_dir: &std::path::Path, pid: &str) -> bool {
    let cf = cache_dir.join(format!("{pid}.json"));
    let Ok(text) = std::fs::read_to_string(&cf) else { return true };
    let Ok(rec) = serde_json::from_str::<serde_json::Value>(&text) else { return true };
    let status = rec.get("http_status").and_then(|s| s.as_u64()).unwrap_or(0);
    let terminal = rec.get("terminal").and_then(|t| t.as_bool()).unwrap_or(false);
    status != 200 && !terminal
}

/// All processed article ids (batch targets).
fn openalex_pending_ids() -> anyhow::Result<Vec<String>> {
    let root = std::path::Path::new("data/processed");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root)? {
        let e = e?;
        if e.path().join("manifest.yaml").is_file() {
            out.push(e.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    Ok(out)
}

/// Articles whose pipeline is incomplete (validated missing): batch targets.
pub fn incomplete_pipeline_ids() -> anyhow::Result<Vec<String>> {
    let root = std::path::Path::new("data/processed");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root)? {
        let e = e?;
        let dir = e.path();
        if !dir.join("manifest.yaml").is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(dir.join("manifest.yaml")) else { continue };
        let Ok(m) = serde_yaml::from_str::<serde_yaml::Value>(&text) else { continue };
        let has_validated = m
            .get("lifecycle_events")
            .and_then(|v| v.as_sequence())
            .map(|s| s.iter().any(|e| e.get("stage").and_then(|x| x.as_str()) == Some("validated")))
            .unwrap_or(false);
        if !has_validated {
            out.push(e.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    Ok(out)
}

/// Outcome of one article's pipeline run (for batch aggregation).
pub struct PipelineOutcome {
    pub paper_id: String,
    pub success: bool,
    pub completed: usize,
    pub failure: Option<String>,
}

/// Batch runner: articles in parallel (SMP). Stages within one article stay
/// sequential (FSM order); independent articles map to threads — GROBID and
/// OpenAlex are shared services designed for concurrent load, manifests are
/// per-article files (no cross-article races).
pub fn run_batch(ids: &[String], workers: usize) -> anyhow::Result<()> {
    let workers = workers.clamp(1, std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    println!("═══ batch: {} articles × {} workers ═══", ids.len(), workers);
    let stdout_lock = std::sync::Mutex::new(());
    let outcomes = std::sync::Mutex::new(Vec::<PipelineOutcome>::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(pid) = ids.get(i) else { break };
                let out = match run_pipeline_for(pid) {
                    Ok(o) => o,
                    Err(e) => PipelineOutcome {
                        paper_id: pid.clone(),
                        success: false,
                        completed: 0,
                        failure: Some(e.to_string()),
                    },
                };
                {
                    let _g = stdout_lock.lock().unwrap();
                    println!(
                        "  {} {pid} (completed={})",
                        if out.success { "✓" } else { "✗" },
                        out.completed
                    );
                }
                outcomes.lock().unwrap().push(out);
            });
        }
    });
    let outcomes = outcomes.into_inner().unwrap();
    let ok = outcomes.iter().filter(|o| o.success).count();
    println!();
    println!("batch: {ok}/{} success", outcomes.len());
    for o in outcomes.iter().filter(|o| !o.success) {
        println!("  ✗ {}: {}", o.paper_id, o.failure.as_deref().unwrap_or("?"));
    }
    if ok < outcomes.len() {
        std::process::exit(1);
    }
    Ok(())
}

fn run_pipeline_for(paper_id: &str) -> anyhow::Result<PipelineOutcome> {
// removed dup import

    // load toolchain + FSM state
    let tc_text = std::fs::read_to_string("data/ontology/source_import_tools.yaml")
        .map_err(|e| anyhow::anyhow!("toolchain: {e}"))?;
    let tc = Toolchain::from_yaml_str(&tc_text)
        .map_err(|e| anyhow::anyhow!("toolchain: {e}"))?;
    let state = load_fsm_state(paper_id)?;

    // Domain routing: resolve which domain profile to use
    let cfg = da_ports::path_config::PathConfig::load();
    let domains = resolve_domain(paper_id, &cfg);
    println!("═══ FSM run: {paper_id} ═══");
    println!("domain: {:?} | stage: {} | grammar: {} ({})", domains, state.current_stage(), state.grammar_profile, state.object_class());
    println!();

    let plan = state.plan(&tc);
    if plan.is_empty() {
        println!("✓ [{paper_id}] pipeline complete — nothing to do");
        return Ok(PipelineOutcome {
            paper_id: paper_id.to_string(),
            success: true,
            completed: 0,
            failure: None,
        });
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
    // stage → rust CLI actions: из toolchain-реестра (DATA), не из кода —
    // мета/промпт принцип (ADR-068); ранее карта была захардкожена тут

    let mut completed = 0;
    let mut skipped_reference = 0;
    let mut failed: Option<String> = None;

    for (stage, required) in &plan {
        println!("── {stage} {}", if *required { "(required)" } else { "(optional)" });
        let action_list_owned = tc.rust_actions(stage);
        let Some(action_first) = action_list_owned.first() else {
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

        let _ = action_first;
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

        // pre-stage hook: команда и кэш-путь из toolchain-реестра (DATA),
        // не из кода — ранее путь scripts/m451_enrich_openalex.py был
        // захардкожен (мета/промпт нарушение, ADR-068)
        let pre_hook = tc.stage(stage).and_then(|s| {
            s.tools.iter().find_map(|t| t.pre_hook.as_ref().map(|h| (h, t.retry.as_ref())))
        });
        if let Some((hook, _)) = pre_hook {
            let cache = std::path::Path::new(&hook.cache_dir)
                .join(format!("{paper_id}.json"));
            if !cache.exists() {
                println!("   [pre] work cache missing — running reference fetcher…");
                let st = std::process::Command::new(&hook.command)
                    .args([&hook.script, paper_id])
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
        for action in &action_list_owned {
            let mut cmd = std::process::Command::new(std::env::current_exe()?);
            if *action == "validate-objects" {
                cmd.args(["validate-objects", "--class", "article", paper_id]);
            } else {
                cmd.args(["source", action, paper_id]);
            }
            if let Some(pdf_path) = &pdf {
                cmd.arg("--pdf").arg(pdf_path);
            }
            // retry из toolchain-реестра (DATA): GROBID warm-up network
            // errors, OpenAlex 504 — бэкофф между попытками
            let retry = tc.retry_for(action);
            let mut result = cmd.output();
            let mut attempt = 1u32;
            loop {
                let failed = match &result {
                    Ok(o) => !o.status.success(),
                    Err(_) => true,
                };
                if !failed {
                    break;
                }
                let err_text = match &result {
                    Ok(o) => format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
                    Err(e) => e.to_string(),
                };
                let max_attempts = retry.max_attempts.max(1);
                let retryable = attempt < max_attempts
                    && (retry.retry_on.is_empty()
                        || retry.retry_on.iter().any(|k| err_text.to_lowercase().contains(&k.to_lowercase()))
                        || err_text.contains("network error"));
                if !retryable {
                    break;
                }
                let backoff = retry.backoff_ms as f32 * retry.factor.powi((attempt as i32) - 1);
                println!("   ↻ {action}: attempt {attempt} failed, retry in {backoff:.0}ms");
                std::thread::sleep(std::time::Duration::from_millis(backoff as u64));
                attempt += 1;
                result = cmd.output();
            }
            match result {
                Ok(o) if o.status.success() => {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let line = stdout
                        .lines()
                        .rfind(|l| !l.contains("Logging") && !l.trim().is_empty())
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
                        .rfind(|l| !l.contains("Logging") && !l.contains("INFO"))
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
        println!("■ [{paper_id}] STOPPED: {f}");
        println!("  completed={completed} reference-skipped={skipped_reference}");
        return Ok(PipelineOutcome {
            paper_id: paper_id.to_string(),
            success: false,
            completed,
            failure: failed,
        });
    }
    println!("✓ [{paper_id}] run complete — completed={completed} reference-skipped={skipped_reference}");

    // final status
    let state2 = load_fsm_state(paper_id)?;
    println!();
    println!("final: stage={}", state2.current_stage());
    if let Some((next, required)) = state2.next_stage(&tc) {
        println!("next:  {next} ({})", if required { "REQUIRED" } else { "optional" });
    } else {
        println!("next:  pipeline complete");
    }
    Ok(PipelineOutcome {
        paper_id: paper_id.to_string(),
        success: true,
        completed,
        failure: None,
    })
}

/// Recompute `data/domains/<domain>/MANIFEST.yaml` (ADR-069 derived cache):
/// papers_processed = processed papers routing to this domain that have
/// an extracted/entities.json; entity_coverage = per-type entity counts;
/// gaps = SCHEMA entity_types with zero coverage.
fn update_domain_manifests(paper_id: &str, domains: &[String]) {
    let processed = std::path::Path::new("data/processed");
    let Ok(rd) = std::fs::read_dir(processed) else { return };

    // map paper_id → domain routing for all processed papers with entities
    let cfg = da_ports::path_config::PathConfig::load();
    let mut papers: Vec<(String, String)> = Vec::new(); // (paper, primary domain)
    let mut ent_counts: std::collections::HashMap<String, Vec<serde_json::Value>> =
        std::collections::HashMap::new();
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        if !e.path().join("extracted/entities.json").is_file() {
            continue;
        }
        let primary = resolve_domain(&id, &cfg).first().cloned().unwrap_or_default();
        if primary.is_empty() {
            continue;
        }
        papers.push((id.clone(), primary.clone()));
        if let Ok(raw) = std::fs::read_to_string(e.path().join("extracted/entities.json"))
            && let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&raw)
        {
            ent_counts.insert(id, arr);
        }
    }

    for domain in domains {
        let dom_dir = std::path::Path::new("data/domains").join(domain);
        if !dom_dir.is_dir() {
            continue;
        }
        let mut processed_count = 0usize;
        let mut coverage: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        for (id, primary) in &papers {
            if primary != domain {
                continue;
            }
            processed_count += 1;
            if let Some(arr) = ent_counts.get(id) {
                for e in arr {
                    if let Some(t) = e.get("entity_type").and_then(|t| t.as_str()) {
                        *coverage.entry(t.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        // SCHEMA entity_types with zero coverage → gaps
        let mut gaps: Vec<String> = Vec::new();
        if let Ok(schema_raw) = std::fs::read_to_string(dom_dir.join("SCHEMA.yaml"))
            && let Ok(schema) = serde_yaml::from_str::<serde_yaml::Value>(&schema_raw)
            && let Some(types) = schema.get("entity_types").and_then(|t| t.as_sequence())
        {
            for t in types {
                if let Some(tid) = t.get("id").and_then(|i| i.as_str())
                    && !coverage.contains_key(tid)
                {
                    gaps.push(format!(
                        "entity_type '{tid}' has 0 coverage across {processed_count} processed papers"
                    ));
                }
            }
        }

        let manifest = serde_yaml::Mapping::from_iter([
            ("schema_version".into(), "domain-manifest.v1".into()),
            ("domain".into(), domain.as_str().into()),
            ("generated_at".into(), chrono_now().into()),
            ("last_paper".into(), paper_id.into()),
            ("papers_processed".into(), (processed_count as u64).into()),
            ("entity_coverage".into(), serde_yaml::to_value(&coverage).unwrap_or_default()),
            ("gaps".into(), serde_yaml::to_value(&gaps).unwrap_or_default()),
            ("note".into(), "derived cache — recomputed on every build-extracted".into()),
        ]);
        let _ = std::fs::write(
            dom_dir.join("MANIFEST.yaml"),
            serde_yaml::to_string(&serde_yaml::Value::Mapping(manifest)).unwrap_or_default(),
        );
    }
}

/// Resolve the domain for a paper from its identity.json coarse_topic_code
/// via the domain_routing table in path_config.yaml.
fn resolve_domain(paper_id: &str, cfg: &da_ports::path_config::PathConfig) -> Vec<String> {
    /// Walk sources recursively; on the paper's dir read identity.json or
    /// article.json for coarse_topic_code and route via the config table.
    fn find_topic_code(dir: &std::path::Path, paper_id: &str) -> Option<String> {
        let Ok(rd) = std::fs::read_dir(dir) else { return None };
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            if p.file_name().is_some_and(|n| n == paper_id) {
                let ident = if p.join("identity.json").exists() {
                    p.join("identity.json")
                } else {
                    p.join("article.json")
                };
                if let Ok(text) = std::fs::read_to_string(&ident)
                    && let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&text)
                    && let Some(code) = v.get("coarse_topic_code").and_then(|c| c.as_str())
                {
                    return Some(code.to_string());
                }
            }
            if let Some(code) = find_topic_code(&p, paper_id) {
                return Some(code);
            }
        }
        None
    }

    match find_topic_code(std::path::Path::new("data/sources"), paper_id) {
        Some(code) => cfg.domains_for_topic(&code).unwrap_or_else(|| {
            // unrecognized code: full-text domain detection
            detect_domain_from_text(paper_id)
        }),
        None => detect_domain_from_text(paper_id),
    }
}

/// Detect domain from paper content when routing table has no match.
/// Primary signal: structural type distribution (math paper → theorem/proof).
fn detect_domain_from_text(paper_id: &str) -> Vec<String> {
    let nav_path = std::path::Path::new("data/processed")
        .join(paper_id)
        .join("context/navigation.json");
    if let Ok(raw) = std::fs::read_to_string(&nav_path)
        && let Ok(nav) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(arr) = nav.as_array()
    {
        let math_count = arr
            .iter()
            .filter(|n| {
                let t = n.get("title").and_then(|t| t.as_str()).unwrap_or("");
                t.starts_with("Theorem")
                    || t.starts_with("Definition")
                    || t.starts_with("Lemma")
                    || t.starts_with("Proof")
                    || t.starts_with("Exercise")
            })
            .count();
        if math_count > arr.len() / 10 {
            return vec!["mathematics".into(), "computer_science".into()];
        }
    }
    vec!["computer_science".to_string()]
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
                // PDF may sit at <id>/*.pdf or the canonical <id>/source/<id>.pdf
                // (article_catalog layout). Walk the paper dir recursively.
                collect_pdfs_recursive(&p, out);
            } else {
                collect_pdfs(&p.to_string_lossy(), paper_id, out);
            }
        }
    }
}

/// Collect all *.pdf under `dir` (depth-first, deterministic order).
fn collect_pdfs_recursive(dir: &std::path::Path, out: &mut Vec<String>) {
    use std::fs;
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect_pdfs_recursive(&p, out);
        } else if p.extension().is_some_and(|x| x == "pdf") {
            out.push(p.to_string_lossy().to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// load_fsm_state reads relative paths (data/processed) — serialize
    /// cwd-dependent tests so parallel tests don't observe our temp cwd.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    // Regression (caught live 2026-08-24 on 2608.20845): run_status used
    // the old load_state which propagated ENOENT for fresh acquisitions
    // (no manifest.yaml yet) → "da source: No such file or directory".
    // Fresh acquisition must yield FSM at raw stage, not an error.
    #[test]
    fn fresh_acquisition_without_manifest_starts_at_raw() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let _restore = CwdGuard::new(&tmp);
        let state = load_fsm_state("9999.99999").expect("fresh paper must not error");
        assert_eq!(state.current_stage(), "raw");
        assert_eq!(state.object_class(), "article");
    }

    #[test]
    fn manifest_lifecycle_events_replayed() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("data/processed/1234.56789");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.yaml"),
            "grammar_profile: arxiv_pdf_dot\nlifecycle_events:\n  - stage: raw\n  - stage: parsed\n  - stage: structured\n",
        )
        .unwrap();
        let _restore = CwdGuard::new(&tmp);
        let state = load_fsm_state("1234.56789").unwrap();
        assert_eq!(state.current_stage(), "structured");
    }

    // Regression (caught live 2026-08-24 on 2608.20845): canonical
    // article_catalog layout puts the PDF at <id>/source/<id>.pdf, one
    // level below the paper dir. collect_pdfs only scanned direct
    // children → "no source PDF found" despite the PDF existing.
    // ADR-069 §2.1 exact-quote gate: spans must verify against canonical
    // text; unverifiable entities keep reduced trust (never silently dropped).
    #[test]
    fn evidence_gate_verifies_exact_span() {
        use da_ports::extractor::ExtractedEntity;
        use da_domain::EntityType;
        let e = ExtractedEntity {
            label: "ISC".into(),
            entity_type: EntityType::Method,
            section_title: "Summary".into(),
            section_ordinal: Some(0),
            char_start: 42,
            char_end: 45,
            surface: "ISC".into(),
        };
        let canonical = "Summary Ingest-time semantic compilation (ISC) compiles";
        let (state, quote) = verify_evidence_span(&e, canonical);
        assert_eq!(state, "verified");
        assert_eq!(quote, "ISC");
    }

    #[test]
    fn evidence_gate_flags_mismatch_as_unverified() {
        use da_ports::extractor::ExtractedEntity;
        use da_domain::EntityType;
        let e = ExtractedEntity {
            label: "GhostMethod".into(),
            entity_type: EntityType::Method,
            section_title: "S".into(),
            section_ordinal: Some(0),
            char_start: 0,
            char_end: 11,
            surface: "GhostMethod".into(),
        };
        let (state, _) = verify_evidence_span(&e, "totally different text here");
        assert_eq!(state, "unverified_span_mismatch");
    }

    #[test]
    fn evidence_gate_empty_canonical_is_unverified() {
        use da_ports::extractor::ExtractedEntity;
        use da_domain::EntityType;
        let e = ExtractedEntity {
            label: "X".into(),
            entity_type: EntityType::Task,
            section_title: "t".into(),
            section_ordinal: None,
            char_start: 0,
            char_end: 1,
            surface: "X".into(),
        };
        let (state, _) = verify_evidence_span(&e, "");
        assert_eq!(state, "unverified_no_canonical");
    }

    #[test]
    fn collect_pdfs_finds_pdf_in_source_subdir() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paper_dir = tmp.path().join("data/sources/arxiv/cs-ai/2608.99999");
        std::fs::create_dir_all(paper_dir.join("source")).unwrap();
        std::fs::write(paper_dir.join("source/2608.99999.pdf"), b"%PDF-fake").unwrap();
        let _restore = CwdGuard::new(&tmp);
        let mut out = Vec::new();
        collect_pdfs("data/sources", "2608.99999", &mut out);
        assert_eq!(out.len(), 1, "must find nested source/ pdf: {out:?}");
        assert!(out[0].ends_with("source/2608.99999.pdf"));
    }

    #[test]
    fn collect_pdfs_finds_toplevel_pdf() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paper_dir = tmp.path().join("data/sources/arxiv/cs-ai/2608.88888");
        std::fs::create_dir_all(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("2608.88888.pdf"), b"%PDF-fake").unwrap();
        let _restore = CwdGuard::new(&tmp);
        let mut out = Vec::new();
        collect_pdfs("data/sources", "2608.88888", &mut out);
        assert_eq!(out.len(), 1, "legacy top-level pdf must still be found: {out:?}");
    }

    /// Restores original cwd on drop.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn new(tmp: &tempfile::TempDir) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(&tmp).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).unwrap();
        }
    }
}
