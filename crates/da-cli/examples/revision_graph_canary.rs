//! One-process real-Samyama ADR-058 acceptance canary.
//!
//! Loads exact local v1 and v5 HTML/catalog metadata under one canonical Paper,
//! verifies revision-scoped structure/current/SUPERSEDES, then replays both
//! through InstrumentedGraphStore and requires zero applied physical writes.

use anyhow::{Context, ensure};
use clap::Parser;
use da_adapters::HtmlParser;
use da_application::{IngestUseCase, RevisionContentContext, RevisionManifestationInput};
use da_ports::FixedClock;
use da_ports::parser::ParserPort;
use kg_storage::{DirectGraphStore, GraphStore, InstrumentedGraphStore, SamyamaGraphStore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        default_value = "data/article_catalog/article_catalog/arxiv/cs-ai/2506.07853"
    )]
    unit_root: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_graph_2506.07853.sgsnap"
    )]
    output: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_graph_2506.07853.report.json"
    )]
    report: PathBuf,
}

#[derive(Serialize)]
struct RevisionObservation {
    revision: u32,
    revision_vid: String,
    is_current: bool,
    sections: usize,
    manifestations: usize,
    pdf_sha256: String,
    html_sha256: String,
}

#[derive(Serialize)]
struct CanaryReport {
    schema_version: &'static str,
    verdict: &'static str,
    paper_id: &'static str,
    paper_nodes: usize,
    revisions: Vec<RevisionObservation>,
    supersedes_edges: usize,
    replay: kg_storage::MutationCounters,
    replay_passes: bool,
    snapshot_path: String,
    snapshot_sha256: String,
    snapshot_bytes: usize,
    import_eligible: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("revision_graph_canary");
    let args = Args::parse();
    let base: Arc<dyn DirectGraphStore> = Arc::new(SamyamaGraphStore::new());
    let observed = Arc::new(InstrumentedGraphStore::new(base));
    let direct: Arc<dyn DirectGraphStore> = observed.clone();
    let ingest = IngestUseCase::materializer_only_with_clock(
        direct.clone(),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );

    let parser = HtmlParser::new();
    let mut contexts = Vec::new();
    let mut parsed_articles = Vec::new();
    for revision in [1_u32, 5] {
        let html = args.unit_root.join(format!("article.v{revision}.html"));
        let meta = args.unit_root.join(format!("article.v{revision}.json"));
        let parsed = parser
            .parse_html(html.to_str().context("non-UTF8 HTML path")?, "2506.07853")
            .await
            .with_context(|| format!("parse {}", html.display()))?;
        let context = context_from_metadata(&meta, revision)?;
        ingest
            .materialize_parsed_with_context(
                &parsed,
                vec![revision as f32, 1.0],
                None,
                Some("arxiv".into()),
                None,
                Some(&context),
            )
            .await
            .with_context(|| format!("materialize v{revision}"))?;
        contexts.push(context);
        parsed_articles.push(parsed);
    }

    let paper_nodes = direct.get_nodes_by_label("Paper").await.len();
    ensure!(paper_nodes == 1, "expected one Paper, got {paper_nodes}");
    let mut revisions = Vec::new();
    for context in &contexts {
        let revision_vid = context.revision_vid()?;
        let node = direct
            .find_node_by_string_property("PaperRevision", "vid", &revision_vid)
            .await
            .with_context(|| format!("missing PaperRevision v{}", context.revision))?;
        let is_current = direct
            .get_node_property_bool(node, "is_current")
            .await
            .unwrap_or(false);
        let sections = direct
            .get_outgoing_edges(node)
            .await
            .into_iter()
            .filter(|(_, edge)| edge == "hasPart")
            .count();
        let manifestations = direct
            .get_outgoing_edges(node)
            .await
            .into_iter()
            .filter(|(_, edge)| edge == "HAS_MANIFESTATION")
            .count();
        let pdf_sha256 = context
            .manifestations
            .iter()
            .find(|m| m.format == "pdf")
            .context("missing PDF manifestation")?
            .sha256
            .clone();
        let html_sha256 = context
            .manifestations
            .iter()
            .find(|m| m.format == "html")
            .context("missing HTML manifestation")?
            .sha256
            .clone();
        revisions.push(RevisionObservation {
            revision: context.revision,
            revision_vid,
            is_current,
            sections,
            manifestations,
            pdf_sha256,
            html_sha256,
        });
    }
    ensure!(
        revisions.iter().all(|revision| revision.sections > 0),
        "each revision must expose non-empty structure"
    );
    ensure!(
        revisions[0].sections != revisions[1].sections,
        "v1 and v5 structure unexpectedly aliased"
    );
    ensure!(
        !revisions[0].is_current && revisions[1].is_current,
        "current projection must be v5"
    );
    ensure!(
        revisions.iter().all(|r| r.manifestations == 2),
        "each revision needs PDF+HTML"
    );

    let v1_node = direct
        .find_node_by_string_property("PaperRevision", "vid", &revisions[0].revision_vid)
        .await
        .context("missing v1 node")?;
    let v5_node = direct
        .find_node_by_string_property("PaperRevision", "vid", &revisions[1].revision_vid)
        .await
        .context("missing v5 node")?;
    let supersedes_edges = direct
        .get_edges_between(v5_node, v1_node, "SUPERSEDES")
        .await
        .len();
    ensure!(supersedes_edges == 1, "expected v5 SUPERSEDES v1");

    observed.reset().await;
    for (context, parsed) in contexts.iter().zip(parsed_articles.iter()) {
        ingest
            .materialize_parsed_with_context(
                parsed,
                vec![context.revision as f32, 1.0],
                None,
                Some("arxiv".into()),
                None,
                Some(context),
            )
            .await
            .with_context(|| format!("replay v{}", context.revision))?;
    }
    let replay = observed.snapshot().await;
    ensure!(replay.replay_passes(), "physical replay failed: {replay:?}");

    let snapshot = GraphStore::export_snapshot(observed.as_ref()).await?;
    atomic_write(&args.output, &snapshot)?;
    let snapshot_sha256 = hex::encode(Sha256::digest(&snapshot));
    let report = CanaryReport {
        schema_version: "revision-graph-canary.v1",
        verdict: "pass",
        paper_id: "2506.07853",
        paper_nodes,
        revisions,
        supersedes_edges,
        replay,
        replay_passes: true,
        snapshot_path: args.output.display().to_string(),
        snapshot_sha256,
        snapshot_bytes: snapshot.len(),
        import_eligible: false,
    };
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!("PASS report={}", args.report.display());
    Ok(())
}

fn context_from_metadata(path: &Path, revision: u32) -> anyhow::Result<RevisionContentContext> {
    let value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path).with_context(|| format!("read {}", path.display()))?,
    )
    .with_context(|| format!("parse {}", path.display()))?;
    ensure!(
        value["article_key"] == "2506.07853",
        "canonical article key drift"
    );
    ensure!(
        value["requested_source"]["revision"] == revision,
        "revision metadata drift"
    );
    let mut manifestations = Vec::new();
    for variant in value["source_variants"]
        .as_array()
        .context("source_variants missing")?
    {
        let format = variant["source_format"]
            .as_str()
            .context("source_format missing")?;
        if format != "pdf" && format != "html" {
            continue;
        }
        manifestations.push(RevisionManifestationInput {
            format: format.into(),
            sha256: variant["sha256"].as_str().context("sha256 missing")?.into(),
            bytes: variant["byte_size"].as_u64().context("byte_size missing")?,
            uri: variant["requested_url"]
                .as_str()
                .context("requested_url missing")?
                .into(),
            path: variant["path"].as_str().context("path missing")?.into(),
            capture_status: variant["capture_status"]
                .as_str()
                .context("capture_status missing")?
                .into(),
        });
    }
    ensure!(
        manifestations.len() == 2,
        "expected PDF+HTML manifestations"
    );
    Ok(RevisionContentContext {
        source_code: "arxiv".into(),
        canonical_id: "2506.07853".into(),
        revision,
        manifestations,
    })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("output has no parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
