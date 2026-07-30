//! Graph algorithm ports — PPR, community detection, similarity.
//!
//! Phase 5 GNN readiness: these ports define the interface for RuVector
//! Tier 2 integration. Adapters (RuVector solver, embedded forward-push)
//! implement these traits.
//!
//! ADR-042: confidence propagation uses PPR over incidence structure.

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
    ///
    /// `seed_nodes`: starting node IDs (uniform initial probability).
    /// `alpha`: teleport probability (0.15 = standard PageRank).
    /// `max_iterations`: convergence limit.
    ///
    /// Returns sorted (node_id, score) pairs, highest first.
    async fn personalized_pagerank(
        &self,
        seed_nodes: &[u64],
        alpha: f64,
        max_iterations: usize,
    ) -> Vec<PPRScore>;

    /// Get neighbors of a node via outgoing edges of a specific type.
    /// Returns (neighbor_id, edge_weight) pairs.
    async fn get_neighbors(&self, node_id: u64, edge_type: &str) -> Vec<(u64, f64)>;

    /// Get all neighbors regardless of edge type.
    async fn get_all_neighbors(&self, node_id: u64) -> Vec<(u64, String, f64)>;
}
