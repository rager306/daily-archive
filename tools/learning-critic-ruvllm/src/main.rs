//! CLI: run the deterministic ruVLLM reflection critic over the embedded
//! redacted sample cases and emit parse-learning-ablation.v1 observations
//! JSON (rows + provenance) for the offline evaluator.

use learning_critic_ruvllm::{CriticCase, RuvllmReflectionCritic, identity_digest};
use sha2::{Digest, Sha256};

const E2E_CORPUS_IDENTITY: &str = "learning-critic-e2e.sample-corpus.v1";

fn well_formed_subject() -> String {
    [
        "sections: 7; references: 41; figures: 3; tables: 2",
        "reading_order: linear; heading_depth_max: 3",
        "abstract_present: true; acknowledgements_present: true",
        "author_block: parsed; affiliation_present: true",
    ]
    .join("\n")
}

fn degraded_subject() -> String {
    ["sections: 7 ((unclosed", "  references: 41", "\tfigures: 3", "abstract_present: false"].join("\n")
}

fn embedded_cases(corpus: &str) -> Vec<CriticCase> {
    vec![
        CriticCase { case_id: "e2e-well-formed".into(), corpus: corpus.into(), order: 0, subject: well_formed_subject() },
        CriticCase { case_id: "e2e-degraded".into(), corpus: corpus.into(), order: 1, subject: degraded_subject() },
    ]
}

fn main() -> anyhow::Result<()> {
    let corpus = identity_digest(E2E_CORPUS_IDENTITY);
    let critic = RuvllmReflectionCritic::new();
    let cases = embedded_cases(&corpus);
    let mut rows = Vec::new();
    for case in &cases {
        rows.push(critic.observe(case)?);
    }
    let out = serde_json::json!({
        "schema": "parse-learning-ablation.v1",
        "corpus": corpus,
        "observations": rows,
        "provenance": critic.provenance(&corpus),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
