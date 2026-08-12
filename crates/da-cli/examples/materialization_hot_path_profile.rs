//! Bounded read/process-local profile for materialization hot-path hypotheses.

use std::time::Instant;

use anyhow::Result;
use da_application::temporal_stamp::edge_type_is_temporal_fact;
use da_ports::graph_store::DirectGraphStore;
use kg_storage::SamyamaGraphStore;
use serde::Serialize;

#[derive(Serialize)]
struct EdgeLookupProfile {
    edge_count: usize,
    probes: usize,
    elapsed_us: u128,
    us_per_probe: f64,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    temporal_classifications: usize,
    temporal_elapsed_us: u128,
    temporal_us_per_call: f64,
    edge_lookup: Vec<EdgeLookupProfile>,
    import_eligible: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("materialization_hot_path_profile");
    let classifications = 1_000;
    let temporal_started = Instant::now();
    let mut facts = 0usize;
    for index in 0..classifications {
        facts += usize::from(edge_type_is_temporal_fact(if index % 2 == 0 {
            "CITES"
        } else {
            "HAS_SECTION"
        }));
    }
    assert_eq!(facts, classifications / 2);
    let temporal_elapsed = temporal_started.elapsed();

    let mut edge_lookup = Vec::new();
    for edge_count in [100usize, 1_000, 3_000] {
        let store = SamyamaGraphStore::new();
        let source = store.create_node("ProfileNode").await?;
        let target = store.create_node("ProfileNode").await?;
        for _ in 0..edge_count {
            store.create_edge(source, target, "PROFILE_EDGE").await?;
        }
        let probes = 500usize;
        let started = Instant::now();
        for _ in 0..probes {
            assert!(
                store
                    .get_edges_between(source, target + 1, "PROFILE_MISSING")
                    .await
                    .is_empty()
            );
        }
        let elapsed = started.elapsed();
        edge_lookup.push(EdgeLookupProfile {
            edge_count,
            probes,
            elapsed_us: elapsed.as_micros(),
            us_per_probe: elapsed.as_secs_f64() * 1_000_000.0 / probes as f64,
        });
    }

    let report = Report {
        schema_version: "materialization-hot-path-profile.v1",
        temporal_classifications: classifications,
        temporal_elapsed_us: temporal_elapsed.as_micros(),
        temporal_us_per_call: temporal_elapsed.as_secs_f64() * 1_000_000.0 / classifications as f64,
        edge_lookup,
        import_eligible: false,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
