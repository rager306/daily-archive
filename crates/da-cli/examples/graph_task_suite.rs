//! Semantic graph task suite runner (ADR-051 §8.1 / M336).
//!
//! Samyama case-study pattern:
//!   import pinned `.sgsnap` → run `// @query` tasks → PASS/EMPTY/ERROR → suite gate.
//!
//! ```bash
//! cargo run -p da-cli --example graph_task_suite -- \
//!   --snapshot artifacts/gepa_hub_production/hub.sgsnap \
//!   --tasks data/graph_tasks/gepa_hub.tasks.cypher \
//!   --report artifacts/graph_tasks/gepa_hub_report.json
//! ```
//!
//! D127: import_eligible stays false. Process-local store only (same-process import).

use clap::Parser;
use da_cli::graph_tasks::{GraphTaskResult, TaskVerdict, parse_tasks_cypher, run_graph_task_suite};
use da_ports::graph_store::GraphStore;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "graph_task_suite",
    about = "ADR-051 §8.1 semantic graph task suite"
)]
struct Args {
    /// Path to durable `.sgsnap` (imported into this process only).
    #[arg(long)]
    snapshot: PathBuf,

    /// Cypher suite file with `// @query` markers.
    #[arg(long)]
    tasks: PathBuf,

    /// Optional JSON report path.
    #[arg(long)]
    report: Option<PathBuf>,

    /// Samyama query graph name (default daily_archive).
    #[arg(long, default_value = "daily_archive")]
    graph: String,
}

#[derive(Debug, Serialize)]
struct SuiteReport {
    suite_file: String,
    snapshot: String,
    import_eligible: bool,
    nodes: usize,
    edges: usize,
    pass: usize,
    empty: usize,
    error: usize,
    failed: bool,
    tasks: Vec<GraphTaskResult>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("graph_task_suite");
    let args = Args::parse();

    // D127 honesty
    println!("graph_task_suite — ADR-051 §8.1 (Samyama case-study DoD pattern)");
    println!("  import_eligible=false (D127 locked)");
    println!("  snapshot: {}", args.snapshot.display());
    println!("  tasks:    {}", args.tasks.display());

    let tasks_src = match std::fs::read_to_string(&args.tasks) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ read tasks {}: {e}", args.tasks.display());
            return ExitCode::from(2);
        }
    };
    let suite = match parse_tasks_cypher(&tasks_src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ parse tasks: {e}");
            return ExitCode::from(2);
        }
    };
    println!("  parsed:   {} task(s)", suite.tasks.len());

    let snap_bytes = match std::fs::read(&args.snapshot) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("❌ read snapshot {}: {e}", args.snapshot.display());
            return ExitCode::from(2);
        }
    };

    // Refuse mistaken persistence path (M332)
    if let Err(e) = kg_storage::refuse_persistence_path_from_env() {
        eprintln!("❌ {e}");
        return ExitCode::from(2);
    }

    let store = da_adapters::SamyamaGraphStore::from_env();
    if let Err(e) = store.import_snapshot(&snap_bytes).await {
        eprintln!("❌ import_snapshot: {e}");
        return ExitCode::from(2);
    }
    let nodes = store.node_count().await;
    let edges = store.edge_count().await;
    println!("  loaded:   {nodes} nodes, {edges} edges (this process only)");
    println!("  ⚠  NON-PERSISTENT — dies when process exits");
    if nodes == 0 {
        eprintln!("❌ snapshot imported 0 nodes — cannot run suite");
        return ExitCode::from(2);
    }

    let executed = run_graph_task_suite(&store, &args.graph, &suite).await;

    println!();
    println!(
        "{:<6} {:<28} {:>6} {:>8}  INSIGHT",
        "VERDICT", "TITLE", "ROWS", "MS"
    );
    println!("{}", "-".repeat(90));

    for row in &executed.tasks {
        println!(
            "{:<6} {:<28} {:>6} {:>8}  {}",
            row.verdict.as_str(),
            truncate(&row.title, 28),
            row.rows,
            row.duration_ms,
            truncate(&row.insight, 40)
        );
        if let Some(ref err) = row.error {
            println!("         error: {err}");
        }
        if row.verdict == TaskVerdict::Empty {
            println!(
                "         observed={:?} expected_rows={:?} expected_min={:?}",
                row.observed_first_cell_number, row.expected_rows, row.expected_min_value
            );
        }
    }

    println!("{}", "-".repeat(90));
    println!(
        "PASS={} EMPTY={} ERROR={}  suite_failed={}",
        executed.pass, executed.empty, executed.error, executed.failed
    );

    let failed = executed.failed;
    let report = SuiteReport {
        suite_file: args.tasks.display().to_string(),
        snapshot: args.snapshot.display().to_string(),
        import_eligible: false,
        nodes,
        edges,
        pass: executed.pass,
        empty: executed.empty,
        error: executed.error,
        failed: executed.failed,
        tasks: executed.tasks,
    };

    if let Some(ref path) = args.report {
        if let Err(e) = write_report(path, &report) {
            eprintln!("❌ write report {}: {e}", path.display());
            return ExitCode::from(2);
        }
        println!("  report: {}", path.display());
    }

    if failed {
        eprintln!("❌ suite FAILED (EMPTY or ERROR present)");
        ExitCode::from(1)
    } else {
        println!("✅ suite PASSED");
        ExitCode::SUCCESS
    }
}

fn write_report(path: &Path, report: &SuiteReport) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json + "\n")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
