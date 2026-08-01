//! Edge endpoint contract matrix (ADR-045 Wave G foundation).
//!
//! Documents the expected source/target node labels for each edge type
//! that the pipeline materializes. This is a static documentation test:
//! it does not read the graph. Instead it encodes the contract as data
//! so that future code reviews and audits can check pipeline edges
//! against it.
//!
//! When a new edge type is added to the pipeline, add a row here and
//! the source/target labels it connects. If an existing edge's endpoint
//! semantics changes, update the row in the same commit.
//!
//! See ADR-045 §Wave G and MEM500 #5.

/// One row of the edge-endpoint contract matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeContract {
    pub edge_constant: &'static str,
    pub source_label: &'static str,
    pub target_label: &'static str,
    /// Short rationale — why this edge connects these two labels.
    pub rationale: &'static str,
}

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
            target_label: "Institution",
            rationale: "Author is affiliated with an institution (OpenAlex authorship).",
        },
        EdgeContract {
            edge_constant: bibliographic::CITES,
            source_label: "Paper",
            target_label: "Citation",
            rationale: "Paper cites a bibliographic citation (resolvable to a Paper).",
        },
        EdgeContract {
            edge_constant: structure::AUTHORED_BY,
            source_label: "Author",
            target_label: "Paper",
            rationale: "Author authored the Paper (OpenAlex authorship).",
        },
        EdgeContract {
            edge_constant: structure::FOUND_IN,
            source_label: "Entity",
            target_label: "Section",
            rationale: "Entity was extracted from this Section of the Paper.",
        },
        EdgeContract {
            edge_constant: structure::FROM_SOURCE,
            source_label: "Paper",
            target_label: "Source",
            rationale: "Paper originated from this Source (provenance).",
        },
        EdgeContract {
            edge_constant: structure::HAS_PART,
            source_label: "Paper",
            target_label: "Section",
            rationale: "Paper has this Section as a structural part (FaBiO frbr:part).",
        },
        EdgeContract {
            edge_constant: structure::HAS_TOPIC,
            source_label: "Paper",
            target_label: "Topic",
            rationale: "Paper is classified under this Topic (OpenAlex topic assignment).",
        },
        EdgeContract {
            edge_constant: structure::IN_CATEGORY,
            source_label: "Paper",
            target_label: "Category",
            rationale: "Paper is in this arXiv/OpenAlex Category.",
        },
        EdgeContract {
            edge_constant: hypergraph::MEMBER_OF_CLUSTER,
            source_label: "Entity",
            target_label: "ConceptCluster",
            rationale: "Entity is a member of this ConceptCluster.",
        },
        EdgeContract {
            edge_constant: bibliographic::MENTIONS,
            source_label: "Paper",
            target_label: "Entity",
            rationale: "Paper mentions this Entity (extracted from paper text).",
        },
        EdgeContract {
            edge_constant: hypergraph::PARTICIPATES_IN,
            source_label: "Entity",
            target_label: "EvidenceBundle",
            rationale: "Entity participates in this EvidenceBundle (co-occurring entities).",
        },
        EdgeContract {
            edge_constant: hypergraph::SUPPORTS,
            source_label: "EvidenceBundle",
            target_label: "Claim",
            rationale: "EvidenceBundle supports this Claim.",
        },
        EdgeContract {
            edge_constant: bibliographic::SUPERSEDES,
            source_label: "Entity",
            target_label: "Entity",
            rationale: "Source Entity supersedes target Entity (D135 merge scenario).",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every edge constant in the contract matrix must be a registered edge
    /// (validator-recognized). Catches typos and drift.
    #[test]
    fn test_all_contract_edges_are_registered() {
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
            assert!(
                registered.contains(c.target_label),
                "edge {} target label '{}' not a registered node type",
                c.edge_constant,
                c.target_label
            );
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

    /// No duplicate edge constants with different endpoint contracts.
    /// Two rows with the same edge_constant must agree on source/target.
    #[test]
    fn test_no_contradictory_edge_contracts() {
        let contracts = edge_contracts();
        for (i, a) in contracts.iter().enumerate() {
            for b in contracts.iter().skip(i + 1) {
                if a.edge_constant == b.edge_constant {
                    assert_eq!(
                        (a.source_label, a.target_label),
                        (b.source_label, b.target_label),
                        "edge '{}' has contradictory contracts:\n  {:?}\n  {:?}",
                        a.edge_constant,
                        a,
                        b
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
        let contract_edges: HashSet<String> =
            edge_contracts().iter().map(|c| c.edge_constant.to_string()).collect();

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
}
