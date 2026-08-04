//! Graph analytics command — wraps kg-storage adapter + graph algorithms.
//!
//! Provides `da analyze` CLI subcommand for running PageRank, personalized
//! PageRank (ForwardPush via kg-algorithms), and WCC on a GraphView projection
//! from any DirectGraphStore.
//!
//! - `page-rank` — classical PR (samyama-graph-algorithms)
//! - `ppr` — personalized PR (GraphViewAlgorithms / ForwardPush); requires `--seed`
//! - `wcc` — weakly connected components
//!
//! ForwardPush PPR is **degree-count** (ignores edge weights). D127 import gate
//! is unchanged — this path is read-only analysis.

use clap::Args;
use kg_algorithms::GraphViewAlgorithms;
use kg_algorithms::port::GraphAlgorithms;
use kg_storage::adapter::graph_view_from;
use kg_storage::traits::DirectGraphStore;
use samyama_graph_algorithms::{PageRankConfig, page_rank, weakly_connected_components};

/// Arguments for `da analyze` subcommand.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Algorithm to run: page-rank, ppr, wcc
    #[arg(long)]
    pub algorithm: String,

    /// Node label to project (e.g., Paper, Entity). If omitted, all nodes.
    #[arg(long)]
    pub label: Option<String>,

    /// Edge type to include (e.g., CITES, MENTIONS). If omitted, all edges.
    #[arg(long)]
    pub edge: Option<String>,

    /// Edge property name for edge weights (e.g., "weight"). If omitted, unweighted.
    /// Note: ForwardPush PPR ignores CSR values; weights still affect GraphView fold.
    #[arg(long)]
    pub weight: Option<String>,

    /// Top-N results to print (default 10)
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Seed NodeId for personalized PageRank (`--algorithm ppr`). Required for ppr.
    #[arg(long)]
    pub seed: Option<u64>,

    /// Teleport probability for PPR (default 0.85). Ignored for page-rank/wcc.
    #[arg(long, default_value = "0.85")]
    pub alpha: f64,
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
        "ppr" => {
            let seed = args.seed.ok_or_else(|| {
                anyhow::anyhow!("--seed <NodeId> is required for --algorithm ppr")
            })?;
            if !(args.alpha > 0.0 && args.alpha < 1.0) {
                anyhow::bail!("--alpha must be in (0, 1), got {}", args.alpha);
            }
            let alg = GraphViewAlgorithms::new(view, edge.map(|s| s.to_string()));
            let scores = alg.personalized_pagerank(&[seed], args.alpha, 0).await;
            if scores.is_empty() {
                // Trait fail-soft: empty means empty graph, empty seeds (guarded),
                // or unknown seed / CSR export failure.
                anyhow::bail!(
                    "ppr returned no scores for seed {seed} (unknown seed or empty/invalid view)"
                );
            }
            print_ppr_results(&scores, args.top, seed);
        }
        "wcc" => {
            let components = weakly_connected_components(&view);
            print_wcc_results(&components);
        }
        other => {
            anyhow::bail!("unknown algorithm: {other}. Available: page-rank, ppr, wcc");
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

fn print_ppr_results(scores: &[(u64, f64)], top: usize, seed: u64) {
    println!("Personalized PageRank (seed={seed}, top {top}):");
    println!("  {:<20} {:>12}", "Node ID", "Score");
    println!("  {:<20} {:>12}", "-------", "-----");
    for (node_id, score) in scores.iter().take(top) {
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

    fn empty_args(algorithm: &str) -> AnalyzeArgs {
        AnalyzeArgs {
            algorithm: algorithm.to_string(),
            label: Some("Paper".to_string()),
            edge: Some("CITES".to_string()),
            weight: None,
            top: 10,
            seed: None,
            alpha: 0.85,
        }
    }

    #[tokio::test]
    async fn test_run_analyze_page_rank_on_empty_graph() {
        let store = MockGraphStore::new();
        let result = run_analyze(&store, &empty_args("page-rank")).await;
        assert!(result.is_ok(), "page-rank on empty graph should succeed");
    }

    #[tokio::test]
    async fn test_run_analyze_wcc_on_empty_graph() {
        let store = MockGraphStore::new();
        let result = run_analyze(&store, &empty_args("wcc")).await;
        assert!(result.is_ok(), "wcc on empty graph should succeed");
    }

    #[tokio::test]
    async fn test_run_analyze_unknown_algorithm_errors() {
        let store = MockGraphStore::new();
        let mut args = empty_args("bogus");
        args.label = None;
        args.edge = None;
        let result = run_analyze(&store, &args).await;
        assert!(result.is_err(), "unknown algorithm should error");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("unknown algorithm"), "error message: {msg}");
        assert!(msg.contains("ppr"), "help should list ppr: {msg}");
    }

    #[tokio::test]
    async fn test_run_analyze_page_rank_on_citation_graph() {
        let store = MockGraphStore::new();
        let p1 = store.create_node("Paper").await.unwrap();
        let p2 = store.create_node("Paper").await.unwrap();
        let p3 = store.create_node("Paper").await.unwrap();
        store.create_edge(p1, p2, "CITES").await.unwrap();
        store.create_edge(p2, p3, "CITES").await.unwrap();

        let result = run_analyze(&store, &empty_args("page-rank")).await;
        assert!(result.is_ok(), "page-rank on citation graph should succeed");
    }

    #[tokio::test]
    async fn test_ppr_requires_seed() {
        let store = MockGraphStore::new();
        let result = run_analyze(&store, &empty_args("ppr")).await;
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("--seed"), "msg={msg}");
    }

    #[tokio::test]
    async fn test_ppr_with_seed_on_citation_graph() {
        let store = MockGraphStore::new();
        let p1 = store.create_node("Paper").await.unwrap();
        let p2 = store.create_node("Paper").await.unwrap();
        let p3 = store.create_node("Paper").await.unwrap();
        store.create_edge(p1, p2, "CITES").await.unwrap();
        store.create_edge(p2, p3, "CITES").await.unwrap();

        let mut args = empty_args("ppr");
        args.seed = Some(p1);
        let result = run_analyze(&store, &args).await;
        assert!(result.is_ok(), "ppr with seed should succeed: {result:?}");
    }

    #[tokio::test]
    async fn test_ppr_unknown_seed_errors() {
        let store = MockGraphStore::new();
        let p1 = store.create_node("Paper").await.unwrap();
        let p2 = store.create_node("Paper").await.unwrap();
        store.create_edge(p1, p2, "CITES").await.unwrap();

        let mut args = empty_args("ppr");
        args.seed = Some(999_999);
        let result = run_analyze(&store, &args).await;
        assert!(result.is_err(), "unknown seed must error");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("no scores") || msg.contains("seed"),
            "msg={msg}"
        );
    }

    #[tokio::test]
    async fn test_ppr_invalid_alpha_errors() {
        let store = MockGraphStore::new();
        let mut args = empty_args("ppr");
        args.seed = Some(1);
        args.alpha = 1.5;
        let result = run_analyze(&store, &args).await;
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("alpha"), "msg={msg}");
    }
}
