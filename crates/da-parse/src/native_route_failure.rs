//! Typed native-route failure recovery (S06/T04).
//!
//! T01's aggregate folds the S03 plane and the S04/S05 bands into one typed
//! document result and carries a band refusal as a closed *family* disposition.
//! Its module doc says the fine per-band failure class "is the responsibility
//! of the typed failure recovery layer, not of this aggregate" — this module is
//! that layer. Without it a refused band, a plane stage that refused, an absent
//! or malformed PDF input and an unknown document id are all indistinguishable
//! from a measurement at the route level, and a silent zero (or a silent Java
//! fallback) is one refactor away.
//!
//! Three properties are deliberate and load-bearing:
//!
//! - **A refusal is carried, never resolved.** Every non-measurement the route
//!   can record becomes a [`RouteFailure`] with a closed
//!   [`RouteRecoveryDisposition`] and a closed [`RouteFailureReason`]. The
//!   reason embeds the *actual* closed vocabulary of the band that refused
//!   ([`NativeHeaderInputError`], [`NativeBibliographyInputError`]) or T01's own
//!   closed band/plane/unmet vocabularies — no mirror, so neither can drift.
//! - **No retry, no Java.** This module classifies and returns; it never
//!   consults a backend, never opens a socket and never retries. The transport
//!   vocabulary ([`FailureCode`], [`FailurePhase`], [`BackendTerminationClass`])
//!   is *reused* rather than restated: the failure shape is expressed in the
//!   project's existing closed classifier, and [`RouteFailure::termination_class`]
//!   delegates to the same pure mapping the transport seam uses.
//! - **A zero is not a measurement.** The attempt report carries the installed
//!   observer's typed observation (mirrored so `NotObserved` can never be read
//!   as `0`) and the caller proves non-vacuity with a positive control.

use serde::{Deserialize, Serialize};

use crate::batch_budget::{BackendTerminationClass, classify_termination};
use crate::java_invocation::JavaInvocationObservation;
use crate::native_bibliography_native_input::NativeBibliographyInputError;
use crate::native_header_native_input::NativeHeaderInputError;
use crate::native_route_result::{
    NativeRouteBodyAxis, NativeRouteFamilyDisposition, NativeRoutePlaneFailure,
    NativeRouteRefusalReason, NativeRouteResult, NativeRouteResultError, NativeRouteStageStance,
    NativeRouteUnmetReason, NativeRouteValidationError,
};
use crate::transport::{FailureCode, FailurePhase, LimitKind};

/// Schema tag of the failure-recovery carrier.
pub const NATIVE_ROUTE_FAILURE_SCHEMA_VERSION: &str = "da-parse.native-route-failure/v1";

/// Closed recovery-disposition vocabulary, in declaration order.
///
/// `no_recovery_required` is the attempt-level "the route measured and nothing
/// was recorded" disposition; it is never carried by a [`RouteFailure`], so a
/// failure can never be read as a measurement.
pub const ROUTE_RECOVERY_DISPOSITION_VOCABULARY: [&str; 7] = [
    "no_recovery_required",
    "typed_refusal_recorded",
    "stage_not_qualified",
    "input_bytes_absent",
    "input_bytes_malformed",
    "unknown_document",
    "ceiling_exceeded",
];

/// Closed prefix vocabulary of [`RouteFailureReason::token`]. Every token the
/// classifier can emit begins with exactly one of these prefixes, so the reason
/// namespace stays machine-readable without restating the band vocabularies.
pub const ROUTE_FAILURE_REASON_PREFIX_VOCABULARY: [&str; 13] = [
    "plane",
    "header",
    "bibliography",
    "band",
    "unmet",
    "body_axis",
    "stage_not_qualified",
    "boundary_refused",
    "invalid_aggregate",
    "input_absent",
    "input_malformed",
    "unknown_document",
    "ceiling_exceeded",
];

/// Scope marker used as `stage_or_family` when the failure is a whole-cascade
/// boundary refusal rather than one stage or family.
pub const ROUTE_SCOPE_CASCADE: &str = "cascade";

/// Scope marker for a refusal of the whole S04 header band.
pub const ROUTE_SCOPE_HEADER_BAND: &str = "header_band";

/// Scope marker for a refusal of the whole S04 bibliography band.
pub const ROUTE_SCOPE_BIBLIOGRAPHY_BAND: &str = "bibliography_band";

/// Scope marker for a refusal of the reproduced S05 body axis.
pub const ROUTE_SCOPE_BODY_AXIS: &str = "body_axis";

/// Scope marker for an input-level refusal (bytes or document identity).
pub const ROUTE_SCOPE_DOCUMENT: &str = "document";

/// Retry is never part of this layer: the transport seam surfaces a refusal
/// honestly and the recovery layer only records it (the same discipline as
/// `FailureCode::Busy`, which is never retried internally).
pub const ROUTE_FAILURE_RETRY: &str = "forbidden";

/// The PDF magic prefix a route input must carry.
const PDF_MAGIC: &[u8] = b"%PDF-";

/// The PDF trailer a complete document carries inside its last kilobyte.
const PDF_TRAILER: &[u8] = b"%%EOF";

/// How many trailing bytes are searched for the trailer.
const PDF_TRAILER_WINDOW: usize = 1024;

/// Closed recovery disposition of one route failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRecoveryDisposition {
    /// The route measured; there is nothing to recover. Never carried by a
    /// [`RouteFailure`].
    NoRecoveryRequired,
    /// A band or stage recorded a closed typed negative. The disposition is
    /// "recorded", never "resolved": the route does not retry, repair or fall
    /// back to Java.
    TypedRefusalRecorded,
    /// A declared in-slice stage was not qualified by this run.
    StageNotQualified,
    /// The declared input bytes for the document are absent.
    InputBytesAbsent,
    /// The declared input bytes are present but are not a readable PDF.
    InputBytesMalformed,
    /// The document id is not in the declared corpus.
    UnknownDocument,
    /// A frozen operational ceiling was exceeded.
    CeilingExceeded,
}

impl RouteRecoveryDisposition {
    /// Stable snake_case token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            Self::NoRecoveryRequired => "no_recovery_required",
            Self::TypedRefusalRecorded => "typed_refusal_recorded",
            Self::StageNotQualified => "stage_not_qualified",
            Self::InputBytesAbsent => "input_bytes_absent",
            Self::InputBytesMalformed => "input_bytes_malformed",
            Self::UnknownDocument => "unknown_document",
            Self::CeilingExceeded => "ceiling_exceeded",
        }
    }

    /// Every disposition, in declaration order.
    pub const ALL: [Self; 7] = [
        Self::NoRecoveryRequired,
        Self::TypedRefusalRecorded,
        Self::StageNotQualified,
        Self::InputBytesAbsent,
        Self::InputBytesMalformed,
        Self::UnknownDocument,
        Self::CeilingExceeded,
    ];

    /// Whether the disposition is the attempt-level "nothing was recorded".
    pub fn is_no_recovery_required(self) -> bool {
        matches!(self, Self::NoRecoveryRequired)
    }
}

/// Why the route refused. Every member is either an existing closed route
/// vocabulary or a closed input-level condition this layer owns.
#[derive(Debug, Clone, PartialEq)]
pub enum RouteFailureReason {
    /// An S03 plane stage refused (T01 vocabulary).
    Plane(NativeRoutePlaneFailure),
    /// The S04 header band refused, named by its own closed cause.
    HeaderBand(NativeHeaderInputError),
    /// The S04 bibliography band refused, named by its own closed cause.
    BibliographyBand(NativeBibliographyInputError),
    /// Any other band refused: T01's closed band tag.
    Band(NativeRouteRefusalReason),
    /// A required family could not be met: T01's closed unmet vocabulary.
    Unmet(NativeRouteUnmetReason),
    /// The reproduced S05 body axis refused rather than measured.
    BodyAxis(NativeRouteRefusalReason),
    /// A declared in-slice stage was not qualified.
    StageNotQualified,
    /// The slice boundary does not classify the cascade.
    BoundaryRefused,
    /// The aggregate violated a closed structural invariant.
    InvalidAggregate,
    /// The declared input bytes are absent (a typed absence, never a zero).
    InputAbsent,
    /// The declared input bytes are not a readable PDF.
    InputMalformed,
    /// The document id is not in the declared corpus.
    UnknownDocument,
    /// A frozen operational ceiling was exceeded.
    CeilingExceeded,
}

impl RouteFailureReason {
    /// Stable token, prefixed by the closed [`ROUTE_FAILURE_REASON_PREFIX_VOCABULARY`]
    /// family and otherwise composed only of closed band/plane tokens.
    pub fn token(&self) -> String {
        match self {
            Self::Plane(stage) => format!("plane:{}", stage.token()),
            Self::HeaderBand(error) => format!("header:{}", header_band_token(error)),
            Self::BibliographyBand(error) => {
                format!("bibliography:{}", bibliography_band_token(error))
            }
            Self::Band(reason) => format!("band:{}", reason.token()),
            Self::Unmet(reason) => format!("unmet:{}", unmet_token(reason)),
            Self::BodyAxis(reason) => format!("body_axis:{}", reason.token()),
            Self::StageNotQualified => "stage_not_qualified".to_owned(),
            Self::BoundaryRefused => "boundary_refused".to_owned(),
            Self::InvalidAggregate => "invalid_aggregate".to_owned(),
            Self::InputAbsent => "input_absent".to_owned(),
            Self::InputMalformed => "input_malformed".to_owned(),
            Self::UnknownDocument => "unknown_document".to_owned(),
            Self::CeilingExceeded => "ceiling_exceeded".to_owned(),
        }
    }
}

/// The closed variant name of a header-band refusal. Exhaustive, so a new
/// [`NativeHeaderInputError`] variant breaks the build instead of drifting.
fn header_band_token(error: &NativeHeaderInputError) -> &'static str {
    match error {
        NativeHeaderInputError::NoAdmittedTokens { .. } => "no_admitted_tokens",
        NativeHeaderInputError::RegionNotRequested { .. } => "region_not_requested",
        NativeHeaderInputError::NoFeatureTokens => "no_feature_tokens",
        NativeHeaderInputError::MissingFontSize { .. } => "missing_font_size",
        NativeHeaderInputError::InvalidFontSize { .. } => "invalid_font_size",
        NativeHeaderInputError::TokenContainsWhitespace { .. } => "token_contains_whitespace",
        NativeHeaderInputError::FeatureRender(_) => "feature_render",
        NativeHeaderInputError::Serialization => "serialization",
    }
}

/// The closed variant name of a bibliography-band refusal.
fn bibliography_band_token(error: &NativeBibliographyInputError) -> &'static str {
    match error {
        NativeBibliographyInputError::NoAdmittedTokens { .. } => "no_admitted_tokens",
        NativeBibliographyInputError::RegionNotRequested { .. } => "region_not_requested",
        NativeBibliographyInputError::NoFeatureTokens => "no_feature_tokens",
        NativeBibliographyInputError::TokenContainsWhitespace { .. } => "token_contains_whitespace",
        NativeBibliographyInputError::AnchorScanNotNative { .. } => "anchor_scan_not_native",
        NativeBibliographyInputError::Feature(_) => "feature",
        NativeBibliographyInputError::Gazetteer(_) => "gazetteer",
        NativeBibliographyInputError::Serialization => "serialization",
    }
}

/// The closed variant name of a T01 unmet reason.
fn unmet_token(reason: &NativeRouteUnmetReason) -> &'static str {
    match reason {
        NativeRouteUnmetReason::RegionNoAdmittedTokens { .. } => "region_no_admitted_tokens",
        NativeRouteUnmetReason::HeaderAtom { .. } => "header_atom",
        NativeRouteUnmetReason::BibliographyAtom { .. } => "bibliography_atom",
        NativeRouteUnmetReason::Visual { .. } => "visual",
        NativeRouteUnmetReason::Callout { .. } => "callout",
        NativeRouteUnmetReason::Section { .. } => "section",
    }
}

/// One typed refusal of the native route.
///
/// `failure_class` / `failure_phase` are the project's existing closed
/// transport vocabulary, reused so the route does not invent a second failure
/// classification; the native route's own causes are carried in
/// [`Self::refusal_reason`] and its recovery action in [`Self::disposition`].
#[derive(Debug, Clone, PartialEq)]
pub struct RouteFailure {
    /// The document the refusal belongs to.
    pub document_id: String,
    /// The stage id, family token, or documented scope marker the refusal
    /// belongs to.
    pub stage_or_family: String,
    /// Closed transport classification of the refusal shape.
    pub failure_class: FailureCode,
    /// Closed transport phase of the refusal shape.
    pub failure_phase: FailurePhase,
    /// The closed recovery disposition. Never `no_recovery_required`.
    pub disposition: RouteRecoveryDisposition,
    /// The closed refusal reason.
    pub refusal_reason: RouteFailureReason,
}

impl RouteFailure {
    /// Build one typed route failure. The disposition must be a real failure
    /// disposition: `no_recovery_required` is refused by construction.
    pub fn new(
        document_id: impl Into<String>,
        stage_or_family: impl Into<String>,
        failure_class: FailureCode,
        failure_phase: FailurePhase,
        disposition: RouteRecoveryDisposition,
        refusal_reason: RouteFailureReason,
    ) -> Self {
        debug_assert!(
            !disposition.is_no_recovery_required(),
            "a route failure can never claim that no recovery is required"
        );
        Self {
            document_id: document_id.into(),
            stage_or_family: stage_or_family.into(),
            failure_class,
            failure_phase,
            disposition,
            refusal_reason,
        }
    }

    /// The closed refusal reason token.
    pub fn reason_token(&self) -> String {
        self.refusal_reason.token()
    }

    /// The closed disposition token.
    pub fn disposition_token(&self) -> &'static str {
        self.disposition.token()
    }

    /// The reused transport termination classification of this refusal's
    /// code/phase shape (see `batch_budget::classify_termination`).
    pub fn termination_class(&self) -> BackendTerminationClass {
        classify_termination(self.failure_class, self.failure_phase)
    }

    /// Every refusal reports `false`: a refusal is never a measurement.
    pub fn is_measured(&self) -> bool {
        false
    }
}

/// Typed, serde-round-trippable mirror of [`JavaInvocationObservation`].
///
/// The mirror exists for the same reason T01 mirrors the S05 verdict enums:
/// the source type is not serializable. `NotObserved` stays a typed absence, so
/// an uninstalled observer can never be read as "0 Java consults".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RouteJavaObservation {
    /// No observer was installed on the seam, so nothing was measured.
    NotObserved,
    /// An observer was installed; zero here is a real measurement.
    Observed {
        /// Transport-level Java consult attempts recorded.
        attempts: u64,
    },
}

impl RouteJavaObservation {
    /// Whether a number was measured at all.
    pub fn installed(self) -> bool {
        matches!(self, Self::Observed { .. })
    }

    /// The measured attempt count, or `None` for the typed absence.
    pub fn attempts(self) -> Option<u64> {
        match self {
            Self::Observed { attempts } => Some(attempts),
            Self::NotObserved => None,
        }
    }

    /// Stable token, closed by the exhaustive match.
    pub fn token(self) -> &'static str {
        match self {
            Self::NotObserved => "not_observed",
            Self::Observed { .. } => "observed",
        }
    }

    /// Whether an installed observer measured exactly zero consults.
    pub fn is_observed_zero(self) -> bool {
        matches!(self, Self::Observed { attempts: 0 })
    }
}

impl From<JavaInvocationObservation> for RouteJavaObservation {
    fn from(observation: JavaInvocationObservation) -> Self {
        match observation {
            JavaInvocationObservation::NotObserved => Self::NotObserved,
            JavaInvocationObservation::Observed { attempts } => Self::Observed { attempts },
        }
    }
}

/// One declared document's route input, as the corpus declaration pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteInputSpec<'a> {
    /// Every document id the declaration pins.
    pub declared_document_ids: &'a [String],
    /// The document this attempt is for.
    pub document_id: &'a str,
    /// The declared source bytes, or `None` when the carrier holds none.
    pub input_bytes: Option<&'a [u8]>,
}

/// Whether bytes look like a complete PDF: the `%PDF-` magic plus an `%%EOF`
/// trailer inside the last [`PDF_TRAILER_WINDOW`] bytes.
///
/// Pure and deterministic; a truncated file is refused rather than parsed.
pub fn is_well_formed_pdf(bytes: &[u8]) -> bool {
    if !bytes.starts_with(PDF_MAGIC) {
        return false;
    }
    let tail_start = bytes.len().saturating_sub(PDF_TRAILER_WINDOW);
    bytes[tail_start..]
        .windows(PDF_TRAILER.len())
        .any(|window| window == PDF_TRAILER)
}

/// Classify a declared route input before any work happens.
///
/// An empty declared list admits nothing, so every document id is unknown by
/// construction. Absent and malformed bytes are distinct dispositions: a
/// missing file is not a corrupt one, and neither is a silent zero.
pub fn classify_route_input(spec: &RouteInputSpec<'_>) -> Result<(), RouteFailure> {
    if !spec
        .declared_document_ids
        .iter()
        .any(|declared| declared == spec.document_id)
    {
        return Err(RouteFailure::new(
            spec.document_id,
            ROUTE_SCOPE_DOCUMENT,
            FailureCode::InvalidRequest,
            FailurePhase::MultipartEncode,
            RouteRecoveryDisposition::UnknownDocument,
            RouteFailureReason::UnknownDocument,
        ));
    }
    match spec.input_bytes {
        None => Err(input_failure(
            spec,
            FailureCode::NoContent,
            RouteRecoveryDisposition::InputBytesAbsent,
            RouteFailureReason::InputAbsent,
        )),
        Some([]) => Err(input_failure(
            spec,
            FailureCode::NoContent,
            RouteRecoveryDisposition::InputBytesAbsent,
            RouteFailureReason::InputAbsent,
        )),
        Some(bytes) if !is_well_formed_pdf(bytes) => Err(input_failure(
            spec,
            FailureCode::InvalidRequest,
            RouteRecoveryDisposition::InputBytesMalformed,
            RouteFailureReason::InputMalformed,
        )),
        Some(_) => Ok(()),
    }
}

fn input_failure(
    spec: &RouteInputSpec<'_>,
    failure_class: FailureCode,
    disposition: RouteRecoveryDisposition,
    refusal_reason: RouteFailureReason,
) -> RouteFailure {
    RouteFailure::new(
        spec.document_id,
        ROUTE_SCOPE_DOCUMENT,
        failure_class,
        FailurePhase::MultipartEncode,
        disposition,
        refusal_reason,
    )
}

/// Every non-measurement T01's aggregate recorded, in declared order: band
/// refusals and unmet families (each family cell), then unqualified stages (in
/// cascade order).
///
/// A `measured` family and a `not_observed` family produce no failure: the
/// former is a measurement, the latter is a typed absence the aggregate already
/// states truthfully.
pub fn route_failures_from_result(result: &NativeRouteResult) -> Vec<RouteFailure> {
    let document_id = result.identity.document_id.as_str();
    let mut failures = Vec::new();
    for cell in &result.families {
        match cell.disposition {
            NativeRouteFamilyDisposition::Refused { reason } => failures.push(RouteFailure::new(
                document_id,
                cell.family.token(),
                FailureCode::ConversionFailed,
                FailurePhase::ReadBody,
                RouteRecoveryDisposition::TypedRefusalRecorded,
                RouteFailureReason::Band(reason),
            )),
            NativeRouteFamilyDisposition::Unmet { reason } => failures.push(RouteFailure::new(
                document_id,
                cell.family.token(),
                FailureCode::ConversionFailed,
                FailurePhase::ReadBody,
                RouteRecoveryDisposition::TypedRefusalRecorded,
                RouteFailureReason::Unmet(reason),
            )),
            NativeRouteFamilyDisposition::Measured { .. }
            | NativeRouteFamilyDisposition::NotObserved => {}
        }
    }
    for stage in &result.stages {
        if stage.stance == NativeRouteStageStance::NotQualified {
            failures.push(RouteFailure::new(
                document_id,
                stage.stage_id.as_str(),
                FailureCode::ConversionFailed,
                FailurePhase::ReadBody,
                RouteRecoveryDisposition::StageNotQualified,
                RouteFailureReason::StageNotQualified,
            ));
        }
    }
    // The body axis is the one aggregate-level refusal outside the family and
    // stage axes, so it is mapped last and never left silent.
    if let NativeRouteBodyAxis::Refused { reason } = result.body_axis {
        failures.push(RouteFailure::new(
            document_id,
            ROUTE_SCOPE_BODY_AXIS,
            FailureCode::ConversionFailed,
            FailurePhase::ReadBody,
            RouteRecoveryDisposition::TypedRefusalRecorded,
            RouteFailureReason::BodyAxis(reason),
        ));
    }
    failures
}

/// Map a hard aggregate error into a typed route failure.
pub fn route_failure_from_aggregate_error(
    document_id: &str,
    error: &NativeRouteResultError,
) -> RouteFailure {
    match error {
        NativeRouteResultError::Plane { stage } => RouteFailure::new(
            document_id,
            stage.token(),
            FailureCode::ConversionFailed,
            FailurePhase::ReadBody,
            RouteRecoveryDisposition::TypedRefusalRecorded,
            RouteFailureReason::Plane(*stage),
        ),
        NativeRouteResultError::Boundary(_) => RouteFailure::new(
            document_id,
            ROUTE_SCOPE_CASCADE,
            FailureCode::InvalidRequest,
            FailurePhase::MultipartEncode,
            RouteRecoveryDisposition::TypedRefusalRecorded,
            RouteFailureReason::BoundaryRefused,
        ),
        NativeRouteResultError::Invalid(error) => RouteFailure::new(
            document_id,
            validation_scope(error),
            FailureCode::InvalidResponse,
            FailurePhase::ReadBody,
            RouteRecoveryDisposition::TypedRefusalRecorded,
            RouteFailureReason::InvalidAggregate,
        ),
    }
}

/// The offending stage/family token of a closed invariant violation.
fn validation_scope(error: &NativeRouteValidationError) -> String {
    match error {
        NativeRouteValidationError::SchemaMismatch { .. } => "schema".to_owned(),
        NativeRouteValidationError::StageMissing { stage_id }
        | NativeRouteValidationError::StageDuplicated { stage_id }
        | NativeRouteValidationError::StageOutsideCascade { stage_id }
        | NativeRouteValidationError::StageStanceMismatch { stage_id } => stage_id.clone(),
        NativeRouteValidationError::FamilyMissing { family }
        | NativeRouteValidationError::FamilyDuplicated { family } => (*family).to_owned(),
        NativeRouteValidationError::BoundaryInvalid(_) => ROUTE_SCOPE_CASCADE.to_owned(),
        NativeRouteValidationError::BodyGapUntyped { verdict } => (*verdict).to_owned(),
    }
}

/// Map a refused S04 header band into a typed route failure, named by the
/// band's own closed cause.
///
/// A region that admitted nothing is `no_content` (there was nothing to
/// convert); every other band refusal is a conversion failure.
pub fn route_failure_from_header_band(
    document_id: &str,
    error: &NativeHeaderInputError,
) -> RouteFailure {
    RouteFailure::new(
        document_id,
        ROUTE_SCOPE_HEADER_BAND,
        header_band_class(error),
        FailurePhase::ReadBody,
        RouteRecoveryDisposition::TypedRefusalRecorded,
        RouteFailureReason::HeaderBand(error.clone()),
    )
}

fn header_band_class(error: &NativeHeaderInputError) -> FailureCode {
    match error {
        NativeHeaderInputError::NoAdmittedTokens { .. }
        | NativeHeaderInputError::RegionNotRequested { .. } => FailureCode::NoContent,
        _ => FailureCode::ConversionFailed,
    }
}

/// Map a refused S04 bibliography band into a typed route failure, named by
/// the band's own closed cause.
pub fn route_failure_from_bibliography_band(
    document_id: &str,
    error: &NativeBibliographyInputError,
) -> RouteFailure {
    RouteFailure::new(
        document_id,
        ROUTE_SCOPE_BIBLIOGRAPHY_BAND,
        bibliography_band_class(error),
        FailurePhase::ReadBody,
        RouteRecoveryDisposition::TypedRefusalRecorded,
        RouteFailureReason::BibliographyBand(error.clone()),
    )
}

fn bibliography_band_class(error: &NativeBibliographyInputError) -> FailureCode {
    match error {
        NativeBibliographyInputError::NoAdmittedTokens { .. }
        | NativeBibliographyInputError::RegionNotRequested { .. } => FailureCode::NoContent,
        _ => FailureCode::ConversionFailed,
    }
}

/// Map an exceeded operational ceiling into a typed route failure. The phase
/// follows the bound: an outbound bound is a send-phase failure, an inbound one
/// is a body-read failure (the same split as `ParseBackendFailure::limit`).
pub fn route_failure_from_ceiling(
    document_id: &str,
    stage_or_family: &str,
    kind: LimitKind,
) -> RouteFailure {
    let phase = match kind {
        LimitKind::RequestBytes => FailurePhase::Send,
        LimitKind::ResponseBytes => FailurePhase::ReadBody,
    };
    RouteFailure::new(
        document_id,
        stage_or_family,
        FailureCode::LimitExceeded,
        phase,
        RouteRecoveryDisposition::CeilingExceeded,
        RouteFailureReason::CeilingExceeded,
    )
}

/// The typed report of one route attempt over one document.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteAttemptReport {
    /// The document this attempt was for.
    pub document_id: String,
    /// Every typed non-measurement, in a deterministic order.
    pub failures: Vec<RouteFailure>,
    /// `true` exactly when the aggregate produced a result with no recorded
    /// failure. A refusal is never measured.
    pub measured: bool,
    /// The Java-consult observation of the seam the attempt ran over.
    pub java: RouteJavaObservation,
}

impl RouteAttemptReport {
    /// The attempt-level recovery disposition: the first failure's disposition,
    /// or `no_recovery_required` exactly when nothing was recorded.
    pub fn recovery_disposition(&self) -> RouteRecoveryDisposition {
        match self.failures.first() {
            Some(failure) => failure.disposition,
            None => RouteRecoveryDisposition::NoRecoveryRequired,
        }
    }

    /// Whether the attempt measured.
    pub fn is_measured(&self) -> bool {
        self.measured
    }

    /// Refuse an attempt that claims to be measured while carrying failures.
    pub fn is_consistent(&self) -> bool {
        self.measured == self.failures.is_empty()
    }
}

/// Run one native route attempt: classify the declared input, build the
/// aggregate through `build`, and record every typed non-measurement.
///
/// The function itself never consults Java, never retries and never touches the
/// filesystem; `java` is the observation of the observer installed over the
/// seam, recorded verbatim so a refusal path can prove `observed_java_calls=0`
/// (a positive control is the caller's job — see the module tests).
pub fn attempt_native_route<Build>(
    spec: &RouteInputSpec<'_>,
    java: JavaInvocationObservation,
    build: Build,
) -> RouteAttemptReport
where
    Build: FnOnce() -> Result<NativeRouteResult, NativeRouteResultError>,
{
    let java = RouteJavaObservation::from(java);
    if let Err(failure) = classify_route_input(spec) {
        return RouteAttemptReport {
            document_id: spec.document_id.to_owned(),
            failures: vec![failure],
            measured: false,
            java,
        };
    }
    match build() {
        Ok(result) => {
            let failures = route_failures_from_result(&result);
            RouteAttemptReport {
                document_id: result.identity.document_id.clone(),
                measured: failures.is_empty(),
                failures,
                java,
            }
        }
        Err(error) => RouteAttemptReport {
            document_id: spec.document_id.to_owned(),
            failures: vec![route_failure_from_aggregate_error(spec.document_id, &error)],
            measured: false,
            java,
        },
    }
}
