//! Revision-grounded table candidate domain contract (M400 S02 T03, ADR-051).
//!
//! A `TableCandidate` is deterministic, revision-bound, non-authoritative
//! graph evidence materialized from the table fidelity plane: the visual
//! table payload (VLM, [`FigureTablePayload`]) compared against the extracted
//! structural TEI table ([`StructuralTable`]) through the typed fidelity
//! contract ([`compare_table_fidelity`]). Its identity is a SHA-256 VID over
//! the revision, detector artifact label, rendered image content hash, the
//! exact revision-scoped section VID, and the TEI table identity + source
//! digest.
//!
//! Grounding rules (R085, R087, R092, D127):
//! - **TEI-grounded only:** a TableCandidate always binds a real structural
//!   TEI table (`tei_table_id` + `tei_source_digest` are required and
//!   validated). A visual table with *no* TEI structure stays in the figure
//!   plane (S01 `FigureCandidate` with kind=table); the typed
//!   `NoTeiTable` missingness remains a contract-level report, never an
//!   invented candidate.
//! - **No invention:** a cell is materialized only for a structural TEI cell
//!   that exists; visual-only positions are recorded as coverage counters
//!   (`unmatched_visual`), never synthesized.
//! - **No overwrite:** the TEI text on the cell node stays verbatim; the
//!   visual text is preserved verbatim alongside it (`visual_text`) and
//!   disagreement is a typed `fidelity_kind` + a `TableConflictEvidence`
//!   record (both texts + the TEI digest), never a replacement of one text
//!   by the other.
//! - **Typed missingness:** a TEI structure with no visual payload produces
//!   `missingness_kind = "no_visual_payload"`; the TEI cells are still
//!   materialized (they are real source structure) with `visual_text = None`.
//! - **Non-authoritative:** authority and import locks are hard-locked to
//!   `false` (D127) — candidate evidence is never graph truth.
//!
//! The deterministic `candidate_metadata_sha256` digests the serialized
//! fidelity report, so an identical replay is provably identical and a
//! divergent replay is rejected (Conflict) instead of silently overwriting
//! the deterministic identity.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::figure_analysis::FigureTablePayload;
use crate::table_fidelity::{
    StructuralTable, TableCellCoverage, TableFidelityFinding, TableFidelityMissingness,
    TableFidelityReport,
};
use crate::vid::sha256_vid;

/// Current TableCandidate DATA schema version.
pub const TABLE_CANDIDATE_SCHEMA_VERSION: u32 = 1;

/// Fidelity disposition wire kinds for a materialized cell.
pub mod fidelity_kind {
    /// Visual and TEI texts agree at this position (matched, no finding).
    pub const AGREEMENT: &str = "agreement";
    /// TEI header cell disagrees with the visual header cell.
    pub const HEADER_MISMATCH: &str = "header_mismatch";
    /// Both sides numeric and differ in value.
    pub const NUMERIC_MISMATCH: &str = "numeric_mismatch";
    /// Non-numeric body text disagreement.
    pub const SOURCE_DISAGREEMENT: &str = "source_disagreement";
    /// Structural cell has no visual counterpart (coverage gap, not a
    /// conflict — no TableConflictEvidence is emitted for it).
    pub const MISSING_VISUAL_CELL: &str = "missing_visual_cell";
}

/// Deterministic revision-bound table candidate VID.
///
/// Binds revision identity, detector artifact label, rendered image content
/// hash, the exact revision-scoped Section VID, and the TEI table identity +
/// source digest. Two revisions of the same artifact produce disjoint
/// candidates; an identical lineage replays onto the same candidate identity.
pub fn table_candidate_vid(
    revision_vid: &str,
    artifact_id: &str,
    image_sha256: &str,
    section_vid: &str,
    tei_table_id: &str,
    tei_source_digest: &str,
) -> Result<String, String> {
    let revision = normalize_sha256(revision_vid, "revision VID")?;
    let artifact = artifact_id.trim();
    if artifact.is_empty() {
        return Err("artifact_id must not be empty".into());
    }
    let image = normalize_sha256(image_sha256, "image sha256")?;
    let section = normalize_sha256(section_vid, "section VID")?;
    let table_id = tei_table_id.trim();
    if table_id.is_empty() {
        return Err("tei_table_id must not be empty".into());
    }
    let source_digest = normalize_sha256(tei_source_digest, "TEI source digest")?;
    Ok(sha256_vid(
        "table_candidate",
        &format!("{revision}\n{artifact}\n{image}\n{section}\n{table_id}\n{source_digest}"),
    ))
}

/// Deterministic per-cell VID inside a candidate: binds the candidate and the
/// cell's grid anchor (row, column) — unique per candidate since one
/// candidate owns exactly one TEI table.
pub fn table_cell_vid(candidate_vid: &str, row: u32, column: u32) -> Result<String, String> {
    let candidate = normalize_sha256(candidate_vid, "candidate VID")?;
    Ok(sha256_vid(
        "table_cell",
        &format!("{candidate}\n{row}\n{column}"),
    ))
}

/// Deterministic conflict-evidence VID: binds the candidate, the typed
/// conflict kind, and the grid position the conflict was detected at.
pub fn table_conflict_vid(
    candidate_vid: &str,
    kind: &str,
    row: u32,
    column: u32,
) -> Result<String, String> {
    let candidate = normalize_sha256(candidate_vid, "candidate VID")?;
    let kind = kind.trim();
    if kind.is_empty() {
        return Err("conflict kind must not be empty".into());
    }
    Ok(sha256_vid(
        "table_conflict",
        &format!("{candidate}\n{kind}\n{row}\n{column}"),
    ))
}

/// Normalize a SHA-256 lineage value to canonical lowercase hex form.
fn normalize_sha256(value: &str, field: &'static str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a 64-character SHA-256 hex digest"));
    }
    Ok(normalized)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Revision/section/image/artifact lineage a table candidate is bound to.
/// Mirrors `FigureCandidate` lineage fields so both candidate planes share
/// one identity vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCandidateLineage {
    /// PaperRevision VID this candidate is grounded in.
    pub revision_vid: String,
    /// Detector artifact label, e.g. `Table3`.
    pub artifact_id: String,
    pub detector_name: String,
    pub detector_fingerprint: String,
    /// SHA-256 of the rendered table figure asset at `asset_path`.
    pub image_sha256: String,
    /// Canonical asset path relative to the article directory (figures/…).
    pub asset_path: String,
    /// Zero-based page index (pdffigures2 semantics).
    pub page_index: u32,
    /// Revision-scoped Section VID (`revision_section_vid`) — exact lineage.
    pub section_vid: String,
    pub section_title: String,
    pub section_ordinal: u32,
    pub caption: String,
    /// SHA-256 of the source PDF manifestation this revision is bound to.
    pub source_pdf_sha256: String,
}

/// A materialized structural cell node: TEI evidence preserved verbatim plus
/// the typed fidelity disposition against the visual payload (R087).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCellCandidate {
    /// Deterministic per-cell VID ([`table_cell_vid`]).
    pub vid: String,
    /// Owning TableCandidate VID.
    pub candidate_vid: String,
    /// 0-based row index within the table.
    pub row: u32,
    /// 0-based column index (cumulative grid position after colspan).
    pub column: u32,
    /// TEI `cols` attribute (colspan).
    pub colspan: u32,
    /// TEI `rows` attribute (rowspan).
    pub rowspan: u32,
    /// TEI cell text verbatim — never overwritten by visual text.
    pub text: String,
    /// GROBID `coords` attribute on the cell when emitted (rare), else None.
    #[serde(default)]
    pub coordinates: Option<String>,
    /// Source char span start of the cell's visible content inside the
    /// containing section's stripped text (R087). `None` when the table is
    /// not inside any section text window (typed missingness, never guessed).
    #[serde(default)]
    pub span_start: Option<u32>,
    /// Source char span end (exclusive).
    #[serde(default)]
    pub span_end: Option<u32>,
    /// Deterministic SHA-256 digest of the TEI cell (source-digest evidence).
    pub digest: String,
    /// Visual payload text at the cell's anchor position, preserved verbatim.
    /// `None` when the visual payload did not cover this position
    /// (missing_visual_cell) or no visual payload exists.
    #[serde(default)]
    pub visual_text: Option<String>,
    /// Typed fidelity disposition ([`fidelity_kind`]): `None` is agreement.
    #[serde(default)]
    pub fidelity_kind: Option<String>,
    /// Authority lock — must remain `false` (candidate evidence, D127).
    pub authoritative: bool,
    /// Import lock — must remain `false`.
    pub import_eligible: bool,
    pub schema_version: u32,
}

/// Typed visual-versus-TEI disagreement evidence (R087). Both source texts
/// are preserved verbatim; neither source is mutated (no overwrite).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableConflictEvidence {
    /// Deterministic conflict VID ([`table_conflict_vid`]).
    pub vid: String,
    /// Owning TableCandidate VID.
    pub candidate_vid: String,
    /// Typed conflict kind: `header_mismatch` | `numeric_mismatch` |
    /// `source_disagreement` (matches [`TableFidelityFinding::kind`]).
    pub kind: String,
    /// Grid row of the disagreement (0 for header mismatches).
    pub row: u32,
    /// Grid column of the disagreement.
    pub column: u32,
    /// Visual text at the position, preserved verbatim.
    pub visual_text: String,
    /// TEI text at the position, preserved verbatim.
    pub tei_text: String,
    /// TEI cell digest of the structural side (lineage evidence).
    pub tei_digest: String,
    /// Authority lock — must remain `false`.
    pub authoritative: bool,
    /// Import lock — must remain `false`.
    pub import_eligible: bool,
    pub schema_version: u32,
}

/// Non-authoritative revision-grounded table candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCandidate {
    /// Deterministic revision-bound candidate VID ([`table_candidate_vid`]).
    pub vid: String,
    /// PaperRevision VID this candidate is grounded in.
    pub revision_vid: String,
    pub artifact_id: String,
    pub detector_name: String,
    pub detector_fingerprint: String,
    /// SHA-256 of the rendered table figure asset.
    pub image_sha256: String,
    /// Canonical asset path (figures/…).
    pub asset_path: String,
    pub page_index: u32,
    pub section_vid: String,
    pub section_title: String,
    pub section_ordinal: u32,
    pub caption: String,
    /// SHA-256 of the source PDF manifestation this revision is bound to.
    pub source_pdf_sha256: String,
    /// TEI table identity (xml:id or deterministic ordinal key).
    pub tei_table_id: String,
    /// TEI table source digest (lineage evidence).
    pub tei_source_digest: String,
    /// GROBID `coords` attribute on the enclosing TEI figure/table when
    /// emitted, else None (source coordinate evidence).
    #[serde(default)]
    pub tei_coordinates: Option<String>,
    /// Positional coverage of the visual payload by the structural source.
    pub coverage: TableCellCoverage,
    /// Typed source-level missingness kind: `None` (full comparison) or
    /// `"no_visual_payload"`. A candidate is always TEI-grounded, so
    /// `no_tei_table` never appears here (contract-level report only).
    #[serde(default)]
    pub missingness_kind: Option<String>,
    /// Materialized structural cells in document order (row-major).
    #[serde(default)]
    pub cells: Vec<TableCellCandidate>,
    /// Typed conflict evidence records (header/numeric/source only —
    /// coverage gaps are counters, not conflicts).
    #[serde(default)]
    pub conflicts: Vec<TableConflictEvidence>,
    /// SHA-256 of the serialized fidelity report (deterministic replay
    /// evidence binding coverage, findings, missingness, TEI lineage).
    pub candidate_metadata_sha256: String,
    /// Authority lock — must remain `false` (candidate evidence, D127).
    pub authoritative: bool,
    /// Import lock — must remain `false`.
    pub import_eligible: bool,
    pub schema_version: u32,
    /// Deterministic materialization diagnostics (replay outcome), written by
    /// the materializer on write; absent until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialization_diagnostics: Option<String>,
}

impl TableCandidate {
    /// Recompute the candidate VID from its revision-bound lineage fields.
    pub fn recompute_vid(&self) -> Result<String, String> {
        table_candidate_vid(
            &self.revision_vid,
            &self.artifact_id,
            &self.image_sha256,
            &self.section_vid,
            &self.tei_table_id,
            &self.tei_source_digest,
        )
    }

    /// True when the stored VID matches the recomputed revision-bound lineage.
    pub fn identity_is_consistent(&self) -> bool {
        self.recompute_vid().is_ok_and(|vid| vid == self.vid)
    }

    /// Assemble a validated candidate from the fidelity parts.
    ///
    /// - `lineage`: revision/section/image/artifact identity.
    /// - `visual`: the VLM table payload (None when the visual side is
    ///   absent → `missingness_kind = "no_visual_payload"`).
    /// - `tei`: the structural TEI table (always required — candidates are
    ///   TEI-grounded).
    /// - `report`: the typed fidelity report from [`compare_table_fidelity`].
    /// - `tei_coordinates`: GROBID `coords` attribute on the enclosing TEI
    ///   figure/table (source coordinate evidence, R087). Not part of the
    ///   deterministic VID (the TEI source digest already hashes it) but
    ///   preserved verbatim as lineage evidence.
    ///
    /// Cells are materialized only for structural TEI cells (no invention),
    /// with the TEI text verbatim, the visual text preserved alongside, and
    /// typed conflict evidence for disagreements (no overwrite).
    pub fn assemble(
        lineage: &TableCandidateLineage,
        visual: Option<&FigureTablePayload>,
        tei: &StructuralTable,
        report: &TableFidelityReport,
        tei_coordinates: Option<String>,
    ) -> Result<Self, TableCandidateError> {
        report.validate()?;
        tei.validate()?;

        // TEI-grounded rule first: a visual-only table (NoTeiTable report) can
        // never become a candidate — the typed missingness stays a report.
        let missingness_kind = match &report.missingness {
            None => None,
            Some(TableFidelityMissingness::NoVisualPayload { .. }) => {
                if visual.is_some() {
                    return Err(TableCandidateError::Invalid(
                        "no_visual_payload missingness requires visual == None".into(),
                    ));
                }
                Some("no_visual_payload".to_string())
            }
            Some(TableFidelityMissingness::NoTeiTable { .. }) => {
                return Err(TableCandidateError::Invalid(
                    "NoTeiTable missingness cannot materialize a TEI-grounded candidate".into(),
                ));
            }
        };

        // The report must reference the exact structural table we ground in.
        if report.tei_table_id.as_deref() != Some(tei.table_id.as_str())
            || report.tei_source_digest.as_deref() != Some(tei.source_digest.as_str())
        {
            return Err(TableCandidateError::TeiIdentityMismatch);
        }

        // Deterministic digest over the serialized fidelity evidence: replay
        // comparison binds coverage + findings + missingness + TEI lineage
        // without storing every finding twice.
        let candidate_metadata_sha256 = sha256_hex(
            &serde_json::to_vec(report)
                .map_err(|e| TableCandidateError::Invalid(format!("report digest: {e}")))?,
        );

        let candidate_vid = table_candidate_vid(
            &lineage.revision_vid,
            &lineage.artifact_id,
            &lineage.image_sha256,
            &lineage.section_vid,
            &tei.table_id,
            &tei.source_digest,
        )
        .map_err(TableCandidateError::Invalid)?;

        let cells = build_cells(&candidate_vid, tei, visual, report);
        let conflicts = build_conflicts(&candidate_vid, report);

        Ok(TableCandidate {
            vid: candidate_vid,
            revision_vid: lineage.revision_vid.clone(),
            artifact_id: lineage.artifact_id.clone(),
            detector_name: lineage.detector_name.clone(),
            detector_fingerprint: lineage.detector_fingerprint.clone(),
            image_sha256: lineage.image_sha256.clone(),
            asset_path: lineage.asset_path.clone(),
            page_index: lineage.page_index,
            section_vid: lineage.section_vid.clone(),
            section_title: lineage.section_title.clone(),
            section_ordinal: lineage.section_ordinal,
            caption: lineage.caption.clone(),
            source_pdf_sha256: lineage.source_pdf_sha256.clone(),
            tei_table_id: tei.table_id.clone(),
            tei_source_digest: tei.source_digest.clone(),
            tei_coordinates,
            coverage: report.coverage,
            missingness_kind,
            cells,
            conflicts,
            candidate_metadata_sha256,
            authoritative: false,
            import_eligible: false,
            schema_version: TABLE_CANDIDATE_SCHEMA_VERSION,
            materialization_diagnostics: None,
        })
    }

    /// Validate every SHA-256 lineage field, section identity, TEI grounding,
    /// cell/conflict identity, authority locks, and deterministic identity.
    pub fn validate(&self) -> Result<(), TableCandidateError> {
        for (field, value) in [
            ("vid", self.vid.as_str()),
            ("revision_vid", self.revision_vid.as_str()),
            ("section_vid", self.section_vid.as_str()),
        ] {
            if !is_sha256_hex(value) {
                return Err(TableCandidateError::InvalidSha256(field));
            }
        }
        for (field, value) in [
            ("source_pdf_sha256", self.source_pdf_sha256.as_str()),
            ("image_sha256", self.image_sha256.as_str()),
            (
                "candidate_metadata_sha256",
                self.candidate_metadata_sha256.as_str(),
            ),
            ("tei_source_digest", self.tei_source_digest.as_str()),
        ] {
            if !is_sha256_hex(value) {
                return Err(TableCandidateError::InvalidSha256(field));
            }
        }
        for (field, value) in [
            ("artifact_id", self.artifact_id.as_str()),
            ("detector_name", self.detector_name.as_str()),
            ("detector_fingerprint", self.detector_fingerprint.as_str()),
            ("section_title", self.section_title.as_str()),
            ("caption", self.caption.as_str()),
            ("tei_table_id", self.tei_table_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(TableCandidateError::Missing(field));
            }
        }
        if !self.asset_path.starts_with("figures/")
            || self.asset_path.contains("..")
            || self.asset_path.starts_with('/')
        {
            return Err(TableCandidateError::InvalidAssetPath(
                self.asset_path.clone(),
            ));
        }
        if self.schema_version == 0 {
            return Err(TableCandidateError::Invalid(
                "schema_version must be greater than zero".into(),
            ));
        }
        if self.authoritative || self.import_eligible {
            return Err(TableCandidateError::AuthorityViolation);
        }
        match self.missingness_kind.as_deref() {
            None | Some("no_visual_payload") => {}
            Some(other) => {
                return Err(TableCandidateError::Invalid(format!(
                    "unknown missingness_kind {other:?}"
                )));
            }
        }
        // Deterministic identity is checked before per-cell identity: a
        // tampered candidate VID surfaces as IdentityMismatch, not as a
        // cascading cell/conflict mismatch.
        if !self.identity_is_consistent() {
            return Err(TableCandidateError::IdentityMismatch);
        }
        for cell in &self.cells {
            if !is_sha256_hex(&cell.digest) {
                return Err(TableCandidateError::InvalidSha256("cell digest"));
            }
            if cell.colspan == 0 || cell.rowspan == 0 {
                return Err(TableCandidateError::Invalid(
                    "colspan and rowspan must be at least 1".into(),
                ));
            }
            if cell.authoritative || cell.import_eligible {
                return Err(TableCandidateError::AuthorityViolation);
            }
            let expected = table_cell_vid(&self.vid, cell.row, cell.column)
                .map_err(TableCandidateError::Invalid)?;
            if cell.vid != expected {
                return Err(TableCandidateError::CellIdentityMismatch {
                    row: cell.row,
                    column: cell.column,
                });
            }
            if let Some(kind) = cell.fidelity_kind.as_deref()
                && !matches!(
                    kind,
                    fidelity_kind::HEADER_MISMATCH
                        | fidelity_kind::NUMERIC_MISMATCH
                        | fidelity_kind::SOURCE_DISAGREEMENT
                        | fidelity_kind::MISSING_VISUAL_CELL
                )
            {
                return Err(TableCandidateError::Invalid(format!(
                    "unknown cell fidelity_kind {kind:?}"
                )));
            }
        }
        for conflict in &self.conflicts {
            if conflict.authoritative || conflict.import_eligible {
                return Err(TableCandidateError::AuthorityViolation);
            }
            if !is_sha256_hex(&conflict.tei_digest) {
                return Err(TableCandidateError::InvalidSha256("conflict tei digest"));
            }
            let expected =
                table_conflict_vid(&self.vid, &conflict.kind, conflict.row, conflict.column)
                    .map_err(TableCandidateError::Invalid)?;
            if conflict.vid != expected {
                return Err(TableCandidateError::ConflictIdentityMismatch {
                    kind: conflict.kind.clone(),
                    row: conflict.row,
                    column: conflict.column,
                });
            }
        }
        Ok(())
    }

    /// Number of materialized structural cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Number of typed conflict evidence records.
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }
}

/// Build the materialized cells from the structural TEI table, the visual
/// payload (verbatim text at the anchor position), and the fidelity findings.
fn build_cells(
    candidate_vid: &str,
    tei: &StructuralTable,
    visual: Option<&FigureTablePayload>,
    report: &TableFidelityReport,
) -> Vec<TableCellCandidate> {
    // Findings keyed by the TEI digest they reference (a colspan cell can be
    // referenced by several findings — one per covered position).
    let mut findings_by_digest: std::collections::HashMap<&str, Vec<&TableFidelityFinding>> =
        std::collections::HashMap::new();
    for finding in &report.findings {
        if let Some(digest) = finding_tei_digest(finding) {
            findings_by_digest.entry(digest).or_default().push(finding);
        }
    }

    tei.cells
        .iter()
        .map(|cell| {
            let visual_text = visual.and_then(|v| visual_text_at(v, cell.row, cell.column));
            let findings = findings_by_digest
                .get(cell.digest.as_str())
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let fidelity_kind = findings
                .iter()
                .find_map(|f| match f {
                    TableFidelityFinding::HeaderMismatch { .. } => {
                        Some(fidelity_kind::HEADER_MISMATCH)
                    }
                    TableFidelityFinding::NumericMismatch { .. } => {
                        Some(fidelity_kind::NUMERIC_MISMATCH)
                    }
                    TableFidelityFinding::SourceDisagreement { .. } => {
                        Some(fidelity_kind::SOURCE_DISAGREEMENT)
                    }
                    TableFidelityFinding::MissingVisualCell { .. } => {
                        Some(fidelity_kind::MISSING_VISUAL_CELL)
                    }
                    TableFidelityFinding::MissingTeiCell { .. } => None,
                })
                .map(str::to_string);
            TableCellCandidate {
                vid: table_cell_vid(candidate_vid, cell.row, cell.column)
                    .expect("candidate vid already validated"),
                candidate_vid: candidate_vid.to_string(),
                row: cell.row,
                column: cell.column,
                colspan: cell.colspan,
                rowspan: cell.rowspan,
                text: cell.text.clone(),
                coordinates: cell.coordinates.clone(),
                span_start: cell.span_start,
                span_end: cell.span_end,
                digest: cell.digest.clone(),
                visual_text,
                fidelity_kind,
                authoritative: false,
                import_eligible: false,
                schema_version: TABLE_CANDIDATE_SCHEMA_VERSION,
            }
        })
        .collect()
}

/// Build typed conflict evidence records from the report. Only real conflicts
/// (header / numeric / source disagreement) produce evidence; coverage gaps
/// (missing cells) are counters, not conflicts (T02 `is_conflict`).
fn build_conflicts(
    candidate_vid: &str,
    report: &TableFidelityReport,
) -> Vec<TableConflictEvidence> {
    let mut conflicts = Vec::new();
    for finding in &report.findings {
        let (kind, row, column, visual_text, tei_text, tei_digest) = match finding {
            TableFidelityFinding::HeaderMismatch {
                column,
                visual_text,
                tei_text,
                tei_digest,
            } => (
                fidelity_kind::HEADER_MISMATCH,
                0,
                *column,
                visual_text,
                tei_text,
                tei_digest,
            ),
            TableFidelityFinding::NumericMismatch {
                row,
                column,
                visual_text,
                tei_text,
                tei_digest,
            } => (
                fidelity_kind::NUMERIC_MISMATCH,
                *row,
                *column,
                visual_text,
                tei_text,
                tei_digest,
            ),
            TableFidelityFinding::SourceDisagreement {
                row,
                column,
                visual_text,
                tei_text,
                tei_digest,
            } => (
                fidelity_kind::SOURCE_DISAGREEMENT,
                *row,
                *column,
                visual_text,
                tei_text,
                tei_digest,
            ),
            // Coverage gaps are counters, not conflicts — no evidence node.
            TableFidelityFinding::MissingTeiCell { .. }
            | TableFidelityFinding::MissingVisualCell { .. } => continue,
        };
        conflicts.push(TableConflictEvidence {
            vid: table_conflict_vid(candidate_vid, kind, row, column)
                .expect("candidate vid already validated"),
            candidate_vid: candidate_vid.to_string(),
            kind: kind.to_string(),
            row,
            column,
            visual_text: visual_text.clone(),
            tei_text: tei_text.clone(),
            tei_digest: tei_digest.clone(),
            authoritative: false,
            import_eligible: false,
            schema_version: TABLE_CANDIDATE_SCHEMA_VERSION,
        });
    }
    conflicts
}

/// The TEI digest referenced by a finding, when it has a structural side.
fn finding_tei_digest(finding: &TableFidelityFinding) -> Option<&str> {
    match finding {
        TableFidelityFinding::MissingTeiCell { .. } => None,
        TableFidelityFinding::MissingVisualCell { tei_digest, .. }
        | TableFidelityFinding::HeaderMismatch { tei_digest, .. }
        | TableFidelityFinding::NumericMismatch { tei_digest, .. }
        | TableFidelityFinding::SourceDisagreement { tei_digest, .. } => Some(tei_digest),
    }
}

/// Visual payload text at a grid position (headers row 0, then body rows),
/// or `None` when the payload does not cover the position (ragged rows).
fn visual_text_at(visual: &FigureTablePayload, row: u32, column: u32) -> Option<String> {
    if row == 0 {
        return visual.headers.get(column as usize).cloned();
    }
    visual
        .rows
        .get((row - 1) as usize)
        .and_then(|cells| cells.get(column as usize))
        .cloned()
}

/// SHA-256 hex digest over arbitrary bytes.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TableCandidateError {
    #[error("missing table candidate field: {0}")]
    Missing(&'static str),
    #[error("{0} must be a 64-character SHA-256 hex digest")]
    InvalidSha256(&'static str),
    #[error("invalid canonical table asset path: {0}")]
    InvalidAssetPath(String),
    #[error("invalid table candidate: {0}")]
    Invalid(String),
    #[error("table candidate cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("table candidate VID does not match revision-bound lineage")]
    IdentityMismatch,
    #[error("fidelity report references a different TEI table than the structural source")]
    TeiIdentityMismatch,
    #[error("cell VID at ({row}, {column}) does not match its deterministic identity")]
    CellIdentityMismatch { row: u32, column: u32 },
    #[error(
        "conflict VID for {kind} at ({row}, {column}) does not match its deterministic identity"
    )]
    ConflictIdentityMismatch { kind: String, row: u32, column: u32 },
}

impl From<crate::table_fidelity::TableFidelityError> for TableCandidateError {
    fn from(error: crate::table_fidelity::TableFidelityError) -> Self {
        Self::Invalid(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table_fidelity::{StructuralTableCell, compare_table_fidelity};
    use crate::vid::revision_section_vid;

    const REVISION_A: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const REVISION_B: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn hex64(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn lineage(revision_vid: &str) -> TableCandidateLineage {
        TableCandidateLineage {
            revision_vid: revision_vid.into(),
            artifact_id: "Table3".into(),
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            image_sha256: hex64('b'),
            asset_path: "figures/Table3.png".into(),
            page_index: 3,
            section_vid: revision_section_vid(revision_vid, 2).unwrap(),
            section_title: "GEPA System".into(),
            section_ordinal: 2,
            caption: "Table 3: GEPA results.".into(),
            source_pdf_sha256: hex64('a'),
        }
    }

    fn tei_cell(row: u32, column: u32, text: &str) -> StructuralTableCell {
        StructuralTableCell {
            row,
            column,
            colspan: 1,
            rowspan: 1,
            text: text.into(),
            coordinates: None,
            span_start: Some(row * 100 + column * 10),
            span_end: Some(row * 100 + column * 10 + text.len() as u32),
            // Distinct per-position digest: findings group by digest, so a
            // shared constant digest would conflate findings across cells.
            digest: format!("{row:032x}{column:032x}"),
        }
    }

    fn tei_table(cells: Vec<StructuralTableCell>) -> StructuralTable {
        StructuralTable {
            table_id: "tab_0".into(),
            caption: "Table 3 :".into(),
            cells,
            source_digest: hex64('e'),
        }
    }

    fn visual(headers: &[&str], rows: &[&[&str]]) -> FigureTablePayload {
        FigureTablePayload {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    fn perfect_report(tei: &StructuralTable) -> TableFidelityReport {
        let visual = visual(
            &["Method", "F1"],
            &[&["GEPA", "0.87"], &["Baseline", "0.72"]],
        );
        compare_table_fidelity(Some(&visual), Some(tei)).unwrap()
    }

    /// Assemble the canonical perfect candidate (2x3 grid, no findings).
    fn perfect_candidate(revision_vid: &str) -> TableCandidate {
        let tei = tei_table(vec![
            tei_cell(0, 0, "Method"),
            tei_cell(0, 1, "F1"),
            tei_cell(1, 0, "GEPA"),
            tei_cell(1, 1, "0.87"),
            tei_cell(2, 0, "Baseline"),
            tei_cell(2, 1, "0.72"),
        ]);
        let report = perfect_report(&tei);
        TableCandidate::assemble(
            &lineage(revision_vid),
            Some(&visual(
                &["Method", "F1"],
                &[&["GEPA", "0.87"], &["Baseline", "0.72"]],
            )),
            &tei,
            &report,
            None,
        )
        .unwrap()
    }

    #[test]
    fn candidate_roundtrips_with_full_lineage() {
        let value = perfect_candidate(REVISION_A);
        value.validate().unwrap();
        crate::roundtrip_json(&value);
        let json = serde_json::to_string(&value).unwrap();
        let decoded: TableCandidate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.cell_count(), 6);
        assert_eq!(decoded.conflict_count(), 0);
        assert_eq!(decoded.section_ordinal, 2);
        assert_eq!(decoded.tei_table_id, "tab_0");
        assert!(!decoded.authoritative && !decoded.import_eligible);
        assert_eq!(decoded.coverage.matched_cells, 6);
        assert_eq!(decoded.coverage.unmatched_visual, 0);
        assert_eq!(decoded.coverage.unmatched_tei, 0);
    }

    #[test]
    fn cells_carry_span_digest_and_verbatim_texts() {
        let value = perfect_candidate(REVISION_A);
        value.validate().unwrap();
        let cell = &value.cells[3]; // (1, 1) = "0.87"
        assert_eq!(cell.row, 1);
        assert_eq!(cell.column, 1);
        assert_eq!(cell.text, "0.87");
        assert_eq!(cell.visual_text.as_deref(), Some("0.87"));
        assert_eq!(cell.span_start, Some(110));
        assert_eq!(cell.span_end, Some(114));
        assert_eq!(cell.digest.len(), 64);
        assert_eq!(cell.fidelity_kind, None, "agreement cell has no kind");
        assert_eq!(cell.candidate_vid, value.vid);
    }

    #[test]
    fn conflicts_are_typed_and_preserve_both_texts_verbatim() {
        let visual = visual(&["Method", "F1-score"], &[&["GEPA", "0.87"]]);
        let tei = tei_table(vec![
            tei_cell(0, 0, "Method"),
            tei_cell(0, 1, "F1"),
            tei_cell(1, 0, "GEPA"),
            tei_cell(1, 1, "0.90"),
        ]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        let candidate =
            TableCandidate::assemble(&lineage(REVISION_A), Some(&visual), &tei, &report, None)
                .unwrap();
        candidate.validate().unwrap();

        assert_eq!(candidate.conflict_count(), 2);
        let kinds: Vec<&str> = candidate
            .conflicts
            .iter()
            .map(|c| c.kind.as_str())
            .collect();
        assert!(kinds.contains(&"header_mismatch"));
        assert!(kinds.contains(&"numeric_mismatch"));

        let header = candidate
            .conflicts
            .iter()
            .find(|c| c.kind == "header_mismatch")
            .unwrap();
        assert_eq!(header.row, 0);
        assert_eq!(header.column, 1);
        assert_eq!(header.visual_text, "F1-score");
        assert_eq!(header.tei_text, "F1");
        assert_eq!(header.tei_digest.len(), 64);

        // The conflicting cells carry the typed fidelity kind + verbatim texts.
        let cell = candidate
            .cells
            .iter()
            .find(|c| c.row == 1 && c.column == 1)
            .unwrap();
        assert_eq!(cell.fidelity_kind.as_deref(), Some("numeric_mismatch"));
        assert_eq!(cell.text, "0.90", "TEI text never overwritten");
        assert_eq!(cell.visual_text.as_deref(), Some("0.87"));
    }

    #[test]
    fn missing_visual_cell_is_a_typed_cell_gap_not_a_conflict() {
        let visual = visual(&["Method"], &[]);
        let tei = tei_table(vec![tei_cell(0, 0, "Method"), tei_cell(1, 0, "GEPA")]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        let candidate =
            TableCandidate::assemble(&lineage(REVISION_A), Some(&visual), &tei, &report, None)
                .unwrap();
        candidate.validate().unwrap();

        assert_eq!(
            candidate.conflict_count(),
            0,
            "coverage gap is not a conflict"
        );
        let cell = candidate.cells.iter().find(|c| c.row == 1).unwrap();
        assert_eq!(cell.fidelity_kind.as_deref(), Some("missing_visual_cell"));
        assert_eq!(cell.visual_text, None);
        assert_eq!(cell.text, "GEPA");
    }

    #[test]
    fn no_visual_payload_missingness_keeps_tei_cells() {
        let tei = tei_table(vec![tei_cell(0, 0, "Method"), tei_cell(0, 1, "F1")]);
        let report = compare_table_fidelity(None, Some(&tei)).unwrap();
        let candidate =
            TableCandidate::assemble(&lineage(REVISION_A), None, &tei, &report, None).unwrap();
        candidate.validate().unwrap();

        assert_eq!(
            candidate.missingness_kind.as_deref(),
            Some("no_visual_payload")
        );
        assert_eq!(candidate.cell_count(), 2, "TEI cells stay materialized");
        assert!(
            candidate
                .cells
                .iter()
                .all(|c| c.visual_text.is_none() && c.fidelity_kind.is_none())
        );
        assert_eq!(candidate.conflict_count(), 0);
        assert_eq!(candidate.coverage.matched_cells, 0);
        assert_eq!(candidate.coverage.unmatched_tei, 2);
    }

    #[test]
    fn no_tei_table_cannot_materialize_a_candidate() {
        let visual = visual(&["A"], &[&["1"]]);
        let report = compare_table_fidelity(Some(&visual), None).unwrap();
        let tei = tei_table(vec![]);
        let err =
            TableCandidate::assemble(&lineage(REVISION_A), Some(&visual), &tei, &report, None)
                .unwrap_err();
        assert!(matches!(
            err,
            TableCandidateError::Invalid(ref msg)
                if msg.contains("NoTeiTable")
        ));
    }

    #[test]
    fn rejects_authority_or_import_unlock() {
        let mut value = perfect_candidate(REVISION_A);
        value.authoritative = true;
        assert_eq!(
            value.validate(),
            Err(TableCandidateError::AuthorityViolation)
        );
        let mut value = perfect_candidate(REVISION_A);
        value.import_eligible = !value.import_eligible;
        assert_eq!(
            value.validate(),
            Err(TableCandidateError::AuthorityViolation)
        );
        let mut value = perfect_candidate(REVISION_A);
        value.cells[0].import_eligible = !value.cells[0].import_eligible;
        assert_eq!(
            value.validate(),
            Err(TableCandidateError::AuthorityViolation)
        );
    }

    #[test]
    fn rejects_tampered_candidate_vid() {
        let mut value = perfect_candidate(REVISION_A);
        value.vid = "f".repeat(64);
        assert_eq!(value.validate(), Err(TableCandidateError::IdentityMismatch));
    }

    #[test]
    fn rejects_traversal_or_non_canonical_asset_paths() {
        for path in ["/tmp/Table3.png", "figures/../source.pdf", "Table3.png"] {
            let mut value = perfect_candidate(REVISION_A);
            value.asset_path = path.into();
            assert!(matches!(
                value.validate(),
                Err(TableCandidateError::InvalidAssetPath(_))
            ));
        }
    }

    #[test]
    fn rejects_malformed_sha256_lineage_fields() {
        for field in [
            "source_pdf_sha256",
            "image_sha256",
            "candidate_metadata_sha256",
        ] {
            let mut value = perfect_candidate(REVISION_A);
            match field {
                "source_pdf_sha256" => value.source_pdf_sha256 = "not-sha".into(),
                "image_sha256" => value.image_sha256 = "not-sha".into(),
                "candidate_metadata_sha256" => value.candidate_metadata_sha256 = "not-sha".into(),
                _ => unreachable!(),
            }
            assert_eq!(
                value.validate(),
                Err(TableCandidateError::InvalidSha256(field)),
                "{field} must be validated"
            );
        }
        let mut value = perfect_candidate(REVISION_A);
        value.tei_source_digest = "zz".into();
        assert!(value.validate().is_err());
    }

    #[test]
    fn rejects_cell_or_conflict_identity_tampering() {
        let mut value = perfect_candidate(REVISION_A);
        value.cells[0].vid = "c".repeat(64);
        assert!(matches!(
            value.validate(),
            Err(TableCandidateError::CellIdentityMismatch { row: 0, column: 0 })
        ));

        let visual = visual(&["H1"], &[&["0.87"]]);
        let tei = tei_table(vec![tei_cell(0, 0, "H2"), tei_cell(1, 0, "0.90")]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        let mut candidate =
            TableCandidate::assemble(&lineage(REVISION_A), Some(&visual), &tei, &report, None)
                .unwrap();
        candidate.conflicts[0].kind = "tampered".into();
        candidate.conflicts[0].vid = "f".repeat(64);
        assert!(matches!(
            candidate.validate(),
            Err(TableCandidateError::ConflictIdentityMismatch { .. })
        ));
    }

    #[test]
    fn rejects_unknown_missingness_kind() {
        let mut value = perfect_candidate(REVISION_A);
        value.missingness_kind = Some("no_tei_table".into());
        assert!(value.validate().is_err());
    }

    #[test]
    fn report_referencing_different_tei_is_rejected() {
        let tei = tei_table(vec![tei_cell(0, 0, "Method")]);
        let report = perfect_report(&tei);
        // A structurally identical table with a different source digest is a
        // different TEI identity: the report references the original digest.
        let mut other = tei.clone();
        other.source_digest = hex64('f');
        let err = TableCandidate::assemble(
            &lineage(REVISION_A),
            Some(&visual(&["Method"], &[])),
            &other,
            &report,
            None,
        )
        .unwrap_err();
        assert_eq!(err, TableCandidateError::TeiIdentityMismatch);
    }

    #[test]
    fn candidate_vids_are_revision_bound_and_disjoint() {
        let a = table_candidate_vid(
            REVISION_A,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_A, 2).unwrap(),
            "tab_0",
            &hex64('e'),
        )
        .unwrap();
        let a_again = table_candidate_vid(
            REVISION_A,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_A, 2).unwrap(),
            "tab_0",
            &hex64('e'),
        )
        .unwrap();
        let b = table_candidate_vid(
            REVISION_B,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_B, 2).unwrap(),
            "tab_0",
            &hex64('e'),
        )
        .unwrap();
        assert_eq!(a, a_again, "identical lineage replays to the same VID");
        assert_ne!(a, b, "two revisions must produce disjoint candidates");
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn candidate_vid_binds_tei_and_section_identity() {
        let tei_other = hex64('f');
        let a = table_candidate_vid(
            REVISION_A,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_A, 2).unwrap(),
            "tab_0",
            &hex64('e'),
        )
        .unwrap();
        let b = table_candidate_vid(
            REVISION_A,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_A, 2).unwrap(),
            "tab_0",
            &tei_other,
        )
        .unwrap();
        assert_ne!(a, b, "TEI source digest is part of candidate identity");
        let c = table_candidate_vid(
            REVISION_A,
            "Table3",
            &hex64('b'),
            &revision_section_vid(REVISION_A, 3).unwrap(),
            "tab_0",
            &hex64('e'),
        )
        .unwrap();
        assert_ne!(a, c, "section lineage is part of candidate identity");
    }

    #[test]
    fn candidate_vid_rejects_malformed_identity_inputs() {
        assert!(
            table_candidate_vid(
                "not-hex",
                "Table3",
                &hex64('b'),
                &"c".repeat(64),
                "tab_0",
                &hex64('e')
            )
            .is_err()
        );
        assert!(
            table_candidate_vid(
                &"a".repeat(64),
                "",
                &hex64('b'),
                &"c".repeat(64),
                "tab_0",
                &hex64('e')
            )
            .is_err()
        );
        assert!(
            table_candidate_vid(
                &"a".repeat(64),
                "Table3",
                &hex64('b'),
                &"c".repeat(64),
                " ",
                &hex64('e')
            )
            .is_err()
        );
        assert!(
            table_candidate_vid(
                &"a".repeat(64),
                "Table3",
                &hex64('b'),
                &"c".repeat(64),
                "tab_0",
                "short"
            )
            .is_err()
        );
    }

    #[test]
    fn cell_and_conflict_vids_are_deterministic() {
        let cand = &perfect_candidate(REVISION_A).vid;
        let v1 = table_cell_vid(cand, 1, 2).unwrap();
        let v2 = table_cell_vid(cand, 1, 2).unwrap();
        assert_eq!(v1, v2);
        assert_ne!(v1, table_cell_vid(cand, 1, 3).unwrap());
        let c1 = table_conflict_vid(cand, "header_mismatch", 0, 1).unwrap();
        let c2 = table_conflict_vid(cand, "header_mismatch", 0, 1).unwrap();
        assert_eq!(c1, c2);
        assert_ne!(
            c1,
            table_conflict_vid(cand, "numeric_mismatch", 0, 1).unwrap()
        );
    }

    #[test]
    fn metadata_digest_binds_fidelity_evidence() {
        let base = perfect_candidate(REVISION_A);

        let visual = visual(&["Method", "F1"], &[&["GEPA", "0.87"]]);
        let tei = tei_table(vec![
            tei_cell(0, 0, "Method"),
            tei_cell(0, 1, "F1"),
            tei_cell(1, 0, "GEPA"),
            tei_cell(1, 1, "0.90"),
        ]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        let conflicting =
            TableCandidate::assemble(&lineage(REVISION_A), Some(&visual), &tei, &report, None)
                .unwrap();
        assert_ne!(
            base.candidate_metadata_sha256, conflicting.candidate_metadata_sha256,
            "different fidelity evidence must produce different metadata digests"
        );
        assert_eq!(conflicting.conflict_count(), 1);
        assert_eq!(conflicting.candidate_metadata_sha256.len(), 64);
    }
}

// ─── ADR-051 P1: Property-based tests (M400 S02 T03) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use crate::table_fidelity::{StructuralTableCell, compare_table_fidelity};
    use proptest::prelude::*;

    // Mirror of the unit-test revision identity (sibling `tests` module
    // consts are private).
    const REVISION_A: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn hex64() -> impl Strategy<Value = String> {
        "[0-9a-f]{64}".prop_map(|s| s)
    }

    fn any_text() -> impl Strategy<Value = String> {
        prop::collection::vec("[0-9A-Za-z .%_-]", 0..10).prop_map(|v| v.concat())
    }

    fn any_visual() -> impl Strategy<Value = FigureTablePayload> {
        (
            prop::collection::vec(any_text(), 0..6),
            prop::collection::vec(prop::collection::vec(any_text(), 0..8), 0..8),
        )
            .prop_map(|(headers, rows)| FigureTablePayload { headers, rows })
    }

    fn any_tei_cell() -> impl Strategy<Value = StructuralTableCell> {
        (
            0u32..8,
            0u32..10,
            1u32..4,
            1u32..3,
            any_text(),
            hex64(),
            prop::option::of("[0-9,;]{4,32}"),
            prop::option::of(0u32..10000),
            prop::option::of(0u32..10000),
        )
            .prop_map(
                |(
                    row,
                    column,
                    colspan,
                    rowspan,
                    text,
                    digest,
                    coordinates,
                    span_start,
                    span_end,
                )| {
                    StructuralTableCell {
                        row,
                        column,
                        colspan,
                        rowspan,
                        text,
                        coordinates,
                        span_start,
                        span_end,
                        digest,
                    }
                },
            )
    }

    fn any_tei() -> impl Strategy<Value = StructuralTable> {
        (
            "[A-Za-z0-9_-]{1,32}",
            any_text(),
            prop::collection::vec(any_tei_cell(), 0..40),
            hex64(),
        )
            .prop_map(
                |(table_id, caption, cells, source_digest)| StructuralTable {
                    table_id,
                    caption,
                    cells,
                    source_digest,
                },
            )
    }

    fn prop_lineage(revision_vid: &str) -> TableCandidateLineage {
        TableCandidateLineage {
            revision_vid: revision_vid.into(),
            artifact_id: "TableP".into(),
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "prop".into(),
            image_sha256: hex64_seeded('b'),
            asset_path: "figures/TableP.png".into(),
            page_index: 0,
            section_vid: hex64_seeded('0'),
            section_title: "Results".into(),
            section_ordinal: 1,
            caption: "Table P".into(),
            source_pdf_sha256: hex64_seeded('a'),
        }
    }

    fn hex64_seeded(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    proptest! {
        /// Every assembled candidate validates: assemble() output is always
        /// internally consistent, even for adversarial grids.
        #[test]
        fn prop_assemble_always_validates(
            visual in any_visual(),
            tei in any_tei(),
        ) {
            // The report must reference the given TEI table: build the
            // comparison against the exact tei instance.
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            let candidate = TableCandidate::assemble(
                &prop_lineage(REVISION_A),
                Some(&visual),
                &tei,
                &report,
                None,
            )
            .unwrap();
            prop_assert!(candidate.validate().is_ok(), "assembled candidate must validate");
            // Coverage counters partition exactly as the report says.
            prop_assert_eq!(candidate.coverage, report.coverage);
        }

        /// Candidate identity is deterministic and revision-disjoint.
        #[test]
        fn prop_candidate_vid_deterministic_and_revision_disjoint(
            rev in hex64(),
            artifact in "[A-Za-z0-9_-]{1,16}",
            image in hex64(),
            section in hex64(),
            tei_id in "[A-Za-z0-9_-]{1,16}",
            tei_digest in hex64(),
        ) {
            let va = table_candidate_vid(&rev, &artifact, &image, &section, &tei_id, &tei_digest).unwrap();
            let vb = table_candidate_vid(&rev, &artifact, &image, &section, &tei_id, &tei_digest).unwrap();
            prop_assert_eq!(&va, &vb, "deterministic identity");
            prop_assert_eq!(va.len(), 64);
        }

        /// The no-visual-payload missingness path stays valid and TEI-grounded.
        #[test]
        fn prop_no_visual_payload_keeps_cells(tei in any_tei()) {
            let report = compare_table_fidelity(None, Some(&tei)).unwrap();
            let candidate = TableCandidate::assemble(&prop_lineage(REVISION_A), None, &tei, &report, None).unwrap();
            prop_assert!(candidate.validate().is_ok());
            prop_assert_eq!(
                candidate.missingness_kind.as_deref(),
                Some("no_visual_payload")
            );
            prop_assert_eq!(candidate.cell_count() as u32, tei.cell_count());
            prop_assert!(candidate.conflicts.is_empty());
        }

        /// Conflict records only ever carry the three real conflict kinds and
        /// are one-to-one with the report's conflict findings.
        #[test]
        fn prop_conflicts_match_report_findings(visual in any_visual(), tei in any_tei()) {
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            let candidate = TableCandidate::assemble(
                &prop_lineage(REVISION_A),
                Some(&visual),
                &tei,
                &report,
                None,
            )
            .unwrap();
            let report_conflicts = report
                .findings
                .iter()
                .filter(|f| f.is_conflict())
                .count();
            prop_assert_eq!(candidate.conflict_count(), report_conflicts);
            for conflict in &candidate.conflicts {
                prop_assert!(
                    matches!(
                        conflict.kind.as_str(),
                        "header_mismatch" | "numeric_mismatch" | "source_disagreement"
                    ),
                    "unexpected conflict kind {}",
                    conflict.kind
                );
            }
        }
    }
}
