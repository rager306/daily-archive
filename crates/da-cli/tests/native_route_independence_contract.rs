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
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const GOVERNOR: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
const LAYOUT_REL: &str = "data/parse/layout_bakeoff.yaml";

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
fn committed_axis_replays_from_the_declared_corpus_and_the_emission_manifest() {
    let layout: serde_yaml::Value =
        serde_yaml::from_str(LAYOUT).expect("the slice declaration parses");
    let declared = &layout["native_route_corpus"];

    // Rebuild the axis from the committed route receipt + emission manifest and
    // require the committed bytes to match the rebuild exactly.
    let built = build_route_independence_receipt(ROUTE.as_bytes(), EMISSION.as_bytes(), true)
        .expect("the builder derives from the committed carriers");
    assert_eq!(built, receipt());
    assert_eq!(
        built.artifact_json_bytes().unwrap().as_slice(),
        INDEPENDENCE.as_bytes()
    );

    // The declared corpus is the denominator the axis is bound to; nothing is
    // hand-typed.
    assert_eq!(
        built.corpus.corpus_id,
        declared["id"].as_str().expect("a declared corpus id")
    );
    assert_eq!(
        built.corpus.documents,
        declared["corpus_n"]
            .as_u64()
            .expect("a declared document count")
    );
    assert_eq!(
        built.corpus.pages,
        declared["page_denominator"]
            .as_u64()
            .expect("a declared page denominator")
    );
    assert_eq!(built.corpus.pointer, LAYOUT_REL);
}

/// The governor half recomputes the receipt's pins from the committed carriers
/// (`scripts/governor/adapters/absorption_progress_reader.py`).  This test
/// asserts the *agreement shape* the governor's closed vocabulary names — the
/// same population/document denominator across the route receipt, the emission
/// manifest and the independence receipt — without importing Python.
#[test]
fn committed_axis_agrees_with_the_governor_population_denominator() {
    let receipt = receipt();
    let emission: Value = serde_json::from_str(EMISSION).expect("the emission artifact parses");
    let route: Value = serde_json::from_str(ROUTE).expect("the route receipt parses");
    let governor: serde_yaml::Value =
        serde_yaml::from_str(GOVERNOR).expect("the governor rule parses");
    let block = &governor["native_route_independence"];

    // The governor's closed denominator vocabulary and measured state.
    let denominator_fields: Vec<&str> = block["expected"]["denominator_fields"]
        .as_sequence()
        .expect("denominator_fields")
        .iter()
        .map(|value| value.as_str().expect("a denominator field"))
        .collect();
    assert_eq!(denominator_fields, ["documents", "pages"]);
    let states: Vec<&str> = block["expected"]["measurement_state_vocabulary"]
        .as_sequence()
        .expect("measurement_state_vocabulary")
        .iter()
        .map(|value| value.as_str().expect("a state"))
        .collect();
    assert!(states.contains(&receipt.axis.state.as_str()));

    // The route receipt's governed population.
    let population = route["populations"]
        .as_array()
        .expect("populations")
        .iter()
        .find(|population| population["population"] == "native_route")
        .expect("the native_route population");
    let population_documents = population["documents"]
        .as_array()
        .expect("population documents");
    let route_documents = population_documents.len() as u64;
    let route_pages: u64 = population_documents
        .iter()
        .map(|document| document["pages"].as_u64().expect("a page count"))
        .sum();

    // Same population/document denominator across every committed carrier.
    assert_eq!(receipt.axis.denominator.documents, receipt.corpus.documents);
    assert_eq!(receipt.axis.denominator.pages, receipt.corpus.pages);
    assert_eq!(receipt.corpus.documents, route_documents);
    assert_eq!(receipt.corpus.pages, route_pages);
    assert_eq!(
        route_documents,
        emission["corpus"]["document_count"]
            .as_u64()
            .expect("an emission document count")
    );
    assert_eq!(
        route_pages,
        emission["corpus"]["page_denominator"]
            .as_u64()
            .expect("an emission page denominator")
    );
    let binding = receipt.emission.as_ref().expect("the emission binding");
    assert_eq!(binding.document_count, route_documents);
    assert_eq!(binding.page_denominator, route_pages);

    // The emission envelope agrees with the receipt on the closed envelope.
    assert_eq!(
        emission["claim_scope"].as_str().expect("a claim scope"),
        receipt.claim_scope
    );
    assert_eq!(
        emission["product_coverage_effect"]
            .as_str()
            .expect("a product coverage effect"),
        receipt.product_coverage_effect
    );
    assert_eq!(
        emission["thresholds"].as_str().expect("thresholds"),
        receipt.thresholds
    );
    for (lock, value) in &receipt.locks {
        assert_eq!(
            emission["locks"][lock].as_str().expect("a lock"),
            value.as_str(),
            "lock {lock}"
        );
    }
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
