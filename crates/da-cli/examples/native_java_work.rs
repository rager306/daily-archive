//! S08/T03 native Java-work emitter.
//!
//! Projects the pinned corpus (`data/parse/layout_bakeoff.yaml#corpus`) and the
//! stage tapes captured for it into `artifacts/parse_compat/native_java_work.json`:
//! per-model Java CRF invocation counts, each bound to the tape identity it was
//! derived from.
//!
//! Two arms, one artifact:
//!
//! - **offline** — no `--tape` (or none readable): every document is typed
//!   `not_observed`, the artifact is still written, and the process exits 0.
//!   Absence is never reported as a zero count.
//! - **`--verify`** — each tape is re-measured against its recorded source PDF,
//!   its stored dump and the live release before any count is accepted, and the
//!   receipt records that a verification run happened. A tape verification
//!   cannot cover is counted as a visible gap, never silently downgraded.
//!
//! The raw dump is read for its digest and immediately discarded: it carries
//! token text and must never reach the repository. Tapes live outside the repo.
//!
//! Nothing here is a production route: no graph write, no import, no promotion.

use anyhow::{Context, Result};
use clap::Parser;
use da_cli::native_java_work::{
    DocumentJavaWork, JAVA_WORK_ARTIFACT_REL, JavaWorkAggregate, JavaWorkInputs,
    NativeJavaWorkReport, TapeVerification, load_corpus_from_manifest, parse_tape_json,
    validate_tape_identity,
};
use da_parse::{StageTape, TapeObservation, verify_tape};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");

#[derive(Parser, Debug)]
struct Args {
    /// Stage tape JSON (`grobid_stage_tape` output) for one pinned document.
    /// May be repeated; each tape is matched to a corpus document by digest.
    #[arg(long = "tape", value_name = "PATH")]
    tapes: Vec<PathBuf>,
    /// Artifact path (repo-relative by convention).
    #[arg(long, default_value = JAVA_WORK_ARTIFACT_REL)]
    artifact: PathBuf,
    /// Re-measure every tape before accepting its counts.
    #[arg(long)]
    verify: bool,
}

struct LoadedTape {
    path: PathBuf,
    tape: StageTape,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Hash a file in bounded chunks.
///
/// A debug dump is tens of megabytes, so re-measuring it must not pull the
/// whole artifact into memory: at 10x the corpus the peak would follow the
/// largest dump rather than stay flat.
fn sha256_file_hex(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_tape(path: &Path) -> Result<StageTape> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    parse_tape_json(&bytes).map_err(|error| anyhow::anyhow!("{error}"))
}

async fn probe_release() -> Result<String> {
    let parser = da_cli::parse_profile::corpus_grobid_parser();
    let probe = parser
        .probe_version()
        .await
        .map_err(|failure| anyhow::anyhow!("release probe failed: {failure}"))?;
    let version = probe.version.trim().to_string();
    anyhow::ensure!(
        !version.is_empty(),
        "release probe returned a blank version; refusing to bind a verification to it"
    );
    Ok(version)
}

/// Re-measure one tape. `None` means verification did not happen for this tape,
/// which the report records as a visible gap rather than as a pass.
fn verify_one(loaded: &LoadedTape, release: &str) -> Option<TapeVerification> {
    let tape = &loaded.tape;
    if let Err(error) = validate_tape_identity(tape) {
        eprintln!("--verify skipped an unusable tape: {error}");
        return None;
    }
    let source_sha256 = match sha256_file_hex(Path::new(&tape.source_pdf)) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!(
                "--verify could not re-read the recorded source {}: {error}",
                tape.source_pdf
            );
            return None;
        }
    };
    let dump_path = loaded
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}.debug.txt", tape.endpoint));
    let dump_sha256 = sha256_file_hex(&dump_path).ok();
    let observation = TapeObservation {
        source_sha256,
        dump_sha256,
        probed_backend_version: release.to_string(),
    };
    let findings = verify_tape(tape, &observation).len();
    Some(TapeVerification {
        tape_schema_version: tape.schema_version.clone(),
        dump_sha256: tape.dump_sha256.clone().unwrap_or_default(),
        backend_version: tape.backend_version.clone(),
        findings,
    })
}

fn write_artifact(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create artifact dir {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write artifact {}", path.display()))
}

fn report_stdout(report: &NativeJavaWorkReport, artifact: &Path, bytes: &[u8]) {
    println!(
        "native java work: corpus={} documents={} unit={} verification_requested={} verification_gaps={}",
        report.corpus.id,
        report.corpus.corpus_n,
        report.unit,
        report.verification_requested,
        report.verification_gaps
    );
    for document in &report.documents {
        match document {
            DocumentJavaWork::Receipt {
                document_id,
                receipt,
                ..
            } => match receipt.measured() {
                Some(counts) => println!(
                    "  {document_id}: state={:?} sections={} models={} invocations={} coverage={:?} tape_verification={:?}",
                    receipt.state(),
                    counts.sections,
                    counts.distinct_models,
                    counts.total_invocations,
                    counts.coverage,
                    counts.tape_verification
                ),
                None => println!("  {document_id}: not_observed (no counts carried)"),
            },
            DocumentJavaWork::Refused {
                document_id,
                refusal,
                ..
            } => println!("  {document_id}: refused code={}", refusal.code),
            DocumentJavaWork::NotObserved {
                document_id,
                reason,
                ..
            } => println!("  {document_id}: not_observed reason={reason:?}"),
        }
    }
    match &report.aggregate {
        JavaWorkAggregate::Measured(aggregate) => {
            println!(
                "aggregate: measured={} filtered={} refused={} of {} invocations={} coverage={:?}",
                aggregate.documents_measured,
                aggregate.documents_filtered,
                aggregate.documents_refused,
                aggregate.documents_total,
                aggregate.total_invocations,
                aggregate.coverage
            );
            for entry in &aggregate.models {
                println!("    {} = {}", entry.model, entry.invocations);
            }
        }
        JavaWorkAggregate::NotObserved { reason } => {
            println!("aggregate: not_observed reason={reason:?} (absence is not zero)");
            println!("capture hint: {}", report.capture_hint);
        }
    }
    println!(
        "artifact: {} bytes={} sha256={}",
        artifact.display(),
        bytes.len(),
        sha256_hex(bytes)
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = da_cli::logging::init_pipeline_logging("native_java_work");

    let corpus = load_corpus_from_manifest(MANIFEST)
        .map_err(|error| anyhow::anyhow!("corpus contract: {error}"))?;

    let mut loaded = Vec::new();
    let mut unreadable_tape_count = 0usize;
    for path in &args.tapes {
        match read_tape(path) {
            Ok(tape) => loaded.push(LoadedTape {
                path: path.clone(),
                tape,
            }),
            Err(error) => {
                unreadable_tape_count += 1;
                // The path is logged, never written into the artifact.
                eprintln!("stage tape {} is unusable: {error}", path.display());
            }
        }
    }

    let mut verifications = Vec::new();
    let mut verification_gaps = 0usize;
    if args.verify {
        let release = probe_release().await?;
        for tape in &loaded {
            match verify_one(tape, &release) {
                Some(record) => verifications.push(record),
                None => verification_gaps += 1,
            }
        }
    }

    let tapes = loaded
        .iter()
        .map(|entry| entry.tape.clone())
        .collect::<Vec<_>>();
    let inputs = JavaWorkInputs {
        tapes: &tapes,
        verifications: &verifications,
        unreadable_tape_count,
        verification_requested: args.verify,
        verification_gaps,
    };
    let report = NativeJavaWorkReport::assemble(&corpus, &inputs)?;
    let bytes = report.artifact_json_bytes()?;
    write_artifact(&args.artifact, &bytes)?;
    report_stdout(&report, &args.artifact, &bytes);
    Ok(())
}
