//! Named stage-invariant registry (ADR-055 / S07/T07, grown in S08/T01).
//!
//! The `hard_invariants` lists in `data/ontology/pipeline_stage_contracts.yaml`
//! were decorative strings: `PipelineStageLedger::validate` only checked they
//! were non-empty. This module is the executable half of that declaration — a
//! named registry of invariants and a pure evaluator that fails when a declared
//! invariant is broken.
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
//! (`reason = "unknown_invariant"`), never a silent pass. A predicate whose
//! required fact is absent (`None`) or empty also fails — an observation that
//! does not carry the evidence an invariant needs cannot satisfy it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::pipeline_ledger::{PipelineStageLedger, StageCacheMode};

/// The invariant that a graph-materialize stage is never served from the
/// compute cache. Declared in `pipeline_stage_contracts.yaml` on
/// `extraction_materialization` (and every other graph-materialize stage).
pub const GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE: &str =
    "graph_materialization_never_skipped_by_compute_cache";

/// `import_eligible` is false: a stage is a candidate/derived producer, never a
/// graph-truth import boundary.
pub const IMPORT_ELIGIBLE_FALSE: &str = "import_eligible_false";

/// D127 unlock count is zero — no graph import was authorised or performed.
pub const D127_FALSE: &str = "D127_false";

/// A candidate payload is not authoritative.
pub const CANDIDATE_NOT_AUTHORITATIVE: &str = "candidate_not_authoritative";

/// An authoritative flag is false.
pub const AUTHORITATIVE_FALSE: &str = "authoritative_false";

/// A derived artifact is not authoritative.
pub const DERIVED_NOT_AUTHORITATIVE: &str = "derived_not_authoritative";

/// A candidate does not cross the graph-truth boundary.
pub const CANDIDATE_NOT_GRAPH_TRUTH: &str = "candidate_not_graph_truth";

/// Replay writes no net new records.
pub const REPLAY_IDEMPOTENT: &str = "replay_idempotent";

/// Every produced vector is finite (no NaN / infinity).
pub const NO_NAN: &str = "no_nan";

/// Missing values carry a typed missingness reason.
pub const TYPED_MISSINGNESS: &str = "typed_missingness";

/// The output schema fingerprint is bound.
pub const OUTPUT_SCHEMA_BOUND: &str = "output_schema_bound";

/// The model id and embedding dimensions are bound.
pub const MODEL_AND_DIMENSIONS_BOUND: &str = "model_and_dimensions_bound";

/// The source checksum is part of the cache key.
pub const SOURCE_CHECKSUM_IN_CACHE_KEY: &str = "source_checksum_in_cache_key";

/// The implementation and configuration fingerprints are part of the cache key.
pub const IMPLEMENTATION_AND_CONFIG_IN_CACHE_KEY: &str = "implementation_and_config_in_cache_key";

/// The parser profile is bound into the configuration fingerprint.
pub const PARSER_PROFILE_IN_CONFIG_FINGERPRINT: &str = "parser_profile_in_config_fingerprint";

/// The policy digest is bound.
pub const POLICY_DIGEST_BOUND: &str = "policy_digest_bound";

/// The package digest is bound.
pub const PACKAGE_DIGEST_BOUND: &str = "package_digest_bound";

/// The report lineage is bound.
pub const REPORT_LINEAGE_BOUND: &str = "report_lineage_bound";

/// The data-policy digest is bound.
pub const DATA_POLICY_BOUND: &str = "data_policy_bound";

/// Evidence keys are present.
pub const EVIDENCE_REQUIRED: &str = "evidence_required";

/// Every invariant name this module can execute. A name in a stage's
/// `hard_invariants` that is absent here has no executable check.
pub const NAMED_STAGE_INVARIANTS: &[&str] = &[
    GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE,
    IMPORT_ELIGIBLE_FALSE,
    D127_FALSE,
    CANDIDATE_NOT_AUTHORITATIVE,
    AUTHORITATIVE_FALSE,
    DERIVED_NOT_AUTHORITATIVE,
    CANDIDATE_NOT_GRAPH_TRUTH,
    REPLAY_IDEMPOTENT,
    NO_NAN,
    TYPED_MISSINGNESS,
    OUTPUT_SCHEMA_BOUND,
    MODEL_AND_DIMENSIONS_BOUND,
    SOURCE_CHECKSUM_IN_CACHE_KEY,
    IMPLEMENTATION_AND_CONFIG_IN_CACHE_KEY,
    PARSER_PROFILE_IN_CONFIG_FINGERPRINT,
    POLICY_DIGEST_BOUND,
    PACKAGE_DIGEST_BOUND,
    REPORT_LINEAGE_BOUND,
    DATA_POLICY_BOUND,
    EVIDENCE_REQUIRED,
];

/// Typed reason a named invariant fails. `unknown_invariant` is the fail-closed
/// outcome for a name this registry does not implement.
pub const UNKNOWN_INVARIANT_REASON: &str = "unknown_invariant";

/// Typed facts an invariant predicate reads. Every field is optional: an
/// observation may carry only the facts relevant to the invariants it is
/// checked against. A predicate whose required fact is absent fails closed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StageInvariantFacts {
    pub import_eligible: Option<bool>,
    pub d127_unlock_count: Option<u64>,
    pub replay_net_writes: Option<u64>,
    pub authoritative: Option<bool>,
    pub graph_truth_boundary: Option<bool>,
    pub vectors_all_finite: Option<bool>,
    pub missingness_typed: Option<bool>,
    pub model_id: Option<String>,
    pub dimensions: Option<u64>,
    pub output_schema_fingerprint: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub cache_key_fields: Vec<String>,
    pub evidence_keys: BTreeSet<String>,
}

/// One observation of a stage execution. The evaluator is pure over this value;
/// the pipeline recorder supplies it. `facts` is optional context for the
/// predicates that need more than cache behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageInvariantObservation {
    pub stage_id: String,
    pub cache_mode: StageCacheMode,
    pub served_from_cache: bool,
    #[serde(default)]
    pub facts: StageInvariantFacts,
}

impl StageInvariantObservation {
    /// Build an observation with no typed facts. Use for invariants that only
    /// depend on cache behavior (`graph_materialization_never_skipped_by_compute_cache`).
    pub fn new(
        stage_id: impl Into<String>,
        cache_mode: StageCacheMode,
        served_from_cache: bool,
    ) -> Self {
        Self {
            stage_id: stage_id.into(),
            cache_mode,
            served_from_cache,
            facts: StageInvariantFacts::default(),
        }
    }
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
/// The `graph_materialization_never_skipped_by_compute_cache` arm returns `Err`
/// when a `GraphMaterialize` stage is served from the compute cache (the graph
/// write was skipped). Every other arm reads a required typed fact and fails
/// when the fact is absent or wrong. An unknown name returns `Err` with reason
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
    let facts = &observation.facts;
    let stage_id = observation.stage_id.as_str();
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
        IMPORT_ELIGIBLE_FALSE => fail_closed(
            facts.import_eligible == Some(false),
            "import_eligible_not_false",
            name,
            stage_id,
        ),
        D127_FALSE => fail_closed(
            facts.d127_unlock_count == Some(0),
            "d127_unlock_count_nonzero_or_absent",
            name,
            stage_id,
        ),
        CANDIDATE_NOT_AUTHORITATIVE | AUTHORITATIVE_FALSE | DERIVED_NOT_AUTHORITATIVE => {
            fail_closed(
                facts.authoritative == Some(false),
                "authoritative_not_false",
                name,
                stage_id,
            )
        }
        CANDIDATE_NOT_GRAPH_TRUTH => fail_closed(
            facts.graph_truth_boundary == Some(false),
            "graph_truth_boundary_not_false",
            name,
            stage_id,
        ),
        REPLAY_IDEMPOTENT => fail_closed(
            facts.replay_net_writes == Some(0),
            "replay_net_writes_nonzero_or_absent",
            name,
            stage_id,
        ),
        NO_NAN => fail_closed(
            facts.vectors_all_finite == Some(true),
            "vectors_not_all_finite",
            name,
            stage_id,
        ),
        TYPED_MISSINGNESS => fail_closed(
            facts.missingness_typed == Some(true),
            "missingness_untyped",
            name,
            stage_id,
        ),
        OUTPUT_SCHEMA_BOUND => fail_closed(
            fact_nonempty(facts.output_schema_fingerprint.as_deref()),
            "output_schema_fingerprint_absent",
            name,
            stage_id,
        ),
        MODEL_AND_DIMENSIONS_BOUND => fail_closed(
            fact_nonempty(facts.model_id.as_deref()) && facts.dimensions.is_some_and(|d| d > 0),
            "model_id_or_dimensions_absent",
            name,
            stage_id,
        ),
        SOURCE_CHECKSUM_IN_CACHE_KEY => fail_closed(
            fields_contain(&facts.cache_key_fields, "source_sha256"),
            "source_checksum_absent_from_cache_key",
            name,
            stage_id,
        ),
        IMPLEMENTATION_AND_CONFIG_IN_CACHE_KEY => fail_closed(
            fields_contain(&facts.cache_key_fields, "implementation_fingerprint")
                && fields_contain(&facts.cache_key_fields, "configuration_fingerprint"),
            "implementation_or_config_absent_from_cache_key",
            name,
            stage_id,
        ),
        PARSER_PROFILE_IN_CONFIG_FINGERPRINT => fail_closed(
            fact_nonempty(facts.configuration_fingerprint.as_deref()),
            "configuration_fingerprint_absent",
            name,
            stage_id,
        ),
        POLICY_DIGEST_BOUND => fail_closed(
            facts.evidence_keys.contains("policy_digest"),
            "policy_digest_absent",
            name,
            stage_id,
        ),
        PACKAGE_DIGEST_BOUND => fail_closed(
            facts.evidence_keys.contains("package_digest"),
            "package_digest_absent",
            name,
            stage_id,
        ),
        REPORT_LINEAGE_BOUND => fail_closed(
            facts.evidence_keys.contains("report_lineage"),
            "report_lineage_absent",
            name,
            stage_id,
        ),
        DATA_POLICY_BOUND => fail_closed(
            facts.evidence_keys.contains("data_policy_digest"),
            "data_policy_digest_absent",
            name,
            stage_id,
        ),
        EVIDENCE_REQUIRED => fail_closed(
            !facts.evidence_keys.is_empty(),
            "evidence_keys_empty",
            name,
            stage_id,
        ),
        _ => Err(violation(UNKNOWN_INVARIANT_REASON)),
    }
}

/// A predicate that is `true` passes; `false` yields a typed violation. Keeps
/// every fail-closed arm a single expression.
fn fail_closed(
    pass: bool,
    reason: &str,
    name: &str,
    stage_id: &str,
) -> Result<(), StageInvariantViolation> {
    if pass {
        Ok(())
    } else {
        Err(StageInvariantViolation {
            invariant: name.to_string(),
            stage_id: stage_id.to_string(),
            reason: reason.to_string(),
        })
    }
}

/// A bound string fact is present and non-empty.
fn fact_nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// A cache-key field list contains the named field.
fn fields_contain(fields: &[String], needle: &str) -> bool {
    fields.iter().any(|field| field == needle)
}

/// Schema version of the DATA `stage_invariant_dispositions` block.
pub const STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION: &str = "stage-invariant-dispositions.v1";

/// How a declared `hard_invariants` name is accounted for: either it resolves
/// to an executable named check, or it is explicitly downgraded with a recorded
/// owner and reason. D554 requires the ledger load to refuse a declared name
/// that carries neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispositionKind {
    /// The name resolves to an executable predicate in [`NAMED_STAGE_INVARIANTS`].
    Checked,
    /// The name has no executable predicate today; a human owner and a recorded
    /// reason justify the gap.
    Downgraded,
}

/// One disposition record for a declared `hard_invariants` name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageInvariantDisposition {
    pub disposition: DispositionKind,
    /// The executable predicate this name maps to. Required for `checked`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    /// The human or slice owner of a `downgraded` gap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The `reason_vocabulary` token explaining a `downgraded` gap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The decision that authorised the downgrade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_ref: Option<String>,
    /// One sentence naming why no executable predicate exists today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// The DATA-declared disposition registry for every stage `hard_invariants`
/// name. `records` is keyed by invariant name; `reason_vocabulary` is the closed
/// set of downgrade reasons a record may cite.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StageInvariantDispositions {
    pub schema_version: String,
    #[serde(default)]
    pub reason_vocabulary: BTreeMap<String, String>,
    #[serde(default)]
    pub records: BTreeMap<String, StageInvariantDisposition>,
}

/// Validate that every invariant the ledger declares has an honest disposition.
///
/// Fail-closed, and pure over parsed values (R138 onion — the loader in
/// `da-adapters` owns I/O, the domain decides):
///
/// - a schema_version that is not [`STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION`]
///   is an error;
/// - a declared name with no record is an error naming the invariant;
/// - a `checked` record with an absent `check_id`, or a `check_id` that is not
///   an executable named invariant, is an error;
/// - a `downgraded` record without a non-empty owner or reason is an error;
/// - a `downgraded` reason absent from `reason_vocabulary` is an error.
pub fn validate_stage_invariant_dispositions(
    ledger: &PipelineStageLedger,
    dispositions: &StageInvariantDispositions,
) -> Result<(), String> {
    if dispositions.schema_version != STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION {
        return Err(format!(
            "stage invariant dispositions schema_version {:?} is not {:?}",
            dispositions.schema_version, STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION
        ));
    }
    let declared: BTreeSet<&str> = ledger
        .stages
        .iter()
        .flat_map(|stage| stage.hard_invariants.iter().map(String::as_str))
        .collect();
    for name in declared {
        let Some(record) = dispositions.records.get(name) else {
            return Err(format!(
                "stage invariant {name} is declared by the ledger but has no disposition record"
            ));
        };
        match record.disposition {
            DispositionKind::Checked => {
                let check_id = record.check_id.as_deref().unwrap_or("");
                if check_id.is_empty() {
                    return Err(format!(
                        "stage invariant {name} is disposition=checked but has no check_id"
                    ));
                }
                if !stage_invariant_is_named(check_id) {
                    return Err(format!(
                        "stage invariant {name} is disposition=checked with check_id {check_id:?} which is not an executable named invariant"
                    ));
                }
            }
            DispositionKind::Downgraded => {
                if record.owner.as_deref().unwrap_or("").is_empty() {
                    return Err(format!(
                        "stage invariant {name} is disposition=downgraded but has no owner"
                    ));
                }
                let reason = record.reason.as_deref().unwrap_or("");
                if reason.is_empty() {
                    return Err(format!(
                        "stage invariant {name} is disposition=downgraded but has no reason"
                    ));
                }
                if !dispositions.reason_vocabulary.contains_key(reason) {
                    return Err(format!(
                        "stage invariant {name} is disposition=downgraded with reason {reason:?} not in reason_vocabulary"
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The literal arm names, kept independent of `NAMED_STAGE_INVARIANTS` so a
    /// registry entry with no evaluator arm (or vice versa) is detectable.
    const EVALUATOR_ARM_NAMES: &[&str] = &[
        "graph_materialization_never_skipped_by_compute_cache",
        "import_eligible_false",
        "D127_false",
        "candidate_not_authoritative",
        "authoritative_false",
        "derived_not_authoritative",
        "candidate_not_graph_truth",
        "replay_idempotent",
        "no_nan",
        "typed_missingness",
        "output_schema_bound",
        "model_and_dimensions_bound",
        "source_checksum_in_cache_key",
        "implementation_and_config_in_cache_key",
        "parser_profile_in_config_fingerprint",
        "policy_digest_bound",
        "package_digest_bound",
        "report_lineage_bound",
        "data_policy_bound",
        "evidence_required",
    ];

    fn good_observation() -> StageInvariantObservation {
        StageInvariantObservation {
            stage_id: "test_stage".to_string(),
            cache_mode: StageCacheMode::PureCompute,
            served_from_cache: true,
            facts: StageInvariantFacts {
                import_eligible: Some(false),
                d127_unlock_count: Some(0),
                replay_net_writes: Some(0),
                authoritative: Some(false),
                graph_truth_boundary: Some(false),
                vectors_all_finite: Some(true),
                missingness_typed: Some(true),
                model_id: Some("bge-m3".to_string()),
                dimensions: Some(1024),
                output_schema_fingerprint: Some("schema:v1".to_string()),
                configuration_fingerprint: Some("config:v1".to_string()),
                cache_key_fields: vec![
                    "source_sha256".to_string(),
                    "implementation_fingerprint".to_string(),
                    "configuration_fingerprint".to_string(),
                ],
                evidence_keys: [
                    "policy_digest",
                    "package_digest",
                    "report_lineage",
                    "data_policy_digest",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            },
        }
    }

    /// Break exactly the fact `name` reads, leaving the others sound.
    fn bad_observation_for(name: &str) -> StageInvariantObservation {
        let mut observation = good_observation();
        match name {
            GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE => {
                observation.cache_mode = StageCacheMode::GraphMaterialize;
                observation.served_from_cache = true;
            }
            IMPORT_ELIGIBLE_FALSE => observation.facts.import_eligible = Some(true),
            D127_FALSE => observation.facts.d127_unlock_count = Some(1),
            CANDIDATE_NOT_AUTHORITATIVE | AUTHORITATIVE_FALSE | DERIVED_NOT_AUTHORITATIVE => {
                observation.facts.authoritative = Some(true)
            }
            CANDIDATE_NOT_GRAPH_TRUTH => observation.facts.graph_truth_boundary = Some(true),
            REPLAY_IDEMPOTENT => observation.facts.replay_net_writes = Some(1),
            NO_NAN => observation.facts.vectors_all_finite = Some(false),
            TYPED_MISSINGNESS => observation.facts.missingness_typed = Some(false),
            OUTPUT_SCHEMA_BOUND => observation.facts.output_schema_fingerprint = None,
            MODEL_AND_DIMENSIONS_BOUND => {
                observation.facts.model_id = None;
                observation.facts.dimensions = Some(0);
            }
            SOURCE_CHECKSUM_IN_CACHE_KEY => observation
                .facts
                .cache_key_fields
                .retain(|field| field != "source_sha256"),
            IMPLEMENTATION_AND_CONFIG_IN_CACHE_KEY => observation.facts.cache_key_fields.clear(),
            PARSER_PROFILE_IN_CONFIG_FINGERPRINT => {
                observation.facts.configuration_fingerprint = None
            }
            POLICY_DIGEST_BOUND => {
                observation.facts.evidence_keys.remove("policy_digest");
            }
            PACKAGE_DIGEST_BOUND => {
                observation.facts.evidence_keys.remove("package_digest");
            }
            REPORT_LINEAGE_BOUND => {
                observation.facts.evidence_keys.remove("report_lineage");
            }
            DATA_POLICY_BOUND => {
                observation.facts.evidence_keys.remove("data_policy_digest");
            }
            EVIDENCE_REQUIRED => observation.facts.evidence_keys.clear(),
            other => panic!("no breaker is defined for invariant {other}"),
        }
        observation
    }

    #[test]
    fn registry_and_evaluator_arms_are_in_bijection() {
        // Forward: every registry name reaches an evaluator arm. A name with no
        // arm would return UNKNOWN_INVARIANT_REASON on its broken observation.
        for name in NAMED_STAGE_INVARIANTS {
            let broken = bad_observation_for(name);
            let violation = evaluate_stage_invariant(name, &broken)
                .expect_err("a named invariant must fail on its broken observation");
            assert_ne!(
                violation.reason, UNKNOWN_INVARIANT_REASON,
                "{name} is registered but has no evaluator arm"
            );
        }
        // Reverse: every evaluator arm is registered, with no duplicates.
        let registered: BTreeSet<&str> = NAMED_STAGE_INVARIANTS.iter().copied().collect();
        let arms: BTreeSet<&str> = EVALUATOR_ARM_NAMES.iter().copied().collect();
        assert_eq!(
            registered.len(),
            NAMED_STAGE_INVARIANTS.len(),
            "NAMED_STAGE_INVARIANTS must not repeat a name"
        );
        assert_eq!(
            registered, arms,
            "the registry and the evaluator arm list must name the same invariants"
        );
    }

    #[test]
    fn every_named_invariant_passes_on_a_good_observation() {
        let good = good_observation();
        for name in NAMED_STAGE_INVARIANTS {
            assert!(
                evaluate_stage_invariant(name, &good).is_ok(),
                "{name} must pass on a known-good observation"
            );
        }
    }

    #[test]
    fn every_named_invariant_fails_on_its_break_with_name_and_stage() {
        for name in NAMED_STAGE_INVARIANTS {
            let broken = bad_observation_for(name);
            let violation = evaluate_stage_invariant(name, &broken)
                .expect_err(&format!("{name} must fail on its broken observation"));
            assert_eq!(violation.invariant, *name);
            assert_eq!(violation.stage_id, "test_stage");
            assert_ne!(
                violation.reason, UNKNOWN_INVARIANT_REASON,
                "{name} must fail for its own reason, not unknown_invariant"
            );
        }
    }

    #[test]
    fn missing_facts_fail_closed() {
        // A required-fact predicate fails when the observation carries no facts.
        let empty =
            StageInvariantObservation::new("test_stage", StageCacheMode::PureCompute, false);
        for name in NAMED_STAGE_INVARIANTS {
            if *name == GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE {
                // Cache-behavior invariant: no served-from-cache hit, so it holds.
                assert!(evaluate_stage_invariant(name, &empty).is_ok());
                continue;
            }
            let violation = evaluate_stage_invariant(name, &empty)
                .expect_err("an absent required fact must fail, never pass");
            assert_ne!(violation.reason, UNKNOWN_INVARIANT_REASON);
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
        let broken = StageInvariantObservation::new(
            "extraction_materialization",
            StageCacheMode::GraphMaterialize,
            true,
        );
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
        let good = StageInvariantObservation::new(
            "extraction_materialization",
            StageCacheMode::GraphMaterialize,
            false,
        );
        assert!(
            evaluate_stage_invariant(GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE, &good)
                .is_ok()
        );
        // A non-graph-materialize stage served from cache is out of scope for
        // this invariant: the check is specific to graph materialization.
        let compute = StageInvariantObservation::new(
            "extraction_materialization",
            StageCacheMode::PureCompute,
            true,
        );
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
        let good = StageInvariantObservation::new(
            "extraction_materialization",
            StageCacheMode::GraphMaterialize,
            false,
        );
        let violation = evaluate_stage_invariant("not_a_declared_invariant", &good)
            .expect_err("an unknown invariant name must fail closed, never pass");
        assert_eq!(violation.reason, UNKNOWN_INVARIANT_REASON);
        assert_eq!(violation.invariant, "not_a_declared_invariant");
        assert_eq!(violation.stage_id, "extraction_materialization");
        assert!(!stage_invariant_is_named("not_a_declared_invariant"));
    }
}
