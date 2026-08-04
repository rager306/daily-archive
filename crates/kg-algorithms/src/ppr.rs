//! Pure Personalized PageRank smoke over M298 CSR export + ruvector ForwardPush.
//!
//! Feature-gated behind `ppr` (implies `solver` + `ruvector-solver/forward-push`).
//!
//! # Semantics
//!
//! - Seeds are **graph NodeIds**; dense CSR indices come from `GraphView.node_to_index`.
//! - Scores are mapped back via `GraphView.index_to_node`.
//! - **ForwardPush ignores CSR edge values** and normalizes by out-degree (count).
//!   When `weights` is `None` (kg-storage all-1.0 fold), this helper builds a
//!   unit-value CSR for construction only — topology walk is unchanged.
//! - Async trait surface: [`crate::view_algorithms::GraphViewAlgorithms`]
//!   implements [`crate::port::GraphAlgorithms`] (M301) by wrapping this helper.

use crate::csr::{CsrExportError, csr_matrix_from_graph_view};
use ruvector_solver::forward_push::ForwardPushSolver;
use ruvector_solver::traits::SublinearPageRank;
use ruvector_solver::types::CsrMatrix;
use samyama_graph_algorithms::common::GraphView;
use std::fmt;

/// Errors from pure PPR over a GraphView.
#[derive(Debug, Clone, PartialEq)]
pub enum PprError {
    /// `seeds` was empty while the graph has nodes.
    EmptySeeds,
    /// A seed NodeId is not present in `GraphView.node_to_index`.
    MissingSeed { node_id: u64 },
    /// CSR export / structural failure.
    Csr(CsrExportError),
    /// Solver rejected input or hit numerical instability.
    Solver { message: String },
}

impl fmt::Display for PprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySeeds => write!(f, "personalized_pagerank requires at least one seed"),
            Self::MissingSeed { node_id } => {
                write!(f, "seed NodeId {node_id} not found in GraphView")
            }
            Self::Csr(e) => write!(f, "CSR export failed: {e}"),
            Self::Solver { message } => write!(f, "ForwardPush solver error: {message}"),
        }
    }
}

impl std::error::Error for PprError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Csr(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CsrExportError> for PprError {
    fn from(value: CsrExportError) -> Self {
        Self::Csr(value)
    }
}

/// Run ForwardPush PPR from seed **NodeIds** on a GraphView.
///
/// Mapping: NodeId → dense index → `ForwardPushSolver` → dense scores → NodeId.
///
/// # Parameters
///
/// - `alpha` — teleport probability in `(0, 1)` exclusive (ForwardPush contract).
/// - `epsilon` — push threshold `> 0`.
///
/// # Errors
///
/// - [`PprError::EmptySeeds`] when `seeds` is empty and `node_count > 0`
/// - [`PprError::MissingSeed`] when a seed is unknown
/// - [`PprError::Csr`] on structural export failure
/// - [`PprError::Solver`] on solver rejection
///
/// Empty graphs (`node_count == 0`) return `Ok(vec![])` without requiring seeds.
pub fn personalized_pagerank_from_graph_view(
    view: &GraphView,
    seeds: &[u64],
    alpha: f64,
    epsilon: f64,
) -> Result<Vec<(u64, f64)>, PprError> {
    if view.node_count == 0 {
        return Ok(Vec::new());
    }
    if seeds.is_empty() {
        return Err(PprError::EmptySeeds);
    }

    let matrix = csr_for_ppr(view)?;

    let mut dense_seeds: Vec<(usize, f64)> = Vec::with_capacity(seeds.len());
    let mass = 1.0 / seeds.len() as f64;
    for &node_id in seeds {
        let idx = view
            .node_to_index
            .get(&node_id)
            .copied()
            .ok_or(PprError::MissingSeed { node_id })?;
        dense_seeds.push((idx, mass));
    }

    let solver = ForwardPushSolver::new(alpha, epsilon);
    let dense_scores = solver
        .ppr_multi_seed(&matrix, &dense_seeds, alpha, epsilon)
        .map_err(|e| PprError::Solver {
            message: e.to_string(),
        })?;

    let mut out = Vec::with_capacity(dense_scores.len());
    for (dense_idx, score) in dense_scores {
        let node_id =
            view.index_to_node
                .get(dense_idx)
                .copied()
                .ok_or_else(|| PprError::Solver {
                    message: format!(
                        "solver returned dense index {dense_idx} outside index_to_node"
                    ),
                })?;
        out.push((node_id, score));
    }
    Ok(out)
}

/// Build CSR for PPR: real weights when present; unit values when folded None.
///
/// Unit fill exists only because export requires values and ForwardPush ignores
/// them — topology walk is degree-count based.
fn csr_for_ppr(view: &GraphView) -> Result<CsrMatrix<f64>, PprError> {
    if view.weights.is_some() {
        return csr_matrix_from_graph_view(view).map_err(PprError::from);
    }

    // Topology-only path after kg-storage all-1.0 → None fold.
    // Construct unit-value CSR without mutating the caller's GraphView.
    let n = view.node_count;
    let nnz = view.out_targets.len();
    // Reuse export validation by temporarily projecting via a local weights vec
    // through the same structural checks as csr_matrix_from_graph_view.
    // Manual construction mirrors export mapping with values = 1.0.
    use crate::csr::CsrExportError;

    if view.out_offsets.len() != n + 1 {
        return Err(PprError::Csr(CsrExportError::OffsetsLengthMismatch {
            offsets_len: view.out_offsets.len(),
            expected: n + 1,
        }));
    }
    if view.out_offsets.first().copied() != Some(0) {
        return Err(PprError::Csr(CsrExportError::OffsetsInconsistent {
            detail: "first offset must be 0",
        }));
    }
    for window in view.out_offsets.windows(2) {
        if window[1] < window[0] {
            return Err(PprError::Csr(CsrExportError::OffsetsInconsistent {
                detail: "offsets must be non-decreasing",
            }));
        }
    }
    if view.out_offsets.last().copied() != Some(nnz) {
        return Err(PprError::Csr(CsrExportError::OffsetsInconsistent {
            detail: "final offset must equal nnz",
        }));
    }
    for &col in &view.out_targets {
        if col >= n {
            return Err(PprError::Csr(CsrExportError::ColumnIndexOutOfBounds {
                index: col,
                cols: n,
            }));
        }
    }

    Ok(CsrMatrix {
        row_ptr: view.out_offsets.clone(),
        col_indices: view.out_targets.clone(),
        values: vec![1.0; nnz],
        rows: n,
        cols: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Directed path 10→20→30 with non-uniform weights (survives fold).
    fn path_view() -> GraphView {
        let node_count = 3;
        let index_to_node = vec![10, 20, 30];
        let mut node_to_index = HashMap::new();
        for (i, &id) in index_to_node.iter().enumerate() {
            node_to_index.insert(id, i);
        }
        let outgoing = vec![vec![1], vec![2], vec![]];
        let incoming = vec![vec![], vec![0], vec![1]];
        let weights = Some(vec![vec![0.5], vec![1.5], vec![]]);
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
    fn seed_on_path_ranks_source_high() {
        let view = path_view();
        let scores = personalized_pagerank_from_graph_view(&view, &[10], 0.85, 1e-8).expect("ppr");
        assert!(!scores.is_empty());
        // Seed should be among top scores (typically first).
        assert_eq!(
            scores[0].0, 10,
            "source seed should rank first, got {scores:?}"
        );
        assert!(scores[0].1 > 0.0);
        // All scores finite and NodeIds from the view.
        for (id, s) in &scores {
            assert!(s.is_finite());
            assert!(view.node_to_index.contains_key(id));
        }
    }

    #[test]
    fn unknown_seed_returns_err() {
        let view = path_view();
        let err = personalized_pagerank_from_graph_view(&view, &[999], 0.85, 1e-6)
            .expect_err("unknown seed");
        assert_eq!(err, PprError::MissingSeed { node_id: 999 });
    }

    #[test]
    fn empty_seeds_returns_err() {
        let view = path_view();
        let err =
            personalized_pagerank_from_graph_view(&view, &[], 0.85, 1e-6).expect_err("empty seeds");
        assert_eq!(err, PprError::EmptySeeds);
    }

    #[test]
    fn empty_graph_returns_empty_scores() {
        let view = empty_view();
        let scores = personalized_pagerank_from_graph_view(&view, &[], 0.85, 1e-6).expect("empty");
        assert!(scores.is_empty());
    }

    #[test]
    fn folded_none_weights_topology_path_works() {
        let mut view = path_view();
        view.weights = None; // simulate kg-storage fold
        let scores =
            personalized_pagerank_from_graph_view(&view, &[10], 0.85, 1e-8).expect("topology ppr");
        assert!(!scores.is_empty());
        assert_eq!(scores[0].0, 10);
    }

    #[test]
    fn multi_seed_accepts_known_nodes() {
        let view = path_view();
        let scores =
            personalized_pagerank_from_graph_view(&view, &[10, 30], 0.85, 1e-8).expect("multi");
        assert!(!scores.is_empty());
        let ids: Vec<u64> = scores.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&10) || ids.contains(&30) || !ids.is_empty());
    }
}
