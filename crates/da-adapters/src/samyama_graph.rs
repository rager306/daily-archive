//! Samyama Graph adapter — da-adapters facade.
//!
//! ADR-050 Phase B step 2 (M285-z09px0/S09): production implementation
//! migrated to `kg-storage::samyama_adapter` behind the `samyama` feature
//! flag. This module re-exports the public API so existing callers
//! (`da_adapters::samyama_graph::SamyamaGraphStore`) keep working.
//!
//! Tests stay here because they reference `da_domain::relation` (not
//! available in the universal kg-storage crate).

pub use kg_storage::samyama_adapter::{
    EdgeId, EdgeType, Label, NodeId, PropertyValue, RecordBatch, SamyamaGraphStore, SamyamaStore,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_node_direct() {
        let store = SamyamaGraphStore::new();
        let id = store.create_node_direct("Paper").await;
        store
            .set_property_direct(id, "title", PropertyValue::String("Test".into()))
            .await;
        assert_eq!(store.node_count().await, 1);
    }

    #[tokio::test]
    async fn test_create_edge_direct() {
        let store = SamyamaGraphStore::new();
        let a = store.create_node_direct("Author").await;
        let p = store.create_node_direct("Paper").await;
        let _edge = store.create_edge_direct(a, p, "AUTHORED").await.unwrap();
        let neighbors = store.outgoing_neighbors(a).await;
        assert!(!neighbors.is_empty());
    }

    #[tokio::test]
    async fn test_get_outgoing_edges_returns_targets() {
        // Phase 5: get_outgoing_edges must return target nodes for PPR traversal.
        use da_ports::graph_store::DirectGraphStore;
        let store = SamyamaGraphStore::new();
        let paper = store.create_node_direct("Paper").await;
        let entity = store.create_node_direct("Entity").await;
        store
            .create_edge_direct(paper, entity, da_domain::relation::bibliographic::MENTIONS)
            .await
            .unwrap();

        // Paper has outgoing MENTIONS edge to Entity
        let outgoing = store.get_outgoing_edges(paper.0).await;
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].0, entity.0); // target node ID
        assert_eq!(outgoing[0].1, da_domain::relation::bibliographic::MENTIONS);

        // Entity has no outgoing edges
        let entity_outgoing = store.get_outgoing_edges(entity.0).await;
        assert!(entity_outgoing.is_empty());
    }

    #[tokio::test]
    async fn test_health_always_true() {
        use da_ports::graph_store::GraphStore;
        let store = SamyamaGraphStore::new();
        assert!(store.health().await.unwrap());
    }
}
