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
pub struct PromptContract {
    pub schema_version: String,
    pub authority: String,
    pub policy_owner: String,
    pub immutable_proof_boundary: ImmutableProofBoundary,
    pub rules: Vec<PromptRule>,
    pub task_rule_bindings: BTreeMap<String, Vec<String>>,
    pub approved_verification_operations: Vec<VerificationOperation>,
    pub change_control: ChangeControl,
    pub negative_mutation_cases: Vec<NegativeMutationCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImmutableProofBoundary {
    pub clauses: Vec<String>,
    pub optimizer_may_change: Vec<String>,
    pub optimizer_may_not_change: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRule {
    pub rule_id: String,
    pub scope: String,
    pub authority: String,
    pub preconditions: Vec<String>,
    pub evidence_obligations: Vec<String>,
    pub verification_operations: Vec<String>,
    pub optimizer_immutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationOperation {
    pub operation_id: String,
    pub parameters: BTreeMap<String, String>,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeControl {
    pub required_for_normative_change: Vec<String>,
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeMutationCase {
    pub case_id: String,
    pub target_rule_id: String,
    pub mutation: String,
    pub expected: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptTaskRegistry {
    pub schema_version: String,
    pub optimizer_enabled: bool,
    pub max_validation_gap_basis_points: u32,
    pub prompt_contract: PromptContract,
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
        validate_prompt_contract(&self.prompt_contract)?;
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
    /// Resolve a task by identity AND version, fail-closed.
    ///
    /// The unified prompt registry binds `(prompt_id, prompt_version)`; an id
    /// that resolves only at another version must not be accepted, so a wrong
    /// version is refused and the message names the registered version.
    pub fn task_versioned(&self, id: &str, version: &str) -> Result<&PromptTaskSpec, String> {
        let task = self.task(id)?;
        if task.version != version {
            return Err(format!(
                "prompt task {id} is registered at version {} not {version}",
                task.version
            ));
        }
        Ok(task)
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
fn validate_prompt_contract(contract: &PromptContract) -> Result<(), String> {
    if contract.schema_version != "prompt-contract.v1"
        || contract.authority != "ADR-059"
        || contract.policy_owner != "data/ontology/prompt_tasks.yaml"
    {
        return Err("prompt contract authority/schema/owner invalid".into());
    }
    let approved = contract
        .approved_verification_operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<BTreeSet<_>>();
    if approved.len() != contract.approved_verification_operations.len() || approved.is_empty() {
        return Err("prompt verification operation identities invalid".into());
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &contract.rules {
        if rule.authority != "ADR-059"
            || rule.rule_id.trim().is_empty()
            || rule.scope.trim().is_empty()
            || rule.preconditions.is_empty()
            || rule.evidence_obligations.is_empty()
            || !rule.optimizer_immutable
        {
            return Err(format!(
                "normative prompt rule {} is not immutable and evidence-bound",
                rule.rule_id
            ));
        }
        if !rule_ids.insert(rule.rule_id.as_str()) {
            return Err(format!("duplicate normative prompt rule {}", rule.rule_id));
        }
        for operation in &rule.verification_operations {
            if !approved.contains(operation.as_str()) {
                return Err(format!(
                    "rule {} uses unapproved verification operation {}",
                    rule.rule_id, operation
                ));
            }
        }
    }
    for (task_id, bindings) in &contract.task_rule_bindings {
        if bindings.is_empty()
            || bindings
                .iter()
                .any(|rule_id| !rule_ids.contains(rule_id.as_str()))
        {
            return Err(format!(
                "task {} has unresolved prompt rule binding",
                task_id
            ));
        }
    }
    let mut case_ids = BTreeSet::new();
    for case in &contract.negative_mutation_cases {
        if !rule_ids.contains(case.target_rule_id.as_str())
            || case.expected != "reject"
            || !case_ids.insert(case.case_id.as_str())
        {
            return Err(format!(
                "negative prompt mutation case {} is invalid",
                case.case_id
            ));
        }
    }
    if contract.immutable_proof_boundary.clauses.is_empty()
        || contract
            .immutable_proof_boundary
            .optimizer_may_not_change
            .is_empty()
        || contract
            .change_control
            .required_for_normative_change
            .is_empty()
        || contract.change_control.forbidden.is_empty()
    {
        return Err("prompt proof/change-control boundary is incomplete".into());
    }
    Ok(())
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
    fn task_versioned_accepts_registered_version_and_refuses_drift() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology/prompt_tasks.yaml");
        let r = load_prompt_task_registry(&p).unwrap();
        let task = r.task_versioned("article_summary", "summary.v1").unwrap();
        assert_eq!(task.task_id, "article_summary");
        let error = r.task_versioned("article_summary", "v9").unwrap_err();
        assert!(error.contains("summary.v1"), "{error}");
        assert!(r.task_versioned("no_such_prompt", "v1").is_err());
    }

    #[test]
    fn unknown_field_and_enabled_optimizer_fail() {
        let p = std::env::temp_dir().join(format!("prompt-registry-{}.yaml", std::process::id()));
        std::fs::write(&p,"schema_version: prompt-task-registry.v1\noptimizer_enabled: true\nmax_validation_gap_basis_points: 500\nroutes: []\noutput_schemas: []\ntasks: []\nunknown: true\n").unwrap();
        assert!(load_prompt_task_registry(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn normative_prompt_mutations_fail_closed() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology/prompt_tasks.yaml");
        let mut registry = load_prompt_task_registry(&p).unwrap();

        registry.prompt_contract.rules[0]
            .evidence_obligations
            .clear();
        assert!(
            registry.validate().is_err(),
            "removing an evidence obligation must fail"
        );

        let mut registry = load_prompt_task_registry(&p).unwrap();
        registry.prompt_contract.rules[0].authority = "unreviewed-source".into();
        assert!(
            registry.validate().is_err(),
            "downgrading authority must fail"
        );

        let mut registry = load_prompt_task_registry(&p).unwrap();
        registry.prompt_contract.rules[0].verification_operations = vec!["free_form.check".into()];
        assert!(
            registry.validate().is_err(),
            "free-form verification must fail"
        );
    }
}
