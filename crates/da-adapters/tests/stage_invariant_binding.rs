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

use std::path::{Path, PathBuf};

use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_domain::{
    GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE, StageCacheMode,
    StageInvariantObservation, evaluate_stage_invariant, stage_invariant_is_named,
};

const LEDGER_REL: &str = "data/ontology/pipeline_stage_contracts.yaml";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn observation(stage_id: &str, served_from_cache: bool) -> StageInvariantObservation {
    StageInvariantObservation {
        stage_id: stage_id.to_string(),
        cache_mode: StageCacheMode::GraphMaterialize,
        served_from_cache,
    }
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
