//! S02/T04 (M516-th0oby) — contract test for the committed native-route
//! independence receipt.
//!
//! This test reads committed bytes alone (`artifacts/parse_compat/
//! native_route_independence.json`, the route receipt and the native TEI
//! emission manifest). It never reads the gitignored scholarly PDF corpus and
//! never runs the route. It proves the committed receipt replays from its
//! carriers, is `measured` with a non-empty denominator, carries an evidence and
//! a comparability identity, keeps the five locks closed and the claim scope
//! diagnostic, leaves the product `java_independence` axis untouched, and that
//! the independent validator refuses the named receipt-level mutations it owns.

use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, PRODUCT_COVERAGE_EFFECT_NONE, THRESHOLDS_NOT_ACCEPTED,
};
use da_cli::native_route_independence::{
    NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION, RouteIndependenceExpectation,
    RouteIndependenceReceipt, RouteIndependenceRefusal, build_route_independence_receipt,
    derive_route_independence_expectation, validate_route_independence_receipt,
};
use serde_json::Value;

const INDEPENDENCE: &str =
    include_str!("../../../artifacts/parse_compat/native_route_independence.json");
const ROUTE: &str = include_str!("../../../artifacts/parse_compat/native_route_e2e.json");
const EMISSION: &str =
    include_str!("../../../artifacts/parse_compat/native_route_tei_emission.json");
const BASELINE: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_baseline.json");

const ZERO_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn receipt() -> RouteIndependenceReceipt {
    RouteIndependenceReceipt::parse_json_bytes(INDEPENDENCE.as_bytes())
        .expect("the committed independence artifact parses")
}

fn expected() -> RouteIndependenceExpectation {
    derive_route_independence_expectation(ROUTE.as_bytes(), EMISSION.as_bytes())
        .expect("the expectation recomputes from the committed carriers")
}

fn refuse(result: Result<(), RouteIndependenceRefusal>) -> RouteIndependenceRefusal {
    result.expect_err("the mutation must be refused")
}

// ────────────────────────────── replay ──────────────────────────────

#[test]
fn committed_receipt_replays_and_validates() {
    let receipt = receipt();
    assert_eq!(
        receipt.schema_version,
        NATIVE_ROUTE_INDEPENDENCE_SCHEMA_VERSION
    );
    validate_route_independence_receipt(&receipt, &expected()).expect("the receipt validates");
}

#[test]
fn committed_artifact_is_reproducible_from_carriers() {
    let built = build_route_independence_receipt(ROUTE.as_bytes(), EMISSION.as_bytes(), true)
        .expect("the builder derives from the committed carriers");
    assert_eq!(built, receipt());
    assert_eq!(
        built.artifact_json_bytes().unwrap().as_slice(),
        INDEPENDENCE.as_bytes()
    );
}

#[test]
fn committed_receipt_is_measured_with_a_non_empty_denominator() {
    let receipt = receipt();
    assert_eq!(receipt.axis.state, "measured");
    assert!(
        receipt.axis.denominator.documents > 0,
        "documents must be > 0"
    );
    assert!(receipt.axis.denominator.pages > 0, "pages must be > 0");
    assert_eq!(
        receipt.axis.value, 0,
        "the native route observes zero Java calls"
    );
    assert_eq!(receipt.axis.denominator.documents, receipt.corpus.documents);
    assert_eq!(receipt.axis.denominator.pages, receipt.corpus.pages);
}

#[test]
fn committed_receipt_carries_evidence_and_comparability_identity() {
    let receipt = receipt();
    assert!(
        receipt
            .axis
            .evidence_identity
            .guard_provenance_sha256
            .starts_with("sha256:")
    );
    assert!(
        receipt.axis.evidence_identity.emission_sha256.is_some(),
        "the emission artifact must be bound by digest"
    );
    let comparability = &receipt.axis.comparability_identity;
    assert!(!comparability.corpus_id.is_empty());
    assert!(!comparability.profile.is_empty());
    assert!(!comparability.oracle.is_empty());
    assert!(comparability.source_revision.starts_with("sha256:"));
    assert_eq!(comparability.corpus_id, receipt.corpus.corpus_id);
}

#[test]
fn committed_receipt_keeps_the_claim_scope_and_locks_closed() {
    let receipt = receipt();
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    for (lock, value) in da_cli::native_family_receipt::REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(lock).map(String::as_str),
            Some(value),
            "lock {lock}"
        );
    }
    for (_, flag) in [
        ("raw_text", receipt.redaction.raw_text),
        ("raw_pdf", receipt.redaction.raw_pdf),
        ("tei_xml", receipt.redaction.tei_xml),
        ("images", receipt.redaction.images),
        ("physical_paths", receipt.redaction.physical_paths),
        ("hostname", receipt.redaction.hostname),
        ("executable", receipt.redaction.executable),
    ] {
        assert!(!flag, "a redaction flag must stay false");
    }
}

/// The receipt is diagnostic-only: the product `java_independence` scorecard row
/// must still read `not_measured` (D499/D508). Moving it needs explicit human
/// authorization and is not this task's business.
#[test]
fn product_java_independence_axis_stays_not_measured() {
    let baseline: Value = serde_json::from_str(BASELINE).expect("the baseline parses");
    let row = baseline["measurements"]
        .as_array()
        .expect("measurements is an array")
        .iter()
        .find(|row| row["metric_id"] == "java_independence.java_free_parse_route")
        .expect("the java_independence row exists");
    assert_eq!(row["state"], "not_measured");
}

// ───────────────────────────── refusals ─────────────────────────────

#[test]
fn refuses_an_unbound_denominator() {
    let mut mutated = receipt();
    mutated.corpus.corpus_id.clear();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundDenominator { .. }
    ));
}

#[test]
fn refuses_a_zero_denominator() {
    let mut mutated = receipt();
    mutated.corpus.documents = 0;
    mutated.axis.denominator.documents = 0;
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::ZeroDenominator { .. }
    ));
}

#[test]
fn refuses_an_unbound_evidence_identity() {
    let mut mutated = receipt();
    mutated.axis.evidence_identity.guard_provenance_sha256 = "not-a-digest".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundEvidenceIdentity { .. }
    ));
}

#[test]
fn refuses_an_unbound_comparability_identity() {
    let mut mutated = receipt();
    mutated.axis.comparability_identity.oracle.clear();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundComparabilityIdentity { .. }
    ));
}

#[test]
fn refuses_a_non_measured_axis() {
    let mut mutated = receipt();
    mutated.axis.state = "not_measured".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::AxisNotMeasured { .. }
    ));
}

#[test]
fn refuses_an_unguarded_zero() {
    let mut mutated = receipt();
    assert_eq!(mutated.axis.value, 0);
    mutated.guard.positive_control_observed = false;
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::GuardNotInstalledForObservedZero { .. }
    ));

    let mut uninstalled = receipt();
    uninstalled.guard.installed = false;
    assert!(matches!(
        refuse(validate_route_independence_receipt(
            &uninstalled,
            &expected()
        )),
        RouteIndependenceRefusal::GuardNotInstalledForObservedZero { .. }
    ));
}

#[test]
fn refuses_an_opened_lock() {
    let mut mutated = receipt();
    mutated
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::LockOpened { .. }
    ));
}

#[test]
fn refuses_a_non_diagnostic_claim_scope() {
    let mut mutated = receipt();
    mutated.claim_scope = "product".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::ClaimScopeNotDiagnostic { .. }
    ));
}

#[test]
fn refuses_a_product_coverage_claim() {
    let mut mutated = receipt();
    mutated.product_coverage_effect = "full".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::ProductCoverageClaimed { .. }
    ));
}

#[test]
fn refuses_a_threshold_claim() {
    let mut mutated = receipt();
    mutated.thresholds = "accepted".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::ThresholdClaimed { .. }
    ));
}

// ───────────────────────────── drift ─────────────────────────────

#[test]
fn refuses_a_denominator_drift_from_the_carriers() {
    let mut mutated = receipt();
    mutated.corpus.documents += 1;
    mutated.axis.denominator.documents += 1;
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundDenominator { .. }
    ));
}

#[test]
fn refuses_a_guard_provenance_drift() {
    let mut mutated = receipt();
    mutated.axis.evidence_identity.guard_provenance_sha256 = ZERO_DIGEST.to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundEvidenceIdentity { .. }
    ));
}

#[test]
fn refuses_an_emission_digest_drift() {
    let mut mutated = receipt();
    mutated.axis.evidence_identity.emission_sha256 = Some(ZERO_DIGEST.to_owned());
    mutated.emission.as_mut().unwrap().artifact_sha256 = ZERO_DIGEST.to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundEvidenceIdentity { .. }
    ));
}

#[test]
fn refuses_a_comparability_drift() {
    let mut mutated = receipt();
    mutated.axis.comparability_identity.oracle = "another-oracle".to_owned();
    assert!(matches!(
        refuse(validate_route_independence_receipt(&mutated, &expected())),
        RouteIndependenceRefusal::UnboundComparabilityIdentity { .. }
    ));
}

#[test]
fn refuses_a_leaked_marker() {
    let mut mutated = receipt();
    mutated.guard.observer = "<TEI leaked".to_owned();
    assert!(matches!(
        mutated.artifact_json_bytes(),
        Err(RouteIndependenceRefusal::RedactionLeak { .. })
    ));
}

#[test]
fn refuses_a_malformed_carrier() {
    assert!(matches!(
        RouteIndependenceReceipt::parse_json_bytes(b"{not json"),
        Err(RouteIndependenceRefusal::MalformedCarrier { .. })
    ));
}
