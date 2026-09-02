use da_application::{
    AblationReport, CriticObservation, CriticStatus, FrozenEvalCase, aggregate_same_n_ablation,
};
use da_ports::{
    CandidateManifest, CandidateReceipt, LearningLineage, PromotionState, Sha256Digest,
};

fn d(c: char) -> Sha256Digest {
    Sha256Digest::new(c.to_string().repeat(64)).unwrap()
}
fn candidate() -> CandidateManifest {
    CandidateManifest {
        receipt: CandidateReceipt {
            schema_version: 1,
            candidate_id: d('a'),
            lineage: LearningLineage {
                source: d('b'),
                corpus: d('c'),
                config: d('d'),
                prior_policy: d('e'),
                conservation: d('f'),
                split: d('1'),
            },
            prior_candidate: Some(d('9')),
            conservation_ok: true,
            import_eligible: false,
            promotion_state: PromotionState::Candidate,
            deployed: false,
        },
        rank: 1,
        hidden_dim: 8,
        train_count: 2,
        validation_count: 1,
        train_quality: 0.8,
        validation_quality: 0.7,
        validation_gap: 0.1,
        weights_digest: d('2'),
    }
}
fn cases() -> Vec<FrozenEvalCase> {
    (0..3)
        .map(|i| FrozenEvalCase {
            case_id: format!("c{i}"),
            corpus: d('c'),
            order: i,
            expected_digest: d('e'),
        })
        .collect()
}
fn rows(p: &str) -> Vec<CriticObservation> {
    (0..3)
        .map(|i| CriticObservation {
            schema_version: 1,
            case_id: format!("c{i}"),
            corpus: d('c'),
            order: i,
            producer: p.into(),
            metric_one: 0.8,
            metric_two: 0.7,
            status: CriticStatus::Ok,
        })
        .collect()
}

#[test]
fn two_independent_offline_aggregations_are_byte_equal() {
    let c = cases();
    let b = rows("ruvllm-critic-a");
    let x = rows("ruvllm-critic-b");
    let one = aggregate_same_n_ablation(&candidate(), &c, &b, &x, true, "sona-trainer").unwrap();
    let two = aggregate_same_n_ablation(&candidate(), &c, &b, &x, true, "sona-trainer").unwrap();
    assert_eq!(
        serde_json::to_vec(&one).unwrap(),
        serde_json::to_vec(&two).unwrap()
    );
    assert_eq!(one.baseline_n, one.candidate_n);
    assert!(one.baseline_n > 0);
    assert!(one.producer_independent && !one.contamination_detected && !one.conservation_veto);
    assert_eq!(one.promotion_state, "candidate");
    assert!(!one.deployed && !one.import_eligible && one.explicit_decision.is_none());
}

#[test]
fn malformed_missing_and_lock_mutations_fail_closed() {
    let c = cases();
    let b = rows("critic-a");
    let mut x = rows("critic-b");
    x[1].status = CriticStatus::Malformed;
    assert!(
        aggregate_same_n_ablation(&candidate(), &c, &b[..2], &x, true, "sona-trainer").is_err()
    );
    assert!(aggregate_same_n_ablation(&candidate(), &c, &b, &x, false, "sona-trainer").is_err());
    let mut forged = candidate();
    forged.receipt.deployed = true;
    assert!(aggregate_same_n_ablation(&forged, &c, &b, &x, true, "sona-trainer").is_err());
}

#[test]
fn timeout_remains_in_denominator() {
    let c = cases();
    let b = rows("critic-a");
    let mut x = rows("critic-b");
    x[2].status = CriticStatus::Timeout;
    let report: AblationReport =
        aggregate_same_n_ablation(&candidate(), &c, &b, &x, true, "sona-trainer").unwrap();
    assert_eq!(report.candidate_n, 3);
    assert_eq!(report.critic_failures, 1);
}
