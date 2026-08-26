//! This-cut TEI IR: raw TEI plus tissues already lifted onto today's article.

use crate::http::GrobidParser;
use da_ports::parser::{
    CitationEntry, ExtractedFormula, ExtractedTable, FormulaMissingReason, Section,
    TableMissingReason,
};

/// Intermediate representation for the GROBID PDF path.
///
/// Stores the TEI string and the tissues already on today's `ParsedArticle`
/// (KTD1 / AE9). Author lists, article-level idno, and non-table figures are
/// intentionally absent — they are not parsed this cut.
#[derive(Debug, Clone)]
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
    pub(crate) fn lift_tei(
        tei_xml: String,
        paper_id: impl Into<String>,
        pdf_hash: impl Into<String>,
    ) -> TeiParseIr {
        let title = Self::extract_title(&tei_xml);
        let abstract_text = Self::extract_abstract(&tei_xml);
        let body_text = Self::extract_body(&tei_xml);
        let sections = Self::extract_sections(&tei_xml);
        let citations = Self::extract_citations(&tei_xml);
        let (tables, table_missingness) = Self::extract_tables(&tei_xml);
        let (formulas, formula_missingness) = Self::extract_formulas(&tei_xml);
        TeiParseIr {
            tei_xml,
            paper_id: paper_id.into(),
            title,
            abstract_text,
            body_text,
            sections,
            citations,
            tables,
            table_missingness,
            formulas,
            formula_missingness,
            pdf_hash: pdf_hash.into(),
        }
    }
}
