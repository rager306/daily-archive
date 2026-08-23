//! Lazy-loaded central path configuration (M450).
//!
//! Meta-prompt principle: paths are DATA (data/path_config.yaml),
//! not code. This module provides process-wide access with env override.
//! Each accessor falls back to the documented default if the config
//! file is missing — but the YAML is the single source of truth.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::path_config::PathConfig;

fn config() -> &'static PathConfig {
    static CONFIG: OnceLock<PathConfig> = OnceLock::new();
    CONFIG.get_or_init(PathConfig::load)
}

/// Root of the article catalog (sources/).
pub fn catalog_root() -> PathBuf {
    config().get_or("catalog_root", "data/sources/arxiv")
}

/// Path to the extraction patterns dictionary.
pub fn extraction_patterns() -> PathBuf {
    config().get_or("extraction_patterns", "data/extraction_patterns.yaml")
}

/// Path to the node schemas ontology.
pub fn node_schemas() -> PathBuf {
    config().get_or("node_schemas", "data/ontology/node_schemas.yaml")
}

/// Path to the edge contracts ontology.
pub fn edge_contracts() -> PathBuf {
    config().get_or("edge_contracts", "data/ontology/edge_contracts.yaml")
}

/// Path to the DOI prefix map.
pub fn doi_prefix_map() -> PathBuf {
    config().get_or("doi_prefix_map", "data/ontology/doi_prefix_map.yaml")
}

/// Directory containing all graph task suites.
pub fn graph_tasks_dir() -> PathBuf {
    config().get_or("graph_tasks_dir", "data/graph_tasks")
}

/// Directory containing gold standard fixtures.
pub fn gold_standard_dir() -> PathBuf {
    config().get_or("gold_standard_dir", "data/gold_standard")
}

/// Directory containing graph snapshots.
pub fn graphs_dir() -> PathBuf {
    config().get_or("graphs_dir", "data/graphs")
}

/// Pipeline cache root (scratch, recomputable).
pub fn pipeline_cache_root() -> PathBuf {
    config().get_or("pipeline_cache", "artifacts/pipeline_cache")
}
