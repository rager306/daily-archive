//! Evaluate extraction on a non-PDF HTML source (GNN textbook chapters).
//!
//! Usage: cargo run -p da-cli --example eval_html -- <html_path> <paper_id>
//! Example:
//!   cargo run -p da-cli --example eval_html -- \
//!     data/article_catalog/article_catalog/gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs/source/chapter.html \
//!     gnn-ch-01

use da_adapters::{HtmlParser, RuleBasedExtractor};
use da_ports::extractor::Extractor;
use da_ports::parser::ParserPort;

#[tokio::main]
async fn main() {
    let html_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            "data/article_catalog/article_catalog/gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs/source/chapter.html".to_string()
        });
    let paper_id = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "gnn-ch-01".to_string());

    println!("=== HTML Extraction Evaluation ===");
    println!("Source: {html_path}");
    println!("Paper ID: {paper_id}\n");

    // Parse HTML
    let parser = HtmlParser::new();
    let parsed = match parser.parse_html(&html_path, &paper_id).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ HTML parse failed: {e}");
            std::process::exit(1);
        }
    };

    println!("Title: {}", parsed.title);
    println!(
        "Abstract: {}",
        if parsed.abstract_text.is_empty() {
            "(none)"
        } else {
            &parsed.abstract_text[..80.min(parsed.abstract_text.len())]
        }
    );
    println!("Sections: {}", parsed.sections.len());
    println!("Body chars: {}", parsed.body_text.len());

    for s in parsed.sections.iter().take(5) {
        let title_preview = if s.title.is_empty() {
            "(untitled)"
        } else {
            &s.title[..40.min(s.title.len())]
        };
        println!(
            "  [h{}] {} ({} chars)",
            s.level,
            title_preview,
            s.text.len()
        );
    }
    if parsed.sections.len() > 5 {
        println!("  ... and {} more sections", parsed.sections.len() - 5);
    }

    // Extract entities
    println!("\n=== Extraction ===");
    let extractor = RuleBasedExtractor::new();
    let sections: Vec<(String, String)> = parsed
        .sections
        .iter()
        .map(|s| (s.title.clone(), s.text.clone()))
        .collect();
    let entities = match extractor.extract(&sections).await {
        Ok(e) => e,
        Err(e) => {
            eprintln!("❌ Extraction failed: {e}");
            std::process::exit(1);
        }
    };

    println!("Extracted: {} entities", entities.len());

    // Group by type
    use std::collections::HashMap;
    let mut by_type: HashMap<String, Vec<String>> = HashMap::new();
    for e in &entities {
        let type_str = format!("{:?}", e.entity_type);
        by_type.entry(type_str).or_default().push(e.label.clone());
    }
    for (etype, labels) in by_type.iter() {
        println!("  {etype}: {}", labels.len());
        for label in labels.iter().take(10) {
            println!("    - {label}");
        }
        if labels.len() > 10 {
            println!("    ... and {} more", labels.len() - 10);
        }
    }

    // Check for GNN-specific entities that should be found
    println!("\n=== GNN-specific entity check ===");
    let entity_labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
    let gnn_terms = [
        "GCN",
        "GAT",
        "GraphSAGE",
        "PPO",
        "DPO",
        "CoT",
        "BERT",
        "GPT-4",
        "Claude",
        "accuracy",
        "F1",
        "MATH",
        "RLHF",
        "RAG",
    ];
    for term in &gnn_terms {
        let found = entity_labels.iter().any(|l| l.eq_ignore_ascii_case(term));
        if found {
            println!("  ✅ {term}");
        }
    }
    let missing: Vec<&&str> = gnn_terms
        .iter()
        .filter(|t| !entity_labels.iter().any(|l| l.eq_ignore_ascii_case(t)))
        .collect();
    if !missing.is_empty() {
        println!(
            "  (not found: {})",
            missing.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
        );
    }

    println!("\n=== Summary ===");
    println!("HTML parser + extraction pipeline: WORKING");
    println!("Multi-source capability: DEMONSTRATED (HTML ≠ PDF)");
}
