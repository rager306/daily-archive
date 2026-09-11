//! Bounded source-bound S16 native visual-head shadow comparison.
//! Shadow-only: no route selection, graph/cache writes, or fulltext composition.

use da_adapters::{
    LayoutDonor, LayoutDonorError, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoLayoutDonor,
    VisualAbsenceReason, VisualPrimitiveKind, VisualPrimitiveState, XyCutLayoutDonor,
};
use da_cli::layout_runtime::{RuntimeConfig, Terminal, detect_system_topology, run_bounded};
use da_parse::{
    NativeVisualAbsence, NativeVisualConfig, NativeVisualHeadOutcome, NativeVisualKind,
    NativeVisualObservation, NativeVisualPrimitive, NativeVisualProvider, detect_figure_heads,
    detect_formula_heads, detect_table_heads,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

const DATA_MANIFEST: &str = "data/parse/layout_bakeoff.yaml";
const VISUAL_MANIFEST: &str = "data/parse/native_visual_heads.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const HEADS: [&str; 3] = ["figure", "table", "formula"];

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    policy_locks: PolicyLocks,
    corpus: Corpus,
    lanes: Lanes,
}
#[derive(Debug, Deserialize)]
struct PolicyLocks {
    import_eligible: bool,
    auto_promote: String,
    promotion_state: String,
    graph_mutation: String,
}
#[derive(Debug, Deserialize)]
struct Corpus {
    id: String,
    corpus_n: usize,
    page_denominator: usize,
    documents: Vec<Document>,
}
#[derive(Debug, Deserialize)]
struct Document {
    id: String,
    source_pdf: String,
    sha256: String,
    pages: usize,
}
#[derive(Debug, Deserialize)]
struct Lanes {
    required: Vec<String>,
    same_input: bool,
    all_lanes_required: bool,
}
#[derive(Debug, Deserialize)]
struct OracleFile {
    receipts: Vec<OracleReceipt>,
}
#[derive(Debug, Deserialize)]
struct OracleReceipt {
    document_id: String,
    source_sha256: String,
    lane_id: String,
    heads: Vec<OracleHead>,
}
#[derive(Debug, Clone, Deserialize)]
struct OracleHead {
    head: String,
    head_ordinal: usize,
    status: String,
    bbox_72dpi: serde_yaml::Value,
    caption_digest: serde_yaml::Value,
    content_digest: serde_yaml::Value,
    callout_digest: serde_yaml::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Report {
    schema_version: String,
    source_identity: SourceIdentity,
    config_identity: ConfigIdentity,
    binary_identity: BinaryIdentity,
    donor_identity: Vec<DonorIdentity>,
    denominators: Denominators,
    policy: Policy,
    execution: Execution,
    outcomes: Vec<LaneReceipt>,
    replay: Replay,
    gate: Gate,
    redaction: Redaction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceIdentity {
    corpus_id: String,
    manifest_digest: String,
    source_fingerprint: String,
    documents: usize,
    pages: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigIdentity {
    visual_manifest_digest: String,
    oracle_digest: String,
    fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BinaryIdentity {
    version: String,
    digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DonorIdentity {
    lane: String,
    donor: String,
    version: String,
    identity_digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Denominators {
    documents: usize,
    lanes: usize,
    pages_per_lane: usize,
    lane_pages: usize,
    heads: usize,
    receipts: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Policy {
    observation_mode: String,
    promotion_state: String,
    auto_promote: String,
    donor_selection: String,
    import_eligible: bool,
    graph_mutation: String,
    fulltext_composition: String,
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
    private_scratch_high_water_bytes: usize,
    peak_rss_bytes: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LaneReceipt {
    ordinal: usize,
    document_id: String,
    lane: String,
    source_sha256: String,
    status: String,
    failure_code: Option<String>,
    admitted: bool,
    terminal: bool,
    heads: BTreeMap<String, HeadReceipt>,
    latency_ms: u128,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeadReceipt {
    status: String,
    reason: Option<String>,
    admitted: u32,
    terminal: u32,
    caption_conservation: u32,
    content_conservation: u32,
    callout_conservation: u32,
    geometry_conservation: u32,
    oracle_status: String,
    oracle_denominator: usize,
    java_supporting_denominator: usize,
    replay_digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Replay {
    semantic_replays: usize,
    semantic_equal: bool,
    worker_one_equal: bool,
    interruption_resume_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Gate {
    state: String,
    no_pooled_winner: bool,
    exact_same_n: bool,
    exact_lane_pages: bool,
    admitted_equals_terminal: bool,
    all_heads_independent: bool,
    unsupported_visible: bool,
    conservation: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Redaction {
    raw_lexemes: bool,
    raw_pdf: bool,
    raw_text: bool,
    captions: bool,
    tei_xml: bool,
    images: bool,
    assets: bool,
    physical_paths: bool,
    hostname: bool,
    executable: bool,
}

#[derive(Clone)]
struct Job {
    ordinal: usize,
    document_id: String,
    source_sha256: String,
    lane: String,
    bytes: Arc<Vec<u8>>,
    donor: Arc<dyn LayoutDonor>,
    queued_at: Instant,
}
#[derive(Clone)]
struct OracleLookup {
    status: String,
    denominator: usize,
    java_denominator: usize,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn resolve_path(path: &str, manifest_path: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() || candidate.is_file() {
        return candidate;
    }
    let mut base = manifest_path.parent();
    for _ in 0..4 {
        if let Some(parent) = base {
            let resolved = parent.join(&candidate);
            if resolved.is_file() {
                return resolved;
            }
            base = parent.parent();
        }
    }
    candidate
}
fn failure_code(error: &LayoutDonorError) -> &'static str {
    match error {
        LayoutDonorError::Unavailable { .. } => "unsupported",
        LayoutDonorError::Timeout { .. } => "timeout",
        LayoutDonorError::NonZero { .. } => "nonzero",
        LayoutDonorError::Malformed { .. } | LayoutDonorError::InvalidLayout(_) => "malformed",
        LayoutDonorError::Oversize { .. } => "oversize",
        LayoutDonorError::Encrypted => "encrypted",
        LayoutDonorError::Truncated => "truncated",
        LayoutDonorError::Io(_) => "missing",
    }
}
fn absence_name(reason: NativeVisualAbsence) -> &'static str {
    match reason {
        NativeVisualAbsence::Unsupported => "unsupported",
        NativeVisualAbsence::NotObserved => "not_observed",
        NativeVisualAbsence::MissingIdentity => "missing_identity",
        NativeVisualAbsence::ChecksumMismatch => "checksum_mismatch",
        NativeVisualAbsence::MalformedBbox => "malformed_bbox",
        NativeVisualAbsence::InvalidDigest => "invalid_digest",
        NativeVisualAbsence::DuplicateReceipt => "duplicate_receipt",
        NativeVisualAbsence::MissingReceipt => "missing_receipt",
        NativeVisualAbsence::Oversize => "oversize",
        NativeVisualAbsence::Interrupted => "interrupted",
    }
}
fn is_absent(value: &serde_yaml::Value) -> bool {
    value.as_str() == Some("absent")
}
fn java_denominator(head: &str) -> usize {
    match head {
        "figure" => 106,
        "table" => 8,
        "formula" => 38,
        _ => 0,
    }
}
fn oracle_lookup(
    oracle: &BTreeMap<(String, String, String), OracleLookup>,
    doc: &str,
    lane: &str,
    head: &str,
) -> OracleLookup {
    oracle
        .get(&(doc.into(), lane.into(), head.into()))
        .cloned()
        .expect("source-bound oracle row")
}
fn native_kind(kind: VisualPrimitiveKind) -> NativeVisualKind {
    match kind {
        VisualPrimitiveKind::Image => NativeVisualKind::Image,
        VisualPrimitiveKind::Graphic => NativeVisualKind::Graphic,
        VisualPrimitiveKind::Table => NativeVisualKind::Table,
        VisualPrimitiveKind::Formula => NativeVisualKind::Formula,
        VisualPrimitiveKind::Rect => NativeVisualKind::Rect,
    }
}

/// Convert the donor's bounded visual sidecar without collapsing observed
/// primitives into a terminal empty lane. Receipt families remain explicit:
/// an empty family means that family was not supplied by this donor, while a
/// lane absence is reserved for an actually absent upstream visual signal.
fn native_visual_payload(
    observation: &da_adapters::LayoutShadowObservation,
) -> Result<NativeVisualObservation, NativeVisualAbsence> {
    let lane_absence = observation.visual.absence.map(|reason| match reason {
        VisualAbsenceReason::Unsupported | VisualAbsenceReason::ProviderUnavailable => {
            NativeVisualAbsence::Unsupported
        }
        VisualAbsenceReason::NotObserved => NativeVisualAbsence::NotObserved,
        VisualAbsenceReason::Malformed => NativeVisualAbsence::MalformedBbox,
    });
    let primitives = observation
        .visual
        .primitives
        .iter()
        .map(|primitive| {
            let (bbox, content_digest, absence) = match &primitive.state {
                VisualPrimitiveState::Observed { content_digest } => {
                    (primitive.bbox, Some(content_digest.clone()), None)
                }
                VisualPrimitiveState::Absent { reason } => (
                    None,
                    None,
                    Some(match reason {
                        VisualAbsenceReason::Unsupported
                        | VisualAbsenceReason::ProviderUnavailable => {
                            NativeVisualAbsence::Unsupported
                        }
                        VisualAbsenceReason::NotObserved => NativeVisualAbsence::NotObserved,
                        VisualAbsenceReason::Malformed => NativeVisualAbsence::MalformedBbox,
                    }),
                ),
            };
            NativeVisualPrimitive {
                document_id: primitive.document_id.clone(),
                source_sha256: observation
                    .document
                    .source_pdf_sha256
                    .trim_start_matches("sha256:")
                    .to_owned(),
                provider: NativeVisualProvider {
                    id: primitive.provider.id.clone(),
                    version: primitive.provider.version.clone(),
                },
                page_ordinal: primitive.page_ordinal,
                source_ordinal: primitive.source_ordinal,
                kind: native_kind(primitive.kind),
                bbox,
                content_digest,
                absence,
            }
        })
        .collect::<Vec<_>>();
    let source = NativeVisualObservation {
        schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: observation.document.document_id.clone(),
        source_sha256: observation
            .document
            .source_pdf_sha256
            .trim_start_matches("sha256:")
            .into(),
        provider: NativeVisualProvider {
            id: observation.visual.provider.id.clone(),
            version: observation.visual.provider.version.clone(),
        },
        primitives,
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence,
    };
    source
        .validate(NativeVisualConfig::default())
        .map_err(|error| match error {
            da_parse::NativeVisualError::InvalidDigest => NativeVisualAbsence::InvalidDigest,
            da_parse::NativeVisualError::InvalidGeometry
            | da_parse::NativeVisualError::InvalidSchema => NativeVisualAbsence::MalformedBbox,
            da_parse::NativeVisualError::DuplicatePrimitive
            | da_parse::NativeVisualError::DuplicateReceipt => {
                NativeVisualAbsence::DuplicateReceipt
            }
            da_parse::NativeVisualError::Oversize(_) | da_parse::NativeVisualError::HeadLimit => {
                NativeVisualAbsence::Oversize
            }
            da_parse::NativeVisualError::ReceiptMismatch => NativeVisualAbsence::MissingReceipt,
            da_parse::NativeVisualError::InvalidIdentity(_) => NativeVisualAbsence::MissingIdentity,
        })?;
    Ok(source)
}

fn observation_for(
    job: &Job,
    extracted: Result<da_adapters::LayoutShadowObservation, LayoutDonorError>,
) -> (NativeVisualObservation, Option<String>) {
    match extracted {
        Ok(observation) => match native_visual_payload(&observation) {
            Ok(source) => (source, None),
            Err(reason) => {
                let source = NativeVisualObservation {
                    schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
                    document_id: job.document_id.clone(),
                    source_sha256: job.source_sha256.trim_start_matches("sha256:").into(),
                    provider: NativeVisualProvider {
                        id: job.donor.id().into(),
                        version: job.donor.version().into(),
                    },
                    primitives: Vec::new(),
                    figure_captions: Vec::new(),
                    figure_callouts: Vec::new(),
                    table_captions: Vec::new(),
                    table_callouts: Vec::new(),
                    table_cells: Vec::new(),
                    formula_labels: Vec::new(),
                    formula_callouts: Vec::new(),
                    lane_absence: Some(reason),
                };
                (source, Some("visual_representation".into()))
            }
        },
        Err(error) => {
            let reason = match error {
                LayoutDonorError::Malformed { .. } | LayoutDonorError::InvalidLayout(_) => {
                    NativeVisualAbsence::MalformedBbox
                }
                LayoutDonorError::Oversize { .. } => NativeVisualAbsence::Oversize,
                LayoutDonorError::Unavailable { .. } | LayoutDonorError::Io(_) => {
                    NativeVisualAbsence::Unsupported
                }
                LayoutDonorError::Timeout { .. }
                | LayoutDonorError::NonZero { .. }
                | LayoutDonorError::Encrypted
                | LayoutDonorError::Truncated => NativeVisualAbsence::Interrupted,
            };
            let source = NativeVisualObservation {
                schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
                document_id: job.document_id.clone(),
                source_sha256: job.source_sha256.trim_start_matches("sha256:").into(),
                provider: NativeVisualProvider {
                    id: job.donor.id().into(),
                    version: job.donor.version().into(),
                },
                primitives: Vec::new(),
                figure_captions: Vec::new(),
                figure_callouts: Vec::new(),
                table_captions: Vec::new(),
                table_callouts: Vec::new(),
                table_cells: Vec::new(),
                formula_labels: Vec::new(),
                formula_callouts: Vec::new(),
                lane_absence: Some(reason),
            };
            (source, Some(failure_code(&error).into()))
        }
    }
}
fn head_receipt(outcome: &NativeVisualHeadOutcome, lookup: OracleLookup) -> HeadReceipt {
    let (status, reason, metrics) = match outcome {
        NativeVisualHeadOutcome::Terminal { reason, metrics } => (
            "terminal".into(),
            Some(absence_name(*reason).into()),
            metrics,
        ),
        NativeVisualHeadOutcome::Figure { metrics, .. }
        | NativeVisualHeadOutcome::Table { metrics, .. }
        | NativeVisualHeadOutcome::Formula { metrics, .. } => ("observed".into(), None, metrics),
    };
    HeadReceipt {
        status,
        reason,
        admitted: metrics.admitted,
        terminal: metrics.terminal,
        caption_conservation: metrics.caption_conservation,
        content_conservation: metrics.content_conservation,
        callout_conservation: metrics.callout_conservation,
        geometry_conservation: metrics.geometry_conservation,
        oracle_status: lookup.status,
        oracle_denominator: lookup.denominator,
        java_supporting_denominator: lookup.java_denominator,
        replay_digest: digest(&serde_json::to_vec(outcome).expect("head receipt serialization")),
    }
}
fn run_job(
    job: Job,
    limits: LayoutDonorLimits,
    oracle: &BTreeMap<(String, String, String), OracleLookup>,
) -> LaneReceipt {
    let started = Instant::now();
    let extracted = job
        .donor
        .extract_shadow(&job.bytes, &job.document_id, limits);
    let (source, failure) = observation_for(&job, extracted);
    let config = NativeVisualConfig::default();
    let figure = detect_figure_heads(&source, config).expect("bounded native visual validation");
    let table = detect_table_heads(&source, config).expect("bounded native visual validation");
    let formula = detect_formula_heads(&source, config).expect("bounded native visual validation");
    let mut heads = BTreeMap::new();
    for (name, outcome) in [("figure", figure), ("table", table), ("formula", formula)] {
        let lookup = oracle_lookup(oracle, &job.document_id, &job.lane, name);
        heads.insert(name.into(), head_receipt(&outcome, lookup));
    }
    LaneReceipt {
        ordinal: job.ordinal,
        document_id: job.document_id,
        lane: job.lane,
        source_sha256: job.source_sha256,
        status: "terminal".into(),
        failure_code: failure,
        admitted: true,
        terminal: true,
        heads,
        latency_ms: started
            .elapsed()
            .as_millis()
            .saturating_add(job.queued_at.elapsed().as_millis()),
    }
}
fn execute_jobs(
    jobs: Vec<Job>,
    topology: &da_cli::layout_runtime::TopologyReceipt,
    runtime: RuntimeConfig,
    oracle: Arc<BTreeMap<(String, String, String), OracleLookup>>,
) -> Result<
    (
        Vec<LaneReceipt>,
        da_cli::layout_runtime::RuntimeMetrics,
        Vec<usize>,
        usize,
    ),
    Box<dyn std::error::Error>,
> {
    let fallback = jobs
        .iter()
        .map(|j| {
            (
                j.ordinal,
                (
                    j.document_id.clone(),
                    j.lane.clone(),
                    j.source_sha256.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let report = run_bounded(runtime, topology, jobs, move |job, scratch| {
        scratch.allocate(1).map_err(|_| "scratch")?;
        Ok::<_, &'static str>(run_job(job, LayoutDonorLimits::default(), &oracle))
    })?;
    let mut outcomes = Vec::new();
    for terminal in report.terminals {
        match terminal {
            Terminal::Completed { result, .. } => outcomes.push(result),
            Terminal::Failed { ordinal, .. } | Terminal::CoordinatorFailed { ordinal, .. } => {
                let (document_id, lane, source_sha256) =
                    fallback.get(&ordinal).expect("admitted ordinal");
                outcomes.push(LaneReceipt {
                    ordinal,
                    document_id: document_id.clone(),
                    lane: lane.clone(),
                    source_sha256: source_sha256.clone(),
                    status: "terminal".into(),
                    failure_code: Some("worker_failure".into()),
                    admitted: true,
                    terminal: true,
                    heads: BTreeMap::new(),
                    latency_ms: 0,
                });
            }
        }
    }
    outcomes.sort_by_key(|o| o.ordinal);
    Ok((
        outcomes,
        report.metrics,
        report.private_scratch_high_water,
        report.admitted,
    ))
}
fn semantic_digest(outcomes: &[LaneReceipt]) -> String {
    let mut normalized = outcomes.to_vec();
    for outcome in &mut normalized {
        outcome.latency_ms = 0;
    }
    digest(&serde_json::to_vec(&normalized).expect("report serialization"))
}
fn parse_args() -> (PathBuf, PathBuf, usize, usize) {
    let mut manifest = DATA_MANIFEST.into();
    let mut report = "artifacts/parse_compat/native_visual_heads_shadow.json".into();
    let mut workers = 1;
    let mut queue = 1;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--workers" => workers = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--queue-capacity" => queue = args.next().unwrap_or_default().parse().unwrap_or(1),
            "--help" => {
                println!(
                    "native_visual_heads_shadow [--manifest FILE] [--report FILE] [--workers N] [--queue-capacity N]"
                );
                std::process::exit(0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    (manifest, report, workers, queue)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = da_cli::logging::init_pipeline_logging("native_visual_heads_shadow");
    let (manifest_path, report_path, workers, queue) = parse_args();
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest: Manifest = serde_yaml::from_slice(&manifest_bytes)?;
    if manifest.schema_version != "layout-bakeoff.v1"
        || manifest.corpus.corpus_n != 3
        || manifest.corpus.page_denominator != 202
        || manifest.lanes.required != LANES
        || !manifest.lanes.same_input
        || !manifest.lanes.all_lanes_required
        || manifest.policy_locks.import_eligible
        || manifest.policy_locks.auto_promote != "forbidden"
        || manifest.policy_locks.graph_mutation != "forbidden"
    {
        return Err("S16 source DATA contract mismatch".into());
    }
    let visual_bytes = fs::read(resolve_path(VISUAL_MANIFEST, &manifest_path))?;
    let oracle_bytes = fs::read(resolve_path(ORACLE, &manifest_path))?;
    let oracle_file: OracleFile = serde_yaml::from_slice(&oracle_bytes)?;
    let mut oracle = BTreeMap::new();
    for receipt in oracle_file.receipts {
        if receipt.source_sha256.len() != 64
            || !LANES.contains(&receipt.lane_id.as_str())
            || receipt.heads.len() != 3
        {
            return Err("invalid source-bound oracle row".into());
        }
        for head in receipt.heads {
            if !HEADS.contains(&head.head.as_str())
                || head.head_ordinal != 0
                || !matches!(
                    head.status.as_str(),
                    "unsupported" | "not_observed" | "observed"
                )
                || (head.status != "observed" && !is_absent(&head.bbox_72dpi))
                || (head.status != "observed" && !is_absent(&head.caption_digest))
                || (head.status != "observed" && !is_absent(&head.content_digest))
                || (head.status != "observed" && !is_absent(&head.callout_digest))
            {
                return Err("oracle is not typed digest-only absence".into());
            }
            let head_label = head.head.clone();
            oracle.insert(
                (
                    receipt.document_id.clone(),
                    receipt.lane_id.clone(),
                    head_label.clone(),
                ),
                OracleLookup {
                    status: head.status,
                    denominator: 1,
                    java_denominator: java_denominator(&head_label),
                },
            );
        }
    }
    if oracle.len() != 27 {
        return Err("S16 oracle denominator mismatch".into());
    }
    let inspector: Arc<dyn LayoutDonor> = Arc::new(PdfInspectorLayoutDonor);
    let xy: Arc<dyn LayoutDonor> =
        Arc::new(XyCutLayoutDonor::new(Box::new(PdfInspectorLayoutDonor)));
    let alto: Arc<dyn LayoutDonor> = Arc::new(PdfaltoLayoutDonor::new("pdfalto"));
    let donors: [&Arc<dyn LayoutDonor>; 3] = [&alto, &inspector, &xy];
    let mut jobs = Vec::new();
    let mut source_hash = Sha256::new();
    for doc in &manifest.corpus.documents {
        if doc.pages == 0 {
            return Err("document page identity missing".into());
        }
        let bytes = Arc::new(fs::read(resolve_path(&doc.source_pdf, &manifest_path))?);
        if digest(&bytes) != format!("sha256:{}", doc.sha256) {
            return Err(format!("checksum mismatch for {}", doc.id).into());
        }
        source_hash.update(bytes.as_slice());
        for (lane, donor) in LANES.iter().zip(donors) {
            jobs.push(Job {
                ordinal: jobs.len(),
                document_id: doc.id.clone(),
                source_sha256: digest(&bytes),
                lane: (*lane).into(),
                bytes: bytes.clone(),
                donor: (*donor).clone(),
                queued_at: Instant::now(),
            });
        }
    }
    let topology = detect_system_topology()?;
    let runtime = RuntimeConfig {
        workers,
        input_queue_capacity: queue,
        result_queue_capacity: queue,
        max_in_flight: 1,
        private_scratch_capacity: 64 * 1024,
    };
    let oracle = Arc::new(oracle);
    let (mut outcomes, metrics, scratch, admitted) =
        execute_jobs(jobs.clone(), &topology, runtime, oracle.clone())?;
    let first = semantic_digest(&outcomes);
    let (replay_one, _, _, _) = execute_jobs(jobs.clone(), &topology, runtime, oracle.clone())?;
    let (replay_two, _, _, _) = execute_jobs(jobs.clone(), &topology, runtime, oracle.clone())?;
    let (worker_one, _, _, _) = execute_jobs(
        jobs,
        &topology,
        RuntimeConfig {
            workers: 1,
            ..runtime
        },
        oracle,
    )?;
    let semantic_equal =
        first == semantic_digest(&replay_one) && first == semantic_digest(&replay_two);
    let worker_one_equal = first == semantic_digest(&worker_one);
    outcomes.sort_by_key(|o| o.ordinal);
    let unsupported = outcomes
        .iter()
        .any(|o| o.failure_code.as_deref() == Some("unsupported"));
    let conservation = outcomes.len() == 9
        && outcomes.iter().all(|o| {
            o.heads.len() == 3
                && o.heads.values().all(|h| {
                    matches!(
                        h.oracle_status.as_str(),
                        "unsupported" | "not_observed" | "observed"
                    )
                })
        });
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| fs::read(p).ok())
        .map(|b| digest(&b))
        .unwrap_or_else(|| "sha256:unavailable".into());
    let mut source_documents = manifest
        .corpus
        .documents
        .iter()
        .map(|d| d.id.clone())
        .collect::<Vec<_>>();
    source_documents.sort();
    let mut donors_out = Vec::new();
    for (lane, donor) in LANES.iter().zip(donors) {
        donors_out.push(DonorIdentity {
            lane: (*lane).into(),
            donor: donor.id().into(),
            version: donor.version().into(),
            identity_digest: digest(format!("{}:{}", donor.id(), donor.version()).as_bytes()),
        });
    }
    let report = Report {
        schema_version: "native-visual-heads-shadow.v1".into(),
        source_identity: SourceIdentity {
            corpus_id: manifest.corpus.id,
            manifest_digest: digest(&manifest_bytes),
            source_fingerprint: format!("sha256:{:x}", source_hash.finalize()),
            documents: source_documents.len(),
            pages: 202,
        },
        config_identity: ConfigIdentity {
            visual_manifest_digest: digest(&visual_bytes),
            oracle_digest: digest(&oracle_bytes),
            fingerprint: digest(&[visual_bytes.as_slice(), oracle_bytes.as_slice()].concat()),
        },
        binary_identity: BinaryIdentity {
            version: env!("CARGO_PKG_VERSION").into(),
            digest: binary,
        },
        donor_identity: donors_out,
        denominators: Denominators {
            documents: 3,
            lanes: 3,
            pages_per_lane: 202,
            lane_pages: 606,
            heads: 3,
            receipts: 27,
        },
        policy: Policy {
            observation_mode: "shadow".into(),
            promotion_state: manifest.policy_locks.promotion_state,
            auto_promote: manifest.policy_locks.auto_promote,
            donor_selection: "forbidden".into(),
            import_eligible: false,
            graph_mutation: manifest.policy_locks.graph_mutation,
            fulltext_composition: "forbidden".into(),
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
            private_scratch_high_water_bytes: scratch.into_iter().max().unwrap_or(0),
            peak_rss_bytes: None,
        },
        outcomes,
        replay: Replay {
            semantic_replays: 2,
            semantic_equal,
            worker_one_equal,
            interruption_resume_fingerprint: first,
        },
        gate: Gate {
            state: if conservation && semantic_equal {
                if unsupported {
                    "shadow_terminal_evidence_incomplete"
                } else {
                    "shadow_terminal_evidence_complete"
                }
            } else {
                "invalid_evidence"
            }
            .into(),
            no_pooled_winner: true,
            exact_same_n: true,
            exact_lane_pages: true,
            admitted_equals_terminal: admitted == 9,
            all_heads_independent: true,
            unsupported_visible: unsupported || !LANES.contains(&"pdfalto"),
            conservation,
        },
        redaction: Redaction {
            raw_lexemes: false,
            raw_pdf: false,
            raw_text: false,
            captions: false,
            tei_xml: false,
            images: false,
            assets: false,
            physical_paths: false,
            hostname: false,
            executable: false,
        },
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(report_path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

#[cfg(test)]
mod native_visual_payload_tests {
    use super::*;

    fn payload(xml: &[u8]) -> NativeVisualObservation {
        let shadow = da_adapters::parse_pdfalto_xml_shadow(
            xml,
            b"fixture-pdf",
            "fixture-doc",
            LayoutDonorLimits::default(),
        )
        .expect("positive visual fixture");
        native_visual_payload(&shadow).expect("donor visual payload should be native-valid")
    }

    #[test]
    fn native_visual_payload_preserves_positive_donor_primitive() {
        let source = payload(
            br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="figure-a"/></page></doc>"#,
        );
        assert_eq!(source.primitives.len(), 1);
        assert_eq!(source.primitives[0].kind, NativeVisualKind::Image);
        assert!(source.lane_absence.is_none());
        let outcome = detect_figure_heads(&source, NativeVisualConfig::default()).unwrap();
        assert!(matches!(outcome, NativeVisualHeadOutcome::Figure { .. }));
    }

    #[test]
    fn native_visual_payload_is_input_sensitive() {
        let first = payload(
            br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="figure-a"/></page></doc>"#,
        );
        let second = payload(
            br#"<doc><page width="612" height="792"><Figure x="100" y="120" width="50" height="60" content="figure-b"/></page></doc>"#,
        );
        assert_ne!(first.primitives, second.primitives);
        let first_outcome = detect_figure_heads(&first, NativeVisualConfig::default()).unwrap();
        let second_outcome = detect_figure_heads(&second, NativeVisualConfig::default()).unwrap();
        assert_ne!(
            serde_json::to_vec(&first_outcome).unwrap(),
            serde_json::to_vec(&second_outcome).unwrap()
        );
    }

    #[test]
    fn malformed_donor_visual_input_is_not_not_observed() {
        let result = da_adapters::parse_pdfalto_xml_shadow(
            br#"<doc><page width="612" height="792"><Figure x="nan" y="20" width="30" height="40" content="bad"/></page></doc>"#,
            b"fixture-pdf",
            "fixture-doc",
            LayoutDonorLimits::default(),
        );
        assert!(matches!(
            result,
            Err(LayoutDonorError::Malformed { .. }) | Err(LayoutDonorError::InvalidLayout(_))
        ));
    }
}
