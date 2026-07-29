//! Governor CLI: whitelist suggestions from FN analysis.
//!
//! SAGA-inspired bi-level loop (deterministic, no LLM):
//! - Inner loop: RuleBasedExtractor → eval_batch → P/R/F1
//! - Outer loop: analyze FN patterns → suggest whitelist additions
//!
//! This script reads the gold-standard fixtures, checks each gold entity
//! against all existing whitelists (KNOWN_METHODS, KNOWN_DATASETS, etc.),
//! and reports entities that are NOT covered by any whitelist — these are
//! candidates for manual whitelist expansion.
//!
//! Usage: cargo run -p da-cli --example suggest_whitelist

use da_adapters::RuleBasedExtractor;
use da_ports::extractor::Extractor;
use std::collections::HashSet;

#[derive(serde::Deserialize)]
struct GoldFile {
    paper_id: String,
    entities: Vec<GoldEntity>,
}

#[derive(serde::Deserialize)]
struct GoldEntity {
    label: String,
    entity_type: String,
}

/// Check if a label would be found by the current extractor.
/// Runs extraction on a synthetic section containing the label.
async fn would_be_extracted(label: &str, entity_type: &str) -> bool {
    let extractor = RuleBasedExtractor::new();
    let text = format!("We evaluate {label} in our experiments.");
    let sections = vec![("Experiments".to_string(), text)];
    let entities = extractor.extract(&sections).await.unwrap_or_default();

    let label_lower = label.to_lowercase();
    entities.iter().any(|e| {
        e.label.to_lowercase() == label_lower
            && format!("{:?}", e.entity_type).eq_ignore_ascii_case(entity_type)
    })
}

#[tokio::main]
async fn main() {
    // Load all gold-standard fixtures
    let mut fixtures: Vec<String> = std::fs::read_dir("data/gold_standard")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".gold.json"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    fixtures.sort();

    println!("=== Whitelist Suggestion Analysis ===\n");

    // Collect all unique gold entities across the corpus
    let mut all_gold: Vec<(String, String, String)> = Vec::new(); // (label, type, paper_id)
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for fixture_path in &fixtures {
        let gold_json = std::fs::read_to_string(fixture_path).unwrap();
        let gold: GoldFile = serde_json::from_str(&gold_json).unwrap();
        for e in &gold.entities {
            let key = (e.label.to_lowercase(), e.entity_type.to_lowercase());
            if seen.insert(key.clone()) {
                all_gold.push((
                    e.label.clone(),
                    e.entity_type.clone(),
                    gold.paper_id.clone(),
                ));
            }
        }
    }

    println!(
        "Total unique gold entities across corpus: {}\n",
        all_gold.len()
    );

    // Check each gold entity: would it be extracted by current whitelists?
    let mut uncovered: Vec<(String, String, String)> = Vec::new();

    for (label, entity_type, paper_id) in &all_gold {
        if !would_be_extracted(label, entity_type).await {
            uncovered.push((label.clone(), entity_type.clone(), paper_id.clone()));
        }
    }

    // Report uncovered entities
    if uncovered.is_empty() {
        println!("✅ All gold entities are covered by current whitelists.");
    } else {
        println!(
            "📋 Gold entities NOT covered by current whitelists ({}):",
            uncovered.len()
        );
        println!("These are candidates for whitelist expansion:\n");
        for (label, etype, paper_id) in &uncovered {
            let is_multi_word = label.contains(' ') || label.contains('-');
            let category = if is_multi_word { "phrase" } else { "word" };
            let suggestion = match etype.as_str() {
                "Method" => {
                    if is_multi_word {
                        format!("Add \"{label}\" to KNOWN_METHOD_PHRASES")
                    } else {
                        format!("Add \"{label}\" to KNOWN_METHODS (check for FP risk)")
                    }
                }
                "Task" => format!("Add \"{label}\" to TASK_PHRASES or TASK_ACRONYMS"),
                "Dataset" => format!("Add \"{label}\" to KNOWN_DATASETS"),
                "Model" => format!("Add \"{label}\" to KNOWN_MODELS"),
                "Metric" => format!("Add \"{label}\" to KNOWN_METRICS"),
                _ => format!("Unknown type: {etype}"),
            };
            println!("  [{etype:7}] \"{label}\" ({category}, paper {paper_id})");
            println!("           → {suggestion}");
        }
    }

    // Summary
    let covered = all_gold.len() - uncovered.len();
    let coverage_pct = (covered as f64 / all_gold.len() as f64) * 100.0;
    println!("\n=== Coverage Summary ===");
    println!(
        "Covered by whitelist:    {}/{} ({:.1}%)",
        covered,
        all_gold.len(),
        coverage_pct
    );
    println!("Uncovered (candidates):  {}", uncovered.len());
}
