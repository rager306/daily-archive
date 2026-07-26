//! Ingest pipeline use case.
//!
//! ADR-037 §4.1: Source → Fetch → Parse → Preprocess → Structure → Embed → Catalog.
//! ADR-040 §1: Samyama Graph as catalog store.
//! ADR-041 §2: HOT path — direct GraphStore API, no Cypher for batch writes.
//! ADR-039: Lifecycle — starts as [proposed], needs canary-10 validation.

use da_domain::paper::Paper;
use da_domain::vid;
use da_ports::parser::ParserPort;
use da_ports::embedder::Embedder;
use da_ports::graph_store::DirectGraphStore;
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

        // 5. HOT PATH: Add vector to index
        self.graph_store
            .add_vector("Paper", "embedding", node_id, vector.clone())
            .await?;

        tracing::info!(
            paper_id,
            node_id,
            vid = %vid_str,
            vector_dim = vector.len(),
            "Graph written (HOT path — direct API, no Cypher)"
        );

        Ok(IngestResult {
            paper_id: paper_id.to_string(),
            title: paper.title.clone(),
            body_chars: parsed.body_text.len(),
            vector_dimensions: vector.len(),
            graph_node_id: Some(node_id),
            import_eligible: false, // D127: always false
        })
    }

    /// Check infrastructure health.
    pub async fn health_check(&self) -> bool {
        use da_ports::graph_store::GraphStore;
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
