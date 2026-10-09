//! Named stage-invariant registry (ADR-055 / S07/T07).
//!
//! The `hard_invariants` lists in `data/ontology/pipeline_stage_contracts.yaml`
//! were decorative strings: `PipelineStageLedger::validate` only checked they
//! were non-empty. This module seeds the executable half of that declaration —
//! a named registry of invariants and a pure evaluator that fails when a
//! declared invariant is broken.
//!
//! The declaration in DATA names the invariant; this module owns the check. The
//! binding between the two is proven by
//! `crates/da-adapters/tests/stage_invariant_binding.rs`, which loads the
//! project ledger and asserts the DATA-declared name for
//! `extraction_materialization` is known here and that a broken observation
//! fails with the invariant name and stage id.
//!
//! No I/O lives here: observations arrive as a plain value (R138 onion — the
//! ledger loader in `da-adapters` reads DATA, the domain decides).
//!
//! Fail-closed: an unknown invariant name is a violation
//! (`reason = "unknown_invariant"`), never a silent pass.

use serde::{Deserialize, Serialize};

use crate::pipeline_ledger::StageCacheMode;

/// The invariant that a graph-materialize stage is never served from the
/// compute cache. Declared in `pipeline_stage_contracts.yaml` on
/// `extraction_materialization` (and every other graph-materialize stage).
pub const GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE: &str =
    "graph_materialization_never_skipped_by_compute_cache";

/// Every invariant name this module can execute. A name in a stage's
/// `hard_invariants` that is absent here has no executable check.
pub const NAMED_STAGE_INVARIANTS: &[&str] = &[GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE];

/// Typed reason a named invariant fails. `unknown_invariant` is the fail-closed
/// outcome for a name this registry does not implement.
pub const UNKNOWN_INVARIANT_REASON: &str = "unknown_invariant";

/// One observation of a stage execution's cache behavior. The evaluator is pure
/// over this value; the pipeline recorder supplies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageInvariantObservation {
    pub stage_id: String,
    pub cache_mode: StageCacheMode,
    pub served_from_cache: bool,
}

/// A broken declared invariant: which invariant failed, on which stage, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageInvariantViolation {
    pub invariant: String,
    pub stage_id: String,
    pub reason: String,
}

/// Whether `name` is an invariant this registry can execute.
pub fn stage_invariant_is_named(name: &str) -> bool {
    NAMED_STAGE_INVARIANTS.contains(&name)
}

/// Evaluate a named invariant against one observation, failing closed.
///
/// `graph_materialization_never_skipped_by_compute_cache` returns `Err` when a
/// `GraphMaterialize` stage is served from the compute cache (the graph write
/// was skipped) and `Ok` otherwise. An unknown name returns `Err` with reason
/// [`UNKNOWN_INVARIANT_REASON`] — never a silent pass.
pub fn evaluate_stage_invariant(
    name: &str,
    observation: &StageInvariantObservation,
) -> Result<(), StageInvariantViolation> {
    let violation = |reason: &str| StageInvariantViolation {
        invariant: name.to_string(),
        stage_id: observation.stage_id.clone(),
        reason: reason.to_string(),
    };
    match name {
        GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE => {
            if observation.cache_mode == StageCacheMode::GraphMaterialize
                && observation.served_from_cache
            {
                Err(violation("graph_materialize_served_from_compute_cache"))
            } else {
                Ok(())
            }
        }
        _ => Err(violation(UNKNOWN_INVARIANT_REASON)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        cache_mode: StageCacheMode,
        served_from_cache: bool,
    ) -> StageInvariantObservation {
        StageInvariantObservation {
            stage_id: "extraction_materialization".to_string(),
            cache_mode,
            served_from_cache,
        }
    }

    #[test]
    fn stage_invariant_registry_names_the_graph_materialization_check() {
        assert!(stage_invariant_is_named(
            GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE
        ));
        assert!(
            NAMED_STAGE_INVARIANTS.contains(&GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE)
        );
    }

    #[test]
    fn stage_invariant_broken_observation_fails_with_name_and_stage() {
        let broken = observation(StageCacheMode::GraphMaterialize, true);
        let violation = evaluate_stage_invariant(
            GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
            &broken,
        )
        .expect_err("a graph materialize served from the compute cache must fail");
        assert_eq!(
            violation.invariant,
            GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE
        );
        assert_eq!(violation.stage_id, "extraction_materialization");
        assert_eq!(
            violation.reason,
            "graph_materialize_served_from_compute_cache"
        );
    }

    #[test]
    fn stage_invariant_good_observation_passes() {
        let good = observation(StageCacheMode::GraphMaterialize, false);
        assert!(
            evaluate_stage_invariant(GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE, &good)
                .is_ok()
        );
        // A non-graph-materialize stage served from cache is out of scope for
        // this invariant: the check is specific to graph materialization.
        let compute = observation(StageCacheMode::PureCompute, true);
        assert!(
            evaluate_stage_invariant(
                GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
                &compute
            )
            .is_ok()
        );
    }

    #[test]
    fn stage_invariant_unknown_name_fails_closed() {
        let good = observation(StageCacheMode::GraphMaterialize, false);
        let violation = evaluate_stage_invariant("not_a_declared_invariant", &good)
            .expect_err("an unknown invariant name must fail closed, never pass");
        assert_eq!(violation.reason, UNKNOWN_INVARIANT_REASON);
        assert_eq!(violation.invariant, "not_a_declared_invariant");
        assert_eq!(violation.stage_id, "extraction_materialization");
        assert!(!stage_invariant_is_named("not_a_declared_invariant"));
    }
}
