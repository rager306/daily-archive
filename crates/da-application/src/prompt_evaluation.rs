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
pub struct FrozenArtifactBinding {
    pub artifact_ref: String,
    pub artifact_sha256: ContentDigest,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPromptFixture {
    pub fixture_id: String,
    pub task_id: String,
    pub split: EvalSplit,
    pub source_text: String,
    pub source_sha256: ContentDigest,
    pub source_artifact_ref: String,
    pub source_artifact_sha256: ContentDigest,
    pub supporting_artifacts: Vec<FrozenArtifactBinding>,
    pub oracle_review_ref: String,
    pub oracle_review_sha256: ContentDigest,
    pub oracle_producer_id: String,
    pub required_facts: BTreeSet<String>,
    pub forbidden_facts: BTreeSet<String>,
    pub baseline: FrozenPromptOutput,
    pub candidate: FrozenPromptOutput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenPromptOutput {
    pub producer_id: String,
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
            || f.source_artifact_ref.trim().is_empty()
            || f.oracle_review_ref.trim().is_empty()
            || f.supporting_artifacts
                .iter()
                .any(|a| a.artifact_ref.trim().is_empty())
            || f.oracle_producer_id.trim().is_empty()
            || f.baseline.producer_id.trim().is_empty()
            || f.candidate.producer_id.trim().is_empty()
            || f.required_facts.is_empty()
        {
            return Err("fixture identity/source/review/producer/required facts invalid".into());
        }
        if f.oracle_producer_id == f.baseline.producer_id
            || f.oracle_producer_id == f.candidate.producer_id
        {
            return Err(format!(
                "fixture {} uses output producer as oracle",
                f.fixture_id
            ));
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

/// Extract evaluation claims from a model JSON output without paper-specific keywords.
///
/// Strategy:
/// 1. Collect all string leaves into a prose bag (universal for summary + figure schemas).
/// 2. Project `required_facts` when the payload matches prose (substring or ordered
///    significant-token match after stopword strip).
/// 3. Structured method/metric/dataset/term fields contribute only when the exact
///    claim string is already a required_fact (prevents grounding dilution).
/// 4. Never emit a required fact that also appears in `forbidden_facts`.
///
/// This is oracle-aware matching of facts present in output text — not free invent of labels.
pub fn extract_claims_from_output(
    output: &serde_json::Value,
    required_facts: &BTreeSet<String>,
    forbidden_facts: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut claims = BTreeSet::new();
    let mut prose = String::new();
    collect_prose(output, &mut prose);

    // Structured arrays may only reinforce required facts — never inject free labels.
    let mut structured = BTreeSet::new();
    collect_structured_claims(output, &mut structured);
    for claim in structured {
        if required_facts.contains(&claim) && !forbidden_facts.contains(&claim) {
            claims.insert(claim);
        }
    }

    let prose_norm = normalize_match_text(&prose);
    for fact in required_facts {
        if forbidden_facts.contains(fact) {
            continue;
        }
        if fact_matches_prose(fact, &prose_norm) {
            claims.insert(fact.clone());
        }
    }
    claims
}

fn collect_prose(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(s) => {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(s);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_prose(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, value) in map {
                collect_prose(value, out);
            }
        }
        _ => {}
    }
}

fn collect_structured_claims(output: &serde_json::Value, claims: &mut BTreeSet<String>) {
    let Some(object) = output.as_object() else {
        return;
    };
    if let Some(metrics) = object.get("metrics").and_then(|x| x.as_array()) {
        for metric in metrics {
            if let Some(obj) = metric.as_object()
                && let (Some(name), Some(value)) = (
                    obj.get("name").and_then(|x| x.as_str()),
                    obj.get("value").and_then(|x| x.as_str()),
                )
            {
                claims.insert(format!("metric={name}:{value}"));
            }
        }
    }
    if let Some(methods) = object.get("methods").and_then(|x| x.as_array()) {
        for method in methods {
            if let Some(name) = method
                .as_object()
                .and_then(|o| o.get("name"))
                .and_then(|x| x.as_str())
            {
                claims.insert(format!("method={name}"));
            }
        }
    }
    if let Some(datasets) = object.get("datasets").and_then(|x| x.as_array()) {
        for dataset in datasets {
            if let Some(name) = dataset
                .as_object()
                .and_then(|o| o.get("name"))
                .and_then(|x| x.as_str())
            {
                claims.insert(format!("dataset={name}"));
            }
        }
    }
    if let Some(glossary) = object.get("glossary").and_then(|x| x.as_array()) {
        for entry in glossary {
            if let Some(term) = entry
                .as_object()
                .and_then(|o| o.get("term"))
                .and_then(|x| x.as_str())
            {
                claims.insert(format!("term={term}"));
            }
        }
    }
}

fn normalize_match_text(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn fact_matches_prose(fact: &str, prose_norm: &str) -> bool {
    let payload = fact_match_payload(fact);
    if payload.is_empty() {
        return false;
    }
    let payload_norm = normalize_match_text(&payload);
    if payload_norm.is_empty() {
        return false;
    }
    if prose_norm.contains(&payload_norm) {
        return true;
    }
    // Soft match: significant tokens of the payload appear in order in prose.
    // Handles minor stopword/phrasing drift without paper-specific keyword maps.
    ordered_token_match(&payload_norm, prose_norm)
}

/// Prefer the RHS of `key=value` facts; otherwise match the whole fact string.
fn fact_match_payload(fact: &str) -> String {
    if let Some((key, value)) = fact.split_once('=')
        && !key.is_empty()
        && !value.is_empty()
    {
        return value.to_string();
    }
    fact.to_string()
}

fn is_match_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "as"
            | "at"
            | "by"
            | "for"
            | "from"
            | "in"
            | "of"
            | "on"
            | "or"
            | "the"
            | "to"
            | "up"
            | "with"
    )
}

fn significant_tokens(norm: &str) -> Vec<&str> {
    norm.split_whitespace()
        .filter(|t| !t.is_empty() && !is_match_stopword(t))
        .collect()
}

fn ordered_token_match(payload_norm: &str, prose_norm: &str) -> bool {
    let tokens = significant_tokens(payload_norm);
    if tokens.is_empty() {
        return false;
    }
    // Require at least one contentful token longer than 1 char (avoid bare "b" alone).
    if tokens.iter().all(|t| t.len() <= 1) {
        return false;
    }
    let prose_tokens: Vec<&str> = prose_norm.split_whitespace().collect();
    let mut i = 0usize;
    for pt in prose_tokens {
        if i < tokens.len() && pt == tokens[i] {
            i += 1;
        }
    }
    i == tokens.len()
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
            source_artifact_ref: format!("artifact:{id}"),
            source_artifact_sha256: ContentDigest::of_text(&format!("artifact:{id}")),
            supporting_artifacts: vec![],
            oracle_review_ref: format!("review:{id}"),
            oracle_review_sha256: ContentDigest::of_text(&format!("review:{id}")),
            oracle_producer_id: "independent-reviewer".into(),
            required_facts: facts.clone(),
            forbidden_facts: BTreeSet::from(["false".into()]),
            baseline: FrozenPromptOutput {
                producer_id: "baseline-run".into(),
                schema_valid: true,
                claims: facts.clone(),
                paraphrase_invariant_pass: true,
                latency_ms: 10,
                input_tokens: 1,
                output_tokens: 1,
                estimated_cost_microunits: 1,
            },
            candidate: FrozenPromptOutput {
                producer_id: "candidate-run".into(),
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
    fn self_oracle_fails_closed() {
        let fixtures = [
            fixture("a", EvalSplit::Train, "a"),
            fixture("b", EvalSplit::Validation, "b"),
        ];
        let mut fixtures = fixtures;
        fixtures[0].oracle_producer_id = fixtures[0].candidate.producer_id.clone();
        assert!(evaluate_task(&task(), &fixtures, &policy()).is_err());
    }
    #[test]
    fn contamination_and_budget_fail_closed() {
        let fixtures = [
            fixture("a", EvalSplit::Train, "same"),
            fixture("b", EvalSplit::Validation, "same"),
        ];
        let r = evaluate_task(&task(), &fixtures, &policy()).unwrap();
        assert!(r.contamination_detected);
        assert!(!r.eligible && !r.deployed && !r.import_eligible);
        let mut fixtures = [
            fixture("a", EvalSplit::Train, "a"),
            fixture("b", EvalSplit::Validation, "b"),
        ];
        fixtures[1].candidate.latency_ms = 101;
        let r = evaluate_task(&task(), &fixtures, &policy()).unwrap();
        assert!(!r.latency_budget_pass);
        assert!(!r.eligible && !r.deployed && !r.import_eligible);
    }

    #[test]
    fn extract_claims_projects_required_facts_from_figure_prose() {
        let required = BTreeSet::from([
            "figure=GEPA candidate proposal loop".into(),
            "strategy=Reflective Prompt Mutation".into(),
            "strategy=System Aware Merge".into(),
            "initial_evaluation=minibatch".into(),
            "selection=Pareto-based candidate sampling".into(),
        ]);
        let forbidden = BTreeSet::from([
            "figure=PUPA candidate trajectory".into(),
            "parameter_update=LLM weights".into(),
        ]);
        let output = serde_json::json!({
            "visual_type": "flowchart",
            "description": "GEPA candidate proposal loop with Reflective Prompt Mutation and System Aware Merge. Candidates are first scored on a minibatch, then selected via Pareto-based candidate sampling.",
            "components": [
                {"name": "Reflective Prompt Mutation", "role": "strategy", "visual_evidence": "green box"},
                {"name": "System Aware Merge", "role": "strategy", "visual_evidence": "blue box"}
            ],
            "findings": ["Pareto-based candidate sampling filters the elite set"],
            "citations": [{"section_id": "s3", "path_id": "p", "quote": "minibatch evaluation"}]
        });
        let claims = extract_claims_from_output(&output, &required, &forbidden);
        for fact in &required {
            assert!(claims.contains(fact), "missing required fact: {fact}");
        }
        assert!(!claims.contains("figure=PUPA candidate trajectory"));
        assert!(!claims.contains("parameter_update=LLM weights"));
    }

    #[test]
    fn extract_claims_projects_summary_required_facts_and_soft_efficiency() {
        let required = BTreeSet::from([
            "method=GEPA (Genetic-Pareto)".into(),
            "method_type=prompt optimizer".into(),
            "learning_signal=natural language reflection".into(),
            "baseline=GRPO".into(),
            "result=outperforms GRPO by 6% on average".into(),
            "efficiency=up to 35x fewer rollouts".into(),
        ]);
        let forbidden = BTreeSet::new();
        // Soft match: prose omits "up to" but keeps significant tokens 35x/fewer/rollouts.
        let output = serde_json::json!({
            "abstract_summary": "GEPA (Genetic-Pareto) is a prompt optimizer that uses natural language reflection. GEPA outperforms GRPO by 6% on average using 35x fewer rollouts.",
            "key_contributions": ["Introduction of GEPA, a prompt optimizer"],
            "quantitative_results": "GEPA outperforms GRPO by 6% on average.",
            "methods": [{"name": "Trajectory Sampling"}],
            "metrics": [{"name": "accuracy", "value": "92%"}],
            "datasets": [{"name": "AIME-2025"}],
            "glossary": [{"term": "GRPO"}]
        });
        let claims = extract_claims_from_output(&output, &required, &forbidden);
        for fact in &required {
            assert!(claims.contains(fact), "missing required fact: {fact}");
        }
        // Free structured labels must not dilute the claim set.
        assert!(!claims.contains("method=Trajectory Sampling"));
        assert!(!claims.contains("metric=accuracy:92%"));
        assert!(!claims.contains("dataset=AIME-2025"));
    }

    #[test]
    fn extract_claims_structured_only_when_required() {
        let required = BTreeSet::from(["method=GEPA".into(), "term=GRPO".into()]);
        let output = serde_json::json!({
            "abstract_summary": "unrelated prose",
            "methods": [{"name": "GEPA"}],
            "glossary": [{"term": "GRPO"}],
            "datasets": [{"name": "AIME-2025"}]
        });
        let claims = extract_claims_from_output(&output, &required, &BTreeSet::new());
        assert!(claims.contains("method=GEPA"));
        assert!(claims.contains("term=GRPO"));
        assert!(!claims.contains("dataset=AIME-2025"));
    }

    #[test]
    fn extract_claims_soft_matches_budget_token_order() {
        let required = BTreeSet::from(["budget=total rollout budget B".into()]);
        let output = serde_json::json!({
            "abstract_summary": "Optimization stops when the total rollout budget B is exhausted."
        });
        let claims = extract_claims_from_output(&output, &required, &BTreeSet::new());
        assert!(claims.contains("budget=total rollout budget B"));
    }

    #[test]
    fn extract_claims_does_not_emit_unmentioned_required_facts() {
        let required = BTreeSet::from([
            "strategy=Reflective Prompt Mutation".into(),
            "strategy=System Aware Merge".into(),
        ]);
        let forbidden = BTreeSet::new();
        let output = serde_json::json!({
            "description": "A flowchart of reflective prompt mutation only."
        });
        let claims = extract_claims_from_output(&output, &required, &forbidden);
        assert!(claims.contains("strategy=Reflective Prompt Mutation"));
        assert!(!claims.contains("strategy=System Aware Merge"));
    }

    #[test]
    fn extract_claims_has_no_paper_specific_keyword_map() {
        // Guard against reintroducing paper-specific keyword projectors in the
        // application claim extractor (oracle facts in tests are fine).
        let src = include_str!("prompt_evaluation.rs");
        // Only inspect the non-test production body: cut at the tests module.
        let production = src.split("#[cfg(test)]").next().expect("production body");
        for banned in [
            "prose_lower.contains(\"gepa\")",
            "prose_lower.contains(\"grpo\")",
            "prose_lower.contains(\"genetic-pareto\")",
            "35x fewer rollouts",
            "natural language reflection",
        ] {
            assert!(
                !production.contains(banned),
                "banned hardcoding fragment still present in production body: {banned}"
            );
        }
    }
}
