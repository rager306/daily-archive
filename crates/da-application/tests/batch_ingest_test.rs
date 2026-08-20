//! Integration tests for batch_ingest using mock ports (no live services).
//!
//! Verifies orchestration: parse → embed → graph write → snapshot export,
//! error accumulation, and D127 import_eligible=false invariant.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use async_trait::async_trait;
use da_application::{
    MaterializationAssuranceUseCase, MaterializationReplayObservation, batch_ingest_pdfs,
    ingest::IngestUseCase,
};
use da_domain::{AssuranceVerdict, ContentDigest, ReplayObservation};
use da_ports::clock::Clock;
use da_ports::embedder::{EmbedResult, Embedder};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParseResult, ParsedArticle, ParserPort};
use kg_storage::InstrumentedGraphStore;

#[derive(Debug)]
struct AdvancingClock(AtomicI64);

impl Clock for AdvancingClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        let seconds = self.0.fetch_add(60, Ordering::SeqCst);
        chrono::DateTime::from_timestamp(seconds, 0).expect("valid test timestamp")
    }
}

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
                ..Default::default()
            }],
            citations: self.citations.clone(),
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
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

fn make_ingest(fail_on: Vec<String>) -> (IngestUseCase, MockGraphStore) {
    make_ingest_with_citations(fail_on, vec![])
}

fn make_ingest_with_citations(
    fail_on: Vec<String>,
    citations: Vec<da_ports::parser::CitationEntry>,
) -> (IngestUseCase, MockGraphStore) {
    let store = MockGraphStore::new();
    let ingest = IngestUseCase::new(
        Box::new(MockParser { fail_on, citations }),
        Box::new(MockEmbedder {
            dims: 1024,
            call_count: Arc::new(AtomicUsize::new(0)),
        }),
        Box::new(store.clone()),
    );
    (ingest, store)
}

#[tokio::test]
async fn materializer_only_ingest_has_no_fake_compute_ports() {
    let store = MockGraphStore::new();
    let ingest = IngestUseCase::materializer_only(Arc::new(store.clone()));
    let error = ingest
        .ingest_pdf("missing.pdf", "2401.00001")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("materializer-only"));

    let parsed = ParsedArticle {
        paper_id: "2401.00001".into(),
        title: "Cached paper".into(),
        abstract_text: String::new(),
        body_text: String::new(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-hash".into(),
    };
    ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    assert_eq!(store.get_nodes_by_label("Paper").await.len(), 1);
}

#[tokio::test]
async fn cached_parsed_materialization_replay_is_idempotent() {
    let (ingest, store) = make_ingest(vec![]);
    let parsed = ParsedArticle {
        paper_id: "2401.00001".into(),
        title: "Cached paper".into(),
        abstract_text: "Cached abstract".into(),
        body_text: "Body".into(),
        sections: vec![da_ports::parser::Section {
            title: "Introduction".into(),
            text: "Intro".into(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-hash".into(),
    };

    ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();

    assert_eq!(store.get_nodes_by_label("Paper").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Section").await.len(), 1);
    assert_eq!(store.node_count_total(), 2);
    assert_eq!(store.vector_add_call_count(), 1);
}

#[tokio::test]
async fn identical_materialization_replay_has_no_physical_writes() {
    let inner = MockGraphStore::new();
    let instrumented = InstrumentedGraphStore::new(Arc::new(inner.clone()));
    let graph: Arc<dyn DirectGraphStore> = Arc::new(instrumented.clone());
    let ingest = IngestUseCase::materializer_only_with_clock(
        graph,
        Arc::new(AdvancingClock(AtomicI64::new(1_700_000_000))),
    );
    let parsed = ParsedArticle {
        paper_id: "2401.00010".into(),
        title: "Replay evidence".into(),
        abstract_text: "ABSTRACT-尾".into(),
        body_text: "Body TAIL-科学".into(),
        sections: vec![da_ports::parser::Section {
            title: "Body".into(),
            text: "Body TAIL-科学".into(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![da_ports::parser::CitationEntry {
            raw_text: "Stable reference".into(),
            doi: Some("10.1000/stable".into()),
            arxiv_id: None,
            title: Some("Stable reference".into()),
            anchors: vec![],
        }],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-replay-hash".into(),
    };

    let first = ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    assert!(instrumented.reset().await.applied() > 0);
    let replay = ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    let replay_mutations = instrumented.snapshot().await;
    assert!(replay_mutations.partition_is_valid());
    assert!(replay_mutations.attempted() > 0);
    assert_eq!(
        replay_mutations.applied(),
        0,
        "identical replay must not change graph state"
    );
    assert_eq!(replay_mutations.rejected(), 0);
    assert!(replay_mutations.unchanged() > 0);

    let assurance = MaterializationAssuranceUseCase::new()
        .evaluate_graph(
            &instrumented,
            &parsed,
            &replay,
            MaterializationReplayObservation::from_mutations(replay_mutations),
            ContentDigest::of_text("test-policy"),
        )
        .await
        .unwrap();
    assert_eq!(assurance.verdict, AssuranceVerdict::Pass);
    assert_eq!(inner.get_nodes_by_label("Paper").await.len(), 1);
    assert_eq!(first.graph_node_id, replay.graph_node_id);
}

#[tokio::test]
async fn parsed_materialization_preserves_full_unicode_section_text_and_checksum() {
    use sha2::{Digest, Sha256};
    let (ingest, store) = make_ingest(vec![]);
    let full_text = format!("{}TAIL-科学", "αβγ".repeat(4_000));
    let full_abstract = format!("{}ABSTRACT-尾", "résumé ".repeat(2_000));
    let parsed = ParsedArticle {
        paper_id: "2401.00009".into(),
        title: "Long section".into(),
        abstract_text: full_abstract.clone(),
        body_text: full_text.clone(),
        sections: vec![da_ports::parser::Section {
            title: "Complete evidence".into(),
            text: full_text.clone(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-hash-long".into(),
    };
    let ingest_result = ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    let paper = store.get_nodes_by_label("Paper").await[0];
    assert_eq!(
        store
            .get_node_property_string(paper, "abstract_text")
            .await
            .as_deref(),
        Some(full_abstract.as_str())
    );
    let expected_abstract_sha256 = hex::encode(Sha256::digest(full_abstract.as_bytes()));
    assert_eq!(
        store
            .get_node_property_string(paper, "abstract_sha256")
            .await
            .as_deref(),
        Some(expected_abstract_sha256.as_str())
    );
    let section = store.get_nodes_by_label("Section").await[0];
    assert_eq!(
        store
            .get_node_property_string(section, "text")
            .await
            .as_deref(),
        Some(full_text.as_str())
    );
    assert_eq!(
        store.get_node_property_int(section, "char_count").await,
        Some(full_text.chars().count() as i64)
    );
    let expected_sha256 = hex::encode(Sha256::digest(full_text.as_bytes()));
    assert_eq!(
        store
            .get_node_property_string(section, "text_sha256")
            .await
            .as_deref(),
        Some(expected_sha256.as_str())
    );

    // ADR-055 Contour 3/4: checker obtains sink values from the real graph
    // port rather than receiving a synthetic copy of the expected values.
    let assurance = MaterializationAssuranceUseCase::new()
        .evaluate_graph(
            &store,
            &parsed,
            &ingest_result,
            MaterializationReplayObservation::Observed(
                ReplayObservation::valid_unchanged_property(1),
            ),
            ContentDigest::of_text("test-policy"),
        )
        .await
        .unwrap();
    assert_eq!(assurance.verdict, AssuranceVerdict::Pass);
    assert!(assurance.layer_results[0].hard_failures.is_empty());

    // D127 scan is graph-wide: an unlock on a non-Paper/Section label must not
    // evade the materialization assurance gate.
    let rogue = store.create_node("Entity").await.unwrap();
    store
        .set_node_property_bool(rogue, "import_eligible", true)
        .await
        .unwrap();
    let blocked = MaterializationAssuranceUseCase::new()
        .evaluate_graph(
            &store,
            &parsed,
            &ingest_result,
            MaterializationReplayObservation::Observed(
                ReplayObservation::valid_unchanged_property(1),
            ),
            ContentDigest::of_text("test-policy"),
        )
        .await
        .unwrap();
    assert_eq!(blocked.verdict, AssuranceVerdict::Fail);
    assert!(
        blocked.layer_results[0]
            .hard_failures
            .contains(&"d127_unlock_count:1".into())
    );
}

#[tokio::test]
async fn changed_embedding_replaces_via_sanctioned_upsert() {
    let (ingest, store) = make_ingest(vec![]);
    let parsed = ParsedArticle {
        paper_id: "2401.00001".into(),
        title: "Cached paper".into(),
        abstract_text: String::new(),
        body_text: String::new(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-hash".into(),
    };
    ingest
        .materialize_parsed(&parsed, vec![0.1; 1024], None, None, None)
        .await
        .unwrap();
    // M416 delivered the storage upsert (replace_vector, D277): a changed
    // embedder replaces the content-node vector in place instead of failing.
    ingest
        .materialize_parsed(&parsed, vec![0.2; 1024], None, None, None)
        .await
        .expect("changed embedding replaces instead of bailing");
    // The second write path went through the sanctioned replace (counted by
    // the instrumented mock: add on first attach, replace on the change).
    assert_eq!(store.vector_add_call_count(), 2);
    // Equal-sha replay is a no-op: a third materialize with the same vector
    // performs no further add/replace.
    ingest
        .materialize_parsed(&parsed, vec![0.2; 1024], None, None, None)
        .await
        .unwrap();
    assert_eq!(store.vector_add_call_count(), 2);
}

#[tokio::test]
async fn cached_catalog_provenance_replay_is_idempotent() {
    let (ingest, store) = make_ingest(vec![]);
    let parsed = ParsedArticle {
        paper_id: "2401.00001".into(),
        title: "Cached paper".into(),
        abstract_text: String::new(),
        body_text: String::new(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "cached-hash".into(),
    };

    for _ in 0..2 {
        ingest
            .materialize_parsed(
                &parsed,
                vec![0.1; 1024],
                Some("cs-cl".into()),
                Some("arxiv".into()),
                None,
            )
            .await
            .unwrap();
    }

    assert_eq!(store.get_nodes_by_label("Paper").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Source").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Category").await.len(), 1);
    assert_eq!(store.node_count_total(), 3);
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
    // M352: no classifier attached → empty triage aggregate
    assert_eq!(result.pdf_triage.classified, 0);
    assert!(!result.pdf_triage.any());
    assert_eq!(store.node_count_total(), 6); // 3 Paper + 3 Section (Summary deferred — needs real summarization)
    assert_eq!(store.snapshot_call_count(), 0); // no snapshot (None)
    store.assert_graph_conforms("test_batch_ingest_all_success");
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
    assert_eq!(store.node_count_total(), 4); // 2 Paper + 2 Section (Summary deferred)
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
    // Export must write non-empty bytes; MockGraphStore now serializes real
    // state as JSON (M306) rather than opaque canned "mock-snapshot-data".
    let content = std::fs::read(tmp.path()).unwrap();
    assert!(!content.is_empty(), "export payload must be non-empty");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&content).is_ok(),
        "default Mock export must be valid JSON state dump"
    );
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
            anchors: vec![],
        },
        CitationEntry {
            raw_text: "Jones 2022".to_string(),
            doi: None,
            arxiv_id: Some("2201.00002".to_string()),
            title: Some("Cited Paper B".to_string()),
            anchors: vec![],
        },
        CitationEntry {
            raw_text: "No id ref".to_string(),
            doi: None,
            arxiv_id: None, // no arxiv_id — should NOT create a Citation node
            title: None,
            anchors: vec![],
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
    // Summary deferred — needs real summarization, not abstract_copy.
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
        anchors: vec![],
    };
    let (ingest, store) = make_ingest_with_citations(vec![], vec![shared_citation.clone()]);
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
    let citation = store.get_nodes_by_label("Citation").await[0];
    let citation_vid = store
        .get_node_property_string(citation, "vid")
        .await
        .unwrap();
    assert_eq!(
        citation_vid,
        da_domain::vid::citation_arxiv_vid("2301.09999").unwrap()
    );
    assert_ne!(citation_vid, da_domain::vid::paper_vid("2301.09999"));
}

#[tokio::test]
async fn invalid_arxiv_citation_remains_reference_only() {
    use da_ports::parser::CitationEntry;
    let malformed = CitationEntry {
        raw_text: "Malformed but retained bibliography entry".into(),
        doi: None,
        arxiv_id: Some("1410.3916.[Yahyaetal.2012".into()),
        title: None,
        anchors: vec![],
    };
    let (ingest, store) = make_ingest_with_citations(vec![], vec![malformed]);
    let result = batch_ingest_pdfs(&ingest, &[("paper.pdf".into(), "2401.00001".into())], None)
        .await
        .unwrap();
    assert_eq!(result.total_cites_resolved, 0);
    assert_eq!(store.get_nodes_by_label("Reference").await.len(), 1);
    assert!(store.get_nodes_by_label("Citation").await.is_empty());
}

#[tokio::test]
async fn test_ingest_cites_edges_stamped_with_temporal_fields() {
    // M327: CITES is a fact edge — must carry valid_at + created_at after ingest.
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::CitationEntry;
    let citations = vec![CitationEntry {
        raw_text: "Smith et al 2023".to_string(),
        doi: None,
        arxiv_id: Some("2301.00001".to_string()),
        title: Some("Cited Paper A".to_string()),
        anchors: vec![],
    }];
    let (ingest, store) = make_ingest_with_citations(vec![], citations);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];
    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();
    assert_eq!(result.ok, 1);
    assert_eq!(result.total_cites_resolved, 1);

    // Locate Paper and Citation nodes
    let papers = store.get_nodes_by_label("Paper").await;
    let citations = store.get_nodes_by_label("Citation").await;
    assert_eq!(papers.len(), 1);
    assert_eq!(citations.len(), 1);

    let cites = store
        .get_edges_between(papers[0], citations[0], "CITES")
        .await;
    assert_eq!(cites.len(), 1, "expected one CITES edge");
    let (edge_id, _) = cites[0];

    let valid_at = store.get_edge_property_string(edge_id, "valid_at").await;
    let created_at = store.get_edge_property_string(edge_id, "created_at").await;
    assert!(
        valid_at.is_some(),
        "CITES fact edge must have valid_at (M327)"
    );
    assert!(
        created_at.is_some(),
        "CITES fact edge must have created_at (M327)"
    );
    // Open fact: no invalid_at yet
    assert!(
        store
            .get_edge_property_string(edge_id, "invalid_at")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn test_ingest_has_part_structural_no_valid_at() {
    // Structural hasPart should get created_at only, not valid_at.
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::CitationEntry;
    let citations = vec![CitationEntry {
        raw_text: "No arxiv ref".to_string(),
        doi: None,
        arxiv_id: None,
        title: None,
        anchors: vec![],
    }];
    let (ingest, store) = make_ingest_with_citations(vec![], citations);
    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];
    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();
    assert_eq!(result.ok, 1);

    let papers = store.get_nodes_by_label("Paper").await;
    let refs = store.get_nodes_by_label("Reference").await;
    assert_eq!(papers.len(), 1);
    assert!(!refs.is_empty());

    let has_part = store.get_edges_between(papers[0], refs[0], "hasPart").await;
    assert_eq!(has_part.len(), 1);
    let (edge_id, _) = has_part[0];
    // Structural: no valid_at (ingest does not stamp structural yet — only created_at if we choose to)
    assert!(
        store
            .get_edge_property_string(edge_id, "valid_at")
            .await
            .is_none(),
        "hasPart must not carry valid_at (structural)"
    );
    // M331: structural edges still get created_at provenance.
    assert!(
        store
            .get_edge_property_string(edge_id, "created_at")
            .await
            .is_some(),
        "hasPart must carry created_at (structural provenance)"
    );
}

#[tokio::test]
async fn test_ingest_cites_valid_at_frozen_with_fixed_clock() {
    // M330: FixedClock freezes TemporalEdge.valid_at on CITES stamp.
    use chrono::DateTime;
    use da_ports::FixedClock;
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::CitationEntry;
    use std::sync::Arc;

    let frozen = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let citations = vec![CitationEntry {
        raw_text: "Smith et al 2023".to_string(),
        doi: None,
        arxiv_id: Some("2301.00001".to_string()),
        title: Some("Cited Paper A".to_string()),
        anchors: vec![],
    }];
    let (mut ingest, store) = make_ingest_with_citations(vec![], citations);
    ingest.clock = Arc::new(FixedClock::new(frozen));

    let pdfs = vec![("paper1.pdf".to_string(), "2401.00001".to_string())];
    let result = batch_ingest_pdfs(&ingest, &pdfs, None).await.unwrap();
    assert_eq!(result.ok, 1);
    assert_eq!(result.total_cites_resolved, 1);

    let papers = store.get_nodes_by_label("Paper").await;
    let citations = store.get_nodes_by_label("Citation").await;
    let cites = store
        .get_edges_between(papers[0], citations[0], "CITES")
        .await;
    assert_eq!(cites.len(), 1);
    let (edge_id, _) = cites[0];
    let valid_at = store
        .get_edge_property_string(edge_id, "valid_at")
        .await
        .expect("valid_at");
    let expected = frozen.to_rfc3339();
    assert_eq!(
        valid_at, expected,
        "CITES valid_at must equal FixedClock instant"
    );
    let created_at = store
        .get_edge_property_string(edge_id, "created_at")
        .await
        .expect("created_at");
    assert_eq!(created_at, expected);
}

#[tokio::test]
async fn revision_ingest_emits_additive_revision_owned_bibliography_edges() {
    // PIR-011 / D246: with a revision_context, ingest emits PaperRevision
    // HAS_PART Reference and PaperRevision CITES Citation edges beside the
    // existing Paper-level edges — without cloning the globally deduped
    // Reference/Citation nodes (MEM471) and without unlocking import (D127).
    use da_application::revision_materialization::{
        RevisionContentContext, RevisionManifestationInput,
    };
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::{CitationEntry, ParsedArticle, Section};

    let paper_id = "2401.00001";
    // 64-hex checksum binding: materialize requires manifestations whose
    // sha256 equals parsed.pdf_hash AND passes source_manifestation_vid.
    let source_sha256 = format!("{:064x}", 42);
    let parsed = ParsedArticle {
        paper_id: paper_id.into(),
        title: "Title of 2401.00001".into(),
        abstract_text: "Abstract of 2401.00001".into(),
        body_text: "x".repeat(1000),
        sections: vec![Section {
            title: "Introduction".into(),
            text: "intro text".into(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![
            CitationEntry {
                raw_text: "Smith et al 2023".to_string(),
                doi: None,
                arxiv_id: Some("2301.00001".to_string()),
                title: Some("Cited Paper A".to_string()),
                anchors: vec![],
            },
            CitationEntry {
                raw_text: "Jones 2022".to_string(),
                doi: None,
                arxiv_id: None, // Reference-only: no Citation node
                title: None,
                anchors: vec![],
            },
        ],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: source_sha256.clone(),
    };
    let revision_context = || RevisionContentContext {
        source_code: "arxiv".into(),
        canonical_id: paper_id.into(),
        revision: 5,
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: source_sha256.clone(),
            bytes: 1234,
            uri: "https://arxiv.org/pdf/2401.00001v5".into(),
            path: "2401.00001v5.pdf".into(),
            capture_status: "captured".into(),
        }],
    };
    let (ingest, store) = make_ingest_with_citations(vec![], vec![]);

    let result = ingest
        .materialize_parsed_with_context(
            &parsed,
            vec![0.1; 1024],
            None,
            Some("arxiv".into()),
            None,
            da_domain::PdfTriageStatus::NotConfigured,
            Some(&revision_context()),
        )
        .await
        .unwrap();
    assert_eq!(result.cites_resolved, 1);
    assert_eq!(result.references_written, 2);

    let paper = store.get_nodes_by_label("Paper").await;
    let revisions = store.get_nodes_by_label("PaperRevision").await;
    let references = store.get_nodes_by_label("Reference").await;
    let citations = store.get_nodes_by_label("Citation").await;
    assert_eq!(paper.len(), 1);
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        references.len(),
        2,
        "Reference nodes stay globally deduped, one per raw_text (MEM471)"
    );
    assert_eq!(
        citations.len(),
        1,
        "only the resolvable arxiv_id gets a Citation"
    );
    let revision_node = revisions[0];

    // Work-level Paper edges remain (existing traversal surface).
    assert_eq!(
        store
            .get_edges_between(paper[0], references[0], "hasPart")
            .await
            .len(),
        1,
        "Paper keeps HAS_PART Reference for work-level traversal"
    );
    assert_eq!(
        store
            .get_edges_between(paper[0], citations[0], "CITES")
            .await
            .len(),
        1,
        "Paper keeps CITES Citation for work-level traversal"
    );

    // Additive revision-owned edges (PIR-011).
    assert_eq!(
        store
            .get_edges_between(revision_node, references[0], "hasPart")
            .await
            .len(),
        1,
        "PaperRevision must own Reference via additive HAS_PART (PIR-011)"
    );
    assert_eq!(
        store
            .get_edges_between(revision_node, references[1], "hasPart")
            .await
            .len(),
        1,
        "Reference-only bibliography entry is also revision-owned"
    );
    assert_eq!(
        store
            .get_edges_between(revision_node, citations[0], "CITES")
            .await
            .len(),
        1,
        "PaperRevision must own Citation via additive CITES (PIR-011)"
    );

    // D127: every written node stays import_eligible=false.
    for node in [
        paper[0],
        revision_node,
        references[0],
        references[1],
        citations[0],
    ] {
        assert_eq!(
            store.get_node_property_bool(node, "import_eligible").await,
            Some(false),
            "D127: import_eligible must be false"
        );
    }

    // Replay: identical revision ingest adds no nodes/edges (applied=0).
    let nodes_before = store.node_count_total();
    let edges_before = store.edge_count_total();
    ingest
        .materialize_parsed_with_context(
            &parsed,
            vec![0.1; 1024],
            None,
            Some("arxiv".into()),
            None,
            da_domain::PdfTriageStatus::NotConfigured,
            Some(&revision_context()),
        )
        .await
        .unwrap();
    assert_eq!(
        store.node_count_total(),
        nodes_before,
        "replay must not create nodes"
    );
    assert_eq!(
        store.edge_count_total(),
        edges_before,
        "replay must not create edges (Paper + PaperRevision edges idempotent)"
    );
}

#[tokio::test]
async fn revision_ingest_writes_in_text_anchors_on_revision_owned_edges_only() {
    // PIR-011 / T03 (D247/D249): in-text citation anchors live on PaperRevision
    // CITES/HAS_PART edges only — never on Paper-sourced edges, never on the
    // globally deduped Citation/Reference nodes (MEM796/MEM798). Missingness
    // is typed: "" when anchors resolve, no_in_text_anchor when the citation
    // has no in-text refs.
    use da_application::revision_materialization::{
        RevisionContentContext, RevisionManifestationInput,
    };
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::{CitationAnchor, CitationEntry, ParsedArticle, Section};

    let paper_id = "2401.00002";
    // 64-hex checksum binding for the revision manifestation.
    let source_sha256 = format!("{:064x}", 7);
    let parsed = ParsedArticle {
        paper_id: paper_id.into(),
        title: "Title of 2401.00002".into(),
        abstract_text: "Abstract of 2401.00002".into(),
        body_text: "x".repeat(1000),
        sections: vec![Section {
            title: "Introduction".into(),
            text: "intro text".into(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![
            CitationEntry {
                raw_text: "Resolvable cited work".to_string(),
                doi: None,
                arxiv_id: Some("2301.00001".to_string()),
                title: Some("Cited Paper A".to_string()),
                // Multi-span: cited twice in the same revision.
                anchors: vec![
                    CitationAnchor {
                        ref_id: "b0".into(),
                        section_path_id: Some("1".into()),
                        quote: "[1]".into(),
                        offset: Some(0),
                    },
                    CitationAnchor {
                        ref_id: "b0".into(),
                        section_path_id: Some("2".into()),
                        quote: "[1]".into(),
                        offset: Some(12),
                    },
                ],
            },
            CitationEntry {
                raw_text: "Reference-only work".to_string(),
                doi: None,
                arxiv_id: None,
                title: None,
                anchors: vec![CitationAnchor {
                    ref_id: "b1".into(),
                    section_path_id: Some("1".into()),
                    quote: "[2]".into(),
                    offset: Some(3),
                }],
            },
            CitationEntry {
                raw_text: "No in-text refs".to_string(),
                doi: None,
                arxiv_id: None,
                title: None,
                anchors: vec![],
            },
        ],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: source_sha256.clone(),
    };
    let revision_context = || RevisionContentContext {
        source_code: "arxiv".into(),
        canonical_id: paper_id.into(),
        revision: 5,
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: source_sha256.clone(),
            bytes: 100,
            uri: format!("https://arxiv.org/pdf/{paper_id}v5"),
            path: format!("{paper_id}v5.pdf"),
            capture_status: "captured".into(),
        }],
    };
    let (ingest, store) = make_ingest_with_citations(vec![], vec![]);

    ingest
        .materialize_parsed_with_context(
            &parsed,
            vec![0.1; 1024],
            None,
            Some("arxiv".into()),
            None,
            da_domain::PdfTriageStatus::NotConfigured,
            Some(&revision_context()),
        )
        .await
        .unwrap();

    let papers = store.get_nodes_by_label("Paper").await;
    let revisions = store.get_nodes_by_label("PaperRevision").await;
    let references = store.get_nodes_by_label("Reference").await;
    let citations = store.get_nodes_by_label("Citation").await;
    assert_eq!(papers.len(), 1);
    assert_eq!(revisions.len(), 1);
    assert_eq!(references.len(), 3);
    assert_eq!(
        citations.len(),
        1,
        "only citation[0] has a resolvable arxiv_id"
    );
    let rev = revisions[0];
    let paper = papers[0];

    // Resolvable citation: anchors on the revision-owned CITES edge (D249).
    let rev_cites = store.get_edges_between(rev, citations[0], "CITES").await;
    assert_eq!(rev_cites.len(), 1);
    let (rev_cites_eid, _) = rev_cites[0];
    let anchors_json = store
        .get_edge_property_string(rev_cites_eid, "citation_anchors_json")
        .await
        .expect("revision-owned CITES edge must carry citation_anchors_json");
    let anchors: Vec<CitationAnchor> = serde_json::from_str(&anchors_json).unwrap();
    assert_eq!(anchors.len(), 2, "both spans survive the roundtrip");
    assert_eq!(anchors[0].ref_id, "b0");
    assert_eq!(anchors[0].section_path_id.as_deref(), Some("1"));
    assert_eq!(anchors[0].quote, "[1]");
    assert_eq!(anchors[0].offset, Some(0));
    assert_eq!(anchors[1].section_path_id.as_deref(), Some("2"));
    assert_eq!(
        store
            .get_edge_property_string(rev_cites_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some(""),
        "anchors present and resolved → empty missingness"
    );

    // Paper-sourced CITES edge: never carries anchor keys (D249).
    let paper_cites = store.get_edges_between(paper, citations[0], "CITES").await;
    assert_eq!(paper_cites.len(), 1);
    let (paper_cites_eid, _) = paper_cites[0];
    assert_eq!(
        store
            .get_edge_property_string(paper_cites_eid, "citation_anchors_json")
            .await,
        None,
        "Paper-sourced edges must never carry citation_anchors_json"
    );
    assert_eq!(
        store
            .get_edge_property_string(paper_cites_eid, "citation_anchor_missingness")
            .await,
        None
    );

    // Reference-only citation with anchors: anchors on revision HAS_PART edge.
    let rev_hp = store.get_edges_between(rev, references[1], "hasPart").await;
    assert_eq!(rev_hp.len(), 1);
    let (rev_hp_eid, _) = rev_hp[0];
    let anchors_json = store
        .get_edge_property_string(rev_hp_eid, "citation_anchors_json")
        .await
        .expect("Reference-only revision HAS_PART edge must carry anchors");
    let anchors: Vec<CitationAnchor> = serde_json::from_str(&anchors_json).unwrap();
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].ref_id, "b1");
    assert_eq!(
        store
            .get_edge_property_string(rev_hp_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some("")
    );
    // Paper HAS_PART edge: no anchor keys.
    let paper_hp = store
        .get_edges_between(paper, references[1], "hasPart")
        .await;
    let (paper_hp_eid, _) = paper_hp[0];
    assert_eq!(
        store
            .get_edge_property_string(paper_hp_eid, "citation_anchors_json")
            .await,
        None
    );

    // Citation without in-text refs: typed missingness no_in_text_anchor.
    let rev_hp_none = store.get_edges_between(rev, references[2], "hasPart").await;
    assert_eq!(rev_hp_none.len(), 1);
    let (rev_hp_none_eid, _) = rev_hp_none[0];
    assert_eq!(
        store
            .get_edge_property_string(rev_hp_none_eid, "citation_anchors_json")
            .await
            .as_deref(),
        Some("[]"),
        "empty anchors serialize as an empty JSON array"
    );
    assert_eq!(
        store
            .get_edge_property_string(rev_hp_none_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some("no_in_text_anchor")
    );

    // Global dedup surface (Citation/Reference nodes): never anchor properties.
    assert_eq!(
        store
            .get_node_property_string(citations[0], "citation_anchors_json")
            .await,
        None,
        "global Citation node must never carry anchors (MEM796)"
    );
    assert_eq!(
        store
            .get_node_property_string(references[0], "citation_anchors_json")
            .await,
        None,
        "global Reference node must never carry anchors (MEM796)"
    );

    // Replay: identical revision ingest adds no nodes/edges (applied=0).
    let nodes_before = store.node_count_total();
    let edges_before = store.edge_count_total();
    ingest
        .materialize_parsed_with_context(
            &parsed,
            vec![0.1; 1024],
            None,
            Some("arxiv".into()),
            None,
            da_domain::PdfTriageStatus::NotConfigured,
            Some(&revision_context()),
        )
        .await
        .unwrap();
    assert_eq!(store.node_count_total(), nodes_before);
    assert_eq!(store.edge_count_total(), edges_before);
}
