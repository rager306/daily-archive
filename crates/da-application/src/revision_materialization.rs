//! Revision-scoped article graph materialization (ADR-058, D223).
//!
//! Additive application use case: canonical Paper identity remains versionless,
//! while immutable PaperRevision, SourceManifestation, and Section identities
//! carry exact source lineage. The universal ArticleLoadUseCase intentionally
//! does not own these graph writes.

use std::sync::Arc;

use da_domain::relation::{bibliographic, structure};
use da_domain::{paper_revision_vid, paper_vid, revision_section_vid, source_manifestation_vid};
use da_ports::Clock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionManifestationInput {
    pub format: String,
    pub sha256: String,
    pub bytes: u64,
    pub uri: String,
    pub path: String,
    pub capture_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionContentContext {
    pub source_code: String,
    pub canonical_id: String,
    pub revision: u32,
    pub manifestations: Vec<RevisionManifestationInput>,
}

impl RevisionContentContext {
    pub fn revision_vid(&self) -> Result<String, RevisionMaterializationError> {
        paper_revision_vid(&self.source_code, &self.canonical_id, self.revision)
            .map_err(RevisionMaterializationError::Invalid)
    }

    pub fn section_vid(&self, ordinal: usize) -> Result<String, RevisionMaterializationError> {
        revision_section_vid(&self.revision_vid()?, ordinal)
            .map_err(RevisionMaterializationError::Invalid)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionMaterializationRequest {
    pub source_code: String,
    pub canonical_id: String,
    pub revision: u32,
    pub parsed: ParsedArticle,
    pub manifestations: Vec<RevisionManifestationInput>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionMaterializationResult {
    pub paper_vid: String,
    pub revision_vid: String,
    pub current_revision: u32,
    pub predecessor_revision: Option<u32>,
    pub nodes_created: usize,
    pub nodes_reused: usize,
    pub edges_created: usize,
    pub edges_reused: usize,
    pub sections_created: usize,
    pub manifestations_created: usize,
    pub import_eligible: bool,
}

#[derive(Debug, Error)]
pub enum RevisionMaterializationError {
    #[error("invalid revision materialization request: {0}")]
    Invalid(String),
    #[error("graph write failed during {operation}: {source}")]
    Graph {
        operation: &'static str,
        #[source]
        source: kg_storage::GraphStoreError,
    },
    #[error("temporal graph write failed during {operation}: {source}")]
    TemporalGraph {
        operation: &'static str,
        #[source]
        source: anyhow::Error,
    },
}

pub struct RevisionMaterializationUseCase {
    graph_store: Arc<dyn DirectGraphStore>,
    clock: Arc<dyn Clock>,
}

impl RevisionMaterializationUseCase {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, clock: Arc<dyn Clock>) -> Self {
        Self { graph_store, clock }
    }

    pub async fn materialize(
        &self,
        request: &RevisionMaterializationRequest,
    ) -> Result<RevisionMaterializationResult, RevisionMaterializationError> {
        validate_request(request)?;
        let paper_vid_value = paper_vid(&request.canonical_id);
        let revision_vid_value = paper_revision_vid(
            &request.source_code,
            &request.canonical_id,
            request.revision,
        )
        .map_err(RevisionMaterializationError::Invalid)?;
        let content_identity_sha256 = content_identity_sha256(&request.manifestations);
        let mut result = RevisionMaterializationResult {
            paper_vid: paper_vid_value.clone(),
            revision_vid: revision_vid_value.clone(),
            import_eligible: false,
            ..RevisionMaterializationResult::default()
        };

        let paper_node = self
            .find_or_create_node("Paper", &paper_vid_value, &mut result)
            .await?;
        self.set_string(paper_node, "vid", &paper_vid_value).await?;
        self.set_string(paper_node, "arxiv_id", &request.canonical_id)
            .await?;
        // PIR-008 / M398-S04: node DateTime wire is a Clock-derived RFC3339
        // UTC string. The int read below is the bounded legacy decoder for
        // pre-PIR-008 snapshots — never overwrite an existing value on replay.
        if self
            .graph_store
            .get_node_property_string(paper_node, "valid_from")
            .await
            .is_none()
            && self
                .graph_store
                .get_node_property_int(paper_node, "valid_from")
                .await
                .is_none()
        {
            self.set_string(paper_node, "valid_from", &self.clock.now().to_rfc3339())
                .await?;
        }
        self.set_bool(paper_node, "import_eligible", false).await?;

        let revision_node = self
            .find_or_create_node("PaperRevision", &revision_vid_value, &mut result)
            .await?;
        self.set_string(revision_node, "vid", &revision_vid_value)
            .await?;
        self.set_string(revision_node, "paper_vid", &paper_vid_value)
            .await?;
        self.set_string(revision_node, "canonical_id", &request.canonical_id)
            .await?;
        self.set_string(
            revision_node,
            "source_code",
            &request.source_code.trim().to_ascii_lowercase(),
        )
        .await?;
        self.set_string(
            revision_node,
            "source_key",
            &format!("{}v{}", request.canonical_id, request.revision),
        )
        .await?;
        self.set_int(revision_node, "revision", request.revision as i64)
            .await?;
        self.set_string(
            revision_node,
            "content_identity_sha256",
            &content_identity_sha256,
        )
        .await?;
        // Abstract is revision-mutable content (ADR-058): never project an
        // older abstract onto canonical Paper or share it across revisions.
        self.set_string(
            revision_node,
            "abstract_text",
            &request.parsed.abstract_text,
        )
        .await?;
        self.set_bool(revision_node, "import_eligible", false)
            .await?;
        self.set_int(revision_node, "schema_version", 1).await?;
        self.ensure_edge(
            paper_node,
            revision_node,
            structure::HAS_REVISION,
            false,
            &mut result,
        )
        .await?;

        for manifestation in &request.manifestations {
            let manifestation_vid = source_manifestation_vid(&manifestation.sha256)
                .map_err(RevisionMaterializationError::Invalid)?;
            let existed = self
                .graph_store
                .find_node_by_string_property("SourceManifestation", "vid", &manifestation_vid)
                .await
                .is_some();
            let node = self
                .find_or_create_node("SourceManifestation", &manifestation_vid, &mut result)
                .await?;
            if !existed {
                result.manifestations_created += 1;
            }
            self.set_string(node, "vid", &manifestation_vid).await?;
            self.set_string(node, "format", &manifestation.format.to_ascii_lowercase())
                .await?;
            self.set_string(node, "sha256", &manifestation.sha256.to_ascii_lowercase())
                .await?;
            self.set_int(node, "bytes", manifestation.bytes as i64)
                .await?;
            self.set_string(node, "uri", &manifestation.uri).await?;
            self.set_string(node, "path", &manifestation.path).await?;
            self.set_string(node, "capture_status", &manifestation.capture_status)
                .await?;
            self.set_bool(node, "import_eligible", false).await?;
            self.set_int(node, "schema_version", 1).await?;
            self.ensure_edge(
                revision_node,
                node,
                structure::HAS_MANIFESTATION,
                false,
                &mut result,
            )
            .await?;
        }

        for (ordinal, section) in request.parsed.sections.iter().enumerate() {
            let section_vid = revision_section_vid(&revision_vid_value, ordinal)
                .map_err(RevisionMaterializationError::Invalid)?;
            let existed = self
                .graph_store
                .find_node_by_string_property("Section", "vid", &section_vid)
                .await
                .is_some();
            let section_node = self
                .find_or_create_node("Section", &section_vid, &mut result)
                .await?;
            if !existed {
                result.sections_created += 1;
            }
            self.set_string(section_node, "vid", &section_vid).await?;
            self.set_string(section_node, "title", &section.title)
                .await?;
            self.set_string(section_node, "text", &section.text).await?;
            self.set_int(section_node, "level", section.level as i64)
                .await?;
            self.set_int(section_node, "order", ordinal as i64).await?;
            self.set_string(section_node, "work_vid", &revision_vid_value)
                .await?;
            // ADR-053 Layer A tree evidence: persist the exact AST ownership
            // fields on revision-scoped Section nodes. Ingest also writes these
            // (ingest.rs section loop), but the standalone materializer — and
            // any caller that does not go through ingest — must not drop AST
            // ownership. PIR-011 depends on this tree identity; S02 summaries
            // already join on path_id. VID stays `revision_section_vid` so
            // replay idempotency is preserved (evidence properties only).
            if let Some(path_id) = &section.path_id {
                self.set_string(section_node, "path_id", path_id).await?;
            }
            if let Some(section_id) = &section.section_id {
                self.set_string(section_node, "section_id", section_id)
                    .await?;
            }
            if let Some(parent_section_id) = &section.parent_section_id {
                self.set_string(section_node, "parent_section_id", parent_section_id)
                    .await?;
            }
            if let Some(order_in_parent) = section.order_in_parent {
                self.set_int(section_node, "order_in_parent", order_in_parent as i64)
                    .await?;
            }
            self.set_int(
                section_node,
                "char_count",
                section.text.chars().count() as i64,
            )
            .await?;
            self.set_string(
                section_node,
                "text_sha256",
                &hex::encode(Sha256::digest(section.text.as_bytes())),
            )
            .await?;
            self.set_bool(section_node, "import_eligible", false)
                .await?;
            self.set_int(section_node, "schema_version", 1).await?;
            self.ensure_edge(
                revision_node,
                section_node,
                structure::HAS_PART,
                false,
                &mut result,
            )
            .await?;
        }

        let observed = self.observed_revisions(&paper_vid_value).await;
        result.current_revision = observed
            .iter()
            .map(|(_, revision)| *revision)
            .max()
            .unwrap_or(request.revision);
        for (node, revision) in &observed {
            self.set_bool(
                node.to_owned(),
                "is_current",
                *revision == result.current_revision,
            )
            .await?;
        }
        // Canonical work-level display title projects only the highest observed
        // revision. Replaying an older revision must not roll the Paper back.
        if request.revision == result.current_revision {
            self.set_string(paper_node, "title", &request.parsed.title)
                .await?;
        }
        result.predecessor_revision = observed
            .iter()
            .filter(|(_, revision)| *revision < request.revision)
            .map(|(_, revision)| *revision)
            .max();
        self.reconcile_supersedes(&observed, &mut result).await?;

        tracing::info!(
            canonical_id = %request.canonical_id,
            revision = request.revision,
            current_revision = result.current_revision,
            nodes_created = result.nodes_created,
            nodes_reused = result.nodes_reused,
            edges_created = result.edges_created,
            edges_reused = result.edges_reused,
            import_eligible = false,
            "article revision graph materialized"
        );
        Ok(result)
    }

    async fn observed_revisions(&self, paper_vid_value: &str) -> Vec<(u64, u32)> {
        let mut out = Vec::new();
        for node in self.graph_store.get_nodes_by_label("PaperRevision").await {
            if self
                .graph_store
                .get_node_property_string(node, "paper_vid")
                .await
                .as_deref()
                == Some(paper_vid_value)
                && let Some(revision) = self
                    .graph_store
                    .get_node_property_int(node, "revision")
                    .await
                    .and_then(|value| u32::try_from(value).ok())
            {
                out.push((node, revision));
            }
        }
        out.sort_by_key(|(_, revision)| *revision);
        out
    }

    async fn reconcile_supersedes(
        &self,
        observed: &[(u64, u32)],
        result: &mut RevisionMaterializationResult,
    ) -> Result<(), RevisionMaterializationError> {
        // Invalidate any previously-direct link that is no longer between
        // adjacent observed revisions (e.g. v5→v1 after v3 is discovered).
        for (newer_index, (newer_node, _)) in observed.iter().enumerate() {
            for (older_index, (older_node, _)) in observed.iter().enumerate() {
                if older_index >= newer_index || older_index + 1 == newer_index {
                    continue;
                }
                for (edge_id, _) in self
                    .graph_store
                    .get_edges_between(*newer_node, *older_node, bibliographic::SUPERSEDES)
                    .await
                {
                    if self
                        .graph_store
                        .get_edge_property_string(edge_id, "invalid_at")
                        .await
                        .is_none()
                    {
                        self.graph_store
                            .set_edge_property_string(
                                edge_id,
                                "invalid_at",
                                &self.clock.now().to_rfc3339(),
                            )
                            .await
                            .map_err(|source| RevisionMaterializationError::Graph {
                                operation: "invalidate stale supersedes edge",
                                source,
                            })?;
                    }
                }
            }
        }

        for adjacent in observed.windows(2) {
            let (older_node, _) = adjacent[0];
            let (newer_node, _) = adjacent[1];
            self.ensure_edge(
                newer_node,
                older_node,
                bibliographic::SUPERSEDES,
                true,
                result,
            )
            .await?;
        }
        Ok(())
    }

    async fn find_or_create_node(
        &self,
        label: &'static str,
        vid: &str,
        result: &mut RevisionMaterializationResult,
    ) -> Result<u64, RevisionMaterializationError> {
        if let Some(node) = self
            .graph_store
            .find_node_by_string_property(label, "vid", vid)
            .await
        {
            result.nodes_reused += 1;
            return Ok(node);
        }
        let node = self
            .graph_store
            .create_node(label)
            .await
            .map_err(|source| RevisionMaterializationError::Graph {
                operation: "create node",
                source,
            })?;
        result.nodes_created += 1;
        Ok(node)
    }

    async fn ensure_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &'static str,
        temporal: bool,
        result: &mut RevisionMaterializationResult,
    ) -> Result<(), RevisionMaterializationError> {
        if !self
            .graph_store
            .get_edges_between(source, target, edge_type)
            .await
            .is_empty()
        {
            result.edges_reused += 1;
            return Ok(());
        }
        if temporal {
            let stamp = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
            crate::temporal_stamp::create_edge_with_temporal(
                self.graph_store.as_ref(),
                source,
                target,
                edge_type,
                &stamp,
            )
            .await
            .map_err(|source| RevisionMaterializationError::TemporalGraph {
                operation: "create temporal edge",
                source,
            })?;
        } else {
            self.graph_store
                .create_edge(source, target, edge_type)
                .await
                .map_err(|source| RevisionMaterializationError::Graph {
                    operation: "create structural edge",
                    source,
                })?;
        }
        result.edges_created += 1;
        Ok(())
    }

    async fn set_string(
        &self,
        node: u64,
        key: &'static str,
        value: &str,
    ) -> Result<(), RevisionMaterializationError> {
        self.graph_store
            .set_node_property_string(node, key, value.to_string())
            .await
            .map_err(|source| RevisionMaterializationError::Graph {
                operation: "set string property",
                source,
            })
    }

    async fn set_int(
        &self,
        node: u64,
        key: &'static str,
        value: i64,
    ) -> Result<(), RevisionMaterializationError> {
        self.graph_store
            .set_node_property_int(node, key, value)
            .await
            .map_err(|source| RevisionMaterializationError::Graph {
                operation: "set integer property",
                source,
            })
    }

    async fn set_bool(
        &self,
        node: u64,
        key: &'static str,
        value: bool,
    ) -> Result<(), RevisionMaterializationError> {
        self.graph_store
            .set_node_property_bool(node, key, value)
            .await
            .map_err(|source| RevisionMaterializationError::Graph {
                operation: "set boolean property",
                source,
            })
    }
}

impl RevisionMaterializationRequest {
    pub fn context(&self) -> RevisionContentContext {
        RevisionContentContext {
            source_code: self.source_code.clone(),
            canonical_id: self.canonical_id.clone(),
            revision: self.revision,
            manifestations: self.manifestations.clone(),
        }
    }
}

fn validate_request(
    request: &RevisionMaterializationRequest,
) -> Result<(), RevisionMaterializationError> {
    paper_revision_vid(
        &request.source_code,
        &request.canonical_id,
        request.revision,
    )
    .map_err(RevisionMaterializationError::Invalid)?;
    if request.manifestations.is_empty() {
        return Err(RevisionMaterializationError::Invalid(
            "at least one source manifestation is required".into(),
        ));
    }
    let parsed_hash = request.parsed.pdf_hash.trim().to_ascii_lowercase();
    if parsed_hash.is_empty() {
        return Err(RevisionMaterializationError::Invalid(
            "parsed article lacks source pdf_hash for checksum-bound revision identity".into(),
        ));
    }
    if !request
        .manifestations
        .iter()
        .any(|manifestation| manifestation.sha256.trim().to_ascii_lowercase() == parsed_hash)
    {
        return Err(RevisionMaterializationError::Invalid(format!(
            "revision manifestations do not bind parsed source digest {parsed_hash}"
        )));
    }
    for manifestation in &request.manifestations {
        if manifestation.format.trim().is_empty() {
            return Err(RevisionMaterializationError::Invalid(
                "manifestation format must not be empty".into(),
            ));
        }
        source_manifestation_vid(&manifestation.sha256)
            .map_err(RevisionMaterializationError::Invalid)?;
        if manifestation.path.trim().is_empty() || manifestation.uri.trim().is_empty() {
            return Err(RevisionMaterializationError::Invalid(
                "manifestation path and uri must not be empty".into(),
            ));
        }
    }
    Ok(())
}

fn content_identity_sha256(manifestations: &[RevisionManifestationInput]) -> String {
    let mut identities = manifestations
        .iter()
        .map(|manifestation| {
            format!(
                "{}:{}",
                manifestation.format.trim().to_ascii_lowercase(),
                manifestation.sha256.trim().to_ascii_lowercase()
            )
        })
        .collect::<Vec<_>>();
    identities.sort();
    let mut hasher = Sha256::new();
    for identity in identities {
        hasher.update(identity.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::FixedClock;
    use da_ports::parser::Section;
    use kg_storage::MockGraphStore;

    fn parsed(id: &str, revision: u32, section_count: usize) -> ParsedArticle {
        ParsedArticle {
            paper_id: id.into(),
            title: format!("Paper {id}"),
            abstract_text: "abstract".into(),
            body_text: "body".into(),
            sections: (0..section_count)
                .map(|ordinal| Section {
                    title: format!("Section {ordinal}"),
                    text: format!("text {ordinal}"),
                    level: 1,
                    // ADR-053 Layer A tree evidence: every fixture section
                    // carries the exact AST ownership fields the materializer
                    // must persist on revision-scoped Section nodes.
                    path_id: Some(format!("{ordinal}")),
                    section_id: Some(format!("sec-{ordinal}")),
                    parent_section_id: (ordinal > 0).then(|| "sec-0".to_string()),
                    order_in_parent: Some(ordinal as u32),
                })
                .collect(),
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: format!("{revision:064x}"),
        }
    }

    fn request(revision: u32, section_count: usize) -> RevisionMaterializationRequest {
        RevisionMaterializationRequest {
            source_code: "arxiv".into(),
            canonical_id: "2506.07853".into(),
            revision,
            parsed: parsed("2506.07853", revision, section_count),
            manifestations: vec![
                RevisionManifestationInput {
                    format: "pdf".into(),
                    sha256: format!("{:064x}", revision),
                    bytes: 100 + revision as u64,
                    uri: format!("https://arxiv.org/pdf/2506.07853v{revision}"),
                    path: format!("source/2506.07853v{revision}.pdf"),
                    capture_status: "captured_local".into(),
                },
                RevisionManifestationInput {
                    format: "html".into(),
                    sha256: format!("{:064x}", revision + 100),
                    bytes: 200 + revision as u64,
                    uri: format!("https://arxiv.org/html/2506.07853v{revision}"),
                    path: format!("article.v{revision}.html"),
                    capture_status: "captured_local".into(),
                },
            ],
        }
    }

    fn use_case(store: MockGraphStore) -> RevisionMaterializationUseCase {
        RevisionMaterializationUseCase::new(
            Arc::new(store),
            Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        )
    }

    #[tokio::test]
    async fn contract_rejects_invalid_or_incomplete_request() {
        let store = MockGraphStore::new();
        let uc = use_case(store);
        let mut zero = request(1, 1);
        zero.revision = 0;
        assert!(matches!(
            uc.materialize(&zero).await,
            Err(RevisionMaterializationError::Invalid(_))
        ));
        let mut empty = request(1, 1);
        empty.manifestations.clear();
        assert!(matches!(
            uc.materialize(&empty).await,
            Err(RevisionMaterializationError::Invalid(_))
        ));
        let mut missing_hash = request(1, 1);
        missing_hash.parsed.pdf_hash.clear();
        let error = uc.materialize(&missing_hash).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("parsed article lacks source pdf_hash")
        );
        let mut mismatched_hash = request(1, 1);
        mismatched_hash.parsed.pdf_hash = format!("{:064x}", 99);
        let error = uc.materialize(&mismatched_hash).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("do not bind parsed source digest")
        );
    }

    #[tokio::test]
    async fn contract_result_roundtrips_and_d127_is_false() {
        let store = MockGraphStore::new();
        let result = use_case(store).materialize(&request(1, 1)).await.unwrap();
        crate::roundtrip_json(&result);
        assert!(!result.import_eligible);
        assert_eq!(result.current_revision, 1);
    }

    #[tokio::test]
    async fn one_paper_two_revisions_distinct_sections_and_manifestations() {
        let store = MockGraphStore::new();
        let uc = use_case(store.clone());
        let v1 = uc.materialize(&request(1, 2)).await.unwrap();
        let v5 = uc.materialize(&request(5, 3)).await.unwrap();

        assert_eq!(store.get_nodes_by_label("Paper").await.len(), 1);
        assert_eq!(store.get_nodes_by_label("PaperRevision").await.len(), 2);
        assert_eq!(
            store.get_nodes_by_label("SourceManifestation").await.len(),
            4
        );
        assert_eq!(store.get_nodes_by_label("Section").await.len(), 5);
        assert_eq!(v1.current_revision, 1);
        assert_eq!(v5.current_revision, 5);
        assert_eq!(v5.predecessor_revision, Some(1));
        assert_ne!(v1.revision_vid, v5.revision_vid);

        let revision_nodes = store.get_nodes_by_label("PaperRevision").await;
        let mut current = 0;
        for node in revision_nodes {
            if store.get_node_property_bool(node, "is_current").await == Some(true) {
                current += 1;
            }
        }
        assert_eq!(current, 1);
        let v5_node = store
            .find_node_by_string_property("PaperRevision", "vid", &v5.revision_vid)
            .await
            .unwrap();
        let v1_node = store
            .find_node_by_string_property("PaperRevision", "vid", &v1.revision_vid)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_node_property_string(v1_node, "canonical_id")
                .await
                .as_deref(),
            Some("2506.07853")
        );
        assert_eq!(
            store
                .get_node_property_string(v1_node, "abstract_text")
                .await
                .as_deref(),
            Some("abstract")
        );
        assert_eq!(
            store
                .get_edges_between(v5_node, v1_node, bibliographic::SUPERSEDES)
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn revision_section_persists_tree_evidence() {
        let store = MockGraphStore::new();
        let uc = use_case(store.clone());
        let v1 = uc.materialize(&request(1, 3)).await.unwrap();

        // Read back every revision-scoped Section via its content-addressed VID
        // and assert the exact AST ownership fields survived the standalone
        // materializer (PIR-011 depends on this tree identity; S02 summaries
        // already join on path_id).
        for ordinal in 0..3 {
            let section_vid = revision_section_vid(&v1.revision_vid, ordinal).unwrap();
            let section = store
                .find_node_by_string_property("Section", "vid", &section_vid)
                .await
                .unwrap();
            let text = format!("text {ordinal}");
            assert_eq!(
                store.get_node_property_string(section, "path_id").await,
                Some(format!("{ordinal}"))
            );
            assert_eq!(
                store.get_node_property_string(section, "section_id").await,
                Some(format!("sec-{ordinal}"))
            );
            let expected_parent = (ordinal > 0).then(|| "sec-0".to_string());
            assert_eq!(
                store
                    .get_node_property_string(section, "parent_section_id")
                    .await,
                expected_parent
            );
            assert_eq!(
                store
                    .get_node_property_int(section, "order_in_parent")
                    .await,
                Some(ordinal as i64)
            );
            assert_eq!(
                store.get_node_property_int(section, "char_count").await,
                Some(text.chars().count() as i64)
            );
            assert_eq!(
                store.get_node_property_string(section, "text_sha256").await,
                Some(hex::encode(Sha256::digest(text.as_bytes())))
            );
            assert_eq!(
                store.get_node_property_string(section, "work_vid").await,
                Some(v1.revision_vid.clone())
            );
            assert_eq!(
                store
                    .get_node_property_bool(section, "import_eligible")
                    .await,
                Some(false)
            );
        }
    }

    #[tokio::test]
    async fn ingest_context_owns_mutable_content_and_sections_on_revision() {
        let store = MockGraphStore::new();
        let ingest = crate::ingest::IngestUseCase::materializer_only_with_clock(
            Arc::new(store.clone()),
            Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        );
        let r1 = request(1, 2);
        let c1 = r1.context();
        let out1 = ingest
            .materialize_parsed_with_context(
                &r1.parsed,
                vec![0.1, 0.2],
                None,
                Some("arxiv".into()),
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                Some(&c1),
            )
            .await
            .unwrap();
        let r5 = request(5, 3);
        let c5 = r5.context();
        let out5 = ingest
            .materialize_parsed_with_context(
                &r5.parsed,
                vec![0.3, 0.4],
                None,
                Some("arxiv".into()),
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                Some(&c5),
            )
            .await
            .unwrap();
        assert_eq!(out1.sections_written, 2);
        assert_eq!(out5.sections_written, 3);
        assert!(out1.revision_vid.is_some() && out5.revision_vid.is_some());
        assert_eq!(store.get_nodes_by_label("Paper").await.len(), 1);
        assert_eq!(store.get_nodes_by_label("PaperRevision").await.len(), 2);
        assert_eq!(store.get_nodes_by_label("Section").await.len(), 5);
        let paper = store.get_nodes_by_label("Paper").await[0];
        assert!(
            store
                .get_node_property_string(paper, "embedding_sha256")
                .await
                .is_none()
        );
        for revision in store.get_nodes_by_label("PaperRevision").await {
            assert!(
                store
                    .get_node_property_string(revision, "embedding_sha256")
                    .await
                    .is_some()
            );
        }
    }

    #[tokio::test]
    async fn extraction_found_in_uses_revision_scoped_section() {
        use da_domain::entity::EntityType;
        use da_ports::extractor::ExtractedEntity;

        let store = MockGraphStore::new();
        let ingest = crate::ingest::IngestUseCase::materializer_only_with_clock(
            Arc::new(store.clone()),
            Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        );
        let req = request(5, 2);
        let context = req.context();
        ingest
            .materialize_parsed_with_context(
                &req.parsed,
                vec![0.3, 0.4],
                None,
                Some("arxiv".into()),
                None,
                da_domain::PdfTriageStatus::NotConfigured,
                Some(&context),
            )
            .await
            .unwrap();
        let extraction = crate::extraction::ExtractionUseCase::materializer_only_with_clock(
            Arc::new(store.clone()),
            Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        );
        let out = extraction
            .materialize_extracted_with_context(
                &req.parsed,
                vec![ExtractedEntity {
                    label: "GEPA".into(),
                    entity_type: EntityType::Method,
                    section_title: "Section 1".into(),
                    section_ordinal: Some(1),
                    char_start: 0,
                    char_end: 4,
                    surface: "GEPA".into(),
                }],
                Some(&context),
            )
            .await
            .unwrap();
        assert_eq!(out.found_in_edges, 1);
        let section_vid = context.section_vid(1).unwrap();
        let section = store
            .find_node_by_string_property("Section", "vid", &section_vid)
            .await
            .unwrap();
        let entity = store.get_nodes_by_label("Entity").await[0];
        assert_eq!(
            store
                .get_edges_between(entity, section, "foundIn")
                .await
                .len(),
            1
        );
        assert!(
            store
                .find_node_by_string_property("Section", "vid", "vid:section:2506.07853:1")
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn replay_creates_no_duplicate_nodes_or_edges() {
        let store = MockGraphStore::new();
        let uc = use_case(store.clone());
        uc.materialize(&request(1, 2)).await.unwrap();
        uc.materialize(&request(5, 3)).await.unwrap();
        let nodes_before = store.node_count_total();
        let edges_before = store.edge_count_total();

        let replay1 = uc.materialize(&request(1, 2)).await.unwrap();
        let replay5 = uc.materialize(&request(5, 3)).await.unwrap();
        assert_eq!(replay1.nodes_created + replay5.nodes_created, 0);
        assert_eq!(replay1.edges_created + replay5.edges_created, 0);
        assert_eq!(store.node_count_total(), nodes_before);
        assert_eq!(store.edge_count_total(), edges_before);
        assert!(replay1.nodes_reused > 0 && replay5.nodes_reused > 0);
    }

    #[tokio::test]
    async fn observed_gap_links_nearest_lower_revision() {
        let store = MockGraphStore::new();
        let uc = use_case(store);
        uc.materialize(&request(1, 1)).await.unwrap();
        let v5 = uc.materialize(&request(5, 1)).await.unwrap();
        assert_eq!(v5.predecessor_revision, Some(1));
    }

    #[tokio::test]
    async fn reverse_load_order_still_links_newer_to_older() {
        let store = MockGraphStore::new();
        let uc = use_case(store.clone());
        uc.materialize(&request(5, 1)).await.unwrap();
        uc.materialize(&request(1, 1)).await.unwrap();
        let v1 = paper_revision_vid("arxiv", "2506.07853", 1).unwrap();
        let v5 = paper_revision_vid("arxiv", "2506.07853", 5).unwrap();
        let v1_node = store
            .find_node_by_string_property("PaperRevision", "vid", &v1)
            .await
            .unwrap();
        let v5_node = store
            .find_node_by_string_property("PaperRevision", "vid", &v5)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_edges_between(v5_node, v1_node, bibliographic::SUPERSEDES)
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn inserting_intermediate_revision_invalidates_old_direct_link() {
        let store = MockGraphStore::new();
        let uc = use_case(store.clone());
        uc.materialize(&request(1, 1)).await.unwrap();
        uc.materialize(&request(5, 1)).await.unwrap();
        uc.materialize(&request(3, 1)).await.unwrap();
        let node = |revision| {
            let store = store.clone();
            async move {
                let vid = paper_revision_vid("arxiv", "2506.07853", revision).unwrap();
                store
                    .find_node_by_string_property("PaperRevision", "vid", &vid)
                    .await
                    .unwrap()
            }
        };
        let v1 = node(1).await;
        let v3 = node(3).await;
        let v5 = node(5).await;
        let direct = store
            .get_edges_between(v5, v1, bibliographic::SUPERSEDES)
            .await;
        assert_eq!(direct.len(), 1);
        assert!(
            store
                .get_edge_property_string(direct[0].0, "invalid_at")
                .await
                .is_some()
        );
        assert_eq!(
            store
                .get_edges_between(v5, v3, bibliographic::SUPERSEDES)
                .await
                .len(),
            1
        );
        assert_eq!(
            store
                .get_edges_between(v3, v1, bibliographic::SUPERSEDES)
                .await
                .len(),
            1
        );
    }
}
