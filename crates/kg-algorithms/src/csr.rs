//! Pure export: `GraphView` (outgoing CSR) → `CsrMatrix<f64>`.
//!
//! Feature-gated behind `solver`. Maps only outgoing topology + weights;
//! does not copy `index_to_node` / `node_to_index` and does not invoke solvers.
//!
//! # Contract notes
//!
//! - **kg-storage fold:** adapters fold uniform all-1.0 weight vectors to
//!   `weights=None`. This export hard-fails those views via
//!   [`CsrExportError::MissingWeights`]. Callers that need unit weights must
//!   call [`materialize_unit_weights`] (explicit) before export — never silent.
//! - **ForwardPush / PPR:** ruvector `ForwardPushSolver` walks topology and
//!   normalizes by **out-degree (edge count)**, ignoring CSR `values`.
//!   Exporting weights is correct for SpMV / future weighted solvers; it does
//!   **not** make current PPR weight-sensitive. Map dense scores back with
//!   `GraphView.index_to_node`.
//! - **Validation:** local f64 structural checks (vendor `validate_csr_matrix`
//!   is f32-only). Covers offsets, col bounds, and finite values.

use ruvector_solver::types::CsrMatrix;
use samyama_graph_algorithms::common::GraphView;
use std::fmt;

/// Errors raised when a [`GraphView`] cannot be projected into a valid CSR matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsrExportError {
    /// `weights` is `None` while `node_count > 0`. Required-weights contract.
    MissingWeights,
    /// Length of `weights` does not equal length of `out_targets` (nnz).
    WeightLengthMismatch { weight_len: usize, nnz: usize },
    /// `out_offsets` length is not `node_count + 1`.
    OffsetsLengthMismatch { offsets_len: usize, expected: usize },
    /// `out_offsets` is non-monotonic or final offset does not equal nnz.
    OffsetsInconsistent { detail: &'static str },
    /// An `out_targets` entry is `>= node_count` (square adjacency OOB).
    ColumnIndexOutOfBounds { index: usize, cols: usize },
    /// A weight is NaN or ±Inf (ruvector SpMV / PPR refuse non-finite values).
    NonFiniteWeight { index: usize },
}

impl fmt::Display for CsrExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingWeights => {
                write!(
                    f,
                    "GraphView has no edge weights; required-weights contract rejects weights=None when node_count > 0"
                )
            }
            Self::WeightLengthMismatch { weight_len, nnz } => {
                write!(
                    f,
                    "weight length {weight_len} does not match out_targets nnz {nnz}"
                )
            }
            Self::OffsetsLengthMismatch {
                offsets_len,
                expected,
            } => {
                write!(
                    f,
                    "out_offsets length {offsets_len} != node_count+1 ({expected})"
                )
            }
            Self::OffsetsInconsistent { detail } => {
                write!(f, "out_offsets inconsistent: {detail}")
            }
            Self::ColumnIndexOutOfBounds { index, cols } => {
                write!(f, "column index {index} out of bounds for cols={cols}")
            }
            Self::NonFiniteWeight { index } => {
                write!(f, "non-finite weight at nnz index {index}")
            }
        }
    }
}

impl std::error::Error for CsrExportError {}

/// Explicitly fill missing weights with `1.0` so export can succeed after
/// kg-storage's all-1.0 → `None` fold.
///
/// No-op when weights already present or `node_count == 0`.
/// Does **not** change topology. Callers choose this; export never injects 1.0.
///
/// # Errors
///
/// [`CsrExportError::OffsetsInconsistent`] / [`OffsetsLengthMismatch`] if
/// outgoing CSR is broken (cannot know nnz safely).
pub fn materialize_unit_weights(view: &mut GraphView) -> Result<(), CsrExportError> {
    if view.node_count == 0 || view.weights.is_some() {
        return Ok(());
    }
    let nnz = view.out_targets.len();
    validate_out_offsets(&view.out_offsets, view.node_count, nnz)?;
    view.weights = Some(vec![1.0; nnz]);
    Ok(())
}

/// Build a square [`CsrMatrix<f64>`] from the outgoing CSR of `view`.
///
/// Mapping:
/// - `out_offsets` → `row_ptr`
/// - `out_targets` → `col_indices`
/// - `weights` → `values`
/// - `node_count` → `rows` and `cols` (square adjacency)
///
/// Constructs via public CSR fields (clone of the CSR arrays).
///
/// # Errors
///
/// - [`CsrExportError::MissingWeights`] when `weights` is `None` and `node_count > 0`
/// - structural failures: offsets, weight length, column OOB, non-finite weights
///
/// Empty graphs (`node_count == 0`) return an empty matrix without requiring weights.
pub fn csr_matrix_from_graph_view(view: &GraphView) -> Result<CsrMatrix<f64>, CsrExportError> {
    let n = view.node_count;

    if n == 0 {
        return Ok(CsrMatrix {
            row_ptr: vec![0],
            col_indices: Vec::new(),
            values: Vec::new(),
            rows: 0,
            cols: 0,
        });
    }

    let weights = view
        .weights
        .as_ref()
        .ok_or(CsrExportError::MissingWeights)?;

    let nnz = view.out_targets.len();
    if weights.len() != nnz {
        return Err(CsrExportError::WeightLengthMismatch {
            weight_len: weights.len(),
            nnz,
        });
    }

    validate_out_offsets(&view.out_offsets, n, nnz)?;
    validate_columns_and_weights(&view.out_targets, weights, n)?;

    Ok(CsrMatrix {
        row_ptr: view.out_offsets.clone(),
        col_indices: view.out_targets.clone(),
        values: weights.clone(),
        rows: n,
        cols: n,
    })
}

fn validate_out_offsets(
    out_offsets: &[usize],
    node_count: usize,
    nnz: usize,
) -> Result<(), CsrExportError> {
    let expected = node_count + 1;
    if out_offsets.len() != expected {
        return Err(CsrExportError::OffsetsLengthMismatch {
            offsets_len: out_offsets.len(),
            expected,
        });
    }

    if out_offsets[0] != 0 {
        return Err(CsrExportError::OffsetsInconsistent {
            detail: "first offset must be 0",
        });
    }

    for window in out_offsets.windows(2) {
        if window[1] < window[0] {
            return Err(CsrExportError::OffsetsInconsistent {
                detail: "offsets must be non-decreasing",
            });
        }
    }

    if *out_offsets.last().expect("len checked as n+1") != nnz {
        return Err(CsrExportError::OffsetsInconsistent {
            detail: "final offset must equal nnz",
        });
    }

    Ok(())
}

fn validate_columns_and_weights(
    out_targets: &[usize],
    weights: &[f64],
    node_count: usize,
) -> Result<(), CsrExportError> {
    for (i, &col) in out_targets.iter().enumerate() {
        if col >= node_count {
            return Err(CsrExportError::ColumnIndexOutOfBounds {
                index: col,
                cols: node_count,
            });
        }
        if !weights[i].is_finite() {
            return Err(CsrExportError::NonFiniteWeight { index: i });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Directed triangle with non-uniform weights:
    /// 0→1 (0.5), 1→2 (1.5), 2→0 (2.5).
    fn weighted_triangle() -> GraphView {
        let node_count = 3;
        let index_to_node = vec![10, 20, 30];
        let mut node_to_index = HashMap::new();
        for (i, &id) in index_to_node.iter().enumerate() {
            node_to_index.insert(id, i);
        }
        let outgoing = vec![vec![1], vec![2], vec![0]];
        let incoming = vec![vec![2], vec![0], vec![1]];
        let weights = Some(vec![vec![0.5], vec![1.5], vec![2.5]]);
        GraphView::from_adjacency_list(
            node_count,
            index_to_node,
            node_to_index,
            outgoing,
            incoming,
            weights,
        )
    }

    fn empty_view() -> GraphView {
        GraphView::from_adjacency_list(0, vec![], HashMap::new(), vec![], vec![], None)
    }

    #[test]
    fn weighted_directed_export_matches_outgoing_csr() {
        let view = weighted_triangle();
        let m = csr_matrix_from_graph_view(&view).expect("weighted view must export");

        assert_eq!(m.rows, 3);
        assert_eq!(m.cols, 3);
        assert_eq!(m.rows, view.node_count);
        assert_eq!(m.row_ptr, view.out_offsets);
        assert_eq!(m.col_indices, view.out_targets);
        assert_eq!(m.values, view.weights.as_ref().unwrap().clone());
        // Non-uniform weights: not the all-1.0 path kg-storage folds to None.
        assert_ne!(m.values, vec![1.0, 1.0, 1.0]);
        assert_eq!(m.row_ptr.len(), view.node_count + 1);
        assert_eq!(m.col_indices.len(), m.values.len());
        assert_eq!(*m.row_ptr.last().unwrap(), m.col_indices.len());
    }

    #[test]
    fn missing_weights_with_nodes_returns_err() {
        let mut view = weighted_triangle();
        view.weights = None;
        let err = csr_matrix_from_graph_view(&view).expect_err("weights=None must fail");
        assert_eq!(err, CsrExportError::MissingWeights);
        let msg = err.to_string();
        assert!(
            msg.contains("weights"),
            "message must mention weights: {msg}"
        );
    }

    #[test]
    fn empty_view_returns_ok_empty_matrix() {
        let view = empty_view();
        let m = csr_matrix_from_graph_view(&view).expect("empty view must succeed");
        assert_eq!(m.rows, 0);
        assert_eq!(m.cols, 0);
        assert_eq!(m.row_ptr, vec![0]);
        assert!(m.col_indices.is_empty());
        assert!(m.values.is_empty());
        assert_eq!(m.nnz(), 0);
    }

    #[test]
    fn row_ptr_length_and_nnz_consistency() {
        let view = weighted_triangle();
        let m = csr_matrix_from_graph_view(&view).expect("export");
        assert_eq!(m.row_ptr.len(), m.rows + 1);
        assert_eq!(m.row_ptr[0], 0);
        assert_eq!(*m.row_ptr.last().unwrap(), m.nnz());
        for window in m.row_ptr.windows(2) {
            assert!(window[1] >= window[0], "row_ptr must be non-decreasing");
        }
    }

    #[test]
    fn weight_length_mismatch_returns_err() {
        let mut view = weighted_triangle();
        // Corrupt: nnz=3 but only two weights.
        view.weights = Some(vec![0.5, 1.5]);
        let err = csr_matrix_from_graph_view(&view).expect_err("weight len mismatch");
        assert_eq!(
            err,
            CsrExportError::WeightLengthMismatch {
                weight_len: 2,
                nnz: 3
            }
        );
    }

    #[test]
    fn offsets_length_mismatch_returns_err() {
        let mut view = weighted_triangle();
        // Corrupt: drop last offset so len != n+1.
        view.out_offsets.pop();
        let err = csr_matrix_from_graph_view(&view).expect_err("offsets len");
        assert_eq!(
            err,
            CsrExportError::OffsetsLengthMismatch {
                offsets_len: 3,
                expected: 4
            }
        );
    }

    #[test]
    fn non_monotonic_offsets_returns_err() {
        let mut view = weighted_triangle();
        // Corrupt: make offsets decrease.
        view.out_offsets = vec![0, 2, 1, 3];
        let err = csr_matrix_from_graph_view(&view).expect_err("non-monotonic");
        match err {
            CsrExportError::OffsetsInconsistent { detail } => {
                assert!(detail.contains("non-decreasing"), "detail={detail}");
            }
            other => panic!("expected OffsetsInconsistent, got {other:?}"),
        }
    }

    #[test]
    fn final_offset_not_equal_nnz_returns_err() {
        let mut view = weighted_triangle();
        // Keep len=n+1 and monotonic, but final != nnz (pad last).
        view.out_offsets = vec![0, 1, 2, 4]; // final 4 != nnz 3
        let err = csr_matrix_from_graph_view(&view).expect_err("final offset");
        match err {
            CsrExportError::OffsetsInconsistent { detail } => {
                assert!(detail.contains("nnz"), "detail={detail}");
            }
            other => panic!("expected OffsetsInconsistent, got {other:?}"),
        }
    }

    #[test]
    fn column_index_out_of_bounds_returns_err() {
        let mut view = weighted_triangle();
        // Corrupt: target index == n (OOB for square n×n).
        view.out_targets[1] = 3;
        let err = csr_matrix_from_graph_view(&view).expect_err("col OOB");
        assert_eq!(
            err,
            CsrExportError::ColumnIndexOutOfBounds { index: 3, cols: 3 }
        );
    }

    #[test]
    fn non_finite_weight_returns_err() {
        let mut view = weighted_triangle();
        view.weights = Some(vec![0.5, f64::NAN, 2.5]);
        let err = csr_matrix_from_graph_view(&view).expect_err("NaN weight");
        assert_eq!(err, CsrExportError::NonFiniteWeight { index: 1 });

        view.weights = Some(vec![0.5, f64::INFINITY, 2.5]);
        let err = csr_matrix_from_graph_view(&view).expect_err("Inf weight");
        assert_eq!(err, CsrExportError::NonFiniteWeight { index: 1 });
    }

    #[test]
    fn materialize_unit_weights_enables_export_after_fold() {
        let mut view = weighted_triangle();
        view.weights = None; // simulates kg-storage all-1.0 fold
        assert_eq!(
            csr_matrix_from_graph_view(&view).unwrap_err(),
            CsrExportError::MissingWeights
        );

        materialize_unit_weights(&mut view).expect("materialize");
        let m = csr_matrix_from_graph_view(&view).expect("export after materialize");
        assert_eq!(m.values, vec![1.0, 1.0, 1.0]);
        assert_eq!(m.rows, 3);

        // Already present: no-op, preserves non-uniform.
        let mut w2 = weighted_triangle();
        materialize_unit_weights(&mut w2).expect("noop");
        assert_eq!(w2.weights.as_ref().unwrap(), &vec![0.5, 1.5, 2.5]);
    }
}
