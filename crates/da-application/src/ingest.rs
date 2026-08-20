//! Ingest pipeline use case.
//!
//! ADR-037 §4.1: Source → Fetch → Parse → Preprocess → Structure → Embed → Catalog.
//! ADR-040 §1: Samyama Graph as catalog store.
//! ADR-041 §2: HOT path — direct GraphStore API, no Cypher for batch writes.
//! ADR-039: Lifecycle — starts as [proposed], needs canary-10 validation.

use anyhow::Context;
use da_domain::paper::Paper;
use da_domain::schema::NodeSchemaDef;
use da_domain::vid;
use da_domain::{
    ImportEligibility, PdfClassification, PdfTriageDecision, PdfTriagePolicy, PdfTriageStatus,
};
use da_ports::PdfClassificationPort;
use da_ports::embedder::Embedder;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{PageTextMissing, ParserPort, page_text_missingness_from};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Ingest pipeline dependencies (injected ports).
pub struct IngestUseCase {
    pub parser: Option<Box<dyn ParserPort>>,
    pub embedder: Option<Box<dyn Embedder>>,
    pub graph_store: std::sync::Arc<dyn DirectGraphStore>,
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
    /// Optional PDF classifier (M347). When present, classifies PDF
    /// before GROBID parse to detect scanned/garbled pages.
    pub pdf_classifier: Option<std::sync::Arc<dyn PdfClassificationPort>>,
    /// Optional override of bundled DATA pdf-triage-policy (M420).
    /// Tests inject a blocking policy; production uses `PdfTriagePolicy::bundled()`.
    pub pdf_triage_policy: Option<PdfTriagePolicy>,
    /// D127 import gate (M357). Default: Locked. Eligible requires --enable-import.
    pub eligibility: ImportEligibility,
}

/// Result of ingesting one paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub paper_id: String,
    pub title: String,
    pub body_chars: usize,
    pub section_count: usize,
    /// Net-new Section nodes written during this call.
    #[serde(default)]
    pub sections_written: usize,
    pub citation_count: usize,
    pub references_written: usize, // all citations as Reference nodes
    pub cites_resolved: usize,     // citations with a resolvable arxiv_id
    pub vector_dimensions: usize,
    pub graph_node_id: Option<u64>,
    /// Present for versioned content; canonical Paper identity remains unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_vid: Option<String>,
    pub import_eligible: bool, // always false (D127)
    /// Pre-GROBID PDF triage when classifier was attached and succeeded (M350).
    /// `None` if no classifier, classify failed, or PDF unreadable for classify.
    pub pdf_classification: Option<PdfClassification>,
    /// Typed triage outcome (M388 S04). Always set; default NotConfigured.
    #[serde(default)]
    pub pdf_triage: PdfTriageStatus,
    /// Typed page-text missingness stamped from pdf-inspector (M422).
    /// `None` = not classified. `Some(empty)` = classified, no OCR pages.
    #[serde(default)]
    pub page_text_missingness: Option<Vec<PageTextMissing>>,
}

impl IngestUseCase {
    pub fn new(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Box<dyn DirectGraphStore>,
    ) -> Self {
        Self::with_clock(
            parser,
            embedder,
            graph_store,
            std::sync::Arc::new(da_ports::SystemClock),
        )
    }

    pub fn with_clock(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Box<dyn DirectGraphStore>,
        clock: std::sync::Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self {
            parser: Some(parser),
            embedder: Some(embedder),
            graph_store: std::sync::Arc::from(graph_store),
            clock,
            pdf_classifier: None,
            pdf_triage_policy: None,
            eligibility: ImportEligibility::default(),
        }
    }

    pub fn from_shared(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
    ) -> Self {
        Self::from_shared_with_clock(
            parser,
            embedder,
            graph_store,
            std::sync::Arc::new(da_ports::SystemClock),
        )
    }

    pub fn from_shared_with_clock(
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
        clock: std::sync::Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self {
            parser: Some(parser),
            embedder: Some(embedder),
            graph_store,
            clock,
            pdf_classifier: None,
            pdf_triage_policy: None,
            eligibility: ImportEligibility::default(),
        }
    }

    /// Construct the always-run graph materialization boundary without fake
    /// parser/embedder implementations. `ingest_pdf` fails closed in this mode.
    pub fn materializer_only(graph_store: std::sync::Arc<dyn DirectGraphStore>) -> Self {
        Self::materializer_only_with_clock(graph_store, std::sync::Arc::new(da_ports::SystemClock))
    }

    /// Construct the graph-only boundary with an injected clock. Replay and
    /// temporal tests must use this instead of relying on wall-clock timing.
    pub fn materializer_only_with_clock(
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
        clock: std::sync::Arc<dyn da_ports::Clock>,
    ) -> Self {
        Self {
            parser: None,
            embedder: None,
            graph_store,
            clock,
            pdf_classifier: None,
            pdf_triage_policy: None,
            eligibility: ImportEligibility::default(),
        }
    }

    /// Set optional PDF classifier (M347). Enables pre-GROBID triage.
    pub fn with_pdf_classifier(
        mut self,
        classifier: std::sync::Arc<dyn PdfClassificationPort>,
    ) -> Self {
        self.pdf_classifier = Some(classifier);
        self
    }

    /// Override bundled DATA pdf-triage-policy (M420). Production leaves this
    /// unset and loads `PdfTriagePolicy::bundled()`.
    pub fn with_pdf_triage_policy(mut self, policy: PdfTriagePolicy) -> Self {
        self.pdf_triage_policy = Some(policy);
        self
    }

    /// Set import eligibility (M357 / D127 unlock). Default: Locked.
    pub fn with_import_eligibility(mut self, eligibility: ImportEligibility) -> Self {
        self.eligibility = eligibility;
        self
    }

    /// Ingest a single PDF: parse → embed → write to graph via HOT path.
    ///
    /// ADR-041: Uses direct GraphStore API (0.01ms per op) instead of Cypher MERGE.
    /// No parse/plan overhead. 100x faster than Cypher for batch writes.
    pub async fn ingest_pdf(&self, pdf_path: &str, paper_id: &str) -> anyhow::Result<IngestResult> {
        self.ingest_pdf_with_context(pdf_path, paper_id, None).await
    }

    /// Revision-aware variant of the HOT-path single PDF ingest. The context
    /// must come from checksum-bound source evidence; callers never invent v1.
    #[tracing::instrument(
        skip(self, pdf_path, revision_context),
        fields(paper_id = %paper_id),
        name = "ingest_pdf"
    )]
    pub async fn ingest_pdf_with_context(
        &self,
        pdf_path: &str,
        paper_id: &str,
        revision_context: Option<&crate::revision_materialization::RevisionContentContext>,
    ) -> anyhow::Result<IngestResult> {
        tracing::info!(paper_id, pdf_path, "Starting ingest (HOT path)");

        // D127 / M357: import eligibility computed from CLI flag (default Locked).
        let eligible = self.eligibility.is_eligible();
        if eligible {
            tracing::warn!(
                paper_id,
                "D127 UNLOCKED: nodes marked eligible for import (--enable-import)"
            );
        }

        // ADR-043: extract scientific_domain from catalog path if available
        let primary_domain = extract_domain_from_path(pdf_path);
        // ADR-044: extract source code from catalog path for Source node
        let source_code = extract_source_from_path(pdf_path);
        tracing::debug!(
            paper_id,
            source_code = source_code.as_deref(),
            "Source detected"
        );

        // M347/M350/M388/M420: Pre-GROBID PDF classification; DATA triage admission.
        let triage_policy = self
            .pdf_triage_policy
            .clone()
            .or_else(|| PdfTriagePolicy::bundled().ok())
            .unwrap_or(PdfTriagePolicy {
                schema_version: "pdf-triage-policy.v1".into(),
                import_eligible: false,
                fail_open: true,
                record_in_ingest_result: true,
                feature_gate: "pdf-classifier".into(),
                encoding_issues_block_grobid: false,
                max_garbled_page_percent: 100,
            });
        let mut pdf_classification: Option<PdfClassification> = None;
        let mut pdf_triage = PdfTriageStatus::NotConfigured;
        if let Some(classifier) = &self.pdf_classifier {
            match std::fs::read(pdf_path) {
                Ok(pdf_bytes) => match classifier.classify(&pdf_bytes).await {
                    Ok(classification) => {
                        tracing::info!(
                            paper_id,
                            pdf_type = ?classification.pdf_type,
                            page_count = classification.page_count,
                            pages_needing_ocr = classification.pages_needing_ocr.len(),
                            has_encoding_issues = classification.has_encoding_issues,
                            confidence = classification.confidence,
                            pdf_triage = "classified",
                            "PDF classified (pre-GROBID)"
                        );
                        if !classification.is_text_extractable() {
                            tracing::warn!(
                                paper_id,
                                pdf_type = ?classification.pdf_type,
                                pages_needing_ocr = ?classification.pages_needing_ocr,
                                "PDF not fully text-extractable; GROBID may produce partial results"
                            );
                        }
                        match triage_policy.decide(&classification) {
                            PdfTriageDecision::Block { ratio } => {
                                tracing::warn!(
                                    paper_id,
                                    ratio,
                                    threshold_percent = triage_policy.max_garbled_page_percent,
                                    fail_open = triage_policy.fail_open,
                                    triage_decision = "block",
                                    "PDF garbled-page ratio exceeds DATA threshold"
                                );
                                if !triage_policy.fail_open {
                                    return Err(anyhow::anyhow!(
                                        "PDF triage blocked GROBID: garbled ratio {ratio:.3} exceeds max_garbled_page_percent={} (fail_open=false)",
                                        triage_policy.max_garbled_page_percent
                                    ));
                                }
                                tracing::warn!(
                                    paper_id,
                                    ratio,
                                    triage_decision = "block_continue",
                                    "PDF triage Block with fail_open=true; continuing to GROBID"
                                );
                            }
                            PdfTriageDecision::Continue => {
                                tracing::info!(
                                    paper_id,
                                    triage_decision = "continue",
                                    "PDF triage Continue; proceeding to GROBID"
                                );
                            }
                        }
                        pdf_classification = Some(classification);
                        pdf_triage = PdfTriageStatus::Classified;
                    }
                    Err(e) => {
                        if triage_policy.fail_open {
                            tracing::warn!(paper_id, error = %e, pdf_triage = "failed_open", "PDF classification failed; continuing to GROBID");
                            pdf_triage = PdfTriageStatus::FailedOpen;
                        } else {
                            return Err(anyhow::anyhow!(
                                "PDF classification failed and fail_open=false: {e}"
                            ));
                        }
                    }
                },
                Err(e) => {
                    if triage_policy.fail_open {
                        tracing::warn!(paper_id, error = %e, pdf_triage = "failed_open", "Cannot read PDF for classification; continuing to GROBID");
                        pdf_triage = PdfTriageStatus::FailedOpen;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Cannot read PDF for classification and fail_open=false: {e}"
                        ));
                    }
                }
            }
        }

        // 1. Parse via GROBID
        let parser = self
            .parser
            .as_ref()
            .context("ingest_pdf unavailable on materializer-only IngestUseCase")?;
        let mut parsed = parser.parse_pdf(pdf_path, paper_id).await?;
        if let Some(classification) = &pdf_classification {
            parsed.page_text_missingness = Some(page_text_missingness_from(classification));
            tracing::info!(
                paper_id,
                page_text_missingness = parsed
                    .page_text_missingness
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0),
                "Stamped page-text missingness from classification"
            );
        }
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
        let embedder = self
            .embedder
            .as_ref()
            .context("ingest_pdf unavailable on materializer-only IngestUseCase")?;
        let vector = embedder.embed(embed_text).await?;
        tracing::info!(paper_id, dimensions = vector.len(), "Embedded");
        self.materialize_parsed_with_context(
            &parsed,
            vector,
            primary_domain,
            source_code,
            pdf_classification,
            pdf_triage,
            revision_context,
        )
        .await
    }

    /// Materialize already-computed parse and embedding artifacts without
    /// invoking parser, classifier, or embedder again. This is an always-run
    /// side-effect boundary; callers must provide artifacts validated through
    /// their content-addressed pipeline cache.
    pub async fn materialize_parsed(
        &self,
        parsed: &da_ports::parser::ParsedArticle,
        vector: Vec<f32>,
        primary_domain: Option<String>,
        source_code: Option<String>,
        pdf_classification: Option<PdfClassification>,
    ) -> anyhow::Result<IngestResult> {
        let pdf_triage = if pdf_classification.is_some() {
            PdfTriageStatus::Classified
        } else {
            PdfTriageStatus::NotConfigured
        };
        self.materialize_parsed_with_triage(
            parsed,
            vector,
            primary_domain,
            source_code,
            pdf_classification,
            pdf_triage,
        )
        .await
    }

    pub async fn materialize_parsed_with_triage(
        &self,
        parsed: &da_ports::parser::ParsedArticle,
        vector: Vec<f32>,
        primary_domain: Option<String>,
        source_code: Option<String>,
        pdf_classification: Option<PdfClassification>,
        pdf_triage: PdfTriageStatus,
    ) -> anyhow::Result<IngestResult> {
        self.materialize_parsed_with_context(
            parsed,
            vector,
            primary_domain,
            source_code,
            pdf_classification,
            pdf_triage,
            None,
        )
        .await
    }

    /// Revision-aware materialization boundary. When context is present,
    /// mutable content properties, embeddings, and Sections belong to the
    /// immutable PaperRevision rather than the canonical Paper work.
    #[allow(clippy::too_many_arguments)] // boundary keeps explicit typed args (pdf_triage M388)
    pub async fn materialize_parsed_with_context(
        &self,
        parsed: &da_ports::parser::ParsedArticle,
        vector: Vec<f32>,
        primary_domain: Option<String>,
        source_code: Option<String>,
        pdf_classification: Option<PdfClassification>,
        pdf_triage: PdfTriageStatus,
        revision_context: Option<&crate::revision_materialization::RevisionContentContext>,
    ) -> anyhow::Result<IngestResult> {
        let paper_id = parsed.paper_id.as_str();
        let eligible = self.eligibility.is_eligible();

        // 3. Create domain Paper (compute VID)
        let paper = Paper::new(paper_id, &parsed.title);
        let vid_str = vid::paper_vid(paper_id);
        // PIR-008 / M398-S04: node DateTime wire is a Clock-derived RFC3339
        // UTC string — integer timestamps are rejected by schema validation.
        let now_rfc3339 = self.clock.now().to_rfc3339();

        // 3b. Validate against schema before writing (GRAPH-SCHEMA.md)
        let paper_schema = da_domain::paper::PaperSchema;
        let mut paper_props = std::collections::HashMap::new();
        paper_props.insert("vid".to_string(), serde_json::json!(vid_str));
        paper_props.insert("arxiv_id".to_string(), serde_json::json!(paper.arxiv_id));
        paper_props.insert("title".to_string(), serde_json::json!(paper.title));
        paper_props.insert(
            "valid_from".to_string(),
            serde_json::Value::String(now_rfc3339.clone()),
        );
        paper_schema.validate(&paper_props)?;

        // 4. HOT PATH: Direct GraphStore API — replay-safe Paper identity.
        let (node_id, paper_created) = match self
            .graph_store
            .find_node_by_string_property("Paper", "vid", &vid_str)
            .await
        {
            Some(node) => (node, false),
            None => {
                let node = self.graph_store.create_node("Paper").await?;
                tracing::debug!(paper_id, node_id = node, "Node created");
                (node, true)
            }
        };

        // Set properties directly (no Cypher!)
        self.graph_store
            .set_node_property_string(node_id, "vid", vid_str.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node_id, "arxiv_id", paper.arxiv_id.clone())
            .await?;
        // M419 S01: materialize pdf-inspector triage onto the Paper node so
        // read planes (query/MCP) see document health without the ingest
        // report. String/bool only; identical values on replay.
        if let Some(classification) = &pdf_classification {
            let pages_needing_ocr = classification
                .pages_needing_ocr
                .iter()
                .map(|page| page.to_string())
                .collect::<Vec<_>>()
                .join(",");
            self.graph_store
                .set_node_property_string(
                    node_id,
                    "pdf_type",
                    format!("{:?}", classification.pdf_type),
                )
                .await?;
            self.graph_store
                .set_node_property_string(node_id, "pdf_pages_needing_ocr", pages_needing_ocr)
                .await?;
            self.graph_store
                .set_node_property_bool(
                    node_id,
                    "pdf_has_encoding_issues",
                    classification.has_encoding_issues,
                )
                .await?;
        }
        if revision_context.is_none() {
            self.graph_store
                .set_node_property_string(node_id, "title", paper.title.clone())
                .await?;
        }

        // The int read stays as the bounded legacy decoder: pre-PIR-008
        // snapshots carry integer valid_from and must never be overwritten on
        // replay; new writes are canonical RFC3339 UTC strings.
        if paper_created
            || (self
                .graph_store
                .get_node_property_string(node_id, "valid_from")
                .await
                .is_none()
                && self
                    .graph_store
                    .get_node_property_int(node_id, "valid_from")
                    .await
                    .is_none())
        {
            self.graph_store
                .set_node_property_string(node_id, "valid_from", now_rfc3339.clone())
                .await?;
        }
        self.graph_store
            .set_node_property_int(node_id, "schema_version", 1)
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "evidence_ready", false)
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "import_eligible", eligible) // D127: always false
            .await?;
        self.graph_store
            .set_node_property_bool(node_id, "retrieval_eligible", true) // D134: live for retrieval
            .await?;

        let (content_node_id, content_label, revision_vid_value, revision_sections_created) =
            if let Some(context) = revision_context {
                anyhow::ensure!(
                    context.canonical_id == paper_id,
                    "revision context canonical id '{}' does not match parsed paper '{}'",
                    context.canonical_id,
                    paper_id
                );
                let request = crate::revision_materialization::RevisionMaterializationRequest {
                    source_code: context.source_code.clone(),
                    canonical_id: context.canonical_id.clone(),
                    revision: context.revision,
                    parsed: parsed.clone(),
                    manifestations: context.manifestations.clone(),
                };
                let result = crate::revision_materialization::RevisionMaterializationUseCase::new(
                    self.graph_store.clone(),
                    self.clock.clone(),
                )
                .materialize(&request)
                .await?;
                let revision_node = self
                    .graph_store
                    .find_node_by_string_property("PaperRevision", "vid", &result.revision_vid)
                    .await
                    .context("revision materializer did not expose created PaperRevision")?;
                (
                    revision_node,
                    "PaperRevision",
                    Some(result.revision_vid),
                    result.sections_created,
                )
            } else {
                (node_id, "Paper", None, 0)
            };

        self.graph_store
            .set_node_property_string(content_node_id, "pdf_hash", parsed.pdf_hash.clone())
            .await?;
        self.graph_store
            .set_node_property_string(
                content_node_id,
                "abstract_text",
                parsed.abstract_text.clone(),
            )
            .await?;
        self.graph_store
            .set_node_property_string(
                content_node_id,
                "abstract_sha256",
                hex::encode(Sha256::digest(parsed.abstract_text.as_bytes())),
            )
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
                        .set_node_property_bool(sn, "import_eligible", eligible) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(sn, "schema_version", 1)
                        .await?;
                    sn
                }
            };
            let edge_type = da_domain::relation::structure::FROM_SOURCE;
            if self
                .graph_store
                .get_edges_between(node_id, source_node, edge_type)
                .await
                .is_empty()
            {
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    node_id,
                    source_node,
                    edge_type,
                    &temporal,
                )
                .await?;
            }
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

            // Create Category node + link Paper → Category (IN_CATEGORY edge).
            let category_vid = format!("vid:cat:{domain}");
            let cat_node = match self
                .graph_store
                .find_node_by_string_property("Category", "vid", &category_vid)
                .await
            {
                Some(node) => node,
                None => {
                    let node = self.graph_store.create_node("Category").await?;
                    self.graph_store
                        .set_node_property_string(node, "vid", category_vid)
                        .await?;
                    self.graph_store
                        .set_node_property_string(node, "code", domain.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "is_primary", true)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "retrieval_eligible", true)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "import_eligible", eligible) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "schema_version", 1)
                        .await?;
                    node
                }
            };
            let edge_type = da_domain::relation::structure::IN_CATEGORY;
            if self
                .graph_store
                .get_edges_between(node_id, cat_node, edge_type)
                .await
                .is_empty()
            {
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    node_id,
                    cat_node,
                    edge_type,
                    &temporal,
                )
                .await?;
            }
        }

        // ADR-053 Layer B: Summary node creation is DEFERRED until real
        // summarization (extractive or LLM) exists — never abstract_copy.
        // Schema (SummarySchema + hasSummary edge) is ready for future wiring.
        // Tracked in doc/STUB-AUDIT.md (honest-deferral).

        // Section + citation metadata (enables Phase 3 extraction queries)
        let section_count = parsed.sections.len();
        let citation_count = parsed.citations.len();
        self.graph_store
            .set_node_property_int(content_node_id, "section_count", section_count as i64)
            .await?;
        self.graph_store
            .set_node_property_int(content_node_id, "citation_count", citation_count as i64)
            .await?;

        // 4b. Create Section nodes (Layer 2 Structure — ONTOLOGY-DESIGN)
        // Each section gets a node with title, level, order, text.
        // Linked to Paper via hasPart edge (FaBiO frbr:part).
        let mut section_nodes = revision_sections_created;
        // Map: section_id (stable hash) -> graph node id. Used by ADR-053 Layer A
        // T04 to wire next_sibling / first_child / parent_section edges after
        // all Section nodes are created.
        let mut section_id_map: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        for (i, section) in parsed.sections.iter().enumerate() {
            let section_vid = match revision_context {
                Some(context) => context.section_vid(i)?,
                None => format!("vid:section:{}:{}", paper_id, i),
            };
            let section_node = match self
                .graph_store
                .find_node_by_string_property("Section", "vid", &section_vid)
                .await
            {
                Some(node) => node,
                None => {
                    let node = self.graph_store.create_node("Section").await?;
                    section_nodes += 1;
                    node
                }
            };
            self.graph_store
                .set_node_property_string(section_node, "vid", section_vid)
                .await?;
            self.graph_store
                .set_node_property_string(section_node, "title", section.title.clone())
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "level", section.level as i64)
                .await?;
            // ADR-053 Layer A: store path_id and section_id on the node so
            // Cypher queries can filter by path_id (e.g. root sections).
            if let Some(path_id) = &section.path_id {
                self.graph_store
                    .set_node_property_string(section_node, "path_id", path_id.clone())
                    .await?;
            }
            if let Some(section_id) = &section.section_id {
                self.graph_store
                    .set_node_property_string(section_node, "section_id", section_id.clone())
                    .await?;
            }
            if let Some(parent_sid) = &section.parent_section_id {
                self.graph_store
                    .set_node_property_string(section_node, "parent_section_id", parent_sid.clone())
                    .await?;
            }
            if let Some(order) = section.order_in_parent {
                self.graph_store
                    .set_node_property_int(section_node, "order_in_parent", order as i64)
                    .await?;
            }
            self.graph_store
                .set_node_property_int(section_node, "order", i as i64)
                .await?;
            self.graph_store
                .set_node_property_string(
                    section_node,
                    "work_vid",
                    revision_vid_value
                        .as_deref()
                        .unwrap_or(vid_str.as_str())
                        .to_string(),
                )
                .await?;
            self.graph_store
                .set_node_property_bool(section_node, "retrieval_eligible", true)
                .await?;
            self.graph_store
                .set_node_property_bool(section_node, "import_eligible", eligible) // D127
                .await?;
            self.graph_store
                .set_node_property_int(section_node, "schema_version", 1)
                .await?;
            // Canonical graph truth preserves the complete parser output.
            // Prompt/context consumers own explicit bounded excerpts with source hashes.
            self.graph_store
                .set_node_property_string(section_node, "text", section.text.clone())
                .await?;
            self.graph_store
                .set_node_property_int(
                    section_node,
                    "char_count",
                    section.text.chars().count() as i64,
                )
                .await?;
            self.graph_store
                .set_node_property_string(
                    section_node,
                    "text_sha256",
                    hex::encode(Sha256::digest(section.text.as_bytes())),
                )
                .await?;
            let edge_type = da_domain::relation::structure::HAS_PART;
            if self
                .graph_store
                .get_edges_between(content_node_id, section_node, edge_type)
                .await
                .is_empty()
            {
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    content_node_id,
                    section_node,
                    edge_type,
                    &temporal,
                )
                .await?;
            }
            // Record section_id -> graph_node_id mapping for ADR-053 Layer A
            // navigation edge wiring (T04).
            if let Some(sid) = &section.section_id {
                section_id_map.insert(sid.clone(), section_node);
            }
        }
        tracing::info!(paper_id, section_nodes, "Section nodes created");

        // 4c. ADR-053 Layer A T04: document tree navigation edges.
        // Build next_sibling / first_child / parent_section edges from the
        // path_id tree computed by GrobidParser::extract_sections (T02).
        // Only emitted when the parser produced nested structure (path_id is
        // Some). Flat-section parsers skip this step (backward compat).
        let nav_edges = build_section_navigation_edges(
            &parsed.sections,
            &section_id_map,
            self.graph_store.as_ref(),
        )
        .await?;
        if nav_edges > 0 {
            tracing::info!(paper_id, nav_edges, "Section navigation edges created");
        }

        // 5. HOT PATH: add-or-replace the vector for this content.
        // M416 delivered the sanctioned same-node upsert (replace_vector,
        // D277): an equal-sha replay is a no-op and a changed embedding
        // (embedder-model change) replaces in place instead of failing.
        match crate::embedding_write::write_node_embedding(
            self.graph_store.as_ref(),
            content_label,
            content_node_id,
            vector.clone(),
        )
        .await
        {
            crate::embedding_write::NodeEmbedWrite::Added => {}
            crate::embedding_write::NodeEmbedWrite::Replaced => {
                tracing::info!(paper_id, content_label, event = "node_embedding_replaced");
            }
            crate::embedding_write::NodeEmbedWrite::Unchanged => {}
        }

        // 6. Create CITES edges for citations with resolvable arxiv_ids
        // (enables citation graph traversal — ADR-038 S_kn tri-source)
        // Idempotent: reuses existing Citation node if one with same arxiv_id exists.
        let mut cites_resolved = 0usize;
        let mut references_written = 0usize;
        // PIR-011 / D246: additive revision-owned bibliography edges. These
        // counters track PaperRevision -> Reference/Citation edges written
        // beside the global Paper-level edges (never cloned nodes, MEM471).
        let mut revision_reference_edges_written = 0usize;
        let mut revision_cites_edges_written = 0usize;
        // PIR-011 / D247+D249: PaperRevision edges that carry in-text anchors
        // (citation_anchors_json + citation_anchor_missingness).
        let mut revision_anchor_edges_written = 0usize;
        let citations_with_anchors = parsed
            .citations
            .iter()
            .filter(|c| !c.anchors.is_empty())
            .count();
        let now_rfc3339 = self.clock.now().to_rfc3339();
        for citation in &parsed.citations {
            // T03: does this citation produce a CITES edge (resolvable arXiv
            // identity)? Anchors live on the CITES edge when it exists and on
            // the HAS_PART edge only for Reference-only citations (D249).
            let resolves_to_citation = citation
                .arxiv_id
                .as_deref()
                .map(|id| vid::citation_arxiv_vid(id).is_ok())
                .unwrap_or(false);
            // Create Reference node for ALL citations (full bibliography).
            // Reference stores raw_text + metadata, even for unresolved citations.
            // Citation node is created separately for resolvable arxiv_ids.
            // Dedup: if a Reference with the same vid exists, reuse it.
            {
                let ref_vid = da_domain::vid::reference_vid(&citation.raw_text);
                let (ref_node, ref_created) = match self
                    .graph_store
                    .find_node_by_string_property("Reference", "vid", &ref_vid)
                    .await
                {
                    Some(id) => (id, false), // reuse existing Reference (idempotent)
                    None => {
                        let new_id = self.graph_store.create_node("Reference").await?;
                        self.graph_store
                            .set_node_property_string(new_id, "vid", ref_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(new_id, "raw_text", citation.raw_text.clone())
                            .await?;
                        if let Some(ref arxiv_id) = citation.arxiv_id {
                            self.graph_store
                                .set_node_property_string(new_id, "arxiv_id", arxiv_id.clone())
                                .await?;
                        }
                        if let Some(ref title) = citation.title {
                            self.graph_store
                                .set_node_property_string(new_id, "title", title.clone())
                                .await?;
                        }
                        (new_id, true)
                    }
                };
                if let Some(ref doi) = citation.doi {
                    self.graph_store
                        .set_node_property_string(ref_node, "doi", doi.clone())
                        .await?;
                }
                if ref_created
                    || (self
                        .graph_store
                        .get_node_property_string(ref_node, "valid_from")
                        .await
                        .is_none()
                        && self
                            .graph_store
                            .get_node_property_int(ref_node, "valid_from")
                            .await
                            .is_none())
                {
                    self.graph_store
                        .set_node_property_string(ref_node, "valid_from", now_rfc3339.clone())
                        .await?;
                }
                self.graph_store
                    .set_node_property_bool(ref_node, "retrieval_eligible", true)
                    .await?;
                self.graph_store
                    .set_node_property_bool(ref_node, "import_eligible", eligible) // D127
                    .await?;
                self.graph_store
                    .set_node_property_int(ref_node, "schema_version", 1)
                    .await?;
                let edge_type = da_domain::relation::structure::HAS_PART;
                if self
                    .graph_store
                    .get_edges_between(node_id, ref_node, edge_type)
                    .await
                    .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        node_id,
                        ref_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                }
                // PIR-011 / D246: when content is a PaperRevision, the exact
                // revision also owns this Reference via its own additive
                // HAS_PART edge. The globally deduped Reference node is NOT
                // cloned (MEM471); ownership is expressed through edges.
                if revision_context.is_some()
                    && self
                        .graph_store
                        .get_edges_between(content_node_id, ref_node, edge_type)
                        .await
                        .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    let edge_id = crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        content_node_id,
                        ref_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                    // T03 (D247/D249): Reference-only citations (no resolvable
                    // arXiv identity, so no CITES edge) carry in-text anchors on
                    // the revision-owned HAS_PART edge. Resolvable citations use
                    // the CITES edge instead — never both, never Paper-sourced
                    // edges, never the global Reference/Citation node.
                    if !resolves_to_citation {
                        write_citation_anchors_on_edge(
                            self.graph_store.as_ref(),
                            edge_id,
                            citation,
                        )
                        .await?;
                        revision_anchor_edges_written += 1;
                    }
                    revision_reference_edges_written += 1;
                    tracing::debug!(
                        paper_id,
                        revision_source = content_node_id,
                        reference_node = ref_node,
                        "Revision-owned HAS_PART Reference edge written"
                    );
                }
                if ref_created {
                    references_written += 1;
                }
            }

            if let Some(ref arxiv_id) = citation.arxiv_id {
                let Ok(cited_vid) = vid::citation_arxiv_vid(arxiv_id) else {
                    tracing::warn!(
                        event = "citation_arxiv_id_rejected",
                        paper_id,
                        arxiv_id,
                        "Citation remains Reference-only because arXiv identity is invalid"
                    );
                    continue;
                };
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
                            .set_node_property_string(new_node, "valid_from", now_rfc3339.clone())
                            .await?;
                        self.graph_store
                            .set_node_property_int(new_node, "schema_version", 1)
                            .await?;
                        self.graph_store
                            .set_node_property_bool(new_node, "retrieval_eligible", true)
                            .await?;
                        self.graph_store
                            .set_node_property_bool(new_node, "import_eligible", eligible) // D127
                            .await?;
                        new_node
                    }
                };
                let edge_type = da_domain::relation::bibliographic::CITES;
                if self
                    .graph_store
                    .get_edges_between(node_id, cited_node, edge_type)
                    .await
                    .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        node_id,
                        cited_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                    cites_resolved += 1;
                }
                // PIR-011 / D246: the exact PaperRevision claims this citation
                // via its own additive CITES edge; the global Paper-level edge
                // remains for work-level traversal. Citation nodes stay
                // globally deduped by arxiv_id (MEM471) — no per-revision clone.
                if revision_context.is_some()
                    && self
                        .graph_store
                        .get_edges_between(content_node_id, cited_node, edge_type)
                        .await
                        .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    let edge_id = crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        content_node_id,
                        cited_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                    // T03 (D247/D249): resolvable citations carry their in-text
                    // anchors on the PaperRevision CITES edge (never on the
                    // globally deduped Citation node or the Paper-sourced edge).
                    write_citation_anchors_on_edge(self.graph_store.as_ref(), edge_id, citation)
                        .await?;
                    revision_anchor_edges_written += 1;
                    revision_cites_edges_written += 1;
                    tracing::debug!(
                        paper_id,
                        revision_source = content_node_id,
                        citation_node = cited_node,
                        "Revision-owned CITES edge written"
                    );
                }
            }
        }

        tracing::info!(
            event = "parsed_article_materialized",
            paper_id,
            node_id,
            vid = %vid_str,
            vector_dim = vector.len(),
            sections_written = section_nodes,
            references_written,
            citation_count,
            cites_edges_written = cites_resolved,
            revision_reference_edges_written,
            revision_cites_edges_written,
            citations_with_anchors,
            revision_anchor_edges_written,
            import_eligible = eligible,
            "Parsed article materialized through direct graph API"
        );

        Ok(IngestResult {
            paper_id: paper_id.to_string(),
            title: paper.title.clone(),
            body_chars: parsed.body_text.len(),
            section_count,
            sections_written: section_nodes,
            citation_count,
            references_written,
            cites_resolved,
            vector_dimensions: vector.len(),
            graph_node_id: Some(node_id),
            revision_vid: revision_vid_value,
            import_eligible: eligible,
            pdf_classification,
            pdf_triage,
            page_text_missingness: parsed.page_text_missingness.clone(),
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

pub fn embedding_fingerprint(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((vector.len() as u64).to_le_bytes());
    for value in vector {
        hasher.update(value.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Extract source code from catalog path.
///
/// Path format: `.../arxiv/<cat-dash>/<id>/...` → "arxiv"
///              `.../textbooks/<book>/...` → "textbook"
/// Returns None if path doesn't match known source patterns.
fn extract_source_from_path(path: &str) -> Option<String> {
    // Prefer concrete path segments over blanket `article_catalog` (which holds
    // every source_code). Order: known non-arxiv codes first, then arxiv.
    let path_str = path.to_lowercase();
    let segments: Vec<&str> = std::path::Path::new(&path_str)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    // DATA-driven known codes that appear as catalog roots.
    for code in [
        "gnn_textbook",
        "textbook",
        "stanford",
        "openalex",
        "crossref",
        "semantic_scholar",
        "company_blog",
        "nature",
        "local",
        "arxiv",
    ] {
        if segments.contains(&code) || path_str.contains(code) {
            return Some(code.to_string());
        }
    }
    None
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

/// Build ADR-053 Layer A navigation edges (next_sibling, first_child,
/// parent_section) between Section nodes from the path_id tree computed by
/// GrobidParser::extract_sections (T02).
///
/// Returns the count of navigation edges created. Skips silently when no
/// section has a populated `section_id` (legacy flat-section parsers).
///
/// All three edges are `temporal_class=structural` (created_at only per
/// ADR-046). No bi-temporal stamping required for structural edges.
pub async fn build_section_navigation_edges(
    sections: &[da_ports::parser::Section],
    section_id_map: &std::collections::HashMap<String, u64>,
    graph_store: &dyn da_ports::graph_store::DirectGraphStore,
) -> anyhow::Result<usize> {
    let mut edges_created = 0usize;

    // Short-circuit: if no section has a section_id, this is a legacy
    // flat-section parser output. Skip navigation edge creation entirely.
    if section_id_map.is_empty() {
        return Ok(0);
    }

    // Pure ordered-tree model (M398 S01 T02): compute the expected navigation
    // edges without graph IO, then materialize them idempotently against the
    // store. Property tests exercise `compute_section_navigation_edges`
    // directly; this wrapper is the only graph-touching part.
    for edge in compute_section_navigation_edges(sections, section_id_map) {
        // Idempotent replay: only create edges that do not already exist.
        if graph_store
            .get_edges_between(edge.from, edge.to, edge.relation.as_edge_type())
            .await
            .is_empty()
        {
            graph_store
                .create_edge(edge.from, edge.to, edge.relation.as_edge_type())
                .await?;
            edges_created += 1;
        }
    }

    Ok(edges_created)
}

/// Write in-text citation anchors onto a revision-owned edge (T03, D247/D249).
///
/// Stores `citation_anchors_json` (JSON array of `{ref_id, section_path_id,
/// quote, offset}`) and `citation_anchor_missingness` (typed missingness:
/// `no_in_text_anchor` when the citation has no in-text refs,
/// `anchor_section_path_unresolved` when anchors exist but some lack a section
/// path, empty string when anchors are fully resolved). Callers invoke this
/// only for PaperRevision-sourced HAS_PART/CITES edges — never for Paper-sourced
/// edges and never for the globally deduped Citation/Reference nodes, which
/// would mix v1/v5 and cross-paper locations (MEM796/MEM798).
async fn write_citation_anchors_on_edge(
    store: &dyn da_ports::graph_store::DirectGraphStore,
    edge_id: u64,
    citation: &da_ports::parser::CitationEntry,
) -> anyhow::Result<()> {
    let anchors_json =
        serde_json::to_string(&citation.anchors).context("serialize citation anchors to JSON")?;
    store
        .set_edge_property_string(edge_id, "citation_anchors_json", &anchors_json)
        .await?;
    let missingness = citation_anchor_missingness(&citation.anchors);
    store
        .set_edge_property_string(edge_id, "citation_anchor_missingness", &missingness)
        .await?;
    Ok(())
}

/// Typed anchor missingness per D249: empty anchors → `no_in_text_anchor`;
/// any anchor without a resolved section path → `anchor_section_path_unresolved`;
/// otherwise empty string (anchors present and fully resolved).
fn citation_anchor_missingness(anchors: &[da_ports::parser::CitationAnchor]) -> String {
    if anchors.is_empty() {
        "no_in_text_anchor".to_string()
    } else if anchors.iter().any(|a| a.section_path_id.is_none()) {
        "anchor_section_path_unresolved".to_string()
    } else {
        String::new()
    }
}

/// Return the parent path of a dotted path_id (everything before the last '.').
/// Returns `None` for root sections (no parent).
///
/// GROBID restart suffixes (`~N`, e.g. "1~2", "1~2.1") are dot-free, so they
/// never introduce a phantom hierarchy level: "1~2" stays a root and the
/// parent of "1~2.1" is "1~2", not "1".
fn parent_path_of(path_id: &str) -> Option<&str> {
    match path_id.rfind('.') {
        Some(pos) => Some(&path_id[..pos]),
        None => None,
    }
}

/// Navigation relation kinds emitted by the ordered-section tree model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavRelation {
    /// Direct edge from a section to its same-parent successor.
    NextSibling,
    /// Edge from a section up to its parent (inverse of FIRST_CHILD).
    ParentSection,
    /// Edge from a parent to its first child in document order.
    FirstChild,
}

impl NavRelation {
    /// The graph edge type string for this relation (structure ontology).
    pub fn as_edge_type(self) -> &'static str {
        match self {
            NavRelation::NextSibling => da_domain::relation::structure::NEXT_SIBLING,
            NavRelation::ParentSection => da_domain::relation::structure::PARENT_SECTION,
            NavRelation::FirstChild => da_domain::relation::structure::FIRST_CHILD,
        }
    }
}

/// A navigation edge in the ordered-section tree, computed without graph IO.
/// `from`/`to` are graph node ids taken from `section_id_map`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavEdge {
    pub from: u64,
    pub to: u64,
    pub relation: NavRelation,
}

/// Pure ordered-tree navigation model (M398 S01 T02).
///
/// Given parsed sections with structural `path_id`s (e.g. "1.2.1") and the
/// `section_id` → graph-node map, computes the navigation edges a correct
/// ordered-section tree must have, with **no graph IO**:
///
/// - `next_sibling`: every non-last child of a parent points **directly** to
///   its actual same-parent successor, even when intervening descendants sit
///   between them (deep-tree fix, M398 S01 T01). Root sections chain to the
///   next root section.
/// - `first_child`: every parent points to its first child in document order.
/// - `parent_section`: every non-root section points to its parent.
///
/// Sibling selection uses the canonical parent identity (the parent path
/// prefix derived from the last '.'). GROBID restart suffixes (`~N`) are
/// dot-free, so a restart path is never misread as a dotted parent:
/// "1~2.1" belongs to parent "1~2", not "1".
///
/// The caller (`build_section_navigation_edges`) applies idempotent
/// dedup-and-create against the store; property tests exercise this function
/// directly without a graph.
pub fn compute_section_navigation_edges(
    sections: &[da_ports::parser::Section],
    section_id_map: &std::collections::HashMap<String, u64>,
) -> Vec<NavEdge> {
    let mut edges = Vec::new();

    // Map each structural path to its graph node (canonical parent lookup).
    let path_to_node: std::collections::HashMap<&str, u64> = sections
        .iter()
        .filter_map(|s| {
            s.path_id
                .as_deref()
                .and_then(|p| s.section_id.as_deref().map(|sid| (p, sid)))
                .and_then(|(p, sid)| section_id_map.get(sid).map(|&n| (p, n)))
        })
        .collect();

    for (i, section) in sections.iter().enumerate() {
        let (Some(path_id), Some(section_id)) =
            (section.path_id.as_deref(), section.section_id.as_deref())
        else {
            continue;
        };
        let Some(&this_node) = section_id_map.get(section_id) else {
            continue;
        };

        // parent_section edge: this → parent.
        if let Some(parent_path) = parent_path_of(path_id)
            && let Some(&parent_node) = path_to_node.get(parent_path)
        {
            edges.push(NavEdge {
                from: this_node,
                to: parent_node,
                relation: NavRelation::ParentSection,
            });
        }

        // next_sibling edge: this → next section with the same parent.
        //
        // Sibling selection uses the canonical parent identity (the parent
        // path prefix) and scans PAST intervening descendants instead of
        // stopping at the first later path (deep-tree fix, M398 S01 T01):
        // in `1, 1.1, 1.1.1, 1.2, 2`, section 1.1's next sibling is 1.2 and
        // section 1's next sibling is 2, even though 1.1 / 1.1.1 / 1.2 sit
        // between them. Root sections (parent_path_of == None) match the next
        // root section; nested sections match the next section whose parent
        // path prefix is identical.
        let this_parent = parent_path_of(path_id);
        let next_sibling = sections[i + 1..].iter().find(|s| {
            let Some(sub_path) = s.path_id.as_deref() else {
                return false;
            };
            // A sibling must be a real graph node (populated section_id).
            if s.section_id.is_none() {
                return false;
            }
            match this_parent {
                None => parent_path_of(sub_path).is_none(),
                Some(parent) => parent_path_of(sub_path) == Some(parent),
            }
        });
        if let Some(subsequent) = next_sibling
            && let Some(sub_sid) = subsequent.section_id.as_deref()
            && let Some(&sibling_node) = section_id_map.get(sub_sid)
        {
            edges.push(NavEdge {
                from: this_node,
                to: sibling_node,
                relation: NavRelation::NextSibling,
            });
        }

        // first_child edge: this → first child (a section whose parent is this).
        // Find the first subsequent section whose parent_path == this path_id.
        if let Some(child) = sections[i + 1..].iter().find(|s| {
            s.path_id
                .as_deref()
                .map(|cp| parent_path_of(cp) == Some(path_id))
                .unwrap_or(false)
        }) && let Some(child_sid) = child.section_id.as_deref()
            && let Some(&child_node) = section_id_map.get(child_sid)
        {
            edges.push(NavEdge {
                from: this_node,
                to: child_node,
                relation: NavRelation::FirstChild,
            });
        }
    }

    edges
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

    // ─── ADR-053 Layer A T04: build_section_navigation_edges tests ───

    #[test]
    fn test_parent_path_of_root_returns_none() {
        assert_eq!(parent_path_of("1"), None);
        assert_eq!(parent_path_of("2"), None);
    }

    #[test]
    fn test_parent_path_of_nested_returns_parent() {
        assert_eq!(parent_path_of("1.1"), Some("1"));
        assert_eq!(parent_path_of("2.1.1"), Some("2.1"));
        assert_eq!(parent_path_of("3.2.1.4"), Some("3.2.1"));
    }

    #[tokio::test]
    async fn test_build_navigation_edges_empty_when_no_section_ids() {
        use da_ports::parser::Section;
        // Legacy flat-section parser output: no section_id populated.
        let sections = vec![
            Section {
                title: "Intro".into(),
                text: "text".into(),
                level: 1,
                path_id: None,
                parent_section_id: None,
                order_in_parent: None,
                section_id: None,
            },
            Section {
                title: "Methods".into(),
                text: "text".into(),
                level: 1,
                path_id: None,
                parent_section_id: None,
                order_in_parent: None,
                section_id: None,
            },
        ];
        let section_id_map = std::collections::HashMap::new();
        let store = kg_storage::MockGraphStore::new();
        let edges = build_section_navigation_edges(&sections, &section_id_map, &store)
            .await
            .expect("navigation edges build");
        assert_eq!(edges, 0, "no navigation edges when section_id_map is empty");
    }

    #[tokio::test]
    async fn test_build_navigation_edges_nested_tree() {
        use da_ports::parser::Section;
        // Build a simple 3-level tree:
        //   1 (root)
        //   1.1 (child of 1)
        //   1.2 (child of 1, sibling of 1.1)
        //   2 (root)
        //   2.1 (child of 2)
        let sections = vec![
            Section {
                title: "Intro".into(),
                text: "text".into(),
                level: 1,
                path_id: Some("1".into()),
                section_id: Some("sec-1".into()),
                parent_section_id: None,
                order_in_parent: Some(0),
            },
            Section {
                title: "Background".into(),
                text: "text".into(),
                level: 2,
                path_id: Some("1.1".into()),
                section_id: Some("sec-1.1".into()),
                parent_section_id: Some("sec-1".into()),
                order_in_parent: Some(0),
            },
            Section {
                title: "Motivation".into(),
                text: "text".into(),
                level: 2,
                path_id: Some("1.2".into()),
                section_id: Some("sec-1.2".into()),
                parent_section_id: Some("sec-1".into()),
                order_in_parent: Some(1),
            },
            Section {
                title: "Methods".into(),
                text: "text".into(),
                level: 1,
                path_id: Some("2".into()),
                section_id: Some("sec-2".into()),
                parent_section_id: None,
                order_in_parent: Some(0),
            },
            Section {
                title: "Model".into(),
                text: "text".into(),
                level: 2,
                path_id: Some("2.1".into()),
                section_id: Some("sec-2.1".into()),
                parent_section_id: Some("sec-2".into()),
                order_in_parent: Some(0),
            },
        ];
        let mut section_id_map = std::collections::HashMap::new();
        // Mock node ids assigned by MockGraphStore start from 0.
        for (i, sid) in ["sec-1", "sec-1.1", "sec-1.2", "sec-2", "sec-2.1"]
            .iter()
            .enumerate()
        {
            section_id_map.insert((*sid).to_string(), i as u64);
        }
        let store = kg_storage::MockGraphStore::new();
        let edges = build_section_navigation_edges(&sections, &section_id_map, &store)
            .await
            .expect("navigation edges build");
        // Expected edges:
        //   1 -> next_sibling -> 2
        //   1 -> first_child -> 1.1
        //   1.1 -> parent_section -> 1
        //   1.1 -> next_sibling -> 1.2
        //   1.2 -> parent_section -> 1
        //   2 -> first_child -> 2.1
        //   2.1 -> parent_section -> 2
        // Total = 7 edges (root sibling 1 -> 2 is emitted now that sibling
        // selection uses canonical parent identity, M398 S01 T01)
        assert_eq!(
            edges, 7,
            "expected 7 navigation edges for 5-section tree, got {}",
            edges
        );
        let replay = build_section_navigation_edges(&sections, &section_id_map, &store)
            .await
            .expect("navigation replay");
        assert_eq!(replay, 0, "navigation replay must create zero net edges");
    }

    #[tokio::test]
    async fn test_build_navigation_edges_deep_tree_descendant_interruption() {
        use da_ports::parser::Section;
        // Deep-tree regression fixture (M398 S01 T01): siblings whose direct
        // successor is hidden behind one or more levels of descendants.
        //   1 (root)
        //   1.1 (child of 1)
        //   1.1.1 (child of 1.1 — descends between 1.1 and its sibling 1.2)
        //   1.2 (child of 1, sibling of 1.1)
        //   1.2.1 (child of 1.2 — descends between 1.2 and its sibling 1.3)
        //   1.3 (child of 1, sibling of 1.2)
        //   2 (root, sibling of 1)
        //   2.1 (child of 2)
        let mk = |title: &str,
                  path: Option<&str>,
                  sid: Option<&str>,
                  parent: Option<&str>,
                  order: Option<u32>| Section {
            title: title.into(),
            text: "text".into(),
            level: 0,
            path_id: path.map(String::from),
            section_id: sid.map(String::from),
            parent_section_id: parent.map(String::from),
            order_in_parent: order,
        };
        let sections = vec![
            mk("Intro", Some("1"), Some("sec-1"), None, Some(0)),
            mk(
                "Background",
                Some("1.1"),
                Some("sec-1.1"),
                Some("sec-1"),
                Some(0),
            ),
            mk(
                "Sub-detail",
                Some("1.1.1"),
                Some("sec-1.1.1"),
                Some("sec-1.1"),
                Some(0),
            ),
            mk(
                "Motivation",
                Some("1.2"),
                Some("sec-1.2"),
                Some("sec-1"),
                Some(1),
            ),
            mk(
                "Deep-child",
                Some("1.2.1"),
                Some("sec-1.2.1"),
                Some("sec-1.2"),
                Some(0),
            ),
            mk(
                "Related",
                Some("1.3"),
                Some("sec-1.3"),
                Some("sec-1"),
                Some(2),
            ),
            mk("Methods", Some("2"), Some("sec-2"), None, Some(1)),
            mk(
                "Model",
                Some("2.1"),
                Some("sec-2.1"),
                Some("sec-2"),
                Some(0),
            ),
        ];
        let mut section_id_map = std::collections::HashMap::new();
        for (i, sid) in [
            "sec-1",
            "sec-1.1",
            "sec-1.1.1",
            "sec-1.2",
            "sec-1.2.1",
            "sec-1.3",
            "sec-2",
            "sec-2.1",
        ]
        .iter()
        .enumerate()
        {
            section_id_map.insert((*sid).to_string(), i as u64);
        }
        let store = kg_storage::MockGraphStore::new();
        let edges = build_section_navigation_edges(&sections, &section_id_map, &store)
            .await
            .expect("navigation edges build");
        // Expected edges:
        //   1 -> next_sibling -> 2            (root sibling past 1.1/1.1.1/1.2/1.2.1/1.3)
        //   1 -> first_child -> 1.1
        //   1.1 -> parent_section -> 1
        //   1.1 -> next_sibling -> 1.2        (sibling past descendant 1.1.1)
        //   1.1 -> first_child -> 1.1.1
        //   1.1.1 -> parent_section -> 1.1
        //   1.2 -> parent_section -> 1
        //   1.2 -> next_sibling -> 1.3        (sibling past descendant 1.2.1)
        //   1.2 -> first_child -> 1.2.1
        //   1.2.1 -> parent_section -> 1.2
        //   1.3 -> parent_section -> 1
        //   2 -> first_child -> 2.1
        //   2.1 -> parent_section -> 2
        // Total = 13 edges (1.2 has no sibling edge to root section 2:
        // same-parent identity is canonical, root 2 is not 1.2's sibling)
        assert_eq!(
            edges, 13,
            "expected 13 navigation edges for deep 8-section tree, got {}",
            edges
        );
        let replay = build_section_navigation_edges(&sections, &section_id_map, &store)
            .await
            .expect("navigation replay");
        assert_eq!(replay, 0, "deep-tree navigation replay must be idempotent");
    }

    // Integration tests require live services (GROBID, fd_api, Samyama embedded)
}
