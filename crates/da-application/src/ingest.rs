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
    pub references_written: usize, // all citations as Reference nodes
    pub cites_resolved: usize,     // citations with a resolvable arxiv_id
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

        // ADR-043: extract scientific_domain from catalog path if available
        let primary_domain = extract_domain_from_path(pdf_path);
        // ADR-044: extract source code from catalog path for Source node
        let source_code = extract_source_from_path(pdf_path);
        tracing::debug!(
            paper_id,
            source_code = source_code.as_deref(),
            "Source detected"
        );

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

        // ADR-044: create Source node + FROM_SOURCE edge (Layer 0 provenance)
        if let Some(ref code) = source_code {
            // Idempotent: check if Source node already exists
            let source_node = self
                .graph_store
                .find_node_by_string_property("Source", "code", code)
                .await;
            let source_node = match source_node {
                Some(existing) => existing,
                None => {
                    let sn = self.graph_store.create_node("Source").await?;
                    self.graph_store
                        .set_node_property_string(sn, "vid", format!("vid:source:{code}"))
                        .await?;
                    self.graph_store
                        .set_node_property_string(sn, "code", code.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(sn, "source_type", "pdf".to_string())
                        .await?;
                    self.graph_store
                        .set_node_property_string(sn, "domain", "scientific_paper".to_string())
                        .await?;
                    self.graph_store
                        .set_node_property_int(sn, "reliability_tier", 2)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(sn, "retrieval_eligible", false)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(sn, "import_eligible", false) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(sn, "schema_version", 1)
                        .await?;
                    sn
                }
            };
            // Link Paper → Source via FROM_SOURCE edge
            let _ = self
                .graph_store
                .create_edge(
                    node_id,
                    source_node,
                    da_domain::relation::structure::FROM_SOURCE,
                )
                .await;
            tracing::debug!(paper_id, source_code = %code, "Source node linked");
        }

        // ADR-043: set scientific_domain fields (extracted from catalog path)
        if let Some(ref domain) = primary_domain {
            self.graph_store
                .set_node_property_string(node_id, "primary_scientific_domain", domain.clone())
                .await?;
            self.graph_store
                .set_node_property_string(node_id, "scientific_domains", domain.clone())
                .await?;
            self.graph_store
                .set_node_property_string(
                    node_id,
                    "domain_assignment_method",
                    "catalog_path".to_string(),
                )
                .await?;
            tracing::debug!(paper_id, domain = %domain, "Set primary_scientific_domain");
        }

        // Section + citation metadata (enables Phase 3 extraction queries)
        let section_count = parsed.sections.len();
        let citation_count = parsed.citations.len();
        self.graph_store
            .set_node_property_int(node_id, "section_count", section_count as i64)
            .await?;
        self.graph_store
            .set_node_property_int(node_id, "citation_count", citation_count as i64)
            .await?;

        // 4b. Create Section nodes (Layer 2 Structure — ONTOLOGY-DESIGN)
        // Each section gets a node with title, level, order, text.
        // Linked to Paper via hasPart edge (FaBiO frbr:part).
        let mut section_nodes = 0usize;
        for (i, section) in parsed.sections.iter().enumerate() {
            let section_node = self.graph_store.create_node("Section").await?;
            self.graph_store
                .set_node_property_string(
                    section_node,
                    "vid",
                    format!("vid:section:{}:{}", paper_id, i),
                )
                .await?;
            self.graph_store
                .set_node_property_string(section_node, "title", section.title.clone())
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "level", section.level as i64)
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "order", i as i64)
                .await?;
            self.graph_store
                .set_node_property_string(section_node, "work_vid", vid_str.clone())
                .await?;
            self.graph_store
                .set_node_property_bool(section_node, "retrieval_eligible", true)
                .await?;
            self.graph_store
                .set_node_property_bool(section_node, "import_eligible", false) // D127
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "schema_version", 1)
                .await?;
            // Truncate text to ~10000 bytes, UTF-8 safe (avoid mid-char panic)
            let text_trunc = section.text.get(..10000).unwrap_or(&section.text);
            self.graph_store
                .set_node_property_string(section_node, "text", text_trunc.to_string())
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "char_count", section.text.len() as i64)
                .await?;
            // Link Section to Paper via hasPart edge
            self.graph_store
                .create_edge(
                    node_id,
                    section_node,
                    da_domain::relation::structure::HAS_PART,
                )
                .await?;
            section_nodes += 1;
        }
        tracing::info!(paper_id, section_nodes, "Section nodes created");

        // 5. HOT PATH: Add vector to index
        self.graph_store
            .add_vector("Paper", "embedding", node_id, vector.clone())
            .await?;

        // 6. Create CITES edges for citations with resolvable arxiv_ids
        // (enables citation graph traversal — ADR-038 S_kn tri-source)
        // Idempotent: reuses existing Citation node if one with same arxiv_id exists.
        let mut cites_resolved = 0usize;
        let mut references_written = 0usize;
        let now_ts = chrono::Utc::now().timestamp();
        for citation in &parsed.citations {
            // Create Reference node for ALL citations (full bibliography).
            // Reference stores raw_text + metadata, even for unresolved citations.
            // Citation node is created separately for resolvable arxiv_ids.
            {
                let ref_vid = da_domain::vid::reference_vid(&citation.raw_text);
                let ref_node = self.graph_store.create_node("Reference").await?;
                self.graph_store
                    .set_node_property_string(ref_node, "vid", ref_vid)
                    .await?;
                self.graph_store
                    .set_node_property_string(ref_node, "raw_text", citation.raw_text.clone())
                    .await?;
                if let Some(ref arxiv_id) = citation.arxiv_id {
                    self.graph_store
                        .set_node_property_string(ref_node, "arxiv_id", arxiv_id.clone())
                        .await?;
                }
                if let Some(ref title) = citation.title {
                    self.graph_store
                        .set_node_property_string(ref_node, "title", title.clone())
                        .await?;
                }
                if let Some(ref doi) = citation.doi {
                    self.graph_store
                        .set_node_property_string(ref_node, "doi", doi.clone())
                        .await?;
                }
                self.graph_store
                    .set_node_property_int(ref_node, "valid_from", now_ts)
                    .await?;
                self.graph_store
                    .set_node_property_bool(ref_node, "retrieval_eligible", true)
                    .await?;
                self.graph_store
                    .set_node_property_bool(ref_node, "import_eligible", false) // D127
                    .await?;
                self.graph_store
                    .set_node_property_int(ref_node, "schema_version", 1)
                    .await?;
                // Link Paper → Reference via hasPart edge (FaBiO)
                let _ = self
                    .graph_store
                    .create_edge(node_id, ref_node, da_domain::relation::structure::HAS_PART)
                    .await;
                references_written += 1;
            }

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
                        self.graph_store
                            .set_node_property_bool(new_node, "import_eligible", false) // D127
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
            references_written,
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

/// Extract source code from catalog path.
///
/// Path format: `.../arxiv/<cat-dash>/<id>/...` → "arxiv"
///              `.../textbooks/<book>/...` → "textbook"
/// Returns None if path doesn't match known source patterns.
fn extract_source_from_path(path: &str) -> Option<String> {
    let path_str = path.to_lowercase();
    if path_str.contains("arxiv") || path_str.contains("article_catalog") {
        Some("arxiv".to_string())
    } else if path_str.contains("textbook") {
        Some("textbook".to_string())
    } else if path_str.contains("stanford") {
        Some("stanford".to_string())
    } else if path_str.contains("openalex") {
        Some("openalex".to_string())
    } else {
        None
    }
}

/// Canonicalize a filesystem-style category segment to arXiv dotted form.
///
/// Filesystem uses dashes: `cs-lg`, `cond-mat-mtrl-sci`, `q-bio-gn`.
/// arXiv uses dots: `cs.LG`, `cond-mat.mtrl-sci`, `q-bio.GN`.
///
/// Strategy: try known multi-component prefixes first, then standard X-YY format.
fn canonicalize_fs_category(seg: &str) -> Option<String> {
    // Known multi-dash prefixes (group.subgroup format in arXiv)
    for prefix in ["cond-mat-", "astro-ph-", "q-bio-", "q-fin-", "nlin-"] {
        if let Some(rest) = seg.strip_prefix(prefix) {
            let group = &prefix[..prefix.len() - 1];
            return Some(format!("{}.{}", group, rest));
        }
    }
    // Standard X-YY format: cs-lg → cs.LG
    if let Some((first, rest)) = seg.split_once('-') {
        return Some(format!("{}.{}", first, rest.to_uppercase()));
    }
    // No dash: standalone like "quant-ph" or "gr-qc" — already canonical
    Some(seg.to_string())
}

/// Extract arXiv-style scientific_domain from catalog path.
///
/// Path format: `.../arxiv/<cat-dash>/<paper-id>/source/<paper-id>.pdf`
/// Example: `.../arxiv/cs-lg/2603.24533/source/2603.24533.pdf` → `cs.LG`
///
/// Returns None if path doesn't match the expected pattern.
/// Validates against the canonical arXiv registry (da_domain::domain module).
fn extract_domain_from_path(pdf_path: &str) -> Option<String> {
    // Find the segment after "arxiv/"
    let path = std::path::Path::new(pdf_path);
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let arxiv_idx = components.iter().position(|c| *c == "arxiv")?;
    let cat_segment = components.get(arxiv_idx + 1)?;

    // Convert filesystem dash format to arXiv dotted form.
    let canonical = canonicalize_fs_category(cat_segment)?;

    // Validate against registry
    if da_domain::domain::is_known(&canonical) {
        Some(canonical)
    } else {
        tracing::warn!(
            path_segment = cat_segment,
            canonical = %canonical,
            "Unknown arXiv category from catalog path"
        );
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain_cs_lg() {
        let path =
            "data/article_catalog/article_catalog/arxiv/cs-lg/2603.24533/source/2603.24533.pdf";
        assert_eq!(extract_domain_from_path(path), Some("cs.LG".to_string()));
    }

    #[test]
    fn test_extract_domain_cs_ai() {
        let path = "/root/daily-archive/data/article_catalog/article_catalog/arxiv/cs-ai/1234.5678/source/1234.5678.pdf";
        assert_eq!(extract_domain_from_path(path), Some("cs.AI".to_string()));
    }

    #[test]
    fn test_extract_domain_physics() {
        let path = "data/article_catalog/article_catalog/arxiv/cond-mat-mtrl-sci/1234.5678/source/1234.5678.pdf";
        // cond-mat.mtrl-sci has multi-part suffix (lowercase in arXiv)
        assert_eq!(
            extract_domain_from_path(path),
            Some("cond-mat.mtrl-sci".to_string())
        );
    }

    #[test]
    fn test_extract_domain_unknown_category() {
        let path =
            "data/article_catalog/article_catalog/arxiv/xx-yy/1234.5678/source/1234.5678.pdf";
        assert_eq!(extract_domain_from_path(path), None);
    }

    #[test]
    fn test_extract_domain_no_arxiv_segment() {
        let path = "/tmp/some/random/path/file.pdf";
        assert_eq!(extract_domain_from_path(path), None);
    }

    #[test]
    fn test_extract_source_arxiv_from_path() {
        let path =
            "data/article_catalog/article_catalog/arxiv/cs-lg/2603.24533/source/2603.24533.pdf";
        assert_eq!(extract_source_from_path(path), Some("arxiv".to_string()));
    }

    #[test]
    fn test_extract_source_textbook_from_path() {
        let path = "data/textbooks/gnn_book/chapter_01/intro.pdf";
        assert_eq!(extract_source_from_path(path), Some("textbook".to_string()));
    }

    #[test]
    fn test_extract_source_unknown_from_path() {
        let path = "/tmp/random/file.pdf";
        // Unknown source — should default but not crash
        assert_eq!(extract_source_from_path(path), None);
    }

    // Integration tests require live services (GROBID, fd_api, Samyama embedded)
}
