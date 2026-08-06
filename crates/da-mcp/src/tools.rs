//! MCP tool handlers for daily-archive knowledge base (ADR-054 Wave 1).
//!
//! Tools are read-only — no write operations. D127 import gate is never
//! affected. All tools use `DirectGraphStore` trait methods.

use serde::{Deserialize, Serialize};
use kg_storage::DirectGraphStore;

// ═══════════════════════════════════════════════════════════════
// Tool parameter/output types
// ═══════════════════════════════════════════════════════════════

/// Parameters for `get_schema` tool.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GetSchemaParams {
    /// If true, include property key examples sampled from the graph.
    #[serde(default)]
    pub include_samples: bool,
}

/// Output for `get_schema` tool.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SchemaOutput {
    /// Node labels present in the graph (from da_domain::registered_node_labels).
    pub node_labels: Vec<String>,
    /// Edge types present in the graph (from da_domain::relation constants).
    pub edge_types: Vec<String>,
    /// Total node count (0 if include_samples is false).
    pub total_nodes: usize,
    /// Total edge count (0 if include_samples is false).
    pub total_edges: usize,
}

/// Output for `get_graph_stats` tool.
#[derive(Serialize, schemars::JsonSchema)]
pub struct GraphStatsOutput {
    pub total_nodes: usize,
    pub total_edges: usize,
    /// Node counts by label (best-effort — walks known labels).
    pub nodes_by_label: Vec<(String, usize)>,
    /// Snapshot file path if snapshot mode.
    pub snapshot_path: Option<String>,
}

// ═══════════════════════════════════════════════════════════════
// Tool implementations
// ═══════════════════════════════════════════════════════════════

/// Get the graph schema: node labels + edge types + optional counts.
///
/// Schema is loaded dynamically from YAML ontology (ADR-050):
/// - node labels via `registered_node_labels_or_yaml`
/// - edge types via `registered_edge_types_or_yaml`
pub async fn get_schema(
    store: &dyn DirectGraphStore,
    params: GetSchemaParams,
) -> anyhow::Result<SchemaOutput> {
    let node_labels: Vec<String> = da_domain::schema::registered_node_labels_or_yaml();

    let edge_types: Vec<String> = da_domain::schema::registered_edge_types_or_yaml();

    let (total_nodes, total_edges) = if params.include_samples {
        (store.node_count().await, store.edge_count().await)
    } else {
        (0, 0)
    };

    Ok(SchemaOutput {
        node_labels,
        edge_types,
        total_nodes,
        total_edges,
    })
}

/// Get graph statistics: node/edge counts + per-label breakdown.
pub async fn get_graph_stats(
    store: &dyn DirectGraphStore,
    snapshot_path: Option<&str>,
) -> anyhow::Result<GraphStatsOutput> {
    let total_nodes = store.node_count().await;
    let total_edges = store.edge_count().await;

    // Walk known labels for per-label counts.
    let mut nodes_by_label = Vec::new();
    for label in da_domain::registered_node_labels() {
        let count = store.get_nodes_by_label(label).await.len();
        if count > 0 {
            nodes_by_label.push((label.to_string(), count));
        }
    }

    Ok(GraphStatsOutput {
        total_nodes,
        total_edges,
        nodes_by_label,
        snapshot_path: snapshot_path.map(|s| s.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_schema_returns_registered_labels() {
        let store = kg_storage::MockGraphStore::new();
        let result = get_schema(&store, GetSchemaParams::default())
            .await
            .unwrap();
        // Must include core labels.
        assert!(result.node_labels.contains(&"Paper".to_string()));
        assert!(result.node_labels.contains(&"Section".to_string()));
        assert!(result.node_labels.contains(&"Entity".to_string()));
        assert!(result.edge_types.contains(&"CITES".to_string()));
        // No samples requested → zero counts.
        assert_eq!(result.total_nodes, 0);
    }

    #[tokio::test]
    async fn test_get_schema_with_samples_returns_counts() {
        let store = kg_storage::MockGraphStore::new();
        // Create a few nodes so counts are non-zero.
        store.create_node("Paper").await.unwrap();
        store.create_node("Section").await.unwrap();
        let result = get_schema(
            &store,
            GetSchemaParams {
                include_samples: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.total_nodes, 2);
    }

    #[tokio::test]
    async fn test_get_graph_stats_empty_graph() {
        let store = kg_storage::MockGraphStore::new();
        let result = get_graph_stats(&store, None).await.unwrap();
        assert_eq!(result.total_nodes, 0);
        assert_eq!(result.total_edges, 0);
        assert!(result.nodes_by_label.is_empty());
        assert!(result.snapshot_path.is_none());
    }

    #[tokio::test]
    async fn test_get_graph_stats_with_nodes() {
        let store = kg_storage::MockGraphStore::new();
        store.create_node("Paper").await.unwrap();
        store.create_node("Paper").await.unwrap();
        store.create_node("Section").await.unwrap();
        let result = get_graph_stats(&store, Some("test.sgsnap"))
            .await
            .unwrap();
        assert_eq!(result.total_nodes, 3);
        assert_eq!(result.snapshot_path.as_deref(), Some("test.sgsnap"));
        // nodes_by_label should have Paper: 2 and Section: 1.
        let paper_count = result
            .nodes_by_label
            .iter()
            .find(|(l, _)| l == "Paper")
            .map(|(_, c)| *c)
            .unwrap_or(0);
        assert_eq!(paper_count, 2);
        let section_count = result
            .nodes_by_label
            .iter()
            .find(|(l, _)| l == "Section")
            .map(|(_, c)| *c)
            .unwrap_or(0);
        assert_eq!(section_count, 1);
    }
}
