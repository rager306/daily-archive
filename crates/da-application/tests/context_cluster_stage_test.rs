use std::collections::BTreeMap;
use std::sync::Arc;

use da_application::{
    ClusterDetectionStage, ClusterUseCase, DocumentContextProjection, DocumentContextStage,
    EntityPapersArtifact, EntityPapersEntry, SectionSummaryInput,
};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, Section};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageExecutionError, StageExecutionRequest};

mod common;
use common::mock_graph_store::MockGraphStore;

fn article() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2507.19457".into(),
        title: "GEPA".into(),
        abstract_text: "abstract".into(),
        body_text: String::new(),
        sections: vec![
            Section {
                title: "Introduction".into(),
                text: "intro".into(),
                level: 1,
                path_id: Some("1".into()),
                parent_section_id: None,
                order_in_parent: Some(0),
                section_id: Some("sec-1".into()),
            },
            Section {
                title: "Method".into(),
                text: "method".into(),
                level: 1,
                path_id: Some("2".into()),
                parent_section_id: None,
                order_in_parent: Some(1),
                section_id: Some("sec-2".into()),
            },
        ],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
        pdf_hash: "a".repeat(64),
        ..Default::default()
    }
}

#[tokio::test]
async fn document_context_joins_summaries_and_navigation_by_path_identity() {
    let request = StageExecutionRequest {
        article_id: "2507.19457".into(),
        dependency_payloads: BTreeMap::from([
            ("parse".into(), serde_json::to_vec(&article()).unwrap()),
            (
                "section_summaries".into(),
                serde_json::to_vec(&vec![SectionSummaryInput {
                    path_id: "2".into(),
                    summary: "GEPA evolves prompts".into(),
                }])
                .unwrap(),
            ),
        ]),
    };
    let output = DocumentContextStage::new().execute(request).await.unwrap();
    let projection: DocumentContextProjection = serde_json::from_slice(&output.payload).unwrap();
    assert_eq!(output.metadata["section_count"], "2");
    assert_eq!(output.metadata["summary_count"], "1");
    assert_eq!(projection.sections[0].next_path_id.as_deref(), Some("2"));
    assert_eq!(
        projection.sections[1].previous_path_id.as_deref(),
        Some("1")
    );
    assert_eq!(
        projection.sections[1].summary.as_deref(),
        Some("GEPA evolves prompts")
    );
}

#[tokio::test]
async fn document_context_rejects_duplicate_and_unresolved_paths() {
    let mut duplicate = article();
    duplicate.sections[1].path_id = Some("1".into());
    let error = DocumentContextStage::new()
        .execute(StageExecutionRequest {
            article_id: duplicate.paper_id.clone(),
            dependency_payloads: BTreeMap::from([
                ("parse".into(), serde_json::to_vec(&duplicate).unwrap()),
                ("section_summaries".into(), b"[]".to_vec()),
            ]),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. } if error_class == "duplicate_section_path_id")
    );

    let error = DocumentContextStage::new()
        .execute(StageExecutionRequest {
            article_id: "2507.19457".into(),
            dependency_payloads: BTreeMap::from([
                ("parse".into(), serde_json::to_vec(&article()).unwrap()),
                (
                    "section_summaries".into(),
                    serde_json::to_vec(&vec![SectionSummaryInput {
                        path_id: "missing".into(),
                        summary: "orphan".into(),
                    }])
                    .unwrap(),
                ),
            ]),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. } if error_class == "unresolved_section_summary_path")
    );
}

#[tokio::test]
async fn cluster_stage_consumes_explicit_payload_and_replays_idempotently() {
    let store = MockGraphStore::new();
    let entity = store.create_node("Entity").await.unwrap();
    for (key, value) in [
        ("vid", "vid:gpt4"),
        ("label", "GPT-4"),
        ("entity_type", "Model"),
    ] {
        store
            .set_node_property_string(entity, key, value.into())
            .await
            .unwrap();
    }
    store
        .set_node_property_bool(entity, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(entity, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(entity, "schema_version", 1)
        .await
        .unwrap();
    let shared: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    let stage = ClusterDetectionStage::new();
    let use_case = ClusterUseCase::from_shared(shared);
    let artifact = EntityPapersArtifact {
        entries: vec![EntityPapersEntry {
            entity_type: "Model".into(),
            label: "  GPT-4  ".into(),
            paper_ids: (0..6).map(|index| format!("paper-{index}")).collect(),
        }],
    };
    let request = || StageExecutionRequest {
        article_id: "corpus".into(),
        dependency_payloads: BTreeMap::from([(
            "entity_papers".into(),
            serde_json::to_vec(&artifact).unwrap(),
        )]),
    };

    let first = stage.execute(request()).await.unwrap();
    let second = stage.execute(request()).await.unwrap();
    assert_eq!(
        first.payload, second.payload,
        "pure detection must be deterministic"
    );
    assert_eq!(first.metadata["cluster_count"], "1");
    assert_eq!(first.metadata["side_effects"], "none");
    assert!(store.get_nodes_by_label("ConceptCluster").await.is_empty());
    let detected: Vec<da_domain::cluster::DetectedCluster> =
        serde_json::from_slice(&first.payload).unwrap();
    let materialized = use_case
        .materialize_detected_clusters(&detected)
        .await
        .unwrap();
    let replay = use_case
        .materialize_detected_clusters(&detected)
        .await
        .unwrap();
    assert_eq!(materialized.clusters_created, 1);
    assert_eq!(materialized.member_edges_created, 1);
    assert_eq!(replay.clusters_created, 0);
    assert_eq!(replay.clusters_reused, 1);
    assert_eq!(replay.member_edges_created, 0);
    assert_eq!(first.metadata["import_eligible"], "false");
    store.assert_graph_conforms("cluster_stage_consumes_explicit_payload_and_replays_idempotently");
}
