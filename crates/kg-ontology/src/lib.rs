//! kg-ontology — Universal knowledge graph ontology.
//!
//! Reusable crate for schema-driven property-graph knowledge bases.
//! Loads ontology from YAML (`data/ontology/*.yaml`), validates nodes
//! and edges against declared schemas, manages schema versioning and
//! migrations, and exposes cross-cutting aspects (provenance, bi-temporal).
//!
//! # Design
//!
//! - **Zero project-specific dependencies.** This crate does not know
//!   about Paper, Claim, daily-archive, or any specific domain. It
//!   works against generic `PropertySnapshot` inputs.
//! - **Data, not code.** All schema data (node types, edge contracts,
//!   aspects, validation rules, versions, dictionaries) lives in YAML.
//!   Rust code holds loader logic + validator logic only.
//! - **Bundled fallback.** A reference ontology is compiled in via
//!   `include_str!` so the crate works zero-config. Callers override
//!   by placing files in `data/ontology/` or by calling
//!   `OntologyRegistry::load_from_dir()`.
//!
//! See ADR-050 for the full architecture decision and migration plan.

pub mod bitemporal;
pub mod edge_contract;
pub mod healing;
pub mod identity_index;
pub mod retry;
pub mod schema;
pub mod temporal;
pub mod validator;
pub mod vid;

pub use identity_index::{
    IDENTITY_INDEX_SCHEMA_VERSION, IdentityIndexError, IdentityIndexPolicy, IdentityIndexSpec,
    PROPERTY_INDEX_SCHEMA_VERSION, PropertyIndexPolicy,
};
pub use temporal::{
    OPEN, RetroactiveExtension, TemporalEdge, parse_datetime, validate_temporal_edge,
};
pub use validator::{PropertySnapshot, Severity, Violation, format_violations};

// Reserved module shells — populated in later migration phases (B-F)
// as code moves in from da-domain. Each module documents which phase
// will populate it, per ADR-050 §Migration path.

// schema_inner removed (M289-u4fzx5/S02): schema is now file-based.

pub mod registry;
pub use registry::{ContractStatus, OntologyRegistry, TemporalClass, YamlEdgeContract};

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
///
/// Verifies that `serialize → deserialize` reproduces the original value.
/// Wire-format drift (custom serde attributes, #[serde(default)], enum
/// renaming) is caught at test time, not in production.
#[cfg(test)]
pub(crate) fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(value, &back, "roundtrip mismatch: JSON = {json}");
}

/// ADR-051 §4.4 — serde roundtrip tests for all public Serialize+Deserialize types.
#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;
    use crate::healing::{HealingActor, HealingOperation, ProvenanceEvent};
    use crate::registry::{YamlEdgeContract, YamlFieldDef, YamlNodeSchema};
    use crate::retry::RetryPolicy;

    // --- HealingOperation: all 7 variants ---

    #[test]
    fn roundtrip_healing_operation_all_variants() {
        let variants = [
            HealingOperation::Correct,
            HealingOperation::Merge,
            HealingOperation::Split,
            HealingOperation::Silence,
            HealingOperation::Migrate,
            HealingOperation::Rollback,
            HealingOperation::RepairEdge,
        ];
        for v in &variants {
            roundtrip(v);
        }
    }

    // --- HealingActor: all 3 variants ---

    #[test]
    fn roundtrip_healing_actor_all_variants() {
        roundtrip(&HealingActor::Human("alice".into()));
        roundtrip(&HealingActor::Agent("bot-42".into()));
        roundtrip(&HealingActor::System("scheduler".into()));
    }

    // --- ProvenanceEvent: with and without changes ---

    #[test]
    fn roundtrip_provenance_event_empty_changes() {
        let event = ProvenanceEvent::new(
            HealingOperation::Correct,
            HealingActor::Human("alice".into()),
            vec!["vid1".into(), "vid2".into()],
            "fixed typo in title".into(),
        );
        roundtrip(&event);
    }

    #[test]
    fn roundtrip_provenance_event_with_changes() {
        let event = ProvenanceEvent::new(
            HealingOperation::Merge,
            HealingActor::System("auto-healer".into()),
            vec!["a".into()],
            "dedup".into(),
        )
        .with_change("title", serde_json::json!("old"), serde_json::json!("new"))
        .with_change(
            "embedding",
            serde_json::json!([1, 2, 3]),
            serde_json::json!([4, 5, 6]),
        );
        roundtrip(&event);
    }

    // --- RetryPolicy: default, fast, custom ---

    #[test]
    fn roundtrip_retry_policy_default() {
        roundtrip(&RetryPolicy::default());
    }

    #[test]
    fn roundtrip_retry_policy_fast() {
        roundtrip(&RetryPolicy::fast());
    }

    #[test]
    fn roundtrip_retry_policy_custom() {
        let p = RetryPolicy {
            initial_delay: 0,
            backoff_multiplier: 1.5,
            max_delay: 100,
            max_retries: 3,
        };
        roundtrip(&p);
    }

    // --- YamlFieldDef ---

    #[test]
    fn roundtrip_yaml_field_def() {
        roundtrip(&YamlFieldDef {
            name: "vid".into(),
            field_type: "String".into(),
        });
        roundtrip(&YamlFieldDef {
            name: "embedding".into(),
            field_type: "Vector(1024)".into(),
        });
    }

    // --- YamlNodeSchema ---

    #[test]
    fn roundtrip_yaml_node_schema() {
        let schema = YamlNodeSchema {
            label: "Paper".into(),
            required_fields: vec![YamlFieldDef {
                name: "vid".into(),
                field_type: "String".into(),
            }],
            optional_fields: vec![
                YamlFieldDef {
                    name: "doi".into(),
                    field_type: "String".into(),
                },
                YamlFieldDef {
                    name: "embedding".into(),
                    field_type: "Vector(1024)".into(),
                },
            ],
        };
        roundtrip(&schema);
    }

    #[test]
    fn roundtrip_yaml_node_schema_empty_fields() {
        let schema = YamlNodeSchema {
            label: "Marker".into(),
            required_fields: vec![],
            optional_fields: vec![],
        };
        roundtrip(&schema);
    }

    // --- TemporalClass ---

    #[test]
    fn roundtrip_temporal_class() {
        roundtrip(&TemporalClass::Structural);
        roundtrip(&TemporalClass::Fact);
        roundtrip(&ContractStatus::Active);
        roundtrip(&ContractStatus::Planned);
    }

    // --- YamlEdgeContract ---

    #[test]
    fn roundtrip_yaml_edge_contract_structural() {
        let contract = YamlEdgeContract {
            edge_constant: "hasPart".into(),
            source_label: "Paper".into(),
            target_labels: vec!["Section".into(), "Reference".into()],
            temporal_class: TemporalClass::Structural,
            rationale: "structural composition".into(),
            implementation_status: ContractStatus::Active,
        };
        roundtrip(&contract);
    }

    #[test]
    fn roundtrip_yaml_edge_contract_fact() {
        let contract = YamlEdgeContract {
            edge_constant: "CITES".into(),
            source_label: "Paper".into(),
            target_labels: vec!["Citation".into()],
            temporal_class: TemporalClass::Fact,
            rationale: "bibliography fact".into(),
            implementation_status: ContractStatus::Active,
        };
        roundtrip(&contract);
    }

    #[test]
    fn roundtrip_yaml_edge_contract_empty_targets() {
        let contract = YamlEdgeContract {
            edge_constant: "EMPTY".into(),
            source_label: "X".into(),
            target_labels: vec![],
            temporal_class: TemporalClass::Structural,
            rationale: String::new(),
            implementation_status: ContractStatus::Active,
        };
        roundtrip(&contract);
    }

    // --- M345: TemporalEdge, RetroactiveExtension, Violation, Severity ---

    #[test]
    fn roundtrip_temporal_edge() {
        use chrono::TimeZone;
        use chrono::Utc;
        let edge = crate::temporal::TemporalEdge {
            valid_at: Utc.timestamp_opt(1700000000, 0).unwrap(),
            invalid_at: None,
            expired_at: Some(Utc.timestamp_opt(1700001000, 0).unwrap()),
            reference_time: Some(Utc.timestamp_opt(1699999000, 0).unwrap()),
            created_at: Utc.timestamp_opt(1700000500, 0).unwrap(),
        };
        roundtrip(&edge);
    }

    #[test]
    fn roundtrip_temporal_edge_open() {
        use chrono::TimeZone;
        use chrono::Utc;
        let edge = crate::temporal::TemporalEdge {
            valid_at: Utc.timestamp_opt(1700000000, 0).unwrap(),
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: Utc.timestamp_opt(1700000000, 0).unwrap(),
        };
        roundtrip(&edge);
    }

    #[test]
    fn roundtrip_retroactive_extension() {
        use chrono::TimeZone;
        use chrono::Utc;
        let ext = crate::temporal::RetroactiveExtension {
            retroactive_to: Some(Utc.timestamp_opt(1600000000, 0).unwrap()),
            retroactivity_basis: Some("regulatory".into()),
        };
        roundtrip(&ext);
    }

    #[test]
    fn roundtrip_retroactive_extension_default() {
        let ext = crate::temporal::RetroactiveExtension::default();
        roundtrip(&ext);
    }

    #[test]
    fn roundtrip_severity_all_variants() {
        roundtrip(&crate::validator::Severity::Critical);
        roundtrip(&crate::validator::Severity::Warning);
    }

    #[test]
    fn roundtrip_violation() {
        let v = crate::validator::Violation::critical(
            "required_field_missing",
            "vid",
            "node vid is required",
        );
        roundtrip(&v);
    }

    #[test]
    fn roundtrip_violation_warning() {
        let v = crate::validator::Violation::warn(
            "type_mismatch",
            "embedding",
            "expected Vector(1024), got String",
        );
        roundtrip(&v);
    }
}
