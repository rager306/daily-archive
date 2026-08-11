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
pub struct ArticleRevisionView {
    pub revision: u32,
    pub revision_vid: String,
    pub source_code: String,
    pub source_key: String,
    pub is_current: bool,
    pub predecessor_revision: Option<u32>,
    pub section_count: usize,
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

        let section_nodes = self.active_targets(node, "hasPart").await;
        let manifestation_nodes = self.active_targets(node, "HAS_MANIFESTATION").await;
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

        Ok(ArticleRevisionView {
            revision,
            revision_vid,
            source_code,
            source_key,
            is_current,
            predecessor_revision,
            section_count: section_nodes.len(),
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
            }
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
