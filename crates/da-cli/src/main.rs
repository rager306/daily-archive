use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// daily-archive v2 — scientific knowledge engine
#[derive(Parser)]
#[command(name = "da", version, about = "Scientific knowledge engine (Rust + Samyama Graph)")]
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
            println!("ADR-037/038/039/040 — Samyama Graph + RuVector + RVF");
            println!("Phase: scaffolding + ingest pipeline");
        }
        Commands::Ingest { pdf, id } => {
            println!("Ingesting: {} → {}", pdf, id);
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                ingest_pdf(&pdf, &id).await;
            });
        }
    }
}

async fn check_health() {
    use da_adapters::{SamyamaGraphStore, GrobidParser, FdApiEmbedder};
    use da_ports::embedder::Embedder;
    use da_ports::graph_store::GraphStore;

    // Samyama (embedded)
    let graph = SamyamaGraphStore::from_env();
    let graph_ok = GraphStore::health(&graph).await.unwrap_or(false);
    let nodes = graph.node_count().await;
    println!("  Samyama Graph:  {} ({} nodes)", if graph_ok { "✅ healthy" } else { "❌ down" }, nodes);

    // GROBID
    let grobid = GrobidParser::from_env();
    let grobid_ok = grobid.is_alive().await;
    println!("  GROBID:         {}", if grobid_ok { "✅ alive" } else { "❌ down" });

    // Embedder
    let embedder = FdApiEmbedder::from_env();
    println!("  Embedder:       {} (dim: {})", Embedder::model_id(&embedder), Embedder::dimensions(&embedder));
}

async fn ingest_pdf(pdf_path: &str, paper_id: &str) {
    use da_adapters::{SamyamaGraphStore, GrobidParser, FdApiEmbedder};
    use da_ports::embedder::Embedder;
    use da_ports::graph_store::GraphStore;
    use da_application::IngestUseCase;

    let parser = Box::new(GrobidParser::from_env());
    let embedder = Box::new(FdApiEmbedder::from_env());
    let graph_store = Box::new(SamyamaGraphStore::from_env());

    let use_case = IngestUseCase::new(parser, embedder, graph_store);

    match use_case.ingest_pdf(pdf_path, paper_id).await {
        Ok(result) => {
            println!("✅ Ingested: {}", result.paper_id);
            println!("   Title:    {}", result.title);
            println!("   Body:     {} chars", result.body_chars);
            println!("   Vector:   {}d", result.vector_dimensions);
            println!("   Graph:    {}", if result.graph_written { "written" } else { "failed" });
            println!("   Import:   {} (D127)", if result.import_eligible { "eligible" } else { "locked" });
        }
        Err(e) => {
            eprintln!("❌ Ingest failed: {e:#}");
            std::process::exit(1);
        }
    }
}
