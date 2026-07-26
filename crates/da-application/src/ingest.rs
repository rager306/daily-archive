//! Ingest pipeline use case.
//!
//! ADR-037 §4.1: Source → Fetch → Parse → Preprocess → Structure → Embed → Catalog.
//! ADR-040 §1: Samyama Graph as catalog store.
//! ADR-039: Lifecycle — starts as [proposed], needs canary-10 validation.

use da_domain::paper::Paper;
use da_ports::graph_store::GraphStore;
use da_ports::parser::ParserPort;
use da_ports::embedder::Embedder;
use serde::{Deserialize, Serialize};

/// Ingest pipeline dependencies (injected ports).
pub struct IngestUseCase {
    parser: Box<dyn ParserPort>,
    embedder: Box<dyn Embedder>,
    graph_store: Box<dyn GraphStore>,
}

/// Result of ingesting one paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub paper_id: String,
    pub title: String,
    pub body_chars: usize,
    pub vector_dimensions: usize,
    pub graph_written: bool,
    pub evidence_ready: bool,
    pub import_eligible: bool, // always false (D127)
}

impl IngestUseCase {
    pub fn new(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Box<dyn GraphStore>,
    ) -> Self {
        Self { parser, embedder, graph_store }
    }

    /// Ingest a single PDF: parse → embed → write to graph.
    ///
    /// This is the simplest end-to-end path proving the Rust pipeline works.
    /// Phase 2: parse + embed + catalog. Phase 3: add extraction.
    pub async fn ingest_pdf(&self, pdf_path: &str, paper_id: &str) -> anyhow::Result<IngestResult> {
        tracing::info!(paper_id, pdf_path, "Starting ingest");

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
        tracing::info!(
            paper_id,
            dimensions = vector.len(),
            "Embedded"
        );

        // 3. Create domain Paper
        let mut paper = Paper::new(paper_id, &parsed.title);
        paper.abstract_text = if parsed.abstract_text.is_empty() {
            None
        } else {
            Some(parsed.abstract_text.clone())
        };
        paper.pdf_hash = Some(parsed.pdf_hash.clone());

        // 4. Upsert to Samyama Graph (manual upsert — Samyama MERGE doesn't support inline props)
        let vid = paper.vid.clone();

        // MERGE is idempotent — no separate exists check needed (Samyama supports MERGE since v1.0)

        // ADR-041: Direct GraphStore API (HOT path) — no Cypher for batch writes.
        // Note: for now we still use GraphStore port's query() method since
        // da-application doesn't have direct access to SamyamaGraphStore.
        // Phase 3 will add a DirectGraphStorePort for hot-path operations.
        let escaped_title = paper.title.replace('"', "'");
        let vector_json = serde_json::to_string(&vector)?;
        let merge_cypher = format!(
            "MERGE (n:Paper {{vid: \"{}\"}}) \
             ON CREATE SET n.arxiv_id = \"{}\", n.title = \"{}\", n.pdf_hash = \"{}\", n.valid_from = {}, n.schema_version = {}, n.evidence_ready = false, n.import_eligible = false, n.embedding = {} \
             ON MATCH SET n.title = \"{}\", n.pdf_hash = \"{}\", n.embedding = {} \
             RETURN n.vid",
            vid, paper.arxiv_id, escaped_title, parsed.pdf_hash, paper.valid_from, paper.schema_version, vector_json,
            escaped_title, parsed.pdf_hash, vector_json
        );
        self.graph_store.query("default", &merge_cypher).await?;
        tracing::info!(paper_id, vid = %vid, "Graph upserted (MERGE via Cypher — Phase 3 will use direct API)");

        Ok(IngestResult {
            paper_id: paper_id.to_string(),
            title: paper.title.clone(),
            body_chars: parsed.body_text.len(),
            vector_dimensions: vector.len(),
            graph_written: true,
            evidence_ready: false,  // Phase 3: evidence grounding
            import_eligible: false, // D127: always false
        })
    }

    /// Check infrastructure health.
    pub async fn health_check(&self) -> bool {
        self.graph_store.health().await.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    // Integration tests require live services (GROBID, fd_api, Samyama)
    // Run manually: cargo test -- --ignored
}
