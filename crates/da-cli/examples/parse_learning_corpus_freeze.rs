//! Freeze the document-disjoint parse-learning corpus (M514-1lss2s/S07/T01).
//!
//! It reads the committed agent trajectory log, groups records by document
//! identity, splits documents with the deterministic SHA-256 rank rule, and
//! writes the frozen corpus through a `.tmp` + rename so a rerun is
//! byte-identical. It writes no graph, promotes no candidate, and touches
//! exactly two paths; it has no network access.

use da_application::parse_learning_split::{
    DOCUMENT_IDENTITY_FIELD, DocumentGroup, DocumentGroupSplit, DocumentRecord,
    FrozenLearningCorpus, GAIN_RULE_STATE_FROZEN, GainRule, LEARNING_CORPUS_SCHEMA_VERSION,
    PROMPT_IDENTITY_ROLE_LINEAGE, REWARD_VERSION_V1, RewardDefinition,
    SPLIT_RULE_DOCUMENT_RANK_MODULO,
};
use da_ports::Sha256Digest;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// The only input path this example reads.
const SOURCE_PATH: &str = "data/agent_memory/summarizer/trajectory.jsonl";
/// The only output path this example writes.
const OUT_PATH: &str = "artifacts/parse_learning/learning_corpus.json";
/// Documents are ranked by `sha256(document_identity)`; every fourth goes to
/// validation.
const VALIDATION_EVERY: usize = 4;

/// The subset of a trajectory row this freezer consumes. Unknown fields are
/// ignored on purpose: the source log carries routing features this corpus does
/// not need.
#[derive(Debug, Deserialize)]
struct TrajectoryRow {
    paper_id: String,
    prompt_sha256: String,
    ok: bool,
    latency_ms: u64,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    grounding_hint: Option<f64>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn main() -> anyhow::Result<()> {
    let source_bytes = std::fs::read(SOURCE_PATH)?;
    let source_sha256 = Sha256Digest::new(sha256_hex(&source_bytes))?;
    let source_text = std::str::from_utf8(&source_bytes)?;

    // Group by document identity, preserving file order inside each document.
    let mut groups: BTreeMap<String, Vec<DocumentRecord>> = BTreeMap::new();
    for (index, line) in source_text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: TrajectoryRow = serde_json::from_str(line)
            .map_err(|error| anyhow::anyhow!("trajectory line {}: {error}", index + 1))?;
        let record = DocumentRecord {
            document_identity: row.paper_id.clone(),
            prompt_identity: Sha256Digest::new(row.prompt_sha256)?,
            quality: row.grounding_hint,
            ok: row.ok,
            latency_ms: row.latency_ms,
            completion_tokens: row.completion_tokens,
        };
        groups.entry(row.paper_id).or_default().push(record);
    }

    // Deterministic ranking: sha256(document identity), then identity as a tie
    // break so the order never depends on hash-map iteration.
    let mut ranked: Vec<(String, String)> = groups
        .keys()
        .map(|identity| (sha256_hex(identity.as_bytes()), identity.clone()))
        .collect();
    ranked.sort();

    let mut train = Vec::new();
    let mut validation = Vec::new();
    for (rank, (_, identity)) in ranked.iter().enumerate() {
        let records = groups
            .remove(identity)
            .expect("every ranked identity was inserted");
        let prompt_identities = records
            .iter()
            .map(|record| record.prompt_identity.clone())
            .collect();
        let group = DocumentGroup {
            document_identity: identity.clone(),
            prompt_identities,
            records,
        };
        if (rank + 1) % VALIDATION_EVERY == 0 {
            validation.push(group);
        } else {
            train.push(group);
        }
    }

    let split = DocumentGroupSplit {
        split_rule: SPLIT_RULE_DOCUMENT_RANK_MODULO.to_string(),
        train,
        validation,
    };
    split.validate()?;

    let reward_definition = RewardDefinition {
        version: REWARD_VERSION_V1.to_string(),
        components: vec!["grounding_hint".to_string(), "ok_indicator".to_string()],
        digest: Sha256Digest::new("0".repeat(64))?,
    }
    .seal();

    let corpus = FrozenLearningCorpus {
        schema_version: LEARNING_CORPUS_SCHEMA_VERSION.to_string(),
        source_path: SOURCE_PATH.to_string(),
        source_sha256,
        document_identity_field: DOCUMENT_IDENTITY_FIELD.to_string(),
        prompt_identity_role: PROMPT_IDENTITY_ROLE_LINEAGE.to_string(),
        split_rule: split.split_rule,
        reward_definition,
        gain_rule: GainRule {
            metric_one: "grounding_hint_mean".to_string(),
            metric_two: "ok_rate".to_string(),
            // Any positive improvement over the baseline counts; the rule is
            // frozen here, before measurement.
            margin: 0.0,
            state: GAIN_RULE_STATE_FROZEN.to_string(),
        },
        train: split.train,
        validation: split.validation,
        corpus_digest: Sha256Digest::new("0".repeat(64))?,
    }
    .seal();
    corpus.validate()?;

    let bytes = serde_json::to_vec_pretty(&corpus)?;
    let out = Path::new(OUT_PATH);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = out.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, out)?;

    println!(
        "train_documents={} validation_documents={} corpus_digest={}",
        corpus.train.len(),
        corpus.validation.len(),
        corpus.corpus_digest.as_str()
    );
    Ok(())
}
