//! Generative article-summary candidate use case (ADR-053, ADR-059).
//!
//! Canonical semantic entities remain owned by extraction. This use case writes
//! only lineage-bound, non-retrieval Summary candidates after strict validation.

use std::{collections::BTreeMap, sync::Arc};

use da_domain::prompt_optimization::{
    PromptExecutionLineage, PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec,
    ResolvedModelBinding,
};
use da_ports::{
    Clock, SystemClock,
    graph_store::DirectGraphStore,
    parser::ParsedArticle,
    prompt_execution::{PromptExecutionPort, PromptExecutionRequest},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub struct SummaryUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
    prompt_executor: Arc<dyn PromptExecutionPort>,
    prompt_task: PromptTaskSpec,
    prompt_route: PromptRoutePolicy,
    resolved_model: ResolvedModelBinding,
    output_schema: PromptOutputSchema,
    clock: Arc<dyn Clock>,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StructuredSummary {
    pub abstract_summary: String,
    #[serde(default)]
    pub key_contributions: Vec<String>,
    pub section_summaries: Vec<SectionSummaryLLM>,
    pub methods: Vec<EntityLLM>,
    pub datasets: Vec<EntityLLM>,
    pub metrics: Vec<MetricLLM>,
    pub quantitative_results: String,
    pub limitations: String,
    #[serde(default)]
    pub future_work: Vec<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub glossary: Vec<GlossaryEntryLLM>,
    pub reproducibility: ReproducibilityLLM,
    #[serde(default)]
    pub reading_difficulty: ReadingDifficultyLLM,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SectionSummaryLLM {
    pub path_id: String,
    #[serde(default)]
    pub title: String,
    pub summary: String,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EntityLLM {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub how_used: String,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct MetricLLM {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub baseline: String,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GlossaryEntryLLM {
    pub term: String,
    pub definition: String,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReproducibilityLLM {
    pub score: u32,
    pub justification: String,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReadingDifficultyLLM {
    pub score: u32,
    pub target_audience: String,
}
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
    pub prompt_lineage: Option<PromptExecutionLineage>,
}

impl SummaryUseCase {
    pub fn new(
        graph_store: Arc<dyn DirectGraphStore>,
        prompt_executor: Arc<dyn PromptExecutionPort>,
        prompt_task: PromptTaskSpec,
        prompt_route: PromptRoutePolicy,
        resolved_model: ResolvedModelBinding,
        output_schema: PromptOutputSchema,
    ) -> anyhow::Result<Self> {
        prompt_task.validate().map_err(anyhow::Error::msg)?;
        prompt_route.validate().map_err(anyhow::Error::msg)?;
        resolved_model.validate().map_err(anyhow::Error::msg)?;
        output_schema.validate().map_err(anyhow::Error::msg)?;
        if prompt_task.task_id != "article_summary"
            || prompt_task.route_policy_id != prompt_route.route_policy_id
            || prompt_task.output_schema_id != output_schema.output_schema_id
            || prompt_route.model_binding_id != resolved_model.binding_id
        {
            anyhow::bail!("summary prompt task route/schema contract mismatch");
        }
        Ok(Self {
            graph_store,
            prompt_executor,
            prompt_task,
            prompt_route,
            resolved_model,
            output_schema,
            clock: Arc::new(SystemClock),
        })
    }
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }
    pub async fn generate_summaries(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        parsed: &ParsedArticle,
    ) -> anyhow::Result<SummaryResult> {
        let variables = BTreeMap::from([
            ("paper_id".into(), paper_id.to_string()),
            ("title".into(), parsed.title.clone()),
            ("abstract".into(), parsed.abstract_text.clone()),
            ("full_text".into(), parsed.body_text.clone()),
        ]);
        let input_bytes = serde_json::to_vec(parsed)?;
        let run_id = format!(
            "summary:{paper_id}:{}",
            hex::encode(Sha256::digest(&input_bytes))
        );
        let response = self
            .prompt_executor
            .execute(PromptExecutionRequest {
                run_id,
                task: self.prompt_task.clone(),
                route: self.prompt_route.clone(),
                resolved_model: self.resolved_model.clone(),
                output_schema: self.output_schema.clone(),
                variables,
                input_bytes,
                media: vec![],
            })
            .await?;
        response.lineage.validate().map_err(anyhow::Error::msg)?;
        if response.lineage.task_id != self.prompt_task.task_id {
            anyhow::bail!("summary execution lineage task mismatch");
        }
        let structured: StructuredSummary = serde_json::from_slice(&response.output_bytes)?;
        self.materialize_candidates(paper_id, paper_node_id, &structured, &response.lineage)
            .await?;
        Ok(SummaryResult {
            paper_id: paper_id.into(),
            abstract_summary: (!structured.abstract_summary.is_empty())
                .then(|| structured.abstract_summary.clone()),
            section_summaries_count: structured.section_summaries.len(),
            methods_count: 0,
            datasets_count: 0,
            metrics_count: 0,
            tags: structured.tags.clone(),
            reproducibility_score: (structured.reproducibility.score > 0)
                .then_some(structured.reproducibility.score),
            tokens_used: u32::try_from(response.lineage.output_tokens).unwrap_or(u32::MAX),
            error: None,
            prompt_lineage: Some(response.lineage),
        })
    }
    async fn materialize_candidates(
        &self,
        paper_id: &str,
        paper_node_id: u64,
        summary: &StructuredSummary,
        lineage: &PromptExecutionLineage,
    ) -> anyhow::Result<()> {
        lineage.validate().map_err(anyhow::Error::msg)?;
        if lineage.task_id != self.prompt_task.task_id {
            anyhow::bail!("summary graph write lineage task mismatch");
        }
        let mut sections = Vec::with_capacity(summary.section_summaries.len());
        for candidate in &summary.section_summaries {
            let section = self
                .resolve_section_node(paper_id, &candidate.path_id)
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "section summary path_id '{}' does not resolve for paper '{}'",
                        candidate.path_id,
                        paper_id
                    )
                })?;
            sections.push((candidate, section));
        }
        if !summary.abstract_summary.trim().is_empty() {
            let node = self
                .upsert_summary_node(
                    paper_id,
                    "abstract",
                    &summary.abstract_summary,
                    "Paper",
                    paper_id,
                    lineage,
                )
                .await?;
            self.link_once(
                paper_node_id,
                node,
                da_domain::relation::structure::HAS_SUMMARY,
            )
            .await?;
        }
        for (candidate, section_node) in sections {
            let granularity = format!("section:{}", candidate.path_id);
            let node = self
                .upsert_summary_node(
                    paper_id,
                    &granularity,
                    &candidate.summary,
                    "Section",
                    &format!("vid:section:{paper_id}:{}", candidate.path_id),
                    lineage,
                )
                .await?;
            self.link_once(
                section_node,
                node,
                da_domain::relation::structure::HAS_SUMMARY,
            )
            .await?;
        }
        tracing::info!(event="summary_candidates_materialized",paper_id,task_id=%lineage.task_id,run_id=%lineage.run_id,section_candidates=summary.section_summaries.len(),semantic_entities_ignored=summary.methods.len()+summary.datasets.len()+summary.metrics.len(),retrieval_eligible=false,authoritative=false,import_eligible=false,"Generative summary candidates materialized");
        Ok(())
    }
    async fn upsert_summary_node(
        &self,
        paper_id: &str,
        granularity: &str,
        text: &str,
        source_label: &str,
        source_id: &str,
        lineage: &PromptExecutionLineage,
    ) -> anyhow::Result<u64> {
        let vid = format!("vid:summary:{paper_id}:{granularity}");
        let node = match self
            .graph_store
            .find_node_by_string_property("Summary", "vid", &vid)
            .await
        {
            Some(id) => id,
            None => self.graph_store.create_node("Summary").await?,
        };
        for (key, value) in [
            ("vid", vid),
            ("text", text.into()),
            ("granularity", granularity.into()),
            ("source_label", source_label.into()),
            ("source_id", source_id.into()),
            ("method", "prompt_candidate".into()),
            ("prompt_task_id", lineage.task_id.clone()),
            ("prompt_run_id", lineage.run_id.clone()),
            (
                "prompt_task_sha256",
                lineage.task_spec_sha256.as_str().into(),
            ),
            (
                "prompt_sha256",
                lineage.rendered_prompt_sha256.as_str().into(),
            ),
            (
                "output_schema_sha256",
                lineage.output_schema_sha256.as_str().into(),
            ),
            (
                "route_policy_sha256",
                lineage.route_policy_sha256.as_str().into(),
            ),
            (
                "model_binding_sha256",
                lineage.model_binding_sha256.as_str().into(),
            ),
            ("model", lineage.model.clone()),
            ("input_sha256", lineage.input_sha256.as_str().into()),
            ("output_sha256", lineage.output_sha256.as_str().into()),
        ] {
            self.graph_store
                .set_node_property_string(node, key, value)
                .await?;
        }
        self.graph_store
            .set_node_property_int(node, "char_count", text.chars().count() as i64)
            .await?;
        self.graph_store
            .set_node_property_string(node, "generated_at", self.clock.now().to_rfc3339())
            .await?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "authoritative", false)
            .await?;
        self.graph_store
            .set_node_property_bool(node, "import_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 2)
            .await?;
        Ok(node)
    }
    async fn resolve_section_node(&self, paper_id: &str, path_id: &str) -> Option<u64> {
        let exact = format!("vid:section:{paper_id}:{path_id}");
        if let Some(node) = self
            .graph_store
            .find_node_by_string_property("Section", "vid", &exact)
            .await
        {
            return Some(node);
        }
        let legacy = format!("section:{paper_id}:{path_id}");
        self.graph_store
            .find_node_by_string_property("Section", "section_id", &legacy)
            .await
    }
    async fn link_once(&self, from: u64, to: u64, relation: &str) -> anyhow::Result<()> {
        if self
            .graph_store
            .get_edges_between(from, to, relation)
            .await
            .is_empty()
        {
            self.graph_store.create_edge(from, to, relation).await?;
        }
        Ok(())
    }
}
