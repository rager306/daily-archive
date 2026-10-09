//! DATA-declared stage invariant is executable, Rust half (S07/T07).
//!
//! The `hard_invariants` lists in `data/ontology/pipeline_stage_contracts.yaml`
//! were decorative strings. This test loads the project ledger, takes the
//! declaration on `extraction_materialization`, and proves the declared name
//! maps to an executable check in `da_domain::stage_invariants` that fails when
//! the invariant is broken and passes when it holds.
//!
//! This is the binding the milestone demo needs: a declared stage invariant
//! whose named check is broken is a failure, not a comment.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use da_adapters::pipeline_ledger::{load_pipeline_stage_ledger, load_stage_invariant_dispositions};
use da_domain::{
    DispositionKind, GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE, NAMED_STAGE_INVARIANTS,
    StageCacheMode, StageInvariantObservation, evaluate_stage_invariant, stage_invariant_is_named,
    validate_stage_invariant_dispositions,
};

const LEDGER_REL: &str = "data/ontology/pipeline_stage_contracts.yaml";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn observation(stage_id: &str, served_from_cache: bool) -> StageInvariantObservation {
    StageInvariantObservation::new(
        stage_id,
        StageCacheMode::GraphMaterialize,
        served_from_cache,
    )
}

#[test]
fn declared_stage_invariant_is_executable_and_fails_when_broken() {
    let ledger = load_pipeline_stage_ledger(&repo_root().join(LEDGER_REL))
        .expect("load the project pipeline stage ledger");

    let stage = ledger
        .stages
        .iter()
        .find(|stage| stage.id == "extraction_materialization")
        .expect("the ledger declares extraction_materialization");

    // The declaration names the invariant and the stage is a graph write.
    assert!(
        stage
            .hard_invariants
            .iter()
            .any(|name| name == GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE),
        "extraction_materialization must declare {}; found {:?}",
        GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
        stage.hard_invariants
    );
    assert_eq!(stage.cache_mode, StageCacheMode::GraphMaterialize);

    // The DATA-declared name resolves to an executable check.
    assert!(
        stage_invariant_is_named(GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE),
        "the declared invariant must have an executable check"
    );

    // Broken: a graph materialize stage served from the compute cache fails,
    // and the violation names the invariant and the stage.
    let broken = evaluate_stage_invariant(
        GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
        &observation(&stage.id, true),
    )
    .expect_err("a graph materialize served from the compute cache must fail");
    assert_eq!(
        broken.invariant,
        GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE
    );
    assert_eq!(broken.stage_id, "extraction_materialization");
    assert_eq!(broken.reason, "graph_materialize_served_from_compute_cache");

    // Good: the same stage actually materialized passes the invariant.
    evaluate_stage_invariant(
        GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
        &observation(&stage.id, false),
    )
    .expect("a graph materialize that ran must pass");
}

/// The real DATA ledger declares 52 distinct invariants; every one of them must
/// resolve either to an executable check or to a recorded downgrade, and the
/// `checked` set must equal the executable registry in both directions (S08/T02,
/// SC#3).
#[test]
fn real_ledger_dispositions_cover_every_declared_invariant() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL))
        .expect("load the project pipeline stage ledger");
    let dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL))
        .expect("load the project disposition registry");

    // The loader itself refuses an undeclared name; re-validate the explicit
    // pair so the assertion does not depend on the loader's wiring.
    validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect("every declared invariant must have an honest disposition");

    let declared: BTreeSet<&str> = ledger
        .stages
        .iter()
        .flat_map(|stage| stage.hard_invariants.iter().map(String::as_str))
        .collect();
    assert_eq!(
        declared.len(),
        52,
        "the ledger declares 52 distinct invariants"
    );
    for name in &declared {
        assert!(
            dispositions.records.contains_key(*name),
            "declared invariant {name} has no disposition record"
        );
    }

    // `checked` records are exactly the executable registry, both directions.
    let checked: BTreeSet<&str> = dispositions
        .records
        .iter()
        .filter(|(_, record)| record.disposition == DispositionKind::Checked)
        .map(|(name, _)| name.as_str())
        .collect();
    let named: BTreeSet<&str> = NAMED_STAGE_INVARIANTS.iter().copied().collect();
    assert_eq!(
        checked, named,
        "checked records must equal the executable named-invariant registry"
    );
    assert_eq!(checked.len(), 20, "S08/T01 grew the registry to 20 names");
    assert_eq!(
        dispositions.records.len(),
        52,
        "every declared name has exactly one disposition record"
    );
}

#[test]
fn ledger_declaring_an_undeclared_invariant_fails_with_its_name() {
    let root = repo_root();
    let mut ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    let mut injected = ledger.stages[0].clone();
    injected
        .hard_invariants
        .push("not_a_declared_invariant".to_string());
    ledger.stages.push(injected);

    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a declared invariant without a disposition must fail");
    assert!(
        error.contains("not_a_declared_invariant"),
        "the error must name the undeclared invariant: {error}"
    );
}

#[test]
fn missing_disposition_record_fails_and_names_the_invariant() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions.records.remove("evidence_required");
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a declared invariant with its record removed must fail");
    assert!(error.contains("evidence_required"), "{error}");
}

#[test]
fn checked_record_with_unknown_check_id_fails() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions
        .records
        .get_mut("evidence_required")
        .unwrap()
        .check_id = Some("not_a_named_check".to_string());
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a checked record with an unknown check_id must fail");
    assert!(error.contains("evidence_required"), "{error}");
    assert!(error.contains("not_a_named_check"), "{error}");
}

#[test]
fn checked_record_without_check_id_fails() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions
        .records
        .get_mut("evidence_required")
        .unwrap()
        .check_id = None;
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a checked record without a check_id must fail");
    assert!(error.contains("evidence_required"), "{error}");
}

#[test]
fn downgraded_record_without_owner_or_reason_fails() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions
        .records
        .get_mut("readonly_policy")
        .unwrap()
        .owner = None;
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a downgraded record without an owner must fail");
    assert!(error.contains("readonly_policy"), "{error}");

    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();
    dispositions
        .records
        .get_mut("readonly_policy")
        .unwrap()
        .reason = None;
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a downgraded record without a reason must fail");
    assert!(error.contains("readonly_policy"), "{error}");
}

#[test]
fn downgraded_record_with_reason_outside_the_vocabulary_fails() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions
        .records
        .get_mut("readonly_policy")
        .unwrap()
        .reason = Some("not_in_vocabulary".to_string());
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a downgraded reason outside the vocabulary must fail");
    assert!(error.contains("readonly_policy"), "{error}");
    assert!(error.contains("not_in_vocabulary"), "{error}");
}

#[test]
fn dispositions_schema_version_mismatch_fails() {
    let root = repo_root();
    let ledger = load_pipeline_stage_ledger(&root.join(LEDGER_REL)).unwrap();
    let mut dispositions = load_stage_invariant_dispositions(&root.join(LEDGER_REL)).unwrap();

    dispositions.schema_version = "stage-invariant-dispositions.v0".to_string();
    let error = validate_stage_invariant_dispositions(&ledger, &dispositions)
        .expect_err("a schema_version mismatch must fail closed");
    assert!(error.contains("schema_version"), "{error}");
}
