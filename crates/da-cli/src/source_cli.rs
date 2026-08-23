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
