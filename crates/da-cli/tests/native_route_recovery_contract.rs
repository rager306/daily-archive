//! S08/T04 contract gate for the native route-recovery ledger (M507-4fk6m6).
//!
//! Default features, fully offline: no service, no filesystem read beyond
//! git-tracked DATA, git-tracked receipts and the declared native surfaces.
//!
//! Negative polarity is the point of this file. The ledger qualifies recovery
//! from DATA plus receipts, so every way of *loosening* that qualification must
//! be a refusal rather than a quieter verdict:
//!
//! 1. an invented stage, an invented model or an unattributed observed model is
//!    refused, and a stage dropped from the table is a gate failure;
//! 2. withdrawing ANY ONE of the four substitution conditions withdraws the
//!    recovery verdict for the stage that owns the component;
//! 3. a Java-work receipt that carries no measurement never yields `recoverable`
//!    and never publishes a numeric total;
//! 4. the named gaps cannot be pooled, collapsed or restated at another value;
//! 5. a stage verdict or guard order outside the closed vocabulary is a
//!    contract violation, not a new state;
//! 6. every lock survives every run and the emitted artifact declares them;
//! 7. H05 is not claimed;
//! 8. two assemblies of the same inputs are byte-identical, and the committed
//!    artifact is exactly what the committed inputs produce.

use da_cli::native_route_recovery::{
    ADMISSION_CONDITION_NAMES, CASCADE_STAGES, ConditionState, JavaClaim, JavaWorkInput,
    ROUTE_RECOVERY_ARTIFACT_REL, ROUTE_RECOVERY_H05_CLOSURE, RecoverableJavaWork,
    RouteRecoveryError, RouteRecoveryInputs, RouteRecoveryLedger, STAGE_VERDICT_VOCABULARY,
    StageVerdict, VERDICT_GUARD_ORDER, VerdictReason, assemble, declared_native_surfaces,
    load_from_root, observe_surfaces,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
const COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
const CORPUS: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const RECEIPT: &str = include_str!("../../../artifacts/parse_compat/native_java_work.json");
const CORE_PROFILE: &str = include_str!("../../../artifacts/parse_compat/native_core_profile.json");
const CORE_SMP: &str = include_str!("../../../artifacts/parse_compat/native_core_smp.json");
const ARTIFACT: &str = include_str!("../../../artifacts/parse_compat/native_route_recovery.json");
/// The measured Java model invocations S08/T03 recorded over the pinned corpus.
const MEASURED_JAVA_INVOCATIONS: usize = 570;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn surfaces() -> BTreeSet<String> {
    let declared = declared_native_surfaces(COMPATIBILITY).expect("declared native surfaces");
    observe_surfaces(&root(), &declared)
}

fn ledger_with(
    composition: &str,
    java_work: &str,
) -> Result<RouteRecoveryLedger, RouteRecoveryError> {
    assemble(&RouteRecoveryInputs {
        composition_yaml: composition,
        compatibility_yaml: COMPATIBILITY,
        corpus_manifest_yaml: CORPUS,
        java_work_json: java_work,
        core_profile_json: CORE_PROFILE,
        core_smp_json: CORE_SMP,
        present_surfaces: &surfaces(),
    })
}

/// The recovery ledger the committed inputs produce.
fn ledger() -> RouteRecoveryLedger {
    ledger_with(COMPOSITION, RECEIPT).expect("the committed inputs must assemble")
}

fn mutate(from: &str, to: &str) -> String {
    let mutated = COMPOSITION.replacen(from, to, 1);
    assert_ne!(
        mutated, COMPOSITION,
        "the mutation anchor `{from}` did not land; a mutation that no-ops proves nothing"
    );
    mutated
}

/// A receipt whose aggregate carries no measurement at all. The shape stays the
/// one the canonical T03 type expects while every count disappears.
fn unobserved_receipt() -> String {
    let mut value: Value = serde_json::from_str(RECEIPT).expect("receipt json");
    value["aggregate"] = serde_json::json!({
        "measurement_state": "not_observed",
        "reason": "no_measured_document",
    });
    serde_json::to_string(&value).expect("receipt json")
}

#[test]
fn the_committed_artifact_is_exactly_what_the_committed_inputs_produce() {
    let first = ledger();
    let bytes = first.artifact_json_bytes().expect("artifact bytes");
    let emitted = String::from_utf8(bytes).expect("utf8");
    assert_eq!(
        emitted, ARTIFACT,
        "`{ROUTE_RECOVERY_ARTIFACT_REL}` is stale: re-run \
         `cargo run -p da-cli --example native_route_recovery`"
    );
    // Determinism is a property of the assembly, not of the file comparison.
    let second = ledger();
    assert_eq!(
        emitted,
        String::from_utf8(second.artifact_json_bytes().expect("artifact bytes")).expect("utf8")
    );
    // The on-disk loader must agree with the in-memory inputs.
    let loaded = load_from_root(&root()).expect("the repository tree assembles");
    assert_eq!(loaded, first);
}

#[test]
fn every_stage_gets_a_closed_verdict_and_only_the_admitted_lanes_recover() {
    let ledger = ledger();
    assert_eq!(ledger.stages.len(), CASCADE_STAGES.len());
    assert_eq!(
        ledger
            .stages
            .iter()
            .map(|row| row.stage.as_str())
            .collect::<Vec<_>>(),
        CASCADE_STAGES
    );
    for row in &ledger.stages {
        assert!(
            STAGE_VERDICT_VOCABULARY.contains(&row.verdict.as_str()),
            "stage {} produced `{}` outside the closed vocabulary",
            row.stage,
            row.verdict.as_str()
        );
        assert!(
            VERDICT_GUARD_ORDER.contains(&row.verdict_reason.as_str()),
            "stage {} produced reason `{}` outside the closed guard order",
            row.stage,
            row.verdict_reason.as_str()
        );
    }
    assert_eq!(
        ledger
            .recoverable_stages()
            .iter()
            .map(|row| row.stage.as_str())
            .collect::<Vec<_>>(),
        ["P3", "P4"],
        "only the independently selectable lanes may be recoverable"
    );
    assert_eq!(
        ledger.stage("P2").expect("P2").verdict,
        StageVerdict::Fallback
    );
    assert_eq!(
        ledger.stage("P6").expect("P6").verdict,
        StageVerdict::Fallback
    );
    assert_eq!(
        ledger.stage("P5").expect("P5").verdict_reason,
        VerdictReason::NoAdmittedComponent
    );
}

#[test]
fn the_measured_sum_is_conserved_and_bound_to_the_receipt_identity() {
    let ledger = ledger();
    let attributed: usize = ledger
        .stages
        .iter()
        .filter_map(|row| row.attributed_java_work.value())
        .sum();
    assert_eq!(
        attributed, MEASURED_JAVA_INVOCATIONS,
        "per-stage attribution must conserve the measured total"
    );
    match &ledger.recoverable_java_work {
        RecoverableJavaWork::Observed {
            value,
            by_stage,
            claim,
            corpus_id,
            ..
        } => {
            assert_eq!(*value, 399);
            assert_eq!(by_stage.get("P3"), Some(&181));
            assert_eq!(by_stage.get("P4"), Some(&218));
            assert_eq!(by_stage.values().sum::<usize>(), *value);
            assert_eq!(corpus_id, "s13-same-input-pdf-corpus");
            assert!(claim.contains("never_a_deployed_reduction"), "{claim}");
        }
        RecoverableJavaWork::NotObserved { .. } => panic!("the committed receipt is measured"),
    }
    // Every measured document is bound to the pinned manifest digest.
    assert_eq!(ledger.corpus.documents.len(), 3);
    for document in &ledger.corpus.documents {
        assert_eq!(document.source_sha256.len(), 64, "{}", document.document_id);
    }
}

#[test]
fn an_invented_stage_model_or_attribution_is_refused() {
    let error = ledger_with(&mutate("  - stage: P9", "  - stage: P10"), RECEIPT)
        .expect_err("an invented stage must be refused");
    assert!(
        matches!(error, RouteRecoveryError::UnknownStage { .. }),
        "{error}"
    );

    let error = ledger_with(
        &mutate(
            "java_models_observed: [fulltext]",
            "java_models_observed: [telepathy]",
        ),
        RECEIPT,
    )
    .expect_err("a model the receipt never observed must be refused");
    assert!(
        matches!(error, RouteRecoveryError::UnknownJavaModel { .. }),
        "{error}"
    );

    let error = ledger_with(
        &mutate(
            "java_models_observed: [fulltext]",
            "java_models_observed: []",
        ),
        RECEIPT,
    )
    .expect_err("an observed model no stage attributes must be refused");
    assert!(
        matches!(error, RouteRecoveryError::UnattributedJavaModel { .. }),
        "{error}"
    );

    let error = ledger_with(
        &mutate(
            "java_models_observed: [fulltext]",
            "java_models_observed: [fulltext, citation]",
        ),
        RECEIPT,
    )
    .expect_err("a double attribution must be refused");
    assert!(
        matches!(error, RouteRecoveryError::DoubleAttributedJavaModel { .. }),
        "{error}"
    );
}

#[test]
fn withdrawing_any_one_substitution_condition_withdraws_recovery() {
    for (from, to) in [
        (
            "source_hash: required_sha256",
            "source_hash: optional_sha256_when_present",
        ),
        (
            "schema_version: native-composition.outcome/v1",
            "schema_version: any",
        ),
        (
            "artifact_sha256: optional_sha256_when_present",
            "artifact_sha256: required_sha256",
        ),
        ("disposition: passed", "disposition: fallback"),
    ] {
        let ledger = ledger_with(&mutate(from, to), RECEIPT)
            .expect("a weakened condition assembles, but must not qualify");
        let row = ledger.stage("P3").expect("P3");
        assert_ne!(
            row.verdict,
            StageVerdict::Recoverable,
            "`{from}` -> `{to}` must withdraw the recovery verdict"
        );
        assert_ne!(ledger.recoverable_java_work.value(), Some(399));
        assert!(
            row.conditions
                .rows()
                .iter()
                .any(|(_, state)| !matches!(state, ConditionState::Met)),
            "the weakened condition must be visible in the evaluation"
        );
    }
    // The four conditions are the four DATA names, not a private list.
    assert_eq!(ADMISSION_CONDITION_NAMES.len(), 4);
    let ledger = ledger();
    for (name, state) in ledger.stage("P3").expect("P3").conditions.rows() {
        assert!(ADMISSION_CONDITION_NAMES.contains(&name), "{name}");
        assert_eq!(state, ConditionState::Met);
    }
}

#[test]
fn a_java_receipt_without_measurement_never_publishes_a_number() {
    let ledger = ledger_with(COMPOSITION, &unobserved_receipt())
        .expect("an unobserved receipt still assembles");
    assert!(!ledger.java_work.is_measured());
    assert!(matches!(
        ledger.java_work,
        JavaWorkInput::NotObserved { .. }
    ));
    assert!(ledger.recoverable_stages().is_empty());
    assert!(matches!(
        ledger.recoverable_java_work,
        RecoverableJavaWork::NotObserved { .. }
    ));
    assert_eq!(ledger.recoverable_java_work.value(), None);
    assert_eq!(
        ledger.stage("P3").expect("P3").verdict,
        StageVerdict::NotObserved
    );
    assert_eq!(
        ledger.stage("P3").expect("P3").verdict_reason,
        VerdictReason::JavaWorkNotObserved
    );
    assert!(matches!(
        ledger.stage("P3").expect("P3").attributed_java_work,
        JavaClaim::NotObserved { .. }
    ));
    let json = String::from_utf8(ledger.artifact_json_bytes().expect("artifact")).expect("utf8");
    assert!(!json.contains("\"state\": \"observed\""));
    assert!(!json.contains("\"value\": 0"));
    // A structural verdict is not overwritten by the absence of a measurement.
    assert_eq!(
        ledger.stage("P2").expect("P2").verdict,
        StageVerdict::Fallback
    );
}

#[test]
fn the_named_gaps_cannot_be_pooled_collapsed_or_restated() {
    let pooled = mutate("      citation_names: 2198", "      citation_names: 2405");
    let error = ledger_with(&pooled, RECEIPT).expect_err("a pooled denominator must be refused");
    assert!(
        matches!(error, RouteRecoveryError::NamedGapDrift { .. }),
        "{error}"
    );

    let collapsed = mutate(
        "    named_gaps:\n      citation_names: 2198\n      references: 207\n",
        "    named_gaps:\n      citation_names: 2198\n      references: 207\n      citation_names_and_references: 2405\n",
    );
    assert!(
        ledger_with(&collapsed, RECEIPT).is_err(),
        "a merged named gap must be refused, not ignored"
    );

    let ledger = ledger();
    assert_eq!(ledger.denominators.named_gaps.citation_names, 2198);
    assert_eq!(ledger.denominators.named_gaps.references, 207);
    assert_eq!(ledger.denominators.named_gaps.pooling, "forbidden");
    assert_eq!(ledger.denominators.pooled, "forbidden");
}

#[test]
fn a_verdict_or_guard_order_outside_the_closed_vocabulary_is_refused() {
    let extra_verdict = mutate(
        "    stage_verdicts: [recoverable, not_qualified, fallback, not_observed]",
        "    stage_verdicts: [recoverable, not_qualified, fallback, not_observed, maybe]",
    );
    let error =
        ledger_with(&extra_verdict, RECEIPT).expect_err("an invented verdict must be refused");
    assert!(
        matches!(error, RouteRecoveryError::VocabularyDrift { .. }),
        "{error}"
    );

    let reordered = mutate(
        "    order: [native_surface_absent, no_admitted_component, component_not_admitted, admission_conditions_unmet, core_qualification_missing, java_work_not_observed, no_attributed_java_work, all_conditions_met]",
        "    order: [no_admitted_component, native_surface_absent, component_not_admitted, admission_conditions_unmet, core_qualification_missing, java_work_not_observed, no_attributed_java_work, all_conditions_met]",
    );
    let error =
        ledger_with(&reordered, RECEIPT).expect_err("a reordered guard rule must be refused");
    assert!(
        matches!(error, RouteRecoveryError::VocabularyDrift { .. }),
        "{error}"
    );

    let dropped_reason = mutate(
        "      all_conditions_met: every recovery guard passed\n",
        "",
    );
    assert!(
        ledger_with(&dropped_reason, RECEIPT).is_err(),
        "a guard with no reason text must be refused"
    );
}

#[test]
fn every_lock_survives_and_h05_stays_unclaimed() {
    let ledger = ledger();
    assert_eq!(ledger.locks.production_route, "forbidden");
    assert_eq!(ledger.locks.auto_promote, "forbidden");
    assert_eq!(ledger.locks.graph_mutation, "forbidden");
    assert_eq!(ledger.locks.selection, "forbidden");
    assert!(!ledger.locks.deployed);
    assert!(!ledger.locks.import_eligible);
    assert_eq!(ledger.locks.decision_point, 6);
    assert_eq!(
        ledger.locks.independently_selectable,
        ["header", "references"],
        "the independently selectable set may not grow"
    );
    assert_eq!(ledger.h05_closure, ROUTE_RECOVERY_H05_CLOSURE);
    assert_eq!(ledger.h05_closure, "not_claimed");

    let json = String::from_utf8(ledger.artifact_json_bytes().expect("artifact")).expect("utf8");
    for pin in [
        "\"production_route\": \"forbidden\"",
        "\"auto_promote\": \"forbidden\"",
        "\"graph_mutation\": \"forbidden\"",
        "\"selection\": \"forbidden\"",
        "\"deployed\": false",
        "\"import_eligible\": false",
        "\"decision_point\": 6",
        "\"h05_closure\": \"not_claimed\"",
        "\"java_cpu_wall_clock\"",
        "\"rss_ceiling\"",
    ] {
        assert!(json.contains(pin), "the artifact must pin `{pin}`");
    }

    for (from, to) in [
        ("    deployed: false\n", "    deployed: true\n"),
        ("    decision_point: 6", "    decision_point: 7"),
        (
            "    decision_point: 6\n    independently_selectable: [header, references]",
            "    decision_point: 6\n    independently_selectable: [header]",
        ),
        ("  h05_closure: not_claimed", "  h05_closure: claimed"),
    ] {
        let mutated = mutate(from, to);
        assert!(
            ledger_with(&mutated, RECEIPT).is_err(),
            "lock mutation `{from}` -> `{to}` must be refused"
        );
    }
}

#[test]
fn the_artifact_carries_no_payload_and_declares_its_redaction() {
    let ledger = ledger();
    let bytes = ledger.artifact_json_bytes().expect("artifact");
    let json = String::from_utf8(bytes.clone()).expect("utf8");
    for marker in [
        "<TEI",
        "<tei",
        "=== model:",
        "#mdl#",
        "#rdr#",
        ".wapiti",
        "http://",
        "https://",
        "localhost",
        "/root",
        "/home",
        "/tmp",
    ] {
        assert!(!json.contains(marker), "artifact leaked `{marker}`");
    }
    // The forbidden set is the composition authority's closed list, verbatim.
    let forbidden: Vec<String> = serde_yaml::from_str::<serde_yaml::Value>(COMPOSITION)
        .expect("composition yaml")
        .get("redaction")
        .and_then(|redaction| redaction.get("forbidden"))
        .map(|value| {
            value
                .as_sequence()
                .expect("forbidden list")
                .iter()
                .map(|entry| entry.as_str().expect("token").to_string())
                .collect()
        })
        .expect("redaction.forbidden");
    assert_eq!(ledger.redaction.forbidden, forbidden);
    for token in &forbidden {
        assert!(
            json.contains(token.as_str()),
            "the artifact must declare `{token}` as forbidden"
        );
    }
    assert_eq!(bytes, ledger.artifact_json_bytes().expect("artifact bytes"));
}
