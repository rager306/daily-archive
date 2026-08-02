//! Serde roundtrip tests (ADR-051 §4.4 Contour 2).
//!
//! Every type that crosses a persistence or API boundary must survive
//! a serialize → deserialize cycle without loss.

use serde::{Serialize, de::DeserializeOwned};

/// JSON roundtrip: serialize → deserialize → re-serialize must produce
/// identical bytes. This catches lossy conversions without requiring
/// PartialEq on the type.
fn assert_json_roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize must succeed");
    let back: T = serde_json::from_str(&json1).expect("deserialize must succeed");
    let json2 = serde_json::to_string(&back).expect("re-serialize must succeed");
    assert_eq!(
        json1, json2,
        "roundtrip must be lossless (json changed after cycle)"
    );
}

/// Bincode roundtrip: serialize → deserialize → re-serialize must
/// produce identical bytes. For types that also derive PartialEq.
fn assert_bincode_roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let bytes1 = bincode::serialize(value).expect("bincode serialize");
    let back: T = bincode::deserialize(&bytes1).expect("bincode deserialize");
    assert_eq!(value, &back, "bincode roundtrip must be lossless");
}

// ─── kg-storage: QueryResult ─────────────────────────────────────────

#[test]
fn test_query_result_roundtrip_empty() {
    use kg_storage::traits::QueryResult;
    let qr = QueryResult {
        columns: vec![],
        records: vec![],
    };
    assert_json_roundtrip(&qr);
}

#[test]
fn test_query_result_roundtrip_with_data() {
    use kg_storage::traits::QueryResult;
    let qr = QueryResult {
        columns: vec!["id".into(), "label".into()],
        records: vec![
            vec!["1".into(), "Paper".into()],
            vec!["2".into(), "Entity".into()],
        ],
    };
    assert_json_roundtrip(&qr);
}

// ─── kg-storage: VectorMetric ────────────────────────────────────────

#[test]
fn test_vector_metric_roundtrip_all_variants() {
    use kg_storage::traits::VectorMetric;
    for metric in [VectorMetric::Cosine, VectorMetric::L2, VectorMetric::Dot] {
        assert_json_roundtrip(&metric);
        assert_bincode_roundtrip(&metric);
    }
}

// ─── kg-storage: VectorSearchResult ──────────────────────────────────

#[test]
fn test_vector_search_result_roundtrip() {
    use kg_storage::traits::VectorSearchResult;
    let vsr = VectorSearchResult {
        vid: "2507.19457".into(),
        score: 0.95,
        properties: serde_json::json!({"label": "Paper"}),
    };
    assert_json_roundtrip(&vsr);
}

// ─── kg-ontology: HealingOperation ───────────────────────────────────

#[test]
fn test_healing_operation_roundtrip_all_variants() {
    use kg_ontology::healing::HealingOperation;
    for op in [
        HealingOperation::Correct,
        HealingOperation::Merge,
        HealingOperation::Split,
        HealingOperation::Silence,
        HealingOperation::Migrate,
        HealingOperation::Rollback,
    ] {
        assert_json_roundtrip(&op);
    }
}

// ─── kg-ontology: HealingActor ───────────────────────────────────────

#[test]
fn test_healing_actor_roundtrip_all_variants() {
    use kg_ontology::healing::HealingActor;
    for actor in [
        HealingActor::Human("alice".into()),
        HealingActor::Agent("claude".into()),
        HealingActor::System("scheduler".into()),
    ] {
        assert_json_roundtrip(&actor);
    }
}

// ─── kg-ontology: ProvenanceEvent ────────────────────────────────────

#[test]
fn test_provenance_event_roundtrip() {
    use kg_ontology::healing::{HealingActor, HealingOperation, ProvenanceEvent};
    let pe = ProvenanceEvent {
        operation: HealingOperation::Merge,
        actor: HealingActor::Agent("claude".into()),
        timestamp: 1725000000,
        affected_vids: vec!["2507.19457".into(), "2501.00001".into()],
        reason: "duplicate entity detected".into(),
        changes: vec![(
            "retrieval_eligible".into(),
            serde_json::json!(true),
            serde_json::json!(false),
        )],
    };
    assert_json_roundtrip(&pe);
}

// ─── kg-ontology: RetryPolicy ────────────────────────────────────────

#[test]
fn test_retry_policy_roundtrip() {
    use kg_ontology::retry::RetryPolicy;
    let rp = RetryPolicy {
        initial_delay: 30,
        backoff_multiplier: 2.0,
        max_delay: 600,
        max_retries: 5,
    };
    assert_json_roundtrip(&rp);
}

// ─── kg-ontology: YamlNodeSchema ─────────────────────────────────────

#[test]
fn test_yaml_node_schema_roundtrip() {
    use kg_ontology::registry::{YamlFieldDef, YamlNodeSchema};
    let schema = YamlNodeSchema {
        label: "Paper".into(),
        required_fields: vec![YamlFieldDef {
            name: "arxiv_id".into(),
            field_type: "string".into(),
        }],
        optional_fields: vec![YamlFieldDef {
            name: "doi".into(),
            field_type: "string".into(),
        }],
    };
    assert_json_roundtrip(&schema);
}

// ─── da-domain: Paper (tested in da-domain crate to respect ADR-050) ──
