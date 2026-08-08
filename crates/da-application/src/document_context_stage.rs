//! Cacheable document-context projection from explicit DAG dependencies.

use async_trait::async_trait;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionSummaryInput {
    pub path_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentContextSection {
    pub path_id: String,
    pub title: String,
    pub parent_section_id: Option<String>,
    pub previous_path_id: Option<String>,
    pub next_path_id: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentContextProjection {
    pub article_id: String,
    pub sections: Vec<DocumentContextSection>,
}

pub struct DocumentContextStage {
    parse_dependency: String,
    summaries_dependency: String,
}

impl DocumentContextStage {
    pub fn new() -> Self {
        Self {
            parse_dependency: "parse".into(),
            summaries_dependency: "section_summaries".into(),
        }
    }

    pub fn with_dependencies(
        mut self,
        parse: impl Into<String>,
        summaries: impl Into<String>,
    ) -> Self {
        self.parse_dependency = parse.into();
        self.summaries_dependency = summaries.into();
        self
    }
}

impl Default for DocumentContextStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PipelineStageExecutor for DocumentContextStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "document_context".into(),
            implementation_fingerprint: "DocumentContextProjection@path-id-v1".into(),
            configuration_fingerprint: "navigation=source-order;summary_join=path_id".into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: "DocumentContextProjection@serde-v1".into(),
        }
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let article: ParsedArticle =
            decode_dependency(&request, &self.parse_dependency, "parsed_article")?;
        if article.paper_id != request.article_id {
            return Err(terminal(
                "article_identity_mismatch",
                format!(
                    "request={} payload={}",
                    request.article_id, article.paper_id
                ),
            ));
        }
        let summaries: Vec<SectionSummaryInput> =
            decode_dependency(&request, &self.summaries_dependency, "section_summaries")?;
        let mut summary_by_path = HashMap::new();
        for summary in summaries {
            if summary.path_id.trim().is_empty() || summary.summary.trim().is_empty() {
                return Err(terminal(
                    "invalid_section_summary",
                    "path_id and summary must be non-empty",
                ));
            }
            if summary_by_path
                .insert(summary.path_id.clone(), summary.summary)
                .is_some()
            {
                return Err(terminal("duplicate_section_summary_path", summary.path_id));
            }
        }
        let mut paths = HashSet::new();
        let mut sections = Vec::with_capacity(article.sections.len());
        for (index, section) in article.sections.iter().enumerate() {
            let path_id = section.path_id.clone().ok_or_else(|| {
                terminal("missing_section_path_id", format!("section index {index}"))
            })?;
            if !paths.insert(path_id.clone()) {
                return Err(terminal("duplicate_section_path_id", path_id));
            }
            sections.push(DocumentContextSection {
                path_id,
                title: section.title.clone(),
                parent_section_id: section.parent_section_id.clone(),
                previous_path_id: None,
                next_path_id: None,
                summary: None,
            });
        }
        for path_id in summary_by_path.keys() {
            if !paths.contains(path_id) {
                return Err(terminal("unresolved_section_summary_path", path_id.clone()));
            }
        }
        for index in 0..sections.len() {
            sections[index].previous_path_id = index
                .checked_sub(1)
                .map(|prior| sections[prior].path_id.clone());
            sections[index].next_path_id = sections.get(index + 1).map(|next| next.path_id.clone());
            sections[index].summary = summary_by_path.remove(&sections[index].path_id);
        }
        let summary_count = sections
            .iter()
            .filter(|section| section.summary.is_some())
            .count();
        let projection = DocumentContextProjection {
            article_id: request.article_id,
            sections,
        };
        let payload = serde_json::to_vec(&projection)
            .map_err(|error| terminal("document_context_serialization", error.to_string()))?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                (
                    "section_count".into(),
                    projection.sections.len().to_string(),
                ),
                ("summary_count".into(), summary_count.to_string()),
                ("path_identity".into(), "unique".into()),
            ]),
            payload,
        })
    }
}

fn decode_dependency<T: for<'de> Deserialize<'de>>(
    request: &StageExecutionRequest,
    dependency: &str,
    class: &str,
) -> Result<T, StageExecutionError> {
    let payload = request.dependency_payloads.get(dependency).ok_or_else(|| {
        terminal(
            "missing_document_context_dependency",
            dependency.to_string(),
        )
    })?;
    serde_json::from_slice(payload)
        .map_err(|error| terminal(&format!("invalid_{class}_payload"), error.to_string()))
}

fn terminal(error_class: &str, message: impl Into<String>) -> StageExecutionError {
    StageExecutionError::Terminal {
        error_class: error_class.into(),
        message: message.into(),
    }
}
