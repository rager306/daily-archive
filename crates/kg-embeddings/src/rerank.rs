//! Pluggable rerankers over hybrid-search candidates (ADR-073 follow-up).
//!
//! RRF fusion ([`RuVectorBridge::hybrid_rrf`]) produces the fused candidate
//! list; a [`Reranker`] re-scores that list with evidence the rank fusion
//! cannot see (graph proximity, mention counts, ...). Graph evidence enters
//! through the [`GraphProximity`] trait, so this crate stays universal —
//! no graph-store dependency.
//!
//! [`RuVectorBridge::hybrid_rrf`]: crate::ruvector::RuVectorBridge::hybrid_rrf

use std::collections::{HashMap, VecDeque};

use crate::ruvector::{CsrTopology, RankedItem};

/// Re-scores hybrid-search candidates.
///
/// Implementations must be deterministic: the same input yields the same
/// output. The returned list is sorted by descending score; ties keep the
/// fused (input) order.
pub trait Reranker {
    fn rerank(&self, candidates: &[RankedItem]) -> Vec<RankedItem>;
}

/// Passthrough reranker: preserves the fused ranking exactly.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityReranker;

impl Reranker for IdentityReranker {
    fn rerank(&self, candidates: &[RankedItem]) -> Vec<RankedItem> {
        candidates.to_vec()
    }
}

/// Source of graph-proximity evidence for [`GraphProximityReranker`].
///
/// Returns a score in `[0.0, 1.0]` (1.0 = closest to the query anchors) or
/// `None` when the node is unknown to the provider (no boost). This seam is
/// what keeps graph stores out of kg-embeddings: an adapter over a real
/// store implements the trait, the reranker only reads it.
pub trait GraphProximity {
    fn proximity(&self, node_id: u64) -> Option<f64>;
}

/// Boosts candidates by graph proximity to the query anchors.
///
/// Final score: `rrf_score * (1.0 + boost * proximity)`; unknown proximity
/// counts as 0.0, so those candidates keep their fused score. `boost = 0.0`
/// degenerates to the identity ranking.
#[derive(Debug)]
pub struct GraphProximityReranker<P: GraphProximity> {
    proximity: P,
    boost: f64,
}

impl<P: GraphProximity> GraphProximityReranker<P> {
    /// `boost >= 0.0` and proximity in `[0.0, 1.0]` is the supported range.
    pub fn new(proximity: P, boost: f64) -> Self {
        Self { proximity, boost }
    }
}

impl<P: GraphProximity> Reranker for GraphProximityReranker<P> {
    fn rerank(&self, candidates: &[RankedItem]) -> Vec<RankedItem> {
        let mut out: Vec<RankedItem> = candidates
            .iter()
            .map(|c| {
                let boost = 1.0 + self.boost * self.proximity.proximity(c.node_id).unwrap_or(0.0);
                RankedItem {
                    node_id: c.node_id,
                    score: c.score * boost,
                }
            })
            .collect();
        // Stable sort: ties keep the fused order.
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }
}

/// BFS-hop proximity over a plain CSR topology, anchored at seed nodes.
///
/// `proximity(node) = 1 / (1 + hops)` where `hops` is the shortest hop
/// distance to any seed (seeds score 1.0). Unreachable or out-of-range
/// nodes score `None` (no boost). Deterministic, dependency-free — same
/// [`CsrTopology`] contract as the feature-gated `ppr` solver.
#[derive(Debug, Default)]
pub struct HopProximity {
    score: HashMap<u64, f64>,
}

impl HopProximity {
    pub fn from_topology(topology: &CsrTopology, seeds: &[u64]) -> Self {
        let node_count = topology.offsets.len().saturating_sub(1);
        let mut hops: HashMap<u64, usize> = HashMap::new();
        let mut queue: VecDeque<u64> = VecDeque::new();
        for &seed in seeds {
            if (seed as usize) < node_count && !hops.contains_key(&seed) {
                hops.insert(seed, 0);
                queue.push_back(seed);
            }
        }
        while let Some(node) = queue.pop_front() {
            let depth = hops[&node];
            let start = topology.offsets.get(node as usize).copied().unwrap_or(0);
            let end = topology
                .offsets
                .get(node as usize + 1)
                .copied()
                .unwrap_or(0);
            for &neighbor in &topology.neighbors[start..end] {
                if (neighbor as usize) < node_count && !hops.contains_key(&neighbor) {
                    hops.insert(neighbor, depth + 1);
                    queue.push_back(neighbor);
                }
            }
        }
        Self {
            score: hops
                .into_iter()
                .map(|(node, depth)| (node, 1.0 / (1.0 + depth as f64)))
                .collect(),
        }
    }
}

impl GraphProximity for HopProximity {
    fn proximity(&self, node_id: u64) -> Option<f64> {
        self.score.get(&node_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(node_id: u64, score: f64) -> RankedItem {
        RankedItem { node_id, score }
    }

    fn line_topology() -> CsrTopology {
        // 0 -> 1 -> 2 -> 3 (chain)
        CsrTopology {
            offsets: vec![0, 1, 2, 3, 4],
            neighbors: vec![1, 2, 3, 0],
        }
    }

    #[test]
    fn identity_reranker_is_exact_passthrough() {
        let candidates = vec![item(7, 0.3), item(2, 0.9), item(5, 0.3)];
        let out = IdentityReranker.rerank(&candidates);
        assert_eq!(out, candidates, "identity must preserve order and scores");
    }

    #[test]
    fn identity_reranker_empty_input_is_empty() {
        assert!(IdentityReranker.rerank(&[]).is_empty());
    }

    #[test]
    fn graph_proximity_boost_flips_ranking_deterministically() {
        let topology = line_topology();
        let prox = HopProximity::from_topology(&topology, &[0]);
        assert!((prox.proximity(0).unwrap() - 1.0).abs() < 1e-12);
        assert!((prox.proximity(1).unwrap() - 0.5).abs() < 1e-12);
        assert!((prox.proximity(3).unwrap() - 0.25).abs() < 1e-12);
        assert_eq!(prox.proximity(99), None, "absent node scores None");

        // Fused: node 3 (2 hops) first, node 0 (seed) second.
        let fused = vec![item(3, 0.030), item(0, 0.028), item(9, 0.020)];
        let reranker = GraphProximityReranker::new(prox, 1.0);
        let out = reranker.rerank(&fused);
        // Seed node 0: 0.028 * (1 + 1.0) = 0.056 beats 0.030 * 1.25 = 0.0375.
        assert_eq!(out[0].node_id, 0);
        assert_eq!(out[1].node_id, 3);
        assert_eq!(out[2].node_id, 9, "unknown node keeps fused score");
        assert!((out[2].score - 0.020).abs() < 1e-12);
        // Deterministic: identical input -> identical output.
        let again = reranker.rerank(&fused);
        assert_eq!(out, again);
    }

    #[test]
    fn graph_proximity_zero_boost_matches_identity() {
        let prox = HopProximity::from_topology(&line_topology(), &[0]);
        let fused = vec![item(3, 0.030), item(0, 0.028)];
        let out = GraphProximityReranker::new(prox, 0.0).rerank(&fused);
        assert_eq!(out, fused, "boost 0 must degenerate to the fused order");
    }

    #[test]
    fn graph_proximity_reranker_empty_input_is_empty() {
        let prox = HopProximity::from_topology(&CsrTopology::default(), &[0]);
        let out = GraphProximityReranker::new(prox, 1.0).rerank(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn hybrid_rrf_with_identity_reranker_matches_plain_hybrid_rrf() {
        let bridge = crate::ruvector::RuVectorBridge::new();
        let dense = vec![item(0, 0.9), item(1, 0.8), item(2, 0.7)];
        let lexical = vec![item(2, 0.8), item(0, 0.6), item(1, 0.5)];
        let plain = bridge.hybrid_rrf(&dense, &lexical).unwrap();
        let reranked = bridge
            .hybrid_rrf_with_reranker(&dense, &lexical, &IdentityReranker)
            .unwrap();
        assert_eq!(plain, reranked);
    }

    #[test]
    fn hybrid_rrf_with_reranker_empty_lists_ok_mismatch_errs() {
        let bridge = crate::ruvector::RuVectorBridge::new();
        let empty = bridge
            .hybrid_rrf_with_reranker(&[], &[], &IdentityReranker)
            .unwrap();
        assert!(empty.is_empty());
        assert!(
            bridge
                .hybrid_rrf_with_reranker(
                    &[item(0, 1.0)],
                    &[item(0, 1.0), item(1, 0.5)],
                    &IdentityReranker
                )
                .is_err()
        );
    }

    #[test]
    fn hybrid_rrf_with_reranker_reorders_fused_list() {
        let bridge = crate::ruvector::RuVectorBridge::new();
        let dense = vec![item(1, 0.9), item(0, 0.8)];
        let lexical = vec![item(1, 0.9), item(0, 0.8)];
        // Fused order: 1 then 0 (node 1 wins both ranks).
        let prox = HopProximity::from_topology(&line_topology(), &[0]);
        let out = bridge
            .hybrid_rrf_with_reranker(&dense, &lexical, &GraphProximityReranker::new(prox, 1.0))
            .unwrap();
        assert_eq!(
            out[0].node_id, 0,
            "seed adjacency must outrank fused winner"
        );
        assert_eq!(out[1].node_id, 1);
    }
}
