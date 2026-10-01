//! Closed train → persist → reload → independent-eval cycle emitter
//! (M514-1lss2s/S07/T03).
//!
//! It reads the frozen document-disjoint corpus and the committed trajectory log
//! the corpus was frozen from, checks that the source still hashes to the digest
//! the corpus records (a drifted source aborts before anything is written),
//! opens a scratch RVF journal (never the canonical
//! `data/agent_memory/summarizer/experience.rvf`), runs the closed learning
//! cycle and writes the candidate-only report through `.tmp` + rename.
//!
//! `--check` re-reads an existing report and validates it against the corpus
//! without retraining; a report that no longer validates exits non-zero and is
//! never overwritten.

use clap::Parser;
use da_adapters::RvfFileStore;
use da_application::{
    FrozenLearningCorpus, LearningCycleReport, learning_lineage_for_corpus, run_learning_cycle,
    validate_learning_cycle_report,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The only corpus this example reads.
const CORPUS_PATH: &str = "artifacts/parse_learning/learning_corpus.json";
/// The committed trajectory log the corpus was frozen from.
const SOURCE_PATH: &str = "data/agent_memory/summarizer/trajectory.jsonl";
/// The only report this example writes.
const REPORT_PATH: &str = "artifacts/parse_learning/candidate_cycle.json";

#[derive(Debug, Parser)]
#[command(about = "Run the closed parse-policy learning cycle on the frozen corpus")]
struct Args {
    #[arg(long, default_value = CORPUS_PATH)]
    corpus: PathBuf,
    #[arg(long, default_value = SOURCE_PATH)]
    source: PathBuf,
    #[arg(long, default_value = REPORT_PATH)]
    report: PathBuf,
    /// Scratch RVF journal. Defaults to a per-process temp file; the canonical
    /// `data/agent_memory/summarizer/experience.rvf` is never opened.
    #[arg(long)]
    store: Option<PathBuf>,
    /// Re-read and validate the existing report instead of retraining.
    #[arg(long)]
    check: bool,
}

fn load_corpus(path: &Path) -> anyhow::Result<FrozenLearningCorpus> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// A drifted source must abort the run instead of producing a report whose
/// lineage no longer binds to the bytes on disk.
fn verify_source(corpus: &FrozenLearningCorpus, source: &Path) -> anyhow::Result<()> {
    let bytes = std::fs::read(source)?;
    let digest = hex::encode(Sha256::digest(&bytes));
    anyhow::ensure!(
        digest == corpus.source_sha256.as_str(),
        "source drift: {} hashes to {digest} but the corpus records {}",
        source.display(),
        corpus.source_sha256.as_str()
    );
    Ok(())
}

fn write_report(report: &LearningCycleReport, path: &Path) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("parse_learning_cycle");
    let args = Args::parse();

    let corpus = load_corpus(&args.corpus)?;
    corpus.validate()?;
    verify_source(&corpus, &args.source)?;

    if args.check {
        let bytes = std::fs::read(&args.report)?;
        let report: LearningCycleReport = serde_json::from_slice(&bytes)?;
        validate_learning_cycle_report(&report, &corpus)?;
        println!(
            "cycle check: outcome={:?} baseline_n={} candidate_n={} digest={} report={}",
            report.outcome,
            report.baseline_n,
            report.candidate_n,
            report.report_digest.as_str(),
            args.report.display()
        );
        return Ok(());
    }

    let store_path = args.store.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "da-parse-learning-cycle-{}.rvf",
            std::process::id()
        ))
    });
    let candidate_path = store_path.with_extension("candidate.json");
    let lineage = learning_lineage_for_corpus(&corpus);
    let mut store = RvfFileStore::open(store_path.clone(), lineage.clone())?;
    let report = run_learning_cycle(&corpus, &mut store, lineage, &candidate_path)?;
    write_report(&report, &args.report)?;

    println!(
        "cycle report: outcome={:?} baseline_n={} candidate_n={} producer_independent={:?} contamination={:?} digest={} report={}",
        report.outcome,
        report.baseline_n,
        report.candidate_n,
        report.producer_independent,
        report.contamination_assessment,
        report.report_digest.as_str(),
        args.report.display()
    );
    Ok(())
}
