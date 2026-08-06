//! Batch ingest: single-process HOT path + snapshot export.
//!
//! ADR-041 Solution B: one process creates embedded store, ingests all
//! papers via direct GraphStore API (<1ms each), exports .sgsnap for durability.
//!
//! Reuses the same ingest logic as IngestUseCase — no duplication.

use da_domain::{PdfClassification, PdfType};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Aggregate pre-GROBID PDF triage across a batch (M352).
///
/// Built only from successful `IngestResult.pdf_classification` values.
/// Empty / zero when no classifier was attached or all classifies failed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BatchPdfTriageSummary {
    /// Papers with a successful classification result.
    pub classified: usize,
    /// Papers with at least one OCR-flagged page.
    pub with_ocr_pages: usize,
    /// Papers with soft `has_encoding_issues`.
    pub encoding_issues: usize,
    pub type_text: usize,
    pub type_scanned: usize,
    pub type_image: usize,
    pub type_mixed: usize,
    pub type_unknown: usize,
    /// Sample paper ids that had OCR pages (capped; full list not required).
    pub papers_with_ocr: Vec<String>,
}

impl BatchPdfTriageSummary {
    const OCR_SAMPLE_CAP: usize = 32;

    /// Fold one successful classification into the summary.
    pub fn record(&mut self, paper_id: &str, c: &PdfClassification) {
        self.classified += 1;
        if c.has_encoding_issues {
            self.encoding_issues += 1;
        }
        if !c.pages_needing_ocr.is_empty() {
            self.with_ocr_pages += 1;
            if self.papers_with_ocr.len() < Self::OCR_SAMPLE_CAP {
                self.papers_with_ocr.push(paper_id.to_string());
            }
        }
        match c.pdf_type {
            PdfType::Text => self.type_text += 1,
            PdfType::Scanned => self.type_scanned += 1,
            PdfType::Image => self.type_image += 1,
            PdfType::Mixed => self.type_mixed += 1,
            PdfType::Unknown => self.type_unknown += 1,
        }
    }

    /// True when any paper was classified (classifier was effective).
    pub fn any(&self) -> bool {
        self.classified > 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngestResult {
    pub total: usize,
    pub ok: usize,
    pub fail: usize,
    pub total_body_chars: usize,
    pub total_sections: usize,
    pub total_citations: usize,
    pub total_cites_resolved: usize,
    pub total_references_written: usize,
    pub duration_ms: u64,
    pub snapshot_path: Option<String>,
    pub errors: Vec<(String, String)>,
    pub import_eligible: bool, // always false (D127)
    /// Aggregate PDF triage from papers that were classified (M352).
    pub pdf_triage: BatchPdfTriageSummary,
}

#[tracing::instrument(
    skip(ingest, pdfs),
    fields(batch_size = pdfs.len()),
    name = "batch_ingest"
)]
pub async fn batch_ingest_pdfs(
    ingest: &super::ingest::IngestUseCase,
    pdfs: &[(String, String)], // (pdf_path, paper_id)
    snapshot_output: Option<&Path>,
) -> anyhow::Result<BatchIngestResult> {
    let start = std::time::Instant::now();
    let mut ok = 0;
    let mut fail = 0;
    let mut total_body_chars = 0;
    let mut total_sections = 0;
    let mut total_citations = 0;
    let mut total_cites_resolved = 0;
    let mut total_references_written = 0;
    let mut errors = Vec::new();
    let mut pdf_triage = BatchPdfTriageSummary::default();

    for (pdf_path, paper_id) in pdfs {
        match ingest.ingest_pdf(pdf_path, paper_id).await {
            Ok(result) => {
                ok += 1;
                total_body_chars += result.body_chars;
                total_sections += result.section_count;
                total_citations += result.citation_count;
                total_cites_resolved += result.cites_resolved;
                total_references_written += result.references_written;
                if let Some(ref c) = result.pdf_classification {
                    pdf_triage.record(paper_id, c);
                }
                tracing::info!(paper_id, "Ingested OK");
            }
            Err(e) => {
                fail += 1;
                let err_msg = format!("{e:#}");
                tracing::error!(paper_id, error = %err_msg, "Ingest FAILED");
                errors.push((paper_id.clone(), err_msg));
            }
        }
    }

    let snapshot_path = if let Some(path) = snapshot_output {
        match ingest.graph_store.export_snapshot().await {
            Ok(data) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(path, &data)?;
                tracing::info!(path = %path.display(), size = data.len(), "Snapshot exported");
                Some(path.display().to_string())
            }
            Err(e) => {
                tracing::warn!(error = %e, "Snapshot export failed (non-fatal)");
                None
            }
        }
    } else {
        None
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let total = pdfs.len();

    tracing::info!(
        total,
        ok,
        fail,
        total_body_chars,
        duration_ms,
        classified = pdf_triage.classified,
        with_ocr_pages = pdf_triage.with_ocr_pages,
        snapshot = ?snapshot_path,
        "Batch ingest complete"
    );

    Ok(BatchIngestResult {
        total,
        ok,
        fail,
        total_body_chars,
        total_sections,
        total_citations,
        total_cites_resolved,
        total_references_written,
        duration_ms,
        snapshot_path,
        errors,
        import_eligible: ingest.eligibility.is_eligible(),
        pdf_triage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::PdfType;

    fn sample(pdf_type: PdfType, ocr: &[u32], encoding: bool) -> PdfClassification {
        PdfClassification {
            pdf_type,
            page_count: 10,
            pages_needing_ocr: ocr.to_vec(),
            ocr_reasons_by_page: vec![],
            confidence: 0.9,
            has_encoding_issues: encoding,
        }
    }

    #[test]
    fn triage_record_counts_types_and_ocr() {
        let mut s = BatchPdfTriageSummary::default();
        s.record("a", &sample(PdfType::Text, &[], false));
        s.record("b", &sample(PdfType::Text, &[69, 84], true));
        s.record("c", &sample(PdfType::Mixed, &[1], false));
        s.record("d", &sample(PdfType::Scanned, &[1, 2, 3], false));

        assert_eq!(s.classified, 4);
        assert_eq!(s.with_ocr_pages, 3);
        assert_eq!(s.encoding_issues, 1);
        assert_eq!(s.type_text, 2);
        assert_eq!(s.type_mixed, 1);
        assert_eq!(s.type_scanned, 1);
        assert_eq!(s.papers_with_ocr, vec!["b", "c", "d"]);
        assert!(s.any());
    }

    #[test]
    fn triage_ocr_sample_capped() {
        let mut s = BatchPdfTriageSummary::default();
        for i in 0..40 {
            s.record(&format!("p{i}"), &sample(PdfType::Text, &[1], false));
        }
        assert_eq!(s.with_ocr_pages, 40);
        assert_eq!(
            s.papers_with_ocr.len(),
            BatchPdfTriageSummary::OCR_SAMPLE_CAP
        );
    }

    #[test]
    fn triage_empty_default() {
        let s = BatchPdfTriageSummary::default();
        assert!(!s.any());
        assert_eq!(s.classified, 0);
    }
}
