//! Consumer of the standalone ruVLLM learning-critic producer
//! (`tools/learning-critic-ruvllm`, D471 `learning.ruvllm_ablation`,
//! ADR-075 §2.7; Э1 of the integration plan).
//!
//! The producer binary runs OUTSIDE this workspace on purpose: the ruvllm
//! stack links `hnsw_rs` 0.3 while the samyama chain links 0.2, and the two
//! must never share one link unit. This example ingests the producer's
//! `parse-learning-ablation.v1` observations JSON, folds the rows through the
//! untouched same-n ablation contract and re-states the invariants:
//! `exact_same_n`, `producer_independence`, `critic_failures` in the
//! denominator. Nothing is promoted, imported, deployed or written to a graph.

use clap::Parser;
use da_application::{
    CriticObservation, CriticStatus, FrozenEvalCase, ProducerProvenance,
    aggregate_same_n_ablation_with_provenance,
};
use da_ports::{
    CandidateManifest, CandidateReceipt, LearningLineage, PromotionState, Sha256Digest,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Ingest ruVLLM critic observations and emit the same-n ablation report")]
struct Args {
    /// Producer JSON emitted by tools/learning-critic-ruvllm.
    #[arg(long, default_value = "/tmp/grobid_audit/e1_observations.json")]
    observations: PathBuf,
}

const E2E_BASELINE_CODE: &str = "e2e-baseline-producer.recorded-quality.v1";
const E2E_TRAINER_CODE: &str = "e2e-trainer-producer.microlora-train.v1";
const E2E_BASELINE_LABEL: &str = "e2e-frozen-recorded-reference";
const E2E_BASELINE_METRIC_ONE: f64 = 0.8;
const E2E_BASELINE_METRIC_TWO: f64 = 0.7;

fn identity_digest(value: &str) -> Sha256Digest {
    let hex = hex::encode(Sha256::digest(value.as_bytes()));
    Sha256Digest::new(hex).expect("sha256 hex is always a valid digest")
}

/// The frozen ordered case set mirrors the producer's embedded cases; the
/// expected digests are demo-level bindings (case id + corpus), not content
/// hashes — a production arm binds the frozen TEI reference here.
fn frozen_cases(rows: &[CriticObservation]) -> Vec<FrozenEvalCase> {
    rows.iter()
        .map(|row| FrozenEvalCase {
            case_id: row.case_id.clone(),
            corpus: row.corpus.clone(),
            order: row.order,
            expected_digest: identity_digest(&format!("{}:{:?}", row.case_id, row.corpus)),
        })
        .collect()
}

fn candidate_manifest(corpus: &Sha256Digest) -> CandidateManifest {
    CandidateManifest {
        receipt: CandidateReceipt {
            schema_version: 1,
            candidate_id: identity_digest("learning-critic-e2e.candidate.v1"),
            lineage: LearningLineage {
                source: identity_digest("learning-critic-e2e.source.v1"),
                corpus: corpus.clone(),
                config: identity_digest("learning-critic-e2e.config.v1"),
                prior_policy: identity_digest("learning-critic-e2e.prior-policy.v1"),
                conservation: identity_digest("learning-critic-e2e.conservation.v1"),
                split: identity_digest("learning-critic-e2e.split.v1"),
            },
            prior_candidate: None,
            conservation_ok: true,
            import_eligible: false,
            promotion_state: PromotionState::Candidate,
            deployed: false,
        },
        rank: 1,
        hidden_dim: 8,
        train_count: 2,
        validation_count: 2,
        train_quality: 0.8,
        validation_quality: 0.7,
        validation_gap: 0.1,
        weights_digest: identity_digest("learning-critic-e2e.weights.v1"),
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let _logging_guard = da_cli::logging::init_pipeline_logging("learning_critic_e2e");
    let raw = std::fs::read_to_string(&args.observations)?;
    let parsed = serde_json::from_str::<serde_json::Value>(&raw)?;
    let corpus_str = parsed["corpus"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("producer JSON missing corpus"))?
        .to_string();
    let corpus = Sha256Digest::new(corpus_str.clone())?;
    let candidate_rows: Vec<CriticObservation> =
        serde_json::from_value(parsed["observations"].clone())?;
    let candidate_producer: ProducerProvenance =
        serde_json::from_value(parsed["provenance"].clone())?;
    let n = candidate_rows.len() as u64;

    let frozen = frozen_cases(&candidate_rows);
    let baseline_rows: Vec<CriticObservation> = frozen
        .iter()
        .map(|case| CriticObservation {
            schema_version: 1,
            case_id: case.case_id.clone(),
            corpus: case.corpus.clone(),
            order: case.order,
            producer: E2E_BASELINE_LABEL.into(),
            metric_one: E2E_BASELINE_METRIC_ONE,
            metric_two: E2E_BASELINE_METRIC_TWO,
            status: CriticStatus::Ok,
        })
        .collect();
    let baseline_producer = ProducerProvenance {
        code_digest: identity_digest(E2E_BASELINE_CODE),
        input_digest: corpus.clone(),
        method: "frozen_recorded_reference".into(),
        authority: "learning-critic-e2e".into(),
    };
    let trainer_producer = ProducerProvenance {
        code_digest: identity_digest(E2E_TRAINER_CODE),
        input_digest: corpus.clone(),
        method: "micro_lora_trainer".into(),
        authority: "learning-critic-e2e".into(),
    };

    let report = aggregate_same_n_ablation_with_provenance(
        &candidate_manifest(&corpus),
        &frozen,
        &baseline_rows,
        &candidate_rows,
        true,
        &baseline_producer,
        &candidate_producer,
        &trainer_producer,
    )?;

    println!("=== AblationReport (parse-learning-ablation.v1) ===");
    println!("{}", serde_json::to_string_pretty(&report)?);
    println!("=== contract invariants ===");
    println!(
        "exact_same_n: baseline_n={} candidate_n={} cases_n={} -> {}",
        report.baseline_n,
        report.candidate_n,
        n,
        report.baseline_n == report.candidate_n && report.candidate_n == n
    );
    println!("producer_independence: {}", report.producer_independent);
    println!(
        "critic_failures: {} (kept in the denominator, n={})",
        report.critic_failures, n
    );
    println!(
        "candidate locks: promotion_state={} deployed={} import_eligible={}",
        report.promotion_state, report.deployed, report.import_eligible
    );

    assert_eq!(report.baseline_n, n);
    assert_eq!(report.candidate_n, n);
    assert!(report.producer_independent);
    assert_eq!(report.schema_version, "parse-learning-ablation.v1");
    Ok(())
}
