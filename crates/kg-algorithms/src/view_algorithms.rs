//! Async [`GraphAlgorithms`] façade over a projected [`GraphView`].
//!
//! Feature-gated behind `ppr`. Closes the dead-port debt: the trait had zero
//! implementations while pure [`personalized_pagerank_from_graph_view`] existed.
//!
//! # Semantics
//!
//! - **PPR** delegates to the pure ForwardPush helper (degree-count; ignores
//!   CSR values). Trait returns `Vec` not `Result` — errors **fail-soft** to
//!   empty `Vec` (0 production callers; honest empty beats silent wrong ranks).
//! - **`max_iterations`** is not used by ForwardPush (epsilon threshold). Fixed
//!   `epsilon = 1e-8`. Documented; not a silent convergence lie.
//! - **Neighbors** read projected topology. `edge_type` on `get_neighbors` is
//!   ignored — the view is already filtered at projection time. Weights are
//!   real when present, else `1.0`.
//! - **`get_all_neighbors`** labels edges with `projected_edge_type` if set,
//!   else `"*"`.

use crate::port::{GraphAlgorithms, PPRScore};
use crate::ppr::personalized_pagerank_from_graph_view;
use async_trait::async_trait;
use samyama_graph_algorithms::common::GraphView;

/// Owned GraphView + optional projected edge-type label for neighbor APIs.
pub struct GraphViewAlgorithms {
    view: GraphView,
    /// Edge type this view was projected with (informational for get_all_neighbors).
    projected_edge_type: Option<String>,
}

impl GraphViewAlgorithms {
    /// Wrap a GraphView. `projected_edge_type` is optional metadata only.
    pub fn new(view: GraphView, projected_edge_type: Option<String>) -> Self {
        Self {
            view,
            projected_edge_type,
        }
    }

    /// Borrow the underlying view (for diagnostics / CSR export).
    pub fn view(&self) -> &GraphView {
        &self.view
    }

    /// Fixed ForwardPush residual threshold (trait has no epsilon parameter).
    pub const DEFAULT_EPSILON: f64 = 1e-8;
}

#[async_trait]
impl GraphAlgorithms for GraphViewAlgorithms {
    async fn personalized_pagerank(
        &self,
        seed_nodes: &[u64],
        alpha: f64,
        _max_iterations: usize,
    ) -> Vec<PPRScore> {
        // max_iterations unused: ForwardPush is epsilon-driven, not iteration-capped.
        personalized_pagerank_from_graph_view(&self.view, seed_nodes, alpha, Self::DEFAULT_EPSILON)
            .unwrap_or_default()
    }

    async fn get_neighbors(&self, node_id: u64, _edge_type: &str) -> Vec<(u64, f64)> {
        // edge_type ignored: GraphView is pre-projected.
        let Some(&idx) = self.view.node_to_index.get(&node_id) else {
            return Vec::new();
        };
        let succ = self.view.successors(idx);
        let weights = self.view.weights(idx);
        succ.iter()
            .enumerate()
            .filter_map(|(i, &dense)| {
                let nid = *self.view.index_to_node.get(dense)?;
                let w = weights.map(|ws| ws[i]).unwrap_or(1.0);
                Some((nid, w))
            })
            .collect()
    }

    async fn get_all_neighbors(&self, node_id: u64) -> Vec<(u64, String, f64)> {
        let label = self
            .projected_edge_type
            .clone()
            .unwrap_or_else(|| "*".to_string());
        self.get_neighbors(node_id, "")
            .await
            .into_iter()
            .map(|(nid, w)| (nid, label.clone(), w))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    #[tokio::test]
    async fn trait_ppr_seed_ranks_high() {
        let alg = GraphViewAlgorithms::new(path_view(), Some("CITES".into()));
        let scores = alg.personalized_pagerank(&[10], 0.85, 100).await;
        assert!(!scores.is_empty());
        assert_eq!(scores[0].0, 10);
        assert!(scores[0].1 > 0.0 && scores[0].1.is_finite());
    }

    #[tokio::test]
    async fn trait_ppr_unknown_seed_fail_soft_empty() {
        let alg = GraphViewAlgorithms::new(path_view(), None);
        let scores = alg.personalized_pagerank(&[999], 0.85, 10).await;
        assert!(
            scores.is_empty(),
            "trait Vec surface fail-softs PprError to empty"
        );
    }

    #[tokio::test]
    async fn get_neighbors_returns_node_ids_and_weights() {
        let alg = GraphViewAlgorithms::new(path_view(), Some("CITES".into()));
        let n = alg.get_neighbors(10, "IGNORED").await;
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].0, 20);
        assert!((n[0].1 - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn get_neighbors_unknown_node_empty() {
        let alg = GraphViewAlgorithms::new(path_view(), None);
        assert!(alg.get_neighbors(999, "CITES").await.is_empty());
    }

    #[tokio::test]
    async fn get_all_neighbors_labels_projected_type() {
        let alg = GraphViewAlgorithms::new(path_view(), Some("CITES".into()));
        let n = alg.get_all_neighbors(10).await;
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].0, 20);
        assert_eq!(n[0].1, "CITES");
        assert!((n[0].2 - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn empty_view_ppr_empty() {
        let view = GraphView::from_adjacency_list(0, vec![], HashMap::new(), vec![], vec![], None);
        let alg = GraphViewAlgorithms::new(view, None);
        let scores = alg.personalized_pagerank(&[], 0.85, 10).await;
        assert!(scores.is_empty());
    }
}
