//! Universal graph-mutation observation and compare-before-write decorator.
//!
//! ACAP replay proof needs physical storage outcomes, not business-level
//! "objects created" counters. This module is generic graph infrastructure and
//! intentionally has no dependency on any `da-*` crate.

use crate::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationDisposition {
    Applied,
    Unchanged,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationFamily {
    Node,
    Property,
    Edge,
    Vector,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationFamilyCounters {
    pub attempted: u64,
    pub applied: u64,
    pub unchanged: u64,
    pub rejected: u64,
}

impl MutationFamilyCounters {
    fn record(&mut self, disposition: MutationDisposition) {
        self.attempted += 1;
        match disposition {
            MutationDisposition::Applied => self.applied += 1,
            MutationDisposition::Unchanged => self.unchanged += 1,
            MutationDisposition::Rejected => self.rejected += 1,
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.attempted += other.attempted;
        self.applied += other.applied;
        self.unchanged += other.unchanged;
        self.rejected += other.rejected;
    }

    pub fn partition_is_valid(self) -> bool {
        self.attempted == self.applied + self.unchanged + self.rejected
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationCounters {
    pub node: MutationFamilyCounters,
    pub property: MutationFamilyCounters,
    pub edge: MutationFamilyCounters,
    pub vector: MutationFamilyCounters,
    /// Operations that may mutate storage but cannot be classified at the
    /// direct-write boundary (write-capable query, index creation, import).
    pub unobserved_operations: u64,
}

impl MutationCounters {
    pub fn attempted(self) -> u64 {
        self.node.attempted + self.property.attempted + self.edge.attempted + self.vector.attempted
    }

    pub fn applied(self) -> u64 {
        self.node.applied + self.property.applied + self.edge.applied + self.vector.applied
    }

    pub fn unchanged(self) -> u64 {
        self.node.unchanged + self.property.unchanged + self.edge.unchanged + self.vector.unchanged
    }

    pub fn rejected(self) -> u64 {
        self.node.rejected + self.property.rejected + self.edge.rejected + self.vector.rejected
    }

    pub fn replay_passes(self) -> bool {
        self.partition_is_valid()
            && self.unobserved_operations == 0
            && self.attempted() > 0
            && self.unchanged() > 0
            && self.applied() == 0
            && self.rejected() == 0
    }

    pub fn merge(&mut self, other: Self) {
        self.node.merge(other.node);
        self.property.merge(other.property);
        self.edge.merge(other.edge);
        self.vector.merge(other.vector);
        self.unobserved_operations += other.unobserved_operations;
    }

    pub fn partition_is_valid(self) -> bool {
        self.node.partition_is_valid()
            && self.property.partition_is_valid()
            && self.edge.partition_is_valid()
            && self.vector.partition_is_valid()
            && self.attempted() == self.applied() + self.unchanged() + self.rejected()
    }

    fn family_mut(&mut self, family: MutationFamily) -> &mut MutationFamilyCounters {
        match family {
            MutationFamily::Node => &mut self.node,
            MutationFamily::Property => &mut self.property,
            MutationFamily::Edge => &mut self.edge,
            MutationFamily::Vector => &mut self.vector,
        }
    }

    fn record(&mut self, family: MutationFamily, disposition: MutationDisposition) {
        self.family_mut(family).record(disposition);
    }
}

/// Decorates any direct graph store with physical mutation observations.
///
/// Property writes are serialized and compare-before-write. Equal writes are
/// classified `Unchanged` and are not forwarded to the inner store. Node/edge
/// creates and vectors remain `Applied` on success because the universal port
/// has no identity/upsert/read contract for them yet.
#[derive(Clone)]
pub struct InstrumentedGraphStore {
    inner: Arc<dyn DirectGraphStore>,
    counters: Arc<Mutex<MutationCounters>>,
    mutation_lock: Arc<AsyncMutex<()>>,
}

impl InstrumentedGraphStore {
    pub fn new(inner: Arc<dyn DirectGraphStore>) -> Self {
        Self {
            inner,
            counters: Arc::new(Mutex::new(MutationCounters::default())),
            mutation_lock: Arc::new(AsyncMutex::new(())),
        }
    }

    pub async fn snapshot(&self) -> MutationCounters {
        let _guard = self.mutation_lock.lock().await;
        *self.counters.lock().expect("mutation counter poisoned")
    }

    pub async fn reset(&self) -> MutationCounters {
        let _guard = self.mutation_lock.lock().await;
        let mut counters = self.counters.lock().expect("mutation counter poisoned");
        std::mem::take(&mut *counters)
    }

    fn invalidate_observation(&self) {
        self.counters
            .lock()
            .expect("mutation counter poisoned")
            .unobserved_operations += 1;
    }

    fn record(&self, family: MutationFamily, disposition: MutationDisposition) {
        self.counters
            .lock()
            .expect("mutation counter poisoned")
            .record(family, disposition);
    }

    fn finish<T>(
        &self,
        family: MutationFamily,
        result: Result<T, GraphStoreError>,
    ) -> Result<T, GraphStoreError> {
        self.record(
            family,
            if result.is_ok() {
                MutationDisposition::Applied
            } else {
                MutationDisposition::Rejected
            },
        );
        result
    }
}

#[async_trait]
impl GraphStore for InstrumentedGraphStore {
    async fn query(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        let _guard = self.mutation_lock.lock().await;
        self.invalidate_observation();
        self.inner.query(graph, cypher).await
    }

    async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.inner.query_readonly(graph, cypher).await
    }

    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()> {
        let _guard = self.mutation_lock.lock().await;
        self.invalidate_observation();
        self.inner
            .create_vector_index(label, property, dimensions, metric)
            .await
    }

    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()> {
        let _guard = self.mutation_lock.lock().await;
        self.invalidate_observation();
        self.inner.create_property_index(label, property).await
    }

    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        self.inner
            .vector_search(label, property, query_vector, k)
            .await
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        self.inner.export_snapshot().await
    }

    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()> {
        let _guard = self.mutation_lock.lock().await;
        self.invalidate_observation();
        self.inner.import_snapshot(data).await
    }

    async fn health(&self) -> GraphResult<bool> {
        self.inner.health().await
    }
}

#[async_trait]
impl DirectGraphStore for InstrumentedGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        self.finish(MutationFamily::Node, self.inner.create_node(label).await)
    }

    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self.inner.get_node_property_string(node_id, key).await == Some(value.clone()) {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner
                .set_node_property_string(node_id, key, value)
                .await,
        )
    }

    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self.inner.get_node_property_int(node_id, key).await == Some(value) {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner.set_node_property_int(node_id, key, value).await,
        )
    }

    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self.inner.get_node_property_bool(node_id, key).await == Some(value) {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner.set_node_property_bool(node_id, key, value).await,
        )
    }

    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self
            .inner
            .get_node_property_float(node_id, key)
            .await
            .is_some_and(|current| current.to_bits() == value.to_bits())
        {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner
                .set_node_property_float(node_id, key, value)
                .await,
        )
    }

    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        self.finish(
            MutationFamily::Edge,
            self.inner.create_edge(source, target, edge_type).await,
        )
    }

    async fn set_edge_property_float(
        &self,
        edge_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self
            .inner
            .get_edge_property_float(edge_id, key)
            .await
            .is_some_and(|current| current.to_bits() == value.to_bits())
        {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner
                .set_edge_property_float(edge_id, key, value)
                .await,
        )
    }

    async fn add_vector(
        &self,
        label: &str,
        property: &str,
        node_id: u64,
        vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        self.finish(
            MutationFamily::Vector,
            self.inner
                .add_vector(label, property, node_id, vector)
                .await,
        )
    }

    async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        self.inner
            .vector_search_direct(label, property, query, k)
            .await
    }

    async fn node_count(&self) -> usize {
        self.inner.node_count().await
    }

    async fn edge_count(&self) -> usize {
        self.inner.edge_count().await
    }

    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        self.inner
            .find_node_by_string_property(label, key, value)
            .await
    }

    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner.get_incoming_edges(node_id).await
    }

    async fn get_outgoing_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner.get_outgoing_edges(node_id).await
    }

    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.inner.get_node_property_string(node_id, key).await
    }

    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        self.inner.get_node_property_int(node_id, key).await
    }

    async fn get_node_property_bool(&self, node_id: u64, key: &str) -> Option<bool> {
        self.inner.get_node_property_bool(node_id, key).await
    }

    async fn get_node_property_float(&self, node_id: u64, key: &str) -> Option<f64> {
        self.inner.get_node_property_float(node_id, key).await
    }

    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.inner.get_nodes_by_label(label).await
    }

    async fn get_node_labels(&self, node_id: u64) -> Vec<String> {
        self.inner.get_node_labels(node_id).await
    }

    async fn get_all_node_ids(&self) -> Vec<u64> {
        self.inner.get_all_node_ids().await
    }

    async fn get_edges_between(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Vec<(u64, String)> {
        self.inner
            .get_edges_between(source, target, edge_type)
            .await
    }

    async fn get_edge_property_string(&self, edge_id: u64, key: &str) -> Option<String> {
        self.inner.get_edge_property_string(edge_id, key).await
    }

    async fn set_edge_property_string(
        &self,
        edge_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), GraphStoreError> {
        let _guard = self.mutation_lock.lock().await;
        if self.inner.get_edge_property_string(edge_id, key).await == Some(value.to_string()) {
            self.record(MutationFamily::Property, MutationDisposition::Unchanged);
            return Ok(());
        }
        self.finish(
            MutationFamily::Property,
            self.inner
                .set_edge_property_string(edge_id, key, value)
                .await,
        )
    }

    async fn get_edge_property_float(&self, edge_id: u64, key: &str) -> Option<f64> {
        self.inner.get_edge_property_float(edge_id, key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockGraphStore;

    fn instrumented() -> InstrumentedGraphStore {
        InstrumentedGraphStore::new(Arc::new(MockGraphStore::new()))
    }

    #[test]
    fn mutation_counter_merge_preserves_partition() {
        let mut total = MutationCounters {
            property: MutationFamilyCounters {
                attempted: 2,
                applied: 1,
                unchanged: 1,
                rejected: 0,
            },
            ..Default::default()
        };
        total.merge(MutationCounters {
            node: MutationFamilyCounters {
                attempted: 1,
                applied: 0,
                unchanged: 1,
                rejected: 0,
            },
            ..Default::default()
        });
        assert_eq!(total.attempted(), 3);
        assert_eq!(total.applied(), 1);
        assert_eq!(total.unchanged(), 2);
        assert!(total.partition_is_valid());
    }

    #[test]
    fn mutation_counter_serde_roundtrip() {
        let counters = MutationCounters {
            property: MutationFamilyCounters {
                attempted: 3,
                applied: 1,
                unchanged: 1,
                rejected: 1,
            },
            ..Default::default()
        };
        crate::roundtrip_json(&counters);
        assert!(counters.partition_is_valid());
    }

    #[tokio::test]
    async fn decorator_preserves_full_graph_store_contract() {
        let store = instrumented();
        crate::contract::graph_store_contract_suite(&store).await;
        assert!(store.snapshot().await.partition_is_valid());
    }

    #[tokio::test]
    async fn equal_property_sets_are_observed_but_not_applied() {
        let store = instrumented();
        let node = store.create_node("Paper").await.unwrap();
        store.reset().await;
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        let counters = store.snapshot().await;
        assert_eq!(counters.property.attempted, 2);
        assert_eq!(counters.property.applied, 1);
        assert_eq!(counters.property.unchanged, 1);
        assert!(counters.partition_is_valid());
    }

    #[tokio::test]
    async fn typed_node_and_edge_properties_share_compare_set_semantics() {
        let store = instrumented();
        let source = store.create_node("Paper").await.unwrap();
        let target = store.create_node("Section").await.unwrap();
        let edge = store.create_edge(source, target, "HAS_PART").await.unwrap();
        store.reset().await;
        for _ in 0..2 {
            store
                .set_node_property_int(source, "count", 1)
                .await
                .unwrap();
            store
                .set_node_property_bool(source, "locked", true)
                .await
                .unwrap();
            store
                .set_node_property_float(source, "score", 0.5)
                .await
                .unwrap();
            store
                .set_edge_property_string(edge, "created_at", "2026-08-11")
                .await
                .unwrap();
            store
                .set_edge_property_float(edge, "weight", 1.0)
                .await
                .unwrap();
        }
        let counters = store.snapshot().await;
        assert_eq!(counters.property.attempted, 10);
        assert_eq!(counters.property.applied, 5);
        assert_eq!(counters.property.unchanged, 5);
        assert!(counters.partition_is_valid());
    }

    #[tokio::test]
    async fn creates_and_vectors_remain_physical_applied_mutations() {
        let store = instrumented();
        let source = store.create_node("Paper").await.unwrap();
        let target = store.create_node("Section").await.unwrap();
        store.create_edge(source, target, "HAS_PART").await.unwrap();
        store
            .add_vector("Paper", "embedding", source, vec![0.1, 0.2])
            .await
            .unwrap();
        let counters = store.snapshot().await;
        assert_eq!(counters.node.applied, 2);
        assert_eq!(counters.edge.applied, 1);
        assert_eq!(counters.vector.applied, 1);
        assert_eq!(counters.applied(), 4);
        assert!(!counters.replay_passes());
    }

    #[tokio::test]
    async fn index_and_snapshot_import_invalidate_replay_observation() {
        let store = instrumented();
        let _ = store.create_property_index("Paper", "vid").await;
        let _ = store.import_snapshot(b"not-a-real-snapshot").await;
        let counters = store.snapshot().await;
        assert_eq!(counters.unobserved_operations, 2);
        assert!(!counters.replay_passes());
    }

    #[tokio::test]
    async fn write_capable_query_invalidates_replay_observation() {
        let store = instrumented();
        let node = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        store.reset().await;
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        store.query("default", "RETURN 1").await.unwrap();
        let counters = store.snapshot().await;
        assert_eq!(counters.unobserved_operations, 1);
        assert!(!counters.replay_passes());
    }

    #[tokio::test]
    async fn unchanged_only_replay_passes_and_empty_observation_does_not() {
        let store = instrumented();
        let node = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        store.reset().await;
        assert!(!store.snapshot().await.replay_passes());
        store
            .set_node_property_string(node, "title", "ACAP".into())
            .await
            .unwrap();
        let replay = store.snapshot().await;
        assert!(replay.replay_passes());
        assert_eq!(replay.unchanged(), 1);
    }
}
