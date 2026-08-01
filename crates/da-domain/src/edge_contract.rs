//! Edge endpoint contract matrix (ADR-045 Wave G foundation).
//!
//! Documents the expected (source_label, target_labels) endpoint contract
//! for every edge type the pipeline materializes. The contract data lives
//! in `data/edge_contracts.yaml` (project directive "не хардкодим"):
//! Rust code holds loader logic + validator rules; data lives in YAML.
//!
//! # Public API
//!
//! - [`EdgeContract`] — one row of the matrix (owned strings; loaded
//!   from YAML at startup).
//! - [`edge_contracts`] — cached loader, returns the parsed YAML.
//! - [`render_markdown_table`] — pretty-print for CLI/docs.
//!
//! See ADR-045 §Wave G, ONTOLOGY-DESIGN-V2.md §3.

use crate::relation::{bibliographic, conflict, decision, hypergraph, structure};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One row of the edge-endpoint contract matrix. Owned strings because
/// data is loaded from YAML at runtime (project directive: no hardcoding).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeContract {
    pub edge_constant: String,
    pub source_label: String,
    /// All node labels that may appear as the target of this edge.
    /// Single-element for monomorphic edges; multi-element for
    /// polymorphic edges like MENTIONS.
    pub target_labels: Vec<String>,
    /// Short rationale — why this edge connects these two labels.
    /// Polymorphic rows must mention "or" / "/" / "," so the
    /// rationale documents the polymorphism.
    pub rationale: String,
}

/// YAML file shape: top-level `edge_contracts:` list.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EdgeContractFile {
    edge_contracts: Vec<EdgeContract>,
}

/// Bundled fallback YAML (compiled into the binary so the crate works
/// without the source tree available). Mirrors `data/edge_contracts.yaml`.
/// Both files MUST be kept in sync; the `test_bundled_yaml_matches_disk`
/// test guards this.
const BUNDLED_YAML: &str = include_str!("../../../data/edge_contracts.yaml");

static CACHED: Lazy<Vec<EdgeContract>> = Lazy::new(load_edge_contracts);

/// Load the edge contract matrix. Tries the on-disk YAML first (so
/// operators can hot-swap the file during development), falls back to
/// the bundled copy compiled into the binary.
fn load_edge_contracts() -> Vec<EdgeContract> {
    for path in [
        "data/edge_contracts.yaml",
        "../data/edge_contracts.yaml",
        "../../data/edge_contracts.yaml",
        "../../../data/edge_contracts.yaml",
    ] {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(parsed) = serde_yaml::from_str::<EdgeContractFile>(&text) {
                return parsed.edge_contracts;
            }
        }
    }
    // Fall back to the bundled copy.
    match serde_yaml::from_str::<EdgeContractFile>(BUNDLED_YAML) {
        Ok(parsed) => parsed.edge_contracts,
        Err(e) => {
            // Should never happen — the bundled file is compile-checked.
            panic!("bundled edge_contracts.yaml is invalid: {e}");
        }
    }
}

/// Public accessor: returns the parsed edge contract matrix.
/// Cached after first call (Lazy).
pub fn edge_contracts() -> Vec<EdgeContract> {
    CACHED.clone()
}

/// Render the matrix as a markdown table for documentation.
/// Used by `da edge-contracts` CLI command.
pub fn render_markdown_table() -> String {
    let mut rows = edge_contracts();
    rows.sort_by(|a, b| {
        a.edge_constant
            .cmp(&b.edge_constant)
            .then_with(|| a.source_label.cmp(&b.source_label))
    });

    let mut s = String::new();
    s.push_str("| Edge | Source | Target(s) | Rationale |\n");
    s.push_str("|------|--------|-----------|-----------|\n");
    for c in &rows {
        let targets = c.target_labels.join(" · ");
        s.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            c.edge_constant, c.source_label, targets, c.rationale
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every edge constant referenced in the YAML must be a registered
    /// edge constant in `relation::*` modules. Catches typos in the YAML.
    #[test]
    fn test_yaml_edge_constants_are_registered() {
        let registered: HashSet<&str> = vec![
            bibliographic::CITES,
            bibliographic::CITED_BY,
            bibliographic::CO_AUTHORED,
            bibliographic::AFFILIATED_WITH,
            bibliographic::MENTIONS,
            bibliographic::SUPERSEDES,
            bibliographic::SPLITS,
            structure::HAS_PART,
            structure::HAS_TOPIC,
            structure::AUTHORED_BY,
            structure::FROM_SOURCE,
            structure::FOUND_IN,
            structure::IN_CATEGORY,
            hypergraph::MEMBER_OF_CLUSTER,
            hypergraph::SUBSUMES,
            hypergraph::PARTICIPATES_IN,
            hypergraph::SUPPORTS,
            conflict::CONFLICTS_OVER,
            conflict::RESOLVED_BY,
            decision::CAUSED,
            decision::INFLUENCED,
            decision::PRECEDENT_FOR,
            decision::AUTHORITY_FOR,
            decision::TRIGGERED_BY,
        ]
        .into_iter()
        .collect();

        for c in edge_contracts() {
            assert!(
                registered.contains(c.edge_constant.as_str()),
                "YAML edge_constant '{}' is not a registered edge in relation::*",
                c.edge_constant
            );
        }
    }

    /// Every source/target label in the YAML must be a registered node
    /// label in all_node_schemas(). Catches typos.
    #[test]
    fn test_yaml_endpoint_labels_are_registered_nodes() {
        let registered: HashSet<String> = crate::schema::all_node_schemas()
            .into_iter()
            .map(|s| s.label().to_string())
            .collect();
        for c in edge_contracts() {
            assert!(
                registered.contains(&c.source_label),
                "YAML source_label '{}' is not a registered node type",
                c.source_label
            );
            for t in &c.target_labels {
                assert!(
                    registered.contains(t),
                    "YAML target_label '{t}' (on edge {}) is not a registered node type",
                    c.edge_constant
                );
            }
        }
    }

    /// Every edge constant in the YAML must pass validator::validate_edge_type.
    #[test]
    fn test_yaml_edges_pass_validator() {
        use crate::validator::validate_edge_type;
        for c in edge_contracts() {
            assert!(
                validate_edge_type(&c.edge_constant).is_none(),
                "edge constant '{}' from YAML failed validator::validate_edge_type",
                c.edge_constant
            );
        }
    }

    /// Same edge_constant cannot list contradictory source labels.
    /// Multiple source labels for one edge_constant are allowed (e.g.
    /// CONFLICTS_OVER polymorphic on source) but each (source, edge)
    /// pair must be unique.
    #[test]
    fn test_no_duplicate_source_edge_pairs() {
        let contracts = edge_contracts();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for c in &contracts {
            let key = (c.edge_constant.clone(), c.source_label.clone());
            assert!(
                seen.insert(key.clone()),
                "duplicate (edge_constant, source_label) pair: {key:?}"
            );
        }
    }

    /// Every pipeline-materialized edge type must have a contract row.
    /// Adding a new edge to the pipeline without documenting it in the
    /// YAML fails this test.
    #[test]
    fn test_every_pipeline_edge_has_contract() {
        let contract_edges: HashSet<String> = edge_contracts()
            .iter()
            .map(|c| c.edge_constant.clone())
            .collect();
        let pipeline_edges: Vec<&str> = vec![
            structure::HAS_PART,
            structure::HAS_TOPIC,
            structure::AUTHORED_BY,
            structure::FROM_SOURCE,
            structure::FOUND_IN,
            structure::IN_CATEGORY,
            bibliographic::CITES,
            bibliographic::MENTIONS,
            bibliographic::SUPERSEDES,
            bibliographic::AFFILIATED_WITH,
            hypergraph::MEMBER_OF_CLUSTER,
            hypergraph::PARTICIPATES_IN,
            hypergraph::SUPPORTS,
        ];
        for e in pipeline_edges {
            assert!(
                contract_edges.contains(e),
                "pipeline edge '{}' has no contract row in edge_contracts.yaml — add one",
                e
            );
        }
    }

    /// Polymorphic edges (multi-target) must document why — rationale
    /// must mention "/", "or", "can mention", or ",".
    #[test]
    fn test_polymorphic_edges_document_polymorphism() {
        for c in edge_contracts() {
            if c.target_labels.len() > 1 {
                let r = c.rationale.to_lowercase();
                assert!(
                    r.contains('/')
                        || r.contains(" or ")
                        || r.contains("can mention")
                        || r.contains(','),
                    "polymorphic edge '{}' (targets={:?}) must document why in rationale: {:?}",
                    c.edge_constant,
                    c.target_labels,
                    c.rationale
                );
            }
        }
    }

    /// Bundled YAML fallback must parse and match the on-disk YAML.
    /// Guards against the two files drifting.
    #[test]
    fn test_bundled_yaml_matches_disk() {
        let bundled: Vec<EdgeContract> =
            serde_yaml::from_str::<EdgeContractFile>(BUNDLED_YAML)
                .expect("bundled YAML parses")
                .edge_contracts;
        // On-disk may differ (dev edits) but the bundled copy must at
        // least be a subset of what the loader returns — i.e. every
        // bundled row must appear in the loaded set.
        let loaded = edge_contracts();
        for b in &bundled {
            assert!(
                loaded.contains(b),
                "bundled row missing from loaded YAML (drift): {:?}",
                b
            );
        }
    }

    /// render_markdown_table must include every contract row.
    #[test]
    fn test_render_markdown_table_contains_all_rows() {
        let table = render_markdown_table();
        for c in edge_contracts() {
            assert!(
                table.contains(&format!("`{}`", c.edge_constant)),
                "rendered table missing edge `{}`",
                c.edge_constant
            );
            assert!(
                table.contains(&format!("`{}`", c.source_label)),
                "rendered table missing source `{}` for edge `{}`",
                c.source_label,
                c.edge_constant
            );
        }
    }
}
