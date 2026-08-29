//! This-cut TEI IR: raw TEI plus tissues already lifted onto today's article.

use crate::http::GrobidParser;
use crate::lift::TeiLifter;
use da_ports::parser::{
    CitationEntry, DeclaredArxivIdentity, DeclaredIdMissingReason, ExtractedFigure,
    ExtractedFormula, ExtractedTable, FormulaMissingReason, HeaderAuthor, Section,
    TableMissingReason, VisualCallout,
};
use serde::{Deserialize, Serialize};

/// Intermediate representation for the GROBID PDF path.
///
/// Stores the TEI string and the tissues already on today's `ParsedArticle`
/// (KTD1 / AE9).
///
/// Since M464-z84in2 S07/T01 the IR mirrors the additive P0 wire families
/// (declared arXiv identity + typed missingness, document-ordered header
/// authors, non-table figures with graphic coordinates, figure/table visual
/// callouts) with `#[serde(default)]` on every new field. The single
/// `TeiLifter` populates them in S07 T02/T03 — until then they stay at typed
/// emptiness, never invented.
///
/// `Serialize`/`Deserialize` since M464-z84in2 S02/T02: the TeiLifter
/// parity tests compare serialized IRs byte-for-byte between the
/// provider-neutral envelope path and the compatibility path (R105).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeiParseIr {
    pub tei_xml: String,
    pub paper_id: String,
    pub title: String,
    pub abstract_text: String,
    pub body_text: String,
    pub sections: Vec<Section>,
    pub citations: Vec<CitationEntry>,
    pub tables: Vec<ExtractedTable>,
    pub table_missingness: Option<TableMissingReason>,
    pub formulas: Vec<ExtractedFormula>,
    pub formula_missingness: Option<FormulaMissingReason>,
    /// S07 P0 mirror families (serde default keeps legacy IR JSON decodable).
    #[serde(default)]
    pub declared_arxiv_id: Option<DeclaredArxivIdentity>,
    #[serde(default)]
    pub declared_id_missingness: Option<DeclaredIdMissingReason>,
    #[serde(default)]
    pub header_authors: Vec<HeaderAuthor>,
    #[serde(default)]
    pub figures: Vec<ExtractedFigure>,
    #[serde(default)]
    pub visual_callouts: Vec<VisualCallout>,
    pub pdf_hash: String,
}

impl GrobidParser {
    /// Lift raw TEI into this-cut IR. Does not call Java.
    ///
    /// `pub` since M464-z84in2 S01/T03: `parse.stage.lift_tei` is a recorded
    /// compatibility stage (data/parse/api_compatibility_ledger.yaml) and the
    /// parse-compat baseline emitter is its first sanctioned out-of-crate
    /// consumer. Since S02/T02 this is a thin delegate onto the single
    /// provider-neutral algorithm ([`TeiLifter::lift_tei`]) — semantics,
    /// determinism, signature and the intentional-absence list are unchanged
    /// (compatibility obligation stays in force).
    pub fn lift_tei(
        tei_xml: String,
        paper_id: impl Into<String>,
        pdf_hash: impl Into<String>,
    ) -> TeiParseIr {
        TeiLifter::lift_tei(tei_xml, paper_id, pdf_hash)
    }
}
