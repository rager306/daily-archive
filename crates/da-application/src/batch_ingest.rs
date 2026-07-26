//! Batch ingest: single-process HOT path + snapshot export.
//!
//! ADR-041 Solution B: one process creates embedded store, ingests all
//! papers via direct GraphStore API (<1ms each), exports .sgsnap for durability.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngestResult {
    pub total: usize,
    pub ok: usize,
    pub fail: usize,
    pub total_body_chars: usize,
    pub duration_ms: u64,
    pub snapshot_path: Option<String>,
    pub errors: Vec<(String, String)>,
    pub import_eligible: bool, // always false (D127)
}

pub async fn batch_ingest_pdfs(
    parser: &dyn da_ports::parser::ParserPort,
    embedder: &dyn da_ports::embedder::Embedder,
    graph_store: &dyn da_ports::graph_store::DirectGraphStore,
    pdfs: &[(String, String)], // (pdf_path, paper_id)
    snapshot_output: Option<&Path>,
) -> anyhow::Result<BatchIngestResult> {
    use da_domain::vid;
    use da_ports::graph_store::GraphStore;

    let start = std::time::Instant::now();
    let mut ok = 0;
    let mut fail = 0;
    let mut total_body_chars = 0;
    let mut errors = Vec::new();

    // Ensure vector index exists
    graph_store
        .create_vector_index("Paper", "embedding", embedder.dimensions(), da_ports::graph_store::VectorMetric::Cosine)
        .await
        .ok(); // ignore if already exists

    for (pdf_path, paper_id) in pdfs {
        tracing::info!(paper_id, pdf_path, "Batch ingest");

        match ingest_one(parser, embedder, graph_store, pdf_path, paper_id).await {
            Ok(body_chars) => {
                ok += 1;
                total_body_chars += body_chars;
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

    // Export snapshot for durability
    let snapshot_path = if let Some(path) = snapshot_output {
        match graph_store.export_snapshot().await {
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
        duration_ms,
        snapshot_path,
        errors,
        import_eligible: false, // D127
    })
}

async fn ingest_one(
    parser: &dyn da_ports::parser::ParserPort,
    embedder: &dyn da_ports::embedder::Embedder,
    graph_store: &dyn da_ports::graph_store::DirectGraphStore,
    pdf_path: &str,
    paper_id: &str,
) -> anyhow::Result<usize> {
    use da_domain::paper::Paper;
    use da_domain::vid;

    // 1. Parse
    let parsed = parser.parse_pdf(pdf_path, paper_id).await?;

    // 2. Embed
    let embed_text = if parsed.abstract_text.is_empty() {
        &parsed.title
    } else {
        &parsed.abstract_text
    };
    let vector = embedder.embed(embed_text).await?;

    // 3. Create domain Paper
    let paper = Paper::new(paper_id, &parsed.title);
    let vid_str = vid::paper_vid(paper_id);
    let now = chrono::Utc::now().timestamp();

    // 4. HOT PATH: Direct GraphStore API
    let node_id = graph_store.create_node("Paper").await?;
    graph_store.set_node_property_string(node_id, "vid", vid_str.clone()).await?;
    graph_store.set_node_property_string(node_id, "arxiv_id", paper.arxiv_id.clone()).await?;
    graph_store.set_node_property_string(node_id, "title", paper.title.clone()).await?;
    graph_store.set_node_property_string(node_id, "pdf_hash", parsed.pdf_hash.clone()).await?;
    graph_store.set_node_property_int(node_id, "valid_from", now).await?;
    graph_store.set_node_property_int(node_id, "schema_version", 1).await?;
    graph_store.set_node_property_bool(node_id, "evidence_ready", false).await?;
    graph_store.set_node_property_bool(node_id, "import_eligible", false).await?;
    graph_store.add_vector("Paper", "embedding", node_id, vector).await?;

    tracing::debug!(paper_id, node_id, vid = %vid_str, "Node created (HOT path)");

    Ok(parsed.body_text.len())
}
