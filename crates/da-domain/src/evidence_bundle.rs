//! Evidence Bundle and Claim domain types (ADR-042 revised, P1).
//!
//! Source-grounded n-ary evidence units and proposition-bearing claims.
//! Distinct from ConceptCluster (derived community) — these carry truth value.

use crate::schema::{FieldType, NodeSchemaDef};

/// EvidenceBundle node — source-grounded n-ary evidence unit.
///
/// Represents a self-contained fact extracted from a document section,
/// connecting multiple entities with roles and grounding to source text.
/// Subtypes: ExperimentSetup, ResultBundle, CitationContext, ClaimBundle.
///
/// PARTICIPATES_IN edges link Entity → EvidenceBundle with role property.
pub struct EvidenceBundleSchema;

impl NodeSchemaDef for EvidenceBundleSchema {
    fn label(&self) -> &'static str {
        "EvidenceBundle"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("bundle_type", FieldType::String),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("normalized_text", FieldType::String),
            ("source_span_id", FieldType::String),
            ("document_id", FieldType::String),
            ("section_id", FieldType::String),
            ("extraction_confidence", FieldType::Float),
            ("verification_status", FieldType::String),
            ("artifact_hash", FieldType::String),
            ("valid_from", FieldType::DateTime),
            ("valid_to", FieldType::DateTime),
            ("schema_version", FieldType::Integer),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

/// Claim node — proposition-bearing truth value target.
///
/// Entities are mentionable terms, not truth-bearing. Claims carry
/// propositions that can be supported, contradicted, or qualified
/// by EvidenceBundles.
///
/// EvidenceBundle → SUPPORTS → Claim
/// EvidenceBundle → CONTRADICTS → Claim
/// EvidenceBundle → QUALIFIES → Claim
pub struct ClaimSchema;

impl NodeSchemaDef for ClaimSchema {
    fn label(&self) -> &'static str {
        "Claim"
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("text", FieldType::String)]
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("claim_type", FieldType::String),
            ("modality", FieldType::String),
            ("scope", FieldType::String),
            ("valid_time", FieldType::DateTime),
            ("source_span_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

// Bundle type and verification status vocabularies moved to
// data/node_vocabulary.yaml. Use crate::vocabulary::is_known_bundle_type()
// and crate::vocabulary::is_known_verification_status() for validation.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_evidence_bundle_schema_label() {
        assert_eq!(EvidenceBundleSchema.label(), "EvidenceBundle");
    }

    #[test]
    fn test_evidence_bundle_required_fields() {
        let fields = EvidenceBundleSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"vid"));
        assert!(names.contains(&"bundle_type"));
    }

    #[test]
    fn test_evidence_bundle_has_source_span() {
        let fields = EvidenceBundleSchema.optional_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"source_span_id"),
            "EvidenceBundle must be source-grounded, got: {names:?}"
        );
        assert!(
            names.contains(&"verification_status"),
            "EvidenceBundle must track verification, got: {names:?}"
        );
    }

    #[test]
    fn test_claim_schema_label() {
        assert_eq!(ClaimSchema.label(), "Claim");
    }

    #[test]
    fn test_claim_required_fields() {
        let fields = ClaimSchema.required_fields();
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"vid"));
        assert!(names.contains(&"text"));
    }

    #[test]
    fn test_claim_validates() {
        let schema = ClaimSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::json!("vid:claim:001"));
        props.insert(
            "text".to_string(),
            serde_json::json!("PPO outperforms DPO on MATH"),
        );
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_bundle_types_from_registry() {
        assert!(crate::vocabulary::is_known_bundle_type("experiment_setup"));
        assert!(crate::vocabulary::is_known_bundle_type("result_bundle"));
    }
}
