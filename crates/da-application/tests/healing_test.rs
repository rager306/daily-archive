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
    assert!(edges >= 3);
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
        2,
        "expected 2 contract violations (source + target), got {:?}",
        violations
    );
    assert!(violations
        .iter()
        .any(|v| v.reason.contains("source must be 'Paper'")));
    assert!(violations
        .iter()
        .any(|v| v.reason.contains("target must be one of")));
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
        .create_edge(
            p,
            e,
            da_domain::relation::bibliographic::MENTIONS,
        )
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
        .create_edge(
            p,
            rp,
            da_domain::relation::bibliographic::MENTIONS,
        )
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
        .set_node_property_string(env, "research_problem_id", "vid:problem:p1:improvement".to_string())
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
