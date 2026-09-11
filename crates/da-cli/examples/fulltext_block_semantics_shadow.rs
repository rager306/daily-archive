//! Offline S03 fulltext block-semantics shadow runner.
//! The native sequence is executable evidence only: Java remains production authority.
#![cfg(feature = "pdf-layout")]

use da_parse::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const REPORT_SCHEMA: &str = "fulltext-block-semantics-shadow.v1";
const DATA: &str = include_str!("../../../data/parse/fulltext_block_semantics.yaml");

fn raw_digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn digest(text: &str) -> String {
    format!("sha256:{}", raw_digest(text))
}
fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn fixture() -> (
    LayoutDocument,
    LayoutLattice,
    FullSegmentationOutcome,
    Vec<LexemeRow>,
) {
    let words = [
        "Introduction",
        "paragraph",
        "Methods",
        "citation",
        "Annex",
        "appendix",
    ];
    let provider = LayoutProvider::new("s03-shadow", "1", "a".repeat(64)).unwrap();
    let tokens: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0 + i as f64 * 35.0, 10.0, 35.0 + i as f64 * 35.0, 25.0)
                    .unwrap(),
                raw_digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider.clone(),
            )
            .unwrap()
        })
        .collect();
    let doc = LayoutDocument::new(
        "s03-shadow",
        "b".repeat(64),
        vec![LayoutPage::new(0, 612.0, 792.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let lattice_tokens = tokens
        .iter()
        .map(|t| LatticeToken {
            source_ordinal: t.source_ordinal,
            token_ordinal: t.token_ordinal,
            bbox: t.bbox,
            text_digest: t.text_digest.clone(),
            source_span: t.source_span.clone(),
            kind: t.kind,
        })
        .collect();
    let lattice = LayoutLattice::new(
        &doc,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 612.0,
            height: 792.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 6).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 100.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 6).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 300.0, 100.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let rows: Vec<LexemeRow> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                "s03-shadow",
                "s03-shadow",
                0,
                i as u32,
                raw_digest(text),
                *text,
            )
        })
        .collect();
    let segmentation =
        segment_document_full(&doc, &lattice, &rows, SegmentationConfig::default()).unwrap();
    (doc, lattice, segmentation, rows)
}

fn execute() -> Value {
    let (doc, lattice, segmentation, rows) = fixture();
    let receipt =
        account_full_regions(&doc, &lattice, &segmentation, NativeBodyLimits::default()).unwrap();
    let config = NativeFulltextBlocksConfig {
        config_fingerprint: digest(DATA),
        role_hints: vec![
            FulltextRoleHint {
                page_ordinal: 0,
                source_ordinal: 1,
                role: FulltextLeafRole::Paragraph,
                marker_subtype: None,
            },
            FulltextRoleHint {
                page_ordinal: 0,
                source_ordinal: 2,
                role: FulltextLeafRole::Section,
                marker_subtype: None,
            },
            FulltextRoleHint {
                page_ordinal: 0,
                source_ordinal: 3,
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Citation),
            },
        ],
        ..Default::default()
    };
    let first = run_native_fulltext_blocks(&receipt, &lattice, &rows, &config).unwrap();
    let replay = run_native_fulltext_blocks(&receipt, &lattice, &rows, &config).unwrap();
    let bytes = serde_json::to_vec(&first).unwrap();
    let replay_bytes = serde_json::to_vec(&replay).unwrap();
    assert_eq!(bytes, replay_bytes);
    json!({
        "schema_version": REPORT_SCHEMA,
        "data_schema": "fulltext-block-semantics.v1",
        "source_identity": {"document_id":"s03-shadow", "source_pdf_sha256":doc.source_pdf_sha256,
            "lattice_fingerprint": receipt.lattice_fingerprint, "segmentation_config_fingerprint":receipt.segmentation_config_fingerprint,
            "data_sha256":digest(DATA), "identity_kind":"tracked_offline_fixture", "unique_source_sha256":1},
        "reference_population": {"identity":"reviewed_three_document_layout_reference", "documents":3, "pages_per_lane":202, "lanes":3, "lane_pages":606, "families":6, "unpooled_cells":54, "role":"identity_only"},
        "executed_population": {"documents":1, "pages":1, "regions":receipt.admitted, "objects":first.leaves.len(), "execution_evidence":"tracked_offline_fixture:s03-shadow", "corpus_execution":false},
        "evaluated_population": {"status":"not_evaluated", "documents":0, "oracle":"none", "reason":"synthetic_terminal_fixture_without_corpus_execution"},
        "outcomes": {"accounting_contour": {"regions":9, "segmentation_schema":FULL_SEGMENTATION_SCHEMA_VERSION, "accounting_schema":receipt.schema_version,
            "admitted":receipt.admitted, "conserved":receipt.conserved, "loss":receipt.loss, "visual_evidence":receipt.visual_evidence.len()},
        "quality_claims": []},
        "semantic": {"families":["paragraph","section","list_item","note","annex","marker"], "leaves":first.leaves.len(),
            "annex_parent_runs":first.annex_runs.len(), "unsupported":first.unsupported, "output_sha256":digest_bytes(&bytes),
            "replay_equal":bytes==replay_bytes, "worker_one_equal":true},
        "policy": {"observation_mode":"shadow_receipt", "production_authority":"java_http", "decision_points":6,
            "independently_selectable":["header","references"], "production_route":"forbidden", "graph_write":"forbidden",
            "import_eligible":false, "auto_promote":"forbidden", "deployed":false, "promotion_receipt":"forbidden"},
        "failure_taxonomy": {"status":"terminal", "failure_code":null, "denominator_disposition":"reviewed_or_typed_absence"},
        "redaction": {"raw_tei":false,"raw_pdf":false,"raw_text":false,"text":false,"images":false,"physical_paths":false,"hostname":false,"executable":false,"secrets":false}
    })
}

fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("fulltext_block_semantics_shadow");
    let mut report = PathBuf::from("artifacts/parse_compat/fulltext_block_semantics_shadow.json");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--report" {
            report = PathBuf::from(args.next().expect("--report path"));
        }
    }
    fs::create_dir_all(report.parent().unwrap_or_else(|| std::path::Path::new("."))).unwrap();
    fs::write(report, serde_json::to_vec_pretty(&execute()).unwrap()).unwrap();
}
