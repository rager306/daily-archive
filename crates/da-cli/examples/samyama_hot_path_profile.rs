//! Measured Samyama hot path: writes, identity lookup, traversal, readonly query,
//! vector search, import/replay and HOT+FROZEN parity. No tuning, no promotion.

use std::time::Instant;

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use kg_storage::{NodeId, SamyamaGraphStore, traits::GraphStore};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "artifacts/gepa_hub_production/hub.sgsnap")]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/samyama_hot_path_profile_m387.json"
    )]
    report: PathBuf,
}

#[derive(Serialize)]
struct WriteProfile {
    scale: usize,
    create_node_us: u64,
    create_edge_us: u64,
    set_property_us: u64,
    identity_lookup_us: u64,
    traversal_us: u64,
    nodes_by_label_us: u64,
    vector_insert_us: u64,
    vector_search_us: u64,
}

#[derive(Serialize)]
struct ParityProfile {
    frozen_nodes: usize,
    frozen_edges: usize,
    hot_node_lookup_us: u64,
    frozen_node_lookup_us: u64,
    parity_delta_us: i64,
    readonly_query_us: u64,
}

#[derive(Serialize)]
struct ImportReplayProfile {
    import_us: u64,
    replay_import_us: u64,
    import_delta_us: i64,
    snapshot_sha256: String,
    replay_nodes: usize,
    replay_edges: usize,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    writes: Vec<WriteProfile>,
    parity: ParityProfile,
    import_replay: ImportReplayProfile,
    import_eligible: bool,
    optimizer_enabled: bool,
    auto_promote: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("samyama_hot_path_profile");
    let args = Args::parse();

    // ---- HOT: live store writes at bounded scales ----
    let mut writes = Vec::new();
    for scale in [100usize, 1_000, 3_000] {
        let store = SamyamaGraphStore::new();
        let mut edge_targets = Vec::new();
        let t = Instant::now();
        let mut node_ids = Vec::new();
        for _ in 0..scale {
            node_ids.push(store.create_node_direct("HotNode").await);
        }
        let create_node_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for i in 0..scale {
            store
                .set_property_direct(node_ids[i % node_ids.len()], "score", (i as f64).into())
                .await;
        }
        let set_property_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for i in 0..scale {
            let e = store
                .create_edge_direct(
                    node_ids[i % node_ids.len()],
                    node_ids[(i + 1) % node_ids.len()],
                    "HOT_EDGE",
                )
                .await;
            edge_targets.push(e);
        }
        let create_edge_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for _ in 0..scale {
            let _ = store.store_read().await.get_node(node_ids[0]);
        }
        let identity_lookup_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for i in 0..scale {
            let _ = store.outgoing_neighbors(node_ids[i % node_ids.len()]).await;
        }
        let traversal_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        let _ = store.nodes_by_label("HotNode").await;
        let nodes_by_label_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for i in 0..scale {
            store
                .add_vector_direct(
                    "HotNode",
                    "vec",
                    node_ids[i % node_ids.len()],
                    vec![i as f32 / scale as f32, 0.5, 0.25],
                )
                .await
                .map_err(anyhow::Error::msg)?;
        }
        let vector_insert_us = t.elapsed().as_micros() as u64;

        let t = Instant::now();
        for _ in 0..10 {
            let _ = store
                .vector_search_direct("HotNode", "vec", &[0.1, 0.5, 0.25], 5)
                .await;
        }
        let vector_search_us = t.elapsed().as_micros() as u64;

        writes.push(WriteProfile {
            scale,
            create_node_us,
            create_edge_us,
            set_property_us,
            identity_lookup_us,
            traversal_us,
            nodes_by_label_us,
            vector_insert_us,
            vector_search_us,
        });
    }

    // ---- FROZEN: load snapshot and compare readonly paths ----
    let snapshot_bytes = std::fs::read(&args.snapshot)?;
    let snapshot_sha256 = ContentDigest::of_bytes(&snapshot_bytes).as_str().into();
    let frozen = SamyamaGraphStore::new();
    let t = Instant::now();
    frozen.import_snapshot(&snapshot_bytes).await?;
    let import_us = t.elapsed().as_micros() as u64;
    let frozen_nodes = frozen.node_count().await;
    let frozen_edges = frozen.edge_count().await;

    // Replay import into a second store; measure idempotency.
    let replay = SamyamaGraphStore::new();
    let t = Instant::now();
    replay.import_snapshot(&snapshot_bytes).await?;
    let replay_import_us = t.elapsed().as_micros() as u64;
    if replay.node_count().await != frozen_nodes || replay.edge_count().await != frozen_edges {
        return Err(anyhow!("snapshot replay changed node/edge counts"));
    }

    let t = Instant::now();
    let hot_lookup = frozen.outgoing_neighbors(NodeId::from(0_u64)).await;
    let hot_node_lookup_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let _ = frozen.store_read().await.get_node(NodeId::from(0_u64));
    let frozen_node_lookup_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let _ = frozen
        .query_readonly("profile", "MATCH (n) RETURN n LIMIT 5")
        .await;
    let readonly_query_us = t.elapsed().as_micros() as u64;
    let _ = hot_lookup;

    let report = Report {
        schema_version: "samyama-hot-path-profile.v1",
        writes,
        parity: ParityProfile {
            frozen_nodes,
            frozen_edges,
            hot_node_lookup_us,
            frozen_node_lookup_us,
            parity_delta_us: frozen_node_lookup_us as i64 - hot_node_lookup_us as i64,
            readonly_query_us,
        },
        import_replay: ImportReplayProfile {
            import_us,
            replay_import_us,
            import_delta_us: replay_import_us as i64 - import_us as i64,
            snapshot_sha256,
            replay_nodes: replay.node_count().await,
            replay_edges: replay.edge_count().await,
        },
        import_eligible: false,
        optimizer_enabled: false,
        auto_promote: false,
    };
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "samyama_hot_path_profile_complete",
        write_scales = report.writes.len(),
        frozen_nodes = report.parity.frozen_nodes,
        frozen_edges = report.parity.frozen_edges,
        import_us = report.import_replay.import_us,
        import_eligible = false,
        "Samyama hot path profile completed"
    );
    println!(
        "samyama-hot-path writes={} frozen_nodes={} frozen_edges={} import_us={} parity_delta_us={} D127=false report={}",
        report.writes.len(),
        report.parity.frozen_nodes,
        report.parity.frozen_edges,
        report.import_replay.import_us,
        report.parity.parity_delta_us,
        args.report.display()
    );
    Ok(())
}
