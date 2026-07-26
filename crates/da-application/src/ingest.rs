//! Ingest pipeline use case.
//!
//! ADR-037 §4.1: Source → Fetch → Parse → Preprocess → Structure → Embed → Catalog.
//! ADR-040 §1: Samyama Graph as catalog store.
//! ADR-041 §2: HOT path — direct GraphStore API, no Cypher for batch writes.
//! ADR-039: Lifecycle — starts as [proposed], needs canary-10 validation.

use da_domain::paper::Paper;
use da_domain::schema::NodeSchemaDef;
use da_domain::vid;
use da_ports::embedder::Embedder;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParserPort;
use serde::{Deserialize, Serialize};

/// Ingest pipeline dependencies (injected ports).
pub struct IngestUseCase {
    pub parser: Box<dyn ParserPort>,
    pub embedder: Box<dyn Embedder>,
    pub graph_store: Box<dyn DirectGraphStore>,
}

/// Result of ingesting one paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub paper_id: String,
    pub title: String,
    pub body_chars: usize,
    pub section_count: usize,
    pub citation_count: usize,
    pub cites_resolved: usize, // citations with a resolvable arxiv_id
    pub vector_dimensions: usize,
    pub graph_node_id: Option<u64>,
    pub import_eligible: bool, // always false (D127)
}

impl IngestUseCase {
    pub fn new(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Box<dyn DirectGraphStore>,
    ) -> Self {
        Self {
            parser,
            embedder,
            graph_store,
        }
    }

    /// Ingest a single PDF: parse → embed → write to graph via HOT path.
    ///
    /// ADR-041: Uses direct GraphStore API (0.01ms per op) instead of Cypher MERGE.
    /// No parse/plan overhead. 100x faster than Cypher for batch writes.
    pub async fn ingest_pdf(&self, pdf_path: &str, paper_id: &str) -> anyhow::Result<IngestResult> {
        tracing::info!(paper_id, pdf_path, "Starting ingest (HOT path)");

        // 1. Parse via GROBID
        let parsed = self.parser.parse_pdf(pdf_path, paper_id).await?;
        tracing::info!(
            paper_id,
            title = %parsed.title,
            body_chars = parsed.body_text.len(),
            pdf_hash = %parsed.pdf_hash,
            "Parsed"
        );

        // 2. Embed the abstract + title
        let embed_text = if parsed.abstract_text.is_empty() {
            &parsed.title
        } else {
            &parsed.abstract_text
        };
        let vector = self.embedder.embed(embed_text).await?;
        tracing::info!(paper_id, dimensions = vector.len(), "Embedded");

        // 3. Create domain Paper (compute VID)
        let paper = Paper::new(paper_id, &parsed.title);
        let vid_str = vid::paper_vid(paper_id);
        let now = chrono::Utc::now().timestamp();

        // 3b. Validate against schema before writing (GRAPH-SCHEMA.md)
        let paper_schema = da_domain::paper::PaperSchema;
        let mut paper_props = std::collections::HashMap::new();
        paper_props.insert("vid".to_string(), serde_json::json!(vid_str));
        paper_props.insert("arxiv_id".to_string(), serde_json::json!(paper.arxiv_id));
        paper_props.insert("title".to_string(), serde_json::json!(paper.title));
        paper_props.insert("valid_from".to_string(), serde_json::json!(now));
        paper_schema.validate(&paper_props)?;

        // 4. HOT PATH: Direct GraphStore API — create Paper node
        let node_id = self.graph_store.create_node("Paper").await?;
        tracing::debug!(paper_id, node_id, "Node created");

        // Set properties directly (no Cypher!)
        self.graph_store
            .set_node_property_string(node_id, "vid", vid_str.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "arxiv_id", paper.arxiv_id.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "title", paper.title.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "pdf_hash", parsed.pdf_hash.clone())
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "valid_from", now)
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "schema_version", 1)
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "evidence_ready", false)
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "import_eligible", false) // D127: always false
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "retrieval_eligible", true) // D134: live for retrieval
            .await?;

        // Section + citation metadata (enables Phase 3 extraction queries)
        let section_count = parsed.sections.len();
        let citation_count = parsed.citations.len();
        self.graph_store
            .set_node_property_int(node_id, "section_count", section_count as i64)
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "citation_count", citation_count as i64)
            .await?;

        // 5. HOT PATH: Add vector to index
        self.graph_store
            .add_vector("Paper", "embedding", node_id, vector.clone())
            .await?;

        // 6. Create CITES edges for citations with resolvable arxiv_ids
        // (enables citation graph traversal — ADR-038 S_kn tri-source)
        // Idempotent: reuses existing Citation node if one with same arxiv_id exists.
        let mut cites_resolved = 0usize;
        let now_ts = chrono::Utc::now().timestamp();
        for citation in &parsed.citations {
            if let Some(ref arxiv_id) = citation.arxiv_id {
                let cited_vid = vid::paper_vid(arxiv_id);
                // Check if Citation node already exists (idempotent)
                let cited_node = match self
                    .graph_store
                    .find_node_by_string_property("Citation", "arxiv_id", arxiv_id)
                    .await
                {
                    Some(existing) => existing,
                    None => {
                        // Create new Citation node
                        let new_node = self.graph_store.create_node("Citation").await?;
                        self.graph_store
                            .set_node_property_string(new_node, "vid", cited_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(new_node, "arxiv_id", arxiv_id.clone())
                            .await?;
                        if let Some(ref title) = citation.title {
                            self.graph_store
                                .set_node_property_string(new_node, "title", title.clone())
                                .await?;
                        }
                        if let Some(ref doi) = citation.doi {
                            self.graph_store
                                .set_node_property_string(new_node, "doi", doi.clone())
                                .await?;
                        }
                        self.graph_store
                            .set_node_property_int(new_node, "valid_from", now_ts)
                            .await?;
                        self.graph_store
                            .set_node_property_int(new_node, "schema_version", 1)
                            .await?;
                        self.graph_store
                            .set_node_property_bool(new_node, "retrieval_eligible", true)
                            .await?;
                        new_node
                    }
                };
                self.graph_store
                    .create_edge(
                        node_id,
                        cited_node,
                        da_domain::relation::bibliographic::CITES,
                    )
                    .await?;
                cites_resolved += 1;
            }
        }

        tracing::info!(
            paper_id,
            node_id,
            vid = %vid_str,
            vector_dim = vector.len(),
            section_count,
            citation_count,
            cites_resolved,
            "Graph written (HOT path — direct API, no Cypher)"
        );

        Ok(IngestResult {
            paper_id: paper_id.to_string(),
            title: paper.title.clone(),
            body_chars: parsed.body_text.len(),
            section_count,
            citation_count,
            cites_resolved,
            vector_dimensions: vector.len(),
            graph_node_id: Some(node_id),
            import_eligible: false, // D127: always false
        })
    }

    /// Check infrastructure health.
    pub async fn health_check(&self) -> bool {
        self.graph_store.health().await.unwrap_or(false)
    }

    /// Get graph statistics.
    pub async fn graph_stats(&self) -> (usize, usize) {
        (
            self.graph_store.node_count().await,
            self.graph_store.edge_count().await,
        )
    }
}

#[cfg(test)]
mod tests {
    // Integration tests require live services (GROBID, fd_api, Samyama embedded)
}
