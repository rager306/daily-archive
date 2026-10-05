//! Consolidated multi-command runner (M516/S01/T02).
//!
//! Twenty-seven un-gated example runners that nothing else referenced were
//! folded into this single target so a `cargo build --examples` / `cargo build
//! -p da-cli --examples` no longer produces twenty-seven statically linked
//! binaries for the same source set. Each former example is preserved verbatim
//! under `examples/da_toolbox/` and stays reachable as a subcommand:
//!
//! ```text
//! cargo run -p da-cli --example da_toolbox -- <command> [args...]
//! ```
//!
//! Arguments are re-dispatched to a child process of this same binary so each
//! program's own `std::env::args()` / clap parsing sees exactly the arguments
//! it saw when it was a standalone example. The child is selected by the
//! `DA_TOOLBOX_COMMAND` environment variable, which means the in-process
//! dispatch path never re-enters the argument shim.

#![allow(clippy::exit)] // process boundary is the point of a command runner

#[path = "da_toolbox/acap_aggregate.rs"]
mod acap_aggregate;
#[path = "da_toolbox/acap_project_pipeline_layers.rs"]
mod acap_project_pipeline_layers;
#[path = "da_toolbox/analyze_figure.rs"]
mod analyze_figure;
#[path = "da_toolbox/article_metadata_audit.rs"]
mod article_metadata_audit;
#[path = "da_toolbox/catalog_drift_stats.rs"]
mod catalog_drift_stats;
#[path = "da_toolbox/catalog_reconcile.rs"]
mod catalog_reconcile;
#[path = "da_toolbox/concept_cluster_canary.rs"]
mod concept_cluster_canary;
#[path = "da_toolbox/detect_figures.rs"]
mod detect_figures;
#[path = "da_toolbox/entity_resolution_canary.rs"]
mod entity_resolution_canary;
#[path = "da_toolbox/extract_probe.rs"]
mod extract_probe;
#[path = "da_toolbox/gold_graph_all_waves.rs"]
mod gold_graph_all_waves;
#[path = "da_toolbox/inventory_fp.rs"]
mod inventory_fp;
#[path = "da_toolbox/materialization_hot_path_profile.rs"]
mod materialization_hot_path_profile;
#[path = "da_toolbox/materialize_index_bench.rs"]
mod materialize_index_bench;
#[path = "da_toolbox/ordered_mutation_canary.rs"]
mod ordered_mutation_canary;
#[path = "da_toolbox/pipeline_acceptance.rs"]
mod pipeline_acceptance;
#[path = "da_toolbox/prompt_eval_inference.rs"]
mod prompt_eval_inference;
#[path = "da_toolbox/prompt_eval_scored.rs"]
mod prompt_eval_scored;
#[path = "da_toolbox/python_archive_audit.rs"]
mod python_archive_audit;
#[path = "da_toolbox/retrieval_benchmark.rs"]
mod retrieval_benchmark;
#[path = "da_toolbox/rust_pipeline_acceptance.rs"]
mod rust_pipeline_acceptance;
#[path = "da_toolbox/samyama_hot_path_profile.rs"]
mod samyama_hot_path_profile;
#[path = "da_toolbox/scholarly_evidence_canary.rs"]
mod scholarly_evidence_canary;
#[path = "da_toolbox/sona_policy_training.rs"]
mod sona_policy_training;
#[path = "da_toolbox/stage_ablation_report.rs"]
mod stage_ablation_report;
#[path = "da_toolbox/tex_source_inspect.rs"]
mod tex_source_inspect;
#[path = "da_toolbox/vertical_scientific_path.rs"]
mod vertical_scientific_path;

/// Normalises the two main-return shapes the folded examples use (`()` and
/// `Result<(), E>`) into a process exit code, preserving the standalone
/// behaviour: a returned error prints and exits non-zero.
trait IntoExit {
    fn into_exit(self) -> i32;
}

impl IntoExit for () {
    fn into_exit(self) -> i32 {
        0
    }
}

impl<E: std::fmt::Debug> IntoExit for Result<(), E> {
    fn into_exit(self) -> i32 {
        match self {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error:?}");
                1
            }
        }
    }
}

fn main() {
    // ADR-052: the composition root owns .env-backed rolling JSON logging and
    // holds the WorkerGuard. Only the parent process initialises here: the
    // child re-dispatch path runs a folded example's own `main`, which
    // initialises logging for itself, so initialising in the child too would
    // double-init the global subscriber (`try_init` would fail) and open a
    // second appender. Keeping this binding alive holds the parent's writer
    // open until the re-dispatched child has finished.
    let _log_guard = if std::env::var("DA_TOOLBOX_COMMAND").is_err() {
        da_cli::logging::init_pipeline_logging("da_toolbox")
    } else {
        None
    };

    let code = run();
    // `process::exit` does not run destructors, and the WorkerGuard flushes the
    // non-blocking rolling JSON writer on drop, so drop it explicitly first.
    drop(_log_guard);
    std::process::exit(code);
}

/// Resolves the requested subcommand to a process exit code.
///
/// Kept separate from `main` so the logging guard is dropped — and the file
/// writer flushed — before `process::exit`, which skips destructors.
fn run() -> i32 {
    if let Ok(command) = std::env::var("DA_TOOLBOX_COMMAND") {
        return dispatch(&command);
    }

    let mut args = std::env::args();
    let _program = args.next();
    let command = match args.next() {
        Some(command) => command,
        None => {
            eprintln!("usage: da_toolbox <command> [args...]");
            return 2;
        }
    };
    let rest: Vec<String> = args.collect();

    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("da_toolbox: cannot resolve current executable: {error}");
            return 2;
        }
    };
    let status = match std::process::Command::new(executable)
        .env("DA_TOOLBOX_COMMAND", &command)
        .args(&rest)
        .status()
    {
        Ok(status) => status,
        Err(error) => {
            eprintln!("da_toolbox: failed to re-dispatch `{command}`: {error}");
            return 2;
        }
    };
    status.code().unwrap_or(1)
}

fn dispatch(command: &str) -> i32 {
    match command {
        "acap_aggregate" => acap_aggregate::main().into_exit(),
        "acap_project_pipeline_layers" => acap_project_pipeline_layers::main().into_exit(),
        "analyze_figure" => analyze_figure::main().into_exit(),
        "article_metadata_audit" => article_metadata_audit::main().into_exit(),
        "catalog_drift_stats" => catalog_drift_stats::main().into_exit(),
        "catalog_reconcile" => catalog_reconcile::main().into_exit(),
        "concept_cluster_canary" => concept_cluster_canary::main().into_exit(),
        "detect_figures" => detect_figures::main().into_exit(),
        "entity_resolution_canary" => entity_resolution_canary::main().into_exit(),
        "extract_probe" => extract_probe::main().into_exit(),
        "gold_graph_all_waves" => gold_graph_all_waves::main().into_exit(),
        "inventory_fp" => {
            inventory_fp::main();
            0
        }
        "materialization_hot_path_profile" => materialization_hot_path_profile::main().into_exit(),
        "materialize_index_bench" => materialize_index_bench::main().into_exit(),
        "ordered_mutation_canary" => ordered_mutation_canary::main().into_exit(),
        "pipeline_acceptance" => pipeline_acceptance::main().into_exit(),
        "prompt_eval_inference" => prompt_eval_inference::main().into_exit(),
        "prompt_eval_scored" => prompt_eval_scored::main().into_exit(),
        "python_archive_audit" => python_archive_audit::main().into_exit(),
        "retrieval_benchmark" => retrieval_benchmark::main().into_exit(),
        "rust_pipeline_acceptance" => rust_pipeline_acceptance::main().into_exit(),
        "samyama_hot_path_profile" => samyama_hot_path_profile::main().into_exit(),
        "scholarly_evidence_canary" => scholarly_evidence_canary::main().into_exit(),
        "sona_policy_training" => sona_policy_training::main().into_exit(),
        "stage_ablation_report" => stage_ablation_report::main().into_exit(),
        "tex_source_inspect" => tex_source_inspect::main().into_exit(),
        "vertical_scientific_path" => vertical_scientific_path::main().into_exit(),
        other => {
            eprintln!("da_toolbox: unknown command `{other}`");
            2
        }
    }
}
