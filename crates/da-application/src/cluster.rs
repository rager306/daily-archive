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

use da_domain::cluster::{self, EntityClusterKey, EntityPapers};
use da_domain::relation::hypergraph::MEMBER_OF_CLUSTER;
use da_ports::graph_store::DirectGraphStore;
use sha2::{Digest, Sha256};

/// Result of cluster materialization.
pub struct ClusterResult {
    pub clusters_created: usize,
    pub clusters_reused: usize,
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
    pub graph_store: std::sync::Arc<dyn DirectGraphStore>,
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
}

impl ClusterUseCase {
    pub fn new(graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self::from_shared(std::sync::Arc::from(graph_store))
    }

    pub fn from_shared(graph_store: std::sync::Arc<dyn DirectGraphStore>) -> Self {
        Self {
            graph_store,
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
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
        // Normalize at the public boundary so direct callers and staged callers
        // share the same typed cluster identity contract.
        let mut normalized = EntityPapers::new();
        for (key, papers) in entity_papers {
            normalized
                .entry(cluster::normalize_entity_key(&key.entity_type, &key.label))
                .or_default()
                .extend(papers.iter().cloned());
        }
        let detected = cluster::detect_clusters(&normalized);
        tracing::info!(
            total_entities = entity_papers.len(),
            clusters_detected = detected.len(),
            "Cluster detection complete"
        );
        self.materialize_detected_clusters(&detected).await
    }

    pub async fn materialize_detected_clusters(
        &self,
        detected: &[da_domain::cluster::DetectedCluster],
    ) -> anyhow::Result<ClusterResult> {
        // Build entity key → node_id lookup from graph.
        let entity_node_ids = self.build_entity_node_lookup().await;

        let mut clusters_created = 0usize;
        let mut clusters_reused = 0usize;
        let mut member_edges_created = 0usize;
        let mut members_skipped = 0usize;

        // 3. Materialize each DetectedCluster
        for dc in detected {
            let vid = cluster_vid(dc);
            let cluster_node = match self
                .graph_store
                .find_node_by_string_property("ConceptCluster", "vid", &vid)
                .await
            {
                Some(node) => {
                    clusters_reused += 1;
                    node
                }
                None => {
                    let node = self.graph_store.create_node("ConceptCluster").await?;
                    self.graph_store
                        .set_node_property_string(node, "vid", vid)
                        .await?;
                    clusters_created += 1;
                    node
                }
            };

            // Set required properties
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
            for member in &dc.members {
                if let Some(&entity_node_id) = entity_node_ids.get(member) {
                    if self
                        .graph_store
                        .get_edges_between(entity_node_id, cluster_node, MEMBER_OF_CLUSTER)
                        .await
                        .is_empty()
                    {
                        let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                        crate::temporal_stamp::create_edge_with_temporal(
                            self.graph_store.as_ref(),
                            entity_node_id,
                            cluster_node,
                            MEMBER_OF_CLUSTER,
                            &temporal,
                        )
                        .await?;
                        member_edges_created += 1;
                    }
                } else {
                    members_skipped += 1;
                    tracing::warn!(entity_type = %member.entity_type, entity = %member.label, "Entity node not found, skipping cluster membership");
                }
            }
        }

        let clusters_skipped = members_skipped;

        tracing::info!(
            clusters_created,
            clusters_reused,
            member_edges_created,
            "Cluster materialization complete"
        );

        Ok(ClusterResult {
            clusters_created,
            clusters_reused,
            member_edges_created,
            clusters_skipped,
        })
    }

    /// Inventory Entity→paper co-occurrence from the live graph via Paper-[:MENTIONS]->Entity.
    ///
    /// Pure graph read (hexagonal: no Cypher). Papers without arxiv_id/vid are skipped.
    /// Entities missing label or entity_type are skipped (typed missingness, not invented).
    pub async fn inventory_entity_papers(&self) -> EntityPapers {
        use da_domain::relation::bibliographic::MENTIONS;
        let mut entity_papers = EntityPapers::new();
        let papers = self.graph_store.get_nodes_by_label("Paper").await;
        for paper in papers {
            let paper_id = match self
                .graph_store
                .get_node_property_string(paper, "arxiv_id")
                .await
                .or(self
                    .graph_store
                    .get_node_property_string(paper, "vid")
                    .await)
            {
                Some(id) if !id.trim().is_empty() => id,
                _ => continue,
            };
            for (target, edge_type) in self.graph_store.get_outgoing_edges(paper).await {
                if edge_type != MENTIONS {
                    continue;
                }
                let label = match self
                    .graph_store
                    .get_node_property_string(target, "label")
                    .await
                {
                    Some(l) if !l.trim().is_empty() => l,
                    _ => continue,
                };
                let entity_type = match self
                    .graph_store
                    .get_node_property_string(target, "entity_type")
                    .await
                {
                    Some(t) if !t.trim().is_empty() => t,
                    _ => continue,
                };
                let key = cluster::normalize_entity_key(&entity_type, &label);
                entity_papers
                    .entry(key)
                    .or_default()
                    .insert(paper_id.clone());
            }
        }
        entity_papers
    }

    /// Detect + materialize clusters from the live graph inventory (always-run).
    pub async fn materialize_from_graph(&self) -> anyhow::Result<(EntityPapers, ClusterResult)> {
        let entity_papers = self.inventory_entity_papers().await;
        let result = self.materialize_clusters(&entity_papers).await?;
        Ok((entity_papers, result))
    }

    /// Build entity label → node_id lookup by querying all Entity nodes.
    ///
    /// Uses get_nodes_by_label + get_node_property_string (hexagonal: no Cypher).
    async fn build_entity_node_lookup(&self) -> std::collections::HashMap<EntityClusterKey, u64> {
        let entity_node_ids = self.graph_store.get_nodes_by_label("Entity").await;
        let mut lookup = std::collections::HashMap::with_capacity(entity_node_ids.len());

        for node_id in entity_node_ids {
            let label = self
                .graph_store
                .get_node_property_string(node_id, "label")
                .await;
            let entity_type = self
                .graph_store
                .get_node_property_string(node_id, "entity_type")
                .await;
            if let (Some(label), Some(entity_type)) = (label, entity_type) {
                lookup.insert(
                    da_domain::cluster::normalize_entity_key(&entity_type, &label),
                    node_id,
                );
            }
        }

        tracing::debug!(entities_found = lookup.len(), "Entity node lookup built");
        lookup
    }
}

fn cluster_vid(cluster: &da_domain::cluster::DetectedCluster) -> String {
    let mut members = cluster
        .members
        .iter()
        .map(|member| format!("{}:{}", member.entity_type, member.label))
        .collect::<Vec<_>>();
    members.sort();
    let digest =
        Sha256::digest(format!("{}|{}", cluster.cluster_type, members.join("|")).as_bytes());
    format!("vid:hyper:{}", hex::encode(digest))
}
