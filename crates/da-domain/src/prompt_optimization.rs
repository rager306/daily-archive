//! Provider-neutral prompt task, execution lineage and candidate FSM (D238).

use crate::assurance::ContentDigest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptTaskSpec {
    pub task_id: String,
    pub version: String,
    pub system_template: String,
    pub user_template: String,
    pub output_schema_id: String,
    pub route_policy_id: String,
    pub temperature_milli: u16,
    pub max_output_tokens: u32,
    pub eval_suite_id: String,
    pub optimizer_enabled: bool,
}
impl PromptTaskSpec {
    pub fn validate(&self) -> Result<(), String> {
        let required = [
            self.task_id.as_str(),
            self.version.as_str(),
            self.system_template.as_str(),
            self.user_template.as_str(),
            self.output_schema_id.as_str(),
            self.route_policy_id.as_str(),
            self.eval_suite_id.as_str(),
        ];
        if required.into_iter().any(|x| x.trim().is_empty()) {
            return Err("prompt task identity/template is incomplete".into());
        }
        if self.temperature_milli > 2_000 || self.max_output_tokens == 0 {
            return Err("prompt task generation bounds invalid".into());
        }
        if self.optimizer_enabled {
            return Err("prompt optimizer must remain disabled until D136 gates pass".into());
        }
        Ok(())
    }
    pub fn spec_sha256(&self) -> Result<ContentDigest, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|b| ContentDigest::of_bytes(&b))
            .map_err(|e| e.to_string())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRoutePolicy {
    pub route_policy_id: String,
    pub model_binding_id: String,
    pub endpoint_override_env: Option<String>,
    pub credential_env: String,
    pub supports_images: bool,
    pub maximum_temperature_milli: u16,
}

impl PromptRoutePolicy {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.route_policy_id.as_str(),
            self.model_binding_id.as_str(),
            self.credential_env.as_str(),
        ]
        .into_iter()
        .any(|value| value.trim().is_empty())
        {
            return Err("prompt route policy is incomplete".into());
        }
        if self
            .endpoint_override_env
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("route endpoint override environment key is empty".into());
        }
        if self.maximum_temperature_milli > 2_000 {
            return Err("route temperature bound invalid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedModelBinding {
    pub binding_id: String,
    pub model_id: String,
    pub provider: String,
    pub protocol: String,
    pub endpoint: String,
    pub model_name: String,
    pub tool_version: String,
    pub policy_version: String,
}

impl ResolvedModelBinding {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.binding_id.as_str(),
            self.model_id.as_str(),
            self.provider.as_str(),
            self.protocol.as_str(),
            self.endpoint.as_str(),
            self.model_name.as_str(),
            self.tool_version.as_str(),
            self.policy_version.as_str(),
        ]
        .into_iter()
        .any(|value| value.trim().is_empty())
        {
            return Err("resolved model binding is incomplete".into());
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<ContentDigest, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|bytes| ContentDigest::of_bytes(&bytes))
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptOutputSchema {
    pub output_schema_id: String,
    pub schema: serde_json::Value,
}

impl PromptOutputSchema {
    pub fn validate(&self) -> Result<(), String> {
        if self.output_schema_id.trim().is_empty() || !self.schema.is_object() {
            return Err("prompt output schema is invalid".into());
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<ContentDigest, String> {
        self.validate()?;
        serde_json::to_vec(&self.schema)
            .map(|bytes| ContentDigest::of_bytes(&bytes))
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptExecutionLineage {
    pub run_id: String,
    pub task_id: String,
    pub task_spec_sha256: ContentDigest,
    pub rendered_prompt_sha256: ContentDigest,
    pub output_schema_sha256: ContentDigest,
    pub route_policy_sha256: ContentDigest,
    pub endpoint_sha256: ContentDigest,
    pub model_binding_sha256: ContentDigest,
    pub provider: String,
    pub model: String,
    pub input_sha256: ContentDigest,
    pub output_sha256: ContentDigest,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub latency_ms: u64,
    pub temperature_milli: u16,
    pub max_output_tokens: u32,
    pub estimated_cost_microunits: Option<u64>,
    pub authoritative: bool,
    pub import_eligible: bool,
}
impl PromptExecutionLineage {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.run_id.as_str(),
            self.task_id.as_str(),
            self.provider.as_str(),
            self.model.as_str(),
        ]
        .into_iter()
        .any(|x| x.trim().is_empty())
        {
            return Err("prompt execution identity incomplete".into());
        }
        if self.authoritative || self.import_eligible {
            return Err("prompt execution is candidate-only and cannot authorize import".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCandidateState {
    BaselineRegistered,
    CandidateProposed,
    Evaluated,
    Eligible,
    Rejected,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptEvaluation {
    pub baseline_n: u64,
    pub candidate_n: u64,
    pub train_fixture_sha256: ContentDigest,
    pub validation_fixture_sha256: ContentDigest,
    pub metric_basis_points: BTreeMap<String, u32>,
    pub validation_gap_basis_points: u32,
    pub hard_invariants_pass: bool,
    pub contamination_detected: bool,
    pub latency_budget_pass: bool,
    pub cost_budget_pass: bool,
}
impl PromptEvaluation {
    /// Structural FSM invariants. Execution gates (hard invariants, contamination,
    /// latency, cost) are NOT part of FSM state validity; they gate eligibility only.
    pub fn validate(&self, max_validation_gap_basis_points: u32) -> Result<(), String> {
        if self.baseline_n == 0 || self.baseline_n != self.candidate_n {
            return Err("prompt evaluation requires nonzero same-n baseline/candidate".into());
        }
        if self.metric_basis_points.is_empty()
            || self.metric_basis_points.values().any(|x| *x > 10_000)
        {
            return Err("prompt evaluation metric vector invalid".into());
        }
        if self.validation_gap_basis_points > max_validation_gap_basis_points {
            return Err("prompt evaluation validation gap exceeds policy".into());
        }
        Ok(())
    }
    /// Execution quality gates; false means fail-closed and never eligible.
    pub fn gates_passed(&self) -> bool {
        self.hard_invariants_pass
            && !self.contamination_detected
            && self.latency_budget_pass
            && self.cost_budget_pass
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptCandidate {
    pub candidate_id: String,
    pub task_id: String,
    pub state: PromptCandidateState,
    pub baseline_spec_sha256: ContentDigest,
    pub candidate_spec_sha256: Option<ContentDigest>,
    pub evaluation: Option<PromptEvaluation>,
    pub human_decision_ref: Option<String>,
    pub deployed: bool,
    pub import_eligible: bool,
}
impl PromptCandidate {
    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_id.trim().is_empty()
            || self.task_id.trim().is_empty()
            || self.deployed
            || self.import_eligible
        {
            return Err("prompt candidate identity/deployment/D127 invalid".into());
        }
        match self.state {
            PromptCandidateState::BaselineRegistered
                if self.candidate_spec_sha256.is_none() && self.evaluation.is_none() =>
            {
                Ok(())
            }
            PromptCandidateState::CandidateProposed
                if self.candidate_spec_sha256.is_some() && self.evaluation.is_none() =>
            {
                Ok(())
            }
            PromptCandidateState::Evaluated
            | PromptCandidateState::Eligible
            | PromptCandidateState::Rejected
                if self.candidate_spec_sha256.is_some() && self.evaluation.is_some() =>
            {
                Ok(())
            }
            _ => Err("prompt candidate state payload mismatch".into()),
        }
    }
    pub fn propose(mut self, spec: ContentDigest) -> Result<Self, String> {
        if self.state != PromptCandidateState::BaselineRegistered {
            return Err("candidate proposal requires baseline_registered".into());
        }
        self.candidate_spec_sha256 = Some(spec);
        self.state = PromptCandidateState::CandidateProposed;
        self.validate()?;
        Ok(self)
    }
    pub fn evaluate(mut self, evaluation: PromptEvaluation, max_gap: u32) -> Result<Self, String> {
        if self.state != PromptCandidateState::CandidateProposed {
            return Err("evaluation requires candidate_proposed".into());
        }
        evaluation.validate(max_gap)?;
        self.evaluation = Some(evaluation);
        self.state = PromptCandidateState::Evaluated;
        self.validate()?;
        Ok(self)
    }
    pub fn mark_eligible(mut self) -> Result<Self, String> {
        if self.state != PromptCandidateState::Evaluated {
            return Err("eligibility requires evaluated".into());
        }
        if !self
            .evaluation
            .as_ref()
            .is_some_and(|evaluation| evaluation.gates_passed())
        {
            return Err("eligibility requires passed execution gates".into());
        }
        self.state = PromptCandidateState::Eligible;
        self.validate()?;
        Ok(self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    fn base() -> PromptCandidate {
        PromptCandidate {
            candidate_id: "c1".into(),
            task_id: "summary".into(),
            state: PromptCandidateState::BaselineRegistered,
            baseline_spec_sha256: ContentDigest::of_text("base"),
            candidate_spec_sha256: None,
            evaluation: None,
            human_decision_ref: None,
            deployed: false,
            import_eligible: false,
        }
    }
    fn eval() -> PromptEvaluation {
        PromptEvaluation {
            baseline_n: 10,
            candidate_n: 10,
            train_fixture_sha256: ContentDigest::of_text("train"),
            validation_fixture_sha256: ContentDigest::of_text("val"),
            metric_basis_points: BTreeMap::from([("faithfulness".into(), 9000)]),
            validation_gap_basis_points: 200,
            hard_invariants_pass: true,
            contamination_detected: false,
            latency_budget_pass: true,
            cost_budget_pass: true,
        }
    }
    #[test]
    fn legal_candidate_path_stops_at_eligible_not_deployed() {
        let x = base()
            .propose(ContentDigest::of_text("candidate"))
            .unwrap()
            .evaluate(eval(), 500)
            .unwrap()
            .mark_eligible()
            .unwrap();
        assert_eq!(x.state, PromptCandidateState::Eligible);
        assert!(!x.deployed);
    }
    #[test]
    fn same_n_gap_and_illegal_transition_fail() {
        let mut e = eval();
        e.candidate_n = 9;
        assert!(
            base()
                .propose(ContentDigest::of_text("x"))
                .unwrap()
                .evaluate(e, 500)
                .is_err()
        );
        assert!(base().mark_eligible().is_err());
    }
    #[test]
    fn task_and_lineage_are_candidate_only() {
        let task = PromptTaskSpec {
            task_id: "summary".into(),
            version: "v1".into(),
            system_template: "system".into(),
            user_template: "{text}".into(),
            output_schema_id: "schema".into(),
            route_policy_id: "anthropic".into(),
            temperature_milli: 300,
            max_output_tokens: 1000,
            eval_suite_id: "summary-v1".into(),
            optimizer_enabled: false,
        };
        task.validate().unwrap();
        let mut bad = task.clone();
        bad.optimizer_enabled = true;
        assert!(bad.validate().is_err());
    }

    proptest! {
        /// ADR-051 P1: no-skip transitions — propose requires BaselineRegistered.
        #[test]
        fn prop_propose_requires_baseline(state in 0u8..=4) {
            let s = match state {
                0 => PromptCandidateState::BaselineRegistered,
                1 => PromptCandidateState::CandidateProposed,
                2 => PromptCandidateState::Evaluated,
                3 => PromptCandidateState::Eligible,
                _ => PromptCandidateState::Rejected,
            };
            let mut c = base();
            c.state = s;
            let result = c.propose(ContentDigest::of_text("x"));
            if s == PromptCandidateState::BaselineRegistered {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }

        /// ADR-051 P1: no-skip — evaluate requires CandidateProposed.
        #[test]
        fn prop_evaluate_requires_proposed(state in 0u8..=4) {
            let s = match state {
                0 => PromptCandidateState::BaselineRegistered,
                1 => PromptCandidateState::CandidateProposed,
                2 => PromptCandidateState::Evaluated,
                3 => PromptCandidateState::Eligible,
                _ => PromptCandidateState::Rejected,
            };
            let mut c = base();
            c.state = s;
            c.candidate_spec_sha256 = Some(ContentDigest::of_text("c"));
            let result = c.evaluate(eval(), 500);
            if s == PromptCandidateState::CandidateProposed {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }

        /// ADR-051 P1: terminal states are frozen — mark_eligible only from Evaluated.
        #[test]
        fn prop_mark_eligible_requires_evaluated(state in 0u8..=4) {
            let s = match state {
                0 => PromptCandidateState::BaselineRegistered,
                1 => PromptCandidateState::CandidateProposed,
                2 => PromptCandidateState::Evaluated,
                3 => PromptCandidateState::Eligible,
                _ => PromptCandidateState::Rejected,
            };
            let mut c = base();
            c.state = s;
            c.candidate_spec_sha256 = Some(ContentDigest::of_text("c"));
            c.evaluation = Some(eval());
            let result = c.mark_eligible();
            if s == PromptCandidateState::Evaluated {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }

        /// ADR-051 P1: D127 and deployed stay false through every valid path.
        #[test]
        fn prop_d127_and_deployed_always_false(path in 0u8..=3) {
            let c = base();
            let result = match path {
                0 => Some(c.clone()),
                1 => Some(c.propose(ContentDigest::of_text("c")).unwrap()),
                2 => Some(c.propose(ContentDigest::of_text("c")).unwrap().evaluate(eval(), 500).unwrap()),
                _ => Some(c.propose(ContentDigest::of_text("c")).unwrap().evaluate(eval(), 500).unwrap().mark_eligible().unwrap()),
            };
            if let Some(r) = result {
                prop_assert!(!r.deployed, "deployed leaked true");
                prop_assert!(!r.import_eligible, "import_eligible leaked true");
            }
        }

        /// ADR-051 P1: mark_eligible fails when execution gates are not passed.
        #[test]
        fn prop_eligible_requires_passed_gates(hard_pass in any::<bool>(), contam in any::<bool>(), lat in any::<bool>(), cost in any::<bool>()) {
            let mut e = eval();
            e.hard_invariants_pass = hard_pass;
            e.contamination_detected = contam;
            e.latency_budget_pass = lat;
            e.cost_budget_pass = cost;
            let gates_ok = hard_pass && !contam && lat && cost;
            let c = base()
                .propose(ContentDigest::of_text("c"))
                .unwrap()
                .evaluate(e, 500)
                .unwrap();
            let result = c.mark_eligible();
            if gates_ok {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }
    }
}
