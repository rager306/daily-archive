//! Document-context / readiness honesty proofs for HTML path identity
//! (M398 S05 T04, PIR-009 / PIR-010).
//!
//! Positive: HTML sections carrying derived dotted path identity
//! (`path_id`, `parent_section_id`, `order_in_parent`, `section_id` — as
//! produced by `HtmlParser::extract_sections`) pass the
//! `DocumentContextStage` projection with unique `path_identity` and
//! document-order prev/next chains, including the deterministic `~N`
//! restart collision convention (dot-free suffixes never become dotted
//! parents).
//!
//! Negative: flat HTML without path identity is blocked fail-closed
//! (`missing_section_path_id` Terminal error) — the stage never claims
//! structure the source does not prove. Partial identity and duplicate
//! paths are blocked too, so readiness can never over-claim.
//!
//! The committed Governor fixtures (`governor/fixtures/html_structure_contract/`) are
//! DATA consumed by `scripts/governor/checks/html_structure_honesty.yaml`;
//! this test pins them to the current stage output and to the blocked-
//! record contract, so the fixtures cannot silently drift from the Rust
//! behavior (same pattern as `navigation_report_fixtures_match_committed_governor_data`).

use std::collections::BTreeMap;
use std::path::Path;

use da_adapters::HtmlParser;
use da_application::{DocumentContextProjection, DocumentContextStage, SectionSummaryInput};
use da_ports::parser::{ParsedArticle, ParserPort, Section};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageExecutionError, StageExecutionRequest};

/// HTML article whose sections carry the full derived identity (the shape
/// `HtmlParser::extract_sections` produces for an h1/h2/h3 heading stack).
fn html_article(paper_id: &str) -> ParsedArticle {
    ParsedArticle {
        paper_id: paper_id.into(),
        title: "Graph Neural Networks".into(),
        abstract_text: "abstract".into(),
        body_text: String::new(),
        sections: vec![
            Section {
                title: "Chapter 1".into(),
                text: "intro".into(),
                level: 1,
                path_id: Some("1".into()),
                parent_section_id: None,
                order_in_parent: Some(0),
                section_id: Some("sec-1".into()),
            },
            Section {
                title: "Section A".into(),
                text: "a".into(),
                level: 2,
                path_id: Some("1.1".into()),
                parent_section_id: Some("sec-1".into()),
                order_in_parent: Some(0),
                section_id: Some("sec-1.1".into()),
            },
            Section {
                title: "Section B".into(),
                text: "b".into(),
                level: 2,
                path_id: Some("1.2".into()),
                parent_section_id: Some("sec-1".into()),
                order_in_parent: Some(1),
                section_id: Some("sec-1.2".into()),
            },
            Section {
                title: "Sub B.1".into(),
                text: "sub".into(),
                level: 3,
                path_id: Some("1.2.1".into()),
                parent_section_id: Some("sec-1.2".into()),
                order_in_parent: Some(0),
                section_id: Some("sec-1.2.1".into()),
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
    }
}

/// Build the stage request for an article with the given summaries payload.
fn request(article: &ParsedArticle, summaries: Vec<SectionSummaryInput>) -> StageExecutionRequest {
    StageExecutionRequest {
        article_id: article.paper_id.clone(),
        dependency_payloads: BTreeMap::from([
            ("parse".into(), serde_json::to_vec(article).unwrap()),
            (
                "section_summaries".into(),
                serde_json::to_vec(&summaries).unwrap(),
            ),
        ]),
    }
}

// ─── Positive proofs: proven HTML path identity passes ────────────────

/// HTML with full derived path identity passes the document-context
/// projection: unique paths, document-order prev/next chains, and
/// summaries joined by path_id.
#[tokio::test]
async fn document_context_stage_accepts_html_derived_path_identity() {
    let article = html_article("gnn-ch-chapters__01-intro-to-graphs");
    let output = DocumentContextStage::new()
        .execute(request(
            &article,
            vec![SectionSummaryInput {
                path_id: "1.2".into(),
                summary: "Sections B covers message passing".into(),
            }],
        ))
        .await
        .expect("proven HTML paths must pass document-context projection");
    assert_eq!(output.metadata["section_count"], "4");
    assert_eq!(output.metadata["summary_count"], "1");
    assert_eq!(output.metadata["path_identity"], "unique");
    let projection: DocumentContextProjection = serde_json::from_slice(&output.payload).unwrap();
    let paths: Vec<&str> = projection
        .sections
        .iter()
        .map(|s| s.path_id.as_str())
        .collect();
    assert_eq!(paths, vec!["1", "1.1", "1.2", "1.2.1"]);
    // Document-order prev/next chains.
    assert_eq!(projection.sections[0].previous_path_id, None);
    assert_eq!(projection.sections[0].next_path_id.as_deref(), Some("1.1"));
    assert_eq!(projection.sections[1].next_path_id.as_deref(), Some("1.2"));
    assert_eq!(
        projection.sections[2].previous_path_id.as_deref(),
        Some("1.1")
    );
    assert_eq!(
        projection.sections[2].next_path_id.as_deref(),
        Some("1.2.1")
    );
    assert_eq!(projection.sections[3].next_path_id, None);
    // Summary joined by path_id, not ordinal.
    assert_eq!(
        projection.sections[2].summary.as_deref(),
        Some("Sections B covers message passing")
    );
    assert_eq!(projection.sections[0].summary, None);
}

/// `~N` restart collision suffixes keep unique identity and never become
/// dotted parents: the projection still passes with unique paths and the
/// suffixed section stays a root sibling of its colliding base path.
#[tokio::test]
async fn document_context_stage_html_restart_paths_keep_unique_identity() {
    let mut article = html_article("gnn-ch-chapters__02-restart");
    article.sections.push(Section {
        title: "Chapter 1 (restart)".into(),
        text: "r".into(),
        level: 1,
        path_id: Some("1~2".into()),
        parent_section_id: None,
        order_in_parent: Some(1),
        section_id: Some("sec-1~2".into()),
    });
    article.sections.push(Section {
        title: "Restart child".into(),
        text: "rc".into(),
        level: 2,
        path_id: Some("1~2.1".into()),
        parent_section_id: Some("sec-1~2".into()),
        order_in_parent: Some(0),
        section_id: Some("sec-1~2.1".into()),
    });
    let output = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .expect("restart suffixes must stay unique and pass projection");
    assert_eq!(output.metadata["path_identity"], "unique");
    let projection: DocumentContextProjection = serde_json::from_slice(&output.payload).unwrap();
    let paths: Vec<&str> = projection
        .sections
        .iter()
        .map(|s| s.path_id.as_str())
        .collect();
    assert_eq!(paths, vec!["1", "1.1", "1.2", "1.2.1", "1~2", "1~2.1"]);
    // Restart root "1~2" is a root sibling (previous = "1.2.1"), its child
    // "1~2.1" follows it — the `~2` suffix is dot-free, never a dotted
    // parent level.
    assert_eq!(projection.sections[4].parent_section_id, None);
    assert_eq!(
        projection.sections[4].previous_path_id.as_deref(),
        Some("1.2.1")
    );
    assert_eq!(projection.sections[5].next_path_id, None);
}

/// End-to-end: `HtmlParser` (ParserPort, h1/h2/h3 heading stack) output
/// flows straight into `DocumentContextStage` and passes — HTML structure
/// honestly proven by the parser is accepted downstream.
#[tokio::test]
async fn document_context_stage_accepts_html_parser_output_end_to_end() {
    let html = r#"
    <article>
    <h1>Chapter 1</h1>
    <p>Intro text here.</p>
    <h2>Section A</h2>
    <p>Section A content.</p>
    <h2>Section B</h2>
    <p>Section B content.</p>
    <h3>Sub B.1</h3>
    <p>Sub content.</p>
    </article>
    "#;
    let dir = tempfile::tempdir().expect("temp dir");
    let html_path = dir.path().join("chapter.html");
    std::fs::write(&html_path, html).expect("write html fixture");
    let parsed = HtmlParser::new()
        .parse_html(
            html_path.to_str().unwrap(),
            "gnn-ch-chapters__01-intro-to-graphs",
        )
        .await
        .expect("html parser must parse the fixture");
    let mut article = html_article("gnn-ch-chapters__01-intro-to-graphs");
    article.sections = parsed.sections;
    let output = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .expect("html_parser-derived sections must pass document-context projection");
    assert_eq!(output.metadata["section_count"], "4");
    let projection: DocumentContextProjection = serde_json::from_slice(&output.payload).unwrap();
    let paths: Vec<&str> = projection
        .sections
        .iter()
        .map(|s| s.path_id.as_str())
        .collect();
    assert_eq!(paths, vec!["1", "1.1", "1.2", "1.2.1"]);
}

/// Determinism: the same proven-HTML input renders byte-identical
/// projection payloads on every run (stable cache identity).
#[tokio::test]
async fn document_context_stage_projection_is_deterministic() {
    let article = html_article("gnn-ch-chapters__01-intro-to-graphs");
    let first = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .unwrap();
    let second = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .unwrap();
    assert_eq!(
        first.payload, second.payload,
        "projection must be deterministic"
    );
    assert_eq!(first.metadata, second.metadata);
}

// ─── Negative proofs: missing identity stays blocked / not_claimed ────

/// Flat HTML sections without any path identity are blocked fail-closed:
/// the stage returns `missing_section_path_id` instead of claiming a
/// document structure the source does not prove.
#[tokio::test]
async fn document_context_stage_blocks_flat_html_with_missing_path_id() {
    let mut article = html_article("gnn-ch-chapters__03-flat");
    article.sections = vec![
        Section {
            title: "Intro".into(),
            text: "i".into(),
            level: 1,
            path_id: None,
            parent_section_id: None,
            order_in_parent: None,
            section_id: None,
        },
        Section {
            title: "Methods".into(),
            text: "m".into(),
            level: 1,
            path_id: None,
            parent_section_id: None,
            order_in_parent: None,
            section_id: None,
        },
    ];
    let error = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. }
            if error_class == "missing_section_path_id"),
        "flat HTML without path identity must be blocked, got {error:?}"
    );
}

/// Partial identity is also fail-closed: a single section without
/// `path_id` blocks the whole projection — readiness never claims more
/// structure than the source proves.
#[tokio::test]
async fn document_context_stage_blocks_partial_identity_fail_closed() {
    let mut article = html_article("gnn-ch-chapters__04-partial");
    // Section B loses its identity (parser produced no path for it).
    article.sections[2].path_id = None;
    article.sections[2].section_id = None;
    article.sections[2].parent_section_id = None;
    let error = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. }
            if error_class == "missing_section_path_id"),
        "partial identity must stay blocked, got {error:?}"
    );
}

/// Duplicate path_ids are rejected: two sections cannot claim the same
/// tree position (the `~N` restart convention exists precisely to keep
/// paths unique).
#[tokio::test]
async fn document_context_stage_rejects_duplicate_path_ids() {
    let mut article = html_article("gnn-ch-chapters__05-dup");
    article.sections[1].path_id = Some("1".into());
    article.sections[1].section_id = Some("sec-1".into());
    let error = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. }
            if error_class == "duplicate_section_path_id"),
        "duplicate paths must be rejected, got {error:?}"
    );
}

/// Missing / unresolved summary paths are rejected: the projection never
/// invents a summary for a section path that does not exist.
#[tokio::test]
async fn document_context_stage_rejects_unresolved_summary_path() {
    let article = html_article("gnn-ch-chapters__01-intro-to-graphs");
    let error = DocumentContextStage::new()
        .execute(request(
            &article,
            vec![SectionSummaryInput {
                path_id: "9.9".into(),
                summary: "orphan".into(),
            }],
        ))
        .await
        .unwrap_err();
    assert!(
        matches!(error, StageExecutionError::Terminal { ref error_class, .. }
            if error_class == "unresolved_section_summary_path"),
        "unresolved summary path must be rejected, got {error:?}"
    );
}

// ─── Governor fixture pinning (PIR-010 / PIR-009) ─────────────────────

/// The committed positive Governor fixture
/// (`governor/fixtures/html_structure_contract/html_path_identity.projection.json`) must
/// equal the current stage output for the proven-HTML fixture, so the DATA
/// consumed by `scripts/governor/checks/html_structure_honesty.yaml` can
/// never silently drift from the Rust contract.
#[tokio::test]
async fn document_context_stage_fixtures_match_committed_governor_data() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../governor/fixtures/html_structure_contract");
    let article = html_article("gnn-ch-chapters__01-intro-to-graphs");
    let output = DocumentContextStage::new()
        .execute(request(&article, vec![]))
        .await
        .expect("proven HTML paths must pass projection");
    let projection: DocumentContextProjection = serde_json::from_slice(&output.payload).unwrap();
    let rendered = serde_json::to_value(&projection).unwrap();
    let committed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir.join("html_path_identity.projection.json"))
            .expect("read html_path_identity.projection.json"),
    )
    .expect("parse html_path_identity.projection.json");
    assert_eq!(
        committed, rendered,
        "html_path_identity fixture must match current stage output; \
         regenerate via the committed projection JSON"
    );
}

/// The committed negative Governor fixture
/// (`governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json`)
/// models the fail-closed outcome for flat HTML: ok=false, readiness
/// stays `not_claimed`, a typed reason names the blocked identity, and
/// import_eligible stays false (D127). Pins the blocked-record contract
/// to the same semantics the tests above enforce.
#[test]
fn document_context_stage_negative_fixture_models_blocked_html_readiness() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../governor/fixtures/html_structure_contract");
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir.join("flat_missing_identity.blocked.json"))
            .expect("read flat_missing_identity.blocked.json"),
    )
    .expect("parse flat_missing_identity.blocked.json");
    assert_eq!(record["ok"], serde_json::Value::Bool(false));
    assert_eq!(
        record["readiness"],
        serde_json::Value::String("not_claimed".into())
    );
    assert_eq!(
        record["reason"],
        serde_json::Value::String("missing_section_path_id".into())
    );
    assert_eq!(record["import_eligible"], serde_json::Value::Bool(false));
    assert!(
        record["detail"].as_str().is_some_and(|d| !d.is_empty()),
        "blocked record must carry a non-empty detail"
    );
}
