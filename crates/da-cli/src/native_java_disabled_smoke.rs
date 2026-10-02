//! S09/T04 (M514-1lss2s) — process-local Java-disabled smoke rehearsal.
//!
//! The slice sketch asks for a Java-disabled smoke that confirms the product
//! scope, but only **after** a genuine human release decision. That decision does
//! not exist today (T01 refuses, T02 records the blocked state), so the harness
//! must exist now and be provably fail-closed: it may never run unauthorized and
//! may never be read as a product claim.
//!
//! This module is that harness, and it is deliberately **not a runtime route**:
//!
//! * it is not registered in `ParserPort`, `source_cli` or any `da` subcommand;
//! * it performs no filesystem, network, clock, RNG or tracing access (pure
//!   `std`/`serde`/`sha2` over `da-parse` value types);
//! * it never writes to production state; the ungated example writes the receipt
//!   artifact and nothing else.
//!
//! ## What it decides
//!
//! It consumes the T01 [`NativeRetirementAuthorization`] (the verbatim verdict,
//! never a re-declaration) and a declared per-family selection, plus a guard
//! observation and a positive-control observation:
//!
//! * **live (not authorized)** — a typed `not_authorized` refusal:
//!   `families_switched = []`, every selection row `switched = false`,
//!   `product_scope_confirmed = false`, no product-coverage claim, the five locks
//!   closed, redaction clean. No measurement is carried, because no rehearsal ran.
//! * **authorized** — Java is disabled for exactly the selected families, the
//!   guard must be installed and identity-isolated, `observed_java_calls` must be
//!   `0`, and a **positive control** must have observed a synthetic Java consult
//!   through the same guard vocabulary. Without that control a zero would be
//!   vacuity rather than a measurement, so the builder refuses it
//!   (`positive_control_not_observed`).
//! * **authorized with a blocked family** — refused: a family whose required
//!   gate is still blocking cannot be switched (`family_gate_blocks`).
//!
//! ## Guard vocabulary
//!
//! The emitted guard row is the same [`GuardProvenanceReceipt`] the S06 route
//! receipt carries (`observer = java_transport_fetch_tei`, `installed`,
//! `identity_isolated`, `observed_java_calls`), and the positive control is
//! driven through the real [`JavaInvocationCounter`] transport guard so a zero
//! read back is a measurement rather than an absence.

use crate::native_family_receipt::{
    CarrierBinding, GuardObserver, GuardProvenance, GuardProvenanceReceipt, LEAK_MARKERS,
    REQUIRED_LOCKS, Redaction, is_repo_relative_path, is_sha256_digest,
};
use crate::native_retirement_readiness::{
    READINESS_ACCEPTANCE_PACKAGE_REL, READINESS_JAVA_DISABLED_SMOKE_REL, READINESS_MANIFEST_REL,
};
use da_parse::{
    JavaInvocationCounter, JavaInvocationObservation, NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT,
    NativeRetirementAuthorization, RETIREMENT_CLAIM_SCOPE_AUTHORIZED,
    RETIREMENT_CLAIM_SCOPE_BLOCKED,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted smoke receipt.
pub const NATIVE_JAVA_DISABLED_SMOKE_SCHEMA_VERSION: &str = "native-java-disabled-smoke.v1";
/// Default artifact path of the smoke receipt. It is the exact path the T02
/// readiness receipt names in `next_required_action.java_disabled_smoke_receipt`,
/// so the two cannot drift.
pub const DEFAULT_JAVA_DISABLED_SMOKE_ARTIFACT_REL: &str = READINESS_JAVA_DISABLED_SMOKE_REL;

/// The rehearsal ran under a recorded authorization: the selected families were
/// switched with Java disabled and zero consults observed.
pub const JAVA_DISABLED_SMOKE_AUTHORIZED: &str = "authorized_smoke";
/// The rehearsal refused: no recorded authorization exists, so nothing ran.
pub const JAVA_DISABLED_SMOKE_NOT_AUTHORIZED: &str = "not_authorized";

/// The only product-coverage effect a smoke receipt may carry. A smoke confirms a
/// scope under an authorization; it never confers product coverage.
pub const SMOKE_PRODUCT_COVERAGE_EFFECT: &str = NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT;

/// The evidence carriers the receipt binds: the S08 gate ledger and the
/// committed human release record. Both are the carriers the T01 verdict rests
/// on, so the smoke cannot be read against a different authorization.
pub const SMOKE_EVIDENCE_CARRIERS: [(&str, &str); 2] = [
    (READINESS_ACCEPTANCE_PACKAGE_REL, "mandatory_gates"),
    (
        READINESS_MANIFEST_REL,
        "release_decision_package.human_release_review",
    ),
];

// ── closed refusal vocabulary ───────────────────────────────────────────────

/// The T01 authorization refuses, so no family may be switched and no smoke ran.
pub const SMOKE_REFUSAL_NOT_AUTHORIZED: &str = "not_authorized";
/// A selected family is authorized but still carries a blocking gate.
pub const SMOKE_REFUSAL_FAMILY_GATE_BLOCKS: &str = "family_gate_blocks";
/// A selected family is not authorized by the T01 verdict.
pub const SMOKE_REFUSAL_FAMILY_NOT_AUTHORIZED: &str = "family_not_authorized";
/// A selected family is absent from the authorization ledger.
pub const SMOKE_REFUSAL_FAMILY_NOT_IN_AUTHORIZATION: &str = "family_not_in_authorization";
/// A family is switched without appearing in the declared selection.
pub const SMOKE_REFUSAL_FAMILY_NOT_SELECTED: &str = "family_not_selected";
/// A family is switched but not marked switched in its selection row.
pub const SMOKE_REFUSAL_FAMILY_NOT_SWITCHED: &str = "family_not_switched";
/// A zero was claimed without an installed guard.
pub const SMOKE_REFUSAL_GUARD_NOT_INSTALLED: &str = "guard_not_installed";
/// The guard would rotate the cache identity, so its zero is unusable.
pub const SMOKE_REFUSAL_GUARD_IDENTITY_NOT_ISOLATED: &str = "guard_identity_not_isolated";
/// A zero was claimed with no positive control, so it is vacuous.
pub const SMOKE_REFUSAL_POSITIVE_CONTROL_NOT_OBSERVED: &str = "positive_control_not_observed";
/// The positive control attempted and observed different counts.
pub const SMOKE_REFUSAL_POSITIVE_CONTROL_INCONSISTENT: &str = "positive_control_inconsistent";
/// Java was consulted while the selected families were disabled.
pub const SMOKE_REFUSAL_JAVA_CONSULTED: &str = "java_consulted_on_disabled_families";
/// A product scope was claimed while the authorization refuses.
pub const SMOKE_REFUSAL_PRODUCT_SCOPE_CLAIMED: &str = "product_scope_claimed_without_authorization";
/// An authorized receipt lost the confirmed scope it must carry.
pub const SMOKE_REFUSAL_PRODUCT_SCOPE_UNCONFIRMED: &str = "product_scope_not_confirmed";
/// The carried authorization disagrees with the re-derived verdict.
pub const SMOKE_REFUSAL_AUTHORIZATION_DRIFT: &str = "authorization_drift";
/// A required typed refusal is absent from a refused receipt.
pub const SMOKE_REFUSAL_MISSING: &str = "required_refusal_absent";

/// The complete closed refusal vocabulary this harness may emit. A code outside
/// this set is an untyped refusal, so the contract test pins the set.
pub const JAVA_DISABLED_SMOKE_REFUSAL_VOCABULARY: [&str; 14] = [
    SMOKE_REFUSAL_NOT_AUTHORIZED,
    SMOKE_REFUSAL_FAMILY_GATE_BLOCKS,
    SMOKE_REFUSAL_FAMILY_NOT_AUTHORIZED,
    SMOKE_REFUSAL_FAMILY_NOT_IN_AUTHORIZATION,
    SMOKE_REFUSAL_FAMILY_NOT_SELECTED,
    SMOKE_REFUSAL_FAMILY_NOT_SWITCHED,
    SMOKE_REFUSAL_GUARD_NOT_INSTALLED,
    SMOKE_REFUSAL_GUARD_IDENTITY_NOT_ISOLATED,
    SMOKE_REFUSAL_POSITIVE_CONTROL_NOT_OBSERVED,
    SMOKE_REFUSAL_POSITIVE_CONTROL_INCONSISTENT,
    SMOKE_REFUSAL_JAVA_CONSULTED,
    SMOKE_REFUSAL_PRODUCT_SCOPE_CLAIMED,
    SMOKE_REFUSAL_PRODUCT_SCOPE_UNCONFIRMED,
    SMOKE_REFUSAL_AUTHORIZATION_DRIFT,
];

// ───────────────────────────── errors ─────────────────────────────

/// Every structural violation is a named variant so a mutation is rejected *by
/// name*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaDisabledSmokeError {
    /// The artifact could not be serialized or parsed.
    Malformed { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim scope.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// A carrier binding is malformed.
    UnboundCarrier { detail: String },
    /// A carrier binding appears twice with a different digest.
    DuplicateCarrier { manifest: String, pointer: String },
    /// The evidence list disagrees with the re-derived carriers.
    EvidenceDrift { detail: String },
    /// The carried authorization disagrees with the re-derived verdict.
    AuthorizationDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A product scope was claimed while the authorization refuses.
    ProductScopeClaimedWithoutAuthorization { detail: String },
    /// The authorized receipt did not confirm the scope.
    ProductScopeNotConfirmed,
    /// A family is switched without an authorization.
    FamilySwitchedWithoutAuthorization { family_id: String },
    /// A selected family is authorized but still gate-blocked.
    FamilyGateBlocks { family_id: String },
    /// A selected family is not authorized.
    FamilyNotAuthorized { family_id: String },
    /// A selected family is absent from the authorization ledger.
    FamilyNotInAuthorization { family_id: String },
    /// A family is switched without appearing in the selection.
    FamilyNotSelected { family_id: String },
    /// A switched family's selection row does not mark it switched.
    FamilyNotSwitched { family_id: String },
    /// A required selection family was dropped.
    SelectionDropped { family_id: String },
    /// A selection row's blocking set disagrees with the verdict.
    SelectionBlockingDrift { family_id: String },
    /// A selection row's authorization disagrees with the verdict.
    SelectionAuthorizationDrift { family_id: String },
    /// A zero was claimed without an installed guard.
    GuardNotInstalled,
    /// The guard would rotate the cache identity.
    GuardIdentityNotIsolated,
    /// Java was consulted while the selected families were disabled.
    ObservedJavaOnDisabledFamilies { attempts: u64 },
    /// A zero was claimed with no positive control.
    PositiveControlNotObserved,
    /// The positive control attempted and observed different counts.
    PositiveControlInconsistent { attempted: u64, observed: u64 },
    /// A required typed refusal is absent.
    MissingTypedRefusal { expected: String },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
}

impl fmt::Display for JavaDisabledSmokeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for JavaDisabledSmokeError {}

// ───────────────────────────── types ─────────────────────────────

/// One refused or authorized selection cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilySelectionRow {
    /// The family id.
    pub family_id: String,
    /// Whether the family was declared in the selection.
    pub selected: bool,
    /// Whether the T01 verdict authorizes the family.
    pub authorized: bool,
    /// The required, non-clear gate ids blocking the family.
    pub blocking_gates: Vec<String>,
    /// Whether the family was switched. Only an authorized, gate-clear family may be.
    pub switched: bool,
    /// Whether Java was disabled for this family. Identical to `switched`.
    pub java_disabled: bool,
}

/// The positive control: a synthetic Java consult observed through the guard, so
/// a later zero cannot be vacuity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositiveControlRow {
    /// The seam the control observes.
    pub observer: GuardObserver,
    /// Synthetic consults attempted.
    pub attempted: u64,
    /// Synthetic consults observed.
    pub observed: u64,
    /// `observed > 0`: the guard was shown to count.
    pub non_vacuous: bool,
}

/// One typed refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmokeRefusal {
    /// One of [`JAVA_DISABLED_SMOKE_REFUSAL_VOCABULARY`].
    pub code: String,
    /// Human-readable detail. Never carries payload or path material.
    pub detail: String,
    /// The family the refusal concerns, when one applies.
    pub family_id: Option<String>,
}

/// The emitted smoke receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JavaDisabledSmokeReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// `authorized_smoke` | `not_authorized`.
    pub verdict: String,
    /// The authorization scope: `blocked_pending_human_authorization` while the
    /// T01 verdict refuses, `authorized_retirement_release` once it authorizes.
    pub claim_scope: String,
    /// Always `none`: a smoke is not product coverage.
    pub product_coverage_effect: String,
    /// `true` only when the rehearsal ran authorized and observed zero consults.
    pub product_scope_confirmed: bool,
    /// The families actually switched. Empty while the authorization refuses.
    pub families_switched: Vec<String>,
    /// One row per declared selection family.
    pub selection: Vec<FamilySelectionRow>,
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The installed guard's observation.
    pub guard: GuardProvenanceReceipt,
    /// The non-vacuous positive control.
    pub positive_control: PositiveControlRow,
    /// The five locks as declared.
    pub locks: BTreeMap<String, String>,
    /// The T01 authorization verdict, carried verbatim.
    pub authorization: NativeRetirementAuthorization,
    /// The typed refusals. Empty only on a fully authorized smoke.
    pub refusals: Vec<SmokeRefusal>,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl JavaDisabledSmokeReceipt {
    /// Parse a receipt from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, JavaDisabledSmokeError> {
        serde_json::from_slice(bytes).map_err(|error| JavaDisabledSmokeError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, JavaDisabledSmokeError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| JavaDisabledSmokeError::Malformed {
                detail: error.to_string(),
            })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| JavaDisabledSmokeError::Malformed {
                detail: error.to_string(),
            })?;
        assert_java_disabled_smoke_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized receipt.
pub fn assert_java_disabled_smoke_redacted(json: &str) -> Result<(), JavaDisabledSmokeError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(JavaDisabledSmokeError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// The builder input.
#[derive(Debug, Clone)]
pub struct JavaDisabledSmokeInput {
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The T01 authorization verdict.
    pub authorization: NativeRetirementAuthorization,
    /// The declared per-family selection.
    pub selection: Vec<String>,
    /// The run guard's observation.
    pub guard: GuardProvenance,
    /// The positive-control observation.
    pub positive_control: PositiveControlProvenance,
}

/// The positive-control observation the builder consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositiveControlProvenance {
    /// Synthetic consults attempted.
    pub attempted: u64,
    /// Synthetic consults observed.
    pub observed: u64,
}

/// The re-derived expectation the validator recomputes against.
#[derive(Debug, Clone)]
pub struct JavaDisabledSmokeExpectation {
    /// The carrier bindings re-derived from the committed bytes.
    pub evidence: Vec<CarrierBinding>,
    /// The families the selection must cover.
    pub required_family_ids: Vec<String>,
    /// The independently re-derived T01 authorization.
    pub authorization: NativeRetirementAuthorization,
}

// ───────────────────────────── guard helpers ─────────────────────────────

/// Read an installed [`JavaInvocationCounter`] guard's observation as the
/// envelope's [`GuardProvenance`]. A counter exists, so its reading is always
/// `Observed`; a zero is a measurement, never an absence.
pub fn guard_from_counter(
    counter: &JavaInvocationCounter,
    identity_isolated: bool,
) -> GuardProvenance {
    match counter.observation() {
        JavaInvocationObservation::Observed { attempts } => GuardProvenance {
            installed: true,
            identity_isolated,
            observed_java_calls: attempts,
        },
        // A counter never reports `NotObserved`; the arm is kept so the mapping
        // is total rather than an `unwrap`.
        JavaInvocationObservation::NotObserved => guard_not_installed(),
    }
}

/// The uninstalled guard, used when the rehearsal does not run (not authorized).
pub const fn guard_not_installed() -> GuardProvenance {
    GuardProvenance {
        installed: false,
        identity_isolated: false,
        observed_java_calls: 0,
    }
}

/// The non-vacuous positive control: one synthetic Java consult through a real
/// counter. Returns the observed total; a counter that cannot grow would make a
/// later zero vacuous.
pub fn observe_positive_control(counter: &JavaInvocationCounter) -> u64 {
    counter.observe_attempt()
}

// ───────────────────────────── helpers ─────────────────────────────

fn check_carrier(binding: &CarrierBinding) -> Result<(), JavaDisabledSmokeError> {
    if !is_repo_relative_path(&binding.manifest)
        || binding.pointer.trim().is_empty()
        || !is_sha256_digest(&binding.revision_sha256)
    {
        return Err(JavaDisabledSmokeError::UnboundCarrier {
            detail: format!(
                "{}#{} ({})",
                binding.manifest, binding.pointer, binding.revision_sha256
            ),
        });
    }
    Ok(())
}

fn check_evidence(evidence: &[CarrierBinding]) -> Result<(), JavaDisabledSmokeError> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for binding in evidence {
        check_carrier(binding)?;
        let key = (binding.manifest.clone(), binding.pointer.clone());
        if !seen.insert(key.clone()) {
            return Err(JavaDisabledSmokeError::DuplicateCarrier {
                manifest: key.0,
                pointer: key.1,
            });
        }
    }
    Ok(())
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), JavaDisabledSmokeError> {
    for (key, expected) in REQUIRED_LOCKS {
        match locks.get(key) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(JavaDisabledSmokeError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(JavaDisabledSmokeError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn sorted_bindings(mut bindings: Vec<CarrierBinding>) -> Vec<CarrierBinding> {
    bindings.sort_by(|a, b| {
        (
            a.manifest.as_str(),
            a.pointer.as_str(),
            a.revision_sha256.as_str(),
        )
            .cmp(&(
                b.manifest.as_str(),
                b.pointer.as_str(),
                b.revision_sha256.as_str(),
            ))
    });
    bindings
}

fn refusal(code: &str, detail: impl Into<String>) -> SmokeRefusal {
    SmokeRefusal {
        code: code.to_owned(),
        detail: detail.into(),
        family_id: None,
    }
}

fn observer() -> GuardObserver {
    GuardObserver::JavaTransportFetchTei
}

// ───────────────────────────── builder ─────────────────────────────

/// Build the smoke receipt, refusing every contract violation by name.
///
/// The live state (the T01 verdict refuses) yields a typed `not_authorized`
/// receipt with no measurement. The authorized state switches exactly the
/// selected, gate-clear families and requires an installed identity-isolated
/// guard with a zero consult count and a non-vacuous positive control.
pub fn build_java_disabled_smoke_receipt(
    input: &JavaDisabledSmokeInput,
) -> Result<JavaDisabledSmokeReceipt, JavaDisabledSmokeError> {
    check_evidence(&input.evidence)?;
    check_locks(&input.authorization.locks)?;

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for family_id in &input.selection {
        if !seen.insert(family_id.as_str()) {
            return Err(JavaDisabledSmokeError::SelectionBlockingDrift {
                family_id: family_id.clone(),
            });
        }
    }

    if !input.authorization.s09_authorized {
        return build_not_authorized(input);
    }
    build_authorized(input)
}

fn build_not_authorized(
    input: &JavaDisabledSmokeInput,
) -> Result<JavaDisabledSmokeReceipt, JavaDisabledSmokeError> {
    // A rehearsal that did not run must not carry a measurement: an installed
    // guard or a positive control would be a product claim behind a refusal.
    if input.guard.installed || input.guard.observed_java_calls > 0 {
        return Err(
            JavaDisabledSmokeError::ProductScopeClaimedWithoutAuthorization {
                detail: format!(
                    "a guard was installed while the T01 verdict is `{}`",
                    input.authorization.verdict
                ),
            },
        );
    }
    if input.positive_control.attempted > 0 || input.positive_control.observed > 0 {
        return Err(
            JavaDisabledSmokeError::ProductScopeClaimedWithoutAuthorization {
                detail: "a positive control was run while the T01 verdict refuses".to_owned(),
            },
        );
    }

    let selection = selection_rows(input)?;
    Ok(JavaDisabledSmokeReceipt {
        schema_version: NATIVE_JAVA_DISABLED_SMOKE_SCHEMA_VERSION.to_owned(),
        verdict: JAVA_DISABLED_SMOKE_NOT_AUTHORIZED.to_owned(),
        claim_scope: RETIREMENT_CLAIM_SCOPE_BLOCKED.to_owned(),
        product_coverage_effect: SMOKE_PRODUCT_COVERAGE_EFFECT.to_owned(),
        product_scope_confirmed: false,
        families_switched: Vec::new(),
        selection,
        evidence: input.evidence.clone(),
        guard: GuardProvenanceReceipt {
            observer: observer(),
            installed: false,
            identity_isolated: false,
            observed_java_calls: 0,
        },
        positive_control: PositiveControlRow {
            observer: observer(),
            attempted: 0,
            observed: 0,
            non_vacuous: false,
        },
        locks: input.authorization.locks.clone(),
        authorization: input.authorization.clone(),
        refusals: vec![refusal(
            SMOKE_REFUSAL_NOT_AUTHORIZED,
            format!(
                "the T01 authorization verdict is `{}` (basis `{}`); no family may be switched and no Java-disabled smoke may run",
                input.authorization.verdict, input.authorization.basis
            ),
        )],
        redaction: Redaction::clean(),
    })
}

fn build_authorized(
    input: &JavaDisabledSmokeInput,
) -> Result<JavaDisabledSmokeReceipt, JavaDisabledSmokeError> {
    if !input.guard.installed {
        return Err(JavaDisabledSmokeError::GuardNotInstalled);
    }
    if !input.guard.identity_isolated {
        return Err(JavaDisabledSmokeError::GuardIdentityNotIsolated);
    }
    if input.guard.observed_java_calls > 0 {
        return Err(JavaDisabledSmokeError::ObservedJavaOnDisabledFamilies {
            attempts: input.guard.observed_java_calls,
        });
    }
    if input.positive_control.attempted == 0 {
        return Err(JavaDisabledSmokeError::PositiveControlNotObserved);
    }
    if input.positive_control.observed != input.positive_control.attempted {
        return Err(JavaDisabledSmokeError::PositiveControlInconsistent {
            attempted: input.positive_control.attempted,
            observed: input.positive_control.observed,
        });
    }

    let mut selection = Vec::with_capacity(input.selection.len());
    let mut families_switched = Vec::with_capacity(input.selection.len());
    for family_id in &input.selection {
        let family = input
            .authorization
            .families
            .iter()
            .find(|family| &family.family_id == family_id)
            .ok_or_else(|| JavaDisabledSmokeError::FamilyNotInAuthorization {
                family_id: family_id.clone(),
            })?;
        if !family.authorized {
            return Err(JavaDisabledSmokeError::FamilyNotAuthorized {
                family_id: family_id.clone(),
            });
        }
        if !family.blocking_gates.is_empty() {
            return Err(JavaDisabledSmokeError::FamilyGateBlocks {
                family_id: family_id.clone(),
            });
        }
        selection.push(FamilySelectionRow {
            family_id: family_id.clone(),
            selected: true,
            authorized: true,
            blocking_gates: Vec::new(),
            switched: true,
            java_disabled: true,
        });
        families_switched.push(family_id.clone());
    }

    Ok(JavaDisabledSmokeReceipt {
        schema_version: NATIVE_JAVA_DISABLED_SMOKE_SCHEMA_VERSION.to_owned(),
        verdict: JAVA_DISABLED_SMOKE_AUTHORIZED.to_owned(),
        claim_scope: RETIREMENT_CLAIM_SCOPE_AUTHORIZED.to_owned(),
        product_coverage_effect: SMOKE_PRODUCT_COVERAGE_EFFECT.to_owned(),
        product_scope_confirmed: true,
        families_switched,
        selection,
        evidence: input.evidence.clone(),
        guard: GuardProvenanceReceipt {
            observer: observer(),
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
        },
        positive_control: PositiveControlRow {
            observer: observer(),
            attempted: input.positive_control.attempted,
            observed: input.positive_control.observed,
            non_vacuous: true,
        },
        locks: input.authorization.locks.clone(),
        authorization: input.authorization.clone(),
        refusals: Vec::new(),
        redaction: Redaction::clean(),
    })
}

fn selection_rows(
    input: &JavaDisabledSmokeInput,
) -> Result<Vec<FamilySelectionRow>, JavaDisabledSmokeError> {
    let mut rows = Vec::with_capacity(input.selection.len());
    for family_id in &input.selection {
        let family = input
            .authorization
            .families
            .iter()
            .find(|family| &family.family_id == family_id)
            .ok_or_else(|| JavaDisabledSmokeError::FamilyNotInAuthorization {
                family_id: family_id.clone(),
            })?;
        rows.push(FamilySelectionRow {
            family_id: family_id.clone(),
            selected: true,
            authorized: family.authorized,
            blocking_gates: family.blocking_gates.clone(),
            switched: false,
            java_disabled: false,
        });
    }
    Ok(rows)
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed receipt and refuse what a builder cannot
/// express: a zero without a guard, a vacuous positive control, a switched
/// family outside the selection, a gate-blocked switched family, a product-scope
/// claim behind a refusal, a dropped selection family or a drifted evidence pin.
pub fn validate_java_disabled_smoke_receipt(
    receipt: &JavaDisabledSmokeReceipt,
    expectation: &JavaDisabledSmokeExpectation,
) -> Result<(), JavaDisabledSmokeError> {
    if receipt.schema_version != NATIVE_JAVA_DISABLED_SMOKE_SCHEMA_VERSION {
        return Err(JavaDisabledSmokeError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_JAVA_DISABLED_SMOKE_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }

    let authorized = match receipt.verdict.as_str() {
        JAVA_DISABLED_SMOKE_AUTHORIZED => true,
        JAVA_DISABLED_SMOKE_NOT_AUTHORIZED => false,
        other => {
            return Err(JavaDisabledSmokeError::SchemaDrift {
                field: "verdict".to_owned(),
                expected: format!(
                    "{JAVA_DISABLED_SMOKE_AUTHORIZED} | {JAVA_DISABLED_SMOKE_NOT_AUTHORIZED}"
                ),
                found: other.to_owned(),
            });
        }
    };

    if receipt.authorization.verdict != expectation.authorization.verdict
        || receipt.authorization.s09_authorized != expectation.authorization.s09_authorized
        || receipt.authorization.blocking_gates != expectation.authorization.blocking_gates
    {
        return Err(JavaDisabledSmokeError::AuthorizationDrift {
            field: "authorization".to_owned(),
            expected: format!(
                "{} ({} blocking gates)",
                expectation.authorization.verdict,
                expectation.authorization.blocking_gates.len()
            ),
            found: format!(
                "{} ({} blocking gates)",
                receipt.authorization.verdict,
                receipt.authorization.blocking_gates.len()
            ),
        });
    }

    let expected_claim_scope = if authorized {
        RETIREMENT_CLAIM_SCOPE_AUTHORIZED
    } else {
        RETIREMENT_CLAIM_SCOPE_BLOCKED
    };
    if receipt.claim_scope != expected_claim_scope {
        return Err(JavaDisabledSmokeError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != SMOKE_PRODUCT_COVERAGE_EFFECT {
        return Err(JavaDisabledSmokeError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    check_locks(&receipt.locks)?;
    check_evidence(&receipt.evidence)?;
    if sorted_bindings(expectation.evidence.clone()) != sorted_bindings(receipt.evidence.clone()) {
        return Err(JavaDisabledSmokeError::EvidenceDrift {
            detail: format!(
                "receipt binds {} carriers; the committed bytes re-derive {}",
                receipt.evidence.len(),
                expectation.evidence.len()
            ),
        });
    }

    // The selection must cover exactly the expected families and agree with the
    // carried verdict.
    let mut selection_ids: BTreeSet<&str> = BTreeSet::new();
    for row in &receipt.selection {
        if !selection_ids.insert(row.family_id.as_str()) {
            return Err(JavaDisabledSmokeError::SelectionBlockingDrift {
                family_id: row.family_id.clone(),
            });
        }
        match receipt
            .authorization
            .families
            .iter()
            .find(|family| family.family_id == row.family_id)
        {
            Some(family) => {
                if family.authorized != row.authorized {
                    return Err(JavaDisabledSmokeError::SelectionAuthorizationDrift {
                        family_id: row.family_id.clone(),
                    });
                }
                if family.blocking_gates != row.blocking_gates {
                    return Err(JavaDisabledSmokeError::SelectionBlockingDrift {
                        family_id: row.family_id.clone(),
                    });
                }
            }
            None => {
                return Err(JavaDisabledSmokeError::FamilyNotInAuthorization {
                    family_id: row.family_id.clone(),
                });
            }
        }
        if row.java_disabled != row.switched {
            return Err(JavaDisabledSmokeError::FamilyNotSwitched {
                family_id: row.family_id.clone(),
            });
        }
    }
    for family_id in &expectation.required_family_ids {
        if !selection_ids.contains(family_id.as_str()) {
            return Err(JavaDisabledSmokeError::SelectionDropped {
                family_id: family_id.clone(),
            });
        }
    }

    if !authorized {
        if receipt.product_scope_confirmed {
            return Err(
                JavaDisabledSmokeError::ProductScopeClaimedWithoutAuthorization {
                    detail: "product_scope_confirmed is true while the verdict refuses".to_owned(),
                },
            );
        }
        if let Some(family_id) = receipt.families_switched.first() {
            return Err(JavaDisabledSmokeError::FamilySwitchedWithoutAuthorization {
                family_id: family_id.clone(),
            });
        }
        if receipt.guard.installed || receipt.guard.observed_java_calls > 0 {
            return Err(
                JavaDisabledSmokeError::ProductScopeClaimedWithoutAuthorization {
                    detail: "an installed guard is carried by a refused receipt".to_owned(),
                },
            );
        }
        if receipt.selection.iter().any(|row| row.switched) {
            let family_id = receipt
                .selection
                .iter()
                .find(|row| row.switched)
                .map(|row| row.family_id.clone())
                .unwrap_or_default();
            return Err(JavaDisabledSmokeError::FamilySwitchedWithoutAuthorization { family_id });
        }
        if !receipt
            .refusals
            .iter()
            .any(|refusal| refusal.code == SMOKE_REFUSAL_NOT_AUTHORIZED)
        {
            return Err(JavaDisabledSmokeError::MissingTypedRefusal {
                expected: SMOKE_REFUSAL_NOT_AUTHORIZED.to_owned(),
            });
        }
    } else {
        if !receipt.product_scope_confirmed {
            return Err(JavaDisabledSmokeError::ProductScopeNotConfirmed);
        }
        if !receipt.guard.installed {
            return Err(JavaDisabledSmokeError::GuardNotInstalled);
        }
        if !receipt.guard.identity_isolated {
            return Err(JavaDisabledSmokeError::GuardIdentityNotIsolated);
        }
        if receipt.guard.observed_java_calls > 0 {
            return Err(JavaDisabledSmokeError::ObservedJavaOnDisabledFamilies {
                attempts: receipt.guard.observed_java_calls,
            });
        }
        if receipt.positive_control.attempted == 0 || receipt.positive_control.observed == 0 {
            return Err(JavaDisabledSmokeError::PositiveControlNotObserved);
        }
        if receipt.positive_control.observed != receipt.positive_control.attempted
            || !receipt.positive_control.non_vacuous
        {
            return Err(JavaDisabledSmokeError::PositiveControlInconsistent {
                attempted: receipt.positive_control.attempted,
                observed: receipt.positive_control.observed,
            });
        }
        let selected: BTreeMap<&str, &FamilySelectionRow> = receipt
            .selection
            .iter()
            .map(|row| (row.family_id.as_str(), row))
            .collect();
        for family_id in &receipt.families_switched {
            let row = selected.get(family_id.as_str()).ok_or_else(|| {
                JavaDisabledSmokeError::FamilyNotSelected {
                    family_id: family_id.clone(),
                }
            })?;
            if !row.authorized {
                return Err(JavaDisabledSmokeError::FamilyNotAuthorized {
                    family_id: family_id.clone(),
                });
            }
            if !row.blocking_gates.is_empty() {
                return Err(JavaDisabledSmokeError::FamilyGateBlocks {
                    family_id: family_id.clone(),
                });
            }
            if !row.switched {
                return Err(JavaDisabledSmokeError::FamilyNotSwitched {
                    family_id: family_id.clone(),
                });
            }
        }
    }

    if receipt.redaction != Redaction::clean() {
        return Err(JavaDisabledSmokeError::RedactionLeak {
            marker: "<redaction flag set>".to_owned(),
        });
    }
    let json =
        serde_json::to_string(receipt).map_err(|error| JavaDisabledSmokeError::Malformed {
            detail: error.to_string(),
        })?;
    assert_java_disabled_smoke_redacted(&json)?;

    Ok(())
}
