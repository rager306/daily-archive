//! Fail-closed retirement authorization gate (M514-1lss2s/S09/T01).
//!
//! S08's acceptance package computes `s09_authorized = false` from twenty
//! blocking gates, and the M513 human release record is `reviewer=null` /
//! `decision=pending`. Both statements exist, but nothing turned them into a
//! typed per-family predicate: "may this family be switched by a recorded human
//! authorization, and is every gate of that family clear?". Without such a
//! predicate a later switch would be a declaration rather than a gate, and an
//! operator could read a green test, a closed milestone or a cancellation
//! waiver as permission.
//!
//! This module is that gate, and it is deliberately pure: no filesystem,
//! network, environment, clock, RNG or tracing access (the `kg-*` /
//! `da-domain` style). The caller reads the committed carriers (the S08 gate
//! ledger, the manifest's human release record, the five locks) and hands in
//! the parsed values; the gate only decides. It imports only `crate`, `serde`
//! and `std`, and reuses the da-parse acceptance vocabulary
//! ([`NATIVE_ACCEPTANCE_OPEN_LOCK_VALUES`], [`REFUSAL_OPEN_LOCK`]) instead of
//! restating the tokens.
//!
//! Rules, all fail-closed:
//!
//! * `s09_authorized` is **computed from the refusal set, never declared**.
//! * a reviewer must be named (non-blank); an absent or empty reviewer is
//!   [`REFUSAL_HUMAN_REVIEWER_UNNAMED`];
//! * the decision must be the explicit recorded authorizing token
//!   (`recorded` / `approved`); `pending`, `rejected` and any unknown token are
//!   [`REFUSAL_HUMAN_DECISION_NOT_RECORDED`];
//! * prose never authorizes: `prose_alone_authorizes_release = true` is
//!   [`REFUSAL_PROSE_DOES_NOT_AUTHORIZE`];
//! * a GSD lifecycle state never authorizes:
//!   [`REFUSAL_LIFECYCLE_STATE_AUTHORIZES`];
//! * the package status must be one of the declared closed states and must agree
//!   with the recorded decision, else [`REFUSAL_PACKAGE_STATUS_UNKNOWN`];
//! * all five locks must be present and exactly as declared, else
//!   [`REFUSAL_OPEN_LOCK`];
//! * every `required` gate of a family must be `met`, `accepted_residual` or
//!   `not_applicable` (a recorded downgrade is a decision, not a measurement,
//!   but it does not block);
//!   anything else is [`REFUSAL_FAMILY_GATE_BLOCKS`] (family gate) or
//!   [`REFUSAL_REQUIRED_GATE_BLOCKS`] (a required gate without a family);
//! * the S08 package pin must be declared and current, else
//!   [`REFUSAL_PACKAGE_ABSENT`] / [`REFUSAL_PACKAGE_PIN_STALE`].
//!
//! The module is a gate, not a release: a recorded authorization does not by
//! itself confer product coverage (authorization evidence is never product
//! coverage evidence), so [`NativeRetirementAuthorization::product_coverage_effect`]
//! stays [`NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::native_acceptance::{
    MandatoryGate, NATIVE_ACCEPTANCE_NOT_APPLICABLE_STATE, NATIVE_ACCEPTANCE_OPEN_LOCK_VALUES,
    NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT, NativeAcceptanceEvaluation, REFUSAL_OPEN_LOCK,
};

/// Schema version of the emitted authorization document.
pub const NATIVE_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION: &str =
    "native-retirement-authorization.v1";

/// The retirement may proceed for the cleared families.
pub const RETIREMENT_AUTHORIZATION_AUTHORIZED: &str = "authorized";
/// The retirement is refused; no family may switch.
pub const RETIREMENT_AUTHORIZATION_REFUSED: &str = "refused";

/// The authorization basis while a recorded human release decision is required.
pub const RETIREMENT_BASIS_HUMAN_RELEASE_REQUIRED: &str = "human_release_decision_required";
/// The authorization basis once a recorded human release decision exists and
/// every gate is clear.
pub const RETIREMENT_BASIS_RECORDED_HUMAN_RELEASE: &str = "recorded_human_release_decision";

/// Claim scope while refused. Nothing in this cut may read the refusal as a
/// product claim.
pub const RETIREMENT_CLAIM_SCOPE_BLOCKED: &str = "blocked_pending_human_authorization";
/// Claim scope once the recorded human authorization and every gate agree.
pub const RETIREMENT_CLAIM_SCOPE_AUTHORIZED: &str = "authorized_retirement_release";

/// The five locks that must stay exactly as declared. A missing or drifted lock
/// is an open lock: the gate never relaxes one.
pub const RETIREMENT_REQUIRED_LOCKS: [(&str, &str); 5] = [
    ("deployed", "false"),
    ("import_eligible", "false"),
    ("auto_promote", "forbidden"),
    ("production_route", "forbidden"),
    ("graph_write", "forbidden"),
];

/// Required gate states that do **not** block a family: `met` and
/// `accepted_residual` are measurements, `not_applicable` is a recorded
/// downgrade. Everything else blocks: a state the gate does not recognise
/// cannot be a pass.
pub const RETIREMENT_NON_BLOCKING_GATE_STATES: [&str; 3] = [
    "met",
    "accepted_residual",
    NATIVE_ACCEPTANCE_NOT_APPLICABLE_STATE,
];

/// The closed human release decision vocabulary (mirrors the Governor's
/// `external_holdout_package` rule).
pub const RETIREMENT_HUMAN_DECISION_VOCABULARY: [&str; 4] =
    ["pending", "recorded", "approved", "rejected"];
/// The subset of [`RETIREMENT_HUMAN_DECISION_VOCABULARY`] that records a
/// decision. `pending` and `rejected` are not authorizations.
pub const RETIREMENT_HUMAN_DECISION_RECORDED_TOKENS: [&str; 2] = ["recorded", "approved"];

/// The closed package status vocabulary (the schema's `authorization.states`
/// plus the manifest's release-package status).
pub const RETIREMENT_PACKAGE_STATUS_VOCABULARY: [&str; 3] = [
    "pending_human_review",
    "owner_directive_research",
    "human_release_recorded",
];
/// The package status that agrees with a recorded human release decision.
pub const RETIREMENT_PACKAGE_STATUS_AUTHORIZED: &str = "human_release_recorded";

// ── closed refusal vocabulary ───────────────────────────────────────────────

/// The reviewer is absent or blank; a blank reviewer is not a named human.
pub const REFUSAL_HUMAN_REVIEWER_UNNAMED: &str = "human_reviewer_unnamed";
/// The decision is not the explicit recorded authorizing token (includes
/// `pending`, `rejected` and any unknown token).
pub const REFUSAL_HUMAN_DECISION_NOT_RECORDED: &str = "human_decision_not_recorded";
/// Prose was asserted to authorize the release; it never does.
pub const REFUSAL_PROSE_DOES_NOT_AUTHORIZE: &str = "prose_does_not_authorize";
/// The caller derived authorization from a GSD lifecycle state.
pub const REFUSAL_LIFECYCLE_STATE_AUTHORIZES: &str = "lifecycle_state_authorizes";
/// A required gate that names a family is not a non-blocking state.
pub const REFUSAL_FAMILY_GATE_BLOCKS: &str = "family_gate_blocks";
/// A required gate without a family is not a non-blocking state.
pub const REFUSAL_REQUIRED_GATE_BLOCKS: &str = "required_gate_blocks";
/// The declared package pin does not match the package bytes.
pub const REFUSAL_PACKAGE_PIN_STALE: &str = "package_pin_stale";
/// No package pin was declared at all.
pub const REFUSAL_PACKAGE_ABSENT: &str = "package_absent";
/// The package status is outside the closed vocabulary or disagrees with the
/// recorded decision.
pub const REFUSAL_PACKAGE_STATUS_UNKNOWN: &str = "package_status_unknown";

/// The complete closed refusal vocabulary this gate may emit. A code outside
/// this set would be an untyped refusal, so the contract test pins the set.
pub const RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY: [&str; 10] = [
    REFUSAL_HUMAN_REVIEWER_UNNAMED,
    REFUSAL_HUMAN_DECISION_NOT_RECORDED,
    REFUSAL_PROSE_DOES_NOT_AUTHORIZE,
    REFUSAL_LIFECYCLE_STATE_AUTHORIZES,
    REFUSAL_FAMILY_GATE_BLOCKS,
    REFUSAL_REQUIRED_GATE_BLOCKS,
    REFUSAL_PACKAGE_PIN_STALE,
    REFUSAL_PACKAGE_ABSENT,
    REFUSAL_PACKAGE_STATUS_UNKNOWN,
    REFUSAL_OPEN_LOCK,
];

// ── input value types ───────────────────────────────────────────────────────

/// The recorded human release decision, carried as a value (never inferred from
/// prose, a commit, an ADR, a green test or a lifecycle state).
///
/// Unknown fields are tolerated so a caller may deserialize the carrier block
/// (`release_decision_package.human_release_review`, which also carries
/// `required` and `authorization_requires`) without a second mapping type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanReleaseRecord {
    /// `release_decision_package.human_release_review.reviewer`; `null` until a
    /// real person is named.
    #[serde(default)]
    pub reviewer: Option<String>,
    /// `release_decision_package.human_release_review.decision`.
    pub decision: String,
    /// `release_decision_package.human_release_review.decision_record`; the
    /// form a recorded decision must resolve to.
    #[serde(default)]
    pub decision_record: Option<String>,
    /// `release_decision_package.human_release_review.prose_alone_authorizes_release`.
    pub prose_alone_authorizes_release: bool,
    /// `release_decision_package.package_status`.
    pub package_status: String,
}

impl HumanReleaseRecord {
    /// A named reviewer is a present, non-blank value.
    pub fn reviewer_named(&self) -> bool {
        self.reviewer
            .as_deref()
            .map(str::trim)
            .is_some_and(|reviewer| !reviewer.is_empty())
    }

    /// The decision is one of the explicit recorded authorizing tokens.
    pub fn recorded_decision(&self) -> bool {
        RETIREMENT_HUMAN_DECISION_RECORDED_TOKENS.contains(&self.decision.trim())
    }
}

/// The S08 acceptance package binding: where it is, the digest the caller
/// declares, and the digest the caller actually read from the bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageBinding {
    /// Repository-relative path of the S08 acceptance package.
    pub path: String,
    /// The declared `sha256:` pin, when one exists.
    #[serde(default)]
    pub declared_sha256: Option<String>,
    /// The digest of the bytes the caller read, when it read them.
    #[serde(default)]
    pub actual_sha256: Option<String>,
}

/// One typed refusal. Every refusal names what it refuses; none is a silent
/// downgrade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementRefusal {
    /// One of [`RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY`].
    pub code: String,
    /// Human-readable detail. Never carries payload or path material.
    pub detail: String,
    /// The family the refusal concerns, when the refusal is a family gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_id: Option<String>,
    /// The gate the refusal concerns, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_id: Option<String>,
}

/// One family's authorization cell: the required gates that block it and
/// whether the family may switch now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyAuthorization {
    /// The required family id, in the order the caller supplied it.
    pub family_id: String,
    /// The required, non-clear gate ids that block this family.
    pub blocking_gates: Vec<String>,
    /// `true` only when the overall authorization holds and no gate blocks.
    pub authorized: bool,
}

/// The complete gate input. Everything is supplied: no hidden file read.
#[derive(Debug, Clone)]
pub struct RetirementAuthorizationInput {
    /// The S08 acceptance package binding.
    pub package: PackageBinding,
    /// The recorded human release decision.
    pub human_release: HumanReleaseRecord,
    /// Whether the caller derived authorization from a GSD lifecycle state.
    pub lifecycle_state_authorizes: bool,
    /// The five locks as declared.
    pub locks: BTreeMap<String, String>,
    /// The required families, read by pointer from the contract.
    pub required_families: Vec<String>,
    /// The S08 mandatory-gate ledger.
    pub evaluation: NativeAcceptanceEvaluation,
}

/// The typed authorization document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRetirementAuthorization {
    /// Schema version.
    pub schema_version: String,
    /// `authorized` | `refused`.
    pub verdict: String,
    /// Computed from the refusal set; never supplied.
    pub s09_authorized: bool,
    /// `human_release_decision_required` | `recorded_human_release_decision`.
    pub basis: String,
    /// Whether a named reviewer was recorded.
    pub human_reviewer_named: bool,
    /// Whether an explicit recorded decision exists.
    pub human_decision_recorded: bool,
    /// Whether the caller asserted a lifecycle-state authorization.
    pub lifecycle_state_authorizes: bool,
    /// The package binding the caller supplied.
    pub package: PackageBinding,
    /// The five locks as supplied.
    pub locks: BTreeMap<String, String>,
    /// One cell per required family, in input order.
    pub families: Vec<FamilyAuthorization>,
    /// Every required, non-clear gate id, in ledger order.
    pub blocking_gates: Vec<String>,
    /// The typed refusals, in evaluation order.
    pub refusals: Vec<RetirementRefusal>,
    /// `blocked_pending_human_authorization` while refused.
    pub claim_scope: String,
    /// Always `none`: authorization evidence is not product coverage.
    pub product_coverage_effect: String,
}

/// The gate's verdict document under the plan's short name. Identical to
/// [`NativeRetirementAuthorization`]; both names resolve to one type.
pub type RetirementAuthorization = NativeRetirementAuthorization;

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn lock_is_open(value: &str) -> bool {
    NATIVE_ACCEPTANCE_OPEN_LOCK_VALUES.contains(&value.trim().to_ascii_lowercase().as_str())
}

fn refusal(code: &str, detail: impl Into<String>) -> RetirementRefusal {
    RetirementRefusal {
        code: code.to_string(),
        detail: detail.into(),
        family_id: None,
        gate_id: None,
    }
}

/// Evaluate the fail-closed retirement authorization gate. `s09_authorized` is
/// computed from the refusal set; the caller never supplies a verdict.
pub fn authorize_native_retirement(
    input: &RetirementAuthorizationInput,
) -> NativeRetirementAuthorization {
    let mut refusals: Vec<RetirementRefusal> = Vec::new();

    // ── the S08 package pin: declared, present and current ──────────────────
    let declared = input.package.declared_sha256.as_deref();
    if is_blank(&input.package.path) || declared.is_none_or(is_blank) {
        refusals.push(refusal(
            REFUSAL_PACKAGE_ABSENT,
            format!(
                "no package pin declared for `{}`; an absent pin cannot authorize anything",
                input.package.path
            ),
        ));
    } else if input.package.actual_sha256.as_deref() != declared {
        refusals.push(refusal(
            REFUSAL_PACKAGE_PIN_STALE,
            format!(
                "declared package pin {} does not match the read bytes {}",
                declared.unwrap_or("<none>"),
                input.package.actual_sha256.as_deref().unwrap_or("<unread>")
            ),
        ));
    }

    // ── the human release record ────────────────────────────────────────────
    let reviewer_named = input.human_release.reviewer_named();
    if !reviewer_named {
        refusals.push(refusal(
            REFUSAL_HUMAN_REVIEWER_UNNAMED,
            "the release record names no reviewer; a blank or absent reviewer is not a named human",
        ));
    }
    let decision_recorded = input.human_release.recorded_decision();
    if !decision_recorded {
        refusals.push(refusal(
            REFUSAL_HUMAN_DECISION_NOT_RECORDED,
            format!(
                "decision `{}` is not a recorded authorizing token ({})",
                input.human_release.decision.trim(),
                RETIREMENT_HUMAN_DECISION_RECORDED_TOKENS.join(", ")
            ),
        ));
    }
    if input.human_release.prose_alone_authorizes_release {
        refusals.push(refusal(
            REFUSAL_PROSE_DOES_NOT_AUTHORIZE,
            "prose_alone_authorizes_release is set; prose, an ADR, a commit and a green test never authorize a release",
        ));
    }
    if input.lifecycle_state_authorizes {
        refusals.push(refusal(
            REFUSAL_LIFECYCLE_STATE_AUTHORIZES,
            "the caller derived authorization from a GSD lifecycle state; a terminal harness state is never a human release decision",
        ));
    }

    // ── the package status must be declared and agree with the decision ─────
    let package_status = input.human_release.package_status.trim();
    if !RETIREMENT_PACKAGE_STATUS_VOCABULARY.contains(&package_status) {
        refusals.push(refusal(
            REFUSAL_PACKAGE_STATUS_UNKNOWN,
            format!("package_status `{package_status}` is outside the closed vocabulary"),
        ));
    } else if decision_recorded && package_status != RETIREMENT_PACKAGE_STATUS_AUTHORIZED {
        refusals.push(refusal(
            REFUSAL_PACKAGE_STATUS_UNKNOWN,
            format!(
                "a recorded decision disagrees with package_status `{package_status}`; the package status must be `{RETIREMENT_PACKAGE_STATUS_AUTHORIZED}`"
            ),
        ));
    }

    // ── the five locks must be present and exactly as declared ──────────────
    for (key, expected) in RETIREMENT_REQUIRED_LOCKS {
        match input.locks.get(key) {
            Some(value) if value.trim() == expected => {}
            Some(value) => {
                if lock_is_open(value) {
                    refusals.push(refusal(
                        REFUSAL_OPEN_LOCK,
                        format!(
                            "lock `{key}` is open (`{value}`); every lock must stay `{expected}`"
                        ),
                    ));
                } else {
                    refusals.push(refusal(
                        REFUSAL_OPEN_LOCK,
                        format!(
                            "lock `{key}` is `{value}` but must stay exactly `{expected}`; an unknown value is treated as open"
                        ),
                    ));
                }
            }
            None => refusals.push(refusal(
                REFUSAL_OPEN_LOCK,
                format!("lock `{key}` is missing; a missing lock is not a closed lock"),
            )),
        }
    }

    // ── the S08 mandatory-gate ledger ───────────────────────────────────────
    let mut blocking_gates: Vec<String> = Vec::new();
    for gate in &input.evaluation.gates {
        if !gate.required || gate_clears(gate) {
            continue;
        }
        blocking_gates.push(gate.gate_id.clone());
        refusals.push(gate_refusal(gate));
    }

    // ── per-family cells; a required family with no gate fails closed ───────
    let mut families: Vec<FamilyAuthorization> = Vec::with_capacity(input.required_families.len());
    for family_id in &input.required_families {
        let family_gates: Vec<&MandatoryGate> = input
            .evaluation
            .gates
            .iter()
            .filter(|gate| gate.required && gate.family_id.as_deref() == Some(family_id.as_str()))
            .collect();
        let family_blocking: Vec<String> = family_gates
            .iter()
            .filter(|gate| !gate_clears(gate))
            .map(|gate| gate.gate_id.clone())
            .collect();
        if family_gates.is_empty() {
            refusals.push(RetirementRefusal {
                code: REFUSAL_FAMILY_GATE_BLOCKS.to_string(),
                detail: format!(
                    "required family `{family_id}` carries no required gate in the ledger; a family the ledger does not cover cannot be authorized"
                ),
                family_id: Some(family_id.clone()),
                gate_id: None,
            });
        }
        families.push(FamilyAuthorization {
            family_id: family_id.clone(),
            blocking_gates: family_blocking,
            // Filled after the overall verdict is known.
            authorized: false,
        });
    }

    let s09_authorized = refusals.is_empty();
    if s09_authorized {
        for family in &mut families {
            family.authorized = true;
        }
    }

    NativeRetirementAuthorization {
        schema_version: NATIVE_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION.to_string(),
        verdict: if s09_authorized {
            RETIREMENT_AUTHORIZATION_AUTHORIZED.to_string()
        } else {
            RETIREMENT_AUTHORIZATION_REFUSED.to_string()
        },
        s09_authorized,
        basis: if s09_authorized {
            RETIREMENT_BASIS_RECORDED_HUMAN_RELEASE.to_string()
        } else {
            RETIREMENT_BASIS_HUMAN_RELEASE_REQUIRED.to_string()
        },
        human_reviewer_named: reviewer_named,
        human_decision_recorded: decision_recorded,
        lifecycle_state_authorizes: input.lifecycle_state_authorizes,
        package: input.package.clone(),
        locks: input.locks.clone(),
        families,
        blocking_gates,
        refusals,
        claim_scope: if s09_authorized {
            RETIREMENT_CLAIM_SCOPE_AUTHORIZED.to_string()
        } else {
            RETIREMENT_CLAIM_SCOPE_BLOCKED.to_string()
        },
        product_coverage_effect: NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT.to_string(),
    }
}

fn gate_clears(gate: &MandatoryGate) -> bool {
    RETIREMENT_NON_BLOCKING_GATE_STATES.contains(&gate.state.trim())
}

fn gate_refusal(gate: &MandatoryGate) -> RetirementRefusal {
    let non_blocking = RETIREMENT_NON_BLOCKING_GATE_STATES.join(", ");
    let detail = match &gate.reason {
        Some(reason) if !is_blank(reason) => format!(
            "required gate `{}` is `{}` (`{reason}`); it must be one of the non-blocking states [{non_blocking}]",
            gate.gate_id,
            gate.state.trim()
        ),
        _ => format!(
            "required gate `{}` is `{}`; it must be one of the non-blocking states [{non_blocking}]",
            gate.gate_id,
            gate.state.trim()
        ),
    };
    RetirementRefusal {
        code: if gate.family_id.is_some() {
            REFUSAL_FAMILY_GATE_BLOCKS.to_string()
        } else {
            REFUSAL_REQUIRED_GATE_BLOCKS.to_string()
        },
        detail,
        family_id: gate.family_id.clone(),
        gate_id: Some(gate.gate_id.clone()),
    }
}
