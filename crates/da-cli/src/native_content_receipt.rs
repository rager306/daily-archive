//! S05/T06 (M514-1lss2s) — corpus-bound content receipt for the native body,
//! section / reading-order, visual and callout bands.
//!
//! S04 established the receipt *envelope* in
//! [`crate::native_family_receipt`]: `claim_scope`/`product_coverage_effect`/
//! `thresholds`, the closed `locks`, a `declaration` carrier binding, a guard
//! provenance row, an independent-oracle identity, typed populations, a pooling
//! separation flag and a redaction block. S05 measures a different thing — the
//! document *body* and its visual semantics — but the evidence must reach the
//! governor in the same shape, so this module **reuses** the envelope types
//! (`CarrierBinding`, `GuardProvenanceReceipt`, `Redaction`, `REQUIRED_LOCKS`,
//! `LEAK_MARKERS`, the claim-scope constants) rather than restating them. It owns
//! only the content-specific vocabulary:
//!
//! * [`ContentFamily`] — `figure`/`table`/`formula`/`callout_marker`/
//!   `body_section`/`body_reading_order`;
//! * [`ContentPopulationId`] — the group strata `digital_text`/`scanned_image`/
//!   `mixed`/`unknown`/`bytes_absent`;
//! * [`ContentBodyAxis`] — the T01 character verdict, published as its own axis.
//!
//! ## Two vocabularies this module does **not** restate
//!
//! The required-family disposition and the unmet reason are the closed
//! vocabularies of [`crate::native_document_group`] (S05/T05). They are carried
//! as *tokens* and parsed back through
//! [`RequiredFamilyDisposition::parse`] /
//! [`RequiredFamilyUnmetReason::from_token`]. That indirection is the whole
//! point: a builder always writes a token the closed vocabulary produced, while
//! a **tampered** receipt can carry `out_of_scope`, and the validator refuses it
//! *by name* ([`ContentReceiptError::RequiredFamilyOutOfScope`]) instead of a
//! generic serde error. `unmet` without a reason is likewise refused by
//! [`RequiredFamilyDisposition::parse`].
//!
//! ## Group-stratified, never pooled
//!
//! [`ContentReceipt::populations`] carries one population **per document group**
//! present in the declared slice, plus a `bytes_absent` stratum for declared
//! documents whose bytes were absent. [`validate_content_receipt`] refuses a
//! receipt whose populations do not stratify exactly that way
//! ([`ContentReceiptError::PoolingClaimed`]), so a required family can never be
//! hidden by folding a scanned document into a digital one.
//!
//! ## Claim scope is not negotiable
//!
//! The receipt is diagnostic only: `claim_scope = diagnostic_only`,
//! `product_coverage_effect = none`, `thresholds = not_accepted`, and the five
//! required locks closed. Nothing here promotes, splices, imports or writes a
//! graph (D127 / ADR-076 §14 / D499). This module is pure `std`/`serde`/`sha2`,
//! so it and its contract test compile in the default feature set.

use crate::native_document_group::{
    DocumentGroup, DocumentGroupError, DocumentGroupEvidence, DocumentGroupPartition,
    RequiredFamilyDisposition, RequiredFamilyUnmetReason,
};
use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, CarrierBinding, DECLARATION_MANIFEST_REL, GuardObserver,
    GuardProvenance, GuardProvenanceReceipt, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE,
    REQUIRED_LOCKS, Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest,
    is_sha256_hex, normalize_document_identity, sha256_hex, sha256_prefixed,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted content receipt artifact.
pub const NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION: &str = "native-content-receipt.v1";
/// Default artifact path of the content receipt.
pub const DEFAULT_CONTENT_ARTIFACT_REL: &str = "artifacts/parse_compat/native_content_receipt.json";
/// Pointer to the slice declaration block inside the declaration carrier.
pub const CONTENT_DECLARATION_POINTER: &str = "java_free_vertical_slice";
/// Pointer to the pinned corpus documents.
pub const CONTENT_CORPUS_POINTER: &str = "corpus.documents";
/// Repository-relative carrier of the preserved-TEI oracle pins (the same
/// carrier S04 binds; this receipt never re-pins it).
pub const CONTENT_ORACLE_MANIFEST_REL: &str =
    "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
/// Repository-relative carrier of the reviewed segmentation oracle manifest.
pub const SEGMENTATION_ORACLE_MANIFEST_REL: &str = "data/parse/document_segmentation.yaml";
/// Repository-relative reviewed segmentation oracle source artifact.
pub const SEGMENTATION_ORACLE_SOURCE_REL: &str =
    "artifacts/parse_compat/document_segmentation_oracle_source.json";

/// The closed content-family vocabulary, in canonical (matrix) order.
pub const CONTENT_FAMILY_ORDER: [ContentFamily; 6] = [
    ContentFamily::Figure,
    ContentFamily::Table,
    ContentFamily::Formula,
    ContentFamily::CalloutMarker,
    ContentFamily::BodySection,
    ContentFamily::BodyReadingOrder,
];

/// The closed population-stratum vocabulary, in canonical order.
pub const CONTENT_POPULATION_ORDER: [ContentPopulationId; 5] = [
    ContentPopulationId::DigitalText,
    ContentPopulationId::ScannedImage,
    ContentPopulationId::Mixed,
    ContentPopulationId::Unknown,
    ContentPopulationId::BytesAbsent,
];

/// The closed character-verdict vocabulary of the body axis. It mirrors the T01
/// `BodyCoverageVerdict` and is carried as a token so the receipt can name it
/// without depending on `da-parse`.
pub const CONTENT_BODY_VERDICT_VOCABULARY: [&str; 3] =
    ["granularity_only", "content_loss", "not_assessable"];

/// The closed body-gap reason vocabulary. It mirrors the T01 `BodyGapReason`
/// unit tags; the receipt carries the tag, never the T01 type.
pub const CONTENT_BODY_REASON_VOCABULARY: [&str; 10] = [
    "matched",
    "p1_native_text_absent",
    "p1_localization_uncertain",
    "p2_region_assignment_gap",
    "p2_localization_uncertain",
    "scope_mismatch",
    "scope_join_mismatch",
    "normalization_mismatch",
    "oracle_empty",
    "population_incomplete",
];

// ───────────────────────────── vocabulary ─────────────────────────────

/// One required content family. `Serde` rejects any other token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFamily {
    /// A figure head.
    Figure,
    /// A table head.
    Table,
    /// A formula head.
    Formula,
    /// A callout marker (citation / figure / table / equation).
    CalloutMarker,
    /// Section runs inside the section-bearing regions.
    BodySection,
    /// The admitted reading order over the closed nine-region vocabulary.
    BodyReadingOrder,
}

impl ContentFamily {
    /// The stable `snake_case` token this family serializes to.
    pub fn token(&self) -> &'static str {
        match self {
            ContentFamily::Figure => "figure",
            ContentFamily::Table => "table",
            ContentFamily::Formula => "formula",
            ContentFamily::CalloutMarker => "callout_marker",
            ContentFamily::BodySection => "body_section",
            ContentFamily::BodyReadingOrder => "body_reading_order",
        }
    }
}

/// One population stratum. A stratum is a `DocumentGroup` or the typed-absence
/// stratum for declared documents whose bytes were absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentPopulationId {
    /// Digital-born documents with a text layer.
    DigitalText,
    /// Image-only / scanned documents, no text layer.
    ScannedImage,
    /// Some pages carry text, some do not.
    Mixed,
    /// The classifier ran and could not decide.
    Unknown,
    /// Declared bytes were absent, so no group exists.
    BytesAbsent,
}

impl ContentPopulationId {
    /// The stable `snake_case` token this stratum serializes to.
    pub fn token(&self) -> &'static str {
        match self {
            ContentPopulationId::DigitalText => "digital_text",
            ContentPopulationId::ScannedImage => "scanned_image",
            ContentPopulationId::Mixed => "mixed",
            ContentPopulationId::Unknown => "unknown",
            ContentPopulationId::BytesAbsent => "bytes_absent",
        }
    }

    /// The stratum a `DocumentGroup` belongs to.
    pub fn of_group(group: DocumentGroup) -> Self {
        match group {
            DocumentGroup::DigitalText => ContentPopulationId::DigitalText,
            DocumentGroup::ScannedImage => ContentPopulationId::ScannedImage,
            DocumentGroup::Mixed => ContentPopulationId::Mixed,
            DocumentGroup::Unknown => ContentPopulationId::Unknown,
        }
    }

    /// The stratum a piece of group evidence belongs to. A typed absence lands
    /// in [`ContentPopulationId::BytesAbsent`], never in `unknown`.
    pub fn of_evidence(evidence: &DocumentGroupEvidence) -> Self {
        match evidence.assigned_group() {
            Some(group) => ContentPopulationId::of_group(group),
            None => ContentPopulationId::BytesAbsent,
        }
    }
}

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals of the content receipt. Every variant is a refusal, never a
/// silent downgrade. The first five after the schema/guard group are the
/// receipt-level refusals a builder cannot express and a validator must:
/// `ThresholdClaimed`, `PoolingClaimed`, `BindingDrift`,
/// `RequiredFamilyOutOfScope` and `OracleVacuous`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentReceiptError {
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// A lock the receipt never relaxes was relaxed.
    LockDrift {
        lock: String,
        expected: String,
        found: String,
    },
    /// A required lock is absent.
    LockMissing { lock: String },
    /// The zero was claimed without an installed guard.
    GuardNotInstalled,
    /// The guard would have rotated a cache key, so its zero is unusable.
    GuardIdentityNotIsolated,
    /// Java was consulted during the run.
    ObservedJavaOnSlice { attempts: u64 },
    /// The observed seam is not the Java transport seam.
    GuardObserverDrift { found: String },
    /// The oracle is the SUT's own generator, or independence was not proven.
    OracleNotIndependent,
    /// The oracle's same-generator negative control was not executed.
    OracleIndependenceUnrefuted,
    /// A required family's oracle carries no positive row anywhere, so
    /// agreement with the native lane's absence would be vacuous.
    OracleVacuous { family: String },
    /// The receipt pooled group strata that must stay separate.
    PoolingClaimed { field: String },
    /// A carrier binding drifted from the pointer the contract requires.
    BindingDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The required-family disposition refused the forbidden `out_of_scope`.
    RequiredFamilyOutOfScope { token: String },
    /// A required family is missing from a document's matrix, or appears twice.
    FamilyMatrixIncomplete { document: String, family: String },
    /// A family cell's disposition, reason and count contradict each other.
    FamilyCellInconsistent {
        document: String,
        family: String,
        detail: String,
    },
    /// A family token is outside the closed vocabulary.
    UnknownFamily { family: String },
    /// The typed partition is not total over the declared corpus.
    PartitionInvalid { document: String },
    /// The group-stratified populations do not partition the declared corpus.
    PopulationStratificationInvalid { detail: String },
    /// The same document appears twice.
    DuplicateDocument { document: String },
    /// The run observed a document the declaration does not pin.
    UnknownDocument { document: String },
    /// The declaration pins a document the receipt never observed.
    MissingDocument { document: String },
    /// The observed sha256 drifted from the pin.
    DocumentShaDrift {
        document: String,
        expected: String,
        found: String,
    },
    /// The observed page count drifted from the pin.
    PageCountDrift {
        document: String,
        expected: u64,
        found: u64,
    },
    /// A declared document has no oracle fixture.
    MissingOracleDocument { document: String },
    /// An oracle fixture names a document the declaration never pins.
    UnknownOracleDocument { document: String },
    /// A digest leaf is not a `sha256:<hex>` string.
    InvalidDigest { field: String },
    /// The body axis carries a token outside its closed vocabulary.
    InvalidBodyVocabulary { field: String, token: String },
    /// A path leaf is absolute or escapes the repository.
    InvalidPath { field: String },
    /// The artifact leaked a payload marker.
    RedactionViolation { marker: String },
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
    /// The corpus pin's document count or page denominator drifted.
    CorpusPinDrift { expected: u64, found: u64 },
}

impl fmt::Display for ContentReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentReceiptError::SchemaDrift {
                field,
                expected,
                found,
            } => write!(f, "`{field}` is `{found}`, expected `{expected}`"),
            ContentReceiptError::ClaimScopeDrift { found } => write!(
                f,
                "claim_scope is `{found}`; only `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}` may be emitted"
            ),
            ContentReceiptError::ProductCoverageClaimed { found } => write!(
                f,
                "product_coverage_effect is `{found}`; only `{PRODUCT_COVERAGE_EFFECT_NONE}` may be emitted"
            ),
            ContentReceiptError::ThresholdClaimed { found } => write!(
                f,
                "thresholds are `{found}`; only `{THRESHOLDS_NOT_ACCEPTED}` may be emitted"
            ),
            ContentReceiptError::LockDrift {
                lock,
                expected,
                found,
            } => write!(f, "lock `{lock}` is `{found}`, expected `{expected}`"),
            ContentReceiptError::LockMissing { lock } => write!(f, "lock `{lock}` is absent"),
            ContentReceiptError::GuardNotInstalled => {
                write!(f, "a counted zero needs an installed guard")
            }
            ContentReceiptError::GuardIdentityNotIsolated => {
                write!(f, "the guard would have rotated the cache identity")
            }
            ContentReceiptError::ObservedJavaOnSlice { attempts } => {
                write!(f, "Java was consulted {attempts} time(s) on the slice")
            }
            ContentReceiptError::GuardObserverDrift { found } => write!(
                f,
                "guard observer is `{found}`, expected the Java transport seam"
            ),
            ContentReceiptError::OracleNotIndependent => {
                write!(f, "the oracle is not independent of the SUT")
            }
            ContentReceiptError::OracleIndependenceUnrefuted => {
                write!(f, "the same-generator negative control was not executed")
            }
            ContentReceiptError::OracleVacuous { family } => write!(
                f,
                "required family `{family}` has no positive oracle row: the oracle is vacuous"
            ),
            ContentReceiptError::PoolingClaimed { field } => {
                write!(f, "`{field}` pools group strata that must stay separate")
            }
            ContentReceiptError::BindingDrift {
                field,
                expected,
                found,
            } => write!(f, "`{field}` is `{found}`, expected `{expected}`"),
            ContentReceiptError::RequiredFamilyOutOfScope { token } => write!(
                f,
                "required-family disposition refuses `{token}`: the vocabulary has no out-of-scope arm"
            ),
            ContentReceiptError::FamilyMatrixIncomplete { document, family } => write!(
                f,
                "document `{document}` has no single cell for family `{family}`"
            ),
            ContentReceiptError::FamilyCellInconsistent {
                document,
                family,
                detail,
            } => write!(f, "document `{document}` family `{family}`: {detail}"),
            ContentReceiptError::UnknownFamily { family } => {
                write!(
                    f,
                    "`{family}` is not a member of the content-family vocabulary"
                )
            }
            ContentReceiptError::PartitionInvalid { document } => write!(
                f,
                "the group partition is not total: document `{document}` is inconsistent"
            ),
            ContentReceiptError::PopulationStratificationInvalid { detail } => {
                write!(f, "the group strata do not partition the corpus: {detail}")
            }
            ContentReceiptError::DuplicateDocument { document } => {
                write!(f, "document `{document}` appears more than once")
            }
            ContentReceiptError::UnknownDocument { document } => {
                write!(f, "document `{document}` was observed but is not declared")
            }
            ContentReceiptError::MissingDocument { document } => {
                write!(f, "declared document `{document}` was not observed")
            }
            ContentReceiptError::DocumentShaDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "document `{document}` sha256 is `{found}`, pinned `{expected}`"
            ),
            ContentReceiptError::PageCountDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "document `{document}` page count is {found}, pinned {expected}"
            ),
            ContentReceiptError::MissingOracleDocument { document } => {
                write!(f, "declared document `{document}` has no oracle fixture")
            }
            ContentReceiptError::UnknownOracleDocument { document } => {
                write!(f, "oracle fixture `{document}` is not a declared document")
            }
            ContentReceiptError::InvalidDigest { field } => {
                write!(f, "`{field}` is not a sha256 digest")
            }
            ContentReceiptError::InvalidBodyVocabulary { field, token } => {
                write!(
                    f,
                    "body axis `{field}` carries `{token}`, outside its vocabulary"
                )
            }
            ContentReceiptError::InvalidPath { field } => {
                write!(f, "`{field}` is not a repository-relative path")
            }
            ContentReceiptError::RedactionViolation { marker } => {
                write!(f, "the artifact leaked marker `{marker}`")
            }
            ContentReceiptError::ArtifactSerialization { reason } => {
                write!(f, "the artifact could not be serialized: {reason}")
            }
            ContentReceiptError::CorpusPinDrift { expected, found } => {
                write!(
                    f,
                    "the corpus pin reports {found}, the documents sum to {expected}"
                )
            }
        }
    }
}

impl std::error::Error for ContentReceiptError {}

fn document_group_error(context: &str, error: DocumentGroupError) -> ContentReceiptError {
    match error {
        DocumentGroupError::RequiredFamilyOutOfScope { token } => {
            ContentReceiptError::RequiredFamilyOutOfScope { token }
        }
        other => ContentReceiptError::FamilyCellInconsistent {
            document: context.to_owned(),
            family: String::new(),
            detail: other.to_string(),
        },
    }
}

// ───────────────────────── receipt types ─────────────────────────

/// One family's typed outcome for one document.
///
/// `disposition` and `unmet_reason` are the closed-vocabulary *tokens* of
/// [`crate::native_document_group`]. They are strings, not the typed enum, so a
/// tampered artifact can carry `out_of_scope` and the validator can refuse it by
/// name instead of failing to deserialize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFamilyCell {
    /// The family this cell measures.
    pub family: ContentFamily,
    /// `measured` | `unmet` | `not_observed`.
    pub disposition: String,
    /// The typed unmet reason token, present exactly when `disposition` is
    /// `unmet`.
    pub unmet_reason: Option<String>,
    /// Measured native item count. Present exactly when `measured`.
    pub observed_count: Option<u64>,
    /// Positive oracle rows the independent oracle carries for this
    /// document × family. Zero means "nothing to meet", never "met".
    pub oracle_positive_rows: u64,
    /// sha256 over the band's normalized observation, when one exists.
    pub observed_digest: Option<String>,
}

impl ContentFamilyCell {
    /// Build a `measured` cell.
    pub fn measured(
        family: ContentFamily,
        observed_count: u64,
        oracle_positive_rows: u64,
        observed_digest: Option<String>,
    ) -> Self {
        Self {
            family,
            disposition: RequiredFamilyDisposition::Measured.token().to_owned(),
            unmet_reason: None,
            observed_count: Some(observed_count),
            oracle_positive_rows,
            observed_digest,
        }
    }

    /// Build an `unmet` cell with a typed reason from the closed vocabulary.
    pub fn unmet(
        family: ContentFamily,
        reason: RequiredFamilyUnmetReason,
        oracle_positive_rows: u64,
    ) -> Self {
        Self {
            family,
            disposition: RequiredFamilyDisposition::Unmet(reason).token().to_owned(),
            unmet_reason: Some(reason.token().to_owned()),
            observed_count: None,
            oracle_positive_rows,
            observed_digest: None,
        }
    }

    /// Build a `not_observed` cell.
    pub fn not_observed(family: ContentFamily, oracle_positive_rows: u64) -> Self {
        Self {
            family,
            disposition: RequiredFamilyDisposition::NotObserved.token().to_owned(),
            unmet_reason: None,
            observed_count: None,
            oracle_positive_rows,
            observed_digest: None,
        }
    }
}

/// The T01 character axis for one document, published beside the family matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBodyAxis {
    /// The P1 plane-preservation verdict token.
    pub plane_preservation: String,
    /// Whether the P1 verdict requires a recorded D498 engine decision.
    pub plane_d498_reopen_required: bool,
    /// Unresolved P1 occurrences sitting inside one native token.
    pub plane_present_in_token_units: u64,
    /// Unresolved P1 occurrences spanning token boundaries.
    pub plane_spanning_token_units: u64,
    /// Unresolved P1 occurrences existing only as character material.
    pub plane_character_material_units: u64,
    /// Oracle occurrences with no character material in the plane.
    pub plane_absent_units: u64,
    /// The P2 body-region verdict token.
    pub body_region: String,
    /// The typed body-gap reason token.
    pub body_reason: String,
    /// Native body token count.
    pub body_token_count: u64,
    /// Oracle lexical units.
    pub oracle_units: u64,
    /// Native plane lexical units.
    pub native_plane_units: u64,
}

/// One pinned document identity inside a population.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentDocumentIdentity {
    pub id: String,
    pub source_pdf: String,
    pub sha256: String,
    pub pages: u64,
}

/// One document's full typed row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentDocumentReceipt {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256 of the pinned bytes.
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
    /// Pages the layout donor observed.
    pub observed_pages: u64,
    /// The T05 group assignment (measured verdict or typed absence).
    pub group: DocumentGroupEvidence,
    /// One cell per family, in canonical family order.
    pub families: Vec<ContentFamilyCell>,
    /// The T01 character axis.
    pub body: ContentBodyAxis,
    /// sha256 over the native pipeline digests.
    pub pipeline_digest: String,
    /// sha256 over the normalized oracle values.
    pub oracle_digest: String,
}

/// One group stratum. The matrix of a stratum contains only the documents of
/// that stratum, so the populations as a whole stratify the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentPopulationReceipt {
    /// The stratum.
    pub population: ContentPopulationId,
    /// Pinned document count in this stratum.
    pub document_count: u64,
    /// Pinned page denominator of this stratum.
    pub page_denominator: u64,
    /// Pinned identities, in carrier order.
    pub documents: Vec<ContentDocumentIdentity>,
    /// One typed row per stratum document, in carrier order.
    pub matrix: Vec<ContentDocumentReceipt>,
}

/// One oracle fixture binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentOracleDocument {
    /// Pinned corpus document id the fixture belongs to.
    pub document_id: String,
    /// Repository-relative preserved-TEI path.
    pub fixture_path: String,
    /// Bare sha256 pin of the preserved bytes.
    pub sha256: String,
    /// sha256 over the normalized oracle values.
    pub oracle_digest: String,
}

/// The independent-oracle identity and the reviewed segmentation binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentOracleReceipt {
    /// Identity of the oracle builder (must differ from the native bands).
    pub generator_id: String,
    /// The backend that produced the preserved bytes.
    pub backend: String,
    /// Repository-relative carrier of the preserved-TEI pins.
    pub manifest: String,
    /// sha256 of the pin carrier bytes.
    pub revision_sha256: String,
    /// Proven by the independence requirements, not asserted.
    pub independent: bool,
    /// The same-generator negative control was executed.
    pub same_generator_refused: bool,
    /// Repository-relative reviewed segmentation oracle manifest.
    pub segmentation_manifest: String,
    /// sha256 of the reviewed segmentation manifest bytes.
    pub segmentation_revision_sha256: String,
    /// Repository-relative reviewed segmentation source artifact.
    pub segmentation_source: String,
    /// sha256 of the reviewed segmentation source bytes, recomputed.
    pub segmentation_source_sha256: String,
    /// One binding per pinned corpus document.
    pub documents: Vec<ContentOracleDocument>,
}

/// Group-strata separation: the receipt never pools one stratum's population
/// with another's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentPoolingReceipt {
    /// Always `false`; `true` is refused as [`ContentReceiptError::PoolingClaimed`].
    pub group_strata_pooled: bool,
}

/// The emitted content receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Claim scope; only `diagnostic_only` is emitted.
    pub claim_scope: String,
    /// Product-coverage effect; only `none` is emitted.
    pub product_coverage_effect: String,
    /// Thresholds; only `not_accepted` is emitted.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// Binding to the slice declaration carrier.
    pub declaration: CarrierBinding,
    /// Guard provenance.
    pub guard: GuardProvenanceReceipt,
    /// Independent-oracle identity.
    pub oracle: ContentOracleReceipt,
    /// The total group partition.
    pub partition: DocumentGroupPartition,
    /// The group-stratified populations.
    pub populations: Vec<ContentPopulationReceipt>,
    /// Group-strata separation.
    pub pooling: ContentPoolingReceipt,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl ContentReceipt {
    /// Serialize the receipt canonically (pretty JSON, deterministic field and
    /// key order) and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, ContentReceiptError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            ContentReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            ContentReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_content_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, ContentReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| ContentReceiptError::ArtifactSerialization {
            reason: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_content_redacted(json: &str) -> Result<(), ContentReceiptError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(ContentReceiptError::RedactionViolation {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────── builder inputs ─────────────────────────

/// One declared document, as the declaration pins it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDeclaredDocument {
    pub id: String,
    pub source_pdf: String,
    pub sha256: String,
    pub pages: u64,
}

/// The validated corpus pin the receipt is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDeclaredCorpus {
    pub manifest: String,
    pub pointer: String,
    pub revision_sha256: String,
    pub corpus_id: String,
    pub document_count: u64,
    pub page_denominator: u64,
    pub documents: Vec<ContentDeclaredDocument>,
}

/// One document's observed pipeline output, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentObservedDocument {
    pub id: String,
    pub sha256: String,
    pub pages: u64,
    pub observed_pages: u64,
    pub group: DocumentGroupEvidence,
    pub families: Vec<ContentFamilyCell>,
    pub body: ContentBodyAxis,
    pub pipeline_digest: String,
    pub oracle_digest: String,
}

/// The independent oracle, as the runner produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentOracleInput {
    pub generator_id: String,
    pub backend: String,
    pub manifest: String,
    pub revision_sha256: String,
    pub independent: bool,
    pub same_generator_refused: bool,
    pub segmentation_manifest: String,
    pub segmentation_revision_sha256: String,
    pub segmentation_source: String,
    pub segmentation_source_sha256: String,
    pub documents: Vec<ContentOracleDocument>,
}

// ───────────────────────── builder ─────────────────────────

/// Build a canonical content receipt.
///
/// The builder enforces every structural invariant it can express: claim scope,
/// product coverage, locks, an installed identity-isolated guard with zero
/// observed Java calls, the corpus pin, exact observed coverage, a total group
/// partition, a complete family matrix, and the group-stratified populations.
/// Receipt-level mutations a builder cannot express (a tampered threshold, a
/// pooled population, an `out_of_scope` token, a vacuous oracle) are refused by
/// [`validate_content_receipt`].
#[allow(clippy::too_many_arguments)]
pub fn build_content_receipt(
    declaration_revision_sha256: &str,
    corpus: &ContentDeclaredCorpus,
    locks: &BTreeMap<String, String>,
    claim_scope: &str,
    product_coverage_effect: &str,
    guard: GuardProvenance,
    oracle: &ContentOracleInput,
    partition: &DocumentGroupPartition,
    observed: &[ContentObservedDocument],
) -> Result<ContentReceipt, ContentReceiptError> {
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(ContentReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(ContentReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }

    let mut checked_locks = BTreeMap::new();
    for (lock, expected) in REQUIRED_LOCKS {
        let found = locks
            .get(lock)
            .ok_or_else(|| ContentReceiptError::LockMissing {
                lock: lock.to_owned(),
            })?;
        if found != expected {
            return Err(ContentReceiptError::LockDrift {
                lock: lock.to_owned(),
                expected: expected.to_owned(),
                found: found.clone(),
            });
        }
        checked_locks.insert(lock.to_owned(), found.clone());
    }

    if !guard.installed {
        return Err(ContentReceiptError::GuardNotInstalled);
    }
    if !guard.identity_isolated {
        return Err(ContentReceiptError::GuardIdentityNotIsolated);
    }
    if guard.observed_java_calls > 0 {
        return Err(ContentReceiptError::ObservedJavaOnSlice {
            attempts: guard.observed_java_calls,
        });
    }

    // Declaration binding.
    require_path("declaration.manifest", &corpus.manifest)?;
    if corpus.manifest != DECLARATION_MANIFEST_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: DECLARATION_MANIFEST_REL.to_owned(),
            found: corpus.manifest.clone(),
        });
    }
    if corpus.pointer != CONTENT_CORPUS_POINTER {
        return Err(ContentReceiptError::BindingDrift {
            field: "declaration.pointer".to_owned(),
            expected: CONTENT_CORPUS_POINTER.to_owned(),
            found: corpus.pointer.clone(),
        });
    }
    require_digest("declaration.revision_sha256", declaration_revision_sha256)?;
    require_digest("corpus.revision_sha256", &corpus.revision_sha256)?;
    if corpus.documents.len() as u64 != corpus.document_count {
        return Err(ContentReceiptError::CorpusPinDrift {
            expected: corpus.documents.len() as u64,
            found: corpus.document_count,
        });
    }
    let page_sum: u64 = corpus.documents.iter().map(|doc| doc.pages).sum();
    if page_sum != corpus.page_denominator {
        return Err(ContentReceiptError::CorpusPinDrift {
            expected: page_sum,
            found: corpus.page_denominator,
        });
    }
    if corpus.corpus_id.trim().is_empty() {
        return Err(ContentReceiptError::SchemaDrift {
            field: "corpus.corpus_id".to_owned(),
            expected: "a non-empty corpus id".to_owned(),
            found: "empty".to_owned(),
        });
    }

    // Oracle identity.
    if !oracle.independent {
        return Err(ContentReceiptError::OracleNotIndependent);
    }
    if !oracle.same_generator_refused {
        return Err(ContentReceiptError::OracleIndependenceUnrefuted);
    }
    require_path("oracle.manifest", &oracle.manifest)?;
    if oracle.manifest != CONTENT_ORACLE_MANIFEST_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.manifest".to_owned(),
            expected: CONTENT_ORACLE_MANIFEST_REL.to_owned(),
            found: oracle.manifest.clone(),
        });
    }
    require_digest("oracle.revision_sha256", &oracle.revision_sha256)?;
    require_path("oracle.segmentation_source", &oracle.segmentation_source)?;
    require_digest(
        "oracle.segmentation_revision_sha256",
        &oracle.segmentation_revision_sha256,
    )?;
    require_digest(
        "oracle.segmentation_source_sha256",
        &oracle.segmentation_source_sha256,
    )?;

    // The oracle fixture set is exactly the pinned corpus.
    let mut oracle_by_key: BTreeMap<String, &ContentOracleDocument> = BTreeMap::new();
    for fixture in &oracle.documents {
        require_path("oracle.fixture_path", &fixture.fixture_path)?;
        if !is_sha256_hex(&fixture.sha256) {
            return Err(ContentReceiptError::InvalidDigest {
                field: format!("oracle.fixture[{}].sha256", fixture.document_id),
            });
        }
        require_digest(
            &format!("oracle.fixture[{}].oracle_digest", fixture.document_id),
            &fixture.oracle_digest,
        )?;
        let key = normalize_document_identity(&fixture.document_id);
        if oracle_by_key.insert(key, fixture).is_some() {
            return Err(ContentReceiptError::DuplicateDocument {
                document: fixture.document_id.clone(),
            });
        }
    }
    for declared in &corpus.documents {
        if !oracle_by_key.contains_key(&normalize_document_identity(&declared.id)) {
            return Err(ContentReceiptError::MissingOracleDocument {
                document: declared.id.clone(),
            });
        }
    }
    for fixture in &oracle.documents {
        let key = normalize_document_identity(&fixture.document_id);
        if !corpus
            .documents
            .iter()
            .any(|declared| normalize_document_identity(&declared.id) == key)
        {
            return Err(ContentReceiptError::UnknownOracleDocument {
                document: fixture.document_id.clone(),
            });
        }
    }

    // Observed coverage: every pinned document exactly once, pinned identity.
    let mut observed_by_id: BTreeMap<&str, &ContentObservedDocument> = BTreeMap::new();
    for document in observed {
        let declared = corpus
            .documents
            .iter()
            .find(|declared| declared.id == document.id)
            .ok_or_else(|| ContentReceiptError::UnknownDocument {
                document: document.id.clone(),
            })?;
        if document.sha256 != declared.sha256 {
            return Err(ContentReceiptError::DocumentShaDrift {
                document: document.id.clone(),
                expected: declared.sha256.clone(),
                found: document.sha256.clone(),
            });
        }
        if document.pages != declared.pages || document.observed_pages != declared.pages {
            return Err(ContentReceiptError::PageCountDrift {
                document: document.id.clone(),
                expected: declared.pages,
                found: document.observed_pages,
            });
        }
        require_path("document.source_pdf", &declared.source_pdf)?;
        if !is_sha256_hex(&document.sha256) {
            return Err(ContentReceiptError::InvalidDigest {
                field: format!("document[{}].sha256", document.id),
            });
        }
        require_digest(
            &format!("document[{}].pipeline_digest", document.id),
            &document.pipeline_digest,
        )?;
        require_digest(
            &format!("document[{}].oracle_digest", document.id),
            &document.oracle_digest,
        )?;
        validate_body_axis(&document.id, &document.body)?;
        // The group evidence must be self-consistent and must belong to this
        // document.
        if document.group.document_id != document.id {
            return Err(ContentReceiptError::PartitionInvalid {
                document: document.id.clone(),
            });
        }
        document
            .group
            .validate()
            .map_err(|error| document_group_error(&document.id, error))?;
        if observed_by_id.insert(&document.id, document).is_some() {
            return Err(ContentReceiptError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
    }
    for declared in &corpus.documents {
        if !observed_by_id.contains_key(declared.id.as_str()) {
            return Err(ContentReceiptError::MissingDocument {
                document: declared.id.clone(),
            });
        }
    }

    // The partition is total over the declared corpus and agrees with the
    // observed group evidence.
    if partition.declared_document_count != corpus.documents.len() {
        return Err(ContentReceiptError::PartitionInvalid {
            document: format!(
                "{} declared vs {} partitioned",
                corpus.documents.len(),
                partition.declared_document_count
            ),
        });
    }
    let mut partition_by_id: BTreeMap<&str, &DocumentGroupEvidence> = BTreeMap::new();
    for evidence in &partition.assignments {
        evidence
            .validate()
            .map_err(|error| document_group_error(&evidence.document_id, error))?;
        if !observed_by_id.contains_key(evidence.document_id.as_str()) {
            return Err(ContentReceiptError::PartitionInvalid {
                document: evidence.document_id.clone(),
            });
        }
        partition_by_id.insert(evidence.document_id.as_str(), evidence);
    }
    for document in observed {
        let evidence = partition_by_id.get(document.id.as_str()).ok_or_else(|| {
            ContentReceiptError::PartitionInvalid {
                document: document.id.clone(),
            }
        })?;
        if *evidence != &document.group {
            return Err(ContentReceiptError::PartitionInvalid {
                document: document.id.clone(),
            });
        }
    }

    // Typed matrix: every family exactly once per document, canonical order.
    let mut rows_by_id: BTreeMap<&str, ContentDocumentReceipt> = BTreeMap::new();
    for declared in &corpus.documents {
        let document = observed_by_id
            .get(declared.id.as_str())
            .expect("observed coverage checked above");
        let families = canonical_families(&declared.id, &document.families)?;
        rows_by_id.insert(
            declared.id.as_str(),
            ContentDocumentReceipt {
                id: declared.id.clone(),
                source_pdf: declared.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: declared.pages,
                observed_pages: document.observed_pages,
                group: document.group.clone(),
                families,
                body: document.body.clone(),
                pipeline_digest: document.pipeline_digest.clone(),
                oracle_digest: document.oracle_digest.clone(),
            },
        );
    }

    // Group-stratified populations, in canonical stratum order.
    let mut populations = Vec::new();
    for stratum in CONTENT_POPULATION_ORDER {
        let stratum_documents: Vec<&ContentDeclaredDocument> = corpus
            .documents
            .iter()
            .filter(|declared| {
                ContentPopulationId::of_evidence(partition_by_id[declared.id.as_str()]) == stratum
            })
            .collect();
        if stratum_documents.is_empty() {
            continue;
        }
        let mut matrix = Vec::with_capacity(stratum_documents.len());
        let mut identities = Vec::with_capacity(stratum_documents.len());
        for declared in &stratum_documents {
            matrix.push(rows_by_id[declared.id.as_str()].clone());
            identities.push(ContentDocumentIdentity {
                id: declared.id.clone(),
                source_pdf: declared.source_pdf.clone(),
                sha256: declared.sha256.clone(),
                pages: declared.pages,
            });
        }
        populations.push(ContentPopulationReceipt {
            population: stratum,
            document_count: stratum_documents.len() as u64,
            page_denominator: stratum_documents.iter().map(|doc| doc.pages).sum(),
            documents: identities,
            matrix,
        });
    }

    let mut oracle_documents = oracle.documents.clone();
    oracle_documents.sort_by_key(|fixture| corpus_position(corpus, &fixture.document_id));

    Ok(ContentReceipt {
        schema_version: NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks: checked_locks,
        declaration: CarrierBinding {
            manifest: corpus.manifest.clone(),
            pointer: CONTENT_DECLARATION_POINTER.to_owned(),
            revision_sha256: declaration_revision_sha256.to_owned(),
        },
        guard: GuardProvenanceReceipt {
            observer: GuardObserver::JavaTransportFetchTei,
            installed: guard.installed,
            identity_isolated: guard.identity_isolated,
            observed_java_calls: guard.observed_java_calls,
        },
        oracle: ContentOracleReceipt {
            generator_id: oracle.generator_id.clone(),
            backend: oracle.backend.clone(),
            manifest: oracle.manifest.clone(),
            revision_sha256: oracle.revision_sha256.clone(),
            independent: oracle.independent,
            same_generator_refused: oracle.same_generator_refused,
            segmentation_manifest: oracle.segmentation_manifest.clone(),
            segmentation_revision_sha256: oracle.segmentation_revision_sha256.clone(),
            segmentation_source: oracle.segmentation_source.clone(),
            segmentation_source_sha256: oracle.segmentation_source_sha256.clone(),
            documents: oracle_documents,
        },
        partition: partition.clone(),
        populations,
        pooling: ContentPoolingReceipt {
            group_strata_pooled: false,
        },
        redaction: Redaction::clean(),
    })
}

fn canonical_families(
    document_id: &str,
    cells: &[ContentFamilyCell],
) -> Result<Vec<ContentFamilyCell>, ContentReceiptError> {
    let mut ordered = Vec::with_capacity(CONTENT_FAMILY_ORDER.len());
    for family in CONTENT_FAMILY_ORDER {
        let mut matching = cells.iter().filter(|cell| cell.family == family);
        let cell = matching
            .next()
            .ok_or_else(|| ContentReceiptError::FamilyMatrixIncomplete {
                document: document_id.to_owned(),
                family: family.token().to_owned(),
            })?;
        if matching.next().is_some() {
            return Err(ContentReceiptError::FamilyMatrixIncomplete {
                document: document_id.to_owned(),
                family: family.token().to_owned(),
            });
        }
        if let Some(digest) = &cell.observed_digest {
            require_digest(
                &format!("document[{document_id}].family[{}].digest", family.token()),
                digest,
            )?;
        }
        parse_family_disposition(document_id, family, cell)?;
        ordered.push(cell.clone());
    }
    if cells.len() != CONTENT_FAMILY_ORDER.len() {
        for cell in cells {
            if !CONTENT_FAMILY_ORDER.contains(&cell.family) {
                return Err(ContentReceiptError::UnknownFamily {
                    family: cell.family.token().to_owned(),
                });
            }
        }
    }
    Ok(ordered)
}

/// Parse a cell's disposition/reason pair through the T05 closed vocabulary.
///
/// This is where `out_of_scope` is refused **by name**: a tampered artifact can
/// carry the token, and this function reports the exact mutation.
fn parse_family_disposition(
    document_id: &str,
    family: ContentFamily,
    cell: &ContentFamilyCell,
) -> Result<RequiredFamilyDisposition, ContentReceiptError> {
    let inconsistent = |detail: &str| ContentReceiptError::FamilyCellInconsistent {
        document: document_id.to_owned(),
        family: family.token().to_owned(),
        detail: detail.to_owned(),
    };
    match cell.disposition.as_str() {
        "measured" => {
            if cell.unmet_reason.is_some() {
                return Err(inconsistent("a `measured` cell carries an unmet reason"));
            }
            if cell.observed_count.is_none() {
                return Err(inconsistent("a `measured` cell carries no observed count"));
            }
            Ok(RequiredFamilyDisposition::Measured)
        }
        "not_observed" => {
            if cell.unmet_reason.is_some() {
                return Err(inconsistent(
                    "a `not_observed` cell carries an unmet reason",
                ));
            }
            if cell.observed_count.is_some() {
                return Err(inconsistent(
                    "a `not_observed` cell carries an observed count",
                ));
            }
            Ok(RequiredFamilyDisposition::NotObserved)
        }
        "unmet" => {
            if cell.observed_count.is_some() {
                return Err(inconsistent("an `unmet` cell carries an observed count"));
            }
            let token = cell
                .unmet_reason
                .as_deref()
                .ok_or_else(|| inconsistent("an `unmet` cell carries no typed reason"))?;
            let reason = RequiredFamilyUnmetReason::from_token(token)
                .map_err(|_| inconsistent(&format!("`{token}` is not a typed unmet reason")))?;
            Ok(RequiredFamilyDisposition::unmet(reason))
        }
        other => match RequiredFamilyDisposition::parse(other) {
            Err(DocumentGroupError::RequiredFamilyOutOfScope { token }) => {
                Err(ContentReceiptError::RequiredFamilyOutOfScope { token })
            }
            Err(error) => Err(inconsistent(&error.to_string())),
            Ok(_) => Err(inconsistent("a disposition token was not parsed")),
        },
    }
}

fn validate_body_axis(
    document_id: &str,
    axis: &ContentBodyAxis,
) -> Result<(), ContentReceiptError> {
    for (field, token, vocabulary) in [
        (
            "plane_preservation",
            axis.plane_preservation.as_str(),
            CONTENT_BODY_VERDICT_VOCABULARY.as_slice(),
        ),
        (
            "body_region",
            axis.body_region.as_str(),
            CONTENT_BODY_VERDICT_VOCABULARY.as_slice(),
        ),
        (
            "body_reason",
            axis.body_reason.as_str(),
            CONTENT_BODY_REASON_VOCABULARY.as_slice(),
        ),
    ] {
        if !vocabulary.contains(&token) {
            return Err(ContentReceiptError::InvalidBodyVocabulary {
                field: format!("document[{document_id}].body.{field}"),
                token: token.to_owned(),
            });
        }
    }
    // A proven P1 loss must name the D498 obligation; any other verdict must not.
    let loss = axis.plane_preservation == "content_loss";
    if loss != axis.plane_d498_reopen_required {
        return Err(ContentReceiptError::FamilyCellInconsistent {
            document: document_id.to_owned(),
            family: "body".to_owned(),
            detail: "the P1 D498 flag does not match the P1 verdict".to_owned(),
        });
    }
    Ok(())
}

fn corpus_position(corpus: &ContentDeclaredCorpus, document_id: &str) -> usize {
    let key = normalize_document_identity(document_id);
    corpus
        .documents
        .iter()
        .position(|declared| normalize_document_identity(&declared.id) == key)
        .unwrap_or(usize::MAX)
}

fn require_digest(field: &str, value: &str) -> Result<(), ContentReceiptError> {
    if !is_sha256_digest(value) {
        return Err(ContentReceiptError::InvalidDigest {
            field: field.to_owned(),
        });
    }
    Ok(())
}

fn require_path(field: &str, value: &str) -> Result<(), ContentReceiptError> {
    if !is_repo_relative_path(value) {
        return Err(ContentReceiptError::InvalidPath {
            field: field.to_owned(),
        });
    }
    Ok(())
}

// ───────────────────────── validation ─────────────────────────

/// The committed sources a receipt is re-read against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentExpectation {
    /// The total group partition recomputed from the classifier and the corpus.
    pub partition: DocumentGroupPartition,
    /// sha256 of the declaration carrier, recomputed from its bytes.
    pub declaration_revision_sha256: String,
    /// sha256 of the preserved-TEI oracle carrier, recomputed from its bytes.
    pub oracle_revision_sha256: String,
    /// Bare sha256 of each preserved-TEI fixture, keyed by normalized document
    /// identity and recomputed from the fixture bytes.
    pub oracle_fixture_sha256: BTreeMap<String, String>,
    /// sha256 of the reviewed segmentation manifest, recomputed.
    pub segmentation_revision_sha256: String,
    /// sha256 of the reviewed segmentation source, recomputed.
    pub segmentation_source_sha256: String,
    /// The families that must have a positive oracle row somewhere on the slice.
    pub required_families: Vec<ContentFamily>,
}

/// Re-read a content receipt against the committed sources it claims.
///
/// This is the independent half of the content contract: it re-runs the
/// structural checks on the *parsed* bytes and refuses the receipt-level
/// mutations a builder cannot express — a raised threshold, a pooled group
/// stratum, a drifted binding, an `out_of_scope` required family, or a vacuous
/// oracle.
pub fn validate_content_receipt(
    receipt: &ContentReceipt,
    expected: &ContentExpectation,
) -> Result<(), ContentReceiptError> {
    if receipt.schema_version != NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION {
        return Err(ContentReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(ContentReceiptError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(ContentReceiptError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(ContentReceiptError::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }

    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(ContentReceiptError::LockMissing {
                    lock: lock.to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(ContentReceiptError::LockDrift {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }

    if receipt.guard.observer != GuardObserver::JavaTransportFetchTei {
        return Err(ContentReceiptError::GuardObserverDrift {
            found: receipt.guard.observer.as_str().to_owned(),
        });
    }
    if !receipt.guard.installed {
        return Err(ContentReceiptError::GuardNotInstalled);
    }
    if !receipt.guard.identity_isolated {
        return Err(ContentReceiptError::GuardIdentityNotIsolated);
    }
    if receipt.guard.observed_java_calls > 0 {
        return Err(ContentReceiptError::ObservedJavaOnSlice {
            attempts: receipt.guard.observed_java_calls,
        });
    }

    if receipt.pooling.group_strata_pooled {
        return Err(ContentReceiptError::PoolingClaimed {
            field: "group_strata_pooled".to_owned(),
        });
    }
    if receipt.redaction != Redaction::clean() {
        return Err(ContentReceiptError::RedactionViolation {
            marker: "redaction flag set".to_owned(),
        });
    }

    if receipt.declaration.manifest != DECLARATION_MANIFEST_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: DECLARATION_MANIFEST_REL.to_owned(),
            found: receipt.declaration.manifest.clone(),
        });
    }
    if receipt.declaration.pointer != CONTENT_DECLARATION_POINTER {
        return Err(ContentReceiptError::BindingDrift {
            field: "declaration.pointer".to_owned(),
            expected: CONTENT_DECLARATION_POINTER.to_owned(),
            found: receipt.declaration.pointer.clone(),
        });
    }
    if receipt.declaration.revision_sha256 != expected.declaration_revision_sha256 {
        return Err(ContentReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: expected.declaration_revision_sha256.clone(),
            found: receipt.declaration.revision_sha256.clone(),
        });
    }
    if receipt.oracle.manifest != CONTENT_ORACLE_MANIFEST_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.manifest".to_owned(),
            expected: CONTENT_ORACLE_MANIFEST_REL.to_owned(),
            found: receipt.oracle.manifest.clone(),
        });
    }
    if receipt.oracle.revision_sha256 != expected.oracle_revision_sha256 {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.revision_sha256".to_owned(),
            expected: expected.oracle_revision_sha256.clone(),
            found: receipt.oracle.revision_sha256.clone(),
        });
    }
    if receipt.oracle.segmentation_manifest != SEGMENTATION_ORACLE_MANIFEST_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.segmentation_manifest".to_owned(),
            expected: SEGMENTATION_ORACLE_MANIFEST_REL.to_owned(),
            found: receipt.oracle.segmentation_manifest.clone(),
        });
    }
    if receipt.oracle.segmentation_revision_sha256 != expected.segmentation_revision_sha256 {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.segmentation_revision_sha256".to_owned(),
            expected: expected.segmentation_revision_sha256.clone(),
            found: receipt.oracle.segmentation_revision_sha256.clone(),
        });
    }
    if receipt.oracle.segmentation_source != SEGMENTATION_ORACLE_SOURCE_REL {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.segmentation_source".to_owned(),
            expected: SEGMENTATION_ORACLE_SOURCE_REL.to_owned(),
            found: receipt.oracle.segmentation_source.clone(),
        });
    }
    if receipt.oracle.segmentation_source_sha256 != expected.segmentation_source_sha256 {
        return Err(ContentReceiptError::BindingDrift {
            field: "oracle.segmentation_source_sha256".to_owned(),
            expected: expected.segmentation_source_sha256.clone(),
            found: receipt.oracle.segmentation_source_sha256.clone(),
        });
    }
    if !receipt.oracle.independent {
        return Err(ContentReceiptError::OracleNotIndependent);
    }
    if !receipt.oracle.same_generator_refused {
        return Err(ContentReceiptError::OracleIndependenceUnrefuted);
    }

    for fixture in &receipt.oracle.documents {
        let key = normalize_document_identity(&fixture.document_id);
        match expected.oracle_fixture_sha256.get(&key) {
            None => {
                return Err(ContentReceiptError::UnknownOracleDocument {
                    document: fixture.document_id.clone(),
                });
            }
            Some(pin) if pin != &fixture.sha256 => {
                return Err(ContentReceiptError::DocumentShaDrift {
                    document: fixture.document_id.clone(),
                    expected: pin.clone(),
                    found: fixture.sha256.clone(),
                });
            }
            Some(_) => {}
        }
    }

    if receipt.partition != expected.partition {
        return Err(ContentReceiptError::BindingDrift {
            field: "partition".to_owned(),
            expected: "the recomputed total group partition".to_owned(),
            found: "a drifted partition".to_owned(),
        });
    }

    // The declared corpus is the union of the populations; the strata must be
    // disjoint and each document must sit in the stratum of its own group.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut matrix_rows: Vec<&ContentDocumentReceipt> = Vec::new();
    for population in &receipt.populations {
        if population.document_count != population.documents.len() as u64 {
            return Err(ContentReceiptError::PopulationStratificationInvalid {
                detail: format!(
                    "stratum `{}` declares {} documents but carries {}",
                    population.population.token(),
                    population.document_count,
                    population.documents.len()
                ),
            });
        }
        if population.matrix.len() != population.documents.len() {
            return Err(ContentReceiptError::PopulationStratificationInvalid {
                detail: format!(
                    "stratum `{}` has {} rows for {} documents",
                    population.population.token(),
                    population.matrix.len(),
                    population.documents.len()
                ),
            });
        }
        let page_sum: u64 = population.documents.iter().map(|doc| doc.pages).sum();
        if page_sum != population.page_denominator {
            return Err(ContentReceiptError::PopulationStratificationInvalid {
                detail: format!(
                    "stratum `{}` page denominator drift",
                    population.population.token()
                ),
            });
        }
        for document in &population.matrix {
            if !seen.insert(document.id.as_str()) {
                return Err(ContentReceiptError::DuplicateDocument {
                    document: document.id.clone(),
                });
            }
            let evidence = receipt
                .partition
                .assignments
                .iter()
                .find(|evidence| evidence.document_id == document.id)
                .ok_or_else(|| ContentReceiptError::PartitionInvalid {
                    document: document.id.clone(),
                })?;
            if ContentPopulationId::of_evidence(evidence) != population.population {
                return Err(ContentReceiptError::PoolingClaimed {
                    field: format!("populations[{}]", population.population.token()),
                });
            }
            matrix_rows.push(document);
        }
    }
    for evidence in &receipt.partition.assignments {
        if !seen.contains(evidence.document_id.as_str()) {
            return Err(ContentReceiptError::PopulationStratificationInvalid {
                detail: format!("document `{}` is in no stratum", evidence.document_id),
            });
        }
    }

    // Family matrix and oracle non-vacuity.
    for document in &matrix_rows {
        let families = canonical_families(&document.id, &document.families)?;
        if document.group.document_id != document.id {
            return Err(ContentReceiptError::PartitionInvalid {
                document: document.id.clone(),
            });
        }
        document
            .group
            .validate()
            .map_err(|error| document_group_error(&document.id, error))?;
        validate_body_axis(&document.id, &document.body)?;
        require_digest(
            &format!("document[{}].pipeline_digest", document.id),
            &document.pipeline_digest,
        )?;
        require_digest(
            &format!("document[{}].oracle_digest", document.id),
            &document.oracle_digest,
        )?;
        let _ = families;
    }

    for family in &expected.required_families {
        let positive: u64 = matrix_rows
            .iter()
            .flat_map(|document| document.families.iter())
            .filter(|cell| cell.family == *family)
            .map(|cell| cell.oracle_positive_rows)
            .sum();
        if positive == 0 {
            return Err(ContentReceiptError::OracleVacuous {
                family: family.token().to_owned(),
            });
        }
    }

    // Canonical re-serialization must not leak a payload marker.
    let bytes = receipt.artifact_json_bytes()?;
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        ContentReceiptError::ArtifactSerialization {
            reason: error.to_string(),
        }
    })?;
    assert_content_redacted(text)?;

    Ok(())
}

/// Recomputed sha256 of some committed bytes, as the runner and validator bind
/// them. Exposed so a caller cannot invent its own hex formatting.
pub fn content_sha256_prefixed(bytes: &[u8]) -> String {
    sha256_prefixed(bytes)
}

/// Bare sha256 of some committed bytes.
pub fn content_sha256_hex(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_document_group::DocumentGroupEvidence;
    use da_domain::pdf_classification::PdfType;

    fn corpus() -> ContentDeclaredCorpus {
        ContentDeclaredCorpus {
            manifest: DECLARATION_MANIFEST_REL.to_owned(),
            pointer: CONTENT_CORPUS_POINTER.to_owned(),
            revision_sha256: sha256_prefixed(b"declaration"),
            corpus_id: "s13-same-input-pdf-corpus".to_owned(),
            document_count: 2,
            page_denominator: 100,
            documents: vec![
                ContentDeclaredDocument {
                    id: "hub".to_owned(),
                    source_pdf: "data/sources/hub.pdf".to_owned(),
                    sha256: "a".repeat(64),
                    pages: 60,
                },
                ContentDeclaredDocument {
                    id: "scan".to_owned(),
                    source_pdf: "data/sources/scan.pdf".to_owned(),
                    sha256: "b".repeat(64),
                    pages: 40,
                },
            ],
        }
    }

    fn partition() -> DocumentGroupPartition {
        DocumentGroupPartition::assemble(
            "s13-same-input-pdf-corpus",
            &["hub".to_owned(), "scan".to_owned()],
            vec![
                DocumentGroupEvidence::measured(
                    "hub",
                    PdfType::Text,
                    "pdf-inspector",
                    "0.1.0",
                    "a".repeat(64),
                ),
                DocumentGroupEvidence::measured(
                    "scan",
                    PdfType::Scanned,
                    "pdf-inspector",
                    "0.1.0",
                    "b".repeat(64),
                ),
            ],
        )
        .expect("a total partition")
    }

    fn oracle() -> ContentOracleInput {
        ContentOracleInput {
            generator_id: "da-cli.native_content_receipt.tei-scanner".to_owned(),
            backend: "java-grobid-crf".to_owned(),
            manifest: CONTENT_ORACLE_MANIFEST_REL.to_owned(),
            revision_sha256: sha256_prefixed(b"provenance"),
            independent: true,
            same_generator_refused: true,
            segmentation_manifest: SEGMENTATION_ORACLE_MANIFEST_REL.to_owned(),
            segmentation_revision_sha256: sha256_prefixed(b"segmentation"),
            segmentation_source: SEGMENTATION_ORACLE_SOURCE_REL.to_owned(),
            segmentation_source_sha256: sha256_prefixed(b"segmentation-source"),
            documents: vec![
                ContentOracleDocument {
                    document_id: "hub".to_owned(),
                    fixture_path: "crates/da-parse/tests/fixtures/preserved/hub.tei.xml".to_owned(),
                    sha256: "c".repeat(64),
                    oracle_digest: sha256_prefixed(b"hub-oracle"),
                },
                ContentOracleDocument {
                    document_id: "scan".to_owned(),
                    fixture_path: "crates/da-parse/tests/fixtures/preserved/scan.tei.xml"
                        .to_owned(),
                    sha256: "d".repeat(64),
                    oracle_digest: sha256_prefixed(b"scan-oracle"),
                },
            ],
        }
    }

    fn family_cells() -> Vec<ContentFamilyCell> {
        CONTENT_FAMILY_ORDER
            .iter()
            .enumerate()
            .map(|(index, family)| {
                ContentFamilyCell::measured(
                    *family,
                    index as u64 + 1,
                    3,
                    Some(sha256_prefixed(b"obs")),
                )
            })
            .collect()
    }

    fn body_axis() -> ContentBodyAxis {
        ContentBodyAxis {
            plane_preservation: "not_assessable".to_owned(),
            plane_d498_reopen_required: false,
            plane_present_in_token_units: 10,
            plane_spanning_token_units: 1,
            plane_character_material_units: 0,
            plane_absent_units: 0,
            body_region: "not_assessable".to_owned(),
            body_reason: "p2_region_assignment_gap".to_owned(),
            body_token_count: 100,
            oracle_units: 90,
            native_plane_units: 110,
        }
    }

    fn observed() -> Vec<ContentObservedDocument> {
        vec![
            ContentObservedDocument {
                id: "hub".to_owned(),
                sha256: "a".repeat(64),
                pages: 60,
                observed_pages: 60,
                group: DocumentGroupEvidence::measured(
                    "hub",
                    PdfType::Text,
                    "pdf-inspector",
                    "0.1.0",
                    "a".repeat(64),
                ),
                families: family_cells(),
                body: body_axis(),
                pipeline_digest: sha256_prefixed(b"hub-pipeline"),
                oracle_digest: sha256_prefixed(b"hub-oracle"),
            },
            ContentObservedDocument {
                id: "scan".to_owned(),
                sha256: "b".repeat(64),
                pages: 40,
                observed_pages: 40,
                group: DocumentGroupEvidence::measured(
                    "scan",
                    PdfType::Scanned,
                    "pdf-inspector",
                    "0.1.0",
                    "b".repeat(64),
                ),
                families: family_cells(),
                body: body_axis(),
                pipeline_digest: sha256_prefixed(b"scan-pipeline"),
                oracle_digest: sha256_prefixed(b"scan-oracle"),
            },
        ]
    }

    fn locks() -> BTreeMap<String, String> {
        REQUIRED_LOCKS
            .iter()
            .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn receipt() -> ContentReceipt {
        build_content_receipt(
            &sha256_prefixed(b"declaration"),
            &corpus(),
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            GuardProvenance {
                installed: true,
                identity_isolated: true,
                observed_java_calls: 0,
            },
            &oracle(),
            &partition(),
            &observed(),
        )
        .expect("the builder accepts a well-formed receipt")
    }

    fn expectation() -> ContentExpectation {
        ContentExpectation {
            partition: partition(),
            declaration_revision_sha256: sha256_prefixed(b"declaration"),
            oracle_revision_sha256: sha256_prefixed(b"provenance"),
            oracle_fixture_sha256: BTreeMap::from([
                ("hub".to_owned(), "c".repeat(64)),
                ("scan".to_owned(), "d".repeat(64)),
            ]),
            segmentation_revision_sha256: sha256_prefixed(b"segmentation"),
            segmentation_source_sha256: sha256_prefixed(b"segmentation-source"),
            required_families: CONTENT_FAMILY_ORDER.to_vec(),
        }
    }

    #[test]
    fn the_builder_emits_two_group_strata_and_validates() {
        let receipt = receipt();
        assert_eq!(receipt.populations.len(), 2, "one stratum per group");
        assert_eq!(
            receipt.populations[0].population,
            ContentPopulationId::DigitalText
        );
        assert_eq!(receipt.populations[0].document_count, 1);
        assert_eq!(
            receipt.populations[1].population,
            ContentPopulationId::ScannedImage
        );
        assert_eq!(receipt.locks.len(), REQUIRED_LOCKS.len());
        validate_content_receipt(&receipt, &expectation()).expect("the receipt validates");
    }

    #[test]
    fn canonical_re_serialization_is_byte_reproducible() {
        let receipt = receipt();
        let first = receipt.artifact_json_bytes().expect("serializes");
        let parsed = ContentReceipt::parse_json_bytes(&first).expect("parses");
        let second = parsed.artifact_json_bytes().expect("re-serializes");
        assert_eq!(first, second, "canonical bytes must round-trip identically");
    }

    #[test]
    fn a_raised_threshold_is_refused_by_name() {
        let mut receipt = receipt();
        receipt.thresholds = "accepted".to_owned();
        assert_eq!(
            validate_content_receipt(&receipt, &expectation()),
            Err(ContentReceiptError::ThresholdClaimed {
                found: "accepted".to_owned()
            })
        );
    }

    #[test]
    fn a_pooled_strata_flag_is_refused_by_name() {
        let mut receipt = receipt();
        receipt.pooling.group_strata_pooled = true;
        assert_eq!(
            validate_content_receipt(&receipt, &expectation()),
            Err(ContentReceiptError::PoolingClaimed {
                field: "group_strata_pooled".to_owned()
            })
        );
    }

    #[test]
    fn an_out_of_scope_required_family_is_refused_by_name() {
        let mut receipt = receipt();
        receipt.populations[0].matrix[0].families[0].disposition = "out_of_scope".to_owned();
        receipt.populations[0].matrix[0].families[0].unmet_reason = None;
        receipt.populations[0].matrix[0].families[0].observed_count = None;
        assert_eq!(
            validate_content_receipt(&receipt, &expectation()),
            Err(ContentReceiptError::RequiredFamilyOutOfScope {
                token: "out_of_scope".to_owned()
            })
        );
    }

    #[test]
    fn a_vacuous_oracle_is_refused_by_name() {
        let mut receipt = receipt();
        for population in &mut receipt.populations {
            for document in &mut population.matrix {
                for cell in &mut document.families {
                    cell.oracle_positive_rows = 0;
                }
            }
        }
        assert_eq!(
            validate_content_receipt(&receipt, &expectation()),
            Err(ContentReceiptError::OracleVacuous {
                family: "figure".to_owned()
            })
        );
    }

    #[test]
    fn a_drifted_binding_is_refused_by_name() {
        let mut receipt = receipt();
        receipt.oracle.segmentation_source_sha256 = sha256_prefixed(b"other");
        assert_eq!(
            validate_content_receipt(&receipt, &expectation()),
            Err(ContentReceiptError::BindingDrift {
                field: "oracle.segmentation_source_sha256".to_owned(),
                expected: sha256_prefixed(b"segmentation-source"),
                found: sha256_prefixed(b"other"),
            })
        );
    }

    #[test]
    fn an_unknown_document_is_refused_by_the_builder() {
        let mut partition = partition();
        partition.assignments.pop();
        let error = build_content_receipt(
            &sha256_prefixed(b"declaration"),
            &corpus(),
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            GuardProvenance {
                installed: true,
                identity_isolated: true,
                observed_java_calls: 0,
            },
            &oracle(),
            &partition,
            &observed(),
        )
        .expect_err("a partition missing a document is refused");
        assert!(matches!(
            error,
            ContentReceiptError::PartitionInvalid { .. }
        ));
    }

    #[test]
    fn a_missing_family_cell_is_refused_by_the_builder() {
        let mut observed = observed();
        observed[0].families.pop();
        let error = build_content_receipt(
            &sha256_prefixed(b"declaration"),
            &corpus(),
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            GuardProvenance {
                installed: true,
                identity_isolated: true,
                observed_java_calls: 0,
            },
            &oracle(),
            &partition(),
            &observed,
        )
        .expect_err("an incomplete family matrix is refused");
        assert!(matches!(
            error,
            ContentReceiptError::FamilyMatrixIncomplete { .. }
        ));
    }

    #[test]
    fn a_redaction_marker_is_refused() {
        assert_eq!(
            assert_content_redacted("{\"note\":\"http://127.0.0.1:8070\"}"),
            Err(ContentReceiptError::RedactionViolation {
                marker: "http://".to_owned()
            })
        );
    }

    #[test]
    fn an_unmet_without_reason_is_refused() {
        let mut receipt = receipt();
        receipt.populations[0].matrix[0].families[0].disposition = "unmet".to_owned();
        receipt.populations[0].matrix[0].families[0].unmet_reason = None;
        receipt.populations[0].matrix[0].families[0].observed_count = None;
        let error = validate_content_receipt(&receipt, &expectation())
            .expect_err("unmet without a reason is refused");
        assert!(matches!(
            error,
            ContentReceiptError::FamilyCellInconsistent { .. }
        ));
    }
}
