//! E2 FP inventory + evidence-based gold promote dry-run (M315).
//!
//! Offline-friendly when given predicted labels; does **not** free-invent.
//! Promote candidates require the (label, type) to already be gold in
//! `min_papers` other fixtures (constrained select / D127 spirit).
//!
//! Usage:
//!   cargo run -p da-cli --example inventory_fp -- 2603.21520
//!   cargo run -p da-cli --example inventory_fp -- 2603.21520 --apply-dry-run
//!
//! Predictions for a paper can be piped as lines `Type\tLabel` via --preds-file,
//! or use built-in sample FP list for the known weak paper 2603.21520.

use da_domain::{
    GoldEntity, MultiPaperGoldIndex, PredictedEntity, list_false_positives, promote_candidates,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct GoldFile {
    paper_id: String,
    entities: Vec<GoldEntity>,
}

fn load_all_gold(dir: &str) -> Vec<(String, Vec<GoldEntity>)> {
    let mut out = Vec::new();
    let rd = std::fs::read_dir(dir).expect("gold_standard dir");
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        if !p
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .ends_with(".gold.json")
        {
            continue;
        }
        let g: GoldFile = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        out.push((g.paper_id, g.entities));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn sample_preds_2603() -> Vec<PredictedEntity> {
    // From eval_extract 2026-08-05 (GROBID + RuleBasedExtractor).
    const ROWS: &[(&str, &str)] = &[
        ("Task", "prompt optimization"),
        ("Metric", "accuracy"),
        ("Dataset", "BBH"),
        ("Method", "self-evolving memory"),
        ("Method", "generalization"),
        ("Method", "OPRO"),
        ("Method", "CoT"),
        ("Method", "in-context learning"),
        ("Dataset", "MATH"),
        ("Dataset", "AGIEval"),
        ("Model", "GPT-4"),
        ("Model", "GPT-4o"),
        ("Model", "Qwen"),
        ("Metric", "recall"),
    ];
    ROWS.iter()
        .map(|(t, l)| PredictedEntity {
            label: (*l).into(),
            entity_type: (*t).into(),
        })
        .collect()
}

fn load_preds_file(path: &PathBuf) -> Vec<PredictedEntity> {
    let text = std::fs::read_to_string(path).expect("preds file");
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let typ = parts.next().unwrap_or("").trim();
        let lab = parts.next().unwrap_or("").trim();
        if typ.is_empty() || lab.is_empty() {
            eprintln!("skip bad line: {line}");
            continue;
        }
        out.push(PredictedEntity {
            label: lab.into(),
            entity_type: typ.into(),
        });
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let paper_id = args.next().unwrap_or_else(|| "2603.21520".to_string());
    let mut preds_file: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut min_papers: usize = 2;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--preds-file" => preds_file = args.next().map(PathBuf::from),
            "--apply-dry-run" => dry_run = true,
            "--min-papers" => {
                min_papers = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
            }
            other => eprintln!("unknown arg: {other}"),
        }
    }

    let all = load_all_gold("data/gold_standard");
    let index = MultiPaperGoldIndex::from_papers(
        all.iter()
            .map(|(pid, ents)| (pid.as_str(), ents.as_slice())),
    );

    let paper = all
        .iter()
        .find(|(pid, _)| pid == &paper_id)
        .unwrap_or_else(|| panic!("no gold for {paper_id}"));
    let gold = &paper.1;

    let predicted = if let Some(pf) = preds_file {
        load_preds_file(&pf)
    } else if paper_id == "2603.21520" {
        sample_preds_2603()
    } else {
        eprintln!("No --preds-file and no built-in sample for {paper_id}; inventory empty.");
        Vec::new()
    };

    let fps = list_false_positives(gold, &predicted);
    println!("=== FP inventory: {paper_id} ===");
    println!(
        "gold={} pred={} fp={}",
        gold.len(),
        predicted.len(),
        fps.len()
    );
    let mut by_type: HashMap<String, usize> = HashMap::new();
    for p in &fps {
        *by_type.entry(p.entity_type.clone()).or_default() += 1;
        let n = index.paper_count(&p.label, &p.entity_type);
        let tag = if n >= min_papers {
            "PROMOTE?"
        } else {
            "noise/free"
        };
        println!(
            "  [{tag}] type={} label={:?} multi_paper_gold_count={n}",
            p.entity_type, p.label
        );
    }
    println!("\nFP by type: {by_type:?}");

    let promoted = promote_candidates(gold, &fps, &index, min_papers);
    println!("\n=== Evidence-based promote candidates (min_papers={min_papers}) ===");
    if promoted.is_empty() {
        println!("(none — free invent rejected or already gold)");
    } else {
        for g in &promoted {
            let n = index.paper_count(&g.label, &g.entity_type);
            println!("  + [{}] {}  (gold in {n} papers)", g.entity_type, g.label);
        }
    }

    if dry_run {
        println!(
            "\n--apply-dry-run: would write {} entities (not writing)",
            promoted.len()
        );
    }
}
