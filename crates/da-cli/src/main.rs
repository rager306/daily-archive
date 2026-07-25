use clap::{Parser, Subcommand};

/// daily-archive v2 — scientific knowledge engine
#[derive(Parser)]
#[command(name = "da", version, about = "Scientific knowledge engine (Rust)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check infrastructure health
    Health,
    /// Show version info
    Version,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Health => {
            println!("daily-archive v2 — health check (not yet implemented)");
            println!("Phase 1: scaffolding");
        }
        Commands::Version => {
            println!("daily-archive v2.0.0 (Rust)");
            println!("ADR-037/038/039/040 — Samyama Graph + RuVector + RVF");
        }
    }
}
