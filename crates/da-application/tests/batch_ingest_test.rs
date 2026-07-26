//! Integration tests for batch_ingest using mock ports (no live services).
//!
//! Verifies orchestration: parse → embed → graph write → snapshot export,
//! error accumulation, and D127 import_eligible=false invariant.

#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use da_application::{batch_ingest_pdfs, ingest::IngestUseCase};
use da_ports::embedder::{EmbedResult, Embedder};
use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use da_ports::parser::{ParseResult, ParsedArticle, ParserPort};

// ---------- Mock Parser ----------

struct MockParser {
    fail_on: Vec<String>, // paper_ids that should fail
}

#[async_trait]
impl ParserPort for MockParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        if self.fail_on.iter().any(|f| f == paper_id) {
            return Err(da_ports::parser::ParserError::ParseFailed(format!(
                "forced failure for {paper_id}"
            )));
        }
        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title: format!("Title of {paper_id}"),
            abstract_text: format!("Abstract of {paper_id}"),
            body_text: "x".repeat(1000),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: format!("hash_{paper_id}"),
        })
    }
}

// ---------- Mock Embedder ----------

struct MockEmbedder {
    dims: usize,
    call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> EmbedResult<Vec<f32>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(vec![0.1; self.dims])
    }
    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.1; self.dims]).collect())
    }
    fn dimensions(&self) -> usize {
        self.dims
    }
    fn model_id(&self) -> &str {
        "mock-embedder"
    }
}

// ---------- Mock GraphStore ----------

struct MockGraphStore {
    nodes: Arc<AtomicUsize>,
    snapshot_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        self.snapshot_calls.fetch_add(1, Ordering::SeqCst);
        Ok(b"mock-snapshot-data".to_vec())
    }
    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, _label: &str) -> Result<u64, GraphStoreError> {
        Ok(self.nodes.fetch_add(1, Ordering::SeqCst) as u64)
    }
    async fn set_node_property_string(
        &self,
        _node_id: u64,
        _key: &str,
        _value: String,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_int(
        &self,
        _node_id: u64,
        _key: &str,
        _value: i64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        _node_id: u64,
        _key: &str,
        _value: bool,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn create_edge(
        &self,
        _source: u64,
        _target: u64,
        _edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        Ok(0)
    }
    async fn add_vector(
        &self,
        _label: &str,
        _property: &str,
        _node_id: u64,
        _vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _label: &str,
        _property: &str,
        _query: &[f32],
        _k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.nodes.load(Ordering::SeqCst)
    }
    async fn edge_count(&self) -> usize {
        0
    }
}

fn make_ingest(fail_on: Vec<String>) -> (IngestUseCase, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let nodes = Arc::new(AtomicUsize::new(0));
    let snapshot_calls = Arc::new(AtomicUsize::new(0));
    let embed_calls = Arc::new(AtomicUsize::new(0));
    let ingest = IngestUseCase::new(
        Box::new(MockParser { fail_on }),
        Box::new(MockEmbedder {
            dims: 1024,
            call_count: embed_calls.clone(),
        }),
        Box::new(MockGraphStore {
            nodes: nodes.clone(),
            snapshot_calls: snapshot_calls.clone(),
        }),
    );
    (ingest, nodes, snapshot_calls)
}

#[tokio::test]
async fn test_batch_ingest_all_success() {
    let (ingest, nodes, snapshot_calls) = make_ingest(vec![]);
    let pdfs = vec![
        ("paper1.pdf".to_string(), "2401.00001".to_string()),
        ("paper2.pdf".to_string(), "2401.00002".to_string()),
        ("paper3.pdf".to_string(), "2401.00003".to_string()),
    ];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.total, 3);
    assert_eq!(result.ok, 3);
    assert_eq!(result.fail, 0);
    assert_eq!(result.total_body_chars, 3000);
    assert!(result.errors.is_empty());
    assert!(!result.import_eligible); // D127
    assert_eq!(nodes.load(Ordering::SeqCst), 3); // 3 nodes created
    assert_eq!(snapshot_calls.load(Ordering::SeqCst), 0); // no snapshot (None)
}

#[tokio::test]
async fn test_batch_ingest_partial_failure() {
    let (ingest, nodes, _snapshot_calls) = make_ingest(vec!["2401.00002".to_string()]);
    let pdfs = vec![
        ("paper1.pdf".to_string(), "2401.00001".to_string()),
        ("paper2.pdf".to_string(), "2401.00002".to_string()), // will fail
        ("paper3.pdf".to_string(), "2401.00003".to_string()),
    ];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.total, 3);
    assert_eq!(result.ok, 2);
    assert_eq!(result.fail, 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].0, "2401.00002");
    assert!(result.errors[0].1.contains("forced failure"));
    assert_eq!(nodes.load(Ordering::SeqCst), 2); // only 2 nodes (failed one didn't create)
}

#[tokio::test]
async fn test_batch_ingest_snapshot_export() {
    let (ingest, _nodes, snapshot_calls) = make_ingest(vec![]);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];
    let tmp = tempfile::NamedTempFile::new().unwrap();

    let result = batch_ingest_pdfs(&ingest, &pdfs, Some(tmp.path()))
        .await
        .unwrap();

    assert_eq!(result.ok, 1);
    assert!(result.snapshot_path.is_some());
    assert_eq!(snapshot_calls.load(Ordering::SeqCst), 1);
    // Snapshot file written
    let content = std::fs::read(tmp.path()).unwrap();
    assert_eq!(content, b"mock-snapshot-data");
}

#[tokio::test]
async fn test_batch_ingest_empty_list() {
    let (ingest, _nodes, _snapshot_calls) = make_ingest(vec![]);
    let pdfs: Vec<(String, String)> = vec![];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.total, 0);
    assert_eq!(result.ok, 0);
    assert_eq!(result.fail, 0);
    assert!(!result.import_eligible); // D127 even on empty
}

#[tokio::test]
async fn test_batch_ingest_import_eligible_always_false() {
    // D127 invariant: import_eligible must never be true, regardless of success
    let (ingest, _nodes, _snapshot_calls) = make_ingest(vec![]);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert!(result.ok > 0);
    assert!(
        !result.import_eligible,
        "D127 violated: import_eligible must be false"
    );
}
