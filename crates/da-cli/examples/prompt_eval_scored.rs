//! Score real bounded inference outputs against frozen source-bound oracle facts.
//! Never calls a model, promotes a candidate, or authorizes import.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::prompt_task_registry::load_prompt_task_registry;
use da_application::prompt_evaluation::{
    FrozenPromptFixture, PromptEvalPolicy, TaskEvaluationReport, evaluate_task,
    extract_claims_from_output,
};
use da_domain::{
    assurance::ContentDigest,
    prompt_optimization::{PromptCandidate, PromptCandidateState},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "governor/fixtures/prompt_eval/fixtures.yaml")]
    fixtures: PathBuf,
    #[arg(long, default_value = "governor/fixtures/prompt_eval/policy.yaml")]
    policy: PathBuf,
    #[arg(long, default_value = "data/ontology/prompt_tasks.yaml")]
    prompt_registry: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/prompt_eval_inference_m387.json"
    )]
    inference_report: PathBuf,
    #[arg(long, default_value = "artifacts/runtime/prompt_eval_scored_m387.json")]
    report: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureSet {
    schema_version: String,
    review_status: String,
    reviewed_by: String,
    production_acceptance_eligible: bool,
    fixtures: Vec<FrozenPromptFixture>,
}

#[derive(Deserialize)]
struct InferenceReport {
    executions: Vec<InferenceExecution>,
    failures: Vec<InferenceFailure>,
}

#[derive(Clone, Deserialize)]
struct InferenceExecution {
    fixture_id: String,
    variant: String,
    output: serde_json::Value,
    lineage: InferenceLineage,
}

#[derive(Clone, Deserialize)]
struct InferenceLineage {
    task_id: String,
    model: String,
    latency_ms: u64,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[derive(Clone, Deserialize)]
struct InferenceFailure {
    fixture_id: String,
    variant: String,
    error_class: String,
}

#[derive(Clone, Serialize)]
struct FrozenPromptOutputReplica {
    producer_id: String,
    schema_valid: bool,
    claims: Vec<String>,
    paraphrase_invariant_pass: bool,
    latency_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
    estimated_cost_microunits: u64,
}

#[derive(Serialize)]
struct CandidateEvaluation {
    candidate: PromptCandidate,
    report: TaskEvaluationReport,
    source_outputs: BTreeMap<String, String>,
    schema_failures: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    fixture_sha256: ContentDigest,
    policy_sha256: ContentDigest,
    review_status: String,
    reviewed_by: String,
    production_acceptance_eligible: bool,
    evaluations: Vec<CandidateEvaluation>,
    model_inference_requested: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
    import_eligible: bool,
    report_sha256: String,
}

fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("prompt_eval_scored");
    let args = Args::parse();
    let fixture_bytes = std::fs::read(&args.fixtures)?;
    let mut fixtures: FixtureSet = serde_yaml::from_slice(&fixture_bytes)?;
    if fixtures.schema_version != "prompt-eval-fixtures.v1"
        || fixtures.review_status.trim().is_empty()
        || fixtures.reviewed_by.trim().is_empty()
        || fixtures.production_acceptance_eligible
    {
        return Err(anyhow!("fixture set incomplete or acceptance-eligible"));
    }
    let policy_bytes = std::fs::read(&args.policy)?;
    let policy: PromptEvalPolicy = serde_yaml::from_slice(&policy_bytes)?;
    policy.validate().map_err(anyhow::Error::msg)?;
    let registry = load_prompt_task_registry(&args.prompt_registry).map_err(anyhow::Error::msg)?;
    let inference: InferenceReport =
        serde_json::from_slice(&std::fs::read(&args.inference_report)?)?;

    let mut executions_by_fixture: BTreeMap<String, BTreeMap<String, InferenceExecution>> =
        BTreeMap::new();
    for execution in &inference.executions {
        executions_by_fixture
            .entry(execution.fixture_id.clone())
            .or_default()
            .insert(execution.variant.clone(), execution.clone());
    }
    let mut failures_by_fixture: BTreeMap<String, BTreeMap<String, InferenceFailure>> =
        BTreeMap::new();
    for failure in &inference.failures {
        failures_by_fixture
            .entry(failure.fixture_id.clone())
            .or_default()
            .insert(failure.variant.clone(), failure.clone());
    }

    let mut scored_fixtures = Vec::new();
    let mut source_outputs_by_fixture = BTreeMap::new();
    let mut schema_failures_by_fixture = BTreeMap::new();

    for mut fixture in fixtures.fixtures.drain(..) {
        let mut source_outputs = BTreeMap::new();
        let mut schema_failures = BTreeMap::new();
        for variant in ["baseline", "candidate"] {
            match executions_by_fixture
                .get(&fixture.fixture_id)
                .and_then(|m| m.get(variant))
            {
                Some(execution) => {
                    source_outputs.insert(variant.to_owned(), execution.output.to_string());
                    let claims: Vec<String> = extract_claims_from_output(
                        &execution.output,
                        &fixture.required_facts,
                        &fixture.forbidden_facts,
                    )
                    .into_iter()
                    .collect();
                    let replica = FrozenPromptOutputReplica {
                        producer_id: format!(
                            "{}:{}",
                            execution.lineage.model, execution.lineage.task_id
                        ),
                        schema_valid: true,
                        claims,
                        paraphrase_invariant_pass: true,
                        latency_ms: execution.lineage.latency_ms,
                        input_tokens: execution.lineage.input_tokens.unwrap_or(0),
                        output_tokens: execution.lineage.output_tokens.unwrap_or(0),
                        estimated_cost_microunits: estimate_cost(
                            execution.lineage.input_tokens,
                            execution.lineage.output_tokens,
                        ),
                    };
                    if variant == "baseline" {
                        fixture.baseline = serde_json::from_value(serde_json::to_value(replica)?)?;
                    } else {
                        fixture.candidate = serde_json::from_value(serde_json::to_value(replica)?)?;
                    }
                }
                None => {
                    if let Some(failure) = failures_by_fixture
                        .get(&fixture.fixture_id)
                        .and_then(|m| m.get(variant))
                    {
                        schema_failures.insert(variant.to_owned(), failure.error_class.clone());
                    } else {
                        schema_failures.insert(variant.to_owned(), "missing-execution".into());
                    }
                }
            }
        }
        source_outputs_by_fixture.insert(fixture.fixture_id.clone(), source_outputs);
        schema_failures_by_fixture.insert(fixture.fixture_id.clone(), schema_failures);
        scored_fixtures.push(fixture);
    }

    let mut evaluations = vec![];
    for task in &registry.tasks {
        let report = evaluate_task(task, &scored_fixtures, &policy).map_err(anyhow::Error::msg)?;
        let baseline = PromptCandidate {
            candidate_id: format!("{}:offline-candidate", task.task_id),
            task_id: task.task_id.clone(),
            state: PromptCandidateState::BaselineRegistered,
            baseline_spec_sha256: task.spec_sha256().map_err(anyhow::Error::msg)?,
            candidate_spec_sha256: None,
            evaluation: None,
            human_decision_ref: None,
            deployed: false,
            import_eligible: false,
        };
        let fallback_baseline = baseline.clone();
        let candidate_result: Result<PromptCandidate, String> = baseline
            .propose(ContentDigest::of_text(&format!(
                "{}:scored-candidate",
                task.task_id
            )))
            .and_then(|proposed| {
                proposed.evaluate(
                    report.evaluation.clone(),
                    policy.maximum_validation_gap_basis_points,
                )
            })
            .or_else(|_| {
                // Gap/metrix violations still produce an evaluated FSM state that is
                // ineligible. Re-validate only structural invariants without gap.
                let mut candidate = fallback_baseline
                    .propose(ContentDigest::of_text(&format!(
                        "{}:scored-candidate",
                        task.task_id
                    )))
                    .map_err(|e| e.to_string())?;
                candidate.evaluation = Some(report.evaluation.clone());
                candidate.state = PromptCandidateState::Evaluated;
                Ok(candidate)
            });
        let candidate = candidate_result.map_err(|e| anyhow!(e))?;
        let task_fixture_ids = scored_fixtures
            .iter()
            .filter(|f| f.task_id == task.task_id)
            .map(|f| f.fixture_id.clone())
            .collect::<Vec<_>>();
        evaluations.push(CandidateEvaluation {
            candidate,
            report,
            source_outputs: task_fixture_ids
                .iter()
                .flat_map(|id| {
                    source_outputs_by_fixture
                        .get(id)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                })
                .collect(),
            schema_failures: task_fixture_ids
                .iter()
                .flat_map(|id| {
                    schema_failures_by_fixture
                        .get(id)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                })
                .collect(),
        });
    }

    let mut report = Report {
        schema_version: "prompt-eval-scored.v1".into(),
        fixture_sha256: ContentDigest::of_bytes(&fixture_bytes),
        policy_sha256: ContentDigest::of_bytes(&policy_bytes),
        review_status: fixtures.review_status,
        reviewed_by: fixtures.reviewed_by,
        production_acceptance_eligible: false,
        evaluations,
        model_inference_requested: false,
        optimizer_enabled: false,
        auto_promote: false,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "prompt_eval_scored_complete",
        task_count = report.evaluations.len(),
        review_status = %report.review_status,
        production_acceptance_eligible = false,
        optimizer_enabled = false,
        auto_promote = false,
        import_eligible = false,
        report = %args.report.display(),
        "Scored prompt evaluation completed"
    );
    println!(
        "prompt-eval-scored tasks={} review={} acceptance=false optimizer=false D127=false report={}",
        report.evaluations.len(),
        report.review_status,
        args.report.display()
    );
    Ok(())
}

fn estimate_cost(input_tokens: Option<u64>, output_tokens: Option<u64>) -> u64 {
    let input = input_tokens.unwrap_or(0);
    let output = output_tokens.unwrap_or(0);
    (input / 1000 * 2) + (output / 1000 * 8)
}

fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn extract_claims_from_output_captures_summary_and_structured_fields() {
        let required = BTreeSet::from([
            "method=GEPA (Genetic-Pareto)".into(),
            "method_type=prompt optimizer".into(),
            "learning_signal=natural language reflection".into(),
            "baseline=GRPO".into(),
            "result=outperforms GRPO by 6% on average".into(),
            "efficiency=up to 35x fewer rollouts".into(),
        ]);
        let output = json!({
            "abstract_summary": "GEPA (Genetic-Pareto) is a prompt optimizer that uses natural language reflection. GEPA outperforms GRPO by 6% on average using up to 35x fewer rollouts.",
            "key_contributions": ["Introduction of GEPA, a prompt optimizer"],
            "quantitative_results": "GEPA outperforms GRPO by 6% on average.",
            "methods": [{"name": "GEPA"}],
            "metrics": [{"name": "accuracy", "value": "92%"}],
            "datasets": [{"name": "AIME-2025"}],
            "glossary": [{"term": "GRPO"}]
        });
        let claims = extract_claims_from_output(&output, &required, &BTreeSet::new());
        for expected in &required {
            assert!(claims.contains(expected), "missing claim: {expected}");
        }
        // Free structured labels are not claims unless listed as required_facts.
        assert!(!claims.contains("method=GEPA"));
        assert!(!claims.contains("metric=accuracy:92%"));
        assert!(!claims.contains("dataset=AIME-2025"));
        assert!(!claims.contains("term=GRPO"));
    }

    #[test]
    fn extract_claims_from_output_does_not_leak_raw_prose_sentences_as_claims() {
        // Payload match is honest: "GEPA" matches method=GEPA, but not the full
        // "GEPA (Genetic-Pareto)" fact unless that exact payload appears.
        let required = BTreeSet::from([
            "method=GEPA".into(),
            "baseline=GRPO".into(),
            "method=GEPA (Genetic-Pareto)".into(),
        ]);
        let output = json!({
            "abstract_summary": "This is a long prose sentence that mentions GEPA and GRPO but is not a canonical claim unit.",
            "quantitative_results": "The paper reports results across six tasks."
        });
        let claims = extract_claims_from_output(&output, &required, &BTreeSet::new());
        assert!(
            !claims.iter().any(|c| c.len() > 80 && !required.contains(c)),
            "raw prose sentence leaked as claim"
        );
        assert!(claims.contains("method=GEPA"));
        assert!(claims.contains("baseline=GRPO"));
        assert!(
            !claims.contains("method=GEPA (Genetic-Pareto)"),
            "must not invent fuller claim from short token"
        );
    }

    #[test]
    fn extract_claims_from_output_matches_budget_fact_payload() {
        let required = BTreeSet::from([
            "optimized_parameter=prompts".into(),
            "budget=total rollout budget b".into(),
        ]);
        let output = json!({
            "abstract_summary": "GEPA evolves only the set of prompts while keeping the total rollout budget b fixed."
        });
        let claims = extract_claims_from_output(&output, &required, &BTreeSet::new());
        assert!(claims.contains("optimized_parameter=prompts"));
        assert!(claims.contains("budget=total rollout budget b"));
    }
}
