//! Strict referentially-complete DATA registry for generative prompt tasks (D238).

use da_domain::{
    assurance::ContentDigest,
    prompt_optimization::{PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptTaskRegistry {
    pub schema_version: String,
    pub optimizer_enabled: bool,
    pub max_validation_gap_basis_points: u32,
    pub routes: Vec<PromptRoutePolicy>,
    pub output_schemas: Vec<PromptOutputSchema>,
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
        let routes = self
            .routes
            .iter()
            .map(|x| (x.route_policy_id.as_str(), x))
            .collect::<BTreeMap<_, _>>();
        let schemas = self
            .output_schemas
            .iter()
            .map(|x| (x.output_schema_id.as_str(), x))
            .collect::<BTreeMap<_, _>>();
        if routes.len() != self.routes.len() || schemas.len() != self.output_schemas.len() {
            return Err("duplicate prompt route/schema identity".into());
        }
        for route in &self.routes {
            route.validate()?;
            if let Some(key) = &route.endpoint_override_env {
                validate_env_key(key)?;
            }
            validate_env_key(&route.credential_env)?;
        }
        for schema in &self.output_schemas {
            schema.validate()?;
            jsonschema::validator_for(&schema.schema)
                .map_err(|e| format!("invalid JSON schema {}: {e}", schema.output_schema_id))?;
        }
        let mut ids = BTreeSet::new();
        for task in &self.tasks {
            task.validate()?;
            if !ids.insert(task.task_id.as_str()) {
                return Err(format!("duplicate prompt task {}", task.task_id));
            }
            let route = routes.get(task.route_policy_id.as_str()).ok_or_else(|| {
                format!(
                    "task {} route {} unresolved",
                    task.task_id, task.route_policy_id
                )
            })?;
            if task.temperature_milli > route.maximum_temperature_milli {
                return Err(format!(
                    "task {} temperature exceeds route policy",
                    task.task_id
                ));
            }
            if !schemas.contains_key(task.output_schema_id.as_str()) {
                return Err(format!(
                    "task {} schema {} unresolved",
                    task.task_id, task.output_schema_id
                ));
            }
            validate_template(&task.system_template)?;
            validate_template(&task.user_template)?;
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
    pub fn route(&self, id: &str) -> Result<&PromptRoutePolicy, String> {
        self.routes
            .iter()
            .find(|x| x.route_policy_id == id)
            .ok_or_else(|| format!("prompt route {id} not registered"))
    }
    pub fn output_schema(&self, id: &str) -> Result<&PromptOutputSchema, String> {
        self.output_schemas
            .iter()
            .find(|x| x.output_schema_id == id)
            .ok_or_else(|| format!("prompt schema {id} not registered"))
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
fn validate_env_key(value: &str) -> Result<(), String> {
    let mut chars = value.chars();
    if !matches!(chars.next(), Some('A'..='Z'))
        || !chars.all(|x| x.is_ascii_uppercase() || x.is_ascii_digit() || x == '_')
    {
        return Err(format!("invalid environment key {value}"));
    }
    Ok(())
}
fn validate_template(value: &str) -> Result<(), String> {
    let mut depth = 0;
    for c in value.chars() {
        match c {
            '{' => {
                if depth != 0 {
                    return Err("nested template placeholder".into());
                }
                depth = 1;
            }
            '}' => {
                if depth != 1 {
                    return Err("unmatched template brace".into());
                }
                depth = 0;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unclosed template placeholder".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_registry_is_referentially_complete_disabled_and_digest_bound() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology/prompt_tasks.yaml");
        let r = load_prompt_task_registry(&p).unwrap();
        assert_eq!(r.tasks.len(), 2);
        assert!(!r.optimizer_enabled);
        let task = r.task("article_summary").unwrap();
        assert!(r.route(&task.route_policy_id).is_ok());
        assert!(r.output_schema(&task.output_schema_id).is_ok());
        assert_eq!(r.sha256().unwrap(), r.sha256().unwrap());
    }
    #[test]
    fn unknown_field_and_enabled_optimizer_fail() {
        let p = std::env::temp_dir().join(format!("prompt-registry-{}.yaml", std::process::id()));
        std::fs::write(&p,"schema_version: prompt-task-registry.v1\noptimizer_enabled: true\nmax_validation_gap_basis_points: 500\nroutes: []\noutput_schemas: []\ntasks: []\nunknown: true\n").unwrap();
        assert!(load_prompt_task_registry(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
