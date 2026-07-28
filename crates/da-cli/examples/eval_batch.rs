//! Batch gold-standard evaluation across all fixtures.
//!
//! Runs REAL rule-based extraction on every paper with a gold-standard fixture,
//! aggregates precision/recall/F1 across the corpus.
//!
//! Usage: cargo run -p da-cli --example eval_batch

use da_adapters::{GrobidParser, RuleBasedExtractor};
use da_domain::eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
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
    // Find all gold-standard fixtures
    let mut fixtures: Vec<String> = std::fs::read_dir("data/gold_standard")
        .unwrap_or_else(|e| {
            eprintln!("Cannot read gold_standard dir: {e}");
            std::process::exit(1);
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".gold.json"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    fixtures.sort();

    println!("=== Batch Extraction Evaluation ===");
    println!("Fixtures: {}\n", fixtures.len());

    let mut results: Vec<PaperResult> = Vec::new();
    let mut total_gold = 0usize;
    let mut total_pred = 0usize;
    let mut total_tp = 0usize;
    let mut total_fp = 0usize;
    let mut total_fn = 0usize;

    for fixture_path in &fixtures {
        let gold_json = std::fs::read_to_string(fixture_path).unwrap();
        let gold: GoldFile = serde_json::from_str(&gold_json).unwrap();
        let paper_id = gold.paper_id.clone();

        // Find PDF
        let pdf = std::process::Command::new("find")
            .args(["data/article_catalog", "-name", &format!("{paper_id}.pdf")])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let pdf_path = match pdf {
            Some(p) => p,
            None => {
                eprintln!("⚠ {paper_id}: PDF not found, skipping");
                continue;
            }
        };

        // Parse via GROBID
        let parser = GrobidParser::from_env();
        let parsed = match parser.parse_pdf(&pdf_path, &paper_id).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("⚠ {paper_id}: GROBID failed: {e}");
                continue;
            }
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

        // Collect FN labels
        let false_negatives: Vec<String> = gold
            .entities
            .iter()
            .filter(|g| {
                let gl = g.label.to_lowercase();
                !predicted.iter().any(|p| {
                    let pl = p.label.to_lowercase();
                    gl.contains(&pl) || pl.contains(&gl)
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
