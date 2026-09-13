//! Offline stage-tape recorder and verifier for the Java GROBID cascade
//! (M506/S01/T03-T04).
//!
//! Two modes, one artifact:
//!
//! - **capture** (default) — record the model debug dump for a pinned PDF and
//!   write a typed tape plus the raw dump.
//! - **`--verify`** — re-measure the source PDF, the stored dump and the live
//!   release, then report every way the tape can no longer be trusted:
//!   source substitution, dump substitution or absence, release drift, a
//!   recorded failure, or content that carried no model section at all.
//!
//! Why verification is part of the tool rather than a reader's job: a tape is
//! only evidence while it is bound to the exact input, bytes and release it was
//! recorded against. Those bindings decay silently — a PDF gets replaced, a
//! dump gets edited, the service gets upgraded — and an unverified tape would
//! keep looking valid.
//!
//! Raw TEI is deliberately NOT captured here: the compatibility tooling already
//! owns that, and a second source of the same truth would drift.
//!
//! This is offline evidence, not a production route: no graph write, no import,
//! no promotion. `debug_capture` is intentionally outside `ParserPort`.

use anyhow::{Context, Result};
use clap::Parser;
use da_parse::{
    CaptureState, ParseBackendRequest, ParseEndpoint, StageTape, TapeObservation, verify_tape,
};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
struct Args {
    /// Pinned PDF to record against (capture mode).
    #[arg(long)]
    pdf: Option<PathBuf>,
    /// Output directory for the tape (capture mode).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Endpoint whose model dump is recorded.
    #[arg(long, default_value = "fulltext")]
    endpoint: String,
    /// Optional `models` filter (comma-separated). Applying it makes the tape a
    /// filtered subset rather than a full dump, which is recorded explicitly.
    #[arg(long)]
    models: Option<String>,
    /// Verify an existing tape instead of capturing a new one.
    #[arg(long)]
    verify: Option<PathBuf>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn endpoint_from_arg(value: &str) -> Result<ParseEndpoint> {
    match value {
        "fulltext" => Ok(ParseEndpoint::Fulltext),
        "header" => Ok(ParseEndpoint::Header),
        "references" => Ok(ParseEndpoint::References),
        other => anyhow::bail!("unknown endpoint `{other}` (expected fulltext|header|references)"),
    }
}

fn model_filter(raw: Option<&str>) -> Option<Vec<String>> {
    raw.map(|value| {
        value
            .split(',')
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .collect::<Vec<_>>()
    })
}

/// Probe the live release, failing closed: a tape must never be bound to an
/// unprobed or blank version.
async fn probed_release() -> Result<String> {
    let parser = da_cli::parse_profile::corpus_grobid_parser();
    let probe = parser
        .probe_version()
        .await
        .map_err(|failure| anyhow::anyhow!("GROBID /api/version probe failed: {failure}"))?;
    let version = probe.version.trim().to_string();
    anyhow::ensure!(
        !version.is_empty(),
        "GROBID /api/version returned a blank version; refusing to bind a tape to it"
    );
    Ok(version)
}

/// Re-measure the facts a tape claims, independently of the tape.
///
/// A missing dump file is reported as `None` rather than as an error: the
/// verifier distinguishes "artifact absent" from "artifact differs", and a
/// caller cannot tell those apart if absence aborts the run.
fn observe(tape: &StageTape, tape_dir: &Path) -> Result<TapeObservation> {
    let source = std::fs::read(&tape.source_pdf).map_err(|error| {
        anyhow::anyhow!(
            "recorded source is unreadable ({}): {error}",
            tape.source_pdf
        )
    })?;
    let dump_path = tape_dir.join(format!("{}.debug.txt", tape.endpoint));
    let dump_sha256 = std::fs::read(&dump_path)
        .ok()
        .map(|bytes| sha256_hex(&bytes));
    Ok(TapeObservation {
        source_sha256: sha256_hex(&source),
        dump_sha256,
        probed_backend_version: String::new(),
    })
}

fn dump_path_for(out: &Path, endpoint: &str) -> PathBuf {
    out.join(format!("{endpoint}.debug.txt"))
}

async fn run_verify(tape_path: &Path) -> Result<()> {
    let raw =
        std::fs::read(tape_path).with_context(|| format!("read tape {}", tape_path.display()))?;
    let tape: StageTape = serde_json::from_slice(&raw)
        .with_context(|| format!("parse tape {}", tape_path.display()))?;
    let tape_dir = tape_path.parent().unwrap_or_else(|| Path::new("."));

    let mut observation = observe(&tape, tape_dir)?;
    observation.probed_backend_version = probed_release().await?;

    let findings = verify_tape(&tape, &observation);
    println!(
        "stage tape verify: schema={} endpoint={} state={} sections={} distinct_models={} findings={}",
        tape.schema_version,
        tape.endpoint,
        tape.capture_state.as_str(),
        tape.sections,
        tape.distinct_models,
        findings.len()
    );
    for finding in &findings {
        println!("  [{}] {finding:?}", finding.code());
    }

    // A tape that no longer holds is a real failure: exiting zero would let a
    // stale artifact pass for evidence.
    if !findings.is_empty() {
        anyhow::bail!(
            "tape {} is not verifiable: {} finding(s)",
            tape_path.display(),
            findings.len()
        );
    }
    Ok(())
}

async fn run_capture(args: &Args) -> Result<()> {
    let pdf = args
        .pdf
        .as_ref()
        .context("--pdf is required in capture mode")?;
    let out = args
        .out
        .as_ref()
        .context("--out is required in capture mode")?;
    let endpoint = endpoint_from_arg(&args.endpoint)?;

    let pdf_bytes =
        std::fs::read(pdf).with_context(|| format!("read pinned pdf {}", pdf.display()))?;
    let source_sha256 = sha256_hex(&pdf_bytes);
    let paper_id = pdf
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("pdf file name is not valid utf-8")?
        .to_string();

    let backend_version = probed_release().await?;
    let filter = model_filter(args.models.as_deref());
    let request = ParseBackendRequest::new(paper_id.clone(), pdf_bytes);

    std::fs::create_dir_all(out).with_context(|| format!("create tape dir {}", out.display()))?;

    let parser = da_cli::parse_profile::corpus_grobid_parser();
    let (dump, failure) = match parser
        .debug_capture(endpoint, &request, filter.as_deref())
        .await
    {
        Ok(text) => (text.into_bytes(), None),
        Err(error) => (
            Vec::new(),
            Some((error.code().to_string(), error.phase().to_string())),
        ),
    };

    let tape = StageTape::from_capture(
        endpoint.as_str(),
        &paper_id,
        &pdf.display().to_string(),
        &source_sha256,
        &backend_version,
        args.models.as_deref(),
        &dump,
        failure
            .as_ref()
            .map(|(code, phase)| (code.as_str(), phase.as_str())),
        vec![
            "debugMode replaces the TEI answer; TEI is captured by the compatibility tooling, not here"
                .to_string(),
            "a missing section in the dump is NOT evidence that the model did not run (best-effort signal)"
                .to_string(),
            "`models` filters the response only; it does not reduce Java computation".to_string(),
            "citations are processed in one batch, so dump section count != reference count"
                .to_string(),
            "dump format is release-dependent; this tape is bound to backend_version".to_string(),
        ],
    );

    if !dump.is_empty() {
        std::fs::write(dump_path_for(out, endpoint.as_str()), &dump).context("write debug dump")?;
    }
    let tape_path = out.join("tape.json");
    std::fs::write(&tape_path, serde_json::to_vec_pretty(&tape)?).context("write tape")?;

    println!(
        "stage tape: endpoint={} state={} dump_bytes={} sections={} distinct_models={} backend_version={} tape={}",
        tape.endpoint,
        tape.capture_state.as_str(),
        tape.dump_bytes,
        tape.sections,
        tape.distinct_models,
        tape.backend_version,
        tape_path.display()
    );

    if tape.capture_state == CaptureState::Failed {
        anyhow::bail!(
            "debug capture failed: code={} phase={}",
            tape.failure_code.as_deref().unwrap_or("unknown"),
            tape.failure_phase.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = da_cli::logging::init_pipeline_logging("grobid_stage_tape");

    match args.verify.as_deref() {
        Some(tape_path) => run_verify(tape_path).await,
        None => run_capture(&args).await,
    }
}
