//! S08/T01 (M514-1lss2s) — contract test for the corpus-bound native acceptance
//! artifact.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it,
//! rather than compiling an empty target. It re-derives the frozen pins and the
//! `610/531/79` partition from the committed carrier bytes, binds the committed
//! artifact and asserts its canonical byte re-serialization, its denominator and
//! its typed gap ledger, and rejects each named mutation.

use da_cli::native_acceptance::{
    ABSORPTION_SCHEMA_REL, AcceptanceError, AcceptanceMetricRow, AcceptancePopulations,
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, GAP_KIND_VOCABULARY, HOLDOUT_MANIFEST_REL, REFERENCE_PINS_REL,
    REFERENCE_PROBE_REL, derive_expectation, validate_native_acceptance,
};
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

fn sha256_prefixed(bytes: &[u8]) -> String {
    da_cli::native_family_receipt::sha256_prefixed(bytes)
}

/// Every named mutation must be refused by name; a mutation that slips through
/// fails this test.
fn expect_refusal(
    label: &str,
    receipt: &da_cli::native_acceptance::NativeAcceptanceReceipt,
    expectation: &da_cli::native_acceptance::AcceptanceExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_native_acceptance(receipt, expectation) {
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
fn native_corpus_acceptance_artifact_is_canonical_and_census_bound() -> Result<(), Box<dyn Error>> {
    let root = repo_root();

    // ── re-derive the expectation from committed bytes ─────────────────────
    let pins_bytes = read(&root, REFERENCE_PINS_REL)?;
    let probe_bytes = read(&root, REFERENCE_PROBE_REL)?;
    let schema_bytes = read(&root, ABSORPTION_SCHEMA_REL)?;
    let manifest_bytes = read(&root, HOLDOUT_MANIFEST_REL)?;
    let expectation = derive_expectation(
        std::str::from_utf8(&pins_bytes)?,
        &sha256_prefixed(&pins_bytes),
        std::str::from_utf8(&probe_bytes)?,
        &sha256_prefixed(&probe_bytes),
        std::str::from_utf8(&schema_bytes)?,
        &sha256_prefixed(&schema_bytes),
        std::str::from_utf8(&manifest_bytes)?,
    )?;

    // ── frozen partition is the full denominator, no shrink ────────────────
    assert_eq!(expectation.populations.assigned, 610, "assigned drifted");
    assert_eq!(
        expectation.populations.measurable, 531,
        "measurable drifted"
    );
    assert_eq!(expectation.populations.excluded, 79, "excluded drifted");
    assert_eq!(expectation.populations.refused, 0, "refused drifted");
    assert_eq!(expectation.documents.len(), 610, "document rows drifted");
    assert_eq!(expectation.thresholds.len(), 3, "frozen thresholds drifted");

    // ── artifact exists, binds the exact carriers and validates ────────────
    let artifact_bytes = read(&root, DEFAULT_ACCEPTANCE_ARTIFACT_REL)?;
    let receipt =
        da_cli::native_acceptance::NativeAcceptanceReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_native_acceptance(&receipt, &expectation)?;

    // ── canonical byte re-serialization is stable ──────────────────────────
    let reserialized = receipt.artifact_json_bytes()?;
    assert_eq!(
        reserialized, artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the denominator is carried, never shrunken ─────────────────────────
    assert_eq!(receipt.populations.assigned, 610);
    assert_eq!(receipt.populations.measurable, 531);
    assert_eq!(receipt.populations.excluded, 79);
    assert_eq!(receipt.documents.len(), 610);

    // ── every frozen threshold is measured OR typed-gapped ─────────────────
    for threshold in &expectation.thresholds {
        let measured = receipt
            .metrics
            .iter()
            .any(|row| row.metric_id == threshold.metric_id && row.denominator > 0);
        let gapped = receipt
            .gaps
            .iter()
            .any(|gap| gap.threshold_id == threshold.threshold_id && !gap.reason.trim().is_empty());
        assert!(
            measured || gapped,
            "frozen threshold `{}` is neither measured nor typed-gapped",
            threshold.threshold_id
        );
        assert!(
            !(measured && gapped),
            "frozen threshold `{}` is both measured and gapped",
            threshold.threshold_id
        );
    }

    // ── the gap ledger is typed and named, not silent ──────────────────────
    for gap in &receipt.gaps {
        assert!(
            GAP_KIND_VOCABULARY.contains(&gap.kind.as_str()),
            "gap kind `{}` is not in the vocabulary",
            gap.kind
        );
        assert!(!gap.reason.trim().is_empty(), "gap carries an empty reason");
    }
    let not_observable = receipt
        .gaps
        .iter()
        .filter(|gap| gap.kind == "not_observable")
        .count();
    let threshold_pending = receipt
        .gaps
        .iter()
        .filter(|gap| gap.kind == "threshold_pending")
        .count();
    assert_eq!(not_observable, 2, "expected two not_observable gaps");
    assert_eq!(
        threshold_pending, 12,
        "expected twelve threshold_pending families"
    );

    // ── named mutations are all refused ────────────────────────────────────
    let mut m = receipt.clone();
    m.documents[0].sha256 = "sha256:0000".to_owned();
    expect_refusal("sha256_drift", &m, &expectation, "DocumentMismatch")?;

    let mut m = receipt.clone();
    m.documents[0].id = "unknown-9999.00000".to_owned();
    expect_refusal("unknown_document", &m, &expectation, "DocumentMismatch")?;

    let mut m = receipt.clone();
    m.guard.installed = false;
    expect_refusal("guardless_zero", &m, &expectation, "GuardlessZero")?;

    let mut m = receipt.clone();
    m.guard.observed_java_calls = 1;
    expect_refusal("java_consulted", &m, &expectation, "JavaConsulted")?;

    let mut m = receipt.clone();
    m.metrics.push(AcceptanceMetricRow {
        metric_id: "tuned_threshold_literal".to_owned(),
        numerator: 1,
        denominator: 2,
    });
    expect_refusal("tuned_threshold", &m, &expectation, "UnknownThreshold")?;

    let mut m = receipt.clone();
    m.populations = AcceptancePopulations {
        assigned: 609,
        measurable: 531,
        refused: 0,
        excluded: 78,
    };
    expect_refusal("shrunken_denominator", &m, &expectation, "Denominator")?;

    let mut m = receipt.clone();
    m.gaps[0].kind = "mystery".to_owned();
    expect_refusal("unknown_gap_kind", &m, &expectation, "UnknownGapKind")?;

    let mut m = receipt.clone();
    m.gaps[0].reason = String::new();
    expect_refusal("empty_gap_reason", &m, &expectation, "EmptyGapReason")?;

    // A measured metric row with a zero (silent) denominator is refused by name.
    let mut m = receipt.clone();
    m.metrics[0].denominator = 0;
    expect_refusal(
        "zero_denominator",
        &m,
        &expectation,
        "MetricZeroDenominator",
    )?;

    // A frozen threshold measured with a positive denominator while its gap cell
    // is still carried is refused by name, never admitted as both.
    let mut m = receipt.clone();
    m.metrics.push(AcceptanceMetricRow {
        metric_id: "extraction.native_capability_recall".to_owned(),
        numerator: 1,
        denominator: 2,
    });
    expect_refusal(
        "both_measured_and_gapped",
        &m,
        &expectation,
        "ThresholdBothMeasuredAndGapped",
    )?;

    // A redaction leak must be refused by the serializer itself.
    let mut m = receipt.clone();
    m.documents[0].source_pdf = "/tmp/leak".to_owned();
    let leak = m.artifact_json_bytes();
    assert!(
        matches!(leak, Err(AcceptanceError::RedactionLeak { .. })),
        "a physical path leak was not refused"
    );

    Ok(())
}
