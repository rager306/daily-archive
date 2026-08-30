//! Bounded, resumable, same-input layout donor bakeoff (ADR-075, S13/T04).
//! Shadow-only: this example never writes graph/cache state and never selects a route.

use da_adapters::{
    LayoutDonor, LayoutDonorError, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoLayoutDonor,
    XyCutLayoutDonor,
};
use da_parse::LayoutDocument;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Instant;

const REPORT_SCHEMA: &str = "layout-bakeoff-report.v2";
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
    outcomes: Vec<Outcome>,
    metrics: Metrics,
    gate: Gate,
    redaction: Redaction,
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
    peak_rss_bytes: Option<u64>,
    peak_rss_status: String,
    latency_p50_ms: u128,
    latency_p95_ms: u128,
    throughput_tokens_per_sec: u64,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Redaction {
    raw_pdf: bool,
    raw_text: bool,
    tei_xml: bool,
    page_images: bool,
    physical_paths: bool,
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
fn measure(
    doc: &LayoutDocument,
    oracle: &BTreeMap<(String, u32), Vec<OracleToken>>,
) -> (
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut spans = 0;
    let mut valid = 0;
    let mut reordered = 0;
    let mut page_receipts = 0;
    let mut token_receipts = 0;
    let mut bbox = 0;
    let mut inv = 0;
    let mut expected = 0;
    let mut seen_pages = 0;
    for p in &doc.pages {
        let mut prev = None;
        if let Some(rows) = oracle.get(&(doc.document_id.clone(), p.page_ordinal)) {
            page_receipts += 1;
            token_receipts += rows.len();
            expected += rows.len();
            seen_pages += 1;
        }
        for t in &p.tokens {
            if !t.source_span.is_absent() {
                spans += 1;
            }
            valid += 1;
            if prev.is_some_and(|x| x > t.source_ordinal) {
                inv += 1;
                reordered += 1;
            }
            prev = Some(t.source_ordinal);
            if let Some(rows) = oracle.get(&(doc.document_id.clone(), p.page_ordinal))
                && let Some(o) = rows.iter().find(|row| row.token_ordinal == t.token_ordinal)
            {
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
    }
    (
        spans,
        valid,
        expected.saturating_sub(doc.pages.iter().map(|p| p.tokens.len()).sum()),
        0,
        reordered,
        page_receipts,
        token_receipts,
        if seen_pages > 0 { Some(inv) } else { None },
        if seen_pages > 0 { Some(bbox) } else { None },
    )
}
fn run_unit(
    u: Unit,
    limits: LayoutDonorLimits,
    oracle: &BTreeMap<(String, u32), Vec<OracleToken>>,
    inflight: &AtomicUsize,
) -> Outcome {
    let wait = u.queued_at.elapsed().as_millis();
    let start = Instant::now();
    let source_sha256 = sha(&u.bytes);
    let result = u.donor.extract(&u.bytes, &u.id, limits);
    inflight.fetch_sub(1, Ordering::SeqCst);
    let latency = start.elapsed().as_millis();
    match result {
        Ok(doc) => {
            let tokens = doc.pages.iter().map(|p| p.tokens.len()).sum();
            let (spans, valid, lost, extra, reordered, receipts, token_receipts, inv, bbox) =
                measure(&doc, oracle);
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
                duplicate_tokens: 0,
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
    semantic_replays_equal: bool,
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
        outcomes,
        metrics,
        gate,
        redaction: Redaction {
            raw_pdf: false,
            raw_text: false,
            tei_xml: false,
            page_images: false,
            physical_paths: false,
        },
    }
}

fn main() -> Result<(), String> {
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
    let mut completed = BTreeMap::<String, Outcome>::new();
    if let Some(source) = partial.filter(|p| {
        p.manifest_fingerprint == mf && p.source_fingerprint == mf && p.donor_fingerprint == df
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
            .unwrap_or(0);
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
                bytes: Arc::new(bytes.clone()),
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
    let total = outcomes.len() + jobs.len();
    let queue_depth = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let max_queue = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let permits = Arc::new(Mutex::new((0..max_in_flight).collect::<Vec<usize>>()));
    let (tx0, rx) = mpsc::sync_channel::<Unit>(queue_capacity);
    let mut tx = Some(tx0);
    let rx = Arc::new(Mutex::new(rx));
    let (result_tx, result_rx) = mpsc::channel();
    let effective_workers = workers.min(max_in_flight).max(1);
    let mut handles = Vec::new();
    for _ in 0..effective_workers {
        let rx = rx.clone();
        let result_tx = result_tx.clone();
        let oracle = oracle.clone();
        let queue_depth = queue_depth.clone();
        let active = active.clone();
        let max_active = max_active.clone();
        let permits = permits.clone();
        handles.push(thread::spawn(move || {
            while let Ok(u) = rx.lock().unwrap().recv() {
                queue_depth.fetch_sub(1, Ordering::SeqCst);
                let permit = loop {
                    if let Some(p) = permits.lock().unwrap().pop() {
                        break p;
                    }
                    thread::yield_now();
                };
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                let o = run_unit(u, limits, &oracle, &active);
                permits.lock().unwrap().push(permit);
                if result_tx.send(o).is_err() {
                    break;
                }
            }
        }));
    }
    drop(result_tx);
    let interrupt_after = std::env::var("DA_LAYOUT_BAKEOFF_INTERRUPT_AFTER")
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    let mut next_job = jobs.into_iter();
    let mut received = 0usize;
    let mut send_closed = false;
    let checkpoint = |outcomes: &Vec<Outcome>, mq: usize, ma: usize| -> Result<(), String> {
        let r = make_report(
            &m,
            &mf,
            &df,
            outcomes.clone(),
            Metrics {
                queue_capacity,
                workers: effective_workers,
                max_in_flight,
                max_observed_queue: mq,
                max_observed_in_flight: ma,
                peak_rss_bytes: rss(),
                peak_rss_status: if rss().is_some() {
                    "measured".into()
                } else {
                    "unavailable".into()
                },
                ..Metrics::default()
            },
            false,
        );
        atomic_write(
            &partial_path,
            &serde_json::to_vec_pretty(&r).map_err(|e| e.to_string())?,
        )
    };
    loop {
        while !send_closed && queue_depth.load(Ordering::SeqCst) < queue_capacity {
            if let Some(mut u) = next_job.next() {
                u.queued_at = Instant::now();
                queue_depth.fetch_add(1, Ordering::SeqCst);
                max_queue.fetch_max(queue_depth.load(Ordering::SeqCst), Ordering::SeqCst);
                if tx.as_ref().expect("sender open").send(u).is_err() {
                    queue_depth.fetch_sub(1, Ordering::SeqCst);
                    return Err("worker queue closed".into());
                }
            } else {
                send_closed = true;
                drop(tx.take());
            }
        }
        if outcomes.len() >= total {
            break;
        }
        match result_rx.recv() {
            Ok(o) => {
                outcomes.push(o);
                received += 1;
                checkpoint(
                    &outcomes,
                    max_queue.load(Ordering::SeqCst),
                    max_active.load(Ordering::SeqCst),
                )?;
                if interrupt_after == Some(received) {
                    return Err("forced interruption after terminal unit".into());
                }
            }
            Err(_) if send_closed => break,
            Err(_) => return Err("result channel closed".into()),
        }
    }
    drop(tx.take());
    for h in handles {
        h.join().map_err(|_| "worker panicked".to_string())?;
    }
    outcomes.sort_by_key(|o| o.ordinal);
    let semantic_replays_equal = prior.as_ref().is_some_and(|p| {
        p.manifest_fingerprint == mf
            && p.source_fingerprint == mf
            && p.donor_fingerprint == df
            && semantic_signature(&p.outcomes) == semantic_signature(&outcomes)
    });
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
    let report = make_report(
        &m,
        &mf,
        &df,
        outcomes,
        Metrics {
            queue_capacity,
            workers: effective_workers,
            max_in_flight,
            max_observed_queue: max_queue.load(Ordering::SeqCst),
            max_observed_in_flight: max_active.load(Ordering::SeqCst),
            peak_rss_bytes: rss(),
            peak_rss_status: if rss().is_some() {
                "measured".into()
            } else {
                "unavailable".into()
            },
            latency_p50_ms: pctl(0.5),
            latency_p95_ms: pctl(0.95),
            throughput_tokens_per_sec: throughput,
        },
        semantic_replays_equal,
    );
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
