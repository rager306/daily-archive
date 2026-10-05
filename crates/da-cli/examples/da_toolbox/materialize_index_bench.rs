//! M443: hot-path lookup benchmark at full-store scale.
//!
//! Loads a .sgsnap into an in-process SamyamaGraphStore and measures
//! `find_node_by_string_property` on the materialization hot keys with and
//! without property indexes. Miss lookups are the worst case for the linear
//! scan (every node of the label is visited).
//!
//! Usage:
//!   cargo run -p da-cli --example da_toolbox -- materialize_index_bench \
//!     --snapshot artifacts/catalog_sweep/catalog_materialized.sgsnap

use anyhow::Result;
use clap::Parser;
use da_adapters::SamyamaGraphStore;
// ADR-052 composition root (governor-pinned).
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/catalog_sweep/catalog_materialized.sgsnap"
    )]
    snapshot: PathBuf,
    /// Lookup probes per phase.
    #[arg(long, default_value_t = 300)]
    probes: usize,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("materialize_index_bench");
    let args = Args::parse();
    let bytes = std::fs::read(&args.snapshot)?;
    println!(
        "snapshot: {} ({} bytes)",
        args.snapshot.display(),
        bytes.len()
    );

    let store = SamyamaGraphStore::try_from_env()?;
    store.import_snapshot(&bytes).await?;
    let nodes = store.node_count().await;
    let edges = store.edge_count().await;
    println!("loaded: {nodes} nodes / {edges} edges");

    // Worst-case probes: syntactically valid vids that do not exist.
    let probes: Vec<String> = (0..args.probes)
        .map(|i| format!("vid:entity:bench-miss-{i}"))
        .collect();

    let miss_arxiv: Vec<String> = (0..args.probes).map(|i| format!("9999.9{i:04}")).collect();

    // Phase 1: no indexes (linear scans).
    let t = Instant::now();
    for p in &probes {
        let _ = store.find_node_by_string_property("Entity", "vid", p).await;
    }
    let entity_scan = t.elapsed();

    let t = Instant::now();
    for p in &miss_arxiv {
        let _ = store
            .find_node_by_string_property("Paper", "arxiv_id", p)
            .await;
    }
    let paper_scan = t.elapsed();

    // Phase 2: create the materialization hot-path indexes (timed).
    let hot = [
        ("Paper", "arxiv_id"),
        ("Paper", "vid"),
        ("PaperRevision", "vid"),
        ("Entity", "vid"),
        ("Section", "vid"),
        ("Reference", "vid"),
        ("SourceManifestation", "vid"),
        ("Category", "vid"),
        ("Source", "code"),
    ];
    let t = Instant::now();
    for (label, prop) in hot {
        store.create_property_index(label, prop).await?;
    }
    let index_build = t.elapsed();

    // Phase 3: same probes against indexes.
    let t = Instant::now();
    for p in &probes {
        let _ = store.find_node_by_string_property("Entity", "vid", p).await;
    }
    let entity_indexed = t.elapsed();

    let t = Instant::now();
    for p in &miss_arxiv {
        let _ = store
            .find_node_by_string_property("Paper", "arxiv_id", p)
            .await;
    }
    let paper_indexed = t.elapsed();

    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!(
        "entity_vid  : scan {:.1} ms vs indexed {:.1} ms ({:.0}x, {:.1} us/probe scan)",
        ms(entity_scan),
        ms(entity_indexed),
        entity_scan.as_secs_f64() / entity_indexed.as_secs_f64().max(1e-9),
        entity_scan.as_secs_f64() * 1e6 / probes.len() as f64,
    );
    println!(
        "paper_arxiv : scan {:.1} ms vs indexed {:.1} ms ({:.0}x)",
        ms(paper_scan),
        ms(paper_indexed),
        paper_scan.as_secs_f64() / paper_indexed.as_secs_f64().max(1e-9),
    );
    println!(
        "index build (9 keys, backfill {nodes} nodes): {:.0} ms",
        ms(index_build)
    );
    Ok(())
}
