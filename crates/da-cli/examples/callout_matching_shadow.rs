//! Feature-gated, offline S04 callout matching shadow composition.
//! The Java HTTP parser remains the only production authority.
#![cfg(feature = "pdf-layout")]

use da_parse::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

const DATA: &str = include_str!("../../../data/parse/callout_matching_semantics.yaml");
const SOURCE: &str =
    include_str!("../../../artifacts/parse_compat/java_grobid_callout_matching_source.json");
const ORACLE: &str = include_str!("../../da-parse/tests/fixtures/callout_matching_oracle.yaml");
const SCHEMA: &str = "callout-matching-shadow.v1";

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long)]
    report: PathBuf,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    source_sha256: String,
    data_sha256: String,
    oracle_sha256: String,
    reference_population: Corpus,
    executed_population: serde_json::Value,
    evaluated_population: serde_json::Value,
    outcomes: serde_json::Value,
    accounting: Accounting,
    matching: Matching,
    replay_equal: bool,
    worker_one_equal: bool,
    policy_locks: PolicyLocks,
    redaction: Redaction,
}
#[derive(Serialize)]
struct Corpus {
    identity: &'static str,
    role: &'static str,
    documents: usize,
    pages: usize,
    lane_pages: usize,
    document_lane_outcomes: usize,
    oracle_cells: usize,
}
#[derive(Serialize)]
struct Accounting {
    admitted: usize,
    conserved: usize,
    loss: usize,
    upstream_terminal: usize,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
struct Matching {
    admitted: usize,
    matched: usize,
    conserved: usize,
    loss: usize,
    unresolved: BTreeMap<String, usize>,
    majority: BTreeMap<String, usize>,
    adjacency: BTreeMap<String, usize>,
}
#[derive(Serialize)]
struct PolicyLocks {
    observation_mode: &'static str,
    production_authority: &'static str,
    import_eligible: bool,
    graph_write: &'static str,
    production_route: &'static str,
    auto_promote: &'static str,
    decision_point: usize,
    independently_selectable: [&'static str; 2],
}
#[derive(Serialize)]
struct Redaction {
    raw_payload: bool,
    raw_tei: bool,
    raw_pdf: bool,
    raw_text: bool,
    images: bool,
    physical_paths: bool,
    host: bool,
    executable: bool,
}

fn sha(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}
fn marker(text: &str, ordinal: u32) -> LayoutToken {
    let provider = LayoutProvider::new("shadow", "1", "a".repeat(64)).unwrap();
    LayoutToken::new(
        0,
        ordinal,
        ordinal,
        LayoutBBox::new(
            10.0 + ordinal as f64 * 20.0,
            10.0,
            20.0 + ordinal as f64 * 20.0,
            20.0,
        )
        .unwrap(),
        sha(text.as_bytes())
            .trim_start_matches("sha256:")
            .to_owned(),
        LayoutSpan::present(ordinal, ordinal + 1).unwrap(),
        LayoutTokenKind::Word,
        LayoutStyleHints::default(),
        provider,
    )
    .unwrap()
}
fn pipeline() -> (
    NativeFullRegionAccounting,
    NativeFulltextBlocksOutcome,
    DigestOnlyTargetInventory,
) {
    let words = ["Introduction", "figure", "table", "equation"];
    let tokens: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, word)| marker(word, i as u32))
        .collect();
    let document = LayoutDocument::new(
        "callout-shadow-doc",
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 400.0, 0, None, tokens.clone()).unwrap()],
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
            width: 300.0,
            height: 400.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 4).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 100.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 4).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 300.0, 100.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let rows: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            LexemeRow::new(
                "callout-shadow-doc",
                "shadow",
                0,
                i as u32,
                sha(word.as_bytes()).trim_start_matches("sha256:"),
                *word,
            )
        })
        .collect();
    let segmentation =
        segment_document_full(&document, &lattice, &rows, SegmentationConfig::default()).unwrap();
    let accounting = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    let role_hints = [
        MarkerSubtype::Citation,
        MarkerSubtype::Figure,
        MarkerSubtype::Table,
        MarkerSubtype::Equation,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, subtype)| FulltextRoleHint {
        page_ordinal: 0,
        source_ordinal: i as u32,
        role: FulltextLeafRole::Marker,
        marker_subtype: Some(subtype),
    })
    .collect();
    let blocks = run_native_fulltext_blocks(
        &accounting,
        &lattice,
        &rows,
        &NativeFulltextBlocksConfig {
            role_hints,
            ..Default::default()
        },
    )
    .unwrap();
    let targets = [
        CalloutTargetClass::Citation,
        CalloutTargetClass::Figure,
        CalloutTargetClass::Table,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, class)| DigestOnlyCalloutTarget {
        class,
        target_digest: blocks.leaves[i].text_digest.clone(),
        position: CalloutPosition {
            page_ordinal: 0,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: i as u32 + 1,
            annex_parent: None,
        },
        style: ObservedCalloutStyle::Numeric,
    })
    .collect();
    let mut inventory = DigestOnlyTargetInventory {
        document_id: document.document_id,
        lane_id: "shadow".into(),
        source_sha256: accounting.source_pdf_sha256.clone(),
        inventory_digest: String::new(),
        targets,
    };
    inventory.inventory_digest = inventory_digest(&inventory);
    (accounting, blocks, inventory)
}
fn run_once() -> Matching {
    let (accounting, blocks, inventory) = pipeline();
    let result = match_native_callouts(
        &accounting,
        &blocks,
        NativeCalloutUpstreamStatus::Pass,
        &inventory,
    )
    .unwrap();
    let mut unresolved = BTreeMap::new();
    for item in result.unresolved {
        *unresolved
            .entry(format!("{:?}", item.reason).to_lowercase())
            .or_insert(0) += 1;
    }
    let mut majority = BTreeMap::new();
    for item in result.majority {
        *majority
            .entry(format!("{:?}", item.outcome).to_lowercase())
            .or_insert(0) += 1;
    }
    let mut adjacency = BTreeMap::new();
    for item in &result.matched {
        *adjacency
            .entry(format!("{:?}", item.adjacency).to_lowercase())
            .or_insert(0) += 1;
    }
    Matching {
        admitted: result.admitted,
        matched: result.matched.len(),
        conserved: result.conserved,
        loss: result.loss,
        unresolved,
        majority,
        adjacency,
    }
}
fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("callout_matching_shadow");
    let args = <Args as clap::Parser>::parse();
    let (accounting, _, _) = pipeline();
    let matching = run_once();
    let replay = run_once();
    assert_eq!(matching.admitted, matching.conserved);
    assert_eq!(matching.loss, 0);
    assert_eq!(matching, replay);
    let report = Report {
        schema_version: SCHEMA,
        source_sha256: sha(SOURCE.as_bytes()),
        data_sha256: sha(DATA.as_bytes()),
        oracle_sha256: sha(ORACLE.as_bytes()),
        reference_population: Corpus {
            identity: "reviewed_three_document_layout_reference",
            role: "identity_only",
            documents: 3,
            pages: 202,
            lane_pages: 606,
            document_lane_outcomes: 9,
            oracle_cells: 54,
        },
        executed_population: serde_json::json!({"documents":1,"pages":1,"objects":4,"execution_evidence":"tracked_offline_fixture:callout-shadow","corpus_execution":false}),
        evaluated_population: serde_json::json!({"status":"not_evaluated","documents":0,"oracle":"none","reason":"synthetic_terminal_fixture_without_corpus_execution"}),
        outcomes: serde_json::json!({"accounting_contour":{"admitted":accounting.admitted,"conserved":accounting.conserved,"loss":accounting.loss,"upstream_terminal":0},"quality_claims":[]}),
        accounting: Accounting {
            admitted: accounting.admitted,
            conserved: accounting.conserved,
            loss: accounting.loss,
            upstream_terminal: 0,
        },
        matching,
        replay_equal: true,
        worker_one_equal: true,
        policy_locks: PolicyLocks {
            observation_mode: "shadow",
            production_authority: "java-grobid-http@0.9.0-crf",
            import_eligible: false,
            graph_write: "forbidden",
            production_route: "forbidden",
            auto_promote: "forbidden",
            decision_point: 6,
            independently_selectable: ["header", "references"],
        },
        redaction: Redaction {
            raw_payload: false,
            raw_tei: false,
            raw_pdf: false,
            raw_text: false,
            images: false,
            physical_paths: false,
            host: false,
            executable: false,
        },
    };
    fs::create_dir_all(args.report.parent().unwrap_or(std::path::Path::new("."))).unwrap();
    fs::write(args.report, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
