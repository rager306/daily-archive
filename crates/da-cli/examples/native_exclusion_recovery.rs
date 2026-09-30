//! S04/T05 (M514-1lss2s) — native header recovery on the 79 typed baseline
//! exclusions.
//!
//! The S04 roadmap requires fatal exclusions to measurably improve, and the
//! sketch names exactly the 79 typed baseline exceptions as the target. Running
//! the native header band over those documents is what turns "improved" from a
//! claim into a measurement:
//!
//! - the population is read by pointer from
//!   `data/parse/holdout_manifests.yaml` (the byte pins) and
//!   `artifacts/parse_compat/external_holdout_release_probe.json#reference_population`
//!   (the typed exceptions), cross-checked against
//!   `artifacts/parse_compat/absorption_progress_baseline.json#/denominator`;
//! - the probe's own identity digest is carried as an identity leaf and is
//!   **never** compared to the PDF sha256: `identity_digest_is_byte_hash` is
//!   false, so the exception digest and the byte hash are different artifacts;
//! - each exception runs the real S03 native plane
//!   (`PdfInspectorLayoutDonor` -> `segment_document_full` ->
//!   `project_native_words` -> `assign_heading_regions` ->
//!   `build_native_word_region_carrier`) and then the T02 header band, under a
//!   per-document typed disposition;
//! - a structural mismatch (identity absent, non-`bytes_present` row, missing
//!   bytes, sha256 drift) is a **typed refusal** recorded in the artifact, and
//!   the process still exits non-zero: a refusal is never a quiet skip;
//! - the frozen affiliation reference (3055/7077) is carried verbatim, the 79
//!   stay the denominator, and `thresholds = not_accepted`.
//!
//! It is diagnostic-only: `product_coverage_effect = none`, no promotion, no
//! production splice, no graph write (D127 / D499). Title recovery is declared
//! `out_of_scope` for every row because the T02 header band has no Title atom
//! (its atoms are Date / PersonalName / Affiliation / AffiliationLink), and
//! inventing a title heuristic would be free invent (D099).
//!
//! A full run over all 79 documents is long, so it is meant to be launched
//! detached. `--smoke N` runs a bounded, reason-spread prefix and never writes
//! the repository artifact.

use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor};
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, SLICE_DECLARATION_DATA_REL, load_declaration, sha256_hex,
};
use da_cli::native_family_receipt::{
    AffiliationReference, CLAIM_SCOPE_DIAGNOSTIC_ONLY, EXCLUSION_BASELINE_REL, EXCLUSION_PROBE_REL,
    EXCLUSION_REASON_ORDER, EXCLUSION_RECOVERY_ARTIFACT_REL, ExclusionReason, ExclusionRecoveryRow,
    ExclusionRefusalReason, FieldDisposition, FieldRecovery, GuardProvenance, HOLDOUT_MANIFEST_REL,
    IntegrityFailure, PopulationId, PopulationKind, RecoveryDisposition, RecoveryField,
    RecoveryMode, RecoveryPopulation, RefusalReason, assert_redacted,
    build_exclusion_recovery_receipt, sha256_prefixed,
};
use da_parse::{
    FullSemanticRegion, GrobidHttpBackend, GrobidProfile, HeaderAtom, HeadingRegionConfig,
    JavaInvocationCounter, NativeHeaderFromInputOutcome, NativeHeaderInputError,
    SegmentationConfig, assign_heading_regions, build_native_header_input,
    build_native_word_region_carrier, project_native_words, segment_document_full,
};
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const USAGE: &str = "native_exclusion_recovery [--root DIR] [--report FILE] [--smoke N] [--workers N] [--timeout-secs N]\n\
    full run (default): all 79 typed exclusions -> artifacts/parse_compat/native_exclusion_recovery.json\n\
    --smoke N: bounded, reason-spread prefix; requires --report and never writes the repository artifact";

// ───────────────────────────── carriers ─────────────────────────────

/// The holdout manifest, as `data/parse/holdout_manifests.yaml` publishes it.
#[derive(Debug, Deserialize)]
struct ManifestDoc {
    articles: Vec<ManifestArticle>,
}

/// One pinned article byte pin.
#[derive(Debug, Clone, Deserialize)]
struct ManifestArticle {
    article_id: String,
    pdf_path: String,
    sha256: String,
    size_bytes: u64,
    status: String,
}

/// One typed probe exception.
#[derive(Debug, Clone, Deserialize)]
struct ProbeException {
    document_id: String,
    identity_sha256: String,
    reason: String,
    size_bytes: u64,
}

/// The probe's identity population, as the carrier declares it.
#[derive(Debug, Deserialize)]
struct ProbeReference {
    source_manifest_sha256: String,
    documents: u64,
    measurable_documents: u64,
    excluded_documents: u64,
    identity_digest_is_byte_hash: bool,
    exceptions: Vec<ProbeException>,
}

/// The baseline exclusion denominator.
#[derive(Debug, Deserialize)]
struct BaselineDenominator {
    assigned: u64,
    excluded: u64,
    exclusion_reasons: BTreeMap<String, u64>,
}

// ───────────────────────────── arguments ─────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    smoke: Option<usize>,
    workers: usize,
    timeout: Duration,
}

fn parse_args() -> Result<Args, String> {
    let mut root = PathBuf::from(".");
    let mut report: Option<PathBuf> = None;
    let mut smoke: Option<usize> = None;
    let mut workers = 4usize;
    let mut timeout_secs = 90u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = Some(args.next().unwrap_or_default().into()),
            "--smoke" => {
                let value = args.next().unwrap_or_default();
                smoke = Some(
                    value
                        .parse()
                        .map_err(|_| format!("--smoke expects an integer, got `{value}`"))?,
                );
            }
            "--workers" => {
                let value = args.next().unwrap_or_default();
                workers = value.parse().unwrap_or(4);
            }
            "--timeout-secs" => {
                let value = args.next().unwrap_or_default();
                timeout_secs = value.parse().unwrap_or(90);
            }
            "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let report = match (&smoke, report) {
        (Some(_), Some(path)) => {
            if path == Path::new(EXCLUSION_RECOVERY_ARTIFACT_REL) {
                return Err(
                    "a smoke run must not write the repository artifact; pass another --report"
                        .to_owned(),
                );
            }
            path
        }
        (Some(_), None) => {
            return Err(
                "a smoke run requires --report (it never writes the repository artifact)"
                    .to_owned(),
            );
        }
        (None, Some(path)) => path,
        (None, None) => PathBuf::from(EXCLUSION_RECOVERY_ARTIFACT_REL),
    };
    Ok(Args {
        root,
        report,
        smoke,
        workers: workers.clamp(1, 16),
        timeout: Duration::from_secs(timeout_secs.max(1)),
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("native_exclusion_recovery: {message}");
            return ExitCode::FAILURE;
        }
    };
    let _log_guard = da_cli::logging::init_pipeline_logging("native_exclusion_recovery");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_exclusion_recovery: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────────────── targets ─────────────────────────────

/// One selected exception, resolved to the closed reason vocabulary.
#[derive(Debug, Clone)]
struct TargetSpec {
    document_id: String,
    reason: ExclusionReason,
    probe_identity_sha256: String,
    probe_size_bytes: u64,
}

/// Pick the run's targets. A full run is the probe order; a smoke run is a
/// bounded, reason-spread prefix that exercises every typed reason.
fn select_targets(
    exceptions: &[ProbeException],
    smoke: Option<usize>,
) -> Result<Vec<usize>, Box<dyn Error>> {
    match smoke {
        None => Ok((0..exceptions.len()).collect()),
        Some(limit) => {
            let mut buckets: Vec<Vec<usize>> =
                EXCLUSION_REASON_ORDER.iter().map(|_| Vec::new()).collect();
            for (index, exception) in exceptions.iter().enumerate() {
                let reason = ExclusionReason::parse(&exception.reason).ok_or_else(|| {
                    format!(
                        "unknown exclusion reason `{}` for `{}`",
                        exception.reason, exception.document_id
                    )
                })?;
                let position = EXCLUSION_REASON_ORDER
                    .iter()
                    .position(|known| *known == reason)
                    .expect("the reason order covers the closed vocabulary");
                buckets[position].push(index);
            }
            let mut selected = Vec::new();
            let mut cursor = 0usize;
            'outer: loop {
                let mut progressed = false;
                for bucket in &buckets {
                    if selected.len() >= limit {
                        break 'outer;
                    }
                    if cursor < bucket.len() {
                        selected.push(bucket[cursor]);
                        progressed = true;
                    }
                }
                if !progressed {
                    break;
                }
                cursor += 1;
            }
            Ok(selected)
        }
    }
}

fn target_specs(exceptions: &[ProbeException], selected: &[usize]) -> Vec<TargetSpec> {
    selected
        .iter()
        .map(|index| {
            let exception = &exceptions[*index];
            TargetSpec {
                document_id: exception.document_id.clone(),
                reason: ExclusionReason::parse(&exception.reason)
                    .expect("selected targets carry closed reasons"),
                probe_identity_sha256: exception.identity_sha256.clone(),
                probe_size_bytes: exception.size_bytes,
            }
        })
        .collect()
}

// ───────────────────────────── native band ─────────────────────────────

/// Minimal project-owned lexicon (no GROBID lexicon bytes are read or
/// committed, D358).
fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["vaswani".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn header_refusal(error: &NativeHeaderInputError) -> RefusalReason {
    match error {
        NativeHeaderInputError::NoAdmittedTokens { .. } => RefusalReason::NoAdmittedTokens,
        NativeHeaderInputError::RegionNotRequested { .. } => RefusalReason::RegionNotRequested,
        NativeHeaderInputError::NoFeatureTokens => RefusalReason::NoFeatureTokens,
        NativeHeaderInputError::MissingFontSize { .. } => RefusalReason::MissingFontSize,
        NativeHeaderInputError::InvalidFontSize { .. } => RefusalReason::InvalidFontSize,
        NativeHeaderInputError::TokenContainsWhitespace { .. } => {
            RefusalReason::TokenContainsWhitespace
        }
        NativeHeaderInputError::FeatureRender(_) => RefusalReason::FeatureBuildRefused,
        NativeHeaderInputError::Serialization => RefusalReason::SerializationRefused,
    }
}

fn header_atom_count(outcome: &NativeHeaderFromInputOutcome, atom: HeaderAtom) -> u64 {
    outcome
        .atoms
        .iter()
        .find(|entry| entry.atom == atom)
        .map(|entry| entry.recognized_count as u64)
        .unwrap_or(0)
}

/// What the native header band did for one document.
enum BandOutcome {
    /// The band executed and reported the per-atom counts.
    Observed {
        date_count: u64,
        author_count: u64,
        pipeline_digest: String,
    },
    /// The Header region admitted no tokens: a typed absence, not a zero.
    TypedAbsence { pipeline_digest: String },
    /// A pipeline step or the band refused; the band never produced a value.
    Refused {
        reason: RefusalReason,
        pipeline_digest: Option<String>,
    },
}

/// Record a typed pipeline refusal without letting the payload reach the
/// artifact; the error text goes to the structured log only.
fn pipeline_refused(
    document_id: &str,
    step: &'static str,
    error: &dyn std::fmt::Display,
) -> BandOutcome {
    tracing::warn!(
        document = document_id,
        step,
        error = %error,
        "native exclusion recovery: pipeline step refused"
    );
    BandOutcome::Refused {
        reason: RefusalReason::PipelineRefused,
        pipeline_digest: None,
    }
}

/// Run the S03 plane and the T02 header band over one PDF's bytes.
fn measure_header(bytes: &[u8], document_id: &str) -> BandOutcome {
    let limits = LayoutDonorLimits::default();
    let observation = match PdfInspectorLayoutDonor.extract_shadow(bytes, document_id, limits) {
        Ok(observation) => observation,
        Err(error) => return pipeline_refused(document_id, "layout", &error),
    };
    let projection = match project_native_words(&observation.document, &observation.lexemes) {
        Ok(projection) => projection,
        Err(error) => return pipeline_refused(document_id, "projection", &error),
    };
    let segmentation = match segment_document_full(
        &observation.document,
        &observation.lattice,
        &observation.lexemes,
        SegmentationConfig::default(),
    ) {
        Ok(segmentation) => segmentation,
        Err(error) => return pipeline_refused(document_id, "segmentation", &error),
    };
    let heading = match assign_heading_regions(
        &observation.document,
        &observation.lattice,
        &projection,
        HeadingRegionConfig::default(),
    ) {
        Ok(heading) => heading,
        Err(error) => return pipeline_refused(document_id, "heading", &error),
    };
    let carrier = match build_native_word_region_carrier(
        &observation.document,
        &observation.lattice,
        &projection,
        &heading,
        &[FullSemanticRegion::Header],
    ) {
        Ok(carrier) => carrier,
        Err(error) => return pipeline_refused(document_id, "carrier", &error),
    };
    let pipeline_digest = match pipeline_digest(&projection, &segmentation, &heading, &carrier) {
        Ok(digest) => digest,
        Err(error) => return pipeline_refused(document_id, "digest", &error),
    };

    match build_native_header_input(&carrier, &lexicon()) {
        Ok(outcome) => BandOutcome::Observed {
            date_count: header_atom_count(&outcome, HeaderAtom::Date),
            author_count: header_atom_count(&outcome, HeaderAtom::PersonalName),
            pipeline_digest,
        },
        Err(NativeHeaderInputError::NoAdmittedTokens { .. }) => {
            BandOutcome::TypedAbsence { pipeline_digest }
        }
        Err(error) => {
            let reason = header_refusal(&error);
            tracing::warn!(
                document = document_id,
                band_refusal = reason.as_str(),
                "native exclusion recovery: header band refused"
            );
            BandOutcome::Refused {
                reason,
                pipeline_digest: Some(pipeline_digest),
            }
        }
    }
}

fn pipeline_digest(
    projection: &da_parse::NativeWordProjection,
    segmentation: &da_parse::FullSegmentationOutcome,
    heading: &da_parse::NativeHeadingRegionOutcome,
    carrier: &da_parse::NativeRegionCarrier,
) -> Result<String, Box<dyn Error>> {
    Ok(sha256_prefixed(
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            projection.digest()?,
            segmentation.order_digest,
            heading.digest()?,
            carrier.digest()?
        )
        .as_bytes(),
    ))
}

// ───────────────────────────── measurement ─────────────────────────────

/// The header fields a row carries. Title is always `out_of_scope`: the T02
/// band has no Title atom, so a title cannot be produced from it.
fn field_matrix(date: FieldDisposition, author: FieldDisposition) -> Vec<FieldRecovery> {
    vec![
        FieldRecovery {
            field: RecoveryField::Date,
            disposition: date,
        },
        FieldRecovery {
            field: RecoveryField::Title,
            disposition: FieldDisposition::OutOfScope,
        },
        FieldRecovery {
            field: RecoveryField::Author,
            disposition: author,
        },
    ]
}

fn disposition_for(reason: ExclusionReason, fields: &[FieldRecovery]) -> RecoveryDisposition {
    match fields
        .iter()
        .find(|entry| entry.field == reason.field())
        .map(|entry| entry.disposition)
    {
        Some(FieldDisposition::Recovered) => RecoveryDisposition::Recovered,
        Some(FieldDisposition::NotRecovered) => RecoveryDisposition::NotRecovered,
        Some(FieldDisposition::OutOfScope) => RecoveryDisposition::OutOfScope,
        None => RecoveryDisposition::NotRecovered,
    }
}

/// One document's measurement plus the hard failure it implies, if any.
struct Outcome {
    row: ExclusionRecoveryRow,
    failure: Option<IntegrityFailure>,
}

impl Outcome {
    fn structural(
        spec: &TargetSpec,
        manifest: Option<&ManifestArticle>,
        code: ExclusionRefusalReason,
    ) -> Self {
        Outcome {
            row: ExclusionRecoveryRow {
                document_id: spec.document_id.clone(),
                reason: spec.reason,
                probe_identity_sha256: spec.probe_identity_sha256.clone(),
                probe_size_bytes: spec.probe_size_bytes,
                source_pdf: manifest.map(|entry| entry.pdf_path.clone()),
                pdf_sha256: None,
                pdf_sha256_pinned: manifest.map(|entry| entry.sha256.clone()),
                pdf_size_bytes: manifest.map(|entry| entry.size_bytes),
                pipeline_digest: None,
                disposition: RecoveryDisposition::TypedRefusal,
                fields: field_matrix(
                    FieldDisposition::NotRecovered,
                    FieldDisposition::NotRecovered,
                ),
                band_typed_absence: false,
                band_refusal: None,
                refusal: Some(code),
            },
            failure: Some(IntegrityFailure {
                code,
                document: Some(spec.document_id.clone()),
            }),
        }
    }

    /// A document that exceeded its wall-clock budget: a typed refusal, never a
    /// pass, and never an integrity failure.
    fn timeout(spec: &TargetSpec, entry: Option<&ManifestArticle>) -> Self {
        Outcome {
            row: ExclusionRecoveryRow {
                document_id: spec.document_id.clone(),
                reason: spec.reason,
                probe_identity_sha256: spec.probe_identity_sha256.clone(),
                probe_size_bytes: spec.probe_size_bytes,
                source_pdf: entry.map(|entry| entry.pdf_path.clone()),
                pdf_sha256: None,
                pdf_sha256_pinned: entry.map(|entry| entry.sha256.clone()),
                pdf_size_bytes: entry.map(|entry| entry.size_bytes),
                pipeline_digest: None,
                disposition: RecoveryDisposition::TypedRefusal,
                fields: field_matrix(
                    FieldDisposition::NotRecovered,
                    FieldDisposition::NotRecovered,
                ),
                band_typed_absence: false,
                band_refusal: Some(RefusalReason::DocumentTimeout),
                refusal: None,
            },
            failure: None,
        }
    }
}

#[allow(clippy::too_many_lines)]
fn process_target(root: &Path, entry: Option<&ManifestArticle>, spec: &TargetSpec) -> Outcome {
    let Some(entry) = entry else {
        return Outcome::structural(spec, None, ExclusionRefusalReason::IdentityNotInManifest);
    };
    if entry.status != "bytes_present" {
        return Outcome::structural(
            spec,
            Some(entry),
            ExclusionRefusalReason::StatusNotBytesPresent,
        );
    }
    let Ok(bytes) = fs::read(root.join(&entry.pdf_path)) else {
        return Outcome::structural(spec, Some(entry), ExclusionRefusalReason::PdfMissing);
    };
    let observed_sha = sha256_hex(&bytes);
    let pinned_sha = entry
        .sha256
        .strip_prefix("sha256:")
        .unwrap_or(&entry.sha256);
    if observed_sha != pinned_sha {
        let mut outcome =
            Outcome::structural(spec, Some(entry), ExclusionRefusalReason::PdfShaDrift);
        outcome.row.pdf_sha256 = Some(observed_sha);
        outcome.row.pdf_size_bytes = Some(bytes.len() as u64);
        return outcome;
    }

    let (fields, disposition, band_typed_absence, band_refusal, pipeline_digest) =
        match measure_header(&bytes, &spec.document_id) {
            BandOutcome::Observed {
                date_count,
                author_count,
                pipeline_digest,
            } => {
                let date = if date_count > 0 {
                    FieldDisposition::Recovered
                } else {
                    FieldDisposition::NotRecovered
                };
                let author = if author_count > 0 {
                    FieldDisposition::Recovered
                } else {
                    FieldDisposition::NotRecovered
                };
                let fields = field_matrix(date, author);
                let disposition = disposition_for(spec.reason, &fields);
                (fields, disposition, false, None, Some(pipeline_digest))
            }
            BandOutcome::TypedAbsence { pipeline_digest } => {
                let fields = field_matrix(
                    FieldDisposition::NotRecovered,
                    FieldDisposition::NotRecovered,
                );
                let disposition = disposition_for(spec.reason, &fields);
                (fields, disposition, true, None, Some(pipeline_digest))
            }
            BandOutcome::Refused {
                reason,
                pipeline_digest,
            } => {
                let fields = field_matrix(
                    FieldDisposition::NotRecovered,
                    FieldDisposition::NotRecovered,
                );
                (
                    fields,
                    RecoveryDisposition::TypedRefusal,
                    false,
                    Some(reason),
                    pipeline_digest,
                )
            }
        };

    Outcome {
        row: ExclusionRecoveryRow {
            document_id: spec.document_id.clone(),
            reason: spec.reason,
            probe_identity_sha256: spec.probe_identity_sha256.clone(),
            probe_size_bytes: spec.probe_size_bytes,
            source_pdf: Some(entry.pdf_path.clone()),
            pdf_sha256: Some(observed_sha),
            pdf_sha256_pinned: Some(pinned_sha.to_owned()),
            pdf_size_bytes: Some(bytes.len() as u64),
            pipeline_digest,
            disposition,
            fields,
            band_typed_absence,
            band_refusal,
            refusal: None,
        },
        failure: None,
    }
}

/// Run the targets across `workers` threads with a per-document wall-clock
/// timeout, preserving target order. A timed-out document is a typed refusal
/// (never a pass) and is not an integrity failure, so it cannot make an
/// otherwise-honest run look clean or look broken.
fn process_targets(
    root: &Path,
    manifest_by_id: &BTreeMap<String, ManifestArticle>,
    specs: &[TargetSpec],
    workers: usize,
    timeout: Duration,
) -> Vec<Outcome> {
    let next = AtomicUsize::new(0);
    let collected: Mutex<Vec<(usize, Outcome)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= specs.len() {
                        break;
                    }
                    let spec = specs[index].clone();
                    let entry = manifest_by_id.get(&spec.document_id).cloned();
                    let document_root = root.to_path_buf();
                    let (sender, receiver) = std::sync::mpsc::channel();
                    let worker = std::thread::spawn(move || {
                        let _ = sender.send(process_target(&document_root, entry.as_ref(), &spec));
                    });
                    let outcome = match receiver.recv_timeout(timeout) {
                        Ok(outcome) => {
                            let _ = worker.join();
                            outcome
                        }
                        Err(_) => {
                            // The worker is deliberately not joined: it keeps
                            // running to completion in the background while this
                            // document is recorded as a typed refusal.
                            tracing::warn!(
                                document = %specs[index].document_id,
                                timeout_secs = timeout.as_secs(),
                                "native exclusion recovery: document timed out"
                            );
                            Outcome::timeout(
                                &specs[index],
                                manifest_by_id.get(&specs[index].document_id),
                            )
                        }
                    };
                    collected
                        .lock()
                        .expect("the collector mutex is not poisoned")
                        .push((index, outcome));
                }
            });
        }
    });
    let mut collected = collected
        .into_inner()
        .expect("the collector mutex is intact");
    collected.sort_by_key(|(index, _)| *index);
    collected.into_iter().map(|(_, outcome)| outcome).collect()
}

// ───────────────────────────── carrier helpers ─────────────────────────────

/// The frozen per-family capability reference the baseline publishes.
fn affiliation_reference(
    baseline: &serde_json::Value,
) -> Result<AffiliationReference, Box<dyn Error>> {
    let measurements = baseline
        .get("measurements")
        .and_then(|value| value.as_array())
        .ok_or("the baseline carries no measurements")?;
    for measurement in measurements {
        let per_family = measurement
            .get("strata")
            .and_then(|strata| strata.get("per_family"));
        let recovered = per_family
            .and_then(|family| family.get("numerator"))
            .and_then(|value| value.as_u64());
        let denominator = per_family
            .and_then(|family| family.get("denominator"))
            .and_then(|value| value.as_u64());
        if let (Some(recovered), Some(denominator)) = (recovered, denominator) {
            let metric_id = measurement
                .get("metric_id")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if !metric_id.is_empty() && denominator > 0 {
                return Ok(AffiliationReference {
                    metric_id: metric_id.to_owned(),
                    recovered,
                    denominator,
                });
            }
        }
    }
    Err("the baseline carries no per-family capability reference".into())
}

// ───────────────────────────── run ─────────────────────────────

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    // The slice contract: locks, claim scope and coverage effect come from the
    // published declaration, never from a restatement in this runner.
    let layout_bytes = fs::read(args.root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(args.root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;
    if declaration.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(format!(
            "declaration claim_scope is `{}`, expected `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`",
            declaration.claim_scope
        )
        .into());
    }

    // Carriers.
    let manifest_bytes = fs::read(args.root.join(HOLDOUT_MANIFEST_REL))?;
    let manifest_revision = sha256_prefixed(&manifest_bytes);
    let manifest: ManifestDoc = serde_yaml::from_slice(&manifest_bytes)?;
    let manifest_count = manifest.articles.len() as u64;
    let manifest_by_id: BTreeMap<String, ManifestArticle> = manifest
        .articles
        .into_iter()
        .map(|article| (article.article_id.clone(), article))
        .collect();

    let probe_bytes = fs::read(args.root.join(EXCLUSION_PROBE_REL))?;
    let probe_revision = sha256_prefixed(&probe_bytes);
    let probe_value: serde_json::Value = serde_json::from_slice(&probe_bytes)?;
    let reference: ProbeReference = serde_json::from_value(
        probe_value
            .get("reference_population")
            .ok_or("the probe carries no reference_population")?
            .clone(),
    )?;

    let baseline_bytes = fs::read(args.root.join(EXCLUSION_BASELINE_REL))?;
    let baseline_revision = sha256_prefixed(&baseline_bytes);
    let baseline_value: serde_json::Value = serde_json::from_slice(&baseline_bytes)?;
    let denominator: BaselineDenominator = serde_json::from_value(
        baseline_value
            .get("denominator")
            .ok_or("the baseline carries no denominator")?
            .clone(),
    )?;
    let affiliation = affiliation_reference(&baseline_value)?;

    // Cross-checks between the three carriers. Each is recorded as a typed hard
    // failure rather than a quiet mismatch.
    let mut failures: Vec<IntegrityFailure> = Vec::new();
    if reference.source_manifest_sha256 != manifest_revision {
        failures.push(IntegrityFailure {
            code: ExclusionRefusalReason::ProbeRevisionMismatch,
            document: None,
        });
    }
    if reference.identity_digest_is_byte_hash {
        failures.push(IntegrityFailure {
            code: ExclusionRefusalReason::ProbeRevisionMismatch,
            document: None,
        });
    }
    if reference.documents != manifest_count {
        failures.push(IntegrityFailure {
            code: ExclusionRefusalReason::BaselineReasonMismatch,
            document: None,
        });
    }
    if reference.excluded_documents != denominator.excluded {
        failures.push(IntegrityFailure {
            code: ExclusionRefusalReason::BaselineReasonMismatch,
            document: None,
        });
    }
    let mut probe_counts: BTreeMap<String, u64> = BTreeMap::new();
    for exception in &reference.exceptions {
        *probe_counts.entry(exception.reason.clone()).or_default() += 1;
    }
    for (reason, expected) in &denominator.exclusion_reasons {
        match probe_counts.get(reason) {
            Some(found) if found == expected => {}
            Some(_) => failures.push(IntegrityFailure {
                code: ExclusionRefusalReason::BaselineReasonMismatch,
                document: None,
            }),
            None => failures.push(IntegrityFailure {
                code: ExclusionRefusalReason::BaselineReasonAbsent,
                document: None,
            }),
        }
    }

    let selected = select_targets(&reference.exceptions, args.smoke)?;
    let specs = target_specs(&reference.exceptions, &selected);

    // The guard on the real Java seam. Nothing below consults Java.
    let counter = JavaInvocationCounter::new();
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());
    let identity_isolated = plain.identity() == watched.identity();
    let installed = watched.java_invocation_observation().is_observed();
    let observed_java_calls = counter
        .observation()
        .attempts()
        .ok_or("an installed observer must read as observed")?;
    let guard = GuardProvenance {
        installed,
        identity_isolated,
        observed_java_calls,
    };

    let outcomes = process_targets(
        &args.root,
        &manifest_by_id,
        &specs,
        args.workers,
        args.timeout,
    );
    let mut notes: Vec<String> = Vec::with_capacity(outcomes.len());
    for outcome in &outcomes {
        if let Some(failure) = &outcome.failure {
            failures.push(failure.clone());
        }
        notes.push(format!(
            "{}: reason={} disposition={} date={} title={} author={}",
            outcome.row.document_id,
            outcome.row.reason.as_str(),
            outcome.row.disposition.as_str(),
            outcome.row.fields[0].disposition.as_str(),
            outcome.row.fields[1].disposition.as_str(),
            outcome.row.fields[2].disposition.as_str(),
        ));
    }
    let rows: Vec<ExclusionRecoveryRow> = outcomes.into_iter().map(|outcome| outcome.row).collect();

    let reasons: BTreeMap<String, u64> = denominator
        .exclusion_reasons
        .iter()
        .map(|(reason, count)| (reason.clone(), *count))
        .collect();
    let population = RecoveryPopulation {
        population_id: PopulationId::ExclusionBaseline,
        population_kind: PopulationKind::Exclusion,
        mode: if args.smoke.is_some() {
            RecoveryMode::Smoke
        } else {
            RecoveryMode::Full
        },
        assumed_complete: args.smoke.is_none(),
        assigned: denominator.assigned,
        excluded: denominator.excluded,
        measurable: reference.measurable_documents,
        targeted: rows.len() as u64,
        reasons,
    };

    let receipt = build_exclusion_recovery_receipt(
        &manifest_revision,
        &probe_revision,
        &baseline_revision,
        &declaration.locks,
        &declaration.claim_scope,
        &declaration.product_coverage_effect,
        guard,
        &population,
        &affiliation,
        &rows,
        &failures,
    )?;
    let artifact = receipt.artifact_json_bytes()?;
    assert_redacted(std::str::from_utf8(&artifact)?)?;

    if let Some(parent) = args
        .report
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.report, &artifact)?;

    let summary = format!(
        "native exclusion recovery: mode={} targeted={} covered={} excluded={} guard installed={} identity_isolated={} java_calls={} affiliation_reference={}/{} thresholds={} integrity_failures={}; {}",
        receipt.population.mode.as_str(),
        receipt.population.targeted,
        receipt.recoveries.len(),
        receipt.population.excluded,
        receipt.guard.installed,
        receipt.guard.identity_isolated,
        receipt.guard.observed_java_calls,
        receipt.affiliation_reference.recovered,
        receipt.affiliation_reference.denominator,
        receipt.thresholds,
        receipt.integrity_failures.len(),
        notes.join(" | ")
    );

    if !failures.is_empty() {
        return Err(format!(
            "{} integrity failure(s) recorded in {}: {}",
            failures.len(),
            args.report.display(),
            failures
                .iter()
                .map(|failure| failure.code.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
        .into());
    }
    Ok(summary)
}
