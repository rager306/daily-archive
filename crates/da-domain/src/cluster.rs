//! Rule-based ConceptCluster detection (Phase 4 offline, ADR-042 revised).
//!
//! Creates derived semantic community clusters based on entity co-occurrence
//! across papers. No LLM required — deterministic.
//!
//! IMPORTANT: These are NOT evidence hyperedges. ConceptCluster is a derived
//! community object for retrieval expansion and topic association.
//! Source-grounded evidence requires EvidenceBundle (future P1 work).
//! Do NOT run evidence PPR over MEMBER_OF_CLUSTER edges.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

// ─── Algorithm parameters (loaded from YAML) ───

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ClusterParamsYaml {
    version: String,
    cluster_detection: ClusterDetectionParams,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ClusterDetectionParams {
    min_cooccurrence: usize,
    min_single_mentions: usize,
    default_cluster_type: String,
    type_cluster_types: HashMap<String, String>,
}

const BUNDLED_PARAMS_YAML: &str = include_str!("../../../data/algorithm_params.yaml");

struct ClusterConfig {
    source_fingerprint: String,
    min_cooccurrence: usize,
    min_single_mentions: usize,
    default_cluster_type: String,
    type_cluster_types: HashMap<String, String>,
}

impl ClusterConfig {
    fn load() -> Self {
        let yaml_str = std::fs::read_to_string("data/algorithm_params.yaml")
            .unwrap_or_else(|_| BUNDLED_PARAMS_YAML.to_string());
        let data: ClusterParamsYaml =
            serde_yaml::from_str(&yaml_str).expect("algorithm_params.yaml must be valid YAML");
        Self {
            source_fingerprint: format!(
                "sha256:{}",
                hex::encode(Sha256::digest(yaml_str.as_bytes()))
            ),
            min_cooccurrence: data.cluster_detection.min_cooccurrence,
            min_single_mentions: data.cluster_detection.min_single_mentions,
            default_cluster_type: data.cluster_detection.default_cluster_type,
            type_cluster_types: data.cluster_detection.type_cluster_types,
        }
    }

    fn instance() -> &'static ClusterConfig {
        static CONFIG: OnceLock<ClusterConfig> = OnceLock::new();
        CONFIG.get_or_init(ClusterConfig::load)
    }
}

/// Minimum co-occurrence count to form a cluster (from YAML config).
pub fn min_cooccurrence() -> usize {
    ClusterConfig::instance().min_cooccurrence
}

/// Minimum single-entity mention count for a standalone cluster (from YAML).
pub fn min_single_mentions() -> usize {
    ClusterConfig::instance().min_single_mentions
}

pub fn cluster_configuration_fingerprint() -> String {
    ClusterConfig::instance().source_fingerprint.clone()
}

/// A detected concept cluster from co-occurrence analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCluster {
    pub label: String,
    pub cluster_type: String,
    pub members: Vec<EntityClusterKey>,
    pub co_occurrence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityClusterKey {
    pub entity_type: String,
    pub label: String,
}

pub fn normalize_entity_key(entity_type: &str, label: &str) -> EntityClusterKey {
    EntityClusterKey {
        entity_type: entity_type.trim().to_string(),
        label: label.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// Input for cluster detection keyed by ontology type plus label.
pub type EntityPapers = HashMap<EntityClusterKey, HashSet<String>>;

/// Detect concept clusters from entity co-occurrence patterns.
///
/// This is a pure function — no graph writes. Returns clusters that SHOULD
/// be created. The caller (application layer) writes them to the graph.
pub fn detect_clusters(entity_papers: &EntityPapers) -> Vec<DetectedCluster> {
    let mut clusters = Vec::new();

    // 1. Single-entity clusters (high-mention entities)
    for (entity, papers) in entity_papers {
        if papers.len() >= min_single_mentions() {
            clusters.push(DetectedCluster {
                label: format!("{} mentions", entity.label),
                cluster_type: cluster_type_for_entity(&entity.entity_type),
                members: vec![entity.clone()],
                co_occurrence_count: papers.len(),
            });
        }
    }

    // 2. Co-occurrence clusters (pairs that appear together in ≥N papers)
    let mut entity_keys: Vec<EntityClusterKey> = entity_papers.keys().cloned().collect();
    entity_keys.sort();
    for i in 0..entity_keys.len() {
        for j in (i + 1)..entity_keys.len() {
            let entity_a = &entity_keys[i];
            let entity_b = &entity_keys[j];

            let Some(papers_a) = entity_papers.get(entity_a) else {
                continue;
            };
            let Some(papers_b) = entity_papers.get(entity_b) else {
                continue;
            };

            let shared = papers_a.intersection(papers_b).count();

            if shared >= min_cooccurrence() {
                let cluster_type = if entity_a.entity_type != entity_b.entity_type {
                    ClusterConfig::instance().default_cluster_type.clone()
                } else {
                    cluster_type_for_entity(&entity_a.entity_type)
                };
                clusters.push(DetectedCluster {
                    label: format!("{} + {}", entity_a.label, entity_b.label),
                    cluster_type,
                    members: vec![entity_a.clone(), entity_b.clone()],
                    co_occurrence_count: shared,
                });
            }
        }
    }

    clusters.sort_by(|left, right| {
        left.cluster_type
            .cmp(&right.cluster_type)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.members.cmp(&right.members))
    });
    clusters
}

/// Map entity type to default cluster type.
fn cluster_type_for_entity(entity_type: &str) -> String {
    let config = ClusterConfig::instance();
    config
        .type_cluster_types
        .get(entity_type)
        .cloned()
        .unwrap_or_else(|| config.default_cluster_type.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thresholds_loaded_from_yaml() {
        assert!(min_cooccurrence() >= 1);
        assert!(min_single_mentions() >= 1);
        // Default values from bundled YAML
        assert_eq!(min_cooccurrence(), 3);
        assert_eq!(min_single_mentions(), 5);
    }

    fn make_papers(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn key(entity_type: &str, label: &str) -> EntityClusterKey {
        EntityClusterKey {
            entity_type: entity_type.into(),
            label: label.into(),
        }
    }

    #[test]
    fn test_detect_single_entity_clusters() {
        let mut entity_papers = EntityPapers::new();
        entity_papers.insert(
            key("Model", "GPT-4"),
            make_papers(&["p1", "p2", "p3", "p4", "p5", "p6"]),
        );
        entity_papers.insert(key("Model", "BERT"), make_papers(&["p1", "p2"]));

        let clusters = detect_clusters(&entity_papers);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members, vec![key("Model", "GPT-4")]);
        assert_eq!(clusters[0].co_occurrence_count, 6);
    }

    #[test]
    fn test_detect_co_occurrence_clusters() {
        let mut entity_papers = EntityPapers::new();
        entity_papers.insert(key("Method", "PPO"), make_papers(&["p1", "p2", "p3", "p4"]));
        entity_papers.insert(
            key("Method", "GRPO"),
            make_papers(&["p1", "p2", "p3", "p4"]),
        );

        let clusters = detect_clusters(&entity_papers);
        let cluster = clusters
            .iter()
            .find(|cluster| cluster.members.len() == 2)
            .unwrap();
        assert!(cluster.members.contains(&key("Method", "PPO")));
        assert!(cluster.members.contains(&key("Method", "GRPO")));
        assert_eq!(cluster.cluster_type, "method_family");
    }

    #[test]
    fn test_no_clusters_for_rare_entities() {
        let mut entity_papers = EntityPapers::new();
        entity_papers.insert(key("Method", "RareMethod"), make_papers(&["p1"]));
        assert!(detect_clusters(&entity_papers).is_empty());
    }

    #[test]
    fn test_cluster_type_mapping_is_data_driven_for_arbitrary_types() {
        assert_eq!(cluster_type_for_entity("Method"), "method_family");
        assert_eq!(cluster_type_for_entity("Dataset"), "benchmark_suite");
        assert_eq!(cluster_type_for_entity("Theorem"), "concept_cluster");
    }
}
