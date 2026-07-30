//! Source provenance node (ONTOLOGY-DESIGN Layer 0) + source registry.
//!
//! Reference data (source codes, types, profiles) loaded from YAML:
//!   data/source_codes.yaml
//!
//! This module provides schema + registry logic. No hardcoded vocabularies.

use crate::schema::{FieldType, NodeSchemaDef};
use std::collections::HashSet;
use std::sync::OnceLock;

// ─── Schema ───

/// Source node schema (ONTOLOGY-DESIGN Layer 0).
pub struct SourceSchema;

impl NodeSchemaDef for SourceSchema {
    fn label(&self) -> &'static str {
        "Source"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("code", FieldType::String),
            ("source_type", FieldType::String),
            ("domain", FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("reliability_tier", FieldType::Integer),
            ("access_method", FieldType::String),
            ("base_url", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

// ─── YAML schema types ───

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct SourceCodesYaml {
    version: String,
    source_codes: Vec<SourceCodeEntry>,
    source_types: Vec<CodeEntry>,
    source_profiles: Vec<CodeEntry>,
    reliability_tiers: Vec<ReliabilityTierEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct SourceCodeEntry {
    code: String,
    name: String,
    reliability_tier: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct CodeEntry {
    code: String,
    name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ReliabilityTierEntry {
    tier: i64,
    name: String,
}

/// Bundled fallback YAML.
const BUNDLED_YAML: &str = include_str!("../../../data/source_codes.yaml");

// ─── Registry ───

/// Source registry — holds codes loaded from YAML.
pub struct SourceRegistry {
    source_codes: HashSet<String>,
    source_types: HashSet<String>,
    source_profiles: HashSet<String>,
}

impl SourceRegistry {
    fn load() -> Self {
        let yaml_str = std::fs::read_to_string("data/source_codes.yaml")
            .unwrap_or_else(|_| BUNDLED_YAML.to_string());
        let data: SourceCodesYaml =
            serde_yaml::from_str(&yaml_str).expect("source_codes.yaml must be valid YAML");

        Self {
            source_codes: data.source_codes.iter().map(|e| e.code.clone()).collect(),
            source_types: data.source_types.iter().map(|e| e.code.clone()).collect(),
            source_profiles: data
                .source_profiles
                .iter()
                .map(|e| e.code.clone())
                .collect(),
        }
    }

    fn instance() -> &'static SourceRegistry {
        static REGISTRY: OnceLock<SourceRegistry> = OnceLock::new();
        REGISTRY.get_or_init(SourceRegistry::load)
    }
}

// ─── Public API ───

/// Check if a source code is known.
pub fn is_known_source_code(code: &str) -> bool {
    SourceRegistry::instance().source_codes.contains(code)
}

/// Check if a source type is known.
pub fn is_known_source_type(code: &str) -> bool {
    SourceRegistry::instance().source_types.contains(code)
}

/// Check if a source profile is known.
pub fn is_known_source_profile(code: &str) -> bool {
    SourceRegistry::instance().source_profiles.contains(code)
}

// Convenience: common source codes validated against YAML registry.
// Use string literals directly or is_known_source_code() for validation.
// YAML remains the single source of truth.

/// Reliability tiers (from YAML).
pub const TIER_CURATED: i64 = 1;
pub const TIER_EXTRACTED: i64 = 2;
pub const TIER_USER: i64 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_schema_label() {
        assert_eq!(SourceSchema.label(), "Source");
    }

    #[test]
    fn test_source_schema_required_fields() {
        let fields = SourceSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"vid"));
        assert!(names.contains(&"code"));
        assert!(names.contains(&"source_type"));
        assert!(names.contains(&"domain"));
    }

    #[test]
    fn test_is_known_source_code() {
        assert!(is_known_source_code("arxiv"));
        assert!(is_known_source_code("openalex"));
        assert!(!is_known_source_code("unknown_source"));
    }

    #[test]
    fn test_is_known_source_type() {
        assert!(is_known_source_type("pdf"));
        assert!(is_known_source_type("html"));
        assert!(!is_known_source_type("unknown_type"));
    }

    #[test]
    fn test_is_known_source_profile() {
        assert!(is_known_source_profile("scientific_paper"));
        assert!(is_known_source_profile("textbook"));
        assert!(!is_known_source_profile("unknown_profile"));
    }

    #[test]
    fn test_source_codes_from_yaml() {
        assert!(is_known_source_code("arxiv"));
        assert!(is_known_source_code("openalex"));
        assert!(is_known_source_code("textbook"));
        assert!(!is_known_source_code("unknown_source"));
    }
}
