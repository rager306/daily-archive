//! Graph analytics command — wraps kg-storage adapter + samyama-graph-algorithms.
//!
//! Provides `da analyze` CLI subcommand for running PageRank, WCC, and other
//! graph algorithms on the knowledge graph. Builds a GraphView projection
//! from any DirectGraphStore via the kg-storage adapter.

use clap::Args;
use kg_storage::adapter::graph_view_from;
use kg_storage::traits::DirectGraphStore;
use samyama_graph_algorithms::{PageRankConfig, page_rank, weakly_connected_components};

/// Arguments for `da analyze` subcommand.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Algorithm to run: page-rank, wcc
    #[arg(long)]
    pub algorithm: String,

    /// Node label to project (e.g., Paper, Entity). If omitted, all nodes.
    #[arg(long)]
    pub label: Option<String>,

    /// Edge type to include (e.g., CITES, MENTIONS). If omitted, all edges.
    #[arg(long)]
    pub edge: Option<String>,

    /// Edge property name for edge weights (e.g., "weight"). If omitted, unweighted.
    #[arg(long)]
    pub weight: Option<String>,

    /// Top-N results to print (default 10)
    #[arg(long, default_value = "10")]
    pub top: usize,
}

/// Run the analysis on a DirectGraphStore and print results.
pub async fn run_analyze(store: &dyn DirectGraphStore, args: &AnalyzeArgs) -> anyhow::Result<()> {
    let label = args.label.as_deref();
    let edge = args.edge.as_deref();
    let weight = args.weight.as_deref();

    let view = graph_view_from(store, label, edge, weight).await?;

    eprintln!(
        "GraphView: {} nodes, {} edges (label={:?}, edge={:?}, weight={:?})",
        view.node_count,
        view.out_targets.len(),
        label,
        edge,
        weight,
    );

    match args.algorithm.as_str() {
        "page-rank" => {
            let ranks = page_rank(&view, PageRankConfig::default());
            print_pagerank_results(&ranks, args.top);
        }
        "wcc" => {
            let components = weakly_connected_components(&view);
            print_wcc_results(&components);
        }
        other => {
            anyhow::bail!("unknown algorithm: {other}. Available: page-rank, wcc");
        }
    }

    Ok(())
}

fn print_pagerank_results(ranks: &std::collections::HashMap<u64, f64>, top: usize) {
    let mut sorted: Vec<(u64, f64)> = ranks
        .iter()
        .map(|(node_id, score)| (*node_id, *score))
        .collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    println!("PageRank (top {top}):");
    println!("  {:<20} {:>12}", "Node ID", "Score");
    println!("  {:<20} {:>12}", "-------", "-----");
    for (node_id, score) in sorted.iter().take(top) {
        println!("  {:<20} {:>12.6}", node_id, score);
    }
}

fn print_wcc_results(components: &samyama_graph_algorithms::WccResult) {
    let num_components = components.components.len();
    let largest = components
        .components
        .values()
        .map(|v| v.len())
        .max()
        .unwrap_or(0);

    println!("Weakly Connected Components:");
    println!("  Total components: {}", num_components);
    println!("  Largest component size: {}", largest);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kg_storage::mock::MockGraphStore;

    fn make_store_with_papers() -> MockGraphStore {
        // Build in test via async — we use a helper that creates a small graph.
        // Since MockGraphStore methods are async, we use tokio runtime in tests.
        MockGraphStore::new()
    }

    #[tokio::test]
    async fn test_run_analyze_page_rank_on_empty_graph() {
        let store = make_store_with_papers();
        let args = AnalyzeArgs {
            algorithm: "page-rank".to_string(),
            label: Some("Paper".to_string()),
            edge: Some("CITES".to_string()),
            weight: None,
            top: 10,
        };
        let result = run_analyze(&store, &args).await;
        assert!(result.is_ok(), "page-rank on empty graph should succeed");
    }

    #[tokio::test]
    async fn test_run_analyze_wcc_on_empty_graph() {
        let store = make_store_with_papers();
        let args = AnalyzeArgs {
            algorithm: "wcc".to_string(),
            label: Some("Paper".to_string()),
            edge: Some("CITES".to_string()),
            weight: None,
            top: 10,
        };
        let result = run_analyze(&store, &args).await;
        assert!(result.is_ok(), "wcc on empty graph should succeed");
    }

    #[tokio::test]
    async fn test_run_analyze_unknown_algorithm_errors() {
        let store = make_store_with_papers();
        let args = AnalyzeArgs {
            algorithm: "bogus".to_string(),
            label: None,
            edge: None,
            weight: None,
            top: 10,
        };
        let result = run_analyze(&store, &args).await;
        assert!(result.is_err(), "unknown algorithm should error");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("unknown algorithm"), "error message: {msg}");
    }

    #[tokio::test]
    async fn test_run_analyze_page_rank_on_citation_graph() {
        let store = MockGraphStore::new();
        let p1 = store.create_node("Paper").await.unwrap();
        let p2 = store.create_node("Paper").await.unwrap();
        let p3 = store.create_node("Paper").await.unwrap();
        store.create_edge(p1, p2, "CITES").await.unwrap();
        store.create_edge(p2, p3, "CITES").await.unwrap();

        let args = AnalyzeArgs {
            algorithm: "page-rank".to_string(),
            label: Some("Paper".to_string()),
            edge: Some("CITES".to_string()),
            weight: None,
            top: 5,
        };
        let result = run_analyze(&store, &args).await;
        assert!(result.is_ok(), "page-rank on citation graph should succeed");
    }
}
