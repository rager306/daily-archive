//! Ontology-driven graph inventory (M322–M324).
//!
//! Production composition injects `OntologyRegistry`; the compatibility wrapper
//! uses compiled contracts only and performs no filesystem access.

use std::collections::BTreeMap;

use da_domain::{contract_edge_types, registered_node_labels};
use da_ports::graph_store::DirectGraphStore;
use serde::{Deserialize, Serialize};

/// Snapshot of node counts by ontology label + full edge-type histogram.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphInventory {
    /// Counts for every registered ontology label (including zeros).
    pub node_counts: BTreeMap<String, usize>,
    /// Outgoing edge types observed in the graph (wire strings → count).
    pub edge_type_histogram: BTreeMap<String, usize>,
    /// Sum of node_counts values.
    pub total_nodes: usize,
    /// Sum of edge histogram values.
    pub total_edges: usize,
}

impl GraphInventory {
    /// Count for a single label (0 if absent).
    pub fn nodes(&self, label: &str) -> usize {
        *self.node_counts.get(label).unwrap_or(&0)
    }

    /// Count for an edge wire string (0 if absent).
    pub fn edges(&self, edge_type: &str) -> usize {
        *self.edge_type_histogram.get(edge_type).unwrap_or(&0)
    }

    /// Compiled compatibility check. Production finalize uses the injected registry variant.
    pub fn missing_contract_edges(&self) -> Vec<String> {
        contract_edge_types()
            .into_iter()
            .filter(|e| !self.edge_type_histogram.contains_key(*e))
            .map(str::to_string)
            .collect()
    }

    pub fn missing_contract_edges_for(
        &self,
        registry: &kg_ontology::registry::OntologyRegistry,
    ) -> Vec<String> {
        registry
            .edge_types()
            .into_iter()
            .filter(|edge| !self.edge_type_histogram.contains_key(edge))
            .collect()
    }

    /// Compiled compatibility check. Production finalize uses the injected registry variant.
    pub fn unknown_edge_types(&self) -> Vec<String> {
        let known: std::collections::HashSet<&str> = contract_edge_types().into_iter().collect();
        self.edge_type_histogram
            .keys()
            .filter(|k| !known.contains(k.as_str()))
            .cloned()
            .collect()
    }

    pub fn unknown_edge_types_for(
        &self,
        registry: &kg_ontology::registry::OntologyRegistry,
    ) -> Vec<String> {
        let known: std::collections::HashSet<String> = registry.edge_types().into_iter().collect();
        self.edge_type_histogram
            .keys()
            .filter(|edge| !known.contains(*edge))
            .cloned()
            .collect()
    }
}

/// Build inventory by walking **ontology-registered** labels only.
///
/// Labels are not hard-coded in the caller: when `all_node_schemas()` gains a
/// type, inventory automatically queries it.
pub async fn collect_graph_inventory(store: &dyn DirectGraphStore) -> GraphInventory {
    let labels = registered_node_labels()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    collect_graph_inventory_for_labels(store, labels).await
}

pub async fn collect_graph_inventory_for_registry(
    store: &dyn DirectGraphStore,
    registry: &kg_ontology::registry::OntologyRegistry,
) -> GraphInventory {
    collect_graph_inventory_for_labels(store, registry.labels_owned()).await
}

async fn collect_graph_inventory_for_labels(
    store: &dyn DirectGraphStore,
    labels: Vec<String>,
) -> GraphInventory {
    let mut node_counts = BTreeMap::new();
    let mut all_node_ids: Vec<u64> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for label in labels {
        let ids = store.get_nodes_by_label(&label).await;
        node_counts.insert(label, ids.len());
        for id in ids {
            if seen_ids.insert(id) {
                all_node_ids.push(id);
            }
        }
    }

    let mut edge_type_histogram: BTreeMap<String, usize> = BTreeMap::new();
    for nid in &all_node_ids {
        let edges = store.get_outgoing_edges(*nid).await;
        for (_target, rel) in edges {
            *edge_type_histogram.entry(rel).or_default() += 1;
        }
    }

    let total_nodes = node_counts.values().sum();
    let total_edges = edge_type_histogram.values().sum();

    GraphInventory {
        node_counts,
        edge_type_histogram,
        total_nodes,
        total_edges,
    }
}

/// Render a compact markdown table for operators (no secrets).
pub fn render_inventory_markdown(inv: &GraphInventory) -> String {
    let mut s = String::from("## Node counts (ontology labels)\n\n");
    s.push_str("| Label | Count |\n|-------|------:|\n");
    for (label, n) in &inv.node_counts {
        if *n > 0 {
            s.push_str(&format!("| `{label}` | {n} |\n"));
        }
    }
    s.push_str(&format!("| **total** | **{}** |\n\n", inv.total_nodes));
    s.push_str("## Edge histogram (wire strings)\n\n");
    s.push_str("| Edge type | Count |\n|-----------|------:|\n");
    for (edge, n) in &inv.edge_type_histogram {
        s.push_str(&format!("| `{edge}` | {n} |\n"));
    }
    s.push_str(&format!("| **total** | **{}** |\n", inv.total_edges));
    let unknown = inv.unknown_edge_types();
    if !unknown.is_empty() {
        s.push_str("\n_Unknown edge types (not in edge_contracts): ");
        s.push_str(&unknown.join(", "));
        s.push_str("_\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::relation::{bibliographic, structure};
    use kg_storage::{DirectGraphStore, mock::MockGraphStore};

    #[test]
    fn inventory_accessors_default_zero() {
        let inv = GraphInventory {
            node_counts: BTreeMap::from([("Paper".into(), 2)]),
            edge_type_histogram: BTreeMap::from([(structure::HAS_PART.to_string(), 5)]),
            total_nodes: 2,
            total_edges: 5,
        };
        assert_eq!(inv.nodes("Paper"), 2);
        assert_eq!(inv.nodes("Section"), 0);
        assert_eq!(inv.edges(structure::HAS_PART), 5);
        assert_eq!(inv.edges(bibliographic::CITES), 0);
    }

    #[test]
    fn unknown_edge_types_flags_drift() {
        let inv = GraphInventory {
            node_counts: BTreeMap::new(),
            edge_type_histogram: BTreeMap::from([
                (structure::HAS_PART.to_string(), 1),
                ("NOT_IN_CONTRACT".into(), 3),
            ]),
            total_nodes: 0,
            total_edges: 4,
        };
        let u = inv.unknown_edge_types();
        assert_eq!(u, vec!["NOT_IN_CONTRACT".to_string()]);
        assert!(
            inv.missing_contract_edges()
                .iter()
                .any(|e| e == bibliographic::CITES)
        );
    }

    #[test]
    fn render_includes_only_nonzero_nodes() {
        let inv = GraphInventory {
            node_counts: BTreeMap::from([("Paper".into(), 1), ("Section".into(), 0)]),
            edge_type_histogram: BTreeMap::from([(structure::HAS_PART.to_string(), 2)]),
            total_nodes: 1,
            total_edges: 2,
        };
        let md = render_inventory_markdown(&inv);
        assert!(md.contains("`Paper`"));
        assert!(!md.contains("| `Section` | 0 |"));
        assert!(md.contains("hasPart"));
    }

    #[tokio::test]
    async fn injected_registry_drives_inventory_without_project_file_lookup() {
        let registry = kg_ontology::registry::OntologyRegistry::load_from_str(
            "node_schemas:\n  - label: QualitySignal\n    required_fields: [{name: vid, field_type: String}]\n",
            "fixture.yaml",
        )
        .unwrap();
        let store = MockGraphStore::new();
        let injected_label = "QualitySignal";
        store.create_node(injected_label).await.unwrap();
        store.create_node("Paper").await.unwrap();
        let inv = collect_graph_inventory_for_registry(&store, &registry).await;
        assert_eq!(inv.nodes("QualitySignal"), 1);
        assert_eq!(inv.nodes("Paper"), 0);
        assert_eq!(inv.total_nodes, 1);
    }

    #[test]
    fn registered_labels_drive_inventory_shape() {
        let labels = registered_node_labels();
        assert!(labels.len() >= 10);
        assert!(labels.contains(&"Paper"));
        let edges = contract_edge_types();
        assert!(edges.contains(&structure::HAS_PART));
        assert!(edges.contains(&bibliographic::MENTIONS));
    }
}
