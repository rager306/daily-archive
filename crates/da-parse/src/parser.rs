//! Provider-neutral parse composition (R105): backend → envelope → lift →
//! projection, assembled exactly once.
//!
//! [`ScholarlyParser`] is the runtime closure over the S02 seams: it invokes
//! a [`ParseBackend`] **exactly once**, hands the resulting [`TeiEnvelope`]
//! to the pure [`TeiLifter`] (the single lift algorithm owner since S02/T02)
//! and projects the historical [`ParsedArticle`] through the sanctioned
//! projection stage (`GrobidParser::project_article`). The composition owns
//! no filesystem, no env, no HTTP — those stay in the [`GrobidParser`]
//! compatibility facade shell and in backends.
//!
//! The facade's `impl ParserPort` (moved here from the projector module in
//! S02/T03) delegates through this composition, so the `grobid_parse`
//! lifecycle span, the identity/size/timing-only aggregate logs and the
//! external `ParserError` mapping are unchanged for existing
//! `Arc<dyn ParserPort>` callers.

use crate::backend::ParseBackend;
use crate::envelope::TeiEnvelope;
use crate::http::GrobidParser;
use crate::lift::TeiLifter;
use crate::transport::ParseBackendFailure;
use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort};

/// Compose one backend fetch with the pure TEI lift and the ParserPort
/// projection (R105).
///
/// Small interface, deep module: the caller supplies a backend plus source
/// bytes; the exactly-once fetch discipline, bytes→text decoding (lossy,
/// total — never TEI normalization) and identity-free projection all stay
/// inside. Offline cached-TEI replay and the GROBID HTTP path are the same
/// composition, differing only in the backend handed to [`Self::new`].
pub struct ScholarlyParser<B: ParseBackend> {
    backend: B,
}

impl<B: ParseBackend> ScholarlyParser<B> {
    /// Compose the pipeline over `backend`.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// The backend this composition drives (exactly one call per parse).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Consume the composition, returning the backend.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Full composition: exactly one backend call, then pure lift and
    /// projection.
    ///
    /// A backend failure short-circuits the pipeline: neither lift nor
    /// projection executes (stateful fake assertions in
    /// `tests/parser_composition.rs`).
    pub async fn parse(
        &self,
        paper_id: &str,
        pdf_bytes: impl Into<Vec<u8>>,
    ) -> ParseResult<ParsedArticle> {
        let source = pdf_bytes.into();
        let pdf_hash = TeiLifter::pdf_hash(&source);
        // Single compatibility conversion boundary: typed backend failures
        // map to the historical `ParserError` exactly once, here — never a
        // second backend call, never a lift/projection of a failed fetch.
        let envelope = self
            .fetch_envelope_once(paper_id, source)
            .await
            .map_err(|failure| failure.to_parser_error())?;
        Ok(self.lift_and_project(paper_id, pdf_hash, envelope))
    }

    /// Run the backend exactly once and return its envelope.
    ///
    /// Single-call discipline: no hidden retries. Since S04/T01 the typed
    /// failure surfaces verbatim (`Result<_, ParseBackendFailure>`), so typed
    /// consumers keep the structured status/phase/limit/retry metadata; the
    /// historical `ParserError` mapping happens exactly once at the
    /// compatibility boundary via `ParseBackendFailure::to_parser_error`.
    pub async fn fetch_envelope_once(
        &self,
        paper_id: &str,
        pdf_bytes: impl Into<Vec<u8>>,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let request = self.backend.default_request(paper_id, pdf_bytes.into());
        self.backend.fetch_tei(request).await
    }

    /// Pure stage: lift the envelope's verbatim TEI and project the
    /// historical `ParsedArticle`.
    ///
    /// Never touches the backend — `pdf_hash` is handed in by the caller so
    /// this stage stays a pure function of (envelope bytes, logical id, hash).
    /// `tei_xml` stays `Some(raw decoded TEI)`, `layout_json` and
    /// `page_text_missingness` stay `None` (classifier stamps later).
    pub fn lift_and_project(
        &self,
        paper_id: &str,
        pdf_hash: impl Into<String>,
        envelope: TeiEnvelope,
    ) -> ParsedArticle {
        let ir = TeiLifter::new().lift(&envelope, paper_id, pdf_hash);
        GrobidParser::project_article(ir)
    }
}

// ---------------------------------------------------------------------------
// GrobidParser compatibility facade: ParserPort delegates to the composition
// ---------------------------------------------------------------------------

#[async_trait]
impl ParserPort for GrobidParser {
    #[tracing::instrument(skip(self, pdf_path), fields(paper_id = %paper_id), name = "grobid_parse")]
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        tracing::info!(paper_id, pdf_path, "GROBID parsing");

        // Compatibility-shell concern: filesystem stays in the facade.
        let pdf_bytes = tokio::fs::read(pdf_path)
            .await
            .map_err(|e| ParserError::ParseFailed(format!("read PDF: {e}")))?;

        let pdf_size = pdf_bytes.len();
        let pdf_hash = TeiLifter::pdf_hash(&pdf_bytes);
        tracing::debug!(
            paper_id,
            pdf_size,
            pdf_hash = %pdf_hash,
            "PDF read, sending to GROBID"
        );

        // Delegation to the provider-neutral composition (R105): the facade
        // drives backend→envelope→lift→project through ScholarlyParser while
        // keeping its historical aggregate, identity/size/timing-only events
        // (raw TEI/PDF never reach logs).
        let composition = ScholarlyParser::new(self.backend().clone());

        let request_start = std::time::Instant::now();
        let envelope = composition
            .fetch_envelope_once(paper_id, pdf_bytes)
            .await
            .map_err(|failure| failure.to_parser_error())?;
        let grobid_elapsed_ms = request_start.elapsed().as_millis() as u64;
        let tei_size = envelope.raw_tei().len();
        tracing::info!(
            paper_id,
            api = "grobid",
            elapsed_ms = grobid_elapsed_ms,
            pdf_size,
            tei_size,
            "GROBID processFulltextDocument response received"
        );

        let parsed = composition.lift_and_project(paper_id, pdf_hash, envelope);

        let sections_with_path_id = parsed
            .sections
            .iter()
            .filter(|s| s.path_id.is_some())
            .count();
        let max_depth = parsed
            .sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .map(|p| p.split('.').count())
            .max()
            .unwrap_or(0);
        let table_cells: usize = parsed.tables.iter().map(|t| t.cells.len()).sum();
        let formula_reprs: usize = parsed
            .formulas
            .iter()
            .filter(|f| f.missingness.is_none())
            .count();

        tracing::info!(
            paper_id,
            title = %parsed.title,
            body_chars = parsed.body_text.len(),
            abstract_chars = parsed.abstract_text.len(),
            sections = parsed.sections.len(),
            sections_with_path_id,
            max_tree_depth = max_depth,
            citations = parsed.citations.len(),
            tables = parsed.tables.len(),
            table_cells,
            table_missingness = ?parsed.table_missingness,
            formulas = parsed.formulas.len(),
            formula_reprs,
            formula_missingness = ?parsed.formula_missingness,
            pdf_hash = %parsed.pdf_hash,
            "GROBID parsed: extraction complete"
        );

        if sections_with_path_id == 0 && !parsed.sections.is_empty() {
            tracing::warn!(
                paper_id,
                "GROBID TEI: no sections have path_id — flat structure without <head n=...> attributes"
            );
        }

        Ok(parsed)
    }
}
