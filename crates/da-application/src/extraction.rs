//! Extraction use case — parse → extract entities → write to graph.
//!
//! ADR-038 Module B: extract textually mentioned entities from paper sections.
//! Phase 3 Slice 2: wires Extractor port to IngestUseCase graph writes.

use da_domain::vid;
use da_ports::extractor::Extractor;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;

/// Extraction use case: given a parsed article, extract entities + write to graph.
pub struct ExtractionUseCase {
    pub extractor: Box<dyn Extractor>,
    pub graph_store: Box<dyn DirectGraphStore>,
    /// Optional embedder for entity label embeddings (Phase 3 GNN readiness).
    /// When Some, each unique entity label gets a bge-m3 embedding written
    /// to the Entity node's vector index.
    pub embedder: Option<Box<dyn da_ports::embedder::Embedder>>,
}

/// Result of extracting entities from one paper.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub paper_id: String,
    pub entities_extracted: usize,
    pub entity_types: Vec<String>,
    pub graph_node_ids: Vec<u64>,
    pub mentions_edges: usize,
    pub found_in_edges: usize,
}

impl ExtractionUseCase {
    pub fn new(extractor: Box<dyn Extractor>, graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            extractor,
            graph_store,
            embedder: None,
        }
    }

    /// Attach an embedder for entity label embeddings (Phase 3).
    pub fn with_embedder(mut self, embedder: Box<dyn da_ports::embedder::Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Extract entities from a parsed article and write Entity nodes to graph.
    /// ADR-038: entities are textually mentioned, grounded to source spans.
    /// Links each Entity to the Paper via MENTIONS edge (bibliographic).
    pub async fn extract_from_parsed(
        &self,
        parsed: &ParsedArticle,
    ) -> anyhow::Result<ExtractionResult> {
        // 1. Build (title, text) pairs from parsed sections
        let sections: Vec<(String, String)> = parsed
            .sections
            .iter()
            .map(|s| (s.title.clone(), s.text.clone()))
            .collect();

        // 2. Extract via Extractor port
        let extracted = self.extractor.extract(&sections).await?;
        tracing::info!(
            paper_id = %parsed.paper_id,
            extractor = self.extractor.name(),
            count = extracted.len(),
            "Entities extracted"
        );

        // 3. Find the Paper node by arxiv_id (to link entities via MENTIONS)
        let paper_node_id = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", &parsed.paper_id)
            .await;

        // 4. Write Entity nodes to graph (HOT path) + MENTIONS edges
        let now = chrono::Utc::now().timestamp();
        let mut node_ids = Vec::new();
        let mut entity_types = Vec::new();
        let mut mentions_edges = 0usize;
        let mut found_in_edges = 0usize;
        for ext in &extracted {
            let entity_vid = vid::entity_vid(ext.entity_type.as_str(), &ext.label);
            // Idempotent: check if entity already exists
            let node_id = match self
                .graph_store
                .find_node_by_string_property("Entity", "vid", &entity_vid)
                .await
            {
                Some(existing) => existing,
                None => {
                    let new_node = self.graph_store.create_node("Entity").await?;
                    self.graph_store
                        .set_node_property_string(new_node, "vid", entity_vid.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(new_node, "label", ext.label.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(
                            new_node,
                            "entity_type",
                            ext.entity_type.as_str().to_string(),
                        )
                        .await?;
                    if !ext.section_title.is_empty() {
                        self.graph_store
                            .set_node_property_string(
                                new_node,
                                "section",
                                ext.section_title.clone(),
                            )
                            .await?;
                    }
                    // Source span (for evidence grounding — Phase 3 Slice 3+)
                    self.graph_store
                        .set_node_property_int(new_node, "char_start", ext.char_start as i64)
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "char_end", ext.char_end as i64)
                        .await?;
                    self.graph_store
                        .set_node_property_string(new_node, "surface", ext.surface.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "valid_from", now)
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "schema_version", 1)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "evidence_ready", false)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "import_eligible", false)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "retrieval_eligible", true)
                        .await?;
                    new_node
                }
            };
            // Link Entity to Paper via MENTIONS edge (if Paper node exists)
            if let Some(paper_id) = paper_node_id {
                let edge_id = self
                    .graph_store
                    .create_edge(
                        paper_id,
                        node_id,
                        da_domain::relation::bibliographic::MENTIONS,
                    )
                    .await?;
                // Set edge weight = extraction confidence (Phase 3 GNN readiness)
                // Rule-based extractor has uniform confidence 1.0
                self.graph_store
                    .set_edge_property_float(edge_id, "weight", 1.0)
                    .await?;
                mentions_edges += 1;
            }

            // Link Entity to Section via FOUND_IN edge (evidence grounding).
            // Enables: retrieval by section, PPR adjacency through Section nodes,
            // and evidence chain construction from Entity → Section → Work.
            if !ext.section_title.is_empty() {
                // Find Section node by title (hexagonal: no Cypher in application).
                if let Some(section_node) = self
                    .graph_store
                    .find_node_by_string_property("Section", "title", &ext.section_title)
                    .await
                {
                    let _ = self
                        .graph_store
                        .create_edge(
                            node_id,
                            section_node,
                            da_domain::relation::structure::FOUND_IN,
                        )
                        .await;
                    found_in_edges += 1;
                }
            }
            node_ids.push(node_id);
            entity_types.push(ext.entity_type.as_str().to_string());
        }

        // Phase 3: Embed entity labels if embedder is available.
        // Each unique entity label gets a bge-m3 embedding stored as a vector
        // on the Entity node. This enables GNN similarity search and PPR.
        let mut embeddings_written = 0usize;
        if let Some(ref embedder) = self.embedder {
            let mut seen_labels: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for (i, ext) in extracted.iter().enumerate() {
                let label_key = ext.label.to_lowercase();
                if seen_labels.contains(&label_key) {
                    continue; // skip duplicate labels
                }
                seen_labels.insert(label_key);

                let node_id = node_ids.get(i).copied();
                if let Some(nid) = node_id {
                    match embedder.embed(&ext.label).await {
                        Ok(vec) => {
                            if let Err(e) = self
                                .graph_store
                                .add_vector("Entity", "embedding", nid, vec)
                                .await
                            {
                                tracing::warn!(node_id = nid, error = %e, "Entity embedding write failed");
                            } else {
                                embeddings_written += 1;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(label = %ext.label, error = %e, "Entity embed failed");
                        }
                    }
                }
            }
        }

        tracing::info!(
            paper_id = %parsed.paper_id,
            entities = extracted.len(),
            mentions_edges,
            found_in_edges,
            embeddings_written,
            paper_linked = paper_node_id.is_some(),
            "Extraction complete"
        );

        Ok(ExtractionResult {
            paper_id: parsed.paper_id.clone(),
            entities_extracted: extracted.len(),
            entity_types,
            graph_node_ids: node_ids,
            mentions_edges,
            found_in_edges,
        })
    }
}
