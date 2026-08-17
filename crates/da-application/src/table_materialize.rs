//! Idempotent TableCandidate/TableCell graph materializer (M400 S02 T03).
//!
//! Application use case over [`DirectGraphStore`]: materializes a validated,
//! revision-bound [`TableCandidate`] — together with its structural
//! [`TableCellCandidate`] nodes and typed [`TableConflictEvidence`] conflict
//! nodes — as non-authoritative DATA. Candidate evidence is never graph truth
//! (D127 / ADR-053): `authoritative` and `import_eligible` are hard-locked to
//! `false` on every node.
//!
//! Graph wiring:
//! - `PaperRevision -[:hasPart]-> TableCandidate` (revision owns the candidate)
//! - `TableCandidate -[:groundedIn]-> Section` (revision-scoped section lineage)
//! - `TableCandidate -[:hasPart]-> TableCell` (structural TEI cells, row/column/
//!   span/digest evidence — slice demo)
//! - `TableCandidate -[:CONFLICTS_OVER]-> TableConflict` (typed visual-versus-TEI
//!   disagreements, ADR-047 conflict plane)
//!
//! Identity contract (mirrors `FigureCandidateMaterializer`, M400 S01):
//! - the candidate VID is recomputed from its revision-bound lineage and must
//!   match the stored VID ([`TableCandidate::validate`]);
//! - the PaperRevision and revision-scoped Section the candidate grounds in
//!   must already exist in the graph (grounding targets are never invented);
//! - an identical replay is idempotent: `applied=false`, `unchanged=true`, and
//!   zero node/edge writes; a replay with divergent properties — or a partially
//!   written candidate (missing cell/conflict/edge) — is rejected (Conflict)
//!   instead of silently overwriting the deterministic identity.
//!
//! This module also owns the application-boundary conversion from the ports
//! wire type ([`da_ports::parser::ExtractedTable`]) to the domain projection
//! ([`StructuralTable`]) — deferred from T02 so da-domain stays dependency-free.

use std::sync::Arc;

use da_domain::relation::{conflict, structure};
use da_domain::table_candidate::{
    TableCandidate, TableCandidateError, TableCellCandidate, TableConflictEvidence,
};
use da_domain::table_fidelity::{StructuralTable, StructuralTableCell};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ExtractedTable;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Deterministic replay-outcome diagnostic written by the materializer.
pub const DIAGNOSTIC_APPLIED: &str = "applied";
/// Diagnostic returned for an idempotent replay of an identical candidate.
pub const DIAGNOSTIC_UNCHANGED_REPLAY: &str = "unchanged:replay";

/// Node label of the materialized candidate.
pub const TABLE_CANDIDATE_LABEL: &str = "TableCandidate";
/// Node label of a materialized structural cell.
pub const TABLE_CELL_LABEL: &str = "TableCell";
/// Node label of a materialized typed conflict evidence record.
pub const TABLE_CONFLICT_LABEL: &str = "TableConflict";

/// Lossless application-boundary conversion from the deterministic TEI table
/// wire type (T01) to the domain projection the fidelity contract consumes
/// (T02). Preserves table id, caption, per-cell row/column/colspan/rowspan/
/// coordinates/source span/digest, and the table source digest.
///
/// Table-level GROBID coordinates are not part of [`StructuralTable`]; they
/// flow separately into [`TableCandidate::assemble`] as `tei_coordinates`.
pub fn structural_table_from_extracted(table: &ExtractedTable) -> StructuralTable {
    let cells = table
        .cells
        .iter()
        .map(|cell| StructuralTableCell {
            row: cell.row,
            column: cell.column,
            colspan: cell.colspan,
            rowspan: cell.rowspan,
            text: cell.text.clone(),
            coordinates: cell.coordinates.clone(),
            span_start: cell.span.map(|span| span.start),
            span_end: cell.span.map(|span| span.end),
            digest: cell.digest.clone(),
        })
        .collect();
    StructuralTable {
        table_id: table.table_id.clone(),
        caption: table.caption.clone(),
        cells,
        source_digest: table.source_digest.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableMaterializationResult {
    /// Deterministic candidate VID written (or already present).
    pub candidate_vid: String,
    pub nodes_created: usize,
    pub nodes_reused: usize,
    pub edges_created: usize,
    pub edges_reused: usize,
    /// Structural cell nodes written by this run.
    pub cells_created: usize,
    /// Structural cell nodes already present and matching.
    pub cells_reused: usize,
    /// Conflict evidence nodes written by this run.
    pub conflicts_created: usize,
    /// Conflict evidence nodes already present and matching.
    pub conflicts_reused: usize,
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
pub enum TableMaterializationError {
    #[error("invalid table candidate: {0}")]
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
        "table candidate {0} is already materialized with divergent properties; refusing to overwrite deterministic identity"
    )]
    Conflict(String),
    #[error("graph write failed during {operation}: {source}")]
    Graph {
        operation: &'static str,
        #[source]
        source: kg_storage::GraphStoreError,
    },
}

impl From<TableCandidateError> for TableMaterializationError {
    fn from(error: TableCandidateError) -> Self {
        Self::Invalid(error.to_string())
    }
}

pub struct TableCandidateMaterializer {
    graph_store: Arc<dyn DirectGraphStore>,
}

impl TableCandidateMaterializer {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Materialize one validated table candidate as non-authoritative graph
    /// evidence. Idempotent: an identical replay applies zero nodes/edges and
    /// reports `unchanged` with positive reuse evidence.
    pub async fn materialize(
        &self,
        candidate: &TableCandidate,
    ) -> Result<TableMaterializationResult, TableMaterializationError> {
        candidate.validate()?;

        // Grounding identity must already exist in the graph: we never invent
        // the revision or the revision-scoped section the candidate grounds in.
        let revision_node = self
            .graph_store
            .find_node_by_string_property("PaperRevision", "vid", &candidate.revision_vid)
            .await
            .ok_or_else(|| {
                TableMaterializationError::MissingRevision(candidate.revision_vid.clone())
            })?;
        let section_node = self
            .graph_store
            .find_node_by_string_property("Section", "vid", &candidate.section_vid)
            .await
            .ok_or_else(|| {
                TableMaterializationError::MissingSection(candidate.section_vid.clone())
            })?;
        let section_work_vid = self
            .graph_store
            .get_node_property_string(section_node, "work_vid")
            .await;
        if section_work_vid.as_deref() != Some(candidate.revision_vid.as_str()) {
            return Err(TableMaterializationError::SectionRevisionMismatch {
                section: candidate.section_vid.clone(),
                revision: candidate.revision_vid.clone(),
            });
        }

        let mut result = TableMaterializationResult {
            candidate_vid: candidate.vid.clone(),
            nodes_created: 0,
            nodes_reused: 0,
            edges_created: 0,
            edges_reused: 0,
            cells_created: 0,
            cells_reused: 0,
            conflicts_created: 0,
            conflicts_reused: 0,
            applied: false,
            unchanged: false,
            import_eligible: false,
            materialization_diagnostics: DIAGNOSTIC_APPLIED.to_string(),
        };

        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
            .await
        {
            // Replay: every deterministic part must already exist and match —
            // a partially written or divergent candidate is a Conflict, never
            // a silent repair of a deterministic identity.
            if !self.candidate_node_matches(existing, candidate).await {
                return Err(TableMaterializationError::Conflict(candidate.vid.clone()));
            }
            result.nodes_reused += 1;

            for cell in &candidate.cells {
                let cell_node = self
                    .graph_store
                    .find_node_by_string_property(TABLE_CELL_LABEL, "vid", &cell.vid)
                    .await
                    .ok_or_else(|| {
                        TableMaterializationError::Conflict(format!(
                            "{}: cell {} is missing on replay",
                            candidate.vid, cell.vid
                        ))
                    })?;
                if !self.cell_node_matches(cell_node, cell).await {
                    return Err(TableMaterializationError::Conflict(format!(
                        "{}: cell {} diverges on replay",
                        candidate.vid, cell.vid
                    )));
                }
                result.cells_reused += 1;
                result.nodes_reused += 1;
            }

            for evidence in &candidate.conflicts {
                let conflict_node = self
                    .graph_store
                    .find_node_by_string_property(TABLE_CONFLICT_LABEL, "vid", &evidence.vid)
                    .await
                    .ok_or_else(|| {
                        TableMaterializationError::Conflict(format!(
                            "{}: conflict {} is missing on replay",
                            candidate.vid, evidence.vid
                        ))
                    })?;
                if !self.conflict_node_matches(conflict_node, evidence).await {
                    return Err(TableMaterializationError::Conflict(format!(
                        "{}: conflict {} diverges on replay",
                        candidate.vid, evidence.vid
                    )));
                }
                result.conflicts_reused += 1;
                result.nodes_reused += 1;
            }

            // Edges are part of the deterministic materialization: an
            // unchanged node must also keep its revision/section/cell/conflict
            // edges.
            self.reuse_edges(
                revision_node,
                existing,
                section_node,
                candidate,
                &mut result,
            )
            .await?;

            result.applied = false;
            result.unchanged = true;
            result.materialization_diagnostics = DIAGNOSTIC_UNCHANGED_REPLAY.to_string();
            tracing::info!(
                candidate_vid = %candidate.vid,
                revision_vid = %candidate.revision_vid,
                section_vid = %candidate.section_vid,
                applied = false,
                unchanged = true,
                cells_reused = result.cells_reused,
                conflicts_reused = result.conflicts_reused,
                "table candidate replay unchanged"
            );
            return Ok(result);
        }

        // First application: candidate + cells + conflicts + edges in one pass.
        // String literals keep the schema_audit_test scanner (create_node
        // call-site audit against node_schemas.yaml) covering these labels.
        let candidate_node = self
            .graph_store
            .create_node("TableCandidate")
            .await
            .map_err(|source| TableMaterializationError::Graph {
                operation: "create TableCandidate node",
                source,
            })?;
        result.nodes_created += 1;
        self.write_candidate_properties(candidate_node, candidate, DIAGNOSTIC_APPLIED)
            .await?;

        for cell in &candidate.cells {
            let cell_node = self
                .graph_store
                .create_node("TableCell")
                .await
                .map_err(|source| TableMaterializationError::Graph {
                    operation: "create TableCell node",
                    source,
                })?;
            result.cells_created += 1;
            result.nodes_created += 1;
            self.write_cell_properties(cell_node, cell).await?;
            self.ensure_edge(candidate_node, cell_node, structure::HAS_PART, &mut result)
                .await?;
        }

        for evidence in &candidate.conflicts {
            let conflict_node = self
                .graph_store
                .create_node("TableConflict")
                .await
                .map_err(|source| TableMaterializationError::Graph {
                    operation: "create TableConflict node",
                    source,
                })?;
            result.conflicts_created += 1;
            result.nodes_created += 1;
            self.write_conflict_properties(conflict_node, evidence)
                .await?;
            self.ensure_edge(
                candidate_node,
                conflict_node,
                conflict::CONFLICTS_OVER,
                &mut result,
            )
            .await?;
        }

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
            cells_created = result.cells_created,
            conflicts_created = result.conflicts_created,
            "table candidate materialized"
        );
        Ok(result)
    }

    /// Verify every expected edge already exists on replay, counting them
    /// into `edges_reused`. A missing edge means a partial write — Conflict.
    async fn reuse_edges(
        &self,
        revision_node: u64,
        candidate_node: u64,
        section_node: u64,
        candidate: &TableCandidate,
        result: &mut TableMaterializationResult,
    ) -> Result<(), TableMaterializationError> {
        self.reuse_edge(
            revision_node,
            candidate_node,
            structure::HAS_PART,
            "revision hasPart candidate",
            result,
        )
        .await?;
        self.reuse_edge(
            candidate_node,
            section_node,
            structure::GROUNDED_IN,
            "candidate groundedIn section",
            result,
        )
        .await?;
        for cell in &candidate.cells {
            let cell_node = self
                .graph_store
                .find_node_by_string_property(TABLE_CELL_LABEL, "vid", &cell.vid)
                .await
                .ok_or_else(|| {
                    TableMaterializationError::Conflict(format!(
                        "{}: cell {} missing on replay",
                        candidate.vid, cell.vid
                    ))
                })?;
            self.reuse_edge(
                candidate_node,
                cell_node,
                structure::HAS_PART,
                "candidate hasPart cell",
                result,
            )
            .await?;
        }
        for evidence in &candidate.conflicts {
            let conflict_node = self
                .graph_store
                .find_node_by_string_property(TABLE_CONFLICT_LABEL, "vid", &evidence.vid)
                .await
                .ok_or_else(|| {
                    TableMaterializationError::Conflict(format!(
                        "{}: conflict {} missing on replay",
                        candidate.vid, evidence.vid
                    ))
                })?;
            self.reuse_edge(
                candidate_node,
                conflict_node,
                conflict::CONFLICTS_OVER,
                "candidate conflictsOver conflict",
                result,
            )
            .await?;
        }
        Ok(())
    }

    async fn reuse_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &'static str,
        operation: &'static str,
        result: &mut TableMaterializationResult,
    ) -> Result<(), TableMaterializationError> {
        if self
            .graph_store
            .get_edges_between(source, target, edge_type)
            .await
            .is_empty()
        {
            return Err(TableMaterializationError::Conflict(format!(
                "replay is missing edge {edge_type} ({operation})"
            )));
        }
        result.edges_reused += 1;
        Ok(())
    }

    /// Write every deterministic candidate property (lineage, TEI identity,
    /// coverage, schema, authority locks, replay diagnostic) onto the node.
    async fn write_candidate_properties(
        &self,
        node: u64,
        candidate: &TableCandidate,
        diagnostic: &str,
    ) -> Result<(), TableMaterializationError> {
        self.set_string(node, "vid", &candidate.vid).await?;
        self.set_string(node, "revision_vid", &candidate.revision_vid)
            .await?;
        self.set_string(node, "artifact_id", &candidate.artifact_id)
            .await?;
        self.set_string(node, "detector_name", &candidate.detector_name)
            .await?;
        self.set_string(
            node,
            "detector_fingerprint",
            &candidate.detector_fingerprint,
        )
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
        self.set_string(node, "source_pdf_sha256", &candidate.source_pdf_sha256)
            .await?;
        self.set_string(node, "tei_table_id", &candidate.tei_table_id)
            .await?;
        self.set_string(node, "tei_source_digest", &candidate.tei_source_digest)
            .await?;
        self.set_optional_string(
            node,
            "tei_coordinates",
            candidate.tei_coordinates.as_deref(),
        )
        .await?;
        // Flattened positional coverage (graph-task query surface).
        self.set_int(
            node,
            "coverage_visual_cells",
            candidate.coverage.visual_cells as i64,
        )
        .await?;
        self.set_int(
            node,
            "coverage_tei_cells",
            candidate.coverage.tei_cells as i64,
        )
        .await?;
        self.set_int(
            node,
            "coverage_matched",
            candidate.coverage.matched_cells as i64,
        )
        .await?;
        self.set_int(
            node,
            "coverage_unmatched_visual",
            candidate.coverage.unmatched_visual as i64,
        )
        .await?;
        self.set_int(
            node,
            "coverage_unmatched_tei",
            candidate.coverage.unmatched_tei as i64,
        )
        .await?;
        self.set_optional_string(
            node,
            "missingness_kind",
            candidate.missingness_kind.as_deref(),
        )
        .await?;
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

    /// Write every deterministic cell property (grid anchor, TEI evidence,
    /// source span, visual text verbatim, typed fidelity disposition, locks).
    async fn write_cell_properties(
        &self,
        node: u64,
        cell: &TableCellCandidate,
    ) -> Result<(), TableMaterializationError> {
        self.set_string(node, "vid", &cell.vid).await?;
        self.set_string(node, "candidate_vid", &cell.candidate_vid)
            .await?;
        self.set_int(node, "row", cell.row as i64).await?;
        self.set_int(node, "column", cell.column as i64).await?;
        self.set_int(node, "colspan", cell.colspan as i64).await?;
        self.set_int(node, "rowspan", cell.rowspan as i64).await?;
        self.set_string(node, "text", &cell.text).await?;
        self.set_optional_string(node, "coordinates", cell.coordinates.as_deref())
            .await?;
        self.set_optional_int(node, "span_start", cell.span_start.map(i64::from))
            .await?;
        self.set_optional_int(node, "span_end", cell.span_end.map(i64::from))
            .await?;
        self.set_string(node, "digest", &cell.digest).await?;
        self.set_optional_string(node, "visual_text", cell.visual_text.as_deref())
            .await?;
        self.set_optional_string(node, "fidelity_kind", cell.fidelity_kind.as_deref())
            .await?;
        self.set_bool(node, "authoritative", false).await?;
        self.set_bool(node, "import_eligible", false).await?;
        self.set_int(node, "schema_version", cell.schema_version as i64)
            .await?;
        Ok(())
    }

    /// Write every deterministic conflict property (typed kind, grid anchor,
    /// both source texts verbatim, TEI digest lineage, locks).
    async fn write_conflict_properties(
        &self,
        node: u64,
        evidence: &TableConflictEvidence,
    ) -> Result<(), TableMaterializationError> {
        self.set_string(node, "vid", &evidence.vid).await?;
        self.set_string(node, "candidate_vid", &evidence.candidate_vid)
            .await?;
        self.set_string(node, "kind", &evidence.kind).await?;
        self.set_int(node, "row", evidence.row as i64).await?;
        self.set_int(node, "column", evidence.column as i64).await?;
        self.set_string(node, "visual_text", &evidence.visual_text)
            .await?;
        self.set_string(node, "tei_text", &evidence.tei_text)
            .await?;
        self.set_string(node, "tei_digest", &evidence.tei_digest)
            .await?;
        self.set_bool(node, "authoritative", false).await?;
        self.set_bool(node, "import_eligible", false).await?;
        self.set_int(node, "schema_version", evidence.schema_version as i64)
            .await?;
        Ok(())
    }

    /// Deterministic replay comparison for the candidate node. The diagnostic
    /// property is excluded — it records the first-application outcome and is
    /// not part of identity.
    async fn candidate_node_matches(&self, node: u64, candidate: &TableCandidate) -> bool {
        let string_fields: &[(&str, &str)] = &[
            ("vid", &candidate.vid),
            ("revision_vid", &candidate.revision_vid),
            ("artifact_id", &candidate.artifact_id),
            ("detector_name", &candidate.detector_name),
            ("detector_fingerprint", &candidate.detector_fingerprint),
            ("image_sha256", &candidate.image_sha256),
            ("asset_path", &candidate.asset_path),
            ("section_vid", &candidate.section_vid),
            ("section_title", &candidate.section_title),
            ("caption", &candidate.caption),
            ("source_pdf_sha256", &candidate.source_pdf_sha256),
            ("tei_table_id", &candidate.tei_table_id),
            ("tei_source_digest", &candidate.tei_source_digest),
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
        let optional_string_fields: &[(&str, Option<&str>)] = &[
            ("tei_coordinates", candidate.tei_coordinates.as_deref()),
            ("missingness_kind", candidate.missingness_kind.as_deref()),
        ];
        for (key, expected) in optional_string_fields {
            if self
                .graph_store
                .get_node_property_string(node, key)
                .await
                .as_deref()
                != *expected
            {
                return false;
            }
        }
        let int_fields: &[(&str, i64)] = &[
            ("page_index", candidate.page_index as i64),
            ("section_ordinal", candidate.section_ordinal as i64),
            ("schema_version", candidate.schema_version as i64),
            (
                "coverage_visual_cells",
                candidate.coverage.visual_cells as i64,
            ),
            ("coverage_tei_cells", candidate.coverage.tei_cells as i64),
            ("coverage_matched", candidate.coverage.matched_cells as i64),
            (
                "coverage_unmatched_visual",
                candidate.coverage.unmatched_visual as i64,
            ),
            (
                "coverage_unmatched_tei",
                candidate.coverage.unmatched_tei as i64,
            ),
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

    /// Deterministic replay comparison for a cell node.
    async fn cell_node_matches(&self, node: u64, cell: &TableCellCandidate) -> bool {
        let string_fields: &[(&str, &str)] = &[
            ("vid", &cell.vid),
            ("candidate_vid", &cell.candidate_vid),
            ("text", &cell.text),
            ("digest", &cell.digest),
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
        let optional_string_fields: &[(&str, Option<&str>)] = &[
            ("coordinates", cell.coordinates.as_deref()),
            ("visual_text", cell.visual_text.as_deref()),
            ("fidelity_kind", cell.fidelity_kind.as_deref()),
        ];
        for (key, expected) in optional_string_fields {
            if self
                .graph_store
                .get_node_property_string(node, key)
                .await
                .as_deref()
                != *expected
            {
                return false;
            }
        }
        let int_fields: &[(&str, i64)] = &[
            ("row", cell.row as i64),
            ("column", cell.column as i64),
            ("colspan", cell.colspan as i64),
            ("rowspan", cell.rowspan as i64),
            ("schema_version", cell.schema_version as i64),
        ];
        for (key, expected) in int_fields {
            if self.graph_store.get_node_property_int(node, key).await != Some(*expected) {
                return false;
            }
        }
        let optional_int_fields: &[(&str, Option<i64>)] = &[
            ("span_start", cell.span_start.map(i64::from)),
            ("span_end", cell.span_end.map(i64::from)),
        ];
        for (key, expected) in optional_int_fields {
            if self.graph_store.get_node_property_int(node, key).await != *expected {
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

    /// Deterministic replay comparison for a conflict evidence node.
    async fn conflict_node_matches(&self, node: u64, evidence: &TableConflictEvidence) -> bool {
        let string_fields: &[(&str, &str)] = &[
            ("vid", &evidence.vid),
            ("candidate_vid", &evidence.candidate_vid),
            ("kind", &evidence.kind),
            ("visual_text", &evidence.visual_text),
            ("tei_text", &evidence.tei_text),
            ("tei_digest", &evidence.tei_digest),
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
            ("row", evidence.row as i64),
            ("column", evidence.column as i64),
            ("schema_version", evidence.schema_version as i64),
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
        result: &mut TableMaterializationResult,
    ) -> Result<(), TableMaterializationError> {
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
            .map_err(|source| TableMaterializationError::Graph {
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
    ) -> Result<(), TableMaterializationError> {
        self.graph_store
            .set_node_property_string(node, key, value.to_string())
            .await
            .map_err(|source| TableMaterializationError::Graph {
                operation: "set string property",
                source,
            })
    }

    /// Set an optional string property only when present (absent on None).
    async fn set_optional_string(
        &self,
        node: u64,
        key: &'static str,
        value: Option<&str>,
    ) -> Result<(), TableMaterializationError> {
        if let Some(value) = value {
            self.set_string(node, key, value).await?;
        }
        Ok(())
    }

    async fn set_int(
        &self,
        node: u64,
        key: &'static str,
        value: i64,
    ) -> Result<(), TableMaterializationError> {
        self.graph_store
            .set_node_property_int(node, key, value)
            .await
            .map_err(|source| TableMaterializationError::Graph {
                operation: "set integer property",
                source,
            })
    }

    /// Set an optional integer property only when present (absent on None).
    async fn set_optional_int(
        &self,
        node: u64,
        key: &'static str,
        value: Option<i64>,
    ) -> Result<(), TableMaterializationError> {
        if let Some(value) = value {
            self.set_int(node, key, value).await?;
        }
        Ok(())
    }

    async fn set_bool(
        &self,
        node: u64,
        key: &'static str,
        value: bool,
    ) -> Result<(), TableMaterializationError> {
        self.graph_store
            .set_node_property_bool(node, key, value)
            .await
            .map_err(|source| TableMaterializationError::Graph {
                operation: "set boolean property",
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::table_candidate::table_candidate_vid;
    use da_domain::vid::revision_section_vid;

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    fn ports_cell(row: u32, column: u32, text: &str) -> da_ports::parser::TableCell {
        da_ports::parser::TableCell {
            row,
            column,
            colspan: 1,
            rowspan: 1,
            text: text.into(),
            coordinates: None,
            span: Some(da_ports::parser::CharSpan {
                start: row * 100 + column * 10,
                end: row * 100 + column * 10 + text.len() as u32,
            }),
            digest: format!("{row:032x}{column:032x}"),
        }
    }

    fn ports_table(cells: Vec<da_ports::parser::TableCell>) -> ExtractedTable {
        ExtractedTable {
            table_id: "tab_0".into(),
            section_path_id: Some("2".into()),
            section_ordinal: Some(2),
            caption: "Table 3 :".into(),
            coordinates: Some("10,20,500,300;3".into()),
            cells,
            source_digest: sha(7),
            missingness: None,
        }
    }

    #[test]
    fn conversion_preserves_spans_digests_and_identity() {
        let extracted = ports_table(vec![ports_cell(0, 0, "Method"), ports_cell(1, 2, "0.87")]);
        let structural = structural_table_from_extracted(&extracted);
        structural.validate().unwrap();
        assert_eq!(structural.table_id, "tab_0");
        assert_eq!(structural.source_digest, sha(7));
        assert_eq!(structural.cells.len(), 2);
        assert_eq!(structural.cells[1].row, 1);
        assert_eq!(structural.cells[1].column, 2);
        assert_eq!(structural.cells[1].text, "0.87");
        assert_eq!(structural.cells[1].span_start, Some(120));
        assert_eq!(structural.cells[1].span_end, Some(124));
        assert_eq!(
            structural.cells[1].digest,
            format!("{:032x}{:032x}", 1u32, 2u32)
        );
    }

    #[test]
    fn candidate_vid_binds_tei_source_digest_and_revision() {
        let a = table_candidate_vid(
            &sha(10),
            "Table3",
            &sha(7),
            &revision_section_vid(&sha(10), 2).unwrap(),
            "tab_0",
            &sha(7),
        )
        .unwrap();
        let b = table_candidate_vid(
            &sha(10),
            "Table3",
            &sha(7),
            &revision_section_vid(&sha(10), 2).unwrap(),
            "tab_0",
            &sha(8),
        )
        .unwrap();
        assert_ne!(a, b);
    }
}
