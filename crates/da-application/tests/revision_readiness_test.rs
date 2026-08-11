//! ADR-051 contract suite for revision-scoped readiness signals.

use std::sync::Arc;

use da_application::revision_readiness::{
    AGENT_READINESS_SIGNAL_TYPE, RevisionReadinessError, RevisionReadinessPolicyBinding,
    RevisionReadinessUseCase,
};
use da_domain::relation::structure::{
    HAS_MANIFESTATION, HAS_PART, HAS_QUALITY_SIGNAL, HAS_SUMMARY,
};
use da_domain::revision_readiness::{RevisionReadinessPolicy, RevisionReadinessState};
use da_ports::{DirectGraphStore, FixedClock};
use kg_storage::MockGraphStore;

fn binding() -> RevisionReadinessPolicyBinding {
    RevisionReadinessPolicyBinding {
        policy: RevisionReadinessPolicy {
            schema_version: "revision-readiness.v1".into(),
            min_sections: 1,
            min_manifestations: 1,
            min_summary_coverage_bps: 8_000,
            expected_summary_types: vec!["methods".into(), "results".into()],
        },
        configuration_sha256: "b".repeat(64),
    }
}

async fn seed_revision(store: &MockGraphStore) -> (u64, String, Vec<u64>) {
    let revision_vid = "a".repeat(64);
    let revision = store.create_node("PaperRevision").await.unwrap();
    store
        .set_node_property_string(revision, "vid", revision_vid.clone())
        .await
        .unwrap();
    let manifestation = store.create_node("SourceManifestation").await.unwrap();
    store
        .set_node_property_string(manifestation, "vid", "manifestation-0".into())
        .await
        .unwrap();
    store
        .create_edge(revision, manifestation, HAS_MANIFESTATION)
        .await
        .unwrap();
    let mut sections = Vec::new();
    for ordinal in 0..2 {
        let section = store.create_node("Section").await.unwrap();
        store
            .set_node_property_string(section, "vid", format!("section-{ordinal}"))
            .await
            .unwrap();
        store
            .create_edge(revision, section, HAS_PART)
            .await
            .unwrap();
        sections.push(section);
    }
    (revision, revision_vid, sections)
}

async fn add_summary(store: &MockGraphStore, section: u64, revision_vid: &str, kind: &str) {
    let source_vid = store
        .get_node_property_string(section, "vid")
        .await
        .unwrap();
    let summary = store.create_node("Summary").await.unwrap();
    store
        .set_node_property_string(summary, "source_id", source_vid)
        .await
        .unwrap();
    store
        .set_node_property_string(summary, "revision_vid", revision_vid.into())
        .await
        .unwrap();
    store
        .set_node_property_string(summary, "summary_type", kind.into())
        .await
        .unwrap();
    store
        .create_edge(section, summary, HAS_SUMMARY)
        .await
        .unwrap();
}

#[tokio::test]
async fn materializes_partial_signal_with_exact_evidence_and_temporal_edge() {
    let store = Arc::new(MockGraphStore::new());
    let (revision, revision_vid, sections) = seed_revision(&store).await;
    add_summary(&store, sections[0], &revision_vid, "methods").await;
    add_summary(&store, sections[1], &revision_vid, "generic").await;
    let use_case = RevisionReadinessUseCase::with_clock(
        store.clone() as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
    .unwrap();
    let result = use_case.materialize(revision, &revision_vid).await.unwrap();
    assert_eq!(result.assessment.state, RevisionReadinessState::Partial);
    assert_eq!(result.assessment.summary_coverage_bps, 10_000);
    assert_eq!(
        result.assessment.diagnostic_codes,
        ["missing_results_summary"]
    );
    assert_eq!(result.observation.section_count, 2);
    assert_eq!(result.observation.grounded_summary_count, 2);
    assert_eq!(result.observation.summary_type_counts["methods"], 1);
    assert_eq!(result.observation.summary_type_counts["generic"], 1);
    assert_eq!(result.observation.manifestation_count, 1);
    assert!(result.edge_created);
    assert!(
        !store
            .get_node_property_bool(result.signal_node_id, "import_eligible")
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_node_property_string(result.signal_node_id, "signal_type")
            .await
            .as_deref(),
        Some(AGENT_READINESS_SIGNAL_TYPE)
    );
    let edges = store
        .get_edges_between(revision, result.signal_node_id, HAS_QUALITY_SIGNAL)
        .await;
    assert_eq!(edges.len(), 1);
    assert_eq!(
        store
            .get_edge_property_string(edges[0].0, "valid_at")
            .await
            .as_deref(),
        Some("2023-11-14T22:13:20+00:00")
    );
}

#[tokio::test]
async fn exact_replay_reuses_signal_and_edge() {
    let store = Arc::new(MockGraphStore::new());
    let (revision, revision_vid, sections) = seed_revision(&store).await;
    add_summary(&store, sections[0], &revision_vid, "methods").await;
    let use_case = RevisionReadinessUseCase::with_clock(
        store.clone() as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
    .unwrap();
    let first = use_case.materialize(revision, &revision_vid).await.unwrap();
    let replay = use_case.materialize(revision, &revision_vid).await.unwrap();
    assert_eq!(replay.signal_vid, first.signal_vid);
    assert_eq!(replay.evidence_sha256, first.evidence_sha256);
    assert!(!replay.edge_created);
    assert_eq!(replay.previous_signals_invalidated, 0);
    assert_eq!(store.get_nodes_by_label("QualitySignal").await.len(), 1);
}

#[tokio::test]
async fn evidence_change_creates_new_current_signal_and_closes_previous_fact() {
    let store = Arc::new(MockGraphStore::new());
    let (revision, revision_vid, sections) = seed_revision(&store).await;
    add_summary(&store, sections[0], &revision_vid, "methods").await;
    let first_use_case = RevisionReadinessUseCase::with_clock(
        store.clone() as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
    .unwrap();
    let first = first_use_case
        .materialize(revision, &revision_vid)
        .await
        .unwrap();
    add_summary(&store, sections[1], &revision_vid, "results").await;
    let second_use_case = RevisionReadinessUseCase::with_clock(
        store.clone() as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_100)),
    )
    .unwrap();
    let second = second_use_case
        .materialize(revision, &revision_vid)
        .await
        .unwrap();
    assert_ne!(second.signal_vid, first.signal_vid);
    assert_eq!(second.assessment.state, RevisionReadinessState::Ready);
    assert_eq!(second.previous_signals_invalidated, 1);
    assert_eq!(
        store
            .get_node_property_bool(first.signal_node_id, "is_current")
            .await,
        Some(false)
    );
    assert_eq!(
        store
            .get_node_property_bool(second.signal_node_id, "is_current")
            .await,
        Some(true)
    );
    let old_edge = store
        .get_edges_between(revision, first.signal_node_id, HAS_QUALITY_SIGNAL)
        .await[0]
        .0;
    assert_eq!(
        store
            .get_edge_property_string(old_edge, "invalid_at")
            .await
            .as_deref(),
        Some("2023-11-14T22:15:00+00:00")
    );
}

#[tokio::test]
async fn foreign_summary_is_excluded_and_zero_evidence_is_blocked() {
    let store = Arc::new(MockGraphStore::new());
    let (revision, revision_vid, sections) = seed_revision(&store).await;
    add_summary(&store, sections[0], &"c".repeat(64), "methods").await;
    let wrong_label = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_string(wrong_label, "vid", "not-a-section".into())
        .await
        .unwrap();
    store
        .create_edge(revision, wrong_label, HAS_PART)
        .await
        .unwrap();
    let use_case = RevisionReadinessUseCase::with_clock(
        store as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
    .unwrap();
    let result = use_case.materialize(revision, &revision_vid).await.unwrap();
    assert_eq!(result.observation.section_count, 2);
    assert_eq!(result.observation.grounded_summary_count, 0);
    assert_eq!(result.assessment.state, RevisionReadinessState::Partial);
    assert!(
        result
            .assessment
            .diagnostic_codes
            .contains(&"low_summary_coverage".into())
    );
}

#[tokio::test]
async fn missing_revision_and_invalid_policy_sha_fail_closed() {
    let store = Arc::new(MockGraphStore::new());
    let invalid = RevisionReadinessUseCase::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        RevisionReadinessPolicyBinding {
            configuration_sha256: "short".into(),
            ..binding()
        },
    );
    assert!(matches!(
        invalid,
        Err(RevisionReadinessError::InvalidPolicySha(_))
    ));
    let use_case = RevisionReadinessUseCase::with_clock(
        store as Arc<dyn DirectGraphStore>,
        binding(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
    .unwrap();
    assert!(matches!(
        use_case.materialize(999, &"a".repeat(64)).await,
        Err(RevisionReadinessError::MissingRevision(_))
    ));
}
