//! M464/S17/T01: typed learning wire contracts and CLI-owned storage config.
use std::collections::HashMap;
use std::path::Path;

use da_cli::parse_config::{
    DA_PARSE_LEARNING_REDB_INDEX_PATH_KEY, DA_PARSE_LEARNING_RVF_PATH_KEY, ParseConfigError,
    ParseRuntimeOverrides, resolve,
};
use da_ports::{
    CVF_SCHEMA_VERSION, CandidateReceipt, CvfEvent, LearningContractError, LearningLineage,
    LearningStorageConfig, PromotionState, Sha256Digest, SplitIdentity,
};

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).unwrap()
}
fn lineage() -> LearningLineage {
    LearningLineage {
        source: digest('a'),
        corpus: digest('b'),
        config: digest('c'),
        prior_policy: digest('d'),
        conservation: digest('e'),
        split: digest('f'),
    }
}

#[test]
fn typed_cvf_and_candidate_roundtrip_without_raw_payloads() {
    let event = CvfEvent {
        schema_version: CVF_SCHEMA_VERSION,
        event_id: digest('1'),
        sequence: 3,
        lineage: lineage(),
        split: SplitIdentity {
            train: vec![digest('2')],
            validation: vec![digest('3')],
        },
        evidence_digest: digest('4'),
    };
    event.validate().unwrap();
    let json = serde_json::to_string(&event).unwrap();
    let back: CvfEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
    assert!(!json.contains("prompt"));

    let candidate = CandidateReceipt {
        schema_version: 1,
        candidate_id: digest('5'),
        lineage: lineage(),
        prior_candidate: None,
        conservation_ok: true,
        import_eligible: false,
        promotion_state: PromotionState::Candidate,
        deployed: false,
    };
    candidate.validate().unwrap();
}

#[test]
fn malformed_digest_split_overlap_raw_content_and_promotion_fail_closed() {
    assert!(matches!(
        Sha256Digest::new("not-a-hash"),
        Err(LearningContractError::InvalidDigest)
    ));
    let overlap = SplitIdentity {
        train: vec![digest('a')],
        validation: vec![digest('a')],
    };
    assert!(matches!(
        overlap.validate(),
        Err(LearningContractError::SplitOverlap)
    ));
    assert!(matches!(
        da_ports::reject_raw_content("<tei>secret"),
        Err(LearningContractError::RawContentForbidden)
    ));
    let mut candidate = CandidateReceipt {
        schema_version: 1,
        candidate_id: digest('5'),
        lineage: lineage(),
        prior_candidate: None,
        conservation_ok: true,
        import_eligible: true,
        promotion_state: PromotionState::Candidate,
        deployed: false,
    };
    assert!(matches!(
        candidate.validate(),
        Err(LearningContractError::PromotionLockViolation)
    ));
    candidate.import_eligible = false;
    candidate.promotion_state = PromotionState::Candidate;
}

#[test]
fn learning_storage_rejects_zero_bounds_and_path_overlap_without_io() {
    let bad = LearningStorageConfig {
        rvf_path: "state/rvf".into(),
        redb_index_path: Some("state/rvf/index.redb".into()),
        rvf_capacity: 1,
        redb_capacity: 1,
    };
    assert!(matches!(
        bad.validate(),
        Err(LearningContractError::PathOverlap)
    ));
    let zero = LearningStorageConfig {
        rvf_path: "state/rvf".into(),
        redb_index_path: None,
        rvf_capacity: 0,
        redb_capacity: 1,
    };
    assert!(matches!(
        zero.validate(),
        Err(LearningContractError::InvalidBound)
    ));
}

#[test]
fn cli_learning_paths_are_typed_and_precede_environment() {
    let env = HashMap::from([
        (
            DA_PARSE_LEARNING_RVF_PATH_KEY.to_string(),
            "env/rvf".to_string(),
        ),
        (
            DA_PARSE_LEARNING_REDB_INDEX_PATH_KEY.to_string(),
            "env/index.redb".to_string(),
        ),
    ]);
    let overrides = ParseRuntimeOverrides {
        learning_rvf_path: Some("cli/rvf".into()),
        learning_redb_path: Some("cli/index.redb".into()),
        ..Default::default()
    };
    let config = resolve(&overrides, &|key| env.get(key).cloned()).unwrap();
    assert_eq!(config.learning_storage().rvf_path, Path::new("cli/rvf"));
    assert_eq!(
        config.learning_storage().redb_index_path.as_deref(),
        Some(Path::new("cli/index.redb"))
    );
}

#[test]
fn cli_learning_path_overlap_is_typed_error() {
    let overrides = ParseRuntimeOverrides {
        learning_rvf_path: Some("state/rvf".into()),
        learning_redb_path: Some("state/rvf/../rvf/index.redb".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve(&overrides, &|_| None).unwrap_err(),
        ParseConfigError::LearningPathOverlaps
    );
}
