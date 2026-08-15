//! Grounded extractive summary materialization (M371/S10).
//!
//! Always-run graph write of pure-domain extractive digests. No LLM. Creates
//! Summary nodes + hasSummary edges for abstract (Paper) and sections
//! (Section). D127 remains false.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use da_domain::extractive_summary::{
    EXTRACTIVE_SUMMARY_METHOD, ExtractiveSummaryBatch, ExtractiveSummaryError,
    ExtractiveSummaryUnit, compute_extractive_summaries,
};
use da_domain::relation::structure::{HAS_PART, HAS_SUMMARY};
use da_domain::section_summary_policy::SectionRolePolicy;
use da_domain::vid::{revision_summary_version_vid, revision_summary_vid};
use da_ports::DirectGraphStore;
use da_ports::clock::{Clock, SystemClock};
use kg_ontology::bitemporal::{
    PropertyMap, is_active_at, read_datetime, validate_bitemporal, was_known_at,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSource {
    pub path_id: String,
    pub source_vid: String,
    pub title: String,
    pub text: String,
    pub node_id: u64,
    /// PIR-007: typed reason when `path_id` was derived from an `order`/
    /// `source_vid` fallback instead of a real dotted AST path. Empty string
    /// means the real dotted path was present. Mirrored onto the Summary node
    /// as `source_path_missingness` by the writer (T02).
    #[serde(default)]
    pub path_id_missingness: String,
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
    pub summary_type_counts: std::collections::BTreeMap<String, usize>,
    pub unmatched_section_summaries: usize,
    pub unmatched_section_titles: std::collections::BTreeMap<String, usize>,
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

#[derive(Debug, Clone)]
pub struct SectionRolePolicyBinding {
    pub policy: SectionRolePolicy,
    pub configuration_sha256: String,
}

pub struct ExtractiveSummaryUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
    role_policy: Option<SectionRolePolicyBinding>,
    // ADR-051 Contour 1: injectable wall clock (tests freeze/advance time so
    // valid_from/valid_to/recorded_at/superseded_at are deterministic).
    clock: Arc<dyn Clock>,
}

impl ExtractiveSummaryUseCase {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self {
            graph_store,
            role_policy: None,
            clock: Arc::new(SystemClock),
        }
    }

    /// Builder for deterministic tests: freeze or advance time between
    /// materialize runs to pin version intervals exactly.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_role_policy(
        graph_store: Arc<dyn DirectGraphStore>,
        binding: SectionRolePolicyBinding,
    ) -> Result<Self, da_domain::section_summary_policy::SectionRolePolicyError> {
        binding.policy.validate()?;
        Ok(Self {
            graph_store,
            role_policy: Some(binding),
            clock: Arc::new(SystemClock),
        })
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
        // PIR-007: mirror the typed path_id fallback reason onto the Summary
        // node so S05 honesty can tell real dotted AST paths from invented
        // ordinals after materialization.
        let mut path_to_missingness = std::collections::BTreeMap::new();
        for section in &input.sections {
            path_to_node.insert(section.path_id.clone(), section.node_id);
            path_to_missingness
                .insert(section.path_id.clone(), section.path_id_missingness.clone());
        }

        let mut abstract_written = false;
        if let Some(unit) = &batch.abstract_summary {
            self.write_summary_node(&input.revision_vid, unit, input.revision_node_id, None, "")
                .await?;
            abstract_written = true;
        }

        let mut section_summaries_written = 0_usize;
        let mut summary_type_counts = std::collections::BTreeMap::new();
        let mut unmatched_section_summaries = 0_usize;
        let mut unmatched_section_titles = std::collections::BTreeMap::new();
        for unit in &batch.section_summaries {
            let section_node = *path_to_node.get(&unit.path_id).ok_or_else(|| {
                ExtractiveSummaryMaterializeError::MissingSectionNode(unit.path_id.clone())
            })?;
            let summary_type = self
                .role_policy
                .as_ref()
                .and_then(|binding| binding.policy.classify(&unit.title))
                .map(|matched| matched.summary_type);
            if let Some(summary_type) = &summary_type {
                *summary_type_counts
                    .entry(summary_type.as_str().to_string())
                    .or_insert(0) += 1;
            } else {
                unmatched_section_summaries += 1;
                *unmatched_section_titles
                    .entry(unit.title.clone())
                    .or_insert(0) += 1;
            }
            // Same keys as path_to_node, so the lookup cannot fail here.
            let source_path_missingness = path_to_missingness
                .get(&unit.path_id)
                .map(String::as_str)
                .unwrap_or("");
            self.write_summary_node(
                &input.revision_vid,
                unit,
                section_node,
                summary_type.as_ref().map(|value| value.as_str()),
                source_path_missingness,
            )
            .await?;
            section_summaries_written += 1;
        }

        Ok(ExtractiveSummaryMaterializeResult {
            paper_id: input.paper_id,
            method: EXTRACTIVE_SUMMARY_METHOD.into(),
            abstract_written,
            section_summaries_written,
            skipped_empty_sections: batch.skipped_empty_sections,
            summary_type_counts,
            unmatched_section_summaries,
            unmatched_section_titles,
            batch,
        })
    }

    async fn write_summary_node(
        &self,
        revision_vid: &str,
        unit: &ExtractiveSummaryUnit,
        link_source: u64,
        summary_type: Option<&str>,
        source_path_missingness: &str,
    ) -> Result<u64, ExtractiveSummaryMaterializeError> {
        let logical_vid = revision_summary_vid(revision_vid, &unit.granularity, &unit.source_id)
            .map_err(ExtractiveSummaryMaterializeError::InvalidIdentity)?;
        let (policy_version, policy_sha256) = self
            .role_policy
            .as_ref()
            .map(|binding| {
                (
                    binding.policy.schema_version.clone(),
                    binding.configuration_sha256.clone(),
                )
            })
            .unwrap_or_else(|| ("none".into(), String::new()));
        // D243: version identity is content+policy addressed. Identical digest
        // pair replays onto the same node; any change mints a fresh version.
        let version_vid =
            revision_summary_version_vid(&logical_vid, &unit.source_text_sha256, &policy_sha256)
                .map_err(ExtractiveSummaryMaterializeError::InvalidIdentity)?;
        let now = self.clock.now().to_rfc3339();

        // PIR-012 replay gate: the exact version already exists. A current
        // replay is a no-op (history is already correct, one current node); a
        // superseded replay reactivates this version and closes whichever
        // sibling is currently current, so "one current node" always holds.
        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property("Summary", "vid", &version_vid)
            .await
        {
            let superseded_at = self
                .graph_store
                .get_node_property_string(existing, "superseded_at")
                .await;
            if superseded_at.as_deref().is_none_or(|s| s.is_empty()) {
                return Ok(existing);
            }
            self.close_current_summary(&logical_vid, &now, &version_vid)
                .await?;
            self.activate_summary(existing, &now).await?;
            return Ok(existing);
        }

        // New version: close the previous current node of the same logical
        // group so the prior digest stays queryable, then write the new node
        // (vid = content-addressed version vid, logical_vid = stable join key).
        self.close_current_summary(&logical_vid, &now, &version_vid)
            .await?;
        let node = self
            .graph_store
            .create_node("Summary")
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;

        self.graph_store
            .set_node_property_string(node, "vid", version_vid)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "logical_vid", logical_vid)
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
        // PIR-007: durable dotted AST path key (e.g. "3.2.1") plus the typed
        // missingness reason when the path was derived from an order/source_vid
        // fallback. Empty missingness = real dotted path present.
        self.graph_store
            .set_node_property_string(node, "source_path_id", unit.path_id.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(
                node,
                "source_path_missingness",
                source_path_missingness.to_string(),
            )
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "revision_vid", revision_vid.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(
                node,
                "summary_type",
                summary_type.unwrap_or("generic").to_string(),
            )
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "role_policy_schema_version", policy_version)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "role_policy_sha256", policy_sha256)
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
            .set_node_property_bool(node, "import_eligible", false) // D127
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_int(node, "schema_version", 1)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        // PIR-012 interval: this version is valid/recorded from `now`; the
        // previously current node was closed at the same instant, so
        // old.valid_to == new.valid_from (adjacent, non-overlapping intervals).
        // RFC3339 strings are an already-accepted DateTime encoding (the LLM
        // summary writer uses the same for `generated_at`); the validator is
        // not tightened (S04 owns the canonical wire). Empty string clears a
        // temporal bound when a version is reactivated (no property-delete API
        // on the store trait).
        self.graph_store
            .set_node_property_string(node, "valid_from", now.clone())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "recorded_at", now)
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

    /// Deterministically select the current (not superseded) Summary node of a
    /// logical group, or `None` when the group has no node yet.
    ///
    /// A node belongs to the group when its `logical_vid` property matches, or
    /// — legacy nodes written before T03 versioning — when it has no
    /// `logical_vid` but its `vid` equals the logical VID. Among current
    /// members the newest `valid_from` wins; `node_id` breaks ties (Q3
    /// advisory: with multiple nodes sharing a logical_vid, resolution must be
    /// deterministic).
    async fn find_current_summary(
        &self,
        logical_vid: &str,
    ) -> Result<Option<u64>, ExtractiveSummaryMaterializeError> {
        // valid_from is an RFC3339 string; lexicographic comparison is
        // chronological for identical UTC formats.
        let mut current: Option<(String, u64)> = None; // (valid_from, node_id)
        for node in summary_group_members(&*self.graph_store, logical_vid).await {
            let superseded_at = self
                .graph_store
                .get_node_property_string(node, "superseded_at")
                .await;
            if superseded_at.as_deref().is_some_and(|s| !s.is_empty()) {
                continue;
            }
            let valid_from = self
                .graph_store
                .get_node_property_string(node, "valid_from")
                .await
                .unwrap_or_default();
            let better = match &current {
                None => true,
                Some((best_from, best_id)) => {
                    valid_from > *best_from || (valid_from == *best_from && node > *best_id)
                }
            };
            if better {
                current = Some((valid_from, node));
            }
        }
        Ok(current.map(|(_, node)| node))
    }

    /// Close the current member of a logical group (if any): it is no longer
    /// the current version (PIR-012). `valid_to`/`superseded_at` are set to
    /// `now`, `superseded_by` points at the replacement version VID, and the
    /// node flips out of the retrieval surface (`retrieval_eligible=false`).
    async fn close_current_summary(
        &self,
        logical_vid: &str,
        now: &str,
        superseded_by: &str,
    ) -> Result<(), ExtractiveSummaryMaterializeError> {
        let Some(current) = self.find_current_summary(logical_vid).await? else {
            return Ok(());
        };
        self.close_summary(current, now, superseded_by).await
    }

    async fn close_summary(
        &self,
        node: u64,
        now: &str,
        superseded_by: &str,
    ) -> Result<(), ExtractiveSummaryMaterializeError> {
        self.graph_store
            .set_node_property_string(node, "valid_to", now.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "superseded_at", now.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "superseded_by", superseded_by.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        Ok(())
    }

    /// Re-activate a previously superseded version as the current one (PIR-012
    /// replay convergence): clear the close markers, open a fresh valid
    /// interval at `now`, and re-enable retrieval. `recorded_at` moves to
    /// `now` so the transaction-time axis stays monotonic per activation.
    async fn activate_summary(
        &self,
        node: u64,
        now: &str,
    ) -> Result<(), ExtractiveSummaryMaterializeError> {
        self.graph_store
            .set_node_property_string(node, "valid_to", String::new())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "superseded_at", String::new())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "superseded_by", String::new())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_bool(node, "retrieval_eligible", true)
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "valid_from", now.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        self.graph_store
            .set_node_property_string(node, "recorded_at", now.to_string())
            .await
            .map_err(|e| ExtractiveSummaryMaterializeError::Graph(e.to_string()))?;
        Ok(())
    }
}

/// Typed skip when a PaperRevision cannot be summarized without inventing identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSummaryWalkSkip {
    pub node_id: u64,
    pub reason: String,
}

/// Typed fallback diagnostic when a Section lacks a non-empty dotted `path_id`
/// and the walker must derive identity from `order` or the section VID
/// (PIR-007). Lets downstream joins distinguish real AST paths from invented
/// ordinals instead of silently joining on the wrong key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathIdFallback {
    pub section_node: u64,
    pub source_vid: String,
    pub path_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSummaryWalk {
    pub inputs: Vec<ExtractiveSummaryMaterializeInput>,
    pub skipped: Vec<RevisionSummaryWalkSkip>,
    pub path_id_fallbacks: Vec<PathIdFallback>,
}

/// Walk PaperRevision nodes already in the graph. Does not invent revision=1.
pub async fn collect_revision_summary_inputs(
    store: &dyn DirectGraphStore,
    max_chars: usize,
) -> RevisionSummaryWalk {
    let mut inputs = Vec::new();
    let mut skipped = Vec::new();
    let mut path_id_fallbacks = Vec::new();
    for revision_node in store.get_nodes_by_label("PaperRevision").await {
        let Some(revision_vid) = store.get_node_property_string(revision_node, "vid").await else {
            skipped.push(RevisionSummaryWalkSkip {
                node_id: revision_node,
                reason: "missing_vid".into(),
            });
            continue;
        };
        let Some(paper_id) = store
            .get_node_property_string(revision_node, "canonical_id")
            .await
        else {
            skipped.push(RevisionSummaryWalkSkip {
                node_id: revision_node,
                reason: "missing_canonical_id".into(),
            });
            continue;
        };
        let abstract_text = store
            .get_node_property_string(revision_node, "abstract_text")
            .await
            .unwrap_or_default();
        let mut sections = Vec::new();
        for (section_node, edge_type) in store.get_outgoing_edges(revision_node).await {
            if edge_type != HAS_PART {
                continue;
            }
            let Some(source_vid) = store.get_node_property_string(section_node, "vid").await else {
                continue;
            };
            // PIR-007: prefer the real dotted AST path; `order` and the section
            // VID are typed fallbacks, never silent identity invention.
            let (path_id, path_id_missingness) = match store
                .get_node_property_string(section_node, "path_id")
                .await
            {
                Some(path_id) if !path_id.trim().is_empty() => (path_id, String::new()),
                _ => {
                    let order = store.get_node_property_int(section_node, "order").await;
                    let (fallback, reason) = match order {
                        Some(ordinal) => (
                            ordinal.to_string(),
                            "path_id_missing_fallback_to_order".to_string(),
                        ),
                        None => (
                            source_vid.clone(),
                            "path_id_missing_fallback_to_source_vid".to_string(),
                        ),
                    };
                    path_id_fallbacks.push(PathIdFallback {
                        section_node,
                        source_vid: source_vid.clone(),
                        path_id: fallback.clone(),
                        reason: reason.clone(),
                    });
                    (fallback, reason)
                }
            };
            sections.push(SectionSource {
                path_id,
                source_vid,
                title: store
                    .get_node_property_string(section_node, "title")
                    .await
                    .unwrap_or_default(),
                text: store
                    .get_node_property_string(section_node, "text")
                    .await
                    .unwrap_or_default(),
                node_id: section_node,
                path_id_missingness,
            });
        }
        sections.sort_by(|left, right| left.path_id.cmp(&right.path_id));
        inputs.push(ExtractiveSummaryMaterializeInput {
            paper_id,
            revision_node_id: revision_node,
            revision_vid,
            abstract_text,
            sections,
            max_chars,
        });
    }
    inputs.sort_by(|a, b| a.revision_vid.cmp(&b.revision_vid));
    RevisionSummaryWalk {
        inputs,
        skipped,
        path_id_fallbacks,
    }
}

/// One Summary version read back from the graph through the as-of surface.
///
/// Temporal bounds are parsed RFC3339 instants (`None` = OPEN / never set), so
/// callers compare intervals without re-parsing. `bitemporal_issues` carries
/// `validate_bitemporal` findings for the version (empty = consistent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryAsOf {
    pub node_id: u64,
    /// Content+policy addressed version VID (D243), e.g.
    /// `revision_summary_version_vid(logical_vid, text_sha256, policy_sha256)`.
    pub vid: String,
    /// Stable logical join key of the version group (unchanged across reruns).
    pub logical_vid: String,
    pub text: String,
    pub granularity: String,
    pub source_label: String,
    pub source_id: String,
    /// PIR-007: durable dotted AST path key (e.g. "3.2.1").
    pub source_path_id: String,
    /// Empty = real dotted path present; non-empty = typed fallback reason.
    pub source_path_missingness: String,
    pub revision_vid: String,
    pub summary_type: String,
    pub role_policy_schema_version: String,
    pub role_policy_sha256: String,
    pub method: String,
    pub source_text_sha256: String,
    pub char_count: i64,
    pub source_chars: i64,
    /// False once superseded: off the current retrieval surface, still readable
    /// through `get_summary_as_of`/`list_summary_versions` (PIR-012).
    pub retrieval_eligible: bool,
    /// Valid-time axis: when the fact became true (OPEN = `None`).
    pub valid_from: Option<DateTime<Utc>>,
    /// Valid-time axis: when the fact stopped being true (`None` = still true).
    pub valid_to: Option<DateTime<Utc>>,
    /// Transaction-time axis: when our system first wrote the fact.
    pub recorded_at: Option<DateTime<Utc>>,
    /// Transaction-time axis: when our system stopped using it (`None` = current).
    pub superseded_at: Option<DateTime<Utc>>,
    /// Version VID of the node that superseded this one (`None` = never).
    pub superseded_by: Option<String>,
    /// `kg_ontology::bitemporal::validate_bitemporal` findings (empty = consistent).
    pub bitemporal_issues: Vec<String>,
}

/// All Summary nodes of one logical group (D243): nodes whose `logical_vid`
/// matches, plus legacy pre-T03 nodes that carry the logical VID directly in
/// `vid` without a `logical_vid` property.
async fn summary_group_members(store: &dyn DirectGraphStore, logical_vid: &str) -> Vec<u64> {
    let mut members = Vec::new();
    for node in store.get_nodes_by_label("Summary").await {
        let logical = store.get_node_property_string(node, "logical_vid").await;
        let vid = store.get_node_property_string(node, "vid").await;
        if logical.as_deref() == Some(logical_vid)
            || (logical.is_none() && vid.as_deref() == Some(logical_vid))
        {
            members.push(node);
        }
    }
    members
}

/// Snapshot one Summary node into the universal PropertyMap shape so the
/// kg-ontology bitemporal helpers (`is_active_at`/`was_known_at`/
/// `validate_bitemporal`) evaluate its temporal state without a project
/// reader. Missing/empty temporal fields stay absent — the helpers treat
/// absent bounds as OPEN.
async fn summary_property_map(store: &dyn DirectGraphStore, node: u64) -> PropertyMap {
    let mut props = PropertyMap::new();
    for key in [
        "vid",
        "logical_vid",
        "text",
        "granularity",
        "source_label",
        "source_id",
        "source_path_id",
        "source_path_missingness",
        "revision_vid",
        "summary_type",
        "role_policy_schema_version",
        "role_policy_sha256",
        "method",
        "source_text_sha256",
        "valid_from",
        "valid_to",
        "recorded_at",
        "superseded_at",
        "superseded_by",
    ] {
        if let Some(value) = store.get_node_property_string(node, key).await {
            props.insert(key.to_string(), serde_json::Value::String(value));
        }
    }
    for key in ["char_count", "source_chars"] {
        if let Some(value) = store.get_node_property_int(node, key).await {
            props.insert(key.to_string(), serde_json::Value::Number(value.into()));
        }
    }
    if let Some(value) = store
        .get_node_property_bool(node, "retrieval_eligible")
        .await
    {
        props.insert(
            "retrieval_eligible".to_string(),
            serde_json::Value::Bool(value),
        );
    }
    props
}

/// Read one Summary version as a typed snapshot with parsed temporal bounds
/// and bitemporal consistency findings.
async fn summary_snapshot(store: &dyn DirectGraphStore, node: u64) -> SummaryAsOf {
    let props = summary_property_map(store, node).await;
    SummaryAsOf {
        node_id: node,
        vid: store
            .get_node_property_string(node, "vid")
            .await
            .unwrap_or_default(),
        logical_vid: store
            .get_node_property_string(node, "logical_vid")
            .await
            .unwrap_or_default(),
        text: store
            .get_node_property_string(node, "text")
            .await
            .unwrap_or_default(),
        granularity: store
            .get_node_property_string(node, "granularity")
            .await
            .unwrap_or_default(),
        source_label: store
            .get_node_property_string(node, "source_label")
            .await
            .unwrap_or_default(),
        source_id: store
            .get_node_property_string(node, "source_id")
            .await
            .unwrap_or_default(),
        source_path_id: store
            .get_node_property_string(node, "source_path_id")
            .await
            .unwrap_or_default(),
        source_path_missingness: store
            .get_node_property_string(node, "source_path_missingness")
            .await
            .unwrap_or_default(),
        revision_vid: store
            .get_node_property_string(node, "revision_vid")
            .await
            .unwrap_or_default(),
        summary_type: store
            .get_node_property_string(node, "summary_type")
            .await
            .unwrap_or_default(),
        role_policy_schema_version: store
            .get_node_property_string(node, "role_policy_schema_version")
            .await
            .unwrap_or_default(),
        role_policy_sha256: store
            .get_node_property_string(node, "role_policy_sha256")
            .await
            .unwrap_or_default(),
        method: store
            .get_node_property_string(node, "method")
            .await
            .unwrap_or_default(),
        source_text_sha256: store
            .get_node_property_string(node, "source_text_sha256")
            .await
            .unwrap_or_default(),
        char_count: store
            .get_node_property_int(node, "char_count")
            .await
            .unwrap_or(0),
        source_chars: store
            .get_node_property_int(node, "source_chars")
            .await
            .unwrap_or(0),
        retrieval_eligible: store
            .get_node_property_bool(node, "retrieval_eligible")
            .await
            .unwrap_or(false),
        valid_from: read_datetime(&props, "valid_from"),
        valid_to: read_datetime(&props, "valid_to"),
        recorded_at: read_datetime(&props, "recorded_at"),
        superseded_at: read_datetime(&props, "superseded_at"),
        superseded_by: store
            .get_node_property_string(node, "superseded_by")
            .await
            .filter(|s| !s.is_empty()),
        bitemporal_issues: validate_bitemporal(&props),
    }
}

/// Same-process as-of read surface (PIR-012): the Summary version of a logical
/// group that was valid AND known at instant `at`, or `None` when no version
/// covers that instant (including before the first write). This is the read
/// surface for both the current version (`at = now`) and historical versions
/// (`at = any past instant`) — old summaries stay queryable after regeneration
/// without unlocking D127.
///
/// Both temporal axes are evaluated through the universal kg-ontology helpers:
/// `is_active_at` (valid_from <= at < valid_to) and `was_known_at`
/// (recorded_at <= at < superseded_at). Among candidates the newest
/// `valid_from` wins with `node_id` breaking ties, matching
/// `find_current_summary` determinism (Q3).
pub async fn get_summary_as_of(
    store: &dyn DirectGraphStore,
    logical_vid: &str,
    at: DateTime<Utc>,
) -> Option<SummaryAsOf> {
    let mut best: Option<(DateTime<Utc>, u64)> = None;
    for node in summary_group_members(store, logical_vid).await {
        let props = summary_property_map(store, node).await;
        if !is_active_at(&props, at) || !was_known_at(&props, at) {
            continue;
        }
        // Legacy nodes (pre-T03, no temporal fields) are active+known at every
        // instant; the epoch valid_from keeps their tie-break deterministic.
        let valid_from = read_datetime(&props, "valid_from")
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).expect("epoch is a valid instant"));
        let better = match &best {
            None => true,
            Some((best_from, best_id)) => {
                valid_from > *best_from || (valid_from == *best_from && node > *best_id)
            }
        };
        if better {
            best = Some((valid_from, node));
        }
    }
    match best {
        Some((_, node)) => Some(summary_snapshot(store, node).await),
        None => None,
    }
}

/// Full version history of one logical group, oldest first (valid_from
/// ascending, `node_id` tie-break). Superseded versions remain readable here
/// even though `retrieval_eligible=false` keeps them off the current retrieval
/// surface — the immutable audit trail PIR-012 requires.
pub async fn list_summary_versions(
    store: &dyn DirectGraphStore,
    logical_vid: &str,
) -> Vec<SummaryAsOf> {
    let mut members: Vec<(String, u64)> = Vec::new();
    for node in summary_group_members(store, logical_vid).await {
        let valid_from = store
            .get_node_property_string(node, "valid_from")
            .await
            .unwrap_or_default();
        members.push((valid_from, node));
    }
    members.sort();
    let mut versions = Vec::with_capacity(members.len());
    for (_, node) in members {
        versions.push(summary_snapshot(store, node).await);
    }
    versions
}
