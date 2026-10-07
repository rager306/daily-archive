//! M516-th0oby / S04 / T01 — contract test for the committed per-document
//! semantic-correctness receipt.
//!
//! This test reads committed bytes alone (the receipt, the native route emission
//! envelope, the layout declaration, the frozen pins, the frozen threshold
//! schema and the git-tracked reference TEI fixtures). It never reads the
//! gitignored scholarly PDF corpus and never touches the network. It proves the
//! committed receipt replays from its carriers, that every document carries a
//! closed well-formedness reason and a closed bibliography-target reason, that
//! the native-route-emission scope never reads the placeholder header as
//! populated nor an empty-target ledger as a pass, that the frozen three-document
//! aggregate and the reference pins are byte-unchanged, and that the independent
//! validator refuses each named receipt-level mutation it owns.

#[path = "../examples/support/native_semantic_measure.rs"]
mod native_semantic_measure;

use da_cli::native_family_receipt::{
    REQUIRED_LOCKS, THRESHOLDS_NOT_ACCEPTED, is_sha256_digest, sha256_prefixed,
};
use da_cli::native_route_semantic_receipt::{
    BT_ALL_REQUIRED_TARGETS_RESOLVED, BT_NO_REQUIRED_TARGETS_DECLARED, NO_AGGREGATE_VERDICT,
    PROFILE_HOLDOUT_STRUCTURAL, PROFILE_PRESERVED_RICH, SCOPE_HEADER_NATIVE_EMITTED,
    SCOPE_HEADER_TYPED_ABSENCE, SCOPE_REFERENCE_LINKS_EMPTY_TARGETS, SemanticReceiptExpectation,
    SemanticReceiptRefusal, derive_semantic_receipt_expectation, is_bibliography_target_reason,
    is_well_formedness_reason, validate_semantic_receipt,
};
use native_semantic_measure::build_semantic_receipt_artifact;
use serde_json::Value;

const RECEIPT: &str =
    include_str!("../../../artifacts/parse_compat/native_route_semantic_receipt.json");
const EMISSION: &str =
    include_str!("../../../artifacts/parse_compat/native_route_tei_emission.json");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const PINS: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_reference_pins.json");
const THRESHOLD_SCHEMA: &str = include_str!("../../../data/parse/absorption_progress.schema.yaml");
const COMPARISON: &str = include_str!("../../../data/parse/comparison_contract.yaml");

const ZERO_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn receipt() -> da_cli::native_route_semantic_receipt::NativeRouteSemanticReceipt {
    da_cli::native_route_semantic_receipt::NativeRouteSemanticReceipt::parse_json_bytes(
        RECEIPT.as_bytes(),
    )
    .expect("the committed semantic receipt parses")
}

fn expectation() -> SemanticReceiptExpectation {
    derive_semantic_receipt_expectation(LAYOUT, EMISSION.as_bytes())
        .expect("the expectation recomputes from the committed carriers")
}

fn refuse(result: Result<(), SemanticReceiptRefusal>) -> SemanticReceiptRefusal {
    result.expect_err("the mutation must be refused")
}

// ────────────────────────────── replay ──────────────────────────────

#[test]
fn committed_receipt_replays_and_validates() {
    let receipt = receipt();
    assert_eq!(receipt.schema_version, "native-route-semantic-receipt/v1");
    validate_semantic_receipt(&receipt, &expectation()).expect("the receipt validates");
}

/// The strongest replay: rebuild the whole artifact from the committed carriers
/// (no PDFs, no network) and require byte-identity with the committed bytes.
#[test]
fn committed_artifact_is_reproducible_from_the_carriers() {
    let artifact = build_semantic_receipt_artifact(&repo_root()).expect("the measurement replays");
    validate_semantic_receipt(&artifact.receipt, &expectation())
        .expect("the rebuilt receipt validates against the recomputed expectation");
    assert_eq!(
        artifact.bytes.as_slice(),
        RECEIPT.as_bytes(),
        "the committed semantic receipt must reproduce byte-for-byte"
    );
}

#[test]
fn committed_receipt_carries_twelve_rows_with_closed_reasons() {
    let receipt = receipt();
    assert_eq!(receipt.documents.len(), 12);
    assert_eq!(receipt.population.documents, 12);
    assert_eq!(receipt.population.composed + receipt.population.refused, 12);
    for row in &receipt.documents {
        assert!(
            is_well_formedness_reason(&row.composer.well_formedness_reason),
            "{}: well-formedness reason `{}` is outside the closed vocabulary",
            row.document_id,
            row.composer.well_formedness_reason
        );
        assert!(
            is_bibliography_target_reason(&row.composer.bibliography_target_reason),
            "{}: bibliography-target reason `{}` is outside the closed vocabulary",
            row.document_id,
            row.composer.bibliography_target_reason
        );
    }
    assert_eq!(receipt.population.aggregate_verdict, NO_AGGREGATE_VERDICT);
}

#[test]
fn committed_receipt_splits_three_preserved_and_nine_holdout() {
    let receipt = receipt();
    let preserved = receipt
        .documents
        .iter()
        .filter(|row| row.profile_class == PROFILE_PRESERVED_RICH)
        .count();
    let holdout = receipt
        .documents
        .iter()
        .filter(|row| row.profile_class == PROFILE_HOLDOUT_STRUCTURAL)
        .count();
    assert_eq!(preserved, 3);
    assert_eq!(holdout, 9);
    // The split is derived from the bound path, never restated.
    for row in &receipt.documents {
        assert!(row.reference_sha256.len() == 64);
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
    assert!(is_sha256_digest(&receipt.corpus.documents_digest));
    assert!(is_sha256_digest(
        &receipt.native_route_emission_scope.sha256
    ));
    assert_eq!(
        receipt.native_route_emission_scope.sha256,
        sha256_prefixed(EMISSION.as_bytes())
    );
}

/// The bibliography-target reason is bound to the composer ledger: a document
/// with no required target must say so, and a document with targets must name
/// their resolution.
#[test]
fn committed_receipt_binds_each_target_reason_to_its_ledger() {
    let receipt = receipt();
    for row in &receipt.documents {
        let ledger = row
            .composer
            .ledger
            .expect("an accepted composer row carries a ledger");
        if ledger.required_targets == 0 {
            assert_eq!(
                row.composer.bibliography_target_reason, BT_NO_REQUIRED_TARGETS_DECLARED,
                "{}: an empty target population must be a typed reason",
                row.document_id
            );
        } else {
            assert_eq!(
                row.composer.bibliography_target_reason, BT_ALL_REQUIRED_TARGETS_RESOLVED,
                "{}: a non-empty target population resolves under the composer gate",
                row.document_id
            );
            assert_eq!(ledger.resolved_targets, ledger.required_targets);
        }
    }
}

// ───────────────────── native-route-emission scope ──────────────────────

/// The placeholder header is disclosed as a named typed-absence reason on every
/// document and is never read as a populated header.
#[test]
fn native_route_scope_never_reads_a_placeholder_header_as_populated() {
    let receipt = receipt();
    assert_eq!(receipt.native_route_emission_scope.document_count, 12);
    assert_eq!(receipt.native_route_emission_scope.documents.len(), 12);
    for row in &receipt.native_route_emission_scope.documents {
        assert_eq!(
            row.header.scope_reason, SCOPE_HEADER_TYPED_ABSENCE,
            "{}: the placeholder header is a typed absence",
            row.document_id
        );
        assert_ne!(row.header.scope_reason, SCOPE_HEADER_NATIVE_EMITTED);
    }
}

/// The native route emission declares zero local targets on every document: a
/// named scope reason, never a bibliography-target pass.
#[test]
fn native_route_scope_discloses_the_empty_reference_links() {
    let receipt = receipt();
    let empty = receipt
        .native_route_emission_scope
        .documents
        .iter()
        .filter(|row| row.reference_links.scope_reason == SCOPE_REFERENCE_LINKS_EMPTY_TARGETS)
        .count();
    // The S02 emitter measured `required_targets == 0` on every document; a scope
    // reason is the only honest reading.
    assert_eq!(empty, 12);
    for row in &receipt.native_route_emission_scope.documents {
        assert_eq!(row.reference_links.required_targets, 0);
        assert_eq!(row.reference_links.target_occurrences, 0);
    }
}

/// The scope section is bound to the committed emission envelope, not restated.
#[test]
fn native_route_scope_matches_the_committed_emission_bytes() {
    let expectation = expectation();
    let receipt = receipt();
    assert_eq!(expectation.emission_scope.len(), 12);
    assert_eq!(
        receipt.native_route_emission_scope.documents,
        expectation.emission_scope
    );
    for (row, source) in receipt
        .native_route_emission_scope
        .documents
        .iter()
        .zip(expectation.emission_scope.iter())
    {
        assert_eq!(row.document_id, source.document_id);
    }
}

// ───────────────────────────── frozen carriers ─────────────────────────────

/// The reference pins and the closed threshold policy are byte-unchanged: the
/// schema names the pins' digest and the pins file still hashes to it.
#[test]
fn frozen_reference_pins_and_threshold_schema_are_untouched() {
    let recomputed = sha256_prefixed(PINS.as_bytes());
    let schema: serde_yaml::Value =
        serde_yaml::from_str(THRESHOLD_SCHEMA).expect("the threshold schema parses");
    let declared = schema["threshold_policy"]["reference_pins"]["sha256"]
        .as_str()
        .expect("the schema names the reference pin digest");
    assert_eq!(declared, recomputed, "the schema pin must not move");
}

/// The frozen three-document `preserved_three_sha` aggregate is byte-unchanged:
/// the receipt reads the expanded 12-document corpus and never rewrites it.
#[test]
fn frozen_three_document_aggregate_is_untouched() {
    let comparison: serde_yaml::Value =
        serde_yaml::from_str(COMPARISON).expect("the comparison contract parses");
    let block = &comparison["native_fulltext_composition_shadow"]["corpus_semantic_regression"];
    assert_eq!(block["identity_kind"].as_str(), Some("preserved_three_sha"));
    assert_eq!(block["unique_source_sha256"].as_u64(), Some(3));
    assert_eq!(block["aggregate"]["tp"].as_u64(), Some(573));
    assert_eq!(block["aggregate"]["fp"].as_u64(), Some(0));
    assert_eq!(block["aggregate"]["fn"].as_u64(), Some(0));
    // The receipt does not carry the frozen aggregate identity.
    let receipt = receipt();
    assert_eq!(receipt.corpus.id, "native-pdf-to-tei-route-corpus");
    assert_ne!(receipt.corpus.id, "preserved_three_sha");
}

/// The receipt publishes counts/digests only: no raw TEI, no field value.
#[test]
fn committed_receipt_is_redacted() {
    let parsed: Value = serde_json::from_str(RECEIPT).expect("the receipt parses");
    assert!(parsed.get("documents").is_some());
    let text = RECEIPT;
    assert!(!text.contains("<TEI"));
    assert!(!text.contains("http://"));
    assert!(!text.contains("/root"));
    assert!(!text.contains("raw_tei"));
}

// ───────────────────────────── refusals ─────────────────────────────

#[test]
fn refuses_a_dropped_document() {
    let mut mutated = receipt();
    mutated.documents.pop();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::DocumentCountMismatch { .. }
    ));
}

#[test]
fn refuses_a_stale_corpus_digest() {
    let mut mutated = receipt();
    mutated.corpus.documents_digest = ZERO_DIGEST.to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::CorpusDigestDrift { .. }
    ));
}

#[test]
fn refuses_a_stale_emission_digest() {
    let mut mutated = receipt();
    mutated.native_route_emission_scope.sha256 = ZERO_DIGEST.to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::EmissionDigestDrift { .. }
    ));
}

#[test]
fn refuses_an_unbound_denominator() {
    let mut mutated = receipt();
    mutated.population.required_targets_total += 1;
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::UnboundDenominator { .. }
    ));
}

#[test]
fn refuses_empty_targets_presented_as_pass() {
    let mut mutated = receipt();
    let row = mutated
        .documents
        .iter_mut()
        .find(|row| {
            row.composer
                .ledger
                .is_some_and(|ledger| ledger.required_targets > 0)
        })
        .expect("at least one document declares a required target");
    let ledger = row.composer.ledger.as_mut().expect("a ledger");
    ledger.required_targets = 0;
    ledger.resolved_targets = 0;
    assert_eq!(
        row.composer.bibliography_target_reason,
        BT_ALL_REQUIRED_TARGETS_RESOLVED
    );
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::EmptyTargetsPresentedAsPass { .. }
    ));
}

#[test]
fn refuses_a_placeholder_header_read_as_populated() {
    let mut mutated = receipt();
    mutated.native_route_emission_scope.documents[0]
        .header
        .scope_reason = SCOPE_HEADER_NATIVE_EMITTED.to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::PlaceholderHeaderReadAsPopulated { .. }
    ));
}

#[test]
fn refuses_an_opened_lock() {
    let mut mutated = receipt();
    mutated
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::LockOpened { .. }
    ));
}

#[test]
fn refuses_an_accepted_threshold_state() {
    let mut mutated = receipt();
    mutated.thresholds = "accepted".to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::ThresholdClaimed { .. }
    ));
}

#[test]
fn refuses_an_aggregate_verdict() {
    let mut mutated = receipt();
    mutated.population.aggregate_verdict = "pass".to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::AggregateVerdictClaimed { .. }
    ));
}

#[test]
fn refuses_a_row_without_per_document_reasons() {
    let mut mutated = receipt();
    mutated.documents[0].composer.well_formedness_reason.clear();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::PerDocumentReasonMissing { .. }
    ));
}

#[test]
fn refuses_a_reason_outside_the_vocabulary() {
    let mut mutated = receipt();
    mutated.documents[0].composer.bibliography_target_reason = "pass".to_owned();
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::ReasonVocabularyViolation { .. }
    ));
}

#[test]
fn refuses_a_scope_row_that_drifts_from_the_committed_emission() {
    let mut mutated = receipt();
    mutated.native_route_emission_scope.documents[0]
        .references
        .disposition = "native_emitted".to_owned();
    // The scope reason now disagrees with the disposition, and the commitment
    // disagrees with the emission bytes.
    assert!(matches!(
        refuse(validate_semantic_receipt(&mutated, &expectation())),
        SemanticReceiptRefusal::ReasonVocabularyViolation { .. }
            | SemanticReceiptRefusal::EmissionScopeDrift { .. }
    ));
}

#[test]
fn refuses_a_malformed_carrier() {
    assert!(matches!(
        da_cli::native_route_semantic_receipt::NativeRouteSemanticReceipt::parse_json_bytes(
            b"{not json"
        ),
        Err(SemanticReceiptRefusal::MalformedCarrier { .. })
    ));
}
