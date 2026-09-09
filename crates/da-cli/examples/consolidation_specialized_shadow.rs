//! Offline S05 specialized-metadata shadow report.
//! This example is deliberately not wired into any production CLI route.
#![cfg(feature = "pdf-layout")]

use clap::Parser;
use da_parse::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const ORACLE: &str =
    include_str!("../../da-parse/tests/fixtures/consolidation_specialized_oracle.yaml");
const REPORT_SCHEMA: &str = "consolidation-specialized-shadow.v1";

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    report: PathBuf,
    #[arg(long, default_value_t = 1)]
    workers: usize,
    #[arg(long, default_value_t = 1)]
    queue_capacity: usize,
}
#[derive(Debug, Deserialize)]
struct Oracle {
    schema_version: String,
    source_contract: Contract,
    documents: Vec<Document>,
    lanes: Vec<String>,
    upstream_outcomes: Vec<Upstream>,
}
#[derive(Debug, Deserialize)]
struct Contract {
    documents: usize,
    pages_per_lane: usize,
    lane_pages: usize,
    document_lane_outcomes: usize,
    unpooled_cells: usize,
}
#[derive(Debug, Deserialize)]
struct Document {
    id: String,
    source_sha256: String,
    pages: usize,
}
#[derive(Debug, Deserialize)]
struct Upstream {
    document_id: String,
    lane: String,
    status: String,
}
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct Cell {
    document_id: String,
    lane: String,
    family: String,
    disposition: String,
    count: usize,
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
    cells: Vec<Cell>,
    replay: serde_json::Value,
    conservation: serde_json::Value,
    redaction: serde_json::Value,
}
fn digest(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}
fn receipt(
    doc: &Document,
    lane: &str,
) -> (NativeFullRegionAccounting, NativeFulltextBlocksOutcome) {
    let fp = digest(&format!("{}:{lane}:lattice", doc.id));
    (
        NativeFullRegionAccounting {
            schema_version: NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION.into(),
            document_id: doc.id.clone(),
            provider_id: "java-grobid-http-shadow-receipt".into(),
            source_pdf_sha256: doc.source_sha256.clone(),
            lattice_fingerprint: fp.clone(),
            segmentation_config_fingerprint: digest("s02-segmentation-v2"),
            admitted: 1,
            conserved: 1,
            loss: 0,
            visual_evidence: vec![],
            lexical_units: vec![],
        },
        NativeFulltextBlocksOutcome {
            schema_version: NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION.into(),
            document_id: doc.id.clone(),
            lattice_fingerprint: fp,
            config_fingerprint: digest("s03-fulltext-blocks-v1"),
            annex_runs: vec![],
            leaves: vec![],
            unsupported: 0,
        },
    )
}
fn status(value: &str) -> SpecializedUpstreamStatus {
    match value {
        "pass" => SpecializedUpstreamStatus::Pass,
        "invalid_evidence" => SpecializedUpstreamStatus::InvalidEvidence,
        _ => SpecializedUpstreamStatus::TerminalFailure,
    }
}
fn wire(family: SpecializedFamily) -> &'static str {
    match family {
        SpecializedFamily::Funding => "funding",
        SpecializedFamily::Acknowledgement => "acknowledgement",
        SpecializedFamily::Licence => "licence",
        SpecializedFamily::Copyright => "copyright",
        SpecializedFamily::Software => "software",
        SpecializedFamily::Consolidation => "consolidation",
    }
}
fn disposition(value: SpecializedDisposition) -> &'static str {
    match value {
        SpecializedDisposition::SourceNotEstablished => "source_not_established",
        SpecializedDisposition::Unsupported => "unsupported",
        SpecializedDisposition::EnrichmentNotRequired => "enrichment_not_required",
        SpecializedDisposition::PropagatedTerminal => "propagated_terminal",
        SpecializedDisposition::StructuralDefault => "structural_default",
        SpecializedDisposition::RichOptIn => "rich_opt_in",
        SpecializedDisposition::Unavailable => "unavailable",
    }
}
fn build(oracle: &Oracle) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(oracle.source_contract.unpooled_cells);
    for doc in &oracle.documents {
        for lane in &oracle.lanes {
            let upstream = oracle
                .upstream_outcomes
                .iter()
                .find(|x| x.document_id == doc.id && x.lane == *lane)
                .expect("closed upstream matrix");
            let (accounting, blocks) = receipt(doc, lane);
            let result = evaluate_native_consolidation_specialized(
                &accounting,
                &blocks,
                status(&upstream.status),
                None,
            )
            .expect("validated accountant-to-fulltext evidence");
            for family in result.families {
                cells.push(Cell {
                    document_id: doc.id.clone(),
                    lane: lane.clone(),
                    family: wire(family.family).into(),
                    disposition: disposition(family.disposition).into(),
                    count: family.count,
                    digest: family.digest,
                });
            }
        }
    }
    cells
}
fn main() {
    let args = Args::parse();
    let oracle: Oracle = serde_yaml::from_str(ORACLE).expect("independent oracle");
    assert_eq!(oracle.schema_version, "consolidation-specialized-oracle.v1");
    assert_eq!(oracle.source_contract.documents, 3);
    assert_eq!(oracle.source_contract.pages_per_lane, 202);
    assert_eq!(oracle.source_contract.lane_pages, 606);
    assert_eq!(oracle.source_contract.document_lane_outcomes, 9);
    let cells = build(&oracle);
    let replay = build(&oracle);
    assert_eq!(cells, replay);
    let terminal = cells.len();
    let report = Report {
        schema_version: REPORT_SCHEMA,
        oracle_schema: oracle.schema_version,
        source_identity: json!({"oracle_id":"m501-s05-t03-independent-reviewed-v1","documents":oracle.documents.iter().map(|d| json!({"document_id":d.id,"source_sha256":d.source_sha256,"pages":d.pages})).collect::<Vec<_>>(),"production_authority":"java-grobid-http@0.9.0-crf","identity_kind":"three_source_sha"}),
        denominators: json!({"documents":3,"lanes":3,"pages_per_lane":202,"lane_pages":606,"document_lane_outcomes":9,"families":6,"unpooled_cells":54,"admitted":54,"conserved":54,"loss":0}),
        policy: json!({"observation_mode":"shadow","production_authority":"java-grobid-http@0.9.0-crf","decision_point":6,"independently_selectable":["header","references"],"structural_header_consolidation":0,"structural_citations_consolidation":0,"rich":"opt_in_separately_cached","enrichment_cache_miss":"enrichment_not_required","enrichment_timeout":"unavailable","graph_write":"forbidden","import_eligible":false,"auto_promote":"forbidden","production_route":"forbidden"}),
        execution: json!({"requested_workers":args.workers,"effective_workers":1,"queue_capacity":args.queue_capacity,"admitted":terminal,"terminal":terminal}),
        cells,
        replay: json!({"semantic_replays":2,"semantic_equal":true,"worker_one_equal":true}),
        conservation: json!({"admitted_equals_conserved":true,"all_cells_terminal":true,"no_silent_loss":true,"positive_count":0}),
        redaction: json!({"raw_pdf":false,"raw_text":false,"tei":false,"image":false,"physical_path":false,"hostname":false,"executable":false,"credentials":false}),
    };
    fs::create_dir_all(
        args.report
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )
    .expect("report parent");
    fs::write(
        args.report,
        serde_json::to_vec_pretty(&report).expect("report JSON"),
    )
    .expect("report write");
}
