//! Bounded real same-n prompt executions. No retry, fallback, scoring, promotion or graph writes.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::{
    model_registry::load_model_registry, openai_compatible_llm::OpenAiCompatibleLlmClient,
    openai_multimodal_prompt::OpenAiMultimodalPromptAdapter,
    prompt_execution::TextPromptExecutionAdapter, prompt_task_registry::load_prompt_task_registry,
};
use da_application::prompt_evaluation::FrozenPromptFixture;
use da_domain::prompt_optimization::{PromptExecutionLineage, PromptRoutePolicy};
use da_ports::prompt_execution::{
    PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptMedia,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "governor/fixtures/prompt_eval/fixtures.yaml")]
    fixtures: PathBuf,
    #[arg(long, default_value = "data/ontology/prompt_tasks.yaml")]
    prompt_registry: PathBuf,
    #[arg(long, default_value = "models.yaml")]
    model_registry: PathBuf,
    #[arg(long, default_value = "governor/fixtures/prompt_eval/inference.yaml")]
    inference_policy: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/prompt_eval_inference_m387.json"
    )]
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
#[serde(deny_unknown_fields)]
struct InferencePolicy {
    schema_version: String,
    maximum_calls: usize,
    request_timeout_seconds: u64,
    variants: Vec<VariantPolicy>,
    tasks: Vec<TaskPolicy>,
    retry_enabled: bool,
    fallback_enabled: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantPolicy {
    id: String,
    system_instruction_suffix: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskPolicy {
    task_id: String,
    route_policy_id: String,
    model_binding_id: String,
    supports_images: bool,
    endpoint_override_env: String,
    credential_env: String,
    maximum_temperature_milli: u16,
}
#[derive(Clone, Serialize)]
struct Execution {
    fixture_id: String,
    variant: String,
    output: serde_json::Value,
    lineage: PromptExecutionLineage,
    authoritative: bool,
    eligible: bool,
    deployed: bool,
    import_eligible: bool,
}
#[derive(Clone, Serialize)]
struct Failure {
    fixture_id: String,
    variant: String,
    error_class: String,
    diagnostic: String,
}
#[derive(Serialize)]
struct Report {
    schema_version: String,
    authorized_models: BTreeMap<String, String>,
    maximum_calls: usize,
    attempted_calls: usize,
    completed_calls: usize,
    same_model: bool,
    retry_enabled: bool,
    fallback_enabled: bool,
    scoring_status: String,
    executions: Vec<Execution>,
    failures: Vec<Failure>,
    optimizer_enabled: bool,
    auto_promote: bool,
    import_eligible: bool,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let _guard = da_cli::logging::init_pipeline_logging("prompt_eval_inference");
    let args = Args::parse();
    let set: FixtureSet = serde_yaml::from_slice(&std::fs::read(&args.fixtures)?)?;
    let policy: InferencePolicy = serde_yaml::from_slice(&std::fs::read(&args.inference_policy)?)?;
    validate_policy(&policy, &set)?;
    let prompts = load_prompt_task_registry(&args.prompt_registry).map_err(anyhow::Error::msg)?;
    let models = load_model_registry(&args.model_registry).map_err(anyhow::Error::msg)?;
    let first_policy = policy
        .tasks
        .first()
        .ok_or_else(|| anyhow!("inference task policy empty"))?;
    let base = std::env::var(&first_policy.endpoint_override_env)
        .map_err(|_| anyhow!("{} is required", first_policy.endpoint_override_env))?;
    let key = std::env::var(&first_policy.credential_env).ok();
    let timeout = policy.request_timeout_seconds;
    let mut executions = vec![];
    let mut failures = vec![];
    let mut attempted_calls = 0;
    let mut authorized_models = BTreeMap::new();
    for tp in &policy.tasks {
        let model = models
            .resolve(&tp.model_binding_id, Some(&base))
            .map_err(anyhow::Error::msg)?;
        authorized_models.insert(tp.task_id.clone(), model.model_name.clone());
    }
    for fixture in &set.fixtures {
        let task_policy = policy
            .tasks
            .iter()
            .find(|p| p.task_id == fixture.task_id)
            .ok_or_else(|| anyhow!("no inference policy for task {}", fixture.task_id))?;
        for variant in &policy.variants {
            let mut task = prompts
                .task(&fixture.task_id)
                .map_err(anyhow::Error::msg)?
                .clone();
            task.version = format!("{}-{}-eval-v1", task.version, variant.id);
            task.system_template
                .push_str(&variant.system_instruction_suffix);
            task.route_policy_id = task_policy.route_policy_id.clone();
            if task_policy.endpoint_override_env != first_policy.endpoint_override_env
                || task_policy.credential_env != first_policy.credential_env
            {
                return Err(anyhow!(
                    "all bounded inference tasks must use one proxy env contract"
                ));
            }
            let route = PromptRoutePolicy {
                route_policy_id: task_policy.route_policy_id.clone(),
                model_binding_id: task_policy.model_binding_id.clone(),
                endpoint_override_env: Some(task_policy.endpoint_override_env.clone()),
                credential_env: task_policy.credential_env.clone(),
                supports_images: task_policy.supports_images,
                maximum_temperature_milli: task_policy.maximum_temperature_milli,
            };
            let model = models
                .resolve(&task_policy.model_binding_id, Some(&base))
                .map_err(anyhow::Error::msg)?;
            let supports_images = task_policy.supports_images;
            let schema = prompts
                .output_schema(&task.output_schema_id)
                .map_err(anyhow::Error::msg)?
                .clone();
            let (variables, media) = request_inputs(fixture, supports_images)?;
            let request = PromptExecutionRequest {
                run_id: format!("m387:{}:{}", fixture.fixture_id, variant.id),
                task,
                route,
                resolved_model: model.clone(),
                output_schema: schema,
                variables,
                input_bytes: fixture.source_text.as_bytes().to_vec(),
                media,
            };
            attempted_calls += 1;
            let response = if supports_images {
                OpenAiMultimodalPromptAdapter::new(key.clone(), timeout)
                    .map_err(anyhow::Error::msg)?
                    .execute(request)
                    .await
            } else {
                let client = OpenAiCompatibleLlmClient::new(model, key.clone(), timeout)
                    .map_err(anyhow::Error::msg)?;
                TextPromptExecutionAdapter::new(Arc::new(client))
                    .execute(request)
                    .await
            };
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    let is_quality_signal = matches!(error, PromptExecutionError::Schema(_));
                    failures.push(Failure {
                        fixture_id: fixture.fixture_id.clone(),
                        variant: variant.id.clone(),
                        error_class: prompt_error_class(&error).into(),
                        diagnostic: error.to_string(),
                    });
                    persist(
                        &args.report,
                        &executions,
                        &failures,
                        attempted_calls,
                        &policy,
                        &authorized_models,
                    )?;
                    if is_quality_signal {
                        tracing::warn!(event="prompt_eval_quality_signal",fixture=%fixture.fixture_id,variant=%variant.id,class=%prompt_error_class(&error),"Schema violation recorded as quality signal, continuing matrix");
                        continue;
                    }
                    return Err(anyhow!(
                        "bounded inference stopped at {}/{}: {}",
                        fixture.fixture_id,
                        variant.id,
                        error
                    ));
                }
            };
            let output: serde_json::Value = serde_json::from_slice(&response.output_bytes)?;
            executions.push(Execution {
                fixture_id: fixture.fixture_id.clone(),
                variant: variant.id.clone(),
                output,
                lineage: response.lineage,
                authoritative: false,
                eligible: false,
                deployed: false,
                import_eligible: false,
            });
            persist(
                &args.report,
                &executions,
                &failures,
                attempted_calls,
                &policy,
                &authorized_models,
            )?;
        }
    }
    tracing::info!(event="prompt_eval_inference_complete",completed_calls=executions.len(),maximum_calls=policy.maximum_calls,models=?authorized_models,retry_enabled=policy.retry_enabled,fallback_enabled=policy.fallback_enabled,optimizer_enabled=policy.optimizer_enabled,auto_promote=policy.auto_promote,import_eligible=false,"Bounded prompt evaluation inference completed");
    Ok(())
}
fn request_inputs(
    f: &FrozenPromptFixture,
    image: bool,
) -> Result<(BTreeMap<String, String>, Vec<PromptMedia>)> {
    if image {
        let a = f
            .supporting_artifacts
            .first()
            .ok_or_else(|| anyhow!("figure fixture lacks image"))?;
        let bytes = std::fs::read(&a.artifact_ref)?;
        let media_type = match Path::new(&a.artifact_ref)
            .extension()
            .and_then(|x| x.to_str())
        {
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            _ => return Err(anyhow!("unsupported figure extension")),
        };
        Ok((
            BTreeMap::from([("grounded_context".into(), f.source_text.clone())]),
            vec![PromptMedia {
                media_type: media_type.into(),
                bytes,
                sha256: a.artifact_sha256.as_str().into(),
            }],
        ))
    } else {
        Ok((
            BTreeMap::from([
                ("paper_id".into(), f.fixture_id.clone()),
                (
                    "title".into(),
                    "Frozen source-bound evaluation excerpt".into(),
                ),
                ("abstract".into(), f.source_text.clone()),
                ("full_text".into(), f.source_text.clone()),
            ]),
            vec![],
        ))
    }
}
fn validate_policy(p: &InferencePolicy, s: &FixtureSet) -> Result<()> {
    if p.schema_version != "prompt-eval-inference-policy.v1"
        || s.schema_version != "prompt-eval-fixtures.v1"
        || s.review_status.trim().is_empty()
        || s.reviewed_by.trim().is_empty()
        || s.production_acceptance_eligible
        || p.maximum_calls == 0
        || p.request_timeout_seconds == 0
        || p.maximum_calls != s.fixtures.len() * p.variants.len()
        || p.variants.len() != 2
        || p.variants[0].id == p.variants[1].id
        || p.tasks.is_empty()
        || p.retry_enabled
        || p.fallback_enabled
        || p.optimizer_enabled
        || p.auto_promote
    {
        return Err(anyhow!(
            "inference policy/fixture contract invalid or unsafe"
        ));
    }
    Ok(())
}
fn prompt_error_class(error: &da_ports::prompt_execution::PromptExecutionError) -> &'static str {
    use da_ports::prompt_execution::PromptExecutionError::*;
    match error {
        Contract(_) => "contract",
        Render(_) => "render",
        Provider(_) => "provider",
        Unavailable(_) => "unavailable",
        Timeout { .. } => "timeout",
        RateLimited { .. } => "rate_limited",
        Schema(_) => "schema",
    }
}
fn persist(
    path: &Path,
    executions: &[Execution],
    failures: &[Failure],
    attempted_calls: usize,
    p: &InferencePolicy,
    models: &BTreeMap<String, String>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let r = Report {
        schema_version: "prompt-eval-inference.v1".into(),
        authorized_models: models.clone(),
        maximum_calls: p.maximum_calls,
        attempted_calls,
        completed_calls: executions.len(),
        same_model: true,
        retry_enabled: p.retry_enabled,
        fallback_enabled: p.fallback_enabled,
        scoring_status: "pending-independent-review".into(),
        executions: executions.to_vec(),
        failures: failures.to_vec(),
        optimizer_enabled: p.optimizer_enabled,
        auto_promote: p.auto_promote,
        import_eligible: false,
    };
    std::fs::write(path, serde_json::to_vec_pretty(&r)?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_policy_is_bounded_and_unsafe_unlocks_fail_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let set: FixtureSet = serde_yaml::from_slice(
            &std::fs::read(root.join("governor/fixtures/prompt_eval/fixtures.yaml")).unwrap(),
        )
        .unwrap();
        let mut policy: InferencePolicy = serde_yaml::from_slice(
            &std::fs::read(root.join("governor/fixtures/prompt_eval/inference.yaml")).unwrap(),
        )
        .unwrap();
        assert!(validate_policy(&policy, &set).is_ok());
        policy.retry_enabled = true;
        assert!(validate_policy(&policy, &set).is_err());
        policy.retry_enabled = false;
        policy.optimizer_enabled = true;
        assert!(validate_policy(&policy, &set).is_err());
    }
}
