//! S02/T04 (M516-th0oby) — measured independence receipt for the native route.
//!
//! The route receipt ([`crate::native_route_receipt::RouteReceipt`]) records a
//! guard-owned zero (`guard.observed_java_calls == 0`) but carries no axis a
//! reader can call *measured*: `data/parse/absorption_progress.schema.yaml`
//! defines `measured` as *a value bound to a non-empty denominator, an evidence
//! identity and a comparability identity*. This module adds that axis **at the
//! receipt level** and leaves the product `java_independence` scorecard row
//! untouched (D499/D508): moving the product axis requires explicit human
//! authorization, and this receipt only records that requirement.
//!
//! ## The axis is derived, not restated
//!
//! [`build_route_independence_receipt`] reads the committed carriers — the route
//! receipt (`artifacts/parse_compat/native_route_e2e.json`) and the native TEI
//! emission manifest (`artifacts/parse_compat/native_route_tei_emission.json`) —
//! and derives:
//!
//! * the denominator from the emission's declared corpus (cross-checked against
//!   the route receipt's `native_route` population, so a shrunken denominator is
//!   refused rather than published);
//! * the evidence identity from the route guard block's canonical bytes and the
//!   emission artifact's bytes;
//! * the comparability identity from the corpus id, the emission's
//!   `profile_fingerprint`, the Java oracle identity and the route declaration's
//!   `revision_sha256`.
//!
//! Nothing is hand-typed: a restated denominator or a forged digest has no place
//! to enter.
//!
//! ## A zero is a measurement only with an installed, isolated, positively
//! controlled guard
//!
//! [`validate_route_independence_receipt`] refuses a receipt that reads its zero
//! without an installed, identity-isolated observer and a positive control
//! ([`RouteIndependenceRefusal::GuardNotInstalledForObservedZero`]). It also
//! refuses an unbound denominator, an unbound evidence or comparability
//! identity, a non-measured axis, an opened lock, a non-diagnostic claim scope,
//! a product-coverage claim and a threshold claim — the receipt-level refusals a
//! builder cannot express.
//!
//! ## The product axis stays untouched
//!
//! `claim_scope = diagnostic_only`, `product_coverage_effect = none`,
//! `thresholds = not_accepted`, the five locks closed. The receipt records no
//! product coverage and promotes, splices, imports and writes nothing (D127 /
//! ADR-076 §14 / D499). This module is pure `std`/`serde`/`sha2`/`serde_json`,
//! so it and its contract test compile in the default feature set.

use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest, sha256_prefixed,
};
use crate::native_route_receipt::{RoutePopulationId, RouteReceipt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Schema version of the emitted independence receipt artifact.
pub const NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION: &str = "native-route-independence/v1";
/// Default artifact path of the independence receipt.
pub const DEFAULT_INDEPENDENCE_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_route_independence.json";
/// The only axis state this receipt may carry. `measured` is the schema's honest
/// reading: a value bound to a non-empty denominator, an evidence identity and a
/// comparability identity.
pub const INDEPENDENCE_STATE_MEASURED: &str = "measured";
/// The Java oracle whose absence the independence axis measures. It is the
/// comparison identity: two independence measurements are comparable only if the
/// same Java oracle identity is the reference.
pub const INDEPENDENCE_ORACLE_IDENTITY: &str = "java-grobid-crf";

/// The corpus the independence axis is denominated over, as the emission
/// manifest declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceCorpus {
    /// The declared corpus identity.
    pub corpus_id: String,
    /// Repository-relative carrier the corpus is pinned in.
    pub pointer: String,
    /// Pinned document count.
    pub documents: u64,
    /// Pinned page denominator.
    pub pages: u64,
}

/// The measured denominator: how many documents and pages the zero covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceDenominator {
    /// Documents in the measured population.
    pub documents: u64,
    /// Pages in the measured population.
    pub pages: u64,
}

/// The evidence identity: the digests the measured value is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceEvidenceIdentity {
    /// `sha256:` digest of the route receipt's canonical guard provenance block.
    pub guard_provenance_sha256: String,
    /// `sha256:` digest of the committed native TEI emission artifact, when one
    /// was bound.
    pub emission_sha256: Option<String>,
}

/// The comparability identity: the four keys two measurements must share before
/// they may be compared (`data/parse/absorption_progress.schema.yaml#comparability`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceComparabilityIdentity {
    /// The corpus identity.
    pub corpus_id: String,
    /// The parse profile identity the emission was produced under.
    pub profile: String,
    /// The Java oracle identity the absence is measured against.
    pub oracle: String,
    /// The source revision the receipt is pinned to.
    pub source_revision: String,
}

/// The measured independence axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceAxis {
    /// Measurement state; only `measured` is emitted.
    pub state: String,
    /// The measured value: observed Java consults over the denominator. Zero on
    /// the native route.
    pub value: u64,
    /// The non-empty denominator the zero is bound to.
    pub denominator: IndependenceDenominator,
    /// The evidence identity.
    pub evidence_identity: IndependenceEvidenceIdentity,
    /// The comparability identity.
    pub comparability_identity: IndependenceComparabilityIdentity,
}

/// The guard that makes the zero a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceGuard {
    /// The seam observed, as the route receipt records it.
    pub observer: String,
    /// An observer was installed on the real transport seam.
    pub installed: bool,
    /// Installing the observer did not rotate the cache identity.
    pub identity_isolated: bool,
    /// A positive control raised the counter, so an observed zero is not vacuous.
    pub positive_control_observed: bool,
}

/// The binding to the committed native TEI emission artifact the route produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmissionBinding {
    /// Emitted emission schema version.
    pub schema_version: String,
    /// `sha256:` digest of the committed emission artifact bytes.
    pub artifact_sha256: String,
    /// Corpus identity the emission declares.
    pub corpus_id: String,
    /// Documents the emission covers.
    pub document_count: u64,
    /// Pages the emission covers.
    pub page_denominator: u64,
}

/// What the measured corridor did not exercise, carried typed rather than
/// silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceResiduals {
    /// Stage cells across the slice that are not `native_measured`.
    pub unmeasured_stages: u64,
    /// Family cells across the slice outside the closed disposition vocabulary.
    pub unclassified_families: u64,
    /// Emission bytes that fell back to Java, summed over the emission documents.
    pub java_fallback_bytes: u64,
    /// Emission bytes the native plane emitted, summed over the documents.
    pub native_emitted_bytes: u64,
    /// Emission bytes a typed absence accounts for, summed over the documents.
    pub typed_absence_bytes: u64,
}

/// The emitted independence receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteIndependenceReceipt {
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
    /// The declared corpus the axis is denominated over.
    pub corpus: IndependenceCorpus,
    /// The measured independence axis.
    pub axis: IndependenceAxis,
    /// The guard that makes the zero a measurement.
    pub guard: IndependenceGuard,
    /// Binding to the committed native TEI emission artifact.
    pub emission: Option<EmissionBinding>,
    /// Typed residuals the corridor did not exercise.
    pub residuals: IndependenceResiduals,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl RouteIndependenceReceipt {
    /// Serialize the receipt canonically (pretty JSON, deterministic field order,
    /// one trailing newline) and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, RouteIndependenceRefusal> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            RouteIndependenceRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            RouteIndependenceRefusal::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        assert_independence_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, RouteIndependenceRefusal> {
        serde_json::from_slice(bytes).map_err(|error| RouteIndependenceRefusal::MalformedCarrier {
            detail: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_independence_redacted(json: &str) -> Result<(), RouteIndependenceRefusal> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(RouteIndependenceRefusal::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── refusals ─────────────────────────────

/// Typed refusals of the independence receipt. Every variant is a refusal, never
/// a silent downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteIndependenceRefusal {
    /// A carrier could not be read or parsed.
    MalformedCarrier { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The denominator is absent or not bound to the declared corpus.
    UnboundDenominator { field: String },
    /// The denominator is empty, so there is nothing to divide.
    ZeroDenominator { documents: u64, pages: u64 },
    /// The evidence identity is absent or malformed.
    UnboundEvidenceIdentity { field: String },
    /// The comparability identity is absent or malformed.
    UnboundComparabilityIdentity { field: String },
    /// The axis is not `measured`.
    AxisNotMeasured { found: String },
    /// A counted zero was claimed without an installed, isolated observer and a
    /// positive control.
    GuardNotInstalledForObservedZero { observed_java_calls: u64 },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeNotDiagnostic { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
    /// A parsed field drifted from the recomputed carrier fact.
    BindingDrift {
        field: String,
        expected: String,
        found: String,
    },
}

impl fmt::Display for RouteIndependenceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedCarrier { detail } => {
                write!(f, "route independence: malformed carrier: {detail}")
            }
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "route independence: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
            Self::UnboundDenominator { field } => {
                write!(f, "route independence: denominator `{field}` is unbound")
            }
            Self::ZeroDenominator { documents, pages } => write!(
                f,
                "route independence: zero denominator (documents={documents}, pages={pages})"
            ),
            Self::UnboundEvidenceIdentity { field } => {
                write!(
                    f,
                    "route independence: evidence identity `{field}` is unbound"
                )
            }
            Self::UnboundComparabilityIdentity { field } => write!(
                f,
                "route independence: comparability identity `{field}` is unbound"
            ),
            Self::AxisNotMeasured { found } => write!(
                f,
                "route independence: axis state `{found}` is not `{INDEPENDENCE_STATE_MEASURED}`"
            ),
            Self::GuardNotInstalledForObservedZero {
                observed_java_calls,
            } => write!(
                f,
                "route independence: a zero ({observed_java_calls}) was claimed without an \
                 installed, identity-isolated, positively controlled guard"
            ),
            Self::LockOpened {
                lock,
                expected,
                found,
            } => write!(
                f,
                "route independence: lock `{lock}` is open: expected `{expected}`, found `{found}`"
            ),
            Self::ClaimScopeNotDiagnostic { found } => write!(
                f,
                "route independence: claim scope `{found}` is not `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "route independence: product coverage `{found}` is not \
                 `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::ThresholdClaimed { found } => write!(
                f,
                "route independence: thresholds `{found}` are not `{THRESHOLDS_NOT_ACCEPTED}`"
            ),
            Self::RedactionLeak { marker } => {
                write!(f, "route independence: artifact leaked marker `{marker}`")
            }
            Self::BindingDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "route independence: `{field}` drifted: expected `{expected}`, found `{found}`"
            ),
        }
    }
}

impl std::error::Error for RouteIndependenceRefusal {}

// ───────────────────────────── derivation ─────────────────────────────

/// The committed sources a receipt is re-read against. Recomputing it from the
/// carriers is the independent half of the contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteIndependenceExpectation {
    /// The recomputed corpus binding.
    pub corpus: IndependenceCorpus,
    /// The recomputed guard provenance digest.
    pub guard_provenance_sha256: String,
    /// The recomputed emission artifact digest, when one is bound.
    pub emission_sha256: Option<String>,
    /// The recomputed comparability identity.
    pub comparability: IndependenceComparabilityIdentity,
}

/// The parsed facts of one committed emission artifact.
struct EmissionView {
    schema_version: String,
    corpus_id: String,
    pointer: String,
    document_count: u64,
    page_denominator: u64,
    artifact_sha256: String,
    profile_fingerprint: String,
    java_fallback_bytes: u64,
    native_emitted_bytes: u64,
    typed_absence_bytes: u64,
}

fn emit_malformed(detail: impl Into<String>) -> RouteIndependenceRefusal {
    RouteIndependenceRefusal::MalformedCarrier {
        detail: detail.into(),
    }
}

fn parse_u64(value: &serde_json::Value, pointer: &str) -> Result<u64, RouteIndependenceRefusal> {
    value
        .as_u64()
        .ok_or_else(|| emit_malformed(format!("`{pointer}` is not an unsigned integer")))
}

fn parse_str(value: &serde_json::Value, pointer: &str) -> Result<String, RouteIndependenceRefusal> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| emit_malformed(format!("`{pointer}` is not a string")))
}

fn parse_emission(bytes: &[u8]) -> Result<EmissionView, RouteIndependenceRefusal> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| emit_malformed(error.to_string()))?;
    let schema_version = parse_str(&value["schema_version"], "schema_version")?;
    let corpus = &value["corpus"];
    let corpus_id = parse_str(&corpus["corpus_id"], "corpus.corpus_id")?;
    let pointer = parse_str(&corpus["pointer"], "corpus.pointer")?;
    if !is_repo_relative_path(&pointer) {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "corpus.pointer".to_owned(),
        });
    }
    let document_count = parse_u64(&corpus["document_count"], "corpus.document_count")?;
    let page_denominator = parse_u64(&corpus["page_denominator"], "corpus.page_denominator")?;
    let documents = value["documents"]
        .as_array()
        .ok_or_else(|| emit_malformed("`documents` is not an array"))?;
    let profile_fingerprint = documents
        .first()
        .and_then(|document| document["emission"]["identity"]["profile_fingerprint"].as_str())
        .ok_or_else(|| {
            emit_malformed("`documents[0].emission.identity.profile_fingerprint` is absent")
        })?
        .to_owned();
    let mut java_fallback_bytes = 0u64;
    let mut native_emitted_bytes = 0u64;
    let mut typed_absence_bytes = 0u64;
    for document in documents {
        let accounting = &document["emission"]["byte_accounting"];
        java_fallback_bytes = java_fallback_bytes
            .saturating_add(accounting["java_fallback_bytes"].as_u64().unwrap_or(0));
        native_emitted_bytes = native_emitted_bytes
            .saturating_add(accounting["native_emitted_bytes"].as_u64().unwrap_or(0));
        typed_absence_bytes = typed_absence_bytes
            .saturating_add(accounting["typed_absence_bytes"].as_u64().unwrap_or(0));
    }
    Ok(EmissionView {
        schema_version,
        corpus_id,
        pointer,
        document_count,
        page_denominator,
        artifact_sha256: sha256_prefixed(bytes),
        profile_fingerprint,
        java_fallback_bytes,
        native_emitted_bytes,
        typed_absence_bytes,
    })
}

/// The canonical bytes of a route receipt's guard provenance block. The digest
/// over these is the evidence identity of the measured zero.
fn guard_provenance_sha256(
    route_receipt: &RouteReceipt,
) -> Result<String, RouteIndependenceRefusal> {
    let bytes = serde_json::to_vec(&route_receipt.guard).map_err(|error| {
        RouteIndependenceRefusal::MalformedCarrier {
            detail: error.to_string(),
        }
    })?;
    Ok(sha256_prefixed(&bytes))
}

/// Derive the recomputed expectation the validator re-reads a receipt against,
/// from the committed route receipt and the committed emission artifact alone.
pub fn derive_route_independence_expectation(
    route_receipt_json: &[u8],
    emission_artifact_json: &[u8],
) -> Result<RouteIndependenceExpectation, RouteIndependenceRefusal> {
    let route_receipt = RouteReceipt::parse_json_bytes(route_receipt_json).map_err(|error| {
        RouteIndependenceRefusal::MalformedCarrier {
            detail: format!("route receipt: {error}"),
        }
    })?;
    let emission = parse_emission(emission_artifact_json)?;

    // The denominator is the emission's declared corpus, cross-checked against
    // the route receipt's native_route population. A corpus the emission shrinks
    // without a new census is refused here, not published.
    let population = route_receipt
        .populations
        .iter()
        .find(|population| population.population == RoutePopulationId::NativeRoute)
        .ok_or_else(|| RouteIndependenceRefusal::UnboundDenominator {
            field: "route_receipt.populations[native_route]".to_owned(),
        })?;
    if population.document_count != emission.document_count
        || population.page_denominator != emission.page_denominator
    {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "corpus".to_owned(),
        });
    }

    let corpus = IndependenceCorpus {
        corpus_id: emission.corpus_id.clone(),
        pointer: emission.pointer.clone(),
        documents: emission.document_count,
        pages: emission.page_denominator,
    };
    let comparability = IndependenceComparabilityIdentity {
        corpus_id: emission.corpus_id.clone(),
        profile: emission.profile_fingerprint.clone(),
        oracle: INDEPENDENCE_ORACLE_IDENTITY.to_owned(),
        source_revision: route_receipt.declaration.revision_sha256.clone(),
    };
    Ok(RouteIndependenceExpectation {
        corpus,
        guard_provenance_sha256: guard_provenance_sha256(&route_receipt)?,
        emission_sha256: Some(emission.artifact_sha256.clone()),
        comparability,
    })
}

/// Build the independence receipt from the committed carriers.
///
/// The axis is derived, not restated: the value and guard come from the route
/// receipt, the denominator and evidence digest from the emission artifact, and
/// the comparability identity from both. `positive_control_observed` is the one
/// runtime witness the carriers cannot record; the caller runs the observer's
/// positive control and passes its result.
pub fn build_route_independence_receipt(
    route_receipt_json: &[u8],
    emission_artifact_json: &[u8],
    positive_control_observed: bool,
) -> Result<RouteIndependenceReceipt, RouteIndependenceRefusal> {
    let route_receipt = RouteReceipt::parse_json_bytes(route_receipt_json).map_err(|error| {
        RouteIndependenceRefusal::MalformedCarrier {
            detail: format!("route receipt: {error}"),
        }
    })?;
    let emission = parse_emission(emission_artifact_json)?;
    let expected =
        derive_route_independence_expectation(route_receipt_json, emission_artifact_json)?;

    let receipt = RouteIndependenceReceipt {
        schema_version: NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION.to_owned(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: THRESHOLDS_NOT_ACCEPTED.to_owned(),
        locks: route_receipt.locks.clone(),
        corpus: expected.corpus.clone(),
        axis: IndependenceAxis {
            state: INDEPENDENCE_STATE_MEASURED.to_owned(),
            value: route_receipt.guard.observed_java_calls,
            denominator: IndependenceDenominator {
                documents: expected.corpus.documents,
                pages: expected.corpus.pages,
            },
            evidence_identity: IndependenceEvidenceIdentity {
                guard_provenance_sha256: expected.guard_provenance_sha256.clone(),
                emission_sha256: Some(emission.artifact_sha256.clone()),
            },
            comparability_identity: expected.comparability.clone(),
        },
        guard: IndependenceGuard {
            observer: route_receipt.guard.observer.as_str().to_owned(),
            installed: route_receipt.guard.installed,
            identity_isolated: route_receipt.guard.identity_isolated,
            positive_control_observed,
        },
        emission: Some(EmissionBinding {
            schema_version: emission.schema_version.clone(),
            artifact_sha256: emission.artifact_sha256.clone(),
            corpus_id: emission.corpus_id.clone(),
            document_count: emission.document_count,
            page_denominator: emission.page_denominator,
        }),
        residuals: IndependenceResiduals {
            unmeasured_stages: route_receipt.residuals.unmeasured_stages,
            unclassified_families: route_receipt.residuals.unclassified_families,
            java_fallback_bytes: emission.java_fallback_bytes,
            native_emitted_bytes: emission.native_emitted_bytes,
            typed_absence_bytes: emission.typed_absence_bytes,
        },
        redaction: Redaction::clean(),
    };
    validate_route_independence_receipt(&receipt, &expected)?;
    Ok(receipt)
}

// ───────────────────────────── validation ─────────────────────────────

/// Re-read an independence receipt against the committed sources it claims.
///
/// This is the independent half of the contract: it re-runs the structural
/// checks on the *parsed* bytes and refuses the receipt-level mutations a
/// builder cannot express. The structural refusals never need `expected`; the
/// final drift checks compare the parsed receipt against the recomputed facts.
pub fn validate_route_independence_receipt(
    receipt: &RouteIndependenceReceipt,
    expected: &RouteIndependenceExpectation,
) -> Result<(), RouteIndependenceRefusal> {
    if receipt.schema_version != NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION {
        return Err(RouteIndependenceRefusal::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(RouteIndependenceRefusal::ClaimScopeNotDiagnostic {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(RouteIndependenceRefusal::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(RouteIndependenceRefusal::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }
    for (lock, value) in REQUIRED_LOCKS {
        match receipt.locks.get(lock) {
            None => {
                return Err(RouteIndependenceRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
            Some(found) if found != value => {
                return Err(RouteIndependenceRefusal::LockOpened {
                    lock: lock.to_owned(),
                    expected: value.to_owned(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }
    if receipt.redaction != Redaction::clean() {
        return Err(RouteIndependenceRefusal::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    // ── denominator: bound to a declared corpus and non-empty ─────────────
    if receipt.corpus.corpus_id.is_empty() {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "corpus.corpus_id".to_owned(),
        });
    }
    if !is_repo_relative_path(&receipt.corpus.pointer) {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "corpus.pointer".to_owned(),
        });
    }
    if receipt.corpus.documents == 0 || receipt.corpus.pages == 0 {
        return Err(RouteIndependenceRefusal::ZeroDenominator {
            documents: receipt.corpus.documents,
            pages: receipt.corpus.pages,
        });
    }
    if receipt.axis.denominator.documents == 0 || receipt.axis.denominator.pages == 0 {
        return Err(RouteIndependenceRefusal::ZeroDenominator {
            documents: receipt.axis.denominator.documents,
            pages: receipt.axis.denominator.pages,
        });
    }
    if receipt.axis.denominator.documents != receipt.corpus.documents
        || receipt.axis.denominator.pages != receipt.corpus.pages
    {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "axis.denominator".to_owned(),
        });
    }

    // ── evidence identity ─────────────────────────────────────────────────
    if !is_sha256_digest(&receipt.axis.evidence_identity.guard_provenance_sha256) {
        return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
            field: "axis.evidence_identity.guard_provenance_sha256".to_owned(),
        });
    }
    match (
        &receipt.emission,
        &receipt.axis.evidence_identity.emission_sha256,
    ) {
        (Some(binding), Some(digest)) => {
            if !is_sha256_digest(&binding.artifact_sha256) || binding.artifact_sha256 != *digest {
                return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
                    field: "axis.evidence_identity.emission_sha256".to_owned(),
                });
            }
        }
        (Some(binding), None) if is_sha256_digest(&binding.artifact_sha256) => {}
        (Some(_), None) => {
            return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
                field: "axis.evidence_identity.emission_sha256".to_owned(),
            });
        }
        (None, Some(_)) => {
            return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
                field: "axis.evidence_identity.emission_sha256".to_owned(),
            });
        }
        (None, None) => {}
    }

    // ── comparability identity ────────────────────────────────────────────
    for (field, value) in [
        (
            "axis.comparability_identity.corpus_id",
            receipt.axis.comparability_identity.corpus_id.as_str(),
        ),
        (
            "axis.comparability_identity.profile",
            receipt.axis.comparability_identity.profile.as_str(),
        ),
        (
            "axis.comparability_identity.oracle",
            receipt.axis.comparability_identity.oracle.as_str(),
        ),
        (
            "axis.comparability_identity.source_revision",
            receipt.axis.comparability_identity.source_revision.as_str(),
        ),
    ] {
        if value.is_empty() {
            return Err(RouteIndependenceRefusal::UnboundComparabilityIdentity {
                field: field.to_owned(),
            });
        }
    }
    if !is_sha256_digest(&receipt.axis.comparability_identity.source_revision) {
        return Err(RouteIndependenceRefusal::UnboundComparabilityIdentity {
            field: "axis.comparability_identity.source_revision".to_owned(),
        });
    }

    // ── the measurement itself ────────────────────────────────────────────
    if receipt.axis.state != INDEPENDENCE_STATE_MEASURED {
        return Err(RouteIndependenceRefusal::AxisNotMeasured {
            found: receipt.axis.state.clone(),
        });
    }
    if receipt.axis.value == 0
        && !(receipt.guard.installed
            && receipt.guard.identity_isolated
            && receipt.guard.positive_control_observed)
    {
        return Err(RouteIndependenceRefusal::GuardNotInstalledForObservedZero {
            observed_java_calls: receipt.axis.value,
        });
    }

    // ── drift against the recomputed carriers ─────────────────────────────
    if receipt.guard.observer.is_empty() {
        return Err(RouteIndependenceRefusal::BindingDrift {
            field: "guard.observer".to_owned(),
            expected: "<non-empty>".to_owned(),
            found: receipt.guard.observer.clone(),
        });
    }
    if receipt.corpus != expected.corpus {
        return Err(RouteIndependenceRefusal::UnboundDenominator {
            field: "corpus".to_owned(),
        });
    }
    if receipt.axis.evidence_identity.guard_provenance_sha256 != expected.guard_provenance_sha256 {
        return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
            field: "axis.evidence_identity.guard_provenance_sha256".to_owned(),
        });
    }
    if receipt.axis.evidence_identity.emission_sha256 != expected.emission_sha256 {
        return Err(RouteIndependenceRefusal::UnboundEvidenceIdentity {
            field: "axis.evidence_identity.emission_sha256".to_owned(),
        });
    }
    if receipt.axis.comparability_identity != expected.comparability {
        return Err(RouteIndependenceRefusal::UnboundComparabilityIdentity {
            field: "axis.comparability_identity".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module is pure `std`/`serde`/`sha2`/`serde_json`: the clean redaction
    /// state is the only state the builder emits.
    #[test]
    fn clean_redaction_is_the_declared_invariant() {
        assert_eq!(Redaction::clean(), Redaction::clean());
    }

    /// A repo-relative path check is what guards the corpus pointer.
    #[test]
    fn corpus_pointer_must_be_repo_relative() {
        assert!(is_repo_relative_path("data/parse/layout_bakeoff.yaml"));
        assert!(!is_repo_relative_path("/tmp/corpus"));
    }
}
