//! M516-th0oby / S11 / T03 — hermetic contract test for the committed
//! independent-preservation-candidate artifact.
//!
//! This test reads committed bytes alone (the candidate artifact, the frozen
//! pins, the S03 threshold candidates, the DATA carriers and the git-tracked
//! reference TEI fixtures). It never reads the gitignored scholarly PDF corpus
//! and never touches the network. It proves the committed artifact replays
//! byte-for-byte from its carriers, that the candidate series is genuinely
//! independent of the frozen reference (so every threshold honestly resolves
//! `not_comparable` / `mixed_scope_comparison`), that the frozen records stay
//! byte-unchanged, and that the independent validator refuses the named
//! receipt- and derivation-level mutations it owns.

#[path = "../examples/support/native_threshold_measure.rs"]
mod native_threshold_measure;

use da_cli::independent_preservation_candidate::{
    ABSORPTION_SCHEMA_REL, ACCEPTANCE_PACKAGE_REL, DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL,
    INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION, IndependentPreservationCandidate,
    IndependentPreservationExpectation, IndependentPreservationRefusal, LAYOUT_DECLARATION_REL,
    M515_RECONCILIATION_REL, RELEASE_PROBE_REL, RESOLUTION_NOT_COMPARABLE, ROUTE_INDEPENDENCE_REL,
    build_independent_preservation_receipt, derive_independent_preservation_expectation,
    validate_independent_preservation_receipt,
};
use da_cli::native_acceptance_package::{
    SERIES_IDENTITY_BASIS_CONSERVATION, SERIES_IDENTITY_BASIS_FINGERPRINTS_DIFFER,
    SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES, SERIES_IDENTITY_VERDICT_SAME_SERIES,
    derive_series_identity,
};
use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    THRESHOLDS_NOT_ACCEPTED, sha256_prefixed,
};
use da_cli::native_route_threshold_candidates::ThresholdCandidateRow;
use da_parse::REASON_MIXED_SCOPE_COMPARISON;
use native_threshold_measure::build_threshold_candidates_artifact;
use serde_json::Value;
use std::fs;

const CANDIDATE: &str =
    include_str!("../../../artifacts/parse_compat/independent_preservation_candidate.json");
const PINS: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_reference_pins.json");
const S03_CANDIDATES: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_threshold_candidates.json");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const SCHEMA: &str = include_str!("../../../data/parse/absorption_progress.schema.yaml");
const HOLDOUT: &str = include_str!("../../../data/parse/holdout_manifests.yaml");
const INDEPENDENCE: &str =
    include_str!("../../../artifacts/parse_compat/native_route_independence.json");
const PROBE: &str =
    include_str!("../../../artifacts/parse_compat/external_holdout_release_probe.json");
const PACKAGE: &str =
    include_str!("../../../artifacts/parse_compat/grobid_retirement_acceptance_package.json");
const M515: &str =
    include_str!("../../../artifacts/parse_compat/m515_requirement_coverage_reconciliation.json");

/// The frozen reference series fingerprint the acceptance package builds; the
/// candidate must differ from it.
const REFERENCE_FINGERPRINT: &str =
    "sha256:c5a9d938f31660452032b4d9b86a4e8bebb621f93ae7d0a57bb28ed12778f161";
/// The frozen reference pins digest the closed threshold policy names.
const PINS_SHA256: &str = "sha256:2befd08dd8b095a6e8e6ea17e6b73e92e78e282ab98cbad64c4727218d6fe158";
/// The three frozen absorption-progress thresholds.
const EXPECTED_THRESHOLD_COUNT: usize = 3;
/// The frozen reference population document count (the `holdout-partition-610-...` census).
const FROZEN_POPULATION_DOCUMENTS: usize = 610;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn receipt() -> IndependentPreservationCandidate {
    IndependentPreservationCandidate::parse_json_bytes(CANDIDATE.as_bytes())
        .expect("the committed candidate artifact parses")
}

fn expectation() -> IndependentPreservationExpectation {
    derive_independent_preservation_expectation(&repo_root())
        .expect("the committed carriers derive the independent expectation")
}

fn measured_rows() -> Vec<ThresholdCandidateRow> {
    build_threshold_candidates_artifact(&repo_root())
        .expect("the measurement replays from the committed carriers")
        .receipt
        .threshold_candidates
}

fn refuse(result: Result<(), IndependentPreservationRefusal>) -> IndependentPreservationRefusal {
    result.expect_err("the mutation must be refused")
}

fn yaml(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).expect("a DATA carrier parses")
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("a JSON carrier parses")
}

/// Materialize the committed carriers into a scratch OS temp root (never a
/// project path) so a derivation can be driven against mutated bytes.
fn materialize_root(layout: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch root");
    let root = dir.path();
    for (rel, bytes) in [
        (LAYOUT_DECLARATION_REL, layout),
        (ROUTE_INDEPENDENCE_REL, INDEPENDENCE),
        (RELEASE_PROBE_REL, PROBE),
        (ABSORPTION_SCHEMA_REL, SCHEMA),
        (ACCEPTANCE_PACKAGE_REL, PACKAGE),
        (M515_RECONCILIATION_REL, M515),
    ] {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("a parent directory")).expect("mkdir -p");
        fs::write(&path, bytes).expect("write carrier");
    }
    dir
}

// ────────────────────────────── replay ──────────────────────────────

#[test]
fn committed_artifact_parses_and_validates() {
    let receipt = receipt();
    assert_eq!(
        receipt.schema_version,
        INDEPENDENT_PRESERVATION_CANDIDATE_SCHEMA_VERSION
    );
    validate_independent_preservation_receipt(&expectation(), &receipt)
        .expect("the committed receipt validates against the recomputed expectation");
}

/// The strongest replay: rebuild the whole artifact from the committed carriers
/// (no PDFs, no network) and require byte-identity with the committed bytes.
#[test]
fn committed_artifact_is_reproducible_from_the_carriers() {
    let expectation = expectation();
    let rebuilt = build_independent_preservation_receipt(&expectation, &measured_rows())
        .expect("the receipt rebuilds from the committed carriers");
    // Re-validate against the independently recomputed expectation: byte-identity
    // alone would not prove the rebuilt receipt is itself honest.
    validate_independent_preservation_receipt(&expectation, &rebuilt)
        .expect("the rebuilt receipt validates against the recomputed expectation");
    assert_eq!(
        rebuilt
            .artifact_json_bytes()
            .expect("serializes redacted")
            .as_slice(),
        CANDIDATE.as_bytes(),
        "the committed candidate artifact must reproduce byte-for-byte"
    );
}

// ────────────────────────── core slice facts ──────────────────────────

/// The slice's headline: the candidate is genuinely independent of the frozen
/// reference, so the derived identity is `independent_series`.
#[test]
fn candidate_series_is_independent_of_the_frozen_reference() {
    let receipt = receipt();
    assert_ne!(
        receipt.candidate_series.fingerprint, receipt.reference_series.fingerprint,
        "the candidate must not be the frozen reference series"
    );
    assert_eq!(
        receipt.series_identity.verdict,
        SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES
    );
    assert_eq!(
        receipt.series_identity.basis,
        SERIES_IDENTITY_BASIS_FINGERPRINTS_DIFFER
    );
    assert_eq!(
        receipt.series_identity.candidate_fingerprint,
        receipt.candidate_series.fingerprint
    );
    assert_eq!(
        receipt.series_identity.reference_fingerprint,
        receipt.reference_series.fingerprint
    );
    assert_eq!(receipt.reference_series.fingerprint, REFERENCE_FINGERPRINT);
}

/// Because the population differs, every frozen threshold honestly resolves
/// `not_comparable` with the typed `mixed_scope_comparison` reason — never a
/// forced `matched`, and never the pin re-labelled as the candidate.
#[test]
fn every_frozen_threshold_resolves_not_comparable_against_the_candidate() {
    let receipt = receipt();
    assert_eq!(receipt.thresholds_resolved.len(), EXPECTED_THRESHOLD_COUNT);
    for row in &receipt.thresholds_resolved {
        assert_eq!(row.resolution, RESOLUTION_NOT_COMPARABLE);
        assert_eq!(row.reason, REASON_MIXED_SCOPE_COMPARISON);
        assert!(
            row.candidate.denominator > 0,
            "a candidate denominator must be non-empty"
        );
        assert_ne!(
            row.candidate.fraction, row.reference.fraction,
            "the candidate must not be the pin re-labelled"
        );
    }
}

/// Every threshold stays unaccepted and all five promotion locks stay closed.
#[test]
fn claim_scope_locks_and_threshold_state_stay_closed() {
    let receipt = receipt();
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(lock).map(String::as_str),
            Some(value),
            "lock {lock}"
        );
    }
    for row in &receipt.thresholds_resolved {
        assert_ne!(row.resolution, "accepted", "no threshold may be accepted");
    }
}

/// The earlier conservation result is retained as its own disclosed record: it
/// still reads `same_series`, its bindings recompute from the committed carrier
/// bytes, and the m515 residual stays disclosed, never resolved.
#[test]
fn retained_conservation_record_stays_same_series_and_disclosed() {
    let receipt = receipt();
    let retained = &receipt.retained_conservation;
    assert_eq!(retained.verdict, SERIES_IDENTITY_VERDICT_SAME_SERIES);
    assert!(retained.basis.contains(SERIES_IDENTITY_BASIS_CONSERVATION));

    let package_binding = retained
        .bindings
        .iter()
        .find(|binding| binding.path == ACCEPTANCE_PACKAGE_REL)
        .expect("the retained record binds the acceptance package");
    assert_eq!(package_binding.sha256, sha256_prefixed(PACKAGE.as_bytes()));
    let m515_binding = retained
        .bindings
        .iter()
        .find(|binding| binding.path == M515_RECONCILIATION_REL)
        .expect("the retained record binds the m515 reconciliation");
    assert_eq!(m515_binding.sha256, sha256_prefixed(M515.as_bytes()));

    let package = json(PACKAGE);
    assert_eq!(
        package["carrier_independence"]["series_identity"]["verdict"],
        SERIES_IDENTITY_VERDICT_SAME_SERIES,
        "the acceptance package still reads same_series"
    );

    let m515 = json(M515);
    let residual = &m515["c1_series_identity_residual"];
    assert_eq!(residual["presented_as_resolved"], false);
    assert_eq!(residual["must_not_be_presented_as_resolved"], true);
}

/// The frozen records are byte-unchanged: the pins digest is the value the
/// closed threshold policy names, and the S03 threshold candidates are still
/// present and unaccepted.
#[test]
fn frozen_reference_pins_schema_and_s03_candidates_are_untouched() {
    let pins_digest = sha256_prefixed(PINS.as_bytes());
    assert_eq!(pins_digest, PINS_SHA256);

    let schema = yaml(SCHEMA);
    assert_eq!(
        schema["threshold_policy"]["reference_pins"]["sha256"].as_str(),
        Some(PINS_SHA256),
        "the closed threshold policy must name the same pin digest"
    );

    let s03 = json(S03_CANDIDATES);
    assert_eq!(s03["thresholds"], THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(
        DEFAULT_INDEPENDENT_PRESERVATION_CANDIDATE_REL,
        "artifacts/parse_compat/independent_preservation_candidate.json"
    );
}

/// The artifact's series keys are derived from the committed carriers, not
/// restated — the candidate binds the route corpus / route-axis profile, and the
/// reference binds the frozen census / executed route profile / freeze revision.
#[test]
fn the_series_keys_are_derived_from_the_committed_carriers() {
    let receipt = receipt();

    let layout = yaml(LAYOUT);
    assert_eq!(
        layout["native_route_corpus"]["id"].as_str(),
        Some(receipt.candidate_series.corpus_id.as_str())
    );

    let independence = json(INDEPENDENCE);
    let identity = &independence["axis"]["comparability_identity"];
    assert_eq!(
        identity["profile"].as_str(),
        Some(receipt.candidate_series.profile.as_str())
    );
    assert_eq!(
        identity["source_revision"].as_str(),
        Some(receipt.candidate_series.source_revision.as_str())
    );

    let schema = yaml(SCHEMA);
    assert_eq!(
        schema["assigned_population"]["census_id"].as_str(),
        Some(receipt.reference_series.corpus_id.as_str())
    );
    assert_eq!(
        schema["threshold_policy"]["freeze_revision"].as_str(),
        Some(receipt.reference_series.source_revision.as_str())
    );

    let probe = json(PROBE);
    assert_eq!(
        probe["evaluated_population"]["oracle"].as_str(),
        Some(receipt.candidate_series.oracle.as_str())
    );
    assert_eq!(
        probe["evaluated_population"]["oracle"].as_str(),
        Some(receipt.reference_series.oracle.as_str())
    );
    assert_eq!(
        probe["executed_population"]["execution_route_profile"].as_str(),
        Some(receipt.reference_series.profile.as_str())
    );

    let holdout = yaml(HOLDOUT);
    assert_eq!(
        holdout["articles"]
            .as_sequence()
            .expect("the holdout manifest names its articles")
            .len(),
        FROZEN_POPULATION_DOCUMENTS
    );
    assert!(receipt.reference_series.corpus_id.contains("610"));
}

// ───────────────────────────── refusals ─────────────────────────────

/// A candidate whose series is the frozen reference is refused by name: an
/// "independent" artifact that is the same series is exactly the self-reference
/// the honesty rule forbids.
#[test]
fn refuses_a_self_reference_candidate_series() {
    let mut expectation = expectation();
    let reference = expectation.reference_series.clone();
    expectation.candidate_series = reference.clone();
    expectation.series_identity =
        derive_series_identity(&Some(reference.clone()), &Some(reference));
    assert_eq!(
        expectation.series_identity.verdict,
        SERIES_IDENTITY_VERDICT_SAME_SERIES
    );
    let error = build_independent_preservation_receipt(&expectation, &measured_rows())
        .expect_err("a self-referencing candidate is refused");
    assert!(matches!(
        error,
        IndependentPreservationRefusal::CandidateNotIndependent { .. }
    ));
}

/// An empty candidate series key is refused by name at the derivation boundary.
#[test]
fn refuses_an_empty_candidate_series_key() {
    let blanked_layout = LAYOUT.replace("  id: native-pdf-to-tei-route-corpus", "  id: \"\"");
    assert_ne!(blanked_layout, LAYOUT, "the layout mutation must apply");
    let dir = materialize_root(&blanked_layout);
    let error = derive_independent_preservation_expectation(dir.path())
        .expect_err("an empty candidate series key is refused");
    assert!(matches!(
        error,
        IndependentPreservationRefusal::SeriesKeyEmpty { side, key }
            if side == "candidate" && key == "corpus_id"
    ));
}

/// An absent candidate-series block does not parse: the field is required.
#[test]
fn refuses_an_absent_candidate_series_block() {
    let mut value: Value = json(CANDIDATE);
    value
        .as_object_mut()
        .expect("the artifact is a JSON object")
        .remove("candidate_series");
    let bytes = serde_json::to_vec_pretty(&value).expect("the mutated value serializes");
    assert!(matches!(
        IndependentPreservationCandidate::parse_json_bytes(&bytes),
        Err(IndependentPreservationRefusal::MalformedCarrier { .. })
    ));
}

#[test]
fn refuses_a_threshold_row_resolved_accepted() {
    let mut mutated = receipt();
    mutated.thresholds_resolved[0].resolution = "accepted".to_owned();
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::ResolutionOutsideVocabulary { .. }
    ));
}

#[test]
fn refuses_an_opened_lock() {
    let mut mutated = receipt();
    mutated
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::LockOpened { .. }
    ));
}

#[test]
fn refuses_a_missing_retained_conservation_binding() {
    let mut mutated = receipt();
    mutated.retained_conservation.bindings.clear();
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::RetainedBindingDrift { .. }
    ));
}

#[test]
fn refuses_a_retained_conservation_verdict_that_is_not_same_series() {
    let mut mutated = receipt();
    mutated.retained_conservation.verdict = SERIES_IDENTITY_VERDICT_INDEPENDENT_SERIES.to_owned();
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::RetainedConservationNotSameSeries { .. }
    ));
}

#[test]
fn refuses_a_zero_candidate_denominator() {
    let mut mutated = receipt();
    mutated.thresholds_resolved[0].candidate.denominator = 0;
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::UnboundCandidateDenominator { .. }
    ));
}

#[test]
fn refuses_a_restated_fraction_value() {
    let mut mutated = receipt();
    mutated.thresholds_resolved[0].candidate.value = -1.0;
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::FractionValueNotDerived { .. }
    ));
}

#[test]
fn refuses_a_receipt_with_no_threshold_rows() {
    let mut mutated = receipt();
    mutated.thresholds_resolved.clear();
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::ThresholdRowsEmpty
    ));
}

#[test]
fn refuses_a_widened_claim_scope() {
    let mut mutated = receipt();
    mutated.claim_scope = "product".to_owned();
    assert!(matches!(
        refuse(validate_independent_preservation_receipt(
            &expectation(),
            &mutated
        )),
        IndependentPreservationRefusal::ClaimScopeNotDiagnostic { .. }
    ));
}

/// A carrier that cannot be read is refused by name, never silently skipped.
#[test]
fn refuses_a_missing_carrier() {
    let dir = tempfile::tempdir().expect("a scratch root");
    assert!(matches!(
        derive_independent_preservation_expectation(dir.path()),
        Err(IndependentPreservationRefusal::MalformedCarrier { .. })
    ));
}
