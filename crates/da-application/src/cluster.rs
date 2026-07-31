//! Cluster use case — detect + materialize ConceptCluster nodes in graph.
//!
//! Runs co-occurrence analysis over extracted entities and writes
//! ConceptCluster nodes + MEMBER_OF_CLUSTER edges to the graph.
//!
//! This is the application-layer bridge between:
//! - da_domain::cluster::detect_clusters() (pure domain logic)
//! - DirectGraphStore (infrastructure)
//!
//! Layer 6 community detection (ADR-042 revised):
//!   ConceptCluster = derived semantic community, NOT evidence unit.
//!   MEMBER_OF_CLUSTER edge = community membership, NOT evidence participation.

use da_domain::cluster::{self, DetectedCluster, EntityPapers};
use da_domain::relation::hypergraph::MEMBER_OF_CLUSTER;
use da_ports::graph_store::DirectGraphStore;

/// Result of cluster materialization.
pub struct ClusterResult {
    pub clusters_created: usize,
    pub member_edges_created: usize,
    pub clusters_skipped: usize,
}

/// Use case: detect concept clusters from entity co-occurrence and write
/// ConceptCluster nodes + MEMBER_OF_CLUSTER edges to the graph.
///
/// This is a batch operation — run after extraction has populated Entity nodes
/// across multiple papers. The caller provides EntityPapers mapping
/// (entity label → entity type + set of paper IDs where it appears).
pub struct ClusterUseCase {
    pub graph_store: Box<dyn DirectGraphStore>,
}

impl ClusterUseCase {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Detect clusters from EntityPapers and materialize in graph.
    ///
    /// Creates ConceptCluster nodes with:
    ///   vid, label, cluster_type, retrieval_eligible=true, import_eligible=false (D127)
    ///
    /// Creates MEMBER_OF_CLUSTER edges: Entity → ConceptCluster
    pub async fn materialize_clusters(
        &self,
        entity_papers: &EntityPapers,
    ) -> anyhow::Result<ClusterResult> {
        // 1. Detect clusters using domain logic (pure function)
        let detected = cluster::detect_clusters(entity_papers);
        tracing::info!(
            total_entities = entity_papers.len(),
            clusters_detected = detected.len(),
            "Cluster detection complete"
        );

        // 2. Build entity label → node_id lookup from graph
        let entity_node_ids = self.build_entity_node_lookup().await;

        let mut clusters_created = 0usize;
        let mut member_edges_created = 0usize;
        let mut clusters_skipped = 0usize;

        // 3. Materialize each DetectedCluster
        for dc in &detected {
            let cluster_node = self.graph_store.create_node("ConceptCluster").await?;

            // Set required properties
            let vid = format!("vid:hyper:{}", slugify(&dc.label));
            self.graph_store
                .set_node_property_string(cluster_node, "vid", vid)
                .await?;
            self.graph_store
                .set_node_property_string(cluster_node, "label", dc.label.clone())
                .await?;
            self.graph_store
                .set_node_property_string(cluster_node, "cluster_type", dc.cluster_type.clone())
                .await?;
            self.graph_store
                .set_node_property_bool(cluster_node, "retrieval_eligible", true)
                .await?;
            self.graph_store
                .set_node_property_bool(cluster_node, "import_eligible", false) // D127
                .await?;
            self.graph_store
                .set_node_property_int(cluster_node, "schema_version", 1)
                .await?;

            // 4. Create MEMBER_OF_CLUSTER edges: Entity → ConceptCluster
            for (member_label, _member_type) in dc.members.iter().zip(dc.member_types.iter()) {
                if let Some(&entity_node_id) = entity_node_ids.get(member_label) {
                    let _ = self
                        .graph_store
                        .create_edge(entity_node_id, cluster_node, MEMBER_OF_CLUSTER)
                        .await;
                    member_edges_created += 1;
                } else {
                    // Entity node not found in graph — skip this member
                    tracing::debug!(entity = %member_label, "Entity node not found, skipping cluster membership");
                }
            }

            clusters_created += 1;
        }

        clusters_skipped = detected
            .iter()
            .map(|d| d.members.len())
            .sum::<usize>()
            .saturating_sub(member_edges_created);

        tracing::info!(
            clusters_created,
            member_edges_created,
            "Cluster materialization complete"
        );

        Ok(ClusterResult {
            clusters_created,
            member_edges_created,
            clusters_skipped,
        })
    }

    /// Build entity label → node_id lookup by querying all Entity nodes.
    ///
    /// Uses get_nodes_by_label + get_node_property_string (hexagonal: no Cypher).
    async fn build_entity_node_lookup(&self) -> std::collections::HashMap<String, u64> {
        let entity_node_ids = self.graph_store.get_nodes_by_label("Entity").await;
        let mut lookup = std::collections::HashMap::with_capacity(entity_node_ids.len());

        for node_id in entity_node_ids {
            if let Some(label) = self
                .graph_store
                .get_node_property_string(node_id, "label")
                .await
            {
                lookup.insert(label, node_id);
            }
        }

        tracing::debug!(entities_found = lookup.len(), "Entity node lookup built");
        lookup
    }
}

/// Convert a label to a filesystem/VID-safe slug.
fn slugify(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
