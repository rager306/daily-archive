//! Revision-scoped grounded summary canary (M363/S01).
//!
//! Same process: import a revision graph snapshot, build deterministic summaries
//! from PaperRevision + its exact revision Sections, replay, run semantic tasks,
//! and export. No LLM or network. D127 remains false.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use da_application::extractive_summary::{
    ExtractiveSummaryMaterializeInput, ExtractiveSummaryUseCase, SectionRolePolicyBinding,
    SectionSource,
};
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::relation::structure::{HAS_PART, HAS_SUMMARY};
use da_ports::{DirectGraphStore, GraphStore};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_graph_2506.07853.sgsnap"
    )]
    snapshot: PathBuf,
    #[arg(
        long,
        default_value = "data/graph_tasks/revision_summaries.tasks.cypher"
    )]
    tasks: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_summary_canary.json"
    )]
    report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_graph_2506.07853_with_summaries.sgsnap"
    )]
    output_snapshot: PathBuf,
    #[arg(long, default_value = "daily_archive")]
    graph: String,
    #[arg(long, default_value_t = 240)]
    max_chars: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("extractive_summary_canary");
    let args = Args::parse();
    let repo_root = std::env::current_dir()?.canonicalize()?;
    let snapshot_path = resolve(&repo_root, &args.snapshot);
    let tasks_path = resolve(&repo_root, &args.tasks);
    let report_path = resolve(&repo_root, &args.report);
    let output_path = resolve(&repo_root, &args.output_snapshot);

    let snapshot_bytes = std::fs::read(&snapshot_path)
        .with_context(|| format!("read {}", snapshot_path.display()))?;
    let input_sha256 = hex::encode(Sha256::digest(&snapshot_bytes));
    kg_storage::refuse_persistence_path_from_env()?;
    let store = Arc::new(da_adapters::SamyamaGraphStore::from_env());
    store.import_snapshot(&snapshot_bytes).await?;

    let loaded_policy =
        da_adapters::section_summary_policy::LoadedSectionRolePolicy::load_project()?;
    let policy_identity = loaded_policy.identity().clone();
    let use_case = ExtractiveSummaryUseCase::with_role_policy(
        store.clone() as Arc<dyn DirectGraphStore>,
        SectionRolePolicyBinding {
            policy: loaded_policy.policy().clone(),
            configuration_sha256: policy_identity.configuration_sha256.clone(),
        },
    )?;
    let mut revision_vids = Vec::new();
    let mut summaries_written = 0_usize;
    let mut summary_type_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut unmatched_section_summaries = 0_usize;
    let mut unmatched_section_titles = std::collections::BTreeMap::<String, usize>::new();
    let mut skipped_empty_sections = 0_usize;
    let mut failures = Vec::new();

    for revision_node in store.get_nodes_by_label("PaperRevision").await {
        let Some(revision_vid) = store.get_node_property_string(revision_node, "vid").await else {
            failures.push(format!("PaperRevision node {revision_node} missing vid"));
            continue;
        };
        let Some(paper_id) = store
            .get_node_property_string(revision_node, "canonical_id")
            .await
        else {
            failures.push(format!("PaperRevision {revision_vid} missing canonical_id"));
            continue;
        };
        let abstract_text = store
            .get_node_property_string(revision_node, "abstract_text")
            .await
            .unwrap_or_default();
        let mut sections = Vec::new();
        for (section_node, edge_type) in store.get_outgoing_edges(revision_node).await {
            if edge_type != HAS_PART {
                continue;
            }
            let Some(source_vid) = store.get_node_property_string(section_node, "vid").await else {
                continue;
            };
            let path_id = store
                .get_node_property_int(section_node, "order")
                .await
                .map(|ordinal| ordinal.to_string())
                .unwrap_or_else(|| source_vid.clone());
            sections.push(SectionSource {
                path_id,
                source_vid,
                title: store
                    .get_node_property_string(section_node, "title")
                    .await
                    .unwrap_or_default(),
                text: store
                    .get_node_property_string(section_node, "text")
                    .await
                    .unwrap_or_default(),
                node_id: section_node,
            });
        }
        sections.sort_by(|left, right| left.path_id.cmp(&right.path_id));
        let request = ExtractiveSummaryMaterializeInput {
            paper_id,
            revision_node_id: revision_node,
            revision_vid: revision_vid.clone(),
            abstract_text,
            sections,
            max_chars: args.max_chars,
        };
        match use_case.materialize(request.clone()).await {
            Ok(first) => match use_case.materialize(request).await {
                Ok(replay) if replay.batch == first.batch => {
                    summaries_written +=
                        usize::from(first.abstract_written) + first.section_summaries_written;
                    for (summary_type, count) in &first.summary_type_counts {
                        *summary_type_counts.entry(summary_type.clone()).or_insert(0) += count;
                    }
                    unmatched_section_summaries += first.unmatched_section_summaries;
                    for (title, count) in &first.unmatched_section_titles {
                        *unmatched_section_titles.entry(title.clone()).or_insert(0) += count;
                    }
                    skipped_empty_sections += first.skipped_empty_sections;
                    revision_vids.push(revision_vid);
                }
                Ok(_) => failures.push(format!("{revision_vid}: replay batch changed")),
                Err(error) => failures.push(format!("{revision_vid}: replay: {error}")),
            },
            Err(error) => failures.push(format!("{revision_vid}: {error}")),
        }
    }

    let suite_src = std::fs::read_to_string(&tasks_path)
        .with_context(|| format!("read tasks {}", tasks_path.display()))?;
    let suite = parse_tasks_cypher(&suite_src).map_err(|error| anyhow!("parse tasks: {error}"))?;
    let task_report = run_graph_task_suite(store.as_ref(), &args.graph, &suite).await;
    let summary_nodes = store.get_nodes_by_label("Summary").await.len();
    let output_bytes = store.export_snapshot().await?;
    let output_sha256 = hex::encode(Sha256::digest(&output_bytes));
    atomic_write(&output_path, &output_bytes)?;

    revision_vids.sort();
    let report = serde_json::json!({
        "schema_version": "revision-summary-canary.v1",
        "input_snapshot": display_path(&repo_root, &snapshot_path),
        "input_snapshot_sha256": input_sha256,
        "output_snapshot": display_path(&repo_root, &output_path),
        "output_snapshot_sha256": output_sha256,
        "method": da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD,
        "revision_vids": revision_vids,
        "revisions_processed": revision_vids.len(),
        "summaries_written": summaries_written,
        "summary_nodes": summary_nodes,
        "summary_type_counts": summary_type_counts,
        "unmatched_section_summaries": unmatched_section_summaries,
        "unmatched_section_titles": unmatched_section_titles,
        "policy_identity": policy_identity,
        "skipped_empty_sections": skipped_empty_sections,
        "failures": failures,
        "tasks": task_report,
        "hasSummary_edge": HAS_SUMMARY,
        "import_eligible": false,
    });
    atomic_write(&report_path, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "revision_summary_canary revisions={} summaries={} methods={} results={} generic={} nodes={} task_failed={} import_eligible=false report={}",
        revision_vids.len(),
        summaries_written,
        summary_type_counts.get("methods").copied().unwrap_or(0),
        summary_type_counts.get("results").copied().unwrap_or(0),
        unmatched_section_summaries,
        summary_nodes,
        task_report.failed,
        display_path(&repo_root, &report_path),
    );
    if !failures.is_empty()
        || revision_vids.len() < 2
        || summary_nodes < 2
        || summary_type_counts.get("methods").copied().unwrap_or(0) == 0
        || task_report.failed
    {
        for failure in &failures {
            eprintln!("failure: {failure}");
        }
        std::process::exit(1);
    }
    Ok(())
}

fn resolve(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}
