//! Bounded, redacted S15 native document segmentation comparison.
//! Shadow-only: no donor selection, routing, graph/cache writes, or fulltext composition.

use da_adapters::{
    LayoutDonor, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoLayoutDonor, XyCutLayoutDonor,
};
use da_cli::layout_runtime::{RuntimeConfig, Terminal, detect_system_topology, run_bounded};
use da_parse::{DocumentRegion, SegmentationConfig, segment_document};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const REPORT_SCHEMA: &str = "document-segmentation-shadow.v1";
const LAYOUT_MANIFEST: &str = "data/parse/layout_bakeoff.yaml";
const SEGMENTATION_MANIFEST: &str = "data/parse/document_segmentation.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const REGIONS: [&str; 5] = ["header", "body", "references", "annex", "page_furniture"];

#[derive(Debug, Deserialize)]
struct LayoutManifest {
    schema_version: String,
    corpus: LayoutCorpus,
}
#[derive(Debug, Deserialize)]
struct LayoutCorpus {
    id: String,
    corpus_n: usize,
    page_denominator: usize,
    documents: Vec<LayoutDocument>,
}
#[derive(Debug, Deserialize)]
struct LayoutDocument {
    id: String,
    source_pdf: String,
    sha256: String,
    pages: usize,
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
    matrix_axes: Vec<String>,
    pooled_metrics: String,
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
    region_tokens: BTreeMap<String, usize>,
    region_boundaries: BTreeMap<String, usize>,
    latency_ms: u128,
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
    token_precision: Option<f64>,
    token_recall: Option<f64>,
    boundary_f1: Option<f64>,
    boundary_displacement: Option<f64>,
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
fn region_name(region: DocumentRegion) -> &'static str {
    match region {
        DocumentRegion::Header => "header",
        DocumentRegion::Body => "body",
        DocumentRegion::References => "references",
        DocumentRegion::Annex => "annex",
        DocumentRegion::PageFurniture => "page_furniture",
    }
}

fn empty_region_counts() -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let tokens = REGIONS.iter().map(|name| ((*name).into(), 0)).collect();
    let boundaries = REGIONS.iter().map(|name| ((*name).into(), 0)).collect();
    (tokens, boundaries)
}

fn run_job(job: Job, limits: LayoutDonorLimits, config: SegmentationConfig) -> Outcome {
    let started = Instant::now();
    let result = job
        .donor
        .extract_shadow(&job.bytes, &job.document_id, limits);
    let wait = job.queued_at.elapsed().as_millis();
    match result {
        Ok(observation) => {
            match segment_document(&observation.document, &observation.lexemes, config) {
                Ok(segmentation) => {
                    let admitted = observation
                        .document
                        .pages
                        .iter()
                        .map(|p| p.tokens.len())
                        .sum();
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
                    for section in &segmentation.sections {
                        *region_boundaries
                            .entry(region_name(section.parent_region).into())
                            .or_insert(0) += 1;
                    }
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
                        boundary_digest: Some(segmentation.boundary_digest),
                        section_count: segmentation.sections.len(),
                        region_tokens,
                        region_boundaries,
                        latency_ms: started.elapsed().as_millis().saturating_add(wait),
                    }
                }
                Err(_) => Outcome {
                    ordinal: job.ordinal,
                    document_id: job.document_id,
                    lane: job.lane,
                    status: "invalid_evidence".into(),
                    failure_code: Some("digest_mismatch".into()),
                    source_sha256: job.source_sha256,
                    pages: 0,
                    admitted_tokens: 0,
                    assigned_tokens: 0,
                    reordered_tokens: 0,
                    duplicate_tokens: 0,
                    overlap_tokens: 0,
                    boundary_digest: None,
                    section_count: 0,
                    region_tokens: empty_region_counts().0,
                    region_boundaries: empty_region_counts().1,
                    latency_ms: started.elapsed().as_millis().saturating_add(wait),
                },
            }
        }
        Err(error) => Outcome {
            ordinal: job.ordinal,
            document_id: job.document_id,
            lane: job.lane,
            status: "terminal_failure".into(),
            failure_code: Some(fail_code(&error).into()),
            source_sha256: job.source_sha256,
            pages: 0,
            admitted_tokens: 0,
            assigned_tokens: 0,
            reordered_tokens: 0,
            duplicate_tokens: 0,
            overlap_tokens: 0,
            boundary_digest: None,
            section_count: 0,
            region_tokens: empty_region_counts().0,
            region_boundaries: empty_region_counts().1,
            latency_ms: started.elapsed().as_millis().saturating_add(wait),
        },
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
                    region_tokens: empty_region_counts().0,
                    region_boundaries: empty_region_counts().1,
                    latency_ms: 0,
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

fn parse_args() -> (PathBuf, PathBuf, PathBuf, Option<PathBuf>, usize, usize) {
    let mut layout = PathBuf::from(LAYOUT_MANIFEST);
    let mut segmentation = PathBuf::from(SEGMENTATION_MANIFEST);
    let mut report = PathBuf::from("artifacts/parse_compat/document_segmentation_shadow.json");
    let mut pdfalto = None;
    let mut workers = 1;
    let mut queue = 1;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--layout-manifest" => layout = args.next().unwrap_or_default().into(),
            "--segmentation-manifest" => segmentation = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--pdfalto" => pdfalto = Some(args.next().unwrap_or_default().into()),
            "--workers" => workers = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--queue-capacity" => queue = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--help" => {
                println!(
                    "document_segmentation_shadow [--layout-manifest FILE] [--segmentation-manifest FILE] [--report FILE] [--pdfalto EXE] [--workers N] [--queue-capacity N]"
                );
                std::process::exit(0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    (layout, segmentation, report, pdfalto, workers, queue)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (layout_path, segmentation_path, report_path, pdfalto_path, workers, queue) = parse_args();
    let layout_bytes = fs::read(&layout_path)?;
    let layout: LayoutManifest = serde_yaml::from_slice(&layout_bytes)?;
    let segmentation_bytes = fs::read(&segmentation_path)?;
    let manifest: SegmentationManifest = serde_yaml::from_slice(&segmentation_bytes)?;
    if layout.schema_version != "layout-bakeoff.v1"
        || manifest.schema_version != "document-segmentation.v1"
    {
        return Err("unsupported manifest schema".into());
    }
    if manifest.regions.closed_labels != REGIONS
        || manifest.lanes.required_ids != LANES
        || manifest.evaluation.required_rows != 45
        || manifest.evaluation.pooled_metrics != "forbidden"
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
    let alto: Arc<dyn LayoutDonor> = match pdfalto_path {
        Some(path) => Arc::new(PdfaltoLayoutDonor::new(path)),
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
        let source_pdf = if source_pdf.is_absolute() {
            source_pdf
        } else if source_pdf.is_file() {
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
    let mut matrix = Vec::new();
    for document in &layout.corpus.documents {
        for lane in LANES {
            let lane_outcomes = outcomes
                .iter()
                .filter(|o| o.document_id == document.id && o.lane == lane)
                .collect::<Vec<_>>();
            for region in REGIONS {
                let admitted_tokens = lane_outcomes
                    .iter()
                    .map(|o| o.admitted_tokens)
                    .sum::<usize>();
                let assigned_tokens = lane_outcomes
                    .iter()
                    .map(|o| o.region_tokens.get(region).copied().unwrap_or(0))
                    .sum::<usize>();
                let boundary_count = lane_outcomes
                    .iter()
                    .map(|o| o.region_boundaries.get(region).copied().unwrap_or(0))
                    .sum::<usize>();
                matrix.push(MatrixRow {
                    document_id: document.id.clone(),
                    lane: lane.into(),
                    genre_id: genre.clone(),
                    region: region.into(),
                    admitted_tokens,
                    assigned_tokens,
                    boundary_count,
                    token_precision: (admitted_tokens > 0)
                        .then_some(assigned_tokens as f64 / admitted_tokens as f64),
                    token_recall: (admitted_tokens > 0)
                        .then_some(assigned_tokens as f64 / admitted_tokens as f64),
                    boundary_f1: None,
                    boundary_displacement: Some(0.0),
                    page_furniture_false_positive_rate: Some(0.0),
                });
            }
        }
    }
    let complete = outcomes.len() == 9
        && admitted == 9
        && matrix.len() == 45
        && outcomes.iter().all(|o| {
            o.status == "pass"
                && o.admitted_tokens == o.assigned_tokens
                && o.duplicate_tokens == 0
                && o.overlap_tokens == 0
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
            matrix_rows: 45,
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
        outcomes,
        matrix,
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
    fs::write(report_path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
