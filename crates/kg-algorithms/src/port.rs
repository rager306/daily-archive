//! Universal graph algorithm port (ADR-050).
//!
//! PPR, neighborhood queries — any graph system can implement this.
//! Extracted from da-ports in M290-j98lkq/S03.

use async_trait::async_trait;

/// Personalized PageRank (PPR) result: (node_id, score).
pub type PPRScore = (u64, f64);

/// Graph algorithm port — enables GNN operations on the knowledge graph.
///
/// Implementations:
/// - Embedded forward-push (pure Rust, small graphs)
/// - RuVector solver (vendored, large graphs)
#[async_trait]
pub trait GraphAlgorithms: Send + Sync {
    /// Personalized PageRank from seed nodes.
    async fn personalized_pagerank(
        &self,
        seed_nodes: &[u64],
        alpha: f64,
        max_iterations: usize,
    ) -> Vec<PPRScore>;

    /// Get neighbors of a node via outgoing edges of a specific type.
    async fn get_neighbors(&self, node_id: u64, edge_type: &str) -> Vec<(u64, f64)>;

    /// Get all neighbors regardless of edge type.
    async fn get_all_neighbors(&self, node_id: u64) -> Vec<(u64, String, f64)>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppr_score_type() {
        let scores: Vec<PPRScore> = vec![(1, 0.5), (2, 0.3)];
        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0].0, 1);
    }
}
