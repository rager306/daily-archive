use da_adapters::RvfFileStore;
use da_application::{TrainingConfig, train_microlora_candidate};
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::{LearningLineage, RvfExperienceStore, Sha256Digest};
use sha2::{Digest, Sha256};

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

#[test]
fn deterministic_candidate_and_rvf_lineage_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("experience.rvf");
    let mut store = RvfFileStore::open(path.clone(), lineage()).unwrap();
    let train = vec![
        record("paper-a", &"a".repeat(64), 0.8),
        record("paper-b", &"b".repeat(64), 0.9),
        record("paper-c", &"c".repeat(64), 0.7),
    ];
    let validation = vec![record("paper-v", &"d".repeat(64), 0.75)];
    let first = train_microlora_candidate(
        &mut store,
        &train,
        &validation,
        lineage(),
        None,
        TrainingConfig::default(),
        Some(&dir.path().join("candidate_manifest.json")),
    )
    .unwrap();
    let digest = store.verify().unwrap().ordered_digest;
    assert_eq!(
        first.manifest.receipt.promotion_state,
        da_ports::PromotionState::Candidate
    );
    assert!(!first.manifest.receipt.import_eligible);
    assert!(!first.manifest.receipt.deployed);
    assert!(first.manifest.receipt.conservation_ok);
    assert_eq!(first.manifest.train_count, 3);
    assert_eq!(first.manifest.validation_count, 1);
    drop(store);

    let mut reopened = RvfFileStore::open(path, lineage()).unwrap();
    assert_eq!(reopened.verify().unwrap().ordered_digest, digest);
    let records = reopened.read_all().unwrap();
    assert!(
        records
            .iter()
            .any(|r| matches!(r, da_ports::RvfRecord::ReasoningBank { .. }))
    );
    assert!(
        records
            .iter()
            .any(|r| matches!(r, da_ports::RvfRecord::Candidate { .. }))
    );
    let second = train_microlora_candidate(
        &mut reopened,
        &train,
        &validation,
        lineage(),
        None,
        TrainingConfig::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        first.manifest.receipt.candidate_id,
        second.manifest.receipt.candidate_id
    );
    assert_eq!(
        first.manifest.weights_digest,
        second.manifest.weights_digest
    );
    assert_eq!(
        reopened.read_all().unwrap().len(),
        2,
        "content-derived RVF retries are duplicates"
    );
}

#[test]
fn insufficient_or_duplicate_corpus_is_not_trainable() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = RvfFileStore::open(dir.path().join("experience.rvf"), lineage()).unwrap();
    let only = vec![
        record("paper-a", &"a".repeat(64), 0.8),
        record("paper-b", &"a".repeat(64), 0.7),
    ];
    let validation = vec![record("paper-v", &"b".repeat(64), 0.7)];
    assert!(matches!(
        train_microlora_candidate(
            &mut store,
            &only,
            &validation,
            lineage(),
            None,
            TrainingConfig::default(),
            None
        ),
        Err(da_application::TrainingError::DuplicateIdentity)
    ));
    assert!(matches!(
        train_microlora_candidate(
            &mut store,
            &[],
            &[],
            lineage(),
            None,
            TrainingConfig::default(),
            None
        ),
        Err(da_application::TrainingError::NotTrainable(_))
    ));
    assert_eq!(
        store.read_all().unwrap().len(),
        0,
        "validation fails before RVF I/O"
    );
}
