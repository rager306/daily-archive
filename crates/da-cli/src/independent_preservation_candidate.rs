//! M516-th0oby / S11 / T01 — the independent preservation candidate validator.
//!
//! The acceptance package (`native_acceptance_package`) builds **one** `Series`
//! from the frozen schema + reference probe and assigns it to *both*
//! `candidate_series` and `reference_series`, so `derive_series_identity`
//! returns `same_series` by construction and the three frozen thresholds can only
//! ever resolve as a *reproduction* of the frozen probe (M515/S06 residual).
//!
//! S11 produces a genuinely independent preservation candidate — a new sibling
//! artifact published **beside** the untouched frozen pins — whose series
//! fingerprint differs from the frozen reference. This module is the single
//! source of truth for that artifact: its schema, its derived identity, its
//! threshold resolutions and its named, fail-closed refusals. The producer
//! (`examples/independent_preservation_candidate.rs`), the hermetic contract test
//! and the governor half all read this module.
//!
//! * [`derive_independent_preservation_expectation`] reads the committed carriers
//!   and builds the two series as [`da_parse::Series`]: the independent candidate
//!   series (the route corpus's own four keys) and the frozen reference series the
//!   acceptance package builds. It derives the series identity and **refuses by
//!   name** if any key is empty or if the two fingerprints are equal — an
//!   "independent" candidate that is the same series is exactly the self-reference
//!   the honesty rule forbids.
//! * [`build_independent_preservation_receipt`] maps the S03 threshold-candidate
//!   rows into `thresholds_resolved`; because the candidate series is independent
//!   every row resolves `not_comparable` with the typed `mixed_scope_comparison`
//!   reason, and both the retained pin and the candidate fraction/denominator are
//!   shown. The earlier conservation result is retained as its own disclosed
//!   record, bound by path + pointer + sha256.
//! * [`validate_independent_preservation_receipt`] is the independent validator
//!   with named, fail-closed refusals: envelope drift, a candidate fingerprint that
//!   equals the reference, a series identity that is not the derived one, a
//!   resolution outside the closed vocabulary, a zero candidate denominator, a
//!   restated fraction value, and a retained-conservation record that no longer
//!   reads `same_series` / `disclosed_unresolved`.
//!
//! The artifact is diagnostic only: `claim_scope = diagnostic_only`,
//! `product_coverage_effect = none`, `thresholds = not_accepted`, all five locks
//! closed. Every threshold stays `not_accepted`; every promotion lock stays closed
//! (D127 / ADR-076 §14 / R140 / D536 / D545).
//!
//! This module reads the committed carriers (filesystem) in the derive function
//! only; [`build_independent_preservation_receipt`] and
//! [`validate_independent_preservation_receipt`] are pure functions of their
//! inputs.

use crate::native_acceptance_package::{
    INDEPENDENCE_SERIES_IDENTITY_RULE, SERIES_IDENTITY_BASIS_CONSERVATION,
    SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES, SERIES_IDENTITY_VERDICT_SAME_SERIES,
    SeriesIdentity, derive_series_identity, series_fingerprint,
};
use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    Redaction, THRESHOLDS_NOT_ACCEPTED, sha256_prefixed,
};
use crate::native_route_threshold_candidates::{
    COMPARABILITY_NOT_COMPARABLE, CandidateValue, ReferenceValue, ThresholdCandidateRow,
};
use da_parse::{REASON_MIXED_SCOPE_COMPARISON, Series};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

/// Schema version of the emitted independent-preservation-candidate artifact.
pub const INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION: &str =
    "independent-preservation-candidate.v1";
/// Default artifact path of the independent-preservation-candidate artifact.
pub const DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL: &str =
    "artifacts/parse_compat/independent_preservation_candidate.json";

/// The only resolution the closed vocabulary admits: an independent candidate is
/// not comparable to the frozen reference population.
pub const RESOLUTION_NOT_COMPARABLE: &str = COMPARABILITY_NOT_COMPARABLE;
/// The closed resolution vocabulary.
pub const RESOLUTION_VOCABULARY: [&str; 1] = [RESOLUTION_NOT_COMPARABLE];

/// Carrier: the DATA declaration that carries the route corpus id.
pub const LAYOUT_DECLARATION_REL: &str = "data/parse/layout_bakeoff.yaml";
/// Carrier: the published route-axis identity (candidate profile + revision).
pub const ROUTE_INDEPENDENCE_REL: &str = "artifacts/parse_compat/native_route_independence.json";
/// Carrier: the executed/evaluated reference probe (reference profile + oracle).
pub const RELEASE_PROBE_REL: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
/// Carrier: the closed absorption-progress schema (census id + freeze revision).
pub const ABSORPTION_SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";
/// Carrier: the acceptance package that carries the earlier conservation result.
pub const ACCEPTANCE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
/// Carrier: the M515 requirement-coverage reconciliation with the disclosed residual.
pub const M515_RECONCILIATION_REL: &str =
    "artifacts/parse_compat/m515_requirement_coverage_reconciliation.json";

/// Pointer to the route corpus id inside [`LAYOUT_DECLARATION_REL`].
pub const POINTER_ROUTE_CORPUS_ID: &str = "native_route_corpus.id";
/// Pointer to the route-axis comparability identity inside [`ROUTE_INDEPENDENCE_REL`].
pub const POINTER_ROUTE_INDEPENDENCE_IDENTITY: &str = "axis.comparability_identity";
/// Pointer to the evaluated oracle inside [`RELEASE_PROBE_REL`].
pub const POINTER_PROBE_EVALUATED_ORACLE: &str = "evaluated_population.oracle";
/// Pointer to the executed route profile inside [`RELEASE_PROBE_REL`].
pub const POINTER_PROBE_EXECUTED_PROFILE: &str = "executed_population.execution_route_profile";
/// Pointer to the assigned census id inside [`ABSORPTION_SCHEMA_REL`].
pub const POINTER_SCHEMA_CENSUS_ID: &str = "assigned_population.census_id";
/// Pointer to the freeze revision inside [`ABSORPTION_SCHEMA_REL`].
pub const POINTER_SCHEMA_FREEZE_REVISION: &str = "threshold_policy.freeze_revision";
/// Pointer to the earlier conservation result inside [`ACCEPTANCE_PACKAGE_REL`].
pub const POINTER_PACKAGE_SERIES_IDENTITY: &str = "carrier_independence.series_identity";
/// Pointer to the disclosed residual inside [`M515_RECONCILIATION_REL`].
pub const POINTER_M515_RESIDUAL: &str = "c1_series_identity_residual";

// ───────────────────────────── value types ─────────────────────────────

/// One side's series identity as emitted: the four keys plus their fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesBlock {
    /// The series corpus id.
    pub corpus_id: String,
    /// The series execution profile.
    pub profile: String,
    /// The series oracle.
    pub oracle: String,
    /// The series source revision.
    pub source_revision: String,
    /// The `sha256:` fingerprint of the four keys under the published
    /// canonicalization.
    pub fingerprint: String,
}

impl SeriesBlock {
    /// Build a block from a series, deriving the fingerprint (never restated).
    pub fn from_series(series: &Series) -> Self {
        Self {
            corpus_id: series.corpus_id.clone(),
            profile: series.profile.clone(),
            oracle: series.oracle.clone(),
            source_revision: series.source_revision.clone(),
            fingerprint: series_fingerprint(series),
        }
    }
}

/// One frozen threshold resolved against the independent candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdResolutionRow {
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
    /// The independently measured candidate value.
    pub candidate: CandidateValue,
    /// The resolution; one of [`RESOLUTION_VOCABULARY`].
    pub resolution: String,
    /// The typed reason the resolution holds.
    pub reason: String,
}

/// A binding of the retained conservation record to a committed carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConservationBinding {
    /// Repository-relative carrier path.
    pub path: String,
    /// Pointer inside the carrier.
    pub pointer: String,
    /// `sha256:` digest of the committed carrier bytes.
    pub sha256: String,
}

/// The earlier conservation result, retained as its own disclosed record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedConservation {
    /// The retained series-identity verdict; always `same_series`.
    pub verdict: String,
    /// The retained basis; always conservation of the frozen reference series.
    pub basis: String,
    /// The committed carriers the record is bound to.
    pub bindings: Vec<ConservationBinding>,
}

/// The emitted independent-preservation-candidate artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentPreservationCandidate {
    /// Schema version.
    pub schema_version: String,
    /// `diagnostic_only`.
    pub claim_scope: String,
    /// `none`.
    pub product_coverage_effect: String,
    /// `not_accepted`.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// The independent candidate series.
    pub candidate_series: SeriesBlock,
    /// The frozen reference series.
    pub reference_series: SeriesBlock,
    /// The derived series identity (verdict, both fingerprints, basis).
    pub series_identity: SeriesIdentity,
    /// The three frozen thresholds resolved against the candidate.
    pub thresholds_resolved: Vec<ThresholdResolutionRow>,
    /// The retained earlier conservation result.
    pub retained_conservation: RetainedConservation,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl IndependentPreservationCandidate {
    /// Serialize canonically (pretty JSON, one trailing newline) and refuse if any
    /// payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, IndependentPreservationRefusal> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            IndependentPreservationRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            IndependentPreservationRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        assert_independent_preservation_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, IndependentPreservationRefusal> {
        serde_json::from_slice(bytes).map_err(|error| {
            IndependentPreservationRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_independent_preservation_redacted(
    json: &str,
) -> Result<(), IndependentPreservationRefusal> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(IndependentPreservationRefusal::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── refusals ─────────────────────────────

/// Typed refusals of the independent-preservation candidate. Every variant is a
/// refusal, never a silent downgrade.
#[derive(Debug, Clone, PartialEq)]
pub enum IndependentPreservationRefusal {
    /// A carrier could not be read, parsed or navigated.
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
    /// A series key was empty, so the identity is unresolved.
    SeriesKeyEmpty { side: String, key: String },
    /// The candidate is the frozen reference series: the artifact is not
    /// independent.
    CandidateNotIndependent {
        verdict: String,
        candidate_fingerprint: String,
        reference_fingerprint: String,
    },
    /// The recorded series identity is not the derived one.
    SeriesIdentityDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The artifact carries no threshold rows.
    ThresholdRowsEmpty,
    /// A resolution is outside the closed vocabulary.
    ResolutionOutsideVocabulary {
        threshold_id: String,
        resolution: String,
    },
    /// An independent identity resolved a row as comparable.
    ResolutionNotNotComparable {
        threshold_id: String,
        resolution: String,
    },
    /// A row's reason drifted from the typed mixed-scope reason.
    ResolutionReasonDrift {
        threshold_id: String,
        expected: String,
        found: String,
    },
    /// A row's candidate denominator is zero or unbound.
    UnboundCandidateDenominator { threshold_id: String },
    /// A row restated a fraction value instead of deriving it.
    FractionValueNotDerived { threshold_id: String, side: String },
    /// The retained conservation record is not the earlier `same_series` record.
    RetainedConservationNotSameSeries { found: String },
    /// The retained conservation record's basis is not conservation.
    RetainedConservationBasisDrift { found: String },
    /// A retained binding drifted from the committed carrier bytes.
    RetainedBindingDrift {
        path: String,
        pointer: String,
        expected: String,
        found: String,
    },
    /// The m515 residual was presented as resolved.
    RetainedResidualPresentedAsResolved {
        presented_as_resolved: bool,
        must_not_be_presented_as_resolved: bool,
    },
}

impl fmt::Display for IndependentPreservationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedCarrier { detail } => {
                write!(
                    f,
                    "independent preservation candidate: malformed carrier: {detail}"
                )
            }
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::BindingDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::ClaimScopeNotDiagnostic { found } => write!(
                f,
                "independent preservation candidate: claim scope `{found}` is not `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "independent preservation candidate: product coverage `{found}` is not `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::ThresholdClaimed { found } => write!(
                f,
                "independent preservation candidate: envelope thresholds `{found}` are not `{THRESHOLDS_NOT_ACCEPTED}`"
            ),
            Self::LockOpened {
                lock,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: lock `{lock}` is open: expected `{expected}`, found `{found}`"
            ),
            Self::RedactionLeak { marker } => write!(
                f,
                "independent preservation candidate: artifact leaked marker `{marker}`"
            ),
            Self::SeriesKeyEmpty { side, key } => write!(
                f,
                "independent preservation candidate: the {side} series key `{key}` is empty"
            ),
            Self::CandidateNotIndependent {
                verdict,
                candidate_fingerprint,
                reference_fingerprint,
            } => write!(
                f,
                "independent preservation candidate: the candidate is not independent (verdict `{verdict}`, candidate `{candidate_fingerprint}`, reference `{reference_fingerprint}`)"
            ),
            Self::SeriesIdentityDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: series identity `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::ThresholdRowsEmpty => write!(
                f,
                "independent preservation candidate: the artifact carries no threshold rows"
            ),
            Self::ResolutionOutsideVocabulary {
                threshold_id,
                resolution,
            } => write!(
                f,
                "independent preservation candidate: `{threshold_id}` resolves `{resolution}`, outside the closed vocabulary"
            ),
            Self::ResolutionNotNotComparable {
                threshold_id,
                resolution,
            } => write!(
                f,
                "independent preservation candidate: `{threshold_id}` resolves `{resolution}` although the candidate series is independent and must resolve `{RESOLUTION_NOT_COMPARABLE}`"
            ),
            Self::ResolutionReasonDrift {
                threshold_id,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: `{threshold_id}` reason drifted: expected `{expected}`, found `{found}`"
            ),
            Self::UnboundCandidateDenominator { threshold_id } => write!(
                f,
                "independent preservation candidate: `{threshold_id}` carries a zero or unbound candidate denominator"
            ),
            Self::FractionValueNotDerived { threshold_id, side } => write!(
                f,
                "independent preservation candidate: `{threshold_id}` restated the {side} fraction value instead of deriving it"
            ),
            Self::RetainedConservationNotSameSeries { found } => write!(
                f,
                "independent preservation candidate: the retained conservation verdict `{found}` is not `{SERIES_IDENTITY_VERDICT_SAME_SERIES}`"
            ),
            Self::RetainedConservationBasisDrift { found } => write!(
                f,
                "independent preservation candidate: the retained conservation basis `{found}` does not name conservation"
            ),
            Self::RetainedBindingDrift {
                path,
                pointer,
                expected,
                found,
            } => write!(
                f,
                "independent preservation candidate: retained binding `{path}#{pointer}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::RetainedResidualPresentedAsResolved {
                presented_as_resolved,
                must_not_be_presented_as_resolved,
            } => write!(
                f,
                "independent preservation candidate: the m515 residual reads presented_as_resolved `{presented_as_resolved}` / must_not_be_presented_as_resolved `{must_not_be_presented_as_resolved}`"
            ),
        }
    }
}

impl std::error::Error for IndependentPreservationRefusal {}

// ───────────────────────────── expectation ─────────────────────────────

/// The recomputed expectation a receipt is validated against: the two series,
/// the derived identity and the retained conservation record, all read from the
/// committed carriers.
#[derive(Debug, Clone, PartialEq)]
pub struct IndependentPreservationExpectation {
    /// The independent candidate series (the route corpus's own four keys).
    pub candidate_series: Series,
    /// The frozen reference series the acceptance package builds.
    pub reference_series: Series,
    /// The derived series identity (verdict, both fingerprints, basis).
    pub series_identity: SeriesIdentity,
    /// The retained conservation record with recomputed bindings.
    pub retained_conservation: RetainedConservation,
    /// The m515 residual's `presented_as_resolved` flag, read from its carrier.
    pub retained_residual_presented_as_resolved: bool,
    /// The m515 residual's `must_not_be_presented_as_resolved` flag.
    pub retained_residual_must_not_be_presented_as_resolved: bool,
}

fn malformed(detail: impl Into<String>) -> IndependentPreservationRefusal {
    IndependentPreservationRefusal::MalformedCarrier {
        detail: detail.into(),
    }
}

fn drift(
    field: &str,
    expected: impl ToString,
    found: impl ToString,
) -> IndependentPreservationRefusal {
    IndependentPreservationRefusal::BindingDrift {
        field: field.to_owned(),
        expected: expected.to_string(),
        found: found.to_string(),
    }
}

fn read_carrier(root: &Path, rel: &str) -> Result<Vec<u8>, IndependentPreservationRefusal> {
    fs::read(root.join(rel)).map_err(|error| malformed(format!("cannot read `{rel}`: {error}")))
}

fn json_navigate<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Result<&'a serde_json::Value, IndependentPreservationRefusal> {
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| malformed(format!("carrier is missing `{path}`")))?;
    }
    Ok(current)
}

fn json_str<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Result<&'a str, IndependentPreservationRefusal> {
    json_navigate(value, path)?
        .as_str()
        .ok_or_else(|| malformed(format!("`{path}` is not a string")))
}

fn json_bool(
    value: &serde_json::Value,
    path: &str,
) -> Result<bool, IndependentPreservationRefusal> {
    json_navigate(value, path)?
        .as_bool()
        .ok_or_else(|| malformed(format!("`{path}` is not a boolean")))
}

fn yaml_navigate<'a>(
    value: &'a Yaml,
    path: &str,
) -> Result<&'a Yaml, IndependentPreservationRefusal> {
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| malformed(format!("carrier is missing `{path}`")))?;
    }
    Ok(current)
}

fn yaml_str<'a>(value: &'a Yaml, path: &str) -> Result<&'a str, IndependentPreservationRefusal> {
    yaml_navigate(value, path)?
        .as_str()
        .ok_or_else(|| malformed(format!("`{path}` is not a string")))
}

/// Assemble the expectation from the two series and the retained facts, refusing
/// by name if any series key is empty or if the candidate is the frozen reference
/// series (equal fingerprints).
fn assemble_expectation(
    candidate_series: Series,
    reference_series: Series,
    retained_conservation: RetainedConservation,
    retained_residual_presented_as_resolved: bool,
    retained_residual_must_not_be_presented_as_resolved: bool,
) -> Result<IndependentPreservationExpectation, IndependentPreservationRefusal> {
    for (side, series) in [
        ("candidate", &candidate_series),
        ("reference", &reference_series),
    ] {
        for (key, value) in [
            ("corpus_id", &series.corpus_id),
            ("profile", &series.profile),
            ("oracle", &series.oracle),
            ("source_revision", &series.source_revision),
        ] {
            if value.trim().is_empty() {
                return Err(IndependentPreservationRefusal::SeriesKeyEmpty {
                    side: side.to_owned(),
                    key: key.to_owned(),
                });
            }
        }
    }

    let identity = derive_series_identity(
        &Some(candidate_series.clone()),
        &Some(reference_series.clone()),
    );
    if identity.verdict != SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES {
        return Err(IndependentPreservationRefusal::CandidateNotIndependent {
            verdict: identity.verdict,
            candidate_fingerprint: identity.candidate_fingerprint,
            reference_fingerprint: identity.reference_fingerprint,
        });
    }

    Ok(IndependentPreservationExpectation {
        candidate_series,
        reference_series,
        series_identity: identity,
        retained_conservation,
        retained_residual_presented_as_resolved,
        retained_residual_must_not_be_presented_as_resolved,
    })
}

/// Read the committed carriers and recompute the expectation the receipt is
/// validated against.
///
/// The candidate series is the independent preservation-measurement series: the
/// route corpus id, the route-axis profile, the *measurement* oracle (the da-cli
/// compact non-validating scan oracle the release probe evaluates, **not** the
/// route-axis Java-guard oracle) and the route-axis source revision. The reference
/// series is the frozen series the acceptance package builds: the assigned census
/// id, the executed route profile, the same evaluated oracle and the frozen freeze
/// revision. A candidate whose four keys equal the reference's is refused by name.
pub fn derive_independent_preservation_expectation(
    root: &Path,
) -> Result<IndependentPreservationExpectation, IndependentPreservationRefusal> {
    let layout_bytes = read_carrier(root, LAYOUT_DECLARATION_REL)?;
    let layout: Yaml = serde_yaml::from_slice(&layout_bytes).map_err(|error| {
        malformed(format!(
            "{LAYOUT_DECLARATION_REL} is not valid YAML: {error}"
        ))
    })?;
    let candidate_corpus_id = yaml_str(&layout, POINTER_ROUTE_CORPUS_ID)?.to_owned();

    let independence_bytes = read_carrier(root, ROUTE_INDEPENDENCE_REL)?;
    let independence: serde_json::Value =
        serde_json::from_slice(&independence_bytes).map_err(|error| {
            malformed(format!(
                "{ROUTE_INDEPENDENCE_REL} is not valid JSON: {error}"
            ))
        })?;
    let candidate_profile = json_str(
        &independence,
        &format!("{POINTER_ROUTE_INDEPENDENCE_IDENTITY}.profile"),
    )?
    .to_owned();
    let candidate_revision = json_str(
        &independence,
        &format!("{POINTER_ROUTE_INDEPENDENCE_IDENTITY}.source_revision"),
    )?
    .to_owned();

    let probe_bytes = read_carrier(root, RELEASE_PROBE_REL)?;
    let probe: serde_json::Value = serde_json::from_slice(&probe_bytes)
        .map_err(|error| malformed(format!("{RELEASE_PROBE_REL} is not valid JSON: {error}")))?;
    let oracle = json_str(&probe, POINTER_PROBE_EVALUATED_ORACLE)?.to_owned();
    let reference_profile = json_str(&probe, POINTER_PROBE_EXECUTED_PROFILE)?.to_owned();

    let schema_bytes = read_carrier(root, ABSORPTION_SCHEMA_REL)?;
    let schema: Yaml = serde_yaml::from_slice(&schema_bytes).map_err(|error| {
        malformed(format!(
            "{ABSORPTION_SCHEMA_REL} is not valid YAML: {error}"
        ))
    })?;
    let reference_corpus_id = yaml_str(&schema, POINTER_SCHEMA_CENSUS_ID)?.to_owned();
    let reference_revision = yaml_str(&schema, POINTER_SCHEMA_FREEZE_REVISION)?.to_owned();

    let candidate_series = Series {
        corpus_id: candidate_corpus_id,
        profile: candidate_profile,
        oracle: oracle.clone(),
        source_revision: candidate_revision,
    };
    let reference_series = Series {
        corpus_id: reference_corpus_id,
        profile: reference_profile,
        oracle,
        source_revision: reference_revision,
    };

    // The earlier conservation result, bound by path + pointer + sha256.
    let package_bytes = read_carrier(root, ACCEPTANCE_PACKAGE_REL)?;
    let package: serde_json::Value = serde_json::from_slice(&package_bytes).map_err(|error| {
        malformed(format!(
            "{ACCEPTANCE_PACKAGE_REL} is not valid JSON: {error}"
        ))
    })?;
    let package_verdict = json_str(
        &package,
        &format!("{POINTER_PACKAGE_SERIES_IDENTITY}.verdict"),
    )?
    .to_owned();
    let package_basis = json_str(
        &package,
        &format!("{POINTER_PACKAGE_SERIES_IDENTITY}.basis"),
    )?
    .to_owned();

    let m515_bytes = read_carrier(root, M515_RECONCILIATION_REL)?;
    let m515: serde_json::Value = serde_json::from_slice(&m515_bytes).map_err(|error| {
        malformed(format!(
            "{M515_RECONCILIATION_REL} is not valid JSON: {error}"
        ))
    })?;
    let residual_presented_as_resolved = json_bool(
        &m515,
        &format!("{POINTER_M515_RESIDUAL}.presented_as_resolved"),
    )?;
    let residual_must_not_be_presented_as_resolved = json_bool(
        &m515,
        &format!("{POINTER_M515_RESIDUAL}.must_not_be_presented_as_resolved"),
    )?;

    let retained_conservation = RetainedConservation {
        verdict: package_verdict,
        basis: package_basis,
        bindings: vec![
            ConservationBinding {
                path: ACCEPTANCE_PACKAGE_REL.to_owned(),
                pointer: POINTER_PACKAGE_SERIES_IDENTITY.to_owned(),
                sha256: sha256_prefixed(&package_bytes),
            },
            ConservationBinding {
                path: M515_RECONCILIATION_REL.to_owned(),
                pointer: POINTER_M515_RESIDUAL.to_owned(),
                sha256: sha256_prefixed(&m515_bytes),
            },
        ],
    };

    assemble_expectation(
        candidate_series,
        reference_series,
        retained_conservation,
        residual_presented_as_resolved,
        residual_must_not_be_presented_as_resolved,
    )
}

// ───────────────────────────── build ─────────────────────────────

fn resolve_row(row: &ThresholdCandidateRow) -> ThresholdResolutionRow {
    ThresholdResolutionRow {
        threshold_id: row.threshold_id.clone(),
        metric_id: row.metric_id.clone(),
        stratum: row.stratum.clone(),
        direction: row.direction.clone(),
        margin: row.margin,
        // The fraction value is derived from the fraction, never restated.
        reference: ReferenceValue {
            fraction: row.reference.fraction,
            value: row.reference.fraction.value(),
        },
        candidate: CandidateValue {
            fraction: row.candidate.fraction,
            value: row.candidate.fraction.value(),
            denominator: row.candidate.denominator,
        },
        resolution: RESOLUTION_NOT_COMPARABLE.to_owned(),
        reason: REASON_MIXED_SCOPE_COMPARISON.to_owned(),
    }
}

/// Build the independent-preservation-candidate receipt from the recomputed
/// expectation and the measured S03 threshold-candidate rows.
///
/// The candidate series is independent, so every row resolves `not_comparable`
/// with the typed `mixed_scope_comparison` reason and both the retained pin and
/// the candidate fraction/denominator are shown. The earlier conservation result
/// is retained verbatim.
pub fn build_independent_preservation_receipt(
    expectation: &IndependentPreservationExpectation,
    measured_rows: &[ThresholdCandidateRow],
) -> Result<IndependentPreservationCandidate, IndependentPreservationRefusal> {
    if expectation.series_identity.verdict != SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES {
        return Err(IndependentPreservationRefusal::CandidateNotIndependent {
            verdict: expectation.series_identity.verdict.clone(),
            candidate_fingerprint: expectation.series_identity.candidate_fingerprint.clone(),
            reference_fingerprint: expectation.series_identity.reference_fingerprint.clone(),
        });
    }

    let thresholds_resolved = measured_rows.iter().map(resolve_row).collect();

    let locks = REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect();

    let receipt = IndependentPreservationCandidate {
        schema_version: INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks,
        candidate_series: SeriesBlock::from_series(&expectation.candidate_series),
        reference_series: SeriesBlock::from_series(&expectation.reference_series),
        series_identity: expectation.series_identity.clone(),
        thresholds_resolved,
        retained_conservation: expectation.retained_conservation.clone(),
        redaction: Redaction::clean(),
    };
    validate_independent_preservation_receipt(expectation, &receipt)?;
    Ok(receipt)
}

// ───────────────────────────── validation ─────────────────────────────

fn check_series_block(
    side: &str,
    block: &SeriesBlock,
    expected: &SeriesBlock,
) -> Result<(), IndependentPreservationRefusal> {
    for (field, found, want) in [
        ("corpus_id", &block.corpus_id, &expected.corpus_id),
        ("profile", &block.profile, &expected.profile),
        ("oracle", &block.oracle, &expected.oracle),
        (
            "source_revision",
            &block.source_revision,
            &expected.source_revision,
        ),
        ("fingerprint", &block.fingerprint, &expected.fingerprint),
    ] {
        if found != want {
            return Err(drift(&format!("{side}.{field}"), want, found));
        }
    }
    Ok(())
}

/// Re-read an independent-preservation-candidate receipt against the committed
/// carriers. This is the independent half of the contract: it refuses the
/// receipt-level mutations a builder cannot express.
pub fn validate_independent_preservation_receipt(
    expectation: &IndependentPreservationExpectation,
    receipt: &IndependentPreservationCandidate,
) -> Result<(), IndependentPreservationRefusal> {
    if receipt.schema_version != INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION {
        return Err(IndependentPreservationRefusal::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(IndependentPreservationRefusal::ClaimScopeNotDiagnostic {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(IndependentPreservationRefusal::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(IndependentPreservationRefusal::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }
    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(IndependentPreservationRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(IndependentPreservationRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }
    if receipt.redaction != Redaction::clean() {
        return Err(IndependentPreservationRefusal::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    // ── the two series, recomputed ────────────────────────────────────────
    let expected_candidate = SeriesBlock::from_series(&expectation.candidate_series);
    let expected_reference = SeriesBlock::from_series(&expectation.reference_series);
    check_series_block(
        "candidate_series",
        &receipt.candidate_series,
        &expected_candidate,
    )?;
    check_series_block(
        "reference_series",
        &receipt.reference_series,
        &expected_reference,
    )?;
    if receipt.candidate_series.fingerprint == receipt.reference_series.fingerprint {
        return Err(IndependentPreservationRefusal::CandidateNotIndependent {
            verdict: expectation.series_identity.verdict.clone(),
            candidate_fingerprint: receipt.candidate_series.fingerprint.clone(),
            reference_fingerprint: receipt.reference_series.fingerprint.clone(),
        });
    }

    // ── the derived series identity ───────────────────────────────────────
    let identity = &receipt.series_identity;
    for (field, found, want) in [
        (
            "verdict",
            &identity.verdict,
            &expectation.series_identity.verdict,
        ),
        (
            "candidate_fingerprint",
            &identity.candidate_fingerprint,
            &expectation.series_identity.candidate_fingerprint,
        ),
        (
            "reference_fingerprint",
            &identity.reference_fingerprint,
            &expectation.series_identity.reference_fingerprint,
        ),
        ("basis", &identity.basis, &expectation.series_identity.basis),
    ] {
        if found != want {
            return Err(IndependentPreservationRefusal::SeriesIdentityDrift {
                field: field.to_owned(),
                expected: want.clone(),
                found: found.clone(),
            });
        }
    }
    if identity.rule != INDEPENDENCE_SERIES_IDENTITY_RULE {
        return Err(IndependentPreservationRefusal::SeriesIdentityDrift {
            field: "rule".to_owned(),
            expected: INDEPENDENCE_SERIES_IDENTITY_RULE.to_owned(),
            found: identity.rule.clone(),
        });
    }

    // ── the threshold rows ────────────────────────────────────────────────
    if receipt.thresholds_resolved.is_empty() {
        return Err(IndependentPreservationRefusal::ThresholdRowsEmpty);
    }
    let independent =
        expectation.series_identity.verdict == SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES;
    for row in &receipt.thresholds_resolved {
        if row.threshold_id.trim().is_empty() || row.metric_id.trim().is_empty() {
            return Err(drift(
                "thresholds_resolved[].threshold_id/metric_id",
                "<non-empty>",
                "<empty>",
            ));
        }
        if !RESOLUTION_VOCABULARY.contains(&row.resolution.as_str()) {
            return Err(
                IndependentPreservationRefusal::ResolutionOutsideVocabulary {
                    threshold_id: row.threshold_id.clone(),
                    resolution: row.resolution.clone(),
                },
            );
        }
        if independent && row.resolution != RESOLUTION_NOT_COMPARABLE {
            return Err(IndependentPreservationRefusal::ResolutionNotNotComparable {
                threshold_id: row.threshold_id.clone(),
                resolution: row.resolution.clone(),
            });
        }
        if row.reason != REASON_MIXED_SCOPE_COMPARISON {
            return Err(IndependentPreservationRefusal::ResolutionReasonDrift {
                threshold_id: row.threshold_id.clone(),
                expected: REASON_MIXED_SCOPE_COMPARISON.to_owned(),
                found: row.reason.clone(),
            });
        }
        if row.candidate.denominator == 0 || row.candidate.fraction.denominator == 0 {
            return Err(
                IndependentPreservationRefusal::UnboundCandidateDenominator {
                    threshold_id: row.threshold_id.clone(),
                },
            );
        }
        if row.reference.fraction.denominator == 0 {
            return Err(
                IndependentPreservationRefusal::UnboundCandidateDenominator {
                    threshold_id: row.threshold_id.clone(),
                },
            );
        }
        if row.reference.value != row.reference.fraction.value() {
            return Err(IndependentPreservationRefusal::FractionValueNotDerived {
                threshold_id: row.threshold_id.clone(),
                side: "reference".to_owned(),
            });
        }
        if row.candidate.value != row.candidate.fraction.value() {
            return Err(IndependentPreservationRefusal::FractionValueNotDerived {
                threshold_id: row.threshold_id.clone(),
                side: "candidate".to_owned(),
            });
        }
    }

    // ── the retained conservation record ──────────────────────────────────
    if expectation.retained_conservation.verdict != SERIES_IDENTITY_VERDICT_SAME_SERIES {
        return Err(
            IndependentPreservationRefusal::RetainedConservationNotSameSeries {
                found: expectation.retained_conservation.verdict.clone(),
            },
        );
    }
    if receipt.retained_conservation.verdict != SERIES_IDENTITY_VERDICT_SAME_SERIES {
        return Err(
            IndependentPreservationRefusal::RetainedConservationNotSameSeries {
                found: receipt.retained_conservation.verdict.clone(),
            },
        );
    }
    if !receipt
        .retained_conservation
        .basis
        .contains(SERIES_IDENTITY_BASIS_CONSERVATION)
    {
        return Err(
            IndependentPreservationRefusal::RetainedConservationBasisDrift {
                found: receipt.retained_conservation.basis.clone(),
            },
        );
    }
    for expected_binding in &expectation.retained_conservation.bindings {
        let found = receipt
            .retained_conservation
            .bindings
            .iter()
            .find(|binding| {
                binding.path == expected_binding.path && binding.pointer == expected_binding.pointer
            })
            .ok_or_else(|| IndependentPreservationRefusal::RetainedBindingDrift {
                path: expected_binding.path.clone(),
                pointer: expected_binding.pointer.clone(),
                expected: expected_binding.sha256.clone(),
                found: "<absent>".to_owned(),
            })?;
        if found.sha256 != expected_binding.sha256 {
            return Err(IndependentPreservationRefusal::RetainedBindingDrift {
                path: expected_binding.path.clone(),
                pointer: expected_binding.pointer.clone(),
                expected: expected_binding.sha256.clone(),
                found: found.sha256.clone(),
            });
        }
    }

    // ── the m515 residual stays disclosed, never resolved ─────────────────
    if expectation.retained_residual_presented_as_resolved
        || !expectation.retained_residual_must_not_be_presented_as_resolved
    {
        return Err(
            IndependentPreservationRefusal::RetainedResidualPresentedAsResolved {
                presented_as_resolved: expectation.retained_residual_presented_as_resolved,
                must_not_be_presented_as_resolved: expectation
                    .retained_residual_must_not_be_presented_as_resolved,
            },
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_route_threshold_candidates::Fraction;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn sample_row() -> ThresholdCandidateRow {
        let reference_fraction = Fraction {
            numerator: 74910,
            denominator: 79621,
        };
        let candidate_fraction = Fraction {
            numerator: 1441,
            denominator: 1457,
        };
        ThresholdCandidateRow {
            threshold_id: "preservation_micro_occurrence".to_owned(),
            metric_id: "preservation.occurrence_conservation".to_owned(),
            stratum: "micro_occurrence".to_owned(),
            direction: "at_least".to_owned(),
            margin: 0.0,
            reference: ReferenceValue {
                fraction: reference_fraction,
                value: reference_fraction.value(),
            },
            candidate: CandidateValue {
                fraction: candidate_fraction,
                value: candidate_fraction.value(),
                denominator: 1457,
            },
            verdict: "not_comparable".to_owned(),
            comparability: "not_comparable".to_owned(),
        }
    }

    fn sample_retained() -> RetainedConservation {
        RetainedConservation {
            verdict: SERIES_IDENTITY_VERDICT_SAME_SERIES.to_owned(),
            basis: SERIES_IDENTITY_BASIS_CONSERVATION.to_owned(),
            bindings: Vec::new(),
        }
    }

    fn real_expectation() -> IndependentPreservationExpectation {
        derive_independent_preservation_expectation(&repo_root())
            .expect("the committed carriers derive an independent expectation")
    }

    #[test]
    fn expectation_derives_independent_series_from_committed_carriers() {
        let expectation = real_expectation();
        assert_eq!(
            expectation.series_identity.verdict,
            SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES
        );
        assert_ne!(
            expectation.series_identity.candidate_fingerprint,
            expectation.series_identity.reference_fingerprint
        );
        assert_eq!(
            expectation.series_identity.reference_fingerprint,
            "sha256:c5a9d938f31660452032b4d9b86a4e8bebb621f93ae7d0a57bb28ed12778f161"
        );
        assert_eq!(
            expectation.retained_conservation.verdict,
            SERIES_IDENTITY_VERDICT_SAME_SERIES
        );
        assert!(!expectation.retained_residual_presented_as_resolved);
        assert!(expectation.retained_residual_must_not_be_presented_as_resolved);
    }

    #[test]
    fn build_validate_roundtrip_from_measured_rows() {
        let expectation = real_expectation();
        let receipt = build_independent_preservation_receipt(&expectation, &[sample_row()])
            .expect("the receipt builds and validates");
        assert_eq!(receipt.thresholds_resolved.len(), 1);
        let row = &receipt.thresholds_resolved[0];
        assert_eq!(row.resolution, RESOLUTION_NOT_COMPARABLE);
        assert_eq!(row.reason, REASON_MIXED_SCOPE_COMPARISON);
        assert_eq!(row.candidate.denominator, 1457);
        assert_eq!(
            row.candidate.value.to_bits(),
            (1441.0f64 / 1457.0).to_bits()
        );

        let bytes = receipt.artifact_json_bytes().expect("serializes redacted");
        let parsed = IndependentPreservationCandidate::parse_json_bytes(&bytes)
            .expect("parses canonical bytes");
        assert_eq!(parsed, receipt);
        validate_independent_preservation_receipt(&expectation, &parsed)
            .expect("the parsed receipt re-validates");
    }

    #[test]
    fn fraction_value_is_derived_not_restated() {
        let expectation = real_expectation();
        // A row that restates the value instead of deriving it is refused.
        let mut receipt =
            build_independent_preservation_receipt(&expectation, &[sample_row()]).expect("builds");
        receipt.thresholds_resolved[0].reference.value = 0.999;
        let error = validate_independent_preservation_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            IndependentPreservationRefusal::FractionValueNotDerived { side, .. } if side == "reference"
        ));

        // Build derives the value from the fraction: a restated value is discarded.
        let mut restated = sample_row();
        restated.candidate.value = 12345.0;
        let built =
            build_independent_preservation_receipt(&expectation, &[restated]).expect("builds");
        assert_eq!(
            built.thresholds_resolved[0].candidate.value.to_bits(),
            (1441.0f64 / 1457.0).to_bits()
        );
    }

    #[test]
    fn identity_derivation_refuses_equal_fingerprint_candidate() {
        let series = Series {
            corpus_id: "corpus".to_owned(),
            profile: "profile".to_owned(),
            oracle: "oracle".to_owned(),
            source_revision: "sha256:0".to_owned(),
        };
        let error = assemble_expectation(series.clone(), series, sample_retained(), false, true)
            .unwrap_err();
        assert!(matches!(
            error,
            IndependentPreservationRefusal::CandidateNotIndependent { .. }
        ));
    }

    #[test]
    fn empty_series_key_is_refused() {
        let candidate = Series {
            corpus_id: String::new(),
            profile: "profile".to_owned(),
            oracle: "oracle".to_owned(),
            source_revision: "sha256:1".to_owned(),
        };
        let reference = Series {
            corpus_id: "corpus".to_owned(),
            profile: "profile".to_owned(),
            oracle: "oracle".to_owned(),
            source_revision: "sha256:2".to_owned(),
        };
        let error =
            assemble_expectation(candidate, reference, sample_retained(), false, true).unwrap_err();
        assert!(matches!(
            error,
            IndependentPreservationRefusal::SeriesKeyEmpty { key, .. } if key == "corpus_id"
        ));
    }

    #[test]
    fn resolution_outside_the_closed_vocabulary_is_refused() {
        let expectation = real_expectation();
        let mut receipt =
            build_independent_preservation_receipt(&expectation, &[sample_row()]).expect("builds");
        receipt.thresholds_resolved[0].resolution = "met".to_owned();
        let error = validate_independent_preservation_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            IndependentPreservationRefusal::ResolutionOutsideVocabulary { .. }
        ));
    }

    #[test]
    fn zero_candidate_denominator_is_refused() {
        let expectation = real_expectation();
        let mut receipt =
            build_independent_preservation_receipt(&expectation, &[sample_row()]).expect("builds");
        receipt.thresholds_resolved[0].candidate.denominator = 0;
        let error = validate_independent_preservation_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            IndependentPreservationRefusal::UnboundCandidateDenominator { .. }
        ));
    }

    #[test]
    fn redaction_sweep_refuses_a_leaked_marker() {
        assert!(assert_independent_preservation_redacted("{\"host\":\"localhost\"}").is_err());
        assert!(assert_independent_preservation_redacted("{\"ok\":true}").is_ok());
    }
}
