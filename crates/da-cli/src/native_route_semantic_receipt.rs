//! M516-th0oby / S04 / T01 — per-document semantic-correctness receipt over the
//! expanded 12-document native route corpus.
//!
//! H14 (the composer's XML well-formedness gate) and H15 (the composer's
//! bibliography id/target integrity) were measured by M511 only on the three
//! sha256-pinned preserved TEI, where they collapsed into a single aggregate
//! (`TP=573`, `P=R=F1=1.000000`). S04 runs the *same two checks* on the
//! 12-document route corpus that S03 declared and publishes a **per-document**
//! receipt: every document carries a closed well-formedness reason and a closed
//! bibliography-target reason, so a mixed valid/absent population is readable as
//! named reasons instead of one aggregate verdict.
//!
//! Two emission surfaces are deliberately kept apart and never conflated:
//!
//! * the **composer** (measured in the example/test plane through
//!   `examples/support/native_compose.rs`) splices admitted native bands into a
//!   preserved Java base and owns H14/H15. The composer is the subject of every
//!   per-document row here; its `ReferenceLinkLedger` is the bibliography-target fact (`required_targets == 0` is a named
//!   `no_required_targets_declared`, never a pass).
//! * the **native route emitter** assembles TEI from the native plane alone and
//!   is disclosed, never judged, under [`NativeRouteEmissionScope`]: on the
//!   expanded corpus the header is a placeholder `<teiHeader/>` (`typed_absence`)
//!   and the emitted `reference_links` declare zero local targets on every
//!   document — a scope reason, never a well-formedness PASS.
//!
//! The module is pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2` (no
//! `da-parse`, no `pdf-layout`), so it and its contract test compile in the
//! default feature set. The composer call stays in the example/test plane (D381);
//! only the schema, the closed reason vocabulary, the builder and the validator
//! live here.
//!
//! The artifact is diagnostic only: `claim_scope = diagnostic_only`,
//! `product_coverage_effect = none`, `thresholds = not_accepted`, all five locks
//! closed. It publishes counts, digests, pinned identities and closed tokens
//! only — never raw TEI, never a field value — and
//! [`assert_semantic_receipt_redacted`] refuses an artifact that leaks one.

use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest, is_sha256_hex,
    sha256_prefixed,
};
use crate::native_route_threshold_candidates::ReferenceDocumentBinding;
use serde::{Deserialize, Serialize};
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::fmt;

/// Schema version of the emitted semantic-correctness receipt.
pub const NATIVE_ROUTE_SEMANTIC_RECEIPT_SCHEMA_VERSION: &str = "native-route-semantic-receipt/v1";
/// Default artifact path of the semantic-correctness receipt.
pub const DEFAULT_SEMANTIC_RECEIPT_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_route_semantic_receipt.json";
/// The committed native route emission envelope the scope section is read from.
pub const DEFAULT_EMISSION_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_route_tei_emission.json";
/// The declared corpus block carrying the route document population.
pub const ROUTE_CORPUS_POINTER: &str =
    "data/parse/layout_bakeoff.yaml#native_route_corpus.documents";
/// The sibling DATA block binding each route document to its reference TEI.
pub const ROUTE_REFERENCE_POINTER: &str =
    "data/parse/layout_bakeoff.yaml#native_route_threshold_reference.documents";
/// The layout carrier both pointers resolve inside.
pub const LAYOUT_REL: &str = "data/parse/layout_bakeoff.yaml";

/// The rich-profile preserved fixture population (`fixtures/preserved/`).
pub const PROFILE_PRESERVED_RICH: &str = "preserved_rich";
/// The structural-profile holdout fixture population (`fixtures/holdout/`).
pub const PROFILE_HOLDOUT_STRUCTURAL: &str = "holdout_structural";

/// The composer accepted the document and its own gate re-read it as well formed.
pub const WF_EMISSION_PARSED_WELL_FORMED: &str = "emission_parsed_well_formed";
/// The emission was not well formed. The composer refuses this case itself
/// (`ComposedOutputNotWellFormed`), so a measured row never carries it; a receipt
/// that claims it is internally inconsistent and is refused.
pub const WF_EMISSION_NOT_WELL_FORMED: &str = "emission_not_well_formed";
/// Prefix of a composer typed refusal; the suffix is the typed class token.
pub const WF_COMPOSER_REFUSAL_PREFIX: &str = "composer_typed_refusal:";

/// Every mandatory local target the composer derived resolved in the emission.
pub const BT_ALL_REQUIRED_TARGETS_RESOLVED: &str = "all_required_targets_resolved";
/// The composer derived no mandatory local `#`-target from the source document.
pub const BT_NO_REQUIRED_TARGETS_DECLARED: &str = "no_required_targets_declared";
/// Prefix of the unresolved-target refusal; the suffix is the unresolved count.
pub const BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX: &str = "unresolved_required_targets:";

/// The admitted band was available for the document.
pub const AB_ACCEPTED: &str = "accepted";
/// Prefix of an unavailable admitted band; the suffix is a closed reason token.
pub const AB_UNAVAILABLE_PREFIX: &str = "admitted_band_unavailable:";

/// The composer accepted the document.
pub const COMPOSER_ACCEPTED: &str = "accepted";
/// The composer refused the document.
pub const COMPOSER_REFUSED: &str = "refused";
/// Closed band-unavailability reason: the source scan could not be measured.
pub const BAND_MEASURE_SOURCE_FAILED: &str = "measure_source_failed";
/// Closed band-unavailability reason: the typed parsers could not admit the band.
pub const BAND_BUILD_ADMITTED_BAND_FAILED: &str = "build_admitted_band_failed";

/// Native route emission scope: the header band was a typed absence.
pub const SCOPE_HEADER_TYPED_ABSENCE: &str = "header_typed_absence";
/// Native route emission scope: the header band was native-emitted.
pub const SCOPE_HEADER_NATIVE_EMITTED: &str = "header_native_emitted";
/// Native route emission scope: the references band was native-emitted.
pub const SCOPE_REFERENCES_NATIVE_EMITTED: &str = "references_native_emitted";
/// Native route emission scope: the references band was a typed absence.
pub const SCOPE_REFERENCES_TYPED_ABSENCE: &str = "references_typed_absence";
/// Native route emission scope: the body band was native-emitted.
pub const SCOPE_BODY_NATIVE_EMITTED: &str = "body_native_emitted";
/// Native route emission scope: the body band was a typed absence.
pub const SCOPE_BODY_TYPED_ABSENCE: &str = "body_typed_absence";
/// Native route emission scope: the emission declares zero local targets.
pub const SCOPE_REFERENCE_LINKS_EMPTY_TARGETS: &str = "reference_links_empty_targets";
/// Native route emission scope: the emission declares at least one local target.
pub const SCOPE_REFERENCE_LINKS_DECLARED_TARGETS: &str = "reference_links_declared_targets";

/// The only aggregate-state token the population block may carry: this receipt
/// publishes per-document reasons and deliberately owns no aggregate verdict.
pub const NO_AGGREGATE_VERDICT: &str = "no_aggregate_verdict";

/// The closed disposition vocabulary of the native route emission families.
pub const EMISSION_NATIVE_EMITTED: &str = "native_emitted";
/// The closed disposition vocabulary of the native route emission families.
pub const EMISSION_TYPED_ABSENCE: &str = "typed_absence";

// ───────────────────────────── value types ─────────────────────────────

/// The composer's `ReferenceLinkLedger`, republished as counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLedgerView {
    /// Direct `biblStruct` children of the source `listBibl` the band republishes.
    pub source_entries: u64,
    /// Admitted native reference entries the band spliced in.
    pub admitted_references: u64,
    /// `ref type="bibr"` occurrences carrying a local `#`-fragment target.
    pub target_occurrences: u64,
    /// Distinct fragments those occurrences demand.
    pub required_targets: u64,
    /// Mandatory targets that resolve in the emitted document's id space.
    pub resolved_targets: u64,
    /// Distinct `xml:id` values in the emitted document.
    pub id_space: u64,
}

/// The admitted-band outcome for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedBandOutcome {
    /// `accepted` or `refused` (the band itself is all-or-nothing here).
    pub outcome: String,
    /// `accepted` or `admitted_band_unavailable:<closed-reason>`.
    pub reason: String,
}

/// The composer measurement for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposerMeasurement {
    /// `accepted` or `refused`.
    pub outcome: String,
    /// The typed refusal class when `outcome == refused`, else `null`.
    pub refusal_class: Option<String>,
    /// The closed well-formedness reason.
    pub well_formedness_reason: String,
    /// The closed bibliography-target reason.
    pub bibliography_target_reason: String,
    /// The composer's ledger when `outcome == accepted`, else `null`.
    pub ledger: Option<ReferenceLedgerView>,
}

/// One per-document semantic-correctness row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDocumentRow {
    /// The pinned route-corpus document id.
    pub document_id: String,
    /// `preserved_rich` or `holdout_structural`.
    pub profile_class: String,
    /// Bare 64-hex sha256 of the committed reference TEI.
    pub reference_sha256: String,
    /// The admitted-band outcome.
    pub admitted_band: AdmittedBandOutcome,
    /// The composer measurement.
    pub composer: ComposerMeasurement,
}

/// One native route emission family's disclosure for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmissionFamilyScope {
    /// `native_emitted` or `typed_absence`.
    pub disposition: String,
    /// The emitter's own reason token for a typed absence, else `null`.
    pub reason: Option<String>,
    /// The closed scope reason derived from `(family, disposition)`.
    pub scope_reason: String,
}

/// The native route emission's reference-link ledger for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLinksScope {
    /// Emitter-declared source bibliography entries.
    pub source_entries: u64,
    /// Emitter-declared admitted references.
    pub admitted_references: u64,
    /// Emitter-declared `ref type="bibr"` target occurrences.
    pub target_occurrences: u64,
    /// Emitter-declared distinct required targets.
    pub required_targets: u64,
    /// Emitter-declared resolved targets.
    pub resolved_targets: u64,
    /// Emitter-declared distinct `xml:id` values.
    pub id_space: u64,
    /// `reference_links_empty_targets` or `reference_links_declared_targets`.
    pub scope_reason: String,
}

/// One document's native route emission disclosure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmissionScopeRow {
    /// The pinned route-corpus document id.
    pub document_id: String,
    /// Header family disclosure.
    pub header: EmissionFamilyScope,
    /// References family disclosure.
    pub references: EmissionFamilyScope,
    /// Body family disclosure.
    pub body: EmissionFamilyScope,
    /// Reference-link ledger disclosure.
    pub reference_links: ReferenceLinksScope,
}

/// The native route emission scope section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteEmissionScope {
    /// The emission envelope this section discloses.
    pub pointer: String,
    /// `sha256:` digest of the committed emission bytes.
    pub sha256: String,
    /// The declared emission document count.
    pub document_count: u64,
    /// Per-document disclosures, in declaration order.
    pub documents: Vec<EmissionScopeRow>,
}

/// The corpus identity the receipt is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCorpus {
    /// The declared route corpus id.
    pub id: String,
    /// The declared document count.
    pub document_count: u64,
    /// The declared page denominator.
    pub page_denominator: u64,
    /// Repository-relative corpus pointer.
    pub corpus_pointer: String,
    /// Repository-relative reference-TEI pointer.
    pub reference_pointer: String,
    /// `sha256:` digest over the `(id, path, sha256)` reference bindings.
    pub documents_digest: String,
}

/// The population counters. Counts only — the receipt owns no aggregate verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPopulation {
    /// The declared document count.
    pub documents: u64,
    /// Documents whose admitted band was available.
    pub admitted_band_available: u64,
    /// Documents the composer accepted.
    pub composed: u64,
    /// Documents the composer refused.
    pub refused: u64,
    /// Documents whose composer ledger declared at least one required target.
    pub documents_with_required_targets: u64,
    /// Sum of `target_occurrences` over the measured ledgers.
    pub target_occurrences_total: u64,
    /// Sum of `required_targets` over the measured ledgers.
    pub required_targets_total: u64,
    /// Sum of `resolved_targets` over the measured ledgers.
    pub resolved_targets_total: u64,
    /// Always [`NO_AGGREGATE_VERDICT`]; any other value is refused.
    pub aggregate_verdict: String,
}

/// The emitted semantic-correctness receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRouteSemanticReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Claim scope; only `diagnostic_only` is emitted.
    pub claim_scope: String,
    /// Product-coverage effect; only `none` is emitted.
    pub product_coverage_effect: String,
    /// Closed acceptance state; only `not_accepted` is emitted.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// The bound corpus identity.
    pub corpus: SemanticCorpus,
    /// The population counters.
    pub population: SemanticPopulation,
    /// The per-document rows, in declaration order.
    pub documents: Vec<SemanticDocumentRow>,
    /// The native route emission disclosure.
    pub native_route_emission_scope: NativeRouteEmissionScope,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl NativeRouteSemanticReceipt {
    /// Serialize canonically (pretty JSON, one trailing newline) and refuse if any
    /// payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, SemanticReceiptRefusal> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            SemanticReceiptRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            SemanticReceiptRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        assert_semantic_receipt_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, SemanticReceiptRefusal> {
        serde_json::from_slice(bytes).map_err(|error| SemanticReceiptRefusal::MalformedCarrier {
            detail: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_semantic_receipt_redacted(json: &str) -> Result<(), SemanticReceiptRefusal> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(SemanticReceiptRefusal::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ─────────────────────────── measured + expected input ───────────────────────

/// One document as measured through the composer in the example/test plane.
///
/// The helper fills this; [`build_semantic_receipt`] turns it into a row with a
/// closed reason pair. The raw measurement never reaches the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredSemanticDocument {
    /// The pinned route-corpus document id.
    pub document_id: String,
    /// `preserved_rich` or `holdout_structural`.
    pub profile_class: String,
    /// Bare 64-hex sha256 of the committed reference TEI.
    pub reference_sha256: String,
    /// `accepted` or `refused`.
    pub admitted_band_outcome: String,
    /// `admitted_band_unavailable:<closed-reason>` when the band is unavailable.
    pub admitted_band_reason: String,
    /// `accepted` or `refused`.
    pub composer_outcome: String,
    /// The composer's typed refusal class when refused, else `None`.
    pub refusal_class: Option<String>,
    /// The composer's ledger when accepted, else `None`.
    pub ledger: Option<ReferenceLedgerView>,
}

/// The recomputed expectation a receipt is validated against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticReceiptExpectation {
    /// The declared route corpus id.
    pub corpus_id: String,
    /// The declared document count.
    pub corpus_document_count: u64,
    /// The declared page denominator.
    pub corpus_page_denominator: u64,
    /// `sha256:` digest over the reference bindings.
    pub corpus_documents_digest: String,
    /// The reference-TEI bindings, in declaration order.
    pub reference_documents: Vec<ReferenceDocumentBinding>,
    /// The emission envelope's repository-relative path.
    pub emission_path: String,
    /// `sha256:` digest of the committed emission bytes.
    pub emission_sha256: String,
    /// The emission's declared document count.
    pub emission_document_count: u64,
    /// The emission's per-document disclosure, in declaration order.
    pub emission_scope: Vec<EmissionScopeRow>,
}

// ───────────────────────────── refusals ─────────────────────────────

/// Typed refusals of the semantic-correctness receipt. Every variant is a
/// refusal, never a silent downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticReceiptRefusal {
    /// A carrier could not be read or parsed.
    MalformedCarrier { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A parsed field drifted from the recomputed carrier fact.
    BindingDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeNotDiagnostic { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The envelope claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
    /// A declared document has no measured row.
    DocumentAbsent { document_id: String },
    /// A row names a document the declaration does not bind.
    DocumentNotDeclared { document_id: String },
    /// The row set does not cover the declared population exactly.
    DocumentCountMismatch { expected: usize, found: usize },
    /// A population counter is unbound from the rows it must equal.
    UnboundDenominator {
        field: String,
        expected: u64,
        found: u64,
    },
    /// A row carries no per-document reason.
    PerDocumentReasonMissing { document_id: String },
    /// A row carries a reason outside the closed vocabulary.
    ReasonVocabularyViolation {
        document_id: String,
        field: String,
        found: String,
    },
    /// A row with an empty target population is presented as a pass.
    EmptyTargetsPresentedAsPass { document_id: String },
    /// A row with unresolved targets is presented as resolved.
    UnresolvedTargetsPresentedAsResolved { document_id: String },
    /// The native route emission's placeholder header is read as populated.
    PlaceholderHeaderReadAsPopulated { document_id: String },
    /// The receipt carries an aggregate verdict it does not own.
    AggregateVerdictClaimed { found: String },
    /// The corpus binding digest drifted from the committed carriers.
    CorpusDigestDrift { expected: String, found: String },
    /// The emission binding digest drifted from the committed bytes.
    EmissionDigestDrift { expected: String, found: String },
    /// The emission scope section disagrees with the committed emission bytes.
    EmissionScopeDrift {
        document_id: String,
        field: String,
        expected: String,
        found: String,
    },
}

impl fmt::Display for SemanticReceiptRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedCarrier { detail } => {
                write!(f, "semantic receipt: malformed carrier: {detail}")
            }
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "semantic receipt: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::BindingDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "semantic receipt: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::ClaimScopeNotDiagnostic { found } => write!(
                f,
                "semantic receipt: claim scope `{found}` is not `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "semantic receipt: product coverage `{found}` is not `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::ThresholdClaimed { found } => write!(
                f,
                "semantic receipt: envelope thresholds `{found}` are not `{THRESHOLDS_NOT_ACCEPTED}`"
            ),
            Self::LockOpened {
                lock,
                expected,
                found,
            } => write!(
                f,
                "semantic receipt: lock `{lock}` is open: expected `{expected}`, found `{found}`"
            ),
            Self::RedactionLeak { marker } => {
                write!(f, "semantic receipt: artifact leaked marker `{marker}`")
            }
            Self::DocumentAbsent { document_id } => {
                write!(f, "semantic receipt: `{document_id}` has no measured row")
            }
            Self::DocumentNotDeclared { document_id } => write!(
                f,
                "semantic receipt: `{document_id}` is not a declared route document"
            ),
            Self::DocumentCountMismatch { expected, found } => write!(
                f,
                "semantic receipt: expected {expected} document rows, found {found}"
            ),
            Self::UnboundDenominator {
                field,
                expected,
                found,
            } => write!(
                f,
                "semantic receipt: `{field}` is unbound: rows sum to {expected}, receipt says {found}"
            ),
            Self::PerDocumentReasonMissing { document_id } => write!(
                f,
                "semantic receipt: `{document_id}` carries no per-document reason"
            ),
            Self::ReasonVocabularyViolation {
                document_id,
                field,
                found,
            } => write!(
                f,
                "semantic receipt: `{document_id}.{field}` carries `{found}`, outside the closed vocabulary"
            ),
            Self::EmptyTargetsPresentedAsPass { document_id } => write!(
                f,
                "semantic receipt: `{document_id}` declares no required target but is presented as resolved"
            ),
            Self::UnresolvedTargetsPresentedAsResolved { document_id } => write!(
                f,
                "semantic receipt: `{document_id}` has unresolved targets but is presented as resolved"
            ),
            Self::PlaceholderHeaderReadAsPopulated { document_id } => write!(
                f,
                "semantic receipt: `{document_id}` reads a typed-absence header as populated"
            ),
            Self::AggregateVerdictClaimed { found } => write!(
                f,
                "semantic receipt: aggregate verdict `{found}` is not `{NO_AGGREGATE_VERDICT}`"
            ),
            Self::CorpusDigestDrift { expected, found } => write!(
                f,
                "semantic receipt: corpus digest drifted: expected `{expected}`, found `{found}`"
            ),
            Self::EmissionDigestDrift { expected, found } => write!(
                f,
                "semantic receipt: emission digest drifted: expected `{expected}`, found `{found}`"
            ),
            Self::EmissionScopeDrift {
                document_id,
                field,
                expected,
                found,
            } => write!(
                f,
                "semantic receipt: emission scope `{document_id}.{field}` drifted: expected \
                 `{expected}`, found `{found}`"
            ),
        }
    }
}

impl std::error::Error for SemanticReceiptRefusal {}

fn malformed(detail: impl Into<String>) -> SemanticReceiptRefusal {
    SemanticReceiptRefusal::MalformedCarrier {
        detail: detail.into(),
    }
}

fn drift(field: &str, expected: impl ToString, found: impl ToString) -> SemanticReceiptRefusal {
    SemanticReceiptRefusal::BindingDrift {
        field: field.to_owned(),
        expected: expected.to_string(),
        found: found.to_string(),
    }
}

// ──────────────────────── the closed reason vocabulary ──────────────────────

/// Whether `reason` is a closed well-formedness reason.
pub fn is_well_formedness_reason(reason: &str) -> bool {
    reason == WF_EMISSION_PARSED_WELL_FORMED
        || reason == WF_EMISSION_NOT_WELL_FORMED
        || reason
            .strip_prefix(WF_COMPOSER_REFUSAL_PREFIX)
            .is_some_and(|class| !class.is_empty())
}

/// Whether `reason` is a closed bibliography-target reason.
pub fn is_bibliography_target_reason(reason: &str) -> bool {
    reason == BT_ALL_REQUIRED_TARGETS_RESOLVED
        || reason == BT_NO_REQUIRED_TARGETS_DECLARED
        || reason
            .strip_prefix(BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX)
            .is_some_and(|count| count.parse::<u64>().is_ok())
        || reason
            .strip_prefix(WF_COMPOSER_REFUSAL_PREFIX)
            .is_some_and(|class| !class.is_empty())
}

/// The closed scope reason for one `(family, disposition)` pair.
pub fn family_scope_reason(family: &str, disposition: &str) -> Option<&'static str> {
    match (family, disposition) {
        ("header", "native_emitted") => Some(SCOPE_HEADER_NATIVE_EMITTED),
        ("header", "typed_absence") => Some(SCOPE_HEADER_TYPED_ABSENCE),
        ("references", "native_emitted") => Some(SCOPE_REFERENCES_NATIVE_EMITTED),
        ("references", "typed_absence") => Some(SCOPE_REFERENCES_TYPED_ABSENCE),
        ("body", "native_emitted") => Some(SCOPE_BODY_NATIVE_EMITTED),
        ("body", "typed_absence") => Some(SCOPE_BODY_TYPED_ABSENCE),
        _ => None,
    }
}

/// The closed scope reason for the emitter's reference-link ledger.
pub fn reference_links_scope_reason(
    required_targets: u64,
    target_occurrences: u64,
) -> &'static str {
    if required_targets == 0 && target_occurrences == 0 {
        SCOPE_REFERENCE_LINKS_EMPTY_TARGETS
    } else {
        SCOPE_REFERENCE_LINKS_DECLARED_TARGETS
    }
}

// ───────────────────────────── carrier parsing ─────────────────────────────

fn yaml_str<'a>(value: &'a Yaml, pointer: &str) -> Result<&'a str, SemanticReceiptRefusal> {
    value
        .as_str()
        .ok_or_else(|| malformed(format!("`{pointer}` is not a string")))
}

fn yaml_u64(value: &Yaml, pointer: &str) -> Result<u64, SemanticReceiptRefusal> {
    value
        .as_u64()
        .ok_or_else(|| malformed(format!("`{pointer}` is not an unsigned integer")))
}

fn json_u64(
    value: &serde_json::Value,
    key: &str,
    pointer: &str,
) -> Result<u64, SemanticReceiptRefusal> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| malformed(format!("`{pointer}.{key}` is not an unsigned integer")))
}

/// Read the native route emission envelope's disclosure for every document.
fn emission_scope_from_json(
    emission_json: &[u8],
) -> Result<(u64, Vec<EmissionScopeRow>), SemanticReceiptRefusal> {
    let value: serde_json::Value = serde_json::from_slice(emission_json)
        .map_err(|error| malformed(format!("emission envelope is not valid JSON: {error}")))?;
    let documents = value
        .get("documents")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| malformed("emission envelope carries no `documents` array"))?;
    let mut rows = Vec::with_capacity(documents.len());
    for (index, document) in documents.iter().enumerate() {
        let pointer = format!("documents[{index}]");
        let document_id = document
            .get("document_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| malformed(format!("`{pointer}.document_id` is absent")))?
            .to_owned();
        let emission = document
            .get("emission")
            .ok_or_else(|| malformed(format!("`{pointer}.emission` is absent")))?;
        let families = emission
            .get("families")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| malformed(format!("`{pointer}.emission.families` is absent")))?;
        let mut header = None;
        let mut references = None;
        let mut body = None;
        for family in families {
            let name = family
                .get("family")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    malformed(format!("`{pointer}.emission.families[].family` is absent"))
                })?;
            if !matches!(name, "header" | "references" | "body") {
                continue;
            }
            let disposition = family
                .get("disposition")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    malformed(format!(
                        "`{pointer}.emission.families[{name}].disposition` is absent"
                    ))
                })?
                .to_owned();
            let reason = family
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let scope_reason = family_scope_reason(name, &disposition)
                .ok_or_else(|| {
                    malformed(format!(
                        "`{pointer}.emission.families[{name}]` disposition `{disposition}` is outside the closed vocabulary"
                    ))
                })?
                .to_owned();
            let scope = EmissionFamilyScope {
                disposition,
                reason,
                scope_reason,
            };
            match name {
                "header" => header = Some(scope),
                "references" => references = Some(scope),
                "body" => body = Some(scope),
                _ => {}
            }
        }
        let links = emission
            .get("reference_links")
            .ok_or_else(|| malformed(format!("`{pointer}.emission.reference_links` is absent")))?;
        let links_pointer = format!("{pointer}.emission.reference_links");
        let source_entries = json_u64(links, "source_entries", &links_pointer)?;
        let admitted_references = json_u64(links, "admitted_references", &links_pointer)?;
        let target_occurrences = json_u64(links, "target_occurrences", &links_pointer)?;
        let required_targets = json_u64(links, "required_targets", &links_pointer)?;
        let resolved_targets = json_u64(links, "resolved_targets", &links_pointer)?;
        let id_space = json_u64(links, "id_space", &links_pointer)?;
        rows.push(EmissionScopeRow {
            document_id,
            header: header.ok_or_else(|| {
                malformed(format!("`{pointer}.emission` carries no header family"))
            })?,
            references: references.ok_or_else(|| {
                malformed(format!("`{pointer}.emission` carries no references family"))
            })?,
            body: body
                .ok_or_else(|| malformed(format!("`{pointer}.emission` carries no body family")))?,
            reference_links: ReferenceLinksScope {
                source_entries,
                admitted_references,
                target_occurrences,
                required_targets,
                resolved_targets,
                id_space,
                scope_reason: reference_links_scope_reason(required_targets, target_occurrences)
                    .to_owned(),
            },
        });
    }
    Ok((documents.len() as u64, rows))
}

/// The canonical `sha256:` digest over an ordered reference-binding list.
pub fn bindings_digest(bindings: &[ReferenceDocumentBinding]) -> String {
    let mut canonical = String::new();
    for binding in bindings {
        canonical.push_str(&binding.id);
        canonical.push('\t');
        canonical.push_str(&binding.path);
        canonical.push('\t');
        canonical.push_str(&binding.sha256);
        canonical.push('\n');
    }
    sha256_prefixed(canonical.as_bytes())
}

/// Recompute the expectation a receipt is validated against from the committed
/// carriers: the layout declaration and the native route emission envelope.
pub fn derive_semantic_receipt_expectation(
    layout_yaml: &str,
    emission_json: &[u8],
) -> Result<SemanticReceiptExpectation, SemanticReceiptRefusal> {
    let layout: Yaml = serde_yaml::from_str(layout_yaml)
        .map_err(|error| malformed(format!("layout_bakeoff.yaml is not valid YAML: {error}")))?;
    let corpus = layout
        .get("native_route_corpus")
        .ok_or_else(|| malformed("layout carries no `native_route_corpus` block"))?;
    let corpus_id = yaml_str(
        corpus
            .get("id")
            .ok_or_else(|| malformed("`native_route_corpus.id` is absent"))?,
        "native_route_corpus.id",
    )?
    .to_owned();
    let declared_count = yaml_u64(
        corpus
            .get("corpus_n")
            .ok_or_else(|| malformed("`native_route_corpus.corpus_n` is absent"))?,
        "native_route_corpus.corpus_n",
    )?;
    let page_denominator = yaml_u64(
        corpus
            .get("page_denominator")
            .ok_or_else(|| malformed("`native_route_corpus.page_denominator` is absent"))?,
        "native_route_corpus.page_denominator",
    )?;

    let bindings =
        crate::native_route_threshold_candidates::reference_document_bindings(layout_yaml)
            .map_err(|error| malformed(format!("reference bindings: {error}")))?;
    if bindings.is_empty() {
        return Err(malformed("the reference block binds no documents"));
    }
    if declared_count != bindings.len() as u64 {
        return Err(malformed(format!(
            "the declared corpus count {declared_count} does not match the {} reference bindings",
            bindings.len()
        )));
    }

    let (emission_document_count, emission_scope) = emission_scope_from_json(emission_json)?;

    Ok(SemanticReceiptExpectation {
        corpus_id,
        corpus_document_count: declared_count,
        corpus_page_denominator: page_denominator,
        corpus_documents_digest: bindings_digest(&bindings),
        reference_documents: bindings,
        emission_path: DEFAULT_EMISSION_ARTIFACT_REL.to_owned(),
        emission_sha256: sha256_prefixed(emission_json),
        emission_document_count,
        emission_scope,
    })
}

// ───────────────────────────── builder ─────────────────────────────

/// Derive the admitted-band outcome token from a measured band result.
pub fn admitted_band_outcome_token(available: bool, reason: &str) -> String {
    if available {
        AB_ACCEPTED.to_owned()
    } else {
        format!("{AB_UNAVAILABLE_PREFIX}{reason}")
    }
}

fn composer_reason_pair(
    outcome: &str,
    refusal_class: Option<&str>,
    ledger: Option<&ReferenceLedgerView>,
) -> Result<(String, String), SemanticReceiptRefusal> {
    match (outcome, ledger) {
        (COMPOSER_ACCEPTED, Some(ledger)) => {
            let well_formedness = WF_EMISSION_PARSED_WELL_FORMED.to_owned();
            let bibliography = if ledger.required_targets == 0 {
                BT_NO_REQUIRED_TARGETS_DECLARED.to_owned()
            } else if ledger.resolved_targets >= ledger.required_targets {
                BT_ALL_REQUIRED_TARGETS_RESOLVED.to_owned()
            } else {
                format!(
                    "{BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX}{}",
                    ledger.required_targets - ledger.resolved_targets
                )
            };
            Ok((well_formedness, bibliography))
        }
        (COMPOSER_REFUSED, None) => {
            let class = refusal_class.unwrap_or("");
            if class.is_empty() {
                return Err(malformed(
                    "a refused composer outcome must name its typed refusal class",
                ));
            }
            let reason = format!("{WF_COMPOSER_REFUSAL_PREFIX}{class}");
            Ok((reason.clone(), reason))
        }
        _ => Err(malformed(format!(
            "composer outcome `{outcome}` and its ledger are inconsistent"
        ))),
    }
}

/// Build the receipt from the recomputed expectation and the measured rows.
pub fn build_semantic_receipt(
    expectation: &SemanticReceiptExpectation,
    measured: &[MeasuredSemanticDocument],
) -> Result<NativeRouteSemanticReceipt, SemanticReceiptRefusal> {
    let mut rows = Vec::with_capacity(expectation.reference_documents.len());
    let mut admitted_band_available = 0u64;
    let mut composed = 0u64;
    let mut refused = 0u64;
    let mut documents_with_required_targets = 0u64;
    let mut target_occurrences_total = 0u64;
    let mut required_targets_total = 0u64;
    let mut resolved_targets_total = 0u64;

    for binding in &expectation.reference_documents {
        let measured = measured
            .iter()
            .find(|row| row.document_id == binding.id)
            .ok_or_else(|| SemanticReceiptRefusal::DocumentAbsent {
                document_id: binding.id.clone(),
            })?;
        if !is_sha256_hex(&measured.reference_sha256) {
            return Err(drift(
                &format!("{}.reference_sha256", binding.id),
                "<bare 64-hex sha256>",
                &measured.reference_sha256,
            ));
        }
        let band_available = measured.admitted_band_outcome == AB_ACCEPTED;
        if band_available {
            admitted_band_available += 1;
        }
        let (well_formedness_reason, bibliography_target_reason) = composer_reason_pair(
            &measured.composer_outcome,
            measured.refusal_class.as_deref(),
            measured.ledger.as_ref(),
        )?;
        if measured.composer_outcome == COMPOSER_ACCEPTED {
            composed += 1;
            if let Some(ledger) = measured.ledger.as_ref() {
                if ledger.required_targets > 0 {
                    documents_with_required_targets += 1;
                }
                target_occurrences_total += ledger.target_occurrences;
                required_targets_total += ledger.required_targets;
                resolved_targets_total += ledger.resolved_targets;
            }
        } else {
            refused += 1;
        }
        rows.push(SemanticDocumentRow {
            document_id: binding.id.clone(),
            profile_class: profile_class_for_path(&binding.path),
            reference_sha256: binding.sha256.clone(),
            admitted_band: AdmittedBandOutcome {
                outcome: measured.admitted_band_outcome.clone(),
                reason: measured.admitted_band_reason.clone(),
            },
            composer: ComposerMeasurement {
                outcome: measured.composer_outcome.clone(),
                refusal_class: measured.refusal_class.clone(),
                well_formedness_reason,
                bibliography_target_reason,
                ledger: measured.ledger,
            },
        });
    }

    let locks = REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect();

    let receipt = NativeRouteSemanticReceipt {
        schema_version: NATIVE_ROUTE_SEMANTIC_RECEIPT_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks,
        corpus: SemanticCorpus {
            id: expectation.corpus_id.clone(),
            document_count: expectation.corpus_document_count,
            page_denominator: expectation.corpus_page_denominator,
            corpus_pointer: ROUTE_CORPUS_POINTER.to_owned(),
            reference_pointer: ROUTE_REFERENCE_POINTER.to_owned(),
            documents_digest: expectation.corpus_documents_digest.clone(),
        },
        population: SemanticPopulation {
            documents: rows.len() as u64,
            admitted_band_available,
            composed,
            refused,
            documents_with_required_targets,
            target_occurrences_total,
            required_targets_total,
            resolved_targets_total,
            aggregate_verdict: NO_AGGREGATE_VERDICT.to_owned(),
        },
        documents: rows,
        native_route_emission_scope: NativeRouteEmissionScope {
            pointer: expectation.emission_path.clone(),
            sha256: expectation.emission_sha256.clone(),
            document_count: expectation.emission_document_count,
            documents: expectation.emission_scope.clone(),
        },
        redaction: Redaction::clean(),
    };
    validate_semantic_receipt(&receipt, expectation)?;
    Ok(receipt)
}

/// The profile class of a reference-TEI path: `preserved_rich` for the
/// preserved corridor fixtures, `holdout_structural` for the holdout fixtures.
pub fn profile_class_for_path(path: &str) -> String {
    if path.contains("/preserved/") {
        PROFILE_PRESERVED_RICH.to_owned()
    } else {
        PROFILE_HOLDOUT_STRUCTURAL.to_owned()
    }
}

// ───────────────────────────── validation ─────────────────────────────

/// Re-read a semantic receipt against the committed carriers.
///
/// This is the independent half of the contract: it refuses the receipt-level
/// mutations a builder cannot express, and it never re-derives the composer's
/// per-document facts (those are the helper's and T02's independent oracle's
/// job); it checks internal consistency and the carrier bindings.
pub fn validate_semantic_receipt(
    receipt: &NativeRouteSemanticReceipt,
    expectation: &SemanticReceiptExpectation,
) -> Result<(), SemanticReceiptRefusal> {
    if receipt.schema_version != NATIVE_ROUTE_SEMANTIC_RECEIPT_SCHEMA_VERSION {
        return Err(SemanticReceiptRefusal::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_ROUTE_SEMANTIC_RECEIPT_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(SemanticReceiptRefusal::ClaimScopeNotDiagnostic {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(SemanticReceiptRefusal::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(SemanticReceiptRefusal::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }
    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(SemanticReceiptRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(SemanticReceiptRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }
    if receipt.redaction != Redaction::clean() {
        return Err(SemanticReceiptRefusal::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    // ── the corpus identity ──────────────────────────────────────────────
    if !is_repo_relative_path(&receipt.corpus.corpus_pointer)
        || !is_repo_relative_path(&receipt.corpus.reference_pointer)
    {
        return Err(drift(
            "corpus.pointer",
            "<repository-relative>",
            format!(
                "{} / {}",
                receipt.corpus.corpus_pointer, receipt.corpus.reference_pointer
            ),
        ));
    }
    if receipt.corpus.id != expectation.corpus_id {
        return Err(drift(
            "corpus.id",
            &expectation.corpus_id,
            &receipt.corpus.id,
        ));
    }
    if receipt.corpus.document_count != expectation.corpus_document_count {
        return Err(drift(
            "corpus.document_count",
            expectation.corpus_document_count,
            receipt.corpus.document_count,
        ));
    }
    if receipt.corpus.page_denominator != expectation.corpus_page_denominator {
        return Err(drift(
            "corpus.page_denominator",
            expectation.corpus_page_denominator,
            receipt.corpus.page_denominator,
        ));
    }
    if receipt.corpus.documents_digest != expectation.corpus_documents_digest {
        return Err(SemanticReceiptRefusal::CorpusDigestDrift {
            expected: expectation.corpus_documents_digest.clone(),
            found: receipt.corpus.documents_digest.clone(),
        });
    }
    if !is_sha256_digest(&receipt.corpus.documents_digest) {
        return Err(drift(
            "corpus.documents_digest",
            "<sha256 digest>",
            &receipt.corpus.documents_digest,
        ));
    }

    // ── the emission binding ─────────────────────────────────────────────
    if receipt.native_route_emission_scope.sha256 != expectation.emission_sha256 {
        return Err(SemanticReceiptRefusal::EmissionDigestDrift {
            expected: expectation.emission_sha256.clone(),
            found: receipt.native_route_emission_scope.sha256.clone(),
        });
    }
    if !is_sha256_digest(&receipt.native_route_emission_scope.sha256) {
        return Err(drift(
            "native_route_emission_scope.sha256",
            "<sha256 digest>",
            &receipt.native_route_emission_scope.sha256,
        ));
    }
    if receipt.native_route_emission_scope.pointer != expectation.emission_path {
        return Err(drift(
            "native_route_emission_scope.pointer",
            &expectation.emission_path,
            &receipt.native_route_emission_scope.pointer,
        ));
    }
    if receipt.native_route_emission_scope.document_count != expectation.emission_document_count {
        return Err(drift(
            "native_route_emission_scope.document_count",
            expectation.emission_document_count,
            receipt.native_route_emission_scope.document_count,
        ));
    }

    // ── the document row set ─────────────────────────────────────────────
    let expected_ids: Vec<&str> = expectation
        .reference_documents
        .iter()
        .map(|binding| binding.id.as_str())
        .collect();
    if expected_ids.len() != receipt.documents.len() {
        return Err(SemanticReceiptRefusal::DocumentCountMismatch {
            expected: expected_ids.len(),
            found: receipt.documents.len(),
        });
    }
    for (index, expected_id) in expected_ids.iter().enumerate() {
        let row = &receipt.documents[index];
        if row.document_id != *expected_id {
            return Err(if expected_ids.contains(&row.document_id.as_str()) {
                SemanticReceiptRefusal::DocumentCountMismatch {
                    expected: expected_ids.len(),
                    found: receipt.documents.len(),
                }
            } else {
                SemanticReceiptRefusal::DocumentNotDeclared {
                    document_id: row.document_id.clone(),
                }
            });
        }
    }

    // ── per-document reason consistency ──────────────────────────────────
    let mut composed = 0u64;
    let mut refused = 0u64;
    let mut documents_with_required_targets = 0u64;
    let mut target_occurrences_total = 0u64;
    let mut required_targets_total = 0u64;
    let mut resolved_targets_total = 0u64;
    for row in &receipt.documents {
        check_document_row(row)?;
        match row.composer.outcome.as_str() {
            COMPOSER_ACCEPTED => {
                composed += 1;
                if let Some(ledger) = row.composer.ledger.as_ref() {
                    if ledger.required_targets > 0 {
                        documents_with_required_targets += 1;
                    }
                    target_occurrences_total += ledger.target_occurrences;
                    required_targets_total += ledger.required_targets;
                    resolved_targets_total += ledger.resolved_targets;
                }
            }
            COMPOSER_REFUSED => refused += 1,
            other => {
                return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.outcome".to_owned(),
                    found: other.to_owned(),
                });
            }
        }
    }

    // ── population counters bound to the rows ────────────────────────────
    let bound = |field: &str, expected: u64, found: u64| {
        if expected == found {
            Ok(())
        } else {
            Err(SemanticReceiptRefusal::UnboundDenominator {
                field: field.to_owned(),
                expected,
                found,
            })
        }
    };
    bound(
        "population.documents",
        receipt.documents.len() as u64,
        receipt.population.documents,
    )?;
    bound("population.composed", composed, receipt.population.composed)?;
    bound("population.refused", refused, receipt.population.refused)?;
    bound(
        "population.documents_with_required_targets",
        documents_with_required_targets,
        receipt.population.documents_with_required_targets,
    )?;
    bound(
        "population.target_occurrences_total",
        target_occurrences_total,
        receipt.population.target_occurrences_total,
    )?;
    bound(
        "population.required_targets_total",
        required_targets_total,
        receipt.population.required_targets_total,
    )?;
    bound(
        "population.resolved_targets_total",
        resolved_targets_total,
        receipt.population.resolved_targets_total,
    )?;
    let admitted_band_available = receipt
        .documents
        .iter()
        .filter(|row| row.admitted_band.outcome == AB_ACCEPTED)
        .count() as u64;
    bound(
        "population.admitted_band_available",
        admitted_band_available,
        receipt.population.admitted_band_available,
    )?;
    if receipt.population.aggregate_verdict != NO_AGGREGATE_VERDICT {
        return Err(SemanticReceiptRefusal::AggregateVerdictClaimed {
            found: receipt.population.aggregate_verdict.clone(),
        });
    }

    // ── the emission scope section ───────────────────────────────────────
    if receipt.native_route_emission_scope.documents.len() != expectation.emission_scope.len() {
        return Err(SemanticReceiptRefusal::DocumentCountMismatch {
            expected: expectation.emission_scope.len(),
            found: receipt.native_route_emission_scope.documents.len(),
        });
    }
    for (index, expected) in expectation.emission_scope.iter().enumerate() {
        let found = &receipt.native_route_emission_scope.documents[index];
        check_emission_scope_row(found)?;
        if found.document_id != expected.document_id {
            return Err(SemanticReceiptRefusal::EmissionScopeDrift {
                document_id: expected.document_id.clone(),
                field: "document_id".to_owned(),
                expected: expected.document_id.clone(),
                found: found.document_id.clone(),
            });
        }
        for (family, expected_family) in [
            ("header", &expected.header),
            ("references", &expected.references),
            ("body", &expected.body),
        ] {
            let found_family = match family {
                "header" => &found.header,
                "references" => &found.references,
                _ => &found.body,
            };
            if found_family != expected_family {
                return Err(SemanticReceiptRefusal::EmissionScopeDrift {
                    document_id: expected.document_id.clone(),
                    field: family.to_owned(),
                    expected: format!(
                        "{}/{:?}/{}",
                        expected_family.disposition,
                        expected_family.reason,
                        expected_family.scope_reason
                    ),
                    found: format!(
                        "{}/{:?}/{}",
                        found_family.disposition, found_family.reason, found_family.scope_reason
                    ),
                });
            }
        }
        if found.reference_links != expected.reference_links {
            return Err(SemanticReceiptRefusal::EmissionScopeDrift {
                document_id: expected.document_id.clone(),
                field: "reference_links".to_owned(),
                expected: format!(
                    "required={} resolved={} scope={}",
                    expected.reference_links.required_targets,
                    expected.reference_links.resolved_targets,
                    expected.reference_links.scope_reason
                ),
                found: format!(
                    "required={} resolved={} scope={}",
                    found.reference_links.required_targets,
                    found.reference_links.resolved_targets,
                    found.reference_links.scope_reason
                ),
            });
        }
    }

    Ok(())
}

fn check_document_row(row: &SemanticDocumentRow) -> Result<(), SemanticReceiptRefusal> {
    if row.composer.well_formedness_reason.trim().is_empty()
        || row.composer.bibliography_target_reason.trim().is_empty()
    {
        return Err(SemanticReceiptRefusal::PerDocumentReasonMissing {
            document_id: row.document_id.clone(),
        });
    }
    if !is_well_formedness_reason(&row.composer.well_formedness_reason) {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "composer.well_formedness_reason".to_owned(),
            found: row.composer.well_formedness_reason.clone(),
        });
    }
    if !is_bibliography_target_reason(&row.composer.bibliography_target_reason) {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "composer.bibliography_target_reason".to_owned(),
            found: row.composer.bibliography_target_reason.clone(),
        });
    }
    if row.admitted_band.reason != AB_ACCEPTED
        && !row
            .admitted_band
            .reason
            .strip_prefix(AB_UNAVAILABLE_PREFIX)
            .is_some_and(|reason| !reason.is_empty())
    {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "admitted_band.reason".to_owned(),
            found: row.admitted_band.reason.clone(),
        });
    }
    let band_refused = row.admitted_band.outcome == COMPOSER_REFUSED;
    let band_accepted = row.admitted_band.outcome == COMPOSER_ACCEPTED;
    if !band_refused && !band_accepted {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "admitted_band.outcome".to_owned(),
            found: row.admitted_band.outcome.clone(),
        });
    }
    if band_accepted != (row.admitted_band.reason == AB_ACCEPTED) {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "admitted_band.outcome/reason".to_owned(),
            found: format!("{}/{}", row.admitted_band.outcome, row.admitted_band.reason),
        });
    }

    match row.composer.outcome.as_str() {
        COMPOSER_ACCEPTED => {
            let ledger = row.composer.ledger.ok_or_else(|| {
                SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.ledger".to_owned(),
                    found: "<absent for an accepted composer outcome>".to_owned(),
                }
            })?;
            if row.composer.well_formedness_reason != WF_EMISSION_PARSED_WELL_FORMED {
                return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.well_formedness_reason".to_owned(),
                    found: row.composer.well_formedness_reason.clone(),
                });
            }
            if ledger.required_targets == 0 {
                if row.composer.bibliography_target_reason == BT_ALL_REQUIRED_TARGETS_RESOLVED {
                    return Err(SemanticReceiptRefusal::EmptyTargetsPresentedAsPass {
                        document_id: row.document_id.clone(),
                    });
                }
                if row.composer.bibliography_target_reason != BT_NO_REQUIRED_TARGETS_DECLARED {
                    return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                        document_id: row.document_id.clone(),
                        field: "composer.bibliography_target_reason".to_owned(),
                        found: row.composer.bibliography_target_reason.clone(),
                    });
                }
            } else if ledger.resolved_targets < ledger.required_targets {
                if row.composer.bibliography_target_reason == BT_ALL_REQUIRED_TARGETS_RESOLVED {
                    return Err(
                        SemanticReceiptRefusal::UnresolvedTargetsPresentedAsResolved {
                            document_id: row.document_id.clone(),
                        },
                    );
                }
                let expected = format!(
                    "{BT_UNRESOLVED_REQUIRED_TARGETS_PREFIX}{}",
                    ledger.required_targets - ledger.resolved_targets
                );
                if row.composer.bibliography_target_reason != expected {
                    return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                        document_id: row.document_id.clone(),
                        field: "composer.bibliography_target_reason".to_owned(),
                        found: row.composer.bibliography_target_reason.clone(),
                    });
                }
            } else if row.composer.bibliography_target_reason != BT_ALL_REQUIRED_TARGETS_RESOLVED {
                return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.bibliography_target_reason".to_owned(),
                    found: row.composer.bibliography_target_reason.clone(),
                });
            }
        }
        COMPOSER_REFUSED => {
            let class = row.composer.refusal_class.as_deref().unwrap_or("");
            if class.is_empty() {
                return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.refusal_class".to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            let expected = format!("{WF_COMPOSER_REFUSAL_PREFIX}{class}");
            if row.composer.well_formedness_reason != expected
                || row.composer.bibliography_target_reason != expected
            {
                return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                    document_id: row.document_id.clone(),
                    field: "composer.refusal_reason".to_owned(),
                    found: format!(
                        "{} / {}",
                        row.composer.well_formedness_reason,
                        row.composer.bibliography_target_reason
                    ),
                });
            }
        }
        _ => {
            return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                document_id: row.document_id.clone(),
                field: "composer.outcome".to_owned(),
                found: row.composer.outcome.clone(),
            });
        }
    }
    Ok(())
}

fn check_emission_scope_row(row: &EmissionScopeRow) -> Result<(), SemanticReceiptRefusal> {
    for family in ["header", "references", "body"] {
        let scope = match family {
            "header" => &row.header,
            "references" => &row.references,
            _ => &row.body,
        };
        let expected = family_scope_reason(family, &scope.disposition).ok_or_else(|| {
            SemanticReceiptRefusal::ReasonVocabularyViolation {
                document_id: row.document_id.clone(),
                field: format!("native_route_emission_scope.{family}.disposition"),
                found: scope.disposition.clone(),
            }
        })?;
        if scope.scope_reason != expected {
            if family == "header" && scope.disposition == EMISSION_TYPED_ABSENCE {
                return Err(SemanticReceiptRefusal::PlaceholderHeaderReadAsPopulated {
                    document_id: row.document_id.clone(),
                });
            }
            return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
                document_id: row.document_id.clone(),
                field: format!("native_route_emission_scope.{family}.scope_reason"),
                found: scope.scope_reason.clone(),
            });
        }
    }
    if !matches!(
        row.reference_links.scope_reason.as_str(),
        SCOPE_REFERENCE_LINKS_EMPTY_TARGETS | SCOPE_REFERENCE_LINKS_DECLARED_TARGETS
    ) {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "native_route_emission_scope.reference_links.scope_reason".to_owned(),
            found: row.reference_links.scope_reason.clone(),
        });
    }
    let expected = reference_links_scope_reason(
        row.reference_links.required_targets,
        row.reference_links.target_occurrences,
    );
    if row.reference_links.scope_reason != expected {
        return Err(SemanticReceiptRefusal::ReasonVocabularyViolation {
            document_id: row.document_id.clone(),
            field: "native_route_emission_scope.reference_links.scope_reason".to_owned(),
            found: row.reference_links.scope_reason.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_vocabulary_is_closed() {
        assert!(is_well_formedness_reason(WF_EMISSION_PARSED_WELL_FORMED));
        assert!(is_well_formedness_reason(
            "composer_typed_refusal:UnresolvedBibliographyTarget"
        ));
        assert!(!is_well_formedness_reason("pass"));
        assert!(is_bibliography_target_reason(
            BT_NO_REQUIRED_TARGETS_DECLARED
        ));
        assert!(is_bibliography_target_reason(
            "unresolved_required_targets:3"
        ));
        assert!(!is_bibliography_target_reason(
            "unresolved_required_targets:many"
        ));
        assert!(!is_bibliography_target_reason(""));
    }

    #[test]
    fn family_scope_reason_covers_the_closed_dispositions() {
        assert_eq!(
            family_scope_reason("header", "typed_absence"),
            Some(SCOPE_HEADER_TYPED_ABSENCE)
        );
        assert_eq!(
            family_scope_reason("references", "native_emitted"),
            Some(SCOPE_REFERENCES_NATIVE_EMITTED)
        );
        assert_eq!(family_scope_reason("figure", "native_emitted"), None);
    }

    #[test]
    fn reference_links_scope_reason_distinguishes_empty_from_declared() {
        assert_eq!(
            reference_links_scope_reason(0, 0),
            SCOPE_REFERENCE_LINKS_EMPTY_TARGETS
        );
        assert_eq!(
            reference_links_scope_reason(3, 5),
            SCOPE_REFERENCE_LINKS_DECLARED_TARGETS
        );
    }

    #[test]
    fn profile_class_is_path_derived() {
        assert_eq!(
            profile_class_for_path("crates/da-parse/tests/fixtures/preserved/x.tei.xml"),
            PROFILE_PRESERVED_RICH
        );
        assert_eq!(
            profile_class_for_path("crates/da-parse/tests/fixtures/holdout/x.tei.xml"),
            PROFILE_HOLDOUT_STRUCTURAL
        );
    }

    #[test]
    fn redaction_refuses_a_leaked_marker() {
        assert!(assert_semantic_receipt_redacted("{ \"note\": \"<TEI\" }").is_err());
        assert!(assert_semantic_receipt_redacted("{ \"note\": \"clean\" }").is_ok());
    }

    fn ledger(required: u64, resolved: u64, occurrences: u64) -> ReferenceLedgerView {
        ReferenceLedgerView {
            source_entries: required,
            admitted_references: required,
            target_occurrences: occurrences,
            required_targets: required,
            resolved_targets: resolved,
            id_space: required + 1,
        }
    }

    #[test]
    fn a_zero_target_ledger_names_the_empty_case() {
        let ledger = ledger(0, 0, 0);
        let (well_formedness, bibliography) =
            composer_reason_pair(COMPOSER_ACCEPTED, None, Some(&ledger)).expect("reason pair");
        assert_eq!(well_formedness, WF_EMISSION_PARSED_WELL_FORMED);
        assert_eq!(bibliography, BT_NO_REQUIRED_TARGETS_DECLARED);
    }

    #[test]
    fn a_resolved_ledger_names_resolution() {
        let ledger = ledger(5, 5, 7);
        let (_, bibliography) =
            composer_reason_pair(COMPOSER_ACCEPTED, None, Some(&ledger)).expect("reason pair");
        assert_eq!(bibliography, BT_ALL_REQUIRED_TARGETS_RESOLVED);
    }

    #[test]
    fn an_unresolved_ledger_names_the_unresolved_count() {
        let ledger = ledger(5, 3, 7);
        let (_, bibliography) =
            composer_reason_pair(COMPOSER_ACCEPTED, None, Some(&ledger)).expect("reason pair");
        assert_eq!(bibliography, "unresolved_required_targets:2");
    }

    #[test]
    fn a_refusal_names_both_reasons_with_the_class() {
        let (well_formedness, bibliography) =
            composer_reason_pair(COMPOSER_REFUSED, Some("UnresolvedBibliographyTarget"), None)
                .expect("reason pair");
        assert_eq!(
            well_formedness,
            "composer_typed_refusal:UnresolvedBibliographyTarget"
        );
        assert_eq!(bibliography, well_formedness);
    }

    #[test]
    fn a_refusal_without_a_class_is_refused() {
        assert!(composer_reason_pair(COMPOSER_REFUSED, None, None).is_err());
    }
}
