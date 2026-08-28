//! This-cut TEI IR: raw TEI plus tissues already lifted onto today's article.

use crate::http::GrobidParser;
use crate::lift::TeiLifter;
use da_ports::parser::{
    CitationEntry, ExtractedFormula, ExtractedTable, FormulaMissingReason, Section,
    TableMissingReason,
};
use serde::{Deserialize, Serialize};

/// Intermediate representation for the GROBID PDF path.
///
/// Stores the TEI string and the tissues already on today's `ParsedArticle`
/// (KTD1 / AE9). Author lists, article-level idno, and non-table figures are
/// intentionally absent — they are not parsed this cut.
///
/// `Serialize`/`Deserialize` since M464-z84in2 S02/T02: the TeiLifter
/// parity tests compare serialized IRs byte-for-byte between the
/// provider-neutral envelope path and the compatibility path (R105).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
