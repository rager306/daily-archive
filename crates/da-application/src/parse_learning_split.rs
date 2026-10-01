//! Frozen, document-disjoint parse-learning corpus carrier (M514-1lss2s/S07/T01).
//!
//! This module is a pure, in-memory contract. It carries a frozen training
//! corpus whose split is keyed on **document identity**, never on the prompt
//! identity; prompt identities are retained only as lineage. Digests are always
//! recomputed from content and are never trusted as an input, so a tampered
//! artifact fails closed with a typed error that names the violated rule.
//!
//! It performs no I/O, names no TEI/Java type, writes no graph and reloads no
//! route. Provenance is *recorded* (`source_path` plus `source_sha256`) but file
//! access is the adapter's job, which keeps the module free of I/O.

use crate::parse_learning_evaluation::digest_value;
use da_ports::Sha256Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const LEARNING_CORPUS_SCHEMA_VERSION: &str = "parse-learning-corpus.v1";
/// The prompt identity is provenance only; it must never key the split.
pub const PROMPT_IDENTITY_ROLE_LINEAGE: &str = "lineage_only";
/// The canonical document identity field of the frozen source.
pub const DOCUMENT_IDENTITY_FIELD: &str = "paper_id";
/// Deterministic split: rank documents by `sha256(document_identity)` and send
/// every fourth document to validation.
pub const SPLIT_RULE_DOCUMENT_RANK_MODULO: &str =
    "document_identity_sha256_rank_modulo;validation_every=4";
pub const REWARD_VERSION_V1: &str = "reward-v1";
/// A gain rule is only usable once it is frozen before measurement.
pub const GAIN_RULE_STATE_FROZEN: &str = "frozen";

/// All-zero SHA-256 sentinel used while a digest is recomputed. This mirrors
/// `AblationReport::report_digest` so every evaluator in this crate seals a
/// digest the same way.
fn zero_digest() -> Sha256Digest {
    Sha256Digest::new("0".repeat(64)).expect("64 zero hex chars form a valid digest")
}

/// One measured observation belonging to a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentRecord {
    pub document_identity: String,
    /// Lineage only, never a split key.
    pub prompt_identity: Sha256Digest,
    /// Grounding quality as measured. `None` is typed missingness and is
    /// deliberately distinct from a measured `0.0`.
    #[serde(default)]
    pub quality: Option<f64>,
    pub ok: bool,
    pub latency_ms: u64,
    #[serde(default)]
    pub completion_tokens: Option<u32>,
}

/// All observations for one document plus the frozen prompt lineage.
///
/// `prompt_identities` intentionally preserves duplicates: the identity is
/// lineage, so repeated prompts are recorded rather than collapsed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentGroup {
    pub document_identity: String,
    pub prompt_identities: Vec<Sha256Digest>,
    pub records: Vec<DocumentRecord>,
}

/// A document-disjoint split. The split rule is carried verbatim so a reader can
/// see how the partition was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentGroupSplit {
    pub split_rule: String,
    pub train: Vec<DocumentGroup>,
    pub validation: Vec<DocumentGroup>,
}

impl DocumentGroupSplit {
    /// Reject an empty side, a duplicated document, a blank document identity,
    /// or a record that names a different document than its group.
    pub fn validate(&self) -> Result<(), LearningCorpusError> {
        validate_split(&self.train, &self.validation)
    }
}

/// The reward a candidate is measured against. Its digest is content-derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardDefinition {
    pub version: String,
    pub components: Vec<String>,
    pub digest: Sha256Digest,
}

impl RewardDefinition {
    /// Recompute the digest from `version` + `components`, ignoring the stored
    /// `digest` field entirely.
    pub fn recomputed_digest(&self) -> Sha256Digest {
        let mut copy = self.clone();
        copy.digest = zero_digest();
        digest_value(&copy)
    }

    /// Content-seal: set `digest` from the current content.
    pub fn seal(mut self) -> Self {
        self.digest = self.recomputed_digest();
        self
    }
}

/// The rule that decides `measured_gain` versus `measured_no_gain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GainRule {
    pub metric_one: String,
    pub metric_two: String,
    pub margin: f64,
    pub state: String,
}

/// The frozen learning corpus. `corpus_digest` is content-derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenLearningCorpus {
    pub schema_version: String,
    pub source_path: String,
    pub source_sha256: Sha256Digest,
    pub document_identity_field: String,
    pub prompt_identity_role: String,
    pub split_rule: String,
    pub reward_definition: RewardDefinition,
    pub gain_rule: GainRule,
    pub train: Vec<DocumentGroup>,
    pub validation: Vec<DocumentGroup>,
    pub corpus_digest: Sha256Digest,
}

impl FrozenLearningCorpus {
    /// Recompute the corpus digest from content, ignoring `corpus_digest`.
    pub fn recomputed_corpus_digest(&self) -> Sha256Digest {
        let mut copy = self.clone();
        copy.corpus_digest = zero_digest();
        digest_value(&copy)
    }

    /// Content-seal: set `corpus_digest` from the current content.
    pub fn seal(mut self) -> Self {
        self.corpus_digest = self.recomputed_corpus_digest();
        self
    }

    /// Fail-closed validation. Every rule has a named error so a caller can
    /// distinguish "no documents" from "prompt keyed the split".
    pub fn validate(&self) -> Result<(), LearningCorpusError> {
        // Structural rules first: they are the most specific diagnosis.
        validate_split(&self.train, &self.validation)?;
        if self.prompt_identity_role != PROMPT_IDENTITY_ROLE_LINEAGE
            || references_prompt_identity(&self.document_identity_field)
            || references_prompt_identity(&self.split_rule)
        {
            return Err(LearningCorpusError::PromptIdentityUsedAsSplitKey);
        }
        // The reward and gain rule are checked before the corpus digest so a
        // tampered sub-field reports its own name rather than a digest drift.
        if self.reward_definition.recomputed_digest() != self.reward_definition.digest {
            return Err(LearningCorpusError::RewardDefinitionMismatch);
        }
        if self.gain_rule.state != GAIN_RULE_STATE_FROZEN {
            return Err(LearningCorpusError::GainRuleNotFrozen);
        }
        if self.recomputed_corpus_digest() != self.corpus_digest {
            return Err(LearningCorpusError::CorpusDigestMismatch);
        }
        Ok(())
    }
}

/// A split keys on a prompt identity when the field name or the split rule
/// mentions "prompt". The canonical rule and field do not, so this is an
/// exact, non-heuristic guard against re-introducing `prompt_sha256` as a key.
fn references_prompt_identity(field: &str) -> bool {
    field.to_ascii_lowercase().contains("prompt")
}

fn validate_split(
    train: &[DocumentGroup],
    validation: &[DocumentGroup],
) -> Result<(), LearningCorpusError> {
    if train.is_empty() || validation.is_empty() {
        return Err(LearningCorpusError::EmptySide);
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for group in train.iter().chain(validation.iter()) {
        if group.document_identity.trim().is_empty() {
            return Err(LearningCorpusError::MissingDocumentIdentity);
        }
        if !seen.insert(group.document_identity.as_str()) {
            return Err(LearningCorpusError::DocumentOverlap);
        }
        for record in &group.records {
            if record.document_identity.trim().is_empty() {
                return Err(LearningCorpusError::MissingDocumentIdentity);
            }
            if record.document_identity != group.document_identity {
                return Err(LearningCorpusError::DocumentIdentityMismatch);
            }
        }
    }
    Ok(())
}

/// Typed, named validation failures. Every variant is a distinct corpus rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningCorpusError {
    /// A document appears in both sides, or twice in one side.
    DocumentOverlap,
    /// At least one side of the split is empty.
    EmptySide,
    /// A document identity is blank.
    MissingDocumentIdentity,
    /// A record names a different document than its group.
    DocumentIdentityMismatch,
    /// The split is keyed on prompt identity (or the prompt role is not lineage only).
    PromptIdentityUsedAsSplitKey,
    /// `corpus_digest` does not match the content-derived digest.
    CorpusDigestMismatch,
    /// `reward_definition.digest` does not match the content-derived digest.
    RewardDefinitionMismatch,
    /// The gain rule was not frozen before measurement.
    GainRuleNotFrozen,
}

impl std::fmt::Display for LearningCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::DocumentOverlap => "a document appears twice or in both split sides",
            Self::EmptySide => "a split side is empty",
            Self::MissingDocumentIdentity => "a document identity is blank",
            Self::DocumentIdentityMismatch => "a record names a different document than its group",
            Self::PromptIdentityUsedAsSplitKey => {
                "the split is keyed on prompt identity instead of document identity"
            }
            Self::CorpusDigestMismatch => "the corpus digest does not match its content",
            Self::RewardDefinitionMismatch => "the reward digest does not match its content",
            Self::GainRuleNotFrozen => "the gain rule is not frozen",
        };
        f.write_str(message)
    }
}

impl std::error::Error for LearningCorpusError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).expect("valid digest")
    }

    fn record(document: &str) -> DocumentRecord {
        DocumentRecord {
            document_identity: document.into(),
            prompt_identity: digest('a'),
            quality: Some(0.5),
            ok: true,
            latency_ms: 10,
            completion_tokens: Some(3),
        }
    }

    fn group(document: &str) -> DocumentGroup {
        DocumentGroup {
            document_identity: document.into(),
            prompt_identities: vec![digest('a')],
            records: vec![record(document)],
        }
    }

    fn reward() -> RewardDefinition {
        RewardDefinition {
            version: REWARD_VERSION_V1.into(),
            components: vec!["grounding_hint".into(), "ok_indicator".into()],
            digest: zero_digest(),
        }
        .seal()
    }

    fn corpus() -> FrozenLearningCorpus {
        FrozenLearningCorpus {
            schema_version: LEARNING_CORPUS_SCHEMA_VERSION.into(),
            source_path: "data/agent_memory/summarizer/trajectory.jsonl".into(),
            source_sha256: digest('b'),
            document_identity_field: DOCUMENT_IDENTITY_FIELD.into(),
            prompt_identity_role: PROMPT_IDENTITY_ROLE_LINEAGE.into(),
            split_rule: SPLIT_RULE_DOCUMENT_RANK_MODULO.into(),
            reward_definition: reward(),
            gain_rule: GainRule {
                metric_one: "grounding_hint_mean".into(),
                metric_two: "ok_rate".into(),
                margin: 0.0,
                state: GAIN_RULE_STATE_FROZEN.into(),
            },
            train: vec![group("doc-a")],
            validation: vec![group("doc-b")],
            corpus_digest: zero_digest(),
        }
        .seal()
    }

    #[test]
    fn a_sealed_corpus_validates() {
        corpus().validate().expect("sealed corpus is valid");
    }

    #[test]
    fn a_duplicated_document_is_rejected() {
        let mut value = corpus();
        value.validation.push(group("doc-a"));
        value = value.seal();
        assert_eq!(value.validate(), Err(LearningCorpusError::DocumentOverlap));
    }

    #[test]
    fn an_empty_side_is_rejected() {
        let mut value = corpus();
        value.train.clear();
        value = value.seal();
        assert_eq!(value.validate(), Err(LearningCorpusError::EmptySide));
    }

    #[test]
    fn a_blank_document_identity_is_rejected() {
        let mut value = corpus();
        value.train[0].document_identity = "  ".into();
        value = value.seal();
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::MissingDocumentIdentity)
        );
    }

    #[test]
    fn a_prompt_keyed_split_is_rejected() {
        let mut value = corpus();
        value.split_rule = "prompt_sha256_rank_modulo;validation_every=4".into();
        value = value.seal();
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::PromptIdentityUsedAsSplitKey)
        );
    }

    #[test]
    fn a_non_lineage_prompt_role_is_rejected() {
        let mut value = corpus();
        value.prompt_identity_role = "split_key".into();
        value = value.seal();
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::PromptIdentityUsedAsSplitKey)
        );
    }

    #[test]
    fn a_tampered_corpus_digest_is_rejected() {
        let mut value = corpus();
        value.corpus_digest = digest('c');
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::CorpusDigestMismatch)
        );
    }

    #[test]
    fn a_tampered_reward_digest_is_rejected() {
        let mut value = corpus();
        value.reward_definition.digest = digest('d');
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::RewardDefinitionMismatch)
        );
    }

    #[test]
    fn a_gain_rule_that_is_not_frozen_is_rejected() {
        let mut value = corpus();
        value.gain_rule.state = "proposed".into();
        value = value.seal();
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::GainRuleNotFrozen)
        );
    }

    #[test]
    fn a_record_naming_a_foreign_document_is_rejected() {
        let mut value = corpus();
        value.train[0].records[0].document_identity = "doc-z".into();
        value = value.seal();
        assert_eq!(
            value.validate(),
            Err(LearningCorpusError::DocumentIdentityMismatch)
        );
    }
}
