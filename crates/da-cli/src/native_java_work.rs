//! S08/T03 — measured Java work derived from a verified GROBID stage tape.
//!
//! The slice demo needs a *measured* reduction of Java work and H13 names Java
//! invocations. The only unit that can be measured offline without inventing
//! numbers is the number of Java CRF model invocations: a `debugMode=1` dump
//! carries one section per model invocation, and [`da_parse::StageTape`] already
//! derives per-model coverage from it. This module is the typed projection of
//! that tape into a redacted receipt and report.
//!
//! Four properties keep the measurement honest:
//!
//! 1. **Absence is typed, never zero.** A missing or unreadable tape produces
//!    [`JavaWorkMeasurement::NotObserved`], which structurally carries no counts
//!    at all: there is no field from which a zero could be misread as a
//!    measurement.
//! 2. **The receipt is bound to the tape identity.** Schema version, endpoint,
//!    `source_sha256`, `dump_sha256` and `backend_version` enter a digest
//!    ([`MeasuredJavaWork::tape_identity`]), so a later consumer cannot silently
//!    mix counts captured from a different input or release.
//! 3. **A filtered tape can never claim full coverage.** The plan keys the claim
//!    off `models_filter` itself, not off `capture_state` alone; asking a
//!    filtered receipt for a full-coverage measurement is a typed refusal.
//! 4. **Nothing dump-derived leaks.** The raw dump is never read, written or
//!    embedded here. Model names are the only dump-derived strings that reach
//!    the artifact, and they are restricted to a conservative charset, so a
//!    crafted dump header cannot smuggle token text or a physical path into a
//!    committable artifact.
//!
//! Java CPU wall-clock per stage and RSS ceilings stay typed `not_observed`:
//! H05 is not closed by this slice and no byte/CPU ceiling is claimed.

use da_parse::{CaptureState, STAGE_TAPE_SCHEMA_VERSION, StageTape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

pub const NATIVE_JAVA_WORK_SCHEMA_VERSION: &str = "native-java-work.v1";
pub const NATIVE_JAVA_WORK_ADR_REF: &str = "ADR-075";
/// The unit of Java work this slice measures.
pub const JAVA_WORK_UNIT: &str = "java_model_invocations";
pub const JAVA_WORK_ARTIFACT_REL: &str = "artifacts/parse_compat/native_java_work.json";
pub const JAVA_WORK_H05_CLOSURE: &str = "not_claimed";
pub const JAVA_WORK_H05_REASON: &str = "per-stage Java CPU wall-clock and RSS ceilings stay typed not_observed; only Java model invocations are measured";
/// Redaction vocabulary, mirroring the closed project-wide list. The unit test
/// in this module cross-checks it against
/// `data/parse/comparison_contract.yaml#native_core_qualification.redaction`,
/// so the two surfaces cannot drift apart silently.
pub const JAVA_WORK_REDACTION_FORBIDDEN: [&str; 9] = [
    "raw_pdf",
    "raw_text",
    "raw_tei",
    "token_text",
    "model_bytes",
    "lexicon_bytes",
    "physical_path",
    "hostname",
    "executable",
];
/// Exact capture command hint carried by the artifact. No physical path and no
/// service URL: the tape directory is a placeholder the operator fills in.
pub const JAVA_WORK_CAPTURE_HINT: &str = "cargo run -p da-cli --example grobid_stage_tape -- --pdf <source_pdf> --out <tape_dir> --endpoint fulltext && cargo run -p da-cli --example grobid_stage_tape -- --verify <tape_dir>/tape.json";
/// Closed set of endpoints a tape may have been recorded against.
pub const JAVA_WORK_ENDPOINTS: [&str; 3] = ["fulltext", "header", "references"];
/// Longest accepted model name. Real GROBID model names are short tokens.
pub const JAVA_WORK_MAX_MODEL_NAME_LEN: usize = 64;
/// Longest accepted identity token (paper id, document id).
pub const JAVA_WORK_MAX_IDENTITY_LEN: usize = 128;

// ───────────────────────────── errors ─────────────────────────────

/// Typed reasons a tape cannot become measured Java-work evidence.
///
/// Every variant is a refusal, never a silent downgrade: the caller either has
/// a measured receipt or a reason why it does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaWorkError {
    /// The corpus manifest text could not be read or parsed as YAML.
    ManifestUnreadable { reason: String },
    /// The `corpus` block is present but violates its contract.
    ManifestContract { path: String, reason: String },
    /// The tape bytes are not readable JSON for [`StageTape`].
    TapeUnreadable { reason: String },
    /// The tape was recorded against a schema this reader does not know.
    UnsupportedTapeSchema { found: String },
    /// The tape names an endpoint outside the closed set.
    UnsupportedTapeEndpoint { found: String },
    /// The recorded source path is absolute, traversing, or empty.
    UnsafeSourcePath { path_class: &'static str },
    /// A digest or identity field is not a well-formed value.
    MalformedIdentity { field: &'static str },
    /// The tape recorded no dump digest, so nothing binds its content.
    DumpDigestAbsent,
    /// A capture that failed: absence of measurement, stated with its cause.
    CaptureFailed { code: String, phase: String },
    /// A capture that returned 200 with no bytes.
    CaptureStateRefused { state: &'static str },
    /// Content was captured but carried no model section.
    NoSections,
    /// Per-model counts do not add up to the recorded section count.
    CoverageInconsistent { sections: usize, counted: usize },
    /// The declared distinct-model count disagrees with the derived one.
    DistinctMismatch { declared: usize, derived: usize },
    /// A model name is not a conservative token (charset or length).
    UnsafeModelName,
    /// A filtered tape was asked for a full-coverage measurement.
    FilteredFullCoverage,
    /// A receipt that carries no measurement was asked for one.
    NotMeasured,
    /// The verification record named a different tape than the one ingested.
    VerificationBindingMismatch { field: &'static str },
    /// Verification ran and reported findings: the tape is not usable.
    VerificationFindings { findings: usize },
    /// The corpus manifest listed no document.
    EmptyCorpus,
    /// A tape matched a pinned document but its source digest differs.
    CorpusDigestMismatch { observed: Option<String> },
}

impl JavaWorkError {
    /// Stable machine-readable code, used in artifacts and assertions.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ManifestUnreadable { .. } => "manifest_unreadable",
            Self::ManifestContract { .. } => "manifest_contract",
            Self::TapeUnreadable { .. } => "tape_unreadable",
            Self::UnsupportedTapeSchema { .. } => "unsupported_tape_schema",
            Self::UnsupportedTapeEndpoint { .. } => "unsupported_tape_endpoint",
            Self::UnsafeSourcePath { .. } => "unsafe_source_path",
            Self::MalformedIdentity { .. } => "malformed_identity",
            Self::DumpDigestAbsent => "dump_digest_absent",
            Self::CaptureFailed { .. } => "capture_failed",
            Self::CaptureStateRefused { .. } => "capture_state_refused",
            Self::NoSections => "no_sections",
            Self::CoverageInconsistent { .. } => "coverage_inconsistent",
            Self::DistinctMismatch { .. } => "distinct_mismatch",
            Self::UnsafeModelName => "unsafe_model_name",
            Self::FilteredFullCoverage => "filtered_full_coverage",
            Self::NotMeasured => "not_measured",
            Self::VerificationBindingMismatch { .. } => "verification_binding_mismatch",
            Self::VerificationFindings { .. } => "verification_findings",
            Self::EmptyCorpus => "empty_corpus",
            Self::CorpusDigestMismatch { .. } => "corpus_digest_mismatch",
        }
    }
}

/// A redacted refusal record: only values that are safe to commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
}

impl JavaWorkError {
    /// Project the error into a redacted record.
    ///
    /// Only closed-vocabulary names, field labels and well-formed digests are
    /// echoed; free-form reasons (which could quote a payload or a physical
    /// path) stay in [`fmt::Display`] for logs and never reach an artifact.
    pub fn refusal(&self) -> Refusal {
        let (field, observed) = match self {
            Self::ManifestContract { path, .. } => (Some(path.clone()), None),
            Self::UnsupportedTapeSchema { found } => {
                (Some("schema_version".to_string()), safe_token(found))
            }
            Self::UnsupportedTapeEndpoint { found } => {
                (Some("endpoint".to_string()), safe_token(found))
            }
            Self::UnsafeSourcePath { path_class } => (
                Some((*path_class).to_string()),
                Some("source_pdf".to_string()),
            ),
            Self::MalformedIdentity { field } => (Some((*field).to_string()), None),
            // A recorded failure code is tape-supplied text: echo it only when
            // it is a closed-vocabulary token, never when it could carry a path.
            Self::CaptureFailed { code, phase } => match (safe_token(code), safe_token(phase)) {
                (Some(code), Some(phase)) => (Some(format!("{code}/{phase}")), None),
                _ => (Some("capture_failure".to_string()), None),
            },
            Self::CaptureStateRefused { state } => (
                Some((*state).to_string()),
                Some("capture_state".to_string()),
            ),
            Self::CoverageInconsistent { sections, counted } => (
                Some("coverage".to_string()),
                Some(format!("sections={sections};counted={counted}")),
            ),
            Self::DistinctMismatch { declared, derived } => (
                Some("distinct_models".to_string()),
                Some(format!("declared={declared};derived={derived}")),
            ),
            Self::VerificationBindingMismatch { field } => (Some((*field).to_string()), None),
            Self::VerificationFindings { findings } => (
                Some("verification".to_string()),
                Some(format!("findings={findings}")),
            ),
            Self::CorpusDigestMismatch { observed } => (
                Some("source_sha256".to_string()),
                observed.clone().filter(|value| is_sha256_hex(value)),
            ),
            _ => (None, None),
        };
        Refusal {
            code: self.code().to_string(),
            field,
            observed,
        }
    }
}

impl fmt::Display for JavaWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestUnreadable { reason } => {
                write!(f, "java work corpus manifest unreadable: {reason}")
            }
            Self::ManifestContract { path, reason } => {
                write!(f, "java work corpus contract violation at {path}: {reason}")
            }
            Self::TapeUnreadable { reason } => {
                write!(f, "stage tape is not readable: {reason}")
            }
            Self::UnsupportedTapeSchema { found } => write!(
                f,
                "stage tape schema `{found}` is not `{STAGE_TAPE_SCHEMA_VERSION}`"
            ),
            Self::UnsupportedTapeEndpoint { found } => write!(
                f,
                "stage tape endpoint `{found}` is outside {:?}",
                JAVA_WORK_ENDPOINTS
            ),
            Self::UnsafeSourcePath { path_class } => write!(
                f,
                "recorded source path is {path_class}; a receipt may only carry a repo-relative path"
            ),
            Self::MalformedIdentity { field } => {
                write!(f, "recorded identity field `{field}` is malformed")
            }
            Self::DumpDigestAbsent => write!(
                f,
                "stage tape records no dump digest, so nothing binds its content"
            ),
            Self::CaptureFailed { code, phase } => {
                write!(f, "recorded capture failed ({code} at {phase})")
            }
            Self::CaptureStateRefused { state } => write!(
                f,
                "capture state `{state}` carries no measurement; absence is not zero"
            ),
            Self::NoSections => write!(
                f,
                "stage tape carries content but no model section: no Java work was observed"
            ),
            Self::CoverageInconsistent { sections, counted } => write!(
                f,
                "per-model counts ({counted}) do not add up to the recorded sections ({sections})"
            ),
            Self::DistinctMismatch { declared, derived } => write!(
                f,
                "declared distinct models ({declared}) disagree with the derived count ({derived})"
            ),
            Self::UnsafeModelName => write!(
                f,
                "a model name is not a conservative token; refusing to embed it"
            ),
            Self::FilteredFullCoverage => write!(
                f,
                "a filtered tape cannot support a full-coverage Java-work claim"
            ),
            Self::NotMeasured => write!(
                f,
                "a receipt that carries no measurement cannot answer a full-coverage claim"
            ),
            Self::VerificationBindingMismatch { field } => write!(
                f,
                "the verification record does not bind this tape at `{field}`"
            ),
            Self::VerificationFindings { findings } => write!(
                f,
                "tape verification reported {findings} finding(s); an unverifiable tape is not evidence"
            ),
            Self::EmptyCorpus => write!(f, "the corpus manifest lists no document to measure"),
            Self::CorpusDigestMismatch { .. } => write!(
                f,
                "the matched tape was recorded against a different source digest than the pinned corpus document"
            ),
        }
    }
}

impl std::error::Error for JavaWorkError {}

// ─────────────────────── identity helpers ───────────────────────

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A conservative identity token: short, and made of characters that cannot
/// carry a path, a token of text, or a control character.
fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= JAVA_WORK_MAX_IDENTITY_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn is_safe_model_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= JAVA_WORK_MAX_MODEL_NAME_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn safe_token(value: &str) -> Option<String> {
    is_safe_token(value).then(|| value.to_string())
}

/// Classify why a recorded path cannot be a repo-relative artifact path.
fn unsafe_path_class(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return Some("empty");
    }
    if value.starts_with('/') || value.starts_with('\\') {
        return Some("absolute");
    }
    if value.contains('\\') {
        return Some("non_unix");
    }
    if value.contains('\0') {
        return Some("control_character");
    }
    let traversing = value
        .split('/')
        .any(|component| component == ".." || component.is_empty());
    if traversing {
        return Some("traversing");
    }
    if value.contains(':') {
        return Some("scheme_or_drive");
    }
    None
}

/// Strip a recorder-added occurrence suffix and validate the base name.
///
/// `StageTape` already groups by base name through
/// `da_parse::model_invocation_counts`, so this is a defensive normalisation
/// for hand-written tapes: `date (occurrence 2 of 2)` counts as `date`, but an
/// invented or unsafe name is refused rather than echoed.
fn normalize_model_name(raw: &str) -> Result<String, JavaWorkError> {
    let base = match raw.split_once(" (occurrence ") {
        Some((base, tail)) => {
            if !tail.ends_with(')') {
                return Err(JavaWorkError::UnsafeModelName);
            }
            base.trim()
        }
        None => raw.trim(),
    };
    if !is_safe_model_name(base) {
        return Err(JavaWorkError::UnsafeModelName);
    }
    Ok(base.to_string())
}

fn tape_identity_digest(
    schema_version: &str,
    endpoint: &str,
    source_sha256: &str,
    dump_sha256: &str,
    backend_version: &str,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        schema_version,
        endpoint,
        source_sha256,
        dump_sha256,
        backend_version,
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

// ────────────────── typed absence and closed vocabularies ──────────────────

/// Typed absence without a number, so no consumer can read a zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedAbsence {
    pub state: AbsenceState,
}

impl TypedAbsence {
    pub const fn not_observed() -> Self {
        Self {
            state: AbsenceState::NotObserved,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsenceState {
    NotObserved,
}

/// Closed measurement vocabulary of a Java-work receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementState {
    /// A full dump: every model section the cascade produced is counted.
    Measured,
    /// A `models`-filtered dump: counted, but a subset by construction.
    Filtered,
    /// No usable measurement; the receipt carries no counts.
    NotObserved,
}

/// Closed coverage vocabulary of a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CoverageClaim {
    Full,
    Partial { reason: PartialReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialReason {
    /// The dump was filtered by `models`, so absent models prove nothing.
    FilteredTape,
    /// Not every pinned corpus document produced a measurement.
    IncompleteCorpus,
}

/// Whether a *verification run* established this tape, not merely that it parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapeVerificationState {
    Verified,
    NotObserved,
}

/// Why a document (or the aggregate) has no measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotObservedReason {
    /// No tape was supplied for this run.
    TapeNotProvided,
    /// A tape was supplied but could not be read or parsed.
    TapeUnreadable,
    /// Tapes were supplied, but none of them belongs to the pinned corpus.
    NoMatchingTape,
    /// No document in the corpus produced a usable measurement.
    NoMeasuredDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInvocationCount {
    pub model: String,
    pub invocations: usize,
}

// ───────────────────────── tape verification record ─────────────────────────

/// Facts produced by an actual verification run over one tape.
///
/// The reader cannot know from the tape alone that anyone re-measured it, so
/// verification is a separate, explicit input. Binding every field that makes
/// the tape an identity (`schema_version`, `dump_sha256`, `backend_version`)
/// means a verification of tape A can never be presented as verification of
/// tape B, and a run with findings refuses rather than downgrades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapeVerification {
    pub tape_schema_version: String,
    pub dump_sha256: String,
    pub backend_version: String,
    pub findings: usize,
}

impl TapeVerification {
    pub fn bind(&self, tape: &StageTape) -> Result<(), JavaWorkError> {
        if self.findings != 0 {
            return Err(JavaWorkError::VerificationFindings {
                findings: self.findings,
            });
        }
        if self.tape_schema_version != tape.schema_version
            || self.tape_schema_version != STAGE_TAPE_SCHEMA_VERSION
        {
            return Err(JavaWorkError::VerificationBindingMismatch {
                field: "schema_version",
            });
        }
        if !is_sha256_hex(&self.dump_sha256)
            || tape.dump_sha256.as_deref() != Some(self.dump_sha256.as_str())
        {
            return Err(JavaWorkError::VerificationBindingMismatch {
                field: "dump_sha256",
            });
        }
        if self.backend_version.is_empty() || self.backend_version != tape.backend_version {
            return Err(JavaWorkError::VerificationBindingMismatch {
                field: "backend_version",
            });
        }
        Ok(())
    }
}

// ──────────────────────── the measured payload ────────────────────────

/// Counts, bound to the exact tape they were derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredJavaWork {
    pub source_pdf: String,
    pub source_sha256: String,
    pub backend_version: String,
    pub endpoint: String,
    pub capture_state: String,
    pub dump_bytes: usize,
    pub dump_sha256: String,
    pub sections: usize,
    pub distinct_models: usize,
    pub models: Vec<ModelInvocationCount>,
    pub total_invocations: usize,
    pub coverage: CoverageClaim,
    pub tape_verification: TapeVerificationState,
    pub tape_identity: String,
}

/// A receipt is either measured/filtered (with counts) or typed absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "measurement_state", rename_all = "snake_case")]
pub enum JavaWorkMeasurement {
    Measured(MeasuredJavaWork),
    Filtered(MeasuredJavaWork),
    NotObserved { reason: NotObservedReason },
}

/// Per-tape receipt: the unit a route ledger consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeJavaWorkReceipt {
    pub schema_version: String,
    #[serde(flatten)]
    pub measurement: JavaWorkMeasurement,
}

impl NativeJavaWorkReceipt {
    pub fn not_observed(reason: NotObservedReason) -> Self {
        Self {
            schema_version: NATIVE_JAVA_WORK_SCHEMA_VERSION.to_string(),
            measurement: JavaWorkMeasurement::NotObserved { reason },
        }
    }

    pub fn state(&self) -> MeasurementState {
        match self.measurement {
            JavaWorkMeasurement::Measured(_) => MeasurementState::Measured,
            JavaWorkMeasurement::Filtered(_) => MeasurementState::Filtered,
            JavaWorkMeasurement::NotObserved { .. } => MeasurementState::NotObserved,
        }
    }

    pub fn measured(&self) -> Option<&MeasuredJavaWork> {
        match &self.measurement {
            JavaWorkMeasurement::Measured(counts) | JavaWorkMeasurement::Filtered(counts) => {
                Some(counts)
            }
            JavaWorkMeasurement::NotObserved { .. } => None,
        }
    }

    /// Refuse a full-coverage claim over a filtered or absent measurement.
    pub fn require_full_coverage(&self) -> Result<&MeasuredJavaWork, JavaWorkError> {
        match &self.measurement {
            JavaWorkMeasurement::Measured(counts) if counts.coverage == CoverageClaim::Full => {
                Ok(counts)
            }
            JavaWorkMeasurement::Measured(_) | JavaWorkMeasurement::Filtered(_) => {
                Err(JavaWorkError::FilteredFullCoverage)
            }
            JavaWorkMeasurement::NotObserved { .. } => Err(JavaWorkError::NotMeasured),
        }
    }

    /// Ingest a tape that has only been parsed, not re-verified.
    pub fn from_tape(tape: &StageTape) -> Result<Self, JavaWorkError> {
        Self::build(tape, TapeVerificationState::NotObserved)
    }

    /// Ingest a tape that a verification run re-measured from disk and the
    /// live release. Bindings are checked before any count is accepted.
    pub fn from_tape_verified(
        tape: &StageTape,
        verification: &TapeVerification,
    ) -> Result<Self, JavaWorkError> {
        verification.bind(tape)?;
        Self::build(tape, TapeVerificationState::Verified)
    }

    fn build(tape: &StageTape, verification: TapeVerificationState) -> Result<Self, JavaWorkError> {
        validate_tape_identity(tape)?;
        let dump_sha256 = tape
            .dump_sha256
            .clone()
            .ok_or(JavaWorkError::DumpDigestAbsent)?;

        // Group by base name: a hand-written tape may still carry recorder
        // occurrence suffixes, and merging them is what makes the count a count
        // of invocations rather than of header lines.
        let mut merged: BTreeMap<String, usize> = BTreeMap::new();
        for entry in &tape.models_observed {
            let model = normalize_model_name(&entry.model)?;
            *merged.entry(model).or_default() += entry.invocations;
        }
        let counted: usize = merged.values().sum();
        if counted != tape.sections {
            return Err(JavaWorkError::CoverageInconsistent {
                sections: tape.sections,
                counted,
            });
        }
        if merged.len() != tape.distinct_models {
            return Err(JavaWorkError::DistinctMismatch {
                declared: tape.distinct_models,
                derived: merged.len(),
            });
        }

        // The plan keys the coverage claim off `models_filter` itself: a
        // filtered dump omits models by construction, so absence there proves
        // nothing, whatever `capture_state` says.
        let filtered = tape.models_filter.is_some() || tape.capture_state == CaptureState::Filtered;
        let (measurement, coverage) = if filtered {
            (
                true,
                CoverageClaim::Partial {
                    reason: PartialReason::FilteredTape,
                },
            )
        } else {
            (false, CoverageClaim::Full)
        };

        let models = merged
            .into_iter()
            .map(|(model, invocations)| ModelInvocationCount { model, invocations })
            .collect::<Vec<_>>();
        let counts = MeasuredJavaWork {
            source_pdf: tape.source_pdf.clone(),
            source_sha256: tape.source_sha256.clone(),
            backend_version: tape.backend_version.clone(),
            endpoint: tape.endpoint.clone(),
            capture_state: tape.capture_state.as_str().to_string(),
            dump_bytes: tape.dump_bytes,
            dump_sha256: dump_sha256.clone(),
            sections: tape.sections,
            distinct_models: models.len(),
            models,
            total_invocations: counted,
            coverage,
            tape_verification: verification,
            tape_identity: tape_identity_digest(
                &tape.schema_version,
                &tape.endpoint,
                &tape.source_sha256,
                &dump_sha256,
                &tape.backend_version,
            ),
        };
        let measurement = if measurement {
            JavaWorkMeasurement::Filtered(counts)
        } else {
            JavaWorkMeasurement::Measured(counts)
        };
        Ok(Self {
            schema_version: NATIVE_JAVA_WORK_SCHEMA_VERSION.to_string(),
            measurement,
        })
    }
}

/// Structural checks that make a tape safe to ingest or to verify, performed
/// before any filesystem access.
///
/// Shared by ingestion and by the in-process verifier so that both refuse the
/// same tapes: a verifier must never be handed a path the reader would reject.
pub fn validate_tape_identity(tape: &StageTape) -> Result<(), JavaWorkError> {
    if tape.schema_version != STAGE_TAPE_SCHEMA_VERSION {
        return Err(JavaWorkError::UnsupportedTapeSchema {
            found: tape.schema_version.clone(),
        });
    }
    if !JAVA_WORK_ENDPOINTS.contains(&tape.endpoint.as_str()) {
        return Err(JavaWorkError::UnsupportedTapeEndpoint {
            found: tape.endpoint.clone(),
        });
    }
    match tape.capture_state {
        CaptureState::Failed => {
            return Err(JavaWorkError::CaptureFailed {
                code: tape
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                phase: tape
                    .failure_phase
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            });
        }
        CaptureState::UncapturedEmpty => {
            return Err(JavaWorkError::CaptureStateRefused {
                state: "uncaptured_empty",
            });
        }
        CaptureState::Captured | CaptureState::Filtered => {}
    }
    if tape.sections == 0 {
        return Err(JavaWorkError::NoSections);
    }
    if let Some(class) = unsafe_path_class(&tape.source_pdf) {
        return Err(JavaWorkError::UnsafeSourcePath { path_class: class });
    }
    if !is_sha256_hex(&tape.source_sha256) {
        return Err(JavaWorkError::MalformedIdentity {
            field: "source_sha256",
        });
    }
    match tape.dump_sha256.as_deref() {
        Some(value) if is_sha256_hex(value) => Ok(()),
        Some(_) => Err(JavaWorkError::MalformedIdentity {
            field: "dump_sha256",
        }),
        None => Err(JavaWorkError::DumpDigestAbsent),
    }
}

/// Parse tape JSON without touching the filesystem.
pub fn parse_tape_json(bytes: &[u8]) -> Result<StageTape, JavaWorkError> {
    serde_json::from_slice(bytes).map_err(|error| JavaWorkError::TapeUnreadable {
        reason: error.to_string(),
    })
}

// ───────────────────────────── corpus identity ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaWorkCorpusDocument {
    #[serde(rename = "id")]
    pub document_id: String,
    /// Recovered through [`de_identity`]: YAML 1.1 resolves an unquoted
    /// `2507.19457` to a float, and a pinned arXiv identity must not be
    /// rejected (or silently retyped) because of that.
    #[serde(default, deserialize_with = "de_identity")]
    pub arxiv_id: String,
    #[serde(default, deserialize_with = "de_optional_identity")]
    pub revision: Option<String>,
    pub source_pdf: String,
    #[serde(rename = "sha256")]
    pub source_sha256: String,
    #[serde(default)]
    pub pages: Option<u32>,
}

/// A YAML scalar recovered as a string identity.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum YamlIdentity {
    String(String),
    Unsigned(u64),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

impl YamlIdentity {
    fn into_string(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Unsigned(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }
}

fn de_identity<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    YamlIdentity::deserialize(deserializer).map(YamlIdentity::into_string)
}

fn de_optional_identity<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<YamlIdentity>::deserialize(deserializer)
        .map(|value| value.map(YamlIdentity::into_string))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaWorkCorpus {
    pub id: String,
    pub corpus_n: usize,
    #[serde(default)]
    pub page_denominator: Option<usize>,
    pub documents: Vec<JavaWorkCorpusDocument>,
}

/// Read the pinned corpus out of `data/parse/layout_bakeoff.yaml#corpus`.
///
/// The manifest is passed in as text so the reader depends on no filesystem
/// path; the example feeds it the git-tracked contract. A weakened corpus
/// (count mismatch, duplicate id, unsafe or non-corpus path, malformed digest)
/// is a typed violation rather than something to skip over.
pub fn load_corpus_from_manifest(manifest_yaml: &str) -> Result<JavaWorkCorpus, JavaWorkError> {
    let manifest: serde_yaml::Value =
        serde_yaml::from_str(manifest_yaml).map_err(|error| JavaWorkError::ManifestUnreadable {
            reason: error.to_string(),
        })?;
    let block = manifest
        .get("corpus")
        .ok_or_else(|| JavaWorkError::ManifestContract {
            path: "corpus".to_string(),
            reason: "block missing from the manifest; loss is not silence".to_string(),
        })?;
    let corpus: JavaWorkCorpus =
        serde_yaml::from_value(block.clone()).map_err(|error| JavaWorkError::ManifestContract {
            path: "corpus".to_string(),
            reason: error.to_string(),
        })?;
    corpus.validate()?;
    Ok(corpus)
}

impl JavaWorkCorpus {
    pub fn validate(&self) -> Result<(), JavaWorkError> {
        let violation = |path: &str, reason: String| JavaWorkError::ManifestContract {
            path: path.to_string(),
            reason,
        };
        if !is_safe_token(&self.id) {
            return Err(violation(
                "corpus.id",
                "corpus id is not a safe token".into(),
            ));
        }
        if self.documents.is_empty() {
            return Err(JavaWorkError::EmptyCorpus);
        }
        if self.corpus_n != self.documents.len() {
            return Err(violation(
                "corpus.corpus_n",
                format!(
                    "declared {} documents but {} are listed",
                    self.corpus_n,
                    self.documents.len()
                ),
            ));
        }
        let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
        for (index, document) in self.documents.iter().enumerate() {
            let at = |field: &str| format!("corpus.documents[{index}].{field}");
            if !is_safe_token(&document.document_id) {
                return Err(violation(
                    &at("id"),
                    "document id is not a safe token".into(),
                ));
            }
            if seen.insert(document.document_id.as_str(), ()).is_some() {
                return Err(violation(&at("id"), "duplicate document id".into()));
            }
            if let Some(class) = unsafe_path_class(&document.source_pdf) {
                return Err(violation(
                    &at("source_pdf"),
                    format!("source path is {class}, expected a repo-relative corpus path"),
                ));
            }
            if !document.source_pdf.starts_with("data/") {
                return Err(violation(
                    &at("source_pdf"),
                    "source path is outside the pinned corpus root `data/`".into(),
                ));
            }
            if !is_sha256_hex(&document.source_sha256) {
                return Err(violation(
                    &at("sha256"),
                    "source digest is not a lowercase sha256 hex digest".into(),
                ));
            }
        }
        Ok(())
    }
}

// ───────────────────────────── the report ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum DocumentJavaWork {
    Receipt {
        document_id: String,
        /// The pinned, repo-relative corpus path (never the tape's raw value).
        source_pdf: String,
        source_sha256: String,
        receipt: NativeJavaWorkReceipt,
    },
    Refused {
        document_id: String,
        source_pdf: String,
        source_sha256: String,
        refusal: Refusal,
    },
    NotObserved {
        document_id: String,
        source_pdf: String,
        source_sha256: String,
        reason: NotObservedReason,
    },
}

impl DocumentJavaWork {
    pub fn document_id(&self) -> &str {
        match self {
            Self::Receipt { document_id, .. }
            | Self::Refused { document_id, .. }
            | Self::NotObserved { document_id, .. } => document_id,
        }
    }

    pub fn receipt(&self) -> Option<&NativeJavaWorkReceipt> {
        match self {
            Self::Receipt { receipt, .. } => Some(receipt),
            _ => None,
        }
    }
}

/// Summed per-model counts across the corpus. Present only when at least one
/// document was measured; otherwise the aggregate is typed absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredAggregate {
    pub documents_measured: usize,
    pub documents_filtered: usize,
    pub documents_refused: usize,
    pub documents_total: usize,
    pub models: Vec<ModelInvocationCount>,
    pub total_invocations: usize,
    pub coverage: CoverageClaim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "measurement_state", rename_all = "snake_case")]
pub enum JavaWorkAggregate {
    Measured(MeasuredAggregate),
    NotObserved { reason: NotObservedReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaWorkUnobserved {
    pub java_cpu_wall_clock: TypedAbsence,
    pub rss_ceiling: TypedAbsence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaWorkLocks {
    pub deployed: bool,
    pub import_eligible: bool,
    pub production_route: String,
    pub auto_promote: String,
    pub selection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaWorkRedaction {
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeJavaWorkReport {
    pub schema_version: String,
    pub adr_ref: String,
    pub unit: String,
    pub corpus: JavaWorkCorpus,
    pub documents: Vec<DocumentJavaWork>,
    pub aggregate: JavaWorkAggregate,
    pub unmatched_tapes: Vec<String>,
    pub unreadable_tape_count: usize,
    /// Whether the operator asked for verification on this run.
    pub verification_requested: bool,
    /// Tapes that verification could not cover. A non-zero value here means the
    /// requested verification is incomplete; it never means "verified".
    pub verification_gaps: usize,
    pub capture_hint: String,
    pub observations: JavaWorkUnobserved,
    pub locks: JavaWorkLocks,
    pub h05_closure: String,
    pub h05_reason: String,
    pub redaction: JavaWorkRedaction,
}

/// Everything the report is assembled from, so the signature stays explicit
/// about what is measured, what is refused and what is merely absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JavaWorkInputs<'a> {
    pub tapes: &'a [StageTape],
    pub verifications: &'a [TapeVerification],
    /// Tapes the caller could not even read or parse.
    pub unreadable_tape_count: usize,
    /// True when verification was requested for this run.
    pub verification_requested: bool,
    /// Tapes verification could not cover at all.
    pub verification_gaps: usize,
}

impl NativeJavaWorkReport {
    /// Assemble the artifact from the pinned corpus and the tapes found on disk.
    ///
    /// Deterministic by construction: documents and models are ordered, and
    /// nothing observational (timing, path of the tape, wall clock) enters the
    /// bytes. A tape that matches no pinned document is reported, never dropped:
    /// "silently fell out" is the failure mode this shape exists to prevent.
    pub fn assemble(
        corpus: &JavaWorkCorpus,
        inputs: &JavaWorkInputs<'_>,
    ) -> Result<Self, JavaWorkError> {
        let JavaWorkInputs {
            tapes,
            verifications,
            unreadable_tape_count,
            verification_requested,
            verification_gaps,
        } = *inputs;
        corpus.validate()?;
        let mut consumed = vec![false; tapes.len()];
        let mut documents = Vec::with_capacity(corpus.documents.len());

        for document in &corpus.documents {
            let matched = match_tape(document, tapes, &consumed);
            let Some(index) = matched else {
                documents.push(DocumentJavaWork::NotObserved {
                    document_id: document.document_id.clone(),
                    source_pdf: document.source_pdf.clone(),
                    source_sha256: document.source_sha256.clone(),
                    reason: if !tapes.is_empty() {
                        NotObservedReason::NoMatchingTape
                    } else if unreadable_tape_count > 0 {
                        NotObservedReason::TapeUnreadable
                    } else {
                        NotObservedReason::TapeNotProvided
                    },
                });
                continue;
            };
            consumed[index] = true;
            let tape = &tapes[index];
            let outcome = if tape.source_sha256 != document.source_sha256 {
                Err(JavaWorkError::CorpusDigestMismatch {
                    observed: Some(tape.source_sha256.clone()),
                })
            } else {
                match verifications
                    .iter()
                    .find(|verification| verification.dump_sha256 == tape_dump_digest(tape))
                {
                    Some(verification) => {
                        NativeJavaWorkReceipt::from_tape_verified(tape, verification)
                    }
                    None => NativeJavaWorkReceipt::from_tape(tape),
                }
            };
            documents.push(match outcome {
                Ok(receipt) => DocumentJavaWork::Receipt {
                    document_id: document.document_id.clone(),
                    source_pdf: document.source_pdf.clone(),
                    source_sha256: document.source_sha256.clone(),
                    receipt,
                },
                Err(error) => DocumentJavaWork::Refused {
                    document_id: document.document_id.clone(),
                    source_pdf: document.source_pdf.clone(),
                    source_sha256: document.source_sha256.clone(),
                    refusal: error.refusal(),
                },
            });
        }

        documents.sort_by(|left, right| left.document_id().cmp(right.document_id()));
        let mut unmatched_tapes = Vec::new();
        for (index, tape) in tapes.iter().enumerate() {
            if !consumed[index] {
                unmatched_tapes.push(match safe_token(&tape.paper_id) {
                    Some(paper_id) => paper_id,
                    None => "<redacted>".to_string(),
                });
            }
        }
        unmatched_tapes.sort();
        unmatched_tapes.dedup();

        let aggregate = aggregate_documents(&documents, corpus.documents.len());
        Ok(Self {
            schema_version: NATIVE_JAVA_WORK_SCHEMA_VERSION.to_string(),
            adr_ref: NATIVE_JAVA_WORK_ADR_REF.to_string(),
            unit: JAVA_WORK_UNIT.to_string(),
            corpus: corpus.clone(),
            documents,
            aggregate,
            unmatched_tapes,
            unreadable_tape_count,
            verification_requested,
            verification_gaps,
            capture_hint: JAVA_WORK_CAPTURE_HINT.to_string(),
            observations: JavaWorkUnobserved {
                java_cpu_wall_clock: TypedAbsence::not_observed(),
                rss_ceiling: TypedAbsence::not_observed(),
            },
            locks: JavaWorkLocks {
                deployed: false,
                import_eligible: false,
                production_route: "forbidden".to_string(),
                auto_promote: "forbidden".to_string(),
                selection: "forbidden".to_string(),
            },
            h05_closure: JAVA_WORK_H05_CLOSURE.to_string(),
            h05_reason: JAVA_WORK_H05_REASON.to_string(),
            redaction: JavaWorkRedaction {
                forbidden: JAVA_WORK_REDACTION_FORBIDDEN
                    .iter()
                    .map(|entry| (*entry).to_string())
                    .collect(),
            },
        })
    }

    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, JavaWorkError> {
        serde_json::to_vec_pretty(self).map_err(|error| JavaWorkError::ManifestUnreadable {
            reason: error.to_string(),
        })
    }
}

fn tape_dump_digest(tape: &StageTape) -> String {
    tape.dump_sha256.clone().unwrap_or_default()
}

/// Match a pinned document to a tape by source digest first, then by source
/// path. The digest is the identity the manifest pins, so a tape recorded from
/// the same bytes matches even when it was recorded under another file name.
fn match_tape(
    document: &JavaWorkCorpusDocument,
    tapes: &[StageTape],
    consumed: &[bool],
) -> Option<usize> {
    tapes
        .iter()
        .enumerate()
        .find(|(index, tape)| !consumed[*index] && tape.source_sha256 == document.source_sha256)
        .or_else(|| {
            tapes
                .iter()
                .enumerate()
                .find(|(index, tape)| !consumed[*index] && tape.source_pdf == document.source_pdf)
        })
        .map(|(index, _)| index)
}

fn aggregate_documents(
    documents: &[DocumentJavaWork],
    documents_total: usize,
) -> JavaWorkAggregate {
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut measured = 0usize;
    let mut filtered = 0usize;
    let mut refused = 0usize;
    for document in documents {
        match document {
            DocumentJavaWork::Receipt { receipt, .. } => match receipt.measured() {
                Some(counts) => {
                    for entry in &counts.models {
                        *totals.entry(entry.model.clone()).or_default() += entry.invocations;
                    }
                    match receipt.state() {
                        MeasurementState::Filtered => filtered += 1,
                        _ => measured += 1,
                    }
                }
                // Unreachable by construction: a receipt outcome is only built
                // from a measured or filtered receipt. Counted as a refusal so
                // that a future divergence cannot silently shrink the totals.
                None => refused += 1,
            },
            DocumentJavaWork::Refused { .. } => refused += 1,
            DocumentJavaWork::NotObserved { .. } => {}
        }
    }
    if measured + filtered == 0 {
        return JavaWorkAggregate::NotObserved {
            reason: NotObservedReason::NoMeasuredDocument,
        };
    }
    let models = totals
        .into_iter()
        .map(|(model, invocations)| ModelInvocationCount { model, invocations })
        .collect::<Vec<_>>();
    let total_invocations = models.iter().map(|entry| entry.invocations).sum();
    let coverage = if filtered > 0 {
        CoverageClaim::Partial {
            reason: PartialReason::FilteredTape,
        }
    } else if measured + refused < documents_total {
        CoverageClaim::Partial {
            reason: PartialReason::IncompleteCorpus,
        }
    } else {
        CoverageClaim::Full
    };
    JavaWorkAggregate::Measured(MeasuredAggregate {
        documents_measured: measured,
        documents_filtered: filtered,
        documents_refused: refused,
        documents_total,
        models,
        total_invocations,
        coverage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");

    const DUMP: &str = "=== model: segmentation ===\ns1\tA\n=== model: date (occurrence 1 of 2) ===\nd1\tB\n=== model: date (occurrence 2 of 2) ===\nd2\tC\n";

    fn tape(dump: &str) -> StageTape {
        StageTape::from_capture(
            "fulltext",
            "2507.19457",
            "data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf",
            "ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
            "0.9.1",
            None,
            dump.as_bytes(),
            None,
            Vec::new(),
        )
    }

    #[test]
    fn counts_are_merged_by_base_name_and_sorted() {
        let receipt = NativeJavaWorkReceipt::from_tape(&tape(DUMP)).expect("receipt");
        let counts = receipt.measured().expect("measured");
        assert_eq!(
            counts.models,
            vec![
                ModelInvocationCount {
                    model: "date".to_string(),
                    invocations: 2,
                },
                ModelInvocationCount {
                    model: "segmentation".to_string(),
                    invocations: 1,
                },
            ]
        );
        assert_eq!(counts.total_invocations, 3);
        assert_eq!(counts.sections, 3);
        assert_eq!(counts.distinct_models, 2);
        assert_eq!(receipt.state(), MeasurementState::Measured);
        assert_eq!(counts.coverage, CoverageClaim::Full);
    }

    #[test]
    fn absence_carries_no_counts_and_survives_serialization() {
        let receipt = NativeJavaWorkReceipt::not_observed(NotObservedReason::TapeNotProvided);
        let json = serde_json::to_string(&receipt).expect("json");
        assert!(json.contains("\"measurement_state\":\"not_observed\""));
        assert!(!json.contains("total_invocations"));
        assert!(!json.contains("sections"));
        assert_eq!(receipt.state(), MeasurementState::NotObserved);
    }

    #[test]
    fn absolute_and_traversing_sources_are_refused() {
        let mut absolute = tape(DUMP);
        absolute.source_pdf = "/root/daily-archive/data/x.pdf".to_string();
        assert_eq!(
            NativeJavaWorkReceipt::from_tape(&absolute).unwrap_err(),
            JavaWorkError::UnsafeSourcePath {
                path_class: "absolute"
            }
        );

        let mut traversing = tape(DUMP);
        traversing.source_pdf = "data/../../etc/passwd".to_string();
        assert_eq!(
            NativeJavaWorkReceipt::from_tape(&traversing).unwrap_err(),
            JavaWorkError::UnsafeSourcePath {
                path_class: "traversing"
            }
        );
    }

    #[test]
    fn a_hostile_model_name_is_refused_rather_than_echoed() {
        let hostile = "=== model: /root/secret.token ===\nd1\tB\n";
        assert_eq!(
            NativeJavaWorkReceipt::from_tape(&tape(hostile)).unwrap_err(),
            JavaWorkError::UnsafeModelName
        );
    }

    #[test]
    fn a_verification_record_of_another_tape_does_not_bind() {
        let tape = tape(DUMP);
        let digest = tape.dump_sha256.clone().expect("digest");
        let bound = TapeVerification {
            tape_schema_version: STAGE_TAPE_SCHEMA_VERSION.to_string(),
            dump_sha256: digest.clone(),
            backend_version: "0.9.1".to_string(),
            findings: 0,
        };
        let receipt =
            NativeJavaWorkReceipt::from_tape_verified(&tape, &bound).expect("verified receipt");
        assert_eq!(
            receipt.measured().expect("counts").tape_verification,
            TapeVerificationState::Verified
        );

        // A verification of different bytes must not be presented as this
        // tape's verification, and a run with findings refuses outright.
        let other_tape = TapeVerification {
            dump_sha256: "0".repeat(64),
            ..bound.clone()
        };
        assert_eq!(
            NativeJavaWorkReceipt::from_tape_verified(&tape, &other_tape).unwrap_err(),
            JavaWorkError::VerificationBindingMismatch {
                field: "dump_sha256"
            }
        );
        let with_findings = TapeVerification {
            findings: 2,
            ..bound.clone()
        };
        assert_eq!(
            NativeJavaWorkReceipt::from_tape_verified(&tape, &with_findings).unwrap_err(),
            JavaWorkError::VerificationFindings { findings: 2 }
        );
        assert_eq!(
            NativeJavaWorkReceipt::from_tape(&tape)
                .expect("unverified receipt")
                .measured()
                .expect("counts")
                .tape_verification,
            TapeVerificationState::NotObserved
        );
    }

    #[test]
    fn a_filtered_tape_cannot_claim_full_coverage() {
        let filtered = StageTape::from_capture(
            "fulltext",
            "2507.19457",
            "data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf",
            "ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
            "0.9.1",
            Some("segmentation"),
            DUMP.as_bytes(),
            None,
            Vec::new(),
        );
        let receipt = NativeJavaWorkReceipt::from_tape(&filtered).expect("receipt");
        assert_eq!(receipt.state(), MeasurementState::Filtered);
        assert_eq!(
            receipt.measured().expect("counts").coverage,
            CoverageClaim::Partial {
                reason: PartialReason::FilteredTape
            }
        );
        assert_eq!(
            receipt.require_full_coverage().unwrap_err(),
            JavaWorkError::FilteredFullCoverage
        );
    }

    #[test]
    fn the_redaction_vocabulary_does_not_drift_from_data() {
        let manifest: serde_yaml::Value = serde_yaml::from_str(MANIFEST).expect("manifest yaml");
        let forbidden = manifest
            .get("native_core_qualification")
            .and_then(|block| block.get("redaction"))
            .and_then(|redaction| redaction.get("forbidden"))
            .and_then(|list| list.as_sequence())
            .expect("native_core_qualification.redaction.forbidden");
        let data_vocabulary = forbidden
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>();
        for entry in JAVA_WORK_REDACTION_FORBIDDEN {
            assert!(
                data_vocabulary.contains(&entry),
                "redaction vocabulary lost `{entry}`; DATA owns the closed list"
            );
        }
    }

    #[test]
    fn the_pinned_corpus_parses_and_is_well_formed() {
        let manifest = include_str!("../../../data/parse/layout_bakeoff.yaml");
        let corpus = load_corpus_from_manifest(manifest).expect("corpus");
        assert_eq!(corpus.corpus_n, 3);
        assert_eq!(corpus.documents.len(), 3);
        assert_eq!(corpus.documents[0].document_id, "hub-2507.19457");
        assert!(
            corpus
                .documents
                .iter()
                .all(|document| is_sha256_hex(&document.source_sha256))
        );
    }

    #[test]
    fn a_weakened_corpus_is_refused_rather_than_skipped() {
        let weakened = "corpus:\n  id: x\n  corpus_n: 2\n  documents:\n    - id: a\n      arxiv_id: '1'\n      source_pdf: data/a.pdf\n      sha256: aa\n";
        let error = load_corpus_from_manifest(weakened).unwrap_err();
        assert_eq!(error.code(), "manifest_contract");

        let missing = load_corpus_from_manifest("schema_version: x\n").unwrap_err();
        assert_eq!(missing.code(), "manifest_contract");

        let unsafe_path = "corpus:\n  id: x\n  corpus_n: 1\n  documents:\n    - id: a\n      arxiv_id: '1'\n      source_pdf: /root/a.pdf\n      sha256: ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec\n";
        let error = load_corpus_from_manifest(unsafe_path).unwrap_err();
        assert_eq!(error.code(), "manifest_contract");
    }
}
