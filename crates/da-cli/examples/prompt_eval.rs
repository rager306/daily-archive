//! Frozen offline prompt evaluation. It never calls a model or promotes a candidate.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::prompt_task_registry::load_prompt_task_registry;
use da_application::prompt_evaluation::{
    FrozenArtifactBinding, FrozenPromptFixture, PromptEvalPolicy, TaskEvaluationReport,
    evaluate_task,
};
use da_domain::{
    assurance::ContentDigest,
    prompt_optimization::{PromptCandidate, PromptCandidateState},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "governor/fixtures/prompt_eval/fixtures.yaml")]
    fixtures: PathBuf,
    #[arg(long, default_value = "governor/fixtures/prompt_eval/policy.yaml")]
    policy: PathBuf,
    #[arg(long, default_value = "data/ontology/prompt_tasks.yaml")]
    prompt_registry: PathBuf,
    #[arg(long, default_value = "artifacts/runtime/prompt_eval_m387.json")]
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
#[derive(Serialize)]
struct CandidateEvaluation {
    candidate: PromptCandidate,
    report: TaskEvaluationReport,
}
#[derive(Serialize)]
struct Report {
    schema_version: String,
    fixture_sha256: ContentDigest,
    policy_sha256: ContentDigest,
    prompt_registry_sha256: ContentDigest,
    review_status: String,
    reviewed_by: String,
    production_acceptance_eligible: bool,
    evaluations: Vec<CandidateEvaluation>,
    model_inference_requested: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
    optimizer_admission_verdict: Option<String>,
    import_eligible: bool,
    report_sha256: String,
}
fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("prompt_eval");
    let args = Args::parse();
    let fixture_bytes = std::fs::read(&args.fixtures)?;
    let fixtures: FixtureSet = serde_yaml::from_slice(&fixture_bytes)?;
    if fixtures.schema_version != "prompt-eval-fixtures.v1"
        || fixtures.review_status.trim().is_empty()
        || fixtures.reviewed_by.trim().is_empty()
        || fixtures.fixtures.is_empty()
        || fixtures.production_acceptance_eligible
    {
        return Err(anyhow!(
            "fixture set incomplete or incorrectly acceptance-eligible"
        ));
    }
    validate_fixture_artifacts(&fixtures.fixtures)?;
    let policy_bytes = std::fs::read(&args.policy)?;
    let policy: PromptEvalPolicy = serde_yaml::from_slice(&policy_bytes)?;
    policy.validate().map_err(anyhow::Error::msg)?;
    let registry_bytes = std::fs::read(&args.prompt_registry)?;
    let registry = load_prompt_task_registry(&args.prompt_registry).map_err(anyhow::Error::msg)?;
    let mut evaluations = vec![];
    for task in &registry.tasks {
        let evaluation =
            evaluate_task(task, &fixtures.fixtures, &policy).map_err(anyhow::Error::msg)?;
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
        let candidate = baseline
            .propose(ContentDigest::of_text(&format!(
                "{}:offline-candidate",
                task.task_id
            )))
            .map_err(anyhow::Error::msg)?
            .evaluate(
                evaluation.evaluation.clone(),
                policy.maximum_validation_gap_basis_points,
            )
            .map_err(anyhow::Error::msg)?;
        assert_eq!(candidate.state, PromptCandidateState::Evaluated);
        evaluations.push(CandidateEvaluation {
            candidate,
            report: evaluation,
        });
    }
    let mut report = Report {
        schema_version: "prompt-eval-report.v1".into(),
        fixture_sha256: ContentDigest::of_bytes(&fixture_bytes),
        policy_sha256: ContentDigest::of_bytes(&policy_bytes),
        prompt_registry_sha256: ContentDigest::of_bytes(&registry_bytes),
        review_status: fixtures.review_status,
        reviewed_by: fixtures.reviewed_by,
        production_acceptance_eligible: false,
        evaluations,
        model_inference_requested: false,
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
    tracing::info!(event="prompt_eval_complete",task_count=report.evaluations.len(),review_status=%report.review_status,production_acceptance_eligible=false,model_inference_requested=false,optimizer_enabled=false,auto_promote=false,import_eligible=false,report=%args.report.display(),"Frozen offline prompt evaluation completed");
    println!(
        "prompt-eval tasks={} state=evaluated acceptance=false inference=false optimizer=false D127=false report={}",
        report.evaluations.len(),
        args.report.display()
    );
    Ok(())
}
fn validate_fixture_artifacts(fixtures: &[FrozenPromptFixture]) -> Result<()> {
    for fixture in fixtures {
        validate_source_artifact(
            &fixture.source_artifact_ref,
            &fixture.source_artifact_sha256,
            &fixture.source_text,
        )?;
        validate_artifact(&fixture.oracle_review_ref, &fixture.oracle_review_sha256)?;
        for artifact in &fixture.supporting_artifacts {
            validate_supporting_artifact(artifact)?;
        }
    }
    Ok(())
}
fn validate_source_artifact(path: &str, expected: &ContentDigest, source_text: &str) -> Result<()> {
    let bytes = validate_artifact(path, expected)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| anyhow!("fixture source artifact must be UTF-8 text"))?;
    if !text.contains(source_text) {
        return Err(anyhow!("fixture source text is not contained in {}", path));
    }
    Ok(())
}
fn validate_supporting_artifact(artifact: &FrozenArtifactBinding) -> Result<()> {
    validate_artifact(&artifact.artifact_ref, &artifact.artifact_sha256).map(|_| ())
}
fn validate_artifact(path: &str, expected: &ContentDigest) -> Result<Vec<u8>> {
    let path = std::path::Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(anyhow!(
            "fixture artifact path must be repo-relative and normalized"
        ));
    }
    let bytes = std::fs::read(path)?;
    let observed = ContentDigest::of_bytes(&bytes);
    if &observed != expected {
        return Err(anyhow!(
            "fixture artifact digest mismatch for {}",
            path.display()
        ));
    }
    Ok(bytes)
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
    #[test]
    fn artifact_digest_mismatch_fails_closed() {
        assert!(validate_artifact("Cargo.toml", &ContentDigest::of_text("wrong")).is_err());
    }
    #[test]
    fn artifact_path_escape_fails_closed_before_io() {
        assert!(validate_artifact("../Cargo.toml", &ContentDigest::of_text("irrelevant")).is_err());
    }
    #[test]
    fn source_text_must_be_contained_in_bound_artifact() {
        let bytes = std::fs::read("Cargo.toml").unwrap();
        assert!(
            validate_source_artifact(
                "Cargo.toml",
                &ContentDigest::of_bytes(&bytes),
                "not-present-in-workspace-manifest"
            )
            .is_err()
        );
    }
}
