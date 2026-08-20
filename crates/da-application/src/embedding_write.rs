//! Shared add-or-replace semantics for node embedding vectors (M418).
//!
//! Every vector writer (journey Paper via the EmbedStale scheduler seam,
//! Entity nodes, and ingest content nodes) goes through one helper so the
//! semantics can never drift per call site:
//! - add on first attach (no stored vector),
//! - sanctioned same-node `replace_vector` (M416 / D277) only when the
//!   `embedding_fingerprint` changed,
//! - no-op on an equal replay so repeated runs never churn the vector store.
//!
//! A `embedding_sha256` property is stamped alongside every write, mirroring
//! the Paper journey path.

use da_ports::graph_store::DirectGraphStore;

/// Result of a sanctioned node-vector write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeEmbedWrite {
    #[allow(dead_code)]
    Added,
    Replaced,
    Unchanged,
}

/// Add-or-replace a node's embedding under `(label, "embedding")`.
///
/// Compares `embedding_fingerprint` against the stored `embedding_sha256`
/// property; replaces via `replace_vector` only on change, adds when absent,
/// and reports `Unchanged` on an equal replay.
pub async fn write_node_embedding(
    graph_store: &dyn DirectGraphStore,
    label: &str,
    node_id: u64,
    vector: Vec<f32>,
) -> NodeEmbedWrite {
    let fingerprint = crate::ingest::embedding_fingerprint(&vector);
    let existing = graph_store
        .get_node_property_string(node_id, "embedding_sha256")
        .await;
    match existing {
        Some(stored) if stored == fingerprint => NodeEmbedWrite::Unchanged,
        Some(_stored) => {
            match graph_store
                .replace_vector(label, "embedding", node_id, vector.clone())
                .await
            {
                Ok(()) => {
                    let _ = graph_store
                        .set_node_property_string(node_id, "embedding_sha256", fingerprint)
                        .await;
                    NodeEmbedWrite::Replaced
                }
                Err(_absent) => {
                    // A sha stamp without a stored vector (partial state): add.
                    let _ = graph_store
                        .add_vector(label, "embedding", node_id, vector)
                        .await;
                    let _ = graph_store
                        .set_node_property_string(node_id, "embedding_sha256", fingerprint)
                        .await;
                    NodeEmbedWrite::Replaced
                }
            }
        }
        None => match graph_store
            .add_vector(label, "embedding", node_id, vector.clone())
            .await
        {
            Ok(()) => {
                let _ = graph_store
                    .set_node_property_string(node_id, "embedding_sha256", fingerprint)
                    .await;
                NodeEmbedWrite::Added
            }
            Err(_conflict) => {
                // A vector without a sha stamp (legacy data): replace it with
                // the new model output and stamp the sha.
                let _ = graph_store
                    .replace_vector(label, "embedding", node_id, vector)
                    .await;
                let _ = graph_store
                    .set_node_property_string(node_id, "embedding_sha256", fingerprint)
                    .await;
                NodeEmbedWrite::Replaced
            }
        },
    }
}
