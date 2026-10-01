//! Closed train → persist → reload → independent-eval cycle for the parse-policy
//! candidate (M514-1lss2s/S07/T03).
//!
//! This module is the single process that binds the three halves that previously
//! only existed apart: the frozen document-disjoint corpus
//! ([`crate::parse_learning_split`]), the deterministic MicroLoRA trainer
//! ([`crate::parse_policy_training`]) and the provider-independent same-n
//! evaluator ([`crate::parse_learning_evaluation`]). Every input is a real,
//! content-checked artifact; nothing here is a hard-coded fixture.
//!
//! The cycle is candidate-only by construction: it trains, persists, reloads,
//! verifies weights, evaluates two independent producers over one frozen ordered
//! set, measures conservation and contamination, and classifies the outcome as
//! `measured_gain`, `measured_no_gain` or `not_trainable`. There is no promotion
//! branch, no graph write, no import authorization and no route reload.
//!
//! Document groups are the unit of the split, but the trainer keys its split on
//! the document identity of one record per document. The cycle therefore
//! aggregates each [`DocumentGroup`] into exactly one [`TrajectoryRecord`] whose
//! `paper_id` is the document identity. That projection is deterministic and is
//! the only place the two representations meet.

use crate::parse_learning_evaluation::{
    CRITIC_SCHEMA_VERSION, ContaminationAssessment, CriticObservation, CriticStatus,
    EvaluationError, FrozenEvalCase, ProducerProvenance, PromotionQualification, ProvenanceBasis,
    aggregate_same_n_ablation_with_provenance, digest_value,
};
use crate::parse_learning_split::{
    DocumentGroup, FrozenLearningCorpus, GainRule, LearningCorpusError,
};
use crate::parse_policy_training::{
    TRAINING_CONFIG, TrainingConfig, TrainingError, candidate_embedding, candidate_forward,
    reload_persisted_candidate, train_microlora_candidate,
};
use da_ports::agent_memory::TrajectoryRecord;
use da_ports::parse_learning::{LearningContractError, LearningLineage, RvfExperienceStore};
use da_ports::{Sha256Digest, SplitIdentity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

/// Schema identity of the closed learning-cycle report.
pub const LEARNING_CYCLE_SCHEMA_VERSION: &str = "parse-learning-cycle.v1";
/// Every cycle report is a candidate; the cycle never promotes.
pub const CYCLE_PROMOTION_STATE: &str = "candidate";
/// Automatic promotion from a measured gain is forbidden by the ADR-076 freeze.
pub const CYCLE_AUTO_PROMOTE: &str = "forbidden";
/// The model label carried by a document's aggregated training record. The
/// corpus is frozen evidence, not a live model, so the label is explicit.
pub const DOCUMENT_MODEL: &str = "frozen-learning-corpus";
/// Producer labels are diagnostic only; independence is decided by the explicit
/// machine provenance below, never by these strings.
const BASELINE_PRODUCER_LABEL: &str = "frozen-recorded-reference";
const CANDIDATE_PRODUCER_LABEL: &str = "candidate-forward";
const BASELINE_CODE: &str = "reference-producer.frozen-recorded-quality.v1";
const CANDIDATE_CODE: &str = "candidate-producer.microlora-forward.v1";
const TRAINER_CODE: &str = "trainer-producer.microlora-train.v1";
const PRIOR_POLICY_LABEL: &str = "no-prior-policy";

fn zero_digest() -> Sha256Digest {
    Sha256Digest::new("0".repeat(64)).expect("64 zero hex chars form a valid digest")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn identity_digest(value: &str) -> Sha256Digest {
    Sha256Digest::new(sha256_hex(value.as_bytes())).expect("sha256 hex is always valid")
}

/// Measured conservation of a cycle: the corpus digest is recomputed after the
/// run and the RVF journal is read twice (a deterministic replay). A drift in
/// either raises the veto, which the aggregation refuses to cross.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningConservation {
    pub corpus_digest_before: Sha256Digest,
    pub corpus_digest_after: Sha256Digest,
    /// RVF `ordered_digest` before this run appended its receipts.
    pub rvf_ordered_digest_before_training: Sha256Digest,
    /// RVF `ordered_digest` after the training append.
    pub rvf_ordered_digest_after_training: Sha256Digest,
    /// RVF `ordered_digest` from a second, independent read.
    pub rvf_replay_digest: Sha256Digest,
    pub veto: bool,
}

/// The honest classification of one cycle run.
///
/// There is deliberately no `promoted` or `deployed` variant: a candidate that
/// wins an offline comparison is still only a measured candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LearningOutcome {
    /// The candidate was at least as good as the frozen baseline by the frozen
    /// margin on both metrics.
    MeasuredGain,
    /// The candidate was measured and did not clear the frozen gain rule.
    MeasuredNoGain,
    /// The corpus could not produce a candidate. The reason is carried verbatim.
    NotTrainable { reason: String },
}

/// The closed report of one train → persist → reload → eval cycle.
///
/// Measured fields are `Option` so a `not_trainable` run cannot present a
/// fabricated zero as a measurement. `deployed`, `import_eligible`,
/// `promotion_state` and `auto_promote` are the candidate-only locks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningCycleReport {
    pub schema_version: String,
    pub candidate_id: Option<Sha256Digest>,
    pub weights_digest: Option<Sha256Digest>,
    /// The document-keyed split, never a prompt-keyed one.
    pub split: SplitIdentity,
    /// Digest of the retained prompt identities (lineage only).
    pub prompt_lineage_digest: Sha256Digest,
    /// Must equal the frozen corpus reward digest.
    pub reward_definition_sha256: Sha256Digest,
    pub gain_rule: GainRule,
    pub baseline_n: u64,
    pub candidate_n: u64,
    pub baseline_metric_one: Option<f64>,
    pub baseline_metric_two: Option<f64>,
    pub candidate_metric_one: Option<f64>,
    pub candidate_metric_two: Option<f64>,
    pub metric_one_delta: Option<f64>,
    pub metric_two_delta: Option<f64>,
    pub validation_gap: Option<f64>,
    pub quality_qualification: Option<PromotionQualification>,
    pub critic_failures: Option<u64>,
    pub producer_independent: Option<bool>,
    pub provenance_basis: Option<ProvenanceBasis>,
    pub contamination_assessment: ContaminationAssessment,
    /// The measured document-identity intersection between train and eval.
    pub train_eval_document_intersection: Vec<String>,
    pub conservation: LearningConservation,
    pub outcome: LearningOutcome,
    pub deployed: bool,
    pub import_eligible: bool,
    pub promotion_state: String,
    pub auto_promote: String,
    pub report_digest: Sha256Digest,
}

impl LearningCycleReport {
    /// Recompute the report digest from content, ignoring `report_digest`.
    pub fn recomputed_report_digest(&self) -> Sha256Digest {
        let mut copy = self.clone();
        copy.report_digest = zero_digest();
        digest_value(&copy)
    }
}

/// Typed, named cycle failures. The validator variants are the only accepted
/// reasons a report may be refused.
#[derive(Debug, Error)]
pub enum LearningCycleError {
    #[error("frozen learning corpus is invalid: {0}")]
    Corpus(#[from] LearningCorpusError),
    #[error("learning contract is invalid: {0}")]
    Contract(#[from] LearningContractError),
    #[error("candidate training failed: {0}")]
    Training(#[from] TrainingError),
    #[error("evaluation aggregation failed: {0}")]
    Evaluation(#[from] EvaluationError),
    #[error("RVF experience store failed: {0}")]
    Store(String),
    #[error("a document group carries no records")]
    EmptyDocumentRecords,
    #[error("the report reward digest does not match the frozen corpus")]
    RewardDefinitionMismatch,
    #[error("producer independence was derived from labels, not explicit lineage")]
    LabelDerivedIndependence,
    #[error("contamination was detected or could not be assessed")]
    ContaminationVeto,
    #[error("conservation was vetoed")]
    ConservationVeto,
    #[error("the candidate-only lock was violated")]
    CandidateLockViolation,
    #[error("the report digest does not match its content")]
    ReportDigestMismatch,
    #[error("a report metric is not finite")]
    NonFiniteMetric,
    #[error("baseline and candidate denominators differ")]
    DenominatorMismatch,
}

/// The document-keyed split of the frozen corpus. Each side member is
/// `sha256(document_identity)`, never a prompt identity.
fn document_split(corpus: &FrozenLearningCorpus) -> SplitIdentity {
    SplitIdentity {
        train: corpus
            .train
            .iter()
            .map(|group| identity_digest(&group.document_identity))
            .collect(),
        validation: corpus
            .validation
            .iter()
            .map(|group| identity_digest(&group.document_identity))
            .collect(),
    }
}

fn prompt_lineage_digest(corpus: &FrozenLearningCorpus) -> Sha256Digest {
    let identities: Vec<&Sha256Digest> = corpus
        .train
        .iter()
        .chain(corpus.validation.iter())
        .flat_map(|group| group.prompt_identities.iter())
        .collect();
    digest_value(&identities)
}

/// The measured train ∩ eval document identities, in sorted order. Empty for a
/// valid frozen corpus; recording it is what turns "presumably disjoint" into a
/// number a reader can check.
fn train_eval_intersection(corpus: &FrozenLearningCorpus) -> Vec<String> {
    let train: BTreeSet<&str> = corpus
        .train
        .iter()
        .map(|group| group.document_identity.as_str())
        .collect();
    let validation: BTreeSet<&str> = corpus
        .validation
        .iter()
        .map(|group| group.document_identity.as_str())
        .collect();
    train
        .intersection(&validation)
        .map(|identity| (*identity).to_string())
        .collect()
}

/// The recorded grounding quality of a document: the mean of its records'
/// `grounding_hint`, with missing hints falling back to the `ok` indicator. This
/// is the baseline's measured metric.
fn document_quality(group: &DocumentGroup) -> f64 {
    let n = group.records.len() as f64;
    group
        .records
        .iter()
        .map(|record| record.quality.unwrap_or(if record.ok { 1.0 } else { 0.0 }))
        .sum::<f64>()
        / n
}

/// A document is `ok` by majority of its records.
fn document_ok(group: &DocumentGroup) -> bool {
    let n = group.records.len() as f64;
    let ok_rate = group
        .records
        .iter()
        .map(|record| if record.ok { 1.0 } else { 0.0 })
        .sum::<f64>()
        / n;
    ok_rate >= 0.5
}

/// Deterministic projection of one document group into the single trajectory the
/// trainer consumes. The split key is the document identity.
fn document_record(group: &DocumentGroup) -> TrajectoryRecord {
    let n = group.records.len() as u64;
    let latency_ms = group
        .records
        .iter()
        .map(|record| record.latency_ms)
        .sum::<u64>()
        / n;
    let tokens: Vec<u32> = group
        .records
        .iter()
        .filter_map(|record| record.completion_tokens)
        .collect();
    let completion_tokens = if tokens.is_empty() {
        None
    } else {
        Some(tokens.iter().sum::<u32>() / tokens.len() as u32)
    };
    TrajectoryRecord {
        task_id: "parse_learning_document".into(),
        paper_id: group.document_identity.clone(),
        model: DOCUMENT_MODEL.into(),
        prompt_sha256: group
            .prompt_identities
            .first()
            .map(|identity| identity.as_str().to_string())
            .unwrap_or_default(),
        ok: document_ok(group),
        latency_ms,
        completion_tokens,
        grounding_hint: Some(document_quality(group)),
        article_features: None,
        failure_pattern: None,
        construction_trace: None,
    }
}

/// Explicit SUT-free reference producer: it replays the frozen recorded quality.
fn baseline_producer(corpus: &Sha256Digest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest_value(&BASELINE_CODE),
        input_digest: corpus.clone(),
        method: "frozen_recorded_reference".into(),
        authority: "frozen-learning-corpus".into(),
    }
}

/// Explicit candidate producer: the vendor MicroLoRA forward pass over the
/// reloaded weights.
fn forward_producer(corpus: &Sha256Digest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest_value(&CANDIDATE_CODE),
        input_digest: corpus.clone(),
        method: "candidate_forward".into(),
        authority: "sona-microlora-candidate".into(),
    }
}

/// Explicit trainer producer: the third identity required by the same-n
/// aggregation.
fn trainer_producer(corpus: &Sha256Digest) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest_value(&TRAINER_CODE),
        input_digest: corpus.clone(),
        method: "micro_lora_trainer".into(),
        authority: "sona-trainer".into(),
    }
}

/// Build the lineage the cycle and the RVF store share. Only `split` is derived
/// from the corpus partition; the rest are explicit content digests so a retry
/// is byte-identical.
pub fn learning_lineage_for_corpus(corpus: &FrozenLearningCorpus) -> LearningLineage {
    let split = document_split(corpus);
    LearningLineage {
        source: corpus.source_sha256.clone(),
        corpus: corpus.corpus_digest.clone(),
        config: digest_value(&TRAINING_CONFIG),
        prior_policy: digest_value(&PRIOR_POLICY_LABEL),
        conservation: digest_value(&corpus.corpus_digest),
        split: digest_value(&split),
    }
}

fn store_error(error: impl std::fmt::Display) -> LearningCycleError {
    LearningCycleError::Store(error.to_string())
}

fn seal_report(mut report: LearningCycleReport) -> LearningCycleReport {
    report.report_digest = report.recomputed_report_digest();
    report
}

/// Measured conservation: recompute the corpus digest and replay the RVF journal
/// twice. Any drift raises the veto.
fn measure_conservation(
    corpus: &FrozenLearningCorpus,
    corpus_digest_before: Sha256Digest,
    rvf_before: Sha256Digest,
    store: &impl RvfExperienceStore,
) -> Result<LearningConservation, LearningCycleError> {
    let corpus_digest_after = corpus.recomputed_corpus_digest();
    let rvf_after = store.verify().map_err(store_error)?.ordered_digest;
    let rvf_replay = store.verify().map_err(store_error)?.ordered_digest;
    let veto = corpus_digest_before != corpus_digest_after || rvf_after != rvf_replay;
    Ok(LearningConservation {
        corpus_digest_before,
        corpus_digest_after,
        rvf_ordered_digest_before_training: rvf_before,
        rvf_ordered_digest_after_training: rvf_after,
        rvf_replay_digest: rvf_replay,
        veto,
    })
}

/// Run exactly one closed learning cycle over a frozen corpus.
///
/// Sequence: validate the corpus and measure contamination → train one
/// document-disjoint candidate and persist it → reload and verify weights →
/// measure the candidate through the vendor forward pass for every validation
/// document → aggregate baseline/candidate/trainer over one frozen ordered set
/// with explicit provenance → measure conservation → classify the outcome.
///
/// The candidate artifact is written to `candidate_path`; the caller owns the
/// report artifact. Nothing is promoted, imported or written to a graph.
pub fn run_learning_cycle(
    corpus: &FrozenLearningCorpus,
    store: &mut impl RvfExperienceStore,
    lineage: LearningLineage,
    candidate_path: &Path,
) -> Result<LearningCycleReport, LearningCycleError> {
    corpus.validate()?;
    if corpus
        .train
        .iter()
        .chain(corpus.validation.iter())
        .any(|group| group.records.is_empty())
    {
        return Err(LearningCycleError::EmptyDocumentRecords);
    }

    let split = document_split(corpus);
    split.validate()?;
    let mut lineage = lineage;
    lineage.split = digest_value(&split);

    let prompt_lineage_digest = prompt_lineage_digest(corpus);
    let corpus_digest_before = corpus.recomputed_corpus_digest();
    let intersection = train_eval_intersection(corpus);

    // Contamination is measured, not assumed. A non-empty intersection or an
    // unmeasurable one refuses the run before any training can happen.
    let contamination_assessment = if corpus.train.is_empty() || corpus.validation.is_empty() {
        ContaminationAssessment::NotAssessed
    } else if intersection.is_empty() {
        ContaminationAssessment::NotDetected
    } else {
        ContaminationAssessment::Detected
    };
    if contamination_assessment != ContaminationAssessment::NotDetected {
        return Err(LearningCycleError::ContaminationVeto);
    }

    let rvf_before = store.verify().map_err(store_error)?.ordered_digest;

    let train_records: Vec<TrajectoryRecord> = corpus.train.iter().map(document_record).collect();
    let validation_records: Vec<TrajectoryRecord> =
        corpus.validation.iter().map(document_record).collect();

    let training = train_microlora_candidate(
        store,
        &train_records,
        &validation_records,
        lineage.clone(),
        None,
        TrainingConfig::default(),
        Some(candidate_path),
    );

    let manifest = match training {
        Ok(outcome) => outcome.manifest,
        Err(TrainingError::NotTrainable(reason)) => {
            let conservation =
                measure_conservation(corpus, corpus_digest_before, rvf_before, store)?;
            let report = LearningCycleReport {
                schema_version: LEARNING_CYCLE_SCHEMA_VERSION.into(),
                candidate_id: None,
                weights_digest: None,
                split,
                prompt_lineage_digest,
                reward_definition_sha256: corpus.reward_definition.digest.clone(),
                gain_rule: corpus.gain_rule.clone(),
                baseline_n: 0,
                candidate_n: 0,
                baseline_metric_one: None,
                baseline_metric_two: None,
                candidate_metric_one: None,
                candidate_metric_two: None,
                metric_one_delta: None,
                metric_two_delta: None,
                validation_gap: None,
                quality_qualification: None,
                critic_failures: None,
                producer_independent: None,
                provenance_basis: None,
                contamination_assessment,
                train_eval_document_intersection: intersection,
                conservation,
                outcome: LearningOutcome::NotTrainable {
                    reason: reason.to_string(),
                },
                deployed: false,
                import_eligible: false,
                promotion_state: CYCLE_PROMOTION_STATE.into(),
                auto_promote: CYCLE_AUTO_PROMOTE.into(),
                report_digest: zero_digest(),
            };
            let report = seal_report(report);
            validate_learning_cycle_report(&report, corpus)?;
            return Ok(report);
        }
        Err(error) => return Err(LearningCycleError::Training(error)),
    };

    // Real read path: reload the persisted artifact, verify the weight digest
    // against the manifest, then evaluate through the vendor forward pass.
    let candidate = reload_persisted_candidate(candidate_path)?;
    let hidden_dim = candidate.manifest.hidden_dim as usize;

    // Fixed evaluation order: sha256(document_identity), identity as tie break.
    let mut validation_groups: Vec<&DocumentGroup> = corpus.validation.iter().collect();
    validation_groups.sort_by_key(|group| {
        (
            sha256_hex(group.document_identity.as_bytes()),
            group.document_identity.clone(),
        )
    });

    let mut cases = Vec::with_capacity(validation_groups.len());
    let mut baseline_rows = Vec::with_capacity(validation_groups.len());
    let mut candidate_rows = Vec::with_capacity(validation_groups.len());
    for (order, group) in validation_groups.iter().enumerate() {
        let order = order as u32;
        let quality = document_quality(group);
        let ok = document_ok(group);
        let case_id = group.document_identity.clone();
        cases.push(FrozenEvalCase {
            case_id: case_id.clone(),
            corpus: corpus.corpus_digest.clone(),
            order,
            expected_digest: digest_value(&quality),
        });
        baseline_rows.push(CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: case_id.clone(),
            corpus: corpus.corpus_digest.clone(),
            order,
            producer: BASELINE_PRODUCER_LABEL.into(),
            metric_one: quality,
            metric_two: if ok { 1.0 } else { 0.0 },
            status: CriticStatus::Ok,
        });
        let input = candidate_embedding(&document_record(group), hidden_dim);
        let output = candidate_forward(&candidate, &input)?;
        let metric_one = (output[0] as f64).clamp(0.0, 1.0);
        candidate_rows.push(CriticObservation {
            schema_version: CRITIC_SCHEMA_VERSION,
            case_id: case_id.clone(),
            corpus: corpus.corpus_digest.clone(),
            order,
            producer: CANDIDATE_PRODUCER_LABEL.into(),
            metric_one,
            metric_two: if metric_one >= 0.5 { 1.0 } else { 0.0 },
            status: CriticStatus::Ok,
        });
    }

    let conservation = measure_conservation(corpus, corpus_digest_before, rvf_before, store)?;
    let ablation = aggregate_same_n_ablation_with_provenance(
        &manifest,
        &cases,
        &baseline_rows,
        &candidate_rows,
        !conservation.veto,
        &baseline_producer(&corpus.corpus_digest),
        &forward_producer(&corpus.corpus_digest),
        &trainer_producer(&corpus.corpus_digest),
    )?;

    let margin = corpus.gain_rule.margin;
    let clears_rule = ablation.candidate_metric_one - ablation.baseline_metric_one >= margin
        && ablation.candidate_metric_two - ablation.baseline_metric_two >= margin;
    let outcome = if clears_rule {
        LearningOutcome::MeasuredGain
    } else {
        LearningOutcome::MeasuredNoGain
    };

    let report = LearningCycleReport {
        schema_version: LEARNING_CYCLE_SCHEMA_VERSION.into(),
        candidate_id: Some(manifest.receipt.candidate_id.clone()),
        weights_digest: Some(manifest.weights_digest.clone()),
        split,
        prompt_lineage_digest,
        reward_definition_sha256: corpus.reward_definition.digest.clone(),
        gain_rule: corpus.gain_rule.clone(),
        baseline_n: ablation.baseline_n,
        candidate_n: ablation.candidate_n,
        baseline_metric_one: Some(ablation.baseline_metric_one),
        baseline_metric_two: Some(ablation.baseline_metric_two),
        candidate_metric_one: Some(ablation.candidate_metric_one),
        candidate_metric_two: Some(ablation.candidate_metric_two),
        metric_one_delta: Some(ablation.candidate_metric_one - ablation.baseline_metric_one),
        metric_two_delta: Some(ablation.candidate_metric_two - ablation.baseline_metric_two),
        validation_gap: Some(ablation.validation_gap),
        quality_qualification: Some(ablation.quality_qualification),
        critic_failures: Some(ablation.critic_failures),
        producer_independent: Some(ablation.producer_independent),
        provenance_basis: Some(ablation.provenance_basis),
        contamination_assessment,
        train_eval_document_intersection: intersection,
        conservation,
        outcome,
        deployed: false,
        import_eligible: false,
        promotion_state: CYCLE_PROMOTION_STATE.into(),
        auto_promote: CYCLE_AUTO_PROMOTE.into(),
        report_digest: zero_digest(),
    };
    let report = seal_report(report);
    validate_learning_cycle_report(&report, corpus)?;
    Ok(report)
}

/// Fail-closed validation of a cycle report against the corpus it claims.
///
/// The report must carry the corpus reward digest, an explicitly-provenanced
/// independent evaluation, a measured `NotDetected` contamination result, an
/// unvetoed conservation record, closed candidate-only locks, finite metrics and
/// a report digest that still matches its content. Every rejection is a named
/// variant, and a `not_trainable` outcome skips only the fields that were never
/// measured.
pub fn validate_learning_cycle_report(
    report: &LearningCycleReport,
    corpus: &FrozenLearningCorpus,
) -> Result<(), LearningCycleError> {
    corpus.validate()?;

    if report.reward_definition_sha256 != corpus.reward_definition.digest {
        return Err(LearningCycleError::RewardDefinitionMismatch);
    }
    if report.conservation.veto
        || report.conservation.corpus_digest_before != corpus.corpus_digest
        || report.conservation.corpus_digest_after != corpus.corpus_digest
    {
        return Err(LearningCycleError::ConservationVeto);
    }
    if report.contamination_assessment != ContaminationAssessment::NotDetected {
        return Err(LearningCycleError::ContaminationVeto);
    }
    if report.deployed
        || report.import_eligible
        || report.promotion_state != CYCLE_PROMOTION_STATE
        || report.auto_promote != CYCLE_AUTO_PROMOTE
    {
        return Err(LearningCycleError::CandidateLockViolation);
    }

    if matches!(
        report.outcome,
        LearningOutcome::MeasuredGain | LearningOutcome::MeasuredNoGain
    ) {
        if report.provenance_basis != Some(ProvenanceBasis::Explicit)
            || report.producer_independent != Some(true)
        {
            return Err(LearningCycleError::LabelDerivedIndependence);
        }
        if report.baseline_n != report.candidate_n {
            return Err(LearningCycleError::DenominatorMismatch);
        }
        let metrics = [
            report.baseline_metric_one,
            report.baseline_metric_two,
            report.candidate_metric_one,
            report.candidate_metric_two,
            report.metric_one_delta,
            report.metric_two_delta,
            report.validation_gap,
        ];
        if metrics.iter().any(|metric| match metric {
            Some(value) => !value.is_finite(),
            None => true,
        }) {
            return Err(LearningCycleError::NonFiniteMetric);
        }
    }

    if report.recomputed_report_digest() != report.report_digest {
        return Err(LearningCycleError::ReportDigestMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_learning_split::{
        DOCUMENT_IDENTITY_FIELD, DocumentRecord, GAIN_RULE_STATE_FROZEN, GainRule,
        LEARNING_CORPUS_SCHEMA_VERSION, PROMPT_IDENTITY_ROLE_LINEAGE, REWARD_VERSION_V1,
        RewardDefinition, SPLIT_RULE_DOCUMENT_RANK_MODULO,
    };
    use da_adapters::RvfFileStore;

    fn digest(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).expect("valid digest")
    }

    fn record(document: &str, quality: f64) -> DocumentRecord {
        DocumentRecord {
            document_identity: document.into(),
            prompt_identity: digest('a'),
            quality: Some(quality),
            ok: true,
            latency_ms: 10,
            completion_tokens: Some(5),
        }
    }

    fn group(document: &str, quality: f64) -> DocumentGroup {
        DocumentGroup {
            document_identity: document.into(),
            prompt_identities: vec![digest('a')],
            records: vec![record(document, quality)],
        }
    }

    fn fixture_corpus() -> FrozenLearningCorpus {
        let reward_definition = RewardDefinition {
            version: REWARD_VERSION_V1.into(),
            components: vec!["grounding_hint".into(), "ok_indicator".into()],
            digest: zero_digest(),
        }
        .seal();
        FrozenLearningCorpus {
            schema_version: LEARNING_CORPUS_SCHEMA_VERSION.into(),
            source_path: "data/agent_memory/summarizer/trajectory.jsonl".into(),
            source_sha256: digest('b'),
            document_identity_field: DOCUMENT_IDENTITY_FIELD.into(),
            prompt_identity_role: PROMPT_IDENTITY_ROLE_LINEAGE.into(),
            split_rule: SPLIT_RULE_DOCUMENT_RANK_MODULO.into(),
            reward_definition,
            gain_rule: GainRule {
                metric_one: "grounding_hint_mean".into(),
                metric_two: "ok_rate".into(),
                margin: 0.0,
                state: GAIN_RULE_STATE_FROZEN.into(),
            },
            train: vec![
                group("doc-a", 0.9),
                group("doc-b", 0.8),
                group("doc-c", 0.7),
            ],
            validation: vec![group("doc-v", 0.75), group("doc-w", 0.6)],
            corpus_digest: zero_digest(),
        }
        .seal()
    }

    fn run_fixture(dir: &Path, tag: &str) -> LearningCycleReport {
        let corpus = fixture_corpus();
        let lineage = learning_lineage_for_corpus(&corpus);
        let mut store =
            RvfFileStore::open(dir.join(format!("experience-{tag}.rvf")), lineage.clone())
                .expect("open store");
        run_learning_cycle(
            &corpus,
            &mut store,
            lineage,
            &dir.join(format!("candidate-{tag}.json")),
        )
        .expect("cycle runs over the fixture")
    }

    #[test]
    fn cycle_produces_a_valid_candidate_only_report() {
        let dir = tempfile::tempdir().unwrap();
        let report = run_fixture(dir.path(), "primary");
        let corpus = fixture_corpus();
        validate_learning_cycle_report(&report, &corpus).expect("report validates");

        assert_eq!(report.baseline_n, report.candidate_n);
        assert!(report.baseline_n > 0);
        assert_eq!(report.producer_independent, Some(true));
        assert_eq!(report.provenance_basis, Some(ProvenanceBasis::Explicit));
        assert_eq!(
            report.contamination_assessment,
            ContaminationAssessment::NotDetected
        );
        assert!(report.train_eval_document_intersection.is_empty());
        assert!(!report.deployed && !report.import_eligible);
        assert_eq!(report.promotion_state, CYCLE_PROMOTION_STATE);
        assert_eq!(report.auto_promote, CYCLE_AUTO_PROMOTE);
        assert!(!report.conservation.veto);
        assert_eq!(report.recomputed_report_digest(), report.report_digest);
        assert!(matches!(
            report.outcome,
            LearningOutcome::MeasuredGain | LearningOutcome::MeasuredNoGain
        ));
    }

    #[test]
    fn a_tampered_report_digest_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut report = run_fixture(dir.path(), "tampered");
        report.report_digest = digest('f');
        assert!(matches!(
            validate_learning_cycle_report(&report, &fixture_corpus()),
            Err(LearningCycleError::ReportDigestMismatch)
        ));
    }

    #[test]
    fn a_label_bridged_report_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut report = run_fixture(dir.path(), "bridged");
        report.provenance_basis = Some(ProvenanceBasis::LegacyLabelBridge);
        let report = seal_report(report);
        assert!(matches!(
            validate_learning_cycle_report(&report, &fixture_corpus()),
            Err(LearningCycleError::LabelDerivedIndependence)
        ));
    }

    #[test]
    fn an_open_candidate_lock_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut report = run_fixture(dir.path(), "unlocked");
        report.deployed = true;
        let report = seal_report(report);
        assert!(matches!(
            validate_learning_cycle_report(&report, &fixture_corpus()),
            Err(LearningCycleError::CandidateLockViolation)
        ));
    }

    #[test]
    fn an_undersized_train_side_is_not_trainable_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut undersized = fixture_corpus();
        undersized.train.truncate(1);
        let corpus = undersized.seal();
        corpus
            .validate()
            .expect("one train and two validation documents form a valid split");

        let lineage = learning_lineage_for_corpus(&corpus);
        let mut store =
            RvfFileStore::open(dir.path().join("experience-short.rvf"), lineage.clone()).unwrap();
        let report = run_learning_cycle(
            &corpus,
            &mut store,
            lineage,
            &dir.path().join("candidate-short.json"),
        )
        .expect("an untrainable corpus is a reported outcome, not an error");

        assert!(matches!(
            report.outcome,
            LearningOutcome::NotTrainable { .. }
        ));
        assert!(report.candidate_id.is_none());
        assert!(report.weights_digest.is_none());
        assert_eq!((report.baseline_n, report.candidate_n), (0, 0));
        assert!(report.producer_independent.is_none());
        assert_eq!(report.recomputed_report_digest(), report.report_digest);
        validate_learning_cycle_report(&report, &corpus)
            .expect("a not_trainable report still validates");
    }
}
