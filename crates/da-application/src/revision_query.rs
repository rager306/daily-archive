//! Read-only article revision queries (ADR-058, M376).
//!
//! Selection semantics live in the application layer. CLI and storage remain
//! provider-neutral composition/observation surfaces.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use da_domain::paper_vid;
use da_ports::graph_store::DirectGraphStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "revision", rename_all = "snake_case")]
pub enum RevisionSelector {
    Current,
    Exact(u32),
    History,
}

impl FromStr for RevisionSelector {
    type Err = RevisionQueryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "current" => Ok(Self::Current),
            "history" => Ok(Self::History),
            value if value.starts_with('v') => {
                let revision = value[1..].parse::<u32>().map_err(|_| {
                    RevisionQueryError::InvalidSelector(format!(
                        "expected current, history, or vN; got {value:?}"
                    ))
                })?;
                Self::exact(revision)
            }
            value => Err(RevisionQueryError::InvalidSelector(format!(
                "expected current, history, or vN; got {value:?}"
            ))),
        }
    }
}

impl RevisionSelector {
    pub fn exact(revision: u32) -> Result<Self, RevisionQueryError> {
        if revision == 0 {
            return Err(RevisionQueryError::InvalidSelector(
                "revision must be greater than zero".into(),
            ));
        }
        Ok(Self::Exact(revision))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionManifestationView {
    pub vid: String,
    pub format: String,
    pub sha256: String,
    pub bytes: i64,
    pub uri: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSummaryView {
    pub vid: String,
    pub source_vid: String,
    pub summary_type: String,
    pub text: String,
    pub source_text_sha256: String,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessView {
    pub vid: String,
    pub state: String,
    pub blocker_codes: Vec<String>,
    pub diagnostic_codes: Vec<String>,
    pub evidence_sha256: String,
    pub policy_schema_version: String,
    pub policy_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleRevisionView {
    pub revision: u32,
    pub revision_vid: String,
    pub source_code: String,
    pub source_key: String,
    pub is_current: bool,
    pub predecessor_revision: Option<u32>,
    pub section_count: usize,
    /// PIR-011 / D246 (T02): VIDs of the Reference nodes this exact revision
    /// owns through additive PaperRevision HAS_PART edges. The nodes stay
    /// globally deduped (MEM471) — ownership is expressed per revision via
    /// edges, never by cloning. Sorted for deterministic serialization.
    pub reference_vids: Vec<String>,
    /// PIR-011 / D246 (T02): arxiv_ids of the Citation nodes this exact
    /// revision owns through additive PaperRevision CITES edges. Sorted for
    /// deterministic serialization.
    pub citation_arxiv_ids: Vec<String>,
    pub summaries: Vec<RevisionSummaryView>,
    pub readiness: Option<RevisionReadinessView>,
    pub manifestations: Vec<RevisionManifestationView>,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionQueryResponse {
    pub schema_version: String,
    pub canonical_id: String,
    pub paper_vid: String,
    pub selector: RevisionSelector,
    pub revisions: Vec<ArticleRevisionView>,
    pub import_eligible: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RevisionQueryError {
    #[error("invalid revision selector: {0}")]
    InvalidSelector(String),
    #[error("canonical Paper not found for {canonical_id}")]
    PaperNotFound { canonical_id: String },
    #[error("no article revisions found for {canonical_id}")]
    NoRevisions { canonical_id: String },
    #[error("revision v{revision} not found for {canonical_id}")]
    RevisionNotFound { canonical_id: String, revision: u32 },
    #[error("no current revision marked for {canonical_id}")]
    MissingCurrent { canonical_id: String },
    #[error("multiple current revisions marked for {canonical_id}: {revisions:?}")]
    AmbiguousCurrent {
        canonical_id: String,
        revisions: Vec<u32>,
    },
    #[error("revision graph data is invalid: {0}")]
    InvalidGraph(String),
}

pub struct RevisionQueryUseCase {
    graph: Arc<dyn DirectGraphStore>,
}

impl RevisionQueryUseCase {
    pub fn new(graph: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph }
    }

    pub async fn query(
        &self,
        canonical_id: &str,
        selector: RevisionSelector,
    ) -> Result<RevisionQueryResponse, RevisionQueryError> {
        let canonical_id = canonical_id.trim();
        if canonical_id.is_empty() {
            return Err(RevisionQueryError::InvalidSelector(
                "canonical article id must not be empty".into(),
            ));
        }
        if matches!(selector, RevisionSelector::Exact(0)) {
            return Err(RevisionQueryError::InvalidSelector(
                "revision must be greater than zero".into(),
            ));
        }
        let paper = self
            .graph
            .find_node_by_string_property("Paper", "arxiv_id", canonical_id)
            .await
            .ok_or_else(|| RevisionQueryError::PaperNotFound {
                canonical_id: canonical_id.into(),
            })?;
        let paper_vid_value = self
            .graph
            .get_node_property_string(paper, "vid")
            .await
            .unwrap_or_else(|| paper_vid(canonical_id));

        let mut nodes = Vec::new();
        for node in self.graph.get_nodes_by_label("PaperRevision").await {
            if self
                .graph
                .get_node_property_string(node, "paper_vid")
                .await
                .as_deref()
                != Some(paper_vid_value.as_str())
            {
                continue;
            }
            let revision = self
                .graph
                .get_node_property_int(node, "revision")
                .await
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    RevisionQueryError::InvalidGraph(format!(
                        "PaperRevision node {node} lacks positive revision"
                    ))
                })?;
            nodes.push((revision, node));
        }
        nodes.sort_by_key(|(revision, _)| *revision);
        if nodes.is_empty() {
            return Err(RevisionQueryError::NoRevisions {
                canonical_id: canonical_id.into(),
            });
        }

        let selected = match selector {
            RevisionSelector::History => nodes,
            RevisionSelector::Exact(revision) => vec![
                nodes
                    .into_iter()
                    .find(|(value, _)| *value == revision)
                    .ok_or_else(|| RevisionQueryError::RevisionNotFound {
                        canonical_id: canonical_id.into(),
                        revision,
                    })?,
            ],
            RevisionSelector::Current => {
                let mut current = Vec::new();
                for candidate @ (_, node) in nodes {
                    if self.graph.get_node_property_bool(node, "is_current").await == Some(true) {
                        current.push(candidate);
                    }
                }
                match current.as_slice() {
                    [] => {
                        return Err(RevisionQueryError::MissingCurrent {
                            canonical_id: canonical_id.into(),
                        });
                    }
                    [only] => vec![*only],
                    _ => {
                        return Err(RevisionQueryError::AmbiguousCurrent {
                            canonical_id: canonical_id.into(),
                            revisions: current.iter().map(|(revision, _)| *revision).collect(),
                        });
                    }
                }
            }
        };

        let mut revisions = Vec::with_capacity(selected.len());
        for (revision, node) in selected {
            revisions.push(self.view_revision(node, revision).await?);
        }
        Ok(RevisionQueryResponse {
            schema_version: "revision-query.v1".into(),
            canonical_id: canonical_id.into(),
            paper_vid: paper_vid_value,
            selector,
            revisions,
            import_eligible: false,
        })
    }

    async fn view_revision(
        &self,
        node: u64,
        revision: u32,
    ) -> Result<ArticleRevisionView, RevisionQueryError> {
        let revision_vid = self.required_string(node, "vid").await?;
        let source_code = self.required_string(node, "source_code").await?;
        let source_key = self.required_string(node, "source_key").await?;
        let is_current = self
            .graph
            .get_node_property_bool(node, "is_current")
            .await
            .unwrap_or(false);
        let import_eligible = self
            .graph
            .get_node_property_bool(node, "import_eligible")
            .await
            .unwrap_or(false);
        if import_eligible {
            return Err(RevisionQueryError::InvalidGraph(format!(
                "PaperRevision v{revision} violates D127 import lock"
            )));
        }

        // D248 / MEM797: PaperRevision hasPart now also targets bibliography
        // Reference nodes (PIR-011 additive revision-owned edges, T02).
        // section_count must count only Section-labeled parts — bibliography
        // nodes must never masquerade as sections (D248).
        let mut section_nodes = Vec::new();
        for target in self.active_targets(node, "hasPart").await {
            if self
                .graph
                .get_node_labels(target)
                .await
                .iter()
                .any(|label| label == "Section")
            {
                section_nodes.push(target);
            }
        }
        // PIR-011 / D246 (T02): the revision also owns its bibliography
        // entries through additive edges — Reference nodes via hasPart,
        // Citation nodes via CITES — without cloning the globally deduped
        // nodes (MEM471). Expose the per-revision citation surface so the
        // PIR-011 acceptance ("citation query points at the exact
        // PaperRevision") is readable from the query use case (T04).
        let mut reference_vids = Vec::new();
        for target in self.active_targets(node, "hasPart").await {
            if self
                .graph
                .get_node_labels(target)
                .await
                .iter()
                .any(|label| label == "Reference")
                && let Some(vid) = self.graph.get_node_property_string(target, "vid").await
            {
                reference_vids.push(vid);
            }
        }
        reference_vids.sort();
        let mut citation_arxiv_ids = Vec::new();
        for target in self.active_targets(node, "CITES").await {
            if let Some(arxiv_id) = self
                .graph
                .get_node_property_string(target, "arxiv_id")
                .await
            {
                citation_arxiv_ids.push(arxiv_id);
            }
        }
        citation_arxiv_ids.sort();
        let manifestation_nodes = self.active_targets(node, "HAS_MANIFESTATION").await;
        let readiness_nodes = self.active_targets(node, "hasQualitySignal").await;
        let predecessor_nodes = self.active_targets(node, "SUPERSEDES").await;
        let predecessor_revision = match predecessor_nodes.as_slice() {
            [] => None,
            [predecessor] => self
                .graph
                .get_node_property_int(*predecessor, "revision")
                .await
                .and_then(|value| u32::try_from(value).ok()),
            _ => {
                return Err(RevisionQueryError::InvalidGraph(format!(
                    "PaperRevision v{revision} has multiple active SUPERSEDES targets"
                )));
            }
        };

        let mut manifestations = Vec::new();
        for manifestation in manifestation_nodes {
            manifestations.push(RevisionManifestationView {
                vid: self.required_string(manifestation, "vid").await?,
                format: self.required_string(manifestation, "format").await?,
                sha256: self.required_string(manifestation, "sha256").await?,
                bytes: self
                    .graph
                    .get_node_property_int(manifestation, "bytes")
                    .await
                    .ok_or_else(|| {
                        RevisionQueryError::InvalidGraph(format!(
                            "SourceManifestation node {manifestation} lacks bytes"
                        ))
                    })?,
                uri: self.required_string(manifestation, "uri").await?,
                path: self.required_string(manifestation, "path").await?,
            });
        }
        manifestations.sort_by(|a, b| a.format.cmp(&b.format).then(a.vid.cmp(&b.vid)));

        let mut summaries = Vec::new();
        for section in &section_nodes {
            let source_vid = self.required_string(*section, "vid").await?;
            for summary in self.active_targets(*section, "hasSummary").await {
                if self
                    .graph
                    .get_node_property_string(summary, "source_id")
                    .await
                    .as_deref()
                    != Some(source_vid.as_str())
                    || self
                        .graph
                        .get_node_property_string(summary, "revision_vid")
                        .await
                        .as_deref()
                        != Some(revision_vid.as_str())
                {
                    continue;
                }
                summaries.push(RevisionSummaryView {
                    vid: self.required_string(summary, "vid").await?,
                    source_vid: source_vid.clone(),
                    summary_type: self.required_string(summary, "summary_type").await?,
                    text: self.required_string(summary, "text").await?,
                    source_text_sha256: self.required_string(summary, "source_text_sha256").await?,
                    method: self.required_string(summary, "method").await?,
                });
            }
        }
        summaries.sort_by(|left, right| {
            left.source_vid
                .cmp(&right.source_vid)
                .then(left.vid.cmp(&right.vid))
        });

        let mut current_readiness = Vec::new();
        for signal in readiness_nodes {
            if self
                .graph
                .get_node_property_string(signal, "signal_type")
                .await
                .as_deref()
                == Some("agent_readiness")
                && self
                    .graph
                    .get_node_property_bool(signal, "is_current")
                    .await
                    == Some(true)
                && self
                    .graph
                    .get_node_property_string(signal, "source_id")
                    .await
                    .as_deref()
                    == Some(revision_vid.as_str())
            {
                current_readiness.push(signal);
            }
        }
        let readiness = match current_readiness.as_slice() {
            [] => None,
            [signal] => Some(RevisionReadinessView {
                vid: self.required_string(*signal, "vid").await?,
                state: self.required_string(*signal, "state").await?,
                blocker_codes: self.required_json_strings(*signal, "blocker_codes").await?,
                diagnostic_codes: self
                    .required_json_strings(*signal, "diagnostic_codes")
                    .await?,
                evidence_sha256: self.required_string(*signal, "evidence_sha256").await?,
                policy_schema_version: self
                    .required_string(*signal, "policy_schema_version")
                    .await?,
                policy_sha256: self.required_string(*signal, "policy_sha256").await?,
            }),
            _ => {
                return Err(RevisionQueryError::InvalidGraph(format!(
                    "PaperRevision v{revision} has multiple current agent_readiness signals"
                )));
            }
        };

        Ok(ArticleRevisionView {
            revision,
            revision_vid,
            source_code,
            source_key,
            is_current,
            predecessor_revision,
            section_count: section_nodes.len(),
            reference_vids,
            citation_arxiv_ids,
            summaries,
            readiness,
            manifestations,
            import_eligible,
        })
    }

    async fn active_targets(&self, source: u64, edge_type: &str) -> Vec<u64> {
        let mut targets = BTreeSet::new();
        for (target, observed_type) in self.graph.get_outgoing_edges(source).await {
            if observed_type != edge_type {
                continue;
            }
            let mut active = false;
            for (edge, _) in self
                .graph
                .get_edges_between(source, target, edge_type)
                .await
            {
                if self
                    .graph
                    .get_edge_property_string(edge, "invalid_at")
                    .await
                    .is_none()
                {
                    active = true;
                    break;
                }
            }
            if active {
                targets.insert(target);
            }
        }
        targets.into_iter().collect()
    }

    async fn required_json_strings(
        &self,
        node: u64,
        property: &str,
    ) -> Result<Vec<String>, RevisionQueryError> {
        let raw = self.required_string(node, property).await?;
        serde_json::from_str(&raw).map_err(|error| {
            RevisionQueryError::InvalidGraph(format!(
                "node {node} has invalid JSON string array {property}: {error}"
            ))
        })
    }

    async fn required_string(
        &self,
        node: u64,
        property: &str,
    ) -> Result<String, RevisionQueryError> {
        self.graph
            .get_node_property_string(node, property)
            .await
            .ok_or_else(|| {
                RevisionQueryError::InvalidGraph(format!(
                    "node {node} lacks required string property {property}"
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kg_storage::{DirectGraphStore, MockGraphStore};

    async fn fixture(current: &[u32]) -> (MockGraphStore, RevisionQueryUseCase) {
        let store = MockGraphStore::new();
        let paper = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(paper, "vid", paper_vid("2506.07853"))
            .await
            .unwrap();
        store
            .set_node_property_string(paper, "arxiv_id", "2506.07853".into())
            .await
            .unwrap();
        for revision in [5_u32, 1] {
            let revision_node = store.create_node("PaperRevision").await.unwrap();
            store
                .set_node_property_string(revision_node, "vid", format!("revision-vid-{revision}"))
                .await
                .unwrap();
            store
                .set_node_property_string(revision_node, "paper_vid", paper_vid("2506.07853"))
                .await
                .unwrap();
            store
                .set_node_property_string(revision_node, "source_code", "arxiv".into())
                .await
                .unwrap();
            store
                .set_node_property_string(
                    revision_node,
                    "source_key",
                    format!("2506.07853v{revision}"),
                )
                .await
                .unwrap();
            store
                .set_node_property_int(revision_node, "revision", revision as i64)
                .await
                .unwrap();
            store
                .set_node_property_bool(revision_node, "is_current", current.contains(&revision))
                .await
                .unwrap();
            store
                .set_node_property_bool(revision_node, "import_eligible", false)
                .await
                .unwrap();
            store
                .create_edge(paper, revision_node, "HAS_REVISION")
                .await
                .unwrap();
            let mut revision_sections = Vec::new();
            for ordinal in 0..if revision == 1 { 2 } else { 3 } {
                let section = store.create_node("Section").await.unwrap();
                store
                    .set_node_property_string(
                        section,
                        "vid",
                        format!("section-{revision}-{ordinal}"),
                    )
                    .await
                    .unwrap();
                store
                    .create_edge(revision_node, section, "hasPart")
                    .await
                    .unwrap();
                revision_sections.push(section);
            }
            // PIR-011 additive revision-owned bibliography edges (T02): the
            // revision also owns its Reference bibliography entries via
            // hasPart and its Citations via a citation edge — without cloning
            // the globally deduped nodes (MEM471). section_count must ignore
            // these (D248).
            let reference = store.create_node("Reference").await.unwrap();
            store
                .set_node_property_string(reference, "vid", format!("reference-{revision}"))
                .await
                .unwrap();
            store
                .create_edge(revision_node, reference, "hasPart")
                .await
                .unwrap();
            let citation = store.create_node("Citation").await.unwrap();
            store
                .set_node_property_string(citation, "vid", format!("citation-{revision}"))
                .await
                .unwrap();
            store
                .set_node_property_string(
                    citation,
                    "arxiv_id",
                    if revision == 1 {
                        "2301.10001".into()
                    } else {
                        "2301.10005".into()
                    },
                )
                .await
                .unwrap();
            crate::temporal_stamp::create_edge_with_temporal(
                &store,
                revision_node,
                citation,
                "CITES",
                &crate::temporal_stamp::open_fact_edge_now(
                    chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                ),
            )
            .await
            .unwrap();
            let summary = store.create_node("Summary").await.unwrap();
            for (property, value) in [
                ("vid", format!("summary-{revision}")),
                ("source_id", format!("section-{revision}-0")),
                ("revision_vid", format!("revision-vid-{revision}")),
                (
                    "summary_type",
                    if revision == 5 {
                        "methods".into()
                    } else {
                        "generic".into()
                    },
                ),
                ("text", format!("summary text v{revision}")),
                ("source_text_sha256", format!("{:064x}", revision)),
                ("method", "extractive.first_sentence.v1".into()),
            ] {
                store
                    .set_node_property_string(summary, property, value)
                    .await
                    .unwrap();
            }
            store
                .create_edge(revision_sections[0], summary, "hasSummary")
                .await
                .unwrap();
            let readiness = store.create_node("QualitySignal").await.unwrap();
            for (property, value) in [
                ("vid", format!("readiness-{revision}")),
                ("signal_type", "agent_readiness".into()),
                ("source_id", format!("revision-vid-{revision}")),
                ("state", "partial".into()),
                ("blocker_codes", "[]".into()),
                (
                    "diagnostic_codes",
                    if revision == 5 {
                        "[\"missing_results_summary\"]".into()
                    } else {
                        "[\"missing_methods_summary\",\"missing_results_summary\"]".into()
                    },
                ),
                ("evidence_sha256", format!("{:064x}", revision + 10)),
                ("policy_schema_version", "revision-readiness.v1".into()),
                ("policy_sha256", "b".repeat(64)),
            ] {
                store
                    .set_node_property_string(readiness, property, value)
                    .await
                    .unwrap();
            }
            store
                .set_node_property_bool(readiness, "is_current", true)
                .await
                .unwrap();
            crate::temporal_stamp::create_edge_with_temporal(
                &store,
                revision_node,
                readiness,
                "hasQualitySignal",
                &crate::temporal_stamp::open_fact_edge_now(
                    chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                ),
            )
            .await
            .unwrap();
            for format in ["pdf", "html"] {
                let manifestation = store.create_node("SourceManifestation").await.unwrap();
                store
                    .set_node_property_string(
                        manifestation,
                        "vid",
                        format!("manifest-{revision}-{format}"),
                    )
                    .await
                    .unwrap();
                store
                    .set_node_property_string(manifestation, "format", format.into())
                    .await
                    .unwrap();
                store
                    .set_node_property_string(
                        manifestation,
                        "sha256",
                        format!("{:064x}", revision + format.len() as u32),
                    )
                    .await
                    .unwrap();
                store
                    .set_node_property_int(manifestation, "bytes", 100)
                    .await
                    .unwrap();
                store
                    .set_node_property_string(
                        manifestation,
                        "uri",
                        format!("https://example/{revision}/{format}"),
                    )
                    .await
                    .unwrap();
                store
                    .set_node_property_string(
                        manifestation,
                        "path",
                        format!("article.v{revision}.{format}"),
                    )
                    .await
                    .unwrap();
                store
                    .create_edge(revision_node, manifestation, "HAS_MANIFESTATION")
                    .await
                    .unwrap();
            }
        }
        let v1 = store
            .find_node_by_string_property("PaperRevision", "vid", "revision-vid-1")
            .await
            .unwrap();
        let v5 = store
            .find_node_by_string_property("PaperRevision", "vid", "revision-vid-5")
            .await
            .unwrap();
        crate::temporal_stamp::create_edge_with_temporal(
            &store,
            v5,
            v1,
            "SUPERSEDES",
            &crate::temporal_stamp::open_fact_edge_now(
                chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            ),
        )
        .await
        .unwrap();
        let use_case = RevisionQueryUseCase::new(Arc::new(store.clone()));
        (store, use_case)
    }

    #[tokio::test]
    async fn contract_history_is_numeric_and_serializable() {
        let (_store, query) = fixture(&[5]).await;
        let nodes_before = _store.node_count_total();
        let edges_before = _store.edge_count_total();
        let response = query
            .query("2506.07853", RevisionSelector::History)
            .await
            .unwrap();
        assert_eq!(
            response
                .revisions
                .iter()
                .map(|r| r.revision)
                .collect::<Vec<_>>(),
            vec![1, 5]
        );
        assert_eq!(response.revisions[0].section_count, 2);
        assert_eq!(response.revisions[1].section_count, 3);
        assert_eq!(response.revisions[1].predecessor_revision, Some(1));
        // PIR-011 / D246 (T02): per-revision citation surface. The fixture
        // gives each revision its own Reference (hasPart) and Citation
        // (CITES) without cloning global nodes — the view must expose the
        // exact revision-owned bibliography sets.
        assert_eq!(response.revisions[0].reference_vids, ["reference-1"]);
        assert_eq!(response.revisions[1].reference_vids, ["reference-5"]);
        assert_eq!(response.revisions[0].citation_arxiv_ids, ["2301.10001"]);
        assert_eq!(response.revisions[1].citation_arxiv_ids, ["2301.10005"]);
        assert_ne!(
            response.revisions[0].reference_vids,
            response.revisions[1].reference_vids
        );
        assert_ne!(
            response.revisions[0].citation_arxiv_ids,
            response.revisions[1].citation_arxiv_ids
        );
        assert_eq!(response.revisions[0].summaries.len(), 1);
        assert_eq!(response.revisions[0].summaries[0].summary_type, "generic");
        assert_eq!(response.revisions[1].summaries[0].summary_type, "methods");
        assert_eq!(
            response.revisions[0]
                .readiness
                .as_ref()
                .unwrap()
                .diagnostic_codes,
            ["missing_methods_summary", "missing_results_summary"]
        );
        assert_eq!(
            response.revisions[1]
                .readiness
                .as_ref()
                .unwrap()
                .diagnostic_codes,
            ["missing_results_summary"]
        );
        assert!(
            response
                .revisions
                .iter()
                .all(|r| r.manifestations.len() == 2)
        );
        let json = serde_json::to_string(&response).unwrap();
        let decoded: RevisionQueryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, response);
        assert!(!response.import_eligible);
        assert_eq!(_store.node_count_total(), nodes_before);
        assert_eq!(_store.edge_count_total(), edges_before);
    }

    #[tokio::test]
    async fn section_count_counts_only_section_labeled_has_part_targets() {
        // D248 / MEM797: PaperRevision hasPart now also targets bibliography
        // Reference nodes (PIR-011 additive revision-owned edges, T02).
        // section_count must filter by Section label — bibliography nodes
        // must never masquerade as sections.
        let (store, query) = fixture(&[5]).await;
        let response = query
            .query("2506.07853", RevisionSelector::History)
            .await
            .unwrap();
        assert_eq!(response.revisions[0].section_count, 2);
        assert_eq!(response.revisions[1].section_count, 3);
        // Sanity: the fixture really carries a Reference hasPart target that
        // must be excluded from section_count.
        let v1 = store
            .find_node_by_string_property("PaperRevision", "vid", "revision-vid-1")
            .await
            .unwrap();
        let has_part_targets = store
            .get_outgoing_edges(v1)
            .await
            .iter()
            .filter(|(_, edge_type)| edge_type == "hasPart")
            .count();
        assert_eq!(
            has_part_targets, 3,
            "fixture must have Section(2) + Reference(1) hasPart targets"
        );
    }

    #[tokio::test]
    async fn current_and_exact_select_without_insertion_order_guessing() {
        let (_store, query) = fixture(&[5]).await;
        let current = query
            .query("2506.07853", RevisionSelector::Current)
            .await
            .unwrap();
        assert_eq!(current.revisions[0].revision, 5);
        let exact = query
            .query("2506.07853", RevisionSelector::exact(1).unwrap())
            .await
            .unwrap();
        assert_eq!(exact.revisions[0].revision, 1);
        assert!(!exact.revisions[0].is_current);
    }

    #[tokio::test]
    async fn missing_and_invalid_selectors_fail_honestly() {
        let (_store, query) = fixture(&[5]).await;
        assert!(matches!(
            query.query("2506.07853", RevisionSelector::Exact(3)).await,
            Err(RevisionQueryError::RevisionNotFound { revision: 3, .. })
        ));
        assert!(matches!(
            query.query("missing", RevisionSelector::History).await,
            Err(RevisionQueryError::PaperNotFound { .. })
        ));
        assert!(RevisionSelector::exact(0).is_err());
        assert_eq!(
            "current".parse::<RevisionSelector>().unwrap(),
            RevisionSelector::Current
        );
        assert_eq!(
            "v5".parse::<RevisionSelector>().unwrap(),
            RevisionSelector::Exact(5)
        );
        assert!("latest".parse::<RevisionSelector>().is_err());
        assert!("v0".parse::<RevisionSelector>().is_err());
        assert!(matches!(
            query.query("2506.07853", RevisionSelector::Exact(0)).await,
            Err(RevisionQueryError::InvalidSelector(_))
        ));
    }

    #[tokio::test]
    async fn duplicate_current_readiness_is_invalid_graph() {
        let (store, query) = fixture(&[5]).await;
        let revision = store
            .find_node_by_string_property("PaperRevision", "vid", "revision-vid-5")
            .await
            .unwrap();
        let duplicate = store.create_node("QualitySignal").await.unwrap();
        for (property, value) in [
            ("vid", "duplicate-readiness"),
            ("signal_type", "agent_readiness"),
            ("source_id", "revision-vid-5"),
        ] {
            store
                .set_node_property_string(duplicate, property, value.into())
                .await
                .unwrap();
        }
        store
            .set_node_property_bool(duplicate, "is_current", true)
            .await
            .unwrap();
        crate::temporal_stamp::create_edge_with_temporal(
            &store,
            revision,
            duplicate,
            "hasQualitySignal",
            &crate::temporal_stamp::open_fact_edge_now(
                chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            ),
        )
        .await
        .unwrap();
        assert!(matches!(
            query.query("2506.07853", RevisionSelector::Exact(5)).await,
            Err(RevisionQueryError::InvalidGraph(message))
                if message.contains("multiple current agent_readiness")
        ));
    }

    #[tokio::test]
    async fn no_or_multiple_current_flags_are_errors() {
        let (_store, missing) = fixture(&[]).await;
        assert!(matches!(
            missing.query("2506.07853", RevisionSelector::Current).await,
            Err(RevisionQueryError::MissingCurrent { .. })
        ));
        let (_store, ambiguous) = fixture(&[1, 5]).await;
        assert!(matches!(
            ambiguous
                .query("2506.07853", RevisionSelector::Current)
                .await,
            Err(RevisionQueryError::AmbiguousCurrent { .. })
        ));
    }
}
