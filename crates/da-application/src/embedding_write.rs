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

use da_ports::graph_store::{DirectGraphStore, GraphResult};

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
///
/// M429: store failures propagate. The sha stamp is part of the write
/// contract (an unstamped vector would make the Unchanged fast-path skip
/// needed rewrites forever), and a failed primary write whose fallback also
/// fails is an error — the helper never reports success for a write that
/// did not happen.
pub async fn write_node_embedding(
    graph_store: &dyn DirectGraphStore,
    label: &str,
    node_id: u64,
    vector: Vec<f32>,
) -> GraphResult<NodeEmbedWrite> {
    let fingerprint = crate::ingest::embedding_fingerprint(&vector);
    let existing = graph_store
        .get_node_property_string(node_id, "embedding_sha256")
        .await;
    match existing {
        Some(stored) if stored == fingerprint => Ok(NodeEmbedWrite::Unchanged),
        Some(_stored) => {
            match graph_store
                .replace_vector(label, "embedding", node_id, vector.clone())
                .await
            {
                Ok(()) => {
                    stamp_sha(graph_store, node_id, &fingerprint).await?;
                    Ok(NodeEmbedWrite::Replaced)
                }
                Err(_absent) => {
                    // A sha stamp without a stored vector (partial state): add.
                    graph_store
                        .add_vector(label, "embedding", node_id, vector)
                        .await?;
                    stamp_sha(graph_store, node_id, &fingerprint).await?;
                    Ok(NodeEmbedWrite::Replaced)
                }
            }
        }
        None => match graph_store
            .add_vector(label, "embedding", node_id, vector.clone())
            .await
        {
            Ok(()) => {
                stamp_sha(graph_store, node_id, &fingerprint).await?;
                Ok(NodeEmbedWrite::Added)
            }
            Err(_conflict) => {
                // A vector without a sha stamp (legacy data): replace it with
                // the new model output and stamp the sha.
                graph_store
                    .replace_vector(label, "embedding", node_id, vector)
                    .await?;
                stamp_sha(graph_store, node_id, &fingerprint).await?;
                Ok(NodeEmbedWrite::Replaced)
            }
        },
    }
}

/// Stamp `embedding_sha256`; a failed stamp is an error (M429) so the sha
/// can never silently drift from the stored vector.
async fn stamp_sha(
    graph_store: &dyn DirectGraphStore,
    node_id: u64,
    fingerprint: &str,
) -> GraphResult<()> {
    graph_store
        .set_node_property_string(node_id, "embedding_sha256", fingerprint.to_string())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_replace_noop_semantics_unchanged() {
        let store = kg_storage::MockGraphStore::new();
        let node = store.create_node("Entity").await.unwrap();
        let w1 = write_node_embedding(&store, "Entity", node, vec![0.1, 0.2])
            .await
            .unwrap();
        assert_eq!(w1, NodeEmbedWrite::Added);
        let w2 = write_node_embedding(&store, "Entity", node, vec![0.1, 0.2])
            .await
            .unwrap();
        assert_eq!(w2, NodeEmbedWrite::Unchanged);
        let w3 = write_node_embedding(&store, "Entity", node, vec![7.0, 8.0])
            .await
            .unwrap();
        assert_eq!(w3, NodeEmbedWrite::Replaced);
        assert_eq!(
            store
                .get_node_property_string(node, "embedding_sha256")
                .await
                .as_deref(),
            Some(crate::ingest::embedding_fingerprint(&[7.0, 8.0]).as_str())
        );
    }

    /// Thin failing wrapper: delegates everything to the inner mock except
    /// the vector writes (add and replace both fail).
    struct FailingVectorStore {
        inner: kg_storage::MockGraphStore,
        fail_writes: bool,
        fail_sha: bool,
    }

    impl FailingVectorStore {
        fn new(inner: kg_storage::MockGraphStore) -> Self {
            Self {
                inner,
                fail_writes: false,
                fail_sha: false,
            }
        }
        fn failing_writes(mut self) -> Self {
            self.fail_writes = true;
            self
        }
        fn failing_sha(mut self) -> Self {
            self.fail_sha = true;
            self
        }
    }

    /// Macro-free delegation: implement both traits by hand but only the
    /// vector-write and sha-stamp methods can fail.
    #[async_trait::async_trait]
    impl da_ports::graph_store::GraphStore for FailingVectorStore {
        async fn query(&self, g: &str, c: &str) -> GraphResult<da_ports::graph_store::QueryResult> {
            self.inner.query(g, c).await
        }
        async fn query_readonly(
            &self,
            g: &str,
            c: &str,
        ) -> GraphResult<da_ports::graph_store::QueryResult> {
            self.inner.query_readonly(g, c).await
        }
        async fn create_vector_index(
            &self,
            l: &str,
            p: &str,
            d: usize,
            m: da_ports::graph_store::VectorMetric,
        ) -> GraphResult<()> {
            self.inner.create_vector_index(l, p, d, m).await
        }
        async fn create_property_index(&self, l: &str, p: &str) -> GraphResult<()> {
            self.inner.create_property_index(l, p).await
        }
        async fn vector_search(
            &self,
            l: &str,
            p: &str,
            q: &[f32],
            k: usize,
        ) -> GraphResult<Vec<da_ports::graph_store::VectorSearchResult>> {
            self.inner.vector_search(l, p, q, k).await
        }
        async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
            self.inner.export_snapshot().await
        }
        async fn import_snapshot(&self, d: &[u8]) -> GraphResult<()> {
            self.inner.import_snapshot(d).await
        }
        async fn health(&self) -> GraphResult<bool> {
            self.inner.health().await
        }
    }

    #[async_trait::async_trait]
    impl DirectGraphStore for FailingVectorStore {
        async fn create_node(&self, l: &str) -> Result<u64, GraphStoreError> {
            self.inner.create_node(l).await
        }
        async fn set_node_property_string(
            &self,
            n: u64,
            k: &str,
            v: String,
        ) -> Result<(), GraphStoreError> {
            if self.fail_sha {
                return Err(GraphStoreError::Storage("forced sha-stamp failure".into()));
            }
            self.inner.set_node_property_string(n, k, v).await
        }
        async fn set_node_property_int(
            &self,
            n: u64,
            k: &str,
            v: i64,
        ) -> Result<(), GraphStoreError> {
            self.inner.set_node_property_int(n, k, v).await
        }
        async fn set_node_property_bool(
            &self,
            n: u64,
            k: &str,
            v: bool,
        ) -> Result<(), GraphStoreError> {
            self.inner.set_node_property_bool(n, k, v).await
        }
        async fn set_node_property_float(
            &self,
            n: u64,
            k: &str,
            v: f64,
        ) -> Result<(), GraphStoreError> {
            self.inner.set_node_property_float(n, k, v).await
        }
        async fn create_edge(&self, s: u64, t: u64, e: &str) -> Result<u64, GraphStoreError> {
            self.inner.create_edge(s, t, e).await
        }
        async fn add_vector(
            &self,
            _l: &str,
            _p: &str,
            _n: u64,
            _v: Vec<f32>,
        ) -> Result<(), GraphStoreError> {
            if self.fail_writes {
                return Err(GraphStoreError::Vector("forced add failure".into()));
            }
            self.inner.add_vector(_l, _p, _n, _v).await
        }
        async fn replace_vector(
            &self,
            _l: &str,
            _p: &str,
            _n: u64,
            _v: Vec<f32>,
        ) -> Result<(), GraphStoreError> {
            if self.fail_writes {
                return Err(GraphStoreError::Vector("forced replace failure".into()));
            }
            self.inner.replace_vector(_l, _p, _n, _v).await
        }
        async fn vector_search_direct(
            &self,
            l: &str,
            p: &str,
            q: &[f32],
            k: usize,
        ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
            self.inner.vector_search_direct(l, p, q, k).await
        }
        async fn node_count(&self) -> usize {
            self.inner.node_count().await
        }
        async fn edge_count(&self) -> usize {
            self.inner.edge_count().await
        }
        async fn find_node_by_string_property(&self, l: &str, k: &str, v: &str) -> Option<u64> {
            self.inner.find_node_by_string_property(l, k, v).await
        }
        async fn get_incoming_edges(&self, n: u64) -> Vec<(u64, String)> {
            self.inner.get_incoming_edges(n).await
        }
        async fn get_outgoing_edges(&self, n: u64) -> Vec<(u64, String)> {
            self.inner.get_outgoing_edges(n).await
        }
        async fn get_node_property_string(&self, n: u64, k: &str) -> Option<String> {
            self.inner.get_node_property_string(n, k).await
        }
        async fn get_node_property_int(&self, n: u64, k: &str) -> Option<i64> {
            self.inner.get_node_property_int(n, k).await
        }
        async fn get_nodes_by_label(&self, l: &str) -> Vec<u64> {
            self.inner.get_nodes_by_label(l).await
        }
        async fn get_edge_property_string(&self, e: u64, k: &str) -> Option<String> {
            self.inner.get_edge_property_string(e, k).await
        }
    }

    /// M429: when both the primary write and its fallback fail the helper
    /// must return Err. The pre-M429 code returned `Replaced` here — a fake
    /// success that made callers emit node_embedding_replaced events for a
    /// write that never happened.
    #[tokio::test]
    async fn dual_write_failure_is_err_not_fake_replaced() {
        let inner = kg_storage::MockGraphStore::new();
        let node = inner.create_node("Entity").await.unwrap();
        let store = FailingVectorStore::new(inner).failing_writes();
        let result = write_node_embedding(&store, "Entity", node, vec![0.1, 0.2]).await;
        let err = result.expect_err("dual-write failure must be Err, not Replaced");
        assert!(
            err.to_string().contains("forced"),
            "error must carry the underlying store failure: {err}"
        );

        // Existing stale sha + failing writes: the replace/add pair fails too.
        let inner = kg_storage::MockGraphStore::new();
        let n2 = inner.create_node("Entity").await.unwrap();
        inner
            .set_node_property_string(n2, "embedding_sha256", "stale".to_string())
            .await
            .unwrap();
        let store = FailingVectorStore::new(inner).failing_writes();
        let result = write_node_embedding(&store, "Entity", n2, vec![1.0]).await;
        assert!(
            result.is_err(),
            "replace failure whose add fallback also fails must be Err"
        );
    }

    /// M429: a failed sha stamp must error so embedding_sha256 can never
    /// silently drift from the stored vector.
    #[tokio::test]
    async fn failed_sha_stamp_is_err_not_fake_added() {
        let inner = kg_storage::MockGraphStore::new();
        let node = inner.create_node("Entity").await.unwrap();
        let store = FailingVectorStore::new(inner).failing_sha();
        let result = write_node_embedding(&store, "Entity", node, vec![0.5]).await;
        let err = result.expect_err("failed sha stamp must be Err, not Added");
        assert!(
            err.to_string().contains("forced sha-stamp failure"),
            "{err}"
        );
    }
}
