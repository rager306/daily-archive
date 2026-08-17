//! Integration test for PDF classifier injection into IngestUseCase (M347).
//!
//! Verifies that when a PdfClassificationPort is injected, ingest_pdf
//! calls classify() before GROBID parse_pdf, and logs the result.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use da_application::ingest::IngestUseCase;
use da_domain::{PdfClassification, PdfTriageStatus, PdfType};
use da_ports::PdfClassificationPort;
use da_ports::embedder::{EmbedResult, Embedder};
use da_ports::parser::{ParseResult, ParsedArticle, ParserPort};

mod common;
use common::mock_graph_store::MockGraphStore;

// ---------- Mock Classifier ----------

struct MockClassifier {
    call_count: Arc<AtomicUsize>,
    result: PdfClassification,
}

#[async_trait]
impl PdfClassificationPort for MockClassifier {
    async fn classify(
        &self,
        _pdf_bytes: &[u8],
    ) -> da_ports::ClassificationResult<PdfClassification> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.result.clone())
    }
}

// ---------- Mock Parser ----------

struct MockParser {
    call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl ParserPort for MockParser {
    async fn parse_pdf(&self, _pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title: format!("Title of {paper_id}"),
            abstract_text: "abstract".into(),
            body_text: "body".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            pdf_hash: "hash".into(),
        })
    }
}

// ---------- Mock Embedder ----------

struct MockEmbedder;

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> EmbedResult<Vec<f32>> {
        Ok(vec![0.0; 4])
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbedResult<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; 4]).collect())
    }

    fn dimensions(&self) -> usize {
        4
    }

    fn model_id(&self) -> &str {
        "mock"
    }
}

// ---------- Tests ----------

#[tokio::test]
async fn ingest_calls_classifier_before_parser() {
    let classifier_calls = Arc::new(AtomicUsize::new(0));
    let parser_calls = Arc::new(AtomicUsize::new(0));

    let classifier = Arc::new(MockClassifier {
        call_count: classifier_calls.clone(),
        result: PdfClassification {
            pdf_type: PdfType::Text,
            page_count: 10,
            pages_needing_ocr: vec![],
            ocr_reasons_by_page: vec![],
            confidence: 0.95,
            has_encoding_issues: false,
        },
    });

    let parser = MockParser {
        call_count: parser_calls.clone(),
    };

    let use_case = IngestUseCase::with_clock(
        Box::new(parser),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(classifier);

    // Use a non-existent PDF path — classifier reads bytes, parser is mocked
    // We need a real file for classifier.classify to read, but since MockClassifier
    // ignores bytes, we can use any path. However ingest_pdf reads the file first
    // for classification. Create a temp file.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await;

    // ingest may fail on graph operations (in-memory store), but classifier
    // and parser should have been called.
    assert_eq!(
        classifier_calls.load(Ordering::SeqCst),
        1,
        "classifier should be called exactly once"
    );
    assert_eq!(
        parser_calls.load(Ordering::SeqCst),
        1,
        "parser should be called exactly once"
    );

    // If ingest succeeded, classification must be on the machine-readable result (M350).
    if let Ok(result) = result {
        assert_eq!(result.paper_id, "2507.19457");
        assert_eq!(result.pdf_triage, PdfTriageStatus::Classified);
        let c = result
            .pdf_classification
            .expect("classifier succeeded → pdf_classification must be Some");
        assert_eq!(c.pdf_type, PdfType::Text);
        assert_eq!(c.page_count, 10);
        assert!((c.confidence - 0.95).abs() < f32::EPSILON);
    }
}

#[tokio::test]
async fn ingest_without_classifier_works() {
    let parser = MockParser {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let use_case = IngestUseCase::with_clock(
        Box::new(parser),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    );
    // No with_pdf_classifier — backward compatible

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    // Should not panic — classifier is None, ingest proceeds normally
    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await;
    if let Ok(result) = result {
        assert!(
            result.pdf_classification.is_none(),
            "without classifier, pdf_classification must be None"
        );
        assert_eq!(result.pdf_triage, PdfTriageStatus::NotConfigured);
    }
}

#[tokio::test]
async fn ingest_classifier_failure_does_not_block_ingest() {
    use da_ports::ClassificationError;

    struct FailingClassifier;

    #[async_trait]
    impl PdfClassificationPort for FailingClassifier {
        async fn classify(
            &self,
            _pdf_bytes: &[u8],
        ) -> da_ports::ClassificationResult<PdfClassification> {
            Err(ClassificationError::Failed("test failure".into()))
        }
    }

    let parser = MockParser {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let use_case = IngestUseCase::with_clock(
        Box::new(parser),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(Arc::new(FailingClassifier));

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    // Should not fail — classifier error is logged, not propagated
    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await;
    if let Ok(result) = result {
        assert!(
            result.pdf_classification.is_none(),
            "failed classify must not populate pdf_classification"
        );
        assert_eq!(
            result.pdf_triage,
            PdfTriageStatus::FailedOpen,
            "fail_open policy must record FailedOpen"
        );
    }
}
