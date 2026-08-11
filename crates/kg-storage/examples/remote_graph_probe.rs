//! Direct product probe for the bounded RemoteGraphStore read plane.

use kg_storage::{GraphStore, RemoteGraphConfig, RemoteGraphStore};

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
    let counts = store
        .query_readonly(
            "daily_archive",
            "MATCH (r:PaperRevision) RETURN count(r) AS revisions",
        )
        .await?;
    let rejected_write = store
        .query_readonly("daily_archive", "CREATE (n:Forbidden) RETURN n")
        .await
        .unwrap_err()
        .to_string();
    let unsupported_snapshot = store.export_snapshot().await.unwrap_err().to_string();
    let report = serde_json::json!({
        "schema_version": "remote-graph-probe.v1",
        "endpoint": endpoint,
        "healthy": healthy,
        "capability": "scalar_read_only",
        "revisions": revisions,
        "counts": counts,
        "rejected_write": rejected_write,
        "unsupported_snapshot": unsupported_snapshot,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
