//! Integration tests for GraphHealingUseCase.

#![cfg(test)]

use da_application::GraphHealingUseCase;
use da_domain::healing::HealingActor;
use da_ports::graph_store::DirectGraphStore;

mod common;

use common::mock_graph_store::MockGraphStore;

fn make_store() -> MockGraphStore {
    MockGraphStore::new()
}

async fn make_entity(store: &MockGraphStore, vid: &str, label: &str) -> u64 {
    let node = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(node, "vid", vid.to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "label", label.to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "entity_type", "Method".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(node, "schema_version", 1)
        .await
        .unwrap();
    node
}

#[tokio::test]
async fn test_silence_sets_retrieval_eligible_false() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    let result = use_case
        .silence(
            "vid:entity:Method:GPT",
            "Entity",
            "wrong extraction",
            HealingActor::System("extractor".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.vid, "vid:entity:Method:GPT");
    assert!(result.previous_eligible);
    assert_eq!(
        result.provenance.operation,
        da_domain::healing::HealingOperation::Silence
    );
    store.assert_graph_conforms("test_silence_sets_retrieval_eligible_false");
}

#[tokio::test]
async fn test_merge_creates_supersedes_edge() {
    let store = make_store();
    let _keep_id = make_entity(&store, "vid:entity:Method:Transformer", "Transformer").await;
    let merge_id = make_entity(&store, "vid:entity:Method:Transformers", "Transformers").await;
    // Simulate a Paper mentioning the merge target
    let paper_id = store.create_node("Paper").await.unwrap();
    store
        .create_edge(
            paper_id,
            merge_id,
            da_domain::relation::bibliographic::MENTIONS,
        )
        .await
        .unwrap();
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    let result = use_case
        .merge(
            "vid:entity:Method:Transformer",
            "vid:entity:Method:Transformers",
            "same entity, different surface forms",
            HealingActor::Human("alice".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.kept_vid, "vid:entity:Method:Transformer");
    assert_eq!(result.merged_vid, "vid:entity:Method:Transformers");
    // SUPERSEDES edge + redirected MENTIONS edge = 2 new edges + original MENTIONS
    // total edges: 1 (original MENTIONS) + 1 (SUPERSEDES) + 1 (redirected MENTIONS) = 3
    assert_eq!(result.edges_redirected, 1);
    let edges = use_case.graph_store.edge_count().await;
    assert_eq!(edges, 3);
    assert_eq!(
        use_case
            .graph_store
            .get_node_property_string(merge_id, "canonical_vid")
            .await
            .as_deref(),
        Some("vid:entity:Method:Transformer")
    );

    let repeated = use_case
        .merge(
            "vid:entity:Method:Transformer",
            "vid:entity:Method:Transformers",
            "same reviewed resolution replayed",
            HealingActor::Human("alice".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(repeated.edges_redirected, 0);
    assert_eq!(use_case.graph_store.edge_count().await, 3);
}

#[tokio::test]
async fn test_merge_rejects_self_and_conflicting_canonical_target() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:A", "A").await;
    make_entity(&store, "vid:entity:Method:B", "B").await;
    make_entity(&store, "vid:entity:Method:C", "C").await;
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    assert!(
        use_case
            .merge(
                "vid:entity:Method:A",
                "vid:entity:Method:A",
                "invalid self merge",
                HealingActor::System("entity-resolution-test".into()),
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("into itself")
    );
    use_case
        .merge(
            "vid:entity:Method:A",
            "vid:entity:Method:B",
            "first reviewed resolution",
            HealingActor::System("entity-resolution-test".into()),
        )
        .await
        .unwrap();
    let error = use_case
        .merge(
            "vid:entity:Method:C",
            "vid:entity:Method:B",
            "conflicting resolution",
            HealingActor::System("entity-resolution-test".into()),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("different canonical VID"));
}

#[tokio::test]
async fn test_correct_updates_property() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    let result = use_case
        .correct(
            "vid:entity:Method:GPT",
            "Entity",
            "label",
            "GPT-4",
            "typo: should be GPT-4",
            HealingActor::Human("bob".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(result.key, "label");
    assert_eq!(result.old_value, "GPT"); // captured from graph, not "unknown"
    assert_eq!(result.new_value, "GPT-4");
    assert_eq!(
        result.provenance.operation,
        da_domain::healing::HealingOperation::Correct
    );
}

#[tokio::test]
async fn test_silence_not_found_errors() {
    let store = make_store();
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    let result = use_case
        .silence(
            "vid:entity:Method:Nonexistent",
            "Entity",
            "test",
            HealingActor::System("test".to_string()),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_unsilence_restores_retrieval() {
    let store = make_store();
    make_entity(&store, "vid:entity:Method:GPT", "GPT").await;
    let use_case = GraphHealingUseCase::new(Box::new(store.clone()));

    // First silence
    use_case
        .silence(
            "vid:entity:Method:GPT",
            "Entity",
            "deprecated",
            HealingActor::System("test".to_string()),
        )
        .await
        .unwrap();

    // Then unsilence
    let result = use_case
        .unsilence(
            "vid:entity:Method:GPT",
            "Entity",
            HealingActor::Human("admin".to_string()),
        )
        .await
        .unwrap();

    assert!(!result.previous_eligible); // was false before unsilence
}

#[tokio::test]
async fn test_edge_contract_validator_catches_wrong_source_label() {
    // Seed a deliberate contract violation: create an Entity node and a
    // ConceptCluster node, then wire HAS_PART (Paper-only source) between
    // them. The validator should flag the wrong source label.
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let e = store.create_node("Entity").await.unwrap();
    let c = store.create_node("ConceptCluster").await.unwrap();
    let _ = store
        .create_edge(e, c, da_domain::relation::structure::HAS_PART)
        .await
        .unwrap();

    let violations = store.validate_edge_contracts();
    assert_eq!(
        violations.len(),
        1,
        "expected one consolidated endpoint violation, got {:?}",
        violations
    );
    assert!(violations[0].reason.contains("Paper ->"));
    assert!(violations[0].reason.contains("Entity -> ConceptCluster"));
}

#[tokio::test]
async fn test_edge_contract_validator_accepts_valid_edges() {
    // Paper → Entity via MENTIONS: should be accepted (contract row
    // lists Entity as a valid target of MENTIONS).
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let p = store.create_node("Paper").await.unwrap();
    let e = store.create_node("Entity").await.unwrap();
    let _ = store
        .create_edge(p, e, da_domain::relation::bibliographic::MENTIONS)
        .await
        .unwrap();

    let violations = store.validate_edge_contracts();
    assert!(
        violations.is_empty(),
        "expected no violations for valid MENTIONS edge, got {:?}",
        violations
    );
}

#[tokio::test]
async fn test_edge_contract_validator_accepts_polymorphic_mentions() {
    // Paper → ResearchProblem via MENTIONS: should be accepted because
    // the MENTIONS contract row lists ResearchProblem as a valid target.
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let p = store.create_node("Paper").await.unwrap();
    let rp = store.create_node("ResearchProblem").await.unwrap();
    let _ = store
        .create_edge(p, rp, da_domain::relation::bibliographic::MENTIONS)
        .await
        .unwrap();

    let violations = store.validate_edge_contracts();
    assert!(
        violations.is_empty(),
        "expected no violations for MENTIONS → ResearchProblem, got {:?}",
        violations
    );
}

#[tokio::test]
async fn test_cross_ref_validator_flags_dangling_required_reference() {
    // Seed a MetricObservation node with run_id pointing to a non-existent
    // ExperimentRun. Validator must flag the dangling reference.
    use common::mock_graph_store::MockGraphStore;
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let m = store.create_node("MetricObservation").await.unwrap();
    store
        .set_node_property_string(m, "vid", "vid:obs:paper1:accuracy".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(m, "metric_definition_id", "vid:metric:accuracy".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(m, "run_id", "run:does-not-exist".to_string())
        .await
        .unwrap();
    store
        .set_node_property_float(m, "value", 0.95)
        .await
        .unwrap();

    let violations = store.validate_cross_references();
    assert!(
        violations
            .iter()
            .any(|v| v.field == "run_id" && v.target_label == "ExperimentRun"),
        "expected run_id dangling reference violation, got {:?}",
        violations
    );
}

#[tokio::test]
async fn test_cross_ref_validator_accepts_satisfied_reference() {
    // Seed a ResearchEnvironment node with research_problem_id pointing
    // to an existing ResearchProblem. Validator must accept it.
    use common::mock_graph_store::MockGraphStore;
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let rp = store.create_node("ResearchProblem").await.unwrap();
    store
        .set_node_property_string(rp, "vid", "vid:problem:p1:improvement".to_string())
        .await
        .unwrap();
    let env = store.create_node("ResearchEnvironment").await.unwrap();
    store
        .set_node_property_string(env, "vid", "vid:env:e1".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(
            env,
            "research_problem_id",
            "vid:problem:p1:improvement".to_string(),
        )
        .await
        .unwrap();

    let violations = store.validate_cross_references();
    let rp_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.source_label == "ResearchEnvironment")
        .collect();
    assert!(
        rp_violations.is_empty(),
        "expected no violations for satisfied reference, got {:?}",
        rp_violations
    );
}

#[tokio::test]
async fn test_cross_ref_validator_flags_missing_required_field() {
    // Seed a MetricObservation without run_id at all. Validator must
    // flag the missing required field.
    use common::mock_graph_store::MockGraphStore;
    use da_ports::graph_store::DirectGraphStore;
    let store = MockGraphStore::new();
    let m = store.create_node("MetricObservation").await.unwrap();
    store
        .set_node_property_string(m, "vid", "vid:obs:p2:f1".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(m, "metric_definition_id", "vid:metric:f1".to_string())
        .await
        .unwrap();
    // run_id missing

    let violations = store.validate_cross_references();
    assert!(
        violations
            .iter()
            .any(|v| v.field == "run_id" && v.source_node_id == m),
        "expected run_id missing-required violation, got {:?}",
        violations
    );
}

#[tokio::test]
async fn test_add_temporal_edge_invalidates_contradicted_old_edge() {
    use chrono::{DateTime, Utc};
    use common::mock_graph_store::MockGraphStore;
    use da_application::healing::GraphHealingUseCase;
    use da_ports::graph_store::DirectGraphStore;
    use kg_ontology::temporal::TemporalEdge;

    let store = MockGraphStore::new();
    let uc = GraphHealingUseCase {
        graph_store: Box::new(store.clone()),
        clock: std::sync::Arc::new(da_ports::SystemClock),
    };

    // Create two papers.
    let p1 = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(p1, "vid", "vid:paper:p1".to_string())
        .await
        .unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(p2, "vid", "vid:paper:p2".to_string())
        .await
        .unwrap();

    // Create an old temporal edge p1→p2 (valid from 2020).
    let old_edge = store.create_edge(p1, p2, "CITES").await.unwrap();
    store
        .set_edge_property_string(old_edge, "valid_at", "2020-01-01T00:00:00Z")
        .await
        .unwrap();
    store
        .set_edge_property_string(old_edge, "created_at", "2020-01-01T00:00:00Z")
        .await
        .unwrap();

    // Add a new temporal edge that supersedes the old one (valid from 2024).
    let new_temporal = TemporalEdge {
        valid_at: "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
        invalid_at: None,
        expired_at: None,
        reference_time: None,
        created_at: "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
    };

    let invalidated = uc
        .add_temporal_edge(
            "vid:paper:p1",
            "Paper",
            "vid:paper:p2",
            "Paper",
            "CITES",
            &new_temporal,
        )
        .await
        .expect("add_temporal_edge must succeed");

    // The old edge (valid from 2020) must be invalidated by the new one (valid from 2024).
    assert_eq!(
        invalidated, 1,
        "exactly 1 old edge must be invalidated (supersede rule)"
    );

    // Verify the old edge now has invalid_at set to new edge's valid_at.
    let old_invalid_at = store.get_edge_property_string(old_edge, "invalid_at").await;
    assert!(
        old_invalid_at.is_some(),
        "old edge must have invalid_at set after supersede"
    );
}

#[tokio::test]
async fn test_add_temporal_edge_retains_both_on_overlap() {
    use chrono::{DateTime, Utc};
    use common::mock_graph_store::MockGraphStore;
    use da_application::healing::GraphHealingUseCase;
    use da_ports::graph_store::DirectGraphStore;
    use kg_ontology::temporal::TemporalEdge;

    let store = MockGraphStore::new();
    let uc = GraphHealingUseCase {
        graph_store: Box::new(store.clone()),
        clock: std::sync::Arc::new(da_ports::SystemClock),
    };

    let p1 = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(p1, "vid", "vid:paper:p1".to_string())
        .await
        .unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(p2, "vid", "vid:paper:p2".to_string())
        .await
        .unwrap();

    // Old edge with same valid_at as new edge → retain both (rule 4).
    let old_edge = store.create_edge(p1, p2, "CITES").await.unwrap();
    store
        .set_edge_property_string(old_edge, "valid_at", "2024-01-01T00:00:00Z")
        .await
        .unwrap();
    store
        .set_edge_property_string(old_edge, "created_at", "2024-01-01T00:00:00Z")
        .await
        .unwrap();

    let new_temporal = TemporalEdge {
        valid_at: "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
        invalid_at: None,
        expired_at: None,
        reference_time: None,
        created_at: "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
    };

    let invalidated = uc
        .add_temporal_edge(
            "vid:paper:p1",
            "Paper",
            "vid:paper:p2",
            "Paper",
            "CITES",
            &new_temporal,
        )
        .await
        .unwrap();

    assert_eq!(
        invalidated, 0,
        "retain_both rule must not invalidate any edge"
    );
}
