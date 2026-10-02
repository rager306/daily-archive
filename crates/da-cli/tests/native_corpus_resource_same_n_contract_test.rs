//! S08/T03 (M514-1lss2s) — contract test for the corpus-bound same-n resource
//! artifact.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it,
//! rather than compiling an empty target. It re-derives the assigned census from
//! the frozen holdout manifest and the freeze revision from the committed
//! governor carriers, binds the committed artifact and asserts its canonical
//! byte re-serialization, its non-empty native denominators, its typed reference
//! absences and rejects each named mutation.

use da_cli::native_acceptance::{
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL, NativeAcceptanceReceipt,
    load_holdout_documents,
};
use da_cli::native_corpus_resource_same_n::{
    AssignedIdentity, CORPUS_RESOURCE_GOVERNOR_POINTER, CORPUS_RESOURCE_GOVERNOR_REL,
    CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL, CORPUS_RESOURCE_ROUTE_RECEIPT_REL,
    CorpusResourceSameNExpectation, CorpusResourceSameNReceipt,
    DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL, carrier_bare_digest, freeze_revision,
    validate_corpus_resource_same_n_receipt,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_resource_same_n::{
    SAME_N_ABSENCE_REASON_VOCABULARY, SAME_N_OBSERVATION_STATE_VOCABULARY,
    SAME_N_REFERENCE_METRIC_ORDER,
};
use da_cli::native_route_receipt::RouteLimitsBinding;
use std::collections::BTreeSet;
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

/// Every named mutation must be refused by name; a mutation that slips through
/// fails this test.
fn expect_refusal(
    label: &str,
    receipt: &CorpusResourceSameNReceipt,
    expectation: &CorpusResourceSameNExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_corpus_resource_same_n_receipt(receipt, expectation) {
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

#[test]
fn native_corpus_resource_same_n_artifact_is_canonical_and_census_bound()
-> Result<(), Box<dyn Error>> {
    let root = repo_root();

    // ── re-derive the assigned census from the frozen manifest ─────────────
    let manifest_bytes = read(&root, HOLDOUT_MANIFEST_REL)?;
    let assigned: Vec<AssignedIdentity> =
        load_holdout_documents(std::str::from_utf8(&manifest_bytes)?)?
            .into_iter()
            .map(|document| AssignedIdentity {
                id: document.id,
                source_pdf: document.pdf_path,
                sha256: document.sha256,
            })
            .collect();
    assert_eq!(assigned.len(), 610, "assigned census drifted");

    // ── re-derive the freeze revision from the committed carriers ──────────
    let governor_bytes = read(&root, CORPUS_RESOURCE_GOVERNOR_REL)?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&governor_bytes)?;
    let table = value
        .get(CORPUS_RESOURCE_GOVERNOR_POINTER)
        .and_then(|node| node.get("limit_policy"))
        .and_then(|node| node.get("freeze_revision"))
        .and_then(|node| node.get("carriers"))
        .and_then(|node| node.as_sequence())
        .ok_or("the governor block declares no freeze carriers")?;
    let mut carriers = Vec::with_capacity(table.len());
    for carrier in table {
        let path = carrier
            .get("path")
            .and_then(|node| node.as_str())
            .ok_or("a freeze carrier has no path")?;
        let bytes = fs::read(root.join(path))?;
        carriers.push((path.to_owned(), carrier_bare_digest(&bytes)));
    }
    let revision = freeze_revision(&carriers);
    assert!(revision.starts_with("sha256:"), "freeze revision malformed");

    // ── bind the T01 carrier and prove it parses ───────────────────────────
    let t01_bytes = read(&root, DEFAULT_ACCEPTANCE_ARTIFACT_REL)?;
    let native_measurement = CarrierBinding {
        manifest: CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(&t01_bytes),
    };
    let _t01 = NativeAcceptanceReceipt::parse_json_bytes(&t01_bytes)?;

    let expectation = CorpusResourceSameNExpectation {
        freeze_revision: revision,
        native_measurement: native_measurement.clone(),
        assigned: assigned.clone(),
    };

    // ── the artifact exists, is canonical and validates ────────────────────
    let artifact_bytes = read(&root, DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL)?;
    let receipt = CorpusResourceSameNReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_corpus_resource_same_n_receipt(&receipt, &expectation)?;
    assert_eq!(
        receipt.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the full assigned denominator is carried, never shrunken ───────────
    assert_eq!(receipt.populations.assigned, 610, "assigned drifted");
    assert_eq!(receipt.populations.measurable, 531, "measurable drifted");
    assert_eq!(receipt.populations.excluded, 79, "excluded drifted");
    assert_eq!(receipt.documents.len(), 610, "document rows drifted");

    // ── every assigned document has exactly one typed row ──────────────────
    let mut seen = BTreeSet::new();
    for document in &receipt.documents {
        assert!(
            seen.insert(document.id.as_str()),
            "duplicate document row `{}`",
            document.id
        );
        assert!(
            ["observed", "refused", "not_run"].contains(&document.plane_state.as_str()),
            "unknown plane state `{}` on `{}`",
            document.plane_state,
            document.id
        );
    }
    assert_eq!(seen.len(), 610);

    // ── native rows carry non-empty denominators (no silent zero) ──────────
    assert!(
        !receipt.native_metrics.is_empty(),
        "no native metric measured"
    );
    for row in &receipt.native_metrics {
        assert!(
            row.denominator > 0,
            "native metric `{}` carries an empty denominator",
            row.metric_id
        );
    }

    // ── reference rows: observed-with-pinned-source or typed not_observed ──
    assert_eq!(
        receipt
            .reference_metrics
            .iter()
            .filter(|row| row.is_observed())
            .count(),
        0,
        "no same-n reference exists for the 610-corpus; none may be observed"
    );
    for row in &receipt.reference_metrics {
        assert!(
            SAME_N_REFERENCE_METRIC_ORDER.contains(&row.id.as_str()),
            "reference metric `{}` is outside the vocabulary",
            row.id
        );
        assert!(
            SAME_N_OBSERVATION_STATE_VOCABULARY.contains(&row.state.as_str()),
            "reference metric `{}` has an unknown state",
            row.id
        );
        let reason = row.reason.as_deref().unwrap_or_default();
        assert!(
            !reason.is_empty(),
            "reference metric `{}` has no reason",
            row.id
        );
        assert!(
            SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason),
            "reference metric `{}` has a reason outside the vocabulary",
            row.id
        );
    }
    for entry in &receipt.reference_carriers {
        assert!(!entry.same_n, "no carrier may be declared same-n here");
        assert!(
            SAME_N_ABSENCE_REASON_VOCABULARY.contains(&entry.reason.as_str()),
            "reference carrier `{}` has an untyped absence reason",
            entry.carrier.manifest
        );
    }

    // ── the limits freeze binding matches the committed route receipt ──────
    let route_bytes = read(&root, CORPUS_RESOURCE_ROUTE_RECEIPT_REL)?;
    let route_value: serde_json::Value = serde_json::from_slice(&route_bytes)?;
    let route_limits: RouteLimitsBinding = serde_json::from_value(
        route_value
            .get("limits")
            .cloned()
            .ok_or("the route receipt carries no `limits` block")?,
    )?;
    assert_eq!(
        receipt.limits, route_limits,
        "limits freeze binding drifted"
    );

    // ── named mutations are all refused ────────────────────────────────────
    let mut m = receipt.clone();
    m.documents[0].sha256 = "sha256:0000".to_owned();
    expect_refusal("document_set_mismatch", &m, &expectation, "Mismatch")?;

    let mut m = receipt.clone();
    m.documents[0].id = "unknown-9999.00000".to_owned();
    expect_refusal("unknown_document", &m, &expectation, "Mismatch")?;

    let mut m = receipt.clone();
    m.pairing.pooled = "allowed".to_owned();
    expect_refusal("pooled_claim", &m, &expectation, "PooledClaimed")?;

    let mut m = receipt.clone();
    m.pairing.saving_claim = "allowed".to_owned();
    expect_refusal("saving_claim", &m, &expectation, "SavingClaimed")?;

    let mut m = receipt.clone();
    m.reference_metrics[0].reason = Some("mystery".to_owned());
    expect_refusal("untyped_reason", &m, &expectation, "AbsenceReasonUnknown")?;

    let mut m = receipt.clone();
    m.native_metrics[0].denominator = 0;
    expect_refusal("empty_denominator", &m, &expectation, "ZeroMasquerade")?;

    let mut m = receipt.clone();
    m.locks
        .insert("graph_write".to_owned(), "allowed".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut drifted = expectation.clone();
    drifted.freeze_revision = sha256_prefixed(b"drifted");
    expect_refusal(
        "freeze_revision_drift",
        &receipt,
        &drifted,
        "StaleFreezeRevision",
    )?;

    // A redaction leak must be refused by the serializer itself.
    let mut m = receipt.clone();
    m.documents[0].source_pdf = "/tmp/leak".to_owned();
    let leak = m.artifact_json_bytes();
    assert!(
        matches!(
            leak,
            Err(da_cli::native_resource_same_n::SameNReceiptError::RedactionLeak { .. })
        ),
        "a physical path leak was not refused"
    );

    Ok(())
}
