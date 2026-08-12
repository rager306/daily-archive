//! Independent, deterministic offline evaluation for prompt candidate FSM evidence.

use da_domain::{
    assurance::ContentDigest,
    prompt_optimization::{PromptEvaluation, PromptTaskSpec},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalSplit {
    Train,
    Validation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPromptFixture {
    pub fixture_id: String,
    pub task_id: String,
    pub split: EvalSplit,
    pub source_text: String,
    pub source_sha256: ContentDigest,
    pub required_facts: BTreeSet<String>,
    pub forbidden_facts: BTreeSet<String>,
    pub baseline: FrozenPromptOutput,
    pub candidate: FrozenPromptOutput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPromptOutput {
    pub schema_valid: bool,
    pub claims: BTreeSet<String>,
    pub paraphrase_invariant_pass: bool,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_microunits: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptEvalPolicy {
    pub schema_version: String,
    pub minimum_metric_basis_points: u32,
    pub maximum_validation_gap_basis_points: u32,
    pub maximum_latency_ms: u64,
    pub maximum_cost_microunits: u64,
    pub optimizer_enabled: bool,
    pub auto_promote: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEvaluationReport {
    pub task_id: String,
    pub baseline_n: u64,
    pub candidate_n: u64,
    pub train_fixture_sha256: ContentDigest,
    pub validation_fixture_sha256: ContentDigest,
    pub baseline_metrics: BTreeMap<String, u32>,
    pub candidate_metrics: BTreeMap<String, u32>,
    pub validation_gap_basis_points: u32,
    pub hard_invariants_pass: bool,
    pub contamination_detected: bool,
    pub latency_budget_pass: bool,
    pub cost_budget_pass: bool,
    pub evaluation: PromptEvaluation,
    pub eligible: bool,
    pub deployed: bool,
    pub import_eligible: bool,
}
impl PromptEvalPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "prompt-eval-policy.v1"
            || self.minimum_metric_basis_points > 10_000
            || self.maximum_validation_gap_basis_points > 10_000
            || self.maximum_latency_ms == 0
            || self.maximum_cost_microunits == 0
            || self.optimizer_enabled
            || self.auto_promote
        {
            return Err("prompt eval policy invalid or optimizer enabled".into());
        }
        Ok(())
    }
}
pub fn evaluate_task(
    task: &PromptTaskSpec,
    fixtures: &[FrozenPromptFixture],
    policy: &PromptEvalPolicy,
) -> Result<TaskEvaluationReport, String> {
    task.validate()?;
    policy.validate()?;
    let selected = fixtures
        .iter()
        .filter(|x| x.task_id == task.task_id)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!("no frozen fixtures for {}", task.task_id));
    }
    let baseline_n = selected.len() as u64;
    let candidate_n = selected.len() as u64;
    let mut ids = BTreeSet::new();
    let mut train_sources = BTreeSet::new();
    let mut validation_sources = BTreeSet::new();
    for f in &selected {
        if f.fixture_id.trim().is_empty()
            || !ids.insert(f.fixture_id.as_str())
            || f.source_text.trim().is_empty()
            || f.required_facts.is_empty()
        {
            return Err("fixture identity/source/required facts invalid".into());
        }
        if ContentDigest::of_text(&f.source_text) != f.source_sha256 {
            return Err(format!("fixture {} source digest mismatch", f.fixture_id));
        }
        if !f.required_facts.is_disjoint(&f.forbidden_facts) {
            return Err(format!(
                "fixture {} required/forbidden overlap",
                f.fixture_id
            ));
        }
        match f.split {
            EvalSplit::Train => {
                train_sources.insert(f.source_sha256.clone());
            }
            EvalSplit::Validation => {
                validation_sources.insert(f.source_sha256.clone());
            }
        }
    }
    if train_sources.is_empty() || validation_sources.is_empty() {
        return Err("both train and validation fixtures are required".into());
    }
    let contamination = !train_sources.is_disjoint(&validation_sources);
    let baseline_metrics = metrics(&selected, false);
    let candidate_metrics = metrics(&selected, true);
    let train_candidate = metrics(
        &selected
            .iter()
            .copied()
            .filter(|x| x.split == EvalSplit::Train)
            .collect::<Vec<_>>(),
        true,
    );
    let val_candidate = metrics(
        &selected
            .iter()
            .copied()
            .filter(|x| x.split == EvalSplit::Validation)
            .collect::<Vec<_>>(),
        true,
    );
    let gap = train_candidate
        .iter()
        .map(|(key, value)| value.abs_diff(*val_candidate.get(key).unwrap_or(&0)))
        .max()
        .unwrap_or(0);
    let hard = candidate_metrics
        .values()
        .all(|x| *x >= policy.minimum_metric_basis_points);
    let latency = selected
        .iter()
        .all(|x| x.candidate.latency_ms <= policy.maximum_latency_ms);
    let cost = selected
        .iter()
        .all(|x| x.candidate.estimated_cost_microunits <= policy.maximum_cost_microunits);
    let evaluation = PromptEvaluation {
        baseline_n,
        candidate_n,
        train_fixture_sha256: digest_fixtures(&selected, EvalSplit::Train)?,
        validation_fixture_sha256: digest_fixtures(&selected, EvalSplit::Validation)?,
        metric_basis_points: candidate_metrics.clone(),
        validation_gap_basis_points: gap,
        hard_invariants_pass: hard,
        contamination_detected: contamination,
        latency_budget_pass: latency,
        cost_budget_pass: cost,
    };
    evaluation.validate(policy.maximum_validation_gap_basis_points)?;
    Ok(TaskEvaluationReport {
        task_id: task.task_id.clone(),
        baseline_n,
        candidate_n,
        train_fixture_sha256: evaluation.train_fixture_sha256.clone(),
        validation_fixture_sha256: evaluation.validation_fixture_sha256.clone(),
        baseline_metrics,
        candidate_metrics,
        validation_gap_basis_points: gap,
        hard_invariants_pass: hard,
        contamination_detected: contamination,
        latency_budget_pass: latency,
        cost_budget_pass: cost,
        evaluation,
        eligible: false,
        deployed: false,
        import_eligible: false,
    })
}
fn metrics(fixtures: &[&FrozenPromptFixture], candidate: bool) -> BTreeMap<String, u32> {
    let mut schema = 0;
    let mut grounded_num = 0;
    let mut grounded_den = 0;
    let mut coverage_num = 0;
    let mut coverage_den = 0;
    let mut paraphrase = 0;
    for f in fixtures {
        let output = if candidate { &f.candidate } else { &f.baseline };
        schema += usize::from(output.schema_valid);
        paraphrase += usize::from(output.paraphrase_invariant_pass);
        grounded_den += output.claims.len();
        grounded_num += output
            .claims
            .iter()
            .filter(|claim| {
                f.required_facts.contains(*claim) && !f.forbidden_facts.contains(*claim)
            })
            .count();
        coverage_den += f.required_facts.len();
        coverage_num += f.required_facts.intersection(&output.claims).count();
    }
    let ratio = |n: usize, d: usize| {
        if d == 0 {
            0
        } else {
            ((n as u128 * 10_000) / d as u128) as u32
        }
    };
    BTreeMap::from([
        ("schema_validity".into(), ratio(schema, fixtures.len())),
        ("grounding".into(), ratio(grounded_num, grounded_den)),
        ("faithfulness".into(), ratio(grounded_num, grounded_den)),
        ("coverage".into(), ratio(coverage_num, coverage_den)),
        (
            "paraphrase_stability".into(),
            ratio(paraphrase, fixtures.len()),
        ),
    ])
}
fn digest_fixtures(
    fixtures: &[&FrozenPromptFixture],
    split: EvalSplit,
) -> Result<ContentDigest, String> {
    let selected = fixtures
        .iter()
        .filter(|x| x.split == split)
        .map(|x| (*x).clone())
        .collect::<Vec<_>>();
    serde_json::to_vec(&selected)
        .map(|x| ContentDigest::of_bytes(&x))
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn task() -> PromptTaskSpec {
        PromptTaskSpec {
            task_id: "summary".into(),
            version: "v1".into(),
            system_template: "s".into(),
            user_template: "u".into(),
            output_schema_id: "x".into(),
            route_policy_id: "r".into(),
            temperature_milli: 0,
            max_output_tokens: 10,
            eval_suite_id: "eval".into(),
            optimizer_enabled: false,
        }
    }
    fn fixture(id: &str, split: EvalSplit, source: &str) -> FrozenPromptFixture {
        let facts = BTreeSet::from(["fact-a".into()]);
        FrozenPromptFixture {
            fixture_id: id.into(),
            task_id: "summary".into(),
            split,
            source_text: source.into(),
            source_sha256: ContentDigest::of_text(source),
            required_facts: facts.clone(),
            forbidden_facts: BTreeSet::from(["false".into()]),
            baseline: FrozenPromptOutput {
                schema_valid: true,
                claims: facts.clone(),
                paraphrase_invariant_pass: true,
                latency_ms: 10,
                input_tokens: 1,
                output_tokens: 1,
                estimated_cost_microunits: 1,
            },
            candidate: FrozenPromptOutput {
                schema_valid: true,
                claims: facts,
                paraphrase_invariant_pass: true,
                latency_ms: 8,
                input_tokens: 1,
                output_tokens: 1,
                estimated_cost_microunits: 1,
            },
        }
    }
    fn policy() -> PromptEvalPolicy {
        PromptEvalPolicy {
            schema_version: "prompt-eval-policy.v1".into(),
            minimum_metric_basis_points: 10_000,
            maximum_validation_gap_basis_points: 0,
            maximum_latency_ms: 100,
            maximum_cost_microunits: 10,
            optimizer_enabled: false,
            auto_promote: false,
        }
    }
    #[test]
    fn independent_same_n_evaluation_stays_candidate_only() {
        let r = evaluate_task(
            &task(),
            &[
                fixture("a", EvalSplit::Train, "source-a"),
                fixture("b", EvalSplit::Validation, "source-b"),
            ],
            &policy(),
        )
        .unwrap();
        assert_eq!(r.baseline_n, r.candidate_n);
        assert!(!r.eligible && !r.deployed && !r.import_eligible);
        assert_eq!(r.candidate_metrics["faithfulness"], 10_000);
    }
    #[test]
    fn contamination_and_budget_fail_closed() {
        let fixtures = [
            fixture("a", EvalSplit::Train, "same"),
            fixture("b", EvalSplit::Validation, "same"),
        ];
        assert!(evaluate_task(&task(), &fixtures, &policy()).is_err());
        let mut fixtures = [
            fixture("a", EvalSplit::Train, "a"),
            fixture("b", EvalSplit::Validation, "b"),
        ];
        fixtures[1].candidate.latency_ms = 101;
        assert!(evaluate_task(&task(), &fixtures, &policy()).is_err());
    }
}
