//! S08/T02 bounded SMP qualification emitter (offline, observational).
//!
//! Sweeps the worker matrix `1..=physical_core_cap` over the real native CRF
//! core (`da_ports::map_input` + `da_ports::viterbi_decode`) on the frozen
//! synthetic Wapiti models, and writes a redacted artifact to
//! `artifacts/parse_compat/native_core_smp.json`.
//!
//! Nothing is selected, spliced or promoted: the artifact records a closed set
//! of breakpoint verdicts, and the *deterministic* projection of two
//! independent matrix runs must be byte-identical or the emitter refuses to
//! write. Wall-clock and runtime gauges stay declared observational
//! field-by-field, outside every acceptance denominator.
//!
//! This binary owns its process-level peak RSS reader. The module itself
//! installs no allocator and reads no `/proc`: peak RSS is injected.

use clap::Parser;
use da_cli::layout_runtime::detect_system_topology;
use da_cli::native_core_smp::{
    NATIVE_CORE_SMP_ARTIFACT_REL, NativeCoreSmpConfig, NativeCoreSmpFamily,
    load_cpu_smp_candidate_from_manifest, qualify_native_core_smp,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const DEFAULT_DOCUMENTS_PER_FAMILY: usize = 6;
const DEFAULT_ROWS_PER_DOCUMENT: usize = 64;
const DEFAULT_REPETITIONS: usize = 3;

#[derive(Parser, Debug)]
struct Args {
    /// Artifact path (defaults to the committed artifact location).
    #[arg(long)]
    artifact: Option<PathBuf>,
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
    let artifact_path = args
        .artifact
        .clone()
        .unwrap_or_else(|| PathBuf::from(NATIVE_CORE_SMP_ARTIFACT_REL));
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

    // Two independent matrix runs: the deterministic projection (job set,
    // per-job semantic digests, conservation, per-worker counts) must be
    // byte-identical, otherwise nothing is written.
    let mut first_rss = peak_rss_bytes;
    let first = qualify_native_core_smp(&data, &families, &topology, &config, &mut first_rss)?;
    let mut second_rss = peak_rss_bytes;
    let second = qualify_native_core_smp(&data, &families, &topology, &config, &mut second_rss)?;
    let first_projection = first.deterministic_projection()?;
    let second_projection = second.deterministic_projection()?;
    if first_projection != second_projection {
        return Err(format!(
            "two independent SMP matrix runs diverged on the deterministic projection ({} vs \
{}); refusing to write an unstable artifact",
            projection_digest(&first_projection),
            projection_digest(&second_projection)
        )
        .into());
    }

    let bytes = first.artifact_json_bytes()?;
    write_artifact(&artifact_path, &bytes)?;
    report(
        &first,
        &projection_digest(&first_projection),
        &bytes,
        &artifact_path,
    );
    Ok(())
}

fn projection_digest(projection: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(projection.as_bytes()))
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

/// Redacted human summary: worker counts, conservation, closed verdicts and the
/// observational throughput. No model token, no physical path, no hostname.
fn report(
    receipt: &da_cli::native_core_smp::NativeCoreSmpReceipt,
    projection_digest: &str,
    bytes: &[u8],
    artifact_path: &Path,
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
    println!("  deterministic_projection={projection_digest}");
    println!(
        "  artifact path={} bytes={} sha256:{:x}",
        artifact_path.display(),
        bytes.len(),
        Sha256::digest(bytes)
    );
}
