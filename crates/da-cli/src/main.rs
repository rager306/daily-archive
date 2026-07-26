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
}

fn main() {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    match cli.command {
        Commands::Health => {
            println!("daily-archive v2 — health check");
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                check_health().await;
            });
        }
        Commands::Version => {
            println!("daily-archive v2.0.0 (Rust)");
            println!("ADR-037/038/039/040/041 — Samyama Graph + RuVector + RVF");
            println!("Phase: scaffolding + ingest pipeline + snapshot durability");
        }
        Commands::Ingest { pdf, id } => {
            println!("Ingesting: {} → {}", pdf, id);
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                ingest_pdf(&pdf, &id).await;
            });
        }
        Commands::BatchIngest { ids, output } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                batch_ingest(&ids, output.as_deref()).await;
            });
        }
        Commands::LoadSnapshot { input } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                load_snapshot(&input).await;
            });
        }
        Commands::GraphStats => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                graph_stats().await;
            });
        }
        Commands::Query { kind, id, hops } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                query_graph(&kind, id.as_deref(), hops).await;
            });
        }
        Commands::Extract { id } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
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
            let rt = tokio::runtime::Runtime::new().unwrap();
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
    use da_application::{batch_ingest_pdfs, IngestUseCase};

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
