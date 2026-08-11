//! Grounded extractive summary materialization (M371/S10).
//!
//! Always-run graph write of pure-domain extractive digests. No LLM. Creates
//! Summary nodes + hasSummary edges for abstract (Paper) and sections
//! (Section). D127 remains false.

use std::sync::Arc;

use da_domain::extractive_summary::{
    EXTRACTIVE_SUMMARY_METHOD, ExtractiveSummaryBatch, ExtractiveSummaryError,
    ExtractiveSummaryUnit, compute_extractive_summaries,
};
use da_domain::relation::structure::HAS_SUMMARY;
use da_domain::vid::revision_summary_vid;
use da_ports::DirectGraphStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSource {
    pub path_id: String,
    pub source_vid: String,
    pub title: String,
    pub text: String,
    pub node_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractiveSummaryMaterializeInput {
    pub paper_id: String,
    pub revision_node_id: u64,
    pub revision_vid: String,
    pub abstract_text: String,
    pub sections: Vec<SectionSource>,
    pub max_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractiveSummaryMaterializeResult {
    pub paper_id: String,
    pub method: String,
    pub abstract_written: bool,
    pub section_summaries_written: usize,
    pub skipped_empty_sections: usize,
    pub batch: ExtractiveSummaryBatch,
}

#[derive(Debug, Error)]
pub enum ExtractiveSummaryMaterializeError {
    #[error(transparent)]
    Domain(#[from] ExtractiveSummaryError),
    #[error("graph: {0}")]
    Graph(String),
    #[error("PaperRevision node missing or mismatched for VID {0}")]
    MissingRevisionNode(String),
    #[error("Section node missing or mismatched for VID {0}")]
    MissingSectionNode(String),
    #[error("invalid summary identity: {0}")]
    InvalidIdentity(String),
}

pub struct ExtractiveSummaryUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
}

impl ExtractiveSummaryUseCase {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Compute pure extractive digests and materialize them into Summary nodes.
    pub async fn materialize(
        &self,
        input: ExtractiveSummaryMaterializeInput,
    ) -> Result<ExtractiveSummaryMaterializeResult, ExtractiveSummaryMaterializeError> {
        let revision_node = self
            .graph_store
            .find_node_by_string_property("PaperRevision", "vid", &input.revision_vid)
            .await;
        if revision_node != Some(input.revision_node_id) {
            return Err(ExtractiveSummaryMaterializeError::MissingRevisionNode(
                input.revision_vid.clone(),
            ));
        }
        for section in &input.sections {
            let section_node = self
                .graph_store
                .find_node_by_string_property("Section", "vid", &section.source_vid)
                .await;
            if section_node != Some(section.node_id) {
                return Err(ExtractiveSummaryMaterializeError::MissingSectionNode(
                    section.source_vid.clone(),
                ));
            }
        }

        let section_tuples = input
            .sections
            .iter()
            .map(|s| {
                (
                    s.path_id.clone(),
                    s.source_vid.clone(),
                    s.title.clone(),
                    s.text.clone(),
                )
            })
            .collect::<Vec<_>>();
        let batch = compute_extractive_summaries(
            &input.paper_id,
            &input.revision_vid,
            &input.abstract_text,
            &section_tuples,
            input.max_chars,
        )?;

        // Map path_id -> node_id from input (caller-owned graph inventory).
        let mut path_to_node = std::collections::BTreeMap::new();
        for section in &input.sections {
            path_to_node.insert(section.path_id.clone(), section.node_id);
        }

        let mut abstract_written = false;
        if let Some(unit) = &batch.abstract_summary {
            self.write_summary_node(&input.revision_vid, unit, input.revision_node_id)
                .await?;
            abstract_written = true;
        }

        let mut section_summaries_written = 0_usize;
        for unit in &batch.section_summaries {
            let section_node = *path_to_node.get(&unit.path_id).ok_or_else(|| {
                ExtractiveSummaryMaterializeError::MissingSectionNode(unit.path_id.clone())
            })?;
            self.write_summary_node(&input.revision_vid, unit, section_node)
                .await?;
            section_summaries_written += 1;
        }

        Ok(ExtractiveSummaryMaterializeResult {
            paper_id: input.paper_id,
            method: EXTRACTIVE_SUMMARY_METHOD.into(),
            abstract_written,
            section_summaries_written,
            skipped_empty_sections: batch.skipped_empty_sections,
            batch,
        })
    }

    async fn write_summary_node(
        &self,
        revision_vid: &str,
        unit: &ExtractiveSummaryUnit,
        link_source: u64,
    ) -> Result<u64, ExtractiveSummaryMaterializeError> {
        let summary_vid = revision_summary_vid(revision_vid, &unit.granularity, &unit.source_id)
            .map_err(ExtractiveSummaryMaterializeError::InvalidIdentity)?;
        let node = match self
            .graph_store
            .find_node_by_string_property("Summary", "vid", &summary_vid)
            .await
        {
            Some(existing) => existing,
            None => self
                .graph_store
                .create_node("Summary")
                .await
                .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?,
        };

        self.graph_store
            .set_node_property_string(node, "vid", summary_vid)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "text", unit.text.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "granularity", unit.granularity.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "source_label", unit.source_label.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "source_id", unit.source_id.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "revision_vid", revision_vid.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "method", unit.method.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "source_text_sha256", unit.source_text_sha256.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_int(node, "char_count", unit.char_count as i64)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_int(node, "source_chars", unit.source_chars as i64)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_bool(node, "import_eligible", false)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 1)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;

        if self
            .graph_store
            .get_edges_between(link_source, node, HAS_SUMMARY)
            .await
            .is_empty()
        {
            self.graph_store
                .create_edge(link_source, node, HAS_SUMMARY)
                .await
                .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        }
        Ok(node)
    }
}
