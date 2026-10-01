//! Contract test for the frozen, document-disjoint parse-learning corpus
//! (M514-1lss2s/S07/T01). It loads the committed artifact, proves the split is
//! document-disjoint and non-vacuous, and drives every named negative rule.

use da_application::{
    DOCUMENT_IDENTITY_FIELD, FrozenLearningCorpus, GAIN_RULE_STATE_FROZEN, LearningCorpusError,
    PROMPT_IDENTITY_ROLE_LINEAGE, SPLIT_RULE_DOCUMENT_RANK_MODULO,
};
use da_ports::Sha256Digest;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Committed frozen corpus written by
/// `cargo run -p da-cli --example parse_learning_corpus_freeze`.
const CORPUS_PATH: &str = "../../artifacts/parse_learning/learning_corpus.json";
/// The committed trajectory log the corpus was frozen from.
const SOURCE_PATH: &str = "../../data/agent_memory/summarizer/trajectory.jsonl";

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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).expect("valid digest")
}

#[test]
fn committed_corpus_is_valid_and_non_vacuous() {
    let corpus = load();
    corpus.validate().expect("committed corpus validates");

    assert_eq!(corpus.schema_version, "parse-learning-corpus.v1");
    assert_eq!(corpus.document_identity_field, DOCUMENT_IDENTITY_FIELD);
    assert_eq!(corpus.prompt_identity_role, PROMPT_IDENTITY_ROLE_LINEAGE);
    assert_eq!(corpus.split_rule, SPLIT_RULE_DOCUMENT_RANK_MODULO);
    assert_eq!(corpus.gain_rule.state, GAIN_RULE_STATE_FROZEN);

    // Both sides carry real documents (the freeze must not be a token split).
    assert!(
        corpus.train.len() >= 2,
        "training side has {} documents",
        corpus.train.len()
    );
    assert!(
        corpus.validation.len() >= 2,
        "validation side has {} documents",
        corpus.validation.len()
    );

    // Document-disjointness: no identity appears on both sides or twice.
    let mut documents = BTreeSet::new();
    for group in corpus.train.iter().chain(corpus.validation.iter()) {
        assert!(
            documents.insert(group.document_identity.as_str()),
            "document {} appears more than once",
            group.document_identity
        );
    }
}

#[test]
fn prompt_identity_is_retained_only_as_lineage() {
    let corpus = load();
    // The canonical corpus has one prompt identity repeated over every record,
    // which is exactly what the split must not key on. The freezer records the
    // duplicates as lineage rather than collapsing them.
    assert_eq!(
        corpus.source_path,
        "data/agent_memory/summarizer/trajectory.jsonl"
    );
    let repeated = corpus
        .train
        .iter()
        .chain(corpus.validation.iter())
        .find(|group| group.records.len() > 1)
        .expect("corpus has a document with more than one record");
    assert_eq!(
        repeated.prompt_identities.len(),
        repeated.records.len(),
        "every record's prompt identity is retained in lineage"
    );
    let distinct: BTreeSet<_> = repeated
        .prompt_identities
        .iter()
        .map(|identity| identity.as_str())
        .collect();
    assert_eq!(
        distinct.len(),
        1,
        "duplicated prompt identities must survive as repeated lineage"
    );
}

#[test]
fn corpus_digest_and_source_digest_are_recomputed_from_content() {
    let corpus = load();

    // The recorded source digest is bound to the committed source bytes.
    let source = std::fs::read(repo_path(SOURCE_PATH)).expect("read trajectory source");
    assert_eq!(
        corpus.source_sha256.as_str(),
        sha256_hex(&source),
        "source_sha256 must be the digest of the committed trajectory log"
    );

    // The corpus digest and the nested reward digest are content-derived, not
    // asserted; recomputing them from the loaded artifact must match.
    assert_eq!(
        corpus.corpus_digest,
        corpus.recomputed_corpus_digest(),
        "corpus_digest must match the recomputed content digest"
    );
    assert_eq!(
        corpus.reward_definition.digest,
        corpus.reward_definition.recomputed_digest(),
        "reward digest must match the recomputed content digest"
    );
    assert_eq!(corpus.reward_definition.version, "reward-v1");
    assert_eq!(
        corpus.reward_definition.components,
        vec!["grounding_hint".to_string(), "ok_indicator".to_string()]
    );
}

#[test]
fn a_document_on_both_sides_is_rejected() {
    let mut corpus = load();
    let duplicate = corpus.train[0].clone();
    corpus.validation.push(duplicate);
    corpus = corpus.seal();
    assert_eq!(corpus.validate(), Err(LearningCorpusError::DocumentOverlap));
}

#[test]
fn an_empty_side_is_rejected() {
    let mut corpus = load();
    corpus.train.clear();
    corpus = corpus.seal();
    assert_eq!(corpus.validate(), Err(LearningCorpusError::EmptySide));
}

#[test]
fn a_blank_document_identity_is_rejected() {
    let mut corpus = load();
    corpus.train[0].document_identity = "  ".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::MissingDocumentIdentity)
    );
}

#[test]
fn a_record_naming_a_foreign_document_is_rejected() {
    let mut corpus = load();
    corpus.train[0].records[0].document_identity = "not-the-group".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::DocumentIdentityMismatch)
    );
}

#[test]
fn a_prompt_keyed_split_rule_is_rejected() {
    let mut corpus = load();
    corpus.split_rule = "prompt_sha256_rank_modulo;validation_every=4".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::PromptIdentityUsedAsSplitKey)
    );
}

#[test]
fn a_prompt_identity_document_field_is_rejected() {
    let mut corpus = load();
    corpus.document_identity_field = "prompt_sha256".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::PromptIdentityUsedAsSplitKey)
    );
}

#[test]
fn a_non_lineage_prompt_role_is_rejected() {
    let mut corpus = load();
    corpus.prompt_identity_role = "split_key".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::PromptIdentityUsedAsSplitKey)
    );
}

#[test]
fn a_tampered_corpus_digest_is_rejected() {
    let mut corpus = load();
    corpus.corpus_digest = digest('c');
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::CorpusDigestMismatch)
    );
}

#[test]
fn a_tampered_reward_digest_is_rejected() {
    let mut corpus = load();
    corpus.reward_definition.digest = digest('d');
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::RewardDefinitionMismatch)
    );
}

#[test]
fn an_unfrozen_gain_rule_is_rejected() {
    let mut corpus = load();
    corpus.gain_rule.state = "proposed".into();
    corpus = corpus.seal();
    assert_eq!(
        corpus.validate(),
        Err(LearningCorpusError::GainRuleNotFrozen)
    );
}
