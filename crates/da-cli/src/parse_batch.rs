//! `da parse-batch` — the R116 composition root for the bounded resumable
//! parse batch (M464-z84in2 S06/T04).
//!
//! This module is the ONLY place where manifest paths, physical cache
//! roots, endpoint URL and accelerator settings meet the provider-neutral
//! batch API: manifest paths are read here and every logical batch item
//! receives bytes; [`crate::parse_config::ParseRuntimeConfig`] owns all
//! physical/tuning knobs; `da-parse` receives none of them.
//!
//! Wiring per run: `GrobidHttpBackend` (D204 profile identity untouched) +
//! `FileParseArtifactCache` (T03 atomic payload cache) inside the T02
//! `TwoStageParseCache` + `ParseBatchExecutor`, driven to one typed
//! terminal outcome per admitted document and one atomic versioned result
//! report. The report is the canonical terminal evidence; the optional
//! redb accelerator (D295, feature `redb`) stores only rebuildable cursor
//! lookup and manifest index rows — deleting or corrupting it never
//! destroys payloads or completed evidence.
//!
//! Redaction: no PDF/TEI bytes, response bodies, credentials, base URL or
//! physical roots enter the report JSON or any log field.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::Args;
use da_adapters::parse_cache::FileParseArtifactCache;
use da_parse::{
    BatchCursor, CacheMode, GrobidHttpBackend, HeaderDialectV1, ParseBackendRequest, ParseBatch,
    ParseBatchCollector, ParseBatchExecutor, ParseBatchItem, ParseBatchLimits,
    ParseBatchMemoryCheckpoint, ParseBatchRunSummary, ParseDocumentFailure, ParseDocumentOutcome,
    ReferencesDialectV1, ordered_input_fingerprint,
};
use serde::{Deserialize, Serialize};

use crate::parse_config::{ParseConfigError, ParseRuntimeOverrides, resolve};

/// Stable schema version of the atomic terminal result report.
pub const PARSE_BATCH_REPORT_SCHEMA_VERSION: u32 = 1;
/// Stable schema version of the CLI manifest format.
pub const PARSE_BATCH_MANIFEST_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// Arguments of `da parse-batch`. Every physical/tuning knob falls through
/// `explicit CLI value > DA_PARSE_* env > composition-root default`.
#[derive(Debug, Clone, Args)]
pub struct ParseBatchArgs {
    /// JSON batch manifest (schema v1): ordered documents with paper ids
    /// and PDF paths (paths are read only here — the batch gets bytes).
    #[arg(long)]
    pub manifest: PathBuf,

    /// Terminal result report destination (JSON schema v1, atomic write,
    /// the canonical durable terminal evidence).
    #[arg(long)]
    pub report: PathBuf,

    /// Cache mode: off | read-only | write-through | refresh
    /// (env DA_PARSE_CACHE_MODE, default write-through).
    #[arg(long)]
    pub mode: Option<String>,

    /// GROBID endpoint (canonical env DA_GROBID_URL; legacy GROBID_URL is
    /// an explicit alias — both set must match or resolution fails closed).
    #[arg(long)]
    pub grobid_url: Option<String>,

    /// Backend selector (env DA_PARSE_BACKEND; closed: grobid-http).
    #[arg(long)]
    pub backend: Option<String>,

    /// Canonical parse artifact cache root (env DA_PARSE_CACHE_ROOT).
    #[arg(long)]
    pub cache_root: Option<PathBuf>,

    /// Maximum documents per admitted batch (env DA_PARSE_BATCH_SIZE).
    #[arg(long)]
    pub batch_size: Option<usize>,

    /// Worker concurrency bound (env DA_PARSE_WORKERS).
    #[arg(long)]
    pub workers: Option<usize>,

    /// Admission queue capacity (env DA_PARSE_QUEUE_CAPACITY).
    #[arg(long)]
    pub queue_capacity: Option<usize>,

    /// Simultaneously executing documents (CLI-only; default: workers).
    #[arg(long)]
    pub max_in_flight: Option<usize>,

    /// Resume from the durable cursor evidence bound to this exact input
    /// fingerprint (accelerator lookup, else the previous terminal report).
    #[arg(long)]
    pub resume: bool,

    /// Enable the redb accelerator at this path (feature `redb`).
    #[arg(long, conflicts_with = "no_redb")]
    pub redb_index: Option<PathBuf>,

    /// Explicitly disable the redb accelerator (default when unset).
    #[arg(long)]
    pub no_redb: bool,

    /// redb accelerator path without flipping the enabled flag
    /// (env DA_PARSE_REDB_INDEX_PATH).
    #[arg(long)]
    pub redb_index_path: Option<PathBuf>,
}

impl ParseBatchArgs {
    /// Project CLI arguments onto the closed override surface.
    fn overrides(&self) -> Result<ParseRuntimeOverrides, ParseBatchCliError> {
        if self.redb_index.is_some() && self.no_redb {
            return Err(ParseBatchCliError::ConflictingRedbFlags);
        }
        Ok(ParseRuntimeOverrides {
            grobid_url: self.grobid_url.clone(),
            backend: self.backend.clone(),
            cache_mode: self.mode.clone(),
            cache_root: self.cache_root.clone(),
            batch_size: self.batch_size,
            workers: self.workers,
            queue_capacity: self.queue_capacity,
            max_in_flight: self.max_in_flight,
            redb_enabled: if self.redb_index.is_some() {
                Some(true)
            } else if self.no_redb {
                Some(false)
            } else {
                None
            },
            redb_path: self
                .redb_index
                .clone()
                .or_else(|| self.redb_index_path.clone()),
        })
    }
}

// ---------------------------------------------------------------------------
// Typed CLI failure vocabulary
// ---------------------------------------------------------------------------

/// Closed `da parse-batch` failure surface (before any document work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseBatchCliError {
    /// Composition config rejected (closed validation, pre-backend).
    Config(ParseConfigError),
    /// `--redb-index` and `--no-redb` were both provided.
    ConflictingRedbFlags,
    /// The accelerator was enabled but this binary was built without the
    /// `redb` feature (fail closed — never a silent disable).
    RedbNotCompiled,
    /// The enabled accelerator could not be opened (corruption is an
    /// error, not a silent miss; deleting the file is a safe rebuild).
    RedbUnavailable(String),
    /// The manifest file could not be read.
    ManifestRead(String),
    /// The manifest is not a schema-v1 document (unknown fields/version).
    ManifestSchema(String),
    /// The manifest violates a closed validation rule.
    ManifestInvalid(String),
    /// A manifest-referenced source PDF could not be read.
    SourceRead(String),
    /// The previous terminal report could not be used as resume evidence.
    ResumeEvidenceUnavailable(String),
    /// The atomic terminal report could not be written.
    ReportWrite(String),
}

impl std::fmt::Display for ParseBatchCliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "parse-batch config: {error}"),
            Self::ConflictingRedbFlags => {
                write!(
                    formatter,
                    "parse-batch: --redb-index and --no-redb conflict"
                )
            }
            Self::RedbNotCompiled => write!(
                formatter,
                "parse-batch: redb accelerator enabled but not compiled in (build with --features redb, or disable the accelerator)"
            ),
            Self::RedbUnavailable(detail) => {
                write!(
                    formatter,
                    "parse-batch: redb accelerator unavailable: {detail}"
                )
            }
            Self::ManifestRead(detail) => write!(formatter, "parse-batch manifest read: {detail}"),
            Self::ManifestSchema(detail) => {
                write!(formatter, "parse-batch manifest schema: {detail}")
            }
            Self::ManifestInvalid(rule) => {
                write!(formatter, "parse-batch manifest invalid: {rule}")
            }
            Self::SourceRead(detail) => {
                write!(formatter, "parse-batch source read: {detail}")
            }
            Self::ResumeEvidenceUnavailable(detail) => write!(
                formatter,
                "parse-batch resume: no durable cursor evidence matches this input fingerprint ({detail})"
            ),
            Self::ReportWrite(detail) => {
                write!(formatter, "parse-batch report write: {detail}")
            }
        }
    }
}

impl std::error::Error for ParseBatchCliError {}

impl From<ParseConfigError> for ParseBatchCliError {
    fn from(error: ParseConfigError) -> Self {
        Self::Config(error)
    }
}

// ---------------------------------------------------------------------------
// Manifest (paths are read ONLY here; the logical batch receives bytes)
// ---------------------------------------------------------------------------

/// Closed per-document endpoint selector (closed manifest vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseManifestEndpoint {
    /// `POST /api/processFulltextDocument` (default).
    Fulltext,
    /// `POST /api/processHeaderDocument`.
    Header,
    /// `POST /api/processReferences`.
    References,
}

impl ParseManifestEndpoint {
    /// Closed wire token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fulltext => "fulltext",
            Self::Header => "header",
            Self::References => "references",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "fulltext" => Some(Self::Fulltext),
            "header" => Some(Self::Header),
            "references" => Some(Self::References),
            _ => None,
        }
    }
}

/// One manifest document: logical paper id + source path + closed endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseManifestEntry {
    /// Logical paper id (never a physical path obligation).
    pub paper_id: String,
    /// Source PDF path — resolved to bytes by the CLI, never exported.
    pub pdf_path: String,
    /// Closed endpoint for the document (default fulltext).
    pub endpoint: ParseManifestEndpoint,
}

/// `da parse-batch` manifest (schema v1). Paths in this document are
/// consumed only by the CLI; unknown fields are rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseBatchManifestV1 {
    /// Manifest schema version (only 1 is accepted).
    pub schema_version: u32,
    /// Ordered documents (the admission order = the ordinal order).
    pub documents: Vec<ParseBatchManifestDocument>,
}

/// One manifest document entry (wire form).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseBatchManifestDocument {
    /// Logical paper id.
    pub paper_id: String,
    /// Source PDF path (CLI-local; read to bytes).
    pub pdf_path: String,
    /// Closed endpoint token; omitted = `fulltext`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Read + validate the manifest into logical entries (paths preserved
/// verbatim for this module's own file reads).
pub fn load_manifest(path: &Path) -> Result<Vec<ParseManifestEntry>, ParseBatchCliError> {
    let raw =
        std::fs::read(path).map_err(|error| ParseBatchCliError::ManifestRead(error.to_string()))?;
    let manifest: ParseBatchManifestV1 = serde_json::from_slice(&raw)
        .map_err(|error| ParseBatchCliError::ManifestSchema(error.to_string()))?;
    if manifest.schema_version != PARSE_BATCH_MANIFEST_SCHEMA_VERSION {
        return Err(ParseBatchCliError::ManifestSchema(format!(
            "unsupported manifest schema_version {} (supported: {PARSE_BATCH_MANIFEST_SCHEMA_VERSION})",
            manifest.schema_version
        )));
    }
    if manifest.documents.is_empty() {
        return Err(ParseBatchCliError::ManifestInvalid(
            "documents must not be empty".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut entries = Vec::with_capacity(manifest.documents.len());
    for document in manifest.documents {
        if document.paper_id.trim().is_empty() {
            return Err(ParseBatchCliError::ManifestInvalid(
                "paper_id must not be blank".into(),
            ));
        }
        if !seen.insert(document.paper_id.clone()) {
            return Err(ParseBatchCliError::ManifestInvalid(format!(
                "duplicate paper_id {}",
                document.paper_id
            )));
        }
        if document.pdf_path.trim().is_empty() {
            return Err(ParseBatchCliError::ManifestInvalid(format!(
                "pdf_path must not be blank for paper {}",
                document.paper_id
            )));
        }
        let endpoint = match document.endpoint.as_deref() {
            None => ParseManifestEndpoint::Fulltext,
            Some(token) => ParseManifestEndpoint::parse(token).ok_or_else(|| {
                ParseBatchCliError::ManifestInvalid(format!(
                    "unknown endpoint token {token} (accepted: fulltext | header | references)"
                ))
            })?,
        };
        entries.push(ParseManifestEntry {
            paper_id: document.paper_id,
            pdf_path: document.pdf_path,
            endpoint,
        });
    }
    Ok(entries)
}

/// Read every manifest source to bytes and project the closed endpoint
/// defaults onto provider-neutral requests (rich legacy dialect mapping —
/// D204 identity untouched).
fn build_requests(
    entries: &[ParseManifestEntry],
) -> Result<Vec<ParseBackendRequest>, ParseBatchCliError> {
    entries
        .iter()
        .map(|entry| {
            let bytes = std::fs::read(&entry.pdf_path)
                .map_err(|error| ParseBatchCliError::SourceRead(error.to_string()))?;
            let request = match entry.endpoint {
                ParseManifestEndpoint::Fulltext => {
                    ParseBackendRequest::new(entry.paper_id.clone(), bytes)
                }
                ParseManifestEndpoint::Header => ParseBackendRequest::header(
                    entry.paper_id.clone(),
                    bytes,
                    HeaderDialectV1::legacy_rich(),
                ),
                ParseManifestEndpoint::References => ParseBackendRequest::references(
                    entry.paper_id.clone(),
                    bytes,
                    ReferencesDialectV1::legacy_rich(),
                ),
            };
            Ok(request)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Terminal result report (canonical durable evidence)
// ---------------------------------------------------------------------------

/// serde projection of the closed run summary (the summary type itself is
/// a plain value in da-parse; the report owns the wire form).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseBatchRunSummaryWire {
    /// Run-summary vocabulary schema version.
    pub schema_version: u32,
    /// Admitted documents.
    pub admitted: u32,
    /// Documents skipped because the resume cursor already passed them.
    pub skipped_on_resume: u32,
    /// Terminal documents this run produced.
    pub completed: u32,
    /// Terminal successes.
    pub succeeded: u32,
    /// Terminal failures (typed, per document).
    pub failed: u32,
    /// Backend invocations (never retried by contract).
    pub backend_calls: u64,
    /// Raw-stage cache hits.
    pub raw_hits: u64,
    /// Read-only legacy identity hits on the raw stage.
    pub raw_legacy_hits: u64,
    /// Raw-stage publications.
    pub raw_writes: u64,
    /// Honest failed raw publications.
    pub raw_write_failures: u64,
    /// Projection-stage cache hits.
    pub projector_hits: u64,
    /// Projection-stage publications.
    pub projector_writes: u64,
    /// Honest failed projection publications.
    pub projector_write_failures: u64,
    /// Observed maximum in-flight documents (bounded-load gauge).
    pub max_in_flight_observed: usize,
    /// Observed maximum queue depth (bounded-load gauge).
    pub max_queue_depth_observed: usize,
    /// Durable sink errors.
    pub sink_errors: u64,
    /// Checkpoint errors.
    pub checkpoint_errors: u64,
}

impl From<&ParseBatchRunSummary> for ParseBatchRunSummaryWire {
    fn from(summary: &ParseBatchRunSummary) -> Self {
        Self {
            schema_version: summary.schema_version(),
            admitted: summary.admitted(),
            skipped_on_resume: summary.skipped_on_resume(),
            completed: summary.completed(),
            succeeded: summary.succeeded(),
            failed: summary.failed(),
            backend_calls: summary.backend_calls(),
            raw_hits: summary.raw_hits(),
            raw_legacy_hits: summary.raw_legacy_hits(),
            raw_writes: summary.raw_writes(),
            raw_write_failures: summary.raw_write_failures(),
            projector_hits: summary.projector_hits(),
            projector_writes: summary.projector_writes(),
            projector_write_failures: summary.projector_write_failures(),
            max_in_flight_observed: summary.max_in_flight_observed(),
            max_queue_depth_observed: summary.max_queue_depth_observed(),
            sink_errors: summary.sink_errors(),
            checkpoint_errors: summary.checkpoint_errors(),
        }
    }
}

/// Success terminal signal of one document: the canonical endpoint/result
/// dialect identity. Deliberately WITHOUT the `ParsedArticle` payload —
/// the projected article (including its `tei_xml` conservation material)
/// is canonical payload evidence of the artifact cache, never report
/// content (raw TEI/PDF never enter logs/reports).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseDocumentSuccessReport {
    /// Canonical result identity the request stamped (endpoint/result
    /// dialect identity preserved verbatim).
    pub result_identity: String,
}

/// One per-document terminal signal entry (schema-versioned; unknown
/// fields rejected). Correlated by admission ordinal and logical paper id,
/// carrying both stage dispositions and the closed terminal result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseDocumentReportEntry {
    /// Outcome vocabulary schema version.
    pub schema_version: u32,
    /// Stable admission ordinal.
    pub ordinal: u32,
    /// Logical paper id.
    pub paper_id: String,
    /// Cache mode of the run.
    pub cache_mode: CacheMode,
    /// Both stage dispositions (raw TEI + projection).
    pub dispositions: da_parse::ParseCacheDispositions,
    /// Terminal result: success identity signal or typed closed failure.
    pub result: Result<ParseDocumentSuccessReport, ParseDocumentFailure>,
}

impl From<&ParseDocumentOutcome> for ParseDocumentReportEntry {
    fn from(outcome: &ParseDocumentOutcome) -> Self {
        let result = match outcome.result() {
            Ok(success) => Ok(ParseDocumentSuccessReport {
                result_identity: success.result_identity().to_string(),
            }),
            Err(failure) => Err(failure.clone()),
        };
        Self {
            schema_version: outcome.schema_version(),
            ordinal: outcome.ordinal(),
            paper_id: outcome.paper_id().to_string(),
            cache_mode: outcome.cache_mode(),
            dispositions: *outcome.dispositions(),
            result,
        }
    }
}

/// Atomic versioned terminal result report (schema v1): the canonical
/// durable terminal evidence of one batch run. No PDF/TEI bytes, response
/// bodies, credentials, base URL or physical roots ever appear here —
/// payload evidence lives in the `FileParseArtifactCache`, not the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseBatchReportV1 {
    /// Report schema version (only 1 is accepted on read).
    pub schema_version: u32,
    /// Ordered input content fingerprint of the admitted batch.
    pub input_fingerprint: String,
    /// Cache mode of the run (cost strategy, not identity).
    pub cache_mode: CacheMode,
    /// Effective bounds of the run (excluded from identity).
    pub limits: ParseBatchLimits,
    /// Durable cursor at the end of the run (contiguous terminal prefix).
    pub cursor: BatchCursor,
    /// Ordinal the run resumed from (0 on a fresh run).
    pub resumed_from_ordinal: u32,
    /// Closed run summary (counters + bounded-load gauges).
    pub summary: ParseBatchRunSummaryWire,
    /// One terminal signal entry per admitted ordinal.
    pub documents: Vec<ParseDocumentReportEntry>,
}

impl ParseBatchReportV1 {
    /// Build the report from run results.
    pub fn build(summary: &ParseBatchRunSummary, documents: &[ParseDocumentOutcome]) -> Self {
        Self {
            schema_version: PARSE_BATCH_REPORT_SCHEMA_VERSION,
            input_fingerprint: summary.input_fingerprint().to_string(),
            cache_mode: summary.mode(),
            limits: *summary.limits(),
            cursor: summary.cursor().clone(),
            resumed_from_ordinal: summary.resumed_from_ordinal(),
            summary: ParseBatchRunSummaryWire::from(summary),
            documents: documents
                .iter()
                .map(ParseDocumentReportEntry::from)
                .collect(),
        }
    }

    /// Deserialize a report, rejecting foreign schema versions.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let report: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if report.schema_version != PARSE_BATCH_REPORT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported report schema_version {} (supported: {PARSE_BATCH_REPORT_SCHEMA_VERSION})",
                report.schema_version
            ));
        }
        Ok(report)
    }
}

static REPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Atomic file write: unique temp sibling (create_new → sync → rename).
/// A failed write never leaves a partially visible report at `path`.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "report.json".to_string());
    let mut last_error = None;
    for _ in 0..8 {
        let sequence = REPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let temp = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp);
        match file {
            Ok(mut handle) => {
                let outcome = (|| -> std::io::Result<()> {
                    std::io::Write::write_all(&mut handle, bytes)?;
                    handle.sync_all()?;
                    Ok(())
                })();
                drop(handle);
                match outcome {
                    Ok(()) => {
                        return std::fs::rename(&temp, path);
                    }
                    Err(error) => {
                        let _ = std::fs::remove_file(&temp);
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("atomic temp file retries exhausted")))
}

// ---------------------------------------------------------------------------
// Resume cursor resolution (accelerator lookup → canonical report fallback)
// ---------------------------------------------------------------------------

/// Resolve the resume cursor for `fingerprint`, in priority order:
///
/// 1. the redb accelerator lookup (when enabled — the fast path);
/// 2. the previous atomic terminal report at `report_path` (canonical
///    durable evidence — also the REBUILD path after the accelerator file
///    was deleted or corrupted);
///
/// No matching evidence is a TYPED error — `--resume` never silently
/// restarts from zero, and a foreign-fingerprint report is not evidence.
fn resolve_resume_cursor(
    fingerprint: &str,
    report_path: &Path,
    accelerator_cursor: Option<Result<Option<u32>, String>>,
) -> Result<BatchCursor, ParseBatchCliError> {
    if let Some(lookup) = accelerator_cursor {
        let ordinal = lookup.map_err(ParseBatchCliError::RedbUnavailable)?;
        if let Some(next_ordinal) = ordinal {
            return Ok(BatchCursor::at(fingerprint, next_ordinal));
        }
    }
    let bytes = match std::fs::read(report_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ParseBatchCliError::ResumeEvidenceUnavailable(
                "no previous terminal report".into(),
            ));
        }
        Err(error) => {
            return Err(ParseBatchCliError::ResumeEvidenceUnavailable(format!(
                "previous report unreadable: {error}"
            )));
        }
    };
    let report = ParseBatchReportV1::from_json(&bytes)
        .map_err(ParseBatchCliError::ResumeEvidenceUnavailable)?;
    if report.input_fingerprint != fingerprint {
        // A foreign fingerprint is not evidence for this input — fail
        // closed instead of silently restarting (the admission layer
        // would reject the mismatched cursor anyway).
        return Err(ParseBatchCliError::ResumeEvidenceUnavailable(
            "previous report is bound to a different input fingerprint".into(),
        ));
    }
    Ok(report.cursor)
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

/// Async run of `da parse-batch` (the CLI main blocks on this).
pub async fn run(args: &ParseBatchArgs) -> Result<(), ParseBatchCliError> {
    run_with_env(args, &|key| std::env::var(key).ok()).await
}

/// Run with an injected environment lookup (test seam — avoids process-env
/// races in parallel tests).
pub async fn run_with_env(
    args: &ParseBatchArgs,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(), ParseBatchCliError> {
    // 1. Composition config: closed validation BEFORE anything opens.
    let overrides = args.overrides()?;
    let config = resolve(&overrides, env)?;

    // 2. Manifest (paths read only here) → logical requests (bytes only).
    let entries = load_manifest(&args.manifest)?;
    let requests = build_requests(&entries)?;
    let items: Vec<ParseBatchItem> = requests
        .iter()
        .enumerate()
        .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request.clone()))
        .collect();
    let fingerprint = ordered_input_fingerprint(&items);

    // 3. Optional accelerator (opened before the backend; enabled means
    //    required — corruption/IO failure is a typed error, never silent).
    #[cfg(feature = "redb")]
    let accelerator: Option<Arc<da_adapters::parse_cursor::ParseCursorAccelerator>> =
        match config.redb().enabled() {
            true => {
                let path = config
                    .redb()
                    .path()
                    .expect("validated: enabled redb carries a path");
                Some(Arc::new(
                    da_adapters::parse_cursor::ParseCursorAccelerator::open(path)
                        .map_err(|error| ParseBatchCliError::RedbUnavailable(error.to_string()))?,
                ))
            }
            false => None,
        };
    #[cfg(not(feature = "redb"))]
    if config.redb().enabled() {
        return Err(ParseBatchCliError::RedbNotCompiled);
    }

    // 4. Resume cursor (accelerator lookup → canonical report fallback).
    #[cfg(feature = "redb")]
    let accelerator_lookup = match accelerator.as_ref() {
        Some(accel) => Some(
            accel
                .load_cursor(&fingerprint)
                .map_err(|error| error.to_string()),
        ),
        None => None,
    };
    #[cfg(not(feature = "redb"))]
    let accelerator_lookup: Option<Result<Option<u32>, String>> = None;
    let resume_cursor = if args.resume {
        Some(resolve_resume_cursor(
            &fingerprint,
            &args.report,
            accelerator_lookup,
        )?)
    } else {
        None
    };

    // 5. Admission (typed, fail-closed — before any backend open).
    let batch = ParseBatch::admit(
        config.cache_mode(),
        config.batch_limits(),
        requests,
        resume_cursor,
    )
    .map_err(|error| ParseBatchCliError::ManifestInvalid(admission_message(&error)))?;

    // 6. Index the (rebuildable) accelerator manifest after admission.
    #[cfg(feature = "redb")]
    if let Some(accel) = accelerator.as_ref() {
        let index_entries: Vec<(u32, String)> = batch
            .items()
            .iter()
            .map(|item| (item.ordinal(), item.paper_id().to_string()))
            .collect();
        accel
            .index_manifest(&fingerprint, &index_entries)
            .map_err(|error| ParseBatchCliError::RedbUnavailable(error.to_string()))?;
    }

    // 7. Wiring: D204-identity backend + atomic payload cache + executor.
    let backend = GrobidHttpBackend::new(Some(config.grobid_url()));
    let cache = FileParseArtifactCache::new(config.cache_root());
    let two_stage = da_parse::TwoStageParseCache::new(cache);
    let executor = ParseBatchExecutor::new(backend, two_stage);

    // 8. Durable checkpoint: accelerator mirror (feature-gated) or memory.
    #[cfg(feature = "redb")]
    let checkpoint: Arc<dyn da_parse::BatchCheckpoint> = match accelerator.as_ref() {
        Some(accel) => Arc::new(da_adapters::parse_cursor::RedbCursorCheckpoint::new(
            accel.clone(),
        )),
        None => Arc::new(ParseBatchMemoryCheckpoint::new()),
    };
    #[cfg(not(feature = "redb"))]
    let checkpoint: Arc<dyn da_parse::BatchCheckpoint> =
        Arc::new(ParseBatchMemoryCheckpoint::new());

    // 9. Bounded execution to one typed terminal outcome per document.
    let sink = Arc::new(ParseBatchCollector::new());
    let summary = executor.run(batch, sink.clone(), checkpoint).await;
    let documents = sink.outcomes_sorted().await;

    // 10. Atomic versioned terminal report (canonical durable evidence).
    let report = ParseBatchReportV1::build(&summary, &documents);
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| ParseBatchCliError::ReportWrite(error.to_string()))?;
    write_atomic(&args.report, &bytes)
        .map_err(|error| ParseBatchCliError::ReportWrite(error.to_string()))?;

    print_run_summary(&summary);
    Ok(())
}

/// Stable machine-readable stdout line (no paths, no URLs, no payloads).
fn print_run_summary(summary: &ParseBatchRunSummary) {
    println!(
        "parse-batch: mode={} admitted={} resumed_from={} skipped_on_resume={} completed={} succeeded={} failed={} backend_calls={} raw_hits={} raw_legacy_hits={} raw_writes={} projector_hits={} projector_writes={} max_in_flight_observed={} max_queue_depth_observed={} cursor_next={} input_fingerprint={} report_schema={}",
        summary.mode().as_str(),
        summary.admitted(),
        summary.resumed_from_ordinal(),
        summary.skipped_on_resume(),
        summary.completed(),
        summary.succeeded(),
        summary.failed(),
        summary.backend_calls(),
        summary.raw_hits(),
        summary.raw_legacy_hits(),
        summary.raw_writes(),
        summary.projector_hits(),
        summary.projector_writes(),
        summary.max_in_flight_observed(),
        summary.max_queue_depth_observed(),
        summary.cursor().next_ordinal(),
        summary.input_fingerprint(),
        PARSE_BATCH_REPORT_SCHEMA_VERSION,
    );
}

/// Closed admission-error text (no payload material; ids and bounds only).
fn admission_message(error: &da_parse::ParseBatchAdmissionError) -> String {
    error.to_string()
}
