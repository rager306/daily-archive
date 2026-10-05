//! M516-th0oby / S03 / T03 — contract test for the committed threshold-candidate
//! artifact.
//!
//! This test reads committed bytes alone (the candidate artifact, the frozen
//! pins, the DATA carriers and the git-tracked reference TEI fixtures). It never
//! reads the gitignored scholarly PDF corpus and never touches the network. It
//! proves the committed artifact replays from its carriers, that the measured
//! candidate is genuinely independent of the retained pin, that the reference
//! pins and the closed threshold policy are byte-unchanged, and that the
//! independent validator refuses the named receipt-level mutations it owns.

#[path = "../examples/support/native_threshold_measure.rs"]
mod native_threshold_measure;

use da_cli::native_family_receipt::{REQUIRED_LOCKS, THRESHOLDS_NOT_ACCEPTED};
use da_cli::native_route_threshold_candidates::{
    COMPARABILITY_MATCHED, COMPARABILITY_NOT_COMPARABLE,
    NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION, ThresholdCandidatesReceipt,
    ThresholdCandidatesRefusal, build_threshold_candidates_receipt, corpus_document_ids,
    derive_threshold_candidates_expectation, reference_document_bindings,
    validate_reference_bindings, validate_threshold_candidates_receipt,
};
use native_threshold_measure::build_threshold_candidates_artifact;
use serde_json::Value;

const CANDIDATES: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_threshold_candidates.json");
const PINS: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_reference_pins.json");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const SCHEMA: &str = include_str!("../../../data/parse/absorption_progress.schema.yaml");
const HOLDOUT: &str = include_str!("../../../data/parse/holdout_manifests.yaml");
const MANIFEST: &str = include_str!("../../../data/parse/acceptance_manifest.yaml");
const REGRESSION: &str = include_str!("../../../data/parse/regression_cases.yaml");
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");

const ZERO_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn receipt() -> ThresholdCandidatesReceipt {
    ThresholdCandidatesReceipt::parse_json_bytes(CANDIDATES.as_bytes())
        .expect("the committed candidate artifact parses")
}

fn expectation() -> da_cli::native_route_threshold_candidates::ThresholdCandidatesExpectation {
    derive_threshold_candidates_expectation(PINS.as_bytes(), LAYOUT, SCHEMA, HOLDOUT)
        .expect("the expectation recomputes from the committed carriers")
}

fn refuse(result: Result<(), ThresholdCandidatesRefusal>) -> ThresholdCandidatesRefusal {
    result.expect_err("the mutation must be refused")
}

fn yaml(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).expect("a DATA carrier parses")
}

// ────────────────────────────── replay ──────────────────────────────

#[test]
fn committed_receipt_replays_and_validates() {
    let receipt = receipt();
    assert_eq!(
        receipt.schema_version,
        NATIVE_ROUTE_THRESHOLD_CANDIDATES_SCHEMA_VERSION
    );
    validate_threshold_candidates_receipt(&receipt, &expectation()).expect("the receipt validates");
}

/// The strongest replay: rebuild the whole artifact from the committed carriers
/// (no PDFs, no network) and require byte-identity with the committed bytes.
#[test]
fn committed_artifact_is_reproducible_from_the_carriers() {
    let artifact =
        build_threshold_candidates_artifact(&repo_root()).expect("the measurement replays");
    // Re-validate the rebuilt receipt against the independently recomputed
    // expectation: byte-identity alone would not prove the rebuilt receipt is
    // itself honest.
    validate_threshold_candidates_receipt(&artifact.receipt, &expectation())
        .expect("the rebuilt receipt validates against the recomputed expectation");
    assert_eq!(
        artifact.bytes.as_slice(),
        CANDIDATES.as_bytes(),
        "the committed candidate artifact must reproduce byte-for-byte"
    );
}

#[test]
fn committed_receipt_names_all_three_frozen_thresholds_with_pin_and_candidate() {
    let receipt = receipt();
    let expectation = expectation();
    assert_eq!(
        receipt.threshold_candidates.len(),
        expectation.reference_thresholds.len()
    );
    assert_eq!(receipt.threshold_candidates.len(), 3);
    for threshold in &expectation.reference_thresholds {
        let row = receipt
            .threshold_candidates
            .iter()
            .find(|row| row.threshold_id == threshold.threshold_id)
            .expect("every frozen threshold must carry a candidate row");
        assert_eq!(row.metric_id, threshold.metric_id);
        assert_eq!(row.stratum, threshold.stratum);
        assert_eq!(row.reference.fraction, threshold.fraction);
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

#[test]
fn committed_receipt_keeps_the_claim_scope_locks_and_state_closed() {
    let receipt = receipt();
    assert_eq!(receipt.claim_scope, "diagnostic_only");
    assert_eq!(receipt.product_coverage_effect, "none");
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(lock).map(String::as_str),
            Some(value),
            "lock {lock}"
        );
    }
    for row in &receipt.threshold_candidates {
        assert_ne!(row.verdict, "accepted", "no threshold may be accepted");
    }
}

/// The measured corpus is the route corpus, not the frozen 610-document holdout,
/// so the honest comparability verdict is `not_comparable` — never `matched`.
#[test]
fn committed_comparability_is_not_forced_matched() {
    let receipt = receipt();
    let comparability = &receipt.corpus.comparability;
    assert_eq!(comparability.verdict, COMPARABILITY_NOT_COMPARABLE);
    assert_ne!(comparability.verdict, COMPARABILITY_MATCHED);
    assert!(!comparability.rationale.trim().is_empty());
    assert_ne!(receipt.corpus.id, receipt.corpus.population.id);
    assert_eq!(receipt.corpus.population.document_count, 610);
    for row in &receipt.threshold_candidates {
        assert_eq!(row.comparability, COMPARABILITY_NOT_COMPARABLE);
        assert_eq!(row.verdict, COMPARABILITY_NOT_COMPARABLE);
    }
}

/// The pins and the closed threshold policy are byte-unchanged: the artifact
/// binds the pins by digest and the schema names the same digest.
#[test]
fn frozen_reference_pins_and_schema_are_untouched() {
    let receipt = receipt();
    let recomputed = da_cli::native_family_receipt::sha256_prefixed(PINS.as_bytes());
    assert_eq!(receipt.frozen_reference.sha256, recomputed);
    assert_eq!(
        receipt.frozen_reference.path,
        "artifacts/parse_compat/absorption_progress_reference_pins.json"
    );

    let schema = yaml(SCHEMA);
    let declared = schema["threshold_policy"]["reference_pins"]["sha256"]
        .as_str()
        .expect("the schema names the reference pin digest");
    assert_eq!(declared, recomputed, "the schema pin must not move");

    let pins: Value = serde_json::from_str(PINS).expect("the pins parse");
    for row in &receipt.threshold_candidates {
        let pin = pins["values"]
            .as_array()
            .expect("pins.values")
            .iter()
            .find(|value| value["threshold_id"] == row.threshold_id)
            .expect("the pin exists");
        assert_eq!(
            pin["fraction"]["numerator"].as_u64(),
            Some(row.reference.fraction.numerator)
        );
        assert_eq!(
            pin["fraction"]["denominator"].as_u64(),
            Some(row.reference.fraction.denominator)
        );
    }
}

// ────────────────────────────── DATA coherence ──────────────────────────────

#[test]
fn reference_bindings_cover_the_route_corpus_exactly() {
    validate_reference_bindings(LAYOUT).expect("the bindings cover the corpus exactly");
    let corpus = corpus_document_ids(LAYOUT).expect("corpus ids parse");
    let bindings = reference_document_bindings(LAYOUT).expect("bindings parse");
    assert_eq!(corpus.len(), 12);
    assert_eq!(bindings.len(), 12);
}

#[test]
fn corridor_documents_bind_the_preserved_fixtures() {
    let regression = yaml(REGRESSION);
    let provenance = yaml(PROVENANCE);
    let bindings = reference_document_bindings(LAYOUT).expect("bindings parse");

    // The three S02 corridor documents must bind the preserved fixture each
    // regression case names, with the case's own sha256.
    let cases = regression["cases"].as_sequence().expect("regression cases");
    for binding in bindings
        .iter()
        .filter(|binding| binding.path.contains("/preserved/"))
    {
        let case = cases
            .iter()
            .find(|case| case["fixture"]["path"].as_str() == Some(binding.path.as_str()))
            .unwrap_or_else(|| panic!("no regression case names {}", binding.path));
        assert_eq!(
            case["fixture"]["sha256"].as_str(),
            Some(binding.sha256.as_str()),
            "the corridor binding sha must equal the regression case pin"
        );
        let fixture = provenance["fixtures"]
            .as_sequence()
            .expect("provenance fixtures")
            .iter()
            .find(|fixture| fixture["path"].as_str() == Some(binding.path.as_str()))
            .unwrap_or_else(|| panic!("no provenance fixture names {}", binding.path));
        assert_eq!(
            fixture["sha256"].as_str(),
            Some(binding.sha256.as_str()),
            "the corridor binding sha must equal the preserved provenance pin"
        );
    }
    let corridor_count = bindings
        .iter()
        .filter(|binding| binding.path.contains("/preserved/"))
        .count();
    assert_eq!(corridor_count, 3);
}

#[test]
fn holdout_documents_bind_the_acceptance_manifest_reference_tei() {
    let manifest = yaml(MANIFEST);
    let rows = manifest["external_holdout_contract"]["release_probe"]["reference_tei"]
        .as_sequence()
        .expect("release_probe.reference_tei");
    let bindings = reference_document_bindings(LAYOUT).expect("bindings parse");

    let mut holdout_count = 0;
    for binding in bindings
        .iter()
        .filter(|binding| binding.path.contains("/holdout/"))
    {
        let row = rows
            .iter()
            .find(|row| row["article_id"].as_str() == Some(binding.id.as_str()))
            .unwrap_or_else(|| panic!("no reference_tei row for {}", binding.id));
        assert_eq!(row["path"].as_str(), Some(binding.path.as_str()));
        let expected = row["sha256"]
            .as_str()
            .expect("a reference_tei sha")
            .strip_prefix("sha256:")
            .unwrap_or_default();
        assert_eq!(
            expected, binding.sha256,
            "the holdout binding sha must match"
        );
        holdout_count += 1;
    }
    assert_eq!(holdout_count, 9);
}

// ───────────────────────────── refusals ─────────────────────────────

#[test]
fn refuses_a_missing_candidate() {
    let mut mutated = receipt();
    mutated.threshold_candidates.remove(0);
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::CandidateAbsent { .. }
    ));
}

#[test]
fn refuses_the_pin_relabelled_as_the_candidate() {
    let mut mutated = receipt();
    let row = &mut mutated.threshold_candidates[0];
    row.candidate.fraction = row.reference.fraction;
    row.candidate.value = row.reference.value;
    row.candidate.denominator = row.reference.fraction.denominator;
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::PinRelabelledAsCandidate { .. }
    ));
}

#[test]
fn refuses_a_threshold_marked_accepted() {
    let mut mutated = receipt();
    mutated.threshold_candidates[0].verdict = "accepted".to_owned();
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::ThresholdAccepted { .. }
    ));
}

#[test]
fn refuses_an_accepted_envelope_state() {
    let mut mutated = receipt();
    mutated.thresholds = "accepted".to_owned();
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::ThresholdClaimed { .. }
    ));
}

#[test]
fn refuses_an_opened_lock() {
    let mut mutated = receipt();
    mutated
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::LockOpened { .. }
    ));
}

#[test]
fn refuses_a_comparability_verdict_without_a_shared_identity() {
    let mut mutated = receipt();
    mutated.corpus.comparability.verdict = COMPARABILITY_MATCHED.to_owned();
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::ComparabilityWithoutSharedIdentity { .. }
    ));
}

#[test]
fn refuses_an_unbound_candidate_denominator() {
    let mut mutated = receipt();
    mutated.threshold_candidates[0].candidate.denominator = 0;
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::UnboundCandidateDenominator { .. }
    ));
}

#[test]
fn refuses_a_frozen_reference_drift() {
    let mut mutated = receipt();
    mutated.frozen_reference.sha256 = ZERO_DIGEST.to_owned();
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::BindingDrift { .. }
    ));
}

#[test]
fn refuses_a_corpus_identity_drift() {
    let mut mutated = receipt();
    mutated.corpus.id = "not-the-route-corpus".to_owned();
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::BindingDrift { .. }
    ));
}

#[test]
fn refuses_a_reference_fraction_drift() {
    let mut mutated = receipt();
    mutated.threshold_candidates[0].reference.fraction.numerator += 1;
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::BindingDrift { .. }
    ));
}

#[test]
fn refuses_a_redaction_flag() {
    let mut mutated = receipt();
    mutated.redaction.tei_xml = true;
    assert!(matches!(
        refuse(validate_threshold_candidates_receipt(
            &mutated,
            &expectation()
        )),
        ThresholdCandidatesRefusal::RedactionLeak { .. }
    ));
}

#[test]
fn refuses_a_malformed_carrier() {
    assert!(matches!(
        ThresholdCandidatesReceipt::parse_json_bytes(b"{not json"),
        Err(ThresholdCandidatesRefusal::MalformedCarrier { .. })
    ));
}

/// The builder refuses a measured set that omits a frozen threshold's metric.
#[test]
fn builder_refuses_a_missing_measured_metric() {
    let expectation = expectation();
    let measured = Vec::new();
    assert!(matches!(
        build_threshold_candidates_receipt(&expectation, &measured, "reason"),
        Err(ThresholdCandidatesRefusal::CandidateAbsent { .. })
    ));
}
