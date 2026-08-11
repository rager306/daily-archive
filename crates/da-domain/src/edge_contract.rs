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
            edge_constant: structure::HAS_PART,
            source_label: "PaperRevision",
            target_labels: &["Section"],
            rationale: "Immutable article revision has this revision-scoped Section (ADR-058).",
        },
        EdgeContract {
            edge_constant: structure::HAS_REVISION,
            source_label: "Paper",
            target_labels: &["PaperRevision"],
            rationale: "Canonical scholarly work has this immutable revision expression (ADR-058).",
        },
        EdgeContract {
            edge_constant: structure::HAS_MANIFESTATION,
            source_label: "PaperRevision",
            target_labels: &["SourceManifestation"],
            rationale: "Article revision is represented by this captured source manifestation (ADR-058).",
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
            // MENTIONS is polymorphic: a paper can mention a generic extracted
            // Entity, a first-class semantic Method/Dataset/Metric, a
            // ResearchProblem, or a MetricObservation. Typed nodes come from
            // the same grounded article context and retain import_eligible=false.
            target_labels: &[
                "Entity",
                "Method",
                "Dataset",
                "Metric",
                "ResearchProblem",
                "MetricObservation",
            ],
            rationale: "Paper mentions this Entity or first-class typed semantic node, grounded by extraction or structured summary evidence.",
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
        EdgeContract {
            edge_constant: bibliographic::SUPERSEDES,
            source_label: "PaperRevision",
            target_labels: &["PaperRevision"],
            rationale: "Newer observed revision supersedes the previous observed article revision (ADR-058).",
        },
        EdgeContract {
            edge_constant: crate::relation::decision::AFFECTS,
            source_label: "Decision",
            target_labels: &["Entity"],
            rationale: "Decision directly affected an Entity (ADR-053 Layer G).",
        },
        EdgeContract {
            edge_constant: crate::relation::decision::CAUSED,
            source_label: "Decision",
            target_labels: &["Decision"],
            rationale: "Decision caused a later decision (ADR-048).",
        },
        EdgeContract {
            edge_constant: crate::relation::decision::INFLUENCED,
            source_label: "Decision",
            target_labels: &["Decision"],
            rationale: "Decision influenced a later decision (ADR-048).",
        },
        EdgeContract {
            edge_constant: crate::relation::decision::PRECEDENT_FOR,
            source_label: "Decision",
            target_labels: &["Decision"],
            rationale: "Decision is precedent for a later decision (ADR-048).",
        },
        EdgeContract {
            edge_constant: crate::relation::decision::TRIGGERED_BY,
            source_label: "Decision",
            target_labels: &["Conflict"],
            rationale: "Decision was triggered by a conflict (ADR-048).",
        },
        // ─── ADR-053 Layer A document tree navigation edges (T03) ───
        EdgeContract {
            edge_constant: structure::NEXT_SIBLING,
            source_label: "Section",
            target_labels: &["Section"],
            rationale: "Section navigation tree (ADR-053 Layer A): next sibling in same parent.",
        },
        EdgeContract {
            edge_constant: structure::FIRST_CHILD,
            source_label: "Section",
            target_labels: &["Section"],
            rationale: "Section navigation tree (ADR-053 Layer A): first child of this section.",
        },
        EdgeContract {
            edge_constant: structure::PARENT_SECTION,
            source_label: "Section",
            target_labels: &["Section"],
            rationale: "Section navigation tree (ADR-053 Layer A): parent section in the document hierarchy.",
        },
        // ─── ADR-053 Layer B: Summary edge (D194 M369 S01) ───
        EdgeContract {
            edge_constant: structure::HAS_SUMMARY,
            source_label: "Paper",
            target_labels: &["Summary"],
            rationale: "Paper has an abstract or paper-level digest (ADR-053 Layer B).",
        },
        EdgeContract {
            edge_constant: structure::HAS_SUMMARY,
            source_label: "Section",
            target_labels: &["Summary"],
            rationale: "Section has a path-identity-linked section digest (ADR-053 Layer B).",
        },
        EdgeContract {
            edge_constant: structure::HAS_QUALITY_SIGNAL,
            source_label: "Paper",
            target_labels: &["QualitySignal"],
            rationale: "Paper has a typed quality signal; this evidence is not a Summary and may be revised.",
        },
        // ─── ADR-053 Layer F: Method lineage (D194 M368 S03) ───
        EdgeContract {
            edge_constant: structure::PREDECESSOR_OF,
            source_label: "Method",
            target_labels: &["Method"],
            rationale: "Method lineage: A predecessor_of B means A builds on B.",
        },
    ]
}

/// Unique edge wire strings from the **compiled** contract matrix.
///
/// Prefer [`contract_edge_types_or_yaml`] for inventory — external meta-ontology first.
pub fn validate_compiled_edge_endpoint(
    edge_type: &str,
    source_label: &str,
    target_label: &str,
) -> Result<(), String> {
    let matches = edge_contracts().into_iter().any(|contract| {
        contract.edge_constant == edge_type
            && contract.source_label == source_label
            && contract.target_labels.contains(&target_label)
    });
    if matches {
        Ok(())
    } else {
        Err(format!(
            "edge endpoint is not registered: {source_label}-[{edge_type}]->{target_label}"
        ))
    }
}

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
        .map(|c| {
            use crate::relation::{bibliographic, hypergraph, structure};
            use kg_ontology::registry::{TemporalClass, YamlEdgeContract};
            // Compiled fallback policy (ADR-046): mirrors data/ontology/edge_contracts.yaml.
            let temporal_class = match c.edge_constant {
                bibliographic::CITES
                | bibliographic::MENTIONS
                | bibliographic::AFFILIATED_WITH
                | bibliographic::SUPERSEDES
                | crate::relation::decision::AFFECTS
                | crate::relation::decision::CAUSED
                | crate::relation::decision::INFLUENCED
                | crate::relation::decision::PRECEDENT_FOR
                | crate::relation::decision::TRIGGERED_BY
                | hypergraph::SUPPORTS
                | hypergraph::PARTICIPATES_IN
                | hypergraph::MEMBER_OF_CLUSTER => TemporalClass::Fact,
                structure::HAS_QUALITY_SIGNAL => TemporalClass::Fact,
                structure::HAS_PART
                | structure::HAS_REVISION
                | structure::HAS_MANIFESTATION
                | structure::FROM_SOURCE
                | structure::AUTHORED_BY
                | structure::FOUND_IN
                | structure::HAS_TOPIC
                | structure::IN_CATEGORY => TemporalClass::Structural,
                _ => TemporalClass::Structural,
            };
            YamlEdgeContract {
                edge_constant: c.edge_constant.to_string(),
                source_label: c.source_label.to_string(),
                target_labels: c.target_labels.iter().map(|s| (*s).to_string()).collect(),
                temporal_class,
                rationale: c.rationale.to_string(),
                implementation_status: kg_ontology::ContractStatus::Active,
            }
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
        temporal_class: String,
        rationale: String,
    }

    let rows: Vec<Row> = edge_contracts_or_yaml()
        .into_iter()
        .map(|c| Row {
            edge_constant: c.edge_constant,
            source_label: c.source_label,
            target_labels: c.target_labels,
            temporal_class: c.temporal_class.as_str().to_string(),
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
    fn compiled_endpoint_validation_supports_polymorphic_wires_and_rejects_drift() {
        assert!(validate_compiled_edge_endpoint("CITES", "Paper", "Citation").is_ok());
        assert!(validate_compiled_edge_endpoint("hasSummary", "Paper", "Summary").is_ok());
        assert!(validate_compiled_edge_endpoint("hasSummary", "Section", "Summary").is_ok());
        assert!(
            validate_compiled_edge_endpoint("hasQualitySignal", "Paper", "QualitySignal").is_ok()
        );
        assert!(validate_compiled_edge_endpoint("CITES", "Paper", "Paper").is_err());
        assert!(validate_compiled_edge_endpoint("hasSummary", "Paper", "QualitySignal").is_err());
    }

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
    fn active_yaml_and_compiled_endpoint_rows_are_exactly_equal() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry =
            kg_ontology::registry::OntologyRegistry::load_from_dir(root.join("data/ontology"))
                .unwrap();
        let yaml = registry
            .edge_contracts()
            .iter()
            .filter(|contract| {
                contract.implementation_status == kg_ontology::registry::ContractStatus::Active
            })
            .flat_map(|contract| {
                contract.target_labels.iter().map(move |target| {
                    (
                        contract.edge_constant.clone(),
                        contract.source_label.clone(),
                        target.clone(),
                    )
                })
            })
            .collect::<std::collections::BTreeSet<_>>();
        let compiled = edge_contracts()
            .into_iter()
            .flat_map(|contract| {
                contract.target_labels.iter().map(move |target| {
                    (
                        contract.edge_constant.to_string(),
                        contract.source_label.to_string(),
                        (*target).to_string(),
                    )
                })
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(compiled, yaml);
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
        assert!(yaml.contains("temporal_class"), "{yaml}");
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
        // ADR-050: production YAML is authoritative for active ontology labels.
        // Use the workspace-aware loader so cargo's test cwd cannot silently
        // fall back to the smaller compiled schema inventory.
        let registry = kg_ontology::registry::OntologyRegistry::load_project_meta_ontology()
            .expect("load production ontology DATA");
        let registered: HashSet<String> =
            registry.labels().into_iter().map(str::to_string).collect();
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

    /// No duplicate edge+source rows with contradictory target contracts.
    /// One edge wire may be overloaded across source labels (e.g. Paper and
    /// Section both have Summary nodes), but a given source has one target set.
    #[test]
    fn test_no_contradictory_edge_contracts() {
        let contracts = edge_contracts();
        for (i, a) in contracts.iter().enumerate() {
            for b in contracts.iter().skip(i + 1) {
                if a.edge_constant == b.edge_constant && a.source_label == b.source_label {
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
            structure::HAS_QUALITY_SIGNAL,
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

    /// ADR-048 causal primitives must all have contracts. These are the
    /// six decision audit and causal edges scoped to scientific KG use. They
    /// unblock ADR-053 Layer G (entity resolution merge recording) and the
    /// ADR-053 Decision Intelligence extension.
    #[test]
    fn test_adr_048_causal_edges_have_contracts() {
        let edges = edge_contracts_or_yaml();
        let causal = [
            crate::relation::decision::CAUSED,
            crate::relation::decision::AFFECTS,
            crate::relation::decision::INFLUENCED,
            crate::relation::decision::PRECEDENT_FOR,
            crate::relation::decision::AUTHORITY_FOR,
            crate::relation::decision::TRIGGERED_BY,
        ];
        for c in causal {
            assert!(
                edges.iter().any(|e| e.edge_constant == c),
                "ADR-048 causal edge '{}' missing contract in edge_contracts_or_yaml() — add it to edge_contracts.yaml",
                c
            );
        }
    }

    /// ADR-048 causal edges are fact edges (retractable) per ADR-046 —
    /// wrong attribution / retracted reasoning can invalidate them.
    #[test]
    fn test_adr_048_causal_edges_are_fact_temporal_class() {
        let edges = edge_contracts_or_yaml();
        let causal = [
            crate::relation::decision::CAUSED,
            crate::relation::decision::AFFECTS,
            crate::relation::decision::INFLUENCED,
            crate::relation::decision::PRECEDENT_FOR,
            crate::relation::decision::AUTHORITY_FOR,
            crate::relation::decision::TRIGGERED_BY,
        ];
        for c in causal {
            let contract = edges
                .iter()
                .find(|e| e.edge_constant == c)
                .unwrap_or_else(|| panic!("causal edge '{}' missing contract", c));
            assert_eq!(
                contract.temporal_class,
                kg_ontology::registry::TemporalClass::Fact,
                "ADR-048 causal edge '{}' must be temporal_class=fact (retractable)",
                c
            );
        }
    }

    /// ADR-053 Layer A navigation edges must be declared as forward contracts
    /// in edge_contracts.yaml. These are not yet implemented in Rust relation.rs
    /// (Wave 1 work, M362) but the YAML forward-declares them so ontology is
    /// the single source of truth.
    /// ADR-053 Layer A navigation edges must be declared as contracts in both
    /// YAML (single source of truth for ontology planning) AND Rust relation.rs
    /// (materialized constants for use in application/cli code). The contract
    /// must be present in the hardcoded edge_contracts() list or the YAML-driven
    /// edge_contracts_or_yaml() fallback.
    #[test]
    fn test_adr_053_navigation_edges_forward_declared_in_yaml() {
        let yaml_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/ontology/edge_contracts.yaml");
        let yaml = std::fs::read_to_string(&yaml_path)
            .unwrap_or_else(|e| panic!("read {}: {}", yaml_path.display(), e));
        for nav in ["next_sibling", "first_child", "parent_section"] {
            assert!(
                yaml.contains(&format!("edge_constant: {}", nav)),
                "ADR-053 Layer A navigation edge '{}' must be forward-declared in edge_contracts.yaml",
                nav
            );
        }
    }

    /// ADR-053 Layer A T03: navigation edges must be materialized as Rust
    /// constants in `relation::structure` and present in `edge_contracts()`
    /// hardcoded list. Verifies the Rust side of the data-driven contract gap
    /// (YAML had forward declarations from earlier session; this test confirms
    /// Rust constants and EdgeContract entries now match).
    #[test]
    fn test_adr_053_navigation_edges_materialized_in_rust() {
        use crate::relation::structure;
        // Constants exist in relation::structure module.
        assert_eq!(structure::NEXT_SIBLING, "next_sibling");
        assert_eq!(structure::FIRST_CHILD, "first_child");
        assert_eq!(structure::PARENT_SECTION, "parent_section");

        // Each constant has a matching EdgeContract entry in the hardcoded list.
        let contracts = edge_contracts();
        for nav in [
            structure::NEXT_SIBLING,
            structure::FIRST_CHILD,
            structure::PARENT_SECTION,
        ] {
            assert!(
                contracts.iter().any(|c| c.edge_constant == nav),
                "navigation edge '{}' must have an EdgeContract entry in edge_contracts()",
                nav
            );
        }
    }

    /// ADR-053 Layer A navigation edges are declared `temporal_class=structural`
    /// in edge_contracts.yaml (created_at only per ADR-046). They are immutable
    /// per PDF hash — no invalidation race like `fact` edges.
    #[test]
    fn test_adr_053_navigation_edges_are_structural_temporal_class() {
        let yaml_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/ontology/edge_contracts.yaml");
        let yaml = std::fs::read_to_string(&yaml_path)
            .unwrap_or_else(|e| panic!("read {}: {}", yaml_path.display(), e));
        // Find each navigation edge block and check temporal_class within it.
        for nav in ["next_sibling", "first_child", "parent_section"] {
            let edge_line = format!("edge_constant: {}", nav);
            let pos = yaml
                .find(&edge_line)
                .unwrap_or_else(|| panic!("edge '{}' not in YAML", nav));
            // Look in the 300 bytes after the edge_constant line for temporal_class.
            let block = &yaml[pos..pos.saturating_add(300).min(yaml.len())];
            assert!(
                block.contains("temporal_class: structural"),
                "navigation edge '{}' must have temporal_class=structural in edge_contracts.yaml",
                nav
            );
        }
    }
}
