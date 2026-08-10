//! Summary generation use case — ADR-053 Layer B (multi-layer).
//!
//! ONE LLM call with FULL article context → structured JSON → graph nodes.
//! Uses MiniMax-M2.7-highspeed (Anthropic-compatible, large context window).
//!
//! Pipeline position: after ingest + extract (needs Paper, Section, Entity nodes).
//! Creates: Summary nodes, enriches Entity nodes, creates QualitySignal nodes.

use da_ports::graph_store::DirectGraphStore;
use da_ports::llm_client::{ChatMessage, ChatOptions, LLMClient};
use da_ports::parser::ParsedArticle;
use da_ports::{Clock, SystemClock};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Summary generation use case (ADR-053 Layer B, full-context single-prompt).
pub struct SummaryUseCase {
    graph_store: std::sync::Arc<dyn DirectGraphStore>,
    llm: std::sync::Arc<dyn LLMClient>,
    clock: std::sync::Arc<dyn Clock>,
}

struct SemanticEntityWrite<'a> {
    label: &'static str,
    name: &'a str,
    description: &'a str,
    value: Option<&'a str>,
    baseline: Option<&'a str>,
}

/// Structured LLM response — rich JSON from one prompt.
#[derive(Debug, Deserialize, Default)]
pub struct StructuredSummary {
    #[serde(default)]
    pub abstract_summary: String,
    #[serde(default)]
    pub key_contributions: Vec<String>,
    #[serde(default)]
    pub section_summaries: Vec<SectionSummaryLLM>,
    #[serde(default)]
    pub methods: Vec<EntityLLM>,
    #[serde(default)]
    pub datasets: Vec<EntityLLM>,
    #[serde(default)]
    pub metrics: Vec<MetricLLM>,
    #[serde(default)]
    pub quantitative_results: String,
    #[serde(default)]
    pub limitations: String,
    #[serde(default)]
    pub future_work: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub glossary: Vec<GlossaryEntryLLM>,
    #[serde(default)]
    pub reproducibility: ReproducibilityLLM,
    #[serde(default)]
    pub reading_difficulty: ReadingDifficultyLLM,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SectionSummaryLLM {
    pub path_id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EntityLLM {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub how_used: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricLLM {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub baseline: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GlossaryEntryLLM {
    pub term: String,
    pub definition: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReproducibilityLLM {
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub justification: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReadingDifficultyLLM {
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub target_audience: String,
}

/// Result of summary generation.
#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub paper_id: String,
    pub abstract_summary: Option<String>,
    pub section_summaries_count: usize,
    pub methods_count: usize,
    pub datasets_count: usize,
    pub metrics_count: usize,
    pub tags: Vec<String>,
    pub reproducibility_score: Option<u32>,
    pub tokens_used: u32,
    pub error: Option<String>,
}

impl SummaryUseCase {
    pub fn new(
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
        llm: std::sync::Arc<dyn LLMClient>,
    ) -> Self {
        Self {
            graph_store,
            llm,
            clock: std::sync::Arc::new(SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Generate full structured summary from ONE LLM call with full article context.
    pub async fn generate_summaries(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        parsed: &ParsedArticle,
    ) -> anyhow::Result<SummaryResult> {
        if !self.llm.can_make_request() {
            return Ok(SummaryResult {
                paper_id: paper_id.to_string(),
                abstract_summary: None,
                section_summaries_count: 0,
                methods_count: 0,
                datasets_count: 0,
                metrics_count: 0,
                tags: vec![],
                reproducibility_score: None,
                tokens_used: 0,
                error: Some("LLM not configured".into()),
            });
        }

        // Build ONE comprehensive prompt with full article context.
        let prompt = self.build_full_context_prompt(paper_id, parsed);

        // Dynamic max_tokens: base 8000 + scale with body size. Cap at 16000.
        let body_tokens_estimate = parsed.body_text.len() / 4; // ~4 chars per token
        let max_tokens = (8000 + body_tokens_estimate / 10).min(16000);

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: prompt,
        }];
        let opts = ChatOptions {
            model: String::new(),
            max_tokens,
            temperature: 0.3,
        };

        tracing::info!(
            paper_id,
            body_chars = parsed.body_text.len(),
            sections = parsed.sections.len(),
            max_tokens,
            "Layer B: sending full article to LLM (single prompt)"
        );

        let resp = match self.llm.chat(&messages, &opts).await {
            Ok(r) if r.ok => r,
            Ok(r) => {
                return Ok(SummaryResult {
                    paper_id: paper_id.to_string(),
                    abstract_summary: None,
                    section_summaries_count: 0,
                    methods_count: 0,
                    datasets_count: 0,
                    metrics_count: 0,
                    tags: vec![],
                    reproducibility_score: None,
                    tokens_used: 0,
                    error: r.error,
                });
            }
            Err(e) => {
                return Ok(SummaryResult {
                    paper_id: paper_id.to_string(),
                    abstract_summary: None,
                    section_summaries_count: 0,
                    methods_count: 0,
                    datasets_count: 0,
                    metrics_count: 0,
                    tags: vec![],
                    reproducibility_score: None,
                    tokens_used: 0,
                    error: Some(e.to_string()),
                });
            }
        };

        let tokens_used = resp.tokens_out.unwrap_or(0);

        // Parse JSON from LLM response.
        let cleaned = resp
            .text
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim();
        let structured: StructuredSummary = match serde_json::from_str(cleaned) {
            Ok(s) => s,
            Err(e) => {
                let response_sha256 = hex::encode(Sha256::digest(cleaned.as_bytes()));
                tracing::error!(
                    event = "summary_response_invalid_json",
                    paper_id,
                    error = %e,
                    response_chars = cleaned.chars().count(),
                    response_sha256,
                    "LLM summary response failed JSON validation"
                );
                return Ok(SummaryResult {
                    paper_id: paper_id.to_string(),
                    abstract_summary: None,
                    section_summaries_count: 0,
                    methods_count: 0,
                    datasets_count: 0,
                    metrics_count: 0,
                    tags: vec![],
                    reproducibility_score: None,
                    tokens_used,
                    error: Some(format!("JSON parse: {e}")),
                });
            }
        };

        // Write structured data to graph.
        self.write_to_graph(paper_id, paper_node_id, &structured)
            .await?;

        Ok(SummaryResult {
            paper_id: paper_id.to_string(),
            abstract_summary: if structured.abstract_summary.is_empty() {
                None
            } else {
                Some(structured.abstract_summary.clone())
            },
            section_summaries_count: structured.section_summaries.len(),
            methods_count: structured.methods.len(),
            datasets_count: structured.datasets.len(),
            metrics_count: structured.metrics.len(),
            tags: structured.tags.clone(),
            reproducibility_score: if structured.reproducibility.score > 0 {
                Some(structured.reproducibility.score)
            } else {
                None
            },
            tokens_used,
            error: None,
        })
    }

    /// Build comprehensive prompt with full article context.
    fn build_full_context_prompt(&self, paper_id: &str, parsed: &ParsedArticle) -> String {
        let mut prompt = String::with_capacity(50000);

        prompt.push_str("You are an expert research assistant analyzing a scientific paper for a knowledge graph.\n");
        prompt.push_str("Read the FULL article below and return a structured JSON response.\n\n");
        prompt.push_str(
            "Return ONLY valid JSON (no markdown fences, no commentary) with this schema:\n",
        );
        prompt.push_str(r#"{
  "abstract_summary": "Problem/Approach/Results/Limitations with SPECIFIC numbers",
  "key_contributions": ["contribution1", ...],
  "section_summaries": [{"path_id": "1", "title": "...", "summary": "1-2 sentences with key details"}],
  "methods": [{"name": "...", "description": "...", "how_used": "..."}],
  "datasets": [{"name": "...", "description": "..."}],
  "metrics": [{"name": "...", "value": "...", "baseline": "..."}],
  "quantitative_results": "All numbers: accuracy, improvement %, speedup, etc.",
  "limitations": "What the authors acknowledge as limitations",
  "future_work": ["direction1", ...],
  "tags": ["keyword1", ...],
  "glossary": [{"term": "...", "definition": "..."}],
  "reproducibility": {"score": 1-5, "justification": "..."},
  "reading_difficulty": {"score": 1-5, "target_audience": "..."}
}

RULES:
- Be SPECIFIC. Include numbers, method names, dataset names. No filler.
- section_summaries: cover ALL major sections (at least 10-20 key ones).
- methods: extract ALL methods/techniques mentioned (not just proposed).
- glossary: define key domain terms a non-expert wouldn't know.

"#);

        // Paper metadata.
        prompt.push_str(&format!("Paper ID: {}\n", paper_id));
        prompt_str(&mut prompt, "Title", &parsed.title);
        prompt.push('\n');

        // Full abstract.
        prompt.push_str("=== ABSTRACT ===\n");
        prompt.push_str(&parsed.abstract_text);
        prompt.push_str("\n\n");

        // Section structure: all titles with path_id.
        prompt.push_str("=== SECTION STRUCTURE ===\n");
        for s in &parsed.sections {
            prompt.push_str(&format!(
                "  [{}] {}\n",
                s.path_id.as_deref().unwrap_or("?"),
                s.title
            ));
        }
        prompt.push('\n');

        // Full body text — the entire article.
        prompt.push_str("=== FULL ARTICLE TEXT ===\n");
        prompt.push_str(&parsed.body_text);
        prompt.push_str("\n\n");

        // Already extracted entities would go here, but ParsedArticle doesn't
        // carry them. Entities come from separate extraction step. The LLM gets
        // full article text and extracts its own methods/datasets/metrics.

        prompt.push_str("=== END OF ARTICLE ===\n\n");
        prompt.push_str("Return the JSON now.");

        prompt
    }

    /// Write structured LLM output to graph as Summary + QualitySignal nodes.
    async fn write_to_graph(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        s: &StructuredSummary,
    ) -> anyhow::Result<()> {
        use da_domain::relation::structure::HAS_SUMMARY;

        // Fail before any graph mutation if an LLM section identity is stale or
        // fabricated. This prevents partial summary writes and orphan digests.
        for section_summary in &s.section_summaries {
            if self
                .resolve_section_node(paper_id, &section_summary.path_id)
                .await
                .is_none()
            {
                anyhow::bail!(
                    "section summary path_id '{}' does not resolve for paper '{}'",
                    section_summary.path_id,
                    paper_id
                );
            }
        }

        // 1. Abstract summary → Summary node.
        if !s.abstract_summary.is_empty() {
            let summary_vid = format!("vid:summary:{paper_id}:abstract");
            let node = match self
                .graph_store
                .find_node_by_string_property("Summary", "vid", &summary_vid)
                .await
            {
                Some(existing) => existing,
                None => self.graph_store.create_node("Summary").await?,
            };
            self.set_props_summary(
                node,
                paper_id,
                &s.abstract_summary,
                "abstract",
                "llm_minimax_full",
            )
            .await?;
            if self
                .graph_store
                .get_edges_between(paper_node_id, node, HAS_SUMMARY)
                .await
                .is_empty()
            {
                self.graph_store
                    .create_edge(paper_node_id, node, HAS_SUMMARY)
                    .await?;
            }
            tracing::info!(
                paper_id,
                chars = s.abstract_summary.len(),
                "Summary: abstract (LLM)"
            );
        }

        // 2. Section summaries → Summary nodes linked to exact path identities.
        for section_summary in &s.section_summaries {
            self.write_section_summary(paper_id, section_summary)
                .await?;
        }

        // 3. Typed semantic entities already present in the structured response.
        for method in &s.methods {
            self.upsert_semantic_entity(
                paper_id,
                paper_node_id,
                SemanticEntityWrite {
                    label: "Method",
                    name: &method.name,
                    description: &method.description,
                    value: None,
                    baseline: None,
                },
            )
            .await?;
        }
        for dataset in &s.datasets {
            self.upsert_semantic_entity(
                paper_id,
                paper_node_id,
                SemanticEntityWrite {
                    label: "Dataset",
                    name: &dataset.name,
                    description: &dataset.description,
                    value: None,
                    baseline: None,
                },
            )
            .await?;
        }
        for metric in &s.metrics {
            self.upsert_semantic_entity(
                paper_id,
                paper_node_id,
                SemanticEntityWrite {
                    label: "Metric",
                    name: &metric.name,
                    description: "",
                    value: Some(&metric.value),
                    baseline: Some(&metric.baseline),
                },
            )
            .await?;
        }

        // 4. Quantitative results → QualitySignal node.
        if !s.quantitative_results.is_empty() {
            self.upsert_quality_signal(
                paper_id,
                paper_node_id,
                "quant",
                "quantitative_results",
                &s.quantitative_results,
                None,
            )
            .await?;
        }

        // 5. Reproducibility → QualitySignal.
        if s.reproducibility.score > 0 {
            self.upsert_quality_signal(
                paper_id,
                paper_node_id,
                "repro",
                "reproducibility",
                &s.reproducibility.justification,
                Some(s.reproducibility.score as i64),
            )
            .await?;
        }

        // 6. Tags → store as Paper property for search.
        if !s.tags.is_empty() {
            self.set_paper_tags(paper_node_id, &s.tags).await?;
        }

        // 7. Limitations → Summary node (granularity=limitations).
        if !s.limitations.is_empty() {
            self.write_limitations(paper_id, paper_node_id, &s.limitations)
                .await?;
        }

        Ok(())
    }

    async fn write_section_summary(
        &self,
        paper_id: &str,
        summary: &SectionSummaryLLM,
    ) -> anyhow::Result<u64> {
        let section = self
            .resolve_section_node(paper_id, &summary.path_id)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "section summary path_id '{}' does not resolve for paper '{}'",
                    summary.path_id,
                    paper_id
                )
            })?;
        let summary_vid = format!("vid:summary:{paper_id}:section:{}", summary.path_id);
        let node = match self
            .graph_store
            .find_node_by_string_property("Summary", "vid", &summary_vid)
            .await
        {
            Some(existing) => existing,
            None => self.graph_store.create_node("Summary").await?,
        };
        self.graph_store
            .set_node_property_string(node, "vid", summary_vid)
            .await?;
        self.graph_store
            .set_node_property_string(node, "text", summary.summary.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node, "granularity", "section".into())
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_label", "Section".into())
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_id", summary.path_id.clone())
            .await?;
        self.graph_store
            .set_node_property_string(node, "method", "llm_minimax_full".into())
            .await?;
        self.graph_store
            .set_node_property_int(node, "char_count", summary.summary.len() as i64)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "import_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 1)
            .await?;
        if self
            .graph_store
            .get_edges_between(section, node, da_domain::relation::structure::HAS_SUMMARY)
            .await
            .is_empty()
        {
            self.graph_store
                .create_edge(section, node, da_domain::relation::structure::HAS_SUMMARY)
                .await?;
        }
        tracing::info!(
            paper_id,
            path_id = summary.path_id,
            summary_node_id = node,
            "Summary: linked section digest"
        );
        Ok(node)
    }

    async fn resolve_section_node(&self, paper_id: &str, path_id: &str) -> Option<u64> {
        let work_vid = da_domain::vid::paper_vid(paper_id);
        for node in self.graph_store.get_nodes_by_label("Section").await {
            let node_path = self
                .graph_store
                .get_node_property_string(node, "path_id")
                .await;
            let node_work = self
                .graph_store
                .get_node_property_string(node, "work_vid")
                .await;
            if node_path.as_deref() == Some(path_id) && node_work.as_deref() == Some(&work_vid) {
                return Some(node);
            }
        }
        None
    }

    async fn upsert_quality_signal(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        identity_suffix: &str,
        signal_type: &str,
        text: &str,
        score: Option<i64>,
    ) -> anyhow::Result<u64> {
        let signal_vid = format!("vid:signal:{paper_id}:{identity_suffix}");
        let node = match self
            .graph_store
            .find_node_by_string_property("QualitySignal", "vid", &signal_vid)
            .await
        {
            Some(existing) => existing,
            None => self.graph_store.create_node("QualitySignal").await?,
        };
        self.graph_store
            .set_node_property_string(node, "vid", signal_vid)
            .await?;
        self.graph_store
            .set_node_property_string(node, "text", text.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "signal_type", signal_type.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_label", "Paper".into())
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_id", paper_id.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "method", "llm_minimax_full".into())
            .await?;
        if let Some(score) = score {
            self.graph_store
                .set_node_property_int(node, "score", score)
                .await?;
        }
        self.graph_store
            .set_node_property_bool(node, "import_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 1)
            .await?;
        da_domain::validate_compiled_edge_endpoint(
            da_domain::relation::structure::HAS_QUALITY_SIGNAL,
            "Paper",
            "QualitySignal",
        )
        .map_err(anyhow::Error::msg)?;
        if self
            .graph_store
            .get_edges_between(
                paper_node_id,
                node,
                da_domain::relation::structure::HAS_QUALITY_SIGNAL,
            )
            .await
            .is_empty()
        {
            let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
            crate::temporal_stamp::create_edge_with_temporal(
                self.graph_store.as_ref(),
                paper_node_id,
                node,
                da_domain::relation::structure::HAS_QUALITY_SIGNAL,
                &temporal,
            )
            .await?;
        }
        Ok(node)
    }

    async fn write_limitations(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        text: &str,
    ) -> anyhow::Result<()> {
        let summary_vid = format!("vid:summary:{paper_id}:limitations");
        let node = match self
            .graph_store
            .find_node_by_string_property("Summary", "vid", &summary_vid)
            .await
        {
            Some(existing) => existing,
            None => self.graph_store.create_node("Summary").await?,
        };
        self.set_props_summary(node, paper_id, text, "limitations", "llm_minimax_full")
            .await?;
        if self
            .graph_store
            .get_edges_between(
                paper_node_id,
                node,
                da_domain::relation::structure::HAS_SUMMARY,
            )
            .await
            .is_empty()
        {
            self.graph_store
                .create_edge(
                    paper_node_id,
                    node,
                    da_domain::relation::structure::HAS_SUMMARY,
                )
                .await?;
        }
        Ok(())
    }

    async fn set_paper_tags(&self, paper_node_id: u64, tags: &[String]) -> anyhow::Result<()> {
        self.graph_store
            .set_node_property_string(paper_node_id, "llm_tags", tags.join(", "))
            .await?;
        Ok(())
    }

    async fn upsert_semantic_entity(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        entity: SemanticEntityWrite<'_>,
    ) -> anyhow::Result<Option<u64>> {
        let name = entity.name.trim();
        if name.is_empty() {
            tracing::warn!(
                paper_id,
                label = entity.label,
                "Skipping unnamed semantic entity"
            );
            return Ok(None);
        }
        let entity_vid = da_domain::vid::entity_vid(entity.label, name);
        let (node, created) = match self
            .graph_store
            .find_node_by_string_property(entity.label, "vid", &entity_vid)
            .await
        {
            Some(existing) => (existing, false),
            None => {
                let node = match entity.label {
                    "Method" => self.create_method_node().await?,
                    "Dataset" => self.create_dataset_node().await?,
                    "Metric" => self.create_metric_node().await?,
                    other => anyhow::bail!("unsupported structured semantic label: {other}"),
                };
                self.graph_store
                    .set_node_property_string(node, "vid", entity_vid)
                    .await?;
                self.graph_store
                    .set_node_property_string(node, "name", name.to_string())
                    .await?;
                self.graph_store
                    .set_node_property_string(node, "paper_arxiv_id", paper_id.to_string())
                    .await?;
                self.graph_store
                    .set_node_property_bool(node, "retrieval_eligible", true)
                    .await?;
                self.graph_store
                    .set_node_property_bool(node, "import_eligible", false)
                    .await?;
                self.graph_store
                    .set_node_property_int(node, "schema_version", 1)
                    .await?;
                self.graph_store
                    .set_node_property_int(node, "valid_from", self.clock.now().timestamp())
                    .await?;
                if !entity.description.trim().is_empty() {
                    self.graph_store
                        .set_node_property_string(
                            node,
                            "description",
                            entity.description.trim().to_string(),
                        )
                        .await?;
                }
                if let Some(value) = entity.value.filter(|value| !value.trim().is_empty()) {
                    self.graph_store
                        .set_node_property_string(node, "value", value.trim().to_string())
                        .await?;
                }
                if let Some(baseline) = entity.baseline.filter(|value| !value.trim().is_empty()) {
                    self.graph_store
                        .set_node_property_string(node, "baseline_for", baseline.trim().to_string())
                        .await?;
                }
                (node, true)
            }
        };
        let existing = self
            .graph_store
            .get_edges_between(
                paper_node_id,
                node,
                da_domain::relation::bibliographic::MENTIONS,
            )
            .await;
        if existing.is_empty() {
            let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
            crate::temporal_stamp::create_edge_with_temporal(
                self.graph_store.as_ref(),
                paper_node_id,
                node,
                da_domain::relation::bibliographic::MENTIONS,
                &temporal,
            )
            .await?;
        }
        tracing::info!(
            paper_id,
            label = entity.label,
            name,
            created,
            linked = existing.is_empty(),
            "Summary: typed semantic entity"
        );
        Ok(Some(node))
    }

    async fn set_props_summary(
        &self,
        node: u64,
        paper_id: &str,
        text: &str,
        granularity: &str,
        method: &str,
    ) -> anyhow::Result<()> {
        self.graph_store
            .set_node_property_string(
                node,
                "vid",
                format!("vid:summary:{}:{}", paper_id, granularity),
            )
            .await?;
        self.graph_store
            .set_node_property_string(node, "text", text.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "granularity", granularity.to_string())
            .await?;
        self.graph_store
            .set_node_property_int(node, "char_count", text.len() as i64)
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_label", "Paper".into())
            .await?;
        self.graph_store
            .set_node_property_string(node, "source_id", paper_id.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(node, "method", method.to_string())
            .await?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "import_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 1)
            .await?;
        Ok(())
    }

    async fn create_method_node(&self) -> anyhow::Result<u64> {
        Ok(self.graph_store.create_node("Method").await?)
    }

    async fn create_dataset_node(&self) -> anyhow::Result<u64> {
        Ok(self.graph_store.create_node("Dataset").await?)
    }

    async fn create_metric_node(&self) -> anyhow::Result<u64> {
        Ok(self.graph_store.create_node("Metric").await?)
    }
}

fn prompt_str(out: &mut String, label: &str, val: &str) {
    if !val.is_empty() {
        out.push_str(&format!("{}: {}\n", label, val));
    }
}
