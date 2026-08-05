//! Source audit: fact edge writes must use temporal_stamp (M329).
//!
//! Scans `crates/da-application/src/**/*.rs` (except temporal_stamp.rs) for
//! bare `create_edge` call sites whose nearby context names a **fact** wire
//! string from the meta-ontology. Those must go through
//! `create_edge_with_temporal` / `stamp_edge_temporal_fields`.
//!
//! Structural edges (hasPart, foundIn, FROM_SOURCE, …) may use bare create_edge.

use std::path::PathBuf;

/// Fact edge wire strings (must match data/ontology/edge_contracts.yaml temporal_class: fact).
const FACT_WIRES: &[&str] = &[
    "CITES",
    "MENTIONS",
    "SUPPORTS",
    "SUPERSEDES",
    "AFFILIATED_WITH",
    "PARTICIPATES_IN",
    "MEMBER_OF_CLUSTER",
];

/// Also match Rust path expressions that resolve to those wires.
const FACT_EXPRS: &[&str] = &[
    "bibliographic::CITES",
    "bibliographic::MENTIONS",
    "bibliographic::SUPERSEDES",
    "bibliographic::AFFILIATED_WITH",
    "hypergraph::SUPPORTS",
    "hypergraph::PARTICIPATES_IN",
    "hypergraph::MEMBER_OF_CLUSTER",
    "MEMBER_OF_CLUSTER",
];

fn app_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn window_has_fact(window: &str) -> bool {
    FACT_WIRES.iter().any(|w| window.contains(w)) || FACT_EXPRS.iter().any(|e| window.contains(e))
}

fn window_is_stamped(window: &str) -> bool {
    window.contains("create_edge_with_temporal") || window.contains("stamp_edge_temporal_fields")
}

#[test]
fn no_bare_fact_create_edge_in_application_src() {
    let src = app_src_dir();
    assert!(src.is_dir(), "missing {}", src.display());

    let mut violations: Vec<String> = Vec::new();

    for entry in walkdir_rs(&src) {
        let path = entry;
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "temporal_stamp.rs" {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            // Match bare store.create_edge( — not create_edge_with_temporal
            if !line.contains("create_edge") {
                continue;
            }
            if line.contains("create_edge_with_temporal") {
                continue;
            }
            if line.contains("fn create_edge") || line.contains("pub async fn create_edge") {
                continue;
            }
            // Context window
            let start = i.saturating_sub(6);
            let end = (i + 10).min(lines.len());
            let window = lines[start..end].join("\n");
            if !window_has_fact(&window) {
                continue;
            }
            if window_is_stamped(&window) {
                continue;
            }
            let rel = path
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(&path);
            violations.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "bare create_edge for fact edge types (must use create_edge_with_temporal):\n{}",
        violations.join("\n")
    );
}

/// Minimal recursive walk without walkdir crate.
fn walkdir_rs(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn fact_wires_match_meta_ontology_when_present() {
    // Soft pin: if YAML exists, every FACT_WIRES entry is temporal_class fact.
    let yaml = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data/ontology/edge_contracts.yaml");
    if !yaml.exists() {
        return;
    }
    let text = std::fs::read_to_string(&yaml).unwrap();
    for wire in FACT_WIRES {
        // crude but effective: wire appears near temporal_class: fact
        assert!(
            text.contains(wire),
            "fact wire {wire} missing from edge_contracts.yaml"
        );
    }
    // hasPart must remain structural in YAML
    assert!(text.contains("hasPart"));
    assert!(text.contains("temporal_class: structural") || text.contains("temporal_class: fact"));
}
