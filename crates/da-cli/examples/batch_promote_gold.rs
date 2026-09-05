//! Batch evidence-based gold promote for weak papers (M316).
//!
//! For each paper id: GROBID parse → RuleBasedExtractor → FP list →
//! `promote_candidates` (multi-paper gold only, no free invent).
//!
//! Usage:
//!   cargo run -p da-cli --example batch_promote_gold -- --ids-file /tmp/m316_weak_ids.txt
//!   cargo run -p da-cli --example batch_promote_gold -- --ids-file ids.txt --write
//!   cargo run -p da-cli --example batch_promote_gold -- 2512.17419 2510.26094 --write
//!
//! Requires GROBID at GROBID_URL (default http://127.0.0.1:8070).
//! Does **not** open graph import (D127).

use da_adapters::RuleBasedExtractor;
use da_domain::{
    ExtractionMetrics, GoldEntity, MultiPaperGoldIndex, PredictedEntity, list_false_positives,
    promote_candidates,
};
use da_ports::extractor::Extractor;
use da_ports::parser::ParserPort;
use std::path::PathBuf;

#[derive(serde::Deserialize, serde::Serialize)]
struct GoldFile {
    paper_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    annotator: String,
    entities: Vec<GoldEntity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    m315_promoted: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    m316_promoted: Option<Vec<PromotedRow>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PromotedRow {
    label: String,
    entity_type: String,
    multi_paper_count: usize,
}

fn load_all_gold(dir: &str) -> Vec<(String, Vec<GoldEntity>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).expect("gold_standard") {
        let p = e.unwrap().path();
        let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
        if !name.ends_with(".gold.json") {
            continue;
        }
        let g: GoldFile = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        out.push((g.paper_id, g.entities));
    }
    out
}

fn find_pdf(paper_id: &str) -> Option<String> {
    let o = std::process::Command::new("find")
        .args(["data/sources/arxiv", "-name", &format!("{paper_id}.pdf")])
        .output()
        .ok()?;
    let s = String::from_utf8(o.stdout).ok()?;
    let t = s.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn parse_args() -> (Vec<String>, usize, bool) {
    let mut ids = Vec::new();
    let mut min_papers = 2usize;
    let mut write = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--ids-file" => {
                let path = args.next().expect("--ids-file needs path");
                let text = std::fs::read_to_string(&path).expect("read ids-file");
                for line in text.lines() {
                    let l = line.trim();
                    if l.is_empty() || l.starts_with('#') {
                        continue;
                    }
                    ids.push(l.to_string());
                }
            }
            "--min-papers" => {
                min_papers = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
            }
            "--write" => write = true,
            "--help" | "-h" => {
                println!(
                    "batch_promote_gold [--ids-file PATH] [--min-papers N] [--write] [paper_id...]"
                );
                std::process::exit(0);
            }
            other if !other.starts_with('-') => ids.push(other.to_string()),
            other => eprintln!("unknown arg: {other}"),
        }
    }
    if ids.is_empty() {
        eprintln!("no paper ids; pass --ids-file or positional ids");
        std::process::exit(2);
    }
    (ids, min_papers, write)
}

#[tokio::main]
async fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("batch_promote_gold");
    let (ids, min_papers, write) = parse_args();
    println!(
        "batch_promote_gold: {} papers, min_papers={min_papers}, write={write}",
        ids.len()
    );

    let all = load_all_gold("data/gold_standard");
    let index = MultiPaperGoldIndex::from_papers(
        all.iter()
            .map(|(pid, ents)| (pid.as_str(), ents.as_slice())),
    );

    // S11: corpus PDF parse must consult the DATA profile route (structural
    // default) — the historical rich from_env facade stays health-only.
    let parser = da_cli::parse_profile::corpus_grobid_parser();
    if !parser.is_alive().await {
        eprintln!("❌ GROBID not alive; start service before batch promote");
        std::process::exit(1);
    }
    let extractor = RuleBasedExtractor::new();

    let mut total_promoted = 0usize;
    let mut papers_changed = 0usize;
    let mut papers_skipped = 0usize;

    for paper_id in &ids {
        println!("\n========== {paper_id} ==========");
        let gold_path = PathBuf::from(format!("data/gold_standard/{paper_id}.gold.json"));
        if !gold_path.exists() {
            println!("  skip: no gold file");
            papers_skipped += 1;
            continue;
        }
        let mut gold: GoldFile =
            serde_json::from_str(&std::fs::read_to_string(&gold_path).unwrap()).unwrap();

        let Some(pdf_path) = find_pdf(paper_id) else {
            println!("  skip: PDF not found");
            papers_skipped += 1;
            continue;
        };

        let parsed = match parser.parse_pdf(&pdf_path, paper_id).await {
            Ok(p) => p,
            Err(e) => {
                println!("  skip: GROBID failed: {e}");
                papers_skipped += 1;
                continue;
            }
        };
        let sections: Vec<(String, String)> = parsed
            .sections
            .iter()
            .map(|s| (s.title.clone(), s.text.clone()))
            .collect();
        let extracted = match extractor.extract(&sections).await {
            Ok(e) => e,
            Err(e) => {
                println!("  skip: extract failed: {e}");
                papers_skipped += 1;
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

        let before = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);
        let fps = list_false_positives(&gold.entities, &predicted);
        let candidates = promote_candidates(&gold.entities, &fps, &index, min_papers);

        println!(
            "  gold={} pred={} fp={} promote_candidates={}",
            gold.entities.len(),
            predicted.len(),
            fps.len(),
            candidates.len()
        );
        println!("  before {}", before.report());

        if candidates.is_empty() {
            println!("  no evidence-based promotes");
            continue;
        }

        let mut rows = Vec::new();
        for c in &candidates {
            let n = index.paper_count(&c.label, &c.entity_type);
            println!(
                "  + [{}] {}  (multi_paper_gold={n})",
                c.entity_type, c.label
            );
            rows.push(PromotedRow {
                label: c.label.clone(),
                entity_type: c.entity_type.clone(),
                multi_paper_count: n,
            });
            gold.entities.push(c.clone());
        }
        total_promoted += candidates.len();
        papers_changed += 1;

        let after = ExtractionMetrics::evaluate_fuzzy(&gold.entities, &predicted);
        println!("  after  {}", after.report());

        if write {
            if !gold.annotator.contains("m316-evidence-promote") {
                gold.annotator = format!("{}+m316-evidence-promote", gold.annotator);
            }
            gold.m316_promoted = Some(rows);
            let json = serde_json::to_string_pretty(&gold).unwrap();
            std::fs::write(&gold_path, json + "\n").expect("write gold");
            println!("  wrote {}", gold_path.display());
        } else {
            println!("  dry-run (pass --write to persist)");
        }
    }

    println!("\n=== Summary ===");
    println!(
        "papers_changed={papers_changed} total_promoted={total_promoted} skipped={papers_skipped} write={write}"
    );
}
