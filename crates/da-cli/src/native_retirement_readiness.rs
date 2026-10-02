//! S09/T02 (M514-1lss2s) — canonical blocked retirement-readiness receipt.
//!
//! The T01 authorization gate computes a typed verdict, but a verdict that lives
//! only in memory is neither reviewable nor governor-checkable: the blocked
//! state has to be recorded as a source-bound artifact that names the exact
//! blocking set, the exact human decision still required, and the fact that the
//! retirement goal is unmet. Without that record an operator can read a green
//! test, a closed milestone or a cancelled slice as permission.
//!
//! This module is that record. It binds, by repository-relative **path +
//! pointer + sha256**, the four carriers the blocked state rests on:
//!
//! * the S08 acceptance package (`mandatory_gates`);
//! * the human release record inside `data/parse/acceptance_manifest.yaml`
//!   (`release_decision_package.human_release_review`);
//! * the S02 `required_scope.families` and `authorization` blocks inside
//!   `data/parse/absorption_progress.schema.yaml`;
//! * the frozen M513 release-decision package (`remainders`).
//!
//! It carries the T01 [`NativeRetirementAuthorization`] verdict verbatim,
//! `families_switched = []`, a per-family plan where every required family names
//! the gate(s) blocking it with `in_plan = false`, and a
//! [`NextRequiredAction`] naming the human release decision and the
//! Java-disabled smoke that may only follow a recorded decision.
//!
//! ## Fail closed by construction
//!
//! * `claim_scope` is [`RETIREMENT_CLAIM_SCOPE_BLOCKED`] and never widens; the
//!   product-coverage effect is always `none` (authorization evidence is never
//!   product coverage);
//! * `goal_unmet = true`; a receipt that marks the goal met is refused;
//! * `families_switched` is empty and no family plan row is `in_plan` while the
//!   authorization refuses;
//! * the human-gate invariance is asserted: the release record is still
//!   `reviewer=null` / `decision=pending` and the M513 release package is still
//!   `pending_human_review`;
//! * the body-axis residual token must survive verbatim
//!   (`p1_localization_uncertain`); a rename is refused;
//! * the five locks stay exactly as declared, and redaction stays clean.
//!
//! The module is pure `std`/`serde`/`serde_json` over `da-parse` value types: it
//! performs no filesystem, network, clock, RNG or evaluator access. The ungated
//! example reads the committed carriers and writes the artifact; the ungated
//! contract test re-derives every pin from the committed bytes.

use crate::native_family_receipt::{
    CarrierBinding, LEAK_MARKERS, REQUIRED_LOCKS, Redaction, is_repo_relative_path,
    is_sha256_digest,
};
use da_parse::{
    NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID, NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT,
    NativeRetirementAuthorization, RETIREMENT_CLAIM_SCOPE_BLOCKED,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted readiness receipt.
pub const NATIVE_RETIREMENT_READINESS_SCHEMA_VERSION: &str = "grobid-retirement-readiness.v1";
/// Default artifact path of the readiness receipt.
pub const DEFAULT_RETIREMENT_READINESS_ARTIFACT_REL: &str =
    "artifacts/parse_compat/grobid_retirement_readiness.json";

// ── carriers ────────────────────────────────────────────────────────────────

/// The S08 acceptance package carrier.
pub const READINESS_ACCEPTANCE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
/// The manifest carrier holding the human release record.
pub const READINESS_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
/// The closed absorption-progress schema carrier.
pub const READINESS_SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";
/// The frozen M513 release-decision package carrier.
pub const READINESS_M513_RELEASE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_release_decision_package.json";

// ── pointers ────────────────────────────────────────────────────────────────

/// Pointer into the S08 package at its mandatory-gate ledger.
pub const POINTER_PACKAGE_MANDATORY_GATES: &str = "mandatory_gates";
/// Pointer into the manifest at the human release review record.
pub const POINTER_MANIFEST_HUMAN_RELEASE_REVIEW: &str =
    "release_decision_package.human_release_review";
/// Pointer into the schema at the required family list.
pub const POINTER_SCHEMA_REQUIRED_SCOPE: &str = "required_scope.families";
/// Pointer into the schema at the authorization block.
pub const POINTER_SCHEMA_AUTHORIZATION: &str = "authorization";
/// Pointer into the M513 package at its remainders.
pub const POINTER_M513_REMAINDERS: &str = "remainders";

/// The evidence carriers the receipt binds, as `(path, pointer)`. Both the
/// producing example and the contract test derive their sha256 from this one
/// list, so the bound set cannot drift.
pub const READINESS_EVIDENCE_CARRIERS: [(&str, &str); 5] = [
    (
        READINESS_ACCEPTANCE_PACKAGE_REL,
        POINTER_PACKAGE_MANDATORY_GATES,
    ),
    (
        READINESS_MANIFEST_REL,
        POINTER_MANIFEST_HUMAN_RELEASE_REVIEW,
    ),
    (READINESS_SCHEMA_REL, POINTER_SCHEMA_REQUIRED_SCOPE),
    (READINESS_SCHEMA_REL, POINTER_SCHEMA_AUTHORIZATION),
    (READINESS_M513_RELEASE_PACKAGE_REL, POINTER_M513_REMAINDERS),
];

// ── fixed vocabulary ────────────────────────────────────────────────────────

/// The only claim scope a blocked readiness receipt may carry. Reuses the T01
/// authorization vocabulary value rather than restating the token.
pub const READINESS_CLAIM_SCOPE_BLOCKED: &str = RETIREMENT_CLAIM_SCOPE_BLOCKED;
/// The only product-coverage effect: authorization is never product coverage.
pub const READINESS_PRODUCT_COVERAGE_EFFECT: &str = NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT;
/// The body-axis residual token that must survive verbatim.
pub const READINESS_BODY_RESIDUAL_TOKEN: &str = "p1_localization_uncertain";
/// The human release record decision while no decision is recorded.
pub const READINESS_HUMAN_DECISION_PENDING: &str = "pending";
/// The manifest release-package status while no decision is recorded.
pub const READINESS_PACKAGE_STATUS_PENDING: &str = "pending_human_review";
/// The M513 release-package status while no decision is recorded.
pub const READINESS_M513_PACKAGE_STATUS_PENDING: &str = "pending_human_review";
/// The named human release action the receipt records as still required.
pub const READINESS_HUMAN_DECISION_ACTION: &str = "named_human_release_decision_required";
/// The Java-disabled smoke action that may only follow a recorded decision.
pub const READINESS_JAVA_DISABLED_SMOKE_ACTION: &str =
    "java_disabled_smoke_required_after_recorded_decision";
/// The receipt the Java-disabled smoke will produce once it is run authorized.
pub const READINESS_JAVA_DISABLED_SMOKE_REL: &str =
    "artifacts/parse_compat/native_java_disabled_smoke.json";

// ───────────────────────────── errors ─────────────────────────────

/// Every refusal is a named variant so a mutation is rejected *by name*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetirementReadinessError {
    /// The artifact could not be serialized or parsed.
    Malformed { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond the blocked scope.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The receipt marked the retirement goal met while the gate refuses.
    GoalClaimedMet,
    /// The receipt was built while the T01 authorization authorized a release.
    AuthorizationNotRefused { verdict: String },
    /// A family is listed as switched without an authorization.
    FamilySwitchedWithoutAuthorization { family_id: String },
    /// A required family is missing from the per-family plan.
    FamilyPlanDropped { family_id: String },
    /// A family appears in the plan that the verdict does not carry.
    FamilyPlanUnexpected { family_id: String },
    /// A family's blocking-gate list disagrees with the verdict.
    FamilyPlanBlockingDrift { family_id: String },
    /// A family is `in_plan` while the authorization refuses.
    InPlanWhileRefused { family_id: String },
    /// The body-axis residual was renamed or dropped.
    BodyResidualRenamed { found: String },
    /// A lock the receipt never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The human release record is not the still-empty pending record.
    HumanReleaseRecordDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt derived authorization from a GSD lifecycle state.
    LifecycleAuthorization,
    /// The frozen M513 release package is no longer `pending_human_review`.
    M513PackageStatusDrift { found: String },
    /// The next required action drifted from the contract.
    NextActionDrift { field: String, found: String },
    /// The T01 package pin disagrees with the bound acceptance-package carrier.
    AuthorizationPackageDrift { expected: String, found: String },
    /// A carrier binding is malformed (not repo-relative, no pointer, no sha256).
    UnboundCarrier { detail: String },
    /// A carrier binding appears twice with a different digest.
    DuplicateCarrier { manifest: String, pointer: String },
    /// The evidence list disagrees with the re-derived carriers.
    EvidenceDrift { detail: String },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
}

impl fmt::Display for RetirementReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RetirementReadinessError {}

// ───────────────────────────── types ─────────────────────────────

/// The human-gate invariance the receipt asserts: the release record is still
/// empty and both release packages still await the human decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanGateInvariance {
    /// Whether a reviewer is named; must be `false` in this cut.
    pub reviewer_named: bool,
    /// The release decision; must be `pending` in this cut.
    pub decision: String,
    /// The form a recorded decision must resolve to.
    pub decision_record: String,
    /// The manifest release-package status; must be `pending_human_review`.
    pub package_status: String,
    /// The frozen M513 release-package status; must be `pending_human_review`.
    pub m513_package_status: String,
    /// Whether a release decision is recorded; must be `false` in this cut.
    pub release_recorded: bool,
}

/// The body-axis residual that must survive verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyResidualBinding {
    /// The residual token; always `p1_localization_uncertain`.
    pub token: String,
    /// The body-axis gate id the residual belongs to.
    pub gate_id: String,
    /// The carrier that evidences the residual.
    pub evidence: CarrierBinding,
}

/// The exact action still required before any family may switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NextRequiredAction {
    /// Always `named_human_release_decision_required`.
    pub human_release_decision: String,
    /// The form a recorded decision must use.
    pub decision_record: String,
    /// Always `java_disabled_smoke_required_after_recorded_decision`.
    pub java_disabled_smoke: String,
    /// The receipt the authorized smoke will produce.
    pub java_disabled_smoke_receipt: String,
    /// The ordered human-readable steps.
    pub steps: Vec<String>,
}

/// One required family's blocked plan cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyPlanRow {
    /// The required family id.
    pub family_id: String,
    /// The required, non-clear gate ids that block this family.
    pub blocking_gates: Vec<String>,
    /// Whether the family is in the switch plan. Always `false` while refused.
    pub in_plan: bool,
    /// Whether the family was switched. Always `false` here.
    pub switched: bool,
}

/// The emitted readiness receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementReadinessReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Always `blocked_pending_human_authorization`.
    pub claim_scope: String,
    /// Always `none`.
    pub product_coverage_effect: String,
    /// Always `true`: the retirement goal is unmet.
    pub goal_unmet: bool,
    /// Always empty: no family may switch without an authorization.
    pub families_switched: Vec<String>,
    /// The five locks as declared.
    pub locks: BTreeMap<String, String>,
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The human-gate invariance.
    pub human_gate: HumanGateInvariance,
    /// The verbatim body-axis residual.
    pub body_residual: BodyResidualBinding,
    /// The exact action still required.
    pub next_required_action: NextRequiredAction,
    /// The per-family blocked plan.
    pub family_plan: Vec<FamilyPlanRow>,
    /// The T01 authorization verdict, carried verbatim.
    pub authorization: NativeRetirementAuthorization,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl RetirementReadinessReceipt {
    /// Parse a receipt from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, RetirementReadinessError> {
        serde_json::from_slice(bytes).map_err(|error| RetirementReadinessError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, RetirementReadinessError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            RetirementReadinessError::Malformed {
                detail: error.to_string(),
            }
        })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| RetirementReadinessError::Malformed {
                detail: error.to_string(),
            })?;
        assert_retirement_readiness_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized receipt.
pub fn assert_retirement_readiness_redacted(json: &str) -> Result<(), RetirementReadinessError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(RetirementReadinessError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// The builder input.
#[derive(Debug, Clone)]
pub struct RetirementReadinessInput {
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The T01 authorization verdict.
    pub authorization: NativeRetirementAuthorization,
    /// The human-gate invariance observed from the committed carriers.
    pub human_gate: HumanGateInvariance,
    /// The body-axis residual bound from the S08 package.
    pub body_residual: BodyResidualBinding,
    /// The action still required.
    pub next_required_action: NextRequiredAction,
}

/// The re-derived expectation the validator recomputes against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetirementReadinessExpectation {
    /// The carrier bindings re-derived from the committed bytes.
    pub evidence: Vec<CarrierBinding>,
    /// The required family ids the contract declares.
    pub required_family_ids: Vec<String>,
    /// The body residual token that must survive verbatim.
    pub body_residual_token: String,
}

// ───────────────────────────── helpers ─────────────────────────────

fn check_carrier(binding: &CarrierBinding) -> Result<(), RetirementReadinessError> {
    if !is_repo_relative_path(&binding.manifest)
        || binding.pointer.trim().is_empty()
        || !is_sha256_digest(&binding.revision_sha256)
    {
        return Err(RetirementReadinessError::UnboundCarrier {
            detail: format!(
                "{}#{} ({})",
                binding.manifest, binding.pointer, binding.revision_sha256
            ),
        });
    }
    Ok(())
}

fn evidence_contains(evidence: &[CarrierBinding], binding: &CarrierBinding) -> bool {
    evidence.iter().any(|candidate| {
        candidate.manifest == binding.manifest
            && candidate.pointer == binding.pointer
            && candidate.revision_sha256 == binding.revision_sha256
    })
}

fn check_evidence(evidence: &[CarrierBinding]) -> Result<(), RetirementReadinessError> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for binding in evidence {
        check_carrier(binding)?;
        let key = (binding.manifest.clone(), binding.pointer.clone());
        if !seen.insert(key.clone()) {
            return Err(RetirementReadinessError::DuplicateCarrier {
                manifest: key.0,
                pointer: key.1,
            });
        }
    }
    Ok(())
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), RetirementReadinessError> {
    for (key, expected) in REQUIRED_LOCKS {
        match locks.get(key) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(RetirementReadinessError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(RetirementReadinessError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_human_gate(
    human: &HumanGateInvariance,
    authorization: &NativeRetirementAuthorization,
) -> Result<(), RetirementReadinessError> {
    let drift = |field: &str, expected: &str, found: &str| {
        RetirementReadinessError::HumanReleaseRecordDrift {
            field: field.to_owned(),
            expected: expected.to_owned(),
            found: found.to_owned(),
        }
    };
    if human.reviewer_named {
        return Err(drift("reviewer_named", "false", "true"));
    }
    if human.decision != READINESS_HUMAN_DECISION_PENDING {
        return Err(drift(
            "decision",
            READINESS_HUMAN_DECISION_PENDING,
            &human.decision,
        ));
    }
    if human.decision_record.trim().is_empty() {
        return Err(drift("decision_record", "<non-empty>", "<empty>"));
    }
    if human.release_recorded {
        return Err(drift("release_recorded", "false", "true"));
    }
    if human.package_status != READINESS_PACKAGE_STATUS_PENDING {
        return Err(drift(
            "package_status",
            READINESS_PACKAGE_STATUS_PENDING,
            &human.package_status,
        ));
    }
    if human.m513_package_status != READINESS_M513_PACKAGE_STATUS_PENDING {
        return Err(RetirementReadinessError::M513PackageStatusDrift {
            found: human.m513_package_status.clone(),
        });
    }
    if human.reviewer_named != authorization.human_reviewer_named {
        return Err(drift(
            "reviewer_named",
            &authorization.human_reviewer_named.to_string(),
            &human.reviewer_named.to_string(),
        ));
    }
    if human.release_recorded != authorization.human_decision_recorded {
        return Err(drift(
            "release_recorded",
            &authorization.human_decision_recorded.to_string(),
            &human.release_recorded.to_string(),
        ));
    }
    Ok(())
}

fn check_next_action(action: &NextRequiredAction) -> Result<(), RetirementReadinessError> {
    let drift = |field: &str, found: &str| RetirementReadinessError::NextActionDrift {
        field: field.to_owned(),
        found: found.to_owned(),
    };
    if action.human_release_decision != READINESS_HUMAN_DECISION_ACTION {
        return Err(drift(
            "human_release_decision",
            &action.human_release_decision,
        ));
    }
    if action.decision_record.trim().is_empty() {
        return Err(drift("decision_record", "<empty>"));
    }
    if action.java_disabled_smoke != READINESS_JAVA_DISABLED_SMOKE_ACTION {
        return Err(drift("java_disabled_smoke", &action.java_disabled_smoke));
    }
    if action.java_disabled_smoke_receipt != READINESS_JAVA_DISABLED_SMOKE_REL {
        return Err(drift(
            "java_disabled_smoke_receipt",
            &action.java_disabled_smoke_receipt,
        ));
    }
    if action.steps.is_empty() {
        return Err(drift("steps", "<empty>"));
    }
    Ok(())
}

fn check_body_residual(
    body: &BodyResidualBinding,
    evidence: &[CarrierBinding],
    expected_token: &str,
) -> Result<(), RetirementReadinessError> {
    if body.token != READINESS_BODY_RESIDUAL_TOKEN || body.token != expected_token {
        return Err(RetirementReadinessError::BodyResidualRenamed {
            found: body.token.clone(),
        });
    }
    if body.gate_id != NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID {
        return Err(RetirementReadinessError::BodyResidualRenamed {
            found: body.gate_id.clone(),
        });
    }
    if !evidence_contains(evidence, &body.evidence) {
        return Err(RetirementReadinessError::UnboundCarrier {
            detail: format!(
                "body residual evidence `{}#{}` is not bound in the receipt evidence list",
                body.evidence.manifest, body.evidence.pointer
            ),
        });
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

// ───────────────────────────── builder ─────────────────────────────

/// Build the blocked readiness receipt, refusing every contract violation by
/// name. The receipt only exists while the T01 authorization refuses: an
/// authorized verdict is rejected rather than recorded as blocked.
pub fn build_retirement_readiness_receipt(
    input: &RetirementReadinessInput,
) -> Result<RetirementReadinessReceipt, RetirementReadinessError> {
    check_evidence(&input.evidence)?;
    check_locks(&input.authorization.locks)?;

    if input.authorization.s09_authorized {
        return Err(RetirementReadinessError::AuthorizationNotRefused {
            verdict: input.authorization.verdict.clone(),
        });
    }
    if input.authorization.lifecycle_state_authorizes {
        return Err(RetirementReadinessError::LifecycleAuthorization);
    }

    check_human_gate(&input.human_gate, &input.authorization)?;
    check_body_residual(
        &input.body_residual,
        &input.evidence,
        READINESS_BODY_RESIDUAL_TOKEN,
    )?;
    check_next_action(&input.next_required_action)?;
    if input.next_required_action.decision_record != input.human_gate.decision_record {
        return Err(RetirementReadinessError::NextActionDrift {
            field: "decision_record".to_owned(),
            found: input.next_required_action.decision_record.clone(),
        });
    }

    let family_plan: Vec<FamilyPlanRow> = input
        .authorization
        .families
        .iter()
        .map(|family| FamilyPlanRow {
            family_id: family.family_id.clone(),
            blocking_gates: family.blocking_gates.clone(),
            // A family is in the plan only once the verdict authorizes it; while
            // refused every family stays out.
            in_plan: family.authorized,
            switched: false,
        })
        .collect();

    Ok(RetirementReadinessReceipt {
        schema_version: NATIVE_RETIREMENT_READINESS_SCHEMA_VERSION.to_owned(),
        claim_scope: READINESS_CLAIM_SCOPE_BLOCKED.to_owned(),
        product_coverage_effect: READINESS_PRODUCT_COVERAGE_EFFECT.to_owned(),
        goal_unmet: true,
        families_switched: Vec::new(),
        locks: input.authorization.locks.clone(),
        evidence: input.evidence.clone(),
        human_gate: input.human_gate.clone(),
        body_residual: input.body_residual.clone(),
        next_required_action: input.next_required_action.clone(),
        family_plan,
        authorization: input.authorization.clone(),
        redaction: Redaction::clean(),
    })
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed receipt and refuse what a builder cannot
/// express: a drifted evidence pin, a dropped or switched family, a goal marked
/// met, a widened claim, a renamed residual, an opened lock, a claimed human
/// decision, or a product-coverage claim.
pub fn validate_retirement_readiness_receipt(
    receipt: &RetirementReadinessReceipt,
    expectation: &RetirementReadinessExpectation,
) -> Result<(), RetirementReadinessError> {
    if receipt.schema_version != NATIVE_RETIREMENT_READINESS_SCHEMA_VERSION {
        return Err(RetirementReadinessError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_RETIREMENT_READINESS_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != READINESS_CLAIM_SCOPE_BLOCKED {
        return Err(RetirementReadinessError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != READINESS_PRODUCT_COVERAGE_EFFECT {
        return Err(RetirementReadinessError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if !receipt.goal_unmet {
        return Err(RetirementReadinessError::GoalClaimedMet);
    }
    if let Some(family_id) = receipt.families_switched.first() {
        return Err(
            RetirementReadinessError::FamilySwitchedWithoutAuthorization {
                family_id: family_id.clone(),
            },
        );
    }
    check_locks(&receipt.locks)?;
    check_evidence(&receipt.evidence)?;

    // The evidence set must equal the re-derived one.
    if sorted_bindings(expectation.evidence.clone()) != sorted_bindings(receipt.evidence.clone()) {
        return Err(RetirementReadinessError::EvidenceDrift {
            detail: format!(
                "receipt binds {} carriers; the committed bytes re-derive {}",
                receipt.evidence.len(),
                expectation.evidence.len()
            ),
        });
    }

    if receipt.authorization.s09_authorized {
        return Err(RetirementReadinessError::AuthorizationNotRefused {
            verdict: receipt.authorization.verdict.clone(),
        });
    }
    if receipt.authorization.lifecycle_state_authorizes {
        return Err(RetirementReadinessError::LifecycleAuthorization);
    }

    check_human_gate(&receipt.human_gate, &receipt.authorization)?;
    check_body_residual(
        &receipt.body_residual,
        &receipt.evidence,
        &expectation.body_residual_token,
    )?;
    check_next_action(&receipt.next_required_action)?;
    if receipt.next_required_action.decision_record != receipt.human_gate.decision_record {
        return Err(RetirementReadinessError::NextActionDrift {
            field: "decision_record".to_owned(),
            found: receipt.next_required_action.decision_record.clone(),
        });
    }

    // The T01 package pin must equal the bound acceptance-package carrier.
    if let Some(carrier) = receipt
        .evidence
        .iter()
        .find(|binding| binding.manifest == READINESS_ACCEPTANCE_PACKAGE_REL)
    {
        let found = receipt
            .authorization
            .package
            .actual_sha256
            .clone()
            .unwrap_or_else(|| "<absent>".to_owned());
        if found != carrier.revision_sha256 {
            return Err(RetirementReadinessError::AuthorizationPackageDrift {
                expected: carrier.revision_sha256.clone(),
                found,
            });
        }
    }

    // The per-family plan must cover exactly the required families and agree
    // with the carried verdict.
    let mut plan_ids: BTreeSet<String> = BTreeSet::new();
    for row in &receipt.family_plan {
        if !plan_ids.insert(row.family_id.clone()) {
            return Err(RetirementReadinessError::FamilyPlanUnexpected {
                family_id: row.family_id.clone(),
            });
        }
        if row.switched {
            return Err(
                RetirementReadinessError::FamilySwitchedWithoutAuthorization {
                    family_id: row.family_id.clone(),
                },
            );
        }
        if row.in_plan {
            return Err(RetirementReadinessError::InPlanWhileRefused {
                family_id: row.family_id.clone(),
            });
        }
        match receipt
            .authorization
            .families
            .iter()
            .find(|family| family.family_id == row.family_id)
        {
            Some(authoritative) => {
                if authoritative.blocking_gates != row.blocking_gates {
                    return Err(RetirementReadinessError::FamilyPlanBlockingDrift {
                        family_id: row.family_id.clone(),
                    });
                }
            }
            None => {
                return Err(RetirementReadinessError::FamilyPlanUnexpected {
                    family_id: row.family_id.clone(),
                });
            }
        }
    }
    for family_id in &expectation.required_family_ids {
        if !plan_ids.contains(family_id) {
            return Err(RetirementReadinessError::FamilyPlanDropped {
                family_id: family_id.clone(),
            });
        }
    }

    if receipt.redaction != Redaction::clean() {
        return Err(RetirementReadinessError::RedactionLeak {
            marker: "<redaction flag set>".to_owned(),
        });
    }
    let json =
        serde_json::to_string(receipt).map_err(|error| RetirementReadinessError::Malformed {
            detail: error.to_string(),
        })?;
    assert_retirement_readiness_redacted(&json)?;

    Ok(())
}
