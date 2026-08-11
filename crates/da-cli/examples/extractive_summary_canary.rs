//! Grounded extractive summary canary (M371/S10).
//!
//! Same-process: import pinned materialization `.sgsnap` → compute pure
//! extractive digests from Section/Paper text already in the graph → write
//! Summary nodes + hasSummary edges → re-run summary-class graph tasks.
//! No LLM. D127 stays false. Does not rebuild the corpus from PDF/GROBID.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::extractive_summary::{
    ExtractiveSummaryMaterializeInput, ExtractiveSummaryUseCase, SectionSource,
};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::relation::structure::{HAS_PART, HAS_SUMMARY};
use da_ports::{DirectGraphStore, GraphStore};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200.sgsnap"
    )]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "data/graph_tasks/materialization_all_200.tasks.cypher"
    )]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/extractive_summary_canary.json"
    )]
    report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/materialization_all_200_with_summaries.sgsnap"
    )]
    output_snapshot: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
    #[arg(long, default_value_t = 240)]
    max_chars: usize,
    /// Optional cap for bounded dry-runs (0 = all papers).
    #[arg(long, default_value_t = 0)]
    limit_papers: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("extractive_summary_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let snapshot_path = resolve(&repo_root, &args.snapshot);
    let tasks_path = resolve(&repo_root, &args.tasks);
    let report_path = resolve(&repo_root, &args.report);
    let out_snap_path = resolve(&repo_root, &args.output_snapshot);

    let snapshot_bytes = std::fs::read(&snapshot_path)
        .with_context(|| format!("read {}", snapshot_path.display()))?;
    let input_snap_sha = hex::encode(Sha256::digest(&snapshot_bytes));

    kg_storage::refuse_persistence_path_from_env()?;
    let store = Arc::new(da_adapters::SamyamaGraphStore::from_env());
    store.import_snapshot(&snapshot_bytes).await?;

    let paper_ids = store.get_nodes_by_label("Paper").await;
    let paper_ids = if args.limit_papers > 0 {
        paper_ids.into_iter().take(args.limit_papers).collect()
    } else {
        paper_ids
    };

    let uc = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>);
    let mut papers_processed = 0_u64;
    let mut abstracts_written = 0_u64;
    let mut section_summaries_written = 0_u64;
    let mut skipped_empty_sections = 0_u64;
    let mut failures: Vec<String> = Vec::new();

    for paper in paper_ids {
        let paper_id = store
            .get_node_property_string(paper, "arxiv_id")
            .await
            .or(store.get_node_property_string(paper, "vid").await)
            .unwrap_or_else(|| format!("node:{paper}"));
        let abstract_text = store
            .get_node_property_string(paper, "abstract_text")
            .await
            .unwrap_or_default();

        let mut sections = Vec::new();
        for (target, edge_type) in store.get_outgoing_edges(paper).await {
            if edge_type != HAS_PART {
                continue;
            }
            // Only Section targets with path_id + text.
            let path_id = match store.get_node_property_string(target, "path_id").await {
                Some(p) if !p.trim().is_empty() => p,
                _ => continue,
            };
            let title = store
                .get_node_property_string(target, "title")
                .await
                .unwrap_or_default();
            let text = store
                .get_node_property_string(target, "text")
                .await
                .unwrap_or_default();
            sections.push(SectionSource {
                path_id,
                title,
                text,
                node_id: target,
            });
        }
        // Stable order by path_id for determinism.
        sections.sort_by(|a, b| a.path_id.cmp(&b.path_id));

        match uc
            .materialize(ExtractiveSummaryMaterializeInput {
                paper_id: paper_id.clone(),
                paper_node_id: paper,
                abstract_text,
                sections,
                max_chars: args.max_chars,
            })
            .await
        {
            Ok(result) => {
                papers_processed += 1;
                abstracts_written += u64::from(result.abstract_written);
                section_summaries_written += result.section_summaries_written as u64;
                skipped_empty_sections += result.skipped_empty_sections as u64;
            }
            Err(error) => failures.push(format!("{paper_id}: {error}")),
        }
    }

    let summary_nodes = store.get_nodes_by_label("Summary").await.len() as u64;
    let out_bytes = store.export_snapshot().await?;
    let out_snap_sha = hex::encode(Sha256::digest(&out_bytes));
    atomic_write(&out_snap_path, &out_bytes)?;

    // Same-process task suite against the enriched in-memory store.
    let suite_src = std::fs::read_to_string(&tasks_path)
        .with_context(|| format!("read {}", tasks_path.display()))?;
    let suite = parse_tasks_cypher(&suite_src).map_err(|e| anyhow!("parse tasks: {e}"))?;
    let task_report = run_graph_task_suite(store.as_ref(), &args.graph, &suite).await;
    let summary_class = task_report
        .class_results
        .get(&da_domain::graph_tasks::GraphTaskClass::Summary)
        .cloned();

    let report = serde_json::json!({
        "schema_version": "extractive-summary-canary.v1",
        "input_snapshot": display_path(&repo_root, &snapshot_path),
        "input_snapshot_sha256": input_snap_sha,
        "output_snapshot": display_path(&repo_root, &out_snap_path),
        "output_snapshot_sha256": out_snap_sha,
        "method": da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD,
        "max_chars": args.max_chars,
        "papers_processed": papers_processed,
        "abstracts_written": abstracts_written,
        "section_summaries_written": section_summaries_written,
        "skipped_empty_sections": skipped_empty_sections,
        "summary_nodes": summary_nodes,
        "failures": failures,
        "import_eligible": false,
        "tasks": {
            "suite": display_path(&repo_root, &tasks_path),
            "pass": task_report.pass,
            "empty": task_report.empty,
            "error": task_report.error,
            "failed": task_report.failed,
            "class_results": task_report.class_results,
            "summary_class": summary_class,
        },
        "hasSummary_edge": HAS_SUMMARY,
    });
    atomic_write(&report_path, &serde_json::to_vec_pretty(&report)?)?;

    println!(
        "extractive_summary_canary papers={} abstracts={} section_summaries={} summary_nodes={} task_failed={} summary_class={:?} import_eligible=false report={}",
        papers_processed,
        abstracts_written,
        section_summaries_written,
        summary_nodes,
        task_report.failed,
        summary_class
            .as_ref()
            .map(|c| format!("{:?}", c.status))
            .unwrap_or_else(|| "absent".into()),
        display_path(&repo_root, &report_path),
    );
    for f in failures.iter().take(10) {
        eprintln!("  failure: {f}");
    }
    // Product gate for this canary: summaries written and summary class present+passed
    // when the suite declares a summary task.
    if !failures.is_empty() {
        std::process::exit(1);
    }
    if let Some(class) = summary_class
        && class.status != da_domain::graph_tasks::TaskClassStatus::Passed
    {
        eprintln!("summary class did not PASS: {:?}", class);
        std::process::exit(1);
    }
    Ok(())
}

fn resolve(repo_root: &Path, relative: &Path) -> PathBuf {
    if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        repo_root.join(relative)
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
    }
    Ok(())
}
