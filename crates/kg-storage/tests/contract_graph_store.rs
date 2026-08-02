//! Port contract suite for GraphStore + DirectGraphStore.
//!
//! ADR-051 Contour 4: a single semantic specification run against
//! every implementation (Mock, Samyama, future RuVector).

#[cfg(test)]
mod tests {
    use kg_storage::mock::MockGraphStore;
    use kg_storage::traits::{DirectGraphStore, GraphStore};

    fn graph_store_contract_suite<T>(store: &T)
    where
        T: DirectGraphStore + GraphStore,
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 1. create_node succeeds
            let id = store.create_node("Paper").await.unwrap();
            // ID can be 0 or positive — just verify it succeeds

            // 2. create_node returns unique IDs
            let id2 = store.create_node("Paper").await.unwrap();
            assert_ne!(id, id2, "create_node must return unique IDs");

            // 3. set and get string property
            let nid = store.create_node("Paper").await.unwrap();
            store
                .set_node_property_string(nid, "title", "Test".into())
                .await
                .unwrap();
            let val = store.get_node_property_string(nid, "title").await;
            assert_eq!(val.as_deref(), Some("Test"));

            // 4. set and get int property
            store
                .set_node_property_int(nid, "count", 42)
                .await
                .unwrap();
            assert_eq!(store.get_node_property_int(nid, "count").await, Some(42));

            // 5. set and get bool property
            store
                .set_node_property_bool(nid, "flag", true)
                .await
                .unwrap();
            assert_eq!(
                store.get_node_property_bool(nid, "flag").await,
                Some(true)
            );

            // 6. set and get float property
            store
                .set_node_property_float(nid, "score", 3.14)
                .await
                .unwrap();
            let f = store.get_node_property_float(nid, "score").await;
            assert!(f.is_some());
            assert!((f.unwrap() - 3.14).abs() < 1e-6);

            // 7. create_edge returns ID
            let a = store.create_node("Paper").await.unwrap();
            let b = store.create_node("Author").await.unwrap();
            let eid = store.create_edge(a, b, "AUTHORED_BY").await.unwrap();
            // Edge ID can be 0 or positive

            // 8. get_outgoing_edges
            let c = store.create_node("Paper").await.unwrap();
            let d = store.create_node("Entity").await.unwrap();
            store.create_edge(c, d, "MENTIONS").await.unwrap();
            let outgoing = store.get_outgoing_edges(c).await;
            assert!(!outgoing.is_empty(), "must have outgoing edges");
            assert_eq!(outgoing[0].1, "MENTIONS");

            // 9. get_incoming_edges
            let incoming = store.get_incoming_edges(d).await;
            assert!(!incoming.is_empty(), "must have incoming edges");

            // 10. node_count increases after creates
            let initial = store.node_count().await;
            store.create_node("Extra").await.unwrap();
            store.create_node("Extra").await.unwrap();
            let after = store.node_count().await;
            assert_eq!(after, initial + 2, "node_count must increase by 2");

            // 11. find_node_by_string_property
            let target = store.create_node("Paper").await.unwrap();
            store
                .set_node_property_string(target, "arxiv_id", "2507.19457".into())
                .await
                .unwrap();
            let found = store
                .find_node_by_string_property("Paper", "arxiv_id", "2507.19457")
                .await;
            assert_eq!(
                found,
                Some(target),
                "find_node_by_string_property must find the node"
            );

            // 12. missing property returns None
            let empty_id = store.create_node("Paper").await.unwrap();
            let missing = store
                .get_node_property_string(empty_id, "nonexistent")
                .await;
            assert_eq!(missing, None, "Missing property must return None");
        });
    }

    #[test]
    fn mock_passes_graph_store_contract() {
        let store = MockGraphStore::new();
        graph_store_contract_suite(&store);
    }
}
