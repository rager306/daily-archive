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
pub const CORPUS_POINTER: &str = "corpus.documents";
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
    /// The declared three-document parity slice (this task).
    ParityThreeDocumentSlice,
}

impl ReceiptKind {
    /// Stable snake_case token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParityThreeDocumentSlice => "parity_three_document_slice",
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
