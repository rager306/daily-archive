//! DirectGraphStore → samyama_graph_algorithms::GraphView adapter.
//!
//! Lives in kg-storage because it depends on:
//!   1. our DirectGraphStore trait (clean, hexagonal)
//!   2. samyama_graph_algorithms::GraphView (vendor type)
//!
//! Any implementor of DirectGraphStore + GraphStore can be projected
//! into a CSR GraphView and fed to page_rank, WCC, BFS, Dijkstra, etc.
//! — all without leaking samyama types back into da-application.
//!
//! Returned GraphView owns its own memory; later mutation of the
//! DirectGraphStore has no effect on the snapshot.

use std::collections::HashMap;

use samyama_graph_algorithms::{GraphView, NodeId as AlgoNodeId};

use crate::traits::DirectGraphStore;

/// Adapter: project any DirectGraphStore into a CSR GraphView.
///
/// `label` and `edge_type` accept `None` to include all labels/edges
/// (full graph projection). `weight_property`, when provided, reads
/// float edge properties via `get_edge_property_string` parse path;
/// when None, weights default to 1.0 per edge (unweighted view).
///
/// # Example
/// ```no_run
/// use kg_storage::adapter::graph_view_from;
/// use kg_storage::mock::MockGraphStore;
/// use samyama_graph_algorithms::{page_rank, PageRankConfig};
/// # async fn run() {
/// let mut store = MockGraphStore::new();
/// let view = graph_view_from(&store, Some("Paper"), Some("CITES"), None).await.unwrap();
/// let ranks = page_rank(&view, &PageRankConfig::default());
/// # }
/// ```
pub async fn graph_view_from<T>(
    store: &T,
    label: Option<&str>,
    edge_type: Option<&str>,
    weight_property: Option<&str>,
) -> Result<GraphView, AdapterError>
where
    T: DirectGraphStore + ?Sized,
{
    // 1. Collect relevant node ids + build index maps.
    let node_ids: Vec<AlgoNodeId> = match label {
        Some(l) => store.get_nodes_by_label(l).await,
        None => {
            // No label filter: collect via node_count + iterate 0..count.
            // NOTE: DirectGraphStore is ID-opaque; without a label filter we
            // still need labels to find nodes. Walk ALL labels is not exposed
            // via the trait, so we fall back to node_count and assume dense
            // ids 0..count. This is correct for MockGraphStore and for
            // SamyamaGraphStore (both use dense u64 ids).
            let count = store.node_count().await;
            (0..count as u64).collect()
        }
    };
    if node_ids.is_empty() {
        return Ok(GraphView::from_adjacency_list(
            0,
            Vec::new(),
            HashMap::new(),
            Vec::new(),
            Vec::new(),
            None,
        ));
    }

    let node_count = node_ids.len();
    let index_to_node: Vec<AlgoNodeId> = node_ids.clone();
    let mut node_to_index: HashMap<AlgoNodeId, usize> = HashMap::with_capacity(node_count);
    for (i, id) in node_ids.iter().enumerate() {
        node_to_index.insert(*id, i);
    }

    // 2. Collect outgoing/incoming adjacency per node, filtering by edge_type.
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut weights_per_node: Vec<Vec<f64>> = vec![Vec::new(); node_count];

    for (idx, &src_id) in index_to_node.iter().enumerate() {
        let edges = store.get_outgoing_edges(src_id).await;
        for (tgt_id, etype) in edges {
            if edge_type.is_some_and(|wanted| etype != wanted) {
                continue;
            }
            // Only include edges whose target is also in the projected node set.
            if let Some(&tgt_idx) = node_to_index.get(&tgt_id) {
                // Skip self-loops early: they break algorithm assumptions
                // (PageRank, WCC) and would desync weights from adjacency.
                if tgt_idx == idx {
                    continue;
                }
                let w = match weight_property {
                    Some(name) => {
                        // Look up the real edge_id via get_edges_between, then
                        // read the float property. If the adapter doesn't
                        // override get_edges_between (returns empty) or the
                        // property is absent, fall back to 1.0 with a warning.
                        let edges_between = store.get_edges_between(src_id, tgt_id, &etype).await;
                        let mut found_weight: Option<f64> = None;
                        for (eid, _) in &edges_between {
                            if let Some(val) = store.get_edge_property_float(*eid, name).await {
                                found_weight = Some(val);
                                break;
                            }
                        }
                        match found_weight {
                            Some(v) => v,
                            None => {
                                tracing::debug!(
                                    src = src_id,
                                    tgt = tgt_id,
                                    edge_type = %etype,
                                    weight_property = name,
                                    "edge weight property not found; using default 1.0"
                                );
                                1.0
                            }
                        }
                    }
                    None => 1.0,
                };
                outgoing[idx].push(tgt_idx);
                incoming[tgt_idx].push(idx);
                weights_per_node[idx].push(w);
            }
        }
    }

    // 3. Fold weights to None if every edge weight is 1.0 (unweighted).
    let all_default = weights_per_node
        .iter()
        .flatten()
        .all(|w| (*w - 1.0).abs() < f64::EPSILON);
    let weights = if all_default {
        None
    } else {
        Some(weights_per_node)
    };

    Ok(GraphView::from_adjacency_list(
        node_count,
        index_to_node,
        node_to_index,
        outgoing,
        incoming,
        weights,
    ))
}

/// Error returned by the adapter. Currently only used for future
/// weight-property lookups; the happy path is infallible.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("adapter internal error: {0}")]
    Internal(String),
}
