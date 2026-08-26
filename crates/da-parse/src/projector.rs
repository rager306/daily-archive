//! Pure projector from this-cut TEI IR onto `ParsedArticle`.

use crate::http::GrobidParser;
use crate::ir::TeiParseIr;
use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort};

impl GrobidParser {
    /// Project IR onto the ports article. `layout_json` and
    /// `page_text_missingness` stay `None` on the GROBID path (classifier stamps later).
    pub(crate) fn project_article(ir: TeiParseIr) -> ParsedArticle {
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
            page_text_missingness: None,
        }
    }
}

#[async_trait]
impl ParserPort for GrobidParser {
    #[tracing::instrument(skip(self, pdf_path), fields(paper_id = %paper_id), name = "grobid_parse")]
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        tracing::info!(paper_id, pdf_path, "GROBID parsing");

        let pdf_bytes = tokio::fs::read(pdf_path)
            .await
            .map_err(|e| ParserError::ParseFailed(format!("read PDF: {e}")))?;

        let pdf_size = pdf_bytes.len();
        let pdf_hash = Self::pdf_hash(&pdf_bytes);
        tracing::debug!(
            paper_id,
            pdf_size,
            pdf_hash = %pdf_hash,
            "PDF read, sending to GROBID"
        );

        let request_start = std::time::Instant::now();
        let tei_xml = self.process_fulltext(&pdf_bytes).await?;
        let grobid_elapsed_ms = request_start.elapsed().as_millis() as u64;
        let tei_size = tei_xml.len();
        tracing::info!(
            paper_id,
            api = "grobid",
            elapsed_ms = grobid_elapsed_ms,
            pdf_size,
            tei_size,
            "GROBID processFulltextDocument response received"
        );

        let ir = Self::lift_tei(tei_xml, paper_id, pdf_hash);
        let sections_with_path_id = ir.sections.iter().filter(|s| s.path_id.is_some()).count();
        let max_depth = ir
            .sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .map(|p| p.split('.').count())
            .max()
            .unwrap_or(0);
        let table_cells: usize = ir.tables.iter().map(|t| t.cells.len()).sum();
        let formula_reprs: usize = ir
            .formulas
            .iter()
            .filter(|f| f.missingness.is_none())
            .count();

        tracing::info!(
            paper_id,
            title = %ir.title,
            body_chars = ir.body_text.len(),
            abstract_chars = ir.abstract_text.len(),
            sections = ir.sections.len(),
            sections_with_path_id,
            max_tree_depth = max_depth,
            citations = ir.citations.len(),
            tables = ir.tables.len(),
            table_cells,
            table_missingness = ?ir.table_missingness,
            formulas = ir.formulas.len(),
            formula_reprs,
            formula_missingness = ?ir.formula_missingness,
            pdf_hash = %ir.pdf_hash,
            "GROBID parsed: extraction complete"
        );

        if sections_with_path_id == 0 && !ir.sections.is_empty() {
            tracing::warn!(
                paper_id,
                "GROBID TEI: no sections have path_id — flat structure without <head n=...> attributes"
            );
        }

        Ok(Self::project_article(ir))
    }
}
