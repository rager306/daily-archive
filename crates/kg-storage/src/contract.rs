//! Port contract suite for GraphStore + DirectGraphStore.
//!
//! ADR-051 Contour 4: a single semantic specification run against
//! every implementation (Mock, Samyama, future RuVector).
//!
//! Every adapter that implements [`DirectGraphStore`] must pass this
//! suite. Integration tests call [`graph_store_contract_suite`] inside
//! a `#[test]` block; the suite itself is assertion-only and performs
//! no harness setup (the caller owns the runtime/store lifecycle).
//!
//! # Coverage (ADR-051 §6 GraphStore checklist)
//!
//! - unique node IDs from `create_node`
//! - set/get string/int/bool/float node properties
//! - missing property → `None` (not-found semantics)
//! - create_edge returns unique edge IDs
//! - outgoing/incoming edges
//! - node_count / edge_count deltas after creates
//! - find_node_by_string_property label isolation (Paper vs Citation)
//! - get_nodes_by_label filtering
//! - get_edges_between type filter
//! - edge property set/get via set_edge_property_string + get_edge_property_string
//! - health() == Ok(true)
//! - export_snapshot Ok non-empty

use crate::traits::{DirectGraphStore, GraphStore};

/// Run the full GraphStore + DirectGraphStore contract suite against `store`.
///
/// Panics on any contract violation. The caller is responsible for
/// providing a fresh, empty store and an async runtime (tests typically
/// wrap this in `#[tokio::test]` or `Runtime::new().block_on(...)`).
///
/// # Example
///
/// ```no_run
/// use kg_storage::contract::graph_store_contract_suite;
/// use kg_storage::mock::MockGraphStore;
/// use kg_storage::traits::{DirectGraphStore, GraphStore};
///
/// #[tokio::test]
/// async fn mock_passes_graph_store_contract() {
///     let store = MockGraphStore::new();
///     graph_store_contract_suite(&store).await;
/// }
/// ```
pub async fn graph_store_contract_suite<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    test_create_node_returns_unique_ids(store).await;
    test_set_and_get_string_property(store).await;
    test_set_and_get_int_property(store).await;
    test_set_and_get_bool_property(store).await;
    test_set_and_get_float_property(store).await;
    test_missing_property_returns_none(store).await;
    test_create_edge_returns_unique_ids(store).await;
    test_outgoing_and_incoming_edges(store).await;
    test_node_count_increases(store).await;
    test_edge_count_increases(store).await;
    test_find_node_by_string_property_label_isolation(store).await;
    test_get_nodes_by_label(store).await;
    test_get_edges_between_type_filter(store).await;
    test_edge_property_set_and_get(store).await;
    test_edge_property_float_round_trip(store).await;
    test_get_edges_between_returns_real_edge_id(store).await;
    test_health_returns_true(store).await;
    test_export_snapshot_non_empty(store).await;
}

// ─── Individual contract checks ──────────────────────────────────────
//
// Each function covers one clause of the ADR-051 §6 GraphStore checklist.
// Keeping them as separate functions (rather than one long body) makes
// failures self-describing: the panic message includes the function name.

async fn test_create_node_returns_unique_ids<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let id1 = store.create_node("Paper").await.unwrap();
    let id2 = store.create_node("Paper").await.unwrap();
    assert_ne!(id1, id2, "create_node must return unique IDs");
}

async fn test_set_and_get_string_property<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let nid = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(nid, "title", "Test".into())
        .await
        .unwrap();
    let val = store.get_node_property_string(nid, "title").await;
    assert_eq!(val.as_deref(), Some("Test"));
}

async fn test_set_and_get_int_property<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let nid = store.create_node("Paper").await.unwrap();
    store.set_node_property_int(nid, "count", 42).await.unwrap();
    assert_eq!(store.get_node_property_int(nid, "count").await, Some(42));
}

async fn test_set_and_get_bool_property<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let nid = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_bool(nid, "flag", true)
        .await
        .unwrap();
    assert_eq!(store.get_node_property_bool(nid, "flag").await, Some(true));
}

async fn test_set_and_get_float_property<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let nid = store.create_node("Paper").await.unwrap();
    let pi = std::f64::consts::PI;
    store
        .set_node_property_float(nid, "score", pi)
        .await
        .unwrap();
    let f = store.get_node_property_float(nid, "score").await;
    assert!(f.is_some(), "float property must be readable");
    assert!(
        (f.unwrap() - pi).abs() < 1e-12,
        "float value must round-trip"
    );
}

async fn test_missing_property_returns_none<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let nid = store.create_node("Paper").await.unwrap();
    let missing = store.get_node_property_string(nid, "nonexistent").await;
    assert_eq!(missing, None, "missing property must return None");
}

async fn test_create_edge_returns_unique_ids<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Author").await.unwrap();
    let c = store.create_node("Author").await.unwrap();
    let e1 = store.create_edge(a, b, "AUTHORED_BY").await.unwrap();
    let e2 = store.create_edge(a, c, "AUTHORED_BY").await.unwrap();
    assert_ne!(e1, e2, "create_edge must return unique edge IDs");
}

async fn test_outgoing_and_incoming_edges<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let c = store.create_node("Paper").await.unwrap();
    let d = store.create_node("Entity").await.unwrap();
    store.create_edge(c, d, "MENTIONS").await.unwrap();
    let outgoing = store.get_outgoing_edges(c).await;
    assert!(
        !outgoing.is_empty(),
        "must have outgoing edges after create_edge"
    );
    assert_eq!(outgoing[0].1, "MENTIONS");
    let incoming = store.get_incoming_edges(d).await;
    assert!(
        !incoming.is_empty(),
        "target must have incoming edges after create_edge"
    );
}

async fn test_node_count_increases<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let initial = store.node_count().await;
    store.create_node("Extra").await.unwrap();
    let after = store.node_count().await;
    assert_eq!(
        after,
        initial + 1,
        "node_count must increase by 1 after one create_node"
    );
}

async fn test_edge_count_increases<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Entity").await.unwrap();
    let initial = store.edge_count().await;
    store.create_edge(a, b, "MENTIONS").await.unwrap();
    let after = store.edge_count().await;
    assert_eq!(
        after,
        initial + 1,
        "edge_count must increase by 1 after one create_edge"
    );
}

async fn test_find_node_by_string_property_label_isolation<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    // Two nodes with the same property value but different labels —
    // find_node_by_string_property must filter by BOTH label and value.
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "arxiv_id", "2507.19457".into())
        .await
        .unwrap();
    let citation = store.create_node("Citation").await.unwrap();
    store
        .set_node_property_string(citation, "arxiv_id", "2507.19457".into())
        .await
        .unwrap();

    let found_paper = store
        .find_node_by_string_property("Paper", "arxiv_id", "2507.19457")
        .await;
    assert_eq!(
        found_paper,
        Some(paper),
        "find_node_by_string_property must match label AND value"
    );
    let found_citation = store
        .find_node_by_string_property("Citation", "arxiv_id", "2507.19457")
        .await;
    assert_eq!(
        found_citation,
        Some(citation),
        "label isolation: same value, different label must return the labeled node"
    );
}

async fn test_get_nodes_by_label<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let baseline = store.get_nodes_by_label("ContractLabel").await.len();
    let p1 = store.create_node("ContractLabel").await.unwrap();
    let p2 = store.create_node("ContractLabel").await.unwrap();
    store.create_node("OtherLabel").await.unwrap(); // different label
    let papers = store.get_nodes_by_label("ContractLabel").await;
    assert_eq!(
        papers.len(),
        baseline + 2,
        "get_nodes_by_label must return only nodes of the requested label"
    );
    assert!(
        papers.contains(&p1) && papers.contains(&p2),
        "get_nodes_by_label must include newly created nodes"
    );
}

async fn test_get_edges_between_type_filter<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Entity").await.unwrap();

    store.create_edge(a, b, "MENTIONS").await.unwrap();
    store.create_edge(a, b, "CITES").await.unwrap();

    let mentions = store.get_edges_between(a, b, "MENTIONS").await;
    assert_eq!(
        mentions.len(),
        1,
        "get_edges_between must filter by edge type"
    );
    assert_eq!(mentions[0].1, "MENTIONS");
}

async fn test_edge_property_set_and_get<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Entity").await.unwrap();
    let edge_id = store.create_edge(a, b, "MENTIONS").await.unwrap();

    store
        .set_edge_property_string(edge_id, "valid_at", "2024-01-01T00:00:00Z")
        .await
        .unwrap();

    let val = store.get_edge_property_string(edge_id, "valid_at").await;
    assert_eq!(
        val.as_deref(),
        Some("2024-01-01T00:00:00Z"),
        "edge property set via set_edge_property_string must be readable via get_edge_property_string"
    );
}

async fn test_edge_property_float_round_trip<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Entity").await.unwrap();
    let edge_id = store.create_edge(a, b, "MENTIONS").await.unwrap();

    store
        .set_edge_property_float(edge_id, "weight", 0.75)
        .await
        .unwrap();

    let w = store.get_edge_property_float(edge_id, "weight").await;
    assert!(w.is_some(), "edge float property must be readable");
    assert!(
        (w.unwrap() - 0.75).abs() < 1e-12,
        "edge float value must round-trip"
    );
}

async fn test_get_edges_between_returns_real_edge_id<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Entity").await.unwrap();
    let real_edge_id = store.create_edge(a, b, "MENTIONS").await.unwrap();

    let edges = store.get_edges_between(a, b, "MENTIONS").await;
    assert!(!edges.is_empty(), "get_edges_between must find the edge");
    let (found_edge_id, found_type) = &edges[0];
    assert_eq!(found_type, "MENTIONS");
    assert_ne!(
        found_edge_id, &0,
        "get_edges_between must return real edge_id, not fabricated 0"
    );
    assert_eq!(
        found_edge_id, &real_edge_id,
        "get_edges_between must return the same edge_id as create_edge"
    );
}

async fn test_health_returns_true<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let healthy = store.health().await.unwrap();
    assert!(healthy, "health() must return Ok(true) for a healthy store");
}

async fn test_export_snapshot_non_empty<T>(store: &T)
where
    T: DirectGraphStore + GraphStore,
{
    let snapshot = store.export_snapshot().await.unwrap();
    assert!(
        !snapshot.is_empty(),
        "export_snapshot must return Ok(non-empty) bytes"
    );
}
