//! Offline stage-tape recorder for the Java GROBID cascade (M506/S01/T03).
//!
//! What this records and why: the migration to a native Rust parser needs a
//! frozen record of what the Java cascade actually computed per model, so a
//! later slice can tell a layout/feature divergence apart from a model-weight
//! divergence. GROBID's `debugMode` is the only source for that, and it is a
//! *replacement* answer: with `debugMode=1` the service returns `text/plain`
//! instead of TEI. TEI and dump are therefore two separate requests, and this
//! recorder captures only the dump.
//!
//! Raw TEI capture is deliberately NOT duplicated here: it already exists in
//! the compatibility tooling (`parse_compat_baseline` and the batch paths).
//! Adding a second TEI capture would create two sources of the same truth.
//!
//! This is offline evidence, not a production route: it never writes a graph,
//! never imports and never promotes. `debug_capture` is intentionally outside
//! `ParserPort` for the same reason.
//!
//! Capture states are explicit, because "no section in the dump" must never be
//! read as "the model did not run":
//!   captured          — the dump came back with content
//!   uncaptured_empty  — a successful response carrying no bytes
//!   filtered          — `--models` was applied, so the dump is a subset
//!   failed            — the request failed; the typed code is recorded
//! A stored tape always carries the probed release, so a tape can never be
//! mistaken for another release's evidence.

use anyhow::{Context, Result};
use clap::Parser;
use da_parse::{ParseBackendRequest, ParseEndpoint, model_invocation_counts, parse_debug_sections};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// Pinned PDF to record against.
    #[arg(long)]
    pdf: PathBuf,
    /// Output directory for the tape.
    #[arg(long)]
    out: PathBuf,
    /// Endpoint whose model dump is recorded.
    #[arg(long, default_value = "fulltext")]
    endpoint: String,
    /// Optional `models` filter (comma-separated). Applying it makes the tape a
    /// filtered subset rather than a full dump, which is recorded explicitly.
    #[arg(long)]
    models: Option<String>,
}

#[derive(Serialize)]
struct ModelCoverage {
    model: String,
    invocations: usize,
}

#[derive(Serialize)]
struct Tape {
    schema_version: &'static str,
    endpoint: String,
    paper_id: String,
    source_pdf: String,
    source_sha256: String,
    backend_version: String,
    models_filter: Option<String>,
    capture_state: &'static str,
    dump_bytes: usize,
    dump_sha256: Option<String>,
    /// Total model sections found: invocations, not distinct models.
    sections: usize,
    /// Distinct models exercised, in first-appearance order.
    distinct_models: usize,
    /// Per-model invocation counts: the coverage view a later slice compares
    /// against, so "the date model ran 46 times" cannot be confused with
    /// "the dump contained this many models".
    models_observed: Vec<ModelCoverage>,
    failure_code: Option<String>,
    failure_phase: Option<String>,
    /// Contract notes a reader must have before trusting the tape.
    notes: Vec<&'static str>,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = da_cli::logging::init_pipeline_logging("grobid_stage_tape");

    let endpoint = endpoint_from_arg(&args.endpoint)?;
    let pdf_bytes = std::fs::read(&args.pdf)
        .with_context(|| format!("read pinned pdf {}", args.pdf.display()))?;
    let source_sha256 = sha256_hex(&pdf_bytes);
    let paper_id = args
        .pdf
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("pdf file name is not valid utf-8")?
        .to_string();

    let parser = da_cli::parse_profile::corpus_grobid_parser();

    // Fail closed: a tape must carry the release it was recorded against, or it
    // is unusable as migration evidence. Blank or failed probes are refused
    // rather than stamped with a historical identity.
    let probe = parser
        .probe_version()
        .await
        .map_err(|failure| anyhow::anyhow!("GROBID /api/version probe failed: {failure}"))?;
    let backend_version = probe.version.trim().to_string();
    anyhow::ensure!(
        !backend_version.is_empty(),
        "GROBID /api/version returned a blank version; refusing to record an unlabelled tape"
    );

    let filter = model_filter(args.models.as_deref());
    let request = ParseBackendRequest::new(paper_id.clone(), pdf_bytes.clone());

    std::fs::create_dir_all(&args.out)
        .with_context(|| format!("create tape dir {}", args.out.display()))?;

    let (capture_state, dump, failure_code, failure_phase) = match parser
        .debug_capture(endpoint, &request, filter.as_deref())
        .await
    {
        Ok(text) => {
            let bytes = text.into_bytes();
            let state = if bytes.is_empty() {
                "uncaptured_empty"
            } else if filter.is_some() {
                "filtered"
            } else {
                "captured"
            };
            (state, bytes, None, None)
        }
        Err(failure) => {
            let code = failure.code().to_string();
            let phase = failure.phase().to_string();
            ("failed", Vec::new(), Some(code), Some(phase))
        }
    };

    let dump_sha256 = (!dump.is_empty()).then(|| sha256_hex(&dump));
    // Coverage is derived from the dump itself rather than from the request: a
    // model can be invoked many times inside one request, and a filtered run
    // proves only that the filter was applied, never that a model did not run.
    let dump_text = String::from_utf8_lossy(&dump);
    let sections = parse_debug_sections(&dump_text).len();
    let models_observed = model_invocation_counts(&dump_text)
        .into_iter()
        .map(|(model, invocations)| ModelCoverage { model, invocations })
        .collect::<Vec<_>>();
    let distinct_models = models_observed.len();
    if !dump.is_empty() {
        std::fs::write(
            args.out.join(format!("{}.debug.txt", endpoint.as_str())),
            &dump,
        )
        .context("write debug dump")?;
    }

    let tape = Tape {
        schema_version: "grobid-stage-tape.v1",
        endpoint: endpoint.as_str().to_string(),
        paper_id,
        source_pdf: args.pdf.display().to_string(),
        source_sha256,
        backend_version,
        models_filter: args.models.clone(),
        capture_state,
        dump_bytes: dump.len(),
        dump_sha256,
        sections,
        distinct_models,
        models_observed,
        failure_code,
        failure_phase,
        notes: vec![
            "debugMode replaces the TEI answer; TEI is captured by the compatibility tooling, not here",
            "a missing section in the dump is NOT evidence that the model did not run (best-effort signal)",
            "`models` filters the response only; it does not reduce Java computation",
            "citations are processed in one batch, so dump section count != reference count",
            "dump format is release-dependent; this tape is bound to backend_version above",
        ],
    };
    let tape_path = args.out.join("tape.json");
    std::fs::write(&tape_path, serde_json::to_vec_pretty(&tape)?).context("write tape")?;

    println!(
        "stage tape: endpoint={} state={} dump_bytes={} sections={} distinct_models={} backend_version={} tape={}",
        tape.endpoint,
        tape.capture_state,
        tape.dump_bytes,
        tape.sections,
        tape.distinct_models,
        tape.backend_version,
        tape_path.display()
    );

    // A failed capture is a real outcome worth a non-zero exit: a green run that
    // silently recorded nothing would be worse than a visible failure.
    if tape.capture_state == "failed" {
        anyhow::bail!(
            "debug capture failed: code={} phase={}",
            tape.failure_code.as_deref().unwrap_or("unknown"),
            tape.failure_phase.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}
