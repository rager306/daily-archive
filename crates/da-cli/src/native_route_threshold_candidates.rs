//! M516-th0oby / S03 / T03 — recompute the three frozen absorption-progress
//! thresholds on the expanded native PDF-to-TEI route corpus.
//!
//! The three frozen thresholds
//! (`data/parse/absorption_progress.schema.yaml#threshold_policy.thresholds`)
//! are currently only "measured" self-referentially: the acceptance package's
//! candidate values equal the pins because both derive from the same frozen
//! probe. The expanded route corpus (`data/parse/layout_bakeoff.yaml#native_route_corpus`,
//! 12 documents / 381 pages) is a genuinely independent measurement. This module
//! turns it into candidate values published *beside* the pins, without touching
//! the pins and without promoting a threshold.
//!
//! * [`derive_threshold_candidates_expectation`] recomputes the identity of the
//!   committed carriers — the frozen reference pins, the declared route corpus and
//!   the frozen reference population.
//! * [`build_threshold_candidates_receipt`] joins a measured candidate row to each
//!   frozen threshold by exact `(metric_id, stratum)`. A metric id carried by a
//!   single frozen threshold legitimately omits the stratum; a metric id shared by
//!   two thresholds must match exactly (MEM1989/MEM1991) — a blanket exact-stratum
//!   rule is wrong for `preservation.occurrence_conservation`.
//! * [`validate_threshold_candidates_receipt`] refuses, by name, a missing
//!   candidate, the reference pin re-labelled as the candidate, a threshold marked
//!   accepted, an opened lock, a comparability verdict claimed without a shared
//!   corpus identity and a zero or unbound candidate denominator.
//!
//! The artifact is diagnostic only: `claim_scope = diagnostic_only`,
//! `product_coverage_effect = none`, `thresholds = not_accepted`, all five locks
//! closed. It never re-pins the reference and never promotes a threshold (D127 /
//! ADR-076 §14 / R140).
//!
//! ## Artifact field name note
//!
//! The envelope carries the closed acceptance state under `thresholds` — the same
//! field every route carrier uses (`"not_accepted"`) — so the list of
//! per-threshold rows is carried under `threshold_candidates`. The plan text names
//! both fields `thresholds`; the two cannot share one JSON field, and the closed
//! acceptance state keeps its established name.
//!
//! Pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2` (no `da-parse`, no
//! `pdf-layout`), so it and its contract test compile in the default feature set.

use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest, is_sha256_hex,
    sha256_prefixed,
};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::fmt;

/// Schema version of the emitted threshold-candidate artifact.
pub const NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION: &str =
    "absorption-progress-threshold-candidates.v1";
/// Default artifact path of the threshold-candidate artifact.
pub const DEFAULT_THRESHOLD_CANDIDATES_ARTIFACT_REL: &str =
    "artifacts/parse_compat/absorption_progress_threshold_candidates.json";
/// The frozen reference pins the candidate is published beside (never rewrites).
pub const FROZEN_REFERENCE_ARTIFACT_REL: &str =
    "artifacts/parse_compat/absorption_progress_reference_pins.json";
/// The declared corpus block carrying the measured documents.
pub const ROUTE_CORPUS_POINTER: &str = "native_route_corpus.documents";
/// The sibling DATA block binding each route document to its reference TEI.
pub const ROUTE_THRESHOLD_REFERENCE_POINTER: &str = "native_route_threshold_reference.documents";
/// The unique preservation metric id: it is carried by one threshold only, so a
/// measured row may legitimately omit the stratum.
pub const PRESERVATION_METRIC_ID: &str = "preservation.occurrence_conservation";
/// The only comparability verdict a matched corpus identity may claim.
pub const COMPARABILITY_MATCHED: &str = "matched";
/// The honest verdict when the measured corpus is not the pin population.
pub const COMPARABILITY_NOT_COMPARABLE: &str = "not_comparable";
/// The numeric verdict when the candidate meets the floored reference.
pub const VERDICT_MET: &str = "met";
/// The numeric verdict when the candidate falls below the floored reference.
pub const VERDICT_UNMET: &str = "unmet";
/// A verdict of `accepted` is a refusal: no threshold may be accepted here.
pub const VERDICT_ACCEPTED: &str = "accepted";

// ─────────────────────────────── value types ───────────────────────────────

/// An exact integer fraction; `value` is derived, never restated as a literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fraction {
    /// Fraction numerator.
    pub numerator: u64,
    /// Fraction denominator; zero is refused.
    pub denominator: u64,
}

impl Fraction {
    /// The decimal value of the fraction.
    pub fn value(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

/// The retained frozen reference value for one threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceValue {
    /// The pinned integer fraction.
    pub fraction: Fraction,
    /// The decimal value of `fraction`.
    pub value: f64,
}

/// The candidate value measured on the expanded corpus for one threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateValue {
    /// The measured integer fraction.
    pub fraction: Fraction,
    /// The decimal value of `fraction`.
    pub value: f64,
    /// The non-empty measurement denominator this candidate is bound to.
    pub denominator: u64,
}

/// The frozen reference population the measured corpus is compared against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencePopulation {
    /// The frozen reference population identity.
    pub id: String,
    /// The frozen reference population document count.
    pub document_count: u64,
}

/// The comparability verdict and the reason it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparabilityVerdict {
    /// `matched` or `not_comparable`; never forced.
    pub verdict: String,
    /// The reason the verdict holds.
    pub rationale: String,
}

/// The measured corpus identity plus the reference population it is compared to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdCorpus {
    /// The measured corpus identity.
    pub id: String,
    /// The declared document count.
    pub document_count: u64,
    /// The declared page denominator.
    pub page_denominator: u64,
    /// The frozen reference population used for the comparability verdict.
    pub population: ReferencePopulation,
    /// The comparability verdict against the frozen reference population.
    pub comparability: ComparabilityVerdict,
}

/// The binding to the frozen reference pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenReferenceBinding {
    /// Repository-relative pins path.
    pub path: String,
    /// `sha256:` digest of the committed pins bytes.
    pub sha256: String,
}

/// One frozen threshold with its retained pin and its measured candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdCandidateRow {
    /// Stable threshold id.
    pub threshold_id: String,
    /// The metric the threshold reads.
    pub metric_id: String,
    /// The declared stratum.
    pub stratum: String,
    /// The comparison direction.
    pub direction: String,
    /// The declared margin.
    pub margin: f64,
    /// The retained frozen reference value.
    pub reference: ReferenceValue,
    /// The measured candidate value.
    pub candidate: CandidateValue,
    /// The numeric verdict (`met`/`unmet`) or `not_comparable`.
    pub verdict: String,
    /// The comparability verdict this row inherits.
    pub comparability: String,
}

/// The emitted threshold-candidate artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdCandidatesReceipt {
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
    /// The frozen reference pins this candidate is published beside.
    pub frozen_reference: FrozenReferenceBinding,
    /// The measured corpus and the frozen reference population.
    pub corpus: ThresholdCorpus,
    /// The per-threshold pin-vs-candidate rows.
    pub threshold_candidates: Vec<ThresholdCandidateRow>,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl ThresholdCandidatesReceipt {
    /// Serialize canonically (pretty JSON, one trailing newline) and refuse if any
    /// payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, ThresholdCandidatesRefusal> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            ThresholdCandidatesRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            ThresholdCandidatesRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        assert_threshold_candidates_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, ThresholdCandidatesRefusal> {
        serde_json::from_slice(bytes).map_err(|error| {
            ThresholdCandidatesRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_threshold_candidates_redacted(json: &str) -> Result<(), ThresholdCandidatesRefusal> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(ThresholdCandidatesRefusal::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ─────────────────────────── measured + expected input ───────────────────────

/// One metric measured on the expanded corpus, before it is joined to a frozen
/// threshold. `stratum` is `None` for a metric whose id is carried by one
/// threshold only (the measured value omits the stratum legitimately).
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredMetric {
    /// The measured metric id.
    pub metric_id: String,
    /// The measured stratum, when the metric id is shared by several thresholds.
    pub stratum: Option<String>,
    /// The measured candidate value.
    pub candidate: CandidateValue,
}

/// One route-corpus document bound to its reference TEI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceDocumentBinding {
    /// The route-corpus document id.
    pub id: String,
    /// Repository-relative reference TEI path.
    pub path: String,
    /// Bare 64-hex sha256 of the reference TEI bytes.
    pub sha256: String,
}

/// One frozen threshold's recomputed identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceThresholdExpectation {
    /// Stable threshold id.
    pub threshold_id: String,
    /// The metric the threshold reads.
    pub metric_id: String,
    /// The declared stratum.
    pub stratum: String,
    /// The comparison direction.
    pub direction: String,
    /// The declared margin.
    pub margin: f64,
    /// The pinned reference fraction.
    pub fraction: Fraction,
}

/// The recomputed expectation a receipt is validated against.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdCandidatesExpectation {
    /// The frozen reference pins path.
    pub frozen_reference_path: String,
    /// `sha256:` digest of the committed pins bytes.
    pub frozen_reference_sha256: String,
    /// The measured corpus id.
    pub corpus_id: String,
    /// The declared document count.
    pub corpus_document_count: u64,
    /// The declared page denominator.
    pub corpus_page_denominator: u64,
    /// The frozen reference population.
    pub reference_population: ReferencePopulation,
    /// The comparability verdict derived from the two identities.
    pub comparability_verdict: String,
    /// The frozen thresholds with their pinned fractions.
    pub reference_thresholds: Vec<ReferenceThresholdExpectation>,
}

// ───────────────────────────── refusals ─────────────────────────────

/// Typed refusals of the threshold-candidate receipt. Every variant is a
/// refusal, never a silent downgrade.
#[derive(Debug, Clone, PartialEq)]
pub enum ThresholdCandidatesRefusal {
    /// A carrier could not be read or parsed.
    MalformedCarrier { detail: String },
    /// A schema field drifted from the contract's constant or the carrier fact.
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
    /// A per-row verdict claimed an accepted threshold.
    ThresholdAccepted { threshold_id: String },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
    /// A frozen threshold has no candidate row.
    CandidateAbsent { threshold_id: String },
    /// The candidate fraction equals the reference fraction: the pin was
    /// re-labelled as the candidate, not independently measured.
    PinRelabelledAsCandidate { threshold_id: String },
    /// The candidate denominator is zero or unbound.
    UnboundCandidateDenominator { threshold_id: String },
    /// A comparability verdict claimed a shared corpus identity it does not have.
    ComparabilityWithoutSharedIdentity { corpus_id: String, verdict: String },
    /// A frozen threshold the receipt does not declare, or a row it declares that
    /// the frozen contract does not carry.
    ThresholdSetMismatch { threshold_id: String },
}

impl fmt::Display for ThresholdCandidatesRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedCarrier { detail } => {
                write!(f, "threshold candidates: malformed carrier: {detail}")
            }
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "threshold candidates: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::BindingDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "threshold candidates: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::ClaimScopeNotDiagnostic { found } => write!(
                f,
                "threshold candidates: claim scope `{found}` is not `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "threshold candidates: product coverage `{found}` is not \
                 `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::ThresholdClaimed { found } => write!(
                f,
                "threshold candidates: envelope thresholds `{found}` are not \
                 `{THRESHOLDS_NOT_ACCEPTED}`"
            ),
            Self::ThresholdAccepted { threshold_id } => write!(
                f,
                "threshold candidates: `{threshold_id}` is marked accepted, which this cut forbids"
            ),
            Self::LockOpened {
                lock,
                expected,
                found,
            } => write!(
                f,
                "threshold candidates: lock `{lock}` is open: expected `{expected}`, found `{found}`"
            ),
            Self::RedactionLeak { marker } => {
                write!(f, "threshold candidates: artifact leaked marker `{marker}`")
            }
            Self::CandidateAbsent { threshold_id } => write!(
                f,
                "threshold candidates: `{threshold_id}` has no candidate row"
            ),
            Self::PinRelabelledAsCandidate { threshold_id } => write!(
                f,
                "threshold candidates: `{threshold_id}` candidate fraction equals the retained \
                 pin; the reference was re-labelled as the candidate"
            ),
            Self::UnboundCandidateDenominator { threshold_id } => write!(
                f,
                "threshold candidates: `{threshold_id}` candidate denominator is zero or unbound"
            ),
            Self::ComparabilityWithoutSharedIdentity { corpus_id, verdict } => write!(
                f,
                "threshold candidates: comparability `{verdict}` was claimed for corpus \
                 `{corpus_id}` without a shared corpus identity"
            ),
            Self::ThresholdSetMismatch { threshold_id } => write!(
                f,
                "threshold candidates: `{threshold_id}` is not in the frozen threshold set"
            ),
        }
    }
}

impl std::error::Error for ThresholdCandidatesRefusal {}

fn malformed(detail: impl Into<String>) -> ThresholdCandidatesRefusal {
    ThresholdCandidatesRefusal::MalformedCarrier {
        detail: detail.into(),
    }
}

// ───────────────────────────── carrier parsing ─────────────────────────────

fn yaml_str<'a>(value: &'a Yaml, pointer: &str) -> Result<&'a str, ThresholdCandidatesRefusal> {
    value
        .as_str()
        .ok_or_else(|| malformed(format!("`{pointer}` is not a string")))
}

fn yaml_u64(value: &Yaml, pointer: &str) -> Result<u64, ThresholdCandidatesRefusal> {
    value
        .as_u64()
        .ok_or_else(|| malformed(format!("`{pointer}` is not an unsigned integer")))
}

fn yaml_f64(value: &Yaml, pointer: &str) -> Result<f64, ThresholdCandidatesRefusal> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|integer| integer as f64))
        .ok_or_else(|| malformed(format!("`{pointer}` is not a number")))
}

fn yaml_get<'a>(
    value: &'a Yaml,
    key: &str,
    pointer: &str,
) -> Result<&'a Yaml, ThresholdCandidatesRefusal> {
    value
        .get(key)
        .ok_or_else(|| malformed(format!("`{pointer}.{key}` is absent")))
}

fn yaml_sequence<'a>(
    value: &'a Yaml,
    key: &str,
    pointer: &str,
) -> Result<&'a Vec<Yaml>, ThresholdCandidatesRefusal> {
    yaml_get(value, key, pointer)?
        .as_sequence()
        .ok_or_else(|| malformed(format!("`{pointer}.{key}` is not a sequence")))
}

/// Read the reference-TEI bindings declared by the sibling DATA block.
pub fn reference_document_bindings(
    layout_yaml: &str,
) -> Result<Vec<ReferenceDocumentBinding>, ThresholdCandidatesRefusal> {
    let layout: Yaml = serde_yaml::from_str(layout_yaml)
        .map_err(|error| malformed(format!("layout_bakeoff.yaml is not valid YAML: {error}")))?;
    let block = yaml_get(&layout, "native_route_threshold_reference", "layout")?;
    let documents = yaml_sequence(
        block,
        "documents",
        "layout.native_route_threshold_reference",
    )?;
    let mut bindings = Vec::with_capacity(documents.len());
    for (index, row) in documents.iter().enumerate() {
        let pointer = format!("layout.native_route_threshold_reference.documents[{index}]");
        let id = yaml_str(yaml_get(row, "id", &pointer)?, &format!("{pointer}.id"))?.to_owned();
        let path =
            yaml_str(yaml_get(row, "path", &pointer)?, &format!("{pointer}.path"))?.to_owned();
        let sha256 = yaml_str(
            yaml_get(row, "sha256", &pointer)?,
            &format!("{pointer}.sha256"),
        )?
        .to_owned();
        if !is_repo_relative_path(&path) {
            return Err(malformed(format!(
                "`{pointer}.path` `{path}` is not repository-relative"
            )));
        }
        // The corpus binds bare 64-hex digests; accept either that or the
        // `sha256:`-prefixed form so the sibling block stays in the corpus's own
        // identity shape.
        if !is_sha256_hex(&sha256) && !is_sha256_digest(&sha256) {
            return Err(malformed(format!(
                "`{pointer}.sha256` is not a sha256 digest"
            )));
        }
        bindings.push(ReferenceDocumentBinding { id, path, sha256 });
    }
    Ok(bindings)
}

/// Read the declared route-corpus document ids.
pub fn corpus_document_ids(layout_yaml: &str) -> Result<Vec<String>, ThresholdCandidatesRefusal> {
    let layout: Yaml = serde_yaml::from_str(layout_yaml)
        .map_err(|error| malformed(format!("layout_bakeoff.yaml is not valid YAML: {error}")))?;
    let block = yaml_get(&layout, "native_route_corpus", "layout")?;
    let documents = yaml_sequence(block, "documents", "layout.native_route_corpus")?;
    let mut ids = Vec::with_capacity(documents.len());
    for (index, row) in documents.iter().enumerate() {
        let pointer = format!("layout.native_route_corpus.documents[{index}].id");
        ids.push(yaml_str(yaml_get(row, "id", &pointer)?, &pointer)?.to_owned());
    }
    Ok(ids)
}

/// Refuse a reference binding set that does not cover the route corpus exactly.
pub fn validate_reference_bindings(layout_yaml: &str) -> Result<(), ThresholdCandidatesRefusal> {
    let corpus = corpus_document_ids(layout_yaml)?;
    let bindings = reference_document_bindings(layout_yaml)?;
    if corpus.is_empty() {
        return Err(malformed("the route corpus declares no documents"));
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for binding in &bindings {
        *seen.entry(binding.id.as_str()).or_insert(0) += 1;
    }
    for id in &corpus {
        match seen.get(id.as_str()) {
            Some(1) => {}
            Some(_) => {
                return Err(malformed(format!(
                    "reference TEI for `{id}` is bound more than once"
                )));
            }
            None => {
                return Err(malformed(format!(
                    "route document `{id}` has no reference TEI binding"
                )));
            }
        }
    }
    for binding in &bindings {
        if !corpus.iter().any(|id| id == &binding.id) {
            return Err(malformed(format!(
                "reference TEI binds `{}` which is not a route document",
                binding.id
            )));
        }
    }
    Ok(())
}

// ───────────────────────────── derivation ─────────────────────────────

/// Recompute the expectation a receipt is validated against from the committed
/// carriers: the frozen pins JSON, the layout declaration, the closed schema and
/// the frozen holdout manifest.
pub fn derive_threshold_candidates_expectation(
    pins_json: &[u8],
    layout_yaml: &str,
    schema_yaml: &str,
    holdout_yaml: &str,
) -> Result<ThresholdCandidatesExpectation, ThresholdCandidatesRefusal> {
    let pins: serde_json::Value = serde_json::from_slice(pins_json)
        .map_err(|error| malformed(format!("reference pins are not valid JSON: {error}")))?;
    let layout: Yaml = serde_yaml::from_str(layout_yaml)
        .map_err(|error| malformed(format!("layout_bakeoff.yaml is not valid YAML: {error}")))?;
    let schema: Yaml = serde_yaml::from_str(schema_yaml).map_err(|error| {
        malformed(format!(
            "absorption_progress.schema.yaml is not valid YAML: {error}"
        ))
    })?;
    let holdout: Yaml = serde_yaml::from_str(holdout_yaml)
        .map_err(|error| malformed(format!("holdout_manifests.yaml is not valid YAML: {error}")))?;

    validate_reference_bindings(layout_yaml)?;

    let corpus = yaml_get(&layout, "native_route_corpus", "layout")?;
    let corpus_id = yaml_str(
        yaml_get(corpus, "id", "layout.native_route_corpus")?,
        "layout.native_route_corpus.id",
    )?
    .to_owned();
    let corpus_document_count = yaml_u64(
        yaml_get(corpus, "corpus_n", "layout.native_route_corpus")?,
        "layout.native_route_corpus.corpus_n",
    )?;
    let corpus_page_denominator = yaml_u64(
        yaml_get(corpus, "page_denominator", "layout.native_route_corpus")?,
        "layout.native_route_corpus.page_denominator",
    )?;

    let holdout_id = yaml_str(
        yaml_get(&holdout, "schema_version", "holdout")?,
        "holdout.schema_version",
    )?
    .to_owned();
    let holdout_articles = yaml_sequence(&holdout, "articles", "holdout")?;
    let reference_population = ReferencePopulation {
        id: holdout_id,
        document_count: holdout_articles.len() as u64,
    };

    let comparability_verdict = if corpus_id == reference_population.id
        && corpus_document_count == reference_population.document_count
    {
        COMPARABILITY_MATCHED
    } else {
        COMPARABILITY_NOT_COMPARABLE
    }
    .to_owned();

    // Frozen pins: threshold_id -> (metric_id, stratum, fraction).
    let pin_values = pins
        .get("values")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| malformed("reference pins carry no `values` array"))?;
    let mut pin_by_threshold: BTreeMap<&str, (&str, &str, Fraction)> = BTreeMap::new();
    for entry in pin_values {
        let threshold_id = entry
            .get("threshold_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| malformed("a pin is missing `threshold_id`"))?;
        let metric_id = entry
            .get("metric_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| malformed("a pin is missing `metric_id`"))?;
        let stratum = entry
            .get("stratum")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| malformed("a pin is missing `stratum`"))?;
        let fraction = entry
            .get("fraction")
            .ok_or_else(|| malformed("a pin is missing `fraction`"))?;
        let numerator = fraction
            .get("numerator")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| malformed("a pin fraction is missing `numerator`"))?;
        let denominator = fraction
            .get("denominator")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| malformed("a pin fraction is missing `denominator`"))?;
        if denominator == 0 {
            return Err(malformed(format!(
                "pin `{threshold_id}` has a zero denominator"
            )));
        }
        pin_by_threshold.insert(
            threshold_id,
            (
                metric_id,
                stratum,
                Fraction {
                    numerator,
                    denominator,
                },
            ),
        );
    }

    // Closed schema: the frozen threshold list, in order.
    let threshold_policy = yaml_get(&schema, "threshold_policy", "schema")?;
    let schema_thresholds =
        yaml_sequence(threshold_policy, "thresholds", "schema.threshold_policy")?;
    let mut reference_thresholds = Vec::with_capacity(schema_thresholds.len());
    for (index, entry) in schema_thresholds.iter().enumerate() {
        let pointer = format!("schema.threshold_policy.thresholds[{index}]");
        let threshold_id = yaml_str(
            yaml_get(entry, "threshold_id", &pointer)?,
            &format!("{pointer}.threshold_id"),
        )?
        .to_owned();
        let metric_id = yaml_str(
            yaml_get(entry, "metric_id", &pointer)?,
            &format!("{pointer}.metric_id"),
        )?
        .to_owned();
        let stratum = yaml_str(
            yaml_get(entry, "stratum", &pointer)?,
            &format!("{pointer}.stratum"),
        )?
        .to_owned();
        let direction = yaml_str(
            yaml_get(entry, "direction", &pointer)?,
            &format!("{pointer}.direction"),
        )?
        .to_owned();
        let margin = yaml_f64(
            yaml_get(entry, "margin", &pointer)?,
            &format!("{pointer}.margin"),
        )?;
        let (pin_metric, pin_stratum, fraction) = pin_by_threshold
            .get(threshold_id.as_str())
            .copied()
            .ok_or_else(|| malformed(format!("the pins do not carry `{threshold_id}`")))?;
        if pin_metric != metric_id || pin_stratum != stratum {
            return Err(ThresholdCandidatesRefusal::SchemaDrift {
                field: format!("{threshold_id}.metric_id/stratum"),
                expected: format!("{metric_id}/{stratum}"),
                found: format!("{pin_metric}/{pin_stratum}"),
            });
        }
        reference_thresholds.push(ReferenceThresholdExpectation {
            threshold_id,
            metric_id,
            stratum,
            direction,
            margin,
            fraction,
        });
    }

    Ok(ThresholdCandidatesExpectation {
        frozen_reference_path: FROZEN_REFERENCE_ARTIFACT_REL.to_owned(),
        frozen_reference_sha256: sha256_prefixed(pins_json),
        corpus_id,
        corpus_document_count,
        corpus_page_denominator,
        reference_population,
        comparability_verdict,
        reference_thresholds,
    })
}

// ───────────────────────────── candidate join ─────────────────────────────

/// Resolve the candidate measured for one frozen threshold.
///
/// A metric id carried by a single frozen threshold legitimately omits the
/// stratum (`preservation.occurrence_conservation`); a metric id shared by two
/// thresholds (`extraction.native_capability_recall`) must match its exact
/// `(metric_id, stratum)`. A blanket exact-stratum rule is wrong here.
fn resolve_candidate<'a>(
    expectation: &ThresholdCandidatesExpectation,
    threshold: &ReferenceThresholdExpectation,
    measured: &'a [MeasuredMetric],
) -> Option<&'a CandidateValue> {
    let sharing = expectation
        .reference_thresholds
        .iter()
        .filter(|candidate| candidate.metric_id == threshold.metric_id)
        .count();
    if sharing == 1 {
        measured
            .iter()
            .find(|row| row.metric_id == threshold.metric_id)
            .map(|row| &row.candidate)
    } else {
        measured
            .iter()
            .find(|row| {
                row.metric_id == threshold.metric_id
                    && row.stratum.as_deref() == Some(threshold.stratum.as_str())
            })
            .map(|row| &row.candidate)
    }
}

fn meets(direction: &str, candidate: f64, reference: f64, margin: f64) -> bool {
    match direction {
        "at_least" => candidate + margin >= reference,
        "at_most" => candidate <= reference + margin,
        _ => false,
    }
}

/// Build the receipt from the recomputed expectation and the measured metrics.
pub fn build_threshold_candidates_receipt(
    expectation: &ThresholdCandidatesExpectation,
    measured: &[MeasuredMetric],
    comparability_rationale: &str,
) -> Result<ThresholdCandidatesReceipt, ThresholdCandidatesRefusal> {
    let mut rows = Vec::with_capacity(expectation.reference_thresholds.len());
    for threshold in &expectation.reference_thresholds {
        let candidate = resolve_candidate(expectation, threshold, measured).ok_or_else(|| {
            ThresholdCandidatesRefusal::CandidateAbsent {
                threshold_id: threshold.threshold_id.clone(),
            }
        })?;
        let verdict = if expectation.comparability_verdict == COMPARABILITY_MATCHED {
            if meets(
                threshold.direction.as_str(),
                candidate.value,
                threshold.fraction.value(),
                threshold.margin,
            ) {
                VERDICT_MET
            } else {
                VERDICT_UNMET
            }
        } else {
            COMPARABILITY_NOT_COMPARABLE
        };
        rows.push(ThresholdCandidateRow {
            threshold_id: threshold.threshold_id.clone(),
            metric_id: threshold.metric_id.clone(),
            stratum: threshold.stratum.clone(),
            direction: threshold.direction.clone(),
            margin: threshold.margin,
            reference: ReferenceValue {
                fraction: threshold.fraction,
                value: threshold.fraction.value(),
            },
            candidate: candidate.clone(),
            verdict: verdict.to_owned(),
            comparability: expectation.comparability_verdict.clone(),
        });
    }

    let locks = REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect();

    let receipt = ThresholdCandidatesReceipt {
        schema_version: NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks,
        frozen_reference: FrozenReferenceBinding {
            path: expectation.frozen_reference_path.clone(),
            sha256: expectation.frozen_reference_sha256.clone(),
        },
        corpus: ThresholdCorpus {
            id: expectation.corpus_id.clone(),
            document_count: expectation.corpus_document_count,
            page_denominator: expectation.corpus_page_denominator,
            population: expectation.reference_population.clone(),
            comparability: ComparabilityVerdict {
                verdict: expectation.comparability_verdict.clone(),
                rationale: comparability_rationale.to_owned(),
            },
        },
        threshold_candidates: rows,
        redaction: Redaction::clean(),
    };
    validate_threshold_candidates_receipt(&receipt, expectation)?;
    Ok(receipt)
}

// ───────────────────────────── validation ─────────────────────────────

fn drift(field: &str, expected: impl ToString, found: impl ToString) -> ThresholdCandidatesRefusal {
    ThresholdCandidatesRefusal::BindingDrift {
        field: field.to_owned(),
        expected: expected.to_string(),
        found: found.to_string(),
    }
}

/// Re-read a threshold-candidate receipt against the committed carriers.
///
/// This is the independent half of the contract: it refuses the receipt-level
/// mutations a builder cannot express.
pub fn validate_threshold_candidates_receipt(
    receipt: &ThresholdCandidatesReceipt,
    expectation: &ThresholdCandidatesExpectation,
) -> Result<(), ThresholdCandidatesRefusal> {
    if receipt.schema_version != NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION {
        return Err(ThresholdCandidatesRefusal::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(ThresholdCandidatesRefusal::ClaimScopeNotDiagnostic {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(ThresholdCandidatesRefusal::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(ThresholdCandidatesRefusal::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }
    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(ThresholdCandidatesRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(ThresholdCandidatesRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }
    if receipt.redaction != Redaction::clean() {
        return Err(ThresholdCandidatesRefusal::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    // ── the frozen reference binding ──────────────────────────────────────
    if receipt.frozen_reference.path != expectation.frozen_reference_path {
        return Err(drift(
            "frozen_reference.path",
            &expectation.frozen_reference_path,
            &receipt.frozen_reference.path,
        ));
    }
    if receipt.frozen_reference.sha256 != expectation.frozen_reference_sha256 {
        return Err(drift(
            "frozen_reference.sha256",
            &expectation.frozen_reference_sha256,
            &receipt.frozen_reference.sha256,
        ));
    }
    if !is_sha256_digest(&receipt.frozen_reference.sha256) {
        return Err(drift(
            "frozen_reference.sha256",
            "<sha256 digest>",
            &receipt.frozen_reference.sha256,
        ));
    }

    // ── the corpus identity ───────────────────────────────────────────────
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
    if receipt.corpus.population != expectation.reference_population {
        return Err(drift(
            "corpus.population",
            format!(
                "{} ({})",
                expectation.reference_population.id,
                expectation.reference_population.document_count
            ),
            format!(
                "{} ({})",
                receipt.corpus.population.id, receipt.corpus.population.document_count
            ),
        ));
    }

    // ── the comparability verdict, never forced ──────────────────────────
    let comparability = &receipt.corpus.comparability;
    if comparability.rationale.trim().is_empty() {
        return Err(drift(
            "corpus.comparability.rationale",
            "<non-empty>",
            comparability.rationale.trim(),
        ));
    }
    if comparability.verdict != COMPARABILITY_MATCHED
        && comparability.verdict != COMPARABILITY_NOT_COMPARABLE
    {
        return Err(
            ThresholdCandidatesRefusal::ComparabilityWithoutSharedIdentity {
                corpus_id: receipt.corpus.id.clone(),
                verdict: comparability.verdict.clone(),
            },
        );
    }
    let shared_identity = receipt.corpus.id == receipt.corpus.population.id
        && receipt.corpus.document_count == receipt.corpus.population.document_count;
    if comparability.verdict == COMPARABILITY_MATCHED && !shared_identity {
        return Err(
            ThresholdCandidatesRefusal::ComparabilityWithoutSharedIdentity {
                corpus_id: receipt.corpus.id.clone(),
                verdict: comparability.verdict.clone(),
            },
        );
    }
    if comparability.verdict != expectation.comparability_verdict {
        return Err(drift(
            "corpus.comparability.verdict",
            &expectation.comparability_verdict,
            &comparability.verdict,
        ));
    }

    // ── the per-threshold rows ───────────────────────────────────────────
    let mut expected_ids: Vec<&str> = expectation
        .reference_thresholds
        .iter()
        .map(|threshold| threshold.threshold_id.as_str())
        .collect();
    expected_ids.sort_unstable();
    let mut observed_ids: Vec<&str> = receipt
        .threshold_candidates
        .iter()
        .map(|row| row.threshold_id.as_str())
        .collect();
    observed_ids.sort_unstable();
    if expected_ids != observed_ids {
        let missing = expected_ids
            .iter()
            .find(|id| !observed_ids.contains(id))
            .copied()
            .unwrap_or("<none>");
        return Err(ThresholdCandidatesRefusal::CandidateAbsent {
            threshold_id: missing.to_owned(),
        });
    }

    for threshold in &expectation.reference_thresholds {
        let row = receipt
            .threshold_candidates
            .iter()
            .find(|row| row.threshold_id == threshold.threshold_id)
            .ok_or_else(|| ThresholdCandidatesRefusal::CandidateAbsent {
                threshold_id: threshold.threshold_id.clone(),
            })?;
        if row.metric_id != threshold.metric_id || row.stratum != threshold.stratum {
            return Err(drift(
                &format!("{}.metric_id/stratum", threshold.threshold_id),
                format!("{}/{}", threshold.metric_id, threshold.stratum),
                format!("{}/{}", row.metric_id, row.stratum),
            ));
        }
        if row.direction != threshold.direction {
            return Err(drift(
                &format!("{}.direction", threshold.threshold_id),
                &threshold.direction,
                &row.direction,
            ));
        }
        if (row.margin - threshold.margin).abs() > f64::EPSILON {
            return Err(drift(
                &format!("{}.margin", threshold.threshold_id),
                threshold.margin,
                row.margin,
            ));
        }
        if row.reference.fraction != threshold.fraction {
            return Err(drift(
                &format!("{}.reference.fraction", threshold.threshold_id),
                format!(
                    "{}/{}",
                    threshold.fraction.numerator, threshold.fraction.denominator
                ),
                format!(
                    "{}/{}",
                    row.reference.fraction.numerator, row.reference.fraction.denominator
                ),
            ));
        }
        if row.reference.value.to_bits() != threshold.fraction.value().to_bits() {
            return Err(drift(
                &format!("{}.reference.value", threshold.threshold_id),
                threshold.fraction.value(),
                row.reference.value,
            ));
        }
        if row.candidate.denominator == 0 || row.candidate.fraction.denominator == 0 {
            return Err(ThresholdCandidatesRefusal::UnboundCandidateDenominator {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if row.candidate.denominator != row.candidate.fraction.denominator {
            return Err(ThresholdCandidatesRefusal::UnboundCandidateDenominator {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if row.candidate.value.to_bits() != row.candidate.fraction.value().to_bits() {
            return Err(drift(
                &format!("{}.candidate.value", threshold.threshold_id),
                row.candidate.fraction.value(),
                row.candidate.value,
            ));
        }
        if row.candidate.fraction == threshold.fraction {
            return Err(ThresholdCandidatesRefusal::PinRelabelledAsCandidate {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if row.verdict == VERDICT_ACCEPTED {
            return Err(ThresholdCandidatesRefusal::ThresholdAccepted {
                threshold_id: threshold.threshold_id.clone(),
            });
        }
        if row.comparability != expectation.comparability_verdict {
            return Err(drift(
                &format!("{}.comparability", threshold.threshold_id),
                &expectation.comparability_verdict,
                &row.comparability,
            ));
        }
        let expected_verdict = if expectation.comparability_verdict == COMPARABILITY_MATCHED {
            if meets(
                threshold.direction.as_str(),
                row.candidate.value,
                threshold.fraction.value(),
                threshold.margin,
            ) {
                VERDICT_MET
            } else {
                VERDICT_UNMET
            }
        } else {
            COMPARABILITY_NOT_COMPARABLE
        };
        if row.verdict != expected_verdict {
            return Err(drift(
                &format!("{}.verdict", threshold.threshold_id),
                expected_verdict,
                &row.verdict,
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_value_is_derived_not_restated() {
        let fraction = Fraction {
            numerator: 1,
            denominator: 4,
        };
        assert_eq!(fraction.value().to_bits(), 0.25f64.to_bits());
    }

    #[test]
    fn reference_binding_validation_covers_the_corpus_exactly() {
        let layout = include_str!("../../../data/parse/layout_bakeoff.yaml");
        validate_reference_bindings(layout).expect("the committed bindings cover the corpus");
        let ids = corpus_document_ids(layout).expect("ids parse");
        let bindings = reference_document_bindings(layout).expect("bindings parse");
        assert_eq!(ids.len(), 12);
        assert_eq!(bindings.len(), 12);
    }
}
