//! Gold-standard evaluation script for extraction quality.
//!
//! Runs REAL rule-based extraction (GROBID + RuleBasedExtractor) on a paper,
//! compares against gold-standard fixtures, outputs precision/recall/F1.
//!
//! Usage:
//!   cargo run -p da-cli --example eval_extract -- 2507.19457
//!   cargo run -p da-cli --example eval_extract -- 2603.21520

use da_adapters::{GrobidParser, RuleBasedExtractor};
use da_domain::eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
use da_ports::extractor::Extractor;
use da_ports::parser::ParserPort;

#[derive(serde::Deserialize)]
struct GoldFile {
    paper_id: String,
    title: String,
    entities: Vec<GoldEntity>,
}

#[tokio::main]
async fn main() {
    let paper_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2507.19457".to_string());
    let gold_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| format!("data/gold_standard/{}.gold.json", paper_id));

    // --- Load gold standard ---
    let gold_json = std::fs::read_to_string(&gold_path).unwrap_or_else(|e| {
        eprintln!("Cannot read gold file {gold_path}: {e}");
        std::process::exit(1);
    });
    let gold: GoldFile = serde_json::from_str(&gold_json).unwrap_or_else(|e| {
        eprintln!("Cannot parse gold JSON: {e}");
        std::process::exit(1);
    });

    println!("=== Gold Standard: {} ===", gold.title);
    println!("Paper: {}", gold.paper_id);
    println!("Gold entities: {}", gold.entities.len());

    // --- Find PDF ---
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
            eprintln!("❌ PDF not found for {paper_id}");
            std::process::exit(1);
        }
    };

    // --- Parse via GROBID ---
    println!("\nParsing {paper_id} via GROBID...");
    let parser = GrobidParser::from_env();
    let parsed = match parser.parse_pdf(&pdf_path, &paper_id).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ GROBID parse failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "   Parsed: {} sections, {} chars",
        parsed.sections.len(),
        parsed.body_text.len()
    );

    // --- Extract via RuleBasedExtractor ---
    println!("\nExtracting entities...");
    let extractor = RuleBasedExtractor::new();
    let sections: Vec<(String, String)> = parsed
        .sections
        .iter()
        .map(|s| (s.title.clone(), s.text.clone()))
        .collect();
    let extracted = match extractor.extract(&sections).await {
        Ok(e) => e,
        Err(e) => {
            eprintln!("❌ Extraction failed: {e}");
            std::process::exit(1);
        }
    };
    println!("   Extracted: {} entities", extracted.len());

    // Convert to PredictedEntity
    let predicted: Vec<PredictedEntity> = extracted
        .iter()
        .map(|e| PredictedEntity {
            label: e.label.clone(),
            entity_type: format!("{:?}", e.entity_type),
        })
        .collect();

    // --- Print predicted by type ---
    let mut pred_by_type: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for e in &predicted {
        *pred_by_type.entry(&e.entity_type).or_default() += 1;
    }
    println!("\n=== Predicted (rule-based) ===");
    for (t, c) in &pred_by_type {
        println!("  {t}: {c}");
    }
    // Print all predicted labels for debugging
    println!("\nAll predicted labels:");
    for e in &predicted {
        println!("  [{}] {}", e.entity_type, e.label);
    }

    // --- Evaluate fuzzy match ---
    let fuzzy = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);
    println!("\n=== Fuzzy Match (substring) ===");
    println!("{}", fuzzy.report());

    // --- Show missed entities (FN) ---
    println!("\n=== Missed entities (false negatives) ===");
    for g in &gold.entities {
        let g_label = g.label.to_lowercase();
        let g_type = g.entity_type.to_lowercase();
        let matched = predicted.iter().any(|p| {
            let p_label = p.label.to_lowercase();
            g_type == p.entity_type.to_lowercase()
                && (g_label.contains(&p_label) || p_label.contains(&g_label))
        });
        if !matched {
            println!("  ❌ {} ({})", g.label, g.entity_type);
        }
    }

    // --- Show spurious entities (FP) ---
    println!("\n=== Spurious entities (false positives) ===");
    for p in &predicted {
        let p_label = p.label.to_lowercase();
        let p_type = p.entity_type.to_lowercase();
        let matched = gold.entities.iter().any(|g| {
            let g_label = g.label.to_lowercase();
            p_type == g.entity_type.to_lowercase()
                && (g_label.contains(&p_label) || p_label.contains(&g_label))
        });
        if !matched {
            println!("  ⚠ [{}] {}", p.entity_type, p.label);
        }
    }
}
