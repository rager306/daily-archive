//! Closed train → persist → reload loop for the parse-policy candidate (M514/S07/T02).
//!
//! These tests prove that the stored candidate is a real, reloadable artifact:
//! `reload_persisted_candidate` recomputes the weights digest from the persisted
//! projections, enforces the candidate-only manifest locks, and `candidate_forward`
//! applies those weights through the vendor MicroLoRA forward pass.

use da_adapters::RvfFileStore;
use da_application::{
    TrainingConfig, TrainingError, candidate_forward, reload_persisted_candidate,
    train_microlora_candidate,
};
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::{CandidateManifest, LearningContractError, LearningLineage, Sha256Digest};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(value.as_bytes()))).unwrap()
}

fn lineage() -> LearningLineage {
    LearningLineage {
        source: digest("source"),
        corpus: digest("corpus"),
        config: digest("config"),
        prior_policy: digest("policy"),
        conservation: digest("conservation"),
        split: digest("split"),
    }
}

fn record(paper: &str, prompt: &str, quality: f64) -> TrajectoryRecord {
    TrajectoryRecord {
        task_id: "parse_summary".into(),
        paper_id: paper.into(),
        model: "frozen-model".into(),
        prompt_sha256: prompt.into(),
        ok: true,
        latency_ms: 10,
        completion_tokens: Some(40),
        grounding_hint: Some(quality),
        article_features: None,
        failure_pattern: None,
        construction_trace: None,
    }
}

/// Train one deterministic candidate over a document-disjoint fixture and persist
/// it. All records share one `prompt_sha256` on purpose: the prompt is lineage, the
/// document is the split key.
fn train_fixture(dir: &Path, tag: &str) -> (PathBuf, CandidateManifest) {
    let mut store =
        RvfFileStore::open(dir.join(format!("experience-{tag}.rvf")), lineage()).unwrap();
    let shared_prompt = "f".repeat(64);
    let train = vec![
        record("paper-a", &shared_prompt, 0.8),
        record("paper-b", &shared_prompt, 0.9),
        record("paper-c", &shared_prompt, 0.7),
    ];
    let validation = vec![record("paper-v", &shared_prompt, 0.75)];
    let candidate_path = dir.join(format!("candidate-{tag}.json"));
    let outcome = train_microlora_candidate(
        &mut store,
        &train,
        &validation,
        lineage(),
        None,
        TrainingConfig::default(),
        Some(&candidate_path),
    )
    .unwrap();
    (candidate_path, outcome.manifest)
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn reload_recomputes_weights_digest_and_rejects_tamper() {
    let dir = tempfile::tempdir().unwrap();
    let (candidate_path, manifest) = train_fixture(dir.path(), "primary");

    // Reload is a real read path: digest verified, then a real forward pass.
    let reloaded = reload_persisted_candidate(&candidate_path).unwrap();
    reloaded.verify_weights().unwrap();
    let output = candidate_forward(&reloaded, &[0.5f32; 8]).unwrap();
    assert_eq!(output.len(), 8);
    assert!(
        output.iter().all(|value| value.is_finite()),
        "reloaded forward pass must be finite"
    );

    // Tampering with a single weight element must be detected by the digest.
    let mut tampered = read_json(&candidate_path);
    let up = tampered
        .get_mut("up_projection")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let first = up[0].as_f64().unwrap();
    up[0] = serde_json::json!(first + 0.25);
    let tampered_weights = dir.path().join("tampered_weights.json");
    write_json(&tampered_weights, &tampered);
    assert!(matches!(
        reload_persisted_candidate(&tampered_weights),
        Err(TrainingError::WeightsDigestMismatch)
    ));

    // Flipping the candidate-only lock is a contract violation on reload.
    let mut unlocked = read_json(&candidate_path);
    unlocked["manifest"]["receipt"]["deployed"] = serde_json::json!(true);
    let tampered_lock = dir.path().join("tampered_lock.json");
    write_json(&tampered_lock, &unlocked);
    assert!(matches!(
        reload_persisted_candidate(&tampered_lock),
        Err(TrainingError::Contract(
            LearningContractError::PromotionLockViolation
        ))
    ));

    // Determinism: a second independent train+persist yields the same identity.
    let (second_path, second_manifest) = train_fixture(dir.path(), "secondary");
    assert_eq!(
        manifest.receipt.candidate_id,
        second_manifest.receipt.candidate_id
    );
    assert_eq!(manifest.weights_digest, second_manifest.weights_digest);
    let second_reloaded = reload_persisted_candidate(&second_path).unwrap();
    assert_eq!(
        reloaded.manifest.receipt.candidate_id,
        second_reloaded.manifest.receipt.candidate_id
    );
}

#[test]
fn reload_and_forward_reject_malformed_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let (candidate_path, _) = train_fixture(dir.path(), "malformed");

    // A missing artifact is unreadable, not a panic.
    assert!(matches!(
        reload_persisted_candidate(&dir.path().join("does-not-exist.json")),
        Err(TrainingError::CandidateArtifactUnreadable(_))
    ));

    // Bytes that do not parse as a candidate are unreadable, not a panic.
    let garbage = dir.path().join("garbage.json");
    std::fs::write(&garbage, b"{ not json").unwrap();
    assert!(matches!(
        reload_persisted_candidate(&garbage),
        Err(TrainingError::CandidateArtifactUnreadable(_))
    ));

    // A wrong-length input cannot be silently padded.
    let candidate = reload_persisted_candidate(&candidate_path).unwrap();
    assert!(matches!(
        candidate_forward(&candidate, &[0.0f32; 7]),
        Err(TrainingError::Artifact(_))
    ));
}
