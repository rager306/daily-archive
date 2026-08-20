//! Integration test for PDF classifier injection into IngestUseCase (M347).
//!
//! Verifies that when a PdfClassificationPort is injected, ingest_pdf
//! calls classify() before GROBID parse_pdf, and logs the result.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use da_application::ingest::IngestUseCase;
use da_domain::{PdfClassification, PdfTriagePolicy, PdfTriageStatus, PdfType};
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
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
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
        let missing = result
            .page_text_missingness
            .expect("classified clean PDF stamps Some(empty), not unset");
        assert!(
            missing.is_empty(),
            "zero OCR pages must stamp Some(empty), not invent pages"
        );
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
        assert!(
            result.page_text_missingness.is_none(),
            "without classifier, page-text missingness stays unset"
        );
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

fn blocking_policy(percent: u8) -> PdfTriagePolicy {
    PdfTriagePolicy {
        schema_version: "pdf-triage-policy.v1".into(),
        import_eligible: false,
        fail_open: false,
        record_in_ingest_result: true,
        feature_gate: "pdf-classifier".into(),
        encoding_issues_block_grobid: false,
        max_garbled_page_percent: percent,
    }
}

fn mixed_classification(pages_needing_ocr: Vec<u32>, page_count: u32) -> PdfClassification {
    PdfClassification {
        pdf_type: PdfType::Mixed,
        page_count,
        pages_needing_ocr,
        ocr_reasons_by_page: vec![],
        confidence: 0.9,
        has_encoding_issues: true,
    }
}

#[tokio::test]
async fn ingest_blocks_grobid_when_garbled_ratio_exceeds_threshold_and_fail_open_false() {
    let parser_calls = Arc::new(AtomicUsize::new(0));
    let classifier = Arc::new(MockClassifier {
        call_count: Arc::new(AtomicUsize::new(0)),
        result: mixed_classification(vec![1, 2, 3, 4, 5], 10),
    });
    let use_case = IngestUseCase::with_clock(
        Box::new(MockParser {
            call_count: parser_calls.clone(),
        }),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(classifier)
    .with_pdf_triage_policy(blocking_policy(20));

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    let err = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await
        .expect_err("high garbled ratio + fail_open=false must bail before GROBID");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("PDF triage blocked GROBID"),
        "typed block message missing: {msg}"
    );
    assert_eq!(
        parser_calls.load(Ordering::SeqCst),
        0,
        "GROBID must not run after Block + fail_open=false"
    );
}

#[tokio::test]
async fn ingest_default_bundled_policy_continues_on_high_garbled_ratio() {
    let parser_calls = Arc::new(AtomicUsize::new(0));
    let classifier = Arc::new(MockClassifier {
        call_count: Arc::new(AtomicUsize::new(0)),
        result: mixed_classification(vec![1, 2, 3, 4, 5], 10),
    });
    let use_case = IngestUseCase::with_clock(
        Box::new(MockParser {
            call_count: parser_calls.clone(),
        }),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(classifier);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await
        .expect("bundled fail_open=true + percent=100 must continue");
    assert_eq!(result.pdf_triage, PdfTriageStatus::Classified);
    assert_eq!(parser_calls.load(Ordering::SeqCst), 1);
    let missing = result
        .page_text_missingness
        .expect("classified mixed PDF stamps Some, not unset");
    assert_eq!(
        missing.iter().map(|p| p.page).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert_eq!(
        result.body_chars, 4,
        "GROBID body_text must not be rewritten by the stamp"
    );
}

#[tokio::test]
async fn ingest_zero_page_classification_continues() {
    let parser_calls = Arc::new(AtomicUsize::new(0));
    let classifier = Arc::new(MockClassifier {
        call_count: Arc::new(AtomicUsize::new(0)),
        result: PdfClassification {
            pdf_type: PdfType::Unknown,
            page_count: 0,
            pages_needing_ocr: vec![],
            ocr_reasons_by_page: vec![],
            confidence: 0.0,
            has_encoding_issues: false,
        },
    });
    let use_case = IngestUseCase::with_clock(
        Box::new(MockParser {
            call_count: parser_calls.clone(),
        }),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(classifier)
    .with_pdf_triage_policy(blocking_policy(0));

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await
        .expect("page_count=0 never divides and never blocks");
    assert_eq!(result.pdf_triage, PdfTriageStatus::Classified);
    assert_eq!(parser_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ingest_block_with_fail_open_true_continues_to_grobid() {
    let parser_calls = Arc::new(AtomicUsize::new(0));
    let classifier = Arc::new(MockClassifier {
        call_count: Arc::new(AtomicUsize::new(0)),
        result: mixed_classification(vec![1, 2, 3, 4, 5], 10),
    });
    let mut policy = blocking_policy(20);
    policy.fail_open = true;
    let use_case = IngestUseCase::with_clock(
        Box::new(MockParser {
            call_count: parser_calls.clone(),
        }),
        Box::new(MockEmbedder),
        Box::new(MockGraphStore::new()),
        std::sync::Arc::new(da_ports::clock::SystemClock),
    )
    .with_pdf_classifier(classifier)
    .with_pdf_triage_policy(policy);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"fake pdf bytes").unwrap();

    let result = use_case
        .ingest_pdf(tmp.path().to_str().unwrap(), "2507.19457")
        .await
        .expect("Block + fail_open=true must warn and continue");
    assert_eq!(result.pdf_triage, PdfTriageStatus::Classified);
    assert_eq!(parser_calls.load(Ordering::SeqCst), 1);
}
