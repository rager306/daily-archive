use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod acquire_arxiv;
mod agent_run;
mod analyze;
mod batch_ids;
mod graph_ping;
mod graph_query;
mod health_report;
mod load;
mod load_journey;
// The lib target exposes these modules as public API and its tests exercise
// the full composition surface; the duplicate bin-module compilation does not
// construct every config knob or accelerator helper directly.
#[allow(dead_code)]
mod parse_batch;
#[allow(dead_code)]
mod parse_config;
mod parse_job;
#[allow(dead_code)]
mod parse_profile;
// The lib target exposes this module as public API and its tests exercise the
// production composition; the duplicate bin-module compilation does not call
// every public composition constructor directly.
#[allow(dead_code)]
mod production;
mod serve_graph;
mod snapshot_identity;
mod summarize_revisions;

/// Open graph store via DA_GRAPH_MODE (default embedded).
/// Remote mode exits with a clear error — no fake success, no server auto-start.
fn open_graph_store() -> da_adapters::SamyamaGraphStore {
    match da_adapters::SamyamaGraphStore::try_from_env() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("❌ graph store open failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Optionally attach pre-GROBID PDF classifier (M348).
///
/// Enabled only when the CLI is built with `--features pdf-classifier`.
/// Default builds stay free of `pdf-inspector` (Null path = no-op).
fn maybe_attach_pdf_classifier(
    use_case: da_application::IngestUseCase,
) -> da_application::IngestUseCase {
    #[cfg(feature = "pdf-classifier")]
    {
        use da_adapters::PdfInspectorClassifier;
        use std::sync::Arc;
        tracing::info!("PDF classifier enabled (pdf-inspector pre-GROBID triage)");
        use_case.with_pdf_classifier(Arc::new(PdfInspectorClassifier::new()))
    }
    #[cfg(not(feature = "pdf-classifier"))]
    {
        use_case
    }
}

/// Open store and optionally import a `.sgsnap` into **this process** (M334).
///
/// Closes the disk→query gap: `load-snapshot` alone cannot feed a later
/// `query`/`graph-stats` process (in-memory dies on exit). Same-process
/// `--snapshot` is the supported read path until Wave 3 server mode.
///
/// Always prints NON-PERSISTENT honesty. D127 unchanged.
async fn open_graph_store_with_optional_snapshot(
    snapshot: Option<&std::path::Path>,
) -> da_adapters::SamyamaGraphStore {
    open_graph_store_with_optional_snapshot_mode(snapshot, true).await
}

async fn open_graph_store_with_optional_snapshot_mode(
    snapshot: Option<&std::path::Path>,
    verbose: bool,
) -> da_adapters::SamyamaGraphStore {
    use da_ports::graph_store::GraphStore;
    let store = open_graph_store();
    if let Some(path) = snapshot {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("❌ Cannot read snapshot {}: {e}", path.display());
                std::process::exit(1);
            }
        };
        if verbose {
            println!(
                "Snapshot preload: {} ({} bytes) into this process...",
                path.display(),
                data.len()
            );
        }
        if let Err(e) = store.import_snapshot(&data).await {
            eprintln!("❌ Snapshot import failed: {e}");
            std::process::exit(1);
        }
        if verbose {
            let nodes = store.node_count().await;
            let edges = store.edge_count().await;
            println!("  loaded: {nodes} nodes, {edges} edges (this process only)");
            println!("  ⚠  NON-PERSISTENT — dies when this process exits");
            println!("  D127: import_eligible default Locked; use --enable-import to unlock");
        }
    }
    store
}

/// daily-archive v2 — scientific knowledge engine
#[derive(Parser)]
#[command(
    name = "da",
    version,
    about = "Scientific knowledge engine (Rust + Samyama Graph)"
)]
struct Cli {
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Source-import FSM surface: status/plan/import per article object
    Source {
        /// Subcommand: status | plan | import | build-* | parse-html | eval-frozen
        action: String,
        /// Object id (paper/chapter/monograph) — not needed by eval-frozen
        #[arg(default_value = "")]
        paper_id: String,
        /// import: report only, do not execute
        #[arg(long, default_value = "true")]
        dry_run: bool,
        /// parse-html: path to the source HTML file
        #[arg(long)]
        html: Option<String>,
        /// parse-pdf: path to the source PDF file
        #[arg(long)]
        pdf: Option<String>,
        /// sona-policy: page_count feature
        #[arg(long)]
        pages: Option<u32>,
        /// sona-policy: toc_bookmarks feature
        #[arg(long)]
        bookmarks: Option<u32>,
        /// sona-policy: text_layer_quality 0..=1
        #[arg(long)]
        quality: Option<f32>,
        /// sona-policy: domain id
        #[arg(long)]
        domain: Option<String>,
    },

    /// ADR-078: candidate learned policy table (learn / inspect). Candidate-only.
    Policy {
        /// Subcommand: learn
        action: String,
        /// Decision point to learn: grammar_route | batch_admission | profile_route
        #[arg(long)]
        point: String,
        /// Observations journal (JSONL)
        #[arg(long, default_value = "data/agent_memory/policy/observations.jsonl")]
        journal: String,
        /// Minimum observations per bucket before it earns a cell
        #[arg(long, default_value_t = 2)]
        min_support: usize,
        /// Artifact output path (default under artifacts/learning/)
        #[arg(long)]
        out: Option<String>,
    },

    /// Validate schema-manifest objects against their DATA norm
    /// (ADR-068: articles today, any schema_objects class later)
    ValidateObjects {
        /// Object class from path_config schema_objects table (default: article)
        #[arg(long, default_value = "article")]
        class: String,

        /// Specific object ids (default: all)
        ids: Vec<String>,
    },

    /// Check infrastructure health
    Health,

    /// Show version info
    Version,

    /// Ingest a single PDF paper
    Ingest {
        /// Path to PDF file
        #[arg(long)]
        pdf: String,

        /// Paper ID (arxiv id)
        #[arg(long)]
        id: String,

        /// Enable production import (D127 unlock, M357). Default: locked.
        /// When set, nodes are marked import_eligible=Eligible.
        #[arg(long, default_value = "false")]
        enable_import: bool,
    },

    /// Acquire arXiv PDF/HTML/metadata into the canonical catalog (Rust-only).
    AcquireArxiv(acquire_arxiv::AcquireArxivArgs),

    /// Batch ingest multiple PDFs in one process (HOT path + snapshot export)
    BatchIngest {
        /// Comma-separated arxiv IDs to ingest (optional if --ids-file set)
        #[arg(long)]
        ids: Option<String>,

        /// Path to file with one arxiv id per line (optional if --ids set)
        #[arg(long)]
        ids_file: Option<PathBuf>,

        /// Output snapshot path (.sgsnap)
        #[arg(long)]
        output: Option<String>,

        /// Write inventory JSON/MD under this directory (ontology-string honest counts)
        #[arg(long)]
        inventory_dir: Option<PathBuf>,

        /// Enable production import (D127 unlock, M357). Default: locked.
        #[arg(long, default_value = "false")]
        enable_import: bool,
    },

    /// Load a snapshot file into the graph (restore durability)
    LoadSnapshot {
        /// Path to .sgsnap file
        #[arg(long)]
        input: String,
    },

    /// Compute exact-byte and strict-semantic identity for a `.sgsnap`.
    SnapshotIdentity(snapshot_identity::SnapshotIdentityArgs),

    /// Show graph statistics (node/edge counts)
    GraphStats {
        /// Optional `.sgsnap` to import into this process before counting (M334).
        /// NON-PERSISTENT: store dies when the process exits.
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },

    /// Query the knowledge graph (Cypher via da-graph builders)
    Query {
        /// Query type: count, by-arxiv, by-vid, sections, orphans, without-evidence
        #[arg(long)]
        kind: String,

        /// Paper arxiv_id or VID (for by-arxiv, by-vid, citation-hops)
        #[arg(long)]
        id: Option<String>,

        /// Max citation hops (for citation-hops)
        #[arg(long, default_value = "2")]
        hops: usize,

        /// Optional `.sgsnap` to import into this process before query (M334).
        /// NON-PERSISTENT: required for disk→query chain (load-snapshot alone does not persist).
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },

    /// Execute one provider-neutral scalar read query in embedded or remote mode.
    GraphQuery(graph_query::GraphQueryArgs),

    /// Durable read-only agent CLI journey (S04): parse plan, fingerprint
    /// the expected .sgsnap, run the verified AgentRunUseCase through a live
    /// read-only serve-graph daemon, capture raw evidence and write an atomic
    /// JSON envelope report (Run). The Verify subcommand recomputes the
    /// evidence chain offline from a written report.
    #[command(subcommand)]
    AgentRun(agent_run::AgentRunCommand),

    /// Query accepted ADR-058 article revision history from a snapshot.
    RevisionQuery {
        /// Versionless canonical article id (for example 2506.07853).
        #[arg(long)]
        id: String,
        /// Selector: current, history, or exact vN (for example v1).
        #[arg(long)]
        selector: String,
        /// Snapshot imported into this process before query.
        #[arg(long)]
        snapshot: PathBuf,
        /// Emit stable JSON instead of human-readable output.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Extract entities from an ingested paper (Phase 3 rule-based)
    Extract {
        /// Paper arxiv_id to extract from
        #[arg(long)]
        id: String,

        /// Optional `.sgsnap` preload into this process before extract (M335).
        #[arg(long)]
        snapshot: Option<PathBuf>,

        /// Optional path to re-export graph after extract (MENTIONS written).
        #[arg(long)]
        output_snapshot: Option<PathBuf>,
    },

    /// Grounded extractive summaries for every PaperRevision already in the graph (M383 S03).
    /// No LLM. Does not invent revision=1 for versionless papers.
    SummarizeRevisions(summarize_revisions::SummarizeRevisionsArgs),

    /// Generate LLM-based summaries for a paper (ADR-053 Layer B)
    Summarize {
        /// Paper arxiv_id to summarize
        #[arg(long)]
        id: String,

        /// Optional `.sgsnap` preload into this process before summarize.
        #[arg(long)]
        snapshot: Option<PathBuf>,

        /// Optional path to re-export graph after summarize (Summary nodes written).
        #[arg(long)]
        output_snapshot: Option<PathBuf>,
    },

    /// Initialize graph schema (indexes) before loading data
    SchemaInit {
        /// Vector dimensions for Paper.embedding index
        #[arg(long, default_value = "1024")]
        dimensions: usize,
    },

    /// Heal/repair graph nodes (silence, correct, merge)
    Heal {
        /// Operation: silence, unsilence, correct, merge
        #[arg(long)]
        op: String,

        /// Node VID to heal
        #[arg(long)]
        vid: String,

        /// Node label (Entity, Paper, etc.)
        #[arg(long, default_value = "Entity")]
        label: String,

        /// For correct: property key to fix
        #[arg(long)]
        key: Option<String>,

        /// For correct: new value
        #[arg(long)]
        value: Option<String>,

        /// For merge: VID of the node to keep
        #[arg(long)]
        keep: Option<String>,

        /// Reason for the healing operation
        #[arg(long, default_value = "manual")]
        reason: String,

        /// Optional `.sgsnap` preload into this process before heal (M335).
        #[arg(long)]
        snapshot: Option<PathBuf>,

        /// Optional path to re-export graph after heal.
        #[arg(long)]
        output_snapshot: Option<PathBuf>,
    },

    /// Enrich paper metadata from OpenAlex (topics, authors, concepts)
    Enrich {
        /// arXiv ID to enrich
        #[arg(long)]
        id: String,

        /// Optional `.sgsnap` preload into this process before enrich (M335).
        #[arg(long)]
        snapshot: Option<PathBuf>,

        /// Optional path to re-export graph after enrich.
        #[arg(long)]
        output_snapshot: Option<PathBuf>,
    },

    /// Batch enrich multiple papers from OpenAlex
    BatchEnrich {
        /// Comma-separated arXiv IDs
        #[arg(long)]
        ids: String,
    },

    /// Run scheduler — process pending OpenAlex enrichment tasks
    SchedulerRun {
        /// Queue directory (default: data/scheduler)
        #[arg(long, default_value = "data/scheduler")]
        queue_dir: String,
    },

    /// Audit pipeline create_node sites and report unregistered labels
    SchemaCheck,

    /// Print the edge endpoint contract matrix as a markdown table.
    EdgeContracts,

    /// Print all registered node schemas as a markdown table.
    /// Use --from-yaml to load via OntologyRegistry instead of hardcoded impls.
    SchemaList {
        /// Load schemas from data/ontology/node_schemas.yaml via OntologyRegistry.
        #[arg(long)]
        from_yaml: bool,
    },

    /// Resolve a prompt by id and version and print its bound schema and fingerprint.
    /// Read-only: prints the resolved registry entry as `key=value` lines and
    /// exits non-zero when the `(id, version)` pair does not resolve.
    PromptRegistry {
        /// Prompt id (e.g. `article_summary`).
        #[arg(long)]
        id: String,
        /// Prompt version (e.g. `summary.v1`).
        #[arg(long)]
        version: String,
    },

    /// Print the cross-reference registry as a markdown table.
    CrossRefs,

    /// Audit pipeline-set fields vs schema-declared fields. Reports
    /// drift between what the pipeline writes and what the schema
    /// declares (ADR-045 / MEM508).
    AuditFields,

    /// Validate the live graph against the schema registry. Reads all
    /// nodes by label and runs the validator on each snapshot.
    /// Requires a running graph store (SamyamaGraphStore).
    ValidateGraph {
        /// Optional: validate only this label (default: all).
        #[arg(long)]
        label: Option<String>,

        /// Optional `.sgsnap` preload before validate (M335).
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },

    /// Validate a single node property snapshot (JSON on stdin).
    /// Example: echo '{"vid":"x","arxiv_id":"y",...}' | da validate-node Paper
    ValidateNode {
        /// Node label
        #[arg(long)]
        label: String,
    },

    /// Export all node schemas to YAML (ADR-050 Phase D). Generates
    /// data/ontology/node_schemas.yaml from all_node_schemas().
    OntologyExport {
        /// Output directory (default: data/ontology/)
        #[arg(long, default_value = "data/ontology")]
        output: String,
    },

    /// Run graph algorithms (page-rank, ppr, wcc) on the knowledge graph.
    /// Builds a GraphView projection from the graph store and prints results.
    Analyze(analyze::AnalyzeArgs),

    /// Start in-memory read-only RESP graph plane (kg-storage ReadOnlyServer).
    /// Accepted: PING ECHO INFO DA.IDENTITY GRAPH.RO_QUERY; mutations rejected
    /// server-side. No RocksDB, no DirectGraphStore, no D127, no auto-start.
    ServeGraph(serve_graph::ServeGraphArgs),

    /// RESP health probe against a running serve-graph (PING → PONG).
    /// Not a full remote GraphStore; does not change open_graph_store.
    GraphPing(graph_ping::GraphPingArgs),

    /// Universal article load plane (ADR-056): locator → ensure catalog assets → LoadReport.
    /// Bare id is catalog-first (offline). An HTTP locator is fetch intent (M461).
    Load(load::LoadArgs),

    /// Bounded resumable parse batch (M464 S06/T04): manifest → typed
    /// per-document outcomes + atomic versioned report, over the
    /// provider-neutral ParseBackend (R116 composition root).
    ParseBatch(parse_batch::ParseBatchArgs),

    /// Opt-in durable local parse-job lifecycle: submit, run, status, cancel.
    #[command(subcommand)]
    ParseJob(parse_job::ParseJobCommand),
}

fn main() {
    // Composition root: load project `.env` so FD_API_KEY / FD_EMBEDDINGS_ENDPOINT
    // reach `from_env` adapters. Missing file is OK (CI / exported env).
    // Never logs secret values. Does not override already-exported vars.
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // M359: structured logging — stderr (compact INFO+) + rolling file (JSON DEBUG+)
    // WorkerGuard must live in main scope — file logs silently stop if dropped early.
    if cli.verbose {
        // Override for verbose mode
        if std::env::var("RUST_LOG").is_err() {
            // SAFETY: single-threaded init before any worker threads spawn
            unsafe { std::env::set_var("RUST_LOG", "debug") };
        }
    }
    let _log_guard = da_cli::logging::init_logging();

    // Single shared Tokio runtime for all async commands (Rust 2026 best practice:
    // avoid repeated Runtime creation; one runtime serves all commands).
    let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Failed to create Tokio runtime: {e}");
        std::process::exit(1);
    });

    match cli.command {
        Commands::Policy {
            action,
            point,
            journal,
            min_support,
            out,
        } => policy_command(&action, &point, &journal, min_support, out.as_deref()),
        Commands::Source {
            action,
            paper_id,
            dry_run,
            html,
            pdf,
            pages,
            bookmarks,
            quality,
            domain,
        } => {
            use da_cli::source_cli;
            let r = match action.as_str() {
                "run" => da_cli::source_cli::run_source_run(&paper_id),
                "run-batch" => {
                    // paper_id here is a comma-separated id list or "auto"
                    // (all processed articles with an incomplete pipeline)
                    let ids: Vec<String> = if paper_id == "auto" {
                        da_cli::source_cli::incomplete_pipeline_ids().unwrap_or_default()
                    } else {
                        paper_id
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    };
                    if ids.is_empty() {
                        println!("run-batch: nothing to do");
                        Ok(())
                    } else {
                        da_cli::source_cli::run_batch(&ids, 8)
                    }
                }
                "acquire" => da_cli::source_acquire::acquire(&paper_id, std::path::Path::new(".")),
                "status" => source_cli::run_status(&paper_id),
                "plan" => source_cli::run_plan(&paper_id),
                "import" => source_cli::run_import(&paper_id, dry_run),
                "build-navigation" => source_cli::run_build_navigation(&paper_id),
                "build-structural" => source_cli::run_build_structural(&paper_id),
                "build-figures" => source_cli::run_build_figures(&paper_id),
                "build-figures-tei" => source_cli::run_build_figures_tei(&paper_id),
                "parse-html" => source_cli::run_parse_html(&paper_id, html.as_deref()),
                "parse-pdf" => {
                    source_cli::run_parse_pdf(&paper_id, pdf.as_deref(), pages, bookmarks, quality)
                }
                "parse-pdf-toc" => source_cli::run_parse_pdf_toc(&paper_id),
                "enrich-layer" => source_cli::run_enrich_layer(&paper_id),
                "audit-fidelity" => source_cli::run_audit_fidelity(&paper_id),
                "sona-analyze" => da_cli::background_tasks::run_sona_analyze(&paper_id),
                "sona-policy" => da_cli::background_tasks::run_sona_policy(
                    &paper_id,
                    pages,
                    bookmarks,
                    quality,
                    domain.as_deref(),
                ),
                "critic-repair" => da_cli::background_tasks::run_critic_repair_cli(&paper_id),
                "memory-admit" => da_cli::background_tasks::run_memory_admit(&paper_id),
                "background-run" => {
                    // da source background-run [task|pid] — task по умолчанию
                    // per_section_summaries; аргумент, похожий на pid (не id задачи),
                    // запускает одну статью
                    let known_tasks = ["per_section_summaries", "debug_html_rebuild"];
                    if known_tasks.contains(&paper_id.as_str()) {
                        da_cli::background_tasks::run_background(&paper_id, "pending")
                    } else {
                        da_cli::background_tasks::run_background("per_section_summaries", &paper_id)
                    }
                }
                "fetch-openalex" => {
                    let mut args: Vec<String> = vec![paper_id.clone()];
                    args.extend(
                        std::env::args()
                            .skip(4)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .filter(|a| a.starts_with("--")),
                    );
                    source_cli::run_fetch_openalex(&args)
                }
                "build-extracted" => source_cli::run_build_extracted(&paper_id),
                "build-summary" => source_cli::run_build_summary(&paper_id),
                "eval-frozen" => source_cli::run_eval_frozen(),
                other => Err(anyhow::anyhow!(
                    "unknown source action {other:?} (status|plan|import|build-navigation|build-structural|build-figures|build-figures-tei|parse-html|parse-pdf-toc|enrich-layer|build-extracted|build-summary|run|acquire|sona-analyze|sona-policy|critic-repair|memory-admit)"
                )),
            };
            if let Err(e) = r {
                eprintln!("da source: {e}");
                std::process::exit(2);
            }
        }
        Commands::ValidateObjects { class, ids } => {
            use da_adapters::fs_schema_objects::FsSchemaObjectStore;
            use da_application::validate_schema_objects::validate_schema_objects;
            use da_ports::path_config::PathConfig;

            let config = PathConfig::load();
            let store = match FsSchemaObjectStore::from_path_config(&config, class.as_str()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("validate-objects: {e}");
                    std::process::exit(2);
                }
            };
            let only: Option<Vec<String>> = if ids.is_empty() {
                None
            } else {
                Some(ids.clone())
            };
            match validate_schema_objects(&store, only.as_deref()) {
                Ok(report) => {
                    for v in &report.violations {
                        println!(
                            "  [{}] {}: {} ({})",
                            format_args!("{:?}", v.kind).to_string().to_lowercase(),
                            v.object_id,
                            v.evidence,
                            v.pattern
                        );
                    }
                    println!(
                        "validated: {} {} objects, {} violations",
                        report.checked,
                        report.object_class,
                        report.violations.len()
                    );
                    if report.is_clean() {
                        std::process::exit(0);
                    } else {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("validate-objects: {e}");
                    std::process::exit(2);
                }
            }
        }
        Commands::Health => {
            rt.block_on(async {
                check_health().await;
            });
        }
        Commands::Version => {
            println!("daily-archive v2.0.0 (Rust)");
            println!("ADR-037/038/039/040/041 — Samyama Graph + RuVector + RVF");
            println!("Phase: ingest + extraction + healing + schema + enrich");
        }
        Commands::Ingest {
            pdf,
            id,
            enable_import,
        } => {
            println!("Ingesting: {} → {}", pdf, id);
            if enable_import {
                println!("⚠  D127 UNLOCKED: --enable-import set; nodes will be marked Eligible");
            }
            rt.block_on(async {
                ingest_pdf(&pdf, &id, enable_import).await;
            });
        }
        Commands::AcquireArxiv(args) => match acquire_arxiv::run(&args) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("acquire-arxiv failed: {error:#}");
                std::process::exit(2);
            }
        },
        Commands::BatchIngest {
            ids,
            ids_file,
            output,
            inventory_dir,
            enable_import,
        } => {
            rt.block_on(async {
                batch_ingest(
                    ids.as_deref(),
                    ids_file.as_deref(),
                    output.as_deref(),
                    inventory_dir.as_deref(),
                    enable_import,
                )
                .await;
            });
        }
        Commands::LoadSnapshot { input } => {
            rt.block_on(async {
                load_snapshot(&input).await;
            });
        }
        Commands::SnapshotIdentity(args) => {
            if let Err(error) = snapshot_identity::run(&args) {
                eprintln!("snapshot-identity failed: {error:#}");
                std::process::exit(2);
            }
        }
        Commands::GraphStats { snapshot } => {
            rt.block_on(async {
                graph_stats(snapshot.as_deref()).await;
            });
        }
        Commands::Query {
            kind,
            id,
            hops,
            snapshot,
        } => {
            rt.block_on(async {
                query_graph(&kind, id.as_deref(), hops, snapshot.as_deref()).await;
            });
        }
        Commands::GraphQuery(args) => {
            rt.block_on(async {
                if let Err(error) = graph_query::run(args).await {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            });
        }
        Commands::AgentRun(command) => match command {
            agent_run::AgentRunCommand::Run(args) => {
                rt.block_on(async {
                    if let Err(error) = agent_run::run(args).await {
                        eprintln!("{error}");
                        std::process::exit(error.exit_code());
                    }
                });
            }
            agent_run::AgentRunCommand::Verify(args) => {
                match agent_run::verify_report(&args.report) {
                    Ok(outcome) => {
                        println!("PASS terminal_state: {}", outcome.terminal_state);
                    }
                    Err(error) => {
                        // Documented --verify contract (D273): print
                        // `FAIL: <first failing check>` for a chain mismatch,
                        // or the plain error for a config problem.
                        if let Some(check) = error.verify_failed_check() {
                            eprintln!("FAIL: {check}");
                        } else {
                            eprintln!("{error}");
                        }
                        std::process::exit(error.exit_code());
                    }
                }
            }
        },
        Commands::RevisionQuery {
            id,
            selector,
            snapshot,
            json,
        } => {
            rt.block_on(async {
                revision_query(&id, &selector, &snapshot, json).await;
            });
        }
        Commands::Extract {
            id,
            snapshot,
            output_snapshot,
        } => {
            rt.block_on(async {
                extract_entities(&id, snapshot.as_deref(), output_snapshot.as_deref()).await;
            });
        }
        Commands::SummarizeRevisions(args) => {
            let code = rt.block_on(async { summarize_revisions::run(&args).await });
            match code {
                Ok(0) => {}
                Ok(status) => std::process::exit(status),
                Err(e) => {
                    eprintln!("summarize-revisions failed: {e:#}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Summarize {
            id,
            snapshot,
            output_snapshot,
        } => {
            rt.block_on(async {
                summarize_paper(&id, snapshot.as_deref(), output_snapshot.as_deref()).await;
            });
        }
        Commands::SchemaInit { dimensions } => {
            schema_init(dimensions);
        }
        Commands::Heal {
            op,
            vid,
            label,
            key,
            value,
            keep,
            reason,
            snapshot,
            output_snapshot,
        } => {
            rt.block_on(async {
                heal_graph(HealGraphArgs {
                    op: &op,
                    vid: &vid,
                    label: &label,
                    key: key.as_deref(),
                    value: value.as_deref(),
                    keep: keep.as_deref(),
                    reason: &reason,
                    snapshot: snapshot.as_deref(),
                    output_snapshot: output_snapshot.as_deref(),
                })
                .await;
            });
        }
        Commands::Enrich {
            id,
            snapshot,
            output_snapshot,
        } => {
            rt.block_on(async {
                enrich_from_openalex(&id, snapshot.as_deref(), output_snapshot.as_deref()).await;
            });
        }
        Commands::BatchEnrich { ids } => {
            rt.block_on(async {
                batch_enrich_from_openalex(&ids).await;
            });
        }
        Commands::SchedulerRun { queue_dir } => {
            rt.block_on(async {
                if let Err(error) = run_scheduler(&queue_dir).await {
                    eprintln!("Scheduler run failed: {error:#}");
                    std::process::exit(1);
                }
            });
        }
        Commands::SchemaCheck => {
            schema_check();
        }
        Commands::EdgeContracts => {
            println!("# Edge endpoint contracts (ADR-045 Wave G)\n");
            print!("{}", da_domain::edge_contract::render_markdown_table());
        }
        Commands::SchemaList { from_yaml } => {
            if from_yaml {
                println!("# Node schemas (ADR-050 Phase F, loaded from YAML)\n");
                let schemas = da_domain::yaml_schema_adapter::all_node_schemas_or_yaml();
                println!("| Label | Required | Optional |\n|-------|----------|----------|");
                for schema in &schemas {
                    let required = schema.required_fields().len();
                    let optional = schema.optional_fields().len();
                    println!("| {} | {} | {} |", schema.label(), required, optional);
                }
                println!("\n_Total: {} schemas_", schemas.len());
            } else {
                println!("# Node schemas (ADR-040 §11.1)\n");
                print!("{}", da_domain::schema::render_node_types_table());
            }
        }
        Commands::PromptRegistry { id, version } => {
            print_prompt_registry_entry(&id, &version);
        }
        Commands::CrossRefs => {
            println!("# Cross-reference registry (ADR-045 Wave F)\n");
            print!("{}", da_domain::validator::render_cross_references_table());
        }
        Commands::AuditFields => {
            audit_fields();
        }
        Commands::ValidateGraph { label, snapshot } => {
            rt.block_on(async {
                validate_graph(label.as_deref(), snapshot.as_deref()).await;
            });
        }
        Commands::ValidateNode { label } => {
            validate_node_stdin(label);
        }

        Commands::OntologyExport { output } => {
            export_ontology_yaml(&output);
        }

        Commands::ServeGraph(args) => {
            println!("serve-graph: starting (Ctrl-C to stop)");
            if let Err(e) = rt.block_on(async { serve_graph::run_in_memory_server(&args).await }) {
                eprintln!("❌ serve-graph failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::GraphPing(args) => {
            if let Err(e) = rt.block_on(async { graph_ping::run_graph_ping(&args).await }) {
                eprintln!("❌ graph-ping failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Analyze(args) => {
            rt.block_on(async {
                let store = open_graph_store_with_optional_snapshot(args.snapshot.as_deref()).await;
                if let Err(e) = analyze::run_analyze(&store, &args).await {
                    eprintln!("Error: {e:#}");
                    std::process::exit(1);
                }
            });
        }
        Commands::Load(args) => {
            if let Err(e) = rt.block_on(async { load::run_load(&args).await }) {
                eprintln!("❌ load failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::ParseBatch(args) => {
            if let Err(e) = rt.block_on(async { parse_batch::run(&args).await }) {
                eprintln!("❌ parse-batch failed: {e}");
                std::process::exit(2);
            }
        }
        Commands::ParseJob(command) => {
            if let Err(e) = rt.block_on(async { parse_job::run(command).await }) {
                eprintln!("❌ parse-job failed: {e}");
                std::process::exit(2);
            }
        }
    }
}

/// Export current process-local store to a `.sgsnap` path (M335).
async fn export_store_snapshot(
    store: &dyn da_ports::graph_store::GraphStore,
    path: &std::path::Path,
) {
    match store.export_snapshot().await {
        Ok(bytes) => {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(path, &bytes) {
                Ok(()) => println!(
                    "  exported snapshot: {} ({} bytes)",
                    path.display(),
                    bytes.len()
                ),
                Err(e) => eprintln!("❌ write snapshot {}: {e}", path.display()),
            }
        }
        Err(e) => eprintln!("❌ export_snapshot failed: {e}"),
    }
}

async fn check_health() {
    use da_adapters::FdApiEmbedder;
    use da_parse::GrobidParser;
    use da_ports::embedder::Embedder;
    use da_ports::graph_store::GraphStore;
    use kg_storage::GraphMode;

    println!("daily-archive v2 — health check");

    // Remote health is a real scalar read probe (M379) but never opens the HOT
    // DirectGraphStore path, auto-starts a daemon, or falls back to embedded.
    match GraphMode::from_env() {
        Ok(GraphMode::Remote) => {
            let status = match std::env::var("DA_GRAPH_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
            {
                None => health_report::RemoteReadHealth::MissingEndpoint,
                Some(endpoint) => match kg_storage::RemoteGraphConfig::new(endpoint)
                    .and_then(kg_storage::RemoteGraphStore::new)
                {
                    Err(error) => health_report::RemoteReadHealth::Unreachable {
                        error_class: health_report::remote_error_class(&error.to_string()),
                    },
                    Ok(graph) => match GraphStore::health(&graph).await {
                        Ok(true) => health_report::RemoteReadHealth::Reachable,
                        Ok(false) => health_report::RemoteReadHealth::Unreachable {
                            error_class: "unhealthy response".into(),
                        },
                        Err(error) => health_report::RemoteReadHealth::Unreachable {
                            error_class: health_report::remote_error_class(&error.to_string()),
                        },
                    },
                },
            };
            for line in health_report::format_remote_graph_health_lines(&status) {
                println!("{line}");
            }
        }
        Ok(GraphMode::Embedded) => match da_adapters::SamyamaGraphStore::try_from_env() {
            Ok(graph) => {
                let healthy = GraphStore::health(&graph).await.unwrap_or(false);
                let nodes = graph.node_count().await;
                for line in health_report::format_graph_health_lines(
                    "embedded",
                    true,
                    healthy,
                    &nodes.to_string(),
                ) {
                    println!("{line}");
                }
            }
            Err(error) => {
                let detail = health_report::open_error_detail(&error);
                for line in
                    health_report::format_graph_health_lines("embedded", false, false, &detail)
                {
                    println!("{line}");
                }
            }
        },
        Err(error) => {
            let detail = health_report::open_error_detail(&error);
            for line in health_report::format_graph_health_lines(&detail, false, false, &detail) {
                println!("{line}");
            }
        }
    }

    // GROBID
    let grobid = GrobidParser::from_env();
    let grobid_ok = grobid.is_alive().await;
    println!(
        "  GROBID:         {}",
        if grobid_ok { "✅ alive" } else { "❌ down" }
    );

    // Embedder env + live probe (M321) — never print key material
    let key_status =
        health_report::FdApiKeyStatus::from_env_value(std::env::var("FD_API_KEY").ok().as_deref());
    println!("  FD_API_KEY:     {}", key_status.display_line());
    if !key_status.is_present() {
        println!("{}", health_report::fd_api_key_tip());
    }
    let embedder = FdApiEmbedder::from_env();
    println!(
        "  Embedder:       {} (dim: {})",
        Embedder::model_id(&embedder),
        Embedder::dimensions(&embedder)
    );
    let probe = if key_status.is_present() {
        match Embedder::embed(&embedder, "daily-archive health probe").await {
            Ok(v) => health_report::EmbedProbeStatus::Ok {
                dimensions: v.len(),
            },
            Err(e) => health_report::EmbedProbeStatus::Failed {
                message: format!("{e}"),
            },
        }
    } else {
        health_report::EmbedProbeStatus::SkippedNoKey
    };
    println!("  Embed probe:    {}", probe.display_line());
}

/// Fail fast when production FdApi path has no key (empty Bearer → 401).
fn require_fd_api_key_for_ingest() {
    let status =
        health_report::FdApiKeyStatus::from_env_value(std::env::var("FD_API_KEY").ok().as_deref());
    if !status.is_present() {
        eprintln!(
            "❌ FD_API_KEY missing after .env load — production embed will 401.\
\n   Set FD_API_KEY in project .env (da loads it at startup) or export it.\
\n   Research path without FD: cargo run -p da-cli --example gepa_hub_research"
        );
        std::process::exit(1);
    }
}

/// ADR-078: `da policy learn` — train a candidate decision table from the
/// observation journal. The run prints the honest measurement (including
/// measured_no_gain when the table equals the observed policy) and never
/// promotes: the artifact is candidate-only, deployed=false, import_eligible=
/// false. Wiring the artifact into a consult is a separate explicit env opt-in.
fn policy_command(action: &str, point: &str, journal: &str, min_support: usize, out: Option<&str>) {
    match action {
        "learn" => {
            let decision_point = da_domain::fsm_policy::DecisionPoint::parse(point)
                .unwrap_or_else(|| panic!("unknown decision point: {point}"));
            use da_application::parse_policy_artifact::save_learned_table;
            use da_application::parse_policy_table::{
                PolicyObservation, TableLearnConfig, learn_decision_table, split_observations,
                table_accuracy,
            };

            let text = std::fs::read_to_string(journal)
                .unwrap_or_else(|e| panic!("observations journal {journal}: {e}"));
            let all: Vec<PolicyObservation> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).expect("journal line must parse"))
                .collect();
            let observations: Vec<PolicyObservation> = all
                .iter()
                .filter(|o| o.decision_point == decision_point)
                .cloned()
                .collect();
            println!(
                "journal: {journal} ({} lines, {}/{} for {point})",
                observations.len(),
                observations.len(),
                all.len()
            );
            if observations.is_empty() {
                eprintln!(
                    "no observations for {point}: run parse-pdf first (the writer appends to the journal), or check --point"
                );
                std::process::exit(1);
            }

            let split = split_observations(&observations).expect("document-disjoint split");
            split.validate().expect("split must validate");
            println!(
                "split ({}): train={} validation={}",
                da_application::parse_policy_table::OBSERVATION_SPLIT_RULE,
                split.train.len(),
                split.validation.len()
            );

            let config = TableLearnConfig { min_support };
            let table = match learn_decision_table(decision_point, &split.train, config) {
                Ok(t) => t,
                Err(e) => {
                    // ADR-078 §5: an honest not-trainable is a result, not a crash.
                    println!("outcome: not_trainable ({e})");
                    println!("locks: candidate-only; nothing written; promotion forbidden");
                    return;
                }
            };
            let (agree, covered) = table_accuracy(&table, &split.validation);
            println!(
                "learned: {} cells, table_sha256={}",
                table.cells.len(),
                &table.table_sha256[..16]
            );
            if covered == 0 {
                println!("validation: no covered buckets (table narrower than validation)");
            } else {
                println!(
                    "validation accuracy: {}/{} = {:.1}%",
                    agree,
                    covered,
                    100.0 * agree as f64 / covered as f64
                );
            }
            println!("locks: candidate-only, deployed=false, import_eligible=false");

            let out_path = out.map(str::to_string).unwrap_or_else(|| {
                std::path::Path::new("artifacts/learning")
                    .join(format!("policy_table_{point}.json"))
                    .to_string_lossy()
                    .into_owned()
            });
            if let Some(parent) = std::path::Path::new(&out_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            save_learned_table(std::path::Path::new(&out_path), &table)
                .unwrap_or_else(|e| panic!("save artifact {out_path}: {e}"));
            println!("artifact: {out_path}");
            println!(
                "opt-in consult: DA_LEARNED_POLICY_TABLE={out_path} (never production by default)"
            );
        }
        other => panic!("unknown policy action {other:?} (learn)"),
    }
}

async fn ingest_pdf(pdf_path: &str, paper_id: &str, enable_import: bool) {
    use da_adapters::FdApiEmbedder;
    use da_application::IngestUseCase;
    use std::sync::Arc;

    require_fd_api_key_for_ingest();

    let parser = Box::new(parse_profile::corpus_grobid_parser());
    let embedder = Box::new(FdApiEmbedder::from_env());
    let context = match production::ProductionContext::from_env(None, None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("❌ Production context failed: {error}");
            std::process::exit(1);
        }
    };
    let graph_store = context.store.clone() as Arc<dyn da_ports::DirectGraphStore>;
    let use_case = maybe_attach_pdf_classifier(IngestUseCase::from_shared_with_clock(
        parser,
        embedder,
        graph_store,
        context.clock.clone(),
    ));
    let use_case = if enable_import {
        use_case.with_import_eligibility(da_domain::ImportEligibility::Eligible)
    } else {
        use_case
    };

    let revision_context = match production::revision_context_for_versioned_pdf(
        std::path::Path::new(pdf_path),
        paper_id,
    ) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("❌ {error}");
            std::process::exit(1);
        }
    };
    match revision_context.as_ref() {
        Some(context) => println!("   Revision: v{} (exact)", context.revision),
        None => println!("   Revision: versionless fallback (no versioned filename evidence)"),
    }

    match use_case
        .ingest_pdf_with_context(pdf_path, paper_id, revision_context.as_ref())
        .await
    {
        Ok(result) => {
            println!("✅ Ingested: {}", result.paper_id);
            println!("   Title:    {}", result.title);
            println!("   Body:     {} chars", result.body_chars);
            println!("   Vector:   {}d", result.vector_dimensions);
            println!(
                "   Graph:    {}",
                result.graph_node_id.map(|_n| "written").unwrap_or("failed")
            );
            println!("   Sections: {}", result.section_count);
            println!(
                "   Citations: {} ({} resolved)",
                result.citation_count, result.cites_resolved
            );
            println!(
                "   Import:   {} (D127)",
                if result.import_eligible {
                    "eligible"
                } else {
                    "locked"
                }
            );
            // M350/M351: machine-readable pre-GROBID triage when classifier ran
            if let Some(ref c) = result.pdf_classification {
                println!(
                    "   PDF:      type={:?} pages={} ocr_pages={:?} encoding_issues={} conf={:.2}",
                    c.pdf_type,
                    c.page_count,
                    c.pages_needing_ocr,
                    c.has_encoding_issues,
                    c.confidence
                );
                // ADR-078/D501: persist the decision condition so later
                // consults and observation writers use the measured features,
                // not the corpus stub. Typed absence: toc_bookmarks is 0
                // because the classifier does not measure it.
                let features =
                    da_application::parse_policy_observation::features_from_classification(c, "");
                let dir = std::path::Path::new("artifacts/pipeline_cache/pdf_policy_features");
                let _ = std::fs::create_dir_all(dir);
                let path = dir.join(format!("{}.json", result.paper_id));
                match serde_json::to_string_pretty(&features) {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&path, json) {
                            eprintln!("⚠️ policy features not cached: {e}");
                        }
                    }
                    Err(e) => eprintln!("⚠️ policy features not cached: {e}"),
                }
                // Compact per-page OCR reasons (e.g. 69=suspected_garbled_text)
                if !c.ocr_reasons_by_page.is_empty() {
                    let reasons: Vec<String> = c
                        .ocr_reasons_by_page
                        .iter()
                        .map(|p| format!("{}={}", p.page, p.reasons.join("+")))
                        .collect();
                    println!("   OCR why:  {}", reasons.join(", "));
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Ingest failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn batch_ingest(
    ids: Option<&str>,
    ids_file: Option<&std::path::Path>,
    output: Option<&str>,
    inventory_dir: Option<&std::path::Path>,
    enable_import: bool,
) {
    use da_adapters::FdApiEmbedder;
    use da_application::{IngestUseCase, batch_ingest_pdfs_with_context};
    use std::sync::Arc;

    let id_list = match batch_ids::resolve_batch_ids(ids, ids_file) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ {e}");
            std::process::exit(1);
        }
    };

    let mut pdfs: Vec<(String, String)> = Vec::new();
    let mut resolved_contexts: Vec<Option<da_application::RevisionContentContext>> = Vec::new();
    for pid in &id_list {
        let resolved = match production::resolve_catalog_pdf(
            std::path::Path::new("data/sources/arxiv"),
            pid,
        ) {
            Ok(resolved) => resolved,
            Err(error) => {
                eprintln!("❌ Catalog revision resolution failed for {pid}: {error}");
                std::process::exit(1);
            }
        };
        let Some(resolved) = resolved else {
            continue;
        };
        if resolved.revision_context.is_some() {
            println!(
                "   {pid}: exact catalog revision binding ({})",
                resolved.binding_method
            );
        } else {
            println!("   {pid}: versionless fallback (no authoritative version match)");
        }
        pdfs.push((resolved.path.display().to_string(), pid.to_string()));
        resolved_contexts.push(resolved.revision_context);
    }

    if pdfs.is_empty() {
        eprintln!("No PDFs found for {} resolved id(s)", id_list.len());
        std::process::exit(1);
    }
    if pdfs.len() < id_list.len() {
        eprintln!(
            "⚠  PDFs found {}/{} — missing catalog entries for some ids",
            pdfs.len(),
            id_list.len()
        );
    }

    println!(
        "Batch ingesting {} papers (HOT path, FdApiEmbedder)...",
        pdfs.len()
    );

    require_fd_api_key_for_ingest();

    let parser = Box::new(parse_profile::corpus_grobid_parser());
    let embedder = Box::new(FdApiEmbedder::from_env());
    let context = match production::ProductionContext::from_env(None, None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("❌ Production context failed: {error}");
            std::process::exit(1);
        }
    };
    let graph_store = context.store.clone() as Arc<dyn da_ports::DirectGraphStore>;
    let ingest = maybe_attach_pdf_classifier(IngestUseCase::from_shared_with_clock(
        parser,
        embedder,
        graph_store,
        context.clock.clone(),
    ));
    let ingest = if enable_import {
        ingest.with_import_eligibility(da_domain::ImportEligibility::Eligible)
    } else {
        ingest
    };

    let snapshot_path = output.map(PathBuf::from);
    let revision_contexts = resolved_contexts;
    let exact = revision_contexts
        .iter()
        .filter(|context| context.is_some())
        .count();
    println!(
        "   Revision bindings: {exact} exact, {} versionless fallback",
        revision_contexts.len() - exact
    );

    let result = batch_ingest_pdfs_with_context(
        &ingest,
        &pdfs,
        &revision_contexts,
        snapshot_path.as_deref(),
    )
    .await;

    match result {
        Ok(r) => {
            println!(
                "✅ Batch complete: {}/{} ok, {} fail, {}ms",
                r.ok, r.total, r.fail, r.duration_ms
            );
            println!("   Body chars: {}", r.total_body_chars);
            println!("   Sections:   {}", r.total_sections);
            println!(
                "   Citations:  {} ({} resolved)",
                r.total_citations, r.total_cites_resolved
            );
            println!("   References written: {}", r.total_references_written);
            println!("   Nodes in graph: {}", ingest.graph_stats().await.0);
            if let Some(ref path) = r.snapshot_path {
                println!("   Snapshot: {}", path);
            }
            // M352: aggregate pre-GROBID PDF triage (when classifier was attached)
            if r.pdf_triage.any() {
                let t = &r.pdf_triage;
                println!(
                    "   PDF triage: classified={} ocr_pages={} encoding_issues={} text={} scanned={} mixed={} image={} unknown={}",
                    t.classified,
                    t.with_ocr_pages,
                    t.encoding_issues,
                    t.type_text,
                    t.type_scanned,
                    t.type_mixed,
                    t.type_image,
                    t.type_unknown
                );
                if !t.papers_with_ocr.is_empty() {
                    println!("   OCR papers (sample): {}", t.papers_with_ocr.join(", "));
                }
            }
            for (pid, err) in &r.errors {
                println!("   FAIL {}: {}", pid, err);
            }

            if let Some(dir) = inventory_dir {
                // Ontology-driven inventory: labels from registered_node_labels(),
                // edge wires from graph histogram (contract_edge_types for known set).
                use da_application::{collect_graph_inventory, render_inventory_markdown};
                use da_domain::relation::{bibliographic, structure};

                let inv = collect_graph_inventory(ingest.graph_store.as_ref()).await;
                let paper_node_count = inv.nodes("Paper");
                let reference_node_count = inv.nodes("Reference");
                let section_node_count = inv.nodes("Section");
                let has_part = inv.edges(structure::HAS_PART);
                let cites = inv.edges(bibliographic::CITES);
                let from_source = inv.edges(structure::FROM_SOURCE);
                let in_category = inv.edges(structure::IN_CATEGORY);
                let mentions = inv.edges(bibliographic::MENTIONS);

                println!(
                    "Inventory: Paper={paper_node_count} Reference={reference_node_count} Section={section_node_count} (ingest sections={}) hasPart={has_part} CITES={cites} FROM_SOURCE={from_source} MENTIONS={mentions} total_nodes={}",
                    r.total_sections, inv.total_nodes
                );
                if !inv.unknown_edge_types().is_empty() {
                    eprintln!(
                        "⚠  unknown edge types (not in edge_contracts): {:?}",
                        inv.unknown_edge_types()
                    );
                }

                let report = serde_json::json!({
                    "embedder": "FdApiEmbedder",
                    "model": "deepvk/USER-bge-m3",
                    "dimensions": 1024,
                    "import_eligible": false,
                    "ids_requested": id_list.len(),
                    "pdfs_found": pdfs.len(),
                    "ingest": {
                        "ok": r.ok,
                        "fail": r.fail,
                        "total": r.total,
                        "duration_ms": r.duration_ms,
                        "total_body_chars": r.total_body_chars,
                        "total_sections": r.total_sections,
                        "total_citations": r.total_citations,
                        "total_cites_resolved": r.total_cites_resolved,
                        "total_references_written": r.total_references_written,
                        "snapshot_path": r.snapshot_path,
                        "errors": r.errors,
                    },
                    "graph": inv,
                    "highlights": {
                        "paper_nodes": paper_node_count,
                        "reference_nodes": reference_node_count,
                        "section_nodes": section_node_count,
                        "cites_edges": cites,
                        "has_part_edges": has_part,
                        "from_source_edges": from_source,
                        "in_category_edges": in_category,
                        "mentions_edges": mentions,
                    },
                    "notes": [
                        "Production path: FdApiEmbedder via dotenv-loaded FD_API_KEY (M321).",
                        "Node labels from da_domain::registered_node_labels() — not CLI hardcode.",
                        "Edge histogram uses store wire strings; known set = edge_contracts().",
                        "MENTIONS=0 expected without extract-to-graph step.",
                        if enable_import {
                            "D127 UNLOCK tested at runtime (--enable-import); durable snapshot stays locked."
                        } else {
                            "D127 import_eligible=false (locked)."
                        },
                        "Process-local graph; durable artifact is snapshot when --output set."
                    ]
                });

                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("⚠  inventory dir create failed: {e}");
                } else {
                    let json_path = dir.join("engine_production.json");
                    match serde_json::to_string_pretty(&report) {
                        Ok(s) => {
                            if let Err(e) = std::fs::write(&json_path, s + "\n") {
                                eprintln!("⚠  write {}: {e}", json_path.display());
                            } else {
                                println!("   Inventory JSON: {}", json_path.display());
                            }
                        }
                        Err(e) => eprintln!("⚠  serialize inventory: {e}"),
                    }
                    let mut md = String::from("# GEPA hub production batch (FdApiEmbedder)\n\n");
                    md.push_str(&format!(
                        "## Ingest\n\n| Metric | Value |\n|--------|--------|\n| PDFs found | {} |\n| OK | {} |\n| Fail | {} |\n| Duration ms | {} |\n| Sections (ingest) | {} |\n| Citations | {} |\n| CITES resolved | {} |\n| References written | {} |\n| Snapshot | `{}` |\n\n",
                        pdfs.len(),
                        r.ok,
                        r.fail,
                        r.duration_ms,
                        r.total_sections,
                        r.total_citations,
                        r.total_cites_resolved,
                        r.total_references_written,
                        r.snapshot_path.as_deref().unwrap_or("none"),
                    ));
                    md.push_str(&render_inventory_markdown(&inv));
                    md.push_str(&format!("\n## Notes\n\n- Embedder: **FdApiEmbedder** (real TEI bge-m3, 1024d).\n- Labels/edges driven by domain ontology + edge_contracts (not hardcoded CLI lists).\n- `import_eligible=false` (D127, durable).{}\n", if enable_import { " Runtime unlock tested (--enable-import); durable state stays locked." } else { "" }));
                    let md_path = dir.join("REPORT.md");
                    if let Err(e) = std::fs::write(&md_path, md) {
                        eprintln!("⚠  write {}: {e}", md_path.display());
                    } else {
                        println!("   Inventory MD: {}", md_path.display());
                    }
                }
            }

            if r.fail > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("❌ Batch failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn load_snapshot(input: &str) {
    use da_ports::graph_store::GraphStore;

    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("❌ Cannot read snapshot {input}: {e}");
            std::process::exit(1);
        }
    };

    println!("Loading snapshot: {} ({} bytes)...", input, data.len());
    // Embedded Samyama is process-local in-memory (GraphStore::new / from_env).
    // import_snapshot here only verifies the bytes decode into a live store;
    // the store dies when this CLI process exits. Cross-process durability is
    // Wave 3 (Samyama server / PersistenceManager) — see doc/PERSISTENCE-ANALYSIS.md.
    let graph = open_graph_store();
    match graph.import_snapshot(&data).await {
        Ok(()) => {
            let nodes = graph.node_count().await;
            println!(
                "✅ Snapshot verified — {} nodes loaded in this process",
                nodes
            );
            eprintln!(
                "⚠  NON-PERSISTENT: in-memory store dies when this process exits.\n                   To query this snapshot: `da query --snapshot {input} --kind count`.\n                   To count nodes: `da graph-stats --snapshot {input}`.\n                   Durable write path: `da batch-ingest --output <snapshot.sgsnap>`.\n                   D127 import_eligible remains locked."
            );
        }
        Err(e) => {
            eprintln!("❌ Snapshot load failed: {e}");
            std::process::exit(1);
        }
    }
}

async fn graph_stats(snapshot: Option<&std::path::Path>) {
    use da_ports::graph_store::GraphStore;
    use kg_storage::GraphMode;

    match GraphMode::from_env() {
        Ok(GraphMode::Remote) => {
            if let Err(error) = graph_query::reject_remote_snapshot(snapshot) {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
            let store = match graph_query::open_remote_store_from_env() {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            };
            let nodes = match store
                .query_readonly("daily_archive", graph_query::REMOTE_NODE_COUNT_CYPHER)
                .await
            {
                Ok(result) => match graph_query::count_from_query_result(&result) {
                    Ok(n) => n,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(error.exit_code());
                    }
                },
                Err(error) => {
                    eprintln!("❌ Query failed: {error}");
                    std::process::exit(1);
                }
            };
            let edges = match store
                .query_readonly("daily_archive", graph_query::REMOTE_EDGE_COUNT_CYPHER)
                .await
            {
                Ok(result) => match graph_query::count_from_query_result(&result) {
                    Ok(n) => n,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(error.exit_code());
                    }
                },
                Err(error) => {
                    eprintln!("❌ Query failed: {error}");
                    std::process::exit(1);
                }
            };
            println!("daily-archive v2 — graph statistics");
            println!("  Nodes: {}", nodes);
            println!("  Edges: {}", edges);
            println!("  Plane: remote RESP read (HOT writes unsupported)");
        }
        Ok(GraphMode::Embedded) => {
            let graph = open_graph_store_with_optional_snapshot(snapshot).await;
            let nodes = graph.node_count().await;
            let edges = graph.edge_count().await;
            println!("daily-archive v2 — graph statistics");
            println!("  Nodes: {}", nodes);
            println!("  Edges: {}", edges);
            if nodes == 0 {
                println!(
                    "  ⚠  Graph is empty (process-local in-memory store).\n     Use `da graph-stats --snapshot <file.sgsnap>` to load from disk in this process.\n     `da load-snapshot` alone does not feed later commands (store dies on exit).\n     Durable write path: `da batch-ingest --output <snapshot.sgsnap>`."
                );
            }
        }
        Err(error) => {
            eprintln!("❌ graph store open failed: {error}");
            std::process::exit(1);
        }
    }
}

async fn query_graph(
    kind: &str,
    id: Option<&str>,
    hops: usize,
    snapshot: Option<&std::path::Path>,
) {
    use da_application::{EntityQueries, PaperQueries};

    // Build the Cypher via da-graph query builders
    let cypher = match kind {
        "count" => PaperQueries::count_all(),
        "by-arxiv" => {
            let aid = match id {
                Some(a) => a,
                None => {
                    eprintln!("❌ --id required for by-arxiv");
                    std::process::exit(1);
                }
            };
            PaperQueries::find_by_arxiv_id(aid)
        }
        "by-vid" => {
            let vid = match id {
                Some(v) => v,
                None => {
                    eprintln!("❌ --id required for by-vid");
                    std::process::exit(1);
                }
            };
            PaperQueries::find_by_vid(vid)
        }
        "orphans" => EntityQueries::orphans(),
        "without-evidence" => {
            let label = id.unwrap_or("Paper");
            EntityQueries::without_evidence(label)
        }
        "citation-hops" => {
            let vid = match id {
                Some(v) => v,
                None => {
                    eprintln!("❌ --id required for citation-hops");
                    std::process::exit(1);
                }
            };
            PaperQueries::citation_neighborhood(vid, hops)
        }
        _ => {
            eprintln!("❌ Unknown query kind: {kind}");
            eprintln!(
                "   Available: count, by-arxiv, by-vid, orphans, without-evidence, citation-hops"
            );
            std::process::exit(1);
        }
    };

    println!("Query kind: {kind}");
    println!("Cypher: {cypher}");

    if graph_query::operator_query_needs_admit(kind)
        && let Err(error) = graph_query::admit_operator_read_from_env(id)
    {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }

    // Embedded: SamyamaGraphStore (WARM path). Remote: RESP read plane.
    // Optional --snapshot is embedded-only (M334 / M421).
    use da_ports::graph_store::GraphStore;
    use kg_storage::GraphMode;
    let result = match GraphMode::from_env() {
        Ok(GraphMode::Remote) => {
            if let Err(error) = graph_query::reject_remote_snapshot(snapshot) {
                eprintln!("{error}");
                std::process::exit(error.exit_code());
            }
            let store = match graph_query::open_remote_store_from_env() {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(error.exit_code());
                }
            };
            store.query_readonly("daily_archive", &cypher).await
        }
        Ok(GraphMode::Embedded) => {
            let store = open_graph_store_with_optional_snapshot(snapshot).await;
            store.query_readonly("daily_archive", &cypher).await
        }
        Err(error) => {
            eprintln!("❌ graph store open failed: {error}");
            std::process::exit(1);
        }
    };
    match result {
        Ok(result) => {
            println!("Columns: {:?}", result.columns);
            println!("Records: {}", result.records.len());
            for (i, row) in result.records.iter().take(10).enumerate() {
                println!("  [{i}] {row:?}");
            }
            if result.records.len() > 10 {
                println!("  ... ({} more)", result.records.len() - 10);
            }
            // Tip when store likely empty (no --snapshot): zero records OR count query = 0
            let looks_empty = result.records.is_empty()
                || (kind == "count"
                    && result
                        .records
                        .first()
                        .map(|r| format!("{r:?}"))
                        .map(|s| s.contains("Number(0)"))
                        .unwrap_or(false));
            if looks_empty && snapshot.is_none() {
                println!(
                    "  ⚠  Empty graph in this process?\n     Disk→query same process: `da query --snapshot <file.sgsnap> --kind {kind}`.\n     `da load-snapshot` in a prior process does not persist (store dies on exit)."
                );
            }
        }
        Err(e) => {
            eprintln!("❌ Query failed: {e}");
            std::process::exit(1);
        }
    }
}

async fn revision_query(
    canonical_id: &str,
    selector: &str,
    snapshot: &std::path::Path,
    json: bool,
) {
    use da_application::{RevisionQueryUseCase, RevisionSelector};
    use std::str::FromStr;
    use std::sync::Arc;

    let selector = match RevisionSelector::from_str(selector) {
        Ok(selector) => selector,
        Err(error) => {
            eprintln!("❌ {error}");
            std::process::exit(2);
        }
    };
    let store = open_graph_store_with_optional_snapshot_mode(Some(snapshot), !json).await;
    let use_case = RevisionQueryUseCase::new(Arc::new(store));
    match use_case.query(canonical_id, selector).await {
        Ok(response) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response)
                        .expect("RevisionQueryResponse is serializable")
                );
            } else {
                println!("Article {} revisions ({selector:?})", response.canonical_id);
                println!("  Paper VID: {}", response.paper_vid);
                for revision in response.revisions {
                    println!(
                        "  v{} current={} sections={} manifestations={} predecessor={}",
                        revision.revision,
                        revision.is_current,
                        revision.section_count,
                        revision.manifestations.len(),
                        revision
                            .predecessor_revision
                            .map(|value| format!("v{value}"))
                            .unwrap_or_else(|| "none".into())
                    );
                }
                println!("  import_eligible: {}", response.import_eligible);
            }
        }
        Err(error) => {
            eprintln!("❌ Revision query failed: {error}");
            std::process::exit(3);
        }
    }
}

/// Resolve a prompt by `(id, version)` from the DATA prompt registry and
/// print its bound schema and fingerprint as stable `key=value` lines.
///
/// Read-only surface: nothing here writes a graph, a cache, or DATA. A load
/// failure or an unresolved pair aborts the command with a non-zero exit so a
/// caller can distinguish "resolved" from "not registered".
fn print_prompt_registry_entry(prompt_id: &str, prompt_version: &str) {
    use da_adapters::{
        prompt_registry::load_prompt_registry, prompt_task_registry::load_prompt_task_registry,
    };
    let task_registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let prompt_registry_path = std::env::var("DA_PROMPT_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_registry.yaml".into());
    let task_registry = match load_prompt_task_registry(std::path::Path::new(&task_registry_path)) {
        Ok(task_registry) => task_registry,
        Err(error) => {
            eprintln!("❌ prompt task registry failed: {error}");
            std::process::exit(1);
        }
    };
    let registry = match load_prompt_registry(
        std::path::Path::new(&prompt_registry_path),
        &task_registry,
        std::path::Path::new("."),
    ) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("❌ prompt registry failed: {error}");
            std::process::exit(1);
        }
    };
    let entry = match registry.resolve(prompt_id, prompt_version) {
        Ok(entry) => entry,
        Err(error) => {
            eprintln!("❌ {error}");
            std::process::exit(1);
        }
    };
    let kind = match entry.kind {
        da_domain::prompt_registry::PromptEntryKind::Task => "task",
        da_domain::prompt_registry::PromptEntryKind::MetaPrompt => "meta_prompt",
    };
    println!("prompt_id={}", entry.prompt_id);
    println!("prompt_version={}", entry.prompt_version);
    println!("kind={kind}");
    println!("prompt_sha256={}", entry.prompt_sha256.as_str());
    println!("schema_ref={}", entry.schema_ref);
    println!("schema_sha256={}", entry.schema_sha256.as_str());
    println!("source_ref={}", entry.source_ref);
}

async fn summarize_paper(
    paper_id: &str,
    snapshot: Option<&std::path::Path>,
    output_snapshot: Option<&std::path::Path>,
) {
    use da_adapters::{
        model_registry::{load_model_registry, validate_prompt_model_bindings},
        openai_compatible_llm::OpenAiCompatibleLlmClient,
        prompt_execution::TextPromptExecutionAdapter,
        prompt_registry::load_prompt_registry,
        prompt_task_registry::load_prompt_task_registry,
    };
    use da_application::SummaryUseCase;
    use da_ports::graph_store::DirectGraphStore;
    use da_ports::parser::ParserPort;
    use std::sync::Arc;

    // Find PDF
    let pdf = std::process::Command::new("find")
        .args(["data/sources/arxiv", "-name", &format!("{}.pdf", paper_id)])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let pdf_path = match pdf {
        Some(p) => p,
        None => {
            eprintln!("\u{274c} PDF not found for {paper_id}");
            std::process::exit(1);
        }
    };

    // Parse via GROBID (DATA profile_route, D204 structural default).
    let parser = parse_profile::corpus_grobid_parser();
    if !parser.is_alive().await {
        eprintln!("\u{274c} GROBID not alive");
        std::process::exit(1);
    }
    let parsed = parser
        .parse_pdf(&pdf_path, paper_id)
        .await
        .expect("GROBID parse failed");
    println!(
        "Parsed: {} sections, {} chars abstract",
        parsed.sections.len(),
        parsed.abstract_text.len()
    );

    // Open graph store + load snapshot
    let store = open_graph_store_with_optional_snapshot(snapshot).await;

    // Resolve the exact requested Paper. Never select the first node from a
    // multi-paper graph: that would attach summaries to the wrong article.
    let paper_node_id = match da_application::find_paper_node_id(&store, paper_id).await {
        Ok(node_id) => node_id,
        Err(error) => {
            eprintln!("❌ {error}");
            std::process::exit(1);
        }
    };
    println!("Paper node: {paper_node_id}");

    // Generate lineage-bound, candidate-only summaries through ADR-059.
    let registry_path = std::env::var("DA_PROMPT_TASK_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_tasks.yaml".into());
    let registry = match load_prompt_task_registry(std::path::Path::new(&registry_path)) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("❌ prompt task registry failed: {error}");
            std::process::exit(1);
        }
    };
    // Bind the local task/schema contracts to the unified prompt registry
    // (S09). The registry is required: a fail-open (no registry) summary is
    // not representable, so a load error aborts the command.
    let prompt_registry_path = std::env::var("DA_PROMPT_REGISTRY")
        .unwrap_or_else(|_| "data/ontology/prompt_registry.yaml".into());
    let prompt_registry = match load_prompt_registry(
        std::path::Path::new(&prompt_registry_path),
        &registry,
        std::path::Path::new("."),
    ) {
        Ok(prompt_registry) => prompt_registry,
        Err(error) => {
            eprintln!("❌ unified prompt registry failed: {error}");
            std::process::exit(1);
        }
    };
    let task = registry.task("article_summary").unwrap().clone();
    let route = registry.route(&task.route_policy_id).unwrap().clone();
    let output_schema = registry
        .output_schema(&task.output_schema_id)
        .unwrap()
        .clone();
    let model_registry_path =
        std::env::var("DA_MODEL_REGISTRY").unwrap_or_else(|_| "models.yaml".into());
    let model_registry = load_model_registry(std::path::Path::new(&model_registry_path))
        .unwrap_or_else(|error| {
            eprintln!("❌ model registry failed: {error}");
            std::process::exit(1);
        });
    validate_prompt_model_bindings(&model_registry, &registry).unwrap_or_else(|error| {
        eprintln!("❌ prompt/model registry mismatch: {error}");
        std::process::exit(1);
    });
    let endpoint_override = route
        .endpoint_override_env
        .as_deref()
        .and_then(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty());
    let resolved_model = model_registry
        .resolve(&route.model_binding_id, endpoint_override.as_deref())
        .unwrap_or_else(|error| {
            eprintln!("❌ model binding failed: {error}");
            std::process::exit(1);
        });
    let credential = std::env::var(&route.credential_env).ok();
    let timeout_seconds = std::env::var("DA_LLM_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let llm: Arc<dyn da_ports::LLMClient> = Arc::new(
        OpenAiCompatibleLlmClient::new(resolved_model.clone(), credential, timeout_seconds)
            .expect("summary LLM composition failed"),
    );
    println!(
        "LLM ready: {} via {}",
        resolved_model.model_name, resolved_model.binding_id
    );
    let prompt_executor = Arc::new(TextPromptExecutionAdapter::new(llm));
    let store_arc: Arc<dyn DirectGraphStore> = Arc::new(store);
    let use_case = SummaryUseCase::new(
        store_arc.clone(),
        prompt_executor,
        task,
        route,
        resolved_model,
        output_schema,
        prompt_registry,
    )
    .expect("summary prompt composition failed");
    let result = use_case
        .generate_summaries(paper_id, paper_node_id, &parsed)
        .await
        .expect("summary generation failed");

    if let Some(e) = &result.error {
        eprintln!("\u{274c} Summary error: {e}");
    } else {
        println!("\u{2705} Summaries generated for {}", result.paper_id);
        if let Some(ref abs) = result.abstract_summary {
            println!("   Abstract summary ({} chars):", abs.len());
            println!("   {}", abs);
        }
        println!("   Section summaries: {}", result.section_summaries_count);
        println!("   Methods: {}", result.methods_count);
        println!("   Datasets: {}", result.datasets_count);
        println!("   Metrics: {}", result.metrics_count);
        println!("   Tags: {}", result.tags.join(", "));
        if let Some(score) = result.reproducibility_score {
            println!("   Reproducibility: {}/5", score);
        }
        println!("   Tokens used: {}", result.tokens_used);
    }

    // Export snapshot if requested
    if let Some(out) = output_snapshot {
        let data = store_arc.export_snapshot().await.expect("export failed");
        std::fs::write(out, &data).expect("write failed");
        println!(
            "Snapshot exported: {} ({} bytes)",
            out.display(),
            data.len()
        );
    }
}

async fn extract_entities(
    paper_id: &str,
    snapshot: Option<&std::path::Path>,
    output_snapshot: Option<&std::path::Path>,
) {
    use da_adapters::RuleBasedExtractor;
    use da_application::ExtractionUseCase;
    use da_ports::parser::ParserPort;
    use std::sync::Arc;

    let resolved =
        match production::resolve_catalog_pdf(std::path::Path::new("data/sources/arxiv"), paper_id)
        {
            Ok(resolved) => resolved,
            Err(error) => {
                eprintln!("❌ Catalog revision resolution failed for {paper_id}: {error}");
                std::process::exit(1);
            }
        };
    let (pdf_path, revision_context, binding_method) = match resolved {
        Some(resolved) => (
            resolved.path.display().to_string(),
            resolved.revision_context,
            resolved.binding_method,
        ),
        None => {
            eprintln!("❌ PDF not found for {paper_id}");
            std::process::exit(1);
        }
    };

    println!("Extracting entities from {paper_id}...");

    // Parse via GROBID (DATA profile_route, D204 structural default).
    let parser = parse_profile::corpus_grobid_parser();
    let parsed = match parser.parse_pdf(&pdf_path, paper_id).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Parse failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "   Parsed: {} sections, {} chars",
        parsed.sections.len(),
        parsed.body_text.len()
    );

    // Extract via rule-based extractor
    let extractor = Box::new(RuleBasedExtractor::new());
    let store = Arc::new(open_graph_store_with_optional_snapshot(snapshot).await);
    let context = match production::ProductionContext::new(store, None, None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("❌ Production context failed: {error}");
            std::process::exit(1);
        }
    };
    let graph_store = context.store.clone() as Arc<dyn da_ports::DirectGraphStore>;
    let use_case =
        ExtractionUseCase::from_shared(extractor, graph_store).with_clock(context.clock.clone());
    match revision_context.as_ref() {
        Some(context) => println!(
            "   Revision: v{} (exact, {binding_method})",
            context.revision
        ),
        None => println!("   Revision: versionless fallback (no authoritative version match)"),
    }

    match use_case
        .extract_from_parsed_with_context(&parsed, revision_context.as_ref())
        .await
    {
        Ok(result) => {
            println!("✅ Extracted {} entities", result.entities_extracted);
            // Group by type
            let mut by_type: std::collections::HashMap<&str, usize> =
                std::collections::HashMap::new();
            for t in &result.entity_types {
                *by_type.entry(t.as_str()).or_default() += 1;
            }
            for (t, count) in &by_type {
                println!("   {t}: {count}");
            }
            println!("   Graph nodes written: {}", result.graph_node_ids.len());
            if let Some(out) = output_snapshot {
                export_store_snapshot(use_case.graph_store.as_ref(), out).await;
            }
        }
        Err(e) => {
            eprintln!("❌ Extraction failed: {e:#}");
            std::process::exit(1);
        }
    }
}

fn schema_init(dimensions: usize) {
    // GRAPH-SCHEMA.md: create all indexes via HOT path (direct API), not Cypher.
    // Samyama property indexes use IndexManager::create_index, not CREATE INDEX DDL.
    println!("daily-archive v2 — schema initialization");
    println!(
        "  Schema version: {}",
        da_application::schema_init::CURRENT_SCHEMA_VERSION
    );

    let identity_policy = match da_cli::gold_graph::load_identity_indexes(
        std::path::Path::new("data/ontology/identity_indexes.yaml"),
        std::path::Path::new("data/ontology/node_schemas.yaml"),
    ) {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("  ❌ Identity index policy failed: {error:#}");
            std::process::exit(1);
        }
    };
    let property_policy = match da_cli::gold_graph::load_property_indexes(
        std::path::Path::new("data/ontology/property_indexes.yaml"),
        std::path::Path::new("data/ontology/node_schemas.yaml"),
    ) {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("  ❌ Property index policy failed: {error:#}");
            std::process::exit(1);
        }
    };
    // Vector indexes: (label, property, dimensions)
    let vector_indexes = [("Paper", "embedding", dimensions)];

    println!(
        "  Indexes to create: {} identity + {} property + {} vector",
        identity_policy.indexes().len(),
        property_policy.indexes().len(),
        vector_indexes.len()
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use da_ports::graph_store::{GraphStore, VectorMetric};
        let store = open_graph_store();
        let mut ok = 0;
        let mut fail = 0;

        match da_cli::gold_graph::ensure_identity_indexes(&store, &identity_policy).await {
            Ok(()) => {
                for index in identity_policy.indexes() {
                    println!("    [identity] {}.{}", index.label, index.property);
                }
                ok += identity_policy.indexes().len();
            }
            Err(error) => {
                eprintln!("  ❌ Identity index policy application failed: {error:#}");
                std::process::exit(1);
            }
        }

        match da_cli::gold_graph::ensure_property_indexes(&store, &property_policy).await {
            Ok(()) => {
                for index in property_policy.indexes() {
                    println!("    [property] {}.{}", index.label, index.property);
                }
                ok += property_policy.indexes().len();
            }
            Err(error) => {
                eprintln!("  ❌ Property index policy application failed: {error:#}");
                std::process::exit(1);
            }
        }

        for (label, property, dims) in &vector_indexes {
            match store
                .create_vector_index(label, property, *dims, VectorMetric::Cosine)
                .await
            {
                Ok(()) => {
                    println!("    [vector] {label}.{property} (dim={dims}, cosine)");
                    ok += 1;
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("  ❌ Vector index {label}.{property} failed: {e}");
                }
            }
        }

        println!("  Result: {ok} created, {fail} failed");
        if fail > 0 {
            std::process::exit(1);
        }
        println!("  Schema ready — load data with `da batch-ingest` / `da extract`");
    });
}

/// Args for `heal_graph` (M349: avoids clippy `too_many_arguments`).
struct HealGraphArgs<'a> {
    op: &'a str,
    vid: &'a str,
    label: &'a str,
    key: Option<&'a str>,
    value: Option<&'a str>,
    keep: Option<&'a str>,
    reason: &'a str,
    snapshot: Option<&'a std::path::Path>,
    output_snapshot: Option<&'a std::path::Path>,
}

async fn heal_graph(args: HealGraphArgs<'_>) {
    use da_application::GraphHealingUseCase;
    use da_domain::healing::HealingActor;

    let HealGraphArgs {
        op,
        vid,
        label,
        key,
        value,
        keep,
        reason,
        snapshot,
        output_snapshot,
    } = args;

    let graph_store = Box::new(open_graph_store_with_optional_snapshot(snapshot).await);
    let use_case = GraphHealingUseCase::new(graph_store);
    let actor = HealingActor::Human("cli".to_string());

    let result = match op {
        "silence" => use_case.silence(vid, label, reason, actor).await.map(|_| {
            println!("✅ Silenced: {vid} ({label})");
            println!("   Reason: {reason}");
        }),
        "unsilence" => use_case.unsilence(vid, label, actor).await.map(|_| {
            println!("✅ Un-silenced: {vid} ({label}) — retrieval_eligible restored");
        }),
        "correct" => {
            let key = key.unwrap_or("label");
            let value = value.unwrap_or("");
            use_case
                .correct(vid, label, key, value, reason, actor)
                .await
                .map(|_| {
                    println!("✅ Corrected: {vid}.{key} → {value}");
                })
        }
        "merge" => {
            let keep_vid = keep.unwrap_or_else(|| {
                eprintln!("❌ --keep required for merge");
                std::process::exit(1);
            });
            use_case.merge(keep_vid, vid, reason, actor).await.map(|_| {
                println!("✅ Merged: {vid} → {keep_vid}");
                println!("   SUPERSEDES edge created");
            })
        }
        _ => {
            eprintln!("❌ Unknown heal op: {op}");
            eprintln!("   Available: silence, unsilence, correct, merge");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("❌ Heal failed: {e:#}");
        std::process::exit(1);
    }
    if let Some(out) = output_snapshot {
        export_store_snapshot(use_case.graph_store.as_ref(), out).await;
    }
}

async fn enrich_from_openalex(
    arxiv_id: &str,
    snapshot: Option<&std::path::Path>,
    output_snapshot: Option<&std::path::Path>,
) {
    use da_adapters::OpenAlexHttpAdapter;
    use da_application::EnrichUseCase;

    println!("Enriching {arxiv_id} from OpenAlex...");

    let openalex = Box::new(OpenAlexHttpAdapter::new());
    let graph_store = Box::new(open_graph_store_with_optional_snapshot(snapshot).await);
    let use_case = EnrichUseCase::new(openalex, graph_store);

    match use_case.enrich_by_arxiv_id(arxiv_id).await {
        Ok(result) => {
            if result.openalex_pending {
                println!("⏳ Pending: OpenAlex has no data for {arxiv_id}");
                println!("   Paper marked openalex_pending=true");
                println!("   Re-run enrich later when OpenAlex indexes this paper");
            } else {
                println!("✅ Enriched: {}", result.title);
                println!("   OpenAlex ID: {}", result.openalex_id);
                if let Some(ref doi) = result.doi {
                    println!("   DOI: {doi}");
                }
                println!("   Topics: {}", result.topics_written);
                println!("   Authors: {}", result.authors_written);
                println!("   Concepts: {} (deprecated)", result.concepts_written);
                println!("   Cited by: {}", result.cited_by_count);
                println!(
                    "   CITES edges: {} (from referenced_works)",
                    result.cites_edges_written
                );
            }
            if let Some(out) = output_snapshot {
                export_store_snapshot(use_case.graph_store.as_ref(), out).await;
            }
        }
        Err(e) => {
            eprintln!("❌ Enrich failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn batch_enrich_from_openalex(ids_str: &str) {
    use da_adapters::OpenAlexHttpAdapter;
    use da_application::EnrichUseCase;

    let ids: Vec<&str> = ids_str.split(',').map(|s| s.trim()).collect();
    println!("Batch enriching {} papers from OpenAlex...", ids.len());

    let openalex = Box::new(OpenAlexHttpAdapter::new());
    let graph_store = Box::new(open_graph_store());
    let use_case = EnrichUseCase::new(openalex, graph_store);

    let mut ok = 0;
    let mut pending = 0;
    let mut fail = 0;
    let mut total_topics = 0;
    let mut total_authors = 0;
    let mut total_cites_edges = 0;

    for arxiv_id in &ids {
        match use_case.enrich_by_arxiv_id(arxiv_id).await {
            Ok(result) => {
                if result.openalex_pending {
                    println!("  ⏳ {}: pending (not in OpenAlex yet)", arxiv_id);
                    pending += 1;
                } else {
                    println!(
                        "  ✅ {}: {} topics, {} authors, {} cites",
                        arxiv_id,
                        result.topics_written,
                        result.authors_written,
                        result.cites_edges_written
                    );
                    total_topics += result.topics_written;
                    total_authors += result.authors_written;
                    total_cites_edges += result.cites_edges_written;
                    ok += 1;
                }
            }
            Err(e) => {
                println!("  ❌ {}: {e}", arxiv_id);
                fail += 1;
            }
        }
    }

    println!(
        "\nBatch enrich complete: {}/{} enriched, {} pending, {} fail, {} topics, {} authors, {} cites edges",
        ok,
        ids.len(),
        pending,
        fail,
        total_topics,
        total_authors,
        total_cites_edges
    );
}

async fn run_scheduler(queue_dir: &str) -> anyhow::Result<()> {
    use da_adapters::OpenAlexHttpAdapter;
    use da_application::SchedulerRunner;
    use da_ports::graph_store::GraphStore;
    use std::path::PathBuf;
    use std::sync::Arc;

    let snapshot_path = PathBuf::from(queue_dir).join("scheduler-state.sgsnap");
    let shared_store = Arc::new(open_graph_store());

    if snapshot_path.exists() {
        let data = std::fs::read(&snapshot_path).map_err(|error| {
            anyhow::anyhow!(
                "read scheduler snapshot {}: {error}",
                snapshot_path.display()
            )
        })?;
        GraphStore::import_snapshot(shared_store.as_ref(), &data)
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "restore scheduler snapshot {}: {error}",
                    snapshot_path.display()
                )
            })?;
        println!("Scheduler: restored state from {}", snapshot_path.display());
    }

    let graph_store: Arc<dyn da_ports::graph_store::DirectGraphStore> = shared_store.clone();
    let openalex: Arc<dyn da_ports::openalex::OpenAlexClient> =
        Arc::new(OpenAlexHttpAdapter::new());
    let extract_executor = da_application::ExtractSchedulerExecutor::new(
        graph_store.clone(),
        Box::new(da_adapters::RuleBasedExtractor::new()),
    );
    let catalog_locator = Arc::new(da_application::FilesystemCatalogPdfLocator::from_env());
    let ingest_executor = da_application::IngestSchedulerExecutor::new(
        catalog_locator.clone(),
        Box::new(parse_profile::corpus_grobid_parser()),
        Box::new(da_adapters::FdApiEmbedder::from_env()),
        graph_store.clone(),
    );
    let reparse_executor = da_application::ReparseSchedulerExecutor::new(
        catalog_locator,
        Box::new(parse_profile::corpus_grobid_parser()),
        Box::new(da_adapters::FdApiEmbedder::from_env()),
        graph_store.clone(),
    );
    let embed_stale_executor = da_application::EmbedStaleSchedulerExecutor::new(
        graph_store.clone(),
        Box::new(da_adapters::FdApiEmbedder::from_env()),
    );
    let report = SchedulerRunner::new(graph_store, openalex)
        .with_executor(Arc::new(extract_executor))
        .with_executor(Arc::new(ingest_executor))
        .with_executor(Arc::new(reparse_executor))
        .with_executor(Arc::new(embed_stale_executor))
        .run_due()
        .await?;

    if let Some(parent) = snapshot_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            anyhow::anyhow!(
                "create scheduler snapshot directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let snapshot = GraphStore::export_snapshot(shared_store.as_ref())
        .await
        .map_err(|error| anyhow::anyhow!("export scheduler snapshot: {error}"))?;
    let temporary = snapshot_path.with_extension("sgsnap.tmp");
    std::fs::write(&temporary, snapshot).map_err(|error| {
        anyhow::anyhow!("write scheduler snapshot {}: {error}", temporary.display())
    })?;
    std::fs::rename(&temporary, &snapshot_path).map_err(|error| {
        anyhow::anyhow!(
            "commit scheduler snapshot {}: {error}",
            snapshot_path.display()
        )
    })?;

    println!(
        "Scheduler run complete: due={}, completed={}, pending={}, failed={}",
        report.due, report.completed, report.pending, report.failed
    );
    for outcome in report.outcomes {
        println!(
            "  {} [{:?}]: {}",
            outcome.arxiv_id, outcome.disposition, outcome.message
        );
    }
    println!("Scheduler: state saved to {}", snapshot_path.display());
    Ok(())
}

/// Audit pipeline source files for create_node("Label") call sites and
/// verify each label is registered in da_domain::schema::all_node_schemas().
/// Exits with code 1 if any unregistered labels are found.
///
/// This is the CLI counterpart of `schema_audit_test.rs`. Run ad-hoc to
/// catch drift between pipeline materialization and the schema registry
/// without waiting for CI (ADR-045 Wave E).
fn schema_check() {
    use da_domain::schema::all_node_schemas;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;

    println!("daily-archive v2 — schema-check (ADR-045 Wave E)");
    println!();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("../da-application/src");
    let src_dir = match src_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Could not locate da-application/src directory");
            std::process::exit(2);
        }
    };

    // Collect all create_node("Label") call sites by scanning *.rs files.
    let mut used_labels: HashSet<String> = HashSet::new();
    let mut scanned_files = 0usize;
    if let Ok(entries) = fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            scanned_files += 1;
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for line in content.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                if let Some(start) = line.find("create_node(\"") {
                    let after = &line[start + "create_node(\"".len()..];
                    if let Some(end) = after.find("\")") {
                        let label = &after[..end];
                        if !label.is_empty()
                            && label.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            used_labels.insert(label.to_string());
                        }
                    }
                }
            }
        }
    }

    let registered: HashSet<String> = all_node_schemas()
        .into_iter()
        .map(|s| s.label().to_string())
        .collect();

    println!("Scanned {scanned_files} .rs files under da-application/src");
    println!(
        "Found {} distinct node labels referenced via create_node()",
        used_labels.len()
    );
    println!(
        "Schema registry contains {} declared node types",
        registered.len()
    );
    println!();

    let mut unregistered: Vec<&String> = used_labels
        .iter()
        .filter(|l| !registered.contains(*l))
        .collect();
    unregistered.sort();
    if unregistered.is_empty() {
        println!("✅ OK — every create_node label is registered in all_node_schemas()");
    } else {
        println!(
            "❌ {} UNREGISTERED label(s) found in pipeline but not in schema registry:",
            unregistered.len()
        );
        for label in &unregistered {
            println!("   - {label}");
        }
        println!();
        println!("Fix: add a Schema struct for each missing label and register it");
        println!("in da_domain::schema::all_node_schemas().");
        std::process::exit(1);
    }
}

/// Validate a single node property snapshot read from stdin.
/// Exits with code 1 if the snapshot has any Critical violations.
/// Output is human-readable; pair with `jq` or `cat` for pipelines.
fn validate_node_stdin(label: String) {
    use std::io::Read;
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("validate-node: failed to read stdin");
        std::process::exit(2);
    }
    let props: da_domain::validator::PropertySnapshot = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("validate-node: invalid JSON on stdin: {e}");
            std::process::exit(2);
        }
    };
    let violations = da_domain::validator::validate_node_properties(&label, &props);
    if violations.is_empty() {
        println!("✅ node `{}` validates cleanly", label);
        return;
    }
    let criticals: Vec<_> = violations
        .iter()
        .filter(|v| v.severity == da_domain::validator::Severity::Critical)
        .collect();
    println!(
        "❌ {} violation(s) ({} critical)",
        violations.len(),
        criticals.len()
    );
    print!("{}", da_domain::validator::format_violations(&violations));
    if !criticals.is_empty() {
        std::process::exit(1);
    }
}

/// Audit pipeline-set fields against schema-declared fields.
///
/// Walks `da-application/src/*.rs` for `create_node("Label")` blocks,
/// extracts every `set_node_property_*(node, "field", ...)` call within
/// 80 lines after each create_node, and compares against fields declared
/// in the corresponding `NodeSchemaDef::required_fields()` +
/// `optional_fields()`. Reports drift.
///
/// Exits with code 1 if any undeclared fields are found.
/// This is the CLI counterpart of the static Python audit documented in
/// MEM508. Run after any pipeline change to catch drift before CI.
fn audit_fields() {
    use std::collections::{BTreeMap, HashSet};
    use std::fs;
    use std::path::PathBuf;

    println!("daily-archive v2 — audit-fields (ADR-045 / MEM508)");
    println!();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // M434: scan BOTH product surfaces that create nodes and set
    // properties — da-application/src and da-cli/src (load_journey stamps
    // embedding_sha256 on Paper; the old single-dir scan was blind across
    // the crate boundary). da-adapters/da-mcp verified free of non-test
    // create_node writers.
    let src_dirs: Vec<PathBuf> = ["../da-application/src", "src"]
        .iter()
        .map(|rel| manifest_dir.join(rel))
        .filter_map(|p| p.canonicalize().ok())
        .collect();
    if src_dirs.is_empty() {
        eprintln!("Could not locate product src directories");
        std::process::exit(2);
    }

    // Build a map: label -> set of fields set via set_node_property_*.
    let mut pipeline_fields: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut scanned_files = 0usize;
    for src_dir in &src_dirs {
        if let Ok(entries) = fs::read_dir(src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                    continue;
                }
                scanned_files += 1;
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let lines: Vec<&str> = content.lines().collect();
                // Find create_node call sites
                let mut create_sites: Vec<(usize, String)> = Vec::new();
                for (i, line) in lines.iter().enumerate() {
                    if let Some(start) = line.find("create_node(\"") {
                        let after = &line[start + "create_node(\"".len()..];
                        if let Some(end) = after.find("\")") {
                            let label = &after[..end];
                            if !label.is_empty()
                                && label.chars().all(|c| c.is_alphanumeric() || c == '_')
                            {
                                create_sites.push((i, label.to_string()));
                            }
                        }
                    }
                }
                // For each create_node, scan within the current function only.
                // A fixed line window can cross helper boundaries and attribute
                // unrelated fields to the previous label (false schema drift).
                for (idx, (line_num, label)) in create_sites.iter().enumerate() {
                    let next_create = create_sites
                        .get(idx + 1)
                        .map(|(n, _)| *n)
                        .unwrap_or(lines.len());
                    let next_function = lines
                        .iter()
                        .enumerate()
                        .skip(line_num + 1)
                        .find_map(|(line_index, line)| {
                            let trimmed = line.trim_start();
                            let is_function = trimmed.starts_with("fn ")
                                || trimmed.starts_with("async fn ")
                                || trimmed.starts_with("pub fn ")
                                || trimmed.starts_with("pub async fn ");
                            is_function.then_some(line_index)
                        })
                        .unwrap_or(lines.len());
                    let block_end = next_create.min(next_function);
                    // M434: doc comments are not code — the scanner was
                    // matching its own docstring examples as drift.
                    let block: String = lines[*line_num..block_end]
                        .iter()
                        .filter(|l| {
                            let t = l.trim_start();
                            !(t.starts_with("///") || t.starts_with("//!"))
                        })
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n");
                    let fields = pipeline_fields.entry(label.clone()).or_default();
                    // M433: block-level extraction via the tested lib helper —
                    // the old line-by-line scan missed every multi-line call
                    // (cargo fmt splits them routinely; M432 found 2 of 4
                    // drifted fields).
                    fields.extend(da_cli::audit_fields::extract_set_fields(&block));
                }
            }
        }
    }

    // Build schema declared fields map
    let mut declared: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for schema in da_domain::schema::all_node_schemas() {
        let label = schema.label().to_string();
        let mut fields = HashSet::new();
        for (n, _) in schema.required_fields() {
            fields.insert(n.to_string());
        }
        for (n, _) in schema.optional_fields() {
            fields.insert(n.to_string());
        }
        declared.insert(label, fields);
    }

    println!("Scanned {scanned_files} .rs files across product src (da-application + da-cli)");
    println!(
        "Found {} distinct labels with set_node_property_* calls",
        pipeline_fields.len()
    );
    println!("Schema registry declares {} node types", declared.len());
    println!();

    let mut total_undeclared = 0usize;
    let mut labels_with_drift = 0usize;
    for (label, fields) in &pipeline_fields {
        let schema_fields = declared.get(label).cloned().unwrap_or_default();
        let undeclared: Vec<&String> = fields
            .iter()
            .filter(|f| !schema_fields.contains(*f))
            .collect();
        if !undeclared.is_empty() {
            labels_with_drift += 1;
            total_undeclared += undeclared.len();
            println!(
                "  {label}: {} undeclared field(s): {}",
                undeclared.len(),
                undeclared
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    println!();
    if total_undeclared == 0 {
        println!("✅ OK — every pipeline-set field is declared in its schema");
    } else {
        println!(
            "❌ {} undeclared field(s) across {} label(s)",
            total_undeclared, labels_with_drift
        );
        println!();
        println!("Fix: add each missing field to the corresponding schema's");
        println!("required_fields() or optional_fields() in crates/da-domain/src/.");
        std::process::exit(1);
    }
}

/// Export meta-ontology to YAML (ADR-050 Phase D / M325).
///
/// Writes:
/// - `<output_dir>/node_schemas.yaml` from compiled `all_node_schemas()`
/// - `<output_dir>/edge_contracts.yaml` from `render_edge_contracts_yaml()`
///   (YAML-first contracts via `edge_contracts_or_yaml`)
fn export_ontology_yaml(output_dir: &str) {
    use serde::Serialize;

    #[derive(Serialize)]
    struct FieldDef {
        name: String,
        field_type: String,
    }

    #[derive(Serialize)]
    struct YamlNodeSchema {
        label: String,
        required_fields: Vec<FieldDef>,
        optional_fields: Vec<FieldDef>,
    }

    #[derive(Serialize)]
    struct OntologyExportRoot {
        temporal_wire_contract: kg_ontology::registry::TemporalWireContract,
        node_schemas: Vec<YamlNodeSchema>,
    }

    let schemas: Vec<YamlNodeSchema> = da_domain::schema::all_node_schemas()
        .into_iter()
        .map(|s| YamlNodeSchema {
            label: s.label().to_string(),
            required_fields: s
                .required_fields()
                .into_iter()
                .map(|(name, fty)| FieldDef {
                    name: name.to_string(),
                    field_type: format!("{:?}", fty),
                })
                .collect(),
            optional_fields: s
                .optional_fields()
                .into_iter()
                .map(|(name, fty)| FieldDef {
                    name: name.to_string(),
                    field_type: format!("{:?}", fty),
                })
                .collect(),
        })
        .collect();

    let schema_count = schemas.len();
    // PIR-008 (M398/S04): the canonical temporal wire contract is part of
    // the ontology DATA. Re-emit it on every export so a regenerated
    // node_schemas.yaml always carries the RFC3339-UTC / OPEN / integer-
    // rejected decision.
    let root = OntologyExportRoot {
        temporal_wire_contract: kg_ontology::registry::TemporalWireContract::default(),
        node_schemas: schemas,
    };

    let yaml = serde_yaml::to_string(&root).expect("YAML serialize failed");
    let output_path = std::path::Path::new(output_dir);
    std::fs::create_dir_all(output_path).expect("create_dir_all failed");
    let yaml_path = output_path.join("node_schemas.yaml");
    std::fs::write(&yaml_path, &yaml).expect("write failed");

    eprintln!(
        "ontology export: {schema_count} schemas → {}",
        yaml_path.display()
    );

    // M325: edge contracts are part of external meta-ontology, not nodes-only export.
    let edges_yaml = da_domain::render_edge_contracts_yaml().expect("edge contracts YAML");
    let edges_path = output_path.join("edge_contracts.yaml");
    std::fs::write(&edges_path, edges_yaml.as_bytes()).expect("write edge_contracts failed");
    let edge_n = edges_yaml.matches("edge_constant:").count();
    eprintln!(
        "ontology export: {edge_n} edge contracts → {}",
        edges_path.display()
    );
}

/// Validate the live graph against the schema registry.
///
/// Reads all nodes (or a single label) from SamyamaGraphStore via
/// `get_nodes_by_label` + `get_node_property_string/int`, builds a
/// PropertySnapshot for each, and runs the validator. Reports total
/// violations and shows the first 10.
///
/// This is the production-side counterpart of `assert_graph_conforms`
/// in integration tests. Wave D production-side enforcement.
async fn validate_graph(label_filter: Option<&str>, snapshot: Option<&std::path::Path>) {
    use da_domain::validator::{PropertySnapshot, format_violations, validate_node_properties};
    use da_ports::graph_store::DirectGraphStore;
    use serde_json::json;

    println!("daily-archive v2 — validate-graph (ADR-045 Wave D production)");
    println!();

    let store = open_graph_store_with_optional_snapshot(snapshot).await;
    let labels: Vec<String> = match label_filter {
        Some(l) => vec![l.to_string()],
        None => da_domain::schema::all_node_schemas()
            .into_iter()
            .map(|s| s.label().to_string())
            .collect(),
    };

    let mut total_nodes = 0usize;
    let mut total_violations = 0usize;
    let mut critical_count = 0usize;
    let mut warning_count = 0usize;
    let mut first_violations: Vec<String> = Vec::new();

    for label in &labels {
        let node_ids = store.get_nodes_by_label(label).await;
        if node_ids.is_empty() {
            continue;
        }
        for node_id in node_ids {
            total_nodes += 1;
            let mut snap = PropertySnapshot::new();
            // Walk the known schema fields and read each one.
            if let Some(schema) = da_domain::schema::schema_for_label(label) {
                for (fname, fty) in schema
                    .required_fields()
                    .iter()
                    .chain(schema.optional_fields().iter())
                {
                    match fty {
                        da_domain::schema::FieldType::Boolean => {
                            if let Some(b) = store.get_node_property_bool(node_id, fname).await {
                                snap.insert(fname.to_string(), json!(b));
                            }
                        }
                        da_domain::schema::FieldType::Integer
                        | da_domain::schema::FieldType::DateTime => {
                            if let Some(i) = store.get_node_property_int(node_id, fname).await {
                                snap.insert(fname.to_string(), json!(i));
                            }
                        }
                        da_domain::schema::FieldType::Float => {
                            if let Some(f) = store.get_node_property_float(node_id, fname).await {
                                snap.insert(fname.to_string(), json!(f));
                            }
                        }
                        _ => {
                            // String / Vector / Any — read as string.
                            if let Some(s) = store.get_node_property_string(node_id, fname).await {
                                snap.insert(fname.to_string(), json!(s));
                            }
                        }
                    }
                }
            }
            let violations = validate_node_properties(label, &snap);
            if !violations.is_empty() {
                total_violations += violations.len();
                for v in &violations {
                    if v.severity == da_domain::validator::Severity::Critical {
                        critical_count += 1;
                    } else {
                        warning_count += 1;
                    }
                }
                if first_violations.len() < 10 {
                    let formatted = format_violations(&violations);
                    first_violations.push(format!(
                        "[{}] node_id={node_id}\n{}",
                        label,
                        formatted.trim_end()
                    ));
                }
            }
        }
    }

    println!(
        "Validated {total_nodes} node(s) across {} label(s)",
        labels.len()
    );
    println!(
        "Total violations: {total_violations} ({critical_count} critical, {warning_count} warning)"
    );
    println!();
    if first_violations.is_empty() {
        println!("✅ OK — graph conforms to schema registry");
    } else {
        println!("First violations (up to 10):");
        for v in &first_violations {
            println!("{v}");
            println!("---");
        }
        if critical_count > 0 {
            std::process::exit(1);
        }
    }
}
