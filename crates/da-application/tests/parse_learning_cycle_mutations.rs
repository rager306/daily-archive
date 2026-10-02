//! M514-1lss2s / S07 / T04 — the mutation matrix over the closed learning cycle.
//!
//! R142 requires mutation evidence for labels, weights, leakage and reward
//! drift. On a clean input a validator is indistinguishable from no validator,
//! so every guard is proven here by a NAMED mutation that flips one real input
//! and is caught by exactly the declared label. The registry below is DATA: each
//! id declares the exact error variant that must detect it. An id with no
//! applier, an applier with no declaration, or an applier caught by an
//! unclassified (`other`) label is a FAIL.
//!
//! All mutations are applied to in-memory clones of real inputs: the frozen
//! corpus (`artifacts/parse_learning/learning_corpus.json`), the candidate the
//! real cycle assembled from it, and the valid cycle report. Each applier
//! asserts its mutation really changed the serialized input, so a mutation that
//! is a no-op is a failure rather than a vacuous pass. On the unchanged inputs
//! the cycle and its validator produce zero errors and a non-empty report, so
//! "zero" is a real zero. Nothing is promoted, imported or written to a graph;
//! the test surface only reads, clones and (for `weight_tamper`) writes a
//! tampered candidate into a temporary directory.

use da_adapters::RvfFileStore;
use da_application::{
    CRITIC_SCHEMA_VERSION, CriticObservation, CriticStatus, EvaluationError, FrozenEvalCase,
    FrozenLearningCorpus, LearningCorpusError, LearningCycleError, LearningCycleReport,
    PersistedCandidate, ProducerProvenance, TrainingError,
    aggregate_same_n_ablation_with_provenance, learning_lineage_for_corpus,
    reload_persisted_candidate, run_learning_cycle, validate_learning_cycle_report,
};
use da_ports::Sha256Digest;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Committed frozen corpus written by
/// `cargo run -p da-cli --example parse_learning_corpus_freeze`.
const CORPUS_PATH: &str = "../../artifacts/parse_learning/learning_corpus.json";

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).expect("64 hex chars form a valid digest")
}

fn load_corpus() -> FrozenLearningCorpus {
    let path = repo_path(CORPUS_PATH);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "read frozen corpus {}: {error}; run `cargo run -p da-cli --example parse_learning_corpus_freeze`",
            path.display()
        )
    });
    serde_json::from_slice(&bytes).expect("frozen corpus deserializes")
}

// ───────────────────────────── the real inputs ──────────────────────────────

/// The three real inputs every mutation flips: the frozen corpus, the candidate
/// the real cycle assembled from it, and the valid cycle report. The cycle is
/// run exactly once and every applier clones from these values.
struct Fixture {
    corpus: FrozenLearningCorpus,
    report: LearningCycleReport,
    candidate: PersistedCandidate,
    dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let corpus = load_corpus();
    let dir = tempfile::tempdir().expect("tempdir");
    let candidate_path = dir.path().join("candidate.json");
    let lineage = learning_lineage_for_corpus(&corpus);
    let mut store =
        RvfFileStore::open(dir.path().join("experience.rvf"), lineage.clone()).expect("open store");
    let report = run_learning_cycle(&corpus, &mut store, lineage, &candidate_path)
        .expect("cycle runs over the real frozen corpus");
    validate_learning_cycle_report(&report, &corpus).expect("clean report validates");
    let candidate = reload_persisted_candidate(&candidate_path).expect("candidate reloads");
    Fixture {
        corpus,
        report,
        candidate,
        dir,
    }
}

// ───────────────────────────── shared helpers ───────────────────────────────

/// Assert the mutation really changed the serialized input. A mutation that
/// leaves the bytes identical is a no-op and fails.
fn assert_changed<T: Serialize>(before: &T, after: &T, id: &str) {
    let before = serde_json::to_vec(before).expect("mutation input serializes");
    let after = serde_json::to_vec(after).expect("mutated input serializes");
    assert_ne!(
        before, after,
        "{id}: mutation did not change the serialized input (no-op)"
    );
}

/// The frozen evaluation case set derived from the real validation documents, in
/// the same `sha256(document_identity)` order the cycle uses.
fn real_cases(corpus: &FrozenLearningCorpus) -> Vec<FrozenEvalCase> {
    let mut groups: Vec<_> = corpus.validation.iter().collect();
    groups.sort_by_key(|group| group.document_identity.clone());
    groups
        .iter()
        .enumerate()
        .map(|(order, group)| FrozenEvalCase {
            case_id: group.document_identity.clone(),
            corpus: corpus.corpus_digest.clone(),
            order: order as u32,
            expected_digest: digest('e'),
        })
        .collect()
}

fn real_rows(producer_label: &str, cases: &[FrozenEvalCase]) -> Vec<CriticObservation> {
    cases
        .iter()
        .map(|case| CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: case.case_id.clone(),
            corpus: case.corpus.clone(),
            order: case.order,
            producer: producer_label.into(),
            metric_one: 0.5,
            metric_two: 0.5,
            status: CriticStatus::Ok,
        })
        .collect()
}

fn producer(code: char, corpus: &Sha256Digest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest(code),
        input_digest: corpus.clone(),
        method: "mutation-fixture".into(),
        authority: "mutation-fixture".into(),
    }
}

fn trainer_producer(corpus: &Sha256Digest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest('7'),
        input_digest: corpus.clone(),
        method: "mutation-fixture-trainer".into(),
        authority: "mutation-fixture".into(),
    }
}

// ─────────────────────── typed-error → declared label ───────────────────────
//
// Each classifier maps the ACTUAL typed variant to the label the registry
// declares. Only the variants the mutation matrix can legitimately produce are
// named; every other variant falls through to `other`, which the harness treats
// as a hard error. That is deliberate: a new guard that starts answering an
// existing mutation with a different variant must fail this test, not pass it
// under a near-miss label.

fn label_of_corpus(error: &LearningCorpusError) -> &'static str {
    match error {
        LearningCorpusError::DocumentOverlap => "DocumentOverlap",
        LearningCorpusError::PromptIdentityUsedAsSplitKey => "PromptIdentityUsedAsSplitKey",
        LearningCorpusError::CorpusDigestMismatch => "CorpusDigestMismatch",
        LearningCorpusError::RewardDefinitionMismatch => "RewardDefinitionMismatch",
        _ => "other",
    }
}

fn label_of_training(error: &TrainingError) -> &'static str {
    match error {
        TrainingError::WeightsDigestMismatch => "WeightsDigestMismatch",
        _ => "other",
    }
}

fn label_of_evaluation(error: &EvaluationError) -> &'static str {
    match error {
        EvaluationError::ProducerNotIndependent => "ProducerNotIndependent",
        EvaluationError::EmptyEvaluation => "EmptyEvaluation",
        EvaluationError::DenominatorMismatch => "DenominatorMismatch",
        _ => "other",
    }
}

fn label_of_cycle(error: &LearningCycleError) -> &'static str {
    match error {
        LearningCycleError::RewardDefinitionMismatch => "RewardDefinitionMismatch",
        LearningCycleError::CandidateLockViolation => "CandidateLockViolation",
        LearningCycleError::Corpus(inner) => label_of_corpus(inner),
        LearningCycleError::Training(inner) => label_of_training(inner),
        LearningCycleError::Evaluation(inner) => label_of_evaluation(inner),
        _ => "other",
    }
}

// ───────────────────────────── the appliers ─────────────────────────────────
//
// An applier returns `Err(label)` when the mutation was detected by exactly that
// label, and `Ok(())` when it survived. `Ok` is always a hard failure.

/// Two half-arms carry different display labels but one `code_digest`. The
/// evaluator must refuse on machine identity, proving independence is not
/// derived from the labels.
fn m_label_only_independence(f: &Fixture) -> Result<(), &'static str> {
    let cases = real_cases(&f.corpus);
    let baseline = real_rows("baseline-display-label-a", &cases);
    let candidate_rows = real_rows("candidate-display-label-b", &cases);
    let corpus = f.corpus.corpus_digest.clone();

    let shared = producer('a', &corpus);
    let distinct = producer('c', &corpus);
    let trainer = trainer_producer(&corpus);

    // Anti-no-op: the clean form gives the two arms distinct code digests; the
    // mutation collapses them to one. Assert the serialized pair changed.
    let clean_pair = (shared.clone(), distinct.clone());
    let mutated_pair = (shared.clone(), shared.clone());
    assert_changed(&clean_pair, &mutated_pair, "label_only_independence");

    // Control: the same rows with distinct machine identities aggregate cleanly,
    // so the shared code digest is the only changed variable.
    aggregate_same_n_ablation_with_provenance(
        &f.candidate.manifest,
        &cases,
        &baseline,
        &candidate_rows,
        true,
        &shared,
        &distinct,
        &trainer,
    )
    .expect("control: distinct producer code digests must aggregate");

    match aggregate_same_n_ablation_with_provenance(
        &f.candidate.manifest,
        &cases,
        &baseline,
        &candidate_rows,
        true,
        &shared,
        &shared,
        &trainer,
    ) {
        Err(error) => Err(label_of_evaluation(&error)),
        Ok(_) => Ok(()),
    }
}

/// Flip one element of the persisted `up_projection`. The reload path recomputes
/// the weights digest from content and must reject the tampered artifact.
fn m_weight_tamper(f: &Fixture) -> Result<(), &'static str> {
    let before = f.candidate.clone();
    let mut tampered = f.candidate.clone();
    tampered.up_projection[0] += 1.0;
    assert_changed(&before, &tampered, "weight_tamper");

    let path = f.dir.path().join("candidate_weight_tamper.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&tampered).expect("tampered candidate serializes"),
    )
    .expect("write tampered candidate");
    match reload_persisted_candidate(&path) {
        Err(error) => Err(label_of_training(&error)),
        Ok(_) => Ok(()),
    }
}

/// Put a training document into validation as well, so one document identity
/// appears on both sides. The corpus validator must name the overlap.
fn m_document_leakage(f: &Fixture) -> Result<(), &'static str> {
    let before = f.corpus.clone();
    let mut mutated = f.corpus.clone();
    let leaked = mutated.train[0].clone();
    mutated.validation.push(leaked);
    assert_changed(&before, &mutated, "document_leakage");
    match mutated.validate() {
        Err(error) => Err(label_of_corpus(&error)),
        Ok(()) => Ok(()),
    }
}

/// Point the split rule at the prompt identity. The corpus validator must refuse
/// a prompt-keyed split even though the structural split is intact.
fn m_prompt_keyed_split(f: &Fixture) -> Result<(), &'static str> {
    let before = f.corpus.clone();
    let mut mutated = f.corpus.clone();
    mutated.split_rule = "prompt_sha256_rank_modulo;validation_every=4".into();
    assert_changed(&before, &mutated, "prompt_keyed_split");
    match mutated.validate() {
        Err(error) => Err(label_of_corpus(&error)),
        Ok(()) => Ok(()),
    }
}

/// Change the report's reward digest. The cycle validator must refuse a report
/// whose reward definition does not match the frozen corpus.
fn m_reward_definition_drift(f: &Fixture) -> Result<(), &'static str> {
    let before = f.report.clone();
    let mut mutated = f.report.clone();
    mutated.reward_definition_sha256 = digest('9');
    mutated.report_digest = mutated.recomputed_report_digest();
    assert_changed(&before, &mutated, "reward_definition_drift");
    match validate_learning_cycle_report(&mutated, &f.corpus) {
        Err(error) => Err(label_of_cycle(&error)),
        Ok(()) => Ok(()),
    }
}

/// Open the candidate-only lock by deploying the report. The validator must
/// refuse any candidate whose locks are not closed.
fn m_opened_lock(f: &Fixture) -> Result<(), &'static str> {
    let before = f.report.clone();
    let mut mutated = f.report.clone();
    mutated.deployed = true;
    mutated.report_digest = mutated.recomputed_report_digest();
    assert_changed(&before, &mutated, "opened_lock");
    match validate_learning_cycle_report(&mutated, &f.corpus) {
        Err(error) => Err(label_of_cycle(&error)),
        Ok(()) => Ok(()),
    }
}

/// Change a corpus record's content without touching the pin. The digest is
/// recomputed from content on validate, so the drift must be named.
fn m_corpus_digest_drift(f: &Fixture) -> Result<(), &'static str> {
    let before = f.corpus.clone();
    let mut mutated = f.corpus.clone();
    mutated.train[0].records[0].quality = Some(0.314_159_265);
    assert_changed(&before, &mutated, "corpus_digest_drift");
    match mutated.validate() {
        Err(error) => Err(label_of_corpus(&error)),
        Ok(()) => Ok(()),
    }
}

/// Empty the frozen evaluation set. An empty denominator has no measurement and
/// must be refused rather than reported as a zero.
fn m_vacuous_denominator(f: &Fixture) -> Result<(), &'static str> {
    let full = real_cases(&f.corpus);
    let empty: Vec<FrozenEvalCase> = Vec::new();
    assert_changed(&full, &empty, "vacuous_denominator");

    let corpus = f.corpus.corpus_digest.clone();
    let baseline: Vec<CriticObservation> = Vec::new();
    let candidate_rows: Vec<CriticObservation> = Vec::new();
    match aggregate_same_n_ablation_with_provenance(
        &f.candidate.manifest,
        &empty,
        &baseline,
        &candidate_rows,
        true,
        &producer('1', &corpus),
        &producer('2', &corpus),
        &trainer_producer(&corpus),
    ) {
        Err(error) => Err(label_of_evaluation(&error)),
        Ok(_) => Ok(()),
    }
}

/// Drop one candidate row so the two arms no longer share a denominator. The
/// evaluator must refuse the mismatched n.
fn m_denominator_mismatch(f: &Fixture) -> Result<(), &'static str> {
    let cases = real_cases(&f.corpus);
    let full_rows = real_rows("candidate-arm", &cases);
    let mut short_rows = full_rows.clone();
    short_rows.pop();
    assert_changed(&full_rows, &short_rows, "denominator_mismatch");

    let corpus = f.corpus.corpus_digest.clone();
    let baseline = real_rows("baseline-arm", &cases);
    match aggregate_same_n_ablation_with_provenance(
        &f.candidate.manifest,
        &cases,
        &baseline,
        &short_rows,
        true,
        &producer('1', &corpus),
        &producer('2', &corpus),
        &trainer_producer(&corpus),
    ) {
        Err(error) => Err(label_of_evaluation(&error)),
        Ok(_) => Ok(()),
    }
}

// ─────────────────────────── the DATA registry ──────────────────────────────

/// The declared mutation registry: `(id, expected_label)`. Each id names the
/// exact error variant that must detect it. `mutation_ids_match_declared_have_\
/// rows`, `every_declared_mutation_is_caught_by_its_declared_label` and the
/// applier registry must agree exactly.
const DECLARED: &[(&str, &str)] = &[
    ("label_only_independence", "ProducerNotIndependent"),
    ("weight_tamper", "WeightsDigestMismatch"),
    ("document_leakage", "DocumentOverlap"),
    ("prompt_keyed_split", "PromptIdentityUsedAsSplitKey"),
    ("reward_definition_drift", "RewardDefinitionMismatch"),
    ("opened_lock", "CandidateLockViolation"),
    ("corpus_digest_drift", "CorpusDigestMismatch"),
    ("vacuous_denominator", "EmptyEvaluation"),
    ("denominator_mismatch", "DenominatorMismatch"),
];

struct Arm {
    id: &'static str,
    apply: fn(&Fixture) -> Result<(), &'static str>,
}

const ARMS: &[Arm] = &[
    Arm {
        id: "label_only_independence",
        apply: m_label_only_independence,
    },
    Arm {
        id: "weight_tamper",
        apply: m_weight_tamper,
    },
    Arm {
        id: "document_leakage",
        apply: m_document_leakage,
    },
    Arm {
        id: "prompt_keyed_split",
        apply: m_prompt_keyed_split,
    },
    Arm {
        id: "reward_definition_drift",
        apply: m_reward_definition_drift,
    },
    Arm {
        id: "opened_lock",
        apply: m_opened_lock,
    },
    Arm {
        id: "corpus_digest_drift",
        apply: m_corpus_digest_drift,
    },
    Arm {
        id: "vacuous_denominator",
        apply: m_vacuous_denominator,
    },
    Arm {
        id: "denominator_mismatch",
        apply: m_denominator_mismatch,
    },
];

/// The declared label for one id, or a panic if the id is undeclared.
fn declared_label(id: &str) -> &'static str {
    DECLARED
        .iter()
        .find(|(declared_id, _)| *declared_id == id)
        .map(|(_, label)| *label)
        .unwrap_or_else(|| panic!("{id}: applier with no declaration in DECLARED"))
}

// ──────────────────────────────── the tests ─────────────────────────────────

/// The registry and the appliers must describe exactly the same set: an id with
/// no applier and an applier with no declaration are both failures.
#[test]
fn mutation_ids_match_declared_rows() {
    let declared: BTreeSet<&str> = DECLARED.iter().map(|(id, _)| *id).collect();
    let implemented: BTreeSet<&str> = ARMS.iter().map(|arm| arm.id).collect();
    assert_eq!(
        declared, implemented,
        "declared mutation ids and implemented appliers must be the same set"
    );
    for arm in ARMS {
        let _ = declared_label(arm.id);
    }
}

/// The unchanged inputs must produce zero errors and a non-empty report, so the
/// mutation harness is not measuring over a vacuous zero.
#[test]
fn clean_inputs_are_non_vacuous() {
    let f = fixture();

    validate_learning_cycle_report(&f.report, &f.corpus)
        .expect("clean report validates with zero errors");
    assert!(
        f.report.baseline_n >= 2,
        "the frozen corpus must yield a non-empty evaluation set, got baseline_n={}",
        f.report.baseline_n
    );
    assert_eq!(f.report.baseline_n, f.report.candidate_n);

    // The clean evaluator path over the real cases and rows also has zero errors.
    let cases = real_cases(&f.corpus);
    assert!(cases.len() >= 2, "the real validation set is non-empty");
    let corpus = f.corpus.corpus_digest.clone();
    let baseline = real_rows("baseline-producer", &cases);
    let candidate_rows = real_rows("candidate-producer", &cases);
    let clean = aggregate_same_n_ablation_with_provenance(
        &f.candidate.manifest,
        &cases,
        &baseline,
        &candidate_rows,
        true,
        &producer('1', &corpus),
        &producer('2', &corpus),
        &trainer_producer(&corpus),
    )
    .expect("clean evaluator inputs aggregate with zero errors");
    assert_eq!(clean.baseline_n as usize, cases.len());
    assert_eq!(clean.candidate_n as usize, cases.len());
    assert!(clean.producer_independent);
}

/// Every declared mutation is applied to a real input clone and is caught by
/// exactly the declared label. A surviving mutation, an `other` label, or a
/// mismatched label is a failure.
#[test]
fn every_declared_mutation_is_caught_by_its_declared_label() {
    let f = fixture();

    // The registry and the appliers must agree before anything is applied.
    let declared: BTreeSet<&str> = DECLARED.iter().map(|(id, _)| *id).collect();
    let implemented: BTreeSet<&str> = ARMS.iter().map(|arm| arm.id).collect();
    assert_eq!(declared, implemented);

    for arm in ARMS {
        let expected = declared_label(arm.id);
        match (arm.apply)(&f) {
            Ok(()) => panic!(
                "{}: mutation survived; it must be caught by {expected}",
                arm.id
            ),
            Err(actual) => {
                assert_ne!(
                    actual, "other",
                    "{}: caught by an unclassified (`other`) label; the declared label is {expected}",
                    arm.id
                );
                assert_eq!(
                    actual, expected,
                    "{}: detected by `{actual}`, declared `{expected}`",
                    arm.id
                );
            }
        }
    }
}
