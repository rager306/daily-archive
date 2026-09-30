//! S08/T02 bounded SMP qualification emitter (offline, two surfaces).
//!
//! Sweeps the worker matrix `1..=physical_core_cap` over the real native CRF
//! core (`da_ports::map_input` + `da_ports::viterbi_decode`) on the frozen
//! synthetic Wapiti models, and writes two artefacts:
//!
//! - the *deterministic* acceptance artifact to
//!   `artifacts/parse_compat/native_core_smp.json` — byte-reproducible, and the
//!   emitter refuses to write when two independent matrix runs disagree on its
//!   bytes (typed [`NativeCoreSmpError::DeterministicDivergence`]);
//! - the *observation* run snapshot to
//!   `artifacts/parse_compat/native_core_smp_observations.json` — declared
//!   `reproducible: false`, `not_a_receipt: true`, bound back to the
//!   deterministic bytes by `sha256`. Because that surface is non-reproducible
//!   by declaration, a snapshot that already binds these deterministic bytes is
//!   left untouched instead of being churned with fresh wall-clock numbers:
//!   re-invoking this binary on unchanged inputs is a no-op on disk, which is
//!   what lets it be used as a verification step without mutating its own
//!   verification target. `--observations-artifact` forces a fresh snapshot
//!   (the explicit refresh path), and a missing/corrupt/schema-drifted/stale
//!   snapshot is always refreshed.
//!
//! Nothing is selected, spliced or promoted: the deterministic artifact records
//! a closed set of breakpoint verdicts and the conservation of every worker
//! count; the wall-clock and runtime gauges classify a worker count and never
//! enter an acceptance denominator.
//!
//! This binary owns its process-level peak RSS reader. The module itself
//! installs no allocator and reads no `/proc`: peak RSS is injected.

use clap::Parser;
use da_cli::layout_runtime::detect_system_topology;
use da_cli::native_core_smp::{
    NATIVE_CORE_SMP_ARTIFACT_REL, NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL, NativeCoreSmpConfig,
    NativeCoreSmpError, NativeCoreSmpFamily, NativeCoreSmpReceipt,
    load_cpu_smp_candidate_from_manifest, observations_snapshot_is_current,
    qualify_native_core_smp,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const DEFAULT_DOCUMENTS_PER_FAMILY: usize = 6;
const DEFAULT_ROWS_PER_DOCUMENT: usize = 64;
const DEFAULT_REPETITIONS: usize = 3;

#[derive(Parser, Debug)]
struct Args {
    /// Deterministic artifact path (defaults to the committed artifact location).
    #[arg(long)]
    artifact: Option<PathBuf>,
    /// Observation snapshot path. Omitted → the committed snapshot location is
    /// validated and only refreshed when it is not a current record of the
    /// deterministic bytes; provided → that path is always written fresh.
    #[arg(long)]
    observations_artifact: Option<PathBuf>,
    /// Documents per frozen family; each document is one job.
    #[arg(long)]
    documents: Option<usize>,
    /// Rows rendered per document. Rows are derived from the model's own
    /// observation table, so the workload stays model-derived and deterministic.
    #[arg(long)]
    rows_per_document: Option<usize>,
    /// Benchmark repetitions per worker count (never pooled into acceptance).
    #[arg(long)]
    repetitions: Option<usize>,
}

/// Frozen synthetic models, listed exactly as S03 shipped them. The row set is
/// derived per model; nothing here is document content.
fn frozen_families() -> [(&'static str, &'static [u8]); 8] {
    [
        (
            "fulltext",
            include_bytes!("../../da-parse/tests/fixtures/native_fulltext_synthetic.wapiti"),
        ),
        (
            "figure",
            include_bytes!("../../da-parse/tests/fixtures/native_figure_synthetic.wapiti"),
        ),
        (
            "table",
            include_bytes!("../../da-parse/tests/fixtures/native_table_synthetic.wapiti"),
        ),
        (
            "funding",
            include_bytes!("../../da-parse/tests/fixtures/native_funding_synthetic.wapiti"),
        ),
        (
            "bibliography_reference",
            include_bytes!(
                "../../da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti"
            ),
        ),
        (
            "bibliography_citation",
            include_bytes!(
                "../../da-parse/tests/fixtures/native_bibliography_citation_synthetic.wapiti"
            ),
        ),
        (
            "bibliography_name",
            include_bytes!(
                "../../da-parse/tests/fixtures/native_bibliography_name_synthetic.wapiti"
            ),
        ),
        (
            "bibliography_date",
            include_bytes!(
                "../../da-parse/tests/fixtures/native_bibliography_date_synthetic.wapiti"
            ),
        ),
    ]
}

/// Process peak RSS from `/proc/self/status` (`VmHWM`). `None` when the kernel
/// does not publish it or the value is unparseable: typed absence, never a
/// fabricated zero.
fn peak_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kibibytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kibibytes.saturating_mul(1024))
}

fn main() {
    // Every da-cli example is a composition root: it owns the .env-backed
    // rolling JSON subscriber (ADR-052) and must hold the guard for its
    // lifetime, otherwise file logging silently stops.
    let _log_guard = da_cli::logging::init_pipeline_logging("native_core_smp");
    let args = Args::parse();
    if let Err(error) = run(&args) {
        eprintln!("native_core_smp: {error}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let deterministic_path = args
        .artifact
        .clone()
        .unwrap_or_else(|| PathBuf::from(NATIVE_CORE_SMP_ARTIFACT_REL));
    let forced_observations_path = args.observations_artifact.clone();
    let observations_path = forced_observations_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL));
    let config = NativeCoreSmpConfig::new(
        args.documents.unwrap_or(DEFAULT_DOCUMENTS_PER_FAMILY),
        args.rows_per_document.unwrap_or(DEFAULT_ROWS_PER_DOCUMENT),
        args.repetitions.unwrap_or(DEFAULT_REPETITIONS),
    );

    let data = load_cpu_smp_candidate_from_manifest(MANIFEST)?;
    let topology = detect_system_topology()?;
    let rows_per_family = config.documents_per_family * config.rows_per_document;
    let families = frozen_families()
        .iter()
        .map(|(family, bytes)| {
            NativeCoreSmpFamily::from_frozen_model(family, bytes, rows_per_family)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Two independent matrix runs: the *deterministic artifact bytes* — not
    // merely the projection digest — must be identical, otherwise nothing is
    // written. The observational values are free to differ; they go to a
    // surface that declares itself non-reproducible.
    let mut first_rss = peak_rss_bytes;
    let first = qualify_native_core_smp(&data, &families, &topology, &config, &mut first_rss)?;
    let mut second_rss = peak_rss_bytes;
    let second = qualify_native_core_smp(&data, &families, &topology, &config, &mut second_rss)?;

    let bytes = first.artifact_json_bytes()?;
    let competing = second.artifact_json_bytes()?;
    if bytes != competing {
        return Err(NativeCoreSmpError::DeterministicDivergence {
            first_digest: sha256_prefixed(&bytes),
            second_digest: sha256_prefixed(&competing),
        }
        .into());
    }

    // Write the deterministic surface first, then bind the observation snapshot
    // to the bytes that were actually written — never to a recomputation that
    // could silently drift from the file on disk.
    write_artifact(&deterministic_path, &bytes)?;
    let deterministic_sha256 = sha256_prefixed(&bytes);
    let observations = first.observations_json_bytes(&deterministic_sha256)?;
    let observations_written = if forced_observations_path.is_some() {
        write_artifact(&observations_path, &observations)?;
        true
    } else {
        write_observations_if_stale(&observations_path, &observations, &deterministic_sha256)?
    };

    report(
        &first,
        &deterministic_path,
        &deterministic_sha256,
        bytes.len(),
        &observations_path,
        observations.len(),
        observations_written,
    );
    Ok(())
}

/// Writes the observation run snapshot unless the file on disk is already a
/// current record of the deterministic bytes just written.
///
/// The observation surface is declared non-reproducible: a rewrite yields
/// different bytes for the same acceptance object, so skipping a current
/// snapshot keeps the emitter idempotent and keeps a verification run from
/// mutating its own verification target. A missing, unreadable, corrupt,
/// schema-drifted or stale-bound snapshot is still refreshed — never trusted.
/// Returns `true` when the file was written.
fn write_observations_if_stale(
    path: &Path,
    fresh: &[u8],
    deterministic_sha256: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // One `Result` test, not the nested `if` clippy suggests collapsing into a
    // let-chain: the workspace MSRV is 1.85 and let-chains need 1.88.
    if std::fs::read(path)
        .is_ok_and(|existing| observations_snapshot_is_current(&existing, deterministic_sha256))
    {
        return Ok(false);
    }
    write_artifact(path, fresh)?;
    Ok(true)
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn write_artifact(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Redacted human summary: worker counts, conservation, closed verdicts, the
/// observational throughput and both surface paths by digest. No model token,
/// no physical path, no hostname.
fn report(
    receipt: &NativeCoreSmpReceipt,
    deterministic_path: &Path,
    deterministic_sha256: &str,
    deterministic_bytes: usize,
    observations_path: &Path,
    observation_bytes: usize,
    observations_written: bool,
) {
    println!(
        "native core smp: families={} jobs={} workers=1..={} repetitions={}",
        receipt.families.len(),
        receipt.workload.jobs,
        receipt.topology.physical_cores,
        receipt.workload.repetitions
    );
    for entry in &receipt.matrix.entries {
        println!(
            "  workers={:<3} admitted={:<4} terminals={:<4} completed={:<4} digest={} \
throughput={}/s p95={}ms scratch_high_water={}B",
            entry.workers,
            entry.admitted,
            entry.terminals,
            entry.completed,
            &entry.semantic_digest[..19],
            entry.throughput_units_per_second,
            entry.latency_ms.p95_ms,
            entry.scratch_high_water_bytes
        );
    }
    for record in &receipt.matrix.breakpoints {
        println!(
            "  breakpoint workers={:<3} verdict={:<16} reason={}",
            record.workers,
            record.verdict.as_str(),
            record.reason
        );
    }
    println!(
        "  scalar_oracle={} selection_forbidden={} cpu_only={} subprocess_io_included={}",
        &receipt.matrix.scalar_oracle_digest[..19],
        receipt.selection_forbidden,
        receipt.cpu_only,
        receipt.subprocess_io_included
    );
    println!(
        "  peak_rss={:?} memory_ceiling={:?} timing_disposition={}",
        receipt.peak_rss_bytes,
        receipt.memory_pressure_ceiling_bytes,
        receipt.observations.disposition
    );
    println!(
        "  deterministic_projection={}",
        receipt.deterministic_projection_digest
    );
    println!(
        "  deterministic artifact path={} bytes={} {}",
        deterministic_path.display(),
        deterministic_bytes,
        deterministic_sha256
    );
    println!(
        "  observations artifact path={} bytes={} disposition={} reproducible=false \
not_a_receipt=true source_sha256={}",
        observations_path.display(),
        observation_bytes,
        if observations_written {
            "refreshed"
        } else {
            "current-unchanged"
        },
        deterministic_sha256
    );
}
