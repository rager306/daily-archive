//! S08/T06 (M514-1lss2s) — hermetic audit of the GROBID retirement acceptance package.
//!
//! `native_acceptance_package_contract_test.rs` (T05) proves the package is the
//! evaluator's own canonical re-serialization. This file is the *hermetic* half
//! the Governor's `acceptance_package` rule points at through
//! `source_markers.acceptance_audit`: it re-derives the package's evidence pins
//! **from the committed bytes only**, re-derives the blocking set from the gate
//! ledger, reads the committed rollback and same-n resource carriers to confirm
//! their byte-identity / typed-absence evidence, and refuses a set of named
//! mutations by their typed error variant.
//!
//! The audit never reads the frozen corpus. R141 requires the Governor to see
//! source-bound progress without treating an artifact's existence as capability,
//! and the frozen 610-document corpus is a gitignored local asset; every pin
//! this test recomputes resolves from a repository-relative *committed* carrier
//! (the package's own fourteen-carrier table). A `committed_carrier_pins` check
//! asserts every bound manifest is a tracked artifact, never a frozen-corpus
//! source PDF, so the audit cannot silently start depending on the corpus.
//!
//! The test is deliberately ungated (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it
//! rather than compiling an empty target.

use da_cli::native_acceptance_package::{
    AUTHORIZATION_BASIS_HUMAN_RELEASE_REQUIRED, BODY_RESIDUAL_TOKEN,
    DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL, GATE_KIND_VOCABULARY, GATE_NON_BLOCKING_STATES,
    GATE_STATE_VOCABULARY, GrobidRetirementAcceptancePackage, NAMED_GAP_H20,
    PACKAGE_DB_CANONICAL_REQUIREMENT_IDS, PACKAGE_EVIDENCE_CARRIERS, PACKAGE_RESOURCE_RECEIPT_REL,
    PACKAGE_ROLLBACK_RECEIPT_REL, gate_blocks_s09, validate_acceptance_package,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

fn read_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(read(root, rel)?)?)
}

/// Re-derive the package's evidence pins from the committed carrier bytes.
///
/// This is the `committed_carrier_pins` half: the table is the T05 constant, so
/// the kept set cannot drift, and every digest comes from `fs::read` of a
/// repository-relative committed file — never from the package's own claim.
fn committed_carrier_pins(root: &Path) -> Result<Vec<CarrierBinding>, Box<dyn Error>> {
    PACKAGE_EVIDENCE_CARRIERS
        .iter()
        .map(|(rel, pointer)| {
            Ok(CarrierBinding {
                manifest: (*rel).to_owned(),
                pointer: (*pointer).to_owned(),
                revision_sha256: sha256_prefixed(&read(root, rel)?),
            })
        })
        .collect()
}

fn expectation(
    root: &Path,
) -> Result<da_cli::native_acceptance_package::AcceptancePackageExpectation, Box<dyn Error>> {
    Ok(
        da_cli::native_acceptance_package::AcceptancePackageExpectation {
            evidence: committed_carrier_pins(root)?,
            threshold_gate_count: 3,
            pending_family_gate_count: 12,
            body_residual: vec![BODY_RESIDUAL_TOKEN.to_owned()],
            db_canonical_requirement_ids: PACKAGE_DB_CANONICAL_REQUIREMENT_IDS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        },
    )
}

fn load_package(root: &Path) -> Result<GrobidRetirementAcceptancePackage, Box<dyn Error>> {
    Ok(GrobidRetirementAcceptancePackage::parse_json_bytes(&read(
        root,
        DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL,
    )?)?)
}

fn expect_refusal(
    label: &str,
    package: &GrobidRetirementAcceptancePackage,
    expectation: &da_cli::native_acceptance_package::AcceptancePackageExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_acceptance_package(package, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

// ── the pins, re-derived from committed bytes ──────────────────────────────

#[test]
fn native_acceptance_package_audit_recomputes_committed_carrier_pins() -> Result<(), Box<dyn Error>>
{
    let root = repo_root();
    let pins = committed_carrier_pins(&root)?;
    assert_eq!(pins.len(), 14, "the evidence carrier table drifted");

    let package = load_package(&root)?;
    let mut declared = package.evidence.clone();
    let mut recomputed = pins.clone();
    declared.sort_by(|a, b| a.manifest.cmp(&b.manifest).then(a.pointer.cmp(&b.pointer)));
    recomputed.sort_by(|a, b| a.manifest.cmp(&b.manifest).then(a.pointer.cmp(&b.pointer)));
    assert_eq!(
        declared, recomputed,
        "a declared pin drifted from the committed bytes"
    );

    let expectation = expectation(&root)?;
    validate_acceptance_package(&package, &expectation)?;
    Ok(())
}

// ── the gate ledger is derived, not trusted ────────────────────────────────

#[test]
fn acceptance_package_gate_ledger_is_derived_not_trusted() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let package = load_package(&root)?;

    // Every gate is typed and its `blocks_s09` re-derives from its own state.
    for gate in &package.mandatory_gates {
        assert!(
            GATE_KIND_VOCABULARY.contains(&gate.kind.as_str()),
            "gate `{}` has kind `{}` outside the vocabulary",
            gate.gate_id,
            gate.kind
        );
        assert!(
            GATE_STATE_VOCABULARY.contains(&gate.state.as_str()),
            "gate `{}` has state `{}` outside the vocabulary",
            gate.gate_id,
            gate.state
        );
        assert_eq!(
            gate.blocks_s09,
            gate_blocks_s09(gate.required, &gate.state),
            "gate `{}` blocks_s09 disagrees with its state",
            gate.gate_id
        );
    }

    // The declared blocking set is exactly the recomputed one, and S09 stays
    // unauthorized because a gate still blocks (`s09_authorized` is computed).
    let recomputed: Vec<(String, String)> = package
        .mandatory_gates
        .iter()
        .filter(|gate| gate.required && !GATE_NON_BLOCKING_STATES.contains(&gate.state.as_str()))
        .map(|gate| (gate.gate_id.clone(), gate.state.clone()))
        .collect();
    let declared: Vec<(String, String)> = package
        .blocking_gates
        .iter()
        .map(|gate| (gate.gate_id.clone(), gate.state.clone()))
        .collect();
    assert_eq!(
        declared, recomputed,
        "the blocking set drifted from the ledger"
    );
    assert!(
        !package.blocking_gates.is_empty(),
        "the frozen corpus cannot close every gate"
    );
    assert!(
        !package.authorization.s09_authorized,
        "s09_authorized must be false"
    );
    assert!(!package.authorization.gsd_lifecycle_authorizes);
    assert_eq!(
        package.authorization.basis,
        AUTHORIZATION_BASIS_HUMAN_RELEASE_REQUIRED
    );

    assert_eq!(
        package.mandatory_gates.len(),
        21,
        "the mandatory gate count drifted"
    );
    assert!(
        package
            .mandatory_gates
            .iter()
            .any(|gate| gate.gate_id == NAMED_GAP_H20),
        "the frozen external-holdout decision remainder is missing"
    );

    // The body residual survives verbatim (D499).
    let body: Vec<_> = package
        .mandatory_gates
        .iter()
        .filter(|gate| gate.kind == "body_axis")
        .collect();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0].state, "not_assessable");
    assert!(
        body[0]
            .reason
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .any(|part| part.trim() == "p1_localization_uncertain"),
        "the body residual was renamed"
    );
    Ok(())
}

// ── the audit never reaches into the gitignored corpus ─────────────────────

#[test]
fn committed_carrier_pins_do_not_reference_the_gitignored_corpus() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let corpus_fragment = format!("data/{}/", "sources");
    for (rel, _pointer) in PACKAGE_EVIDENCE_CARRIERS {
        assert!(
            !rel.contains(&corpus_fragment) && !rel.ends_with(".pdf"),
            "carrier `{rel}` reaches into the frozen corpus"
        );
    }

    // The two corpus-bound carriers the package binds are the *committed*
    // receipts, not the corpus itself.
    assert!(PACKAGE_ROLLBACK_RECEIPT_REL.starts_with("artifacts/parse_compat/"));
    assert!(PACKAGE_RESOURCE_RECEIPT_REL.starts_with("artifacts/parse_compat/"));

    // The audit's own source must not name the corpus either.
    let source = read_text(
        &root,
        "crates/da-cli/tests/native_acceptance_package_test.rs",
    )?;
    assert!(
        !source.contains(&corpus_fragment),
        "the hermetic audit acquired a corpus path"
    );
    Ok(())
}

// ── named mutations are refused by variant ─────────────────────────────────

#[test]
fn acceptance_package_refuses_named_mutations() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let package = load_package(&root)?;
    let expectation = expectation(&root)?;

    let mut m = package.clone();
    m.claim_scope = "product_progress".to_owned();
    expect_refusal("non_diagnostic_claim", &m, &expectation, "ClaimScopeDrift")?;

    let mut m = package.clone();
    for gate in &mut m.mandatory_gates {
        if gate.kind == "body_axis" {
            gate.reason = Some("p1_localization_unknown".to_owned());
        }
    }
    expect_refusal(
        "renamed_body_residual",
        &m,
        &expectation,
        "BodyResidualRenamed",
    )?;

    let mut m = package.clone();
    m.authorization.s09_authorized = true;
    expect_refusal(
        "unmet_gate_authorized",
        &m,
        &expectation,
        "UnmetGateAuthorized",
    )?;

    let mut m = package.clone();
    m.authorization.gsd_lifecycle_authorizes = true;
    expect_refusal(
        "gsd_lifecycle_authorization",
        &m,
        &expectation,
        "GsdLifecycleAuthorization",
    )?;

    let mut m = package.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut m = package.clone();
    m.mandatory_gates
        .retain(|gate| gate.gate_id != NAMED_GAP_H20);
    expect_refusal(
        "dropped_mandatory_gate",
        &m,
        &expectation,
        "BlockingSetDrift",
    )?;

    let mut m = package.clone();
    m.mandatory_gates[0].state = "mystery".to_owned();
    expect_refusal("unknown_gate_state", &m, &expectation, "UnknownGateState")?;

    let mut m = package.clone();
    m.authorization.basis = "auto_authorized".to_owned();
    expect_refusal(
        "authorization_basis_drift",
        &m,
        &expectation,
        "AuthorizationBasisDrift",
    )?;

    let mut m = package.clone();
    m.mandatory_gates.push(m.mandatory_gates[0].clone());
    expect_refusal("duplicate_gate", &m, &expectation, "DuplicateGate")?;

    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &package, &drifted, "EvidenceDrift")?;

    Ok(())
}

// ── the committed rollback / resource carriers carry their evidence ────────

/// The rollback claim is real only when the per-document evidence backs it.
fn rollback_without_byte_identity(document: &Value) -> Result<(), String> {
    let claim = document
        .get("corpus_claim")
        .ok_or("rollback_without_byte_identity: missing corpus_claim")?;
    if claim.get("byte_identity_holds") != Some(&Value::Bool(true)) {
        return Err("rollback_without_byte_identity: byte_identity_holds is not true".to_owned());
    }
    if claim.get("native_fragment_leak_free") != Some(&Value::Bool(true)) {
        return Err(
            "rollback_without_byte_identity: native_fragment_leak_free is not true".to_owned(),
        );
    }
    let residuals = document
        .get("residuals")
        .ok_or("rollback_without_byte_identity: missing residuals")?;
    for key in ["byte_identity_violations", "native_fragment_leaks"] {
        if residuals.get(key).and_then(Value::as_u64) != Some(0) {
            return Err(format!("rollback_without_byte_identity: {key} is not zero"));
        }
    }
    let rehearsed = claim.get("rehearsed_documents").and_then(Value::as_u64);
    let declared = claim.get("declared_documents").and_then(Value::as_u64);
    if rehearsed.is_none() || rehearsed != declared {
        return Err("rollback_without_byte_identity: rehearsed != declared".to_owned());
    }
    let documents = document
        .get("documents")
        .and_then(Value::as_array)
        .ok_or("rollback_without_byte_identity: missing documents")?;
    let byte_identical = documents
        .iter()
        .filter(|row| row.get("byte_identity") == Some(&Value::Bool(true)))
        .count() as u64;
    if Some(byte_identical) != rehearsed {
        return Err(
            "rollback_without_byte_identity: per-document byte identity does not cover the rehearsed set"
                .to_owned(),
        );
    }
    Ok(())
}

/// A same-n resource claim is real only when every reference row is a typed
/// absence and the pairing forbids pooling and savings.
fn pooled_resource_claim(document: &Value) -> Result<(), String> {
    let pairing = document
        .get("pairing")
        .ok_or("pooled_resource_claim: missing pairing")?;
    if pairing.get("rule").and_then(Value::as_str) != Some("same_documents_only") {
        return Err("pooled_resource_claim: pairing.rule drifted".to_owned());
    }
    if pairing.get("pooled").and_then(Value::as_str) != Some("forbidden") {
        return Err("pooled_resource_claim: pairing.pooled is not forbidden".to_owned());
    }
    if pairing.get("saving_claim").and_then(Value::as_str) != Some("forbidden") {
        return Err("pooled_resource_claim: pairing.saving_claim is not forbidden".to_owned());
    }
    let rows = document
        .get("reference_metrics")
        .and_then(Value::as_array)
        .ok_or("pooled_resource_claim: missing reference_metrics")?;
    if rows.is_empty() {
        return Err("pooled_resource_claim: no reference metric rows".to_owned());
    }
    for row in rows {
        if row.get("state").and_then(Value::as_str) != Some("not_observed") {
            return Err(
                "pooled_resource_claim: a reference metric is not a typed absence".to_owned(),
            );
        }
        if row
            .get("source")
            .map(|value| !value.is_null())
            .unwrap_or(false)
        {
            return Err("pooled_resource_claim: a reference metric claims a source".to_owned());
        }
    }
    Ok(())
}

#[test]
fn acceptance_package_refuses_a_rollback_claim_without_byte_identity() -> Result<(), Box<dyn Error>>
{
    let root = repo_root();
    let mut rollback: Value = serde_json::from_slice(&read(&root, PACKAGE_ROLLBACK_RECEIPT_REL)?)?;
    rollback_without_byte_identity(&rollback)
        .map_err(|error| -> Box<dyn Error> { error.into() })?;

    rollback["corpus_claim"]["byte_identity_holds"] = Value::Bool(false);
    assert!(rollback_without_byte_identity(&rollback).is_err());
    Ok(())
}

#[test]
fn acceptance_package_refuses_a_pooled_resource_claim() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let mut resource: Value = serde_json::from_slice(&read(&root, PACKAGE_RESOURCE_RECEIPT_REL)?)?;
    pooled_resource_claim(&resource).map_err(|error| -> Box<dyn Error> { error.into() })?;

    resource["pairing"]["pooled"] = Value::String("allowed".to_owned());
    assert!(pooled_resource_claim(&resource).is_err());

    let mut resource: Value = serde_json::from_slice(&read(&root, PACKAGE_RESOURCE_RECEIPT_REL)?)?;
    resource["reference_metrics"][0]["state"] = Value::String("observed".to_owned());
    assert!(pooled_resource_claim(&resource).is_err());
    Ok(())
}
