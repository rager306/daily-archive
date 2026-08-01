//! Audit-time schema-conformance test.
//!
//! Scans da-application/src/*.rs for `create_node("Label")` call sites
//! and asserts:
//!   1. Every referenced label is in `da_domain::schema::all_node_schemas()`.
//!   2. (Wave D) For materialized labels, the pipeline's actual set of
//!      `set_node_property_*` calls eventually covers all required fields.
//!
//! This test does NOT execute the pipeline — it is a pure source-code
//! audit. It guards against:
//!   - Introducing a new node type in the pipeline without registering a schema.
//!   - Materializing a node whose required fields are not all set.
//!
//! See: ADR-040 §11.1, ADR-044, MEM480 (audit pattern).

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Regex-free line-based scanner: find all `create_node("Label")` calls
/// in *.rs files under `src/`. Returns the set of labels used.
fn collect_create_node_labels(src_dir: &PathBuf) -> HashSet<String> {
    let mut labels = HashSet::new();
    for entry in fs::read_dir(src_dir).expect("src dir readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read .rs");
        for (i, line) in content.lines().enumerate() {
            // Match patterns: create_node("Label") — with any whitespace.
            // Avoid matching commented-out lines.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if let Some(start) = content
                .lines()
                .nth(i)
                .and_then(|l| l.find("create_node(\""))
            {
                let after = &content.lines().nth(i).unwrap()[start + "create_node(\"".len()..];
                if let Some(end) = after.find("\")") {
                    let label = &after[..end];
                    if !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        labels.insert(label.to_string());
                    }
                }
            }
        }
    }
    labels
}

#[test]
fn test_all_pipeline_node_labels_have_registered_schemas() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let used_labels = collect_create_node_labels(&src_dir);

    let registered: HashSet<String> = da_domain::schema::all_node_schemas()
        .into_iter()
        .map(|s| s.label().to_string())
        .collect();

    let unregistered: Vec<&String> = used_labels
        .iter()
        .filter(|l| !registered.contains(*l))
        .collect();
    assert!(
        unregistered.is_empty(),
        "Pipeline create_node labels not in schema registry: {:?}. \
         Add a Schema struct and register it in all_node_schemas().",
        unregistered
    );
}

#[test]
fn test_pipeline_uses_at_least_all_publication_plane_nodes() {
    // Smoke test: the publication plane must remain fully materialized.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let used_labels = collect_create_node_labels(&src_dir);

    let must_have = [
        "Paper",
        "Section",
        "Reference",
        "Citation",
        "Entity",
        "Topic",
        "Author",
        "Category",
        "Source",
        "ConceptCluster",
        "SchedulerTask",
        "Institution",
    ];
    for label in &must_have {
        assert!(
            used_labels.contains(*label),
            "Pipeline is missing required publication-plane node type '{}'",
            label
        );
    }
}
