//! S06/T06 (M514-1lss2s) — contract test for the committed same-n resource
//! artifact.
//!
//! This test audits the *shape and bindings* of
//! `artifacts/parse_compat/native_resource_same_n.json` from committed bytes
//! alone: it never reads the gitignored scholarly PDF corpus. The hermetic byte
//! recompute of both receipts lives in T08; this file proves the artifact is
//! well-formed, freeze-bound, pairs only with pinned measured same-n sources,
//! types every absence with a closed reason, carries no pooled or saving claim,
//! and that the independent validator refuses the named receipt-level mutations
//! it owns.

use da_cli::native_family_receipt::{is_repo_relative_path, sha256_hex, sha256_prefixed};
use da_cli::native_resource_same_n::{
    NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION, SAME_N_ABSENCE_POLICY, SAME_N_ABSENCE_REASON_VOCABULARY,
    SAME_N_FROZEN_CAP_METRICS, SAME_N_MISMATCH_HARD_FAILURE, SAME_N_NATIVE_METRIC_ORDER,
    SAME_N_OBSERVATION_STATE_VOCABULARY, SAME_N_PAIRING_RULE, SAME_N_POOLED_FORBIDDEN,
    SAME_N_POPULATION_VOCABULARY, SAME_N_REFERENCE_BUDGET_REL, SAME_N_REFERENCE_METRIC_ORDER,
    SAME_N_REFERENCE_WORK_REL, SAME_N_SAVING_CLAIM_FORBIDDEN, SameNExpectation,
    SameNMetricObservation, SameNReceipt, validate_same_n_receipt,
};
use da_cli::native_route_receipt::RouteReceipt;
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet};

const ARTIFACT: &str = include_str!("../../../artifacts/parse_compat/native_resource_same_n.json");
const ROUTE_ARTIFACT: &str = include_str!("../../../artifacts/parse_compat/native_route_e2e.json");
const WORK: &str = include_str!("../../../artifacts/parse_compat/native_java_work.json");
const BUDGET: &str = include_str!("../../../artifacts/parse_compat/parse_batch_budget_live.json");
const GOVERNOR: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const COMPARISON: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");
const THIS_SOURCE: &str = include_str!("native_resource_same_n_contract.rs");

fn receipt() -> SameNReceipt {
    SameNReceipt::parse_json_bytes(ARTIFACT.as_bytes())
        .expect("the committed artifact parses as a SameNReceipt")
}

fn yaml(text: &str) -> Value {
    serde_yaml::from_str(text).expect("the committed carrier parses as YAML")
}

fn route() -> RouteReceipt {
    RouteReceipt::parse_json_bytes(ROUTE_ARTIFACT.as_bytes())
        .expect("the committed route receipt parses")
}

fn governor_limit_policy() -> Value {
    yaml(GOVERNOR)["native_route_e2e"]["limit_policy"].clone()
}

/// Recompute `sha256(join("<path>:<sha256-hex>", "\n"))` over the freeze
/// carriers the T02 freeze declares, from their committed bytes.
fn recompute_freeze_revision() -> String {
    let carriers: BTreeMap<&str, &str> = [
        ("data/parse/layout_bakeoff.yaml", LAYOUT),
        ("data/parse/comparison_contract.yaml", COMPARISON),
        ("data/parse/native_fulltext_composition.yaml", COMPOSITION),
        (
            "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
            PROVENANCE,
        ),
    ]
    .into_iter()
    .collect();
    let declared: Vec<String> = governor_limit_policy()["freeze_revision"]["carriers"]
        .as_sequence()
        .expect("the freeze declares carriers")
        .iter()
        .map(|carrier| carrier["path"].as_str().expect("carrier path").to_owned())
        .collect();
    let mut lines = Vec::new();
    for path in declared {
        let bytes = carriers
            .get(path.as_str())
            .unwrap_or_else(|| panic!("carrier `{path}` is not committed"));
        lines.push(format!("{path}:{}", sha256_hex(bytes.as_bytes())));
    }
    sha256_prefixed(lines.join("\n").as_bytes())
}

/// The reference document identities as the pinned work carrier declares them.
fn reference_documents() -> Vec<da_cli::native_resource_same_n::SameNDocumentIdentity> {
    let work = yaml(WORK);
    work["corpus"]["documents"]
        .as_sequence()
        .expect("work corpus documents")
        .iter()
        .map(
            |entry| da_cli::native_resource_same_n::SameNDocumentIdentity {
                id: entry["id"].as_str().expect("id").to_owned(),
                source_pdf: entry["source_pdf"].as_str().expect("source_pdf").to_owned(),
                sha256: entry["sha256"].as_str().expect("sha256").to_owned(),
                pages: entry["pages"].as_u64().expect("pages"),
            },
        )
        .collect()
}

fn expectation() -> SameNExpectation {
    SameNExpectation {
        freeze_revision: recompute_freeze_revision(),
        corpus: receipt().corpus,
        reference_documents: reference_documents(),
    }
}

fn committed_carrier(manifest: &str) -> &'static str {
    match manifest {
        SAME_N_REFERENCE_WORK_REL => WORK,
        SAME_N_REFERENCE_BUDGET_REL => BUDGET,
        other => panic!("unknown committed reference carrier `{other}`"),
    }
}

fn refusal(receipt: &SameNReceipt, expectation: &SameNExpectation) -> String {
    format!(
        "{:?}",
        validate_same_n_receipt(receipt, expectation).expect_err("the mutation must be refused")
    )
}

// ───────────────────────────── shape and binding ─────────────────────────────

#[test]
fn artifact_is_the_canonical_bytes_of_the_parsed_receipt() {
    let receipt = receipt();
    assert_eq!(
        receipt.artifact_json_bytes().expect("canonical bytes"),
        ARTIFACT.as_bytes(),
        "the committed bytes are the canonical re-serialization"
    );
}

#[test]
fn envelope_is_diagnostic_only_with_the_five_locks_closed() {
    let receipt = receipt();
    assert_eq!(
        receipt.schema_version,
        NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION
    );
    assert_eq!(receipt.claim_scope, "diagnostic_only");
    assert_eq!(receipt.product_coverage_effect, "none");
    assert_eq!(receipt.thresholds, "not_accepted");
    for (lock, expected) in da_cli::native_family_receipt::REQUIRED_LOCKS {
        assert_eq!(
            receipt.locks.get(lock).map(String::as_str),
            Some(expected),
            "lock `{lock}` must be closed"
        );
    }
    assert!(!receipt.redaction.raw_text);
    assert!(!receipt.redaction.raw_pdf);
    assert!(!receipt.redaction.tei_xml);
    assert!(!receipt.redaction.physical_paths);
    assert!(!receipt.redaction.hostname);
}

#[test]
fn corpus_is_three_documents_and_202_pages_bound_to_the_java_reference() {
    let receipt = receipt();
    assert_eq!(receipt.corpus.document_count, 3);
    assert_eq!(receipt.corpus.page_denominator, 202);
    assert_eq!(receipt.corpus.documents.len(), 3);

    let route = route();
    let reference: Vec<_> = reference_documents()
        .iter()
        .map(|document| (document.id.clone(), document.sha256.clone(), document.pages))
        .collect();
    let same_n_documents: Vec<_> = receipt
        .corpus
        .documents
        .iter()
        .map(|document| (document.id.clone(), document.sha256.clone(), document.pages))
        .collect();
    assert_eq!(
        same_n_documents, reference,
        "the same-n corpus is exactly the Java-reference corpus"
    );
    assert_eq!(
        receipt.limits, route.limits,
        "both receipts bind the same freeze revision and metric dictionary"
    );
    assert_eq!(receipt.guard.observed_java_calls, 0);
    assert!(receipt.guard.installed && receipt.guard.identity_isolated);
}

#[test]
fn ceilings_come_from_the_frozen_limits() {
    let receipt = receipt();
    assert_eq!(receipt.limits.freeze_revision, recompute_freeze_revision());
    assert_eq!(receipt.limits.freeze_state, "frozen");
    assert_eq!(receipt.limits.tuned_after_scoring, "forbidden");
    assert_eq!(receipt.limits.claim_without_freeze, "forbidden");
    assert!(receipt.limits.zero_is_not_a_measurement);
    assert!(receipt.limits.pass_requires_frozen_threshold);

    let frozen: BTreeMap<&str, &str> = receipt
        .limits
        .metrics
        .iter()
        .map(|declaration| (declaration.id.as_str(), declaration.direction.as_str()))
        .collect();
    let native = receipt
        .populations
        .iter()
        .find(|population| population.population == "native")
        .expect("a native population");
    for row in &native.metrics {
        if SAME_N_FROZEN_CAP_METRICS.contains(&row.id.as_str()) {
            assert_eq!(
                row.direction.as_deref(),
                Some("ceiling"),
                "metric `{}` must carry the frozen ceiling direction",
                row.id
            );
            assert_eq!(
                frozen.get(row.id.as_str()),
                Some(&"ceiling"),
                "metric `{}` must be in the frozen dictionary",
                row.id
            );
        } else {
            assert!(
                row.direction.is_none() && !frozen.contains_key(row.id.as_str()),
                "metric `{}` is observation-only and must not claim a frozen direction",
                row.id
            );
        }
    }
}

// ───────────────────────── observation discipline ─────────────────────────

fn all_rows(receipt: &SameNReceipt) -> Vec<&SameNMetricObservation> {
    let mut rows: Vec<&SameNMetricObservation> = Vec::new();
    for population in &receipt.populations {
        rows.extend(population.metrics.iter());
    }
    for document in &receipt.documents {
        rows.extend(document.reference.iter());
    }
    rows
}

#[test]
fn every_metric_is_observed_or_typed_with_a_closed_reason() {
    let receipt = receipt();
    let rows = all_rows(&receipt);
    // One row per declared metric per population, plus one reference row per
    // declared reference metric on each document. The count is derived from the
    // closed vocabularies and the bound corpus, not a loose floor, so a dropped
    // row fails rather than sliding under a threshold.
    let expected_rows = SAME_N_NATIVE_METRIC_ORDER.len()
        + SAME_N_REFERENCE_METRIC_ORDER.len()
        + receipt.documents.len() * SAME_N_REFERENCE_METRIC_ORDER.len();
    assert_eq!(
        rows.len(),
        expected_rows,
        "the artifact carries one row per declared metric, per population and per document"
    );

    for row in &rows {
        assert!(
            SAME_N_OBSERVATION_STATE_VOCABULARY.contains(&row.state.as_str()),
            "metric `{}` has an untyped state `{}`",
            row.id,
            row.state
        );
        match row.state.as_str() {
            "observed" => {
                assert!(
                    row.value.is_some(),
                    "metric `{}` observed without a value",
                    row.id
                );
                assert!(
                    row.source.is_some(),
                    "metric `{}` observed without a source",
                    row.id
                );
                assert!(row.reason.is_none());
            }
            _ => {
                assert!(
                    row.value.is_none(),
                    "metric `{}` reports a number as a typed absence (zero masquerade)",
                    row.id
                );
                let reason = row
                    .reason
                    .as_deref()
                    .expect("a typed absence names a reason");
                assert!(
                    SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason),
                    "metric `{}` names unknown absence reason `{reason}`",
                    row.id
                );
            }
        }
    }
}

#[test]
fn reference_rows_are_pinned_to_a_measured_same_n_source() {
    let receipt = receipt();
    let mut observed_reference = 0usize;
    for row in all_rows(&receipt) {
        let Some(source) = row.source.as_ref() else {
            continue;
        };
        if source.kind != "measured_reference" {
            continue;
        }
        observed_reference += 1;
        let manifest = source.manifest.as_deref().expect("a reference manifest");
        assert!(
            is_repo_relative_path(manifest),
            "reference manifest `{manifest}` is not repository-relative"
        );
        let digest = source.sha256.as_deref().expect("a reference digest");
        assert_eq!(
            digest,
            sha256_hex(committed_carrier(manifest).as_bytes()),
            "reference row `{}` digest drifted from the committed carrier",
            row.id
        );
        assert_eq!(source.measurement_state.as_deref(), Some("measured"));
        assert!(
            source
                .pointer
                .as_deref()
                .is_some_and(|pointer| !pointer.is_empty())
        );
    }
    assert!(
        observed_reference > 0,
        "at least one reference row is pinned and measured"
    );
    // The Java CPU row is honestly absent: the batch observation is
    // container-wide, not a per-document same-n row (D510).
    let budget_rows: Vec<&SameNMetricObservation> = all_rows(&receipt)
        .into_iter()
        .filter(|row| row.id == "java_cpu_wall_clock_ns")
        .collect();
    assert!(!budget_rows.is_empty());
    for row in budget_rows {
        assert_eq!(row.state, "not_observed");
        assert_eq!(row.reason.as_deref(), Some("reference_row_not_same_n"));
    }
}

#[test]
fn native_measurements_are_non_vacuous_and_typed_absence_is_not_a_zero() {
    let receipt = receipt();
    let native = receipt
        .populations
        .iter()
        .find(|population| population.population == "native")
        .expect("a native population");
    let observed = native
        .metrics
        .iter()
        .filter(|row| row.is_observed())
        .count();
    assert!(
        observed >= 4,
        "the native side carries measured rows, found {observed}"
    );
    // The retained-bytes row is honestly unmeasured rather than a fabricated 0.
    let retained = native
        .metrics
        .iter()
        .find(|row| row.id == "peak_retained_bytes")
        .expect("the retained-bytes row");
    assert_eq!(retained.state, "not_observed");
    assert_eq!(retained.value, None);
    assert_eq!(
        retained.reason.as_deref(),
        Some("native_metric_not_measured")
    );
    // Every document carries a measured native latency sample.
    for document in &receipt.documents {
        assert!(
            document.native_latency_ms.is_some(),
            "document `{}` has no measured native latency",
            document.id
        );
    }
}

#[test]
fn populations_are_closed_and_never_pooled() {
    let receipt = receipt();
    assert_eq!(receipt.pairing.rule, SAME_N_PAIRING_RULE);
    assert_eq!(receipt.pairing.pooled, SAME_N_POOLED_FORBIDDEN);
    assert_eq!(receipt.pairing.saving_claim, SAME_N_SAVING_CLAIM_FORBIDDEN);
    assert_eq!(receipt.pairing.absence_policy, SAME_N_ABSENCE_POLICY);
    assert_eq!(
        receipt.pairing.mismatch_disposition,
        SAME_N_MISMATCH_HARD_FAILURE
    );
    assert!(!SAME_N_POPULATION_VOCABULARY.contains(&"pooled"));

    let names: BTreeSet<&str> = receipt
        .populations
        .iter()
        .map(|population| population.population.as_str())
        .collect();
    assert_eq!(
        names,
        SAME_N_POPULATION_VOCABULARY.iter().copied().collect()
    );
}

#[test]
fn metric_ids_are_within_the_declared_vocabularies() {
    let receipt = receipt();
    for population in &receipt.populations {
        let vocabulary: &[&str] = if population.population == "native" {
            &SAME_N_NATIVE_METRIC_ORDER
        } else {
            &SAME_N_REFERENCE_METRIC_ORDER
        };
        for row in &population.metrics {
            assert!(
                vocabulary.contains(&row.id.as_str()),
                "metric `{}` is outside the `{}` vocabulary",
                row.id,
                population.population
            );
        }
    }
}

#[test]
fn the_validator_accepts_the_committed_artifact() {
    validate_same_n_receipt(&receipt(), &expectation()).expect("the committed receipt validates");
}

// ───────────────────────── named negative mutations ─────────────────────────

#[test]
fn stale_freeze_revision_is_refused_by_name() {
    let receipt = receipt();
    let mut expectation = expectation();
    expectation.freeze_revision = sha256_prefixed(b"drifted");
    assert!(refusal(&receipt, &expectation).contains("StaleFreezeRevision"));
}

#[test]
fn document_mismatch_is_a_named_hard_failure() {
    let mut receipt = receipt();
    receipt.corpus.documents[0].sha256 = "f".repeat(64);
    let mut expectation = expectation();
    expectation.corpus = receipt.corpus.clone();
    assert!(refusal(&receipt, &expectation).contains("Mismatch"));
}

#[test]
fn a_reference_set_that_is_not_the_native_set_is_refused() {
    let receipt = receipt();
    let mut expectation = expectation();
    expectation.reference_documents[0].id = "invented-0000.00000".to_owned();
    assert!(refusal(&receipt, &expectation).contains("Mismatch"));
}

#[test]
fn opened_lock_is_refused_by_name() {
    let mut receipt = receipt();
    receipt
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(refusal(&receipt, &expectation()).contains("LockOpened"));
}

#[test]
fn claimed_threshold_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.thresholds = "accepted".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("ThresholdClaimed"));
}

#[test]
fn claimed_product_coverage_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.product_coverage_effect = "full".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("ProductCoverageClaimed"));
}

#[test]
fn guard_missing_for_the_observed_zero_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.guard.installed = false;
    assert!(refusal(&receipt, &expectation()).contains("GuardMissingForObservedZero"));
}

#[test]
fn observed_java_calls_are_refused_by_name() {
    let mut receipt = receipt();
    receipt.guard.observed_java_calls = 1;
    assert!(refusal(&receipt, &expectation()).contains("ObservedJavaOnRoute"));
}

#[test]
fn pooled_claim_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.pairing.pooled = "allowed".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("PooledClaimed"));
}

#[test]
fn saving_claim_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.pairing.saving_claim = "claimed".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("SavingClaimed"));
}

#[test]
fn pooling_rule_drift_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.pairing.rule = "cross_corpus".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("PairingRuleDrift"));
}

#[test]
fn a_metric_without_an_observation_state_is_refused_by_name() {
    let mut receipt = receipt();
    let native = receipt
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    native.metrics[0].state = "maybe".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("ZeroMasquerade"));
}

#[test]
fn a_not_observed_row_carrying_a_number_is_refused_by_name() {
    let mut receipt = receipt();
    let native = receipt
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    let retained = native
        .metrics
        .iter_mut()
        .find(|row| row.id == "peak_retained_bytes")
        .expect("the retained-bytes row");
    retained.value = Some(0);
    assert!(refusal(&receipt, &expectation()).contains("ZeroMasquerade"));
}

#[test]
fn an_invented_absence_reason_is_refused_by_name() {
    let mut receipt = receipt();
    let native = receipt
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    let retained = native
        .metrics
        .iter_mut()
        .find(|row| row.id == "peak_retained_bytes")
        .expect("the retained-bytes row");
    retained.reason = Some("because".to_owned());
    assert!(refusal(&receipt, &expectation()).contains("AbsenceReasonUnknown"));
}

#[test]
fn an_unpinned_reference_source_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.documents[0].reference[0]
        .source
        .as_mut()
        .expect("a source")
        .manifest = Some("/var/tmp/leak.json".to_owned());
    assert!(refusal(&receipt, &expectation()).contains("ReferenceSourceUnpinned"));
}

#[test]
fn a_not_measured_reference_source_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.documents[0].reference[0]
        .source
        .as_mut()
        .expect("a source")
        .measurement_state = Some("not_observed".to_owned());
    assert!(refusal(&receipt, &expectation()).contains("ReferenceSourceUnpinned"));
}

#[test]
fn a_cap_metric_direction_drift_is_refused_by_name() {
    let mut receipt = receipt();
    let native = receipt
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    native.metrics[0].direction = Some("floor".to_owned());
    assert!(refusal(&receipt, &expectation()).contains("DirectionDrift"));
}

#[test]
fn an_unfrozen_limit_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.limits.freeze_state = "draft".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("LimitNotFrozen"));
}

#[test]
fn a_limit_tuned_after_scoring_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.limits.tuned_after_scoring = "allowed".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("LimitTunedAfterScoring"));
}

#[test]
fn a_metric_outside_the_vocabulary_is_refused_by_name() {
    let mut receipt = receipt();
    let native = receipt
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    native.metrics[0].id = "invented_metric".to_owned();
    assert!(refusal(&receipt, &expectation()).contains("MetricUnknown"));
}

#[test]
fn residuals_that_do_not_recompute_are_refused_by_name() {
    let mut receipt = receipt();
    receipt.residuals.observed_metrics += 1;
    assert!(refusal(&receipt, &expectation()).contains("ZeroMasquerade"));
}

// ───────────────────────── hermeticity ─────────────────────────

#[test]
fn the_test_reads_no_gitignored_corpus_and_no_filesystem() {
    let marker = ["data", "sources"].join("/");
    assert!(
        !THIS_SOURCE.contains(&marker),
        "the contract test must not name the gitignored corpus path"
    );
    assert!(
        !THIS_SOURCE.contains(concat!("fs", "::read")),
        "the contract test audits committed bytes, not the filesystem"
    );
}
