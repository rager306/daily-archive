//! Semantic enrichment contracts for StructuredSummary graph writes.

use std::sync::Arc;

use async_trait::async_trait;
use da_application::SummaryUseCase;
use da_domain::vid;
use da_ports::graph_store::DirectGraphStore;
use da_ports::llm_client::{
    ChatMessage, ChatOptions, ChatResponse, LLMClient, LLMError, LLMResult,
};
use da_ports::parser::ParsedArticle;

mod common;
use common::mock_graph_store::MockGraphStore;

struct FixedSummaryLlm;

#[async_trait]
impl LLMClient for FixedSummaryLlm {
    async fn chat(
        &self,
        _messages: &[ChatMessage],
        _opts: &ChatOptions,
    ) -> LLMResult<ChatResponse> {
        Ok(ChatResponse {
            text: serde_json::json!({
                "abstract_summary": "GEPA optimizes textual components using reflection.",
                "methods": [{
                    "name": "GEPA",
                    "description": "Reflective prompt evolution",
                    "how_used": "Optimizes system prompts"
                }],
                "datasets": [{
                    "name": "ARC-Challenge",
                    "description": "Reasoning benchmark"
                }],
                "metrics": [{
                    "name": "Accuracy",
                    "value": "89.4%",
                    "baseline": "83.1%"
                }],
                "tags": ["prompt optimization"]
            })
            .to_string(),
            ok: true,
            error: None,
            tokens_in: Some(10),
            tokens_out: Some(20),
        })
    }

    async fn extract_structured(
        &self,
        _text: &str,
        _schema: &serde_json::Value,
    ) -> LLMResult<serde_json::Value> {
        Err(LLMError::LLM("unused".into()))
    }

    fn can_make_request(&self) -> bool {
        true
    }

    fn provider_id(&self) -> &str {
        "fixed-summary"
    }
}

async fn store_with_paper(arxiv_id: &str) -> (MockGraphStore, u64) {
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "vid", vid::paper_vid(arxiv_id))
        .await
        .unwrap();
    store
        .set_node_property_string(paper, "arxiv_id", arxiv_id.into())
        .await
        .unwrap();
    store
        .set_node_property_string(paper, "title", "GEPA".into())
        .await
        .unwrap();
    store
        .set_node_property_int(paper, "valid_from", 1)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(paper, "schema_version", 1)
        .await
        .unwrap();
    (store, paper)
}

fn parsed(arxiv_id: &str) -> ParsedArticle {
    ParsedArticle {
        paper_id: arxiv_id.into(),
        title: "GEPA".into(),
        abstract_text: "Abstract".into(),
        body_text: "GEPA uses ARC-Challenge and reports accuracy.".into(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        pdf_hash: "fixture-sha".into(),
    }
}

#[tokio::test]
async fn structured_summary_materializes_typed_nodes_with_d127_lock() {
    let (store, paper) = store_with_paper("2507.19457").await;
    let use_case = SummaryUseCase::new(Arc::new(store.clone()), Arc::new(FixedSummaryLlm));

    let result = use_case
        .generate_summaries("2507.19457", paper, &parsed("2507.19457"))
        .await
        .unwrap();

    assert_eq!(result.methods_count, 1);
    assert_eq!(result.datasets_count, 1);
    assert_eq!(result.metrics_count, 1);
    for (label, name) in [
        ("Method", "GEPA"),
        ("Dataset", "ARC-Challenge"),
        ("Metric", "Accuracy"),
    ] {
        let nodes = store.get_nodes_by_label(label).await;
        assert_eq!(nodes.len(), 1, "missing first-class {label}");
        let node = nodes[0];
        assert_eq!(
            store
                .get_node_property_string(node, "name")
                .await
                .as_deref(),
            Some(name)
        );
        assert_eq!(
            store.get_node_property_string(node, "vid").await,
            Some(vid::entity_vid(label, name))
        );
        assert_eq!(
            store
                .get_node_property_string(node, "paper_arxiv_id")
                .await
                .as_deref(),
            Some("2507.19457")
        );
        assert_eq!(
            store.get_node_property_int(node, "schema_version").await,
            Some(1)
        );
        let snap = store.snapshot_node(node).unwrap();
        assert_eq!(
            snap.get("import_eligible").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            snap.get("retrieval_eligible").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            store
                .get_edges_between(paper, node, da_domain::relation::bibliographic::MENTIONS,)
                .await
                .len(),
            1
        );
    }

    let metric = store.get_nodes_by_label("Metric").await[0];
    assert_eq!(
        store
            .get_node_property_string(metric, "value")
            .await
            .as_deref(),
        Some("89.4%")
    );
    assert_eq!(
        store
            .get_node_property_string(metric, "baseline_for")
            .await
            .as_deref(),
        Some("83.1%")
    );
    store.assert_graph_conforms("structured_summary_materializes_typed_nodes_with_d127_lock");
}

#[tokio::test]
async fn repeated_summary_is_idempotent_for_typed_nodes_and_edges() {
    let (store, paper) = store_with_paper("2507.19457").await;
    let use_case = SummaryUseCase::new(Arc::new(store.clone()), Arc::new(FixedSummaryLlm));

    use_case
        .generate_summaries("2507.19457", paper, &parsed("2507.19457"))
        .await
        .unwrap();
    use_case
        .generate_summaries("2507.19457", paper, &parsed("2507.19457"))
        .await
        .unwrap();

    assert_eq!(store.get_nodes_by_label("Method").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Dataset").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Metric").await.len(), 1);
    for label in ["Method", "Dataset", "Metric"] {
        let node = store.get_nodes_by_label(label).await[0];
        assert_eq!(
            store
                .get_edges_between(paper, node, da_domain::relation::bibliographic::MENTIONS,)
                .await
                .len(),
            1,
            "duplicate MENTIONS edge for {label}"
        );
    }
}
