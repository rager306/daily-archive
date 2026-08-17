//! Idempotent FigureCandidate graph materializer (M400 S01, ADR-051).
//!
//! Application use case over [`DirectGraphStore`]: materializes a validated,
//! revision-bound [`FigureCandidate`] as a non-authoritative DATA node with
//! exact section lineage. Candidate evidence is never graph truth (D127 /
//! ADR-053): `authoritative` and `import_eligible` are hard-locked to `false`.
//!
//! Identity contract:
//! - the candidate VID is recomputed from its revision-bound lineage and must
//!   match the stored VID (`FigureCandidate::validate`);
//! - the PaperRevision and revision-scoped Section the candidate grounds in
//!   must already exist in the graph (grounding targets are never invented);
//! - an identical replay is idempotent: applied=false, unchanged=true, and
//!   zero node/edge writes; a replay with divergent properties is rejected
//!   (Conflict) instead of silently overwriting the deterministic identity.

use std::sync::Arc;

use da_domain::VisualArtifactKind;
use da_domain::figure_candidate::{FigureCandidate, FigureCandidateError};
use da_domain::relation::structure;
use da_ports::graph_store::DirectGraphStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Wire value for `VisualArtifactKind` on the FigureCandidate node.
fn kind_wire(kind: VisualArtifactKind) -> &'static str {
    match kind {
        VisualArtifactKind::Figure => "figure",
        VisualArtifactKind::Table => "table",
    }
}

/// Deterministic replay-outcome diagnostic written by the materializer.
pub const DIAGNOSTIC_APPLIED: &str = "applied";
/// Diagnostic returned for an idempotent replay of an identical candidate.
pub const DIAGNOSTIC_UNCHANGED_REPLAY: &str = "unchanged:replay";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureMaterializationResult {
    /// Deterministic candidate VID written (or already present).
    pub candidate_vid: String,
    pub nodes_created: usize,
    pub nodes_reused: usize,
    pub edges_created: usize,
    pub edges_reused: usize,
    /// True when this run wrote the candidate (first application).
    pub applied: bool,
    /// True when the candidate already existed with identical lineage
    /// properties (idempotent replay).
    pub unchanged: bool,
    /// Hard-locked false: candidate evidence is never import-eligible (D127).
    pub import_eligible: bool,
    /// `applied` on first write, `unchanged:replay` on identical replay.
    pub materialization_diagnostics: String,
}

#[derive(Debug, Error)]
pub enum FigureMaterializationError {
    #[error("invalid figure candidate: {0}")]
    Invalid(String),
    #[error("revision {0} is not materialized in the graph")]
    MissingRevision(String),
    #[error("section {0} is not materialized in the graph")]
    MissingSection(String),
    #[error(
        "section {section} does not belong to revision {revision}: grounding lineage is broken"
    )]
    SectionRevisionMismatch { section: String, revision: String },
    #[error(
        "figure candidate {0} is already materialized with divergent properties; refusing to overwrite deterministic identity"
    )]
    Conflict(String),
    #[error("graph write failed during {operation}: {source}")]
    Graph {
        operation: &'static str,
        #[source]
        source: kg_storage::GraphStoreError,
    },
}

impl From<FigureCandidateError> for FigureMaterializationError {
    fn from(error: FigureCandidateError) -> Self {
        Self::Invalid(error.to_string())
    }
}

pub struct FigureCandidateMaterializer {
    graph_store: Arc<dyn DirectGraphStore>,
}

impl FigureCandidateMaterializer {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Materialize one validated figure candidate as non-authoritative graph
    /// evidence. Idempotent: an identical replay applies zero nodes/edges and
    /// reports `unchanged` with positive reuse evidence.
    pub async fn materialize(
        &self,
        candidate: &FigureCandidate,
    ) -> Result<FigureMaterializationResult, FigureMaterializationError> {
        candidate.validate()?;

        // Grounding identity must already exist in the graph: we never invent
        // the revision or the revision-scoped section the candidate grounds in.
        let revision_node = self
            .graph_store
            .find_node_by_string_property("PaperRevision", "vid", &candidate.revision_vid)
            .await
            .ok_or_else(|| {
                FigureMaterializationError::MissingRevision(candidate.revision_vid.clone())
            })?;
        let section_node = self
            .graph_store
            .find_node_by_string_property("Section", "vid", &candidate.section_vid)
            .await
            .ok_or_else(|| {
                FigureMaterializationError::MissingSection(candidate.section_vid.clone())
            })?;
        let section_work_vid = self
            .graph_store
            .get_node_property_string(section_node, "work_vid")
            .await;
        if section_work_vid.as_deref() != Some(candidate.revision_vid.as_str()) {
            return Err(FigureMaterializationError::SectionRevisionMismatch {
                section: candidate.section_vid.clone(),
                revision: candidate.revision_vid.clone(),
            });
        }

        let mut result = FigureMaterializationResult {
            candidate_vid: candidate.vid.clone(),
            nodes_created: 0,
            nodes_reused: 0,
            edges_created: 0,
            edges_reused: 0,
            applied: false,
            unchanged: false,
            import_eligible: false,
            materialization_diagnostics: DIAGNOSTIC_APPLIED.to_string(),
        };

        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property("FigureCandidate", "vid", &candidate.vid)
            .await
        {
            if !self.node_matches(existing, candidate).await {
                return Err(FigureMaterializationError::Conflict(candidate.vid.clone()));
            }
            result.nodes_reused += 1;
            // Edges are part of the deterministic materialization: an
            // unchanged node must also keep its revision/section edges.
            result.edges_reused += self
                .reuse_edges(revision_node, existing, section_node, &mut result)
                .await;
            result.applied = false;
            result.unchanged = true;
            result.materialization_diagnostics = DIAGNOSTIC_UNCHANGED_REPLAY.to_string();
            tracing::info!(
                candidate_vid = %candidate.vid,
                revision_vid = %candidate.revision_vid,
                section_vid = %candidate.section_vid,
                applied = false,
                unchanged = true,
                "figure candidate replay unchanged"
            );
            return Ok(result);
        }

        let candidate_node = self
            .graph_store
            .create_node("FigureCandidate")
            .await
            .map_err(|source| FigureMaterializationError::Graph {
                operation: "create FigureCandidate node",
                source,
            })?;
        result.nodes_created += 1;
        self.write_candidate_properties(candidate_node, candidate, DIAGNOSTIC_APPLIED)
            .await?;
        self.ensure_edge(
            revision_node,
            candidate_node,
            structure::HAS_PART,
            &mut result,
        )
        .await?;
        self.ensure_edge(
            candidate_node,
            section_node,
            structure::GROUNDED_IN,
            &mut result,
        )
        .await?;
        result.applied = true;
        result.materialization_diagnostics = DIAGNOSTIC_APPLIED.to_string();
        tracing::info!(
            candidate_vid = %candidate.vid,
            revision_vid = %candidate.revision_vid,
            section_vid = %candidate.section_vid,
            applied = true,
            unchanged = false,
            "figure candidate materialized"
        );
        Ok(result)
    }

    /// Reuse existing candidate edges, counting them into `edges_reused`.
    /// Returns the number of edges found (both expected edges).
    async fn reuse_edges(
        &self,
        revision_node: u64,
        candidate_node: u64,
        section_node: u64,
        result: &mut FigureMaterializationResult,
    ) -> usize {
        let mut reused = 0;
        if !self
            .graph_store
            .get_edges_between(revision_node, candidate_node, structure::HAS_PART)
            .await
            .is_empty()
        {
            reused += 1;
        }
        if !self
            .graph_store
            .get_edges_between(candidate_node, section_node, structure::GROUNDED_IN)
            .await
            .is_empty()
        {
            reused += 1;
        }
        result.edges_reused += reused;
        reused
    }

    /// Write every deterministic candidate property (lineage, section, schema,
    /// authority locks, replay diagnostic) onto the node.
    async fn write_candidate_properties(
        &self,
        node: u64,
        candidate: &FigureCandidate,
        diagnostic: &str,
    ) -> Result<(), FigureMaterializationError> {
        self.set_string(node, "vid", &candidate.vid).await?;
        self.set_string(node, "revision_vid", &candidate.revision_vid)
            .await?;
        self.set_string(node, "artifact_id", &candidate.artifact_id)
            .await?;
        self.set_string(node, "kind", kind_wire(candidate.kind))
            .await?;
        self.set_string(node, "detector_name", &candidate.detector_name)
            .await?;
        self.set_string(
            node,
            "detector_fingerprint",
            &candidate.detector_fingerprint,
        )
        .await?;
        self.set_string(
            node,
            "detector_metadata_sha256",
            &candidate.detector_metadata_sha256,
        )
        .await?;
        self.set_string(node, "source_pdf_sha256", &candidate.source_pdf_sha256)
            .await?;
        self.set_string(node, "image_sha256", &candidate.image_sha256)
            .await?;
        self.set_string(node, "asset_path", &candidate.asset_path)
            .await?;
        self.set_int(node, "page_index", candidate.page_index as i64)
            .await?;
        self.set_string(node, "section_vid", &candidate.section_vid)
            .await?;
        self.set_string(node, "section_title", &candidate.section_title)
            .await?;
        self.set_int(node, "section_ordinal", candidate.section_ordinal as i64)
            .await?;
        self.set_string(node, "caption", &candidate.caption).await?;
        self.set_string(
            node,
            "candidate_metadata_sha256",
            &candidate.candidate_metadata_sha256,
        )
        .await?;
        // Authority locks: candidate evidence is never graph truth (D127).
        self.set_bool(node, "authoritative", false).await?;
        self.set_bool(node, "import_eligible", false).await?;
        self.set_int(node, "schema_version", candidate.schema_version as i64)
            .await?;
        self.set_string(node, "materialization_diagnostics", diagnostic)
            .await?;
        Ok(())
    }

    /// Deterministic replay comparison: every lineage property on the existing
    /// node must equal the candidate. The diagnostic property is excluded — it
    /// records the first-application outcome and is not part of identity.
    async fn node_matches(&self, node: u64, candidate: &FigureCandidate) -> bool {
        let string_fields: &[(&str, &str)] = &[
            ("vid", &candidate.vid),
            ("revision_vid", &candidate.revision_vid),
            ("artifact_id", &candidate.artifact_id),
            ("kind", kind_wire(candidate.kind)),
            ("detector_name", &candidate.detector_name),
            ("detector_fingerprint", &candidate.detector_fingerprint),
            (
                "detector_metadata_sha256",
                &candidate.detector_metadata_sha256,
            ),
            ("source_pdf_sha256", &candidate.source_pdf_sha256),
            ("image_sha256", &candidate.image_sha256),
            ("asset_path", &candidate.asset_path),
            ("section_vid", &candidate.section_vid),
            ("section_title", &candidate.section_title),
            ("caption", &candidate.caption),
            (
                "candidate_metadata_sha256",
                &candidate.candidate_metadata_sha256,
            ),
        ];
        for (key, expected) in string_fields {
            if self
                .graph_store
                .get_node_property_string(node, key)
                .await
                .as_deref()
                != Some(*expected)
            {
                return false;
            }
        }
        let int_fields: &[(&str, i64)] = &[
            ("page_index", candidate.page_index as i64),
            ("section_ordinal", candidate.section_ordinal as i64),
            ("schema_version", candidate.schema_version as i64),
        ];
        for (key, expected) in int_fields {
            if self.graph_store.get_node_property_int(node, key).await != Some(*expected) {
                return false;
            }
        }
        let bool_fields: &[(&str, bool)] = &[("authoritative", false), ("import_eligible", false)];
        for (key, expected) in bool_fields {
            if self.graph_store.get_node_property_bool(node, key).await != Some(*expected) {
                return false;
            }
        }
        true
    }

    async fn ensure_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &'static str,
        result: &mut FigureMaterializationResult,
    ) -> Result<(), FigureMaterializationError> {
        if !self
            .graph_store
            .get_edges_between(source, target, edge_type)
            .await
            .is_empty()
        {
            result.edges_reused += 1;
            return Ok(());
        }
        self.graph_store
            .create_edge(source, target, edge_type)
            .await
            .map_err(|source| FigureMaterializationError::Graph {
                operation: "create structural edge",
                source,
            })?;
        result.edges_created += 1;
        Ok(())
    }

    async fn set_string(
        &self,
        node: u64,
        key: &'static str,
        value: &str,
    ) -> Result<(), FigureMaterializationError> {
        self.graph_store
            .set_node_property_string(node, key, value.to_string())
            .await
            .map_err(|source| FigureMaterializationError::Graph {
                operation: "set string property",
                source,
            })
    }

    async fn set_int(
        &self,
        node: u64,
        key: &'static str,
        value: i64,
    ) -> Result<(), FigureMaterializationError> {
        self.graph_store
            .set_node_property_int(node, key, value)
            .await
            .map_err(|source| FigureMaterializationError::Graph {
                operation: "set integer property",
                source,
            })
    }

    async fn set_bool(
        &self,
        node: u64,
        key: &'static str,
        value: bool,
    ) -> Result<(), FigureMaterializationError> {
        self.graph_store
            .set_node_property_bool(node, key, value)
            .await
            .map_err(|source| FigureMaterializationError::Graph {
                operation: "set boolean property",
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::figure_candidate::figure_candidate_vid;
    use da_domain::vid::revision_section_vid;
    use kg_storage::MockGraphStore;

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    /// Seed the graph with a PaperRevision and its revision-scoped Sections
    /// (the grounding targets a candidate must bind to).
    async fn seed_revision(store: &MockGraphStore, revision_vid: &str, section_count: u32) {
        let revision = store
            .create_node("PaperRevision")
            .await
            .expect("create revision");
        store
            .set_node_property_string(revision, "vid", revision_vid.to_string())
            .await
            .expect("revision vid");
        for ordinal in 1..=section_count {
            let section_vid = revision_section_vid(revision_vid, ordinal as usize).unwrap();
            let section = store.create_node("Section").await.expect("create section");
            store
                .set_node_property_string(section, "vid", section_vid.clone())
                .await
                .expect("section vid");
            store
                .set_node_property_string(section, "work_vid", revision_vid.to_string())
                .await
                .expect("section work_vid");
            store
                .set_node_property_string(section, "title", format!("Section {ordinal}"))
                .await
                .expect("section title");
        }
    }

    fn candidate(revision_vid: &str, section_ordinal: u32) -> FigureCandidate {
        let section_vid = revision_section_vid(revision_vid, section_ordinal as usize).unwrap();
        let vid = figure_candidate_vid(
            revision_vid,
            "Figure3",
            "pdffigures2-0.0.12",
            &sha(7),
            &section_vid,
        )
        .unwrap();
        FigureCandidate {
            vid,
            revision_vid: revision_vid.into(),
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            detector_metadata_sha256: sha(3),
            source_pdf_sha256: sha(1),
            image_sha256: sha(7),
            asset_path: "figures/Figure3.png".into(),
            page_index: 3,
            section_vid,
            section_title: "GEPA System".into(),
            section_ordinal,
            caption: "Figure 3: Overview of the GEPA system.".into(),
            candidate_metadata_sha256: sha(4),
            authoritative: false,
            import_eligible: false,
            schema_version: da_domain::figure_candidate::FIGURE_CANDIDATE_SCHEMA_VERSION,
            materialization_diagnostics: None,
        }
    }

    fn materializer(store: MockGraphStore) -> FigureCandidateMaterializer {
        FigureCandidateMaterializer::new(Arc::new(store))
    }

    #[tokio::test]
    async fn contract_rejects_unvalidated_or_ungrounded_candidates() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        // Candidate whose lineage does not validate (bad image hash).
        let mut bad = candidate(&sha(10), 2);
        bad.image_sha256 = "not-sha".into();
        assert!(matches!(
            uc.materialize(&bad).await,
            Err(FigureMaterializationError::Invalid(_))
        ));
        // Missing revision target.
        let missing_revision = candidate(&sha(11), 2);
        assert!(matches!(
            uc.materialize(&missing_revision).await,
            Err(FigureMaterializationError::MissingRevision(_))
        ));
        // Revision exists but section does not.
        seed_revision(&store, &sha(12), 2).await;
        let missing_section = candidate(&sha(12), 5);
        assert!(matches!(
            uc.materialize(&missing_section).await,
            Err(FigureMaterializationError::MissingSection(_))
        ));
    }

    #[tokio::test]
    async fn rejects_section_owned_by_another_revision() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        seed_revision(&store, &sha(20), 3).await;
        seed_revision(&store, &sha(21), 3).await;
        // Candidate bound to revision 20 but section_vid derived from revision 21:
        // Section 2 of rev21 exists in the graph, yet does not belong to rev20.
        let other_section_vid = revision_section_vid(&sha(21), 2).unwrap();
        let mut candidate = candidate(&sha(20), 2);
        candidate.section_vid = other_section_vid;
        candidate.vid = figure_candidate_vid(
            &sha(20),
            "Figure3",
            "pdffigures2-0.0.12",
            &sha(7),
            &candidate.section_vid,
        )
        .unwrap();
        assert!(matches!(
            uc.materialize(&candidate).await,
            Err(FigureMaterializationError::SectionRevisionMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn materializes_node_with_revision_and_section_lineage_and_locks() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        let revision_vid = sha(30);
        seed_revision(&store, &revision_vid, 3).await;
        let candidate = candidate(&revision_vid, 2);
        let result = uc.materialize(&candidate).await.unwrap();

        assert!(result.applied && !result.unchanged);
        assert_eq!(result.nodes_created, 1);
        assert_eq!(result.edges_created, 2);
        assert!(!result.import_eligible);
        assert_eq!(result.materialization_diagnostics, DIAGNOSTIC_APPLIED);

        let node = store
            .find_node_by_string_property("FigureCandidate", "vid", &candidate.vid)
            .await
            .unwrap();
        assert_eq!(
            store.get_node_property_string(node, "revision_vid").await,
            Some(revision_vid.clone())
        );
        assert_eq!(
            store.get_node_property_string(node, "section_vid").await,
            Some(candidate.section_vid.clone())
        );
        assert_eq!(
            store.get_node_property_string(node, "kind").await,
            Some("figure".into())
        );
        assert_eq!(
            store.get_node_property_string(node, "section_title").await,
            Some("GEPA System".into())
        );
        assert_eq!(
            store
                .get_node_property_string(node, "materialization_diagnostics")
                .await,
            Some(DIAGNOSTIC_APPLIED.into())
        );
        assert_eq!(
            store.get_node_property_bool(node, "authoritative").await,
            Some(false)
        );
        assert_eq!(
            store.get_node_property_bool(node, "import_eligible").await,
            Some(false)
        );
        assert_eq!(
            store.get_node_property_int(node, "schema_version").await,
            Some(1)
        );

        // Lineage edges: revision owns the candidate; candidate is grounded in
        // the revision-scoped section.
        let revision_node = store
            .find_node_by_string_property("PaperRevision", "vid", &revision_vid)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_edges_between(revision_node, node, structure::HAS_PART)
                .await
                .len(),
            1
        );
        let section_node = store
            .find_node_by_string_property("Section", "vid", &candidate.section_vid)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_edges_between(node, section_node, structure::GROUNDED_IN)
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn identical_replay_is_idempotent_with_unchanged_positive_evidence() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        let revision_vid = sha(40);
        seed_revision(&store, &revision_vid, 3).await;
        let candidate = candidate(&revision_vid, 2);

        uc.materialize(&candidate).await.unwrap();
        let nodes_before = store.node_count_total();
        let edges_before = store.edge_count_total();

        let replay = uc.materialize(&candidate).await.unwrap();
        assert!(!replay.applied && replay.unchanged);
        assert_eq!(replay.nodes_created, 0);
        assert_eq!(replay.edges_created, 0);
        assert!(replay.nodes_reused >= 1);
        assert!(replay.edges_reused >= 2);
        assert_eq!(
            replay.materialization_diagnostics,
            DIAGNOSTIC_UNCHANGED_REPLAY
        );
        assert_eq!(store.node_count_total(), nodes_before);
        assert_eq!(store.edge_count_total(), edges_before);

        // The node still carries first-application diagnostics.
        let node = store
            .find_node_by_string_property("FigureCandidate", "vid", &candidate.vid)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_node_property_string(node, "materialization_diagnostics")
                .await,
            Some(DIAGNOSTIC_APPLIED.into())
        );
    }

    #[tokio::test]
    async fn divergent_replay_is_rejected_as_conflict() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        let revision_vid = sha(50);
        seed_revision(&store, &revision_vid, 3).await;
        let candidate = candidate(&revision_vid, 2);
        uc.materialize(&candidate).await.unwrap();

        // Same VID but divergent caption (tampered lineage payload).
        let mut tampered = candidate.clone();
        tampered.caption = "Figure 3: rewritten caption.".into();
        tampered.candidate_metadata_sha256 = sha(99);
        assert!(matches!(
            uc.materialize(&tampered).await,
            Err(FigureMaterializationError::Conflict(_))
        ));
        // The stored node must be untouched.
        let node = store
            .find_node_by_string_property("FigureCandidate", "vid", &candidate.vid)
            .await
            .unwrap();
        assert_eq!(
            store.get_node_property_string(node, "caption").await,
            Some(candidate.caption)
        );
    }

    #[tokio::test]
    async fn two_revisions_produce_disjoint_candidates() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        let revision_a = sha(60);
        let revision_b = sha(61);
        seed_revision(&store, &revision_a, 3).await;
        seed_revision(&store, &revision_b, 3).await;
        let a = candidate(&revision_a, 2);
        let b = candidate(&revision_b, 2);
        assert_ne!(a.vid, b.vid, "revision-bound identity is disjoint");
        uc.materialize(&a).await.unwrap();
        uc.materialize(&b).await.unwrap();
        assert_eq!(store.get_nodes_by_label("FigureCandidate").await.len(), 2);
        let nodes = store.get_nodes_by_label("FigureCandidate").await;
        let mut revisions = Vec::new();
        for node in nodes {
            revisions.push(
                store
                    .get_node_property_string(node, "revision_vid")
                    .await
                    .unwrap(),
            );
        }
        revisions.sort();
        assert_eq!(revisions, vec![revision_a, revision_b]);
    }

    #[tokio::test]
    async fn result_roundtrips_and_authority_locks_stay_false() {
        let store = MockGraphStore::new();
        let uc = materializer(store.clone());
        let revision_vid = sha(70);
        seed_revision(&store, &revision_vid, 3).await;
        let result = uc.materialize(&candidate(&revision_vid, 2)).await.unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let decoded: FigureMaterializationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, result);
        assert!(!decoded.import_eligible);
    }
}
