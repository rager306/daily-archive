//! Real Samyama semantic snapshot identity probe.

use kg_storage::{DirectGraphStore, GraphStore, SamyamaGraphStore, fingerprint_snapshot};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: snapshot_identity_probe <input.sgsnap>")?;
    let input = std::fs::read(&path)?;
    let store = SamyamaGraphStore::new();
    store.import_snapshot(&input).await?;
    let export_a = store.export_snapshot().await?;
    let export_b = store.export_snapshot().await?;
    let identity_a = fingerprint_snapshot(&export_a)?;
    let identity_b = fingerprint_snapshot(&export_b)?;

    let reloaded = SamyamaGraphStore::new();
    reloaded.import_snapshot(&export_a).await?;
    let export_reloaded = reloaded.export_snapshot().await?;
    let identity_reloaded = fingerprint_snapshot(&export_reloaded)?;

    let revision = reloaded
        .get_nodes_by_label("PaperRevision")
        .await
        .into_iter()
        .next()
        .ok_or("real snapshot has no PaperRevision")?;
    reloaded
        .set_node_property_string(revision, "semantic_probe", "changed".into())
        .await?;
    let mutated = fingerprint_snapshot(&reloaded.export_snapshot().await?)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "snapshot-identity-probe.v1",
            "input_path": path,
            "export_a": identity_a,
            "export_b": identity_b,
            "reloaded": identity_reloaded,
            "mutated": mutated,
            "unchanged_semantic_stable": identity_a.semantic_sha256 == identity_b.semantic_sha256
                && identity_a.semantic_sha256 == identity_reloaded.semantic_sha256,
            "physical_exports_differ": identity_a.physical_sha256 != identity_b.physical_sha256,
            "mutation_invalidates_semantic": identity_a.semantic_sha256 != mutated.semantic_sha256,
        }))?
    );
    Ok(())
}
