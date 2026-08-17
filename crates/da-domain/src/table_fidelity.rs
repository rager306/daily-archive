//! Pure table fidelity domain contract (M400 S02 T02).
//!
//! Compares a visual table payload (VLM output: [`FigureTablePayload`]) with
//! an extracted structural table (GROBID TEI, projected domain-side as
//! [`StructuralTable`]) and emits typed fidelity evidence.
//!
//! Contract rules (R085, R087, R092, D127):
//! - **No invention:** a visual cell is only ever matched against a
//!   structural cell whose grid coverage includes its (row, column)
//!   position; cells that do not exist in a source are never synthesized.
//! - **No overwrite:** neither source is mutated; disagreement becomes a
//!   typed [`TableFidelityFinding`] (header / numeric / source), never a
//!   silent replacement of one text by the other.
//! - **Typed missingness:** absence of one whole source is reported via
//!   [`TableFidelityMissingness`], distinguishable from a present-but-empty
//!   structure (which degrades to per-cell [`TableFidelityFinding::MissingTeiCell`]).
//! - **Coverage is positional, fidelity is textual:** [`TableCellCoverage`]
//!   counts grid positions grounded in the structural source; findings carry
//!   the text-level disagreements at matched positions.
//! - **Non-authoritative:** the report is candidate evidence only;
//!   [`TableFidelityReport::validate`] rejects `authoritative` /
//!   `import_eligible` (D127, R092 — fidelity evidence never flips the
//!   authoritative AST plane).
//!
//! da-domain stays dependency-free: the structural side is the domain
//! projection of `da_ports::ExtractedTable`; the adapter-level conversion
//! from the ports wire types lives at the application boundary (T03).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use crate::figure_analysis::FigureTablePayload;

/// A single structural table cell (domain projection of `da_ports::TableCell`).
///
/// `row`/`column` are 0-based grid coordinates: `column` accounts for the
/// cumulative colspan of every preceding cell in the row (T01). Empty cells
/// are preserved so the grid stays aligned — nothing is collapsed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralTableCell {
    /// 0-based row index within the table.
    pub row: u32,
    /// 0-based column index (cumulative grid position after colspan).
    pub column: u32,
    /// TEI `cols` attribute (colspan); the cell covers `column..column+colspan`.
    #[serde(default = "default_one")]
    pub colspan: u32,
    /// TEI `rows` attribute (rowspan); the cell covers `row..row+rowspan`.
    #[serde(default = "default_one")]
    pub rowspan: u32,
    /// Trimmed visible text of the cell. Empty for preserved empty cells.
    pub text: String,
    /// GROBID `coords` attribute on the cell when emitted (rare), else None.
    #[serde(default)]
    pub coordinates: Option<String>,
    /// Source char span start of the cell's visible content inside the
    /// containing section's stripped text (`da_ports::CharSpan`, M400 S02 T01).
    /// `None` when the table is not inside any section text window (typed
    /// missingness, never guessed). The domain projection keeps the span so
    /// materialized cells carry source-span evidence (T03 slice demo).
    #[serde(default)]
    pub span_start: Option<u32>,
    /// Source char span end (exclusive) — pairs with `span_start`.
    #[serde(default)]
    pub span_end: Option<u32>,
    /// Deterministic SHA-256 digest of (row, column, colspan, rowspan, text,
    /// coordinates) — the cell's source-digest evidence (R087).
    pub digest: String,
}

fn default_one() -> u32 {
    1
}

/// Domain projection of a deterministic TEI table structure
/// (`da_ports::ExtractedTable`), carrying the evidence the fidelity
/// contract consumes: identity, caption, source digest and grid cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralTable {
    /// Stable table identifier (xml:id or deterministic ordinal key).
    pub table_id: String,
    /// Caption from the enclosing figure head (may be empty).
    #[serde(default)]
    pub caption: String,
    /// Cells in document order (row-major), including preserved empty cells.
    #[serde(default)]
    pub cells: Vec<StructuralTableCell>,
    /// Deterministic SHA-256 digest of the whole table (R087).
    pub source_digest: String,
}

impl StructuralTable {
    /// Validates the structural projection shape. Called at the application
    /// conversion boundary (T03); the fidelity contract itself stays total
    /// over any input and never panics on degenerate structures.
    pub fn validate(&self) -> Result<(), TableFidelityError> {
        if self.table_id.trim().is_empty() {
            return Err(TableFidelityError::Missing("table_id"));
        }
        if !is_sha256_hex(&self.source_digest) {
            return Err(TableFidelityError::Invalid(format!(
                "source_digest must be a SHA-256 digest, got {}",
                self.source_digest
            )));
        }
        for cell in &self.cells {
            if !is_sha256_hex(&cell.digest) {
                return Err(TableFidelityError::Invalid(format!(
                    "cell digest must be a SHA-256 digest, got {}",
                    cell.digest
                )));
            }
            if cell.colspan == 0 || cell.rowspan == 0 {
                return Err(TableFidelityError::Invalid(
                    "colspan and rowspan must be at least 1".into(),
                ));
            }
        }
        Ok(())
    }

    /// Number of structural cells (including preserved empty cells).
    pub fn cell_count(&self) -> u32 {
        self.cells.len() as u32
    }
}

/// Positional coverage of the visual payload by the structural source.
///
/// A visual cell counts as *matched* when a structural cell's covered grid
/// region (anchor plus colspan/rowspan) includes its (row, column) position.
/// Matching is positional: text disagreements at matched positions are
/// reported as typed findings, not counted as missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCellCoverage {
    /// Total visual cells (headers row plus body rows).
    pub visual_cells: u32,
    /// Total structural cells (including preserved empty cells).
    pub tei_cells: u32,
    /// Visual cells grounded in a structural cell's covered region.
    pub matched_cells: u32,
    /// Visual cells with no structural cell covering their position.
    pub unmatched_visual: u32,
    /// Structural cells whose anchor position has no visual cell.
    pub unmatched_tei: u32,
}

impl TableCellCoverage {
    /// Fraction of visual cells grounded in the structural source.
    /// `None` when the visual payload has no cells at all.
    pub fn matched_fraction(&self) -> Option<f64> {
        if self.visual_cells == 0 {
            None
        } else {
            Some(self.matched_cells as f64 / self.visual_cells as f64)
        }
    }
}

/// Typed per-cell fidelity finding (R087). Disagreement is typed, never an
/// overwrite: each variant preserves the exact source texts verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableFidelityFinding {
    /// A visual cell at (row, column) has no structural counterpart.
    MissingTeiCell {
        row: u32,
        column: u32,
        /// Exact visual text, preserved verbatim (no normalization loss).
        visual_text: String,
    },
    /// A structural cell at (row, column) has no visual counterpart.
    MissingVisualCell {
        row: u32,
        column: u32,
        /// Exact structural text, preserved verbatim.
        tei_text: String,
        /// Structural cell source digest (lineage evidence).
        tei_digest: String,
    },
    /// Header row (visual row 0) disagrees with the structural row 0 cell.
    HeaderMismatch {
        column: u32,
        visual_text: String,
        tei_text: String,
        tei_digest: String,
    },
    /// Body cell disagreement where both sides are numeric and differ in value.
    NumericMismatch {
        row: u32,
        column: u32,
        visual_text: String,
        tei_text: String,
        tei_digest: String,
    },
    /// Body cell text disagreement (non-numeric, or one side non-numeric).
    SourceDisagreement {
        row: u32,
        column: u32,
        visual_text: String,
        tei_text: String,
        tei_digest: String,
    },
}

impl TableFidelityFinding {
    /// Stable wire kind, e.g. `"header_mismatch"` (used by graph task
    /// reports to aggregate conflict types without pattern-matching).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MissingTeiCell { .. } => "missing_tei_cell",
            Self::MissingVisualCell { .. } => "missing_visual_cell",
            Self::HeaderMismatch { .. } => "header_mismatch",
            Self::NumericMismatch { .. } => "numeric_mismatch",
            Self::SourceDisagreement { .. } => "source_disagreement",
        }
    }

    /// True for text-level conflicts (header / numeric / source). Missing
    /// cells are coverage gaps, not conflicts.
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::HeaderMismatch { .. }
                | Self::NumericMismatch { .. }
                | Self::SourceDisagreement { .. }
        )
    }
}

/// Typed missingness for one side of the comparison (R087).
///
/// This is source-level absence: one whole source has nothing to compare.
/// A present-but-empty structure is *not* this variant — it degrades to
/// per-cell [`TableFidelityFinding::MissingTeiCell`] findings so the report
/// stays honest about what exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableFidelityMissingness {
    /// Visual payload exists but no structural TEI table was provided.
    NoTeiTable {
        /// Number of visual cells that therefore have no structural grounding.
        visual_cells: u32,
    },
    /// Structural TEI table exists but no visual payload was produced.
    NoVisualPayload {
        /// Number of structural cells with no visual counterpart.
        tei_cells: u32,
    },
}

/// Fidelity evidence report: coverage, typed findings, typed missingness,
/// and structural lineage. Never authoritative (D127).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableFidelityReport {
    pub coverage: TableCellCoverage,
    #[serde(default)]
    pub findings: Vec<TableFidelityFinding>,
    #[serde(default)]
    pub missingness: Option<TableFidelityMissingness>,
    /// Structural table identity when a TEI counterpart was provided.
    #[serde(default)]
    pub tei_table_id: Option<String>,
    /// Structural table source digest (lineage evidence).
    #[serde(default)]
    pub tei_source_digest: Option<String>,
    /// Fidelity evidence is candidate-only (D127): must stay false.
    pub authoritative: bool,
    pub import_eligible: bool,
}

impl TableFidelityReport {
    /// Structural self-consistency + non-authoritative locks (D127, R092).
    ///
    /// Invariants verified:
    /// - `matched + unmatched_visual == visual_cells` (coverage partitions
    ///   the visual grid);
    /// - every per-cell missing finding corresponds to a coverage counter
    ///   (missingness-`None` full comparisons only);
    /// - typed source-level missingness implies empty findings and zero
    ///   coverage on the absent side;
    /// - `authoritative` / `import_eligible` are false.
    pub fn validate(&self) -> Result<(), TableFidelityError> {
        if self.authoritative || self.import_eligible {
            return Err(TableFidelityError::AuthorityViolation);
        }
        let coverage = self.coverage;
        if coverage.matched_cells + coverage.unmatched_visual != coverage.visual_cells {
            return Err(TableFidelityError::Invalid(
                "coverage must partition visual cells into matched and unmatched".into(),
            ));
        }
        if coverage.matched_cells > coverage.visual_cells
            || coverage.unmatched_tei > coverage.tei_cells
        {
            return Err(TableFidelityError::Invalid(
                "coverage counters exceed their source totals".into(),
            ));
        }
        match &self.missingness {
            None => {
                if self.tei_table_id.is_none() || self.tei_source_digest.is_none() {
                    return Err(TableFidelityError::Invalid(
                        "a full comparison requires TEI identity evidence".into(),
                    ));
                }
                let missing_tei = self
                    .findings
                    .iter()
                    .filter(|f| matches!(f, TableFidelityFinding::MissingTeiCell { .. }))
                    .count() as u32;
                let missing_visual = self
                    .findings
                    .iter()
                    .filter(|f| matches!(f, TableFidelityFinding::MissingVisualCell { .. }))
                    .count() as u32;
                if missing_tei != coverage.unmatched_visual
                    || missing_visual != coverage.unmatched_tei
                {
                    return Err(TableFidelityError::Invalid(
                        "per-cell missing findings must match coverage counters".into(),
                    ));
                }
            }
            Some(TableFidelityMissingness::NoTeiTable { visual_cells }) => {
                if self.tei_table_id.is_some()
                    || self.tei_source_digest.is_some()
                    || !self.findings.is_empty()
                    || coverage.tei_cells != 0
                    || coverage.matched_cells != 0
                    || coverage.unmatched_tei != 0
                    || *visual_cells != coverage.visual_cells
                    || coverage.unmatched_visual != coverage.visual_cells
                {
                    return Err(TableFidelityError::Invalid(
                        "NoTeiTable missingness must leave the structural side empty".into(),
                    ));
                }
            }
            Some(TableFidelityMissingness::NoVisualPayload { tei_cells }) => {
                if self.tei_table_id.is_none()
                    || self.tei_source_digest.is_none()
                    || !self.findings.is_empty()
                    || coverage.visual_cells != 0
                    || coverage.matched_cells != 0
                    || coverage.unmatched_visual != 0
                    || *tei_cells != coverage.tei_cells
                    || coverage.unmatched_tei != coverage.tei_cells
                {
                    return Err(TableFidelityError::Invalid(
                        "NoVisualPayload missingness must leave the visual side empty".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Number of header disagreements.
    pub fn header_mismatches(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f, TableFidelityFinding::HeaderMismatch { .. }))
            .count()
    }

    /// Number of numeric value disagreements.
    pub fn numeric_mismatches(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f, TableFidelityFinding::NumericMismatch { .. }))
            .count()
    }

    /// Number of generic source disagreements (non-numeric text).
    pub fn source_disagreements(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f, TableFidelityFinding::SourceDisagreement { .. }))
            .count()
    }

    /// Number of visual cells missing a structural counterpart.
    pub fn missing_tei_cells(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f, TableFidelityFinding::MissingTeiCell { .. }))
            .count()
    }

    /// Number of structural cells missing a visual counterpart.
    pub fn missing_visual_cells(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| matches!(f, TableFidelityFinding::MissingVisualCell { .. }))
            .count()
    }

    /// Total text-level conflicts (header + numeric + source disagreement).
    /// Coverage gaps (missing cells) are deliberately excluded.
    pub fn total_conflicts(&self) -> usize {
        self.findings.iter().filter(|f| f.is_conflict()).count()
    }
}

/// Pure table fidelity comparison (M400 S02 T02).
///
/// `visual` and `tei` are both optional: either side may be absent, and the
/// absence is typed ([`TableFidelityMissingness`]) rather than silently
/// treated as an empty structure. Both `None` is a caller error.
///
/// The comparison never mutates either source and never invents cells:
/// visual cells are matched against the structural covered-region map
/// (anchor + colspan/rowspan, first-in-document-order wins on overlap),
/// and text disagreements produce typed findings preserving source texts
/// verbatim.
pub fn compare_table_fidelity(
    visual: Option<&FigureTablePayload>,
    tei: Option<&StructuralTable>,
) -> Result<TableFidelityReport, TableFidelityError> {
    let Some(visual) = visual else {
        let Some(tei) = tei else {
            return Err(TableFidelityError::NoSources);
        };
        // Structural-only: typed missingness, no invented visual cells.
        let tei_cells = tei.cell_count();
        return Ok(TableFidelityReport {
            coverage: TableCellCoverage {
                visual_cells: 0,
                tei_cells,
                matched_cells: 0,
                unmatched_visual: 0,
                unmatched_tei: tei_cells,
            },
            findings: Vec::new(),
            missingness: Some(TableFidelityMissingness::NoVisualPayload { tei_cells }),
            tei_table_id: Some(tei.table_id.clone()),
            tei_source_digest: Some(tei.source_digest.clone()),
            authoritative: false,
            import_eligible: false,
        });
    };

    let Some(tei) = tei else {
        // Visual-only: typed missingness; per-cell findings would be pure
        // noise when the whole structural source is absent (R087: the
        // missingness reason is the typed story).
        let visual_cells = visual_cell_count(visual);
        return Ok(TableFidelityReport {
            coverage: TableCellCoverage {
                visual_cells,
                tei_cells: 0,
                matched_cells: 0,
                unmatched_visual: visual_cells,
                unmatched_tei: 0,
            },
            findings: Vec::new(),
            missingness: Some(TableFidelityMissingness::NoTeiTable { visual_cells }),
            tei_table_id: None,
            tei_source_digest: None,
            authoritative: false,
            import_eligible: false,
        });
    };

    // Full comparison: build the structural covered-region map and walk the
    // visual grid in document order (headers row 0, then body rows).
    let mut covered: HashMap<(u32, u32), &StructuralTableCell> = HashMap::new();
    for cell in &tei.cells {
        let colspan = cell.colspan.max(1);
        let rowspan = cell.rowspan.max(1);
        for dr in 0..rowspan {
            for dc in 0..colspan {
                covered
                    .entry((cell.row + dr, cell.column + dc))
                    .or_insert(cell);
            }
        }
    }

    let visual_positions: Vec<(u32, u32, &str)> = {
        let mut positions = Vec::new();
        for (column, header) in visual.headers.iter().enumerate() {
            positions.push((0, column as u32, header.as_str()));
        }
        for (row_index, row) in visual.rows.iter().enumerate() {
            for (column, cell) in row.iter().enumerate() {
                positions.push(((row_index + 1) as u32, column as u32, cell.as_str()));
            }
        }
        positions
    };
    let visual_set: HashSet<(u32, u32)> =
        visual_positions.iter().map(|(r, c, _)| (*r, *c)).collect();

    let mut matched_cells = 0u32;
    let mut unmatched_visual = 0u32;
    let mut findings: Vec<TableFidelityFinding> = Vec::new();

    for (row, column, raw_text) in &visual_positions {
        let Some(tei_cell) = covered.get(&(*row, *column)) else {
            unmatched_visual += 1;
            findings.push(TableFidelityFinding::MissingTeiCell {
                row: *row,
                column: *column,
                visual_text: (*raw_text).to_string(),
            });
            continue;
        };
        matched_cells += 1;
        let visual_norm = normalize(raw_text);
        let tei_norm = normalize(&tei_cell.text);
        if visual_norm == tei_norm {
            continue;
        }
        if *row == 0 {
            // Header row disagreement is typed separately from body text.
            findings.push(TableFidelityFinding::HeaderMismatch {
                column: *column,
                visual_text: (*raw_text).to_string(),
                tei_text: tei_cell.text.clone(),
                tei_digest: tei_cell.digest.clone(),
            });
        } else if let (Some(visual_value), Some(tei_value)) =
            (numeric_value(&visual_norm), numeric_value(&tei_norm))
        {
            if visual_value != tei_value {
                findings.push(TableFidelityFinding::NumericMismatch {
                    row: *row,
                    column: *column,
                    visual_text: (*raw_text).to_string(),
                    tei_text: tei_cell.text.clone(),
                    tei_digest: tei_cell.digest.clone(),
                });
            }
            // Equal numeric values agree even when the strings differ
            // ("0.87" vs "0.870") — no invention, no false conflict.
        } else {
            findings.push(TableFidelityFinding::SourceDisagreement {
                row: *row,
                column: *column,
                visual_text: (*raw_text).to_string(),
                tei_text: tei_cell.text.clone(),
                tei_digest: tei_cell.digest.clone(),
            });
        }
    }

    // Structural cells whose anchor position has no visual cell.
    let mut unmatched_tei = 0u32;
    for cell in &tei.cells {
        if !visual_set.contains(&(cell.row, cell.column)) {
            unmatched_tei += 1;
            findings.push(TableFidelityFinding::MissingVisualCell {
                row: cell.row,
                column: cell.column,
                tei_text: cell.text.clone(),
                tei_digest: cell.digest.clone(),
            });
        }
    }

    Ok(TableFidelityReport {
        coverage: TableCellCoverage {
            visual_cells: visual_positions.len() as u32,
            tei_cells: tei.cell_count(),
            matched_cells,
            unmatched_visual,
            unmatched_tei,
        },
        findings,
        missingness: None,
        tei_table_id: Some(tei.table_id.clone()),
        tei_source_digest: Some(tei.source_digest.clone()),
        authoritative: false,
        import_eligible: false,
    })
}

/// Total visual cells: headers row plus every body row cell.
fn visual_cell_count(visual: &FigureTablePayload) -> u32 {
    visual.headers.len() as u32 + visual.rows.iter().map(|row| row.len() as u32).sum::<u32>()
}

/// Whitespace-collapsed comparison key (also trims leading/trailing space).
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Best-effort numeric interpretation: whitespace-collapsed, an optional
/// trailing `%` is stripped, then parsed as a finite f64.
fn numeric_value(text: &str) -> Option<f64> {
    let core = text.strip_suffix('%').unwrap_or(text);
    core.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// 64 lowercase-hex SHA-256 shape check (digest evidence passthrough).
fn is_sha256_hex(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TableFidelityError {
    #[error("missing table fidelity field: {0}")]
    Missing(&'static str),
    #[error("invalid table fidelity: {0}")]
    Invalid(String),
    #[error("table fidelity evidence cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("table fidelity comparison requires at least one source")]
    NoSources,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex64(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn cell(row: u32, column: u32, text: &str) -> StructuralTableCell {
        StructuralTableCell {
            row,
            column,
            colspan: 1,
            rowspan: 1,
            text: text.into(),
            coordinates: None,
            span_start: None,
            span_end: None,
            digest: hex64('d'),
        }
    }

    fn table(table_id: &str, cells: Vec<StructuralTableCell>) -> StructuralTable {
        StructuralTable {
            table_id: table_id.into(),
            caption: String::new(),
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

    fn digest_cell(row: u32, column: u32, text: &str, digest: &str) -> StructuralTableCell {
        StructuralTableCell {
            row,
            column,
            colspan: 1,
            rowspan: 1,
            text: text.into(),
            coordinates: None,
            span_start: None,
            span_end: None,
            digest: digest.into(),
        }
    }

    #[test]
    fn perfect_match_has_no_findings() {
        let visual = visual(
            &["Method", "F1"],
            &[&["GEPA", "0.87"], &["Baseline", "0.72"]],
        );
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                cell(0, 1, "F1"),
                cell(1, 0, "GEPA"),
                cell(1, 1, "0.87"),
                cell(2, 0, "Baseline"),
                cell(2, 1, "0.72"),
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.missingness, None);
        assert!(report.findings.is_empty());
        assert_eq!(report.coverage.matched_cells, 6);
        assert_eq!(report.coverage.unmatched_visual, 0);
        assert_eq!(report.coverage.unmatched_tei, 0);
        assert_eq!(report.coverage.matched_fraction(), Some(1.0));
        assert_eq!(report.tei_table_id.as_deref(), Some("tab_0"));
        report.validate().unwrap();
    }

    #[test]
    fn header_mismatch_is_typed() {
        let visual = visual(&["Method", "F1-score"], &[]);
        let tei = table("tab_0", vec![cell(0, 0, "Method"), cell(0, 1, "F1")]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.header_mismatches(), 1);
        assert_eq!(report.total_conflicts(), 1);
        let finding = &report.findings[0];
        assert!(matches!(
            finding,
            TableFidelityFinding::HeaderMismatch {
                column: 1,
                visual_text,
                tei_text,
                tei_digest,
            } if visual_text == "F1-score" && tei_text == "F1" && tei_digest == &hex64('d')
        ));
        assert_eq!(finding.kind(), "header_mismatch");
        report.validate().unwrap();
    }

    #[test]
    fn numeric_mismatch_is_typed() {
        let visual = visual(&["Method", "F1"], &[&["GEPA", "0.87"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                cell(0, 1, "F1"),
                cell(1, 0, "GEPA"),
                cell(1, 1, "0.90"),
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.numeric_mismatches(), 1);
        assert_eq!(report.source_disagreements(), 0);
        assert!(matches!(
            &report.findings[0],
            TableFidelityFinding::NumericMismatch {
                row: 1,
                column: 1,
                ..
            }
        ));
        report.validate().unwrap();
    }

    #[test]
    fn numeric_equivalent_texts_agree() {
        // "0.87" vs "0.870" parse to the same finite value: no false conflict.
        let visual = visual(&["v"], &[&["0.87"]]);
        let tei = table("tab_0", vec![cell(0, 0, "v"), cell(1, 0, "0.870")]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert!(report.findings.is_empty());
        assert_eq!(report.coverage.matched_cells, 2);
        report.validate().unwrap();
    }

    #[test]
    fn source_disagreement_is_typed() {
        let visual = visual(&["Method"], &[&["GEPA"]]);
        let tei = table("tab_0", vec![cell(0, 0, "Method"), cell(1, 0, "GEPEA")]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.source_disagreements(), 1);
        assert_eq!(report.numeric_mismatches(), 0);
        assert!(matches!(
            &report.findings[0],
            TableFidelityFinding::SourceDisagreement {
                row: 1,
                column: 0,
                ..
            }
        ));
        report.validate().unwrap();
    }

    #[test]
    fn missing_tei_cell_is_typed() {
        let visual = visual(&["Method", "F1"], &[&["GEPA", "0.87"]]);
        // Structural row 1 has only one cell: column 1 is ungrounded.
        let tei = table(
            "tab_0",
            vec![cell(0, 0, "Method"), cell(0, 1, "F1"), cell(1, 0, "GEPA")],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.missing_tei_cells(), 1);
        assert_eq!(report.coverage.matched_cells, 3);
        assert_eq!(report.coverage.unmatched_visual, 1);
        assert!(matches!(
            &report.findings[0],
            TableFidelityFinding::MissingTeiCell {
                row: 1,
                column: 1,
                visual_text,
            } if visual_text == "0.87"
        ));
        report.validate().unwrap();
    }

    #[test]
    fn missing_visual_cell_carries_digest() {
        let visual = visual(&["Method"], &[]);
        let tei = table(
            "tab_0",
            vec![cell(0, 0, "Method"), digest_cell(1, 0, "GEPA", &hex64('f'))],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.missing_visual_cells(), 1);
        assert_eq!(report.coverage.unmatched_tei, 1);
        assert!(matches!(
            &report.findings[0],
            TableFidelityFinding::MissingVisualCell {
                row: 1,
                column: 0,
                tei_text,
                tei_digest,
            } if tei_text == "GEPA" && tei_digest == &hex64('f')
        ));
        report.validate().unwrap();
    }

    #[test]
    fn colspan_coverage_matches_both_positions() {
        // Structural cell at (1, 0) with colspan 2 covers visual (1,0)+(1,1);
        // the visual duplicates the merged value so both positions agree.
        let visual = visual(&["Method", "F1"], &[&["GEPA", "GEPA"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                cell(0, 1, "F1"),
                StructuralTableCell {
                    row: 1,
                    column: 0,
                    colspan: 2,
                    rowspan: 1,
                    text: "GEPA".into(),
                    coordinates: None,
                    span_start: None,
                    span_end: None,
                    digest: hex64('f'),
                },
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.coverage.matched_cells, 4);
        assert_eq!(report.coverage.unmatched_visual, 0);
        assert_eq!(report.coverage.unmatched_tei, 0);
        assert!(report.findings.is_empty());
        report.validate().unwrap();
    }

    #[test]
    fn colspan_disagreement_at_covered_position_is_typed() {
        // A TEI colspan cell claims the covered column; a conflicting visual
        // value there is a typed source disagreement, not an overwrite.
        let visual = visual(&["Method", "F1"], &[&["GEPA", "0.87"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                cell(0, 1, "F1"),
                StructuralTableCell {
                    row: 1,
                    column: 0,
                    colspan: 2,
                    rowspan: 1,
                    text: "GEPA".into(),
                    coordinates: None,
                    span_start: None,
                    span_end: None,
                    digest: hex64('f'),
                },
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.coverage.matched_cells, 4);
        assert_eq!(report.source_disagreements(), 1);
        assert!(matches!(
            &report.findings[0],
            TableFidelityFinding::SourceDisagreement {
                row: 1,
                column: 1,
                visual_text,
                tei_text,
                tei_digest,
            } if visual_text == "0.87" && tei_text == "GEPA" && tei_digest == &hex64('f')
        ));
        report.validate().unwrap();
    }

    #[test]
    fn rowspan_coverage_matches_both_positions() {
        let visual = visual(&["Method"], &[&["GEPA"], &["GEPA"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                StructuralTableCell {
                    row: 1,
                    column: 0,
                    colspan: 1,
                    rowspan: 2,
                    text: "GEPA".into(),
                    coordinates: None,
                    span_start: None,
                    span_end: None,
                    digest: hex64('f'),
                },
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.coverage.matched_cells, 3);
        assert_eq!(report.coverage.unmatched_visual, 0);
        assert_eq!(report.coverage.unmatched_tei, 0);
        assert!(report.findings.is_empty());
        report.validate().unwrap();
    }

    #[test]
    fn no_tei_table_missingness_is_typed() {
        let visual = visual(&["Method", "F1"], &[&["GEPA", "0.87"]]);
        let report = compare_table_fidelity(Some(&visual), None).unwrap();
        assert_eq!(
            report.missingness,
            Some(TableFidelityMissingness::NoTeiTable { visual_cells: 4 })
        );
        assert!(report.findings.is_empty());
        assert_eq!(report.coverage.tei_cells, 0);
        assert_eq!(report.coverage.matched_cells, 0);
        assert_eq!(report.coverage.matched_fraction(), Some(0.0));
        assert!(report.tei_table_id.is_none());
        report.validate().unwrap();
    }

    #[test]
    fn no_visual_payload_missingness_is_typed() {
        let tei = table("tab_0", vec![cell(0, 0, "Method"), cell(0, 1, "F1")]);
        let report = compare_table_fidelity(None, Some(&tei)).unwrap();
        assert_eq!(
            report.missingness,
            Some(TableFidelityMissingness::NoVisualPayload { tei_cells: 2 })
        );
        assert!(report.findings.is_empty());
        assert_eq!(report.coverage.visual_cells, 0);
        assert_eq!(report.coverage.matched_cells, 0);
        assert_eq!(report.coverage.matched_fraction(), None);
        assert_eq!(report.tei_table_id.as_deref(), Some("tab_0"));
        report.validate().unwrap();
    }

    #[test]
    fn both_sources_missing_is_error() {
        let err = compare_table_fidelity(None, None).unwrap_err();
        assert_eq!(err, TableFidelityError::NoSources);
    }

    #[test]
    fn ragged_visual_rows_stay_total() {
        // Ragged body rows are compared positionally: no invention, no panic.
        let visual = visual(&["A"], &[&["x", "y"], &["z"]]);
        let tei = table(
            "tab_0",
            vec![cell(0, 0, "A"), cell(1, 0, "x"), cell(2, 0, "z")],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.coverage.visual_cells, 4);
        assert_eq!(report.coverage.matched_cells, 3);
        assert_eq!(report.coverage.unmatched_visual, 1);
        assert_eq!(report.missing_tei_cells(), 1);
        report.validate().unwrap();
    }

    #[test]
    fn empty_tei_structure_degrades_to_per_cell_missing() {
        // A present-but-empty structural table is NOT source-level missingness:
        // it degrades to per-cell MissingTeiCell findings.
        let visual = visual(&["Method"], &[&["GEPA"]]);
        let tei = table("tab_0", vec![]);
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.missingness, None);
        assert_eq!(report.missing_tei_cells(), 2);
        assert_eq!(report.coverage.matched_cells, 0);
        assert_eq!(report.coverage.unmatched_visual, 2);
        report.validate().unwrap();
    }

    #[test]
    fn authority_lock_is_rejected() {
        let visual = visual(&["A"], &[]);
        let tei = table("tab_0", vec![cell(0, 0, "A")]);
        let mut report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        report.authoritative = true;
        assert_eq!(
            report.validate(),
            Err(TableFidelityError::AuthorityViolation)
        );
    }

    #[test]
    fn import_eligible_lock_is_rejected() {
        let visual = visual(&["A"], &[]);
        let tei = table("tab_0", vec![cell(0, 0, "A")]);
        let mut report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        report.import_eligible = !report.import_eligible;
        assert_eq!(
            report.validate(),
            Err(TableFidelityError::AuthorityViolation)
        );
    }

    #[test]
    fn structural_table_validate_rejects_bad_digest() {
        let tei = table("tab_0", vec![cell(0, 0, "A")]);
        let mut bad = tei.clone();
        bad.source_digest = "not-a-digest".into();
        assert!(bad.validate().is_err());

        let mut bad_cell = tei;
        bad_cell.cells[0].digest = "zz".into();
        assert!(bad_cell.validate().is_err());
    }

    #[test]
    fn structural_table_validate_rejects_empty_id_and_zero_span() {
        let tei = table("tab_0", vec![cell(0, 0, "A")]);
        let mut no_id = tei.clone();
        no_id.table_id = "  ".into();
        assert_eq!(
            no_id.validate(),
            Err(TableFidelityError::Missing("table_id"))
        );

        let mut zero_span = tei;
        zero_span.cells[0].colspan = 0;
        assert!(zero_span.validate().is_err());
    }

    #[test]
    fn finding_wire_kinds_are_stable() {
        assert_eq!(
            TableFidelityFinding::MissingTeiCell {
                row: 0,
                column: 0,
                visual_text: "x".into(),
            }
            .kind(),
            "missing_tei_cell"
        );
        assert_eq!(
            TableFidelityFinding::MissingVisualCell {
                row: 0,
                column: 0,
                tei_text: "x".into(),
                tei_digest: hex64('a'),
            }
            .kind(),
            "missing_visual_cell"
        );
        assert_eq!(
            TableFidelityFinding::HeaderMismatch {
                column: 0,
                visual_text: "x".into(),
                tei_text: "y".into(),
                tei_digest: hex64('a'),
            }
            .kind(),
            "header_mismatch"
        );
        assert_eq!(
            TableFidelityFinding::NumericMismatch {
                row: 0,
                column: 0,
                visual_text: "1".into(),
                tei_text: "2".into(),
                tei_digest: hex64('a'),
            }
            .kind(),
            "numeric_mismatch"
        );
        assert_eq!(
            TableFidelityFinding::SourceDisagreement {
                row: 0,
                column: 0,
                visual_text: "x".into(),
                tei_text: "y".into(),
                tei_digest: hex64('a'),
            }
            .kind(),
            "source_disagreement"
        );
    }

    #[test]
    fn serde_roundtrip_report() {
        let visual = visual(&["Method", "F1-score"], &[&["GEPA", "0.87"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "Method"),
                cell(0, 1, "F1"),
                cell(1, 0, "GEPA"),
                cell(1, 1, "0.90"),
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        crate::roundtrip_json(&report);
        crate::roundtrip_json(&report.coverage);
        crate::roundtrip_json(&report.missingness);
    }

    #[test]
    fn serde_roundtrip_missingness_and_structural() {
        let tei = table("tab_0", vec![cell(0, 0, "A")]);
        crate::roundtrip_json(&tei);
        let report = compare_table_fidelity(Some(&visual(&["A"], &[])), None).unwrap();
        crate::roundtrip_json(&report);
        let report = compare_table_fidelity(None, Some(&tei)).unwrap();
        crate::roundtrip_json(&report);
    }

    #[test]
    fn finding_kind_counts_aggregate() {
        let visual = visual(&["H1", "H2"], &[&["0.87", "x"]]);
        let tei = table(
            "tab_0",
            vec![
                cell(0, 0, "H1"),
                cell(0, 1, "different"),
                cell(1, 0, "0.90"),
                cell(1, 1, "y"),
            ],
        );
        let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
        assert_eq!(report.header_mismatches(), 1);
        assert_eq!(report.numeric_mismatches(), 1);
        assert_eq!(report.source_disagreements(), 1);
        assert_eq!(report.total_conflicts(), 3);
        assert_eq!(report.missing_tei_cells(), 0);
        assert_eq!(report.missing_visual_cells(), 0);
        report.validate().unwrap();
    }

    #[test]
    fn coverage_fraction_edge_cases() {
        let empty = TableCellCoverage {
            visual_cells: 0,
            tei_cells: 0,
            matched_cells: 0,
            unmatched_visual: 0,
            unmatched_tei: 0,
        };
        assert_eq!(empty.matched_fraction(), None);
        let half = TableCellCoverage {
            visual_cells: 4,
            tei_cells: 4,
            matched_cells: 2,
            unmatched_visual: 2,
            unmatched_tei: 2,
        };
        assert_eq!(half.matched_fraction(), Some(0.5));
    }
}

// ─── ADR-051 P1: Property-based tests (M400 S02 T02) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    fn hex64() -> impl Strategy<Value = String> {
        "[0-9a-f]{64}".prop_map(|s| s)
    }

    /// Short printable text: mostly alphabetic, occasionally numeric-looking.
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
            prop::option::of(0u32..10_000),
            prop::option::of(0u32..10_000),
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
                        digest,
                        coordinates,
                        span_start,
                        span_end,
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

    /// Builds the structural mirror of a visual grid (perfect positional +
    /// textual agreement) — the positive-fixture generator.
    fn perfect_tei(visual: &FigureTablePayload) -> StructuralTable {
        let mut cells = Vec::new();
        for (column, header) in visual.headers.iter().enumerate() {
            cells.push(StructuralTableCell {
                row: 0,
                column: column as u32,
                colspan: 1,
                rowspan: 1,
                text: header.clone(),
                coordinates: None,
                span_start: None,
                span_end: None,
                digest: hex64_seeded('a'),
            });
        }
        for (row_index, row) in visual.rows.iter().enumerate() {
            for (column, cell_text) in row.iter().enumerate() {
                cells.push(StructuralTableCell {
                    row: (row_index + 1) as u32,
                    column: column as u32,
                    colspan: 1,
                    rowspan: 1,
                    text: cell_text.clone(),
                    coordinates: None,
                    span_start: None,
                    span_end: None,
                    digest: hex64_seeded('b'),
                });
            }
        }
        StructuralTable {
            table_id: "tab_prop".into(),
            caption: String::new(),
            cells,
            source_digest: hex64_seeded('c'),
        }
    }

    fn hex64_seeded(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    proptest! {
        /// The report is total and always internally consistent: for ANY
        /// visual and structural inputs the report validates.
        #[test]
        fn prop_fidelity_report_always_validates(visual in any_visual(), tei in any_tei()) {
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            prop_assert!(report.validate().is_ok(), "report must validate for arbitrary inputs");
        }

        /// Coverage partitions the visual grid: every visual cell is either
        /// matched or unmatched, never both and never neither.
        #[test]
        fn prop_fidelity_coverage_partitions_visual_cells(
            visual in any_visual(),
            tei in any_tei(),
        ) {
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            let coverage = report.coverage;
            prop_assert_eq!(
                coverage.matched_cells + coverage.unmatched_visual,
                coverage.visual_cells,
                "coverage must partition visual cells"
            );
            prop_assert!(
                coverage.unmatched_tei <= coverage.tei_cells,
                "unmatched TEI cells cannot exceed TEI cells"
            );
        }

        /// The comparison is deterministic: identical inputs yield identical
        /// reports (serde-stable evidence for replay).
        #[test]
        fn prop_fidelity_deterministic(visual in any_visual(), tei in any_tei()) {
            let first = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            let second = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            prop_assert_eq!(first, second, "comparison must be deterministic");
        }

        /// Positive fixture: the structural mirror of a visual grid grounds
        /// every cell with zero findings and zero gaps.
        #[test]
        fn prop_fidelity_perfect_grid_has_no_findings(visual in any_visual()) {
            let tei = perfect_tei(&visual);
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            prop_assert_eq!(report.missingness, None);
            prop_assert!(
                report.findings.is_empty(),
                "perfect grid must produce no findings: {:?}",
                report.findings
            );
            prop_assert_eq!(report.coverage.matched_cells, report.coverage.visual_cells);
            prop_assert_eq!(report.coverage.unmatched_visual, 0);
            prop_assert_eq!(report.coverage.unmatched_tei, 0);
        }

        /// Source-level absence is typed, never treated as an empty grid.
        #[test]
        fn prop_fidelity_missingness_typed_by_source_absence(
            visual in any_visual(),
            tei in any_tei(),
        ) {
            let visual_only = compare_table_fidelity(Some(&visual), None).unwrap();
            prop_assert_eq!(
                visual_only.missingness.as_ref(),
                Some(&TableFidelityMissingness::NoTeiTable {
                    visual_cells: visual_cell_count(&visual),
                })
            );
            prop_assert!(visual_only.findings.is_empty());
            prop_assert!(visual_only.validate().is_ok());

            let tei_only = compare_table_fidelity(None, Some(&tei)).unwrap();
            prop_assert_eq!(
                tei_only.missingness.as_ref(),
                Some(&TableFidelityMissingness::NoVisualPayload {
                    tei_cells: tei.cell_count(),
                })
            );
            prop_assert!(tei_only.findings.is_empty());
            prop_assert!(tei_only.validate().is_ok());
        }

        /// Per-cell missing findings mirror the coverage counters exactly
        /// (the validate() invariant, asserted directly on the report).
        #[test]
        fn prop_fidelity_missing_findings_match_coverage(
            visual in any_visual(),
            tei in any_tei(),
        ) {
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            prop_assert_eq!(
                report.missing_tei_cells() as u32,
                report.coverage.unmatched_visual
            );
            prop_assert_eq!(
                report.missing_visual_cells() as u32,
                report.coverage.unmatched_tei
            );
        }

        /// A matched cell never produces a MissingTeiCell finding: findings
        /// only reference cells that exist in at least one source.
        #[test]
        fn prop_fidelity_findings_only_reference_existing_cells(
            visual in any_visual(),
            tei in any_tei(),
        ) {
            let report = compare_table_fidelity(Some(&visual), Some(&tei)).unwrap();
            let visual_set: HashSet<(u32, u32)> = visual_cell_positions(&visual).into_iter().collect();
            let tei_anchors: HashSet<(u32, u32)> = tei.cells.iter().map(|c| (c.row, c.column)).collect();
            for finding in &report.findings {
                match finding {
                    TableFidelityFinding::MissingTeiCell { row, column, .. } => {
                        prop_assert!(
                            visual_set.contains(&(*row, *column)),
                            "MissingTeiCell must reference a real visual cell"
                        );
                    }
                    TableFidelityFinding::MissingVisualCell { row, column, .. } => {
                        prop_assert!(
                            tei_anchors.contains(&(*row, *column)),
                            "MissingVisualCell must reference a real TEI anchor"
                        );
                    }
                    TableFidelityFinding::HeaderMismatch { column, .. } => {
                        prop_assert!(
                            visual_set.contains(&(0, *column)),
                            "HeaderMismatch must reference a real visual header"
                        );
                    }
                    TableFidelityFinding::NumericMismatch { row, column, .. }
                    | TableFidelityFinding::SourceDisagreement { row, column, .. } => {
                        prop_assert!(
                            visual_set.contains(&(*row, *column)),
                            "conflict findings must reference a real visual cell"
                        );
                    }
                }
            }
        }
    }

    /// All (row, column) positions occupied by a visual payload.
    fn visual_cell_positions(visual: &FigureTablePayload) -> Vec<(u32, u32)> {
        let mut positions = Vec::new();
        for column in 0..visual.headers.len() {
            positions.push((0, column as u32));
        }
        for (row_index, row) in visual.rows.iter().enumerate() {
            for column in 0..row.len() {
                positions.push(((row_index + 1) as u32, column as u32));
            }
        }
        positions
    }
}
