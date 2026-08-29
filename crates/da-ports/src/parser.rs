//! Parser port — raw source → ParsedArticle.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Parse failed: {0}")]
    ParseFailed(String),
    #[error("Service unavailable: {0}")]
    Unavailable(String),
}

pub type ParseResult<T> = Result<T, ParserError>;

/// A parsed article from GROBID + ODL.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedArticle {
    pub paper_id: String,
    pub title: String,
    pub abstract_text: String,
    pub body_text: String,
    pub sections: Vec<Section>,
    pub citations: Vec<CitationEntry>,
    pub layout_json: Option<serde_json::Value>,
    pub tei_xml: Option<String>,
    pub pdf_hash: String,
    /// Deterministic TEI table structures (M400 S02 T01). `#[serde(default)]`
    /// keeps legacy cached/fixture JSON decodable (R085) — legacy payloads
    /// without the field deserialize to an empty Vec.
    #[serde(default)]
    pub tables: Vec<ExtractedTable>,
    /// Typed missingness when the source TEI has no table structure at all
    /// (R087 — no silent loss: an empty `tables` Vec alone cannot distinguish
    /// "document genuinely has no tables" from "parser did not populate
    /// tables"). `None` means the parser does not extract tables (legacy
    /// payload / HTML parser) or extraction ran and found tables.
    #[serde(default)]
    pub table_missingness: Option<TableMissingReason>,
    /// Deterministic TEI formula structures (M400 S03 T01). `#[serde(default)]`
    /// keeps legacy cached/fixture JSON decodable (R085) — legacy payloads
    /// without the field deserialize to an empty Vec.
    #[serde(default)]
    pub formulas: Vec<ExtractedFormula>,
    /// Typed missingness when the source TEI has no formula structure at all
    /// (R087 — no silent loss: an empty `formulas` Vec alone cannot
    /// distinguish "document genuinely has no formulas" from "parser did not
    /// populate formulas"). `None` means the parser does not extract formulas
    /// (legacy payload / HTML parser) or extraction ran and found formulas.
    #[serde(default)]
    pub formula_missingness: Option<FormulaMissingReason>,
    /// Typed page-text missingness from pdf-inspector classification (M422).
    ///
    /// `None` = not stamped (parser did not classify; legacy JSON).
    /// `Some(empty)` = classified and no page needs OCR.
    /// `Some(non-empty)` = those 1-indexed pages are garbled/scanned/no-text.
    /// GROBID `body_text` is **not** rewritten — this is detection, not repair.
    #[serde(default)]
    pub page_text_missingness: Option<Vec<PageTextMissing>>,

    // ─── S07 P0 lift families (M464-z84in2 S07 T01) ───
    /// Declared arXiv identity from the TEI header `<idno type="arXiv">`.
    /// `None` = typed `declared_id_missingness` explains why; the revision is
    /// carried only when explicitly declared — never guessed (S07).
    #[serde(default)]
    pub declared_arxiv_id: Option<DeclaredArxivIdentity>,
    /// Typed missingness when `declared_arxiv_id` is `None` (R087 — no
    /// silent loss). `None` means the identity is declared (or the payload
    /// predates S07).
    #[serde(default)]
    pub declared_id_missingness: Option<DeclaredIdMissingReason>,
    /// Document-ordered header authors (`<persName>` inside the TEI header;
    /// `sourceDesc`/`biblStruct` on the GROBID path). Bibliography persName
    /// entries never appear here. Empty until the lifter populates it
    /// (S07 T02); the HTML path keeps it typed-empty.
    #[serde(default)]
    pub header_authors: Vec<HeaderAuthor>,
    /// Non-table figures in document order (S07 T03). Table wrappers
    /// (`<figure type="table">`) are extracted as `tables`, never here.
    #[serde(default)]
    pub figures: Vec<ExtractedFigure>,
    /// Visual callouts (`<ref type="figure|table">`) in document order, with
    /// an explicit target-unresolved state (S07 T03). Bibr refs stay on
    /// `CitationAnchor`; `ptr`/`note` refs stay outside this model.
    #[serde(default)]
    pub visual_callouts: Vec<VisualCallout>,
}

/// A source char span `[start, end)` (end exclusive) inside a section's
/// stripped text (M400 S02 T01). Offsets are 0-based char offsets into
/// `Section.text`, mirroring `CitationAnchor.offset` semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharSpan {
    pub start: u32,
    pub end: u32,
}

/// One 1-indexed page whose text layer is not honestly extractable (M422).
///
/// Built from [`da_domain::PdfClassification::pages_needing_ocr`] plus
/// [`da_domain::PdfClassification::ocr_reasons_by_page`]. Reasons may be
/// empty when the classifier listed the page without a reason vector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageTextMissing {
    /// 1-indexed page number (matches pdf-inspector / GROBID coords).
    pub page: u32,
    /// Machine-readable OCR reason identifiers (may be empty).
    #[serde(default)]
    pub reasons: Vec<String>,
}

/// Map a pdf-inspector classification onto typed page-text missingness.
///
/// Every page in `pages_needing_ocr` appears exactly once. Reasons come
/// from `ocr_reasons_by_page` when present; otherwise the page is stamped
/// with an empty reason list (never invented).
pub fn page_text_missingness_from(
    classification: &da_domain::PdfClassification,
) -> Vec<PageTextMissing> {
    classification
        .pages_needing_ocr
        .iter()
        .copied()
        .map(|page| {
            let reasons = classification
                .ocr_reason_for_page(page)
                .map(|r| r.to_vec())
                .unwrap_or_default();
            PageTextMissing { page, reasons }
        })
        .collect()
}

/// Typed missingness reason for TEI table structure (M400 S02 T01).
///
/// `ParsedArticle.table_missingness` records the article-level reason when no
/// `<table>` element exists in the source TEI; `ExtractedTable.missingness`
/// records per-table reasons when a `<table>` element was found but its
/// structure is unusable. Either way the parser is fail-closed: cells are
/// never invented and neither source is overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableMissingReason {
    /// The source TEI contains no `<table>` element at all.
    NoTableElements,
    /// A `<table>` element was found but its structure could not be parsed
    /// deterministically (e.g. zero rows or zero cells).
    Malformed(String),
}

/// A single table cell from deterministic TEI structure (M400 S02 T01).
///
/// Cells are preserved in document order including empty `<cell></cell>`
/// entries, so column alignment survives for visual-versus-structural
/// fidelity comparison (T02) — nothing is collapsed or invented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// 0-based row index within the table.
    pub row: u32,
    /// 0-based column index within the row, accounting for the `cols`
    /// (colspan) of every preceding cell (cumulative grid position).
    pub column: u32,
    /// TEI `cols` attribute (colspan); default 1.
    #[serde(default = "default_one")]
    pub colspan: u32,
    /// TEI `rows` attribute (rowspan); default 1.
    #[serde(default = "default_one")]
    pub rowspan: u32,
    /// Trimmed visible text of the cell. Empty for `<cell></cell>` — empty
    /// cells are preserved so the grid stays aligned.
    pub text: String,
    /// GROBID `coords` attribute on the cell when emitted (rare), else None.
    #[serde(default)]
    pub coordinates: Option<String>,
    /// Source char span of the cell's visible content inside the containing
    /// section's stripped text. `None` when the table is not inside any
    /// section text window (typed missingness, never guessed).
    #[serde(default)]
    pub span: Option<CharSpan>,
    /// Deterministic SHA-256 digest of (row, column, colspan, rowspan, text,
    /// coordinates) — the cell's source-digest evidence (R087).
    pub digest: String,
}

fn default_one() -> u32 {
    1
}

/// Typed missingness reason for TEI formula structure (M400 S03 T01).
///
/// `ParsedArticle.formula_missingness` records the article-level reason when
/// no `<formula>` element exists in the source TEI; `ExtractedFormula.missingness`
/// records per-formula reasons when a `<formula>` element was found but its
/// content is unusable. Either way the parser is fail-closed: representations
/// are never invented and neither source is overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormulaMissingReason {
    /// The source TEI contains no `<formula>` element at all.
    NoFormulaElements,
    /// A `<formula>` element was found but its content is unusable
    /// (e.g. empty with no MathML/LaTeX/text representation).
    Malformed(String),
}

/// The representation of a TEI `<formula>` element (M400 S03 T01).
///
/// GROBID emits formulas as `<formula xml:id="…">` with the PDF-layout text
/// approximation, optionally a MathML `<math>` subtree (math extraction
/// enabled) or a `<texMath>` element (other TEI producers). Exactly one
/// variant is populated per formula; the parser never invents a
/// representation where the source had none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormulaRepresentation {
    /// Raw MathML subtree (`<math …>…</math>`) from the TEI.
    #[serde(rename = "math_ml")]
    MathML(String),
    /// LaTeX source from a `<texMath>` element.
    #[serde(rename = "tex_math")]
    TeXMath(String),
    /// Stripped visible text — the PDF-layout approximation GROBID emits
    /// when math parsing is unavailable or disabled.
    #[serde(rename = "plain_text")]
    PlainText(String),
}

impl FormulaRepresentation {
    /// The representation content (MathML XML, LaTeX source, or plain text).
    pub fn as_str(&self) -> &str {
        match self {
            FormulaRepresentation::MathML(s)
            | FormulaRepresentation::TeXMath(s)
            | FormulaRepresentation::PlainText(s) => s,
        }
    }
}

/// A deterministic TEI formula structure (M400 S03 T01).
///
/// GROBID emits formulas as `<formula xml:id="formula_0">…<label>(1)</label>
/// </formula>` inline inside `<p>` elements within the `<body>` (coordinates
/// requested via `teiCoordinates=["…","formula"]`). Section identity mirrors
/// the section extractor exactly: a formula maps to the `path_id` of the
/// section text window that contains it, or `None` when it sits outside every
/// section window (top-level gap / trailing region).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedFormula {
    /// Stable formula identifier: the `xml:id` of the `<formula>` element
    /// (e.g. "formula_0"), falling back to the deterministic ordinal key
    /// "formula-{index}".
    pub formula_id: String,
    /// `path_id` of the section whose text window contains this formula.
    /// `None` when the formula is outside every section window — typed
    /// missingness at ingest, never guessed.
    #[serde(default)]
    pub section_path_id: Option<String>,
    /// 0-based ordinal of that section in `ParsedArticle.sections` (section
    /// titles repeat, so the ordinal disambiguates, mirroring
    /// `ExtractedEntity.section_ordinal`). `None` together with
    /// `section_path_id`.
    #[serde(default)]
    pub section_ordinal: Option<usize>,
    /// Equation number/label from the `<label>` child (e.g. "(1)"), if any.
    #[serde(default)]
    pub label: Option<String>,
    /// GROBID `coords` attribute on the `<formula>` element when emitted
    /// (requested via `teiCoordinates=["…","formula"]`), else None. Format
    /// "x1,y1,x2,y2;page" (page is the last value).
    #[serde(default)]
    pub coordinates: Option<String>,
    /// The formula's representation (MathML / LaTeX / plain text).
    pub representation: FormulaRepresentation,
    /// Source char span of the formula's visible content inside the
    /// containing section's stripped text. `None` when the formula is not
    /// inside any section text window (typed missingness, never guessed).
    #[serde(default)]
    pub span: Option<CharSpan>,
    /// Deterministic SHA-256 digest of formula_id + label + coordinates +
    /// representation kind + representation content — the formula's
    /// source-digest evidence (R087).
    pub source_digest: String,
    /// Typed missingness when this formula's content is unusable (e.g. empty
    /// with no representation) — fail-closed, never invented representations.
    #[serde(default)]
    pub missingness: Option<FormulaMissingReason>,
}

/// A deterministic TEI table structure (M400 S02 T01).
///
/// GROBID emits tables as `<figure type="table" xml:id="tab_0"><head>…</head>
/// <table><row><cell …>…</cell></row></table></figure>` inside the `<body>`.
/// Section identity mirrors the section extractor exactly: a table maps to
/// the `path_id` of the section text window that contains it, or `None` when
/// it sits outside every section window (top-level gap / trailing region).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedTable {
    /// Stable table identifier: the `xml:id` of the enclosing
    /// `<figure type="table">` (e.g. "tab_0") or of the `<table>` itself,
    /// falling back to the deterministic ordinal key "table-{index}".
    pub table_id: String,
    /// `path_id` of the section whose text window contains this table.
    /// `None` when the table is outside every section window — typed
    /// missingness at ingest, never guessed.
    #[serde(default)]
    pub section_path_id: Option<String>,
    /// 0-based ordinal of that section in `ParsedArticle.sections` (section
    /// titles repeat, so the ordinal disambiguates, mirroring
    /// `ExtractedEntity.section_ordinal`). `None` together with
    /// `section_path_id`.
    #[serde(default)]
    pub section_ordinal: Option<usize>,
    /// Caption from the enclosing `<figure><head>` (e.g. "Table 1 :").
    #[serde(default)]
    pub caption: String,
    /// GROBID `coords` attribute on the enclosing figure/table when emitted,
    /// else None. Format "x1,y1,x2,y2;page" (page is the last value).
    #[serde(default)]
    pub coordinates: Option<String>,
    /// Cells in document order (row-major).
    #[serde(default)]
    pub cells: Vec<TableCell>,
    /// Deterministic SHA-256 digest of table_id + caption + coordinates +
    /// every cell digest — the table's source-digest evidence (R087).
    pub source_digest: String,
    /// Typed missingness when this table's structure is unusable (e.g. found
    /// but zero rows/cells) — fail-closed, never invented cells.
    #[serde(default)]
    pub missingness: Option<TableMissingReason>,
}

// ─── S07 P0 lift families (M464-z84in2 S07 T01) ───────────────────────────
//
// Additive wire model for the P0 losses M463 proved are already present in
// saved Java GROBID TEI: declared arXiv identity, document-ordered header
// authors, non-table figures with graphic coordinates, and figure/table
// visual callouts. The single pure TeiLifter populates these (S07 T02/T03);
// every new field is `#[serde(default)]` so legacy cached/fixture JSON keeps
// decoding (R085), and nothing is invented before the lifter implements
// (typed emptiness, never fabricated content).

/// Declared arXiv identity lifted from the TEI header (M464-z84in2 S07).
///
/// Semantics are declared-only: the identity exists when the header carries a
/// usable `<idno type="arXiv">` value (GROBID emits e.g.
/// `arXiv:2507.19457v2[cs.CL]`). `raw` preserves the full declared value
/// verbatim; `revision` carries the revision suffix ONLY when explicitly
/// declared in the raw value — never guessed and never defaulted to `v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredArxivIdentity {
    /// Full declared idno value verbatim (e.g. `"arXiv:2507.19457v2[cs.CL]"`).
    pub raw: String,
    /// Revisionless arXiv id parsed out of the declared value
    /// (e.g. `"2507.19457"`). Empty only when the raw value carries no
    /// parseable arXiv id — accompanied by typed `declared_id_missingness`.
    pub bare_id: String,
    /// Explicitly declared revision suffix (e.g. `"v2"`) when the raw value
    /// carries one; `None` when undeclared — never guessed.
    pub revision: Option<String>,
    /// Deterministic SHA-256 digest of the raw declared value — identity
    /// evidence (R087).
    pub source_digest: String,
}

/// Typed missingness for the declared arXiv idno (M464-z84in2 S07).
///
/// Accompanies `declared_arxiv_id = None`: no-silent-loss evidence of WHY the
/// header declared no usable arXiv identity (R087). A missing/eroded idno is
/// never repaired into a synthetic value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredIdMissingReason {
    /// The TEI header contains no `<idno type="arXiv">` element.
    NoIdno,
    /// An `<idno type="arXiv">` element exists but its value carries no
    /// parseable arXiv id (detail describes the unusable shape).
    Malformed(String),
}

/// One document-ordered header author (M464-z84in2 S07).
///
/// Built from a `<persName>` inside the TEI header only — bibliography
/// persName entries are citations, never authors. Order is source document
/// order (no sorting); Unicode and incomplete names (e.g. surname only) are
/// preserved verbatim — never silently dropped, never reordered, never split
/// by guesswork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderAuthor {
    /// Full display name as declared, joined in document order. Empty only
    /// when the source persName itself is empty (count honesty beats
    /// dropping the entry).
    pub name: String,
    /// Declared forename when the persName explicitly decomposes one;
    /// `None` when undeclared — never guessed from the display name.
    #[serde(default)]
    pub forename: Option<String>,
    /// Declared surname when the persName explicitly decomposes one;
    /// `None` when undeclared — never guessed from the display name.
    #[serde(default)]
    pub surname: Option<String>,
    /// Deterministic SHA-256 digest of the display name — the author's
    /// source-digest evidence (R087).
    pub source_digest: String,
}

/// Typed missingness for the graphic part of a non-table figure
/// (M464-z84in2 S07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureGraphicMissingReason {
    /// The `<figure>` element carries no `<graphic>` child at all.
    NoGraphicChild,
    /// A `<graphic>` child exists but carries no `coords` attribute.
    GraphicWithoutCoordinates,
    /// A graphic/coordinate structure is present but unusable (detail
    /// describes the unusable shape).
    Malformed(String),
}

/// A non-table figure lifted in document order (M464-z84in2 S07).
///
/// GROBID emits visual figures as `<figure xml:id="fig_0">` inside the
/// `<body>`. Table wrappers (`<figure type="table">`) are NEVER figures here
/// — they are extracted as [`ExtractedTable`]. Figures keep document order
/// (no id-based sorting); every optional part is honestly absent via
/// `Option`/typed missingness — nothing is invented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedFigure {
    /// Stable figure identifier: the `xml:id` of the `<figure>` element
    /// (e.g. `"fig_0"`), falling back to the deterministic ordinal key
    /// `"figure-{index}"` (document order, 0-based) when undeclared —
    /// mirroring the `ExtractedTable`/`ExtractedFormula` fallback convention.
    pub figure_id: String,
    /// Label from the `<label>` child (e.g. `"Figure 1"`), if any.
    #[serde(default)]
    pub label: Option<String>,
    /// Caption from the `<head>` child, if any.
    #[serde(default)]
    pub caption: Option<String>,
    /// GROBID `coords` attribute owned by the `<graphic>` child when
    /// emitted, else `None`. The value is preserved verbatim (page-prefixed
    /// comma-separated GROBID coordinate values, e.g.
    /// `"4,70.87,220.12,470.27,117.03"`) — never re-parsed for storage.
    #[serde(default)]
    pub coordinates: Option<String>,
    /// Typed graphic missingness when the figure carries no usable graphic
    /// coordinates (no graphic child / graphic without coords / malformed).
    /// `None` = graphic with declared coordinates.
    #[serde(default)]
    pub graphic_missingness: Option<FigureGraphicMissingReason>,
    /// Deterministic SHA-256 digest of figure_id + label + caption +
    /// coordinates + graphic missingness — the figure's source-digest
    /// evidence (R087).
    pub source_digest: String,
}

/// Closed kind of a visual cross-reference (M464-z84in2 S07).
///
/// Only `<ref type="figure">` and `<ref type="table">` surface here; other
/// ref kinds keep their existing semantics (`bibr` → [`CitationAnchor`], and
/// `ptr`/`note` refs stay outside this model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualCalloutKind {
    Figure,
    Table,
}

/// Typed unresolved state for a visual callout target (M464-z84in2 S07).
///
/// Callout links are kept honestly unresolved — never guessed into
/// figure/table identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutUnresolvedReason {
    /// The `<ref>` carries no usable `target` attribute (absent or empty).
    NoTarget,
    /// The declared target matches no lifted figure/table inventory entry.
    TargetNotInInventory,
}

/// One visual cross-reference in document order (M464-z84in2 S07).
///
/// Generalizes the citation-anchor section-window mechanics to
/// `<ref type="figure|table">`: closed kind, declared target, in-text quote,
/// bounded offset and section context, plus an explicit target-unresolved
/// state. Figure targets resolve against the lifted figure inventory, table
/// targets against `tables`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualCallout {
    /// Closed callout kind (figure or table).
    pub kind: VisualCalloutKind,
    /// Declared target id with any leading `#` stripped (e.g. `"fig_0"`)
    /// when the ref carries one; `None` = typed `no_target` unresolved
    /// state.
    #[serde(default)]
    pub target: Option<String>,
    /// Explicit target-unresolved state; `None` = the target resolved
    /// against the lifted figure/table inventory. Never guessed.
    #[serde(default)]
    pub unresolved: Option<CalloutUnresolvedReason>,
    /// In-text surface text of the ref.
    pub quote: String,
    /// 0-based char offset of the quote within the containing section's
    /// stripped text (`Section.text`), mirroring `CitationAnchor.offset`.
    /// `None` when there is no section text to anchor against.
    #[serde(default)]
    pub offset: Option<u32>,
    /// `path_id` of the section containing the callout; `None` when the ref
    /// is outside every section window (typed missingness, never guessed).
    #[serde(default)]
    pub section_path_id: Option<String>,
    /// 0-based ordinal of that section in `ParsedArticle.sections` (section
    /// titles repeat, so the ordinal disambiguates). `None` together with
    /// `section_path_id`.
    #[serde(default)]
    pub section_ordinal: Option<usize>,
    /// Deterministic SHA-256 digest of kind + target + quote + offset +
    /// section identity — the callout's source-digest evidence (R087).
    pub source_digest: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Section {
    /// Display title of the section (from TEI `<head>`).
    pub title: String,
    /// Full text content of the section.
    pub text: String,
    /// Depth level (0 = root, 1 = top-level, ...). Matches TEI nesting depth.
    pub level: u32,

    // ─── ADR-053 Layer A navigation fields (M362-d4qoaf/S01 T01) ───
    // All optional for backward compat with parsers that do not populate them.
    // When present, enable document-tree navigation: path_id, parent_section_id,
    // order_in_parent, section_id. The first_child / next_sibling / parent_section
    // edges are created in the ingest use case (T04) from these fields.
    /// Structural path computed from TEI nested `<div n="N">` structure,
    /// e.g. "3.2.1". Stable across re-ingest of the same PDF.
    /// `None` for legacy parsers that produce flat section lists.
    #[serde(default)]
    pub path_id: Option<String>,

    /// `section_id` (not graph node id) of the parent section in the document
    /// tree. `None` for root sections or legacy flat lists.
    #[serde(default)]
    pub parent_section_id: Option<String>,

    /// 0-based sibling index within the parent. `None` for legacy flat lists.
    #[serde(default)]
    pub order_in_parent: Option<u32>,

    /// Stable hash of `path_id + title` — used as the graph node identifier
    /// and the parent_section_id reference target. `None` for legacy lists.
    #[serde(default)]
    pub section_id: Option<String>,
}

/// One in-text occurrence of a citation in the source document
/// (GROBID TEI `<ref type="bibr" target="#bN">`). Anchors are
/// revision-owned evidence: they are stored on PaperRevision CITES/HAS_PART
/// edges, never on the globally deduped Citation/Reference nodes (D247/D249),
/// so v1/v5 spans and cross-paper locations cannot mix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationAnchor {
    /// GROBID reference id — the `xml:id` of the cited `<biblStruct>`
    /// (e.g. "b12"), with any leading `#` from the `<ref target>` stripped.
    pub ref_id: String,
    /// `path_id` of the section containing the anchor (e.g. "3.2"), or `None`
    /// when the ref is inside the body but not within any section text window
    /// (typed missingness `anchor_section_path_unresolved` at ingest).
    pub section_path_id: Option<String>,
    /// In-text surface text of the ref (e.g. "[12]" or "(Shao et al., 2024)").
    pub quote: String,
    /// 0-based char offset of the quote within the containing section's
    /// stripped text (`Section.text`). `None` when there is no section text
    /// to anchor against.
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationEntry {
    pub raw_text: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    /// In-text anchors (GROBID `<ref type="bibr">` occurrences) mapped to
    /// this bibliography entry. Empty when the TEI has no resolvable in-text
    /// refs — ingest then records typed missingness `no_in_text_anchor` on
    /// the revision-owned edge. `#[serde(default)]` keeps legacy cached/fixture
    /// JSON decodable (R085).
    #[serde(default)]
    pub anchors: Vec<CitationAnchor>,
}

/// Parser port — GROBID HTTP + HTML parser implement this.
#[async_trait]
pub trait ParserPort: Send + Sync {
    /// Parse a PDF file into a ParsedArticle (GROBID path).
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle>;

    /// Parse an HTML file into a ParsedArticle (textbook/lecture path).
    /// Default implementation returns Unavailable — adapters override.
    async fn parse_html(&self, _html_path: &str, _paper_id: &str) -> ParseResult<ParsedArticle> {
        Err(ParserError::Unavailable(
            "HTML parsing not supported by this parser".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{PageOcrReasons, PdfClassification, PdfType};

    fn classification(pages: Vec<u32>, reasons: Vec<PageOcrReasons>) -> PdfClassification {
        PdfClassification {
            pdf_type: PdfType::Mixed,
            page_count: 10,
            pages_needing_ocr: pages,
            ocr_reasons_by_page: reasons,
            confidence: 0.9,
            has_encoding_issues: false,
        }
    }

    #[test]
    fn page_text_missingness_from_maps_pages_and_reasons() {
        let stamped = page_text_missingness_from(&classification(
            vec![3, 7],
            vec![PageOcrReasons {
                page: 3,
                reasons: vec!["suspected_garbled_text".into()],
            }],
        ));
        assert_eq!(stamped.len(), 2);
        assert_eq!(stamped[0].page, 3);
        assert_eq!(stamped[0].reasons, ["suspected_garbled_text".to_string()]);
        assert_eq!(stamped[1].page, 7);
        assert!(
            stamped[1].reasons.is_empty(),
            "missing reason vector is empty, never invented"
        );
    }

    #[test]
    fn page_text_missingness_from_clean_classification_is_empty() {
        let stamped = page_text_missingness_from(&classification(vec![], vec![]));
        assert!(stamped.is_empty());
    }
}
