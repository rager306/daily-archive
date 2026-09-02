//! Offline ruVLLM-boundary same-n ablation emitter (M464/S17/T05).
//! This example uses redacted, frozen observations; a production adapter may
//! replace the observation source with ruVLLM receipts without changing the
//! application aggregation contract. It never writes graph or route state.

use clap::Parser;
use da_application::{CriticObservation, CriticStatus, FrozenEvalCase, aggregate_same_n_ablation};
use da_ports::{
    CandidateManifest, CandidateReceipt, LearningLineage, PromotionState, Sha256Digest,
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Emit a deterministic redacted same-n parse-learning ablation report")]
struct Args {
    #[arg(long, default_value = "artifacts/parse_learning/ablation.json")]
    report: PathBuf,
}

fn digest(c: char) -> Sha256Digest {
    Sha256Digest::new(c.to_string().repeat(64)).expect("fixed digest")
}

fn candidate() -> CandidateManifest {
    CandidateManifest {
        receipt: CandidateReceipt {
            schema_version: 1,
            candidate_id: digest('a'),
            lineage: LearningLineage {
                source: digest('b'),
                corpus: digest('c'),
                config: digest('d'),
                prior_policy: digest('e'),
                conservation: digest('f'),
                split: digest('1'),
            },
            prior_candidate: Some(digest('9')),
            conservation_ok: true,
            import_eligible: false,
            promotion_state: PromotionState::Candidate,
            deployed: false,
        },
        rank: 1,
        hidden_dim: 8,
        train_count: 2,
        validation_count: 1,
        train_quality: 0.8,
        validation_quality: 0.7,
        validation_gap: 0.1,
        weights_digest: digest('2'),
    }
}

fn cases() -> Vec<FrozenEvalCase> {
    (0..3)
        .map(|order| FrozenEvalCase {
            case_id: format!("frozen-case-{order}"),
            corpus: digest('c'),
            order,
            expected_digest: digest('e'),
        })
        .collect()
}

fn observations(producer: &str) -> Vec<CriticObservation> {
    (0..3)
        .map(|order| CriticObservation {
            schema_version: 1,
            case_id: format!("frozen-case-{order}"),
            corpus: digest('c'),
            order,
            producer: producer.to_owned(),
            metric_one: 0.8,
            metric_two: 0.7,
            status: CriticStatus::Ok,
        })
        .collect()
}

fn main() -> anyhow::Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("parse_learning_ablation");
    let args = Args::parse();
    let report = aggregate_same_n_ablation(
        &candidate(),
        &cases(),
        &observations("ruvllm-critic-a"),
        &observations("ruvllm-critic-b"),
        true,
        "sona-trainer",
    )?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = args.report.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &args.report)?;
    println!(
        "same-n ablation: baseline_n={} candidate_n={} failures={} digest={} report={}",
        report.baseline_n,
        report.candidate_n,
        report.critic_failures,
        report.report_digest.as_str(),
        args.report.display()
    );
    Ok(())
}
