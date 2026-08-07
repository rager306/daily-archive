//! Summary generation use case — ADR-053 Layer B.
//!
//! Uses LLM (Anthropic-compatible: MiniMax, Claude) to generate
//! multi-granularity summaries from paper abstract + section titles.
//! NOT abstract_copy — real LLM-generated digest.
//!
//! Pipeline position: after ingest (has abstract + sections), before extraction.
//! Creates Summary nodes linked via hasSummary edge.

use std::sync::Arc;

use da_ports::graph_store::DirectGraphStore;
use da_ports::llm_client::{ChatMessage, ChatOptions, LLMClient};
use da_ports::parser::ParsedArticle;

/// Summary generation use case (ADR-053 Layer B).
pub struct SummaryUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
    llm: Arc<dyn LLMClient>,
}

/// Result of summary generation for a single paper.
#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub paper_id: String,
    pub abstract_summary: Option<String>,
    pub section_summaries: Vec<SectionSummary>,
    pub tokens_used: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SectionSummary {
    pub section_title: String,
    pub summary: String,
}

impl SummaryUseCase {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, llm: Arc<dyn LLMClient>) -> Self {
        Self { graph_store, llm }
    }

    /// Generate summaries for a paper and write Summary nodes to the graph.
    /// Requires: Paper node + Section nodes already exist (after ingest).
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
                section_summaries: vec![],
                tokens_used: 0,
                error: Some("LLM not configured (ANTHROPIC_API_KEY not set)".into()),
            });
        }

        let mut tokens_used = 0u32;
        let mut section_summaries = vec![];

        // 1. Generate abstract-level summary (concise digest of the paper).
        let abstract_summary = if !parsed.abstract_text.is_empty() {
            let prompt = format!(
                "Summarize this paper abstract in 2-3 sentences. Focus on: what problem, what approach, what result.\n\nAbstract: {}",
                parsed.abstract_text
            );
            let messages = vec![ChatMessage {
                role: "user".into(),
                content: prompt,
            }];
            let opts = ChatOptions {
                model: String::new(),
                max_tokens: 300,
                temperature: 0.3,
            };
            match self.llm.chat(&messages, &opts).await {
                Ok(resp) if resp.ok => {
                    tokens_used += resp.tokens_out.unwrap_or(0);
                    Some(resp.text)
                }
                Ok(resp) => {
                    return Ok(SummaryResult {
                        paper_id: paper_id.to_string(),
                        abstract_summary: None,
                        section_summaries: vec![],
                        tokens_used,
                        error: resp.error,
                    });
                }
                Err(e) => {
                    return Ok(SummaryResult {
                        paper_id: paper_id.to_string(),
                        abstract_summary: None,
                        section_summaries: vec![],
                        tokens_used,
                        error: Some(e.to_string()),
                    });
                }
            }
        } else {
            None
        };

        // 2. Write abstract summary to graph as Summary node.
        if let Some(ref summary_text) = abstract_summary {
            let summary_node = self.graph_store.create_node("Summary").await?;
            self.graph_store
                .set_node_property_string(
                    summary_node,
                    "vid",
                    format!("vid:summary:{paper_id}:abstract"),
                )
                .await?;
            self.graph_store
                .set_node_property_string(summary_node, "text", summary_text.clone())
                .await?;
            self.graph_store
                .set_node_property_string(summary_node, "granularity", "abstract".to_string())
                .await?;
            self.graph_store
                .set_node_property_int(summary_node, "char_count", summary_text.len() as i64)
                .await?;
            self.graph_store
                .set_node_property_string(summary_node, "source_label", "Paper".to_string())
                .await?;
            self.graph_store
                .set_node_property_string(summary_node, "source_id", paper_id.to_string())
                .await?;
            self.graph_store
                .set_node_property_string(summary_node, "method", "llm_minimax".to_string())
                .await?;
            self.graph_store
                .set_node_property_bool(summary_node, "retrieval_eligible", true)
                .await?;
            self.graph_store
                .set_node_property_bool(summary_node, "import_eligible", false) // D127
                .await?;
            self.graph_store
                .set_node_property_int(summary_node, "schema_version", 1)
                .await?;
            // Link Paper → Summary via hasSummary edge.
            self.graph_store
                .create_edge(
                    paper_node_id,
                    summary_node,
                    da_domain::relation::structure::HAS_SUMMARY,
                )
                .await?;
            tracing::info!(
                paper_id,
                chars = summary_text.len(),
                tokens = tokens_used,
                "Layer B: LLM summary created (granularity=abstract)"
            );
        }

        // 3. Generate section-level summaries (first 5 sections only — cost control).
        for section in parsed.sections.iter().take(5) {
            if section.text.is_empty() || section.text.len() < 100 {
                continue;
            }
            let prompt = format!(
                "Summarize this section in 1-2 sentences. Section title: {}\n\nText (first 500 chars): {}",
                section.title,
                &section.text[..section.text.len().min(500)]
            );
            let messages = vec![ChatMessage {
                role: "user".into(),
                content: prompt,
            }];
            let opts = ChatOptions {
                model: String::new(),
                max_tokens: 150,
                temperature: 0.3,
            };
            if let Ok(resp) = self.llm.chat(&messages, &opts).await {
                if resp.ok {
                    tokens_used += resp.tokens_out.unwrap_or(0);
                    section_summaries.push(SectionSummary {
                        section_title: section.title.clone(),
                        summary: resp.text,
                    });
                }
            }
        }

        Ok(SummaryResult {
            paper_id: paper_id.to_string(),
            abstract_summary,
            section_summaries,
            tokens_used,
            error: None,
        })
    }
}
