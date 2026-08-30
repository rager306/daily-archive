//! Bounded, resumable, same-input layout donor bakeoff (ADR-075, S13/T04).
//! Shadow-only: this example never writes graph/cache state and never selects a route.

use da_adapters::{
    LayoutDonor, LayoutDonorError, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoLayoutDonor,
    XyCutLayoutDonor,
};
use da_cli::layout_runtime::{
    CpuKernelCharacterization, RuntimeConfig, Terminal, characterize_cpu_kernel,
    detect_system_topology, run_bounded,
};
use da_parse::LayoutDocument;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const REPORT_SCHEMA: &str = "layout-bakeoff-report.v3";
const MANIFEST_SCHEMA: &str = "layout-bakeoff.v1";
const REQUIRED_LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    corpus: Corpus,
    lanes: LaneConfig,
    oracle: Option<OracleSpec>,
}
#[derive(Debug, Deserialize)]
struct Corpus {
    id: String,
    corpus_n: usize,
    page_denominator: usize,
    documents: Vec<DocumentSpec>,
}
#[derive(Debug, Deserialize)]
struct DocumentSpec {
    id: String,
    source_pdf: String,
    sha256: String,
    pages: usize,
}
#[derive(Debug, Deserialize)]
struct LaneConfig {
    required: Vec<String>,
    same_input: bool,
    all_lanes_required: bool,
}
#[derive(Debug, Deserialize)]
struct OracleSpec {
    fixture: String,
}
#[derive(Debug, Deserialize)]
struct OracleFile {
    receipts: Vec<OracleReceipt>,
}
#[derive(Debug, Deserialize)]
struct OracleReceipt {
    document_id: String,
    page_count: usize,
    page_ordinals: Vec<u32>,
    token_receipts: Vec<OracleToken>,
}
#[derive(Debug, Clone, Deserialize)]
struct OracleToken {
    page_ordinal: u32,
    token_ordinal: u32,
    bbox_72dpi_top_left: [f64; 4],
    token_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Report {
    schema_version: String,
    manifest_fingerprint: String,
    source_fingerprint: String,
    donor_fingerprint: String,
    corpus_id: String,
    corpus_n: usize,
    page_denominator: usize,
    lanes: Vec<String>,
    all_lanes_required: bool,
    promotion_state: String,
    auto_promote: String,
    same_input: bool,
    topology: TopologyReport,
    execution: ExecutionEnvelope,
    worker_matrix: Vec<WorkerRow>,
    acceptance_denominator: Denominator,
    benchmark_denominator: Denominator,
    breakpoints: Breakpoints,
    rollback_target: String,
    route_state: String,
    outcomes: Vec<Outcome>,
    metrics: Metrics,
    optimization: CpuKernelCharacterization,
    gate: Gate,
    redaction: Redaction,
    /// Optional so reports written before this field existed (the canonical S13
    /// artifact) still deserialize as `prior` for replay verification.
    #[serde(default)]
    evidence_binding: EvidenceBinding,
    /// "final" for the canonical report write, "partial" for an interrupted
    /// incremental checkpoint. Empty in pre-existing reports.
    #[serde(default)]
    report_state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Outcome {
    ordinal: usize,
    unit_key: String,
    document_id: String,
    lane: String,
    source_sha256: String,
    status: String,
    failure_code: Option<String>,
    pages: usize,
    expected_pages: usize,
    token_count: usize,
    span_count: usize,
    coordinate_valid: usize,
    coordinate_coverage: String,
    lost_tokens: usize,
    extra_tokens: usize,
    duplicate_tokens: usize,
    reordered_tokens: usize,
    oracle_receipts: usize,
    oracle_token_receipts: usize,
    oracle_order_inversions: Option<usize>,
    oracle_bbox_mismatches: Option<usize>,
    semantic_hash: Option<String>,
    latency_ms: u128,
    throughput_tokens_per_sec: Option<u64>,
    queue_wait_ms: u128,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Metrics {
    queue_capacity: usize,
    workers: usize,
    max_in_flight: usize,
    max_observed_queue: usize,
    max_observed_in_flight: usize,
    input_queue_high_water: usize,
    result_queue_high_water: usize,
    queue_wait_ms: u128,
    peak_rss_bytes: Option<u64>,
    peak_rss_status: String,
    allocation_or_private_capacity_bytes: usize,
    private_scratch_high_water_bytes: usize,
    latency_p50_ms: u128,
    latency_p95_ms: u128,
    latency_p99_ms: u128,
    throughput_tokens_per_sec: u64,
    throughput_units_per_second: u64,
    per_worker_terminal_counts: BTreeMap<String, usize>,
    per_lane: BTreeMap<String, LaneMetrics>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TopologyReport {
    source: String,
    identity: String,
    allowed_cpus: usize,
    physical_core_cap: usize,
    digest: String,
    status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionEnvelope {
    scope: String,
    requested_workers: usize,
    effective_workers: usize,
    max_in_flight: usize,
    input_queue_capacity: usize,
    result_queue_capacity: usize,
    private_scratch_capacity_bytes: usize,
    admission: String,
    cpu_only_excludes_subprocess_io: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerRow {
    requested_workers: usize,
    effective_workers: usize,
    status: String,
    admitted: usize,
    terminal: usize,
    semantic_equal_to_baseline: bool,
    // Additive fields: defaulted so pre-v3 reports still deserialize as prior.
    #[serde(default)]
    latency_p50_ms: u128,
    #[serde(default)]
    latency_p95_ms: u128,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    lane_scope: String,
    breakpoint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Denominator {
    corpus_n: usize,
    lanes_n: usize,
    pages_per_lane: usize,
    total_lane_pages: usize,
    repetitions: usize,
    pooled_with_acceptance: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Breakpoints {
    saturation: String,
    first_regression: String,
    queue_pressure: String,
    memory_pressure: String,
    production_default: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LaneMetrics {
    units: usize,
    admitted: usize,
    terminal: usize,
    p50_ms: u128,
    p95_ms: u128,
    p99_ms: u128,
    throughput_units_per_second: u64,
    queue_wait_ms: u128,
    semantic_equal_to_baseline: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Gate {
    exact_same_n: bool,
    exact_unique_unit_keys: bool,
    exact_page_denominator: bool,
    lane_page_denominators: BTreeMap<String, usize>,
    total_lane_pages: usize,
    complete_receipts: bool,
    semantic_replays_equal: bool,
    conservation_complete: bool,
    oracle_complete: bool,
    hard_vetoes: Vec<String>,
    result: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EvidenceBinding {
    /// sha256 over (this runner binary digest | manifest fingerprint | crate
    /// version | report schema). The binary digest makes the identity exact:
    /// any source change that reaches the tested executable changes the value.
    #[serde(default)]
    tested_source_revision: String,
    /// Env-declared clean-tree claim. This is a DECLARATION, not proof; the
    /// binding proof is the durable_evidence_ref captured by the invoking
    /// flow while the tree was actually clean.
    #[serde(default)]
    declared_clean_tree: bool,
    /// sha256 over the exact argv of this invocation. Digest only — argv text
    /// (and therefore paths) never enters durable evidence.
    #[serde(default)]
    command_digest: String,
    /// sha256 over the full runtime configuration: workers, max in-flight,
    /// input queue, result queue and private scratch capacity.
    #[serde(default)]
    runtime_config_fingerprint: String,
    /// sha256 over (DATA limits fingerprint | donor fingerprint).
    #[serde(default)]
    configuration_fingerprint: String,
    #[serde(default)]
    topology_fingerprint: String,
    #[serde(default)]
    durable_evidence_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Redaction {
    raw_pdf: bool,
    raw_text: bool,
    tei_xml: bool,
    page_images: bool,
    physical_paths: bool,
    token_text: bool,
    executable: bool,
    hostname: bool,
    topology_path: bool,
}
#[derive(Clone)]
struct Unit {
    ordinal: usize,
    id: String,
    bytes: Arc<Vec<u8>>,
    lane: String,
    donor: Arc<dyn LayoutDonor>,
    expected_pages: usize,
    queued_at: Instant,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn fingerprint(m: &Manifest, inputs: &[(String, Vec<u8>)]) -> String {
    let mut h = Sha256::new();
    h.update(m.schema_version.as_bytes());
    h.update(m.corpus.id.as_bytes());
    if let Some(oracle) = &m.oracle {
        h.update(oracle.fixture.as_bytes());
        if let Ok(bytes) = fs::read(&oracle.fixture) {
            h.update(sha(&bytes).as_bytes());
        }
    }
    for (id, b) in inputs {
        h.update(id.as_bytes());
        h.update(sha(b).as_bytes());
    }
    format!("sha256:{:x}", h.finalize())
}
struct UnavailableDonor;
impl LayoutDonor for UnavailableDonor {
    fn id(&self) -> &'static str {
        "pdfalto"
    }
    fn version(&self) -> &'static str {
        "unavailable"
    }
    fn extract(
        &self,
        _pdf_bytes: &[u8],
        _document_id: &str,
        _limits: LayoutDonorLimits,
    ) -> Result<LayoutDocument, LayoutDonorError> {
        Err(LayoutDonorError::Unavailable {
            executable: "not-configured".into(),
        })
    }
}

fn failure(e: &LayoutDonorError) -> &'static str {
    match e {
        LayoutDonorError::Unavailable { .. } => "unavailable",
        LayoutDonorError::Timeout { .. } => "timeout",
        LayoutDonorError::Malformed { .. } => "malformed",
        LayoutDonorError::NonZero { .. } => "nonzero",
        LayoutDonorError::Oversize { .. } => "oversize",
        LayoutDonorError::Encrypted => "encrypted",
        LayoutDonorError::Truncated => "truncated",
        LayoutDonorError::InvalidLayout(_) => "invalid_layout",
        LayoutDonorError::Io(_) => "io",
    }
}
type ParsedArgs = (
    PathBuf,
    PathBuf,
    Option<PathBuf>,
    LayoutDonorLimits,
    usize,
    usize,
    usize,
    bool,
);
type LoadedManifest = (Manifest, Vec<(String, Vec<u8>)>);

fn parse_args() -> Result<ParsedArgs, String> {
    let mut manifest = None;
    let mut report = None;
    let mut pdfalto = None;
    let mut limits = LayoutDonorLimits::default();
    let mut workers = 1usize;
    let mut queue_capacity = 2usize;
    let mut max_in_flight = 1usize;
    let mut resume = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        macro_rules! val {
            () => {
                args.next().ok_or_else(|| format!("missing value for {a}"))
            };
        }
        match a.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(val!()?)),
            "--report" => report = Some(PathBuf::from(val!()?)),
            "--pdfalto" => pdfalto = Some(PathBuf::from(val!()?)),
            "--workers" => workers = val!()?.parse().map_err(|_| "invalid workers".to_string())?,
            "--queue-capacity" => {
                queue_capacity = val!()?
                    .parse()
                    .map_err(|_| "invalid queue-capacity".to_string())?
            }
            "--max-in-flight" => {
                max_in_flight = val!()?
                    .parse()
                    .map_err(|_| "invalid max-in-flight".to_string())?
            }
            "--max-pages" => {
                limits.max_pages = val!()?
                    .parse()
                    .map_err(|_| "invalid max-pages".to_string())?
            }
            "--max-tokens" => {
                limits.max_tokens = val!()?
                    .parse()
                    .map_err(|_| "invalid max-tokens".to_string())?
            }
            "--timeout-ms" => {
                limits.timeout_ms = val!()?
                    .parse()
                    .map_err(|_| "invalid timeout-ms".to_string())?
            }
            "--max-output-bytes" => {
                limits.max_output_bytes = val!()?
                    .parse()
                    .map_err(|_| "invalid max-output-bytes".to_string())?
            }
            "--resume" => resume = true,
            "--help" => {
                println!(
                    "layout_bakeoff --manifest FILE --report FILE --pdfalto EXE [--workers N --queue-capacity N --max-in-flight N --resume]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if workers == 0 || queue_capacity == 0 || max_in_flight == 0 {
        return Err("workers, queue-capacity and max-in-flight must be positive".into());
    }
    Ok((
        manifest.ok_or("--manifest is required")?,
        report.ok_or("--report is required")?,
        pdfalto,
        limits,
        workers,
        queue_capacity,
        max_in_flight,
        resume,
    ))
}
fn load_manifest(path: &Path) -> Result<LoadedManifest, String> {
    let raw = fs::read(path).map_err(|e| format!("manifest read: {e}"))?;
    let m: Manifest = serde_yaml::from_slice(&raw).map_err(|e| format!("manifest schema: {e}"))?;
    if m.schema_version != MANIFEST_SCHEMA {
        return Err("unsupported manifest schema_version".into());
    }
    if !m.lanes.same_input || m.corpus.documents.len() != m.corpus.corpus_n {
        return Err("same-n corpus denominator mismatch".into());
    }
    if m.lanes.required.len() != REQUIRED_LANES.len()
        || REQUIRED_LANES
            .iter()
            .any(|x| !m.lanes.required.iter().any(|y| y == x))
        || m.lanes
            .required
            .iter()
            .any(|x| !REQUIRED_LANES.contains(&x.as_str()))
    {
        return Err("required donor lane set must be exactly the three independent lanes".into());
    }
    let mut ids = BTreeSet::new();
    let mut pages = 0;
    let mut out = Vec::new();
    for d in &m.corpus.documents {
        if !ids.insert(d.id.clone()) {
            return Err("duplicate document id".into());
        }
        let b = fs::read(&d.source_pdf).map_err(|e| format!("source read for {}: {e}", d.id))?;
        if sha(&b) != d.sha256 {
            return Err(format!("checksum mismatch for {}", d.id));
        }
        pages += d.pages;
        out.push((d.id.clone(), b));
    }
    if pages != m.corpus.page_denominator {
        return Err("page denominator mismatch".into());
    }
    if let Some(oracle) = &m.oracle {
        fs::read(&oracle.fixture).map_err(|e| format!("oracle read: {e}"))?;
    }
    Ok((m, out))
}
fn load_oracle(m: &Manifest) -> Result<BTreeMap<(String, u32), Vec<OracleToken>>, String> {
    let Some(s) = &m.oracle else {
        return Ok(BTreeMap::new());
    };
    let raw = fs::read(&s.fixture).map_err(|e| format!("oracle read: {e}"))?;
    let f: OracleFile = serde_yaml::from_slice(&raw).map_err(|e| format!("oracle schema: {e}"))?;
    let mut out = BTreeMap::new();
    for r in f.receipts {
        if r.page_count != r.page_ordinals.len()
            || r.page_ordinals.iter().copied().ne(0..r.page_count as u32)
        {
            return Err(format!(
                "oracle page denominator mismatch for {}",
                r.document_id
            ));
        }
        // The independent fixture uses zero-based physical page indexes while
        // LayoutDocument conserves the donors' one-based page identity.
        for fixture_page in r.page_ordinals {
            let document_page = fixture_page + 1;
            let tokens = r
                .token_receipts
                .iter()
                .filter(|token| token.page_ordinal == fixture_page)
                .cloned()
                .collect();
            if out
                .insert((r.document_id.clone(), document_page), tokens)
                .is_some()
            {
                return Err("duplicate oracle page".into());
            }
        }
    }
    Ok(out)
}
/// (spans, coordinate_valid, lost, extra, reordered, page_receipts,
/// token_receipts, order_inversions, bbox_mismatches, duplicates)
type MeasureCounts = (
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
    usize,
);

fn measure(
    doc: &LayoutDocument,
    oracle: &BTreeMap<(String, u32), Vec<OracleToken>>,
) -> MeasureCounts {
    let mut spans = 0;
    let mut valid = 0;
    let mut reordered = 0;
    let mut page_receipts = 0;
    let mut token_receipts = 0;
    let mut bbox = 0;
    let mut inv = 0;
    let mut expected = 0;
    let mut seen_pages = 0;
    let mut extra = 0usize;
    let mut duplicates = 0usize;
    for p in &doc.pages {
        let mut prev = None;
        if let Some(rows) = oracle.get(&(doc.document_id.clone(), p.page_ordinal)) {
            page_receipts += 1;
            token_receipts += rows.len();
            expected += rows.len();
            seen_pages += 1;
        }
        // Duplicate tokens: repetitions of the same produced token_ordinal on
        // one page beyond its first occurrence. Defined from the produced
        // document alone, so the counter is observable even without oracle rows.
        let mut seen_ordinals = std::collections::BTreeSet::new();
        let mut matched_on_page = 0usize;
        for t in &p.tokens {
            if !seen_ordinals.insert(t.token_ordinal) {
                duplicates += 1;
            }
            if !t.source_span.is_absent() {
                spans += 1;
            }
            let b = [t.bbox.x_min, t.bbox.y_min, t.bbox.x_max, t.bbox.y_max];
            if b.iter().all(|v| v.is_finite())
                && t.bbox.x_min <= t.bbox.x_max
                && t.bbox.y_min <= t.bbox.y_max
            {
                valid += 1;
            }
            if prev.is_some_and(|x| x > t.source_ordinal) {
                inv += 1;
                reordered += 1;
            }
            prev = Some(t.source_ordinal);
            if let Some(rows) = oracle.get(&(doc.document_id.clone(), p.page_ordinal))
                && let Some(o) = rows.iter().find(|row| row.token_ordinal == t.token_ordinal)
            {
                matched_on_page += 1;
                let b = [t.bbox.x_min, t.bbox.y_min, t.bbox.x_max, t.bbox.y_max];
                if b.iter()
                    .zip(o.bbox_72dpi_top_left)
                    .any(|(a, b)| (a - b).abs() > 0.01)
                    || t.text_digest != o.token_digest
                {
                    bbox += 1;
                }
            }
        }
        // Extra tokens: produced tokens the oracle does not match on that
        // page. On pages without oracle rows every produced token is extra.
        extra += p.tokens.len().saturating_sub(matched_on_page);
    }
    (
        spans,
        valid,
        expected.saturating_sub(doc.pages.iter().map(|p| p.tokens.len()).sum()),
        extra,
        reordered,
        page_receipts,
        token_receipts,
        if seen_pages > 0 { Some(inv) } else { None },
        if seen_pages > 0 { Some(bbox) } else { None },
        duplicates,
    )
}
fn run_unit(
    u: Unit,
    limits: LayoutDonorLimits,
    oracle: &BTreeMap<(String, u32), Vec<OracleToken>>,
) -> Outcome {
    let wait = u.queued_at.elapsed().as_millis();
    let start = Instant::now();
    let source_sha256 = sha(&u.bytes);
    let result = u.donor.extract(&u.bytes, &u.id, limits);
    let latency = start.elapsed().as_millis();
    match result {
        Ok(doc) => {
            let tokens = doc.pages.iter().map(|p| p.tokens.len()).sum();
            let (
                spans,
                valid,
                lost,
                extra,
                reordered,
                receipts,
                token_receipts,
                inv,
                bbox,
                duplicates,
            ) = measure(&doc, oracle);
            let hash = doc.digest().ok();
            Outcome {
                ordinal: u.ordinal,
                unit_key: format!("{}:{}", u.id, u.lane),
                document_id: u.id,
                lane: u.lane,
                source_sha256,
                status: "ok".into(),
                failure_code: None,
                pages: doc.pages.len(),
                expected_pages: u.expected_pages,
                token_count: tokens,
                span_count: spans,
                coordinate_valid: valid,
                coordinate_coverage: if tokens == valid {
                    "complete".into()
                } else {
                    "partial".into()
                },
                lost_tokens: lost,
                extra_tokens: extra,
                duplicate_tokens: duplicates,
                reordered_tokens: reordered,
                oracle_receipts: receipts,
                oracle_token_receipts: token_receipts,
                oracle_order_inversions: inv,
                oracle_bbox_mismatches: bbox,
                semantic_hash: hash,
                latency_ms: latency,
                throughput_tokens_per_sec: (latency > 0)
                    .then_some((tokens as u128 * 1000 / latency).min(u64::MAX as u128) as u64),
                queue_wait_ms: wait,
            }
        }
        Err(e) => Outcome {
            ordinal: u.ordinal,
            unit_key: format!("{}:{}", u.id, u.lane),
            document_id: u.id,
            lane: u.lane,
            source_sha256,
            status: "terminal_failure".into(),
            failure_code: Some(failure(&e).into()),
            pages: 0,
            expected_pages: u.expected_pages,
            token_count: 0,
            span_count: 0,
            coordinate_valid: 0,
            coordinate_coverage: "none".into(),
            lost_tokens: 0,
            extra_tokens: 0,
            duplicate_tokens: 0,
            reordered_tokens: 0,
            oracle_receipts: 0,
            oracle_token_receipts: 0,
            oracle_order_inversions: None,
            oracle_bbox_mismatches: None,
            semantic_hash: None,
            latency_ms: latency,
            throughput_tokens_per_sec: None,
            queue_wait_ms: wait,
        },
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}
fn rss() -> Option<u64> {
    let raw = fs::read_to_string("/proc/self/status").ok()?;
    raw.lines().find_map(|l| {
        l.strip_prefix("VmHWM:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
            .map(|kb| kb * 1024)
    })
}
fn make_report(
    m: &Manifest,
    mf: &str,
    df: &str,
    outcomes: Vec<Outcome>,
    metrics: Metrics,
    optimization: CpuKernelCharacterization,
    semantic_replays_equal: bool,
    topology: &da_cli::layout_runtime::TopologyReceipt,
    config: RuntimeConfig,
    worker_terminal_counts: &BTreeMap<String, usize>,
    worker_matrix: Vec<WorkerRow>,
) -> Report {
    let total = m.corpus.corpus_n * REQUIRED_LANES.len();
    let keys: BTreeSet<_> = outcomes.iter().map(|o| o.unit_key.clone()).collect();
    let exact_unique = outcomes.len() == total
        && keys.len() == total
        && outcomes
            .iter()
            .all(|o| REQUIRED_LANES.contains(&o.lane.as_str()));
    let mut lane_pages = BTreeMap::new();
    for lane in REQUIRED_LANES {
        let pages = outcomes
            .iter()
            .filter(|o| o.lane == lane && o.status == "ok")
            .map(|o| o.pages)
            .sum();
        lane_pages.insert(lane.to_string(), pages);
    }
    let total_lane_pages = lane_pages.values().sum();
    let per_document_pages = outcomes
        .iter()
        .all(|o| o.status == "ok" && o.pages == o.expected_pages);
    let pages_ok = per_document_pages
        && REQUIRED_LANES.iter().all(|lane| {
            lane_pages.get(*lane).copied() == Some(m.corpus.page_denominator)
                && m.corpus.documents.iter().all(|d| {
                    outcomes.iter().any(|o| {
                        o.document_id == d.id
                            && o.lane == *lane
                            && o.status == "ok"
                            && o.pages == d.pages
                    })
                })
        })
        && total_lane_pages == m.corpus.page_denominator * REQUIRED_LANES.len();
    let mut hard: Vec<String> = outcomes
        .iter()
        .filter(|o| o.status != "ok")
        .map(|o| {
            format!(
                "{}:{}:{}",
                o.document_id,
                o.lane,
                o.failure_code.as_deref().unwrap_or("unknown")
            )
        })
        .collect();
    for outcome in outcomes.iter().filter(|o| o.status == "ok") {
        if outcome.lost_tokens > 0 || outcome.extra_tokens > 0 || outcome.duplicate_tokens > 0 {
            hard.push(format!(
                "{}:{}:conservation:{}:{}:{}",
                outcome.document_id,
                outcome.lane,
                outcome.lost_tokens,
                outcome.extra_tokens,
                outcome.duplicate_tokens
            ));
        }
        if outcome.oracle_receipts != outcome.expected_pages {
            hard.push(format!(
                "{}:{}:oracle_pages:{}/{}",
                outcome.document_id, outcome.lane, outcome.oracle_receipts, outcome.expected_pages
            ));
        }
    }
    let conservation = outcomes.iter().all(|o| {
        o.status == "ok"
            && o.coordinate_coverage == "complete"
            && o.lost_tokens == 0
            && o.extra_tokens == 0
            && o.duplicate_tokens == 0
    });
    let oracle_complete = m.oracle.is_some()
        && outcomes.iter().all(|o| {
            o.status == "ok"
                && o.oracle_receipts == o.expected_pages
                && o.oracle_order_inversions.is_some()
                && o.oracle_bbox_mismatches.is_some()
        });
    let gate = Gate {
        exact_same_n: outcomes.len() == total,
        exact_unique_unit_keys: exact_unique,
        exact_page_denominator: pages_ok,
        lane_page_denominators: lane_pages,
        total_lane_pages,
        complete_receipts: exact_unique,
        semantic_replays_equal,
        conservation_complete: conservation,
        oracle_complete,
        hard_vetoes: hard.clone(),
        result: if exact_unique
            && pages_ok
            && semantic_replays_equal
            && conservation
            && oracle_complete
            && hard.is_empty()
        {
            "candidate_evidence_complete".into()
        } else {
            "no_selection".into()
        },
    };
    let mut per_lane = BTreeMap::new();
    for lane in REQUIRED_LANES {
        let mut samples = outcomes
            .iter()
            .filter(|o| o.lane == lane)
            .map(|o| o.latency_ms)
            .collect::<Vec<_>>();
        samples.sort_unstable();
        let percentile = |p: usize| {
            samples
                .get((samples.len().saturating_sub(1) * p) / 100)
                .copied()
                .unwrap_or(0)
        };
        let units = samples.len();
        per_lane.insert(
            lane.to_string(),
            LaneMetrics {
                units,
                admitted: units,
                terminal: units,
                p50_ms: percentile(50),
                p95_ms: percentile(95),
                p99_ms: percentile(99),
                throughput_units_per_second: if percentile(50) == 0 {
                    0
                } else {
                    (units as u128 * 1000 / percentile(50)).min(u64::MAX as u128) as u64
                },
                queue_wait_ms: outcomes
                    .iter()
                    .filter(|o| o.lane == lane)
                    .map(|o| o.queue_wait_ms)
                    .sum(),
                semantic_equal_to_baseline: semantic_replays_equal,
            },
        );
    }
    Report {
        schema_version: REPORT_SCHEMA.into(),
        manifest_fingerprint: mf.into(),
        source_fingerprint: mf.into(),
        donor_fingerprint: df.into(),
        corpus_id: m.corpus.id.clone(),
        corpus_n: m.corpus.corpus_n,
        page_denominator: m.corpus.page_denominator,
        lanes: REQUIRED_LANES.iter().map(|x| x.to_string()).collect(),
        all_lanes_required: m.lanes.all_lanes_required,
        promotion_state: "candidate".into(),
        auto_promote: "forbidden".into(),
        same_input: true,
        topology: TopologyReport {
            source: "linux_allowed_cpu_set_intersected_with_sysfs_package_core".into(),
            identity: "physical_package_id_and_core_id".into(),
            allowed_cpus: topology.allowed_cpus,
            physical_core_cap: topology.physical_cores,
            digest: topology.digest.clone(),
            status: "measured".into(),
        },
        execution: ExecutionEnvelope {
            scope: "invocation_private".into(),
            requested_workers: config.workers,
            effective_workers: config.workers,
            max_in_flight: config.max_in_flight,
            input_queue_capacity: config.input_queue_capacity,
            result_queue_capacity: config.result_queue_capacity,
            private_scratch_capacity_bytes: config.private_scratch_capacity,
            admission: "bounded_backpressure_no_drop".into(),
            cpu_only_excludes_subprocess_io: true,
        },
        worker_matrix,
        acceptance_denominator: Denominator {
            corpus_n: m.corpus.corpus_n,
            lanes_n: REQUIRED_LANES.len(),
            pages_per_lane: m.corpus.page_denominator,
            total_lane_pages: m.corpus.page_denominator * REQUIRED_LANES.len(),
            repetitions: 1,
            pooled_with_acceptance: false,
        },
        benchmark_denominator: Denominator {
            corpus_n: m.corpus.corpus_n,
            lanes_n: REQUIRED_LANES.len(),
            pages_per_lane: m.corpus.page_denominator,
            total_lane_pages: m.corpus.page_denominator * REQUIRED_LANES.len(),
            repetitions: optimization.repetitions,
            pooled_with_acceptance: false,
        },
        breakpoints: Breakpoints {
            saturation: "inconclusive".into(),
            first_regression: "inconclusive".into(),
            queue_pressure: "inconclusive".into(),
            memory_pressure: "inconclusive".into(),
            production_default: "forbidden".into(),
        },
        rollback_target: "java-grobid-http@0.9.0-crf".into(),
        route_state: "stage.pdfalto_layout=fallback".into(),
        outcomes,
        metrics: Metrics {
            per_worker_terminal_counts: worker_terminal_counts.clone(),
            per_lane,
            ..metrics
        },
        optimization,
        gate,
        redaction: Redaction {
            raw_pdf: false,
            raw_text: false,
            tei_xml: false,
            page_images: false,
            physical_paths: false,
            token_text: false,
            executable: false,
            hostname: false,
            topology_path: false,
        },
        // Bound post-construction by the caller (keeps make_report arity at
        // its pre-existing shape); defaults keep the struct initializable.
        evidence_binding: EvidenceBinding::default(),
        report_state: String::new(),
    }
}

fn main() -> Result<(), String> {
    // ADR-052 composition root: .env-backed rolling JSON logging.
    let _log_guard = da_cli::logging::init_pipeline_logging("layout_bakeoff");
    let (
        manifest_path,
        report_path,
        pdfalto,
        limits,
        workers,
        queue_capacity,
        max_in_flight,
        resume,
    ) = parse_args()?;
    let (m, inputs) = load_manifest(&manifest_path)?;
    let oracle = load_oracle(&m)?;
    let mf = fingerprint(&m, &inputs);
    // Exact tested source identity: sha256 over (this runner binary digest |
    // manifest fingerprint | crate version | report schema). The binary digest
    // makes it source-exact — any change that reaches the compiled runner
    // changes the value. Digests only; no paths or command text in evidence.
    let tested_source_revision = {
        let binary_digest = std::env::current_exe()
            .ok()
            .and_then(|p| fs::read(p).ok())
            .map(|bytes| format!("{:x}", Sha256::digest(&bytes)))
            .unwrap_or_else(|| "unavailable".to_string());
        format!(
            "sha256:{:x}",
            Sha256::digest(format!(
                "{}|{}|{}|{}",
                binary_digest,
                mf,
                env!("CARGO_PKG_VERSION"),
                REPORT_SCHEMA
            ))
        )
    };
    // Redacted command binding: digest of the exact argv of this invocation.
    let command_digest = format!(
        "sha256:{:x}",
        Sha256::digest(
            std::env::args()
                .collect::<Vec<_>>()
                .join("\u{1f}")
                .as_bytes()
        )
    );
    let limits_fp = format!(
        "{}:{}:{}:{}",
        limits.max_pages, limits.max_tokens, limits.max_output_bytes, limits.timeout_ms
    );
    let df = format!(
        "sha256:{:x}",
        Sha256::digest(format!(
            "{}|{}|{}|{}|{}",
            REQUIRED_LANES.join("\n"),
            limits_fp,
            workers,
            max_in_flight,
            queue_capacity
        ))
    );
    let prior = fs::read(&report_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Report>(&b).ok());
    let partial_path = report_path.with_extension("partial.json");
    let partial = if resume {
        fs::read(&partial_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Report>(&b).ok())
    } else {
        None
    };
    let config = RuntimeConfig {
        workers,
        input_queue_capacity: queue_capacity,
        result_queue_capacity: queue_capacity,
        max_in_flight,
        private_scratch_capacity: 64 * 1024,
    };
    let runtime_config_fp = format!(
        "sha256:{:x}",
        Sha256::digest(format!(
            "{}|{}|{}|{}|{}",
            config.workers,
            config.max_in_flight,
            config.input_queue_capacity,
            config.result_queue_capacity,
            config.private_scratch_capacity
        ))
    );
    let mut completed = BTreeMap::<String, Outcome>::new();
    if let Some(source) = partial.filter(|p| {
        p.manifest_fingerprint == mf
            && p.source_fingerprint == mf
            && p.donor_fingerprint == df
            && p.evidence_binding.runtime_config_fingerprint == runtime_config_fp
    }) {
        for o in source.outcomes {
            completed.entry(o.unit_key.clone()).or_insert(o);
        }
    }
    let inspector: Arc<dyn LayoutDonor> = Arc::new(PdfInspectorLayoutDonor);
    let xy: Arc<dyn LayoutDonor> =
        Arc::new(XyCutLayoutDonor::new(Box::new(PdfInspectorLayoutDonor)));
    let alto: Arc<dyn LayoutDonor> = match pdfalto {
        Some(p) => Arc::new(PdfaltoLayoutDonor::new(p)),
        None => Arc::new(UnavailableDonor),
    };
    let mut donors = BTreeMap::<String, Arc<dyn LayoutDonor>>::new();
    donors.insert("pdfalto".into(), alto);
    donors.insert("pdf_inspector_positioned".into(), inspector);
    donors.insert("pdf_inspector_positioned_xy_cut".into(), xy);
    let mut outcomes = Vec::new();
    let mut jobs = Vec::new();
    let mut ordinal = 0;
    for (id, bytes) in &inputs {
        let expected = m
            .corpus
            .documents
            .iter()
            .find(|d| d.id == *id)
            .map(|d| d.pages)
            .ok_or_else(|| format!("manifest document missing from corpus inputs: {id}"))?;
        // Materialize each source once per invocation; lane units share the
        // immutable bytes and never create a per-lane PDF copy.
        let shared_bytes = Arc::new(bytes.clone());
        for lane in REQUIRED_LANES {
            let key = format!("{}:{}", id, lane);
            if let Some(mut o) = completed.remove(&key) {
                o.ordinal = ordinal;
                outcomes.push(o);
                ordinal += 1;
                continue;
            }
            jobs.push(Unit {
                ordinal,
                id: id.clone(),
                bytes: shared_bytes.clone(),
                lane: lane.into(),
                donor: donors
                    .get(lane)
                    .cloned()
                    .ok_or_else(|| format!("donor lane not configured: {lane}"))?,
                expected_pages: expected,
                queued_at: Instant::now(),
            });
            ordinal += 1;
        }
    }
    // CPU-scaling matrix must never attribute real-donor subprocess/I/O to
    // worker scaling (T05 F04): it runs only over in-process deterministic
    // lanes. The real donor composition (pdfalto included) runs once, at the
    // single representative configured worker count, in the main run below.
    let donor_jobs = jobs;
    let cpu_jobs: Vec<Unit> = donor_jobs
        .iter()
        .filter(|unit| unit.lane != "pdfalto")
        .cloned()
        .collect();
    let topology = detect_system_topology().map_err(|e| format!("topology_unavailable: {e}"))?;
    // The matrix is a sequential benchmark dimension, never pooled into the
    // acceptance denominator. It runs the deterministic in-process CPU lanes
    // (see cpu_jobs above) so pdfalto subprocess/I/O is never attributed to
    // CPU scaling; the full donor composition runs once in the main run below.
    let mut worker_matrix = Vec::new();
    let mut matrix_baseline = None;
    for matrix_workers in 1..=topology.physical_cores {
        let matrix_config = RuntimeConfig {
            workers: matrix_workers,
            ..config
        };
        let matrix_oracle = oracle.clone();
        let matrix = run_bounded(
            matrix_config,
            &topology,
            cpu_jobs.clone(),
            move |u, scratch| {
                scratch.allocate(1).map_err(|e| e.to_string())?;
                Ok::<Outcome, String>(run_unit(u, limits, &matrix_oracle))
            },
        )
        .map_err(|e| format!("invalid_config: {e}"))?;
        let matrix_signature = matrix
            .terminals
            .iter()
            .filter_map(|terminal| match terminal {
                Terminal::Completed { result, .. } => Some(result.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let matrix_complete = matrix.admitted == matrix.terminals.len()
            && matrix.terminals.len() == cpu_jobs.len()
            && matrix.coordinator_failure.is_none();
        let signature = semantic_signature(&matrix_signature);
        let semantic_equal = matrix_complete
            && matrix_baseline
                .as_ref()
                .is_some_and(|baseline| baseline == &signature);
        let row_equal = semantic_equal || (matrix_workers == 1 && matrix_complete);
        if matrix_baseline.is_none() && matrix_complete {
            matrix_baseline = Some(signature);
        }
        // Observed latency receipts for this CPU-matrix row (in-process lanes
        // only — no subprocess/I/O attribution to CPU scaling).
        let mut row_lat = matrix_signature
            .iter()
            .map(|o| o.latency_ms)
            .collect::<Vec<_>>();
        row_lat.sort_unstable();
        let row_pct = |p: f64| {
            row_lat
                .get(((row_lat.len().saturating_sub(1)) as f64 * p) as usize)
                .copied()
                .unwrap_or(0)
        };
        let row_tokens: u64 = matrix_signature.iter().map(|o| o.token_count as u64).sum();
        worker_matrix.push(WorkerRow {
            requested_workers: matrix_workers,
            effective_workers: matrix_workers,
            status: if matrix_complete {
                "observed"
            } else {
                "inconclusive"
            }
            .into(),
            admitted: matrix.admitted,
            terminal: matrix.terminals.len(),
            semantic_equal_to_baseline: row_equal,
            latency_p50_ms: row_pct(0.5),
            latency_p95_ms: row_pct(0.95),
            total_tokens: row_tokens,
            lane_scope: "cpu_in_process".into(),
            breakpoint: if !matrix_complete {
                "incomplete".into()
            } else if row_equal {
                "none".into()
            } else {
                "first_mismatch".into()
            },
        });
    }
    let runtime = run_bounded(config, &topology, donor_jobs, move |u, scratch| {
        // The arena is intentionally exercised by the coordinator callback;
        // donor-owned PDF bytes remain immutable and shared through Arc.
        scratch.allocate(1).map_err(|e| e.to_string())?;
        Ok::<Outcome, String>(run_unit(u, limits, &oracle))
    })
    .map_err(|e| format!("invalid_config: {e}"))?;
    if runtime.admitted != runtime.terminals.len() {
        return Err(format!(
            "coordinator failure: admitted={} terminal={}",
            runtime.admitted,
            runtime.terminals.len()
        ));
    }
    let mut worker_terminal_counts = BTreeMap::<String, usize>::new();
    // Incremental interruption: when the env threshold is set, stop consuming
    // terminals as soon as the accumulated receipt count (including resumed
    // outcomes) reaches it. Remaining terminals stay conserved inside the
    // runtime result; the partial checkpoint below is written before exit.
    let interrupt_after = std::env::var("DA_LAYOUT_BAKEOFF_INTERRUPT_AFTER")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|after| *after > 0);
    let mut interrupted_at: Option<usize> = None;
    for terminal in runtime.terminals {
        let worker = match &terminal {
            Terminal::Completed { worker, .. }
            | Terminal::Failed { worker, .. }
            | Terminal::CoordinatorFailed { worker, .. } => *worker,
        };
        *worker_terminal_counts
            .entry(format!("worker-{worker}"))
            .or_default() += 1;
        match terminal {
            Terminal::Completed { result, .. } => {
                outcomes.push(result);
                if let Some(after) = interrupt_after
                    && outcomes.len() >= after
                {
                    interrupted_at = Some(outcomes.len());
                    break;
                }
            }
            Terminal::Failed { error, .. } => return Err(format!("worker job failure: {error}")),
            Terminal::CoordinatorFailed { failure, .. } => {
                return Err(format!("coordinator failure: {failure:?}"));
            }
        }
    }
    outcomes.sort_by_key(|o| o.ordinal);
    let semantic_replays_equal = if interrupted_at.is_some() {
        false
    } else {
        prior.as_ref().is_some_and(|p| {
            p.manifest_fingerprint == mf
                && p.source_fingerprint == mf
                && p.donor_fingerprint == df
                && semantic_signature(&p.outcomes) == semantic_signature(&outcomes)
        })
    };
    let mut lat = outcomes.iter().map(|o| o.latency_ms).collect::<Vec<_>>();
    lat.sort_unstable();
    let pctl = |p: f64| {
        lat.get(((lat.len().saturating_sub(1)) as f64 * p).round() as usize)
            .copied()
            .unwrap_or(0)
    };
    let throughput = outcomes
        .iter()
        .filter_map(|outcome| outcome.throughput_tokens_per_sec)
        .sum();
    // Characterize only the in-process receipt/digest kernel. Donor
    // subprocess and filesystem time is intentionally excluded from this
    // optimization evidence and remains in the lane outcomes above.
    let characterization_input = semantic_signature(&outcomes).into_bytes();
    let work_units = outcomes.len() as u64;
    let optimization = characterize_cpu_kernel(1, 5, 64 * 1024, work_units, move |scratch| {
        let size = characterization_input.len();
        let slot = scratch.allocate(size)?;
        slot.copy_from_slice(&characterization_input);
        Ok(slot.to_vec())
    })
    .map_err(|e| format!("CPU characterization failed: {e}"))?;
    let queue_wait_ms = outcomes.iter().map(|o| o.queue_wait_ms).sum();
    let outcome_count = outcomes.len();
    let private_scratch_high_water_bytes = runtime
        .private_scratch_high_water
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let mut report = make_report(
        &m,
        &mf,
        &df,
        outcomes,
        Metrics {
            queue_capacity,
            workers: config.workers,
            max_in_flight,
            max_observed_queue: runtime
                .metrics
                .input_queue_high_water
                .max(runtime.metrics.result_queue_high_water),
            max_observed_in_flight: runtime.metrics.in_flight_high_water,
            peak_rss_bytes: rss(),
            peak_rss_status: if rss().is_some() {
                "measured".into()
            } else {
                "unavailable".into()
            },
            latency_p50_ms: pctl(0.5),
            latency_p95_ms: pctl(0.95),
            latency_p99_ms: pctl(0.99),
            input_queue_high_water: runtime.metrics.input_queue_high_water,
            result_queue_high_water: runtime.metrics.result_queue_high_water,
            queue_wait_ms,
            allocation_or_private_capacity_bytes: config.private_scratch_capacity * config.workers,
            private_scratch_high_water_bytes,
            throughput_tokens_per_sec: throughput,
            throughput_units_per_second: if lat.is_empty() {
                0
            } else {
                (outcome_count as u128 * 1000 / lat.iter().sum::<u128>().max(1))
                    .min(u64::MAX as u128) as u64
            },
            per_worker_terminal_counts: BTreeMap::new(),
            per_lane: BTreeMap::new(),
        },
        optimization,
        semantic_replays_equal,
        &topology,
        config,
        &worker_terminal_counts,
        worker_matrix,
    );
    report.evidence_binding = EvidenceBinding {
        tested_source_revision,
        declared_clean_tree: std::env::var("DA_LAYOUT_BAKEOFF_CLEAN_TREE")
            .map(|v| v == "true")
            .unwrap_or(false),
        command_digest,
        runtime_config_fingerprint: runtime_config_fp,
        configuration_fingerprint: format!(
            "sha256:{:x}",
            Sha256::digest(format!("{limits_fp}|{df}"))
        ),
        topology_fingerprint: topology.digest.clone(),
        durable_evidence_ref: std::env::var("DA_LAYOUT_BAKEOFF_EVIDENCE_REF").ok(),
    };
    report.report_state = "final".into();
    if let Some(after) = interrupted_at {
        let mut partial_report = report;
        partial_report.gate.result = "no_selection".into();
        partial_report.report_state = "partial".into();
        atomic_write(
            &partial_path,
            &serde_json::to_vec_pretty(&partial_report).map_err(|e| e.to_string())?,
        )?;
        return Err(format!(
            "interrupted after {after} terminal receipts; incremental partial checkpoint written"
        ));
    }
    atomic_write(
        &report_path,
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    fs::remove_file(partial_path).ok();
    println!(
        "layout-bakeoff: schema={} corpus_n={} outcomes={} lanes={} gate={} promotion_state=candidate",
        REPORT_SCHEMA,
        report.corpus_n,
        report.outcomes.len(),
        report.lanes.len(),
        report.gate.result
    );
    Ok(())
}
#[derive(Serialize)]
struct SemanticReceipt<'a> {
    unit_key: &'a String,
    status: &'a String,
    source_sha256: &'a String,
    failure_code: &'a Option<String>,
    semantic_hash: &'a Option<String>,
    pages: usize,
    token_count: usize,
    span_count: usize,
    coordinate_valid: usize,
    coordinate_coverage: &'a String,
    lost_tokens: usize,
    extra_tokens: usize,
    duplicate_tokens: usize,
    reordered_tokens: usize,
    oracle_receipts: usize,
    oracle_token_receipts: usize,
    oracle_order_inversions: &'a Option<usize>,
    oracle_bbox_mismatches: &'a Option<usize>,
}

fn semantic_signature(outcomes: &[Outcome]) -> String {
    let mut ordered = outcomes.to_vec();
    ordered.sort_by(|a, b| a.unit_key.cmp(&b.unit_key));
    let receipts = ordered
        .iter()
        .map(|o| SemanticReceipt {
            unit_key: &o.unit_key,
            status: &o.status,
            source_sha256: &o.source_sha256,
            failure_code: &o.failure_code,
            semantic_hash: &o.semantic_hash,
            pages: o.pages,
            token_count: o.token_count,
            span_count: o.span_count,
            coordinate_valid: o.coordinate_valid,
            coordinate_coverage: &o.coordinate_coverage,
            lost_tokens: o.lost_tokens,
            extra_tokens: o.extra_tokens,
            duplicate_tokens: o.duplicate_tokens,
            reordered_tokens: o.reordered_tokens,
            oracle_receipts: o.oracle_receipts,
            oracle_token_receipts: o.oracle_token_receipts,
            oracle_order_inversions: &o.oracle_order_inversions,
            oracle_bbox_mismatches: &o.oracle_bbox_mismatches,
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&receipts).expect("semantic receipt serialization");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprint_is_stable_and_lane_set_is_three() {
        let m = Manifest {
            schema_version: MANIFEST_SCHEMA.into(),
            corpus: Corpus {
                id: "x".into(),
                corpus_n: 0,
                page_denominator: 0,
                documents: vec![],
            },
            lanes: LaneConfig {
                required: REQUIRED_LANES.iter().map(|x| x.to_string()).collect(),
                same_input: true,
                all_lanes_required: true,
            },
            oracle: None,
        };
        assert_eq!(fingerprint(&m, &[]), fingerprint(&m, &[]));
        assert_eq!(REQUIRED_LANES.len(), 3);
    }
}
