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

mod common;

use common::mock_graph_store::MockGraphStore;

fn make_ingest(
    fail_on: Vec<String>,
) -> (IngestUseCase, MockGraphStore) {
    make_ingest_with_citations(fail_on, vec![])
}

fn make_ingest_with_citations(
    fail_on: Vec<String>,
    citations: Vec<da_ports::parser::CitationEntry>,
) -> (IngestUseCase, MockGraphStore) {
    let store = MockGraphStore::new();
    let ingest = IngestUseCase::new(
        Box::new(MockParser { fail_on, citations }),
        Box::new(MockEmbedder { dims: 1024, call_count: Arc::new(AtomicUsize::new(0)) }),
        Box::new(store.clone()),
    );
    (ingest, store)
}

#[tokio::test]
async fn test_batch_ingest_all_success() {
    let (ingest, store) = make_ingest(vec![]);
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
    assert_eq!(store.node_count_total(), 6); // 3 Paper + 3 Section nodes
    assert_eq!(store.snapshot_call_count(), 0); // no snapshot (None)
}

#[tokio::test]
async fn test_batch_ingest_partial_failure() {
    let (ingest, store) = make_ingest(vec!["2401.00002".to_string()]);
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
    assert_eq!(store.node_count_total(), 4); // 2 Paper + 2 Section (failed one didn't create)
}

#[tokio::test]
async fn test_batch_ingest_snapshot_export() {
    let (ingest, store) = make_ingest(vec![]);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];
    let tmp = tempfile::NamedTempFile::new().unwrap();

    let result = batch_ingest_pdfs(&ingest, &pdfs, Some(tmp.path()))
        .await
        .unwrap();

    assert_eq!(result.ok, 1);
    assert!(result.snapshot_path.is_some());
    assert_eq!(store.snapshot_call_count(), 1);
    // Snapshot file written
    let content = std::fs::read(tmp.path()).unwrap();
    assert_eq!(content, b"mock-snapshot-data");
}

#[tokio::test]
async fn test_batch_ingest_empty_list() {
    let (ingest, _store) = make_ingest(vec![]);
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
    let (ingest, _store) = make_ingest(vec![]);
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
    let (ingest, store) = make_ingest_with_citations(vec![], citations);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];

    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();

    assert_eq!(result.ok, 1);
    assert_eq!(result.total_citations, 3);
    assert_eq!(result.total_cites_resolved, 2); // only 2 have arxiv_id
    assert_eq!(result.total_sections, 1);
    // 1 Paper + 1 Section + 2 Citation + 3 Reference = 7 total
    // Reference nodes are created for ALL citations (full bibliography).
    assert_eq!(store.node_count_total(), 7);
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
    let (ingest, store) =
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
        store.node_count_total(),
        6,
        "expected dedup: 2 Paper + 2 Section + 1 Citation + 1 Reference = 6 nodes, got {}",
        store.node_count_total()
    );
}
