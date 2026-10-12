//! Closed train → persist → reload → independent-eval cycle over the real frozen
//! corpus (M514-1lss2s/S07/T03).
//!
//! The test binds the committed `artifacts/parse_learning/learning_corpus.json`
//! to the real candidate path: the candidate is trained, persisted, reloaded
//! (weight digest verified), applied through the vendor MicroLoRA forward pass
//! and evaluated against an independent frozen reference producer. No network
//! access and no gitignored `data/sources/**` read.

use da_adapters::RvfFileStore;
use da_application::{
    ContaminationAssessment, FrozenLearningCorpus, LearningOutcome, ProvenanceBasis,
    learning_lineage_for_corpus, reload_persisted_candidate, run_learning_cycle,
    validate_learning_cycle_report,
};
use da_ports::RvfExperienceStore;
use std::path::{Path, PathBuf};

/// Committed frozen corpus written by
/// `cargo run -p da-cli --example parse_learning_corpus_freeze`.
const CORPUS_PATH: &str = "../../artifacts/parse_learning/learning_corpus.json";

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn load() -> FrozenLearningCorpus {
    let path = repo_path(CORPUS_PATH);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "read frozen corpus {}: {error}; run `cargo run -p da-cli --example parse_learning_corpus_freeze`",
            path.display()
        )
    });
    serde_json::from_slice(&bytes).expect("frozen corpus deserializes")
}

#[test]
fn cycle_trains_persists_reloads_and_evaluates_on_frozen_corpus() {
    let corpus = load();
    let dir = tempfile::tempdir().unwrap();
    let candidate_path = dir.path().join("candidate_cycle.json");
    let lineage = learning_lineage_for_corpus(&corpus);
    let mut store = RvfFileStore::open(dir.path().join("experience.rvf"), lineage.clone()).unwrap();

    let report = run_learning_cycle(&corpus, &mut store, lineage, &candidate_path)
        .expect("cycle runs over the real corpus");

    validate_learning_cycle_report(&report, &corpus).expect("report validates against the corpus");

    // Exact same-n denominators, and a real measured outcome.
    assert_eq!(report.baseline_n, report.candidate_n);
    assert!(
        report.baseline_n > 0,
        "the frozen corpus has validation documents"
    );
    assert_eq!(report.producer_independent, Some(true));
    assert_eq!(report.provenance_basis, Some(ProvenanceBasis::Explicit));
    assert_eq!(
        report.contamination_assessment,
        ContaminationAssessment::NotDetected
    );
    assert!(report.train_eval_document_intersection.is_empty());

    // Candidate-only locks are closed.
    assert!(!report.deployed);
    assert!(!report.import_eligible);
    assert_eq!(report.promotion_state, "candidate");
    assert_eq!(report.auto_promote, "forbidden");

    // Measured conservation, not an assertion.
    assert!(!report.conservation.veto);
    assert_eq!(
        report.conservation.corpus_digest_before,
        corpus.corpus_digest
    );
    assert_eq!(
        report.conservation.corpus_digest_after,
        corpus.corpus_digest
    );
    assert_eq!(
        report.conservation.rvf_ordered_digest_after_training,
        report.conservation.rvf_replay_digest
    );

    assert!(matches!(
        report.outcome,
        LearningOutcome::MeasuredGain | LearningOutcome::MeasuredNoGain
    ));
    assert_eq!(report.recomputed_report_digest(), report.report_digest);
    assert!(report.candidate_id.is_some());
    assert!(report.weights_digest.is_some());

    // The persisted candidate is a genuinely reloadable artifact whose weights
    // still match the digest the report carries.
    let reloaded = reload_persisted_candidate(&candidate_path).expect("candidate reloads");
    assert_eq!(
        reloaded.manifest.weights_digest,
        report.weights_digest.clone().unwrap()
    );

    // The RVF journal holds the deterministic training receipts.
    let records = store.read_all().unwrap();
    assert!(
        records.len() >= 2,
        "training must append a bank and a candidate receipt, got {}",
        records.len()
    );
}

#[test]
fn cycle_is_deterministic() {
    let corpus = load();
    let dir = tempfile::tempdir().unwrap();

    let run = |tag: &str| {
        let lineage = learning_lineage_for_corpus(&corpus);
        let mut store = RvfFileStore::open(
            dir.path().join(format!("experience-{tag}.rvf")),
            lineage.clone(),
        )
        .unwrap();
        run_learning_cycle(
            &corpus,
            &mut store,
            lineage,
            &dir.path().join(format!("candidate-{tag}.json")),
        )
        .unwrap()
    };

    let first = run("a");
    let second = run("b");

    assert_eq!(first.candidate_id, second.candidate_id);
    assert_eq!(first.weights_digest, second.weights_digest);
    assert_eq!(first.report_digest, second.report_digest);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap(),
        "two runs over the same corpus must be byte-identical"
    );
}

/// The vendor ReasoningBank stamps every learned pattern with wall-clock
/// seconds, so a cycle run must stay byte-identical even when it straddles a
/// second boundary. `cycle_is_deterministic` exercises the same invariant but
/// only flakily (its two runs usually share a second); the explicit sleep below
/// makes the wall-clock dependency deterministic, so this test fails whenever a
/// volatile field leaks back into the receipt (M516/S13/T05).
#[test]
fn cycle_report_digest_is_stable_across_a_second_boundary() {
    let corpus = load();
    let dir = tempfile::tempdir().unwrap();

    let run = |tag: &str| {
        let lineage = learning_lineage_for_corpus(&corpus);
        let mut store = RvfFileStore::open(
            dir.path().join(format!("boundary-{tag}.rvf")),
            lineage.clone(),
        )
        .unwrap();
        run_learning_cycle(
            &corpus,
            &mut store,
            lineage,
            &dir.path().join(format!("boundary-candidate-{tag}.json")),
        )
        .unwrap()
    };

    let first = run("a");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let second = run("b");

    assert_eq!(first.report_digest, second.report_digest);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap(),
        "a wall-clock second boundary must not change the receipt"
    );
}
