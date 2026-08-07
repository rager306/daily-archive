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
use serde::Deserialize;

/// Summary generation use case (ADR-053 Layer B, full-context single-prompt).
pub struct SummaryUseCase {
    graph_store: std::sync::Arc<dyn DirectGraphStore>,
    llm: std::sync::Arc<dyn LLMClient>,
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
        Self { graph_store, llm }
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
                tracing::error!(paper_id, error = %e, "LLM JSON parse failed");
                tracing::debug!(paper_id, raw = %cleaned[..cleaned.len().min(2000)], "Raw LLM response");
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

        // 1. Abstract summary → Summary node.
        if !s.abstract_summary.is_empty() {
            let node = self.graph_store.create_node("Summary").await?;
            self.set_props_summary(
                node,
                paper_id,
                &s.abstract_summary,
                "abstract",
                "llm_minimax_full",
            )
            .await?;
            self.graph_store
                .create_edge(paper_node_id, node, HAS_SUMMARY)
                .await?;
            tracing::info!(
                paper_id,
                chars = s.abstract_summary.len(),
                "Summary: abstract (LLM)"
            );
        }

        // 2. Section summaries → Summary nodes.
        for ss in &s.section_summaries {
            let node = self.graph_store.create_node("Summary").await?;
            let vid = format!("vid:summary:{}:section:{}", paper_id, ss.path_id);
            self.graph_store
                .set_node_property_string(node, "vid", vid)
                .await?;
            self.graph_store
                .set_node_property_string(node, "text", ss.summary.clone())
                .await?;
            self.graph_store
                .set_node_property_string(node, "granularity", "section".into())
                .await?;
            self.graph_store
                .set_node_property_string(node, "source_label", "Section".into())
                .await?;
            self.graph_store
                .set_node_property_string(node, "source_id", ss.path_id.clone())
                .await?;
            self.graph_store
                .set_node_property_string(node, "method", "llm_minimax_full".into())
                .await?;
            self.graph_store
                .set_node_property_bool(node, "retrieval_eligible", true)
                .await?;
            self.graph_store
                .set_node_property_int(node, "schema_version", 1)
                .await?;
        }

        // 3. Quantitative results → QualitySignal node.
        if !s.quantitative_results.is_empty() {
            let node = self.graph_store.create_node("QualitySignal").await?;
            self.graph_store
                .set_node_property_string(node, "vid", format!("vid:signal:{}:quant", paper_id))
                .await?;
            self.graph_store
                .set_node_property_string(node, "text", s.quantitative_results.clone())
                .await?;
            self.graph_store
                .set_node_property_string(node, "signal_type", "quantitative_results".into())
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
            self.graph_store
                .create_edge(paper_node_id, node, HAS_SUMMARY)
                .await?;
        }

        // 4. Reproducibility → QualitySignal.
        if s.reproducibility.score > 0 {
            let node = self.graph_store.create_node("QualitySignal").await?;
            self.graph_store
                .set_node_property_string(node, "vid", format!("vid:signal:{}:repro", paper_id))
                .await?;
            self.graph_store
                .set_node_property_string(node, "text", s.reproducibility.justification.clone())
                .await?;
            self.graph_store
                .set_node_property_string(node, "signal_type", "reproducibility".into())
                .await?;
            self.graph_store
                .set_node_property_int(node, "score", s.reproducibility.score as i64)
                .await?;
            self.graph_store
                .create_edge(paper_node_id, node, HAS_SUMMARY)
                .await?;
        }

        // 5. Tags → store as Paper property for search.
        if !s.tags.is_empty() {
            self.graph_store
                .set_node_property_string(paper_node_id, "llm_tags", s.tags.join(", "))
                .await?;
        }

        // 6. Limitations → Summary node (granularity=limitations).
        if !s.limitations.is_empty() {
            let node = self.graph_store.create_node("Summary").await?;
            self.set_props_summary(
                node,
                paper_id,
                &s.limitations,
                "limitations",
                "llm_minimax_full",
            )
            .await?;
            self.graph_store
                .create_edge(paper_node_id, node, HAS_SUMMARY)
                .await?;
        }

        Ok(())
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
            .set_node_property_int(node, "schema_version", 1)
            .await?;
        Ok(())
    }
}

fn prompt_str(out: &mut String, label: &str, val: &str) {
    if !val.is_empty() {
        out.push_str(&format!("{}: {}\n", label, val));
    }
}
