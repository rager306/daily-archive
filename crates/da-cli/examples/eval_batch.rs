//! Batch gold-standard evaluation across all fixtures.
//!
//! Runs REAL rule-based extraction on every paper with a gold-standard fixture,
//! aggregates precision/recall/F1 across the corpus.
//!
//! Usage: cargo run -p da-cli --example eval_batch

use da_adapters::{HtmlParser, RuleBasedExtractor};
use da_cli::gold_corpus::{CanonicalSourceKind, build_gold_corpus_manifest};
use da_domain::eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
use da_parse::GrobidParser;
use da_ports::extractor::Extractor;
use da_ports::parser::ParserPort;

#[derive(serde::Deserialize)]
struct GoldFile {
    paper_id: String,
    entities: Vec<GoldEntity>,
}

struct PaperResult {
    paper_id: String,
    precision: f64,
    recall: f64,
    f1: f64,
    false_negatives: Vec<String>,
}

#[tokio::main]
async fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("eval_batch");
    let manifest = build_gold_corpus_manifest(
        std::path::Path::new("data/gold_standard"),
        std::path::Path::new("data/sources/arxiv"),
        10,
    )
    .unwrap_or_else(|error| {
        eprintln!("Cannot build gold corpus manifest: {error}");
        std::process::exit(1);
    });

    println!("=== Batch Extraction Evaluation ===");
    println!(
        "Fixtures: {} | source-ready: {} | unavailable: {} | batches: {}\n",
        manifest.fixture_count,
        manifest.source_ready_count,
        manifest.unavailable_count,
        manifest.batch_count
    );

    let mut results: Vec<PaperResult> = Vec::new();
    let mut total_gold = 0usize;
    let mut total_pred = 0usize;
    let mut total_tp = 0usize;
    let mut total_fp = 0usize;
    let mut total_fn = 0usize;

    for record in &manifest.records {
        let gold_json = std::fs::read_to_string(&record.fixture_path).unwrap();
        let gold: GoldFile = serde_json::from_str(&gold_json).unwrap();
        let paper_id = gold.paper_id.clone();
        let Some(source_path) = record.source.path.as_deref() else {
            eprintln!(
                "⚠ {paper_id}: {}, skipping",
                record
                    .source
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("canonical source unavailable")
            );
            continue;
        };

        let parsed = match record.source.kind {
            CanonicalSourceKind::Pdf => {
                let parser = GrobidParser::from_env();
                match parser.parse_pdf(source_path, &paper_id).await {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        eprintln!("⚠ {paper_id}: GROBID failed: {error}");
                        continue;
                    }
                }
            }
            CanonicalSourceKind::Html => {
                let parser = HtmlParser::new();
                match parser.parse_html(source_path, &paper_id).await {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        eprintln!("⚠ {paper_id}: HTML parse failed: {error}");
                        continue;
                    }
                }
            }
            CanonicalSourceKind::Unavailable => unreachable!("unavailable source has no path"),
        };

        // Extract
        let extractor = RuleBasedExtractor::new();
        let sections: Vec<(String, String)> = parsed
            .sections
            .iter()
            .map(|s| (s.title.clone(), s.text.clone()))
            .collect();
        let extracted = match extractor.extract(&sections).await {
            Ok(e) => e,
            Err(e) => {
                eprintln!("⚠ {paper_id}: extraction failed: {e}");
                continue;
            }
        };

        let predicted: Vec<PredictedEntity> = extracted
            .iter()
            .map(|e| PredictedEntity {
                label: e.label.clone(),
                entity_type: format!("{:?}", e.entity_type),
            })
            .collect();

        let metrics = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);

        // Collect FN labels (type-checked, consistent with evaluate_fuzzy)
        let false_negatives: Vec<String> = gold
            .entities
            .iter()
            .filter(|g| {
                let gl = g.label.to_lowercase();
                let gt = g.entity_type.to_lowercase();
                !predicted.iter().any(|p| {
                    let pl = p.label.to_lowercase();
                    gt == p.entity_type.to_lowercase() && (gl.contains(&pl) || pl.contains(&gl))
                })
            })
            .map(|g| format!("{} ({})", g.label, g.entity_type))
            .collect();

        total_gold += gold.entities.len();
        total_pred += predicted.len();
        total_tp += metrics.true_positives;
        total_fp += metrics.false_positives;
        total_fn += metrics.false_negatives;

        results.push(PaperResult {
            paper_id: paper_id.clone(),
            precision: metrics.precision,
            recall: metrics.recall,
            f1: metrics.f1,
            false_negatives,
        });

        println!(
            "{paper_id}: P={:.3} R={:.3} F1={:.3} (gold={}, pred={})",
            metrics.precision,
            metrics.recall,
            metrics.f1,
            gold.entities.len(),
            predicted.len()
        );
    }

    // Aggregate metrics (micro-average)
    let micro_p = if total_pred > 0 {
        total_tp as f64 / total_pred as f64
    } else {
        0.0
    };
    let micro_r = if total_gold > 0 {
        total_tp as f64 / total_gold as f64
    } else {
        0.0
    };
    let micro_f1 = if micro_p + micro_r > 0.0 {
        2.0 * micro_p * micro_r / (micro_p + micro_r)
    } else {
        0.0
    };

    // Macro-average
    let macro_p = results.iter().map(|r| r.precision).sum::<f64>() / results.len() as f64;
    let macro_r = results.iter().map(|r| r.recall).sum::<f64>() / results.len() as f64;
    let macro_f1 = results.iter().map(|r| r.f1).sum::<f64>() / results.len() as f64;

    println!("\n=== Corpus Summary ===");
    println!("Papers evaluated: {}", results.len());
    println!("Total gold entities: {total_gold}");
    println!("Total predicted: {total_pred}");
    println!("Total TP: {total_tp} | FP: {total_fp} | FN: {total_fn}");
    println!("\nMicro-averaged (aggregate TP/FP/FN):");
    println!("  P={:.3} R={:.3} F1={:.3}", micro_p, micro_r, micro_f1);
    println!("\nMacro-averaged (mean per-paper):");
    println!("  P={:.3} R={:.3} F1={:.3}", macro_p, macro_r, macro_f1);

    // Show all FN
    println!("\n=== All False Negatives ===");
    for r in &results {
        if !r.false_negatives.is_empty() {
            println!("  {}:", r.paper_id);
            for fn_label in &r.false_negatives {
                println!("    ❌ {fn_label}");
            }
        }
    }
}
