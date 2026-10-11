//! M516-th0oby / S12 / T01 — the release-readiness receipt schema and its
//! fail-closed validator.
//!
//! S12 is the terminal slice of the milestone: it does not measure anything new,
//! it **assembles** the release-readiness view a human must act on. The three
//! quality / cost / rights release gates already exist as DATA
//! (`data/parse/acceptance_manifest.yaml#release_decision_package`), the machine
//! authorization verdict already exists
//! (`artifacts/parse_compat/grobid_retirement_acceptance_package.json`), and the
//! milestone evidence already exists (S03 corpus + thresholds, S05 scope-downgrade
//! registry, S10 governed-process record, S11 independent preservation candidate).
//! What does not exist is one machine-readable receipt that projects those three
//! gates with evidence **recomputed from committed bytes**, names exactly the
//! pending human decisions, binds the milestone evidence, and proves the machine
//! authorizes nothing.
//!
//! This module is the single source of truth for that artifact
//! (`artifacts/parse_compat/grobid_retirement_release_readiness.json`, schema
//! `grobid-retirement-release-readiness.v1`). The producer
//! (`examples/release_readiness_receipt.rs`), the hermetic contract test and the
//! governor half all read this module.
//!
//! * [`derive_release_readiness_expectation`] reads the committed carriers and
//!   recomputes every evidence sha256 from the committed bytes — it never copies a
//!   declared digest. It fails closed on an unreadable or unresolved carrier.
//! * [`build_release_readiness_receipt`] assembles the receipt and refuses, by
//!   name, every contract violation: a missing / extra / duplicate gate, a gate
//!   with no evidence, an unbound evidence binding, a claimed human decision, an
//!   empty decision record, an opened or missing lock, a claimed machine
//!   authorization, a widened claim scope, an accepted threshold, a claimed
//!   product coverage, a missing / duplicate / unexpected milestone slice and a
//!   redaction leak.
//! * [`validate_release_readiness_receipt`] re-derives the evidence set and
//!   refuses drift.
//!
//! The receipt is a **projection, not an authority**: `claim_scope =
//! release_readiness_preparation_only`, `product_coverage_effect = none`,
//! `thresholds = not_accepted`, `nothing_here_authorizes_a_release = true`, and
//! every promotion lock stays closed. It carries the human release record verbatim
//! (`reviewer = null`, `decision = pending`) and the machine-authorization verdict
//! verbatim (`s09_authorized = false`, `gsd_lifecycle_authorizes = false`,
//! `human_release_recorded = false`, five locks closed) — it can never *become* an
//! authorization (D127 / ADR-076 §14 / R140).
//!
//! The module is pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2`: it compiles
//! in the default feature set (no `pdf-layout`, no `pdf-classifier`). It reads the
//! committed carriers (filesystem) in the derive function only; the builder and
//! the validator are pure functions of their inputs.
//!
//! **Scope note (honesty):** the manifest declares the rights gate's evidence as
//! `doc/review/manifest.yaml` and `doc/review/grobid-absorption-review-2026-09-10.jsonl`.
//! This receipt does not re-bind those two carriers by sha256; the rights gate
//! carries the manifest's rights declaration (`release_decision_package.rights`)
//! and the S05 required-scope-downgrade registry instead. The receipt projects the
//! gate and its declared effect, it does not replace the human rights review.

use crate::native_family_receipt::{
    CarrierBinding, LEAK_MARKERS, REQUIRED_LOCKS, Redaction, is_repo_relative_path,
    is_sha256_digest, sha256_prefixed,
};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as Yaml;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

/// Schema version of the emitted release-readiness receipt.
pub const RELEASE_READINESS_SCHEMA_VERSION: &str = "grobid-retirement-release-readiness.v1";
/// Default artifact path of the release-readiness receipt.
pub const DEFAULT_RELEASE_READINESS_ARTIFACT_REL: &str =
    "artifacts/parse_compat/grobid_retirement_release_readiness.json";
/// The only claim scope the receipt may carry: it prepares a release, it does not
/// authorise one.
pub const RELEASE_READINESS_CLAIM_SCOPE: &str = "release_readiness_preparation_only";
/// The only product-coverage effect the receipt may declare.
pub const RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT: &str = "none";
/// Thresholds are never accepted in this cut; the receipt only assembles.
pub const RELEASE_READINESS_THRESHOLDS: &str = "not_accepted";
/// The only human decision state a preparation receipt may carry.
pub const RELEASE_READINESS_HUMAN_DECISION_PENDING: &str = "pending";
/// The three release gates, in canonical order. The receipt carries exactly these.
pub const RELEASE_GATE_ORDER: [&str; 3] = ["quality", "cost", "rights"];
/// The four milestone slices whose evidence the receipt binds.
pub const MILESTONE_SLICE_ORDER: [&str; 4] = ["S03", "S05", "S10", "S11"];

// ───────────────────────────── carriers ─────────────────────────────

/// Carrier: the DATA declaration of the three release gates + the human record.
pub const ACCEPTANCE_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
/// Carrier: the acceptance package that carries the machine-authorization verdict.
pub const ACCEPTANCE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
/// Carrier: the S03 native route corpus.
pub const LAYOUT_BAKEOFF_REL: &str = "data/parse/layout_bakeoff.yaml";
/// Carrier: the S03 absorption-progress threshold candidates.
pub const THRESHOLD_CANDIDATES_REL: &str =
    "artifacts/parse_compat/absorption_progress_threshold_candidates.json";
/// Carrier: the S05 GROBID compatibility parity ledger.
pub const GROBID_COMPATIBILITY_REL: &str = "data/parse/grobid_compatibility.yaml";
/// Carrier: the S10 governed-process record fixture (the real runtime record).
pub const GOVERNED_RECORD_REL: &str =
    "governor/fixtures/governed_process_record/record_legal.positive.json";
/// Carrier: the S11 independent preservation candidate.
pub const INDEPENDENT_CANDIDATE_REL: &str =
    "artifacts/parse_compat/independent_preservation_candidate.json";

// ───────────────────────────── pointers ─────────────────────────────

/// Pointer to the release-decision package inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_RELEASE_DECISION_PACKAGE: &str = "release_decision_package";
/// Pointer to the quality gate inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_GATE_QUALITY: &str = "release_decision_package.quality";
/// Pointer to the cost gate inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_GATE_COST: &str = "release_decision_package.cost";
/// Pointer to the rights gate inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_GATE_RIGHTS: &str = "release_decision_package.rights";
/// Pointer to the human release review inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_HUMAN_RELEASE_REVIEW: &str = "release_decision_package.human_release_review";
/// Pointer to the cost bounds inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_COST_BOUNDS: &str = "external_holdout_contract.cost_bounds";
/// Pointer to the required gates inside [`ACCEPTANCE_MANIFEST_REL`].
pub const POINTER_REQUIRED_GATES: &str = "release_decision_package.required_gates";
/// Pointer to the machine authorization inside [`ACCEPTANCE_PACKAGE_REL`].
pub const POINTER_PACKAGE_AUTHORIZATION: &str = "authorization";
/// Pointer to the five locks inside [`ACCEPTANCE_PACKAGE_REL`].
pub const POINTER_PACKAGE_LOCKS: &str = "locks";
/// Pointer to the mandatory gates inside [`ACCEPTANCE_PACKAGE_REL`].
pub const POINTER_PACKAGE_MANDATORY_GATES: &str = "mandatory_gates";
/// Pointer to the S03 native route corpus inside [`LAYOUT_BAKEOFF_REL`].
pub const POINTER_NATIVE_ROUTE_CORPUS: &str = "native_route_corpus";
/// Pointer to the S03 threshold candidates inside [`THRESHOLD_CANDIDATES_REL`].
pub const POINTER_THRESHOLD_CANDIDATES: &str = "threshold_candidates";
/// Pointer to the S05 required-scope downgrades inside [`GROBID_COMPATIBILITY_REL`].
pub const POINTER_REQUIRED_SCOPE_DOWNGRADES: &str = "parity_ledger.required_scope_downgrades";
/// Pointer to the S10 governed-process record (the single fixture record).
pub const POINTER_GOVERNED_RECORD: &str = "0";
/// Pointer to the S11 series identity inside [`INDEPENDENT_CANDIDATE_REL`].
pub const POINTER_SERIES_IDENTITY: &str = "series_identity";

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals. Every variant is a refusal, never a silent downgrade: the
/// receipt either justifies what it says or says why it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseReadinessError {
    /// A carrier could not be read, parsed or navigated.
    MalformedCarrier { detail: String },
    /// A schema field drifted from the contract's constant.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The receipt tried to widen its claim beyond `release_readiness_preparation_only`.
    ClaimScopeDrift { found: String },
    /// The receipt declared a product-coverage effect while claiming none.
    ProductCoverageClaimed { found: String },
    /// The receipt claimed accepted thresholds.
    ThresholdClaimed { found: String },
    /// The receipt did not assert that nothing here authorises a release.
    NothingAuthorizesFlagFalse,
    /// A required release gate is absent.
    ReleaseGateMissing { gate_id: String },
    /// A release gate the contract does not declare is present.
    ReleaseGateUnexpected { gate_id: String },
    /// A release gate appears twice.
    DuplicateReleaseGate { gate_id: String },
    /// A release gate carries no evidence.
    ReleaseGateEvidenceEmpty { gate_id: String },
    /// A release-gate field is empty (status or release_effect).
    ReleaseGateFieldEmpty { gate_id: String, field: String },
    /// An evidence binding is not a repo-relative path, has an empty pointer, or
    /// is not a sha256 digest.
    UnboundEvidence { detail: String },
    /// An evidence binding appears twice.
    DuplicateEvidence { manifest: String, pointer: String },
    /// The receipt claims a human decision (a named reviewer, or a decision that
    /// is not `pending`).
    HumanDecisionClaimed { field: String, found: String },
    /// The decision record the human must fill is empty.
    DecisionRecordEmpty,
    /// A lock the receipt never relaxes was opened, absent or unexpected.
    LockOpened {
        lock: String,
        expected: String,
        found: String,
    },
    /// The receipt claims the machine authorizes something.
    MachineAuthorizationClaimed { field: String },
    /// A required milestone-evidence slice is absent.
    MilestoneSliceMissing { slice: String },
    /// A milestone-evidence slice appears twice.
    DuplicateMilestoneSlice { slice: String },
    /// A milestone-evidence slice the contract does not declare is present.
    MilestoneSliceUnexpected { slice: String },
    /// A milestone-evidence binding is malformed.
    UnboundMilestoneEvidence { detail: String },
    /// A release gate drifted from the re-derived one.
    ReleaseGateDrift {
        gate_id: String,
        field: String,
        expected: String,
        found: String,
    },
    /// A gate's declared status disagrees with the human decision's gate status.
    GateStatusAlignmentDrift {
        gate_id: String,
        expected: String,
        found: String,
    },
    /// The human decision drifted from the re-derived one.
    HumanDecisionDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A milestone-evidence binding drifted from the re-derived one.
    MilestoneEvidenceDrift {
        slice: String,
        expected: String,
        found: String,
    },
    /// The evidence set disagrees with the re-derived bindings.
    EvidenceDrift { detail: String },
    /// The artifact leaked a payload, endpoint or host path marker.
    RedactionLeak { marker: String },
}

impl fmt::Display for ReleaseReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ReleaseReadinessError {}

// ───────────────────────────── value types ─────────────────────────────

/// One release gate: its declared status, its declared release effect and the
/// evidence bindings (path + pointer + sha256) recomputed from committed bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseGateRow {
    /// The gate id: `quality`, `cost` or `rights`.
    pub gate_id: String,
    /// The gate status as the manifest declares it (never restated in code).
    pub status: String,
    /// The gate's declared release effect.
    pub release_effect: String,
    /// The carriers that evidence the gate, sha256 recomputed from committed bytes.
    pub evidence: Vec<CarrierBinding>,
}

/// The exact pending human decision the receipt names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanDecision {
    /// The reviewer: `null` while the decision is pending.
    pub reviewer: Option<String>,
    /// The decision: `pending` while the decision is pending.
    pub decision: String,
    /// The form a recorded decision must resolve to.
    pub decision_record: String,
    /// What a recorded authorization requires.
    pub authorization_requires: Vec<String>,
    /// The live status of each release gate the human must see pass.
    pub gate_status: BTreeMap<String, String>,
}

/// The machine-authorization verdict, carried verbatim: the machine authorizes
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineAuthorization {
    /// Always `false`: no S09 authorization.
    pub s09_authorized: bool,
    /// Always `false`: a GSD lifecycle state never authorizes a release.
    pub gsd_lifecycle_authorizes: bool,
    /// Always `false`: no human release decision is recorded.
    pub human_release_recorded: bool,
    /// The five locks, exactly as the contract declares them.
    pub locks: BTreeMap<String, String>,
}

/// One milestone slice's evidence, bound by path + pointer + sha256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilestoneEvidence {
    /// The milestone slice id (`S03`, `S05`, `S10`, `S11`).
    pub slice: String,
    /// The repository-relative carrier path.
    pub manifest: String,
    /// The pointer inside the carrier.
    pub pointer: String,
    /// The sha256 of the exact carrier bytes, recomputed from committed bytes.
    pub revision_sha256: String,
}

/// The emitted release-readiness receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseReadinessReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Always `release_readiness_preparation_only`.
    pub claim_scope: String,
    /// Always `none`.
    pub product_coverage_effect: String,
    /// Always `not_accepted`.
    pub thresholds: String,
    /// Always `true`: nothing in this receipt authorises a release.
    pub nothing_here_authorizes_a_release: bool,
    /// The three release gates with recomputed evidence.
    pub release_gates: Vec<ReleaseGateRow>,
    /// The pending human decision.
    pub human_decision: HumanDecision,
    /// The machine-authorization verdict (none).
    pub machine_authorization: MachineAuthorization,
    /// The milestone evidence bindings.
    pub milestone_evidence: Vec<MilestoneEvidence>,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl ReleaseReadinessReceipt {
    /// Serialize canonically (pretty JSON, one trailing newline) and refuse if any
    /// payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, ReleaseReadinessError> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            ReleaseReadinessError::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        bytes.push(b'\n');
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            ReleaseReadinessError::MalformedCarrier {
                detail: error.to_string(),
            }
        })?;
        assert_release_readiness_redacted(text)?;
        Ok(bytes)
    }

    /// Parse a receipt from its canonical bytes.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, ReleaseReadinessError> {
        serde_json::from_slice(bytes).map_err(|error| ReleaseReadinessError::MalformedCarrier {
            detail: error.to_string(),
        })
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_release_readiness_redacted(json: &str) -> Result<(), ReleaseReadinessError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(ReleaseReadinessError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// The re-derived evidence set the validator recomputes against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseReadinessExpectation {
    /// The three release gates, evidence sha256 recomputed from committed bytes.
    pub release_gates: Vec<ReleaseGateRow>,
    /// The pending human decision, read from the manifest.
    pub human_decision: HumanDecision,
    /// The machine-authorization verdict, read from the acceptance package.
    pub machine_authorization: MachineAuthorization,
    /// The milestone evidence bindings, sha256 recomputed from committed bytes.
    pub milestone_evidence: Vec<MilestoneEvidence>,
}

// ───────────────────────────── helpers ─────────────────────────────

fn malformed(detail: impl Into<String>) -> ReleaseReadinessError {
    ReleaseReadinessError::MalformedCarrier {
        detail: detail.into(),
    }
}

fn read_carrier(root: &Path, rel: &str) -> Result<Vec<u8>, ReleaseReadinessError> {
    fs::read(root.join(rel)).map_err(|error| malformed(format!("cannot read `{rel}`: {error}")))
}

fn json_navigate<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Result<&'a serde_json::Value, ReleaseReadinessError> {
    let mut current = value;
    for segment in path.split('.') {
        current = match current {
            serde_json::Value::Object(map) => map
                .get(segment)
                .ok_or_else(|| malformed(format!("carrier is missing `{path}`")))?,
            serde_json::Value::Array(items) => {
                let index: usize = segment
                    .parse()
                    .map_err(|_| malformed(format!("carrier is missing `{path}`")))?;
                items
                    .get(index)
                    .ok_or_else(|| malformed(format!("carrier is missing `{path}`")))?
            }
            _ => return Err(malformed(format!("carrier is missing `{path}`"))),
        };
    }
    Ok(current)
}

fn json_bool(value: &serde_json::Value, path: &str) -> Result<bool, ReleaseReadinessError> {
    json_navigate(value, path)?
        .as_bool()
        .ok_or_else(|| malformed(format!("`{path}` is not a boolean")))
}

fn json_str_map(
    value: &serde_json::Value,
    path: &str,
) -> Result<BTreeMap<String, String>, ReleaseReadinessError> {
    let object = json_navigate(value, path)?
        .as_object()
        .ok_or_else(|| malformed(format!("`{path}` is not an object")))?;
    let mut out = BTreeMap::new();
    for (key, item) in object {
        let text = item
            .as_str()
            .ok_or_else(|| malformed(format!("`{path}.{key}` is not a string")))?;
        out.insert(key.clone(), text.to_owned());
    }
    Ok(out)
}

fn yaml_navigate<'a>(value: &'a Yaml, path: &str) -> Result<&'a Yaml, ReleaseReadinessError> {
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| malformed(format!("carrier is missing `{path}`")))?;
    }
    Ok(current)
}

fn yaml_str<'a>(value: &'a Yaml, path: &str) -> Result<&'a str, ReleaseReadinessError> {
    yaml_navigate(value, path)?
        .as_str()
        .ok_or_else(|| malformed(format!("`{path}` is not a string")))
}

fn yaml_optional_str(value: &Yaml, path: &str) -> Result<Option<String>, ReleaseReadinessError> {
    match yaml_navigate(value, path)? {
        Yaml::Null => Ok(None),
        Yaml::String(text) => Ok(Some(text.clone())),
        _ => Err(malformed(format!("`{path}` is neither a string nor null"))),
    }
}

fn yaml_str_seq(value: &Yaml, path: &str) -> Result<Vec<String>, ReleaseReadinessError> {
    let sequence = yaml_navigate(value, path)?
        .as_sequence()
        .ok_or_else(|| malformed(format!("`{path}` is not a sequence")))?;
    sequence
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed(format!("`{path}` carries a non-string element")))
        })
        .collect()
}

fn parse_yaml(bytes: &[u8], rel: &str) -> Result<Yaml, ReleaseReadinessError> {
    serde_yaml::from_slice(bytes)
        .map_err(|error| malformed(format!("{rel} is not valid YAML: {error}")))
}

fn parse_json(bytes: &[u8], rel: &str) -> Result<serde_json::Value, ReleaseReadinessError> {
    serde_json::from_slice(bytes)
        .map_err(|error| malformed(format!("{rel} is not valid JSON: {error}")))
}

/// Require the acceptance package's `mandatory_gates` to resolve to a non-empty
/// array of unique gate ids; the machine authorization is only meaningful when its
/// gate set is resolvable.
fn check_mandatory_gates(package: &serde_json::Value) -> Result<(), ReleaseReadinessError> {
    let gates = json_navigate(package, POINTER_PACKAGE_MANDATORY_GATES)?
        .as_array()
        .ok_or_else(|| malformed("`mandatory_gates` is not an array"))?;
    if gates.is_empty() {
        return Err(malformed("`mandatory_gates` is empty"));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for gate in gates {
        let gate_id = gate
            .get("gate_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| malformed("a `mandatory_gates` row has no string `gate_id`"))?;
        if !seen.insert(gate_id.to_owned()) {
            return Err(malformed(format!(
                "`mandatory_gates` carries a duplicate gate id `{gate_id}`"
            )));
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

// ───────────────────────────── derive ─────────────────────────────

/// Read the committed carriers and recompute the expectation the receipt is
/// validated against.
///
/// Every evidence sha256 is recomputed from the exact committed bytes; a declared
/// digest is never copied. The function fails closed on an unreadable carrier or
/// an unresolved pointer.
pub fn derive_release_readiness_expectation(
    root: &Path,
) -> Result<ReleaseReadinessExpectation, ReleaseReadinessError> {
    // ── the DATA authority: the three gates + the human record ──────────────
    let manifest_bytes = read_carrier(root, ACCEPTANCE_MANIFEST_REL)?;
    let manifest = parse_yaml(&manifest_bytes, ACCEPTANCE_MANIFEST_REL)?;
    let manifest_sha = sha256_prefixed(&manifest_bytes);

    let required_gates = yaml_str_seq(&manifest, POINTER_REQUIRED_GATES)?;
    if required_gates != RELEASE_GATE_ORDER {
        return Err(malformed(format!(
            "`{POINTER_REQUIRED_GATES}` declares {required_gates:?}, not {RELEASE_GATE_ORDER:?}"
        )));
    }

    let quality_status = yaml_str(&manifest, &format!("{POINTER_GATE_QUALITY}.status"))?.to_owned();
    let quality_effect =
        yaml_str(&manifest, &format!("{POINTER_GATE_QUALITY}.release_effect"))?.to_owned();
    let cost_status = yaml_str(&manifest, &format!("{POINTER_GATE_COST}.status"))?.to_owned();
    let cost_effect =
        yaml_str(&manifest, &format!("{POINTER_GATE_COST}.release_effect"))?.to_owned();
    let rights_status = yaml_str(&manifest, &format!("{POINTER_GATE_RIGHTS}.status"))?.to_owned();
    let rights_effect =
        yaml_str(&manifest, &format!("{POINTER_GATE_RIGHTS}.release_effect"))?.to_owned();

    let reviewer = yaml_optional_str(
        &manifest,
        &format!("{POINTER_HUMAN_RELEASE_REVIEW}.reviewer"),
    )?;
    let decision = yaml_str(
        &manifest,
        &format!("{POINTER_HUMAN_RELEASE_REVIEW}.decision"),
    )?
    .to_owned();
    let decision_record = yaml_str(
        &manifest,
        &format!("{POINTER_HUMAN_RELEASE_REVIEW}.decision_record"),
    )?
    .to_owned();
    let authorization_requires = yaml_str_seq(
        &manifest,
        &format!("{POINTER_HUMAN_RELEASE_REVIEW}.authorization_requires"),
    )?;

    // ── the machine authorization: the acceptance package ──────────────────
    let package_bytes = read_carrier(root, ACCEPTANCE_PACKAGE_REL)?;
    let package = parse_json(&package_bytes, ACCEPTANCE_PACKAGE_REL)?;
    let package_sha = sha256_prefixed(&package_bytes);
    check_mandatory_gates(&package)?;

    let s09_authorized = json_bool(
        &package,
        &format!("{POINTER_PACKAGE_AUTHORIZATION}.s09_authorized"),
    )?;
    let gsd_lifecycle_authorizes = json_bool(
        &package,
        &format!("{POINTER_PACKAGE_AUTHORIZATION}.gsd_lifecycle_authorizes"),
    )?;
    let human_release_recorded = json_bool(
        &package,
        &format!("{POINTER_PACKAGE_AUTHORIZATION}.human_release_recorded"),
    )?;
    let locks = json_str_map(&package, POINTER_PACKAGE_LOCKS)?;

    // ── the milestone carriers (resolve each pointer, recompute each sha) ───
    let layout_bytes = read_carrier(root, LAYOUT_BAKEOFF_REL)?;
    let layout = parse_yaml(&layout_bytes, LAYOUT_BAKEOFF_REL)?;
    yaml_navigate(&layout, POINTER_NATIVE_ROUTE_CORPUS)?;
    let layout_sha = sha256_prefixed(&layout_bytes);

    let candidates_bytes = read_carrier(root, THRESHOLD_CANDIDATES_REL)?;
    let candidates = parse_json(&candidates_bytes, THRESHOLD_CANDIDATES_REL)?;
    json_navigate(&candidates, POINTER_THRESHOLD_CANDIDATES)?;
    let candidates_sha = sha256_prefixed(&candidates_bytes);

    let compatibility_bytes = read_carrier(root, GROBID_COMPATIBILITY_REL)?;
    let compatibility = parse_yaml(&compatibility_bytes, GROBID_COMPATIBILITY_REL)?;
    yaml_navigate(&compatibility, POINTER_REQUIRED_SCOPE_DOWNGRADES)?;
    let compatibility_sha = sha256_prefixed(&compatibility_bytes);

    let governed_bytes = read_carrier(root, GOVERNED_RECORD_REL)?;
    let governed = parse_json(&governed_bytes, GOVERNED_RECORD_REL)?;
    json_navigate(&governed, POINTER_GOVERNED_RECORD)?;
    let governed_sha = sha256_prefixed(&governed_bytes);

    let candidate_bytes = read_carrier(root, INDEPENDENT_CANDIDATE_REL)?;
    let candidate = parse_json(&candidate_bytes, INDEPENDENT_CANDIDATE_REL)?;
    json_navigate(&candidate, POINTER_SERIES_IDENTITY)?;
    let candidate_sha = sha256_prefixed(&candidate_bytes);

    // ── assemble ────────────────────────────────────────────────────────────
    let release_gates = vec![
        ReleaseGateRow {
            gate_id: "quality".to_owned(),
            status: quality_status.clone(),
            release_effect: quality_effect,
            evidence: vec![
                CarrierBinding {
                    manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
                    pointer: POINTER_GATE_QUALITY.to_owned(),
                    revision_sha256: manifest_sha.clone(),
                },
                CarrierBinding {
                    manifest: LAYOUT_BAKEOFF_REL.to_owned(),
                    pointer: POINTER_NATIVE_ROUTE_CORPUS.to_owned(),
                    revision_sha256: layout_sha.clone(),
                },
                CarrierBinding {
                    manifest: THRESHOLD_CANDIDATES_REL.to_owned(),
                    pointer: POINTER_THRESHOLD_CANDIDATES.to_owned(),
                    revision_sha256: candidates_sha.clone(),
                },
                CarrierBinding {
                    manifest: ACCEPTANCE_PACKAGE_REL.to_owned(),
                    pointer: POINTER_PACKAGE_MANDATORY_GATES.to_owned(),
                    revision_sha256: package_sha.clone(),
                },
            ],
        },
        ReleaseGateRow {
            gate_id: "cost".to_owned(),
            status: cost_status.clone(),
            release_effect: cost_effect,
            evidence: vec![CarrierBinding {
                manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
                pointer: POINTER_COST_BOUNDS.to_owned(),
                revision_sha256: manifest_sha.clone(),
            }],
        },
        ReleaseGateRow {
            gate_id: "rights".to_owned(),
            status: rights_status.clone(),
            release_effect: rights_effect,
            evidence: vec![
                CarrierBinding {
                    manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
                    pointer: POINTER_GATE_RIGHTS.to_owned(),
                    revision_sha256: manifest_sha.clone(),
                },
                CarrierBinding {
                    manifest: GROBID_COMPATIBILITY_REL.to_owned(),
                    pointer: POINTER_REQUIRED_SCOPE_DOWNGRADES.to_owned(),
                    revision_sha256: compatibility_sha.clone(),
                },
            ],
        },
    ];

    let mut gate_status = BTreeMap::new();
    gate_status.insert("quality".to_owned(), quality_status);
    gate_status.insert("cost".to_owned(), cost_status);
    gate_status.insert("rights".to_owned(), rights_status);
    let human_decision = HumanDecision {
        reviewer,
        decision,
        decision_record,
        authorization_requires,
        gate_status,
    };

    let machine_authorization = MachineAuthorization {
        s09_authorized,
        gsd_lifecycle_authorizes,
        human_release_recorded,
        locks,
    };

    let milestone_evidence = vec![
        MilestoneEvidence {
            slice: "S03".to_owned(),
            manifest: LAYOUT_BAKEOFF_REL.to_owned(),
            pointer: POINTER_NATIVE_ROUTE_CORPUS.to_owned(),
            revision_sha256: layout_sha,
        },
        MilestoneEvidence {
            slice: "S05".to_owned(),
            manifest: GROBID_COMPATIBILITY_REL.to_owned(),
            pointer: POINTER_REQUIRED_SCOPE_DOWNGRADES.to_owned(),
            revision_sha256: compatibility_sha,
        },
        MilestoneEvidence {
            slice: "S10".to_owned(),
            manifest: GOVERNED_RECORD_REL.to_owned(),
            pointer: POINTER_GOVERNED_RECORD.to_owned(),
            revision_sha256: governed_sha,
        },
        MilestoneEvidence {
            slice: "S11".to_owned(),
            manifest: INDEPENDENT_CANDIDATE_REL.to_owned(),
            pointer: POINTER_SERIES_IDENTITY.to_owned(),
            revision_sha256: candidate_sha,
        },
    ];

    Ok(ReleaseReadinessExpectation {
        release_gates,
        human_decision,
        machine_authorization,
        milestone_evidence,
    })
}

// ───────────────────────────── shape checks ─────────────────────────────

fn check_binding(binding: &CarrierBinding) -> Result<(), ReleaseReadinessError> {
    if !is_repo_relative_path(&binding.manifest)
        || binding.pointer.trim().is_empty()
        || !is_sha256_digest(&binding.revision_sha256)
    {
        return Err(ReleaseReadinessError::UnboundEvidence {
            detail: format!(
                "{}#{} ({})",
                binding.manifest, binding.pointer, binding.revision_sha256
            ),
        });
    }
    Ok(())
}

fn check_gates(gates: &[ReleaseGateRow]) -> Result<(), ReleaseReadinessError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut evidence_seen: BTreeSet<(String, String)> = BTreeSet::new();
    for gate in gates {
        if !RELEASE_GATE_ORDER.contains(&gate.gate_id.as_str()) {
            return Err(ReleaseReadinessError::ReleaseGateUnexpected {
                gate_id: gate.gate_id.clone(),
            });
        }
        if !seen.insert(gate.gate_id.clone()) {
            return Err(ReleaseReadinessError::DuplicateReleaseGate {
                gate_id: gate.gate_id.clone(),
            });
        }
        for (field, value) in [
            ("status", &gate.status),
            ("release_effect", &gate.release_effect),
        ] {
            if value.trim().is_empty() {
                return Err(ReleaseReadinessError::ReleaseGateFieldEmpty {
                    gate_id: gate.gate_id.clone(),
                    field: field.to_owned(),
                });
            }
        }
        if gate.evidence.is_empty() {
            return Err(ReleaseReadinessError::ReleaseGateEvidenceEmpty {
                gate_id: gate.gate_id.clone(),
            });
        }
        for binding in &gate.evidence {
            check_binding(binding)?;
            let key = (binding.manifest.clone(), binding.pointer.clone());
            if !evidence_seen.insert(key.clone()) {
                return Err(ReleaseReadinessError::DuplicateEvidence {
                    manifest: key.0,
                    pointer: key.1,
                });
            }
        }
    }
    for gate_id in RELEASE_GATE_ORDER {
        if !seen.contains(gate_id) {
            return Err(ReleaseReadinessError::ReleaseGateMissing {
                gate_id: gate_id.to_owned(),
            });
        }
    }
    Ok(())
}

fn check_human_decision(human: &HumanDecision) -> Result<(), ReleaseReadinessError> {
    if let Some(reviewer) = &human.reviewer {
        return Err(ReleaseReadinessError::HumanDecisionClaimed {
            field: "reviewer".to_owned(),
            found: reviewer.clone(),
        });
    }
    if human.decision != RELEASE_READINESS_HUMAN_DECISION_PENDING {
        return Err(ReleaseReadinessError::HumanDecisionClaimed {
            field: "decision".to_owned(),
            found: human.decision.clone(),
        });
    }
    if human.decision_record.trim().is_empty() {
        return Err(ReleaseReadinessError::DecisionRecordEmpty);
    }
    if human.authorization_requires.is_empty() {
        return Err(ReleaseReadinessError::HumanDecisionClaimed {
            field: "authorization_requires".to_owned(),
            found: "<empty>".to_owned(),
        });
    }
    Ok(())
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), ReleaseReadinessError> {
    for (lock, expected) in REQUIRED_LOCKS {
        match locks.get(lock) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(ReleaseReadinessError::LockOpened {
                    lock: lock.to_owned(),
                    expected: expected.to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(ReleaseReadinessError::LockOpened {
                    lock: lock.to_owned(),
                    expected: expected.to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    for (lock, value) in locks {
        if !REQUIRED_LOCKS.iter().any(|(name, _)| name == lock) {
            return Err(ReleaseReadinessError::LockOpened {
                lock: lock.clone(),
                expected: "<absent>".to_owned(),
                found: value.clone(),
            });
        }
    }
    Ok(())
}

fn check_machine_authorization(
    machine: &MachineAuthorization,
) -> Result<(), ReleaseReadinessError> {
    for (field, value) in [
        ("s09_authorized", machine.s09_authorized),
        ("gsd_lifecycle_authorizes", machine.gsd_lifecycle_authorizes),
        ("human_release_recorded", machine.human_release_recorded),
    ] {
        if value {
            return Err(ReleaseReadinessError::MachineAuthorizationClaimed {
                field: field.to_owned(),
            });
        }
    }
    check_locks(&machine.locks)
}

fn check_milestone_evidence(rows: &[MilestoneEvidence]) -> Result<(), ReleaseReadinessError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for row in rows {
        if !MILESTONE_SLICE_ORDER.contains(&row.slice.as_str()) {
            return Err(ReleaseReadinessError::MilestoneSliceUnexpected {
                slice: row.slice.clone(),
            });
        }
        if !seen.insert(row.slice.clone()) {
            return Err(ReleaseReadinessError::DuplicateMilestoneSlice {
                slice: row.slice.clone(),
            });
        }
        if !is_repo_relative_path(&row.manifest)
            || row.pointer.trim().is_empty()
            || !is_sha256_digest(&row.revision_sha256)
        {
            return Err(ReleaseReadinessError::UnboundMilestoneEvidence {
                detail: format!("{}#{} ({})", row.manifest, row.pointer, row.revision_sha256),
            });
        }
    }
    for slice in MILESTONE_SLICE_ORDER {
        if !seen.contains(slice) {
            return Err(ReleaseReadinessError::MilestoneSliceMissing {
                slice: slice.to_owned(),
            });
        }
    }
    Ok(())
}

fn check_gate_status_alignment(
    gates: &[ReleaseGateRow],
    human: &HumanDecision,
) -> Result<(), ReleaseReadinessError> {
    for gate in gates {
        match human.gate_status.get(&gate.gate_id) {
            Some(status) if status == &gate.status => {}
            Some(status) => {
                return Err(ReleaseReadinessError::GateStatusAlignmentDrift {
                    gate_id: gate.gate_id.clone(),
                    expected: gate.status.clone(),
                    found: status.clone(),
                });
            }
            None => {
                return Err(ReleaseReadinessError::GateStatusAlignmentDrift {
                    gate_id: gate.gate_id.clone(),
                    expected: gate.status.clone(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

// ───────────────────────────── drift checks ─────────────────────────────

fn check_gate_drift(
    expected: &[ReleaseGateRow],
    actual: &[ReleaseGateRow],
) -> Result<(), ReleaseReadinessError> {
    for want in expected {
        let found = actual
            .iter()
            .find(|gate| gate.gate_id == want.gate_id)
            .ok_or_else(|| ReleaseReadinessError::ReleaseGateMissing {
                gate_id: want.gate_id.clone(),
            })?;
        for (field, want_value, found_value) in [
            ("status", &want.status, &found.status),
            (
                "release_effect",
                &want.release_effect,
                &found.release_effect,
            ),
        ] {
            if want_value != found_value {
                return Err(ReleaseReadinessError::ReleaseGateDrift {
                    gate_id: want.gate_id.clone(),
                    field: field.to_owned(),
                    expected: want_value.clone(),
                    found: found_value.clone(),
                });
            }
        }
        if sorted_bindings(want.evidence.clone()) != sorted_bindings(found.evidence.clone()) {
            return Err(ReleaseReadinessError::EvidenceDrift {
                detail: format!(
                    "gate `{}` evidence differs from the re-derived bindings",
                    want.gate_id
                ),
            });
        }
    }
    Ok(())
}

fn check_human_drift(
    expected: &HumanDecision,
    actual: &HumanDecision,
) -> Result<(), ReleaseReadinessError> {
    let drift =
        |field: &str, want: String, found: String| ReleaseReadinessError::HumanDecisionDrift {
            field: field.to_owned(),
            expected: want,
            found,
        };
    if expected.reviewer != actual.reviewer {
        return Err(drift(
            "reviewer",
            format!("{:?}", expected.reviewer),
            format!("{:?}", actual.reviewer),
        ));
    }
    if expected.decision != actual.decision {
        return Err(drift(
            "decision",
            expected.decision.clone(),
            actual.decision.clone(),
        ));
    }
    if expected.decision_record != actual.decision_record {
        return Err(drift(
            "decision_record",
            expected.decision_record.clone(),
            actual.decision_record.clone(),
        ));
    }
    if expected.authorization_requires != actual.authorization_requires {
        return Err(drift(
            "authorization_requires",
            format!("{:?}", expected.authorization_requires),
            format!("{:?}", actual.authorization_requires),
        ));
    }
    if expected.gate_status != actual.gate_status {
        return Err(drift(
            "gate_status",
            format!("{:?}", expected.gate_status),
            format!("{:?}", actual.gate_status),
        ));
    }
    Ok(())
}

fn check_milestone_drift(
    expected: &[MilestoneEvidence],
    actual: &[MilestoneEvidence],
) -> Result<(), ReleaseReadinessError> {
    for want in expected {
        let found = actual
            .iter()
            .find(|row| row.slice == want.slice)
            .ok_or_else(|| ReleaseReadinessError::MilestoneSliceMissing {
                slice: want.slice.clone(),
            })?;
        if found.manifest != want.manifest
            || found.pointer != want.pointer
            || found.revision_sha256 != want.revision_sha256
        {
            return Err(ReleaseReadinessError::MilestoneEvidenceDrift {
                slice: want.slice.clone(),
                expected: format!(
                    "{}#{}@{}",
                    want.manifest, want.pointer, want.revision_sha256
                ),
                found: format!(
                    "{}#{}@{}",
                    found.manifest, found.pointer, found.revision_sha256
                ),
            });
        }
    }
    Ok(())
}

// ───────────────────────────── builder ─────────────────────────────

/// Build the release-readiness receipt from the recomputed expectation, refusing
/// every contract violation by name.
///
/// The envelope (`claim_scope`, `thresholds`, `product_coverage_effect`,
/// `nothing_here_authorizes_a_release`) is set from the module constants, never
/// taken from the input, so a caller cannot widen the claim. The assembled receipt
/// is validated before it is returned.
pub fn build_release_readiness_receipt(
    expectation: &ReleaseReadinessExpectation,
) -> Result<ReleaseReadinessReceipt, ReleaseReadinessError> {
    let receipt = ReleaseReadinessReceipt {
        schema_version: RELEASE_READINESS_SCHEMA_VERSION.to_owned(),
        claim_scope: RELEASE_READINESS_CLAIM_SCOPE.to_owned(),
        product_coverage_effect: RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT.to_owned(),
        thresholds: RELEASE_READINESS_THRESHOLDS.to_owned(),
        nothing_here_authorizes_a_release: true,
        release_gates: expectation.release_gates.clone(),
        human_decision: expectation.human_decision.clone(),
        machine_authorization: expectation.machine_authorization.clone(),
        milestone_evidence: expectation.milestone_evidence.clone(),
        redaction: Redaction::clean(),
    };
    validate_release_readiness_receipt(expectation, &receipt)?;
    Ok(receipt)
}

// ───────────────────────────── validator ─────────────────────────────

/// Re-read a release-readiness receipt against the committed carriers. This is
/// the independent half of the contract: it refuses the receipt-level mutations a
/// builder cannot express.
pub fn validate_release_readiness_receipt(
    expectation: &ReleaseReadinessExpectation,
    receipt: &ReleaseReadinessReceipt,
) -> Result<(), ReleaseReadinessError> {
    // ── the envelope ───────────────────────────────────────────────────────
    if receipt.schema_version != RELEASE_READINESS_SCHEMA_VERSION {
        return Err(ReleaseReadinessError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: RELEASE_READINESS_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.claim_scope != RELEASE_READINESS_CLAIM_SCOPE {
        return Err(ReleaseReadinessError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT {
        return Err(ReleaseReadinessError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }
    if receipt.thresholds != RELEASE_READINESS_THRESHOLDS {
        return Err(ReleaseReadinessError::ThresholdClaimed {
            found: receipt.thresholds.clone(),
        });
    }
    if !receipt.nothing_here_authorizes_a_release {
        return Err(ReleaseReadinessError::NothingAuthorizesFlagFalse);
    }
    if receipt.redaction != Redaction::clean() {
        return Err(ReleaseReadinessError::RedactionLeak {
            marker: "redaction flag set".to_owned(),
        });
    }

    // ── the shape ──────────────────────────────────────────────────────────
    check_gates(&receipt.release_gates)?;
    check_human_decision(&receipt.human_decision)?;
    check_machine_authorization(&receipt.machine_authorization)?;
    check_milestone_evidence(&receipt.milestone_evidence)?;
    check_gate_status_alignment(&receipt.release_gates, &receipt.human_decision)?;

    // ── the drift ──────────────────────────────────────────────────────────
    check_gate_drift(&expectation.release_gates, &receipt.release_gates)?;
    check_human_drift(&expectation.human_decision, &receipt.human_decision)?;
    check_milestone_drift(&expectation.milestone_evidence, &receipt.milestone_evidence)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn real_expectation() -> ReleaseReadinessExpectation {
        derive_release_readiness_expectation(&repo_root())
            .expect("the committed carriers derive a release-readiness expectation")
    }

    fn valid_receipt() -> (ReleaseReadinessExpectation, ReleaseReadinessReceipt) {
        let expectation = real_expectation();
        let receipt = build_release_readiness_receipt(&expectation)
            .expect("the receipt builds and validates");
        (expectation, receipt)
    }

    fn gate<'a>(gates: &'a [ReleaseGateRow], gate_id: &str) -> &'a ReleaseGateRow {
        gates
            .iter()
            .find(|row| row.gate_id == gate_id)
            .expect("the gate is present")
    }

    #[test]
    fn expectation_derives_from_committed_carriers() {
        let expectation = real_expectation();
        assert_eq!(expectation.release_gates.len(), 3);
        let ids: Vec<&str> = expectation
            .release_gates
            .iter()
            .map(|row| row.gate_id.as_str())
            .collect();
        assert_eq!(ids, RELEASE_GATE_ORDER.to_vec());
        assert_eq!(
            gate(&expectation.release_gates, "quality").status,
            "blocked"
        );
        assert_eq!(
            gate(&expectation.release_gates, "cost").status,
            "bounded_not_measured"
        );
        assert_eq!(gate(&expectation.release_gates, "rights").status, "blocked");
        for row in &expectation.release_gates {
            assert!(!row.evidence.is_empty());
            for binding in &row.evidence {
                assert!(is_repo_relative_path(&binding.manifest));
                assert!(!binding.pointer.trim().is_empty());
                assert!(is_sha256_digest(&binding.revision_sha256));
            }
        }
        assert_eq!(expectation.human_decision.reviewer, None);
        assert_eq!(
            expectation.human_decision.decision,
            RELEASE_READINESS_HUMAN_DECISION_PENDING
        );
        assert!(!expectation.human_decision.decision_record.trim().is_empty());
        assert_eq!(expectation.human_decision.authorization_requires.len(), 5);
        assert!(!expectation.machine_authorization.s09_authorized);
        assert!(!expectation.machine_authorization.gsd_lifecycle_authorizes);
        assert!(!expectation.machine_authorization.human_release_recorded);
        assert_eq!(
            expectation.machine_authorization.locks.len(),
            REQUIRED_LOCKS.len()
        );
        let slices: Vec<&str> = expectation
            .milestone_evidence
            .iter()
            .map(|row| row.slice.as_str())
            .collect();
        assert_eq!(slices, MILESTONE_SLICE_ORDER.to_vec());
    }

    #[test]
    fn build_validate_roundtrip_from_committed_carriers() {
        let (expectation, receipt) = valid_receipt();
        assert_eq!(receipt.schema_version, RELEASE_READINESS_SCHEMA_VERSION);
        assert_eq!(receipt.claim_scope, RELEASE_READINESS_CLAIM_SCOPE);
        assert_eq!(
            receipt.product_coverage_effect,
            RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT
        );
        assert_eq!(receipt.thresholds, RELEASE_READINESS_THRESHOLDS);
        assert!(receipt.nothing_here_authorizes_a_release);
        assert_eq!(receipt.redaction, Redaction::clean());

        let bytes = receipt.artifact_json_bytes().expect("serializes redacted");
        assert!(bytes.ends_with(b"\n"));
        let parsed =
            ReleaseReadinessReceipt::parse_json_bytes(&bytes).expect("parses canonical bytes");
        assert_eq!(parsed, receipt);
        validate_release_readiness_receipt(&expectation, &parsed)
            .expect("the parsed receipt re-validates");
    }

    #[test]
    fn missing_release_gate_is_refused() {
        let mut expectation = real_expectation();
        expectation
            .release_gates
            .retain(|row| row.gate_id != "cost");
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ReleaseGateMissing { gate_id } if gate_id == "cost"
        ));
    }

    #[test]
    fn extra_release_gate_is_refused() {
        let mut expectation = real_expectation();
        expectation.release_gates.push(ReleaseGateRow {
            gate_id: "speed".to_owned(),
            status: "met".to_owned(),
            release_effect: "does_not_authorize_anything".to_owned(),
            evidence: vec![CarrierBinding {
                manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
                pointer: POINTER_GATE_QUALITY.to_owned(),
                revision_sha256: sha256_prefixed(b"speed"),
            }],
        });
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ReleaseGateUnexpected { gate_id } if gate_id == "speed"
        ));
    }

    #[test]
    fn duplicate_release_gate_is_refused() {
        let mut expectation = real_expectation();
        let duplicate = gate(&expectation.release_gates, "quality").clone();
        expectation.release_gates.push(duplicate);
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::DuplicateReleaseGate { gate_id } if gate_id == "quality"
        ));
    }

    #[test]
    fn gate_with_empty_evidence_is_refused() {
        let mut expectation = real_expectation();
        for row in &mut expectation.release_gates {
            if row.gate_id == "rights" {
                row.evidence.clear();
            }
        }
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ReleaseGateEvidenceEmpty { gate_id } if gate_id == "rights"
        ));
    }

    #[test]
    fn unbound_evidence_binding_is_refused() {
        for mutate in [
            |binding: &mut CarrierBinding| binding.manifest = "/absolute/path.yaml".to_owned(),
            |binding: &mut CarrierBinding| binding.pointer = String::new(),
            |binding: &mut CarrierBinding| binding.revision_sha256 = "not-a-digest".to_owned(),
        ] {
            let mut expectation = real_expectation();
            for row in &mut expectation.release_gates {
                if row.gate_id == "quality" {
                    mutate(&mut row.evidence[0]);
                }
            }
            let error = build_release_readiness_receipt(&expectation).unwrap_err();
            assert!(matches!(
                error,
                ReleaseReadinessError::UnboundEvidence { .. }
            ));
        }
    }

    #[test]
    fn duplicate_evidence_binding_is_refused() {
        let mut expectation = real_expectation();
        for row in &mut expectation.release_gates {
            if row.gate_id == "quality" {
                let duplicate = row.evidence[0].clone();
                row.evidence.push(duplicate);
            }
        }
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::DuplicateEvidence { .. }
        ));
    }

    #[test]
    fn claimed_human_decision_is_refused() {
        let mut expectation = real_expectation();
        expectation.human_decision.reviewer = Some("Ada".to_owned());
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::HumanDecisionClaimed { field, .. } if field == "reviewer"
        ));

        let mut expectation = real_expectation();
        expectation.human_decision.decision = "approved".to_owned();
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::HumanDecisionClaimed { field, .. } if field == "decision"
        ));
    }

    #[test]
    fn empty_decision_record_is_refused() {
        let mut expectation = real_expectation();
        expectation.human_decision.decision_record = String::new();
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(error, ReleaseReadinessError::DecisionRecordEmpty));
    }

    #[test]
    fn opened_lock_is_refused() {
        let mut expectation = real_expectation();
        expectation
            .machine_authorization
            .locks
            .insert("import_eligible".to_owned(), "true".to_owned());
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::LockOpened { lock, .. } if lock == "import_eligible"
        ));
    }

    #[test]
    fn missing_lock_is_refused() {
        let mut expectation = real_expectation();
        expectation.machine_authorization.locks.remove("deployed");
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::LockOpened { lock, found, .. } if lock == "deployed" && found == "<absent>"
        ));
    }

    #[test]
    fn machine_authorization_claimed_is_refused() {
        for field in [
            "s09_authorized",
            "gsd_lifecycle_authorizes",
            "human_release_recorded",
        ] {
            let mut expectation = real_expectation();
            match field {
                "s09_authorized" => expectation.machine_authorization.s09_authorized = true,
                "gsd_lifecycle_authorizes" => {
                    expectation.machine_authorization.gsd_lifecycle_authorizes = true;
                }
                _ => expectation.machine_authorization.human_release_recorded = true,
            }
            let error = build_release_readiness_receipt(&expectation).unwrap_err();
            assert!(matches!(
                error,
                ReleaseReadinessError::MachineAuthorizationClaimed { field: found } if found == field
            ));
        }
    }

    #[test]
    fn missing_milestone_slice_is_refused() {
        let mut expectation = real_expectation();
        expectation
            .milestone_evidence
            .retain(|row| row.slice != "S11");
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::MilestoneSliceMissing { slice } if slice == "S11"
        ));
    }

    #[test]
    fn duplicate_milestone_slice_is_refused() {
        let mut expectation = real_expectation();
        let duplicate = expectation.milestone_evidence[0].clone();
        expectation.milestone_evidence.push(duplicate);
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::DuplicateMilestoneSlice { slice } if slice == "S03"
        ));
    }

    #[test]
    fn unexpected_milestone_slice_is_refused() {
        let mut expectation = real_expectation();
        expectation.milestone_evidence[0].slice = "S99".to_owned();
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::MilestoneSliceUnexpected { slice } if slice == "S99"
        ));
    }

    #[test]
    fn unbound_milestone_evidence_is_refused() {
        let mut expectation = real_expectation();
        expectation.milestone_evidence[0].revision_sha256 = "sha256:0".to_owned();
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::UnboundMilestoneEvidence { .. }
        ));
    }

    #[test]
    fn schema_drift_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.schema_version = "release-readiness.v2".to_owned();
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::SchemaDrift { field, .. } if field == "schema_version"
        ));
    }

    #[test]
    fn widened_claim_scope_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.claim_scope = "release_authorized".to_owned();
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ClaimScopeDrift { .. }
        ));
    }

    #[test]
    fn claimed_product_coverage_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.product_coverage_effect = "full".to_owned();
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ProductCoverageClaimed { .. }
        ));
    }

    #[test]
    fn accepted_threshold_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.thresholds = "accepted".to_owned();
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ThresholdClaimed { .. }
        ));
    }

    #[test]
    fn nothing_authorizes_flag_false_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.nothing_here_authorizes_a_release = false;
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::NothingAuthorizesFlagFalse
        ));
    }

    #[test]
    fn redaction_flag_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.redaction.raw_text = true;
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(error, ReleaseReadinessError::RedactionLeak { .. }));
    }

    #[test]
    fn gate_status_alignment_drift_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt
            .human_decision
            .gate_status
            .insert("quality".to_owned(), "met".to_owned());
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::GateStatusAlignmentDrift { gate_id, .. } if gate_id == "quality"
        ));
    }

    #[test]
    fn release_gate_field_empty_is_refused() {
        let mut expectation = real_expectation();
        for row in &mut expectation.release_gates {
            if row.gate_id == "quality" {
                row.status = String::new();
            }
        }
        let error = build_release_readiness_receipt(&expectation).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ReleaseGateFieldEmpty { gate_id, field } if gate_id == "quality" && field == "status"
        ));
    }

    #[test]
    fn release_gate_drift_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        for row in &mut receipt.release_gates {
            if row.gate_id == "quality" {
                row.status = "met".to_owned();
            }
        }
        receipt
            .human_decision
            .gate_status
            .insert("quality".to_owned(), "met".to_owned());
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::ReleaseGateDrift { gate_id, field, .. } if gate_id == "quality" && field == "status"
        ));
    }

    #[test]
    fn stale_evidence_sha_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        for row in &mut receipt.release_gates {
            if row.gate_id == "quality" {
                row.evidence[0].revision_sha256 = sha256_prefixed(b"stale");
            }
        }
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(error, ReleaseReadinessError::EvidenceDrift { .. }));
    }

    #[test]
    fn human_decision_drift_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        receipt.human_decision.decision_record = "doc/review/other.md".to_owned();
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::HumanDecisionDrift { field, .. } if field == "decision_record"
        ));
    }

    #[test]
    fn milestone_evidence_drift_is_refused() {
        let (expectation, mut receipt) = valid_receipt();
        for row in &mut receipt.milestone_evidence {
            if row.slice == "S03" {
                row.revision_sha256 = sha256_prefixed(b"drifted");
            }
        }
        let error = validate_release_readiness_receipt(&expectation, &receipt).unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::MilestoneEvidenceDrift { slice, .. } if slice == "S03"
        ));
    }

    #[test]
    fn unreadable_carrier_fails_closed() {
        let error = derive_release_readiness_expectation(std::path::Path::new("/nonexistent/root"))
            .unwrap_err();
        assert!(matches!(
            error,
            ReleaseReadinessError::MalformedCarrier { .. }
        ));
    }

    #[test]
    fn redaction_sweep_refuses_a_leaked_marker() {
        assert!(assert_release_readiness_redacted("{\"host\":\"localhost\"}").is_err());
        assert!(assert_release_readiness_redacted("{\"ok\":true}").is_ok());
    }
}
