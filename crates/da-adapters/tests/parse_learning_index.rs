#![cfg(feature = "redb")]

use da_adapters::parse_experience_rvf::RvfFileStore;
use da_adapters::parse_learning_index::{LearningIndex, LearningIndexError};
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::parse_learning::{LearningLineage, RvfExperienceStore, RvfRecord, Sha256Digest};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn digest(seed: &str) -> Sha256Digest {
    Sha256Digest::new(hex::encode(Sha256::digest(seed.as_bytes()))).unwrap()
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
fn record(paper: &str) -> TrajectoryRecord {
    TrajectoryRecord {
        task_id: "parse_summary".into(),
        paper_id: paper.into(),
        model: "test-model".into(),
        prompt_sha256: "prompt-identity".into(),
        ok: true,
        latency_ms: 3,
        completion_tokens: None,
        grounding_hint: None,
        article_features: None,
        failure_pattern: None,
        construction_trace: None,
    }
}
fn store(path: PathBuf) -> RvfFileStore {
    RvfFileStore::open(path, lineage()).unwrap()
}

#[test]
fn rebuild_indexes_verified_rvf_and_restart_is_bound_to_canonical_digest() {
    let dir = tempfile::tempdir().unwrap();
    let rvf_path = dir.path().join("experience.rvf");
    let index_path = dir.path().join("index.redb");
    let mut rvf = store(rvf_path);
    rvf.append(RvfRecord::trajectory(record("paper-a")))
        .unwrap();
    rvf.append(RvfRecord::trajectory(record("paper-b")))
        .unwrap();
    let canonical = rvf.verify().unwrap();

    let receipt = LearningIndex::rebuild_from_rvf(&index_path, &rvf).unwrap();
    assert_eq!(receipt.experience_digest, canonical.ordered_digest);
    assert_eq!(receipt.event_count, 2);
    drop(rvf);

    let reopened = LearningIndex::open(&index_path).unwrap();
    assert!(reopened.bound_to(&canonical).unwrap());
    assert_eq!(reopened.receipt().unwrap(), Some(receipt));
    assert_eq!(
        reopened.event(0).unwrap(),
        Some(("parse_summary".into(), "paper-a".into()))
    );
    assert_eq!(
        reopened.event(1).unwrap(),
        Some(("parse_summary".into(), "paper-b".into()))
    );
}

#[test]
fn deleting_or_corrupting_index_does_not_change_rvf_and_rebuild_restores_it() {
    let dir = tempfile::tempdir().unwrap();
    let rvf_path = dir.path().join("experience.rvf");
    let index_path = dir.path().join("index.redb");
    let mut rvf = store(rvf_path.clone());
    rvf.append(RvfRecord::trajectory(record("paper-a")))
        .unwrap();
    let before = rvf.verify().unwrap();
    LearningIndex::rebuild_from_rvf(&index_path, &rvf).unwrap();
    drop(rvf);

    std::fs::remove_file(&index_path).unwrap();
    let rvf = store(rvf_path.clone());
    assert_eq!(rvf.verify().unwrap(), before);
    assert!(LearningIndex::rebuild_from_rvf(&index_path, &rvf).is_ok());
    drop(rvf);

    std::fs::write(&index_path, b"not redb").unwrap();
    assert!(matches!(
        LearningIndex::open(&index_path),
        Err(LearningIndexError::OpenFailed)
    ));
    let rvf = store(rvf_path);
    assert_eq!(
        LearningIndex::rebuild_from_rvf(&index_path, &rvf)
            .unwrap()
            .experience_digest,
        before.ordered_digest
    );
    assert!(
        LearningIndex::open(&index_path)
            .unwrap()
            .bound_to(&before)
            .unwrap()
    );
}

#[test]
fn missing_or_empty_canonical_rvf_cannot_produce_standalone_report() {
    let dir = tempfile::tempdir().unwrap();
    let rvf_path = dir.path().join("experience.rvf");
    let index_path = dir.path().join("index.redb");
    let rvf = store(rvf_path);
    assert!(matches!(
        LearningIndex::rebuild_from_rvf(&index_path, &rvf),
        Err(LearningIndexError::CanonicalUnavailable)
    ));
    assert!(!index_path.exists());
}

#[test]
fn rebuild_replaces_stale_index_only_after_new_canonical_verification() {
    let dir = tempfile::tempdir().unwrap();
    let rvf_path = dir.path().join("experience.rvf");
    let index_path = dir.path().join("index.redb");
    let mut rvf = store(rvf_path);
    rvf.append(RvfRecord::trajectory(record("paper-a")))
        .unwrap();
    let first = LearningIndex::rebuild_from_rvf(&index_path, &rvf).unwrap();
    rvf.append(RvfRecord::trajectory(record("paper-b")))
        .unwrap();
    let second = LearningIndex::rebuild_from_rvf(&index_path, &rvf).unwrap();
    assert_ne!(first.experience_digest, second.experience_digest);
    let index = LearningIndex::open(index_path).unwrap();
    assert!(index.bound_to(&rvf.verify().unwrap()).unwrap());
    assert_eq!(
        index.event(1).unwrap(),
        Some(("parse_summary".into(), "paper-b".into()))
    );
}
