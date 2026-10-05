//! S04/T04 (M514-1lss2s) — typed family-parity receipt for the native header and
//! bibliography families on the declared three-document slice.
//!
//! S04 links the S03 native plane (positioned layout + native segmentation) to
//! the header and bibliography atoms. A link is not a measurement, so this
//! module turns the run into a **corpus-bound, reviewable receipt**:
//!
//! - the population is the three pinned documents read by pointer from
//!   `data/parse/layout_bakeoff.yaml#java_free_vertical_slice` (this module never
//!   restates the pin),
//! - every declared document carries one **typed** cell per family and one
//!   **character-coverage** row per region, so a missing measurement is a typed
//!   disposition, never a silent zero,
//! - the expectation side is an **independent oracle** (preserved TEI produced by
//!   `java-grobid-crf`, sha256-pinned in `provenance.yaml`); a same-generator
//!   oracle is refused,
//! - the zero on the Java seam is reachable only under an installed, identity-
//!   isolated guard.
//!
//! The module is pure `std`/`serde`/`sha2`: it compiles in the default feature
//! set (no `pdf-layout`, no `da-parse`) and owns no route, no Java and no graph
//! write. It is a diagnostic receipt, not a product claim (D499):
//! `thresholds = not_accepted` and `product_coverage_effect = none`.
//!
//! Every emitted leaf is a count, a sha256, a closed enum token, a pinned
//! document identity or a repository-relative path. The article text, the PDF
//! bytes, the TEI bytes, absolute paths and endpoints never reach the artifact,
//! and [`assert_redacted`] refuses an artifact that leaks one.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

/// Schema version of the emitted receipt artifact.
pub const NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION: &str = "native-family-parity-receipt.v1";
/// Default artifact path of the parity receipt.
pub const DEFAULT_ARTIFACT_REL: &str = "artifacts/parse_compat/native_family_parity.json";
/// The only claim scope the receipt may carry. Anything else is a product claim.
pub const CLAIM_SCOPE_DIAGNOSTIC_ONLY: &str = "diagnostic_only";
/// The only product-coverage effect the receipt may declare.
pub const PRODUCT_COVERAGE_EFFECT_NONE: &str = "none";
/// Thresholds are never accepted in this cut; the receipt only measures.
pub const THRESHOLDS_NOT_ACCEPTED: &str = "not_accepted";
/// Repository-relative carrier of the slice declaration and the corpus pin.
pub const DECLARATION_MANIFEST_REL: &str = "data/parse/layout_bakeoff.yaml";
/// Pointer to the slice declaration block inside the carrier.
pub const DECLARATION_POINTER: &str = "java_free_vertical_slice";
/// Pointer to the pinned corpus documents.
pub const CORPUS_POINTER: &str = "native_route_corpus.documents";
/// Repository-relative carrier of the preserved-TEI oracle pins.
pub const ORACLE_MANIFEST_REL: &str = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
/// Identity of the oracle builder. It must differ from both native-band
/// generator ids, or the oracle is the SUT and proves nothing.
pub const ORACLE_GENERATOR_ID: &str = "da-cli.native_family_parity.tei-lexical-scanner";
/// The backend that produced the preserved oracle bytes (the other generator).
pub const ORACLE_BACKEND_ID: &str = "java-grobid-crf";
/// Locks that must stay exactly as declared; the receipt never relaxes one.
pub const REQUIRED_LOCKS: [(&str, &str); 5] = [
    ("deployed", "false"),
    ("import_eligible", "false"),
    ("auto_promote", "forbidden"),
    ("production_route", "forbidden"),
    ("graph_write", "forbidden"),
];
/// Markers that must never reach the emitted artifact. Payload text, TEI bytes,
/// service endpoints and host filesystem layouts are all represented here.
pub const LEAK_MARKERS: [&str; 14] = [
    "<TEI",
    "<tei",
    "http://",
    "https://",
    "file://",
    "127.0.0.1",
    "localhost",
    "/root",
    "/home",
    "/tmp",
    "C:\\",
    ".wapiti",
    "#mdl#",
    "SENTINEL-NOT-FOR-ARTIFACT",
];
/// The closed family vocabulary, in canonical (matrix) order.
pub const NATIVE_FAMILY_ORDER: [NativeFamilyId; 7] = [
    NativeFamilyId::HeaderDate,
    NativeFamilyId::HeaderPersonalName,
    NativeFamilyId::HeaderAffiliation,
    NativeFamilyId::HeaderAffiliationLink,
    NativeFamilyId::BibliographyCitationFields,
    NativeFamilyId::BibliographyReferenceSegments,
    NativeFamilyId::BibliographyCitationAnchors,
];
/// The closed coverage-region vocabulary, in canonical order.
pub const COVERAGE_REGION_ORDER: [CoverageRegion; 2] =
    [CoverageRegion::Header, CoverageRegion::References];

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals. Every variant is a refusal, never a silent downgrade: the
/// receipt either justifies what it says or says why it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFamilyReceiptError {
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming to have none.
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
    /// A carrier binding drifted from the pointer the contract requires.
    BindingDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The zero was claimed without an installed guard.
    GuardNotInstalled,
    /// The guard would have rotated a cache key, so its zero is unusable.
    GuardIdentityNotIsolated,
    /// Java was consulted during the run; the families are not Java-free.
    ObservedJavaOnSlice { attempts: u64 },
    /// The oracle is the SUT's own generator, or its independence was not proven.
    OracleNotIndependent,
    /// The oracle's negative control was not executed, so independence is unproven.
    OracleIndependenceUnrefuted,
    /// The pinned corpus count drifted from the documents it restates.
    CorpusCountDrift { expected: u64, found: u64 },
    /// The corpus page denominator drifted from the pinned documents.
    CorpusPageDrift { expected: u64, found: u64 },
    /// The run observed a document the declaration does not pin.
    UnknownDocument { document: String },
    /// The declaration pins a document the run never observed.
    MissingDocument { document: String },
    /// A declared document has no oracle fixture.
    MissingOracleDocument { document: String },
    /// An oracle fixture names a document the declaration never pins.
    UnknownOracleDocument { document: String },
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
    /// The same document appears twice.
    DuplicateDocument { document: String },
    /// A document is missing a family cell, or carries one twice.
    IncompleteFamilyMatrix { document: String, family: String },
    /// A document is missing a coverage row, or carries one twice.
    IncompleteRegionMatrix { document: String, region: String },
    /// A family reports a count while its disposition is not `observed`.
    CountOnNonObservedDisposition { document: String, family: String },
    /// A family whose disposition is `observed` carries no count.
    ObservedWithoutCount { document: String, family: String },
    /// A disposition carries a refusal reason it must not.
    RefusalOnObservedDisposition { document: String, family: String },
    /// A refused cell carries no refusal reason.
    RefusalWithoutReason { document: String, family: String },
    /// The bibliography families were pooled with the Java `citation_names`
    /// population (R128).
    PoolingClaimed { field: String },
    /// A digest leaf is not a `sha256:<hex>` string.
    InvalidDigest { field: String },
    /// A path leaf is absolute or escapes the repository.
    InvalidPath { field: String },
    /// The typed population matrix is not the one the receipt kind requires.
    PopulationMissing { population: String },
    /// More than one population, or the wrong one, for this receipt kind.
    PopulationMismatch { population: String },
    /// The artifact leaked a payload marker.
    RedactionViolation { marker: String },
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
    /// The frozen exclusion denominator omits one of the typed reasons.
    ExclusionDenominatorAbsent { reason: String },
    /// The frozen exclusion denominator carries a token outside the vocabulary.
    ExclusionUnknownReason { reason: String },
    /// A frozen exclusion denominator drifted from the population it restates.
    ExclusionDenominatorDrift {
        reason: String,
        expected: u64,
        found: u64,
    },
    /// The recovered row count contradicts the declared mode and target.
    ExclusionPopulationCoverage { expected: u64, found: u64 },
    /// The same excluded document appears twice.
    ExclusionDuplicateDocument { document: String },
    /// A recovery row is missing the canonical field matrix.
    ExclusionFieldMatrix { document: String, field: String },
    /// A row or field claims recovery without an independent attribution proof,
    /// which this cut cannot produce (D502).
    ExclusionFieldRecoveryUnattributed { document: String, field: String },
    /// A `recognition_only` field carries no recognition evidence, or its
    /// evidence contradicts the disposition.
    ExclusionFieldRecognitionInconsistent { document: String, field: String },
    /// A field's disposition requires a typed cause and carries none, or carries
    /// one it must not.
    ExclusionFieldCauseInconsistent { document: String, field: String },
    /// A recovery row's disposition contradicts its reason's field state.
    ExclusionDispositionMismatch {
        document: String,
        expected: String,
        found: String,
    },
    /// A typed refusal row carries no refusal reason.
    ExclusionRefusalMissing { document: String },
    /// A non-refusal row carries a refusal reason.
    ExclusionRefusalInconsistent { document: String },
    /// The frozen affiliation reference is absent or malformed.
    AffiliationReferenceMissing,
}

impl fmt::Display for NativeFamilyReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: `{field}` is `{found}` but `{expected}` is required"
            ),
            Self::ClaimScopeDrift { found } => write!(
                f,
                "native family receipt: claim_scope is `{found}`; only \
                 `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}` is allowed (D499)"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "native family receipt: product_coverage_effect is `{found}`; \
                 a scoped diagnostic must declare `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::ThresholdClaimed { found } => write!(
                f,
                "native family receipt: thresholds is `{found}`; \
                 `{THRESHOLDS_NOT_ACCEPTED}` is required"
            ),
            Self::LockDrift {
                lock,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: lock `{lock}` is `{found}` but `{expected}` is required"
            ),
            Self::LockMissing { lock } => {
                write!(f, "native family receipt: required lock `{lock}` is absent")
            }
            Self::BindingDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: `{field}` is `{found}` but must reuse `{expected}`"
            ),
            Self::GuardNotInstalled => write!(
                f,
                "native family receipt: a counted zero requires an installed guard"
            ),
            Self::GuardIdentityNotIsolated => write!(
                f,
                "native family receipt: the guard rotates the cache identity, \
                 so its zero cannot be attributed"
            ),
            Self::ObservedJavaOnSlice { attempts } => write!(
                f,
                "native family receipt: `{attempts}` Java consult(s) were observed"
            ),
            Self::OracleNotIndependent => write!(
                f,
                "native family receipt: the oracle is the SUT generator, or its \
                 independence was not proven"
            ),
            Self::OracleIndependenceUnrefuted => write!(
                f,
                "native family receipt: the same-generator negative control was not \
                 executed, so independence is unproven"
            ),
            Self::CorpusCountDrift { expected, found } => write!(
                f,
                "native family receipt: the corpus declares `{found}` documents but \
                 the pin restates `{expected}`"
            ),
            Self::CorpusPageDrift { expected, found } => write!(
                f,
                "native family receipt: the corpus page denominator is `{found}` but \
                 the pinned documents sum to `{expected}`"
            ),
            Self::UnknownDocument { document } => write!(
                f,
                "native family receipt: the run observed `{document}`, which the \
                 declaration never pins"
            ),
            Self::MissingDocument { document } => write!(
                f,
                "native family receipt: the declaration pins `{document}` but the \
                 run never observed it"
            ),
            Self::MissingOracleDocument { document } => write!(
                f,
                "native family receipt: `{document}` has no oracle fixture"
            ),
            Self::UnknownOracleDocument { document } => write!(
                f,
                "native family receipt: oracle fixture `{document}` is not a pinned \
                 corpus document"
            ),
            Self::DocumentShaDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: `{document}` sha256 is `{found}` but `{expected}` \
                 is pinned"
            ),
            Self::PageCountDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: `{document}` observed `{found}` pages but \
                 `{expected}` are pinned"
            ),
            Self::DuplicateDocument { document } => write!(
                f,
                "native family receipt: document `{document}` is observed twice"
            ),
            Self::IncompleteFamilyMatrix { document, family } => write!(
                f,
                "native family receipt: `{document}` is missing family `{family}` \
                 exactly once"
            ),
            Self::IncompleteRegionMatrix { document, region } => write!(
                f,
                "native family receipt: `{document}` is missing region `{region}` \
                 exactly once"
            ),
            Self::CountOnNonObservedDisposition { document, family } => write!(
                f,
                "native family receipt: `{document}`/`{family}` carries a count while \
                 its disposition is not `observed`"
            ),
            Self::ObservedWithoutCount { document, family } => write!(
                f,
                "native family receipt: `{document}`/`{family}` is `observed` with no count"
            ),
            Self::RefusalOnObservedDisposition { document, family } => write!(
                f,
                "native family receipt: `{document}`/`{family}` carries a refusal while \
                 its disposition is `observed`"
            ),
            Self::RefusalWithoutReason { document, family } => write!(
                f,
                "native family receipt: `{document}`/`{family}` is refused with no reason"
            ),
            Self::PoolingClaimed { field } => write!(
                f,
                "native family receipt: `{field}` claims the Java citation_names \
                 population was pooled (R128 forbids it)"
            ),
            Self::InvalidDigest { field } => write!(
                f,
                "native family receipt: `{field}` is not a `sha256:<hex>` digest"
            ),
            Self::InvalidPath { field } => write!(
                f,
                "native family receipt: `{field}` is not a repository-relative path"
            ),
            Self::PopulationMissing { population } => write!(
                f,
                "native family receipt: population `{population}` is absent from the matrix"
            ),
            Self::PopulationMismatch { population } => write!(
                f,
                "native family receipt: population `{population}` is not the one this \
                 receipt kind requires"
            ),
            Self::RedactionViolation { marker } => write!(
                f,
                "native family receipt: the artifact leaked `{marker}`; only counts, \
                 digests, closed tokens and repository-relative paths may leave"
            ),
            Self::ArtifactSerialization { reason } => write!(
                f,
                "native family receipt: artifact serialization failed: {reason}"
            ),
            Self::ExclusionDenominatorAbsent { reason } => write!(
                f,
                "native family receipt: the frozen denominator omits `{reason}`"
            ),
            Self::ExclusionUnknownReason { reason } => write!(
                f,
                "native family receipt: `{reason}` is not a typed exclusion reason"
            ),
            Self::ExclusionDenominatorDrift {
                reason,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: exclusion denominator `{reason}` is `{found}` \
                 but the frozen population restates `{expected}`"
            ),
            Self::ExclusionPopulationCoverage { expected, found } => write!(
                f,
                "native family receipt: the run recovered `{found}` rows but `{expected}` \
                 are required by its mode"
            ),
            Self::ExclusionDuplicateDocument { document } => write!(
                f,
                "native family receipt: exclusion document `{document}` is observed twice"
            ),
            Self::ExclusionFieldMatrix { document, field } => write!(
                f,
                "native family receipt: recovery row `{document}` is missing field `{field}` \
                 exactly once in canonical order"
            ),
            Self::ExclusionFieldRecoveryUnattributed { document, field } => write!(
                f,
                "native family receipt: recovery row `{document}` claims field `{field}` \
                 recovered without an independent attribution proof, which atom recognition \
                 cannot supply (D502)"
            ),
            Self::ExclusionFieldRecognitionInconsistent { document, field } => write!(
                f,
                "native family receipt: recovery row `{document}` field `{field}` carries \
                 recognition evidence inconsistent with its disposition"
            ),
            Self::ExclusionFieldCauseInconsistent { document, field } => write!(
                f,
                "native family receipt: recovery row `{document}` field `{field}` carries a \
                 typed cause inconsistent with its disposition"
            ),
            Self::ExclusionDispositionMismatch {
                document,
                expected,
                found,
            } => write!(
                f,
                "native family receipt: recovery row `{document}` is `{found}` but its \
                 reason requires `{expected}`"
            ),
            Self::ExclusionRefusalMissing { document } => write!(
                f,
                "native family receipt: recovery row `{document}` is a typed refusal with no \
                 refusal reason"
            ),
            Self::ExclusionRefusalInconsistent { document } => write!(
                f,
                "native family receipt: recovery row `{document}` is not a refusal but carries \
                 a refusal reason"
            ),
            Self::AffiliationReferenceMissing => write!(
                f,
                "native family receipt: the frozen affiliation reference is absent or malformed"
            ),
        }
    }
}

impl std::error::Error for NativeFamilyReceiptError {}

// ───────────────────────── digests and paths ─────────────────────────

/// Bare sha256 hex of the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// `sha256:<hex>` digest string of the given bytes.
pub fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A bare 64-character lowercase hex digest.
pub fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A `sha256:`-prefixed 64-character hex digest.
pub fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

/// A repository-relative path: non-empty, no leading `/`, no `..` segment.
pub fn is_repo_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains(':')
        && !value.split(['/', '\\']).any(|segment| segment == "..")
        && !value.starts_with("~/")
}

/// Normalize a document identity so the pinned corpus id (`hub-2507.19457`) and
/// the oracle fixture id (`hub_2507.19457`) join deterministically. Only
/// alphanumerics and dots survive; casing is folded.
pub fn normalize_document_identity(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .collect::<String>()
        .to_ascii_lowercase()
}

// ───────────────────────── closed vocabularies ─────────────────────────

/// Which receipt this is. The vocabulary is closed so a new population kind is
/// a deliberate addition rather than a free string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    /// The declared three-document parity slice (T04).
    ParityThreeDocumentSlice,
    /// The 79 typed baseline exclusions the sibling runner measures (T05).
    ExclusionRecoveryBaseline,
}

impl ReceiptKind {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParityThreeDocumentSlice => "parity_three_document_slice",
            Self::ExclusionRecoveryBaseline => "exclusion_recovery_baseline",
        }
    }
}

/// A population the receipt matrix may carry. `three_document_slice` is the
/// declared S04 parity population; `exclusion_baseline` is the 79 typed
/// exclusions the sibling runner measures and must never be pooled with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationId {
    ThreeDocumentSlice,
    ExclusionBaseline,
}

impl PopulationId {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThreeDocumentSlice => "three_document_slice",
            Self::ExclusionBaseline => "exclusion_baseline",
        }
    }
}

/// Whether a population is a corpus slice or a typed-exclusion denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationKind {
    Corpus,
    Exclusion,
}

impl PopulationKind {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Corpus => "corpus",
            Self::Exclusion => "exclusion",
        }
    }
}

/// The native families the receipt measures, one per T02/T03 atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFamilyId {
    HeaderDate,
    HeaderPersonalName,
    HeaderAffiliation,
    HeaderAffiliationLink,
    BibliographyCitationFields,
    BibliographyReferenceSegments,
    BibliographyCitationAnchors,
}

impl NativeFamilyId {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeaderDate => "header_date",
            Self::HeaderPersonalName => "header_personal_name",
            Self::HeaderAffiliation => "header_affiliation",
            Self::HeaderAffiliationLink => "header_affiliation_link",
            Self::BibliographyCitationFields => "bibliography_citation_fields",
            Self::BibliographyReferenceSegments => "bibliography_reference_segments",
            Self::BibliographyCitationAnchors => "bibliography_citation_anchors",
        }
    }
}

/// The typed state of one family for one document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyDisposition {
    /// The band executed and produced a measured outcome.
    Observed,
    /// The region admitted no tokens: a typed absence, never a zero.
    TypedAbsence,
    /// The band refused with a typed reason (e.g. an invalid donor font size).
    Refused,
    /// The family has no native input class in this cut (e.g. TEI anchor markup).
    OutOfScope,
}

impl FamilyDisposition {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::TypedAbsence => "typed_absence",
            Self::Refused => "refused",
            Self::OutOfScope => "out_of_scope",
        }
    }
}

/// The closed per-family atom disposition, mirroring the T02/T03 vocabularies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomDisposition {
    Recognized,
    Empty,
    Malformed,
    Unsupported,
    Ambiguous,
    Oversized,
    Truncated,
    OutOfScope,
}

impl AtomDisposition {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recognized => "recognized",
            Self::Empty => "empty",
            Self::Malformed => "malformed",
            Self::Unsupported => "unsupported",
            Self::Ambiguous => "ambiguous",
            Self::Oversized => "oversized",
            Self::Truncated => "truncated",
            Self::OutOfScope => "out_of_scope",
        }
    }
}

/// The closed refusal vocabulary of a family cell. A refusal carries a reason,
/// never a free-form message and never a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    NoAdmittedTokens,
    RegionNotRequested,
    NoFeatureTokens,
    MissingFontSize,
    InvalidFontSize,
    TokenContainsWhitespace,
    FeatureBuildRefused,
    GazetteerRefused,
    AnchorScanNotNative,
    SerializationRefused,
    /// A native pipeline step before the band (layout, projection, heading,
    /// carrier) refused, so the band could not run.
    PipelineRefused,
    /// The document exceeded its per-document wall-clock budget. A typed
    /// refusal, never a pass.
    DocumentTimeout,
}

impl RefusalReason {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoAdmittedTokens => "no_admitted_tokens",
            Self::RegionNotRequested => "region_not_requested",
            Self::NoFeatureTokens => "no_feature_tokens",
            Self::MissingFontSize => "missing_font_size",
            Self::InvalidFontSize => "invalid_font_size",
            Self::TokenContainsWhitespace => "token_contains_whitespace",
            Self::FeatureBuildRefused => "feature_build_refused",
            Self::GazetteerRefused => "gazetteer_refused",
            Self::AnchorScanNotNative => "anchor_scan_not_native",
            Self::SerializationRefused => "serialization_refused",
            Self::PipelineRefused => "pipeline_refused",
            Self::DocumentTimeout => "document_timeout",
        }
    }
}

/// The regions the character-coverage comparison covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageRegion {
    Header,
    References,
}

impl CoverageRegion {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::References => "references",
        }
    }
}

/// Typed verdict of a character-level oracle comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageVerdict {
    /// The native side covers every oracle character; differences are
    /// granularity (tokenization/spacing), not lost content.
    GranularityOnly,
    /// The native side is missing characters the oracle carried.
    ContentLoss,
    /// The oracle is empty, so nothing can be assessed. Not a pass.
    NotAssessable,
}

impl CoverageVerdict {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GranularityOnly => "granularity_only",
            Self::ContentLoss => "content_loss",
            Self::NotAssessable => "not_assessable",
        }
    }
}

/// The Java seam the guard observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardObserver {
    #[serde(rename = "java_transport_fetch_tei")]
    JavaTransportFetchTei,
}

impl GuardObserver {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JavaTransportFetchTei => "java_transport_fetch_tei",
        }
    }
}

/// The oracle producer identity (closed: a new generator is a new variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleGenerator {
    #[serde(rename = "da-cli.native_family_parity.tei-lexical-scanner")]
    TeiLexicalScanner,
}

impl OracleGenerator {
    /// Stable identity token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TeiLexicalScanner => ORACLE_GENERATOR_ID,
        }
    }
}

/// The backend that produced the preserved oracle bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleBackend {
    #[serde(rename = "java-grobid-crf")]
    JavaGrobidCrf,
}

impl OracleBackend {
    /// Stable identity token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JavaGrobidCrf => ORACLE_BACKEND_ID,
        }
    }
}

// ───────────────────────── receipt ─────────────────────────

/// Binding of the receipt to a carrier it was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierBinding {
    /// Repository-relative carrier path.
    pub manifest: String,
    /// Pointer inside the carrier.
    pub pointer: String,
    /// sha256 of the exact carrier bytes this receipt was derived from.
    pub revision_sha256: String,
}

/// Guard provenance: whether a Java-consult observer was installed on the real
/// transport seam, whether it is identity-isolated, and what it measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardProvenance {
    /// An observer was installed on `ParseBackend::fetch_tei`.
    pub installed: bool,
    /// Installing the observer did not rotate the cache identity.
    pub identity_isolated: bool,
    /// Consult attempts the installed observer measured. Zero is a measurement.
    pub observed_java_calls: u64,
}

/// The guard row as emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardProvenanceReceipt {
    /// The seam the guard observes.
    pub observer: GuardObserver,
    /// An observer was installed.
    pub installed: bool,
    /// Installing it did not rotate the cache identity.
    pub identity_isolated: bool,
    /// Measured Java consult attempts.
    pub observed_java_calls: u64,
}

/// R128 separation: the bibliography families never pool with the Java
/// `citation_names` population.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolingReceipt {
    /// Always `false`; `true` is a refusal.
    pub citation_names_pooled_with_biblstruct: bool,
}

/// Redaction contract; every flag is the invariant the artifact must satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redaction {
    pub raw_text: bool,
    pub raw_pdf: bool,
    pub tei_xml: bool,
    pub images: bool,
    pub physical_paths: bool,
    pub hostname: bool,
    pub executable: bool,
}

impl Redaction {
    /// The only redaction state a receipt may carry: nothing leaked.
    pub const fn clean() -> Self {
        Self {
            raw_text: false,
            raw_pdf: false,
            tei_xml: false,
            images: false,
            physical_paths: false,
            hostname: false,
            executable: false,
        }
    }
}

/// One family's typed outcome for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyCell {
    /// The family this cell measures.
    pub family: NativeFamilyId,
    /// The typed state.
    pub disposition: FamilyDisposition,
    /// The atom disposition, when the band executed.
    pub atom_disposition: Option<AtomDisposition>,
    /// Measured recognized count. `None` alongside a non-`observed` disposition:
    /// a typed absence is never a zero.
    pub observed_count: Option<u64>,
    /// Measured item count (fields/segments), when the atom reports one.
    pub observed_item_count: Option<u64>,
    /// sha256 over the band's normalized recognized values.
    pub observed_digest: Option<String>,
    /// Expected count from the independent oracle, when one exists.
    pub oracle_count: Option<u64>,
    /// The typed refusal, when the band refused.
    pub refusal: Option<RefusalReason>,
}

/// One region's character-coverage row for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionCoverageRow {
    /// The region compared.
    pub region: CoverageRegion,
    /// Distinct normalized alphanumeric characters the oracle carried.
    pub oracle_char_count: u64,
    /// Distinct normalized alphanumeric characters the native region carried.
    pub native_char_count: u64,
    /// Oracle characters the native region did not carry.
    pub missing_char_count: u64,
    /// The typed verdict.
    pub verdict: CoverageVerdict,
    /// sha256 over the joined oracle region text.
    pub oracle_text_digest: String,
    /// sha256 over the joined native region text.
    pub native_text_digest: String,
}

/// One document's typed matrix row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentReceipt {
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
    /// One cell per family, in canonical family order.
    pub families: Vec<FamilyCell>,
    /// One coverage row per region, in canonical region order.
    pub regions: Vec<RegionCoverageRow>,
    /// sha256 over the native pipeline digests (projection, heading, carrier).
    pub pipeline_digest: String,
    /// sha256 over the normalized oracle field values.
    pub oracle_digest: String,
}

/// One pinned document identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentIdentity {
    pub id: String,
    pub source_pdf: String,
    pub sha256: String,
    pub pages: u64,
}

/// The typed population matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationReceipt {
    /// Which population this is.
    pub population_id: PopulationId,
    /// Whether it is a corpus slice or an exclusion denominator.
    pub population_kind: PopulationKind,
    /// Repository-relative carrier of the pin.
    pub manifest: String,
    /// Pointer inside the carrier.
    pub pointer: String,
    /// sha256 of the carrier bytes.
    pub revision_sha256: String,
    /// Pinned document count.
    pub document_count: u64,
    /// Pinned page denominator.
    pub page_denominator: u64,
    /// Pinned document identities, in carrier order.
    pub documents: Vec<DocumentIdentity>,
    /// One typed matrix row per pinned document, in carrier order.
    pub matrix: Vec<DocumentReceipt>,
}

/// One oracle fixture binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleDocument {
    /// Pinned corpus document id the fixture belongs to.
    pub document_id: String,
    /// Repository-relative preserved-TEI path.
    pub fixture_path: String,
    /// Bare sha256 pin of the preserved bytes.
    pub sha256: String,
    /// sha256 over the normalized oracle field values.
    pub oracle_digest: String,
}

/// The independent-oracle identity and per-document bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleReceipt {
    /// The oracle builder identity (must differ from the SUT generator).
    pub generator_id: OracleGenerator,
    /// The backend that produced the preserved bytes.
    pub backend: OracleBackend,
    /// Repository-relative carrier of the pins.
    pub manifest: String,
    /// sha256 of the pin carrier bytes.
    pub revision_sha256: String,
    /// Proven by the independence requirements, not asserted.
    pub independent: bool,
    /// The same-generator negative control was executed.
    pub same_generator_refused: bool,
    /// One binding per pinned corpus document.
    pub documents: Vec<OracleDocument>,
}

/// The emitted parity receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFamilyReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Which receipt this is.
    pub receipt_kind: ReceiptKind,
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
    pub oracle: OracleReceipt,
    /// The typed population matrix.
    pub populations: Vec<PopulationReceipt>,
    /// R128 separation.
    pub pooling: PoolingReceipt,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl NativeFamilyReceipt {
    /// Serialize the receipt canonically (pretty JSON, deterministic field and
    /// key order) and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, NativeFamilyReceiptError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes (used by the hermetic replay).
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, NativeFamilyReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_redacted(json: &str) -> Result<(), NativeFamilyReceiptError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(NativeFamilyReceiptError::RedactionViolation {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────── builder inputs ─────────────────────────

/// One pinned corpus document, as the declaration declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDocument {
    pub id: String,
    pub source_pdf: String,
    pub sha256: String,
    pub pages: u64,
}

/// The validated corpus pin the receipt is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredCorpus {
    pub manifest: String,
    pub pointer: String,
    pub revision_sha256: String,
    pub corpus_id: String,
    pub document_count: u64,
    pub page_denominator: u64,
    pub documents: Vec<DeclaredDocument>,
}

/// One document's observed pipeline output, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDocument {
    pub id: String,
    pub sha256: String,
    pub pages: u64,
    pub observed_pages: u64,
    pub families: Vec<FamilyCell>,
    pub regions: Vec<RegionCoverageRow>,
    pub pipeline_digest: String,
    pub oracle_digest: String,
}

/// The independent oracle, as the runner produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleInput {
    pub manifest: String,
    pub revision_sha256: String,
    /// Proven by `require_independent_*` on the real provenance.
    pub independent: bool,
    /// The same-generator negative control was executed and refused.
    pub same_generator_refused: bool,
    pub documents: Vec<OracleDocument>,
}

// ───────────────────────── builder ─────────────────────────

fn require_digest(field: &str, value: &str) -> Result<(), NativeFamilyReceiptError> {
    if is_sha256_digest(value) {
        Ok(())
    } else {
        Err(NativeFamilyReceiptError::InvalidDigest {
            field: field.to_owned(),
        })
    }
}

fn require_path(field: &str, value: &str) -> Result<(), NativeFamilyReceiptError> {
    if is_repo_relative_path(value) {
        Ok(())
    } else {
        Err(NativeFamilyReceiptError::InvalidPath {
            field: field.to_owned(),
        })
    }
}

fn index_of_family(family: NativeFamilyId) -> usize {
    NATIVE_FAMILY_ORDER
        .iter()
        .position(|known| *known == family)
        .expect("the family order covers the closed vocabulary")
}

fn index_of_region(region: CoverageRegion) -> usize {
    COVERAGE_REGION_ORDER
        .iter()
        .position(|known| *known == region)
        .expect("the region order covers the closed vocabulary")
}

/// Build the three-document parity receipt, refusing anything the declaration,
/// the guard or the oracle does not justify.
///
/// The builder is the single place the matrix invariants are enforced: every
/// pinned document is observed exactly once with its pinned sha256 and page
/// count, every document carries every family and region exactly once, an
/// `observed` cell carries a count and no refusal, a non-`observed` cell
/// carries no count, and the bibliography families are not pooled with the Java
/// `citation_names` population.
#[allow(clippy::too_many_arguments)]
pub fn build_parity_receipt(
    declaration_revision_sha256: &str,
    corpus: &DeclaredCorpus,
    locks: &BTreeMap<String, String>,
    claim_scope: &str,
    product_coverage_effect: &str,
    guard: GuardProvenance,
    oracle: &OracleInput,
    observed: &[ObservedDocument],
) -> Result<NativeFamilyReceipt, NativeFamilyReceiptError> {
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(NativeFamilyReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(NativeFamilyReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }

    // Locks: every required lock present with the exact declared value.
    let mut checked_locks = BTreeMap::new();
    for (lock, expected) in REQUIRED_LOCKS {
        let found = locks
            .get(lock)
            .ok_or_else(|| NativeFamilyReceiptError::LockMissing {
                lock: lock.to_owned(),
            })?;
        if found != expected {
            return Err(NativeFamilyReceiptError::LockDrift {
                lock: lock.to_owned(),
                expected: expected.to_owned(),
                found: found.clone(),
            });
        }
        checked_locks.insert(lock.to_owned(), found.clone());
    }

    // Guard: a counted zero needs an installed, identity-isolated guard.
    if !guard.installed {
        return Err(NativeFamilyReceiptError::GuardNotInstalled);
    }
    if !guard.identity_isolated {
        return Err(NativeFamilyReceiptError::GuardIdentityNotIsolated);
    }
    if guard.observed_java_calls > 0 {
        return Err(NativeFamilyReceiptError::ObservedJavaOnSlice {
            attempts: guard.observed_java_calls,
        });
    }

    // Declaration binding.
    require_path("declaration.manifest", &corpus.manifest)?;
    if corpus.manifest != DECLARATION_MANIFEST_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: DECLARATION_MANIFEST_REL.to_owned(),
            found: corpus.manifest.clone(),
        });
    }
    if corpus.pointer != CORPUS_POINTER {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.pointer".to_owned(),
            expected: CORPUS_POINTER.to_owned(),
            found: corpus.pointer.clone(),
        });
    }
    require_digest("declaration.revision_sha256", declaration_revision_sha256)?;

    // Corpus pin.
    if corpus.corpus_id.trim().is_empty() {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "corpus.corpus_id".to_owned(),
            expected: "a non-empty corpus id".to_owned(),
            found: "empty".to_owned(),
        });
    }
    require_digest("corpus.revision_sha256", &corpus.revision_sha256)?;
    if corpus.documents.len() as u64 != corpus.document_count {
        return Err(NativeFamilyReceiptError::CorpusCountDrift {
            expected: corpus.documents.len() as u64,
            found: corpus.document_count,
        });
    }
    let page_sum: u64 = corpus.documents.iter().map(|doc| doc.pages).sum();
    if page_sum != corpus.page_denominator {
        return Err(NativeFamilyReceiptError::CorpusPageDrift {
            expected: page_sum,
            found: corpus.page_denominator,
        });
    }

    // Oracle identity and independence.
    if !oracle.independent {
        return Err(NativeFamilyReceiptError::OracleNotIndependent);
    }
    if !oracle.same_generator_refused {
        return Err(NativeFamilyReceiptError::OracleIndependenceUnrefuted);
    }
    require_path("oracle.manifest", &oracle.manifest)?;
    if oracle.manifest != ORACLE_MANIFEST_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "oracle.manifest".to_owned(),
            expected: ORACLE_MANIFEST_REL.to_owned(),
            found: oracle.manifest.clone(),
        });
    }
    require_digest("oracle.revision_sha256", &oracle.revision_sha256)?;

    // Oracle fixture set must be exactly the pinned corpus, joined by the
    // normalized identity so the two id spellings cannot silently diverge.
    let mut oracle_by_key: BTreeMap<String, &OracleDocument> = BTreeMap::new();
    for fixture in &oracle.documents {
        require_path("oracle.fixture_path", &fixture.fixture_path)?;
        if !is_sha256_hex(&fixture.sha256) {
            return Err(NativeFamilyReceiptError::InvalidDigest {
                field: format!("oracle.fixture[{}].sha256", fixture.document_id),
            });
        }
        require_digest(
            &format!("oracle.fixture[{}].oracle_digest", fixture.document_id),
            &fixture.oracle_digest,
        )?;
        let key = normalize_document_identity(&fixture.document_id);
        if oracle_by_key.insert(key, fixture).is_some() {
            return Err(NativeFamilyReceiptError::DuplicateDocument {
                document: fixture.document_id.clone(),
            });
        }
    }
    for declared in &corpus.documents {
        let key = normalize_document_identity(&declared.id);
        if !oracle_by_key.contains_key(&key) {
            return Err(NativeFamilyReceiptError::MissingOracleDocument {
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
            return Err(NativeFamilyReceiptError::UnknownOracleDocument {
                document: fixture.document_id.clone(),
            });
        }
    }

    // Observed coverage: every pinned document exactly once, pinned identity.
    let mut observed_by_id: BTreeMap<&str, &ObservedDocument> = BTreeMap::new();
    for document in observed {
        let declared = corpus
            .documents
            .iter()
            .find(|declared| declared.id == document.id)
            .ok_or_else(|| NativeFamilyReceiptError::UnknownDocument {
                document: document.id.clone(),
            })?;
        if document.sha256 != declared.sha256 {
            return Err(NativeFamilyReceiptError::DocumentShaDrift {
                document: document.id.clone(),
                expected: declared.sha256.clone(),
                found: document.sha256.clone(),
            });
        }
        if document.pages != declared.pages || document.observed_pages != declared.pages {
            return Err(NativeFamilyReceiptError::PageCountDrift {
                document: document.id.clone(),
                expected: declared.pages,
                found: document.observed_pages,
            });
        }
        require_path("document.source_pdf", &declared.source_pdf)?;
        if !is_sha256_hex(&document.sha256) {
            return Err(NativeFamilyReceiptError::InvalidDigest {
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
        if observed_by_id.insert(&document.id, document).is_some() {
            return Err(NativeFamilyReceiptError::DuplicateDocument {
                document: document.id.clone(),
            });
        }
    }
    for declared in &corpus.documents {
        if !observed_by_id.contains_key(declared.id.as_str()) {
            return Err(NativeFamilyReceiptError::MissingDocument {
                document: declared.id.clone(),
            });
        }
    }

    // Typed matrix: every family and region exactly once per document, with
    // the count/disposition invariant.
    let mut matrix = Vec::with_capacity(corpus.documents.len());
    for declared in &corpus.documents {
        let document = observed_by_id
            .get(declared.id.as_str())
            .expect("observed coverage checked above");

        let mut families: Vec<FamilyCell> = Vec::with_capacity(NATIVE_FAMILY_ORDER.len());
        for family in NATIVE_FAMILY_ORDER {
            let mut matching = document
                .families
                .iter()
                .filter(|cell| cell.family == family);
            let cell = matching.next().ok_or_else(|| {
                NativeFamilyReceiptError::IncompleteFamilyMatrix {
                    document: declared.id.clone(),
                    family: family.as_str().to_owned(),
                }
            })?;
            if matching.next().is_some() {
                return Err(NativeFamilyReceiptError::IncompleteFamilyMatrix {
                    document: declared.id.clone(),
                    family: family.as_str().to_owned(),
                });
            }
            let has_count = cell.observed_count.is_some() || cell.observed_item_count.is_some();
            match cell.disposition {
                FamilyDisposition::Observed => {
                    if cell.observed_count.is_none() {
                        return Err(NativeFamilyReceiptError::ObservedWithoutCount {
                            document: declared.id.clone(),
                            family: family.as_str().to_owned(),
                        });
                    }
                    if cell.refusal.is_some() {
                        return Err(NativeFamilyReceiptError::RefusalOnObservedDisposition {
                            document: declared.id.clone(),
                            family: family.as_str().to_owned(),
                        });
                    }
                }
                _ => {
                    if has_count {
                        return Err(NativeFamilyReceiptError::CountOnNonObservedDisposition {
                            document: declared.id.clone(),
                            family: family.as_str().to_owned(),
                        });
                    }
                    if cell.disposition == FamilyDisposition::Refused && cell.refusal.is_none() {
                        return Err(NativeFamilyReceiptError::RefusalWithoutReason {
                            document: declared.id.clone(),
                            family: family.as_str().to_owned(),
                        });
                    }
                }
            }
            families.push(cell.clone());
        }
        families.sort_by_key(|cell| index_of_family(cell.family));

        let mut regions: Vec<RegionCoverageRow> = Vec::with_capacity(COVERAGE_REGION_ORDER.len());
        for region in COVERAGE_REGION_ORDER {
            let mut matching = document.regions.iter().filter(|row| row.region == region);
            let row = matching.next().ok_or_else(|| {
                NativeFamilyReceiptError::IncompleteRegionMatrix {
                    document: declared.id.clone(),
                    region: region.as_str().to_owned(),
                }
            })?;
            if matching.next().is_some() {
                return Err(NativeFamilyReceiptError::IncompleteRegionMatrix {
                    document: declared.id.clone(),
                    region: region.as_str().to_owned(),
                });
            }
            require_digest(
                &format!("document[{}].region_text_digest", declared.id),
                &row.oracle_text_digest,
            )?;
            require_digest(
                &format!("document[{}].native_text_digest", declared.id),
                &row.native_text_digest,
            )?;
            regions.push(row.clone());
        }
        regions.sort_by_key(|row| index_of_region(row.region));

        matrix.push(DocumentReceipt {
            id: declared.id.clone(),
            source_pdf: declared.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: declared.pages,
            observed_pages: document.observed_pages,
            families,
            regions,
            pipeline_digest: document.pipeline_digest.clone(),
            oracle_digest: document.oracle_digest.clone(),
        });
    }

    let population = PopulationReceipt {
        population_id: PopulationId::ThreeDocumentSlice,
        population_kind: PopulationKind::Corpus,
        manifest: corpus.manifest.clone(),
        pointer: corpus.pointer.clone(),
        revision_sha256: corpus.revision_sha256.clone(),
        document_count: corpus.document_count,
        page_denominator: corpus.page_denominator,
        documents: corpus
            .documents
            .iter()
            .map(|doc| DocumentIdentity {
                id: doc.id.clone(),
                source_pdf: doc.source_pdf.clone(),
                sha256: doc.sha256.clone(),
                pages: doc.pages,
            })
            .collect(),
        matrix,
    };

    let mut oracle_documents = oracle.documents.clone();
    oracle_documents.sort_by_key(|fixture| corpus_position(corpus, &fixture.document_id));

    Ok(NativeFamilyReceipt {
        schema_version: NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION.to_owned(),
        receipt_kind: ReceiptKind::ParityThreeDocumentSlice,
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks: checked_locks,
        declaration: CarrierBinding {
            manifest: corpus.manifest.clone(),
            pointer: DECLARATION_POINTER.to_owned(),
            revision_sha256: declaration_revision_sha256.to_owned(),
        },
        guard: GuardProvenanceReceipt {
            observer: GuardObserver::JavaTransportFetchTei,
            installed: guard.installed,
            identity_isolated: guard.identity_isolated,
            observed_java_calls: guard.observed_java_calls,
        },
        oracle: OracleReceipt {
            generator_id: OracleGenerator::TeiLexicalScanner,
            backend: OracleBackend::JavaGrobidCrf,
            manifest: oracle.manifest.clone(),
            revision_sha256: oracle.revision_sha256.clone(),
            independent: oracle.independent,
            same_generator_refused: oracle.same_generator_refused,
            documents: oracle_documents,
        },
        populations: vec![population],
        pooling: PoolingReceipt {
            citation_names_pooled_with_biblstruct: false,
        },
        redaction: Redaction::clean(),
    })
}

fn corpus_position(corpus: &DeclaredCorpus, document_id: &str) -> usize {
    let key = normalize_document_identity(document_id);
    corpus
        .documents
        .iter()
        .position(|declared| normalize_document_identity(&declared.id) == key)
        .unwrap_or(usize::MAX)
}

/// The set of normalized alphanumeric characters in `text`. Whitespace and
/// punctuation are granularity, not content.
pub fn char_set(text: &str) -> std::collections::BTreeSet<char> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Classify oracle-vs-native character coverage. An empty oracle is
/// `NotAssessable` and is not a pass.
pub fn classify_coverage(
    oracle: &std::collections::BTreeSet<char>,
    native: &std::collections::BTreeSet<char>,
) -> CoverageVerdict {
    if oracle.is_empty() {
        return CoverageVerdict::NotAssessable;
    }
    if oracle.is_subset(native) {
        CoverageVerdict::GranularityOnly
    } else {
        CoverageVerdict::ContentLoss
    }
}

// ─────────────────────────── independent re-read ───────────────────────────

/// The committed inputs a parity receipt must still match.
///
/// Every field is recomputed by the caller from the committed carrier bytes —
/// the declaration, the corpus pin and the preserved-TEI fixtures — and is
/// never copied from the receipt. [`validate_parity_receipt`] is the independent
/// re-read an audit drives: a receipt is a self-declaration until something
/// else rebuilds what it claims from the sources it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityExpectation {
    /// The declared corpus, read from the declaration carrier.
    pub corpus: DeclaredCorpus,
    /// sha256 of the preserved-TEI oracle carrier, recomputed from its bytes.
    pub oracle_revision_sha256: String,
    /// Bare sha256 of each preserved-TEI fixture, keyed by normalized document
    /// identity and recomputed from the fixture bytes.
    pub oracle_fixture_sha256: BTreeMap<String, String>,
}

/// Re-read a parity receipt against the committed sources it claims.
///
/// The pins, the corpus and the oracle fixtures come from
/// [`ParityExpectation`]; the measurement side comes from the receipt. The
/// receipt's own matrix is re-fed through [`build_parity_receipt`], so every
/// structural invariant the builder enforces is enforced again on the parsed
/// bytes, and any drift the builder cannot express — a raised threshold, a
/// pooled bibliography, a non-clean redaction flag, a stale pin — is refused by
/// name before the rebuild.
pub fn validate_parity_receipt(
    receipt: &NativeFamilyReceipt,
    expected: &ParityExpectation,
) -> Result<(), NativeFamilyReceiptError> {
    if receipt.schema_version != NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.receipt_kind != ReceiptKind::ParityThreeDocumentSlice {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "receipt_kind".to_owned(),
            expected: ReceiptKind::ParityThreeDocumentSlice.as_str().to_owned(),
            found: receipt.receipt_kind.as_str().to_owned(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(NativeFamilyReceiptError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(NativeFamilyReceiptError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(NativeFamilyReceiptError::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }

    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(NativeFamilyReceiptError::LockMissing {
                    lock: lock.to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(NativeFamilyReceiptError::LockDrift {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }

    // A counted zero is reachable only under an installed, identity-isolated
    // guard. This is the receipt-level half of the `Observed(0)` rule; the
    // per-cell half is re-enforced by the rebuild below.
    if receipt.guard.observer != GuardObserver::JavaTransportFetchTei {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "guard.observer".to_owned(),
            expected: GuardObserver::JavaTransportFetchTei.as_str().to_owned(),
            found: receipt.guard.observer.as_str().to_owned(),
        });
    }
    if !receipt.guard.installed {
        return Err(NativeFamilyReceiptError::GuardNotInstalled);
    }
    if !receipt.guard.identity_isolated {
        return Err(NativeFamilyReceiptError::GuardIdentityNotIsolated);
    }
    if receipt.guard.observed_java_calls > 0 {
        return Err(NativeFamilyReceiptError::ObservedJavaOnSlice {
            attempts: receipt.guard.observed_java_calls,
        });
    }

    if receipt.pooling.citation_names_pooled_with_biblstruct {
        return Err(NativeFamilyReceiptError::PoolingClaimed {
            field: "citation_names_pooled_with_biblstruct".to_owned(),
        });
    }
    if receipt.redaction != Redaction::clean() {
        return Err(NativeFamilyReceiptError::RedactionViolation {
            marker: "redaction flag set".to_owned(),
        });
    }

    // Carrier bindings against the recomputed pins. A receipt pinned to bytes
    // that are no longer the committed bytes is stale, not a re-pin.
    if receipt.declaration.manifest != expected.corpus.manifest {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: expected.corpus.manifest.clone(),
            found: receipt.declaration.manifest.clone(),
        });
    }
    if receipt.declaration.pointer != DECLARATION_POINTER {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.pointer".to_owned(),
            expected: DECLARATION_POINTER.to_owned(),
            found: receipt.declaration.pointer.clone(),
        });
    }
    if receipt.declaration.revision_sha256 != expected.corpus.revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: expected.corpus.revision_sha256.clone(),
            found: receipt.declaration.revision_sha256.clone(),
        });
    }
    if receipt.oracle.manifest != ORACLE_MANIFEST_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "oracle.manifest".to_owned(),
            expected: ORACLE_MANIFEST_REL.to_owned(),
            found: receipt.oracle.manifest.clone(),
        });
    }
    if receipt.oracle.revision_sha256 != expected.oracle_revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "oracle.revision_sha256".to_owned(),
            expected: expected.oracle_revision_sha256.clone(),
            found: receipt.oracle.revision_sha256.clone(),
        });
    }
    if receipt.oracle.generator_id != OracleGenerator::TeiLexicalScanner {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "oracle.generator_id".to_owned(),
            expected: OracleGenerator::TeiLexicalScanner.as_str().to_owned(),
            found: receipt.oracle.generator_id.as_str().to_owned(),
        });
    }
    if receipt.oracle.backend != OracleBackend::JavaGrobidCrf {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "oracle.backend".to_owned(),
            expected: OracleBackend::JavaGrobidCrf.as_str().to_owned(),
            found: receipt.oracle.backend.as_str().to_owned(),
        });
    }
    if !receipt.oracle.independent {
        return Err(NativeFamilyReceiptError::OracleNotIndependent);
    }
    if !receipt.oracle.same_generator_refused {
        return Err(NativeFamilyReceiptError::OracleIndependenceUnrefuted);
    }

    if receipt.populations.len() != 1 {
        return Err(NativeFamilyReceiptError::PopulationMismatch {
            population: format!("{} populations", receipt.populations.len()),
        });
    }
    let population = &receipt.populations[0];
    if population.population_id != PopulationId::ThreeDocumentSlice
        || population.population_kind != PopulationKind::Corpus
    {
        return Err(NativeFamilyReceiptError::PopulationMismatch {
            population: population.population_id.as_str().to_owned(),
        });
    }
    if population.manifest != expected.corpus.manifest {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "population.manifest".to_owned(),
            expected: expected.corpus.manifest.clone(),
            found: population.manifest.clone(),
        });
    }
    if population.pointer != CORPUS_POINTER {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "population.pointer".to_owned(),
            expected: CORPUS_POINTER.to_owned(),
            found: population.pointer.clone(),
        });
    }
    if population.revision_sha256 != expected.corpus.revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "population.revision_sha256".to_owned(),
            expected: expected.corpus.revision_sha256.clone(),
            found: population.revision_sha256.clone(),
        });
    }

    // The oracle fixtures must be the pinned preserved TEI bytes, joined by the
    // normalized identity so the two id spellings cannot silently diverge.
    for fixture in &receipt.oracle.documents {
        let key = normalize_document_identity(&fixture.document_id);
        match expected.oracle_fixture_sha256.get(&key) {
            None => {
                return Err(NativeFamilyReceiptError::UnknownOracleDocument {
                    document: fixture.document_id.clone(),
                });
            }
            Some(pin) if pin != &fixture.sha256 => {
                return Err(NativeFamilyReceiptError::DocumentShaDrift {
                    document: fixture.document_id.clone(),
                    expected: pin.clone(),
                    found: fixture.sha256.clone(),
                });
            }
            Some(_) => {}
        }
    }

    // Rebuild the canonical receipt from the expectation and the receipt's own
    // measurement side. Every structural invariant the builder enforces runs
    // again on the parsed bytes, and any residual field drift is refused rather
    // than passed through.
    let observed: Vec<ObservedDocument> = population
        .matrix
        .iter()
        .map(|document| ObservedDocument {
            id: document.id.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
            observed_pages: document.observed_pages,
            families: document.families.clone(),
            regions: document.regions.clone(),
            pipeline_digest: document.pipeline_digest.clone(),
            oracle_digest: document.oracle_digest.clone(),
        })
        .collect();
    let oracle = OracleInput {
        manifest: ORACLE_MANIFEST_REL.to_owned(),
        revision_sha256: expected.oracle_revision_sha256.clone(),
        independent: receipt.oracle.independent,
        same_generator_refused: receipt.oracle.same_generator_refused,
        documents: receipt.oracle.documents.clone(),
    };
    let rebuilt = build_parity_receipt(
        &expected.corpus.revision_sha256,
        &expected.corpus,
        &receipt.locks,
        &receipt.claim_scope,
        &receipt.product_coverage_effect,
        GuardProvenance {
            installed: receipt.guard.installed,
            identity_isolated: receipt.guard.identity_isolated,
            observed_java_calls: receipt.guard.observed_java_calls,
        },
        &oracle,
        &observed,
    )?;
    if rebuilt != *receipt {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "receipt".to_owned(),
            expected: "the canonical rebuild from the committed pins".to_owned(),
            found: "a drifted receipt".to_owned(),
        });
    }
    Ok(())
}

// ───────────────────── exclusion recovery (S04/T05) ─────────────────────

/// Schema version of the exclusion-recovery artifact.
pub const NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION: &str = "native-exclusion-recovery.v1";
/// Default artifact path of the exclusion-recovery receipt.
pub const EXCLUSION_RECOVERY_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_exclusion_recovery.json";
/// Repository-relative carrier of the holdout article manifest (the byte pins).
pub const HOLDOUT_MANIFEST_REL: &str = "data/parse/holdout_manifests.yaml";
/// Repository-relative carrier of the external holdout release probe.
pub const EXCLUSION_PROBE_REL: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
/// Pointer inside the probe carrier to the typed exception population.
pub const EXCLUSION_PROBE_POINTER: &str = "reference_population";
/// Repository-relative carrier of the absorption progress baseline.
pub const EXCLUSION_BASELINE_REL: &str = "artifacts/parse_compat/absorption_progress_baseline.json";
/// Pointer inside the baseline carrier to the exclusion denominator.
pub const EXCLUSION_BASELINE_POINTER: &str = "denominator";
/// The three typed baseline exclusion reasons, in canonical order.
pub const EXCLUSION_REASON_ORDER: [ExclusionReason; 3] = [
    ExclusionReason::HeaderDateAbsent,
    ExclusionReason::HeaderTitleAbsent,
    ExclusionReason::HeaderAuthorAbsent,
];
/// The three header fields each recovery row reports, in canonical order.
pub const RECOVERY_FIELD_ORDER: [RecoveryField; 3] = [
    RecoveryField::Date,
    RecoveryField::Title,
    RecoveryField::Author,
];

/// Whether an exclusion-recovery run covered the whole denominator or a bounded
/// prefix. The mode is carried so a smoke receipt can never be mistaken for the
/// corpus-bound full run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryMode {
    Full,
    Smoke,
}

impl RecoveryMode {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Smoke => "smoke",
        }
    }
}

/// The typed baseline exclusion reasons, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    HeaderDateAbsent,
    HeaderTitleAbsent,
    HeaderAuthorAbsent,
}

impl ExclusionReason {
    /// Stable snake_case token; it restates the probe/baseline vocabulary.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeaderDateAbsent => "header_date_absent",
            Self::HeaderTitleAbsent => "header_title_absent",
            Self::HeaderAuthorAbsent => "header_author_absent",
        }
    }

    /// The header field the reason is about.
    pub const fn field(self) -> RecoveryField {
        match self {
            Self::HeaderDateAbsent => RecoveryField::Date,
            Self::HeaderTitleAbsent => RecoveryField::Title,
            Self::HeaderAuthorAbsent => RecoveryField::Author,
        }
    }

    /// Parse the probe/baseline token, or `None` outside the closed vocabulary.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "header_date_absent" => Some(Self::HeaderDateAbsent),
            "header_title_absent" => Some(Self::HeaderTitleAbsent),
            "header_author_absent" => Some(Self::HeaderAuthorAbsent),
            _ => None,
        }
    }
}

/// The three header fields a recovery row reports, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryField {
    Date,
    Title,
    Author,
}

impl RecoveryField {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Title => "title",
            Self::Author => "author",
        }
    }

    /// The native Header atom this field is measured by in this cut, when one
    /// exists. `Title` has no native atom (the T02 band declares it out of
    /// scope), so a title field can never carry recognition evidence.
    pub const fn native_atom(self) -> Option<RecoveryFieldAtom> {
        match self {
            Self::Date => Some(RecoveryFieldAtom::Date),
            Self::Author => Some(RecoveryFieldAtom::PersonalName),
            Self::Title => None,
        }
    }
}

/// Typed per-field recovery state.
///
/// The vocabulary separates *recognizing a line* from *recovering a field*.
/// Atom recognition is not field attribution (D502): without the header CRF
/// decode (out of scope, D358) nothing binds a recognized line to a field, and
/// every pure atom accepts lines that belong to other fields (a digit-leading
/// date reads as an affiliation marker; a title reads as a personal name). So
/// [`FieldDisposition::Recovered`] is **refused by the builder** in this cut —
/// it stays a wire token so the vocabulary remains closed, but no producer may
/// emit it — and a field whose atom recognized lines is reported as
/// [`FieldDisposition::RecognitionOnly`], which is explicitly *not* a recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldDisposition {
    /// The field's value was recovered with an independent field attribution.
    /// No producer in this cut can justify one (D502), so the builder refuses
    /// it rather than let atom recognition masquerade as recovery.
    Recovered,
    /// The field's atom recognized at least one native Header line, but
    /// recognition is not field attribution, so the field is **not** recovered.
    /// The recognition is carried as diagnostic evidence only.
    RecognitionOnly,
    /// The field's atom recognized nothing, the region was an admitted typed
    /// absence, or the band/pipeline refused. Always carries a typed cause.
    NotRecovered,
    /// The field has no native input class in this cut; e.g. the T02 header band
    /// has no Title atom, so a title cannot be produced from it.
    OutOfScope,
}

impl FieldDisposition {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recovered => "recovered",
            Self::RecognitionOnly => "recognition_only",
            Self::NotRecovered => "not_recovered",
            Self::OutOfScope => "out_of_scope",
        }
    }
}

/// Typed per-document disposition against the exclusion reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    Recovered,
    NotRecovered,
    OutOfScope,
    TypedRefusal,
}

impl RecoveryDisposition {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recovered => "recovered",
            Self::NotRecovered => "not_recovered",
            Self::OutOfScope => "out_of_scope",
            Self::TypedRefusal => "typed_refusal",
        }
    }
}

/// Typed structural refusal of an exclusion-recovery row. Every variant is a
/// hard integrity failure: the runner writes the artifact and then exits
/// non-zero, so a refusal is never a quiet skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionRefusalReason {
    /// The probe names a document the manifest does not pin.
    IdentityNotInManifest,
    /// The manifest row is not `bytes_present`.
    StatusNotBytesPresent,
    /// The pinned PDF bytes cannot be read.
    PdfMissing,
    /// The observed PDF sha256 drifted from the manifest pin.
    PdfShaDrift,
    /// The probe restates a manifest revision other than the one on disk.
    ProbeRevisionMismatch,
    /// The baseline declares no denominator for a probe reason.
    BaselineReasonAbsent,
    /// The baseline denominator disagrees with the probe population.
    BaselineReasonMismatch,
    /// The frozen affiliation reference is absent or malformed.
    AffiliationReferenceMissing,
}

impl ExclusionRefusalReason {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityNotInManifest => "identity_not_in_manifest",
            Self::StatusNotBytesPresent => "status_not_bytes_present",
            Self::PdfMissing => "pdf_missing",
            Self::PdfShaDrift => "pdf_sha_drift",
            Self::ProbeRevisionMismatch => "probe_revision_mismatch",
            Self::BaselineReasonAbsent => "baseline_reason_absent",
            Self::BaselineReasonMismatch => "baseline_reason_mismatch",
            Self::AffiliationReferenceMissing => "affiliation_reference_missing",
        }
    }
}

/// The native header atom a recovery field is measured by. The receipt module
/// carries its own closed vocabulary because it must compile in the default
/// feature set, without `da-parse` (the band is `pdf-layout`-gated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryFieldAtom {
    Date,
    PersonalName,
    Affiliation,
    AffiliationLink,
}

impl RecoveryFieldAtom {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::PersonalName => "personal_name",
            Self::Affiliation => "affiliation",
            Self::AffiliationLink => "affiliation_link",
        }
    }
}

/// Typed cause of a field's non-recovery. Every non-`Recovered`, non-`OutOfScope`
/// field carries one; a `not_recovered` with no cause would be exactly the
/// silent zero this receipt exists to forbid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldNotRecoveredCause {
    /// No native atom exists for this field in this cut, so it cannot be
    /// recovered at all.
    NoNativeAtom,
    /// The field's atom recognized no line in the Header run.
    NoRecognition,
    /// The field's atom recognized lines, but recognition is not field
    /// attribution (D502): the CRF decode that would bind a line to a field is
    /// out of scope (D358), so recovery is not claimed. This is the
    /// recognition-only state, reported honestly as non-recovery.
    RecognitionWithoutFieldAttribution,
    /// The Header region admitted no tokens: a typed absence, not a zero.
    BandTypedAbsence,
    /// The header band refused to execute.
    BandRefused,
    /// A native pipeline step before the band refused.
    PipelineRefused,
    /// The row could not be measured at all (a structural refusal).
    StructuralRefusal,
}

impl FieldNotRecoveredCause {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoNativeAtom => "no_native_atom",
            Self::NoRecognition => "no_recognition",
            Self::RecognitionWithoutFieldAttribution => "recognition_without_field_attribution",
            Self::BandTypedAbsence => "band_typed_absence",
            Self::BandRefused => "band_refused",
            Self::PipelineRefused => "pipeline_refused",
            Self::StructuralRefusal => "structural_refusal",
        }
    }
}

/// Diagnostic, redaction-safe recognition evidence for one field. It records
/// what the field's atom recognized; it is **never** a recovery claim (D502).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRecognition {
    /// The atom that ran for this field.
    pub atom: RecoveryFieldAtom,
    /// Number of native Header lines the atom recognized.
    pub recognized_count: u64,
    /// sha256 of the normalized recognized-line text (redaction-safe). The text
    /// itself never reaches the artifact.
    pub evidence_digest: String,
    /// The other atoms that recognized at least one line in the same Header
    /// run: the D502 ambiguity (each pure atom accepts other fields' lines) made
    /// explicit for this field. Never a recovery proof.
    pub co_firing_atoms: Vec<RecoveryFieldAtom>,
    /// The co-firing atoms that recognized exactly the same normalized line set
    /// (equal evidence digest). A non-empty list is *sufficient* evidence that
    /// the atom's recognition is not field-specific; an empty list is not proof
    /// of attribution, because digest equality is sufficient, not necessary, for
    /// line-set overlap.
    pub same_line_set_atoms: Vec<RecoveryFieldAtom>,
}

/// One header field's typed recovery state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRecovery {
    pub field: RecoveryField,
    pub disposition: FieldDisposition,
    /// The typed cause of a non-recovery. `Some` for `RecognitionOnly` and
    /// `NotRecovered`; `None` for `Recovered`/`OutOfScope`.
    pub cause: Option<FieldNotRecoveredCause>,
    /// Diagnostic recognition evidence, present exactly when the field's atom
    /// recognized at least one line.
    pub recognition: Option<FieldRecognition>,
}

/// One pinned exclusion and the native header recovery observed for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExclusionRecoveryRow {
    /// The probe / manifest identity.
    pub document_id: String,
    /// The typed baseline reason the document was excluded for.
    pub reason: ExclusionReason,
    /// The probe's own identity digest. It is deliberately **not** the PDF byte
    /// hash (`identity_digest_is_byte_hash = false`), so it is carried as an
    /// identity leaf and never compared to the PDF sha256.
    pub probe_identity_sha256: String,
    /// The probe's identity artifact size, in bytes.
    pub probe_size_bytes: u64,
    /// Repository-relative PDF path, when the manifest pins one.
    pub source_pdf: Option<String>,
    /// Observed bare sha256 of the PDF bytes.
    pub pdf_sha256: Option<String>,
    /// The manifest's bare sha256 pin for the PDF.
    pub pdf_sha256_pinned: Option<String>,
    /// Observed PDF size in bytes.
    pub pdf_size_bytes: Option<u64>,
    /// sha256 over the native pipeline digests, when the band ran.
    pub pipeline_digest: Option<String>,
    /// The reason-scoped disposition.
    pub disposition: RecoveryDisposition,
    /// One entry per recovery field, in canonical order.
    pub fields: Vec<FieldRecovery>,
    /// The Header region admitted no tokens: a typed absence, not a zero.
    pub band_typed_absence: bool,
    /// The band's typed refusal, when it refused to execute.
    pub band_refusal: Option<RefusalReason>,
    /// The structural refusal, when the row could not be measured at all.
    pub refusal: Option<ExclusionRefusalReason>,
}

/// Recovered / not-recovered / out-of-scope / refused counts for one reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAggregate {
    pub reason: ExclusionReason,
    /// Rows observed for this reason in this run.
    pub denominator: u64,
    pub recovered: u64,
    pub not_recovered: u64,
    pub out_of_scope: u64,
    pub typed_refusal: u64,
}

/// The exclusion population this receipt measures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPopulation {
    pub population_id: PopulationId,
    pub population_kind: PopulationKind,
    pub mode: RecoveryMode,
    /// `true` only for a full run over the frozen denominator.
    pub assumed_complete: bool,
    /// The baseline's assigned population (610).
    pub assigned: u64,
    /// The frozen exclusion denominator (79).
    pub excluded: u64,
    /// The probe's measurable population (531).
    pub measurable: u64,
    /// Rows this run targeted (`excluded` for a full run, `N` for a smoke run).
    pub targeted: u64,
    /// The frozen baseline denominator per reason token.
    pub reasons: BTreeMap<String, u64>,
}

/// The frozen affiliation capability reference, carried verbatim so the
/// exclusion measurement cannot silently absorb it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffiliationReference {
    /// The baseline metric the reference belongs to.
    pub metric_id: String,
    /// Frozen numerator (3055).
    pub recovered: u64,
    /// Frozen denominator (7077).
    pub denominator: u64,
}

/// One hard integrity failure. The runner exits non-zero for each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityFailure {
    pub code: ExclusionRefusalReason,
    /// The document the failure is about, when it is document-scoped.
    pub document: Option<String>,
}

/// The emitted exclusion-recovery receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeExclusionRecoveryReceipt {
    pub schema_version: String,
    pub receipt_kind: ReceiptKind,
    pub claim_scope: String,
    pub product_coverage_effect: String,
    pub thresholds: String,
    pub locks: BTreeMap<String, String>,
    pub declaration: CarrierBinding,
    pub probe: CarrierBinding,
    pub baseline: CarrierBinding,
    pub guard: GuardProvenanceReceipt,
    pub affiliation_reference: AffiliationReference,
    pub population: RecoveryPopulation,
    pub aggregates: Vec<RecoveryAggregate>,
    pub recoveries: Vec<ExclusionRecoveryRow>,
    pub integrity_failures: Vec<IntegrityFailure>,
    pub redaction: Redaction,
}

impl NativeExclusionRecoveryReceipt {
    /// Serialize canonically and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, NativeFamilyReceiptError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes (hermetic replay).
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, NativeFamilyReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| {
            NativeFamilyReceiptError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })
    }
}

fn recompute_aggregates(rows: &[ExclusionRecoveryRow]) -> Vec<RecoveryAggregate> {
    EXCLUSION_REASON_ORDER
        .iter()
        .map(|reason| {
            let mut aggregate = RecoveryAggregate {
                reason: *reason,
                denominator: 0,
                recovered: 0,
                not_recovered: 0,
                out_of_scope: 0,
                typed_refusal: 0,
            };
            for row in rows.iter().filter(|row| row.reason == *reason) {
                aggregate.denominator += 1;
                match row.disposition {
                    RecoveryDisposition::Recovered => aggregate.recovered += 1,
                    RecoveryDisposition::NotRecovered => aggregate.not_recovered += 1,
                    RecoveryDisposition::OutOfScope => aggregate.out_of_scope += 1,
                    RecoveryDisposition::TypedRefusal => aggregate.typed_refusal += 1,
                }
            }
            aggregate
        })
        .collect()
}

/// Validate the negative cross-recognition control attached to a field's
/// recognition evidence.
///
/// The control is what makes the diagnostic evidence trustworthy rather than
/// decorative: the evidence must be about the field's *own* atom, and the two
/// atom lists must be well-formed (no self-reference, no repeats, and the
/// same-line-set subset drawn from the co-firing set). A malformed control is
/// refused, so a hand-edited artifact cannot smuggle a recovery proof in
/// through the diagnostic channel (D502).
fn require_recognition_control(
    row: &ExclusionRecoveryRow,
    field: RecoveryField,
    recognition: &FieldRecognition,
) -> Result<(), NativeFamilyReceiptError> {
    let inconsistent = || NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent {
        document: row.document_id.clone(),
        field: field.as_str().to_owned(),
    };
    if field.native_atom() != Some(recognition.atom) {
        return Err(inconsistent());
    }
    let well_formed = |atoms: &[RecoveryFieldAtom]| {
        let mut seen = std::collections::BTreeSet::new();
        atoms
            .iter()
            .all(|atom| *atom != recognition.atom && seen.insert(*atom))
    };
    if !well_formed(&recognition.co_firing_atoms) || !well_formed(&recognition.same_line_set_atoms)
    {
        return Err(inconsistent());
    }
    let co_firing: std::collections::BTreeSet<RecoveryFieldAtom> =
        recognition.co_firing_atoms.iter().copied().collect();
    if !recognition
        .same_line_set_atoms
        .iter()
        .all(|atom| co_firing.contains(atom))
    {
        return Err(inconsistent());
    }
    Ok(())
}

/// Validate one field's disposition against its typed cause and its recognition
/// evidence.
///
/// Recognition is not attribution (D502), so a `recovered` field is refused
/// outright (no attribution proof exists in this cut), and a
/// `recognition_only` field must carry real recognition evidence while
/// remaining an explicit non-recovery. A `not_recovered` field carries a typed
/// cause, never a silent zero, and an `out_of_scope` field carries no
/// recognition and names the absent atom as its cause.
fn require_recovery_field(
    row: &ExclusionRecoveryRow,
    entry: &FieldRecovery,
) -> Result<(), NativeFamilyReceiptError> {
    let document = row.document_id.clone();
    let field = entry.field.as_str().to_owned();
    match entry.disposition {
        FieldDisposition::Recovered => {
            Err(NativeFamilyReceiptError::ExclusionFieldRecoveryUnattributed { document, field })
        }
        FieldDisposition::RecognitionOnly => {
            if entry.cause != Some(FieldNotRecoveredCause::RecognitionWithoutFieldAttribution) {
                return Err(NativeFamilyReceiptError::ExclusionFieldCauseInconsistent {
                    document,
                    field,
                });
            }
            match &entry.recognition {
                Some(recognition) if recognition.recognized_count > 0 => {
                    require_recognition_control(row, entry.field, recognition)?;
                    require_digest(
                        &format!(
                            "recovery[{}].fields[{}].recognition.evidence_digest",
                            row.document_id,
                            entry.field.as_str()
                        ),
                        &recognition.evidence_digest,
                    )
                }
                _ => Err(
                    NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent {
                        document,
                        field,
                    },
                ),
            }
        }
        FieldDisposition::NotRecovered => {
            match entry.cause {
                Some(FieldNotRecoveredCause::RecognitionWithoutFieldAttribution) | None => {
                    return Err(NativeFamilyReceiptError::ExclusionFieldCauseInconsistent {
                        document,
                        field,
                    });
                }
                Some(_) => {}
            }
            if entry
                .recognition
                .as_ref()
                .is_some_and(|recognition| recognition.recognized_count > 0)
            {
                return Err(
                    NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent {
                        document,
                        field,
                    },
                );
            }
            Ok(())
        }
        FieldDisposition::OutOfScope => {
            if entry.recognition.is_some() {
                return Err(
                    NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent {
                        document,
                        field,
                    },
                );
            }
            if entry.cause != Some(FieldNotRecoveredCause::NoNativeAtom) {
                return Err(NativeFamilyReceiptError::ExclusionFieldCauseInconsistent {
                    document,
                    field,
                });
            }
            Ok(())
        }
    }
}

/// Build the exclusion-recovery receipt, refusing anything the manifest, the
/// probe, the baseline, the guard or the rows do not justify.
///
/// The builder is the single place the invariants are enforced: the frozen
/// denominator is exactly the three typed reasons; a full run covers every
/// excluded document exactly once; every row carries every recovery field in
/// canonical order; `recovered`/`not_recovered`/`out_of_scope` agrees with the
/// row's reason field; a typed refusal carries a refusal reason; and the
/// recomputed aggregates reproduce the frozen denominator per reason.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn build_exclusion_recovery_receipt(
    declaration_revision_sha256: &str,
    probe_revision_sha256: &str,
    baseline_revision_sha256: &str,
    locks: &BTreeMap<String, String>,
    claim_scope: &str,
    product_coverage_effect: &str,
    guard: GuardProvenance,
    population: &RecoveryPopulation,
    affiliation_reference: &AffiliationReference,
    rows: &[ExclusionRecoveryRow],
    integrity_failures: &[IntegrityFailure],
) -> Result<NativeExclusionRecoveryReceipt, NativeFamilyReceiptError> {
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(NativeFamilyReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(NativeFamilyReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }

    let mut checked_locks = BTreeMap::new();
    for (lock, expected) in REQUIRED_LOCKS {
        let found = locks
            .get(lock)
            .ok_or_else(|| NativeFamilyReceiptError::LockMissing {
                lock: lock.to_owned(),
            })?;
        if found != expected {
            return Err(NativeFamilyReceiptError::LockDrift {
                lock: lock.to_owned(),
                expected: expected.to_owned(),
                found: found.clone(),
            });
        }
        checked_locks.insert(lock.to_owned(), found.clone());
    }

    if !guard.installed {
        return Err(NativeFamilyReceiptError::GuardNotInstalled);
    }
    if !guard.identity_isolated {
        return Err(NativeFamilyReceiptError::GuardIdentityNotIsolated);
    }
    if guard.observed_java_calls > 0 {
        return Err(NativeFamilyReceiptError::ObservedJavaOnSlice {
            attempts: guard.observed_java_calls,
        });
    }

    require_digest("declaration.revision_sha256", declaration_revision_sha256)?;
    require_digest("probe.revision_sha256", probe_revision_sha256)?;
    require_digest("baseline.revision_sha256", baseline_revision_sha256)?;

    if population.population_id != PopulationId::ExclusionBaseline {
        return Err(NativeFamilyReceiptError::PopulationMismatch {
            population: population.population_id.as_str().to_owned(),
        });
    }
    if population.population_kind != PopulationKind::Exclusion {
        return Err(NativeFamilyReceiptError::PopulationMismatch {
            population: population.population_kind.as_str().to_owned(),
        });
    }

    for reason in EXCLUSION_REASON_ORDER {
        if !population.reasons.contains_key(reason.as_str()) {
            return Err(NativeFamilyReceiptError::ExclusionDenominatorAbsent {
                reason: reason.as_str().to_owned(),
            });
        }
    }
    for token in population.reasons.keys() {
        if ExclusionReason::parse(token).is_none() {
            return Err(NativeFamilyReceiptError::ExclusionUnknownReason {
                reason: token.clone(),
            });
        }
    }
    let reason_sum: u64 = population.reasons.values().sum();
    if reason_sum != population.excluded {
        return Err(NativeFamilyReceiptError::ExclusionDenominatorDrift {
            reason: "sum".to_owned(),
            expected: population.excluded,
            found: reason_sum,
        });
    }
    if population.assigned < population.excluded
        || population.assigned.saturating_sub(population.excluded) != population.measurable
    {
        return Err(NativeFamilyReceiptError::ExclusionDenominatorDrift {
            reason: "measurable".to_owned(),
            expected: population.assigned.saturating_sub(population.excluded),
            found: population.measurable,
        });
    }

    let covered = rows.len() as u64;
    match population.mode {
        RecoveryMode::Full => {
            if !population.assumed_complete
                || population.targeted != population.excluded
                || covered != population.excluded
            {
                return Err(NativeFamilyReceiptError::ExclusionPopulationCoverage {
                    expected: population.excluded,
                    found: covered,
                });
            }
        }
        RecoveryMode::Smoke => {
            if population.assumed_complete || covered != population.targeted {
                return Err(NativeFamilyReceiptError::ExclusionPopulationCoverage {
                    expected: population.targeted,
                    found: covered,
                });
            }
        }
    }

    let mut seen = std::collections::BTreeSet::new();
    for row in rows {
        if !seen.insert(row.document_id.clone()) {
            return Err(NativeFamilyReceiptError::ExclusionDuplicateDocument {
                document: row.document_id.clone(),
            });
        }
        if !is_sha256_hex(&row.probe_identity_sha256) {
            return Err(NativeFamilyReceiptError::InvalidDigest {
                field: format!("recovery[{}].probe_identity_sha256", row.document_id),
            });
        }
        if let Some(digest) = &row.pipeline_digest {
            require_digest(
                &format!("recovery[{}].pipeline_digest", row.document_id),
                digest,
            )?;
        }
        if let Some(path) = &row.source_pdf {
            require_path("recovery.source_pdf", path)?;
        }
        if row.fields.len() != RECOVERY_FIELD_ORDER.len() {
            return Err(NativeFamilyReceiptError::ExclusionFieldMatrix {
                document: row.document_id.clone(),
                field: "count".to_owned(),
            });
        }
        for field in RECOVERY_FIELD_ORDER {
            if row
                .fields
                .iter()
                .filter(|entry| entry.field == field)
                .count()
                != 1
            {
                return Err(NativeFamilyReceiptError::ExclusionFieldMatrix {
                    document: row.document_id.clone(),
                    field: field.as_str().to_owned(),
                });
            }
            let entry = row
                .fields
                .iter()
                .find(|entry| entry.field == field)
                .expect("the field matrix was checked above");
            require_recovery_field(row, entry)?;
        }
        match row.disposition {
            RecoveryDisposition::TypedRefusal => {
                if row.refusal.is_none() && row.band_refusal.is_none() {
                    return Err(NativeFamilyReceiptError::ExclusionRefusalMissing {
                        document: row.document_id.clone(),
                    });
                }
            }
            _ => {
                if row.refusal.is_some() || row.band_refusal.is_some() {
                    return Err(NativeFamilyReceiptError::ExclusionRefusalInconsistent {
                        document: row.document_id.clone(),
                    });
                }
            }
        }
        let reason_field = row.reason.field();
        let observed = row
            .fields
            .iter()
            .find(|entry| entry.field == reason_field)
            .map(|entry| entry.disposition)
            .expect("the field matrix was checked above");
        match row.disposition {
            // A row can only be `recovered` if its reason field is attributed,
            // and no field can be attributed in this cut (D502). Refuse the
            // claim rather than let atom recognition stand in for attribution.
            RecoveryDisposition::Recovered => {
                return Err(
                    NativeFamilyReceiptError::ExclusionFieldRecoveryUnattributed {
                        document: row.document_id.clone(),
                        field: reason_field.as_str().to_owned(),
                    },
                );
            }
            RecoveryDisposition::NotRecovered => {
                if !matches!(
                    observed,
                    FieldDisposition::NotRecovered | FieldDisposition::RecognitionOnly
                ) {
                    return Err(NativeFamilyReceiptError::ExclusionDispositionMismatch {
                        document: row.document_id.clone(),
                        expected: FieldDisposition::NotRecovered.as_str().to_owned(),
                        found: observed.as_str().to_owned(),
                    });
                }
            }
            RecoveryDisposition::OutOfScope => {
                if observed != FieldDisposition::OutOfScope {
                    return Err(NativeFamilyReceiptError::ExclusionDispositionMismatch {
                        document: row.document_id.clone(),
                        expected: FieldDisposition::OutOfScope.as_str().to_owned(),
                        found: observed.as_str().to_owned(),
                    });
                }
            }
            RecoveryDisposition::TypedRefusal => {
                // A refused band produced no recognition, so a refusal row
                // cannot carry a field the band would have had to attribute.
                if let Some(entry) = row.fields.iter().find(|entry| {
                    matches!(
                        entry.disposition,
                        FieldDisposition::RecognitionOnly | FieldDisposition::Recovered
                    )
                }) {
                    return Err(
                        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent {
                            document: row.document_id.clone(),
                            field: entry.field.as_str().to_owned(),
                        },
                    );
                }
            }
        }
    }

    let aggregates = recompute_aggregates(rows);
    for aggregate in &aggregates {
        let frozen = population
            .reasons
            .get(aggregate.reason.as_str())
            .copied()
            .unwrap_or_default();
        if population.mode == RecoveryMode::Full && aggregate.denominator != frozen {
            return Err(NativeFamilyReceiptError::ExclusionDenominatorDrift {
                reason: aggregate.reason.as_str().to_owned(),
                expected: frozen,
                found: aggregate.denominator,
            });
        }
    }

    if affiliation_reference.metric_id.trim().is_empty() || affiliation_reference.denominator == 0 {
        return Err(NativeFamilyReceiptError::AffiliationReferenceMissing);
    }

    Ok(NativeExclusionRecoveryReceipt {
        schema_version: NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION.to_owned(),
        receipt_kind: ReceiptKind::ExclusionRecoveryBaseline,
        claim_scope: claim_scope.to_owned(),
        product_coverage_effect: product_coverage_effect.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks: checked_locks,
        declaration: CarrierBinding {
            manifest: HOLDOUT_MANIFEST_REL.to_owned(),
            pointer: "articles".to_owned(),
            revision_sha256: declaration_revision_sha256.to_owned(),
        },
        probe: CarrierBinding {
            manifest: EXCLUSION_PROBE_REL.to_owned(),
            pointer: EXCLUSION_PROBE_POINTER.to_owned(),
            revision_sha256: probe_revision_sha256.to_owned(),
        },
        baseline: CarrierBinding {
            manifest: EXCLUSION_BASELINE_REL.to_owned(),
            pointer: EXCLUSION_BASELINE_POINTER.to_owned(),
            revision_sha256: baseline_revision_sha256.to_owned(),
        },
        guard: GuardProvenanceReceipt {
            observer: GuardObserver::JavaTransportFetchTei,
            installed: guard.installed,
            identity_isolated: guard.identity_isolated,
            observed_java_calls: guard.observed_java_calls,
        },
        affiliation_reference: affiliation_reference.clone(),
        population: population.clone(),
        aggregates,
        recoveries: rows.to_vec(),
        integrity_failures: integrity_failures.to_vec(),
        redaction: Redaction::clean(),
    })
}

/// The committed inputs an exclusion-recovery receipt must still match.
///
/// Every pin and the whole census are recomputed by the caller from the
/// committed carrier bytes (holdout manifest, external holdout release probe,
/// absorption baseline) and from the probe's exception list; nothing is copied
/// from the receipt. [`validate_exclusion_recovery_receipt`] is the independent
/// re-read of the sibling receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusionExpectation {
    /// sha256 of the holdout manifest carrier, recomputed from its bytes.
    pub declaration_revision_sha256: String,
    /// sha256 of the external holdout release probe, recomputed from its bytes.
    pub probe_revision_sha256: String,
    /// sha256 of the absorption baseline, recomputed from its bytes.
    pub baseline_revision_sha256: String,
    /// The frozen census, read from the probe and baseline carriers.
    pub population: RecoveryPopulation,
    /// The frozen affiliation reference (3055/7077), read from the baseline.
    pub affiliation_reference: AffiliationReference,
    /// The typed exception identities the probe pins, by document id.
    pub documents: std::collections::BTreeSet<String>,
}

/// Re-read an exclusion-recovery receipt against the committed sources it
/// claims.
///
/// Carrier pins, the frozen census, the affiliation reference and the typed
/// exception identities come from [`ExclusionExpectation`]; the rows come from
/// the receipt and are re-fed through [`build_exclusion_recovery_receipt`], so
/// the denominator, the field matrix and the D502 no-recovery rule are enforced
/// again. A shrunken denominator, an unknown or lost exception document, a
/// raised threshold, a stale pin or a product-coverage claim is refused by
/// name.
pub fn validate_exclusion_recovery_receipt(
    receipt: &NativeExclusionRecoveryReceipt,
    expected: &ExclusionExpectation,
) -> Result<(), NativeFamilyReceiptError> {
    if receipt.schema_version != NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.receipt_kind != ReceiptKind::ExclusionRecoveryBaseline {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "receipt_kind".to_owned(),
            expected: ReceiptKind::ExclusionRecoveryBaseline.as_str().to_owned(),
            found: receipt.receipt_kind.as_str().to_owned(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(NativeFamilyReceiptError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(NativeFamilyReceiptError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(NativeFamilyReceiptError::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }

    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(NativeFamilyReceiptError::LockMissing {
                    lock: lock.to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(NativeFamilyReceiptError::LockDrift {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }

    if receipt.guard.observer != GuardObserver::JavaTransportFetchTei {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "guard.observer".to_owned(),
            expected: GuardObserver::JavaTransportFetchTei.as_str().to_owned(),
            found: receipt.guard.observer.as_str().to_owned(),
        });
    }
    if !receipt.guard.installed {
        return Err(NativeFamilyReceiptError::GuardNotInstalled);
    }
    if !receipt.guard.identity_isolated {
        return Err(NativeFamilyReceiptError::GuardIdentityNotIsolated);
    }
    if receipt.guard.observed_java_calls > 0 {
        return Err(NativeFamilyReceiptError::ObservedJavaOnSlice {
            attempts: receipt.guard.observed_java_calls,
        });
    }

    if receipt.redaction != Redaction::clean() {
        return Err(NativeFamilyReceiptError::RedactionViolation {
            marker: "redaction flag set".to_owned(),
        });
    }

    // Carrier pins against the recomputed hashes.
    if receipt.declaration.manifest != HOLDOUT_MANIFEST_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.manifest".to_owned(),
            expected: HOLDOUT_MANIFEST_REL.to_owned(),
            found: receipt.declaration.manifest.clone(),
        });
    }
    if receipt.declaration.revision_sha256 != expected.declaration_revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: expected.declaration_revision_sha256.clone(),
            found: receipt.declaration.revision_sha256.clone(),
        });
    }
    if receipt.probe.manifest != EXCLUSION_PROBE_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "probe.manifest".to_owned(),
            expected: EXCLUSION_PROBE_REL.to_owned(),
            found: receipt.probe.manifest.clone(),
        });
    }
    if receipt.probe.pointer != EXCLUSION_PROBE_POINTER {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "probe.pointer".to_owned(),
            expected: EXCLUSION_PROBE_POINTER.to_owned(),
            found: receipt.probe.pointer.clone(),
        });
    }
    if receipt.probe.revision_sha256 != expected.probe_revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "probe.revision_sha256".to_owned(),
            expected: expected.probe_revision_sha256.clone(),
            found: receipt.probe.revision_sha256.clone(),
        });
    }
    if receipt.baseline.manifest != EXCLUSION_BASELINE_REL {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "baseline.manifest".to_owned(),
            expected: EXCLUSION_BASELINE_REL.to_owned(),
            found: receipt.baseline.manifest.clone(),
        });
    }
    if receipt.baseline.pointer != EXCLUSION_BASELINE_POINTER {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "baseline.pointer".to_owned(),
            expected: EXCLUSION_BASELINE_POINTER.to_owned(),
            found: receipt.baseline.pointer.clone(),
        });
    }
    if receipt.baseline.revision_sha256 != expected.baseline_revision_sha256 {
        return Err(NativeFamilyReceiptError::BindingDrift {
            field: "baseline.revision_sha256".to_owned(),
            expected: expected.baseline_revision_sha256.clone(),
            found: receipt.baseline.revision_sha256.clone(),
        });
    }

    // Rebuild from the frozen expectation and the receipt's bands. The builder
    // re-enforces the denominator partition, the field matrix, the refusal
    // consistency and the D502 no-recovery rule on the parsed bytes. It runs
    // before the identity-set check so a shrunken denominator is refused as the
    // coverage drift it is, not as a side effect of a swapped document.
    let rebuilt = build_exclusion_recovery_receipt(
        &expected.declaration_revision_sha256,
        &expected.probe_revision_sha256,
        &expected.baseline_revision_sha256,
        &receipt.locks,
        &receipt.claim_scope,
        &receipt.product_coverage_effect,
        GuardProvenance {
            installed: receipt.guard.installed,
            identity_isolated: receipt.guard.identity_isolated,
            observed_java_calls: receipt.guard.observed_java_calls,
        },
        &expected.population,
        &expected.affiliation_reference,
        &receipt.recoveries,
        &receipt.integrity_failures,
    )?;
    if rebuilt != *receipt {
        return Err(NativeFamilyReceiptError::SchemaDrift {
            field: "receipt".to_owned(),
            expected: "the canonical rebuild from the frozen census".to_owned(),
            found: "a drifted receipt".to_owned(),
        });
    }

    // No typed exception document may be silently swapped or lost: the receipt's
    // row set must be exactly the probe's exception identities.
    let mut seen = std::collections::BTreeSet::new();
    for row in &receipt.recoveries {
        if !expected.documents.contains(&row.document_id) {
            return Err(NativeFamilyReceiptError::UnknownDocument {
                document: row.document_id.clone(),
            });
        }
        if !seen.insert(row.document_id.clone()) {
            return Err(NativeFamilyReceiptError::ExclusionDuplicateDocument {
                document: row.document_id.clone(),
            });
        }
    }
    for document in &expected.documents {
        if !seen.contains(document) {
            return Err(NativeFamilyReceiptError::MissingDocument {
                document: document.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> DeclaredCorpus {
        DeclaredCorpus {
            manifest: DECLARATION_MANIFEST_REL.to_owned(),
            pointer: CORPUS_POINTER.to_owned(),
            revision_sha256: sha256_prefixed(b"layout-bakeoff"),
            corpus_id: "s13-same-input-pdf-corpus".to_owned(),
            document_count: 2,
            page_denominator: 5,
            documents: vec![
                DeclaredDocument {
                    id: "hub-2507.19457".to_owned(),
                    source_pdf: "data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf"
                        .to_owned(),
                    sha256: "a".repeat(64),
                    pages: 3,
                },
                DeclaredDocument {
                    id: "rlm-2512.24601".to_owned(),
                    source_pdf: "data/sources/arxiv/cs-ai/2512.24601/source/original.pdf"
                        .to_owned(),
                    sha256: "b".repeat(64),
                    pages: 2,
                },
            ],
        }
    }

    fn locks() -> BTreeMap<String, String> {
        REQUIRED_LOCKS
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn guard() -> GuardProvenance {
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
        }
    }

    fn oracle_for(corpus: &DeclaredCorpus) -> OracleInput {
        OracleInput {
            manifest: ORACLE_MANIFEST_REL.to_owned(),
            revision_sha256: sha256_prefixed(b"provenance"),
            independent: true,
            same_generator_refused: true,
            documents: corpus
                .documents
                .iter()
                .map(|doc| OracleDocument {
                    document_id: doc.id.clone(),
                    fixture_path: format!(
                        "crates/da-parse/tests/fixtures/preserved/{}.tei.xml",
                        doc.id
                    ),
                    sha256: "c".repeat(64),
                    oracle_digest: sha256_prefixed(doc.id.as_bytes()),
                })
                .collect(),
        }
    }

    fn observed_for(corpus: &DeclaredCorpus) -> Vec<ObservedDocument> {
        corpus
            .documents
            .iter()
            .map(|doc| {
                let families = NATIVE_FAMILY_ORDER
                    .iter()
                    .map(|family| FamilyCell {
                        family: *family,
                        disposition: FamilyDisposition::Observed,
                        atom_disposition: Some(AtomDisposition::Recognized),
                        observed_count: Some(1),
                        observed_item_count: None,
                        observed_digest: Some(sha256_prefixed(family.as_str().as_bytes())),
                        oracle_count: Some(1),
                        refusal: None,
                    })
                    .collect();
                let regions = COVERAGE_REGION_ORDER
                    .iter()
                    .map(|region| RegionCoverageRow {
                        region: *region,
                        oracle_char_count: 4,
                        native_char_count: 4,
                        missing_char_count: 0,
                        verdict: CoverageVerdict::GranularityOnly,
                        oracle_text_digest: sha256_prefixed(b"oracle"),
                        native_text_digest: sha256_prefixed(b"native"),
                    })
                    .collect();
                ObservedDocument {
                    id: doc.id.clone(),
                    sha256: doc.sha256.clone(),
                    pages: doc.pages,
                    observed_pages: doc.pages,
                    families,
                    regions,
                    pipeline_digest: sha256_prefixed(doc.id.as_bytes()),
                    oracle_digest: sha256_prefixed(doc.id.as_bytes()),
                }
            })
            .collect()
    }

    fn build() -> NativeFamilyReceipt {
        let corpus = corpus();
        build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .expect("the happy path builds")
    }

    #[test]
    fn canonical_bytes_are_reproducible_and_replay_exact() {
        let receipt = build();
        let first = receipt.artifact_json_bytes().unwrap();
        let second = receipt.artifact_json_bytes().unwrap();
        assert_eq!(first, second, "canonical serialization is stable");
        let parsed = NativeFamilyReceipt::parse_json_bytes(&first).unwrap();
        assert_eq!(parsed, receipt, "the artifact round-trips");
        assert_eq!(
            parsed.artifact_json_bytes().unwrap(),
            first,
            "re-serialization is byte-identical"
        );
    }

    #[test]
    fn receipt_declares_no_product_claim() {
        let receipt = build();
        assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
        assert_eq!(
            receipt.product_coverage_effect,
            PRODUCT_COVERAGE_EFFECT_NONE
        );
        assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
        assert!(!receipt.pooling.citation_names_pooled_with_biblstruct);
        assert_eq!(receipt.redaction, Redaction::clean());
    }

    #[test]
    fn redaction_refuses_a_leaked_marker() {
        assert!(assert_redacted("{ \"note\": \"<TEI\" }").is_err());
        assert!(assert_redacted("{ \"note\": \"http://127.0.0.1:8070\" }").is_err());
        assert!(assert_redacted("{ \"note\": \"/root/x\" }").is_err());
        assert!(assert_redacted("{ \"note\": \"clean\" }").is_ok());
    }

    #[test]
    fn claim_scope_drift_is_refused() {
        let corpus = corpus();
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            "product",
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::ClaimScopeDrift { .. }
        ));
    }

    #[test]
    fn product_coverage_claim_is_refused() {
        let corpus = corpus();
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            "covers_all",
            guard(),
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::ProductCoverageClaimed { .. }
        ));
    }

    #[test]
    fn lock_drift_is_refused() {
        let corpus = corpus();
        let mut locks = locks();
        locks.insert("graph_write".to_owned(), "allowed".to_owned());
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks,
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(error, NativeFamilyReceiptError::LockDrift { .. }));
    }

    #[test]
    fn guard_is_required_for_the_zero() {
        let corpus = corpus();
        let mut open = guard();
        open.installed = false;
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            open,
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(error, NativeFamilyReceiptError::GuardNotInstalled));

        let mut consulted = guard();
        consulted.observed_java_calls = 3;
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            consulted,
            &oracle_for(&corpus),
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::ObservedJavaOnSlice { attempts: 3 }
        ));
    }

    #[test]
    fn oracle_independence_is_required_and_unrefuted() {
        let corpus = corpus();
        let mut oracle = oracle_for(&corpus);
        oracle.independent = false;
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle,
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::OracleNotIndependent
        ));

        let mut oracle = oracle_for(&corpus);
        oracle.same_generator_refused = false;
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle,
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::OracleIndependenceUnrefuted
        ));
    }

    #[test]
    fn a_missing_family_cell_is_refused() {
        let corpus = corpus();
        let mut observed = observed_for(&corpus);
        observed[0]
            .families
            .retain(|cell| cell.family != NativeFamilyId::BibliographyCitationAnchors);
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::IncompleteFamilyMatrix { .. }
        ));
    }

    #[test]
    fn a_count_on_an_absence_is_refused() {
        let corpus = corpus();
        let mut observed = observed_for(&corpus);
        let cell = observed[0]
            .families
            .iter_mut()
            .find(|cell| cell.family == NativeFamilyId::BibliographyCitationAnchors)
            .unwrap();
        cell.disposition = FamilyDisposition::TypedAbsence;
        // count stays Some(1): a typed absence carrying a count is the defect.
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::CountOnNonObservedDisposition { .. }
        ));
    }

    #[test]
    fn an_unknown_document_and_a_sha_drift_are_refused() {
        let corpus = corpus();
        let mut observed = observed_for(&corpus);
        observed[0].id = "not-pinned".to_owned();
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::UnknownDocument { .. }
        ));

        let mut observed = observed_for(&corpus);
        observed[0].sha256 = "d".repeat(64);
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle_for(&corpus),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::DocumentShaDrift { .. }
        ));
    }

    #[test]
    fn a_missing_oracle_fixture_is_refused() {
        let corpus = corpus();
        let mut oracle = oracle_for(&corpus);
        oracle.documents.pop();
        let error = build_parity_receipt(
            &sha256_prefixed(b"layout-bakeoff"),
            &corpus,
            &locks(),
            CLAIM_SCOPE_DIAGNOSTIC_ONLY,
            PRODUCT_COVERAGE_EFFECT_NONE,
            guard(),
            &oracle,
            &observed_for(&corpus),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeFamilyReceiptError::MissingOracleDocument { .. }
        ));
    }

    #[test]
    fn doc_identity_normalization_joins_the_two_spellings() {
        assert_eq!(
            normalize_document_identity("hub-2507.19457"),
            normalize_document_identity("hub_2507.19457")
        );
        assert_eq!(
            normalize_document_identity("reca-2605.26525v1"),
            normalize_document_identity("reca_2605.26525v1")
        );
    }

    #[test]
    fn coverage_verdict_separates_granularity_from_loss() {
        let oracle = char_set("abcdef");
        assert_eq!(
            classify_coverage(&oracle, &char_set("abcdef")),
            CoverageVerdict::GranularityOnly
        );
        assert_eq!(
            classify_coverage(&oracle, &char_set("abc")),
            CoverageVerdict::ContentLoss
        );
        assert_eq!(
            classify_coverage(&std::collections::BTreeSet::new(), &char_set("abc")),
            CoverageVerdict::NotAssessable
        );
    }
}
