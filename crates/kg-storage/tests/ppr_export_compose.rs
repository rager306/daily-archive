//! Compose contract: MockGraphStore → k-hop GraphView → ForwardPush PPR.
//!
//! Closes M299 follow-up gap without pulling solver into kg-storage production
//! deps. Requires:
//! - `algorithms` (subgraph_view_from adapter)
//! - dev-dep `kg-algorithms` with feature `ppr` (Cargo.toml)
//!
//! Semantics: ForwardPush is **degree-count** PPR (ignores CSR values).
//! Weighted edges prove export honesty; unweighted fold still runs topology PPR.
//!
//! ADR-051 Contour 4 (contract) + Contour 5 (adapter integration via Mock).

#![cfg(feature = "algorithms")]

use kg_algorithms::{PprError, personalized_pagerank_from_graph_view};
use kg_storage::adapter::subgraph_view_from;
use kg_storage::mock::MockGraphStore;
use kg_storage::traits::DirectGraphStore;

/// Directed path A→B→C with non-uniform CITES weights.
async fn weighted_path_store() -> (MockGraphStore, u64, u64, u64) {
    let store = MockGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    let c = store.create_node("Paper").await.unwrap();
    let e_ab = store.create_edge(a, b, "CITES").await.unwrap();
    let e_bc = store.create_edge(b, c, "CITES").await.unwrap();
    store
        .set_edge_property_float(e_ab, "weight", 0.5)
        .await
        .unwrap();
    store
        .set_edge_property_float(e_bc, "weight", 1.5)
        .await
        .unwrap();
    (store, a, b, c)
}

#[tokio::test]
async fn weighted_subgraph_ppr_ranks_seed() {
    let (store, a, _b, _c) = weighted_path_store().await;

    let view = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), Some("weight"))
        .await
        .expect("weighted k-hop cut");

    assert!(view.node_count >= 2);
    assert!(
        view.weights.is_some(),
        "non-uniform weights must survive fold"
    );
    assert!(
        view.node_to_index.contains_key(&a),
        "seed must be in view map"
    );

    let scores = personalized_pagerank_from_graph_view(&view, &[a], 0.85, 1e-8)
        .expect("PPR on weighted cut");
    assert!(!scores.is_empty());
    assert_eq!(
        scores[0].0, a,
        "seed NodeId should rank first, got {scores:?}"
    );
    assert!(scores[0].1 > 0.0 && scores[0].1.is_finite());
    for (id, s) in &scores {
        assert!(s.is_finite());
        assert!(
            view.node_to_index.contains_key(id),
            "score NodeId {id} must be in view"
        );
    }
}

#[tokio::test]
async fn unweighted_fold_still_runs_topology_ppr() {
    let (store, a, _b, _c) = weighted_path_store().await;

    // weight_property=None → all 1.0 → fold to None.
    let view = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), None)
        .await
        .expect("unweighted cut");

    assert!(
        view.weights.is_none(),
        "all-1.0 path must fold to None (kg-storage honesty)"
    );

    // PPR still works: unit CSR construction is topology-only (ForwardPush
    // ignores values anyway).
    let scores = personalized_pagerank_from_graph_view(&view, &[a], 0.85, 1e-8)
        .expect("topology PPR after fold");
    assert!(!scores.is_empty());
    assert_eq!(scores[0].0, a);
}

#[tokio::test]
async fn unknown_seed_after_cut_returns_missing_seed() {
    let (store, a, _b, _c) = weighted_path_store().await;

    let view = subgraph_view_from(&store, &[a], 1, None, Some("CITES"), Some("weight"))
        .await
        .expect("cut");

    let err = personalized_pagerank_from_graph_view(&view, &[999_999], 0.85, 1e-6)
        .expect_err("unknown seed");
    assert_eq!(err, PprError::MissingSeed { node_id: 999_999 });
}
