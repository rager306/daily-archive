//! Direct product probe for the bounded RemoteGraphStore read plane.

use kg_storage::{GraphStore, RemoteGraphConfig, RemoteGraphStore};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:16379".into());
    let store = RemoteGraphStore::new(RemoteGraphConfig::new(endpoint.clone())?)?;
    let healthy = store.health().await?;
    let revisions = store
        .query_readonly(
            "daily_archive",
            "MATCH (r:PaperRevision) RETURN r.revision AS revision, r.is_current AS current",
        )
        .await?;
    let count_query = "MATCH (r:PaperRevision) RETURN count(r) AS revisions";
    let counts_before = store.query_readonly("daily_archive", count_query).await?;
    let started = Instant::now();
    let mut reads = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let store = store.clone();
        reads.spawn(async move { store.query_readonly("daily_archive", count_query).await });
    }
    let mut concurrent_counts = Vec::new();
    while let Some(result) = reads.join_next().await {
        concurrent_counts.push(result??);
    }
    let concurrent_duration_ms = started.elapsed().as_millis();
    let counts_after = store.query_readonly("daily_archive", count_query).await?;
    let rejected_write = store
        .query_readonly("daily_archive", "CREATE (n:Forbidden) RETURN n")
        .await
        .unwrap_err()
        .to_string();
    let unsupported_snapshot = store.export_snapshot().await.unwrap_err().to_string();
    let unsupported_index = store
        .create_property_index("PaperRevision", "vid")
        .await
        .unwrap_err()
        .to_string();
    let report = serde_json::json!({
        "schema_version": "remote-graph-probe.v1",
        "endpoint": endpoint,
        "healthy": healthy,
        "capability": "scalar_read_only",
        "revisions": revisions,
        "counts_before": counts_before,
        "concurrent_counts": concurrent_counts,
        "concurrent_requests": 8,
        "concurrent_duration_ms": concurrent_duration_ms,
        "counts_after": counts_after,
        "rejected_write": rejected_write,
        "unsupported_snapshot": unsupported_snapshot,
        "unsupported_index": unsupported_index,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
