//! Pure projector from this-cut TEI IR onto `ParsedArticle`.
//!
//! Since M464-z84in2 S02/T03 the runtime `impl ParserPort for GrobidParser`
//! lives in [`crate::parser`], where the facade delegates through the
//! provider-neutral [`crate::parser::ScholarlyParser`] composition. This
//! module stays the pure projection stage consumed by that composition and
//! by the recorded `parse.stage.project_article` compatibility surface.

use crate::ir::TeiParseIr;
use da_ports::parser::ParsedArticle;

impl crate::http::GrobidParser {
    /// Project IR onto the ports article. `layout_json` and
    /// `page_text_missingness` stay `None` on the GROBID path (classifier stamps later).
    ///
    /// `pub` since M464-z84in2 S01/T03: `parse.stage.project_article` is a
    /// recorded compatibility stage and the parse-compat baseline emitter is
    /// its first sanctioned out-of-crate consumer. Projection stays total and
    /// allocation-safe over any `TeiParseIr` (compatibility obligation).
    pub fn project_article(ir: TeiParseIr) -> ParsedArticle {
        ParsedArticle {
            paper_id: ir.paper_id,
            title: ir.title,
            abstract_text: ir.abstract_text,
            body_text: ir.body_text,
            sections: ir.sections,
            citations: ir.citations,
            layout_json: None,
            tei_xml: Some(ir.tei_xml),
            pdf_hash: ir.pdf_hash,
            tables: ir.tables,
            table_missingness: ir.table_missingness,
            formulas: ir.formulas,
            formula_missingness: ir.formula_missingness,
            // S07 P0 families flow through verbatim (typed emptiness until
            // S07 T02/T03 populate the IR side).
            declared_arxiv_id: ir.declared_arxiv_id,
            declared_id_missingness: ir.declared_id_missingness,
            header_authors: ir.header_authors,
            figures: ir.figures,
            visual_callouts: ir.visual_callouts,
            page_text_missingness: None,
        }
    }
}
