//! Materialize one verified gold cache wave into one shared Samyama graph.

use anyhow::{Context, Result};
use clap::Parser;
use da_adapters::{FilePipelineCache, SamyamaGraphStore};
use da_cli::gold_corpus::GoldCorpusManifest;
use da_cli::gold_graph::{atomic_write, load_wave_report, materialize_wave, sha256};
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    wave_report: PathBuf,
    #[arg(long)]
    cache_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/gold_corpus/gold_corpus_manifest.json"
    )]
    manifest: PathBuf,
    #[arg(long)]
    input_snapshot: Option<PathBuf>,
    #[arg(long)]
    output_snapshot: PathBuf,
    #[arg(long)]
    report: PathBuf,
}

type CatalogMetadata = (Option<String>, Option<String>);

#[derive(Debug, Deserialize)]
struct ArticleMetadata {
    source_code: Option<String>,
    coarse_topic_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct GraphWaveReport {
    materialization: da_cli::gold_graph::BatchGraphMaterialization,
    input_snapshot_sha256: Option<String>,
    output_snapshot_sha256: String,
    output_snapshot: String,
    same_process: bool,
    import_eligible: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    kg_storage::refuse_persistence_path_from_env()?;
    let wave = load_wave_report(&args.wave_report)?;
    let manifest_bytes = std::fs::read(&args.manifest)
        .with_context(|| format!("read {}", args.manifest.display()))?;
    anyhow::ensure!(
        sha256(&manifest_bytes) == wave.manifest_sha256,
        "wave report does not bind the supplied corpus manifest"
    );
    let manifest: GoldCorpusManifest =
        serde_json::from_slice(&manifest_bytes).context("parse corpus manifest")?;
    let metadata = metadata_index(&manifest)?;

    let concrete = Arc::new(SamyamaGraphStore::from_env());
    let input_snapshot_sha256 = if let Some(path) = &args.input_snapshot {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        concrete.import_snapshot(&bytes).await?;
        Some(sha256(&bytes))
    } else {
        None
    };
    let store: Arc<dyn DirectGraphStore> = concrete.clone();
    let materialization = materialize_wave(
        store,
        &FilePipelineCache::new(&args.cache_root),
        &wave,
        |paper_id| {
            metadata
                .get(paper_id)
                .cloned()
                .with_context(|| format!("canonical metadata missing for {paper_id}"))
        },
    )
    .await?;
    if !materialization.passed() {
        eprintln!(
            "materialization_failure={}",
            serde_json::to_string(&materialization)?
        );
    }
    anyhow::ensure!(
        materialization.passed(),
        "cache-to-graph replay gate failed"
    );
    anyhow::ensure!(
        concrete.node_count().await > 0,
        "materialized graph is empty"
    );

    let snapshot = concrete.export_snapshot().await?;
    let output_snapshot_sha256 = sha256(&snapshot);
    atomic_write(&args.output_snapshot, &snapshot)?;
    let report = GraphWaveReport {
        materialization,
        input_snapshot_sha256,
        output_snapshot_sha256,
        output_snapshot: args.output_snapshot.display().to_string(),
        same_process: true,
        import_eligible: false,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn metadata_index(manifest: &GoldCorpusManifest) -> Result<BTreeMap<String, CatalogMetadata>> {
    let mut index = BTreeMap::new();
    for record in &manifest.records {
        let Some(source) = record.source.path.as_deref() else {
            continue;
        };
        let metadata_path = article_root(Path::new(source)).map(|root| root.join("article.json"));
        let value = metadata_path
            .filter(|path| path.is_file())
            .map(|path| {
                serde_json::from_slice::<ArticleMetadata>(
                    &std::fs::read(&path).with_context(|| format!("read {}", path.display()))?,
                )
                .with_context(|| format!("parse {}", path.display()))
            })
            .transpose()?;
        let fallback = infer_catalog_metadata(Path::new(source));
        index.insert(
            record.paper_id.clone(),
            (
                value
                    .as_ref()
                    .and_then(|item| item.coarse_topic_code.clone())
                    .or(fallback.0),
                value
                    .as_ref()
                    .and_then(|item| item.source_code.clone())
                    .or(fallback.1),
            ),
        );
    }
    Ok(index)
}

fn article_root(source: &Path) -> Option<PathBuf> {
    let parent = source.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some("source") {
        parent.parent().map(Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn infer_catalog_metadata(source: &Path) -> CatalogMetadata {
    let parts = source
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let Some(index) = parts.iter().position(|part| part == "arxiv") else {
        return (None, None);
    };
    (parts.get(index + 1).cloned(), Some("arxiv".into()))
}
