use da_application::{GraphFinalizeUseCase, PipelineRunReport, StageDisposition, StageRunRecord};
use da_domain::finalize::{FinalizePolicy, FinalizeVerdict};
use da_domain::graph_tasks::{
    ClassifiedTaskEvidence, GraphTaskClass, TaskClassResult, TaskClassStatus,
};
use da_ports::graph_store::DirectGraphStore;
use kg_storage::MockGraphStore;

fn pipeline(pass: bool) -> PipelineRunReport {
    PipelineRunReport {
        pipeline_id: "semantic-v1".into(),
        article_id: "2507.19457".into(),
        completed: pass,
        stages: [
            "semantic_enrichment",
            "entity_resolution",
            "document_context",
            "cluster_detection",
        ]
        .into_iter()
        .map(|stage| StageRunRecord {
            step_name: stage.into(),
            stage_id: stage.into(),
            resolved_from: None,
            cache_key_sha256: None,
            disposition: if pass {
                StageDisposition::Executed {
                    output_sha256: format!("sha256:{stage}"),
                }
            } else {
                StageDisposition::Failed {
                    error_class: "fixture".into(),
                    message: "failed".into(),
                    retryable: false,
                }
            },
        })
        .collect(),
    }
}

fn cache_hit_pipeline() -> PipelineRunReport {
    let mut report = pipeline(true);
    for stage in &mut report.stages {
        stage.disposition = StageDisposition::CacheHit {
            output_sha256: format!("sha256:{}", "b".repeat(64)),
        };
    }
    report
}

async fn valid_paper(store: &MockGraphStore, import_eligible: bool) -> u64 {
    let paper = store.create_node("Paper").await.unwrap();
    for (key, value) in [
        ("vid", da_domain::vid::paper_vid("2507.19457")),
        ("arxiv_id", "2507.19457".into()),
        ("title", "GEPA".into()),
    ] {
        store
            .set_node_property_string(paper, key, value)
            .await
            .unwrap();
    }
    store
        .set_node_property_int(paper, "valid_from", 1)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "import_eligible", import_eligible)
        .await
        .unwrap();
    store
        .set_node_property_int(paper, "schema_version", 1)
        .await
        .unwrap();
    paper
}

#[tokio::test]
async fn valid_locked_graph_and_required_stages_pass_without_import_unlock() {
    let store = MockGraphStore::new();
    valid_paper(&store, false).await;

    let report = GraphFinalizeUseCase::load_project()
        .unwrap()
        .finalize(
            &store,
            &pipeline(true),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;

    assert_eq!(report.verdict, FinalizeVerdict::Pass);
    assert!(report.snapshot_eligible);
    assert!(!report.import_eligible);
    assert_eq!(report.total_nodes, 1);
    assert_eq!(report.validated_nodes, 1);
    assert_eq!(report.schema_violations, 0);
    assert_eq!(report.import_eligible_true, 0);
    assert!(report.policy_fingerprint.starts_with("sha256:"));
}

#[tokio::test]
async fn d127_unlock_fails_finalize_and_snapshot_eligibility() {
    let store = MockGraphStore::new();
    valid_paper(&store, true).await;

    let report = GraphFinalizeUseCase::load_project()
        .unwrap()
        .finalize(
            &store,
            &pipeline(true),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;

    assert_eq!(report.verdict, FinalizeVerdict::Fail);
    assert!(!report.snapshot_eligible);
    assert_eq!(report.import_eligible_true, 1);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "d127_import_unlock")
    );
}

#[tokio::test]
async fn semantic_evidence_and_registered_labels_are_required() {
    let store = MockGraphStore::new();
    valid_paper(&store, false).await;
    store.create_node("FreeInventLabel").await.unwrap();

    let report = GraphFinalizeUseCase::load_project()
        .unwrap()
        .finalize(
            &store,
            &pipeline(true),
            ClassifiedTaskEvidence::overall_only(false),
        )
        .await;

    assert_eq!(report.verdict, FinalizeVerdict::Fail);
    assert!(!report.snapshot_eligible);
    assert!(!report.semantic_tasks_passed);
    assert_eq!(report.unregistered_node_labels.len(), 1);
    assert!(report.unregistered_node_labels[0].starts_with("FreeInventLabel:"));
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "semantic_tasks_failed_or_missing")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "unregistered_node_labels")
    );
}

#[tokio::test]
async fn injected_constructor_validates_policy_registry_and_fingerprint() {
    let (policy, fingerprint) = FinalizePolicy::load_project().unwrap();
    let registry = kg_ontology::OntologyRegistry::load_project_meta_ontology().unwrap();
    assert!(GraphFinalizeUseCase::new(policy.clone(), fingerprint, registry).is_ok());
    let registry = kg_ontology::OntologyRegistry::load_project_meta_ontology().unwrap();
    assert!(GraphFinalizeUseCase::new(policy, "bad".into(), registry).is_err());
}

#[tokio::test]
async fn cache_hit_stages_pass_and_missing_required_label_is_explicit() {
    let store = MockGraphStore::new();
    valid_paper(&store, false).await;
    let report = GraphFinalizeUseCase::load_project()
        .unwrap()
        .finalize(
            &store,
            &cache_hit_pipeline(),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;
    assert_eq!(report.verdict, FinalizeVerdict::Pass);
    assert!(report.stage_outcomes.values().all(|passed| *passed));

    let empty = MockGraphStore::new();
    let report = GraphFinalizeUseCase::load_project()
        .unwrap()
        .finalize(
            &empty,
            &cache_hit_pipeline(),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;
    assert!(
        report.findings.iter().any(|finding| {
            finding.code == "required_label_empty" && finding.evidence == "Paper"
        })
    );
}

#[tokio::test]
async fn schema_unknown_edge_empty_graph_and_missing_stage_fail_closed() {
    let finalizer = GraphFinalizeUseCase::load_project().unwrap();
    let empty = MockGraphStore::new();
    let report = finalizer
        .finalize(
            &empty,
            &pipeline(true),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;
    assert_eq!(report.verdict, FinalizeVerdict::Fail);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "empty_graph")
    );

    let store = MockGraphStore::new();
    let paper = valid_paper(&store, false).await;
    let broken = store.create_node("Paper").await.unwrap();
    store
        .create_edge(paper, broken, "UNKNOWN_WIRE")
        .await
        .unwrap();
    let report = finalizer
        .finalize(
            &store,
            &pipeline(false),
            ClassifiedTaskEvidence::overall_only(true),
        )
        .await;
    assert_eq!(report.verdict, FinalizeVerdict::Fail);
    assert!(report.schema_violations > 0);
    assert_eq!(report.unknown_edge_types, vec!["UNKNOWN_WIRE"]);
    assert!(report.findings.iter().any(|finding| {
        finding.code == "required_stage_not_passed" && finding.evidence == "entity_resolution"
    }));
}

#[tokio::test]
async fn required_task_class_gate_fails_when_summary_absent() {
    let (mut policy, fingerprint) = FinalizePolicy::load_project().unwrap();
    policy.required_task_classes = vec!["structural".into(), "summary".into()];
    let registry = kg_ontology::OntologyRegistry::load_project_meta_ontology().unwrap();
    let finalizer = GraphFinalizeUseCase::new(policy, fingerprint, registry).unwrap();
    let store = MockGraphStore::new();
    valid_paper(&store, false).await;

    let mut by_class = std::collections::BTreeMap::new();
    by_class.insert(
        GraphTaskClass::Structural,
        TaskClassResult {
            class: GraphTaskClass::Structural,
            status: TaskClassStatus::Passed,
            task_count: 1,
            pass: 1,
            empty: 0,
            error: 0,
        },
    );
    // overall_passed true, but required summary class absent → Fail without collapsing
    // structural success into a single boolean green light.
    let evidence = ClassifiedTaskEvidence {
        overall_passed: true,
        by_class,
        required_classes: vec![GraphTaskClass::Structural, GraphTaskClass::Summary],
    };
    let report = finalizer.finalize(&store, &pipeline(true), evidence).await;
    assert_eq!(report.verdict, FinalizeVerdict::Fail);
    assert!(report.findings.iter().any(|finding| {
        finding.code == "required_task_class_failed_or_missing"
            && finding.evidence.contains("summary")
    }));
    assert_eq!(
        report
            .task_class_results
            .get("structural")
            .map(String::as_str),
        Some("passed")
    );
    assert_eq!(
        report.task_class_results.get("summary").map(String::as_str),
        Some("absent")
    );
    assert_eq!(
        report.required_task_classes,
        vec!["structural".to_string(), "summary".to_string()]
    );
}
