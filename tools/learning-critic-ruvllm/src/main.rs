//! CLI:
//! `derive-cases` — TEI paths -> redacted structural critic cases (JSON).
//! `observe`      — cases JSON -> parse-learning-ablation.v1 observations
//!                  (deterministic reflection critic, no model).
//! `observe-gguf` — cases JSON -> observations through real GGUF inference
//!                  on CPU (`--model <registry-id|dir>`, greedy, T2 Э2).

use learning_critic_ruvllm::{
    derive_subject, identity_digest, CriticCase, GgufCritic, RuvllmReflectionCritic,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn sha256_file(path: &std::path::Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn derive_cases(tei_paths: &[PathBuf], corpus_id: &str) -> anyhow::Result<Vec<CriticCase>> {
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
                    "--tei" => {
                        i += 1;
                        tei.push(PathBuf::from(&args[i]));
                    }
                    "--corpus-id" => {
                        i += 1;
                        corpus_id = args[i].clone();
                    }
                    other => anyhow::bail!("unexpected arg {other}"),
                }
                i += 1;
            }
            if tei.is_empty() {
                anyhow::bail!("no --tei paths given");
            }
            let cases = derive_cases(&tei, &corpus_id)?;
            println!("{}", serde_json::to_string_pretty(&cases)?);
        }
        Some("observe") => {
            let mut cases_path = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--cases" => {
                        i += 1;
                        cases_path = Some(PathBuf::from(&args[i]));
                    }
                    other => anyhow::bail!("unexpected arg {other}"),
                }
                i += 1;
            }
            let cases: Vec<CriticCase> = serde_json::from_str(&std::fs::read_to_string(
                cases_path.ok_or_else(|| anyhow::anyhow!("--cases <json> required"))?,
            )?)?;
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
        Some("observe-gguf") => {
            let mut cases_path = None;
            let mut model = String::new();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--cases" => {
                        i += 1;
                        cases_path = Some(PathBuf::from(&args[i]));
                    }
                    "--model" => {
                        i += 1;
                        model = args[i].clone();
                    }
                    other => anyhow::bail!("unexpected arg {other}"),
                }
                i += 1;
            }
            if model.is_empty() {
                anyhow::bail!("--model <registry-id|dir> required");
            }
            let cases: Vec<CriticCase> = serde_json::from_str(&std::fs::read_to_string(
                cases_path.ok_or_else(|| anyhow::anyhow!("--cases <json> required"))?,
            )?)?;
            let t0 = std::time::Instant::now();
            let critic = GgufCritic::load(&model)?;
            let t_load = t0.elapsed();
            eprintln!(
                "model {} loaded in {}ms (gguf sha256 {})",
                critic.spec().id,
                t_load.as_millis(),
                critic.model_sha256()
            );
            let corpus = cases.first().map(|c| c.corpus.clone()).unwrap_or_default();
            let mut rows = Vec::new();
            let mut stats = Vec::new();
            let mut generations = Vec::new();
            for case in &cases {
                let out = critic.observe(case)?;
                eprintln!(
                    "{}: metric_one={:.3} status={:?} gen {:.1} tok/s raw={:?}",
                    out.observation.case_id,
                    out.observation.metric_one,
                    out.observation.status,
                    out.stats.tokens_per_second,
                    out.raw.chars().take(80).collect::<String>()
                );
                rows.push(out.observation);
                stats.push(out.stats);
                generations.push(serde_json::json!({
                    "case_id": case.case_id,
                    "raw": out.raw,
                }));
            }
            let total_gen: usize = stats.iter().map(|s| s.generated_tokens_approx).sum();
            let total_ms: u128 = stats.iter().map(|s| s.elapsed_ms).sum();
            let tps = total_gen as f64 / (total_ms as f64 / 1000.0).max(1e-9);
            let doc = serde_json::json!({
                "schema": "parse-learning-ablation.v1",
                "corpus": corpus,
                "observations": rows,
                "provenance": critic.provenance(&corpus),
                "model": {
                    "id": critic.spec().id,
                    "quant": critic.spec().quant,
                    "arch": critic.spec().arch,
                    "ctx": critic.spec().ctx,
                    "load_ms": t_load.as_millis(),
                    "tokens_per_second": tps,
                    "generated_tokens_approx": total_gen,
                },
                "generations": generations,
            });
            println!("{}", serde_json::to_string_pretty(&doc)?);
        }
        other => anyhow::bail!("usage: {other:?} derive-cases|observe|observe-gguf"),
    }
    Ok(())
}
