use da_adapters::parse_experience_rvf::RvfFileStore;
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::parse_learning::{
    LearningLineage, RvfExperienceStore, RvfRecord, RvfStoreError, Sha256Digest,
};
use std::path::PathBuf;

fn digest(seed: &str) -> Sha256Digest {
    use sha2::{Digest, Sha256};
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
        completion_tokens: Some(8),
        grounding_hint: Some(0.9),
        article_features: None,
        failure_pattern: None,
        construction_trace: None,
    }
}

fn store(path: PathBuf) -> RvfFileStore {
    RvfFileStore::open(path, lineage()).unwrap()
}

#[test]
fn restart_preserves_ordered_digest_and_checkpoint_lineage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("experience.rvf");
    let mut first = store(path.clone());
    first
        .append(RvfRecord::trajectory(record("paper-a")))
        .unwrap();
    first
        .append(RvfRecord::trajectory(record("paper-b")))
        .unwrap();
    let before = first.verify().unwrap();
    let checkpoint = first.checkpoint().unwrap();
    assert_eq!(checkpoint.experience_digest, before.ordered_digest);

    let mut reopened = store(path);
    assert_eq!(
        reopened.verify().unwrap().ordered_digest,
        before.ordered_digest
    );
    reopened
        .append(RvfRecord::trajectory(record("paper-c")))
        .unwrap();
    assert_eq!(reopened.read_all().unwrap().len(), 3);
    assert_ne!(
        reopened.verify().unwrap().ordered_digest,
        before.ordered_digest
    );
    let next = reopened.checkpoint().unwrap();
    assert_eq!(next.previous_checkpoint, Some(before.ordered_digest));
}

#[test]
fn append_is_content_idempotent_and_migration_is_repeatable() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("trajectory.jsonl");
    let first = serde_json::to_string(&record("paper-a")).unwrap();
    let second = serde_json::to_string(&record("paper-b")).unwrap();
    std::fs::write(&legacy, format!("{first}\n{second}\n")).unwrap();
    let legacy_before = std::fs::read(&legacy).unwrap();

    let path = dir.path().join("experience.rvf");
    let mut rvf = store(path.clone());
    assert_eq!(rvf.migrate_legacy_jsonl(&legacy).unwrap(), 2);
    assert_eq!(rvf.migrate_legacy_jsonl(&legacy).unwrap(), 0);
    let duplicate = rvf
        .append(RvfRecord::trajectory(record("paper-a")))
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(rvf.read_all().unwrap().len(), 2);
    assert_eq!(std::fs::read(&legacy).unwrap(), legacy_before);

    let reopened = store(path);
    assert_eq!(reopened.verify().unwrap().event_count, 2);
}

#[test]
fn malformed_or_unknown_frames_fail_closed_before_append() {
    let dir = tempfile::tempdir().unwrap();
    let truncated = dir.path().join("truncated.rvf");
    std::fs::write(&truncated, b"RVF1\0\0\0\x10short").unwrap();
    assert!(matches!(
        RvfFileStore::open(truncated, lineage()),
        Err(RvfStoreError::Truncated { .. })
    ));

    let unknown = dir.path().join("unknown.rvf");
    std::fs::write(&unknown, b"NOPE\0\0\0\x01x").unwrap();
    assert!(matches!(
        RvfFileStore::open(unknown, lineage()),
        Err(RvfStoreError::UnknownRecord)
    ));

    let invalid_json = dir.path().join("corrupt.rvf");
    std::fs::write(&invalid_json, b"RVF1\0\0\0\x03{}!").unwrap();
    assert!(matches!(
        RvfFileStore::open(invalid_json, lineage()),
        Err(RvfStoreError::Corrupt { .. })
    ));
}

#[test]
fn agent_memory_writes_rvf_not_legacy_jsonl() {
    use da_adapters::agent_memory::CvfAgentMemory;
    use da_ports::agent_memory::AgentMemory;
    let dir = tempfile::tempdir().unwrap();
    let mut memory = CvfAgentMemory::open(dir.path().to_path_buf()).unwrap();
    memory.record_trajectory(&record("paper-a")).unwrap();
    memory.persist().unwrap();
    assert!(dir.path().join("experience.rvf").is_file());
    assert!(dir.path().join("experience.checkpoint.json").is_file());
    assert!(!dir.path().join("trajectory.jsonl").exists());
    assert_eq!(memory.read_trajectories().unwrap().len(), 1);
    assert!(memory.verify());
}
