//! Same-process semantic DAG fragment: document context + ConceptCluster.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use da_adapters::FilePipelineCache;
use da_application::{
    ArtifactPayloadStage, ClusterDetectionStage, ClusterUseCase, DocumentContextProjection,
    DocumentContextStage, EntityPapersArtifact, PipelineOrchestrator, SectionSummaryInput,
    StageDisposition,
};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_cache::{CacheLookup, PipelineCache, SourceFormat, SourceIdentity};
use kg_pipeline::PipelineBuilder;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    article_id: String,
    #[arg(long)]
    parsed_article: PathBuf,
    #[arg(long)]
    section_summaries: PathBuf,
    #[arg(long)]
    entity_papers: PathBuf,
    #[arg(long)]
    cache_root: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("pipeline_semantic");
    let args = Args::parse();
    let parsed_bytes = std::fs::read(&args.parsed_article)
        .with_context(|| format!("read {}", args.parsed_article.display()))?;
    let article: ParsedArticle =
        serde_json::from_slice(&parsed_bytes).context("deserialize canonical ParsedArticle")?;
    let summaries_bytes = std::fs::read(&args.section_summaries)
        .with_context(|| format!("read {}", args.section_summaries.display()))?;
    let _: Vec<SectionSummaryInput> =
        serde_json::from_slice(&summaries_bytes).context("deserialize section summaries")?;
    let entity_papers_bytes = std::fs::read(&args.entity_papers)
        .with_context(|| format!("read {}", args.entity_papers.display()))?;
    let entity_papers: EntityPapersArtifact = serde_json::from_slice(&entity_papers_bytes)
        .context("deserialize entity-papers artifact")?;

    let cache = Arc::new(FilePipelineCache::new(&args.cache_root));
    let stage_registry =
        da_cli::production::embedded_stage_registry().map_err(anyhow::Error::msg)?;
    let orchestrator = PipelineOrchestrator::new(
        cache.clone(),
        Arc::new(da_ports::SystemClock),
        stage_registry,
    )
    .with_executor(
        "parsed_artifact",
        Arc::new(ArtifactPayloadStage::new(
            "parsed_artifact",
            parsed_bytes,
            "ParsedArticle@serde-v1",
        )),
    )?
    .with_executor(
        "summaries_artifact",
        Arc::new(ArtifactPayloadStage::new(
            "summaries_artifact",
            summaries_bytes,
            "Vec<SectionSummaryInput>@serde-v1",
        )),
    )?
    .with_executor(
        "entity_papers_artifact",
        Arc::new(ArtifactPayloadStage::new(
            "entity_papers_artifact",
            entity_papers_bytes,
            "EntityPapersArtifact@serde-v1",
        )),
    )?
    .with_executor("document_context", Arc::new(DocumentContextStage::new()))?
    .with_executor("cluster_detection", Arc::new(ClusterDetectionStage::new()))?;
    let pipeline = PipelineBuilder::new("semantic-context-clusters-v1")
        .step("parse", "parsed_artifact", &[])
        .step("section_summaries", "summaries_artifact", &[])
        .step("entity_papers", "entity_papers_artifact", &[])
        .step(
            "context",
            "document_context",
            &["parse", "section_summaries"],
        )
        .step("clusters", "cluster_detection", &["entity_papers"])
        .build()
        .map_err(|error| anyhow::anyhow!("build semantic pipeline: {error}"))?;
    let source = SourceIdentity {
        article_id: args.article_id.clone(),
        selected_format: SourceFormat::MetadataOnly,
        pdf_sha256: None,
        html_sha256: None,
        acquisition_fingerprint: "explicit-canonical-artifacts-v1".into(),
        normalization_fingerprint: "path-id-context-v1".into(),
    };
    let first = orchestrator.run(&pipeline, source.clone()).await?;
    let second = orchestrator.run(&pipeline, source).await?;
    anyhow::ensure!(
        second
            .stages
            .iter()
            .all(|record| matches!(record.disposition, StageDisposition::CacheHit { .. })),
        "second pure DAG run did not fully hit cache"
    );
    let manifests = cache.manifest(&args.article_id)?;
    let now = Utc::now().to_rfc3339();
    let context_entry = manifests
        .iter()
        .find(|entry| entry.key.stage_id == "context")
        .context("document context cache entry missing")?;
    let cluster_entry = manifests
        .iter()
        .find(|entry| entry.key.stage_id == "clusters")
        .context("cluster detection cache entry missing")?;
    let context_payload = match cache.lookup(&context_entry.key, &now)? {
        CacheLookup::Hit(hit) => hit.payload,
        other => anyhow::bail!("document context cache not readable: {other:?}"),
    };
    let cluster_payload = match cache.lookup(&cluster_entry.key, &now)? {
        CacheLookup::Hit(hit) => hit.payload,
        other => anyhow::bail!("cluster detection cache not readable: {other:?}"),
    };
    let projection: DocumentContextProjection = serde_json::from_slice(&context_payload)?;
    let detected: Vec<da_domain::cluster::DetectedCluster> =
        serde_json::from_slice(&cluster_payload)?;

    let store = Arc::new(da_adapters::SamyamaGraphStore::from_env());
    for entry in &entity_papers.entries {
        let key = da_domain::cluster::normalize_entity_key(&entry.entity_type, &entry.label);
        let vid = da_domain::vid::entity_vid(&key.entity_type, &key.label);
        if store
            .find_node_by_string_property("Entity", "vid", &vid)
            .await
            .is_none()
        {
            let node = store.create_node("Entity").await?;
            for (key, value) in [
                ("vid", vid),
                ("label", key.label),
                ("entity_type", key.entity_type),
            ] {
                store.set_node_property_string(node, key, value).await?;
            }
            store
                .set_node_property_bool(node, "retrieval_eligible", true)
                .await?;
            store
                .set_node_property_bool(node, "import_eligible", false)
                .await?;
            store
                .set_node_property_int(node, "schema_version", 1)
                .await?;
        }
    }
    let graph_store: Arc<dyn DirectGraphStore> = store.clone();
    // Graph writes are deliberately outside PipelineStageExecutor/cache-hit paths.
    let materialized = ClusterUseCase::from_shared(graph_store)
        .materialize_detected_clusters(&detected)
        .await?;

    let cluster_nodes = store.get_nodes_by_label("ConceptCluster").await.len();
    let output = serde_json::json!({
        "article_id": args.article_id,
        "context_sections": projection.sections.len(),
        "context_summaries": projection.sections.iter().filter(|section| section.summary.is_some()).count(),
        "cluster_nodes": cluster_nodes,
        "pure_dag_first": first.stages.iter().map(|record| format!("{:?}", record.disposition)).collect::<Vec<_>>(),
        "pure_dag_second_all_cache_hit": true,
        "cluster_detection": {
            "cluster_count": detected.len(),
            "side_effects": "none"
        },
        "cluster_materialization": {
            "clusters_created": materialized.clusters_created,
            "clusters_reused": materialized.clusters_reused,
            "member_edges_created": materialized.member_edges_created,
            "members_skipped": materialized.clusters_skipped,
            "execution": "always-run-outside-cache"
        },
        "authoritative": false,
        "import_eligible": false,
        "store_lifetime": "same-process"
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    anyhow::ensure!(
        article.paper_id == output["article_id"].as_str().unwrap_or_default(),
        "article identity changed"
    );
    Ok(())
}
