//! Frozen-threshold evaluator and typed mandatory-gate ledger (M514-1lss2s/S08/T02).
//!
//! The frozen regression floor exists as reference-derived values in
//! `artifacts/parse_compat/absorption_progress_reference_pins.json`, frozen by
//! the schema carrier `data/parse/absorption_progress.schema.yaml` and bound to
//! the revision of that carrier. Nothing in the runtime answered "does a
//! candidate pass these on the real corpus" before this module.
//!
//! This module is the answer, and it is deliberately pure: no filesystem,
//! network, environment, clock, RNG or tracing access (the `kg-*` / `da-domain`
//! style). The caller reads the committed carriers and hands in their bytes and
//! parsed documents; the evaluator only decides. That keeps it testable against
//! committed pins without a second source of truth.
//!
//! Three rules make a pass meaningful instead of self-referential:
//!
//! * a threshold value is resolved **by pointer** (`reference.path` +
//!   `reference.pointer`) with the same `score_number` rule the governor domain
//!   uses — a `{numerator, denominator}` fraction is a number only when the
//!   denominator is positive. A raw numeric literal on a threshold is refused
//!   rather than trusted;
//! * a threshold may only resolve against the separately pinned reference
//!   (`threshold_policy.reference_pins.path`). A reference pointing anywhere
//!   else — including the candidate itself — is refused, because comparing a
//!   candidate to its own restated score would let it move its own floor;
//! * a candidate may only be compared inside one series
//!   (`corpus_id`, `profile`, `oracle`, `source_revision`); a different series is
//!   `not_comparable`, never a quiet subtraction.
//!
//! The output is a typed per-threshold verdict (`met` / `unmet` /
//! `not_comparable` / `threshold_pending`) plus a mandatory-gate ledger that
//! names every unmet gate: the three threshold gates, the body-axis
//! `not_assessable` / `p1_localization_uncertain` residual, and every
//! schema-declared `threshold_pending` required family — either as a
//! `threshold_pending_family` gate or, when the contract carries a recorded
//! `required_scope.downgrades` record, as a `required_family_downgrade`
//! `not_applicable` gate naming the recorded decision. The ledger is derived
//! from `required_scope.families` in the supplied contract document, so the
//! family list is read by pointer and never restated in code.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Schema version of the evaluation document this module emits.
pub const NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION: &str = "native-acceptance-evaluation.v1";
/// The only claim scope this diagnostic surface may carry.
pub const NATIVE_ACCEPTANCE_CLAIM_SCOPE: &str = "diagnostic_only";
/// The product-coverage effect of a diagnostic evaluation.
pub const NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT: &str = "none";

/// Per-threshold verdict vocabulary.
pub const NATIVE_ACCEPTANCE_VERDICT_VOCABULARY: [&str; 4] =
    ["met", "unmet", "not_comparable", "threshold_pending"];
/// Mandatory-gate state vocabulary: threshold verdicts, the body-axis residual
/// (`not_assessable` because the frozen local corpus cannot decide it) and the
/// recorded downgrade state (`not_applicable`, a decision rather than a
/// measurement).
pub const NATIVE_ACCEPTANCE_GATE_STATE_VOCABULARY: [&str; 6] = [
    "met",
    "unmet",
    "not_comparable",
    "threshold_pending",
    "not_assessable",
    "not_applicable",
];
/// Direction vocabulary (the schema declares the same two values).
pub const NATIVE_ACCEPTANCE_DIRECTION_VOCABULARY: [&str; 2] = ["at_least", "at_most"];
/// Required-scope family state vocabulary.
pub const NATIVE_ACCEPTANCE_FAMILY_STATE_VOCABULARY: [&str; 2] = ["required", "threshold_pending"];
/// The comparability keys that define one measurement series.
pub const NATIVE_ACCEPTANCE_COMPARABILITY_KEYS: [&str; 4] =
    ["corpus_id", "profile", "oracle", "source_revision"];
/// Lock values that mean the lock is open. Every other value is treated as a
/// closed lock: the evaluator refuses an open one rather than assuming.
pub const NATIVE_ACCEPTANCE_OPEN_LOCK_VALUES: [&str; 7] =
    ["true", "allowed", "enabled", "permitted", "yes", "1", "on"];
/// A required, corpus-measurable family state.
pub const NATIVE_ACCEPTANCE_REQUIRED_STATE: &str = "required";
/// A family state that carries no threshold and cannot pass.
pub const NATIVE_ACCEPTANCE_THRESHOLD_PENDING_STATE: &str = "threshold_pending";
/// The gate state a recorded required-family downgrade carries. The family
/// stays required; only the gate disposition is downgraded, and a downgrade is
/// never counted as a measurement (M516-th0oby/S05, D548/D551).
pub const NATIVE_ACCEPTANCE_NOT_APPLICABLE_STATE: &str = "not_applicable";
/// Gate kind carried by a recorded required-family downgrade.
pub const NATIVE_ACCEPTANCE_REQUIRED_FAMILY_DOWNGRADE_KIND: &str = "required_family_downgrade";
/// The body-axis gate id; the residual below stays named, not renamed.
pub const NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID: &str = "body_axis.body_not_assessable";

// ── fatal refusal codes ─────────────────────────────────────────────────────

pub const REFUSAL_CONTRACT_NOT_AN_OBJECT: &str = "contract_not_an_object";
pub const REFUSAL_THRESHOLD_POLICY_NOT_FROZEN: &str = "threshold_policy_not_frozen";
pub const REFUSAL_FROZEN_REFERENCE_PATH_MISSING: &str = "frozen_reference_path_missing";
pub const REFUSAL_THRESHOLD_LITERAL: &str = "threshold_literal";
pub const REFUSAL_THRESHOLD_REFERENCE_UNBOUND: &str = "threshold_reference_unbound";
pub const REFUSAL_SELF_REFERENCE: &str = "self_reference";
pub const REFUSAL_REFERENCE_NOT_INDEPENDENTLY_PINNED: &str = "reference_not_independently_pinned";
pub const REFUSAL_GATE_NOT_IN_REQUIRED_SCOPE: &str = "gate_not_in_required_scope";
pub const REFUSAL_THRESHOLD_ON_PENDING_FAMILY: &str = "threshold_on_pending_family";
pub const REFUSAL_UNKNOWN_FAMILY_STATE: &str = "unknown_family_state";
pub const REFUSAL_REFERENCE_DOCUMENT_MISSING: &str = "reference_document_missing";
pub const REFUSAL_REFERENCE_DIGEST_MISMATCH: &str = "reference_digest_mismatch";
pub const REFUSAL_THRESHOLD_MISSING_FOR_REQUIRED_METRIC: &str =
    "threshold_missing_for_required_metric";
pub const REFUSAL_DIRECTION_UNKNOWN: &str = "direction_unknown";
pub const REFUSAL_UNNAMED_GATE: &str = "unnamed_gate";
pub const REFUSAL_DROPPED_EXCLUSION: &str = "dropped_exclusion";
pub const REFUSAL_UNNAMED_EXCLUSION: &str = "unnamed_exclusion";
pub const REFUSAL_OPEN_LOCK: &str = "open_lock";
/// A recorded downgrade names no owner.
pub const REFUSAL_DOWNGRADE_WITHOUT_OWNER: &str = "downgrade_without_owner";
/// A recorded downgrade names no reason.
pub const REFUSAL_DOWNGRADE_WITHOUT_REASON: &str = "downgrade_without_reason";
/// A recorded downgrade reason is outside the pending-reason vocabulary.
pub const REFUSAL_DOWNGRADE_UNKNOWN_REASON: &str = "downgrade_unknown_reason";
/// A recorded downgrade names no decision_ref.
pub const REFUSAL_DOWNGRADE_WITHOUT_DECISION: &str = "downgrade_without_decision";
/// A recorded downgrade names no rationale.
pub const REFUSAL_DOWNGRADE_WITHOUT_RATIONALE: &str = "downgrade_without_rationale";
/// A recorded downgrade targets a family outside `required_scope.families`.
pub const REFUSAL_DOWNGRADE_UNKNOWN_FAMILY: &str = "downgrade_unknown_family";
/// A recorded downgrade targets a family the corpus measures.
pub const REFUSAL_DOWNGRADE_FOR_MEASURED_FAMILY: &str = "downgrade_for_measured_family";
/// A recorded downgrade disposition is outside its declared vocabulary.
pub const REFUSAL_DOWNGRADE_UNKNOWN_DISPOSITION: &str = "downgrade_unknown_disposition";

// ── typed not-comparable reasons ────────────────────────────────────────────

pub const REASON_METRIC_ABSENT: &str = "metric_absent";
pub const REASON_NO_DENOMINATOR: &str = "no_denominator";
pub const REASON_COMPARABILITY_FIELDS_MISSING: &str = "comparability_fields_missing";
pub const REASON_MIXED_SCOPE_COMPARISON: &str = "mixed_scope_comparison";
pub const REASON_REFERENCE_UNBOUND: &str = "reference_unbound";

/// A fatal, typed evaluation refusal. A refusal means the input shape would let
/// a false pass through, so no evaluation document is emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAcceptanceRefusal {
    pub code: String,
    pub detail: String,
}

impl NativeAcceptanceRefusal {
    fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for NativeAcceptanceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for NativeAcceptanceRefusal {}

/// One measurement series identity. Two measurements may only be compared when
/// all four keys are equal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Series {
    pub corpus_id: String,
    pub profile: String,
    pub oracle: String,
    pub source_revision: String,
}

impl Series {
    /// All four keys present and non-blank.
    pub fn is_complete(&self) -> bool {
        [
            self.corpus_id.as_str(),
            self.profile.as_str(),
            self.oracle.as_str(),
            self.source_revision.as_str(),
        ]
        .iter()
        .all(|key| !key.trim().is_empty())
    }
}

/// A candidate metric row as measured by the corpus-bound runner (T01 shape).
/// `series` is optional so an un-enriched row still deserializes; an absent
/// series is `not_comparable`, never assumed equal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredMetric {
    pub metric_id: String,
    #[serde(default)]
    pub numerator: Option<f64>,
    #[serde(default)]
    pub denominator: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stratum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Series>,
}

impl MeasuredMetric {
    /// The row's value under the `score_number` rule: a denominator must be
    /// positive, otherwise there is nothing to divide and the value is `None`.
    pub fn value(&self) -> Option<f64> {
        match (self.numerator, self.denominator) {
            (Some(numerator), Some(denominator)) if denominator > 0.0 => {
                Some(numerator / denominator)
            }
            _ => None,
        }
    }

    /// The row's denominator when it is a positive number.
    pub fn positive_denominator(&self) -> Option<f64> {
        match self.denominator {
            Some(denominator) if denominator > 0.0 => Some(denominator),
            _ => None,
        }
    }
}

/// A reference artifact supplied by the caller: its repository-relative path,
/// optional declared digest and bytes (used to verify the digest), and the
/// parsed document the pointer is resolved against.
#[derive(Debug, Clone)]
pub struct ReferenceDocument {
    pub path: String,
    pub sha256: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub document: Value,
}

/// The pinned reference set and the series identity of the measured reference.
#[derive(Debug, Clone)]
pub struct ReferenceSet {
    pub reference_series: Option<Series>,
    pub documents: Vec<ReferenceDocument>,
}

impl ReferenceSet {
    fn document(&self, path: &str) -> Option<&ReferenceDocument> {
        self.documents.iter().find(|doc| doc.path == path)
    }
}

/// The body-axis residual the frozen local corpus cannot decide. `verdict` is
/// `not_assessable`; the residual names stay verbatim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyAxisResidual {
    pub verdict: String,
    #[serde(default)]
    pub residual: Vec<String>,
    pub evidence_path: String,
}

/// The candidate's denominator partition. The 79 typed exclusions stay counted:
/// a partition that does not sum is a dropped exclusion, not a narrower scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationDenominator {
    pub assigned: u64,
    pub executed: u64,
    pub refused: u64,
    pub excluded: u64,
    #[serde(default)]
    pub exclusion_reasons: BTreeMap<String, u64>,
}

/// The measured candidate: where it came from, the series it belongs to, its
/// metric rows and its denominator partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateMeasurement {
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Series>,
    pub metrics: Vec<MeasuredMetric>,
    pub denominator: PopulationDenominator,
}

impl CandidateMeasurement {
    fn metric(&self, metric_id: &str, stratum: Option<&str>) -> Option<&MeasuredMetric> {
        // A threshold names `(metric_id, stratum)`; prefer the exact stratum when
        // the row carries one, else the first row for that metric.
        let exact = stratum.and_then(|stratum| {
            self.metrics
                .iter()
                .find(|row| row.metric_id == metric_id && row.stratum.as_deref() == Some(stratum))
        });
        exact.or_else(|| self.metrics.iter().find(|row| row.metric_id == metric_id))
    }
}

/// The complete evaluator input. Everything is supplied: no hidden file read.
#[derive(Debug, Clone)]
pub struct NativeAcceptanceInput {
    pub candidate: CandidateMeasurement,
    pub contract: Value,
    pub references: ReferenceSet,
    pub locks: BTreeMap<String, String>,
    pub body_axis: BodyAxisResidual,
}

/// One typed threshold verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdVerdict {
    pub threshold_id: String,
    pub family_id: String,
    pub metric_id: String,
    pub stratum: Option<String>,
    pub direction: String,
    pub margin: f64,
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub frozen_reference_value: Option<f64>,
    pub candidate_value: Option<f64>,
    pub candidate_denominator: Option<f64>,
    /// `matched` | `missing` | `mixed` | `not_required`.
    pub comparability: String,
}

/// One typed mandatory gate in the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandatoryGate {
    pub gate_id: String,
    /// `threshold` | `body_axis` | `threshold_pending_family` |
    /// `required_family_downgrade`.
    pub kind: String,
    pub state: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What evidence would clear the gate.
    pub evidence: String,
}

/// The typed evaluation document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAcceptanceEvaluation {
    pub schema_version: String,
    pub claim_scope: String,
    pub product_coverage_effect: String,
    pub candidate_source_path: String,
    pub candidate_series: Option<Series>,
    pub threshold_policy_state: String,
    pub frozen_reference_path: String,
    pub thresholds: Vec<ThresholdVerdict>,
    pub gates: Vec<MandatoryGate>,
}

/// The `score_number` rule: a scalar number, or a `{numerator, denominator}`
/// map read as a number only when its denominator is positive. Mirrors the
/// governor domain so the frozen reference and the candidate cannot disagree.
pub fn score_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::Object(map) => {
            let numerator = map.get("numerator")?;
            let denominator = map.get("denominator")?;
            let numerator = match numerator {
                Value::Number(number) => number.as_f64()?,
                _ => return None,
            };
            let denominator = match denominator {
                Value::Number(number) => number.as_f64()?,
                _ => return None,
            };
            if denominator > 0.0 {
                Some(numerator / denominator)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Follow a slash-separated pointer into a JSON document.
///
/// Grammar (deliberately small, so it is reviewable):
///   * `key` addresses an object member;
///   * `key[field=value]` selects, from the list at `key`, the first mapping
///     whose `field` stringifies to `value`.
///
/// Returns `None` when any segment does not resolve.
pub fn resolve_pointer<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut node = document;
    for raw in pointer.split('/') {
        let segment = raw.trim();
        if segment.is_empty() {
            return None;
        }
        match segment.split_once('[') {
            Some((key, rest)) => {
                let inner = rest.strip_suffix(']')?;
                let (field, wanted) = inner.split_once('=')?;
                let list = node.get(key.trim())?.as_array()?;
                node = list.iter().find(|item| {
                    lookup_string(item.get(field.trim()).unwrap_or(&Value::Null))
                        .is_some_and(|value| value == wanted)
                })?;
            }
            None => {
                node = node.get(segment)?;
            }
        }
    }
    Some(node)
}

fn lookup_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Lowercased-hex sha256 of `bytes`, formatted `sha256:<hex>`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(7 + digest.len() * 2);
    hex.push_str("sha256:");
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn lock_is_open(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    NATIVE_ACCEPTANCE_OPEN_LOCK_VALUES.contains(&normalized.as_str())
}

fn member<'a>(object: &'a Value, key: &str) -> Option<&'a Value> {
    object.as_object()?.get(key)
}

fn member_str<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    member(object, key).and_then(Value::as_str)
}

fn member_number(object: &Value, key: &str) -> Option<f64> {
    member(object, key).and_then(Value::as_f64)
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// A recorded `not_applicable` downgrade for one required family the frozen
/// corpus cannot measure (M516-th0oby/S05, D548/D551). The record is read from
/// `required_scope.downgrades.records` in the supplied contract; an absent
/// block means no downgrades and stays backward compatible.
#[derive(Debug, Clone)]
struct DowngradeRecord {
    disposition: String,
    owner: String,
    reason: String,
    decision_ref: String,
    rationale: String,
}

/// Read the recorded required-scope downgrade registry from the contract.
///
/// The block is optional: absent or null means no downgrades (backward
/// compatible). When present it must be a registry object carrying a `records`
/// map, and every record must name a declared `threshold_pending` family, carry
/// a disposition from the declared vocabulary and the four mandatory fields,
/// and carry a reason from the contract's existing pending-reason vocabulary. A
/// malformed record is fatal rather than silently leaving a family at
/// `threshold_pending`.
fn read_required_scope_downgrades(
    contract_object: &serde_json::Map<String, Value>,
    family_states: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, DowngradeRecord>, NativeAcceptanceRefusal> {
    let Some(scope) = contract_object
        .get("required_scope")
        .and_then(Value::as_object)
    else {
        return Ok(BTreeMap::new());
    };
    let Some(downgrades) = scope.get("downgrades") else {
        return Ok(BTreeMap::new());
    };
    if downgrades.is_null() {
        return Ok(BTreeMap::new());
    }
    let downgrades = downgrades.as_object().ok_or_else(|| {
        NativeAcceptanceRefusal::new(
            REFUSAL_DOWNGRADE_UNKNOWN_FAMILY,
            "required_scope.downgrades is present but is not a downgrade registry object",
        )
    })?;
    let records = downgrades
        .get("records")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_UNKNOWN_FAMILY,
                "required_scope.downgrades carries no `records` map; a present registry must name its families",
            )
        })?;
    let disposition_vocabulary: Vec<&str> = downgrades
        .get("disposition_vocabulary")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    // The reason vocabulary is the one the schema already declares beside the
    // pending families; the registry reaches it by pointer, so the evaluator
    // reads it here instead of copying the seven tokens.
    let reason_vocabulary: Vec<&str> = scope
        .get("pending_reason_vocabulary")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut parsed: BTreeMap<String, DowngradeRecord> = BTreeMap::new();
    for (family_id, record) in records {
        match family_states.get(family_id.as_str()).map(String::as_str) {
            None => {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_DOWNGRADE_UNKNOWN_FAMILY,
                    format!(
                        "downgrade targets family `{family_id}`, which is outside required_scope.families"
                    ),
                ));
            }
            Some(NATIVE_ACCEPTANCE_REQUIRED_STATE) => {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_DOWNGRADE_FOR_MEASURED_FAMILY,
                    format!(
                        "family `{family_id}` is measured by corpus parity and may not carry a downgrade"
                    ),
                ));
            }
            Some(_) => {}
        }
        let Some(record) = record.as_object() else {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_WITHOUT_OWNER,
                format!("downgrade for `{family_id}` is not a record object; it names no owner"),
            ));
        };
        let disposition = record
            .get("disposition")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !disposition_vocabulary.contains(&disposition) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_UNKNOWN_DISPOSITION,
                format!(
                    "downgrade for `{family_id}` declares disposition `{disposition}`, outside the declared vocabulary [{}]",
                    disposition_vocabulary.join(", ")
                ),
            ));
        }
        let field = |name: &str| -> String {
            record
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let owner = field("owner");
        if owner.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_WITHOUT_OWNER,
                format!("downgrade for `{family_id}` names no owner"),
            ));
        }
        let reason = field("reason");
        if reason.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_WITHOUT_REASON,
                format!("downgrade for `{family_id}` names no reason"),
            ));
        }
        if !reason_vocabulary.contains(&reason.as_str()) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_UNKNOWN_REASON,
                format!(
                    "downgrade for `{family_id}` declares reason `{reason}`, outside the existing pending-reason vocabulary"
                ),
            ));
        }
        let decision_ref = field("decision_ref");
        if decision_ref.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_WITHOUT_DECISION,
                format!("downgrade for `{family_id}` names no decision_ref"),
            ));
        }
        let rationale = field("rationale");
        if rationale.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DOWNGRADE_WITHOUT_RATIONALE,
                format!("downgrade for `{family_id}` names no rationale"),
            ));
        }
        parsed.insert(
            family_id.clone(),
            DowngradeRecord {
                disposition: disposition.to_string(),
                owner,
                reason,
                decision_ref,
                rationale,
            },
        );
    }
    Ok(parsed)
}

/// Evaluate a candidate against the frozen contract and build the typed
/// mandatory-gate ledger. Fails closed on every shape that would allow a false
/// pass.
pub fn evaluate_native_acceptance(
    input: &NativeAcceptanceInput,
) -> Result<NativeAcceptanceEvaluation, NativeAcceptanceRefusal> {
    let contract = &input.contract;
    let contract_object = contract.as_object().ok_or_else(|| {
        NativeAcceptanceRefusal::new(
            REFUSAL_CONTRACT_NOT_AN_OBJECT,
            "contract document is not an object",
        )
    })?;

    // ── locks: any open lock refuses the evaluation ──────────────────────────
    for (key, value) in &input.locks {
        if lock_is_open(value) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_OPEN_LOCK,
                format!("lock `{key}` is open (`{value}`); all five locks must stay closed"),
            ));
        }
    }

    // ── denominator: the typed exclusions stay counted ───────────────────────
    check_denominator(&input.candidate.denominator, contract_object)?;

    // ── threshold policy must be frozen and independently pinned ─────────────
    let policy = contract_object
        .get("threshold_policy")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_THRESHOLD_POLICY_NOT_FROZEN,
                "threshold_policy block is missing",
            )
        })?;
    let policy_state = policy.get("state").and_then(Value::as_str).ok_or_else(|| {
        NativeAcceptanceRefusal::new(
            REFUSAL_THRESHOLD_POLICY_NOT_FROZEN,
            "threshold_policy.state is missing",
        )
    })?;
    if policy_state != "frozen" {
        return Err(NativeAcceptanceRefusal::new(
            REFUSAL_THRESHOLD_POLICY_NOT_FROZEN,
            format!("threshold_policy.state is `{policy_state}`, not `frozen`"),
        ));
    }
    let frozen_reference_path = policy
        .get("reference_pins")
        .and_then(Value::as_object)
        .and_then(|pins| pins.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_FROZEN_REFERENCE_PATH_MISSING,
                "threshold_policy.reference_pins.path is missing",
            )
        })?;
    let direction_vocabulary: Vec<String> = policy
        .get("direction_vocabulary")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| {
            NATIVE_ACCEPTANCE_DIRECTION_VOCABULARY
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        });

    // ── required scope families, read by pointer from the contract ───────────
    let families = contract_object
        .get("required_scope")
        .and_then(|scope| scope.get("families"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_UNNAMED_GATE,
                "required_scope.families is missing; the family list must be read from the contract",
            )
        })?;
    let mut family_states: BTreeMap<String, String> = BTreeMap::new();
    let mut pending_families: Vec<(String, Option<String>)> = Vec::new();
    for entry in families {
        let family_id = member_str(entry, "family_id")
            .unwrap_or("")
            .trim()
            .to_string();
        if family_id.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_UNNAMED_GATE,
                "required_scope.families carries an entry without a family_id",
            ));
        }
        let state = member_str(entry, "state").unwrap_or("").trim().to_string();
        if !NATIVE_ACCEPTANCE_FAMILY_STATE_VOCABULARY.contains(&state.as_str()) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_UNKNOWN_FAMILY_STATE,
                format!("family `{family_id}` declares unknown state `{state}`"),
            ));
        }
        if state == NATIVE_ACCEPTANCE_THRESHOLD_PENDING_STATE {
            pending_families.push((
                family_id.clone(),
                member_str(entry, "reason").map(str::to_string),
            ));
        }
        family_states.insert(family_id, state);
    }

    // ── recorded downgrades, read by pointer from the contract ───────────────
    // A family may only leave `threshold_pending` through a valid recorded
    // downgrade; a malformed record is fatal rather than a silent
    // not-applicable.
    let downgrades = read_required_scope_downgrades(contract_object, &family_states)?;

    // ── per-threshold evaluation ─────────────────────────────────────────────
    let thresholds = policy
        .get("thresholds")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_THRESHOLD_REFERENCE_UNBOUND,
                "threshold_policy.thresholds is missing",
            )
        })?;

    let mut verdicts: Vec<ThresholdVerdict> = Vec::with_capacity(thresholds.len());
    let mut covered_required_families: Vec<String> = Vec::new();

    for entry in thresholds {
        let threshold_id = member_str(entry, "threshold_id")
            .unwrap_or("")
            .trim()
            .to_string();
        if threshold_id.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_UNNAMED_GATE,
                "a frozen threshold carries no threshold_id",
            ));
        }
        // A literal threshold value is refused: the value must come from the
        // independently pinned reference by pointer.
        for literal_key in ["value", "threshold", "fraction"] {
            if member(entry, literal_key).is_some() {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_THRESHOLD_LITERAL,
                    format!(
                        "threshold `{threshold_id}` carries a `{literal_key}` literal; \
                         the floor must resolve by reference pointer"
                    ),
                ));
            }
        }

        let family_id = member_str(entry, "family_id")
            .unwrap_or("")
            .trim()
            .to_string();
        if family_id.is_empty() {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_UNNAMED_GATE,
                format!("threshold `{threshold_id}` carries no family_id"),
            ));
        }
        let metric_id = member_str(entry, "metric_id")
            .unwrap_or("")
            .trim()
            .to_string();
        let stratum = member_str(entry, "stratum").map(str::to_string);
        let direction = member_str(entry, "direction")
            .unwrap_or("")
            .trim()
            .to_string();
        if !direction_vocabulary.iter().any(|known| known == &direction) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_DIRECTION_UNKNOWN,
                format!("threshold `{threshold_id}` declares unknown direction `{direction}`"),
            ));
        }
        let margin = member_number(entry, "margin").unwrap_or(0.0);

        let reference = member(entry, "reference")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                NativeAcceptanceRefusal::new(
                    REFUSAL_THRESHOLD_REFERENCE_UNBOUND,
                    format!("threshold `{threshold_id}` has no reference block"),
                )
            })?;
        let reference_path = reference
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeAcceptanceRefusal::new(
                    REFUSAL_THRESHOLD_REFERENCE_UNBOUND,
                    format!("threshold `{threshold_id}` reference has no path"),
                )
            })?;
        let reference_pointer = reference
            .get("pointer")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeAcceptanceRefusal::new(
                    REFUSAL_THRESHOLD_REFERENCE_UNBOUND,
                    format!("threshold `{threshold_id}` reference has no pointer"),
                )
            })?;

        // Candidate used as its own reference.
        if reference_path == input.candidate.source_path {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_SELF_REFERENCE,
                format!(
                    "threshold `{threshold_id}` resolves against the candidate (`{reference_path}`); \
                     a candidate may not set its own floor"
                ),
            ));
        }
        // Substituted reference: must be the separately pinned artifact.
        if reference_path != frozen_reference_path {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_REFERENCE_NOT_INDEPENDENTLY_PINNED,
                format!(
                    "threshold `{threshold_id}` reference path `{reference_path}` is not the pinned \
                     reference `{frozen_reference_path}`"
                ),
            ));
        }

        // Family state decides whether a threshold may exist at all.
        match family_states.get(&family_id).map(String::as_str) {
            None => {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_GATE_NOT_IN_REQUIRED_SCOPE,
                    format!(
                        "threshold `{threshold_id}` family `{family_id}` is outside required_scope"
                    ),
                ));
            }
            Some(NATIVE_ACCEPTANCE_THRESHOLD_PENDING_STATE) => {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_THRESHOLD_ON_PENDING_FAMILY,
                    format!(
                        "threshold `{threshold_id}` is placed on threshold_pending family `{family_id}`; \
                         a threshold the corpus cannot measure is a false pass"
                    ),
                ));
            }
            Some(NATIVE_ACCEPTANCE_REQUIRED_STATE) => {}
            Some(other) => {
                return Err(NativeAcceptanceRefusal::new(
                    REFUSAL_UNKNOWN_FAMILY_STATE,
                    format!("family `{family_id}` declares unknown state `{other}`"),
                ));
            }
        }
        covered_required_families.push(family_id.clone());

        // Resolve the frozen reference from the supplied document by pointer.
        let reference_document = input.references.document(reference_path).ok_or_else(|| {
            NativeAcceptanceRefusal::new(
                REFUSAL_REFERENCE_DOCUMENT_MISSING,
                format!("reference document `{reference_path}` was not supplied"),
            )
        })?;
        verify_reference_digest(reference_document)?;
        let frozen_reference_value = score_number(
            resolve_pointer(&reference_document.document, reference_pointer)
                .unwrap_or(&Value::Null),
        );

        let metric = input.candidate.metric(&metric_id, stratum.as_deref());
        let candidate_value = metric.and_then(MeasuredMetric::value);
        let candidate_denominator = metric.and_then(MeasuredMetric::positive_denominator);
        let effective_series = metric
            .and_then(|row| row.series.clone())
            .or_else(|| input.candidate.series.clone());

        let (verdict, reason, comparability) = match (&frozen_reference_value, metric) {
            (None, _) => (
                "not_comparable".to_string(),
                Some(REASON_REFERENCE_UNBOUND.to_string()),
                "not_required".to_string(),
            ),
            (Some(_), None) => (
                "not_comparable".to_string(),
                Some(REASON_METRIC_ABSENT.to_string()),
                "not_required".to_string(),
            ),
            (Some(_), Some(_)) if candidate_value.is_none() => (
                "not_comparable".to_string(),
                Some(REASON_NO_DENOMINATOR.to_string()),
                "not_required".to_string(),
            ),
            (Some(reference_value), Some(_)) => {
                match compare_series(
                    effective_series.as_ref(),
                    input.references.reference_series.as_ref(),
                ) {
                    SeriesMatch::Missing => (
                        "not_comparable".to_string(),
                        Some(REASON_COMPARABILITY_FIELDS_MISSING.to_string()),
                        "missing".to_string(),
                    ),
                    SeriesMatch::Mixed => (
                        "not_comparable".to_string(),
                        Some(REASON_MIXED_SCOPE_COMPARISON.to_string()),
                        "mixed".to_string(),
                    ),
                    SeriesMatch::Matched => {
                        let candidate_value = candidate_value.expect("guarded above");
                        let met = match direction.as_str() {
                            "at_least" => candidate_value >= reference_value - margin,
                            "at_most" => candidate_value <= reference_value + margin,
                            _ => false,
                        };
                        (
                            if met { "met" } else { "unmet" }.to_string(),
                            None,
                            "matched".to_string(),
                        )
                    }
                }
            }
        };

        verdicts.push(ThresholdVerdict {
            threshold_id,
            family_id,
            metric_id,
            stratum,
            direction,
            margin,
            verdict,
            reason,
            frozen_reference_value,
            candidate_value,
            candidate_denominator,
            comparability,
        });
    }

    // Every required (measurable) family must carry exactly one threshold.
    for (family_id, state) in &family_states {
        if state == NATIVE_ACCEPTANCE_REQUIRED_STATE
            && !covered_required_families
                .iter()
                .any(|covered| covered == family_id)
        {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_THRESHOLD_MISSING_FOR_REQUIRED_METRIC,
                format!("required family `{family_id}` carries no frozen threshold"),
            ));
        }
    }

    // ── mandatory-gate ledger ────────────────────────────────────────────────
    let mut gates: Vec<MandatoryGate> = Vec::new();
    for verdict in &verdicts {
        gates.push(MandatoryGate {
            gate_id: format!("threshold.{}", verdict.threshold_id),
            kind: "threshold".to_string(),
            state: verdict.verdict.clone(),
            required: true,
            family_id: Some(verdict.family_id.clone()),
            metric_id: Some(verdict.metric_id.clone()),
            reason: verdict.reason.clone(),
            evidence: format!(
                "measured {} on series [{}] from {} in the pinned reference {}",
                verdict.metric_id,
                input
                    .references
                    .reference_series
                    .as_ref()
                    .map(series_label)
                    .unwrap_or_else(|| "<undeclared series>".to_string()),
                input.candidate.source_path,
                frozen_reference_path,
            ),
        });
    }

    if is_blank(&input.body_axis.verdict) {
        return Err(NativeAcceptanceRefusal::new(
            REFUSAL_UNNAMED_GATE,
            "body-axis residual gate carries no verdict",
        ));
    }
    if input.body_axis.residual.iter().any(|name| is_blank(name)) {
        return Err(NativeAcceptanceRefusal::new(
            REFUSAL_UNNAMED_GATE,
            "body-axis residual carries an unnamed residual",
        ));
    }
    gates.push(MandatoryGate {
        gate_id: NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID.to_string(),
        kind: "body_axis".to_string(),
        state: input.body_axis.verdict.clone(),
        required: true,
        family_id: None,
        metric_id: None,
        reason: Some(input.body_axis.residual.join(",")),
        evidence: format!(
            "body-axis residual {} is not assessable from the frozen local corpus; evidence carrier {}",
            input.body_axis.verdict, input.body_axis.evidence_path
        ),
    });

    for (family_id, reason) in &pending_families {
        if let Some(record) = downgrades.get(family_id) {
            // A recorded downgrade keeps the same gate id and the same count,
            // but carries the decision instead of a fabricated threshold.
            gates.push(MandatoryGate {
                gate_id: format!("required_scope.{family_id}"),
                kind: NATIVE_ACCEPTANCE_REQUIRED_FAMILY_DOWNGRADE_KIND.to_string(),
                state: NATIVE_ACCEPTANCE_NOT_APPLICABLE_STATE.to_string(),
                required: true,
                family_id: Some(family_id.clone()),
                metric_id: None,
                reason: Some(record.reason.clone()),
                evidence: format!(
                    "family `{family_id}` is recorded `{disposition}` on the frozen corpus by {owner} \
                     (decision {decision}); {rationale}",
                    disposition = record.disposition,
                    owner = record.owner,
                    decision = record.decision_ref,
                    rationale = record.rationale,
                ),
            });
        } else {
            gates.push(MandatoryGate {
                gate_id: format!("required_scope.{family_id}"),
                kind: "threshold_pending_family".to_string(),
                state: NATIVE_ACCEPTANCE_THRESHOLD_PENDING_STATE.to_string(),
                required: true,
                family_id: Some(family_id.clone()),
                metric_id: None,
                reason: reason.clone(),
                evidence: format!(
                    "family `{family_id}` has no measurable frozen-corpus mechanism; \
                     a threshold here would be a false pass"
                ),
            });
        }
    }

    Ok(NativeAcceptanceEvaluation {
        schema_version: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_string(),
        claim_scope: NATIVE_ACCEPTANCE_CLAIM_SCOPE.to_string(),
        product_coverage_effect: NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT.to_string(),
        candidate_source_path: input.candidate.source_path.clone(),
        candidate_series: input.candidate.series.clone(),
        threshold_policy_state: policy_state.to_string(),
        frozen_reference_path: frozen_reference_path.to_string(),
        thresholds: verdicts,
        gates,
    })
}

enum SeriesMatch {
    Matched,
    Missing,
    Mixed,
}

fn compare_series(candidate: Option<&Series>, reference: Option<&Series>) -> SeriesMatch {
    match (candidate, reference) {
        (Some(candidate), Some(reference))
            if candidate.is_complete() && reference.is_complete() =>
        {
            if candidate == reference {
                SeriesMatch::Matched
            } else {
                SeriesMatch::Mixed
            }
        }
        _ => SeriesMatch::Missing,
    }
}

fn series_label(series: &Series) -> String {
    format!(
        "corpus_id={}, profile={}, oracle={}, source_revision={}",
        series.corpus_id, series.profile, series.oracle, series.source_revision
    )
}

fn verify_reference_digest(document: &ReferenceDocument) -> Result<(), NativeAcceptanceRefusal> {
    if let (Some(declared), Some(bytes)) = (&document.sha256, &document.bytes) {
        let actual = sha256_hex(bytes);
        if &actual != declared {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_REFERENCE_DIGEST_MISMATCH,
                format!(
                    "reference `{}` declared {declared} but the supplied bytes hash to {actual}",
                    document.path
                ),
            ));
        }
    }
    Ok(())
}

fn check_denominator(
    denominator: &PopulationDenominator,
    contract_object: &serde_json::Map<String, Value>,
) -> Result<(), NativeAcceptanceRefusal> {
    let total = denominator
        .executed
        .saturating_add(denominator.refused)
        .saturating_add(denominator.excluded);
    if denominator.assigned != total {
        return Err(NativeAcceptanceRefusal::new(
            REFUSAL_DROPPED_EXCLUSION,
            format!(
                "denominator partition does not sum: assigned={} executed={} refused={} excluded={}; \
                 a dropped exclusion narrows the denominator",
                denominator.assigned,
                denominator.executed,
                denominator.refused,
                denominator.excluded
            ),
        ));
    }
    let reason_total: u64 = denominator.exclusion_reasons.values().sum();
    if reason_total != denominator.excluded {
        return Err(NativeAcceptanceRefusal::new(
            REFUSAL_DROPPED_EXCLUSION,
            format!(
                "typed exclusion reasons sum to {reason_total} but excluded={}; \
                 every exclusion must stay named and counted",
                denominator.excluded
            ),
        ));
    }
    let vocabulary: Vec<String> = contract_object
        .get("denominator")
        .and_then(|block| block.get("exclusion_reasons"))
        .and_then(|block| block.get("values"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    for reason in denominator.exclusion_reasons.keys() {
        if !vocabulary.iter().any(|known| known == reason) {
            return Err(NativeAcceptanceRefusal::new(
                REFUSAL_UNNAMED_EXCLUSION,
                format!("exclusion reason `{reason}` is not in the closed vocabulary"),
            ));
        }
    }
    Ok(())
}
