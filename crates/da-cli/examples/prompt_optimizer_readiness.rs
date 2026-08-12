//! Emit optimizer-readiness facts without enabling or promoting an optimizer.

use anyhow::Result;
use clap::Parser;
use da_adapters::{
    model_registry::{load_model_registry, validate_prompt_model_bindings},
    prompt_task_registry::load_prompt_task_registry,
};
use da_domain::assurance::{ContentDigest, MissingnessReason};
use serde::Serialize;
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/ontology/prompt_tasks.yaml")]
    prompt_registry: PathBuf,
    #[arg(long, default_value = "models.yaml")]
    model_registry: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/prompt_optimizer_readiness_m387.json"
    )]
    report: PathBuf,
}
#[derive(Serialize)]
struct TaskReadiness {
    task_id: String,
    eval_suite_id: String,
    model_binding_id: String,
    resolved_model_id: String,
    resolved_model_name: String,
    supports_images: bool,
    prompt_spec_sha256: ContentDigest,
    route_sha256: ContentDigest,
    model_binding_sha256: ContentDigest,
    output_schema_sha256: ContentDigest,
    lineage_fields: Vec<&'static str>,
    baseline_n: Option<u64>,
    candidate_n: Option<u64>,
    evaluation_missingness: MissingnessReason,
    fsm_state: &'static str,
    eligible: bool,
    deployed: bool,
}
#[derive(Serialize)]
struct Report {
    schema_version: String,
    prompt_registry_sha256: ContentDigest,
    model_registry_sha256: ContentDigest,
    max_validation_gap_basis_points: u32,
    tasks: Vec<TaskReadiness>,
    required_evaluation_metrics: Vec<&'static str>,
    same_n_required: bool,
    contamination_gate_required: bool,
    budget_gate_required: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
    optimizer_admission_verdict: Option<String>,
    import_eligible: bool,
    report_sha256: String,
}
fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("prompt_optimizer_readiness");
    let args = Args::parse();
    let prompt_bytes = std::fs::read(&args.prompt_registry)?;
    let prompt = load_prompt_task_registry(&args.prompt_registry).map_err(anyhow::Error::msg)?;
    let models = load_model_registry(&args.model_registry).map_err(anyhow::Error::msg)?;
    validate_prompt_model_bindings(&models, &prompt).map_err(anyhow::Error::msg)?;
    let model_bytes = std::fs::read(&args.model_registry)?;
    let mut tasks = vec![];
    for task in &prompt.tasks {
        let route = prompt
            .route(&task.route_policy_id)
            .map_err(anyhow::Error::msg)?;
        let schema = prompt
            .output_schema(&task.output_schema_id)
            .map_err(anyhow::Error::msg)?;
        let model = models
            .resolve(&route.model_binding_id, None)
            .map_err(anyhow::Error::msg)?;
        tasks.push(TaskReadiness {
            task_id: task.task_id.clone(),
            eval_suite_id: task.eval_suite_id.clone(),
            model_binding_id: model.binding_id.clone(),
            resolved_model_id: model.model_id.clone(),
            resolved_model_name: model.model_name.clone(),
            supports_images: route.supports_images,
            prompt_spec_sha256: task.spec_sha256().map_err(anyhow::Error::msg)?,
            route_sha256: ContentDigest::of_bytes(&serde_json::to_vec(route)?),
            model_binding_sha256: model.sha256().map_err(anyhow::Error::msg)?,
            output_schema_sha256: schema.sha256().map_err(anyhow::Error::msg)?,
            lineage_fields: vec![
                "task_spec_sha256",
                "rendered_prompt_sha256",
                "output_schema_sha256",
                "route_policy_sha256",
                "endpoint_sha256",
                "model_binding_sha256",
                "model",
                "input_sha256",
                "output_sha256",
                "input_tokens",
                "output_tokens",
                "latency_ms",
                "temperature_milli",
                "max_output_tokens",
            ],
            baseline_n: None,
            candidate_n: None,
            evaluation_missingness: MissingnessReason::NotMaterialized,
            fsm_state: "baseline_registered",
            eligible: false,
            deployed: false,
        });
    }
    let mut report = Report {
        schema_version: "prompt-optimizer-readiness.v1".into(),
        prompt_registry_sha256: ContentDigest::of_bytes(&prompt_bytes),
        model_registry_sha256: ContentDigest::of_bytes(&model_bytes),
        max_validation_gap_basis_points: prompt.max_validation_gap_basis_points,
        tasks,
        required_evaluation_metrics: vec![
            "schema_validity",
            "grounding",
            "faithfulness",
            "coverage",
            "paraphrase_stability",
            "latency",
            "tokens",
            "estimated_cost",
        ],
        same_n_required: true,
        contamination_gate_required: true,
        budget_gate_required: true,
        optimizer_enabled: false,
        auto_promote: false,
        optimizer_admission_verdict: None,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(event="prompt_optimizer_readiness_complete",task_count=report.tasks.len(),optimizer_enabled=false,auto_promote=false,import_eligible=false,report=%args.report.display(),"Prompt optimizer readiness emitted");
    println!(
        "prompt-readiness tasks={} optimizer=false auto_promote=false D127=false report={}",
        report.tasks.len(),
        args.report.display()
    );
    Ok(())
}
fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}
