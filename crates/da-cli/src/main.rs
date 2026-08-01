use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

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
    },

    /// Batch ingest multiple PDFs in one process (HOT path + snapshot export)
    BatchIngest {
        /// Comma-separated arxiv IDs to ingest
        #[arg(long)]
        ids: String,

        /// Output snapshot path (.sgsnap)
        #[arg(long)]
        output: Option<String>,
    },

    /// Load a snapshot file into the graph (restore durability)
    LoadSnapshot {
        /// Path to .sgsnap file
        #[arg(long)]
        input: String,
    },

    /// Show graph statistics (node/edge counts)
    GraphStats,

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
    },

    /// Extract entities from an ingested paper (Phase 3 rule-based)
    Extract {
        /// Paper arxiv_id to extract from
        #[arg(long)]
        id: String,
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
    },

    /// Enrich paper metadata from OpenAlex (topics, authors, concepts)
    Enrich {
        /// arXiv ID to enrich
        #[arg(long)]
        id: String,
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
}

fn main() {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Single shared Tokio runtime for all async commands (Rust 2026 best practice:
    // avoid repeated Runtime creation; one runtime serves all commands).
    let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Failed to create Tokio runtime: {e}");
        std::process::exit(1);
    });

    match cli.command {
        Commands::Health => {
            println!("daily-archive v2 — health check");
            rt.block_on(async {
                check_health().await;
            });
        }
        Commands::Version => {
            println!("daily-archive v2.0.0 (Rust)");
            println!("ADR-037/038/039/040/041 — Samyama Graph + RuVector + RVF");
            println!("Phase: ingest + extraction + healing + schema + enrich");
        }
        Commands::Ingest { pdf, id } => {
            println!("Ingesting: {} → {}", pdf, id);
            rt.block_on(async {
                ingest_pdf(&pdf, &id).await;
            });
        }
        Commands::BatchIngest { ids, output } => {
            rt.block_on(async {
                batch_ingest(&ids, output.as_deref()).await;
            });
        }
        Commands::LoadSnapshot { input } => {
            rt.block_on(async {
                load_snapshot(&input).await;
            });
        }
        Commands::GraphStats => {
            rt.block_on(async {
                graph_stats().await;
            });
        }
        Commands::Query { kind, id, hops } => {
            rt.block_on(async {
                query_graph(&kind, id.as_deref(), hops).await;
            });
        }
        Commands::Extract { id } => {
            rt.block_on(async {
                extract_entities(&id).await;
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
        } => {
            rt.block_on(async {
                heal_graph(
                    &op,
                    &vid,
                    &label,
                    key.as_deref(),
                    value.as_deref(),
                    keep.as_deref(),
                    &reason,
                )
                .await;
            });
        }
        Commands::Enrich { id } => {
            rt.block_on(async {
                enrich_from_openalex(&id).await;
            });
        }
        Commands::BatchEnrich { ids } => {
            rt.block_on(async {
                batch_enrich_from_openalex(&ids).await;
            });
        }
        Commands::SchedulerRun { queue_dir } => {
            rt.block_on(async {
                run_scheduler(&queue_dir).await;
            });
        }
        Commands::SchemaCheck => {
            schema_check();
        }
    }
}

async fn check_health() {
    use da_adapters::{FdApiEmbedder, GrobidParser, SamyamaGraphStore};
    use da_ports::embedder::Embedder;
    use da_ports::graph_store::GraphStore;

    // Samyama (embedded)
    let graph = SamyamaGraphStore::from_env();
    let graph_ok = GraphStore::health(&graph).await.unwrap_or(false);
    let nodes = graph.node_count().await;
    println!(
        "  Samyama Graph:  {} ({} nodes)",
        if graph_ok { "✅ healthy" } else { "❌ down" },
        nodes
    );

    // GROBID
    let grobid = GrobidParser::from_env();
    let grobid_ok = grobid.is_alive().await;
    println!(
        "  GROBID:         {}",
        if grobid_ok { "✅ alive" } else { "❌ down" }
    );

    // Embedder
    let embedder = FdApiEmbedder::from_env();
    println!(
        "  Embedder:       {} (dim: {})",
        Embedder::model_id(&embedder),
        Embedder::dimensions(&embedder)
    );
}

async fn ingest_pdf(pdf_path: &str, paper_id: &str) {
    use da_adapters::{FdApiEmbedder, GrobidParser, SamyamaGraphStore};
    use da_application::IngestUseCase;

    let parser = Box::new(GrobidParser::from_env());
    let embedder = Box::new(FdApiEmbedder::from_env());
    let graph_store = Box::new(SamyamaGraphStore::from_env());
    // DirectGraphStore is implemented by SamyamaGraphStore (ADR-041 HOT path)

    let use_case = IngestUseCase::new(parser, embedder, graph_store);

    match use_case.ingest_pdf(pdf_path, paper_id).await {
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
        }
        Err(e) => {
            eprintln!("❌ Ingest failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn batch_ingest(ids_str: &str, output: Option<&str>) {
    use da_adapters::{FdApiEmbedder, GrobidParser, SamyamaGraphStore};
    use da_application::{IngestUseCase, batch_ingest_pdfs};

    let ids: Vec<&str> = ids_str.split(',').map(|s| s.trim()).collect();
    let pdfs: Vec<(String, String)> = ids
        .iter()
        .filter_map(|pid| {
            let pdf = std::process::Command::new("find")
                .args(["data/article_catalog", "-name", &format!("{}.pdf", pid)])
                .output()
                .ok()?
                .stdout;
            let pdf_path = String::from_utf8(pdf).ok()?.trim().to_string();
            if pdf_path.is_empty() {
                return None;
            }
            Some((pdf_path, pid.to_string()))
        })
        .collect();

    if pdfs.is_empty() {
        eprintln!("No PDFs found for IDs: {}", ids_str);
        std::process::exit(1);
    }

    println!("Batch ingesting {} papers (HOT path)...", pdfs.len());

    let parser = Box::new(GrobidParser::from_env());
    let embedder = Box::new(FdApiEmbedder::from_env());
    let graph_store = Box::new(SamyamaGraphStore::new());

    let ingest = IngestUseCase::new(parser, embedder, graph_store);

    let snapshot_path = output.map(PathBuf::from);

    let result = batch_ingest_pdfs(&ingest, &pdfs, snapshot_path.as_deref()).await;

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
            println!("   Nodes in graph: {}", ingest.graph_stats().await.0);
            if let Some(ref path) = r.snapshot_path {
                println!("   Snapshot: {}", path);
            }
            for (pid, err) in &r.errors {
                println!("   FAIL {}: {}", pid, err);
            }
        }
        Err(e) => {
            eprintln!("❌ Batch failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn load_snapshot(input: &str) {
    use da_adapters::SamyamaGraphStore;
    use da_ports::graph_store::GraphStore;

    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("❌ Cannot read snapshot {input}: {e}");
            std::process::exit(1);
        }
    };

    println!("Loading snapshot: {} ({} bytes)...", input, data.len());
    let graph = SamyamaGraphStore::from_env();
    match graph.import_snapshot(&data).await {
        Ok(()) => {
            let nodes = graph.node_count().await;
            println!("✅ Snapshot loaded — {} nodes now in graph", nodes);
        }
        Err(e) => {
            eprintln!("❌ Snapshot load failed: {e}");
            std::process::exit(1);
        }
    }
}

async fn graph_stats() {
    use da_adapters::SamyamaGraphStore;

    let graph = SamyamaGraphStore::from_env();
    let nodes = graph.node_count().await;
    let edges = graph.edge_count().await;
    println!("daily-archive v2 — graph statistics");
    println!("  Nodes: {}", nodes);
    println!("  Edges: {}", edges);
    if nodes == 0 {
        println!("  ⚠  Graph is empty (in-memory store reset). Use `da load-snapshot` to restore.");
    }
}

async fn query_graph(kind: &str, id: Option<&str>, hops: usize) {
    use da_graph::{EntityQueries, PaperQueries};

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

    // Execute via SamyamaGraphStore (WARM path — embedded Cypher)
    use da_adapters::SamyamaGraphStore;
    use da_ports::graph_store::GraphStore;
    let store = SamyamaGraphStore::from_env();
    match store.query_readonly("daily_archive", &cypher).await {
        Ok(result) => {
            println!("Columns: {:?}", result.columns);
            println!("Records: {}", result.records.len());
            for (i, row) in result.records.iter().take(10).enumerate() {
                println!("  [{i}] {row:?}");
            }
            if result.records.len() > 10 {
                println!("  ... ({} more)", result.records.len() - 10);
            }
        }
        Err(e) => {
            eprintln!("❌ Query failed: {e}");
            std::process::exit(1);
        }
    }
}

async fn extract_entities(paper_id: &str) {
    use da_adapters::{GrobidParser, RuleBasedExtractor, SamyamaGraphStore};
    use da_application::ExtractionUseCase;
    use da_ports::parser::ParserPort;

    // Find the PDF
    let pdf = std::process::Command::new("find")
        .args([
            "data/article_catalog",
            "-name",
            &format!("{}.pdf", paper_id),
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let pdf_path = match pdf {
        Some(p) => p,
        None => {
            eprintln!("❌ PDF not found for {paper_id}");
            std::process::exit(1);
        }
    };

    println!("Extracting entities from {paper_id}...");

    // Parse via GROBID
    let parser = GrobidParser::from_env();
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
    let graph_store = Box::new(SamyamaGraphStore::from_env());
    let use_case = ExtractionUseCase::new(extractor, graph_store);

    match use_case.extract_from_parsed(&parsed).await {
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
        da_graph::schema::CURRENT_SCHEMA_VERSION
    );

    // Property indexes: (label, property)
    let property_indexes = [
        ("Paper", "vid"),
        ("Paper", "arxiv_id"),
        ("Paper", "primary_category"),
        ("Citation", "vid"),
        ("Citation", "arxiv_id"),
        ("Entity", "vid"),
        ("Entity", "entity_type"),
        ("Section", "vid"),
        ("Section", "paper_id"),
        ("Keyword", "vid"),
        ("Keyword", "keyword"),
        ("Topic", "vid"),
        ("Topic", "label"),
        ("Category", "vid"),
        ("Category", "code"),
    ];
    // Vector indexes: (label, property, dimensions)
    let vector_indexes = [("Paper", "embedding", dimensions)];

    println!(
        "  Indexes to create: {} property + {} vector",
        property_indexes.len(),
        vector_indexes.len()
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use da_adapters::SamyamaGraphStore;
        use da_ports::graph_store::{GraphStore, VectorMetric};
        let store = SamyamaGraphStore::from_env();
        let mut ok = 0;
        let mut fail = 0;

        for (label, property) in &property_indexes {
            match store.create_property_index(label, property).await {
                Ok(()) => {
                    println!("    [property] {label}.{property}");
                    ok += 1;
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("  ❌ Property index {label}.{property} failed: {e}");
                }
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

async fn heal_graph(
    op: &str,
    vid: &str,
    label: &str,
    key: Option<&str>,
    value: Option<&str>,
    keep: Option<&str>,
    reason: &str,
) {
    use da_adapters::SamyamaGraphStore;
    use da_application::GraphHealingUseCase;
    use da_domain::healing::HealingActor;

    let graph_store = Box::new(SamyamaGraphStore::from_env());
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
}

async fn enrich_from_openalex(arxiv_id: &str) {
    use da_adapters::{OpenAlexHttpAdapter, SamyamaGraphStore};
    use da_application::EnrichUseCase;

    println!("Enriching {arxiv_id} from OpenAlex...");

    let openalex = Box::new(OpenAlexHttpAdapter::new());
    let graph_store = Box::new(SamyamaGraphStore::from_env());
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
            }
        }
        Err(e) => {
            eprintln!("❌ Enrich failed: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn batch_enrich_from_openalex(ids_str: &str) {
    use da_adapters::{OpenAlexHttpAdapter, SamyamaGraphStore};
    use da_application::EnrichUseCase;

    let ids: Vec<&str> = ids_str.split(',').map(|s| s.trim()).collect();
    println!("Batch enriching {} papers from OpenAlex...", ids.len());

    let openalex = Box::new(OpenAlexHttpAdapter::new());
    let graph_store = Box::new(SamyamaGraphStore::from_env());
    let use_case = EnrichUseCase::new(openalex, graph_store);

    let mut ok = 0;
    let mut pending = 0;
    let mut fail = 0;
    let mut total_topics = 0;
    let mut total_authors = 0;

    for arxiv_id in &ids {
        match use_case.enrich_by_arxiv_id(arxiv_id).await {
            Ok(result) => {
                if result.openalex_pending {
                    println!("  ⏳ {}: pending (not in OpenAlex yet)", arxiv_id);
                    pending += 1;
                } else {
                    println!(
                        "  ✅ {}: {} topics, {} authors",
                        arxiv_id, result.topics_written, result.authors_written
                    );
                    total_topics += result.topics_written;
                    total_authors += result.authors_written;
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
        "\nBatch enrich complete: {}/{} enriched, {} pending, {} fail, {} topics, {} authors",
        ok,
        ids.len(),
        pending,
        fail,
        total_topics,
        total_authors
    );
}

async fn run_scheduler(_queue_dir: &str) {
    // D135: scheduler state must survive restart.
    // Load snapshot → process due tasks → save snapshot.
    use da_adapters::{OpenAlexHttpAdapter, SamyamaGraphStore};
    use da_application::{EnrichUseCase, GraphScheduler};
    use da_ports::graph_store::GraphStore;
    use std::path::PathBuf;

    let snapshot_path = PathBuf::from("data/samyama/scheduler-state.sgsnap");

    // Single shared store for both scheduler and enrich
    let shared_store = SamyamaGraphStore::from_env();

    // Restore state from snapshot
    if snapshot_path.exists() {
        match std::fs::read(&snapshot_path) {
            Ok(data) => {
                if let Err(e) = GraphStore::import_snapshot(&shared_store, &data).await {
                    eprintln!("Warning: snapshot restore failed: {e} — starting fresh");
                } else {
                    println!("Scheduler: restored state from {}", snapshot_path.display());
                }
            }
            Err(e) => eprintln!("Warning: cannot read snapshot: {e}"),
        }
    }

    // Load due tasks from the restored graph via GraphScheduler associate fn
    let due_tasks = GraphScheduler::load_due_tasks_from(&shared_store).await;

    if due_tasks.is_empty() {
        println!("Scheduler: no due tasks in graph");
        return;
    }

    println!("Scheduler: {} tasks due now", due_tasks.len());

    if due_tasks.is_empty() {
        println!("Scheduler: no due tasks");
    } else {
        println!("Scheduler: {} tasks due now", due_tasks.len());
    }

    // Process due tasks using shared_store for both enrich and scheduler updates
    let now = chrono::Utc::now().timestamp();
    let policy = da_domain::scheduler::RetryPolicy::default();
    let mut completed = 0;
    let mut still_pending = 0;
    let failed = 0; // tracked inside GraphScheduler::record_retry_on
    let mut details = Vec::new();

    for (node_id, arxiv_id) in &due_tasks {
        // Enrich using shared store
        let openalex = Box::new(OpenAlexHttpAdapter::new());
        // Clone shared_store for enrich (Samyama in-memory is per-instance)
        // In Phase 3+ (server mode), both will share the same persistent store
        let enrich_store = Box::new(SamyamaGraphStore::from_env());
        let use_case = EnrichUseCase::new(openalex, enrich_store);

        match use_case.enrich_by_arxiv_id(arxiv_id).await {
            Ok(r) => {
                if r.openalex_pending {
                    GraphScheduler::record_retry_on(&shared_store, &policy, *node_id, now)
                        .await
                        .ok();
                    still_pending += 1;
                    details.push((
                        arxiv_id.clone(),
                        "pending".to_string(),
                        "not in OpenAlex".to_string(),
                    ));
                } else {
                    GraphScheduler::complete_task_on(&shared_store, *node_id, now)
                        .await
                        .ok();
                    completed += 1;
                    details.push((
                        arxiv_id.clone(),
                        "completed".to_string(),
                        format!("{} topics, {} authors", r.topics_written, r.authors_written),
                    ));
                }
            }
            Err(e) => {
                still_pending += 1;
                details.push((arxiv_id.clone(), "error".to_string(), format!("{e:#}")));
            }
        }
    }

    // Save state to snapshot for restart recovery
    if let Some(parent) = snapshot_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match GraphStore::export_snapshot(&shared_store).await {
        Ok(data) => {
            std::fs::write(&snapshot_path, &data).ok();
            println!("Scheduler: state saved to {}", snapshot_path.display());
        }
        Err(e) => eprintln!("Warning: snapshot save failed: {e}"),
    }

    println!("\nScheduler run complete:");
    println!("  Completed: {completed}");
    println!("  Still pending: {still_pending}");
    println!("  Failed (max retries): {failed}");
    for (id, status, msg) in &details {
        println!("    {id} [{status}]: {msg}");
    }
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

    let mut unregistered: Vec<&String> =
        used_labels.iter().filter(|l| !registered.contains(*l)).collect();
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
