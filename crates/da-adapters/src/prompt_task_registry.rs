//! Strict DATA registry for generative prompt tasks (D238).

use da_domain::{assurance::ContentDigest, prompt_optimization::PromptTaskSpec};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptTaskRegistry {
    pub schema_version: String,
    pub optimizer_enabled: bool,
    pub max_validation_gap_basis_points: u32,
    pub tasks: Vec<PromptTaskSpec>,
}
impl PromptTaskRegistry {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "prompt-task-registry.v1"
            || self.optimizer_enabled
            || self.max_validation_gap_basis_points > 10_000
            || self.tasks.is_empty()
        {
            return Err("prompt registry schema/optimizer/gap/tasks invalid".into());
        }
        let mut ids = BTreeSet::new();
        for task in &self.tasks {
            task.validate()?;
            if !ids.insert(task.task_id.as_str()) {
                return Err(format!("duplicate prompt task {}", task.task_id));
            }
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<ContentDigest, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|b| ContentDigest::of_bytes(&b))
            .map_err(|e| e.to_string())
    }
    pub fn task(&self, id: &str) -> Result<&PromptTaskSpec, String> {
        self.tasks
            .iter()
            .find(|x| x.task_id == id)
            .ok_or_else(|| format!("prompt task {id} not registered"))
    }
}
pub fn load_prompt_task_registry(path: &Path) -> Result<PromptTaskRegistry, String> {
    let m = std::fs::metadata(path)
        .map_err(|e| format!("stat prompt registry {}: {e}", path.display()))?;
    if !m.is_file() || m.len() > 1024 * 1024 {
        return Err(format!(
            "prompt registry {} is not a bounded file",
            path.display()
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|e| format!("read prompt registry {}: {e}", path.display()))?;
    let registry: PromptTaskRegistry = serde_yaml::from_slice(&bytes)
        .map_err(|e| format!("parse prompt registry {}: {e}", path.display()))?;
    registry.validate()?;
    Ok(registry)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_registry_is_strict_disabled_and_digest_bound() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology/prompt_tasks.yaml");
        let r = load_prompt_task_registry(&p).unwrap();
        assert_eq!(r.tasks.len(), 2);
        assert!(!r.optimizer_enabled);
        assert!(r.task("article_summary").is_ok());
        assert_eq!(r.sha256().unwrap(), r.sha256().unwrap());
    }
    #[test]
    fn unknown_field_and_enabled_optimizer_fail() {
        let p = std::env::temp_dir().join(format!("prompt-registry-{}.yaml", std::process::id()));
        std::fs::write(&p,"schema_version: prompt-task-registry.v1\noptimizer_enabled: true\nmax_validation_gap_basis_points: 500\ntasks: []\nunknown: true\n").unwrap();
        assert!(load_prompt_task_registry(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
