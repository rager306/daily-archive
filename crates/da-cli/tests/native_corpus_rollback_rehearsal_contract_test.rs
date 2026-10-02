//! S08/T04 (M514-1lss2s) — contract test for the real-corpus rollback rehearsal.
//!
//! This test is gated on `pdf-layout` (the feature the S08 sibling runners and
//! the architecture gate's feature-gated contract step build with). It
//! re-derives the assigned census from the frozen holdout manifest, the TEI
//! fixture pins from the committed provenance carrier and the freeze revision
//! from the committed governor carriers, binds the committed artifact, asserts
//! its canonical byte re-serialization, and then **re-runs the in-memory
//! `baseline -> switched -> returned` sequence on every committed preserved-TEI
//! fixture** to prove byte identity across the corpus. It rejects each named
//! mutation, including a missing fixture silently claimed byte-identical.

#![cfg(feature = "pdf-layout")]

use da_cli::native_acceptance::{
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL, load_holdout_documents,
};
use da_cli::native_corpus_resource_same_n::{
    AssignedIdentity, carrier_bare_digest, freeze_revision,
};
use da_cli::native_corpus_rollback_rehearsal::{
    CORPUS_ROLLBACK_GOVERNOR_POINTER, CORPUS_ROLLBACK_GOVERNOR_REL,
    CORPUS_ROLLBACK_TEI_PROVENANCE_REL, DEFAULT_CORPUS_ROLLBACK_REHEARSAL_ARTIFACT_REL,
    RollbackRehearsalExpectation, RollbackRehearsalReceipt, load_tei_fixture_pins,
    native_band_fragment, native_output_of, parse_native_measurement, run_rollback_sequence,
    validate_rollback_rehearsal_receipt,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use std::collections::BTreeMap;
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

fn expect_refusal(
    label: &str,
    receipt: &RollbackRehearsalReceipt,
    expectation: &RollbackRehearsalExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_rollback_rehearsal_receipt(receipt, expectation) {
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
fn native_corpus_rollback_rehearsal_artifact_is_canonical_and_corpus_bound()
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

    // ── re-derive the TEI pins from the committed provenance carrier ───────
    let provenance_bytes = read(&root, CORPUS_ROLLBACK_TEI_PROVENANCE_REL)?;
    let tei_pins = load_tei_fixture_pins(std::str::from_utf8(&provenance_bytes)?)?;
    assert_eq!(tei_pins.len(), 610, "TEI provenance census drifted");

    // ── re-derive the freeze revision from the committed carriers ──────────
    let governor_bytes = read(&root, CORPUS_ROLLBACK_GOVERNOR_REL)?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&governor_bytes)?;
    let table = value
        .get(CORPUS_ROLLBACK_GOVERNOR_POINTER)
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
        manifest: DEFAULT_ACCEPTANCE_ARTIFACT_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(&t01_bytes),
    };
    let t01 = parse_native_measurement(&t01_bytes)?;
    let tei_provenance = CarrierBinding {
        manifest: CORPUS_ROLLBACK_TEI_PROVENANCE_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(&provenance_bytes),
    };

    let expectation = RollbackRehearsalExpectation {
        freeze_revision: revision,
        native_measurement: native_measurement.clone(),
        tei_provenance: tei_provenance.clone(),
        assigned: assigned.clone(),
    };

    // ── the artifact exists, is canonical and validates ────────────────────
    let artifact_bytes = read(&root, DEFAULT_CORPUS_ROLLBACK_REHEARSAL_ARTIFACT_REL)?;
    let receipt = RollbackRehearsalReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_rollback_rehearsal_receipt(&receipt, &expectation)?;
    assert_eq!(
        receipt.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the full assigned denominator is carried, never shrunken ───────────
    assert_eq!(receipt.populations.assigned, 610, "assigned drifted");
    assert_eq!(receipt.populations.rehearsed, 610, "rehearsed drifted");
    assert_eq!(receipt.populations.refused, 0, "refused drifted");
    assert_eq!(receipt.documents.len(), 610, "document rows drifted");
    assert!(
        receipt.corpus_claim.byte_identity_holds,
        "the corpus byte-identity claim does not hold"
    );
    assert!(
        receipt.corpus_claim.native_fragment_leak_free,
        "the corpus is not native-fragment leak free"
    );
    assert!(
        receipt.corpus_claim.switch_positive_control,
        "the switch positive control did not fire"
    );

    // ── one typed row per assigned document, binding the pinned fixture ────
    let pins_by_id: BTreeMap<&str, &_> = tei_pins
        .iter()
        .map(|pin| (pin.article_id.as_str(), pin))
        .collect();
    let t01_by_id: BTreeMap<&str, &_> = t01
        .documents
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect();
    assert_eq!(t01_by_id.len(), 610, "T01 document rows drifted");

    for document in &receipt.documents {
        let pin = pins_by_id
            .get(document.id.as_str())
            .ok_or_else(|| format!("no pinned provenance for `{}`", document.id))?;
        assert_eq!(document.tei_path, pin.path, "tei_path drifted");
        assert_eq!(
            document.tei_sha256_pin.as_deref(),
            Some(pin.sha256.as_str()),
            "tei pin drifted"
        );
        assert_eq!(
            document.state, "rehearsed",
            "row `{}` is not rehearsed",
            document.id
        );

        // Re-run the declared sequence on the committed preserved-TEI bytes and
        // prove byte identity independently of the artifact's own digests.
        let baseline = fs::read(root.join(&pin.path))?;
        assert_eq!(
            sha256_prefixed(&baseline),
            pin.sha256,
            "fixture `{}` drifted from the provenance pin",
            document.id
        );
        assert_eq!(
            document.baseline_sha256.as_deref(),
            Some(pin.sha256.as_str()),
            "baseline digest drifted for `{}`",
            document.id
        );
        let t01_row = t01_by_id
            .get(document.id.as_str())
            .ok_or_else(|| format!("no T01 row for `{}`", document.id))?;
        let (native_state, layout_digest) = native_output_of(t01_row);
        assert_eq!(
            document.native_output_state, native_state,
            "plane state drifted"
        );
        let outcome = run_rollback_sequence(
            &baseline,
            &native_band_fragment(native_state, layout_digest),
        );
        assert!(
            outcome.byte_identity,
            "returned != baseline for `{}`",
            document.id
        );
        assert!(
            !outcome.native_fragment_leaked,
            "native fragment leaked for `{}`",
            document.id
        );
        assert_eq!(
            outcome.returned_sha256, outcome.baseline_sha256,
            "returned digest differs for `{}`",
            document.id
        );
    }

    // ── the sequence declaration is the declared one ───────────────────────
    assert_eq!(
        receipt.sequence.steps,
        vec![
            "baseline".to_owned(),
            "switched".to_owned(),
            "returned".to_owned()
        ]
    );
    assert_eq!(receipt.sequence.switch_scope, "in_memory_only");
    assert!(receipt.sequence.carriers_read_only);

    // ── named mutations are all refused ────────────────────────────────────
    let mut m = receipt.clone();
    m.documents[0].returned_sha256 = m.documents[0].switched_sha256.clone();
    expect_refusal(
        "returned_not_baseline",
        &m,
        &expectation,
        "RollbackNotByteIdentical",
    )?;

    let mut m = receipt.clone();
    m.documents[0].native_fragment_leaked = true;
    expect_refusal(
        "native_fragment_leaked",
        &m,
        &expectation,
        "NativeFragmentLeaked",
    )?;

    let mut m = receipt.clone();
    m.documents[0].byte_identity = false;
    expect_refusal(
        "byte_identity_false",
        &m,
        &expectation,
        "RollbackNotByteIdentical",
    )?;

    // A fixture the artifact claims was rehearsed while it refuses to carry a
    // baseline: this is the `silent_skip_missing_tei_fixture` shape. The row and
    // counters are made internally consistent, so only the corpus claim lies.
    let mut m = receipt.clone();
    m.documents[0].state = "refused".to_owned();
    m.documents[0].refusal_reason = Some("missing_tei_fixture".to_owned());
    m.documents[0].baseline_sha256 = None;
    m.documents[0].switched_sha256 = None;
    m.documents[0].returned_sha256 = None;
    m.documents[0].byte_identity = false;
    m.populations.rehearsed -= 1;
    m.populations.refused += 1;
    m.residuals.rehearsed_documents -= 1;
    m.residuals.refused_documents += 1;
    m.residuals.refused_fixtures += 1;
    m.corpus_claim.rehearsed_documents -= 1;
    m.corpus_claim.byte_identity_holds = true;
    expect_refusal(
        "missing_fixture_claimed_identical",
        &m,
        &expectation,
        "CorpusClaimFalsified",
    )?;

    let mut m = receipt.clone();
    m.corpus_claim.byte_identity_holds = false;
    expect_refusal(
        "corpus_claim_falsified",
        &m,
        &expectation,
        "CorpusClaimFalsified",
    )?;

    let mut m = receipt.clone();
    m.hermeticity.network_observed = true;
    expect_refusal(
        "non_hermetic_network",
        &m,
        &expectation,
        "NonHermeticRehearsal",
    )?;

    let mut m = receipt.clone();
    m.hermeticity.switch_in_memory_only = false;
    expect_refusal(
        "switch_not_in_memory",
        &m,
        &expectation,
        "NonHermeticRehearsal",
    )?;

    let mut m = receipt.clone();
    m.documents[0].tei_path = "/tmp/leak.tei.xml".to_owned();
    expect_refusal(
        "absolute_fixture_path",
        &m,
        &expectation,
        "NonHermeticInput",
    )?;

    let mut m = receipt.clone();
    m.sequence.steps = vec!["baseline".to_owned(), "returned".to_owned()];
    expect_refusal("sequence_drift", &m, &expectation, "SequenceDrift")?;

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
            Err(
                da_cli::native_corpus_rollback_rehearsal::RollbackRehearsalError::RedactionLeak { .. }
            )
        ),
        "a physical path leak was not refused"
    );

    Ok(())
}
