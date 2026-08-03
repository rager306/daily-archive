//! TDD integration test for graph_view_from adapter (ADR-051 Contour 4).
//!
//! Proves the adapter path works end-to-end against a known-small
//! graph:
//!
//! ```text
//!     Paper1 -[CITES]-> Paper2 -[CITES]-> Paper3
//!     Paper1 -[CITES]-> Paper3
//! ```
//!
//! Expected PageRank (damping=0.85, 100 iterations):
//!   Paper3 ranks highest (most cited).

#![cfg(feature = "algorithms")]

use kg_storage::adapter::graph_view_from;
use kg_storage::mock::MockGraphStore;
use kg_storage::traits::DirectGraphStore;
use samyama_graph_algorithms::{page_rank, weakly_connected_components};

#[tokio::test]
async fn test_graph_view_from_citation_graph() {
    let store = MockGraphStore::new();
    let p1 = store.create_node("Paper").await.unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    let p3 = store.create_node("Paper").await.unwrap();
    store.create_edge(p1, p2, "CITES").await.unwrap();
    store.create_edge(p2, p3, "CITES").await.unwrap();
    store.create_edge(p1, p3, "CITES").await.unwrap();

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .expect("adapter must succeed");

    assert_eq!(view.node_count, 3, "GraphView must contain 3 Paper nodes");
}

#[tokio::test]
async fn test_pagerank_cited_paper_ranks_highest() {
    let store = MockGraphStore::new();
    let p1 = store.create_node("Paper").await.unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    let p3 = store.create_node("Paper").await.unwrap();
    store.create_edge(p1, p2, "CITES").await.unwrap();
    store.create_edge(p2, p3, "CITES").await.unwrap();
    store.create_edge(p1, p3, "CITES").await.unwrap();

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .unwrap();

    let ranks = page_rank(&view, samyama_graph_algorithms::PageRankConfig::default());

    // Paper3 (most cited) should rank highest.
    let p3_score = ranks.get(&p3).copied().unwrap_or(-1.0);
    let p1_score = ranks.get(&p1).copied().unwrap_or(-1.0);
    assert!(
        p3_score > p1_score,
        "Paper3 (most cited) must outrank Paper1 (no incoming). Got p3={p3_score}, p1={p1_score}"
    );
}

#[tokio::test]
async fn test_wcc_connected_graph() {
    let store = MockGraphStore::new();
    let p1 = store.create_node("Paper").await.unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    store.create_edge(p1, p2, "CITES").await.unwrap();

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .unwrap();

    let wcc = weakly_connected_components(&view);
    let component_count = wcc.components.len();
    assert_eq!(
        component_count, 1,
        "connected graph must yield one WCC component"
    );
}

#[tokio::test]
async fn test_wcc_two_isolated_papers() {
    let store = MockGraphStore::new();
    let _p1 = store.create_node("Paper").await.unwrap();
    let _p2 = store.create_node("Paper").await.unwrap();

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .unwrap();

    assert_eq!(view.node_count, 2);
    let wcc = weakly_connected_components(&view);
    assert_eq!(
        wcc.components.len(),
        2,
        "two isolated papers must yield two WCC components"
    );
}

#[tokio::test]
async fn test_empty_graph_returns_empty_view() {
    let store = MockGraphStore::new();
    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .unwrap();
    assert_eq!(
        view.node_count, 0,
        "empty store must produce empty GraphView"
    );
}

#[tokio::test]
async fn test_edge_type_filter_excludes_other_types() {
    let store = MockGraphStore::new();
    let p1 = store.create_node("Paper").await.unwrap();
    let p2 = store.create_node("Paper").await.unwrap();
    store.create_edge(p1, p2, "CITES").await.unwrap();
    store.create_edge(p1, p2, "RELATED_TO").await.unwrap();

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .unwrap();

    assert_eq!(view.node_count, 2);
    let p1_idx = view.node_to_index.get(&p1).copied().unwrap();
    let out_degree = view.out_offsets[p1_idx + 1] - view.out_offsets[p1_idx];
    assert_eq!(out_degree, 1, "edge_type filter must exclude RELATED_TO");
}
