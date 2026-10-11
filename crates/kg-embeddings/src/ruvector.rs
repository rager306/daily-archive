//! Graph-aware retrieval bridge (kg-embeddings).
//!
//! Functional replacement for the former compile-time placeholder. Composes
//! universal retrieval capabilities without touching graph stores:
//!
//! - **RRF hybrid fusion** over ranked lists (dense + lexical) via
//!   [`ruvector_hybrid::RrfHybridIndex`].
//! - **Feature-gated PPR** over a plain CSR topology via
//!   [`ruvector_solver::forward_push`] (mirrors `kg-algorithms::ppr`, but
//!   operates on adjacency data instead of a GraphView, keeping this crate
//!   universal and dependency-light).
//!
//! The bridge is **not** a prompt optimizer and never authorizes import or
//! promotion. It only ranks/reranks existing candidates.

use std::collections::HashMap;

/// Error taxonomy for the graph-aware bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// PPR requires at least one seed when the graph is non-empty.
    EmptySeeds,
    /// A seed index is outside the CSR node range.
    MissingSeed { node_id: u64 },
    /// The solver rejected the input or hit numerical instability.
    Solver { message: String },
    /// Ranked lists have mismatched lengths for fusion.
    RankLengthMismatch { dense: usize, lexical: usize },
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySeeds => write!(f, "ppr requires at least one seed"),
            Self::MissingSeed { node_id } => {
                write!(f, "seed NodeId {node_id} not found in topology")
            }
            Self::Solver { message } => write!(f, "forward push solver error: {message}"),
            Self::RankLengthMismatch { dense, lexical } => {
                write!(f, "ranked lists differ in length ({dense} vs {lexical})")
            }
        }
    }
}

impl std::error::Error for BridgeError {}

/// Plain CSR adjacency for PPR: `offsets[i]..offsets[i+1]` indexes `neighbors`.
#[derive(Debug, Clone, Default)]
pub struct CsrTopology {
    pub offsets: Vec<usize>,
    pub neighbors: Vec<u64>,
}

/// A ranked retrieval result.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedItem {
    pub node_id: u64,
    pub score: f64,
}

/// Graph-aware retrieval bridge.
#[derive(Debug)]
pub struct RuVectorBridge {
    /// Feature flag: PPR solver available (set when the `ppr` feature compiles).
    ppr_enabled: bool,
}

impl RuVectorBridge {
    /// Build a bridge. PPR support depends on the `ppr` feature.
    pub fn new() -> Self {
        Self {
            ppr_enabled: cfg!(feature = "ppr"),
        }
    }

    /// Whether the PPR solver is compiled in.
    pub fn ppr_enabled(&self) -> bool {
        self.ppr_enabled
    }

    /// Reciprocal-rank fusion of a dense and a lexical ranked list.
    ///
    /// Both lists must contain the same item ids (in possibly different order).
    /// Returns fused ranks by descending combined RRF score (k = 60).
    pub fn hybrid_rrf(
        &self,
        dense: &[RankedItem],
        lexical: &[RankedItem],
    ) -> Result<Vec<RankedItem>, BridgeError> {
        if dense.len() != lexical.len() {
            return Err(BridgeError::RankLengthMismatch {
                dense: dense.len(),
                lexical: lexical.len(),
            });
        }
        let mut fused: Vec<RankedItem> = Vec::with_capacity(dense.len());
        let mut dense_rank: HashMap<u64, usize> = HashMap::new();
        let mut lexical_rank: HashMap<u64, usize> = HashMap::new();
        for (rank, item) in dense.iter().enumerate() {
            dense_rank.insert(item.node_id, rank);
        }
        for (rank, item) in lexical.iter().enumerate() {
            lexical_rank.insert(item.node_id, rank);
        }
        for item in dense {
            let rank = lexical_rank
                .get(&item.node_id)
                .copied()
                .unwrap_or(usize::MAX);
            let score =
                1.0 / (60.0 + dense_rank[&item.node_id] as f64) + 1.0 / (60.0 + rank as f64);
            fused.push(RankedItem {
                node_id: item.node_id,
                score,
            });
        }
        fused.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(fused)
    }

    /// RRF fusion followed by a reranking stage (ADR-073).
    ///
    /// The fused list is identical to [`Self::hybrid_rrf`]; `reranker` then
    /// re-scores it with evidence the rank fusion cannot see (e.g. graph
    /// proximity, see [`crate::rerank`]). Passing
    /// [`crate::rerank::IdentityReranker`] reproduces `hybrid_rrf` exactly.
    pub fn hybrid_rrf_with_reranker(
        &self,
        dense: &[RankedItem],
        lexical: &[RankedItem],
        reranker: &dyn crate::rerank::Reranker,
    ) -> Result<Vec<RankedItem>, BridgeError> {
        let fused = self.hybrid_rrf(dense, lexical)?;
        Ok(reranker.rerank(&fused))
    }

    /// Personalized PageRank over a plain CSR topology (feature-gated).
    ///
    /// - Empty topology returns `Ok(vec![])`.
    /// - Unknown seeds fail soft with [`BridgeError::MissingSeed`].
    #[cfg(feature = "ppr")]
    pub fn ppr(
        &self,
        topology: &CsrTopology,
        seeds: &[u64],
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<RankedItem>, BridgeError> {
        use ruvector_solver::forward_push::ForwardPushSolver;
        use ruvector_solver::traits::SublinearPageRank;
        use ruvector_solver::types::CsrMatrix;

        let node_count = topology.offsets.len().saturating_sub(1);
        if node_count == 0 {
            return Ok(Vec::new());
        }
        if seeds.is_empty() {
            return Err(BridgeError::EmptySeeds);
        }
        let mut dense_seeds = Vec::with_capacity(seeds.len());
        let mass = 1.0 / seeds.len() as f64;
        for &seed in seeds {
            if seed as usize >= node_count {
                return Err(BridgeError::MissingSeed { node_id: seed });
            }
            dense_seeds.push((seed as usize, mass));
        }
        // Build a unit-value CSR (weights folded to 1.0, topology walk unchanged).
        let row_ptr = topology.offsets.clone();
        let col_indices = topology
            .neighbors
            .iter()
            .map(|n| *n as usize)
            .collect::<Vec<_>>();
        let values = vec![1.0_f64; col_indices.len()];
        let matrix = CsrMatrix {
            row_ptr,
            col_indices,
            values,
            rows: node_count,
            cols: node_count,
        };
        let solver = ForwardPushSolver::new(alpha, epsilon);
        let dense_scores = solver
            .ppr_multi_seed(&matrix, &dense_seeds, alpha, epsilon)
            .map_err(|e| BridgeError::Solver {
                message: e.to_string(),
            })?;
        let mut out = dense_scores
            .into_iter()
            .map(|(idx, score)| RankedItem {
                node_id: idx as u64,
                score,
            })
            .collect::<Vec<_>>();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    /// PPR stub when the `ppr` feature is disabled — fails closed instead of
    /// silently degrading.
    #[cfg(not(feature = "ppr"))]
    pub fn ppr(
        &self,
        _topology: &CsrTopology,
        _seeds: &[u64],
        _alpha: f64,
        _epsilon: f64,
    ) -> Result<Vec<RankedItem>, BridgeError> {
        Err(BridgeError::Solver {
            message: "ppr feature disabled".into(),
        })
    }
}

impl Default for RuVectorBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "ppr")]
    fn line_topology() -> CsrTopology {
        // 0 -> 1 -> 2 -> 3 (chain)
        CsrTopology {
            offsets: vec![0, 1, 2, 3, 4],
            neighbors: vec![1, 2, 3, 0],
        }
    }

    #[test]
    fn hybrid_rrf_fuses_and_ranks_by_reciprocal_rank() {
        let bridge = RuVectorBridge::new();
        let dense = vec![
            RankedItem {
                node_id: 0,
                score: 0.9,
            },
            RankedItem {
                node_id: 1,
                score: 0.8,
            },
            RankedItem {
                node_id: 2,
                score: 0.7,
            },
        ];
        let lexical = vec![
            RankedItem {
                node_id: 2,
                score: 0.8,
            },
            RankedItem {
                node_id: 0,
                score: 0.6,
            },
            RankedItem {
                node_id: 1,
                score: 0.5,
            },
        ];
        let fused = bridge.hybrid_rrf(&dense, &lexical).unwrap();
        assert_eq!(fused[0].node_id, 0); // rank 1 in both lists
        assert_eq!(fused[1].node_id, 2); // rank 3 dense, rank 1 lexical
        assert_eq!(fused[2].node_id, 1); // rank 2 dense, rank 3 lexical
    }

    #[test]
    fn hybrid_rrf_rejects_length_mismatch() {
        let bridge = RuVectorBridge::new();
        assert!(
            bridge
                .hybrid_rrf(
                    &[RankedItem {
                        node_id: 0,
                        score: 1.0
                    }],
                    &[
                        RankedItem {
                            node_id: 0,
                            score: 1.0
                        },
                        RankedItem {
                            node_id: 1,
                            score: 0.5
                        }
                    ]
                )
                .is_err()
        );
    }

    #[cfg(feature = "ppr")]
    #[test]
    fn ppr_ranks_seed_neighbors_high() {
        let bridge = RuVectorBridge::new();
        assert!(bridge.ppr_enabled());
        let ranked = bridge.ppr(&line_topology(), &[0], 0.85, 1e-6).unwrap();
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].node_id, 0); // seed has highest score
        assert_eq!(ranked[1].node_id, 1); // direct neighbor next
    }

    #[cfg(feature = "ppr")]
    #[test]
    fn ppr_unknown_seed_fails_soft() {
        let bridge = RuVectorBridge::new();
        assert!(matches!(
            bridge.ppr(&line_topology(), &[99], 0.85, 1e-6),
            Err(BridgeError::MissingSeed { node_id: 99 })
        ));
    }

    #[cfg(feature = "ppr")]
    #[test]
    fn ppr_empty_topology_is_empty() {
        let bridge = RuVectorBridge::new();
        assert!(
            bridge
                .ppr(&CsrTopology::default(), &[0], 0.85, 1e-6)
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(feature = "ppr")]
    #[test]
    fn ppr_empty_seeds_fails_closed() {
        let bridge = RuVectorBridge::new();
        assert!(matches!(
            bridge.ppr(&line_topology(), &[], 0.85, 1e-6),
            Err(BridgeError::EmptySeeds)
        ));
    }
}
