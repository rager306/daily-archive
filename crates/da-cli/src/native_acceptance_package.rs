//! S08/T05 (M514-1lss2s) — GROBID retirement acceptance package.
//!
//! The S08 slice produces four independent, corpus-bound evidence carriers: the
//! T01 native measurement over the frozen 610-document holdout, the T02
//! frozen-threshold evaluation with its typed mandatory-gate ledger, the T03
//! same-n resource receipt and the T04 real-corpus rollback rehearsal. This
//! module assembles them into one acceptance package: it binds each evidence
//! carrier **by repository-relative path + pointer + sha256**, carries the T02
//! evaluation verbatim, and enumerates **every** mandatory gate with a typed
//! state — the three frozen thresholds, the body-axis `not_assessable` residual
//! (the name `p1_localization_uncertain` is preserved, never renamed), the
//! twelve `threshold_pending` required families, the named gaps
//! (`named_gap_learning_ruvllm_ablation` / D471, the H20 and H21 remainders), the
//! S07 `measured_no_gain` disposition and the container-wide Java CPU absence.
//!
//! ## `s09_authorized` is computed, never declared
//!
//! A gate blocks S09 when it is `required` and its state is neither `met` nor an
//! explicitly `accepted_residual`. The package declares the exact blocking set
//! (gate id, state, the evidence needed to clear it) and sets
//! `s09_authorized = blocking_gates.is_empty()`. A package that declares
//! `s09_authorized = true` while a blocking gate exists is refused, and so is a
//! blocking set that disagrees with the gate ledger.
//!
//! ## Authorization is separate from the product and from the harness
//!
//! `gsd_lifecycle_authorizes` must be `false`: a terminal GSD state is harness
//! state and never authorizes S09. The release decision remains a named human
//! act (`basis = human_release_decision_required`), so prose and lifecycle
//! completion cannot stand in for it. The five locks stay closed, the claim is
//! `diagnostic_only`, the product-coverage effect is `none` and thresholds are
//! `not_accepted`.
//!
//! The module is pure `std`/`serde`/`sha2` over `da-parse` value types: it
//! performs no filesystem, network, clock or evaluator access. The gated example
//! reads the committed carriers, invokes the T02 evaluator and writes the
//! artifact.

use crate::native_family_receipt::{
    CarrierBinding, LEAK_MARKERS, REQUIRED_LOCKS, Redaction, is_repo_relative_path,
    is_sha256_digest,
};
use da_parse::{
    MandatoryGate, NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID,
    NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION, ThresholdVerdict,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted acceptance package artifact.
pub const NATIVE_ACCEPTANCE_PACKAGE_SCHEMA_VERSION: &str =
    "grobid-retirement-acceptance-package.v1";
/// Default artifact path of the acceptance package.
pub const DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL: &str =
    "artifacts/parse_compat/grobid_retirement_acceptance_package.json";

/// The only claim scope this diagnostic surface may carry.
pub const ACCEPTANCE_PACKAGE_CLAIM_SCOPE_DIAGNOSTIC_ONLY: &str = "diagnostic_only";
/// The only product-coverage effect this diagnostic surface may carry.
pub const ACCEPTANCE_PACKAGE_PRODUCT_COVERAGE_EFFECT_NONE: &str = "none";
/// Thresholds are recorded, never accepted, by this package.
pub const ACCEPTANCE_PACKAGE_THRESHOLDS_NOT_ACCEPTED: &str = "not_accepted";

// ── evidence carriers ───────────────────────────────────────────────────────

/// The T01 corpus-bound native measurement carrier.
pub const PACKAGE_NATIVE_MEASUREMENT_REL: &str =
    "artifacts/parse_compat/native_corpus_acceptance.json";
/// The T03 corpus-bound same-n resource receipt carrier.
pub const PACKAGE_RESOURCE_RECEIPT_REL: &str =
    "artifacts/parse_compat/native_corpus_resource_same_n.json";
/// The T04 real-corpus rollback rehearsal carrier.
pub const PACKAGE_ROLLBACK_RECEIPT_REL: &str =
    "artifacts/parse_compat/native_corpus_rollback_rehearsal.json";
/// The frozen prospective-regression-floor pins carrier.
pub const PACKAGE_REFERENCE_PINS_REL: &str =
    "artifacts/parse_compat/absorption_progress_reference_pins.json";
/// The executed reference run the pins derive from.
pub const PACKAGE_REFERENCE_PROBE_REL: &str =
    "artifacts/parse_compat/external_holdout_release_probe.json";
/// The closed absorption-progress schema carrier.
pub const PACKAGE_SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";
/// The T02 evaluator source the package binds its verdicts to.
pub const PACKAGE_EVALUATOR_MODULE_REL: &str = "crates/da-parse/src/native_acceptance.rs";
/// The frozen M513 release decision package carrier.
pub const PACKAGE_M513_RELEASE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_release_decision_package.json";
/// The named-gap carrier inside the parity ledger.
pub const PACKAGE_COMPATIBILITY_REL: &str = "data/parse/grobid_compatibility.yaml";
/// The horizon-followup carrier whose stages name H20/H21.
pub const PACKAGE_FOLLOWUP_REL: &str = "doc/review/grobid-absorption-followup.json";
/// The S07 closed learning-cycle carrier.
pub const PACKAGE_LEARNING_CYCLE_REL: &str = "artifacts/parse_learning/candidate_cycle.json";
/// The S05 body-residual carrier.
pub const PACKAGE_BODY_RESIDUAL_REL: &str =
    "artifacts/parse_compat/native_body_uncertainty_split.json";
/// The container-wide Java batch-budget carrier.
pub const PACKAGE_BUDGET_REL: &str = "artifacts/parse_compat/parse_batch_budget_live.json";

// ── evidence pointers ───────────────────────────────────────────────────────

/// Pointer into the T01 measurement at the per-document rows.
pub const POINTER_MEASUREMENT_DOCUMENTS: &str = "documents";
/// Pointer into the T03 receipt at the reference metric rows.
pub const POINTER_RESOURCE_REFERENCE_METRICS: &str = "reference_metrics";
/// Pointer into the T04 receipt at the corpus claim.
pub const POINTER_ROLLBACK_CORPUS_CLAIM: &str = "corpus_claim";
/// Pointer into the pins at the frozen values.
pub const POINTER_PINS_VALUES: &str = "values";
/// Pointer into the probe at the evaluated population.
pub const POINTER_PROBE_EVALUATED_POPULATION: &str = "evaluated_population";
/// Pointer into the schema at the required-scope families.
pub const POINTER_SCHEMA_REQUIRED_SCOPE: &str = "required_scope.families";
/// Pointer into the schema at the carrier's own requirement ids.
pub const POINTER_SCHEMA_REQUIREMENT_IDS: &str = "requirement_ids";
/// Pointer into the evaluator source at its entry point.
pub const POINTER_EVALUATOR_FN: &str = "evaluate_native_acceptance";
/// Pointer into the M513 package at its remainders.
pub const POINTER_M513_REMAINDERS: &str = "remainders";
/// Pointer into the parity ledger at the named gaps.
pub const POINTER_COMPATIBILITY_NAMED_GAPS: &str = "parity_ledger.named_gaps";
/// Pointer into the followup at its horizon stages.
pub const POINTER_FOLLOWUP_STAGES: &str = "stages";
/// Pointer into the learning cycle at its outcome.
pub const POINTER_LEARNING_OUTCOME: &str = "outcome";
/// Pointer into the body-residual carrier at its aggregate.
pub const POINTER_BODY_RESIDUAL_AGGREGATE: &str = "aggregate";
/// Pointer into the batch-budget carrier at its executed population.
pub const POINTER_BUDGET_EXECUTED_POPULATION: &str = "executed_population";

// ── closed vocabularies ─────────────────────────────────────────────────────

/// The closed gate-kind vocabulary.
pub const GATE_KIND_VOCABULARY: [&str; 7] = [
    "threshold",
    "body_axis",
    "threshold_pending_family",
    "named_gap",
    "learning_disposition",
    "resource_absence",
    "human_release",
];
/// The closed gate-state vocabulary. It is the T02 gate-state vocabulary plus
/// the typed absence states the named gaps, the learning disposition and the
/// resource comparison contribute.
pub const GATE_STATE_VOCABULARY: [&str; 9] = [
    "met",
    "unmet",
    "not_comparable",
    "threshold_pending",
    "not_assessable",
    "not_observed",
    "typed_refusal",
    "pending",
    "accepted_residual",
];
/// The states under which a required gate does **not** block S09.
pub const GATE_NON_BLOCKING_STATES: [&str; 2] = ["met", "accepted_residual"];

/// The frozen-threshold policy state the T02 evaluation must carry.
pub const EVALUATION_THRESHOLD_POLICY_FROZEN: &str = "frozen";

/// The named-gap remainder for the ruVLLM ablation (D471).
pub const NAMED_GAP_LEARNING_RUVLLM_ABLATION: &str = "named_gap_learning_ruvllm_ablation";
/// The frozen-external-holdout decision remainder.
pub const NAMED_GAP_H20: &str = "h20_frozen_external_holdout_decision";
/// The revision-label residual remainder.
pub const NAMED_GAP_H21: &str = "h21_revision_label_residual";
/// The S07 measured-no-gain disposition gate id.
pub const LEARNING_DISPOSITION_GATE_ID: &str = "learning.s07_measured_no_gain";
/// The container-wide Java CPU absence gate id.
pub const JAVA_CPU_GATE_ID: &str = "resource.container_java_cpu";
/// The body-residual token that must survive verbatim.
pub const BODY_RESIDUAL_TOKEN: &str = "p1_localization_uncertain";

/// The authorization basis the package must declare.
pub const AUTHORIZATION_BASIS_HUMAN_RELEASE_REQUIRED: &str = "human_release_decision_required";
/// The M513 remainder state that does not block S09.
pub const M513_REMAINDER_ACCEPTED_RESIDUAL: &str = "accepted_residual";
/// The DB-linkage status the attribution block must record.
pub const DB_LINKAGE_STATUS_ARTIFACT_INTERNAL_ONLY: &str = "artifact_internal_only";

/// The S08 planning surface names these three requirements.
pub const PACKAGE_DECLARED_REQUIREMENT_IDS: [&str; 3] = ["R140", "R141", "R142"];
/// The GSD DB canonical store holds these five for the milestone.
pub const PACKAGE_DB_CANONICAL_REQUIREMENT_IDS: [&str; 5] =
    ["R133", "R134", "R140", "R141", "R142"];
/// These requirements are touched through the shipped carriers' bindings.
pub const PACKAGE_BINDING_REQUIREMENT_IDS: [&str; 3] = ["R139", "R123", "R124"];

/// The evidence carriers the package binds, as `(repository-relative path,
/// pointer)`. Both the producing example and the contract test derive their
/// sha256 from this one list, so the kept set cannot drift.
pub const PACKAGE_EVIDENCE_CARRIERS: [(&str, &str); 14] = [
    (
        PACKAGE_NATIVE_MEASUREMENT_REL,
        POINTER_MEASUREMENT_DOCUMENTS,
    ),
    (
        PACKAGE_RESOURCE_RECEIPT_REL,
        POINTER_RESOURCE_REFERENCE_METRICS,
    ),
    (PACKAGE_ROLLBACK_RECEIPT_REL, POINTER_ROLLBACK_CORPUS_CLAIM),
    (PACKAGE_REFERENCE_PINS_REL, POINTER_PINS_VALUES),
    (
        PACKAGE_REFERENCE_PROBE_REL,
        POINTER_PROBE_EVALUATED_POPULATION,
    ),
    (PACKAGE_SCHEMA_REL, POINTER_SCHEMA_REQUIRED_SCOPE),
    (PACKAGE_SCHEMA_REL, POINTER_SCHEMA_REQUIREMENT_IDS),
    (PACKAGE_EVALUATOR_MODULE_REL, POINTER_EVALUATOR_FN),
    (PACKAGE_M513_RELEASE_PACKAGE_REL, POINTER_M513_REMAINDERS),
    (PACKAGE_COMPATIBILITY_REL, POINTER_COMPATIBILITY_NAMED_GAPS),
    (PACKAGE_FOLLOWUP_REL, POINTER_FOLLOWUP_STAGES),
    (PACKAGE_LEARNING_CYCLE_REL, POINTER_LEARNING_OUTCOME),
    (PACKAGE_BODY_RESIDUAL_REL, POINTER_BODY_RESIDUAL_AGGREGATE),
    (PACKAGE_BUDGET_REL, POINTER_BUDGET_EXECUTED_POPULATION),
];

// ───────────────────────────── errors ─────────────────────────────

/// Every refusal is a named variant so a mutation is rejected *by name*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptancePackageError {
    /// The artifact could not be serialized or parsed.
    Malformed { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The package tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The package declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The package claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// A lock the package never relaxes was opened or is absent.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The T02 evaluation was not frozen.
    ThresholdPolicyNotFrozen { found: String },
    /// The evaluation names a different candidate carrier than the package binds.
    CandidateSourceDrift { expected: String, found: String },
    /// The evaluation names a different frozen reference than the package binds.
    ReferencePathDrift { expected: String, found: String },
    /// A carrier binding is malformed (not repo-relative, no pointer, no sha256).
    UnboundCarrier { detail: String },
    /// A carrier binding appears twice with a different digest.
    DuplicateCarrier { manifest: String, pointer: String },
    /// A gate carries no usable evidence.
    GateWithoutEvidence { gate_id: String },
    /// A gate id appears twice.
    DuplicateGate { gate_id: String },
    /// A gate kind is outside the closed vocabulary.
    UnknownGateKind { kind: String },
    /// A gate state is outside the closed vocabulary.
    UnknownGateState { state: String },
    /// The body-axis gate is absent.
    BodyAxisGateMissing { found: usize },
    /// The body-axis residual was renamed or dropped.
    BodyResidualRenamed { found: String },
    /// The threshold gate count disagrees with the evaluation.
    ThresholdCountDrift { expected: u64, found: u64 },
    /// The pending-family gate count disagrees with the expectation.
    PendingFamilyCountDrift { expected: u64, found: u64 },
    /// The declared blocking set disagrees with the gate ledger.
    BlockingSetDrift { detail: String },
    /// The package authorized S09 while a blocking gate exists.
    UnmetGateAuthorized { gate_id: String },
    /// The package refused S09 while no gate blocks.
    FalseAuthorization { detail: String },
    /// The package derived authorization from a GSD lifecycle state.
    GsdLifecycleAuthorization,
    /// The authorization basis drifted.
    AuthorizationBasisDrift { found: String },
    /// A declared requirement is not in the DB-canonical set.
    DeclaredRequirementNotCanonical { requirement: String },
    /// A contract-carried requirement is not in the DB-canonical set.
    ContractRequirementNotCanonical { requirement: String },
    /// The attribution block drifted from the recomputed reconciliation.
    AttributionDrift { detail: String },
    /// The evidence list disagrees with the re-derived carriers.
    EvidenceDrift { detail: String },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
}

impl fmt::Display for AcceptancePackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AcceptancePackageError {}

// ───────────────────────────── types ─────────────────────────────

/// One mandatory gate with the carrier that evidences it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceGate {
    /// Stable gate id.
    pub gate_id: String,
    /// `threshold` | `body_axis` | `threshold_pending_family` | `named_gap` |
    /// `learning_disposition` | `resource_absence` | `human_release`.
    pub kind: String,
    /// Typed state from [`GATE_STATE_VOCABULARY`].
    pub state: String,
    /// Whether the gate is mandatory for S09.
    pub required: bool,
    /// The carrier that evidences this gate.
    pub evidence: CarrierBinding,
    /// The human-readable rationale carried from the source ledger.
    pub detail: String,
    /// The required family, when the gate is a family gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_id: Option<String>,
    /// The frozen metric, when the gate is a threshold gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    /// The typed reason, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What evidence would clear the gate.
    pub evidence_needed: String,
    /// Whether the gate blocks S09 (recomputed, never trusted).
    pub blocks_s09: bool,
}

/// One entry of the declared blocking set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockingGate {
    /// The blocking gate id.
    pub gate_id: String,
    /// The gate state.
    pub state: String,
    /// What evidence would clear the gate.
    pub evidence_needed: String,
}

/// The T02 evaluation carried verbatim (summary + threshold verdicts).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBinding {
    /// The evaluator document schema version.
    pub schema_version: String,
    /// The frozen-threshold policy state the evaluator read.
    pub threshold_policy_state: String,
    /// The candidate carrier the evaluator measured.
    pub candidate_source_path: String,
    /// The frozen reference the evaluator resolved thresholds against.
    pub frozen_reference_path: String,
    /// The per-threshold verdicts, in contract order.
    pub thresholds: Vec<ThresholdVerdict>,
}

/// The attribution reconciliation between the declared set, the DB-canonical
/// set and the requirement ids bound through the shipped carriers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributionReconciliation {
    /// The requirement ids the S08 planning surface declares.
    pub declared_requirement_ids: Vec<String>,
    /// The requirement ids the GSD DB holds for the milestone.
    pub db_canonical_requirement_ids: Vec<String>,
    /// The requirement ids touched through the shipped carriers' bindings.
    pub binding_requirement_ids: Vec<String>,
    /// The requirement ids the closed schema carrier itself declares.
    pub contract_requirement_ids: Vec<String>,
    /// `declared \ db` — must be empty.
    pub declared_not_in_db: Vec<String>,
    /// `db \ declared` — the recorded attribution gap.
    pub db_only: Vec<String>,
    /// `binding \ declared` — the recorded binding-only set.
    pub binding_only: Vec<String>,
    /// How the DB linkage stands; always `artifact_internal_only` here.
    pub db_linkage_status: String,
    /// A one-line note naming the unresolved DB-level linkage.
    pub note: String,
    /// The carrier that evidences the contract-carried requirement ids.
    pub source: CarrierBinding,
}

/// The authorization separation block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageAuthorization {
    /// Whether S09 may proceed. `true` only when no gate blocks.
    pub s09_authorized: bool,
    /// Always `human_release_decision_required`.
    pub basis: String,
    /// Always `false`: a GSD lifecycle state never authorizes S09.
    pub gsd_lifecycle_authorizes: bool,
    /// Whether a human release decision is recorded.
    pub human_release_recorded: bool,
}

/// The emitted acceptance package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrobidRetirementAcceptancePackage {
    /// Package schema version.
    pub schema_version: String,
    /// `diagnostic_only`.
    pub claim_scope: String,
    /// `none`.
    pub product_coverage_effect: String,
    /// `not_accepted`.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The T02 evaluation.
    pub evaluation: EvaluationBinding,
    /// Every mandatory gate, in ledger order.
    pub mandatory_gates: Vec<AcceptanceGate>,
    /// The declared blocking set; must equal the recomputed set.
    pub blocking_gates: Vec<BlockingGate>,
    /// The attribution reconciliation.
    pub attribution: AttributionReconciliation,
    /// The authorization separation block.
    pub authorization: PackageAuthorization,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl GrobidRetirementAcceptancePackage {
    /// Parse a package from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, AcceptancePackageError> {
        serde_json::from_slice(bytes).map_err(|error| AcceptancePackageError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, AcceptancePackageError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| AcceptancePackageError::Malformed {
                detail: error.to_string(),
            })?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| AcceptancePackageError::Malformed {
                detail: error.to_string(),
            })?;
        assert_acceptance_package_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized package.
pub fn assert_acceptance_package_redacted(json: &str) -> Result<(), AcceptancePackageError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(AcceptancePackageError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// The source carriers the T02 evaluation gates bind to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateSources {
    /// The T01 candidate measurement carrier.
    pub candidate_measurement: CarrierBinding,
    /// The frozen reference pins carrier.
    pub pins: CarrierBinding,
    /// The closed schema carrier.
    pub schema: CarrierBinding,
    /// The S05 body-residual carrier.
    pub body_residual: CarrierBinding,
}

/// The builder input.
#[derive(Debug, Clone)]
pub struct AcceptancePackageInput {
    /// The declared claim scope.
    pub claim_scope: String,
    /// The declared product-coverage effect.
    pub product_coverage_effect: String,
    /// The declared thresholds state.
    pub thresholds: String,
    /// The five locks.
    pub locks: BTreeMap<String, String>,
    /// The T02 evaluation.
    pub evaluation: EvaluationBinding,
    /// The raw T02 mandatory gates.
    pub evaluation_gates: Vec<MandatoryGate>,
    /// The source bindings for the evaluation gates.
    pub gate_sources: GateSources,
    /// The additional named gates (named gaps, learning disposition, resource).
    pub extra_gates: Vec<AcceptanceGate>,
    /// The bound evidence carriers.
    pub evidence: Vec<CarrierBinding>,
    /// The attribution reconciliation.
    pub attribution: AttributionReconciliation,
    /// Whether a human release decision is recorded.
    pub human_release_recorded: bool,
}

/// The re-derived expectation the validator recomputes against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptancePackageExpectation {
    /// The carrier bindings re-derived from the committed bytes.
    pub evidence: Vec<CarrierBinding>,
    /// The number of threshold gates the evaluation must carry.
    pub threshold_gate_count: u64,
    /// The number of `threshold_pending` family gates.
    pub pending_family_gate_count: u64,
    /// The body residual tokens that must survive verbatim.
    pub body_residual: Vec<String>,
    /// The DB-canonical requirement ids the attribution must agree with.
    pub db_canonical_requirement_ids: Vec<String>,
}

// ───────────────────────────── helpers ─────────────────────────────

/// Whether a gate with the given `required` flag and state blocks S09.
pub fn gate_blocks_s09(required: bool, state: &str) -> bool {
    required && !GATE_NON_BLOCKING_STATES.contains(&state)
}

/// The blocking set implied by the gate ledger, in ledger order.
pub fn blocking_gates_of(gates: &[AcceptanceGate]) -> Vec<BlockingGate> {
    gates
        .iter()
        .filter(|gate| gate.blocks_s09)
        .map(|gate| BlockingGate {
            gate_id: gate.gate_id.clone(),
            state: gate.state.clone(),
            evidence_needed: gate.evidence_needed.clone(),
        })
        .collect()
}

fn check_carrier(binding: &CarrierBinding) -> Result<(), AcceptancePackageError> {
    if !is_repo_relative_path(&binding.manifest)
        || binding.pointer.trim().is_empty()
        || !is_sha256_digest(&binding.revision_sha256)
    {
        return Err(AcceptancePackageError::UnboundCarrier {
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

fn check_evidence(evidence: &[CarrierBinding]) -> Result<(), AcceptancePackageError> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for binding in evidence {
        check_carrier(binding)?;
        let key = (binding.manifest.clone(), binding.pointer.clone());
        if !seen.insert(key.clone()) {
            return Err(AcceptancePackageError::DuplicateCarrier {
                manifest: key.0,
                pointer: key.1,
            });
        }
    }
    Ok(())
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), AcceptancePackageError> {
    for (key, expected) in REQUIRED_LOCKS {
        match locks.get(key) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(AcceptancePackageError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(AcceptancePackageError::LockOpened {
                    lock: key.to_owned(),
                    expected: expected.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_claim(scope: &str, effect: &str, thresholds: &str) -> Result<(), AcceptancePackageError> {
    if scope != ACCEPTANCE_PACKAGE_CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(AcceptancePackageError::ClaimScopeDrift {
            found: scope.to_owned(),
        });
    }
    if effect != ACCEPTANCE_PACKAGE_PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(AcceptancePackageError::ProductCoverageClaimed {
            found: effect.to_owned(),
        });
    }
    if thresholds != ACCEPTANCE_PACKAGE_THRESHOLDS_NOT_ACCEPTED {
        return Err(AcceptancePackageError::ThresholdClaimed {
            found: thresholds.to_owned(),
        });
    }
    Ok(())
}

fn body_residual_named(gate: &AcceptanceGate, tokens: &[String]) -> bool {
    let named = gate.reason.clone().unwrap_or_default();
    tokens
        .iter()
        .all(|token| named.split(',').any(|part| part.trim() == token))
}

fn check_gate(
    gate: &AcceptanceGate,
    evidence: &[CarrierBinding],
) -> Result<(), AcceptancePackageError> {
    if gate.gate_id.trim().is_empty() {
        return Err(AcceptancePackageError::GateWithoutEvidence {
            gate_id: "<empty>".to_owned(),
        });
    }
    if !GATE_KIND_VOCABULARY.contains(&gate.kind.as_str()) {
        return Err(AcceptancePackageError::UnknownGateKind {
            kind: gate.kind.clone(),
        });
    }
    if !GATE_STATE_VOCABULARY.contains(&gate.state.as_str()) {
        return Err(AcceptancePackageError::UnknownGateState {
            state: gate.state.clone(),
        });
    }
    if gate.detail.trim().is_empty() || gate.evidence_needed.trim().is_empty() {
        return Err(AcceptancePackageError::GateWithoutEvidence {
            gate_id: gate.gate_id.clone(),
        });
    }
    check_carrier(&gate.evidence).map_err(|_| AcceptancePackageError::GateWithoutEvidence {
        gate_id: gate.gate_id.clone(),
    })?;
    if !evidence_contains(evidence, &gate.evidence) {
        return Err(AcceptancePackageError::UnboundCarrier {
            detail: format!(
                "gate `{}` evidence `{}#{}` is not bound in the package evidence list",
                gate.gate_id, gate.evidence.manifest, gate.evidence.pointer
            ),
        });
    }
    let expected_blocks = gate_blocks_s09(gate.required, &gate.state);
    if gate.blocks_s09 != expected_blocks {
        return Err(AcceptancePackageError::BlockingSetDrift {
            detail: format!(
                "gate `{}` declares blocks_s09={} but state `{}` implies {}",
                gate.gate_id, gate.blocks_s09, gate.state, expected_blocks
            ),
        });
    }
    Ok(())
}

fn check_attribution(
    attribution: &AttributionReconciliation,
    evidence: &[CarrierBinding],
) -> Result<(), AcceptancePackageError> {
    check_carrier(&attribution.source)?;
    if !evidence_contains(evidence, &attribution.source) {
        return Err(AcceptancePackageError::UnboundCarrier {
            detail: format!(
                "attribution source `{}#{}` is not bound in the package evidence list",
                attribution.source.manifest, attribution.source.pointer
            ),
        });
    }
    if attribution.db_linkage_status != DB_LINKAGE_STATUS_ARTIFACT_INTERNAL_ONLY {
        return Err(AcceptancePackageError::AttributionDrift {
            detail: format!(
                "db_linkage_status is `{}`, expected `{}`",
                attribution.db_linkage_status, DB_LINKAGE_STATUS_ARTIFACT_INTERNAL_ONLY
            ),
        });
    }
    for requirement in &attribution.declared_requirement_ids {
        if !attribution
            .db_canonical_requirement_ids
            .contains(requirement)
        {
            return Err(AcceptancePackageError::DeclaredRequirementNotCanonical {
                requirement: requirement.clone(),
            });
        }
    }
    for requirement in &attribution.contract_requirement_ids {
        if !attribution
            .db_canonical_requirement_ids
            .contains(requirement)
        {
            return Err(AcceptancePackageError::ContractRequirementNotCanonical {
                requirement: requirement.clone(),
            });
        }
    }
    let declared_not_in_db: Vec<String> = attribution
        .declared_requirement_ids
        .iter()
        .filter(|requirement| {
            !attribution
                .db_canonical_requirement_ids
                .contains(requirement)
        })
        .cloned()
        .collect();
    if declared_not_in_db != attribution.declared_not_in_db {
        return Err(AcceptancePackageError::AttributionDrift {
            detail: format!(
                "declared_not_in_db declared {:?}, recomputed {:?}",
                attribution.declared_not_in_db, declared_not_in_db
            ),
        });
    }
    let db_only: Vec<String> = attribution
        .db_canonical_requirement_ids
        .iter()
        .filter(|requirement| !attribution.declared_requirement_ids.contains(requirement))
        .cloned()
        .collect();
    if db_only != attribution.db_only {
        return Err(AcceptancePackageError::AttributionDrift {
            detail: format!(
                "db_only declared {:?}, recomputed {:?}",
                attribution.db_only, db_only
            ),
        });
    }
    let binding_only: Vec<String> = attribution
        .binding_requirement_ids
        .iter()
        .filter(|requirement| !attribution.declared_requirement_ids.contains(requirement))
        .cloned()
        .collect();
    if binding_only != attribution.binding_only {
        return Err(AcceptancePackageError::AttributionDrift {
            detail: format!(
                "binding_only declared {:?}, recomputed {:?}",
                attribution.binding_only, binding_only
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

/// Recompute the attribution reconciliation, refusing a declared requirement
/// the DB does not hold and a contract-carried requirement outside the DB set.
pub fn build_attribution(
    declared_requirement_ids: &[String],
    db_canonical_requirement_ids: &[String],
    binding_requirement_ids: &[String],
    contract_requirement_ids: &[String],
    source: CarrierBinding,
) -> Result<AttributionReconciliation, AcceptancePackageError> {
    let declared: Vec<String> = declared_requirement_ids.to_vec();
    let db: Vec<String> = db_canonical_requirement_ids.to_vec();
    let binding: Vec<String> = binding_requirement_ids.to_vec();
    let contract: Vec<String> = contract_requirement_ids.to_vec();
    for requirement in &declared {
        if !db.contains(requirement) {
            return Err(AcceptancePackageError::DeclaredRequirementNotCanonical {
                requirement: requirement.clone(),
            });
        }
    }
    for requirement in &contract {
        if !db.contains(requirement) {
            return Err(AcceptancePackageError::ContractRequirementNotCanonical {
                requirement: requirement.clone(),
            });
        }
    }
    Ok(AttributionReconciliation {
        declared_not_in_db: declared
            .iter()
            .filter(|requirement| !db.contains(requirement))
            .cloned()
            .collect(),
        db_only: db
            .iter()
            .filter(|requirement| !declared.contains(requirement))
            .cloned()
            .collect(),
        binding_only: binding
            .iter()
            .filter(|requirement| !declared.contains(requirement))
            .cloned()
            .collect(),
        declared_requirement_ids: declared,
        db_canonical_requirement_ids: db,
        binding_requirement_ids: binding,
        contract_requirement_ids: contract,
        db_linkage_status: DB_LINKAGE_STATUS_ARTIFACT_INTERNAL_ONLY.to_owned(),
        note: "the reconciliation is artifact-internal evidence of a recorded gap: the GSD DB \
               supporting_slices for R133/R134/R139/R123/R124 do not link M514-1lss2s/S08, so this \
               block records the discrepancy rather than closing it"
            .to_owned(),
        source,
    })
}

// ───────────────────────────── builder ─────────────────────────────

fn gate_from_evaluation_gate(
    gate: &MandatoryGate,
    sources: &GateSources,
) -> Result<AcceptanceGate, AcceptancePackageError> {
    let (source, evidence_needed) = match gate.kind.as_str() {
        "threshold" => (
            sources.candidate_measurement.clone(),
            format!(
                "measure `{}` in the frozen candidate series and resolve threshold `{}` against the pinned reference",
                gate.metric_id.clone().unwrap_or_default(),
                gate.gate_id.trim_start_matches("threshold.")
            ),
        ),
        "body_axis" => (
            sources.body_residual.clone(),
            format!(
                "an independent body-localization oracle that decides `{BODY_RESIDUAL_TOKEN}` on the frozen corpus"
            ),
        ),
        "threshold_pending_family" => (
            sources.schema.clone(),
            format!(
                "provide a measurable frozen-corpus mechanism for family `{}` and a reference-derived threshold",
                gate.family_id.clone().unwrap_or_default()
            ),
        ),
        other => {
            return Err(AcceptancePackageError::UnknownGateKind {
                kind: other.to_owned(),
            });
        }
    };
    Ok(AcceptanceGate {
        gate_id: gate.gate_id.clone(),
        kind: gate.kind.clone(),
        state: gate.state.clone(),
        required: gate.required,
        evidence: source,
        detail: gate.evidence.clone(),
        family_id: gate.family_id.clone(),
        metric_id: gate.metric_id.clone(),
        reason: gate.reason.clone(),
        evidence_needed,
        blocks_s09: gate_blocks_s09(gate.required, &gate.state),
    })
}

/// Build the acceptance package, refusing every contract violation by name.
pub fn build_acceptance_package(
    input: &AcceptancePackageInput,
) -> Result<GrobidRetirementAcceptancePackage, AcceptancePackageError> {
    check_claim(
        &input.claim_scope,
        &input.product_coverage_effect,
        &input.thresholds,
    )?;
    check_locks(&input.locks)?;
    check_evidence(&input.evidence)?;

    if input.evaluation.schema_version != NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION {
        return Err(AcceptancePackageError::SchemaDrift {
            field: "evaluation.schema_version".to_owned(),
            expected: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_owned(),
            found: input.evaluation.schema_version.clone(),
        });
    }
    if input.evaluation.threshold_policy_state != EVALUATION_THRESHOLD_POLICY_FROZEN {
        return Err(AcceptancePackageError::ThresholdPolicyNotFrozen {
            found: input.evaluation.threshold_policy_state.clone(),
        });
    }
    if input.evaluation.candidate_source_path != input.gate_sources.candidate_measurement.manifest {
        return Err(AcceptancePackageError::CandidateSourceDrift {
            expected: input.gate_sources.candidate_measurement.manifest.clone(),
            found: input.evaluation.candidate_source_path.clone(),
        });
    }
    if input.evaluation.frozen_reference_path != input.gate_sources.pins.manifest {
        return Err(AcceptancePackageError::ReferencePathDrift {
            expected: input.gate_sources.pins.manifest.clone(),
            found: input.evaluation.frozen_reference_path.clone(),
        });
    }

    // Exactly one body-axis gate, named verbatim.
    let body_gates: Vec<&MandatoryGate> = input
        .evaluation_gates
        .iter()
        .filter(|gate| gate.kind == "body_axis")
        .collect();
    if body_gates.len() != 1 {
        return Err(AcceptancePackageError::BodyAxisGateMissing {
            found: body_gates.len(),
        });
    }
    let body = body_gates[0];
    if body.gate_id != NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID || body.state != "not_assessable" {
        return Err(AcceptancePackageError::BodyResidualRenamed {
            found: format!("{} state={}", body.gate_id, body.state),
        });
    }
    let body_named = body.reason.clone().unwrap_or_default();
    if !body_named
        .split(',')
        .any(|part| part.trim() == BODY_RESIDUAL_TOKEN)
    {
        return Err(AcceptancePackageError::BodyResidualRenamed { found: body_named });
    }

    let threshold_gates = input
        .evaluation_gates
        .iter()
        .filter(|gate| gate.kind == "threshold")
        .count() as u64;
    if input.evaluation.thresholds.len() as u64 != threshold_gates {
        return Err(AcceptancePackageError::ThresholdCountDrift {
            expected: threshold_gates,
            found: input.evaluation.thresholds.len() as u64,
        });
    }

    let mut gates: Vec<AcceptanceGate> = Vec::new();
    for gate in &input.evaluation_gates {
        gates.push(gate_from_evaluation_gate(gate, &input.gate_sources)?);
    }
    for gate in &input.extra_gates {
        gates.push(gate.clone());
    }

    let mut ids: BTreeSet<String> = BTreeSet::new();
    for gate in &gates {
        check_gate(gate, &input.evidence)?;
        if !ids.insert(gate.gate_id.clone()) {
            return Err(AcceptancePackageError::DuplicateGate {
                gate_id: gate.gate_id.clone(),
            });
        }
    }

    check_attribution(&input.attribution, &input.evidence)?;

    let blocking = blocking_gates_of(&gates);
    let s09_authorized = blocking.is_empty();

    Ok(GrobidRetirementAcceptancePackage {
        schema_version: NATIVE_ACCEPTANCE_PACKAGE_SCHEMA_VERSION.to_owned(),
        claim_scope: input.claim_scope.clone(),
        product_coverage_effect: input.product_coverage_effect.clone(),
        thresholds: input.thresholds.clone(),
        locks: input.locks.clone(),
        evidence: input.evidence.clone(),
        evaluation: input.evaluation.clone(),
        mandatory_gates: gates,
        blocking_gates: blocking,
        attribution: input.attribution.clone(),
        authorization: PackageAuthorization {
            s09_authorized,
            basis: AUTHORIZATION_BASIS_HUMAN_RELEASE_REQUIRED.to_owned(),
            gsd_lifecycle_authorizes: false,
            human_release_recorded: input.human_release_recorded,
        },
        redaction: Redaction::clean(),
    })
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed package and refuse what a builder cannot
/// express: drifted evidence, a renamed residual, a blocking-set drift, or an
/// authorization the gates do not support.
pub fn validate_acceptance_package(
    package: &GrobidRetirementAcceptancePackage,
    expectation: &AcceptancePackageExpectation,
) -> Result<(), AcceptancePackageError> {
    if package.schema_version != NATIVE_ACCEPTANCE_PACKAGE_SCHEMA_VERSION {
        return Err(AcceptancePackageError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_ACCEPTANCE_PACKAGE_SCHEMA_VERSION.to_owned(),
            found: package.schema_version.clone(),
        });
    }
    check_claim(
        &package.claim_scope,
        &package.product_coverage_effect,
        &package.thresholds,
    )?;
    check_locks(&package.locks)?;
    check_evidence(&package.evidence)?;

    if package.evaluation.schema_version != NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION {
        return Err(AcceptancePackageError::SchemaDrift {
            field: "evaluation.schema_version".to_owned(),
            expected: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_owned(),
            found: package.evaluation.schema_version.clone(),
        });
    }
    if package.evaluation.threshold_policy_state != EVALUATION_THRESHOLD_POLICY_FROZEN {
        return Err(AcceptancePackageError::ThresholdPolicyNotFrozen {
            found: package.evaluation.threshold_policy_state.clone(),
        });
    }

    // The evidence set must equal the re-derived one.
    if sorted_bindings(expectation.evidence.clone()) != sorted_bindings(package.evidence.clone()) {
        return Err(AcceptancePackageError::EvidenceDrift {
            detail: format!(
                "package binds {} carriers; the committed bytes re-derive {}",
                package.evidence.len(),
                expectation.evidence.len()
            ),
        });
    }

    let mut ids: BTreeSet<String> = BTreeSet::new();
    for gate in &package.mandatory_gates {
        check_gate(gate, &package.evidence)?;
        if !ids.insert(gate.gate_id.clone()) {
            return Err(AcceptancePackageError::DuplicateGate {
                gate_id: gate.gate_id.clone(),
            });
        }
    }

    let body_gates: Vec<&AcceptanceGate> = package
        .mandatory_gates
        .iter()
        .filter(|gate| gate.kind == "body_axis")
        .collect();
    if body_gates.len() != 1 {
        return Err(AcceptancePackageError::BodyAxisGateMissing {
            found: body_gates.len(),
        });
    }
    let body = body_gates[0];
    if body.gate_id != NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID || body.state != "not_assessable" {
        return Err(AcceptancePackageError::BodyResidualRenamed {
            found: format!("{} state={}", body.gate_id, body.state),
        });
    }
    if !body_residual_named(body, &expectation.body_residual) {
        return Err(AcceptancePackageError::BodyResidualRenamed {
            found: body.reason.clone().unwrap_or_default(),
        });
    }

    let threshold_gates = package
        .mandatory_gates
        .iter()
        .filter(|gate| gate.kind == "threshold")
        .count() as u64;
    if threshold_gates != expectation.threshold_gate_count {
        return Err(AcceptancePackageError::ThresholdCountDrift {
            expected: expectation.threshold_gate_count,
            found: threshold_gates,
        });
    }
    let pending_gates = package
        .mandatory_gates
        .iter()
        .filter(|gate| gate.kind == "threshold_pending_family")
        .count() as u64;
    if pending_gates != expectation.pending_family_gate_count {
        return Err(AcceptancePackageError::PendingFamilyCountDrift {
            expected: expectation.pending_family_gate_count,
            found: pending_gates,
        });
    }

    let recomputed = blocking_gates_of(&package.mandatory_gates);
    if recomputed != package.blocking_gates {
        return Err(AcceptancePackageError::BlockingSetDrift {
            detail: format!(
                "declared {} blocking gates, recomputed {}",
                package.blocking_gates.len(),
                recomputed.len()
            ),
        });
    }

    if package.authorization.gsd_lifecycle_authorizes {
        return Err(AcceptancePackageError::GsdLifecycleAuthorization);
    }
    if package.authorization.basis != AUTHORIZATION_BASIS_HUMAN_RELEASE_REQUIRED {
        return Err(AcceptancePackageError::AuthorizationBasisDrift {
            found: package.authorization.basis.clone(),
        });
    }
    let expected_authorized = package.blocking_gates.is_empty();
    if package.authorization.s09_authorized != expected_authorized {
        if package.authorization.s09_authorized {
            return Err(AcceptancePackageError::UnmetGateAuthorized {
                gate_id: package
                    .blocking_gates
                    .first()
                    .map(|gate| gate.gate_id.clone())
                    .unwrap_or_default(),
            });
        }
        return Err(AcceptancePackageError::FalseAuthorization {
            detail: "s09_authorized is false while no gate blocks".to_owned(),
        });
    }

    // The attribution must agree with the DB-canonical set the caller re-read.
    let mut expected_attribution = package.attribution.clone();
    expected_attribution.db_canonical_requirement_ids =
        expectation.db_canonical_requirement_ids.clone();
    check_attribution(&expected_attribution, &package.evidence)?;

    if package.redaction != Redaction::clean() {
        return Err(AcceptancePackageError::RedactionLeak {
            marker: "<redaction flag set>".to_owned(),
        });
    }
    let re_serialized =
        serde_json::to_vec(package).map_err(|error| AcceptancePackageError::Malformed {
            detail: error.to_string(),
        })?;
    let re_serialized =
        std::str::from_utf8(&re_serialized).map_err(|error| AcceptancePackageError::Malformed {
            detail: error.to_string(),
        })?;
    assert_acceptance_package_redacted(re_serialized)?;

    Ok(())
}
