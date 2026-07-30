//! Rule-based ConceptCluster detection (Phase 4 offline, ADR-042 revised).
//!
//! Creates derived semantic community clusters based on entity co-occurrence
//! across papers. No LLM required — deterministic.
//!
//! IMPORTANT: These are NOT evidence hyperedges. ConceptCluster is a derived
//! community object for retrieval expansion and topic association.
//! Source-grounded evidence requires EvidenceBundle (future P1 work).
//! Do NOT run evidence PPR over MEMBER_OF_CLUSTER edges.

use crate::entity::EntityType;
use std::collections::{HashMap, HashSet};

/// Minimum co-occurrence count to form a cluster (configurable).
pub const MIN_COOCCURRENCE: usize = 3;

/// Minimum single-entity mention count for a standalone cluster.
pub const MIN_SINGLE_MENTIONS: usize = 5;

/// A detected concept cluster from co-occurrence analysis.
#[derive(Debug, Clone)]
pub struct DetectedCluster {
    pub label: String,
    pub cluster_type: String,
    pub members: Vec<String>,
    pub member_types: Vec<String>,
    pub co_occurrence_count: usize,
}

/// Input for cluster detection: entity label → (entity type, set of paper IDs).
pub type EntityPapers = HashMap<String, (EntityType, HashSet<String>)>;

/// Detect concept clusters from entity co-occurrence patterns.
///
/// This is a pure function — no graph writes. Returns clusters that SHOULD
/// be created. The caller (application layer) writes them to the graph.
pub fn detect_clusters(entity_papers: &EntityPapers) -> Vec<DetectedCluster> {
    let mut clusters = Vec::new();

    // 1. Single-entity clusters (high-mention entities)
    for (label, (etype, papers)) in entity_papers {
        if papers.len() >= MIN_SINGLE_MENTIONS {
            let cluster_type = cluster_type_for_entity(etype);
            clusters.push(DetectedCluster {
                label: format!("{label} mentions"),
                cluster_type: cluster_type.to_string(),
                members: vec![label.clone()],
                member_types: vec![format!("{etype:?}")],
                co_occurrence_count: papers.len(),
            });
        }
    }

    // 2. Co-occurrence clusters (pairs that appear together in ≥N papers)
    let entity_labels: Vec<String> = entity_papers.keys().cloned().collect();
    for i in 0..entity_labels.len() {
        for j in (i + 1)..entity_labels.len() {
            let label_a = &entity_labels[i];
            let label_b = &entity_labels[j];

            let Some((etype_a, papers_a)) = entity_papers.get(label_a) else {
                continue;
            };
            let Some((etype_b, papers_b)) = entity_papers.get(label_b) else {
                continue;
            };

            let shared = papers_a.intersection(papers_b).count();

            if shared >= MIN_COOCCURRENCE {
                let cluster_type = if etype_a != etype_b {
                    "concept_cluster"
                } else {
                    cluster_type_for_entity(etype_a)
                };
                clusters.push(DetectedCluster {
                    label: format!("{label_a} + {label_b}"),
                    cluster_type: cluster_type.to_string(),
                    members: vec![label_a.clone(), label_b.clone()],
                    member_types: vec![format!("{etype_a:?}"), format!("{etype_b:?}")],
                    co_occurrence_count: shared,
                });
            }
        }
    }

    clusters
}

/// Map entity type to default cluster type.
fn cluster_type_for_entity(etype: &EntityType) -> &'static str {
    match etype {
        EntityType::Method => "method_family",
        EntityType::Dataset => "benchmark_suite",
        _ => "concept_cluster",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_papers(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_detect_single_entity_clusters() {
        let mut entity_papers = EntityPapers::new();
        // GPT-4 mentioned in 6 papers → standalone cluster
        entity_papers.insert(
            "GPT-4".to_string(),
            (
                EntityType::Model,
                make_papers(&["p1", "p2", "p3", "p4", "p5", "p6"]),
            ),
        );
        // BERT mentioned in 2 papers → no cluster
        entity_papers.insert(
            "BERT".to_string(),
            (EntityType::Model, make_papers(&["p1", "p2"])),
        );

        let clusters = detect_clusters(&entity_papers);
        assert_eq!(
            clusters.len(),
            1,
            "only GPT-4 should get a cluster, got: {clusters:?}"
        );
        assert_eq!(clusters[0].members, vec!["GPT-4"]);
        assert_eq!(clusters[0].co_occurrence_count, 6);
    }

    #[test]
    fn test_detect_co_occurrence_clusters() {
        let mut entity_papers = EntityPapers::new();
        // PPO and GRPO co-occur in 4 papers → method_family cluster
        entity_papers.insert(
            "PPO".to_string(),
            (EntityType::Method, make_papers(&["p1", "p2", "p3", "p4"])),
        );
        entity_papers.insert(
            "GRPO".to_string(),
            (EntityType::Method, make_papers(&["p1", "p2", "p3", "p4"])),
        );

        let clusters = detect_clusters(&entity_papers);
        // PPO has 4 mentions (< 5, so no single-entity cluster)
        // GRPO has 4 mentions (< 5, so no single-entity cluster)
        // But PPO + GRPO co-occur in 4 papers (≥ 3) → co-occurrence cluster
        let co_occurrence = clusters.iter().find(|c| c.members.len() == 2);
        assert!(
            co_occurrence.is_some(),
            "expected a co-occurrence cluster, got: {clusters:?}"
        );
        let cluster = co_occurrence.unwrap();
        assert!(cluster.members.contains(&"PPO".to_string()));
        assert!(cluster.members.contains(&"GRPO".to_string()));
        assert_eq!(cluster.cluster_type, "method_family");
    }

    #[test]
    fn test_no_clusters_for_rare_entities() {
        let mut entity_papers = EntityPapers::new();
        entity_papers.insert(
            "RareMethod".to_string(),
            (EntityType::Method, make_papers(&["p1"])),
        );

        let clusters = detect_clusters(&entity_papers);
        assert!(clusters.is_empty(), "rare entity should not get a cluster");
    }

    #[test]
    fn test_cluster_type_mapping() {
        assert_eq!(
            cluster_type_for_entity(&EntityType::Method),
            "method_family"
        );
        assert_eq!(
            cluster_type_for_entity(&EntityType::Dataset),
            "benchmark_suite"
        );
        assert_eq!(
            cluster_type_for_entity(&EntityType::Model),
            "concept_cluster"
        );
    }
}
