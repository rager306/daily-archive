//! Node vocabulary registry — bundle types, verification statuses, cluster types.
//!
//! Reference data loaded from YAML: data/node_vocabulary.yaml
//! No hardcoded vocabularies — single source of truth is YAML.

use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct NodeVocabularyYaml {
    version: String,
    bundle_types: Vec<VocabEntry>,
    verification_statuses: Vec<VocabEntry>,
    cluster_types: Vec<VocabEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct VocabEntry {
    code: String,
    name: String,
}

const BUNDLED_YAML: &str = include_str!("../../../data/node_vocabulary.yaml");

/// Vocabulary registry for Layer 6 node type codes.
pub struct NodeVocabularyRegistry {
    pub bundle_types: HashSet<String>,
    pub verification_statuses: HashSet<String>,
    pub cluster_types: HashSet<String>,
}

impl NodeVocabularyRegistry {
    fn load() -> Self {
        let yaml_str = std::fs::read_to_string("data/node_vocabulary.yaml")
            .unwrap_or_else(|_| BUNDLED_YAML.to_string());
        let data: NodeVocabularyYaml =
            serde_yaml::from_str(&yaml_str).expect("node_vocabulary.yaml must be valid YAML");
        Self {
            bundle_types: data.bundle_types.iter().map(|e| e.code.clone()).collect(),
            verification_statuses: data
                .verification_statuses
                .iter()
                .map(|e| e.code.clone())
                .collect(),
            cluster_types: data.cluster_types.iter().map(|e| e.code.clone()).collect(),
        }
    }

    fn instance() -> &'static NodeVocabularyRegistry {
        static REGISTRY: OnceLock<NodeVocabularyRegistry> = OnceLock::new();
        REGISTRY.get_or_init(NodeVocabularyRegistry::load)
    }
}

/// Check if a bundle type code is known.
pub fn is_known_bundle_type(code: &str) -> bool {
    NodeVocabularyRegistry::instance()
        .bundle_types
        .contains(code)
}

/// Check if a verification status is known.
pub fn is_known_verification_status(code: &str) -> bool {
    NodeVocabularyRegistry::instance()
        .verification_statuses
        .contains(code)
}

/// Check if a cluster type code is known.
pub fn is_known_cluster_type(code: &str) -> bool {
    NodeVocabularyRegistry::instance()
        .cluster_types
        .contains(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_types_from_yaml() {
        assert!(is_known_bundle_type("experiment_setup"));
        assert!(is_known_bundle_type("result_bundle"));
        assert!(is_known_bundle_type("citation_context"));
        assert!(is_known_bundle_type("claim_bundle"));
        assert!(!is_known_bundle_type("unknown_bundle"));
    }

    #[test]
    fn test_verification_statuses_from_yaml() {
        assert!(is_known_verification_status("pending"));
        assert!(is_known_verification_status("verified"));
        assert!(is_known_verification_status("disputed"));
        assert!(!is_known_verification_status("unknown_status"));
    }

    #[test]
    fn test_cluster_types_from_yaml() {
        assert!(is_known_cluster_type("concept_cluster"));
        assert!(is_known_cluster_type("method_family"));
        assert!(is_known_cluster_type("benchmark_suite"));
        assert!(!is_known_cluster_type("unknown_cluster"));
    }
}
