//! Agent manifest types (M451 W1): agents are DATA objects per the
//! agent.schema norm. Wire types only; invariants ride the generic
//! schema_invariants engine over the manifest as a Value.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentManifest {
    pub schema_version: String,
    pub agent_id: String,
    pub agent_version: String,
    #[serde(default)]
    pub schema_ref: String,
    pub identity: Identity,
    pub model: ModelBinding,
    pub prompt: PromptSpec,
    #[serde(default)]
    pub tools: Tools,
    pub memory: Memory,
    pub optimization: Optimization,
    pub guardrails: Guardrails,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub role: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBinding {
    pub generate_binding: String,
    #[serde(default)]
    pub fallback_binding: Option<String>,
    #[serde(default)]
    pub judge_binding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSpec {
    pub system_file: String,
    #[serde(default)]
    pub inputs: serde_yaml::Mapping,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tools {
    #[serde(default)]
    pub read_tools: Vec<String>,
    #[serde(default)]
    pub write_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub backend: String, // cvf | none
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub trajectory: bool,
    #[serde(default)]
    pub feedback_sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Optimization {
    pub optimizer_enabled: bool, // lock: false
    #[serde(default)]
    pub frozen_eval: Option<String>,
    #[serde(default)]
    pub candidate_slots: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guardrails {
    pub injection_policy: String,
    pub max_output_tokens: usize,
    pub rate_limit_per_min: u32,
}

impl AgentManifest {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// Hard lock check (ADR-059/D136): optimizer must stay off in prod
    /// manifests; enabled requires frozen_eval present.
    pub fn check_locks(&self) -> Result<(), String> {
        if self.optimization.optimizer_enabled && self.optimization.frozen_eval.is_none() {
            return Err(format!(
                "agent {}: optimizer_enabled=true without frozen_eval (ADR-059 lock)",
                self.agent_id
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "schema_version: agent-manifest.v1\nagent_id: summarizer\nagent_version: 1.0.0\nidentity: {role: summarizer, description: d, enabled: true}\nmodel: {generate_binding: paper-summary-generate-default}\nprompt: {system_file: data/prompts/summarizer/system.v1.md}\nmemory: {backend: cvf, trajectory: true}\noptimization: {optimizer_enabled: false, candidate_slots: []}\nguardrails: {injection_policy: p, max_output_tokens: 4000, rate_limit_per_min: 12}\n";

    #[test]
    fn parses_manifest_and_locks_hold() {
        let m = AgentManifest::from_yaml_str(M).unwrap();
        assert_eq!(m.agent_id, "summarizer");
        assert_eq!(m.model.generate_binding, "paper-summary-generate-default");
        assert!(m.check_locks().is_ok());
        assert!(m.memory.trajectory);
    }

    #[test]
    fn optimizer_without_frozen_eval_is_rejected() {
        let bad = M.replace("optimizer_enabled: false", "optimizer_enabled: true");
        let m = AgentManifest::from_yaml_str(&bad).unwrap();
        assert!(m.check_locks().is_err());
    }
}
