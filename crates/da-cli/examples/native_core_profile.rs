//! S08/T01 native CRF core profile emitter (offline, observational).
//!
//! Profiles the real scalar CRF core (`da_domain::scalar_crf`) on the frozen
//! synthetic Wapiti models, derives `dense_numeric_kernel_found` from the DATA
//! threshold in `data/parse/comparison_contract.yaml`, and writes a redacted,
//! byte-reproducible artifact to `artifacts/parse_compat/native_core_profile.json`.
//!
//! No production route, cache, graph, import, or promotion capability is
//! touched: this binary only measures, and `da-domain` is not modified.
//!
//! The counting `#[global_allocator]` lives here — in the example binary, which
//! is its own crate — and never in `da-cli`, `da-parse`, or `da-domain`. The
//! profile module receives allocation deltas through an injected snapshot
//! closure instead of installing an allocator itself.

use clap::Parser;
use da_cli::native_core_profile::{
    AllocatorObservation, AllocatorSnapshot, NATIVE_CORE_PROFILE_ARTIFACT_REL,
    NativeCoreFamilySpec, NativeCoreProfileOptions, load_qualification_from_manifest,
    profile_native_core,
};
use sha2::{Digest, Sha256};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const ROWS_PER_FAMILY: usize = 1024;

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Parser, Debug)]
struct Args {
    /// Artifact path (defaults to the committed artifact location).
    #[arg(long)]
    artifact: Option<PathBuf>,
    /// Rows rendered per family; rows are drawn from the model's own
    /// observation table, so the workload stays model-derived and deterministic.
    #[arg(long)]
    rows: Option<usize>,
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

fn counting_snapshot() -> AllocatorSnapshot {
    AllocatorSnapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

fn main() {
    // Every da-cli example is a composition root: it owns the .env-backed
    // rolling JSON subscriber (ADR-052) and must hold the guard for its
    // lifetime, otherwise file logging silently stops.
    let _log_guard = da_cli::logging::init_pipeline_logging("native_core_profile");
    let args = Args::parse();
    if let Err(error) = run(&args) {
        eprintln!("native_core_profile: {error}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_path = args
        .artifact
        .clone()
        .unwrap_or_else(|| PathBuf::from(NATIVE_CORE_PROFILE_ARTIFACT_REL));
    let rows = args.rows.unwrap_or(ROWS_PER_FAMILY);

    let data = load_qualification_from_manifest(MANIFEST)?;
    let options = NativeCoreProfileOptions::from_data(&data, rows);
    let specs = frozen_families()
        .iter()
        .map(|(family, bytes)| NativeCoreFamilySpec::from_frozen_model(family, bytes, rows))
        .collect::<Result<Vec<_>, _>>()?;

    // Two independent emitter runs must agree byte-for-byte, and
    // `profile_native_core` already replays internally; a disagreement is
    // reported instead of emitting a flaky claim.
    let mut first_snapshot = counting_snapshot;
    let first = profile_native_core(&data, &specs, &options, &mut first_snapshot)?;
    let first_bytes = first.artifact_json_bytes()?;
    let mut second_snapshot = counting_snapshot;
    let second = profile_native_core(&data, &specs, &options, &mut second_snapshot)?;
    let second_bytes = second.artifact_json_bytes()?;
    if first_bytes != second_bytes {
        return Err(format!(
            "independent emitter runs diverged ({} vs {}); refusing to write an unstable artifact",
            format!("sha256:{:x}", Sha256::digest(&first_bytes)),
            format!("sha256:{:x}", Sha256::digest(&second_bytes))
        )
        .into());
    }

    write_artifact(&artifact_path, &first_bytes)?;
    report(&first, &first_bytes, &artifact_path);
    Ok(())
}

fn write_artifact(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Redacted human summary: counts, closed dispositions, and phase shares in
/// basis points. No model token, no path, no hostname.
fn report(
    receipt: &da_cli::native_core_profile::NativeCoreProfileReceipt,
    bytes: &[u8],
    artifact_path: &Path,
) {
    let allocator = match &receipt.allocator {
        AllocatorObservation::Stable {
            allocations,
            allocated_bytes,
        } => format!("stable allocations={allocations} allocated_bytes={allocated_bytes}"),
        AllocatorObservation::Unstable { first, second } => format!(
            "unstable first=({},{}) second=({},{})",
            first.allocations, first.allocated_bytes, second.allocations, second.allocated_bytes
        ),
    };
    println!(
        "native core profile: families={} rows_per_family={} allocator={}",
        receipt.families.len(),
        receipt
            .families
            .first()
            .map(|family| family.rows)
            .unwrap_or(0),
        allocator
    );
    for family in &receipt.families {
        println!(
            "  {:<24} state={:?} dense_found={} dense_phase={} shares_bp={:?}",
            family.family,
            family.disposition.optimization_state,
            family.disposition.dense_numeric_kernel_found,
            family
                .disposition
                .dense_phase
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            family.phase_share_basis_points
        );
    }
    println!(
        "  aggregate state={:?} pgo={} selection_forbidden={}",
        receipt.disposition.optimization_state,
        receipt.disposition.pgo_disposition,
        receipt.disposition.selection_forbidden
    );
    println!(
        "  artifact path={} bytes={}",
        artifact_path.display(),
        bytes.len()
    );
    println!("  artifact sha256:{:x}", Sha256::digest(bytes));
    println!(
        "  timing disposition: {}",
        da_cli::native_core_profile::NATIVE_CORE_TIMING_DISPOSITION
    );
}
