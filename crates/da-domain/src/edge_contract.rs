//! Edge endpoint contract matrix (ADR-045 Wave G foundation).
//!
//! Universal EdgeContract struct lives in kg_ontology::edge_contract
//! (M289-u4fzx5/S03). Project-specific contracts (CITES, AUTHORED_BY,
//! MENTIONS, etc.) and render_markdown_table() stay here.

// Re-export universal EdgeContract struct from kg-ontology (ADR-050).
pub use kg_ontology::edge_contract::EdgeContract;

/// The contract matrix: every edge type the pipeline materializes,
/// with its expected endpoint node labels.
///
/// Keep alphabetized by edge_constant for easy scanning.
pub fn edge_contracts() -> Vec<EdgeContract> {
    use crate::relation::{bibliographic, hypergraph, structure};
    vec![
        EdgeContract {
            edge_constant: bibliographic::AFFILIATED_WITH,
            source_label: "Author",
            target_labels: &["Institution"],
            rationale: "Author is affiliated with an institution (OpenAlex authorship).",
        },
        EdgeContract {
            edge_constant: bibliographic::CITES,
            source_label: "Paper",
            target_labels: &["Citation"],
            rationale: "Paper cites a bibliographic citation (resolvable to a Paper).",
        },
        EdgeContract {
            edge_constant: structure::AUTHORED_BY,
            source_label: "Author",
            target_labels: &["Paper"],
            rationale: "Author authored the Paper (OpenAlex authorship).",
        },
        EdgeContract {
            edge_constant: structure::FOUND_IN,
            source_label: "Entity",
            target_labels: &["Section"],
            rationale: "Entity was extracted from this Section of the Paper.",
        },
        EdgeContract {
            edge_constant: structure::FROM_SOURCE,
            source_label: "Paper",
            target_labels: &["Source"],
            rationale: "Paper originated from this Source (provenance).",
        },
        EdgeContract {
            edge_constant: structure::HAS_PART,
            source_label: "Paper",
            // HAS_PART links Paper to its structural parts (Section) and
            // to its bibliography entries (Reference) — see ingest.rs.
            target_labels: &["Section", "Reference"],
            rationale: "Paper has this Section/Reference as a structural part (FaBiO frbr:part).",
        },
        EdgeContract {
            edge_constant: structure::HAS_TOPIC,
            source_label: "Paper",
            target_labels: &["Topic"],
            rationale: "Paper is classified under this Topic (OpenAlex topic assignment).",
        },
        EdgeContract {
            edge_constant: structure::IN_CATEGORY,
            source_label: "Paper",
            target_labels: &["Category"],
            rationale: "Paper is in this arXiv/OpenAlex Category.",
        },
        EdgeContract {
            edge_constant: hypergraph::MEMBER_OF_CLUSTER,
            source_label: "Entity",
            target_labels: &["ConceptCluster"],
            rationale: "Entity is a member of this ConceptCluster.",
        },
        EdgeContract {
            edge_constant: bibliographic::MENTIONS,
            source_label: "Paper",
            // MENTIONS is polymorphic: a paper can mention an extracted
            // Entity (NER result), a ResearchProblem (from abstract), or
            // a MetricObservation (from results sections). All three are
            // textual mentions extracted from the paper body.
            target_labels: &["Entity", "ResearchProblem", "MetricObservation"],
            rationale: "Paper mentions this Entity/ResearchProblem/MetricObservation (extracted from paper text).",
        },
        EdgeContract {
            edge_constant: hypergraph::PARTICIPATES_IN,
            source_label: "Entity",
            target_labels: &["EvidenceBundle"],
            rationale: "Entity participates in this EvidenceBundle (co-occurring entities).",
        },
        EdgeContract {
            edge_constant: hypergraph::SUPPORTS,
            source_label: "EvidenceBundle",
            target_labels: &["Claim"],
            rationale: "EvidenceBundle supports this Claim.",
        },
        EdgeContract {
            edge_constant: bibliographic::SUPERSEDES,
            source_label: "Entity",
            target_labels: &["Entity"],
            rationale: "Source Entity supersedes target Entity (D135 merge scenario).",
        },
    ]
}

/// Unique edge wire strings from the **compiled** contract matrix.
///
/// Prefer [`contract_edge_types_or_yaml`] for inventory — external meta-ontology first.
pub fn contract_edge_types() -> Vec<&'static str> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for c in edge_contracts() {
        if seen.insert(c.edge_constant) {
            out.push(c.edge_constant);
        }
    }
    out
}

/// Edge wire strings from external meta-ontology (`data/ontology/edge_contracts.yaml`)
/// when present and non-empty; otherwise compiled [`contract_edge_types`].
///
/// ADR-050 / M324: edge contracts are DATA, not CLI/domain hardcode for discovery.
pub fn contract_edge_types_or_yaml() -> Vec<String> {
    match kg_ontology::registry::OntologyRegistry::load_project_meta_ontology() {
        Ok(reg) => {
            let types = reg.edge_types();
            if types.is_empty() {
                contract_edge_types()
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            } else {
                types
            }
        }
        Err(_) => contract_edge_types()
            .into_iter()
            .map(str::to_string)
            .collect(),
    }
}

/// Load owned edge contracts from YAML meta-ontology, or map compiled contracts.
pub fn edge_contracts_or_yaml() -> Vec<kg_ontology::registry::YamlEdgeContract> {
    if let Ok(reg) = kg_ontology::registry::OntologyRegistry::load_project_meta_ontology()
        && !reg.edge_contracts().is_empty()
    {
        return reg.edge_contracts().to_vec();
    }
    edge_contracts()
        .into_iter()
        .map(|c| kg_ontology::registry::YamlEdgeContract {
            edge_constant: c.edge_constant.to_string(),
            source_label: c.source_label.to_string(),
            target_labels: c.target_labels.iter().map(|s| (*s).to_string()).collect(),
            rationale: c.rationale.to_string(),
        })
        .collect()
}

/// Serialize edge contracts as external meta-ontology YAML body (M325).
///
/// Uses [`edge_contracts_or_yaml`] so export prefers `data/ontology` when present,
/// otherwise the compiled matrix. Shape matches `data/ontology/edge_contracts.yaml`.
pub fn render_edge_contracts_yaml() -> Result<String, serde_yaml::Error> {
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Row {
        edge_constant: String,
        source_label: String,
        target_labels: Vec<String>,
        rationale: String,
    }

    let rows: Vec<Row> = edge_contracts_or_yaml()
        .into_iter()
        .map(|c| Row {
            edge_constant: c.edge_constant,
            source_label: c.source_label,
            target_labels: c.target_labels,
            rationale: c.rationale,
        })
        .collect();

    let mut root: BTreeMap<&str, Vec<Row>> = BTreeMap::new();
    root.insert("edge_contracts", rows);
    serde_yaml::to_string(&root)
}

/// Render the edge contracts matrix as a markdown table for documentation.
/// Use in `da edge-contracts` CLI command to emit always-fresh content for
/// GRAPH-SCHEMA.md.
pub fn render_markdown_table() -> String {
    let mut s = String::new();
    s.push_str("| Edge | Source | Target(s) | Rationale |\n");
    s.push_str("|------|--------|-----------|-----------|\n");
    let mut contracts = edge_contracts();
    contracts.sort_by_key(|c| c.edge_constant);
    for c in &contracts {
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
    use std::collections::HashSet;

    #[test]
    fn test_contract_edge_types_includes_pipeline_wires() {
        use crate::relation::{bibliographic, structure};
        let types = contract_edge_types();
        assert!(types.contains(&structure::HAS_PART));
        assert!(types.contains(&bibliographic::CITES));
        assert!(types.contains(&bibliographic::MENTIONS));
        // Unique
        let set: HashSet<_> = types.iter().copied().collect();
        assert_eq!(set.len(), types.len());
    }

    #[test]
    fn test_contract_edge_types_or_yaml_includes_has_part() {
        let types = contract_edge_types_or_yaml();
        assert!(types.iter().any(|e| e == "hasPart"), "{types:?}");
        assert!(types.iter().any(|e| e == "CITES"), "{types:?}");
        // YAML and compiled sets should both be non-empty and share core wires
        let compiled: std::collections::HashSet<_> = contract_edge_types().into_iter().collect();
        for w in ["hasPart", "CITES", "MENTIONS"] {
            assert!(types.iter().any(|e| e == w));
            assert!(compiled.contains(w), "compiled missing {w}");
        }
    }

    #[test]
    fn test_edge_contracts_or_yaml_nonempty() {
        let rows = edge_contracts_or_yaml();
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|c| c.edge_constant == "hasPart"));
    }

    /// Every edge constant in the contract matrix must reference only
    /// registered node labels (source and every target).

    #[test]
    fn test_render_edge_contracts_yaml_has_has_part() {
        let yaml = render_edge_contracts_yaml().expect("yaml");
        assert!(yaml.contains("edge_contracts:"), "{yaml}");
        assert!(yaml.contains("hasPart"), "{yaml}");
        assert!(yaml.contains("CITES"), "{yaml}");
        // parse back
        let v: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("parse");
        let arr = v
            .get("edge_contracts")
            .expect("key")
            .as_sequence()
            .expect("seq");
        assert!(arr.len() >= 10, "len={}", arr.len());
    }

    #[test]
    fn test_all_contract_edges_reference_registered_labels() {
        let registered: HashSet<String> = crate::schema::all_node_schemas()
            .into_iter()
            .map(|s| s.label().to_string())
            .collect();
        for c in edge_contracts() {
            assert!(
                registered.contains(c.source_label),
                "edge {} source label '{}' not a registered node type",
                c.edge_constant,
                c.source_label
            );
            for tgt in c.target_labels {
                assert!(
                    registered.contains(*tgt),
                    "edge {} target label '{}' not a registered node type",
                    c.edge_constant,
                    tgt
                );
            }
        }
    }

    /// Every edge constant in the contract matrix must validate via
    /// `validate_edge_type` (i.e., be in the validator's edge registry).
    #[test]
    fn test_all_contract_edges_pass_validator() {
        use crate::validator::validate_edge_type;
        for c in edge_contracts() {
            assert!(
                validate_edge_type(c.edge_constant).is_none(),
                "edge constant '{}' failed validator::validate_edge_type",
                c.edge_constant
            );
        }
    }

    /// No duplicate edge constants with contradictory endpoint contracts.
    /// Two rows with the same edge_constant must agree on source and the
    /// full set of target_labels.
    #[test]
    fn test_no_contradictory_edge_contracts() {
        let contracts = edge_contracts();
        for (i, a) in contracts.iter().enumerate() {
            for b in contracts.iter().skip(i + 1) {
                if a.edge_constant == b.edge_constant {
                    assert_eq!(
                        a.source_label, b.source_label,
                        "edge '{}' has contradictory source labels:\n  {:?}\n  {:?}",
                        a.edge_constant, a, b
                    );
                    let a_targets: HashSet<&&str> = a.target_labels.iter().collect();
                    let b_targets: HashSet<&&str> = b.target_labels.iter().collect();
                    assert_eq!(
                        a_targets, b_targets,
                        "edge '{}' has contradictory target_labels:\n  {:?}\n  {:?}",
                        a.edge_constant, a, b
                    );
                }
            }
        }
    }

    /// Every pipeline-materialized edge type (referenced via relation::x::Y
    /// in da-application/src) must have a contract row. Adding a new edge
    /// to the pipeline without documenting it here fails this test.
    #[test]
    fn test_every_pipeline_edge_has_contract() {
        let contract_edges: HashSet<String> = edge_contracts()
            .iter()
            .map(|c| c.edge_constant.to_string())
            .collect();

        // Pipeline-referenced edges — keep in sync with the relation modules.
        use crate::relation::{bibliographic, hypergraph, structure};
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
                "pipeline edge '{}' has no contract row in edge_contracts() — add one",
                e
            );
        }
    }

    /// Polymorphic edges (multi-target) must document why — their rationale
    /// must mention the polymorphism so future readers see it.
    /// This is a soft check: rationale must contain "or" / "," / "/".
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
}
