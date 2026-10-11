//! CLI:
//! `derive-cases` — TEI paths -> redacted structural critic cases (JSON).
//! `observe`      — cases JSON -> parse-learning-ablation.v1 observations.

use learning_critic_ruvllm::{CriticCase, RuvllmReflectionCritic, identity_digest};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn sha256_file(path: &std::path::Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// Redacted structural summary: tag-count profile only — no TEI bytes,
/// no text content cross this boundary (the redaction barrier).
fn derive_subject(tei: &str) -> String {
    let count = |tag: &str| tei.matches(&format!("<{tag}")).count();
    format!(
        "sections: {}; references: {}; figures: {}; tables: {}; formulas: {}; \
         abstract_present: {}; funding_present: {}; notes: {}; \
         keywords: {}; language_set: {}",
        count("<div"),
        count("<biblStruct"),
        count("<figure"),
        count("<table"),
        count("<formula"),
        count("<abstract") > 0,
        count("<funding") > 0,
        count("<note"),
        count("<keywords") > 0,
        count("xml:lang") > 0,
    )
}

fn derive_cases(tei_paths: &[PathBuf], corpus_id: &str) -> anyhow::Result<Vec<CriticCase>> {
    let corpus = identity_digest(corpus_id);
    let mut file_digests = Vec::new();
    for p in tei_paths {
        file_digests.push(sha256_file(p)?);
    }
    // Corpus digest binds the ordered file digest list.
    let corpus = identity_digest(&format!("{corpus_id}:{}", file_digests.join(",")));
    let mut cases = Vec::new();
    for (order, p) in tei_paths.iter().enumerate() {
        let tei = std::fs::read_to_string(p)?;
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("doc")
            .to_string();
        cases.push(CriticCase {
            case_id: format!("frozen-{stem}"),
            corpus: corpus.clone(),
            order: order as u32,
            subject: derive_subject(&tei),
        });
    }
    Ok(cases)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("derive-cases") => {
            let mut tei = Vec::new();
            let mut corpus_id = String::from("frozen-oracle.v1");
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--tei" => { i += 1; tei.push(PathBuf::from(&args[i])); }
                    "--corpus-id" => { i += 1; corpus_id = args[i].clone(); }
                    other => anyhow::bail!("unexpected arg {other}"),
                }
                i += 1;
            }
            if tei.is_empty() { anyhow::bail!("no --tei paths given"); }
            let cases = derive_cases(&tei, &corpus_id)?;
            println!("{}", serde_json::to_string_pretty(&cases)?);
        }
        Some("observe") => {
            let mut cases_path = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--cases" => { i += 1; cases_path = Some(PathBuf::from(&args[i])); }
                    other => anyhow::bail!("unexpected arg {other}"),
                }
                i += 1;
            }
            let cases: Vec<CriticCase> = serde_json::from_str(
                &std::fs::read_to_string(
                    cases_path.ok_or_else(|| anyhow::anyhow!("--cases <json> required"))?,
                )?,
            )?;
            let critic = RuvllmReflectionCritic::new();
            let corpus = cases.first().map(|c| c.corpus.clone()).unwrap_or_default();
            let mut rows = Vec::new();
            for case in &cases {
                rows.push(critic.observe(case)?);
            }
            let doc = serde_json::json!({
                "schema": "parse-learning-ablation.v1",
                "corpus": corpus,
                "observations": rows,
                "provenance": critic.provenance(&corpus),
            });
            println!("{}", serde_json::to_string_pretty(&doc)?);
        }
        other => anyhow::bail!("usage: {other:?} derive-cases|observe"), 
    }
    Ok(())
}
