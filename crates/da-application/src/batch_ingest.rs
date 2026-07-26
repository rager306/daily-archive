//! Batch ingest: single-process HOT path + snapshot export.
//!
//! ADR-041 Solution B: one process creates embedded store, ingests all
//! papers via direct GraphStore API (<1ms each), exports .sgsnap for durability.
//!
//! Reuses the same ingest logic as IngestUseCase — no duplication.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngestResult {
    pub total: usize,
    pub ok: usize,
    pub fail: usize,
    pub total_body_chars: usize,
    pub total_sections: usize,
    pub total_citations: usize,
    pub total_cites_resolved: usize,
    pub duration_ms: u64,
    pub snapshot_path: Option<String>,
    pub errors: Vec<(String, String)>,
    pub import_eligible: bool, // always false (D127)
}

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
    let mut errors = Vec::new();

    for (pdf_path, paper_id) in pdfs {
        match ingest.ingest_pdf(pdf_path, paper_id).await {
            Ok(result) => {
                ok += 1;
                total_body_chars += result.body_chars;
                total_sections += result.section_count;
                total_citations += result.citation_count;
                total_cites_resolved += result.cites_resolved;
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
        total, ok, fail, total_body_chars, duration_ms,
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
        duration_ms,
        snapshot_path,
        errors,
        import_eligible: false, // D127
    })
}
