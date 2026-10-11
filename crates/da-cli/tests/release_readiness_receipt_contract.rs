//! M516-th0oby / S12 / T03 — the hermetic contract for the release-readiness
//! receipt.
//!
//! WHY THIS TEST EXISTS: a validator only the builder exercises is not an
//! independent check. T01 built the receipt and T02 published it, but nothing
//! replayed the committed artifact from committed bytes or proved the validator
//! still refuses a mutation. This test closes that surface:
//!
//! * [`release_readiness_receipt_replays_byte_for_byte_from_committed_carriers`]
//!   reads `artifacts/parse_compat/grobid_retirement_release_readiness.json`,
//!   re-derives the expectation from the committed carriers, rebuilds the receipt
//!   and asserts the rebuilt bytes equal the committed bytes **exactly** — a
//!   receipt that drifted from its carriers fails closed.
//! * One `#[test]` per named refusal asserts the exact [`ReleaseReadinessError`]
//!   variant: a dropped / extra / duplicate gate, a gate with no evidence, an
//!   unbound or stale evidence binding, a claimed human decision, an opened lock,
//!   a claimed machine authorization, a widened claim scope, an accepted
//!   threshold, a claimed product coverage, a false nothing-authorizes flag, a
//!   dropped milestone slice and a redaction leak.
//! * [`frozen_carriers_are_byte_unchanged`] recomputes the sha256 of the four
//!   frozen carriers and compares each to the binding the receipt (or the carrier
//!   it binds) declares.
//!
//! The test is deliberately **ungated** (no `#![cfg(feature = ...)]`): it needs
//! neither `pdf-layout` nor `pdf-classifier`, so it compiles and runs in the
//! default feature set.

use da_cli::native_family_receipt::{CarrierBinding, REQUIRED_LOCKS, Redaction, sha256_prefixed};
use da_cli::release_readiness_receipt::{
    ACCEPTANCE_MANIFEST_REL, ACCEPTANCE_PACKAGE_REL, DEFAULT_RELEASE_READINESS_ARTIFACT_REL,
    MILESTONE_SLICE_ORDER, MilestoneEvidence, POINTER_GATE_QUALITY, RELEASE_GATE_ORDER,
    RELEASE_READINESS_CLAIM_SCOPE, RELEASE_READINESS_HUMAN_DECISION_PENDING,
    RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT, RELEASE_READINESS_SCHEMA_VERSION,
    RELEASE_READINESS_THRESHOLDS, ReleaseGateRow, ReleaseReadinessError,
    ReleaseReadinessExpectation, ReleaseReadinessReceipt, build_release_readiness_receipt,
    derive_release_readiness_expectation, validate_release_readiness_receipt,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// The M513 release-decision package. The receipt does not bind it directly, but
/// the acceptance package the receipt binds does — at pointer `remainders`.
const FROZEN_RELEASE_DECISION_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_release_decision_package.json";

/// The S09 retirement-readiness receipt, frozen by the S09 contract test. S12
/// publishes beside it and must never rewrite it. Its digest is pinned here
/// because no committed carrier binds it; the pin freezes the bytes at S12
/// authoring so a silent rewrite of the S09 artifact fails this test.
const FROZEN_S09_READINESS_REL: &str = "artifacts/parse_compat/grobid_retirement_readiness.json";
const FROZEN_S09_READINESS_SHA256: &str =
    "sha256:5da9943dbb52d55af8f586c385e5ac8122a2c47cbf74fbd4588e01671f5199b6";

// ───────────────────────────── helpers ─────────────────────────────

/// Resolve the repository root the way the sibling da-cli contract tests do.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate lives two levels under the repository root")
}

fn read(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

fn committed_receipt(root: &Path) -> Result<ReleaseReadinessReceipt, Box<dyn Error>> {
    let bytes = read(root, DEFAULT_RELEASE_READINESS_ARTIFACT_REL)?;
    Ok(ReleaseReadinessReceipt::parse_json_bytes(&bytes)?)
}

fn gate_mut<'a>(gates: &'a mut [ReleaseGateRow], gate_id: &str) -> &'a mut ReleaseGateRow {
    gates
        .iter_mut()
        .find(|row| row.gate_id == gate_id)
        .expect("the gate is present in the committed receipt")
}

fn milestone_mut<'a>(rows: &'a mut [MilestoneEvidence], slice: &str) -> &'a mut MilestoneEvidence {
    rows.iter_mut()
        .find(|row| row.slice == slice)
        .expect("the milestone slice is present in the committed receipt")
}

/// The exact variant name of a refusal, so a test asserts the *named* refusal and
/// not merely "some error".
fn variant_name(error: &ReleaseReadinessError) -> &'static str {
    use ReleaseReadinessError as E;
    match error {
        E::MalformedCarrier { .. } => "MalformedCarrier",
        E::SchemaDrift { .. } => "SchemaDrift",
        E::ClaimScopeDrift { .. } => "ClaimScopeDrift",
        E::ProductCoverageClaimed { .. } => "ProductCoverageClaimed",
        E::ThresholdClaimed { .. } => "ThresholdClaimed",
        E::NothingAuthorizesFlagFalse => "NothingAuthorizesFlagFalse",
        E::ReleaseGateMissing { .. } => "ReleaseGateMissing",
        E::ReleaseGateUnexpected { .. } => "ReleaseGateUnexpected",
        E::DuplicateReleaseGate { .. } => "DuplicateReleaseGate",
        E::ReleaseGateEvidenceEmpty { .. } => "ReleaseGateEvidenceEmpty",
        E::ReleaseGateFieldEmpty { .. } => "ReleaseGateFieldEmpty",
        E::UnboundEvidence { .. } => "UnboundEvidence",
        E::DuplicateEvidence { .. } => "DuplicateEvidence",
        E::HumanDecisionClaimed { .. } => "HumanDecisionClaimed",
        E::DecisionRecordEmpty => "DecisionRecordEmpty",
        E::LockOpened { .. } => "LockOpened",
        E::MachineAuthorizationClaimed { .. } => "MachineAuthorizationClaimed",
        E::MilestoneSliceMissing { .. } => "MilestoneSliceMissing",
        E::DuplicateMilestoneSlice { .. } => "DuplicateMilestoneSlice",
        E::MilestoneSliceUnexpected { .. } => "MilestoneSliceUnexpected",
        E::UnboundMilestoneEvidence { .. } => "UnboundMilestoneEvidence",
        E::ReleaseGateDrift { .. } => "ReleaseGateDrift",
        E::GateStatusAlignmentDrift { .. } => "GateStatusAlignmentDrift",
        E::HumanDecisionDrift { .. } => "HumanDecisionDrift",
        E::MilestoneEvidenceDrift { .. } => "MilestoneEvidenceDrift",
        E::EvidenceDrift { .. } => "EvidenceDrift",
        E::RedactionLeak { .. } => "RedactionLeak",
    }
}

/// Assert the validator refuses `receipt` with exactly `variant`.
fn refuse(
    expectation: &ReleaseReadinessExpectation,
    receipt: &ReleaseReadinessReceipt,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_release_readiness_receipt(expectation, receipt) {
        Ok(()) => Err(format!("the mutation was admitted; expected refusal `{variant}`").into()),
        Err(error) => {
            let found = variant_name(&error);
            if found == variant {
                Ok(())
            } else {
                Err(format!("the mutation was refused as `{found}`, expected `{variant}`").into())
            }
        }
    }
}

/// Every sha256 the receipt binds, keyed by the manifest it binds.
fn receipt_bound_digests(receipt: &ReleaseReadinessReceipt) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for gate in &receipt.release_gates {
        for binding in &gate.evidence {
            out.entry(binding.manifest.clone())
                .or_default()
                .insert(binding.revision_sha256.clone());
        }
    }
    for row in &receipt.milestone_evidence {
        out.entry(row.manifest.clone())
            .or_default()
            .insert(row.revision_sha256.clone());
    }
    out
}

/// Find the `revision_sha256` of the first evidence row binding `manifest`.
fn find_binding_sha(value: &serde_json::Value, manifest: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("manifest").and_then(serde_json::Value::as_str) == Some(manifest) {
                if let Some(sha) = map
                    .get("revision_sha256")
                    .and_then(serde_json::Value::as_str)
                {
                    return Some(sha.to_owned());
                }
            }
            map.values()
                .find_map(|child| find_binding_sha(child, manifest))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|child| find_binding_sha(child, manifest)),
        _ => None,
    }
}

fn expectation_and_receipt()
-> Result<(ReleaseReadinessExpectation, ReleaseReadinessReceipt), Box<dyn Error>> {
    let root = repo_root();
    let expectation = derive_release_readiness_expectation(&root)?;
    let receipt = committed_receipt(&root)?;
    Ok((expectation, receipt))
}

// ───────────────────────────── replay ─────────────────────────────

/// Test 1 — replay: the committed artifact is the builder's own canonical
/// re-serialization of the carriers, byte for byte.
#[test]
fn release_readiness_receipt_replays_byte_for_byte_from_committed_carriers()
-> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let expectation = derive_release_readiness_expectation(&root)?;
    let committed_bytes = read(&root, DEFAULT_RELEASE_READINESS_ARTIFACT_REL)?;
    let committed = ReleaseReadinessReceipt::parse_json_bytes(&committed_bytes)?;

    // The committed artifact validates against the re-derived expectation…
    validate_release_readiness_receipt(&expectation, &committed)?;

    // …and the committed bytes are exactly what the builder re-emits. A receipt
    // that drifted from its carriers would fail here.
    let rebuilt = build_release_readiness_receipt(&expectation)?;
    assert_eq!(
        rebuilt.artifact_json_bytes()?,
        committed_bytes,
        "the committed receipt is not the builder's canonical re-serialization"
    );

    // ── the envelope is the preparation-only projection ────────────────────
    assert_eq!(committed.schema_version, RELEASE_READINESS_SCHEMA_VERSION);
    assert_eq!(committed.claim_scope, RELEASE_READINESS_CLAIM_SCOPE);
    assert_eq!(
        committed.product_coverage_effect,
        RELEASE_READINESS_PRODUCT_COVERAGE_EFFECT
    );
    assert_eq!(committed.thresholds, RELEASE_READINESS_THRESHOLDS);
    assert!(committed.nothing_here_authorizes_a_release);
    assert_eq!(committed.redaction, Redaction::clean());

    // ── exactly three gates, in canonical order, each with evidence ────────
    let ids: Vec<&str> = committed
        .release_gates
        .iter()
        .map(|row| row.gate_id.as_str())
        .collect();
    assert_eq!(ids, RELEASE_GATE_ORDER.to_vec());
    for row in &committed.release_gates {
        assert!(
            !row.evidence.is_empty(),
            "gate `{}` has no evidence",
            row.gate_id
        );
        for binding in &row.evidence {
            assert!(binding.revision_sha256.starts_with("sha256:"));
        }
    }

    // ── the human decision stays pending ───────────────────────────────────
    assert_eq!(committed.human_decision.reviewer, None);
    assert_eq!(
        committed.human_decision.decision,
        RELEASE_READINESS_HUMAN_DECISION_PENDING
    );

    // ── the machine authorizes nothing; the five locks stay closed ─────────
    assert!(!committed.machine_authorization.s09_authorized);
    assert!(!committed.machine_authorization.gsd_lifecycle_authorizes);
    assert!(!committed.machine_authorization.human_release_recorded);
    for (lock, expected) in REQUIRED_LOCKS {
        assert_eq!(
            committed
                .machine_authorization
                .locks
                .get(lock)
                .map(String::as_str),
            Some(expected),
            "lock `{lock}` drifted"
        );
    }

    // ── the four milestone bindings, in canonical order ────────────────────
    let slices: Vec<&str> = committed
        .milestone_evidence
        .iter()
        .map(|row| row.slice.as_str())
        .collect();
    assert_eq!(slices, MILESTONE_SLICE_ORDER.to_vec());

    Ok(())
}

// ───────────────────────────── frozen carriers ─────────────────────────────

/// The four frozen carriers are byte-unchanged: every recomputed sha256 equals
/// the value the receipt — or the carrier the receipt binds — declares.
#[test]
fn frozen_carriers_are_byte_unchanged() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let committed = committed_receipt(&root)?;
    let bound = receipt_bound_digests(&committed);

    // The two carriers the receipt binds directly.
    for rel in [ACCEPTANCE_MANIFEST_REL, ACCEPTANCE_PACKAGE_REL] {
        let actual = sha256_prefixed(&read(&root, rel)?);
        let matched = bound
            .get(rel)
            .map(|set| set.contains(&actual))
            .unwrap_or(false);
        assert!(
            matched,
            "frozen carrier `{rel}` drifted from the receipt binding (recomputed {actual})"
        );
    }

    // The M513 release-decision package is bound by the acceptance package's
    // evidence, which the receipt binds; recompute and match that binding.
    let package: serde_json::Value = serde_json::from_slice(&read(&root, ACCEPTANCE_PACKAGE_REL)?)?;
    let declared = find_binding_sha(&package, FROZEN_RELEASE_DECISION_PACKAGE_REL)
        .ok_or("the acceptance package does not bind the release-decision package")?;
    assert_eq!(
        sha256_prefixed(&read(&root, FROZEN_RELEASE_DECISION_PACKAGE_REL)?),
        declared,
        "frozen carrier `{FROZEN_RELEASE_DECISION_PACKAGE_REL}` drifted"
    );

    // The S09 retirement-readiness receipt is frozen by the S09 contract; S12
    // publishes beside it and must not rewrite it.
    assert_eq!(
        sha256_prefixed(&read(&root, FROZEN_S09_READINESS_REL)?),
        FROZEN_S09_READINESS_SHA256,
        "frozen carrier `{FROZEN_S09_READINESS_REL}` drifted"
    );

    Ok(())
}

// ───────────────────────────── named refusals ─────────────────────────────

#[test]
fn dropped_release_gate_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.release_gates.retain(|row| row.gate_id != "cost");
    refuse(&expectation, &receipt, "ReleaseGateMissing")
}

#[test]
fn extra_release_gate_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.release_gates.push(ReleaseGateRow {
        gate_id: "speed".to_owned(),
        status: "met".to_owned(),
        release_effect: "does_not_authorize_anything".to_owned(),
        evidence: vec![CarrierBinding {
            manifest: ACCEPTANCE_MANIFEST_REL.to_owned(),
            pointer: POINTER_GATE_QUALITY.to_owned(),
            revision_sha256: sha256_prefixed(b"speed"),
        }],
    });
    refuse(&expectation, &receipt, "ReleaseGateUnexpected")
}

#[test]
fn duplicate_release_gate_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    let duplicate = receipt.release_gates[0].clone();
    receipt.release_gates.push(duplicate);
    refuse(&expectation, &receipt, "DuplicateReleaseGate")
}

#[test]
fn gate_with_empty_evidence_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    gate_mut(&mut receipt.release_gates, "rights")
        .evidence
        .clear();
    refuse(&expectation, &receipt, "ReleaseGateEvidenceEmpty")
}

#[test]
fn non_repo_relative_evidence_path_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    gate_mut(&mut receipt.release_gates, "quality").evidence[0].manifest =
        "/absolute/path.yaml".to_owned();
    refuse(&expectation, &receipt, "UnboundEvidence")
}

#[test]
fn non_digest_evidence_sha256_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    gate_mut(&mut receipt.release_gates, "quality").evidence[0].revision_sha256 =
        "not-a-digest".to_owned();
    refuse(&expectation, &receipt, "UnboundEvidence")
}

#[test]
fn stale_evidence_sha256_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    // A well-formed digest that no longer matches the committed bytes: the shape
    // check passes and the drift check must refuse.
    gate_mut(&mut receipt.release_gates, "quality").evidence[0].revision_sha256 =
        sha256_prefixed(b"stale");
    refuse(&expectation, &receipt, "EvidenceDrift")
}

#[test]
fn named_reviewer_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.human_decision.reviewer = Some("Ada".to_owned());
    refuse(&expectation, &receipt, "HumanDecisionClaimed")
}

#[test]
fn non_pending_decision_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.human_decision.decision = "approved".to_owned();
    refuse(&expectation, &receipt, "HumanDecisionClaimed")
}

#[test]
fn opened_lock_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt
        .machine_authorization
        .locks
        .insert("import_eligible".to_owned(), "true".to_owned());
    refuse(&expectation, &receipt, "LockOpened")
}

#[test]
fn claimed_s09_authorization_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.machine_authorization.s09_authorized = true;
    refuse(&expectation, &receipt, "MachineAuthorizationClaimed")
}

#[test]
fn claimed_gsd_lifecycle_authorization_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.machine_authorization.gsd_lifecycle_authorizes = true;
    refuse(&expectation, &receipt, "MachineAuthorizationClaimed")
}

#[test]
fn claimed_human_release_record_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.machine_authorization.human_release_recorded = true;
    refuse(&expectation, &receipt, "MachineAuthorizationClaimed")
}

#[test]
fn widened_claim_scope_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.claim_scope = "release_authorized".to_owned();
    refuse(&expectation, &receipt, "ClaimScopeDrift")
}

#[test]
fn accepted_threshold_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.thresholds = "accepted".to_owned();
    refuse(&expectation, &receipt, "ThresholdClaimed")
}

#[test]
fn claimed_product_coverage_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.product_coverage_effect = "full".to_owned();
    refuse(&expectation, &receipt, "ProductCoverageClaimed")
}

#[test]
fn nothing_authorizes_flag_false_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.nothing_here_authorizes_a_release = false;
    refuse(&expectation, &receipt, "NothingAuthorizesFlagFalse")
}

#[test]
fn dropped_milestone_slice_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.milestone_evidence.retain(|row| row.slice != "S11");
    refuse(&expectation, &receipt, "MilestoneSliceMissing")
}

#[test]
fn redaction_flag_true_is_refused() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    receipt.redaction.raw_text = true;
    refuse(&expectation, &receipt, "RedactionLeak")
}

// A guard against a future edit that adds a milestone slice mutation without a
// matching refusal test: the milestone_mut helper is used so the binding shape
// stays exercised.
#[test]
fn milestone_binding_shape_is_bound_to_the_receipt() -> Result<(), Box<dyn Error>> {
    let (expectation, mut receipt) = expectation_and_receipt()?;
    milestone_mut(&mut receipt.milestone_evidence, "S03").revision_sha256 =
        sha256_prefixed(b"drifted");
    refuse(&expectation, &receipt, "MilestoneEvidenceDrift")
}
