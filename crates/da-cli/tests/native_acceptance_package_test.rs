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
    GATE_STATE_VOCABULARY, GrobidRetirementAcceptancePackage, INDEPENDENCE_COMMAND, NAMED_GAP_H20,
    PACKAGE_DB_CANONICAL_REQUIREMENT_IDS, PACKAGE_EVIDENCE_CARRIERS,
    PACKAGE_NATIVE_MEASUREMENT_REL, PACKAGE_REFERENCE_PINS_REL, PACKAGE_REFERENCE_PROBE_REL,
    PACKAGE_RESOURCE_RECEIPT_REL, PACKAGE_ROLLBACK_RECEIPT_REL, RECOMPUTE_CARRIER_DIGEST_ALGORITHM,
    gate_blocks_s09, validate_acceptance_package,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_resource_same_n::SAME_N_ABSENCE_REASON_VOCABULARY;
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

/// Resolve a named surface or judge manifest through the filesystem and
/// confirm the manifest text carries the named symbol, so a renamed or moved
/// judge fails the audit instead of being taken on trust.
fn assert_manifest_names(
    root: &Path,
    manifest: &str,
    symbol: &str,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    assert!(!manifest.trim().is_empty(), "{label} has an empty manifest");
    assert!(!symbol.trim().is_empty(), "{label} has an empty symbol");
    let path = root.join(manifest);
    let metadata = fs::metadata(&path).map_err(|error| -> Box<dyn Error> {
        format!("{label} manifest `{manifest}` does not resolve: {error}").into()
    })?;
    assert!(
        metadata.is_file(),
        "{label} manifest `{manifest}` is not a file"
    );
    let text = String::from_utf8(fs::read(&path)?)?;
    assert!(
        text.contains(symbol),
        "{label} manifest `{manifest}` does not carry symbol `{symbol}`"
    );
    Ok(())
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

// ── R134 (a) recompute and (c) provenance statements are carried and real ──

#[test]
fn acceptance_package_states_recomputability_and_independence() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let package = load_package(&root)?;

    // ── R134 clause (a): reviewer recomputability over committed bytes ────
    let recompute = &package.reviewer_recomputation;
    assert_eq!(
        recompute.carrier_digest_algorithm, RECOMPUTE_CARRIER_DIGEST_ALGORITHM,
        "the recompute digest algorithm is not sha256"
    );
    assert_eq!(recompute.carrier_digest_algorithm, "sha256");
    assert!(
        recompute.committed_bytes_only,
        "recomputation must read committed bytes only"
    );
    assert!(
        !recompute.corpus_derived_pins,
        "no pin may be derived from the gitignored corpus"
    );
    assert_eq!(
        recompute.carrier_count,
        package.evidence.len() as u64,
        "carrier_count is not the bound carrier count"
    );
    assert_eq!(
        recompute.carrier_count, 14,
        "the package must carry the fourteen bound carriers"
    );
    assert!(
        recompute.recompute_surfaces.len() >= 2,
        "fewer than two recompute surfaces are named"
    );
    assert!(!recompute.statement.trim().is_empty());
    assert!(!recompute.canonicalization.trim().is_empty());
    assert!(!recompute.self_canonicalization.trim().is_empty());

    // The three named recompute surfaces must resolve to real committed
    // manifests that carry the named symbol.
    let expected_surfaces = [
        (
            "crates/da-cli/tests/native_acceptance_package_test.rs",
            "native_acceptance_package_audit_recomputes_committed_carrier_pins",
        ),
        (
            "scripts/governor/adapters/absorption_progress_reader.py",
            "_acceptance_package_violations",
        ),
        (
            "scripts/governor/checks/grobid_absorption_progress.yaml",
            "acceptance_package",
        ),
    ];
    for (manifest, symbol) in expected_surfaces {
        assert!(
            recompute
                .recompute_surfaces
                .iter()
                .any(|surface| surface.manifest == manifest && surface.symbol == symbol),
            "the recompute surface `{manifest}#{symbol}` is not named"
        );
        assert_manifest_names(&root, manifest, symbol, "recompute surface")?;
    }
    for surface in &recompute.recompute_surfaces {
        assert_manifest_names(
            &root,
            &surface.manifest,
            &surface.symbol,
            "recompute surface",
        )?;
    }

    // ── R134 clause (c): same-generator/different-label linkage ───────────
    let independence = &package.carrier_independence;
    assert_eq!(
        independence.candidate_manifest, PACKAGE_NATIVE_MEASUREMENT_REL,
        "the candidate manifest is not the T01 native measurement carrier"
    );
    assert!(
        independence
            .reference_manifests
            .contains(&PACKAGE_REFERENCE_PINS_REL.to_owned()),
        "the frozen reference pins are not judged against"
    );
    assert!(
        independence
            .reference_manifests
            .contains(&PACKAGE_REFERENCE_PROBE_REL.to_owned()),
        "the executed reference probe is not judged against"
    );
    assert!(
        !independence
            .reference_manifests
            .iter()
            .any(|manifest| manifest == &independence.candidate_manifest),
        "the candidate is listed among its own references"
    );
    assert!(!independence.same_generator_rule.trim().is_empty());
    assert!(!independence.series_identity_rule.trim().is_empty());
    assert!(!independence.statement.trim().is_empty());
    assert!(!independence.limitation.trim().is_empty());
    assert_eq!(
        independence.command, INDEPENDENCE_COMMAND,
        "the declared judge command drifted"
    );
    assert_eq!(
        independence.command, "cargo test -p da-application --test promotion_qualification",
        "the declared command is not the promotion_qualification test target"
    );

    // The four existing mechanisms must be named by manifest and symbol, and
    // each manifest must resolve and carry that symbol.
    let expected_judges = [
        (
            "crates/da-application/src/native_family_promotion_gate.rs",
            "ProducerNotIndependent",
        ),
        (
            "crates/da-application/tests/promotion_qualification.rs",
            "shared_producer_digest_is_unqualified",
        ),
        (
            "crates/da-application/tests/promotion_qualification.rs",
            "weak_producer_independence_is_unqualified_and_thresholds_are_explicit",
        ),
        (
            "crates/da-parse/src/native_acceptance.rs",
            "REFUSAL_SELF_REFERENCE",
        ),
    ];
    for (manifest, symbol) in expected_judges {
        assert!(
            independence
                .judges
                .iter()
                .any(|judge| judge.manifest == manifest && judge.symbol == symbol),
            "the independence judge `{manifest}#{symbol}` is not named"
        );
        assert_manifest_names(&root, manifest, symbol, "independence judge")?;
    }
    for judge in &independence.judges {
        assert_manifest_names(&root, &judge.manifest, &judge.symbol, "independence judge")?;
    }

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

    // R134 clause (a): an emptied recompute-surface list is refused by name.
    let mut m = package.clone();
    m.reviewer_recomputation.recompute_surfaces.clear();
    expect_refusal(
        "recompute_surfaces_cleared",
        &m,
        &expectation,
        "ReviewerRecomputationDrift",
    )?;

    // R134 clause (c): an emptied judge list is refused by name.
    let mut m = package.clone();
    m.carrier_independence.judges.clear();
    expect_refusal(
        "judges_cleared",
        &m,
        &expectation,
        "CarrierIndependenceDrift",
    )?;

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

/// A same-n resource claim is real when every reference row is either an
/// observed row carrying a same-n source or a typed absence, the pairing
/// forbids pooling and savings, and the receipt's bound-row count matches the
/// rows that actually carry a source.
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
    let mut bound = 0u64;
    for row in rows {
        let has_source = row
            .get("source")
            .map(|value| !value.is_null())
            .unwrap_or(false);
        match row.get("state").and_then(Value::as_str) {
            Some("observed") => {
                if !has_source {
                    return Err(
                        "pooled_resource_claim: an observed reference metric has no source"
                            .to_owned(),
                    );
                }
                bound += 1;
            }
            Some("not_observed") => {
                let reason = row
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason) {
                    return Err(
                        "pooled_resource_claim: a not_observed reference metric has an untyped reason"
                            .to_owned(),
                    );
                }
                if has_source {
                    return Err(
                        "pooled_resource_claim: a not_observed reference metric claims a source"
                            .to_owned(),
                    );
                }
            }
            other => {
                return Err(format!(
                    "pooled_resource_claim: a reference metric has state `{}`, expected observed or not_observed",
                    other.unwrap_or("<absent>")
                ));
            }
        }
    }
    let declared_bound = document
        .get("residuals")
        .and_then(|residuals| residuals.get("reference_rows_bound"))
        .and_then(Value::as_u64)
        .ok_or("pooled_resource_claim: missing residuals.reference_rows_bound")?;
    if declared_bound != bound {
        return Err(format!(
            "pooled_resource_claim: residuals.reference_rows_bound is {declared_bound}, but {bound} rows carry a source"
        ));
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
    let resource: Value = serde_json::from_slice(&read(&root, PACKAGE_RESOURCE_RECEIPT_REL)?)?;
    // The committed receipt carries one observed same-n row and two typed
    // absences, and its bound-row count matches the rows with a source.
    pooled_resource_claim(&resource).map_err(|error| -> Box<dyn Error> { error.into() })?;

    // Pooling is refused.
    let mut pooled = resource.clone();
    pooled["pairing"]["pooled"] = Value::String("allowed".to_owned());
    assert!(pooled_resource_claim(&pooled).is_err());

    // A saving claim is refused.
    let mut saving = resource.clone();
    saving["pairing"]["saving_claim"] = Value::String("allowed".to_owned());
    assert!(pooled_resource_claim(&saving).is_err());

    // An observed row without a same-n source is refused.
    let mut unbound = resource.clone();
    let observed = unbound["reference_metrics"]
        .as_array_mut()
        .expect("reference_metrics")
        .iter_mut()
        .find(|row| row["state"].as_str() == Some("observed"))
        .expect("an observed reference row");
    observed["source"] = Value::Null;
    assert!(pooled_resource_claim(&unbound).is_err());

    // A not_observed row with an untyped reason is refused.
    let mut untyped = resource.clone();
    let absent = untyped["reference_metrics"]
        .as_array_mut()
        .expect("reference_metrics")
        .iter_mut()
        .find(|row| row["state"].as_str() == Some("not_observed"))
        .expect("a not_observed reference row");
    absent["reason"] = Value::String("untyped".to_owned());
    assert!(pooled_resource_claim(&untyped).is_err());

    // A drifted bound-row count is refused.
    let mut drifted = resource.clone();
    drifted["residuals"]["reference_rows_bound"] = Value::from(0u64);
    assert!(pooled_resource_claim(&drifted).is_err());

    Ok(())
}
