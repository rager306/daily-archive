//! Compose contract: MockGraphStore → GraphView adapter → CsrMatrix export.
//!
//! Closes the M298 audit gap (Mock→view→matrix) without pulling solver into
//! kg-storage production deps. Requires:
//! - `algorithms` (subgraph/graph_view_from adapters)
//! - dev-dep `kg-algorithms` with feature `solver` (always on via Cargo.toml)
//!
//! ADR-051 Contour 4 (contract) + Contour 5 (adapter integration via Mock).

#![cfg(feature = "algorithms")]

use kg_algorithms::{CsrExportError, csr_matrix_from_graph_view, materialize_unit_weights};
use kg_storage::adapter::{graph_view_from, subgraph_view_from};
use kg_storage::mock::MockGraphStore;
use kg_storage::traits::DirectGraphStore;

/// Weighted triangle A→B (0.5), B→C (1.5), C→A (2.5) — non-uniform so fold
/// does not collapse weights to None.
async fn weighted_triangle_store() -> (MockGraphStore, u64, u64, u64) {
    let store = MockGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    let c = store.create_node("Paper").await.unwrap();
    let e_ab = store.create_edge(a, b, "CITES").await.unwrap();
    let e_bc = store.create_edge(b, c, "CITES").await.unwrap();
    let e_ca = store.create_edge(c, a, "CITES").await.unwrap();
    store
        .set_edge_property_float(e_ab, "weight", 0.5)
        .await
        .unwrap();
    store
        .set_edge_property_float(e_bc, "weight", 1.5)
        .await
        .unwrap();
    store
        .set_edge_property_float(e_ca, "weight", 2.5)
        .await
        .unwrap();
    (store, a, b, c)
}

#[tokio::test]
async fn weighted_subgraph_exports_to_csr_matrix() {
    let (store, a, _b, _c) = weighted_triangle_store().await;

    let view = subgraph_view_from(&store, &[a], 2, None, Some("CITES"), Some("weight"))
        .await
        .expect("weighted k-hop cut");

    assert!(view.node_count >= 2, "seed neighborhood must expand");
    assert!(
        view.weights.is_some(),
        "non-uniform weights must survive fold"
    );

    let m = csr_matrix_from_graph_view(&view).expect("export weighted view");
    assert_eq!(m.rows, view.node_count);
    assert_eq!(m.cols, view.node_count);
    assert_eq!(m.row_ptr, view.out_offsets);
    assert_eq!(m.col_indices, view.out_targets);
    assert_eq!(m.values, *view.weights.as_ref().unwrap());
    // Dense scores map back via GraphView — not on the matrix.
    assert_eq!(view.index_to_node.len(), m.rows);
}

#[tokio::test]
async fn unweighted_view_hard_fails_export() {
    let (store, a, _b, _c) = weighted_triangle_store().await;

    // weight_property=None → all 1.0 → fold to None.
    let view = subgraph_view_from(&store, &[a], 1, None, Some("CITES"), None)
        .await
        .expect("unweighted cut");

    assert!(
        view.weights.is_none(),
        "all-1.0 path must fold to None (kg-storage honesty)"
    );
    let err = csr_matrix_from_graph_view(&view).expect_err("export must hard-fail");
    assert_eq!(err, CsrExportError::MissingWeights);
}

#[tokio::test]
async fn materialize_unit_weights_then_export_ok() {
    let (store, a, _b, _c) = weighted_triangle_store().await;

    let mut view = subgraph_view_from(&store, &[a], 1, None, Some("CITES"), None)
        .await
        .expect("unweighted cut");

    materialize_unit_weights(&mut view).expect("explicit unit fill");
    let m = csr_matrix_from_graph_view(&view).expect("export after materialize");
    assert_eq!(m.rows, view.node_count);
    assert!(m.values.iter().all(|&w| (w - 1.0).abs() < f64::EPSILON));
    assert_eq!(m.values.len(), view.out_targets.len());
}

#[tokio::test]
async fn empty_graph_view_exports_empty_matrix() {
    let store = MockGraphStore::new();
    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None)
        .await
        .expect("empty store");
    assert_eq!(view.node_count, 0);

    let m = csr_matrix_from_graph_view(&view).expect("empty export");
    assert_eq!(m.rows, 0);
    assert_eq!(m.cols, 0);
    assert_eq!(m.nnz(), 0);
}

#[tokio::test]
async fn full_graph_view_from_weighted_exports() {
    let (store, _a, _b, _c) = weighted_triangle_store().await;

    let view = graph_view_from(&store, Some("Paper"), Some("CITES"), Some("weight"))
        .await
        .expect("full weighted projection");

    let m = csr_matrix_from_graph_view(&view).expect("export");
    assert_eq!(m.rows, 3);
    assert_eq!(m.nnz(), 3);
    let mut vals = m.values.clone();
    vals.sort_by(|x, y| x.partial_cmp(y).unwrap());
    assert_eq!(vals, vec![0.5, 1.5, 2.5]);
}
