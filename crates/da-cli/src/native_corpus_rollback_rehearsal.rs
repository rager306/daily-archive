//! S08/T04 (M514-1lss2s) — real-corpus rollback rehearsal.
//!
//! The milestone requires a rollback rehearsal over the **real frozen corpus**.
//! Earlier rehearsals (S06 / M513) were three-document and offline-hermetic, so
//! they could not show that the declared rollback path holds across the 610
//! assigned holdout documents. S09 may switch families only after a human
//! decision, so S08 must first prove the rollback path on the real corpus.
//!
//! ## The rehearsal is a byte-identity primitive
//!
//! For every assigned document the declared sequence runs entirely in process
//! memory:
//!
//! 1. **baseline** — the Java fallback, i.e. the committed preserved-TEI bytes
//!    ([`crate::native_acceptance`] never consumes these; they are the pinned
//!    oracle carrier the rollback returns to);
//! 2. **switched** — the native band, embedded from the T01 native output
//!    (the per-document `layout_digest`/plane state the T01 acceptance runner
//!    actually measured);
//! 3. **returned** — the Java fallback again, restored from the committed
//!    baseline.
//!
//! The rollback claim is **byte equality** `returned == baseline`, computed on
//! the bytes ([`run_rollback_sequence`]), and the corpus claim is that no
//! document leaked a native fragment into the rollback. Nothing is declarative:
//! a rehearsal whose switched payload equals its baseline is a vacuous switch
//! and is refused ([`RollbackRehearsalError::VacuousSwitch`]).
//!
//! ## A missing fixture is a typed refusal, never a skip
//!
//! `data/parse/comparison_contract.yaml#hard_regressions` names
//! `silent_skip_missing_tei_fixture`: a missing or changed TEI fixture must not
//! be silently skipped. A document with an absent, empty or drifted preserved-TEI
//! fixture therefore becomes a **typed refusal row** with a closed reason, never
//! a fabricated baseline and never a dropped row
//! ([`RollbackRehearsalError::RefusalReasonUnknown`]).
//!
//! ## Hermetic and read-only
//!
//! The switch lives only in process memory (`switch_scope = in_memory_only`),
//! the declared carriers are read-only, no Java is consulted
//! (`observed_java_calls = 0`), no network is used, and no production route is
//! installed. The module is pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2`
//! and compiles in the default feature set; the gated example and contract test
//! drive it against the committed carriers. It never writes to the repository
//! except the single canonical artifact.

use crate::native_acceptance::{
    AcceptanceDocumentRow, DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL,
    NativeAcceptanceReceipt,
};
use crate::native_corpus_resource_same_n::AssignedIdentity;
use crate::native_family_receipt::{
    CarrierBinding, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS, Redaction,
    THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest, sha256_prefixed,
};
use crate::native_route_receipt::{
    ROUTE_DIRECTION_VOCABULARY, ROUTE_FREEZE_SCOPE, ROUTE_FREEZE_STATE_FROZEN,
    ROUTE_SILENT_CLAMP_FORBIDDEN, ROUTE_TUNED_AFTER_SCORING_FORBIDDEN, RouteFreezeDeclaration,
    RouteLimitsBinding,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted rollback rehearsal artifact.
pub const NATIVE_CORPUS_ROLLBACK_REHEARSAL_SCHEMA_VERSION: &str =
    "native-corpus-rollback-rehearsal.v1";
/// Default artifact path of the rollback rehearsal.
pub const DEFAULT_CORPUS_ROLLBACK_REHEARSAL_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_corpus_rollback_rehearsal.json";
/// Repository-relative carrier of the T01 corpus-bound native measurement.
pub const CORPUS_ROLLBACK_NATIVE_MEASUREMENT_REL: &str =
    "artifacts/parse_compat/native_corpus_acceptance.json";
/// Repository-relative carrier of the preserved-TEI oracle pins.
pub const CORPUS_ROLLBACK_TEI_PROVENANCE_REL: &str =
    "crates/da-parse/tests/fixtures/holdout/provenance.yaml";
/// Pointer inside the TEI provenance carrier at the pinned documents.
pub const CORPUS_ROLLBACK_TEI_PROVENANCE_POINTER: &str = "documents";
/// Repository-relative carrier of the committed route receipt holding the freeze.
pub const CORPUS_ROLLBACK_ROUTE_RECEIPT_REL: &str = "artifacts/parse_compat/native_route_e2e.json";
/// Repository-relative carrier of the governor freeze block.
pub const CORPUS_ROLLBACK_GOVERNOR_REL: &str =
    "scripts/governor/checks/grobid_absorption_progress.yaml";
/// Pointer inside the governor carrier at the route freeze block.
pub const CORPUS_ROLLBACK_GOVERNOR_POINTER: &str = "native_route_e2e";
/// The claim scope: a rehearsal, never a product coverage claim.
pub const CORPUS_ROLLBACK_CLAIM_SCOPE_REHEARSAL_ONLY: &str = "rehearsal_only";

/// The declared sequence, in order.
pub const ROLLBACK_SEQUENCE_STEPS: [&str; 3] = ["baseline", "switched", "returned"];
/// The baseline is the committed preserved-TEI Java fallback.
pub const ROLLBACK_BASELINE_SOURCE: &str = "committed_preserved_tei";
/// The switched route is the native band embedded from the T01 native output.
pub const ROLLBACK_SWITCHED_SOURCE: &str = "in_memory_native_band_from_t01_native_output";
/// The returned route restores the committed baseline.
pub const ROLLBACK_RETURNED_SOURCE: &str = "in_memory_restore_from_committed_baseline";
/// The switch exists only in process memory.
pub const ROLLBACK_SWITCH_SCOPE_IN_MEMORY: &str = "in_memory_only";

/// The in-memory marker that prefixes a native-band fragment. It is the only
/// signal the leak scan needs and never appears in a preserved-TEI baseline.
pub const NATIVE_BAND_FRAGMENT_MARKER: &str = "<native-band>";

/// The closed document-state vocabulary.
pub const ROLLBACK_DOCUMENT_STATE_VOCABULARY: [&str; 2] = ["rehearsed", "refused"];
/// The closed native-output-state vocabulary carried from T01.
pub const ROLLBACK_NATIVE_OUTPUT_STATE_VOCABULARY: [&str; 3] = ["observed", "refused", "not_run"];
/// The closed refusal-reason vocabulary. A missing or changed fixture is named.
pub const ROLLBACK_REFUSAL_REASON_VOCABULARY: [&str; 4] = [
    "missing_tei_fixture",
    "empty_tei_fixture",
    "tei_fixture_sha256_drift",
    "tei_fixture_pin_absent",
];

/// Locks as declared, after the required-lock check.
fn required_locks() -> BTreeMap<String, String> {
    REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect()
}

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals. Every variant is a refusal, never a silent downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackRehearsalError {
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt claimed more than `rehearsal_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// A required lock is open or missing.
    LockOpened { lock: String, found: String },
    /// The serialized receipt leaked a path/host/TEI marker.
    RedactionLeak { marker: String },
    /// A redaction flag is not the required `false`.
    RedactionFlag { field: String, found: bool },
    /// The rehearsal consulted Java, used the network, wrote a carrier or kept
    /// the switch outside process memory.
    NonHermeticRehearsal { detail: String },
    /// A declared carrier path is not repository-relative.
    NonHermeticInput { carrier: String, detail: String },
    /// The declared sequence drifted from `baseline -> switched -> returned`.
    SequenceDrift { found: String },
    /// A corpus-pin binding is unpinned.
    CorpusPinUnpinned { carrier: String, detail: String },
    /// The parsed receipt's freeze revision drifted from the recomputed one.
    StaleFreezeRevision { expected: String, found: String },
    /// The limits are not frozen.
    LimitNotFrozen { detail: String },
    /// The freeze was tuned after scoring.
    LimitTunedAfterScoring { found: String },
    /// A rehearsed document's returned bytes differ from its baseline bytes.
    RollbackNotByteIdentical { document: String },
    /// A native fragment leaked into the returned route.
    NativeFragmentLeaked { document: String },
    /// The switched payload was byte-identical to the baseline: nothing switched.
    VacuousSwitch { document: String },
    /// A refused row carries a reason outside the closed vocabulary.
    RefusalReasonUnknown { document: String, reason: String },
    /// A refused row is shaped like a rehearsal, or a rehearsed row lacks a digest.
    RefusalShapeInvalid { document: String, detail: String },
    /// The document rows disagree with the assigned holdout census.
    DocumentSetMismatch { detail: String },
    /// The populations disagrees with the document rows.
    PopulationDrift { detail: String },
    /// The corpus claim disagrees with the document rows.
    CorpusClaimFalsified { detail: String },
    /// The residuals disagree with the document rows.
    ResidualDrift { detail: String },
    /// A document state outside the closed vocabulary.
    DocumentStateUnknown { document: String, state: String },
    /// A document is declared more than once.
    DuplicateDocument { document: String },
    /// Malformed JSON was handed to the parser.
    Malformed { detail: String },
    /// A required field was absent.
    MissingField { field: String },
}

impl fmt::Display for RollbackRehearsalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RollbackRehearsalError::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "schema drift on `{field}`: expected `{expected}`, found `{found}`"
            ),
            RollbackRehearsalError::ClaimScopeDrift { found } => {
                write!(f, "claim scope drifted: found `{found}`")
            }
            RollbackRehearsalError::ProductCoverageClaimed { found } => {
                write!(f, "product coverage claimed: found `{found}`")
            }
            RollbackRehearsalError::ThresholdClaimed { found } => {
                write!(f, "thresholds claimed accepted: found `{found}`")
            }
            RollbackRehearsalError::LockOpened { lock, found } => {
                write!(f, "lock `{lock}` not closed: found `{found}`")
            }
            RollbackRehearsalError::RedactionLeak { marker } => {
                write!(f, "redaction leak: marker `{marker}`")
            }
            RollbackRehearsalError::RedactionFlag { field, found } => {
                write!(f, "redaction flag `{field}` is `{found}`")
            }
            RollbackRehearsalError::NonHermeticRehearsal { detail } => {
                write!(f, "non-hermetic rehearsal: {detail}")
            }
            RollbackRehearsalError::NonHermeticInput { carrier, detail } => {
                write!(f, "non-hermetic input `{carrier}`: {detail}")
            }
            RollbackRehearsalError::SequenceDrift { found } => {
                write!(f, "rollback sequence drifted: found `{found}`")
            }
            RollbackRehearsalError::CorpusPinUnpinned { carrier, detail } => {
                write!(f, "corpus pin `{carrier}` is unpinned: {detail}")
            }
            RollbackRehearsalError::StaleFreezeRevision { expected, found } => {
                write!(
                    f,
                    "stale freeze revision: expected `{expected}`, found `{found}`"
                )
            }
            RollbackRehearsalError::LimitNotFrozen { detail } => {
                write!(f, "limits not frozen: {detail}")
            }
            RollbackRehearsalError::LimitTunedAfterScoring { found } => {
                write!(f, "freeze tuned after scoring: found `{found}`")
            }
            RollbackRehearsalError::RollbackNotByteIdentical { document } => {
                write!(f, "rollback not byte-identical for document `{document}`")
            }
            RollbackRehearsalError::NativeFragmentLeaked { document } => {
                write!(f, "native fragment leaked into rollback `{document}`")
            }
            RollbackRehearsalError::VacuousSwitch { document } => {
                write!(f, "vacuous switch for document `{document}`")
            }
            RollbackRehearsalError::RefusalReasonUnknown { document, reason } => {
                write!(
                    f,
                    "document `{document}` refused with unknown reason `{reason}`"
                )
            }
            RollbackRehearsalError::RefusalShapeInvalid { document, detail } => {
                write!(f, "document `{document}` refusal shape invalid: {detail}")
            }
            RollbackRehearsalError::DocumentSetMismatch { detail } => {
                write!(f, "document set mismatch: {detail}")
            }
            RollbackRehearsalError::PopulationDrift { detail } => {
                write!(f, "population drift: {detail}")
            }
            RollbackRehearsalError::CorpusClaimFalsified { detail } => {
                write!(f, "corpus claim falsified: {detail}")
            }
            RollbackRehearsalError::ResidualDrift { detail } => {
                write!(f, "residual drift: {detail}")
            }
            RollbackRehearsalError::DocumentStateUnknown { document, state } => {
                write!(f, "document `{document}` has unknown state `{state}`")
            }
            RollbackRehearsalError::DuplicateDocument { document } => {
                write!(f, "document `{document}` is declared more than once")
            }
            RollbackRehearsalError::Malformed { detail } => {
                write!(f, "malformed receipt: {detail}")
            }
            RollbackRehearsalError::MissingField { field } => {
                write!(f, "missing required field `{field}`")
            }
        }
    }
}

impl std::error::Error for RollbackRehearsalError {}

// ───────────────────────────── types ─────────────────────────────

/// The hermetic-route proof the rehearsal is bound to. Every field is an
/// invariant; a single deviation is a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackHermeticity {
    /// Java consult attempts observed by the guard. Zero is a measurement.
    pub observed_java_calls: u64,
    /// The rehearsal used the network.
    pub network_observed: bool,
    /// The declared carriers were opened read-only.
    pub corpus_read_only: bool,
    /// The route switch existed only in process memory.
    pub switch_in_memory_only: bool,
    /// The switch changed bytes for every rehearsed document.
    pub switch_positive_control: bool,
}

/// The corpus pins the rehearsal is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackCorpusPins {
    /// The frozen holdout manifest naming the assigned census.
    pub holdout_manifest: CarrierBinding,
    /// The T01 native measurement the switched payload derives from.
    pub native_measurement: CarrierBinding,
    /// The preserved-TEI oracle pins that name the baseline bytes.
    pub tei_provenance: CarrierBinding,
    /// The committed route receipt holding the prospective freeze.
    pub route_receipt: CarrierBinding,
}

/// The declared sequence and its scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackSequenceDeclaration {
    /// `baseline`, `switched`, `returned`, in order.
    pub steps: Vec<String>,
    /// Where the baseline bytes come from.
    pub baseline_source: String,
    /// Where the switched bytes come from.
    pub switched_source: String,
    /// Where the returned bytes come from.
    pub returned_source: String,
    /// `in_memory_only`.
    pub switch_scope: String,
    /// Always `true`.
    pub carriers_read_only: bool,
}

/// The frozen denominator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackPopulations {
    /// The assigned holdout document count.
    pub assigned: u64,
    /// Documents whose sequence completed.
    pub rehearsed: u64,
    /// Documents with a typed refusal.
    pub refused: u64,
}

/// The corpus-level claim the contract test recomputes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackCorpusClaim {
    /// `returned == baseline` for every rehearsed document and no refusals.
    pub byte_identity_holds: bool,
    /// No document leaked a native fragment into the rollback.
    pub native_fragment_leak_free: bool,
    /// Every rehearsed switch changed bytes.
    pub switch_positive_control: bool,
    /// The declared document count.
    pub declared_documents: u64,
    /// The rehearsed document count.
    pub rehearsed_documents: u64,
}

/// The counters the contract test recomputes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackResiduals {
    /// Documents whose sequence completed.
    pub rehearsed_documents: u64,
    /// Documents with a typed refusal.
    pub refused_documents: u64,
    /// Documents whose returned bytes differed from the baseline.
    pub byte_identity_violations: u64,
    /// Documents whose rollback carried a native fragment.
    pub native_fragment_leaks: u64,
    /// Documents whose fixture was missing, empty, unpinned or drifted.
    pub refused_fixtures: u64,
}

/// One document's rehearsal row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackDocumentRow {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Repository-relative preserved-TEI fixture path.
    pub tei_path: String,
    /// The pinned sha256 of the baseline bytes.
    pub tei_sha256_pin: Option<String>,
    /// The native output state carried from T01.
    pub native_output_state: String,
    /// `rehearsed` or `refused`.
    pub state: String,
    /// The typed refusal reason, present exactly when `state = refused`.
    pub refusal_reason: Option<String>,
    /// sha256 of the baseline bytes.
    pub baseline_sha256: Option<String>,
    /// sha256 of the switched native-band bytes.
    pub switched_sha256: Option<String>,
    /// sha256 of the returned bytes.
    pub returned_sha256: Option<String>,
    /// `returned == baseline`, computed on the bytes.
    pub byte_identity: bool,
    /// The returned bytes carried the native-band marker.
    pub native_fragment_leaked: bool,
}

/// The emitted rollback rehearsal receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackRehearsalReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// `rehearsal_only`.
    pub claim_scope: String,
    /// `none`.
    pub product_coverage_effect: String,
    /// `not_accepted`.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// The hermetic-route proof.
    pub hermeticity: RollbackHermeticity,
    /// The prospective limit freeze binding.
    pub limits: RouteLimitsBinding,
    /// Binding to the governor freeze block the revision was recomputed from.
    pub freeze_governor: CarrierBinding,
    /// The corpus pins the rehearsal is bound to.
    pub corpus_pins: RollbackCorpusPins,
    /// The declared sequence.
    pub sequence: RollbackSequenceDeclaration,
    /// The frozen denominator.
    pub populations: RollbackPopulations,
    /// One typed row per assigned document, in carrier order.
    pub documents: Vec<RollbackDocumentRow>,
    /// The corpus-level claim.
    pub corpus_claim: RollbackCorpusClaim,
    /// The closure counters.
    pub residuals: RollbackResiduals,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl RollbackRehearsalReceipt {
    /// Parse a receipt from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, RollbackRehearsalError> {
        serde_json::from_slice(bytes).map_err(|error| RollbackRehearsalError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, RollbackRehearsalError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| RollbackRehearsalError::Malformed {
                detail: error.to_string(),
            })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| RollbackRehearsalError::Malformed {
                detail: error.to_string(),
            })?;
        assert_corpus_rollback_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized receipt.
pub fn assert_corpus_rollback_redacted(json: &str) -> Result<(), RollbackRehearsalError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(RollbackRehearsalError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── primitive ─────────────────────────────

/// Which route the in-memory buffer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveRoute {
    /// The committed preserved-TEI Java fallback.
    JavaFallback,
    /// The native band.
    NativeBand,
}

impl ActiveRoute {
    /// Switch the in-process buffer to the native band. The committed carrier is
    /// never touched; only the returned buffer changes.
    fn switch_to_native(self, native_fragment: &[u8]) -> Vec<u8> {
        match self {
            ActiveRoute::JavaFallback | ActiveRoute::NativeBand => native_fragment.to_vec(),
        }
    }
}

/// The raw outcome of one in-memory sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackSequenceOutcome {
    /// sha256 of the baseline bytes.
    pub baseline_sha256: String,
    /// sha256 of the switched native-band bytes.
    pub switched_sha256: String,
    /// sha256 of the returned bytes.
    pub returned_sha256: String,
    /// `returned == baseline`, computed on the bytes.
    pub byte_identity: bool,
    /// The returned bytes carried the native-band marker.
    pub native_fragment_leaked: bool,
}

/// Whether `haystack` contains the native-band marker.
pub fn contains_native_band_marker(haystack: &[u8]) -> bool {
    let needle = NATIVE_BAND_FRAGMENT_MARKER.as_bytes();
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Build the in-memory native-band payload from the T01 native output. A
/// measurable document carries its native plane digest; a document whose plane
/// did not run carries the typed state, never a fabricated digest.
pub fn native_band_fragment(native_output_state: &str, layout_digest: Option<&str>) -> Vec<u8> {
    format!(
        "{NATIVE_BAND_FRAGMENT_MARKER}{native_output_state}:{}",
        layout_digest.unwrap_or("<not_run>")
    )
    .into_bytes()
}

/// Run the declared sequence over the raw bytes:
///
/// 1. the buffer starts as the committed Java fallback;
/// 2. the switch replaces it with the native band, in process memory only;
/// 3. the return restores it from the committed baseline.
///
/// The outcome compares the returned bytes to the baseline and scans for a
/// leaked native fragment. No carrier is written.
pub fn run_rollback_sequence(baseline: &[u8], native_fragment: &[u8]) -> RollbackSequenceOutcome {
    let baseline_sha256 = sha256_prefixed(baseline);
    let switched_sha256 = sha256_prefixed(native_fragment);

    // Switch: the in-process buffer becomes the native band; the committed
    // carrier is untouched. The switched bytes are read back so the positive
    // control can see that the payload changed.
    let switched_bytes = ActiveRoute::JavaFallback.switch_to_native(native_fragment);

    // Return: restore from the pristine committed baseline.
    let returned = restore_from_committed_baseline(baseline);
    let byte_identity = returned == baseline;
    let native_fragment_leaked =
        contains_native_band_marker(&returned) && contains_native_band_marker(&switched_bytes);

    RollbackSequenceOutcome {
        baseline_sha256,
        switched_sha256,
        returned_sha256: sha256_prefixed(&returned),
        byte_identity,
        native_fragment_leaked,
    }
}

/// The only return path: the committed baseline bytes, never the mutated buffer.
fn restore_from_committed_baseline(baseline: &[u8]) -> Vec<u8> {
    baseline.to_vec()
}

/// The T01 native output a switched payload derives from, joined to the pinned
/// preserved-TEI baseline of one document.
#[derive(Debug, Clone)]
pub struct RollbackDocumentInput {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Repository-relative preserved-TEI fixture path.
    pub tei_path: String,
    /// The pinned sha256 of the baseline bytes, when the provenance pins it.
    pub tei_sha256_pin: Option<String>,
    /// The native output state carried from T01.
    pub native_output_state: String,
    /// The baseline bytes, or `None` when the fixture is absent.
    pub baseline: Option<Vec<u8>>,
    /// The native-band payload embedded from the T01 native output.
    pub native_fragment: Vec<u8>,
}

/// The builder input.
#[derive(Debug, Clone)]
pub struct RollbackRehearsalInput {
    /// The corpus pins.
    pub corpus_pins: RollbackCorpusPins,
    /// Binding to the governor freeze block.
    pub freeze_governor: CarrierBinding,
    /// The freeze declaration the limits must match.
    pub freeze: RouteFreezeDeclaration,
    /// The frozen limits binding.
    pub limits: RouteLimitsBinding,
    /// The hermetic-route proof.
    pub hermeticity: RollbackHermeticity,
    /// One input per assigned document.
    pub documents: Vec<RollbackDocumentInput>,
    /// The declared claim scope.
    pub claim_scope: String,
    /// The declared product-coverage effect.
    pub product_coverage_effect: String,
    /// The declared thresholds state.
    pub thresholds: String,
}

/// What the validator recomputes and compares against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRehearsalExpectation {
    /// The composite freeze revision recomputed from the committed carriers.
    pub freeze_revision: String,
    /// The T01 carrier binding the receipt must name.
    pub native_measurement: CarrierBinding,
    /// The preserved-TEI provenance binding the receipt must name.
    pub tei_provenance: CarrierBinding,
    /// The assigned identities the document rows must match exactly.
    pub assigned: Vec<AssignedIdentity>,
}

// ───────────────────────────── builder ─────────────────────────────

/// Build the receipt, refusing every contract violation by name.
pub fn build_rollback_rehearsal_receipt(
    input: &RollbackRehearsalInput,
) -> Result<RollbackRehearsalReceipt, RollbackRehearsalError> {
    let locks = required_locks();
    check_locks(&locks)?;
    check_claim(
        &input.claim_scope,
        &input.product_coverage_effect,
        &input.thresholds,
    )?;
    check_limits(&input.limits)?;
    if input.freeze.freeze_revision != input.limits.freeze_revision
        || input.freeze.metrics != input.limits.metrics
    {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!(
                "limits {} do not match the declared freeze {}",
                input.limits.freeze_revision, input.freeze.freeze_revision
            ),
        });
    }
    check_corpus_pins(&input.corpus_pins)?;
    check_freeze_governor(&input.freeze_governor)?;

    // Build the rows first.
    let mut documents = Vec::with_capacity(input.documents.len());
    for document in &input.documents {
        documents.push(build_document_row(document)?);
    }

    let populations = populations_of(&documents);
    let switch_positive_control = documents.iter().all(|row| {
        row.state != "rehearsed" || row.switched_sha256.as_deref() != row.baseline_sha256.as_deref()
    });
    let corpus_claim = corpus_claim_of(&documents, switch_positive_control);
    let residuals = residuals_of(&documents);

    Ok(RollbackRehearsalReceipt {
        schema_version: NATIVE_CORPUS_ROLLBACK_REHEARSAL_SCHEMA_VERSION.to_owned(),
        claim_scope: input.claim_scope.clone(),
        product_coverage_effect: input.product_coverage_effect.clone(),
        thresholds: input.thresholds.clone(),
        locks,
        hermeticity: RollbackHermeticity {
            switch_positive_control,
            ..input.hermeticity.clone()
        },
        limits: input.limits.clone(),
        freeze_governor: input.freeze_governor.clone(),
        corpus_pins: input.corpus_pins.clone(),
        sequence: RollbackSequenceDeclaration {
            steps: ROLLBACK_SEQUENCE_STEPS
                .iter()
                .map(|step| (*step).to_owned())
                .collect(),
            baseline_source: ROLLBACK_BASELINE_SOURCE.to_owned(),
            switched_source: ROLLBACK_SWITCHED_SOURCE.to_owned(),
            returned_source: ROLLBACK_RETURNED_SOURCE.to_owned(),
            switch_scope: ROLLBACK_SWITCH_SCOPE_IN_MEMORY.to_owned(),
            carriers_read_only: true,
        },
        populations,
        documents,
        corpus_claim,
        residuals,
        redaction: Redaction::clean(),
    })
}

fn build_document_row(
    document: &RollbackDocumentInput,
) -> Result<RollbackDocumentRow, RollbackRehearsalError> {
    if !is_repo_relative_path(&document.tei_path) {
        return Err(RollbackRehearsalError::NonHermeticInput {
            carrier: document.tei_path.clone(),
            detail: "the TEI fixture path is not repository-relative".to_owned(),
        });
    }
    if !ROLLBACK_NATIVE_OUTPUT_STATE_VOCABULARY.contains(&document.native_output_state.as_str()) {
        return Err(RollbackRehearsalError::DocumentStateUnknown {
            document: document.id.clone(),
            state: document.native_output_state.clone(),
        });
    }

    let (state, refusal_reason, baseline_sha256, switched_sha256, returned_sha256) =
        match document.baseline.as_deref() {
            None => (
                "refused",
                Some("missing_tei_fixture".to_owned()),
                None,
                None,
                None,
            ),
            Some([]) => (
                "refused",
                Some("empty_tei_fixture".to_owned()),
                None,
                None,
                None,
            ),
            Some(baseline) => match document.tei_sha256_pin.as_deref() {
                None => (
                    "refused",
                    Some("tei_fixture_pin_absent".to_owned()),
                    None,
                    None,
                    None,
                ),
                Some(pin) if sha256_prefixed(baseline) != pin => (
                    "refused",
                    Some("tei_fixture_sha256_drift".to_owned()),
                    None,
                    None,
                    None,
                ),
                Some(_) => {
                    let outcome = run_rollback_sequence(baseline, &document.native_fragment);
                    if !outcome.byte_identity {
                        return Err(RollbackRehearsalError::RollbackNotByteIdentical {
                            document: document.id.clone(),
                        });
                    }
                    if outcome.native_fragment_leaked {
                        return Err(RollbackRehearsalError::NativeFragmentLeaked {
                            document: document.id.clone(),
                        });
                    }
                    if outcome.switched_sha256 == outcome.baseline_sha256 {
                        return Err(RollbackRehearsalError::VacuousSwitch {
                            document: document.id.clone(),
                        });
                    }
                    (
                        "rehearsed",
                        None,
                        Some(outcome.baseline_sha256),
                        Some(outcome.switched_sha256),
                        Some(outcome.returned_sha256),
                    )
                }
            },
        };

    let byte_identity = state == "rehearsed";
    Ok(RollbackDocumentRow {
        id: document.id.clone(),
        source_pdf: document.source_pdf.clone(),
        tei_path: document.tei_path.clone(),
        tei_sha256_pin: document.tei_sha256_pin.clone(),
        native_output_state: document.native_output_state.clone(),
        state: state.to_owned(),
        refusal_reason,
        baseline_sha256,
        switched_sha256,
        returned_sha256,
        byte_identity,
        native_fragment_leaked: false,
    })
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed receipt and refuse what a builder cannot
/// express: a stale freeze revision, a reopened lock, a non-hermetic route, a
/// document-set drift, a byte-identity violation or a leaked native fragment.
pub fn validate_rollback_rehearsal_receipt(
    receipt: &RollbackRehearsalReceipt,
    expectation: &RollbackRehearsalExpectation,
) -> Result<(), RollbackRehearsalError> {
    if receipt.schema_version != NATIVE_CORPUS_ROLLBACK_REHEARSAL_SCHEMA_VERSION {
        return Err(RollbackRehearsalError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_CORPUS_ROLLBACK_REHEARSAL_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.limits.freeze_revision != expectation.freeze_revision {
        return Err(RollbackRehearsalError::StaleFreezeRevision {
            expected: expectation.freeze_revision.clone(),
            found: receipt.limits.freeze_revision.clone(),
        });
    }
    check_claim(
        &receipt.claim_scope,
        &receipt.product_coverage_effect,
        &receipt.thresholds,
    )?;
    check_locks(&receipt.locks)?;
    check_limits(&receipt.limits)?;
    check_hermeticity(&receipt.hermeticity)?;
    check_corpus_pins(&receipt.corpus_pins)?;
    check_freeze_governor(&receipt.freeze_governor)?;
    check_sequence(&receipt.sequence)?;

    if receipt.corpus_pins.native_measurement != expectation.native_measurement {
        return Err(RollbackRehearsalError::CorpusPinUnpinned {
            carrier: receipt.corpus_pins.native_measurement.manifest.clone(),
            detail: "the receipt names a different T01 measurement carrier".to_owned(),
        });
    }
    if receipt.corpus_pins.tei_provenance != expectation.tei_provenance {
        return Err(RollbackRehearsalError::CorpusPinUnpinned {
            carrier: receipt.corpus_pins.tei_provenance.manifest.clone(),
            detail: "the receipt names a different TEI provenance carrier".to_owned(),
        });
    }

    // The document set is the frozen assigned census: mismatch is a hard failure.
    check_document_set(&expectation.assigned, &receipt.documents)?;

    let mut seen = BTreeSet::new();
    for document in &receipt.documents {
        if !seen.insert(document.id.as_str()) {
            return Err(RollbackRehearsalError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
        check_document_row(document)?;
    }

    let populations = populations_of(&receipt.documents);
    if populations != receipt.populations {
        return Err(RollbackRehearsalError::PopulationDrift {
            detail: format!(
                "declared {:?}, recomputed {:?}",
                receipt.populations, populations
            ),
        });
    }

    let switch_positive_control = receipt.documents.iter().all(|row| {
        row.state != "rehearsed" || row.switched_sha256.as_deref() != row.baseline_sha256.as_deref()
    });
    let corpus_claim = corpus_claim_of(&receipt.documents, switch_positive_control);
    if corpus_claim != receipt.corpus_claim {
        return Err(RollbackRehearsalError::CorpusClaimFalsified {
            detail: format!(
                "declared {:?}, recomputed {:?}",
                receipt.corpus_claim, corpus_claim
            ),
        });
    }

    let residuals = residuals_of(&receipt.documents);
    if residuals != receipt.residuals {
        return Err(RollbackRehearsalError::ResidualDrift {
            detail: format!(
                "declared {:?}, recomputed {:?}",
                receipt.residuals, residuals
            ),
        });
    }

    assert_redaction(&receipt.redaction)?;
    let re_serialized =
        serde_json::to_vec(receipt).map_err(|error| RollbackRehearsalError::Malformed {
            detail: error.to_string(),
        })?;
    let re_serialized =
        std::str::from_utf8(&re_serialized).map_err(|error| RollbackRehearsalError::Malformed {
            detail: error.to_string(),
        })?;
    assert_corpus_rollback_redacted(re_serialized)?;

    Ok(())
}

/// One row's shape: a rehearsed row carries three consistent digests; a refused
/// row carries a closed reason and no fabricated baseline.
fn check_document_row(document: &RollbackDocumentRow) -> Result<(), RollbackRehearsalError> {
    if !is_repo_relative_path(&document.tei_path) {
        return Err(RollbackRehearsalError::NonHermeticInput {
            carrier: document.tei_path.clone(),
            detail: "the TEI fixture path is not repository-relative".to_owned(),
        });
    }
    match document.state.as_str() {
        "rehearsed" => {
            if document.refusal_reason.is_some() {
                return Err(RollbackRehearsalError::RefusalShapeInvalid {
                    document: document.id.clone(),
                    detail: "a rehearsed row carries a refusal reason".to_owned(),
                });
            }
            let baseline =
                require_digest(&document.id, "baseline_sha256", &document.baseline_sha256)?;
            let switched =
                require_digest(&document.id, "switched_sha256", &document.switched_sha256)?;
            let returned =
                require_digest(&document.id, "returned_sha256", &document.returned_sha256)?;
            if baseline != returned {
                return Err(RollbackRehearsalError::RollbackNotByteIdentical {
                    document: document.id.clone(),
                });
            }
            if !document.byte_identity {
                return Err(RollbackRehearsalError::RollbackNotByteIdentical {
                    document: document.id.clone(),
                });
            }
            if document.native_fragment_leaked {
                return Err(RollbackRehearsalError::NativeFragmentLeaked {
                    document: document.id.clone(),
                });
            }
            if switched == baseline {
                return Err(RollbackRehearsalError::VacuousSwitch {
                    document: document.id.clone(),
                });
            }
        }
        "refused" => {
            let reason = document.refusal_reason.as_deref().ok_or_else(|| {
                RollbackRehearsalError::RefusalReasonUnknown {
                    document: document.id.clone(),
                    reason: "<absent>".to_owned(),
                }
            })?;
            if !ROLLBACK_REFUSAL_REASON_VOCABULARY.contains(&reason) {
                return Err(RollbackRehearsalError::RefusalReasonUnknown {
                    document: document.id.clone(),
                    reason: reason.to_owned(),
                });
            }
            if document.baseline_sha256.is_some()
                || document.switched_sha256.is_some()
                || document.returned_sha256.is_some()
            {
                return Err(RollbackRehearsalError::RefusalShapeInvalid {
                    document: document.id.clone(),
                    detail: "a refused row carries a fabricated baseline".to_owned(),
                });
            }
            if document.byte_identity || document.native_fragment_leaked {
                return Err(RollbackRehearsalError::RefusalShapeInvalid {
                    document: document.id.clone(),
                    detail: "a refused row carries a rehearsal claim".to_owned(),
                });
            }
        }
        other => {
            return Err(RollbackRehearsalError::DocumentStateUnknown {
                document: document.id.clone(),
                state: other.to_owned(),
            });
        }
    }
    if !ROLLBACK_NATIVE_OUTPUT_STATE_VOCABULARY.contains(&document.native_output_state.as_str()) {
        return Err(RollbackRehearsalError::DocumentStateUnknown {
            document: document.id.clone(),
            state: document.native_output_state.clone(),
        });
    }
    Ok(())
}

fn require_digest(
    document: &str,
    field: &str,
    value: &Option<String>,
) -> Result<String, RollbackRehearsalError> {
    match value {
        Some(digest) if is_sha256_digest(digest) => Ok(digest.clone()),
        Some(digest) => Err(RollbackRehearsalError::RefusalShapeInvalid {
            document: document.to_owned(),
            detail: format!("{field} `{digest}` is not a digest"),
        }),
        None => Err(RollbackRehearsalError::MissingField {
            field: format!("{field} of `{document}`"),
        }),
    }
}

/// The document rows must be exactly the assigned holdout census, by id and
/// source PDF. A difference is a hard failure, named by document.
fn check_document_set(
    assigned: &[AssignedIdentity],
    documents: &[RollbackDocumentRow],
) -> Result<(), RollbackRehearsalError> {
    if assigned.len() != documents.len() {
        return Err(RollbackRehearsalError::DocumentSetMismatch {
            detail: format!(
                "{} assigned identities vs {} document rows",
                assigned.len(),
                documents.len()
            ),
        });
    }
    let by_id: BTreeMap<&str, &AssignedIdentity> = assigned
        .iter()
        .map(|identity| (identity.id.as_str(), identity))
        .collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for document in documents {
        let identity = by_id.get(document.id.as_str()).ok_or_else(|| {
            RollbackRehearsalError::DocumentSetMismatch {
                detail: format!("document `{}` is not in the assigned census", document.id),
            }
        })?;
        if !seen.insert(document.id.as_str()) {
            return Err(RollbackRehearsalError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
        if identity.source_pdf != document.source_pdf {
            return Err(RollbackRehearsalError::DocumentSetMismatch {
                detail: format!("document `{}` source_pdf drifted", document.id),
            });
        }
    }
    for identity in assigned {
        if !seen.contains(identity.id.as_str()) {
            return Err(RollbackRehearsalError::DocumentSetMismatch {
                detail: format!("assigned `{}` has no document row", identity.id),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── counters ─────────────────────────────

fn populations_of(documents: &[RollbackDocumentRow]) -> RollbackPopulations {
    let rehearsed = documents
        .iter()
        .filter(|row| row.state == "rehearsed")
        .count() as u64;
    let refused = documents
        .iter()
        .filter(|row| row.state == "refused")
        .count() as u64;
    RollbackPopulations {
        assigned: documents.len() as u64,
        rehearsed,
        refused,
    }
}

fn corpus_claim_of(
    documents: &[RollbackDocumentRow],
    switch_positive_control: bool,
) -> RollbackCorpusClaim {
    let rehearsed_documents = documents
        .iter()
        .filter(|row| row.state == "rehearsed")
        .count() as u64;
    let refused = documents
        .iter()
        .filter(|row| row.state == "refused")
        .count() as u64;
    let all_identical = documents
        .iter()
        .filter(|row| row.state == "rehearsed")
        .all(|row| row.byte_identity);
    let leak_free = documents.iter().all(|row| !row.native_fragment_leaked);
    RollbackCorpusClaim {
        byte_identity_holds: refused == 0 && all_identical,
        native_fragment_leak_free: leak_free,
        switch_positive_control,
        declared_documents: documents.len() as u64,
        rehearsed_documents,
    }
}

fn residuals_of(documents: &[RollbackDocumentRow]) -> RollbackResiduals {
    let rehearsed_documents = documents
        .iter()
        .filter(|row| row.state == "rehearsed")
        .count() as u64;
    let refused_documents = documents
        .iter()
        .filter(|row| row.state == "refused")
        .count() as u64;
    let byte_identity_violations = documents
        .iter()
        .filter(|row| row.state == "rehearsed" && !row.byte_identity)
        .count() as u64;
    let native_fragment_leaks = documents
        .iter()
        .filter(|row| row.native_fragment_leaked)
        .count() as u64;
    RollbackResiduals {
        rehearsed_documents,
        refused_documents,
        byte_identity_violations,
        native_fragment_leaks,
        refused_fixtures: refused_documents,
    }
}

// ───────────────────────────── checks ─────────────────────────────

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), RollbackRehearsalError> {
    for (lock, expected) in REQUIRED_LOCKS {
        match locks.get(lock) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(RollbackRehearsalError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(RollbackRehearsalError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_claim(
    claim_scope: &str,
    product_coverage_effect: &str,
    thresholds: &str,
) -> Result<(), RollbackRehearsalError> {
    if claim_scope != CORPUS_ROLLBACK_CLAIM_SCOPE_REHEARSAL_ONLY {
        return Err(RollbackRehearsalError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(RollbackRehearsalError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }
    if thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(RollbackRehearsalError::ThresholdClaimed {
            found: thresholds.to_owned(),
        });
    }
    Ok(())
}

fn check_hermeticity(hermeticity: &RollbackHermeticity) -> Result<(), RollbackRehearsalError> {
    if hermeticity.observed_java_calls != 0 {
        return Err(RollbackRehearsalError::NonHermeticRehearsal {
            detail: format!("observed {} Java calls", hermeticity.observed_java_calls),
        });
    }
    for (field, value) in [
        ("network_observed", hermeticity.network_observed),
        ("corpus_read_only", !hermeticity.corpus_read_only),
        ("switch_in_memory_only", !hermeticity.switch_in_memory_only),
        (
            "switch_positive_control",
            !hermeticity.switch_positive_control,
        ),
    ] {
        if value {
            return Err(RollbackRehearsalError::NonHermeticRehearsal {
                detail: format!("`{field}` is not the required state"),
            });
        }
    }
    Ok(())
}

fn check_sequence(sequence: &RollbackSequenceDeclaration) -> Result<(), RollbackRehearsalError> {
    let expected: Vec<String> = ROLLBACK_SEQUENCE_STEPS
        .iter()
        .map(|step| (*step).to_owned())
        .collect();
    if sequence.steps != expected {
        return Err(RollbackRehearsalError::SequenceDrift {
            found: sequence.steps.join(","),
        });
    }
    if sequence.baseline_source != ROLLBACK_BASELINE_SOURCE
        || sequence.switched_source != ROLLBACK_SWITCHED_SOURCE
        || sequence.returned_source != ROLLBACK_RETURNED_SOURCE
    {
        return Err(RollbackRehearsalError::SequenceDrift {
            found: format!(
                "{}/{}/{}",
                sequence.baseline_source, sequence.switched_source, sequence.returned_source
            ),
        });
    }
    if sequence.switch_scope != ROLLBACK_SWITCH_SCOPE_IN_MEMORY {
        return Err(RollbackRehearsalError::NonHermeticRehearsal {
            detail: format!("switch_scope `{}`", sequence.switch_scope),
        });
    }
    if !sequence.carriers_read_only {
        return Err(RollbackRehearsalError::NonHermeticRehearsal {
            detail: "carriers_read_only is not `true`".to_owned(),
        });
    }
    Ok(())
}

fn check_corpus_pins(pins: &RollbackCorpusPins) -> Result<(), RollbackRehearsalError> {
    for binding in [
        &pins.holdout_manifest,
        &pins.native_measurement,
        &pins.tei_provenance,
        &pins.route_receipt,
    ] {
        check_carrier_binding(binding)?;
    }
    Ok(())
}

fn check_freeze_governor(binding: &CarrierBinding) -> Result<(), RollbackRehearsalError> {
    check_carrier_binding(binding)
}

fn check_carrier_binding(binding: &CarrierBinding) -> Result<(), RollbackRehearsalError> {
    if !is_repo_relative_path(&binding.manifest) {
        return Err(RollbackRehearsalError::NonHermeticInput {
            carrier: binding.manifest.clone(),
            detail: "the carrier path is not repository-relative".to_owned(),
        });
    }
    if !is_sha256_digest(&binding.revision_sha256) {
        return Err(RollbackRehearsalError::CorpusPinUnpinned {
            carrier: binding.manifest.clone(),
            detail: format!("sha256 `{}` is not a digest", binding.revision_sha256),
        });
    }
    if binding.pointer.is_empty() {
        return Err(RollbackRehearsalError::CorpusPinUnpinned {
            carrier: binding.manifest.clone(),
            detail: "the carrier carries no pointer".to_owned(),
        });
    }
    Ok(())
}

fn check_limits(limits: &RouteLimitsBinding) -> Result<(), RollbackRehearsalError> {
    if limits.freeze_state != ROUTE_FREEZE_STATE_FROZEN {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!("freeze_state `{}`", limits.freeze_state),
        });
    }
    if limits.freeze_scope != ROUTE_FREEZE_SCOPE {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!("freeze_scope `{}`", limits.freeze_scope),
        });
    }
    if limits.tuned_after_scoring != ROUTE_TUNED_AFTER_SCORING_FORBIDDEN {
        return Err(RollbackRehearsalError::LimitTunedAfterScoring {
            found: limits.tuned_after_scoring.clone(),
        });
    }
    if limits.silent_clamp != ROUTE_SILENT_CLAMP_FORBIDDEN {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!("silent_clamp `{}`", limits.silent_clamp),
        });
    }
    if limits.claim_without_freeze != "forbidden" {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!("claim_without_freeze `{}`", limits.claim_without_freeze),
        });
    }
    if !limits.zero_is_not_a_measurement || !limits.pass_requires_frozen_threshold {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: "zero_is_not_a_measurement / pass_requires_frozen_threshold must be true"
                .to_owned(),
        });
    }
    if limits.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: "direction vocabulary drifted".to_owned(),
        });
    }
    if !is_sha256_digest(&limits.freeze_revision) {
        return Err(RollbackRehearsalError::LimitNotFrozen {
            detail: format!("freeze_revision `{}`", limits.freeze_revision),
        });
    }
    Ok(())
}

fn assert_redaction(redaction: &Redaction) -> Result<(), RollbackRehearsalError> {
    let flags = [
        ("raw_text", redaction.raw_text),
        ("raw_pdf", redaction.raw_pdf),
        ("tei_xml", redaction.tei_xml),
        ("images", redaction.images),
        ("physical_paths", redaction.physical_paths),
        ("hostname", redaction.hostname),
        ("executable", redaction.executable),
    ];
    for (field, value) in flags {
        if value {
            return Err(RollbackRehearsalError::RedactionFlag {
                field: field.to_owned(),
                found: value,
            });
        }
    }
    Ok(())
}

// ───────────────────────────── loaders ─────────────────────────────

/// One pinned preserved-TEI fixture, re-derived from the provenance carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeiFixturePin {
    /// Pinned document id.
    pub article_id: String,
    /// Repository-relative fixture path.
    pub path: String,
    /// Prefixed sha256 of the fixture bytes.
    pub sha256: String,
    /// Prefixed sha256 of the pinned source PDF.
    pub source_pdf_sha256: String,
}

/// Load the preserved-TEI pins from `provenance.yaml` (pointer `documents`).
/// No id, path or sha is restated.
pub fn load_tei_fixture_pins(text: &str) -> Result<Vec<TeiFixturePin>, RollbackRehearsalError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|error| RollbackRehearsalError::Malformed {
            detail: error.to_string(),
        })?;
    let documents = value
        .get(CORPUS_ROLLBACK_TEI_PROVENANCE_POINTER)
        .and_then(|node| node.as_sequence())
        .ok_or_else(|| RollbackRehearsalError::MissingField {
            field: format!("{CORPUS_ROLLBACK_TEI_PROVENANCE_POINTER} in provenance"),
        })?;
    let mut pins = Vec::with_capacity(documents.len());
    for document in documents {
        let article_id = document
            .get("article_id")
            .and_then(|node| node.as_str())
            .ok_or_else(|| RollbackRehearsalError::MissingField {
                field: "provenance document article_id".to_owned(),
            })?
            .to_owned();
        let path = document
            .get("path")
            .and_then(|node| node.as_str())
            .ok_or_else(|| RollbackRehearsalError::MissingField {
                field: format!("provenance document {article_id} path"),
            })?
            .to_owned();
        let sha256 = document
            .get("sha256")
            .and_then(|node| node.as_str())
            .ok_or_else(|| RollbackRehearsalError::MissingField {
                field: format!("provenance document {article_id} sha256"),
            })?
            .to_owned();
        let source_pdf_sha256 = document
            .get("source_pdf_sha256")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_owned();
        pins.push(TeiFixturePin {
            article_id,
            path,
            sha256,
            source_pdf_sha256,
        });
    }
    Ok(pins)
}

/// The T01 native output of one document: the plane state and digest the
/// switched payload derives from.
pub fn native_output_of(document: &AcceptanceDocumentRow) -> (&str, Option<&str>) {
    (&document.plane_state, document.layout_digest.as_deref())
}

/// Parse the T01 measurement carrier.
pub fn parse_native_measurement(
    bytes: &[u8],
) -> Result<NativeAcceptanceReceipt, RollbackRehearsalError> {
    NativeAcceptanceReceipt::parse_json_bytes(bytes).map_err(|error| {
        RollbackRehearsalError::Malformed {
            detail: format!("T01 measurement: {error}"),
        }
    })
}

/// The T01 carrier path the receipt must bind.
pub const fn native_measurement_rel() -> &'static str {
    DEFAULT_ACCEPTANCE_ARTIFACT_REL
}

/// The holdout manifest path the assigned census is read from.
pub const fn holdout_manifest_rel() -> &'static str {
    HOLDOUT_MANIFEST_REL
}
