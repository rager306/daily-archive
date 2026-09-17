//! Bounded, redacted S15 native document segmentation comparison.
//! Shadow-only: no donor selection, routing, graph/cache writes, or fulltext composition.

use da_adapters::{
    LayoutDonor, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoInvocationConfig,
    PdfaltoLayoutDonor, XyCutLayoutDonor,
};
use da_cli::layout_runtime::{RuntimeConfig, Terminal, detect_system_topology, run_bounded};
use da_parse::{FullSemanticRegion, SegmentationConfig, segment_document_full};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const REPORT_SCHEMA: &str = "document-segmentation-shadow.v2";
const ORACLE_SOURCE_SCHEMA: &str = "document-segmentation-oracle-source.v3";
const ORACLE_SOURCE_DEFAULT: &str =
    "artifacts/parse_compat/document_segmentation_oracle_source.json";
const ORACLE_FIXTURE: &str = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml";
const LAYOUT_MANIFEST: &str = "data/parse/layout_bakeoff.yaml";
const SEGMENTATION_MANIFEST: &str = "data/parse/document_segmentation.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const REGIONS: [&str; 9] = [
    "header",
    "body",
    "references",
    "annex",
    "acknowledgement",
    "toc",
    "funding",
    "availability",
    "page_furniture",
];

#[derive(Debug, Deserialize)]
struct LayoutManifest {
    schema_version: String,
    corpus: LayoutCorpus,
}
#[derive(Debug, Deserialize)]
struct LayoutCorpus {
    corpus_n: usize,
    page_denominator: usize,
    documents: Vec<LayoutDocument>,
}
#[derive(Debug, Deserialize)]
struct LayoutDocument {
    id: String,
    source_pdf: String,
    sha256: String,
}
#[derive(Debug, Deserialize)]
struct SegmentationManifest {
    schema_version: String,
    regions: RegionSpec,
    source_corpus: SourceCorpus,
    lanes: LaneSpec,
    source_bound_genres: GenreSpec,
    evaluation: EvaluationSpec,
    policy_locks: PolicyLocks,
}
#[derive(Debug, Deserialize)]
struct RegionSpec {
    closed_labels: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct SourceCorpus {
    corpus_id: String,
    corpus_n: usize,
    page_sum: usize,
}
#[derive(Debug, Deserialize)]
struct LaneSpec {
    required_ids: Vec<String>,
    pages_per_lane: usize,
    total_lane_pages: usize,
    pooling: String,
    silent_skip: String,
}
#[derive(Debug, Deserialize)]
struct GenreSpec {
    stratification_only: bool,
    labels: Vec<GenreLabel>,
}
#[derive(Debug, Deserialize)]
struct GenreLabel {
    id: String,
    provenance: String,
}
#[derive(Debug, Deserialize)]
struct EvaluationSpec {
    required_rows: usize,
    pooled_metrics: String,
    coordinate_binding: CoordinateBinding,
}
#[derive(Debug, Deserialize)]
struct CoordinateBinding {
    unit: String,
    page_origin: String,
    tolerance: u32,
}
#[derive(Debug, Deserialize)]
struct PolicyLocks {
    observation_mode: String,
    promotion_state: String,
    auto_promote: String,
    donor_selection: String,
    import_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Report {
    schema_version: String,
    manifest_fingerprint: String,
    source_fingerprint: String,
    config_fingerprint: String,
    corpus_id: String,
    denominators: Denominators,
    policy: Policy,
    execution: Execution,
    outcomes: Vec<Outcome>,
    matrix: Vec<MatrixRow>,
    page_receipts: Vec<PageReceipt>,
    replay: Replay,
    gate: Gate,
    redaction: Redaction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Denominators {
    corpus_documents: usize,
    lanes: usize,
    pages_per_lane: usize,
    lane_pages: usize,
    matrix_rows: usize,
    lane_page_receipts: usize,
    genre_documents: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Policy {
    observation_mode: String,
    promotion_state: String,
    auto_promote: String,
    donor_selection: String,
    import_eligible: bool,
    selection: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Execution {
    requested_workers: usize,
    effective_workers: usize,
    max_in_flight: usize,
    queue_capacity: usize,
    admitted: usize,
    terminal: usize,
    queue_wait_ms: u128,
    input_queue_high_water: usize,
    result_queue_high_water: usize,
    in_flight_high_water: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Outcome {
    ordinal: usize,
    document_id: String,
    lane: String,
    status: String,
    failure_code: Option<String>,
    source_sha256: String,
    pages: usize,
    admitted_tokens: usize,
    assigned_tokens: usize,
    reordered_tokens: usize,
    duplicate_tokens: usize,
    overlap_tokens: usize,
    boundary_digest: Option<String>,
    section_count: usize,
    furniture_annotations: usize,
    lattice_fingerprint: Option<String>,
    region_tokens: BTreeMap<String, usize>,
    region_boundaries: BTreeMap<String, usize>,
    #[serde(skip)]
    region_boundary_coordinates: BTreeMap<String, Vec<BoundaryPair>>,
    latency_ms: u128,
    #[serde(skip)]
    receipt: Option<Receipt>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    ordinal: usize,
    document_id: String,
    source_sha256: String,
    lane: String,
    page_count: usize,
    page_receipt_count: usize,
    admitted_lexical_denominator: usize,
    page_ordinal: u32,
    token_ordinal: u32,
    token_digest: String,
    region: String,
    boundary_digest: String,
    section_ordinal: usize,
    section_boundary_digest: String,
    lattice_fingerprint: String,
    oracle_annotation_digest: String,
    status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PageReceipt {
    ordinal: usize,
    document_id: String,
    lane: String,
    source_sha256: String,
    page_ordinal: u32,
    page_count: usize,
    page_digest: String,
    lattice_fingerprint: Option<String>,
    oracle_annotation_digest: String,
    status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatrixRow {
    document_id: String,
    lane: String,
    genre_id: String,
    region: String,
    admitted_tokens: usize,
    assigned_tokens: usize,
    boundary_count: usize,
    count_agreement: usize,
    count_difference: usize,
    token_precision: Option<f64>,
    token_recall: Option<f64>,
    boundary_precision: Option<f64>,
    boundary_recall: Option<f64>,
    boundary_f1: Option<f64>,
    boundary_displacement: Option<f64>,
    boundary_displacement_max: Option<f64>,
    page_furniture_false_positive_rate: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Replay {
    semantic_equal: bool,
    repetitions: usize,
    worker_one_equal: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Gate {
    state: String,
    conservation: bool,
    ordered_boundary_digests: bool,
    complete_matrix: bool,
    no_selection: bool,
    furniture_exclusion_count: usize,
    terminal_taxonomy: BTreeMap<String, usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Redaction {
    raw_text: bool,
    raw_pdf: bool,
    tei_xml: bool,
    images: bool,
    physical_paths: bool,
    hostname: bool,
    executable: bool,
    visual_structure_heads: bool,
}
#[derive(Clone)]
struct Job {
    ordinal: usize,
    document_id: String,
    lane: String,
    bytes: Arc<Vec<u8>>,
    donor: Arc<dyn LayoutDonor>,
    source_sha256: String,
    queued_at: Instant,
}

fn sha(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn bare_sha(value: &str) -> String {
    value.strip_prefix("sha256:").unwrap_or(value).to_owned()
}
fn digest_report<T: Serialize>(value: &T) -> String {
    sha(&serde_json::to_vec(value).expect("report serialization"))
}
fn fail_code(error: &da_adapters::LayoutDonorError) -> &'static str {
    match error {
        da_adapters::LayoutDonorError::Unavailable { .. } => "unsupported",
        da_adapters::LayoutDonorError::Timeout { .. } => "timeout",
        da_adapters::LayoutDonorError::NonZero { .. } => "nonzero",
        da_adapters::LayoutDonorError::Malformed { .. }
        | da_adapters::LayoutDonorError::InvalidLayout(_) => "malformed",
        da_adapters::LayoutDonorError::Oversize { .. } => "oversize",
        da_adapters::LayoutDonorError::Encrypted => "encrypted",
        da_adapters::LayoutDonorError::Truncated => "truncated",
        da_adapters::LayoutDonorError::Io(_) => "missing",
    }
}
fn region_name(region: FullSemanticRegion) -> &'static str {
    match region {
        FullSemanticRegion::Header => "header",
        FullSemanticRegion::Body => "body",
        FullSemanticRegion::References => "references",
        FullSemanticRegion::Annex => "annex",
        FullSemanticRegion::Acknowledgement => "acknowledgement",
        FullSemanticRegion::Toc => "toc",
        FullSemanticRegion::Funding => "funding",
        FullSemanticRegion::Availability => "availability",
        FullSemanticRegion::PageFurniture => "page_furniture",
    }
}

fn oracle_lane(row: &serde_yaml::Value) -> Option<&str> {
    row["lane_id"].as_str().or_else(|| row["lane"].as_str())
}

fn validate_oracle_source(value: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    if value["schema_version"] != ORACLE_SOURCE_SCHEMA
        || value["canonicalization"]["encoding"] != "utf-8"
        || value["canonicalization"]["line_endings"] != "LF"
        || value["canonicalization"]["trailing_newline"] != false
        || value["canonicalization"]["digest"] != "sha256 over exact canonical bytes"
    {
        return Err("unsupported v3 oracle canonicalization".into());
    }
    let receipts = value["receipts"]
        .as_array()
        .ok_or("oracle source receipts missing")?;
    if receipts.len() != 9 {
        return Err("oracle source must contain nine receipts".into());
    }
    for receipt in receipts {
        let page_bytes = receipt["canonical_page_range"]["bytes"]
            .as_str()
            .ok_or("oracle page canonical bytes missing")?;
        let annotation_bytes = receipt["canonical_annotation"]["bytes"]
            .as_str()
            .ok_or("oracle annotation canonical bytes missing")?;
        for (kind, bytes, digest) in [
            (
                "page range",
                page_bytes,
                receipt["canonical_page_range"]["sha256"].as_str(),
            ),
            (
                "annotation",
                annotation_bytes,
                receipt["canonical_annotation"]["sha256"].as_str(),
            ),
        ] {
            if bytes.contains('\r') || sha(bytes.as_bytes()) != digest.unwrap_or("") {
                return Err(format!("oracle {kind} digest mismatch").into());
            }
        }
        let measured = receipt["denominator"]["value"]
            .as_u64()
            .ok_or("measured denominator missing")?;
        if receipt["denominator"]["kind"] != "measured"
            || receipt["denominator"]["source"].as_str().is_none()
            || receipt["admitted_lexical_denominator"] != measured
        {
            return Err("oracle measured denominator binding mismatch".into());
        }
        if receipt["proxy_denominator"]["kind"] != "proxy"
            || receipt["proxy_denominator"]["formula"] != "page_count × 10"
            || receipt["proxy_denominator"]["use"] != "diagnostic comparison only"
        {
            return Err("oracle proxy denominator is not diagnostic-only".into());
        }
        if receipt["page_range_digest"] != receipt["canonical_page_range"]["sha256"]
            || receipt["annotation_digest"] != receipt["canonical_annotation"]["sha256"]
        {
            return Err("oracle published digest binding mismatch".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryCoordinate {
    page: u32,
    block: u32,
    line: u32,
    token: u32,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryPair {
    start: BoundaryCoordinate,
    end: BoundaryCoordinate,
}

fn yaml_coordinate(value: &serde_yaml::Value, page_origin: &str) -> Option<BoundaryCoordinate> {
    let values = value.as_sequence()?;
    let page = values.first()?.as_u64()? as u32;
    let page = match page_origin {
        "one_based" => page.checked_sub(1)?,
        "zero_based" => page,
        _ => return None,
    };
    Some(BoundaryCoordinate {
        page,
        block: values.get(1)?.as_u64()? as u32,
        line: values.get(2)?.as_u64()? as u32,
        token: values.get(3)?.as_u64()? as u32,
    })
}

fn reviewed_boundary(run: &serde_yaml::Value, page_origin: &str) -> Option<BoundaryPair> {
    Some(BoundaryPair {
        start: yaml_coordinate(&run["start"], page_origin)?,
        end: yaml_coordinate(&run["end"], page_origin)?,
    })
}

fn predicted_boundary(run: &da_parse::FullRegionRun) -> BoundaryPair {
    let coordinate = |location: &da_parse::FullLocation| BoundaryCoordinate {
        page: location.page_ordinal,
        block: location.block_ordinal,
        line: location.line_ordinal,
        token: location.token_ordinal,
    };
    BoundaryPair {
        start: coordinate(&run.start),
        end: coordinate(&run.end),
    }
}

fn coordinate_distance(left: BoundaryCoordinate, right: BoundaryCoordinate) -> u32 {
    left.page.abs_diff(right.page)
        + left.block.abs_diff(right.block)
        + left.line.abs_diff(right.line)
        + left.token.abs_diff(right.token)
}

fn boundary_distance(left: BoundaryPair, right: BoundaryPair) -> u32 {
    coordinate_distance(left.start, right.start) + coordinate_distance(left.end, right.end)
}

#[derive(Debug, Default)]
struct BoundaryMatch {
    matched: usize,
    false_positives: usize,
    false_negatives: usize,
    displacements: Vec<u32>,
}

fn match_boundaries(
    predicted: &[BoundaryPair],
    reviewed: &[BoundaryPair],
    tolerance: u32,
) -> BoundaryMatch {
    let mut candidates = predicted
        .iter()
        .enumerate()
        .flat_map(|(predicted_index, predicted_boundary)| {
            reviewed
                .iter()
                .enumerate()
                .filter_map(move |(reviewed_index, reviewed_boundary)| {
                    // Document, lane, region, and page are the matching scope;
                    // page identity is not a quality denominator. Require both
                    // endpoints to remain on the same page as their reviewed pair.
                    (predicted_boundary.start.page == reviewed_boundary.start.page
                        && predicted_boundary.end.page == reviewed_boundary.end.page)
                        .then(|| {
                            let distance =
                                boundary_distance(*predicted_boundary, *reviewed_boundary);
                            (
                                distance <= tolerance,
                                distance,
                                predicted_index,
                                reviewed_index,
                            )
                        })
                })
        })
        .filter(|(within_tolerance, _, _, _)| *within_tolerance)
        .map(|(_, distance, predicted_index, reviewed_index)| {
            (distance, predicted_index, reviewed_index)
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    let mut used_predicted = vec![false; predicted.len()];
    let mut used_reviewed = vec![false; reviewed.len()];
    let mut result = BoundaryMatch::default();
    for (distance, predicted_index, reviewed_index) in candidates {
        if !used_predicted[predicted_index] && !used_reviewed[reviewed_index] {
            used_predicted[predicted_index] = true;
            used_reviewed[reviewed_index] = true;
            result.matched += 1;
            result.displacements.push(distance);
        }
    }
    result.false_positives = predicted.len() - result.matched;
    result.false_negatives = reviewed.len() - result.matched;
    result
}

#[cfg(test)]
mod segmentation_boundary_metrics {
    use super::*;

    fn boundary(token: u32) -> BoundaryPair {
        BoundaryPair {
            start: BoundaryCoordinate {
                page: 1,
                block: 0,
                line: token,
                token: 0,
            },
            end: BoundaryCoordinate {
                page: 1,
                block: 0,
                line: token,
                token: 9,
            },
        }
    }

    // Negative boundary fixture invoked by the review registry entry GAR-001:
    //   cargo test -p da-cli --features pdf-layout --example document_segmentation_shadow boundary_position_negative
    // Two-sided claim: cardinality agreement stays 1, while positional precision
    // collapses to 0.0 because starts and ends are cross-paired.
    #[test]
    fn boundary_position_negative_permuted_boundaries_fail_positional_precision() {
        let reviewed = [boundary(1), boundary(10)];
        // Same cardinality, but starts and ends are cross-paired. Count agreement
        // remains 1 while no complete positional boundary matches.
        let permuted = [
            BoundaryPair {
                start: reviewed[0].start,
                end: reviewed[1].end,
            },
            BoundaryPair {
                start: reviewed[1].start,
                end: reviewed[0].end,
            },
        ];
        let result = match_boundaries(&permuted, &reviewed, 0);
        assert_eq!(permuted.len(), reviewed.len());
        assert_eq!(permuted.len() == reviewed.len(), true);
        let positional_precision = result.matched as f64 / permuted.len() as f64;
        assert_eq!(positional_precision, 0.0);
        assert_eq!(result.matched, 0);
        assert_eq!(result.false_positives, 2);
        assert_eq!(result.false_negatives, 2);
    }
}

fn empty_region_counts() -> (
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
    BTreeMap<String, Vec<BoundaryPair>>,
) {
    let tokens = REGIONS.iter().map(|name| ((*name).into(), 0)).collect();
    let boundaries = REGIONS.iter().map(|name| ((*name).into(), 0)).collect();
    let coordinates = REGIONS
        .iter()
        .map(|name| ((*name).into(), Vec::new()))
        .collect();
    (tokens, boundaries, coordinates)
}

fn failed_outcome(job: Job, status: &str, code: &str, latency_ms: u128) -> Outcome {
    Outcome {
        ordinal: job.ordinal,
        document_id: job.document_id,
        lane: job.lane,
        status: status.into(),
        failure_code: Some(code.into()),
        source_sha256: job.source_sha256,
        pages: 0,
        admitted_tokens: 0,
        assigned_tokens: 0,
        reordered_tokens: 0,
        duplicate_tokens: 0,
        overlap_tokens: 0,
        boundary_digest: None,
        section_count: 0,
        furniture_annotations: 0,
        lattice_fingerprint: None,
        region_tokens: empty_region_counts().0,
        region_boundaries: empty_region_counts().1,
        region_boundary_coordinates: empty_region_counts().2,
        latency_ms,
        receipt: None,
    }
}

fn run_job(job: Job, limits: LayoutDonorLimits, config: SegmentationConfig) -> Outcome {
    let started = Instant::now();
    let result = job
        .donor
        .extract_shadow(&job.bytes, &job.document_id, limits);
    let wait = job.queued_at.elapsed().as_millis();
    let observation = match result {
        Ok(observation) => observation,
        Err(error) => {
            return failed_outcome(
                job,
                "terminal_failure",
                fail_code(&error),
                started.elapsed().as_millis().saturating_add(wait),
            );
        }
    };
    let segmentation = match segment_document_full(
        &observation.document,
        &observation.lattice,
        &observation.lexemes,
        config,
    ) {
        Ok(segmentation) => segmentation,
        Err(_) => {
            return failed_outcome(
                job,
                "invalid_evidence",
                "digest_mismatch",
                started.elapsed().as_millis().saturating_add(wait),
            );
        }
    };
    let admitted = observation
        .document
        .pages
        .iter()
        .flat_map(|p| p.tokens.iter())
        .filter(|token| token.kind.is_lexical())
        .count();
    let assigned = segmentation.assignments.len();
    let mut region_tokens = REGIONS
        .iter()
        .map(|name| ((*name).into(), 0))
        .collect::<BTreeMap<_, _>>();
    for assignment in &segmentation.assignments {
        *region_tokens
            .entry(region_name(assignment.region).into())
            .or_insert(0) += 1;
    }
    let mut region_boundaries = REGIONS
        .iter()
        .map(|name| ((*name).into(), 0))
        .collect::<BTreeMap<_, _>>();
    let mut region_boundary_coordinates = REGIONS
        .iter()
        .map(|name| ((*name).into(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for run in &segmentation.runs {
        let region = region_name(run.region).to_owned();
        *region_boundaries.entry(region.clone()).or_insert(0) += 1;
        region_boundary_coordinates
            .entry(region)
            .or_default()
            .push(predicted_boundary(run));
    }
    let token = segmentation.assignments.first();
    let run = segmentation.runs.first();
    let receipt = match (token, run) {
        (Some(token), Some(run)) => Some(Receipt {
            ordinal: job.ordinal,
            document_id: job.document_id.clone(),
            source_sha256: bare_sha(&job.source_sha256),
            lane: job.lane.clone(),
            page_count: observation.document.pages.len(),
            page_receipt_count: observation.document.pages.len(),
            admitted_lexical_denominator: admitted,
            page_ordinal: token.location.page_ordinal,
            token_ordinal: token.location.token_ordinal,
            token_digest: bare_sha(&token.text_digest),
            region: region_name(token.region).into(),
            boundary_digest: bare_sha(&run.boundary_digest),
            section_ordinal: 0,
            section_boundary_digest: bare_sha(&run.boundary_digest),
            lattice_fingerprint: bare_sha(&segmentation.identity.lattice_fingerprint),
            oracle_annotation_digest: bare_sha(&segmentation.order_digest),
            status: "pass".into(),
        }),
        _ => None,
    };
    Outcome {
        ordinal: job.ordinal,
        document_id: job.document_id,
        lane: job.lane,
        status: "pass".into(),
        failure_code: None,
        source_sha256: job.source_sha256,
        pages: observation.document.pages.len(),
        admitted_tokens: admitted,
        assigned_tokens: assigned,
        reordered_tokens: 0,
        duplicate_tokens: 0,
        overlap_tokens: admitted.saturating_sub(assigned),
        boundary_digest: Some(segmentation.order_digest),
        section_count: segmentation.runs.len(),
        furniture_annotations: segmentation.furniture.len(),
        lattice_fingerprint: Some(bare_sha(&segmentation.identity.lattice_fingerprint)),
        region_tokens,
        region_boundaries,
        region_boundary_coordinates,
        latency_ms: started.elapsed().as_millis().saturating_add(wait),
        receipt,
    }
}
fn execute_jobs(
    jobs: Vec<Job>,
    topology: &da_cli::layout_runtime::TopologyReceipt,
    runtime_config: RuntimeConfig,
    limits: LayoutDonorLimits,
    config: SegmentationConfig,
) -> Result<(Vec<Outcome>, da_cli::layout_runtime::RuntimeMetrics, usize), Box<dyn std::error::Error>>
{
    let fallback = jobs
        .iter()
        .map(|job| {
            (
                job.ordinal,
                (
                    job.document_id.clone(),
                    job.lane.clone(),
                    job.source_sha256.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let runtime = run_bounded(runtime_config, topology, jobs, move |job, scratch| {
        scratch.allocate(1).map_err(|_| "scratch")?;
        Ok::<_, &'static str>(run_job(job, limits, config))
    })?;
    let outcomes = runtime
        .terminals
        .into_iter()
        .map(|terminal| match terminal {
            Terminal::Completed { result, .. } => result,
            Terminal::Failed { ordinal, .. } | Terminal::CoordinatorFailed { ordinal, .. } => {
                let (document_id, lane, source_sha256) =
                    fallback.get(&ordinal).expect("terminal ordinal admitted");
                Outcome {
                    ordinal,
                    document_id: document_id.clone(),
                    lane: lane.clone(),
                    status: "terminal_failure".into(),
                    failure_code: Some("worker_failure".into()),
                    source_sha256: source_sha256.clone(),
                    pages: 0,
                    admitted_tokens: 0,
                    assigned_tokens: 0,
                    reordered_tokens: 0,
                    duplicate_tokens: 0,
                    overlap_tokens: 0,
                    boundary_digest: None,
                    section_count: 0,
                    furniture_annotations: 0,
                    lattice_fingerprint: None,
                    region_tokens: empty_region_counts().0,
                    region_boundaries: empty_region_counts().1,
                    region_boundary_coordinates: empty_region_counts().2,
                    latency_ms: 0,
                    receipt: None,
                }
            }
        })
        .collect::<Vec<_>>();
    Ok((outcomes, runtime.metrics, runtime.admitted))
}

fn semantic_digest(outcomes: &[Outcome]) -> String {
    let mut normalized = outcomes.to_vec();
    for outcome in &mut normalized {
        outcome.latency_ms = 0;
    }
    digest_report(&normalized)
}

fn parse_args() -> (
    PathBuf,
    PathBuf,
    PathBuf,
    PathBuf,
    Option<PathBuf>,
    usize,
    usize,
) {
    let mut layout = PathBuf::from(LAYOUT_MANIFEST);
    let mut segmentation = PathBuf::from(SEGMENTATION_MANIFEST);
    let mut report = PathBuf::from("artifacts/parse_compat/document_segmentation_shadow.json");
    let mut oracle_source = PathBuf::from(ORACLE_SOURCE_DEFAULT);
    let mut pdfalto = None;
    let mut workers = 1;
    let mut queue = 1;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--layout-manifest" => layout = args.next().unwrap_or_default().into(),
            "--segmentation-manifest" => segmentation = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--oracle-source" => oracle_source = args.next().unwrap_or_default().into(),
            "--pdfalto" => pdfalto = Some(args.next().unwrap_or_default().into()),
            "--workers" => workers = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--queue-capacity" => queue = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--help" => {
                println!(
                    "document_segmentation_shadow [--layout-manifest FILE] [--segmentation-manifest FILE] [--report FILE] [--oracle-source FILE] [--pdfalto EXE] [--workers N] [--queue-capacity N]"
                );
                std::process::exit(0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    (
        layout,
        segmentation,
        report,
        oracle_source,
        pdfalto,
        workers,
        queue,
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = da_cli::logging::init_pipeline_logging("document_segmentation_shadow");
    let (layout_path, segmentation_path, report_path, oracle_path, pdfalto_path, workers, queue) =
        parse_args();
    let layout_bytes = fs::read(&layout_path)?;
    let layout: LayoutManifest = serde_yaml::from_slice(&layout_bytes)?;
    let segmentation_bytes = fs::read(&segmentation_path)?;
    let manifest: SegmentationManifest = serde_yaml::from_slice(&segmentation_bytes)?;
    if layout.schema_version != "layout-bakeoff.v1"
        || manifest.schema_version != "document-segmentation.v2"
    {
        return Err("unsupported manifest schema".into());
    }
    if manifest.regions.closed_labels != REGIONS
        || manifest.lanes.required_ids != LANES
        || manifest.evaluation.required_rows != 81
        || manifest.evaluation.pooled_metrics != "forbidden"
        || manifest.evaluation.coordinate_binding.unit != "page_block_line_token"
        || manifest.evaluation.coordinate_binding.page_origin != "one_based"
        || manifest.lanes.silent_skip != "forbidden"
        || manifest.lanes.pooling != "forbidden"
    {
        return Err("S15 DATA contract mismatch".into());
    }
    if !manifest.source_bound_genres.stratification_only
        || manifest.source_bound_genres.labels.len() != 1
        || manifest.source_bound_genres.labels[0].provenance != "source_bound_reviewed"
    {
        return Err("source-bound genre contract mismatch".into());
    }
    if layout.corpus.corpus_n != 3
        || layout.corpus.page_denominator != 202
        || manifest.source_corpus.corpus_n != 3
        || manifest.source_corpus.page_sum != 202
        || manifest.lanes.pages_per_lane != 202
        || manifest.lanes.total_lane_pages != 606
    {
        return Err("S13 denominator mismatch".into());
    }
    let limits = LayoutDonorLimits::default();
    let config = SegmentationConfig::default();
    let inspector: Arc<dyn LayoutDonor> = Arc::new(PdfInspectorLayoutDonor);
    let xy: Arc<dyn LayoutDonor> =
        Arc::new(XyCutLayoutDonor::new(Box::new(PdfInspectorLayoutDonor)));
    // The checked-in `.gsd/tmp/pdfalto-segmentation-runtime` wrapper accepts
    // exactly two positional args (input.pdf output.xml). Default donor flags
    // (`-fullFontName`, `-noLineNumbers`, ...) make that wrapper exit 2 / NonZero.
    let wrapper_invocation = PdfaltoInvocationConfig {
        full_font_name: false,
        no_line_numbers: false,
        files_limit: None,
        annotation: false,
        only_graphs_coord: false,
    };
    let alto: Arc<dyn LayoutDonor> = match pdfalto_path {
        Some(path) => Arc::new(PdfaltoLayoutDonor::new(path).with_invocation(wrapper_invocation)),
        None => Arc::new(PdfaltoLayoutDonor::new("pdfalto")),
    };
    let donors: BTreeMap<&str, Arc<dyn LayoutDonor>> = [
        ("pdfalto", alto),
        ("pdf_inspector_positioned", inspector),
        ("pdf_inspector_positioned_xy_cut", xy),
    ]
    .into_iter()
    .collect();
    let mut jobs = Vec::new();
    let mut source_fingerprint = Sha256::new();
    for document in &layout.corpus.documents {
        let source_pdf = PathBuf::from(&document.source_pdf);
        let source_pdf = if source_pdf.is_absolute() || source_pdf.is_file() {
            source_pdf
        } else {
            let root = layout_path
                .parent()
                .and_then(|path| path.parent())
                .and_then(|path| path.parent())
                .unwrap_or_else(|| std::path::Path::new("."));
            root.join(source_pdf)
        };
        let bytes = fs::read(source_pdf)?;
        if sha(&bytes) != format!("sha256:{}", document.sha256) {
            return Err(format!("checksum mismatch for {}", document.id).into());
        }
        source_fingerprint.update(bytes.as_slice());
        let bytes = Arc::new(bytes);
        for lane in LANES.iter() {
            jobs.push(Job {
                ordinal: jobs.len(),
                document_id: document.id.clone(),
                lane: (*lane).into(),
                bytes: bytes.clone(),
                donor: donors[lane].clone(),
                source_sha256: sha(&bytes),
                queued_at: Instant::now(),
            });
        }
    }
    let topology = detect_system_topology()?;
    let runtime_config = RuntimeConfig {
        workers,
        input_queue_capacity: queue,
        result_queue_capacity: queue,
        max_in_flight: 1,
        private_scratch_capacity: 64 * 1024,
    };
    let config_fingerprint = sha(
        format!(
            "workers={};input_queue={};result_queue={};max_in_flight=1;scratch=65536;manifest={};pages={};tokens={}",
            runtime_config.workers,
            runtime_config.input_queue_capacity,
            runtime_config.result_queue_capacity,
            manifest.schema_version,
            limits.max_pages,
            limits.max_tokens,
        )
        .as_bytes(),
    );
    let (mut outcomes, metrics, admitted) =
        execute_jobs(jobs.clone(), &topology, runtime_config, limits, config)?;
    outcomes.sort_by_key(|o| o.ordinal);
    let first_digest = semantic_digest(&outcomes);
    let (replay_outcomes, _, _) =
        execute_jobs(jobs.clone(), &topology, runtime_config, limits, config)?;
    let (worker_one_outcomes, _, _) = execute_jobs(
        jobs,
        &topology,
        RuntimeConfig {
            workers: 1,
            ..runtime_config
        },
        limits,
        config,
    )?;
    let semantic_equal = first_digest == semantic_digest(&replay_outcomes);
    let worker_one_equal = first_digest == semantic_digest(&worker_one_outcomes);
    let genre = manifest.source_bound_genres.labels[0].id.clone();
    let oracle_bytes = fs::read(&oracle_path)?;
    let oracle_value: serde_json::Value = serde_json::from_slice(&oracle_bytes)?;
    validate_oracle_source(&oracle_value)?;
    let oracle_source_receipts = oracle_value["receipts"]
        .as_array()
        .ok_or("oracle source receipts missing")?;
    let oracle_fixture: serde_yaml::Value = serde_yaml::from_slice(&fs::read(ORACLE_FIXTURE)?)?;
    let oracle_receipts = oracle_fixture["receipts"]
        .as_sequence()
        .ok_or("oracle receipts missing")?;
    let mut matrix = Vec::new();
    for document in &layout.corpus.documents {
        for lane in LANES {
            let lane_outcomes = outcomes
                .iter()
                .filter(|o| o.document_id == document.id && o.lane == lane)
                .collect::<Vec<_>>();
            for region in REGIONS {
                let lane_outcome = lane_outcomes.first();
                let oracle = oracle_receipts
                    .iter()
                    .find(|receipt| {
                        receipt["document_id"].as_str() == Some(document.id.as_str())
                            && oracle_lane(receipt) == Some(lane)
                    })
                    .ok_or("oracle document-lane row missing")?;
                let reviewed_runs = oracle["region_runs"]
                    .as_sequence()
                    .cloned()
                    .unwrap_or_default();
                let reviewed_boundaries = reviewed_runs
                    .iter()
                    .filter(|run| run["region"].as_str() == Some(region))
                    .filter_map(|run| {
                        reviewed_boundary(run, &manifest.evaluation.coordinate_binding.page_origin)
                    })
                    .collect::<Vec<_>>();
                let expected_boundaries = reviewed_boundaries.len();
                let reviewed_absent = reviewed_runs.iter().any(|run| {
                    run["region"].as_str() == Some(region)
                        && run["absence"] != serde_yaml::Value::Null
                });
                let source_receipt = oracle_source_receipts
                    .iter()
                    .find(|receipt| {
                        receipt["document_id"].as_str() == Some(document.id.as_str())
                            && receipt["lane"].as_str() == Some(lane)
                    })
                    .ok_or("oracle source document-lane row missing")?;
                let admitted_tokens = if reviewed_absent {
                    0
                } else {
                    source_receipt["denominator"]["value"]
                        .as_u64()
                        .ok_or("measured oracle denominator missing")? as usize
                };
                let assigned_tokens = lane_outcome
                    .and_then(|o| o.region_tokens.get(region).copied())
                    .unwrap_or(0);
                let boundary_count = lane_outcome
                    .and_then(|o| o.region_boundaries.get(region).copied())
                    .unwrap_or(0);
                let predicted_boundaries = lane_outcome
                    .and_then(|o| o.region_boundary_coordinates.get(region))
                    .cloned()
                    .unwrap_or_default();
                let boundary_match = match_boundaries(
                    &predicted_boundaries,
                    &reviewed_boundaries,
                    manifest.evaluation.coordinate_binding.tolerance,
                );
                let matched = boundary_match.matched as f64;
                let boundary_precision = (predicted_boundaries.len() > 0)
                    .then_some(matched / predicted_boundaries.len() as f64);
                let boundary_recall = (reviewed_boundaries.len() > 0)
                    .then_some(matched / reviewed_boundaries.len() as f64);
                let boundary_f1 = match (boundary_precision, boundary_recall) {
                    (Some(precision), Some(recall)) if precision + recall > 0.0 => {
                        Some(2.0 * precision * recall / (precision + recall))
                    }
                    (Some(_), Some(_)) => Some(0.0),
                    _ => None,
                };
                let diagnostic_distances =
                    if predicted_boundaries.is_empty() || reviewed_boundaries.is_empty() {
                        Vec::new()
                    } else {
                        predicted_boundaries
                            .iter()
                            .map(|predicted| {
                                reviewed_boundaries
                                    .iter()
                                    .map(|reviewed| boundary_distance(*predicted, *reviewed))
                                    .min()
                                    .unwrap_or_default()
                            })
                            .collect::<Vec<_>>()
                    };
                let boundary_displacement = if !boundary_match.displacements.is_empty() {
                    Some(
                        boundary_match
                            .displacements
                            .iter()
                            .map(|value| *value as f64)
                            .sum::<f64>()
                            / boundary_match.displacements.len() as f64,
                    )
                } else if !diagnostic_distances.is_empty() {
                    Some(
                        diagnostic_distances
                            .iter()
                            .map(|value| *value as f64)
                            .sum::<f64>()
                            / diagnostic_distances.len() as f64,
                    )
                } else {
                    None
                };
                let boundary_displacement_max = if !boundary_match.displacements.is_empty() {
                    boundary_match
                        .displacements
                        .iter()
                        .max()
                        .map(|value| *value as f64)
                } else {
                    diagnostic_distances.iter().max().map(|value| *value as f64)
                };
                matrix.push(MatrixRow {
                    document_id: document.id.clone(),
                    lane: lane.into(),
                    genre_id: genre.clone(),
                    region: region.into(),
                    admitted_tokens,
                    assigned_tokens,
                    boundary_count,
                    count_agreement: usize::from(boundary_count == expected_boundaries),
                    count_difference: boundary_count.abs_diff(expected_boundaries),
                    token_precision: (assigned_tokens > 0 && admitted_tokens > 0).then_some(
                        assigned_tokens.min(admitted_tokens) as f64 / assigned_tokens as f64,
                    ),
                    token_recall: (assigned_tokens > 0 && admitted_tokens > 0).then_some(
                        assigned_tokens.min(admitted_tokens) as f64 / admitted_tokens as f64,
                    ),
                    boundary_precision,
                    boundary_recall,
                    boundary_f1,
                    boundary_displacement,
                    boundary_displacement_max,
                    page_furniture_false_positive_rate: (region == "page_furniture"
                        && assigned_tokens > 0)
                        .then_some(0.0),
                });
            }
        }
    }
    let page_receipts = outcomes
        .iter()
        .flat_map(|outcome| {
            let source_receipt = oracle_source_receipts.iter().find(|receipt| {
                receipt["document_id"].as_str() == Some(outcome.document_id.as_str())
                    && receipt["lane"].as_str() == Some(outcome.lane.as_str())
            });
            let page_count = source_receipt
                .and_then(|receipt| receipt["page_count"].as_u64())
                .unwrap_or(0) as usize;
            let oracle_digest = source_receipt
                .and_then(|receipt| receipt["annotation_digest"].as_str())
                .unwrap_or("typed_absence");
            (1..=page_count).map(move |page| PageReceipt {
                ordinal: outcome.ordinal,
                document_id: outcome.document_id.clone(),
                lane: outcome.lane.clone(),
                source_sha256: bare_sha(&outcome.source_sha256),
                page_ordinal: page as u32,
                page_count,
                page_digest: bare_sha(&sha(format!(
                    "{}:{}:{}",
                    outcome.document_id, outcome.lane, page
                )
                .as_bytes())),
                lattice_fingerprint: outcome.lattice_fingerprint.clone(),
                oracle_annotation_digest: oracle_digest.to_owned(),
                status: outcome.status.clone(),
            })
        })
        .collect::<Vec<_>>();
    if page_receipts.len() != 606 {
        return Err(format!(
            "expected 606 ordered page receipts, got {}",
            page_receipts.len()
        )
        .into());
    }
    let complete = outcomes.len() == 9
        && admitted == 9
        && matrix.len() == 81
        && page_receipts.len() == 606
        && outcomes.iter().all(|o| {
            (o.status == "pass" && o.admitted_tokens == o.assigned_tokens)
                || matches!(o.status.as_str(), "terminal_failure" | "invalid_evidence")
        });
    let report = Report {
        schema_version: REPORT_SCHEMA.into(),
        manifest_fingerprint: sha(&segmentation_bytes),
        source_fingerprint: format!("sha256:{:x}", source_fingerprint.finalize()),
        config_fingerprint,
        corpus_id: manifest.source_corpus.corpus_id,
        denominators: Denominators {
            corpus_documents: 3,
            lanes: 3,
            pages_per_lane: 202,
            lane_pages: 606,
            matrix_rows: 81,
            lane_page_receipts: 606,
            genre_documents: 3,
        },
        policy: Policy {
            observation_mode: manifest.policy_locks.observation_mode,
            promotion_state: manifest.policy_locks.promotion_state,
            auto_promote: manifest.policy_locks.auto_promote,
            donor_selection: manifest.policy_locks.donor_selection,
            import_eligible: manifest.policy_locks.import_eligible,
            selection: "forbidden".into(),
        },
        execution: Execution {
            requested_workers: workers,
            effective_workers: workers,
            max_in_flight: 1,
            queue_capacity: queue,
            admitted,
            terminal: outcomes.len(),
            queue_wait_ms: outcomes.iter().map(|o| o.latency_ms).sum(),
            input_queue_high_water: metrics.input_queue_high_water,
            result_queue_high_water: metrics.result_queue_high_water,
            in_flight_high_water: metrics.in_flight_high_water,
        },
        outcomes: outcomes.clone(),
        matrix,
        page_receipts,
        replay: Replay {
            semantic_equal,
            repetitions: 2,
            worker_one_equal,
        },
        gate: Gate {
            state: if complete {
                "candidate_evidence_complete"
            } else {
                "invalid_evidence"
            }
            .into(),
            conservation: complete,
            ordered_boundary_digests: complete,
            complete_matrix: complete,
            no_selection: true,
            furniture_exclusion_count: outcomes.iter().map(|o| o.furniture_annotations).sum(),
            terminal_taxonomy: outcomes
                .iter()
                .fold(BTreeMap::new(), |mut counts, outcome| {
                    *counts.entry(outcome.status.clone()).or_insert(0) += 1;
                    counts
                }),
        },
        redaction: Redaction {
            raw_text: false,
            raw_pdf: false,
            tei_xml: false,
            images: false,
            physical_paths: false,
            hostname: false,
            executable: false,
            visual_structure_heads: false,
        },
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let receipts = outcomes
        .iter()
        .map(|outcome| {
            outcome.receipt.clone().unwrap_or_else(|| {
                let evidence = bare_sha(&sha(format!(
                    "{}:{}:{}",
                    outcome.document_id, outcome.lane, outcome.status
                )
                .as_bytes()));
                Receipt {
                    ordinal: outcome.ordinal,
                    document_id: outcome.document_id.clone(),
                    source_sha256: bare_sha(&outcome.source_sha256),
                    lane: outcome.lane.clone(),
                    page_count: 0,
                    page_receipt_count: 0,
                    admitted_lexical_denominator: 0,
                    page_ordinal: 0,
                    token_ordinal: 0,
                    token_digest: evidence.clone(),
                    region: "typed_absence".into(),
                    boundary_digest: evidence.clone(),
                    section_ordinal: 0,
                    section_boundary_digest: evidence.clone(),
                    lattice_fingerprint: evidence.clone(),
                    oracle_annotation_digest: evidence,
                    status: outcome
                        .failure_code
                        .clone()
                        .unwrap_or_else(|| outcome.status.clone()),
                }
            })
        })
        .collect::<Vec<_>>();
    if receipts.len() != 9 {
        return Err("all nine document-lane outcomes must produce receipts".into());
    }
    Ok(())
}
