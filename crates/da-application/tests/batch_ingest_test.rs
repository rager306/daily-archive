//! Integration tests for batch_ingest using mock ports (no live services).
//!
//! Verifies orchestration: parse → embed → graph write → snapshot export,
//! error accumulation, and D127 import_eligible=false invariant.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    fail_on: Vec<String>,                            // paper_ids that should fail
    citations: Vec<da_ports::parser::CitationEntry>, // citations to include
}

#[async_trait]
impl ParserPort for MockParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        use da_ports::parser::Section;
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
            sections: vec![Section {
                title: "Introduction".to_string(),
                text: "intro text".to_string(),
                level: 1,
            }],
            citations: self.citations.clone(),
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
    // Track string properties for find_node_by_string_property testing
    props: std::sync::Mutex<std::collections::HashMap<(u64, String), String>>,
    // Track node labels for find_node_by_string_property label filtering
    // (real SamyamaGraphStore filters by label; mock must match that contract).
    labels: std::sync::Mutex<std::collections::HashMap<u64, String>>,
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
    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
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
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let id = self.nodes.fetch_add(1, Ordering::SeqCst) as u64;
        self.labels.lock().unwrap().insert(id, label.to_string());
        Ok(id)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .insert((node_id, key.to_string()), value);
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
    async fn set_node_property_float(
        &self,
        _node_id: u64,
        _key: &str,
        _value: f64,
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
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        let props = self.props.lock().unwrap();
        let labels = self.labels.lock().unwrap();
        for ((node_id, k), v) in props.iter() {
            if k == key && v == value {
                // Verify the node label matches (real SamyamaGraphStore filters by label).
                if labels.get(node_id).map(|s| s.as_str()) == Some(label) {
                    return Some(*node_id);
                }
            }
        }
        None
    }
    async fn get_incoming_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_node_property_string(&self, _node_id: u64, _key: &str) -> Option<String> {
        None
    }
    async fn get_node_property_int(&self, _node_id: u64, _key: &str) -> Option<i64> {
        None
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.labels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l.as_str() == label)
            .map(|(id, _)| *id)
            .collect()
    }
}

fn make_ingest(fail_on: Vec<String>) -> (IngestUseCase, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    make_ingest_with_citations(fail_on, vec![])
}

fn make_ingest_with_citations(
    fail_on: Vec<String>,
    citations: Vec<da_ports::parser::CitationEntry>,
) -> (IngestUseCase, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let nodes = Arc::new(AtomicUsize::new(0));
    let snapshot_calls = Arc::new(AtomicUsize::new(0));
    let embed_calls = Arc::new(AtomicUsize::new(0));
    let ingest = IngestUseCase::new(
        Box::new(MockParser { fail_on, citations }),
        Box::new(MockEmbedder {
            dims: 1024,
            call_count: embed_calls.clone(),
        }),
        Box::new(MockGraphStore {
            nodes: nodes.clone(),
            snapshot_calls: snapshot_calls.clone(),
            props: std::sync::Mutex::new(std::collections::HashMap::new()),
            labels: std::sync::Mutex::new(std::collections::HashMap::new()),
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
    assert_eq!(result.total_sections, 3); // 1 section per paper
    assert_eq!(result.total_citations, 0);
    assert_eq!(result.total_cites_resolved, 0);
    assert!(result.errors.is_empty());
    assert!(!result.import_eligible); // D127
    assert_eq!(nodes.load(Ordering::SeqCst), 6); // 3 Paper + 3 Section nodes
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
    assert_eq!(nodes.load(Ordering::SeqCst), 4); // 2 Paper + 2 Section (failed one didn't create)
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

#[tokio::test]
async fn test_ingest_citations_create_cites_edges() {
    // Citations with arxiv_id should create Citation nodes + CITES edges
    use da_ports::parser::CitationEntry;
    let citations = vec![
        CitationEntry {
            raw_text: "Smith et al 2023".to_string(),
            doi: Some("10.1000/a".to_string()),
            arxiv_id: Some("2301.00001".to_string()),
            title: Some("Cited Paper A".to_string()),
        },
        CitationEntry {
            raw_text: "Jones 2022".to_string(),
            doi: None,
            arxiv_id: Some("2201.00002".to_string()),
            title: Some("Cited Paper B".to_string()),
        },
        CitationEntry {
            raw_text: "No id ref".to_string(),
            doi: None,
            arxiv_id: None, // no arxiv_id — should NOT create a Citation node
            title: None,
        },
    ];
    let (ingest, nodes, _snapshot_calls) = make_ingest_with_citations(vec![], citations);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.ok, 1);
    assert_eq!(result.total_citations, 3);
    assert_eq!(result.total_cites_resolved, 2); // only 2 have arxiv_id
    assert_eq!(result.total_sections, 1);
    // 1 Paper + 1 Section + 2 Citation + 3 Reference = 7 total
    // Reference nodes are created for ALL citations (full bibliography).
    assert_eq!(nodes.load(Ordering::SeqCst), 7);
}

#[tokio::test]
async fn test_ingest_citation_dedup_shared_reference() {
    // Two papers citing the same arxiv_id should create ONE Citation node
    // with TWO CITES edges (idempotent creation).
    use da_ports::parser::CitationEntry;
    let shared_citation = CitationEntry {
        raw_text: "Shared ref".to_string(),
        doi: None,
        arxiv_id: Some("2301.09999".to_string()),
        title: Some("Shared Paper".to_string()),
    };
    let (ingest, nodes, _snapshot_calls) =
        make_ingest_with_citations(vec![], vec![shared_citation.clone()]);
    let pdfs = vec![
        ("paper1.pdf".to_string(), "2401.00001".to_string()),
        ("paper2.pdf".to_string(), "2401.00002".to_string()),
    ];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.ok, 2);
    assert_eq!(result.total_cites_resolved, 2); // both papers cite it
    // 2 Paper + 2 Section + 1 Citation (deduped) + 1 Reference (deduped) = 6 total
    assert_eq!(
        nodes.load(Ordering::SeqCst),
        6,
        "expected dedup: 2 Paper + 2 Section + 1 Citation + 1 Reference = 6 nodes, got {}",
        nodes.load(Ordering::SeqCst)
    );
}
