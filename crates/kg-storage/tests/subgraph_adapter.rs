//! TDD integration tests for subgraph_view_from adapter (ADR-051 Contour 4).
//!
//! Pins directed k-hop neighborhood semantics against MockGraphStore before
//! implementation (T01 red). Expected fixture:
//!
//! ```text
//!     A -[CITES]-> B -[CITES]-> C -[CITES]-> D
//!     A -[RELATED_TO]-> X   (noise edge_type)
//! ```
//!
//! Contract (T02):
//! - empty seeds → empty GraphView
//! - all missing seeds → empty (ignore unknown)
//! - k=0 → only existing seeds, no expansion
//! - k=1 / k=2 → exact directed membership
//! - edge_type filter excludes other types on expand + project
//! - optional label drops seeds outside label set
//! - same inputs twice → identical sorted node sets / indexes
//! - weight_property preserves Mock float weights (honesty path)

#![cfg(feature = "algorithms")]

use kg_storage::adapter::subgraph_view_from;
use kg_storage::mock::MockGraphStore;
use kg_storage::traits::DirectGraphStore;

/// Build directed chain A→B→C→D with CITES, plus A→X RELATED_TO noise.
async fn chain_fixture() -> (MockGraphStore, u64, u64, u64, u64, u64) {
    let store = MockGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    let c = store.create_node("Paper").await.unwrap();
    let d = store.create_node("Paper").await.unwrap();
    let x = store.create_node("Paper").await.unwrap();
    store.create_edge(a, b, "CITES").await.unwrap();
    store.create_edge(b, c, "CITES").await.unwrap();
    store.create_edge(c, d, "CITES").await.unwrap();
    store.create_edge(a, x, "RELATED_TO").await.unwrap();
    (store, a, b, c, d, x)
}

fn sorted_nodes(view: &samyama_graph_algorithms::GraphView) -> Vec<u64> {
    let mut ids = view.index_to_node.clone();
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn empty_seeds_yields_empty_view() {
    let store = MockGraphStore::new();
    let _ = store.create_node("Paper").await.unwrap();

    let view = subgraph_view_from(&store, &[], 2, None, Some("CITES"), None)
        .await
        .expect("empty seeds must succeed");

    assert_eq!(view.node_count, 0, "empty seeds → empty GraphView");
    assert!(view.index_to_node.is_empty());
    assert!(view.node_to_index.is_empty());
}

#[tokio::test]
async fn all_missing_seeds_yields_empty_view() {
    let store = MockGraphStore::new();
    // No nodes created; seeds reference nothing.
    let view = subgraph_view_from(&store, &[999_001, 999_002], 1, None, Some("CITES"), None)
        .await
        .expect("missing seeds must succeed (ignored)");

    assert_eq!(
        view.node_count, 0,
        "all missing seeds → empty GraphView (ignore unknown)"
    );
}

#[tokio::test]
async fn k_zero_keeps_only_existing_seeds() {
    let (store, a, b, _c, _d, _x) = chain_fixture().await;

    let view = subgraph_view_from(&store, &[a, b, 999_999], 0, None, Some("CITES"), None)
        .await
        .expect("k=0 must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(
        nodes,
        vec![a, b],
        "k=0 keeps only existing seeds; no expansion; drop missing"
    );
    assert_eq!(view.node_count, 2);
    // Deterministic ascending CSR indexes.
    assert_eq!(view.index_to_node, nodes);
}

#[tokio::test]
async fn k_one_membership_on_directed_chain() {
    let (store, a, b, c, _d, x) = chain_fixture().await;

    // From A with k=1 CITES: A, B (not X — wrong type; not C — 2 hops).
    let view = subgraph_view_from(&store, &[a], 1, None, Some("CITES"), None)
        .await
        .expect("k=1 must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(
        nodes,
        vec![a, b],
        "k=1 from A via CITES must be {{A,B}}; exclude X (RELATED_TO) and C (2 hops)"
    );
    assert!(!nodes.contains(&c));
    assert!(!nodes.contains(&x));

    // Out-degree of A inside view must be 1 (only B).
    let a_idx = *view.node_to_index.get(&a).expect("A in view");
    let out_deg = view.out_offsets[a_idx + 1] - view.out_offsets[a_idx];
    assert_eq!(out_deg, 1, "projected CITES out-degree of A is 1");
}

#[tokio::test]
async fn k_two_membership_on_directed_chain() {
    let (store, a, b, c, d, x) = chain_fixture().await;

    // From A with k=2 CITES: A, B, C (not D — 3 hops; not X — wrong type).
    let view = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), None)
        .await
        .expect("k=2 must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(
        nodes,
        vec![a, b, c],
        "k=2 from A via CITES must be {{A,B,C}}; exclude D and X"
    );
    assert!(!nodes.contains(&d));
    assert!(!nodes.contains(&x));
}

#[tokio::test]
async fn edge_type_filter_excludes_other_types_on_expand() {
    let (store, a, _b, _c, _d, x) = chain_fixture().await;

    // Expand via RELATED_TO only: A reaches X in 1 hop, not B.
    let view = subgraph_view_from(&store, &[a], 1, None, Some("RELATED_TO"), None)
        .await
        .expect("RELATED_TO filter must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(nodes, vec![a, x], "edge_type RELATED_TO expands only A→X");
}

#[tokio::test]
async fn label_filter_drops_seeds_outside_label_set() {
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    let entity = store.create_node("Entity").await.unwrap();
    // No edges: pure seed-filter case (entity cannot re-enter via expansion).
    let view = subgraph_view_from(&store, &[paper, entity], 0, Some("Paper"), None, None)
        .await
        .expect("label filter must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(
        nodes,
        vec![paper],
        "optional label drops seeds outside get_nodes_by_label set"
    );
    assert!(
        !nodes.contains(&entity),
        "Entity seed must be dropped by label=Paper"
    );
}

#[tokio::test]
async fn same_inputs_twice_yield_identical_sorted_indexes() {
    let (store, a, b, c, _d, _x) = chain_fixture().await;

    let v1 = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), None)
        .await
        .unwrap();
    let v2 = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), None)
        .await
        .unwrap();

    assert_eq!(v1.node_count, v2.node_count);
    assert_eq!(
        v1.index_to_node, v2.index_to_node,
        "deterministic ascending index_to_node"
    );
    assert_eq!(v1.out_offsets, v2.out_offsets);
    assert_eq!(v1.out_targets, v2.out_targets);
    // CSR indexes must be sorted by node id ascending.
    let mut expected = vec![a, b, c];
    expected.sort_unstable();
    assert_eq!(v1.index_to_node, expected);
}

#[tokio::test]
async fn weight_property_preserves_mock_float_weights() {
    let store = MockGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    let c = store.create_node("Paper").await.unwrap();
    let e1 = store.create_edge(a, b, "CITES").await.unwrap();
    let e2 = store.create_edge(a, c, "CITES").await.unwrap();
    store
        .set_edge_property_float(e1, "weight", 0.5)
        .await
        .unwrap();
    store
        .set_edge_property_float(e2, "weight", 2.0)
        .await
        .unwrap();

    let view = subgraph_view_from(&store, &[a], 1, None, Some("CITES"), Some("weight"))
        .await
        .expect("weight_property path must succeed");

    assert_eq!(view.node_count, 3);
    let a_idx = *view.node_to_index.get(&a).unwrap();
    let start = view.out_offsets[a_idx];
    let end = view.out_offsets[a_idx + 1];
    let weights = view
        .weights
        .as_ref()
        .expect("non-default weights must not fold to None");
    let a_weights: Vec<f64> = weights[start..end].to_vec();
    assert_eq!(a_weights.len(), 2);
    assert!(
        a_weights.contains(&0.5),
        "must preserve weight 0.5, got {a_weights:?}"
    );
    assert!(
        a_weights.contains(&2.0),
        "must preserve weight 2.0, got {a_weights:?}"
    );
}

#[tokio::test]
async fn missing_seed_mixed_with_valid_is_ignored() {
    let (store, a, b, _c, _d, _x) = chain_fixture().await;

    let view = subgraph_view_from(&store, &[a, 42_000], 1, None, Some("CITES"), None)
        .await
        .expect("mixed missing seed must succeed");

    let nodes = sorted_nodes(&view);
    assert_eq!(
        nodes,
        vec![a, b],
        "unknown seed ignored; expand from A only"
    );
}
