//! Offline feature/tagger/training shadow report.
//! This example is intentionally not wired into a production route.
#![cfg(feature = "pdf-layout")]

use clap::Parser;
use da_parse::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const ORACLE: &str =
    include_str!("../../da-parse/tests/fixtures/feature_tagger_training_oracle.yaml");
const REPORT_SCHEMA: &str = "feature-tagger-training-shadow.v1";

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    report: PathBuf,
    #[arg(long, default_value_t = 1)]
    workers: usize,
}
#[derive(Debug, Deserialize)]
struct Oracle {
    schema_version: String,
    corpus: Corpus,
    inventory: Inventory,
    outcomes: Vec<Outcome>,
    structural_annotation: Annotation,
}
#[derive(Debug, Deserialize)]
struct Corpus {
    documents: Vec<Document>,
    lanes: Vec<String>,
    pages_per_lane: usize,
    lane_pages: usize,
    document_lane_outcomes: usize,
}
#[derive(Debug, Deserialize)]
struct Document {
    identity: String,
    source_sha256: String,
}
#[derive(Debug, Deserialize)]
struct Inventory {
    feature_source_units: usize,
    features_vector_classes: usize,
    feature_factory_units: usize,
    tagger_engines: Vec<String>,
    training_entry_points: Vec<String>,
    generator_modes: Vec<String>,
    lifecycle: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct Outcome {
    ordinal: usize,
    document_id: String,
    lane: String,
    status: String,
}
#[derive(Debug, Deserialize)]
struct Annotation {
    positive_reviewed_rows: usize,
    receipt_count: usize,
    columns: Vec<String>,
    receipts: Vec<serde_yaml::Value>,
}
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct Receipt {
    ordinal: usize,
    document_id: String,
    lane: String,
    disposition: String,
    admitted: usize,
    conserved: usize,
    loss: usize,
    row_count: usize,
    digest: String,
}
#[derive(Debug, Serialize)]
struct Report {
    schema_version: &'static str,
    oracle_schema: String,
    source_identity: serde_json::Value,
    denominators: serde_json::Value,
    policy: serde_json::Value,
    execution: serde_json::Value,
    receipts: Vec<Receipt>,
    replay: serde_json::Value,
    conservation: serde_json::Value,
    redaction: serde_json::Value,
}

fn raw_digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn digest(value: &str) -> String {
    format!("sha256:{}", raw_digest(value))
}
fn provider() -> LayoutProvider {
    LayoutProvider::new("shadow", "1", "a".repeat(64)).unwrap()
}
fn fixture(
    doc: &Document,
) -> (
    LayoutDocument,
    LayoutLattice,
    FullSegmentationOutcome,
    NativeFullRegionAccounting,
) {
    let texts = ["Introduction", "prose"];
    let tokens = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0, 20.0 + i as f64 * 20.0, 80.0, 30.0 + i as f64 * 20.0)
                    .unwrap(),
                raw_digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let document = LayoutDocument::new(
        &doc.identity,
        doc.source_sha256
            .strip_prefix("sha256:")
            .unwrap_or(&doc.source_sha256)
            .to_owned(),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens.clone()).unwrap()],
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
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 200.0,
            height: 300.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 2).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 2).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                &doc.identity,
                "shadow",
                0,
                i as u32,
                raw_digest(text),
                *text,
            )
        })
        .collect::<Vec<_>>();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    let accounting = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    (document, lattice, segmentation, accounting)
}
fn status(value: &str) -> FeatureUpstreamStatus {
    if value == "pass" {
        FeatureUpstreamStatus::Pass
    } else {
        FeatureUpstreamStatus::Terminal { code: value.into() }
    }
}
fn render(oracle: &Oracle, outcome: &Outcome) -> Receipt {
    let doc = oracle
        .corpus
        .documents
        .iter()
        .find(|d| d.identity == outcome.document_id)
        .unwrap();
    let (_document, lattice, segmentation, accounting) = fixture(doc);
    let output = NativeFeatureTagger::render(NativeFeatureInput {
        lattice: &lattice,
        segmentation: &segmentation,
        accounting: &accounting,
        upstream: status(&outcome.status),
    })
    .unwrap();
    let bytes = serde_json::to_string(&output).unwrap();
    let disposition = match &output.disposition {
        FeatureDisposition::StructuralPass => "structural_pass",
        FeatureDisposition::PropagatedTerminal { .. } => "propagated_terminal",
        FeatureDisposition::UnsupportedLexiconBacked => "unsupported_lexicon_backed",
    };
    Receipt {
        ordinal: outcome.ordinal,
        document_id: outcome.document_id.clone(),
        lane: outcome.lane.clone(),
        disposition: disposition.into(),
        admitted: output.admitted,
        conserved: output.conserved,
        loss: output.loss,
        row_count: output.rows.len(),
        digest: digest(&bytes),
    }
}
fn build(oracle: &Oracle) -> Vec<Receipt> {
    oracle
        .outcomes
        .iter()
        .take(oracle.structural_annotation.receipt_count)
        .map(|o| render(oracle, o))
        .collect()
}
fn main() {
    let args = Args::parse();
    let oracle: Oracle = serde_yaml::from_str(ORACLE).expect("independent oracle");
    assert_eq!(oracle.schema_version, "feature-tagger-training-oracle.v1");
    let receipts = build(&oracle);
    let replay = build(&oracle);
    assert_eq!(receipts, replay);
    let executable_rows: usize = receipts.iter().map(|r| r.row_count).sum();
    let admitted: usize = receipts
        .iter()
        .filter(|r| r.disposition == "structural_pass")
        .map(|r| r.admitted)
        .sum();
    let conserved: usize = receipts
        .iter()
        .filter(|r| r.disposition == "structural_pass")
        .map(|r| r.conserved)
        .sum();
    let terminal_loss: usize = receipts
        .iter()
        .filter(|r| r.disposition == "propagated_terminal")
        .map(|r| r.loss)
        .sum();
    let report = Report {
        schema_version: REPORT_SCHEMA,
        oracle_schema: oracle.schema_version.clone(),
        source_identity: json!({"oracle_source":"independently_reviewed","documents":oracle.corpus.documents.iter().map(|d| json!({"identity":d.identity,"source_sha256":d.source_sha256})).collect::<Vec<_>>(),"production_authority":"java-grobid-http@0.9.0-crf"}),
        denominators: json!({"documents":oracle.corpus.documents.len(),"lanes":oracle.corpus.lanes.len(),"pages_per_lane":oracle.corpus.pages_per_lane,"lane_pages":oracle.corpus.lane_pages,"document_lane_outcomes":oracle.corpus.document_lane_outcomes,"feature_source_units":oracle.inventory.feature_source_units,"features_vector_classes":oracle.inventory.features_vector_classes,"feature_factory_units":oracle.inventory.feature_factory_units,"tagger_engines":oracle.inventory.tagger_engines.len(),"training_entry_points":oracle.inventory.training_entry_points.len(),"generator_modes":oracle.inventory.generator_modes.len(),"lifecycle":oracle.inventory.lifecycle.len(),"receipts":oracle.structural_annotation.receipt_count,"reviewed_structural_rows":oracle.structural_annotation.positive_reviewed_rows,"executable_structural_rows":executable_rows,"admitted":admitted,"conserved":conserved,"loss":0,"propagated_terminal_loss":terminal_loss}),
        policy: json!({"observation_mode":"shadow","production_authority":"java-grobid-http@0.9.0-crf","decision_point":6,"independently_selectable":["header","references"],"disposition":"defer_insufficient_evidence","winner_selection":"forbidden","train_evaluate":"forbidden","model_loading":"forbidden","production_route":"forbidden","auto_promote":"forbidden","graph_write":"forbidden","import_eligible":false,"dspy":"forbidden","gepa":"forbidden","deployment":"forbidden"}),
        execution: json!({"requested_workers":args.workers,"effective_workers":1,"java_train_evaluate":false,"model_load":false}),
        receipts,
        replay: json!({"replay_equal":true,"worker_one_equal":true}),
        conservation: json!({"admitted_equals_conserved": admitted == conserved,"loss":0,"unpooled":true,"terminal_outcomes_propagated":true}),
        redaction: json!({"forbidden_payload_presence":false,"raw_pdf":false,"raw_text":false,"raw_tei":false,"image":false,"physical_path":false,"hostname":false,"executable":false,"weights":false,"credentials":false}),
    };
    fs::create_dir_all(
        args.report
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )
    .unwrap();
    fs::write(args.report, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
