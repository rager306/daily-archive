//! Candidate learned decision-table trainer (ADR-074 candidate-only learning).
//!
//! Trains a [`LearnedDecisionTable`] from recorded policy observations instead
//! of MicroLoRA weights. The observation carries the DECISION CONDITION
//! ([`ArticleFeatures`]), the decision actually taken, and the measured reward;
//! the learner groups by [`feature_bucket`] and picks, per bucket, the decision
//! with the best mean reward under a minimum-support floor.
//!
//! Why a table and not weights: the policy sees four coarse features and answers
//! a closed enum. A rank-1 MicroLoRA over that input is a threshold table with
//! extra moving parts; the table is the same function, exactly replayable and
//! trivially auditable.
//!
//! The learner is pure, deterministic and candidate-only. It never promotes, it
//! omits buckets below the support floor (the consult then fail-opens to native
//! DATA rules), and it refuses a non-document-disjoint split.

use da_domain::fsm_policy::{
    Admission, ArticleFeatures, DecisionPoint, GrammarRoute, LearnedDecisionTable, ProfileRoute,
    feature_bucket,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// One recorded policy decision and its measured outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyObservation {
    /// Document identity — the split key. Never the prompt.
    pub document_identity: String,
    pub decision_point: DecisionPoint,
    /// The decision condition the policy saw.
    pub features: ArticleFeatures,
    /// The decision code the policy actually answered (closed enum value).
    pub decision_code: String,
    /// Measured reward for that decision. Higher is better.
    pub reward: f64,
    /// Raw outcome evidence the reward was computed from, so the reward rule
    /// stays re-computable and auditable after the fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<PolicyOutcomeEvidence>,
}

/// The raw signals behind a reward. The first honest rule is binary
/// (`ok` -> 1.0, failure -> 0.0); richer re-scoring can use the rest without
/// re-running the pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyOutcomeEvidence {
    /// Did the stage the decision gated complete without error.
    pub ok: bool,
    pub section_count: usize,
    pub body_chars: usize,
    pub latency_ms: u64,
    /// Backend identity the decision ran against (e.g. GROBID version).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
}

/// Binary reward rule: ok -> 1.0, failure -> 0.0. Explicit and boring on
/// purpose; every observation also carries the evidence for re-scoring.
pub fn binary_reward(outcome: &PolicyOutcomeEvidence) -> f64 {
    if outcome.ok { 1.0 } else { 0.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableLearnConfig {
    /// Minimum observations a bucket needs before it earns a cell.
    pub min_support: usize,
}

impl Default for TableLearnConfig {
    fn default() -> Self {
        Self { min_support: 2 }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum TableLearnError {
    #[error("table learning: decision point {0} is not learnable by a table")]
    UnlearnablePoint(String),
    #[error("table learning: a decision code is outside the closed enum: {0}")]
    UnknownCode(String),
    #[error("table learning: reward is not finite")]
    NonFiniteReward,
    #[error("table learning: a document identity is blank")]
    BlankIdentity,
    #[error("table learning: train/validation documents overlap")]
    SplitOverlap,
    #[error("table learning: no bucket reached the support floor")]
    NoLearnableCells,
    #[error("table learning: {0}")]
    InvalidConfig(String),
}

/// The decision codes a point may emit, in deterministic tie-break order.
fn allowed_codes(point: DecisionPoint) -> Option<&'static [&'static str]> {
    match point {
        DecisionPoint::GrammarRoute => Some(&["page_outline", "xy_cut", "grobid_structural"]),
        DecisionPoint::BatchAdmission => Some(&["accept", "defer", "skip"]),
        DecisionPoint::ProfileRoute => Some(&["structural", "rich"]),
        _ => None,
    }
}

fn code_is_valid(point: DecisionPoint, code: &str) -> bool {
    allowed_codes(point).is_some_and(|codes| codes.contains(&code))
}

/// Validate one observation against its point's closed code set.
fn validate_observation(obs: &PolicyObservation) -> Result<(), TableLearnError> {
    if obs.document_identity.trim().is_empty() {
        return Err(TableLearnError::BlankIdentity);
    }
    if !obs.reward.is_finite() {
        return Err(TableLearnError::NonFiniteReward);
    }
    if allowed_codes(obs.decision_point).is_none() {
        return Err(TableLearnError::UnlearnablePoint(
            obs.decision_point.as_str().to_string(),
        ));
    }
    if !code_is_valid(obs.decision_point, &obs.decision_code) {
        return Err(TableLearnError::UnknownCode(obs.decision_code.clone()));
    }
    Ok(())
}

/// Deterministic tie-break: highest mean reward, then lowest observation count
/// deficit, then lexicographically smallest code. A stable order means two runs
/// over the same observations always seal the same table digest.
fn pick_code(stats: &BTreeMap<String, (f64, usize)>) -> Option<String> {
    let mut best: Option<(String, f64, usize)> = None;
    for (code, (sum, count)) in stats {
        let mean = sum / (*count as f64);
        let better = match &best {
            None => true,
            Some((best_code, best_mean, _)) => {
                mean > *best_mean || (mean == *best_mean && code < best_code)
            }
        };
        if better {
            best = Some((code.clone(), mean, *count));
        }
    }
    best.map(|(code, _, _)| code)
}

/// Learn a sealed table for `point` from observations, omitting buckets below
/// the support floor. The caller owns promotion; the result is a candidate.
pub fn learn_decision_table(
    point: DecisionPoint,
    observations: &[PolicyObservation],
    config: TableLearnConfig,
) -> Result<LearnedDecisionTable, TableLearnError> {
    if config.min_support == 0 {
        return Err(TableLearnError::InvalidConfig(
            "min_support must be at least 1".into(),
        ));
    }
    if allowed_codes(point).is_none() {
        return Err(TableLearnError::UnlearnablePoint(
            point.as_str().to_string(),
        ));
    }

    // bucket -> code -> (reward sum, count)
    let mut by_bucket: BTreeMap<String, BTreeMap<String, (f64, usize)>> = BTreeMap::new();
    for obs in observations.iter().filter(|o| o.decision_point == point) {
        validate_observation(obs)?;
        let key = feature_bucket(point, &obs.features);
        let entry = by_bucket
            .entry(key)
            .or_default()
            .entry(obs.decision_code.clone())
            .or_insert((0.0, 0));
        entry.0 += obs.reward;
        entry.1 += 1;
    }

    let mut cells: BTreeMap<String, String> = BTreeMap::new();
    let mut cell_support: BTreeMap<String, u32> = BTreeMap::new();
    for (bucket, stats) in &by_bucket {
        let support: usize = stats.values().map(|(_, c)| c).sum();
        if support < config.min_support {
            continue;
        }
        if let Some(code) = pick_code(stats) {
            cells.insert(bucket.clone(), code);
            cell_support.insert(bucket.clone(), support as u32);
        }
    }
    if cells.is_empty() {
        return Err(TableLearnError::NoLearnableCells);
    }
    Ok(LearnedDecisionTable::seal_with_support(
        point,
        cells,
        cell_support,
    ))
}

/// A train/validation split of observations, keyed on document identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationSplit {
    pub train: Vec<PolicyObservation>,
    pub validation: Vec<PolicyObservation>,
}

impl ObservationSplit {
    /// Reject a blank identity or a document present on both sides.
    /// Reject a blank identity, or a document present on BOTH sides.
    ///
    /// A document may legitimately appear more than once on the SAME side:
    /// ADR-080 two-arm pairs put two observations of one document (one per
    /// arm) into the same split half. The invariant is document-disjointness
    /// between train and validation, not global uniqueness of the identity.
    pub fn validate(&self) -> Result<(), TableLearnError> {
        use std::collections::BTreeSet;
        let mut train = BTreeSet::new();
        let mut validation = BTreeSet::new();
        for obs in &self.train {
            if obs.document_identity.trim().is_empty() {
                return Err(TableLearnError::BlankIdentity);
            }
            train.insert(obs.document_identity.as_str());
        }
        for obs in &self.validation {
            if obs.document_identity.trim().is_empty() {
                return Err(TableLearnError::BlankIdentity);
            }
            validation.insert(obs.document_identity.as_str());
        }
        if !train.is_disjoint(&validation) {
            return Err(TableLearnError::SplitOverlap);
        }
        Ok(())
    }
}

/// Deterministic document-disjoint split, mirroring the frozen-corpus rule
/// (`document_identity_sha256_rank_modulo;validation_every=4`): rank documents
/// by sha256(identity) and send every fourth to validation.
pub const OBSERVATION_SPLIT_RULE: &str = "document_identity_sha256_rank_modulo;validation_every=4";

pub fn split_observations(
    observations: &[PolicyObservation],
) -> Result<ObservationSplit, TableLearnError> {
    use sha2::Digest;
    use std::collections::BTreeMap;
    // Keep one bucket per document: all observations of a document must land
    // on the same side, or the split leaks the decision condition.
    let mut by_doc: BTreeMap<&str, Vec<&PolicyObservation>> = BTreeMap::new();
    for obs in observations {
        if obs.document_identity.trim().is_empty() {
            return Err(TableLearnError::BlankIdentity);
        }
        by_doc
            .entry(obs.document_identity.as_str())
            .or_default()
            .push(obs);
    }
    let mut ranked: Vec<(&str, u64)> = by_doc
        .keys()
        .map(|id| {
            let mut hasher = sha2::Sha256::new();
            hasher.update(id.as_bytes());
            let digest = hasher.finalize();
            let rank = u64::from_be_bytes(digest[..8].try_into().expect("8 bytes"));
            (*id, rank)
        })
        .collect();
    ranked.sort_by_key(|(_, rank)| *rank);
    let mut train = Vec::new();
    let mut validation = Vec::new();
    for (index, (id, _)) in ranked.iter().enumerate() {
        let bucket = by_doc.get(id).expect("key came from by_doc");
        let target = if (index + 1) % 4 == 0 {
            &mut validation
        } else {
            &mut train
        };
        target.extend(bucket.iter().map(|obs| (*obs).clone()));
    }
    Ok(ObservationSplit { train, validation })
}

/// Accuracy of a sealed table over observations: the fraction of covered
/// observations whose learned code equals the recorded decision. Uncovered
/// observations are excluded (they fail open to native at runtime, so they are
/// not evidence for or against the table).
pub fn table_accuracy(
    table: &LearnedDecisionTable,
    observations: &[PolicyObservation],
) -> (usize, usize) {
    let mut covered = 0usize;
    let mut agree = 0usize;
    for obs in observations {
        let learned = match obs.decision_point {
            DecisionPoint::GrammarRoute => table.grammar_for(&obs.features).map(|r| r.as_str()),
            DecisionPoint::BatchAdmission => table.admission_for(&obs.features).map(|a| a.as_str()),
            DecisionPoint::ProfileRoute => table.profile_for(&obs.features).map(|p| p.as_str()),
            _ => None,
        };
        if let Some(code) = learned {
            covered += 1;
            if code == obs.decision_code {
                agree += 1;
            }
        }
    }
    (agree, covered)
}

/// ROUGE-L: F-measure of the Longest Common Subsequence between two
/// whitespace-tokenised texts (ADR-080 metric v3, Bevendorff et al.).
/// Deterministic, no allocation-heavy suffix structures — sections are small.
pub fn rouge_l(a: &str, b: &str) -> f64 {
    let ta: Vec<&str> = a.split_whitespace().collect();
    let tb: Vec<&str> = b.split_whitespace().collect();
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let n = ta.len();
    let m = tb.len();
    let mut dp = vec![0u64; m + 1];
    for i in 1..=n {
        let mut prev_diag = 0u64;
        for j in 1..=m {
            let tmp = dp[j];
            if ta[i - 1].eq_ignore_ascii_case(tb[j - 1]) {
                prev_diag = prev_diag + 1;
            } else if dp[j] > prev_diag {
                prev_diag = dp[j];
            }
            dp[j] = prev_diag;
        }
    }
    let lcs = dp[m] as f64;
    if lcs == 0.0 {
        return 0.0;
    }
    let prec = lcs / n as f64;
    let rec = lcs / m as f64;
    if prec + rec > 0.0 {
        2.0 * prec * rec / (prec + rec)
    } else {
        0.0
    }
}

/// Deterministic mean reward of a table's decisions over covered observations.
pub fn table_mean_reward(table: &LearnedDecisionTable, observations: &[PolicyObservation]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for obs in observations {
        let covered = match obs.decision_point {
            DecisionPoint::GrammarRoute => table.grammar_for(&obs.features).is_some(),
            DecisionPoint::BatchAdmission => table.admission_for(&obs.features).is_some(),
            DecisionPoint::ProfileRoute => table.profile_for(&obs.features).is_some(),
            _ => false,
        };
        if covered {
            sum += obs.reward;
            count += 1;
        }
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// Closed enum views used by callers that want a typed code, not a string.
pub fn grammar_of(table: &LearnedDecisionTable, f: &ArticleFeatures) -> Option<GrammarRoute> {
    table.grammar_for(f)
}
pub fn admission_of(table: &LearnedDecisionTable, f: &ArticleFeatures) -> Option<Admission> {
    table.admission_for(f)
}
pub fn profile_of(table: &LearnedDecisionTable, f: &ArticleFeatures) -> Option<ProfileRoute> {
    table.profile_for(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: "computer_science".into(),
        }
    }

    fn obs(id: &str, f: ArticleFeatures, code: &str, reward: f64) -> PolicyObservation {
        PolicyObservation {
            document_identity: id.into(),
            decision_point: DecisionPoint::GrammarRoute,
            features: f,
            decision_code: code.into(),
            reward,
            outcome: None,
        }
    }

    #[test]
    fn learns_majority_by_reward_per_bucket() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
            obs("d3", feat(25, 0, 0.90), "page_outline", 0.1),
        ];
        let table = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap();
        assert_eq!(
            table.grammar_for(&feat(18, 0, 0.85)),
            Some(GrammarRoute::XyCut)
        );
    }

    #[test]
    fn omits_buckets_below_support_floor() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
            // A lone observation in another bucket must not earn a cell.
            obs("d3", feat(700, 40, 0.9), "page_outline", 0.9),
        ];
        let table = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap();
        assert!(table.grammar_for(&feat(18, 0, 0.85)).is_some());
        assert_eq!(
            table.grammar_for(&feat(700, 40, 0.9)),
            None,
            "single-observation bucket must fail open to native"
        );
    }

    #[test]
    fn rejects_point_without_closed_codes() {
        let mut o = obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9);
        o.decision_point = DecisionPoint::ConcurrencyTune;
        let err = learn_decision_table(
            DecisionPoint::ConcurrencyTune,
            &[o],
            TableLearnConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(err, TableLearnError::UnlearnablePoint(_)));
    }

    #[test]
    fn rejects_code_outside_closed_enum() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "invented_route", 0.9),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
        ];
        let err = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, TableLearnError::UnknownCode("invented_route".into()));
    }

    #[test]
    fn rejects_non_finite_reward() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", f64::NAN),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
        ];
        let err = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, TableLearnError::NonFiniteReward);
    }

    #[test]
    fn refuses_when_no_bucket_reaches_support() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9),
            obs("d2", feat(700, 40, 0.9), "page_outline", 0.9),
        ];
        let err = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap_err();
        assert_eq!(err, TableLearnError::NoLearnableCells);
    }

    #[test]
    fn deterministic_tie_break_by_code() {
        // Equal reward and support: the lexicographically smallest code wins,
        // so two runs seal the same digest.
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.5),
            obs("d2", feat(20, 0, 0.80), "grobid_structural", 0.5),
        ];
        let table = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap();
        assert_eq!(
            table.grammar_for(&feat(18, 0, 0.85)),
            Some(GrammarRoute::GrobidStructural),
            "grobid_structural sorts before xy_cut"
        );
    }

    #[test]
    fn split_rejects_a_document_on_both_sides() {
        let split = ObservationSplit {
            train: vec![obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9)],
            validation: vec![obs("d1", feat(20, 0, 0.80), "xy_cut", 0.8)],
        };
        assert_eq!(split.validate(), Err(TableLearnError::SplitOverlap));
    }

    #[test]
    fn split_allows_a_pair_on_one_side() {
        // ADR-080: one document, two arms, same half - not an overlap.
        let split = ObservationSplit {
            train: vec![
                obs("d1", feat(18, 0, 0.85), "xy_cut", 1.0),
                obs("d1", feat(18, 0, 0.85), "page_outline", 0.0),
            ],
            validation: vec![obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8)],
        };
        split.validate().expect("a pair on one side is legal");
    }

    #[test]
    fn accuracy_counts_only_covered_observations() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
        ];
        let table = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap();
        let (agree, covered) = table_accuracy(&table, &observations);
        assert_eq!((agree, covered), (2, 2));
        // An uncovered observation is not counted.
        let extra = vec![obs("d3", feat(700, 40, 0.9), "page_outline", 0.9)];
        let (a2, c2) = table_accuracy(&table, &extra);
        assert_eq!((a2, c2), (0, 0));
    }

    #[test]
    fn sealed_table_from_learner_validates() {
        let observations = vec![
            obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9),
            obs("d2", feat(20, 0, 0.80), "xy_cut", 0.8),
        ];
        let table = learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig { min_support: 2 },
        )
        .unwrap();
        table.validate().expect("learner output must validate");
    }

    #[test]
    fn split_is_document_disjoint_and_deterministic() {
        let observations: Vec<PolicyObservation> = (1..=8)
            .map(|i| obs(&format!("d{i}"), feat(18, 0, 0.85), "xy_cut", 0.9))
            .collect();
        let a = split_observations(&observations).unwrap();
        let b = split_observations(&observations).unwrap();
        assert_eq!(a, b, "split must be deterministic");
        a.validate().expect("documents must not straddle the split");
        assert!(!a.train.is_empty());
        assert!(!a.validation.is_empty());
    }

    #[test]
    fn split_keeps_one_documents_observations_together() {
        let mut observations = vec![obs("d1", feat(18, 0, 0.85), "xy_cut", 0.9)];
        observations.push(obs("d1", feat(700, 40, 0.9), "page_outline", 0.4));
        let split = split_observations(&observations).unwrap();
        let in_train = split
            .train
            .iter()
            .filter(|o| o.document_identity == "d1")
            .count();
        let in_val = split
            .validation
            .iter()
            .filter(|o| o.document_identity == "d1")
            .count();
        assert!(
            in_train == 0 || in_val == 0,
            "a document's observations must not straddle the split"
        );
    }

    #[test]
    fn binary_reward_is_explicit() {
        let ok = PolicyOutcomeEvidence {
            ok: true,
            section_count: 3,
            body_chars: 100,
            latency_ms: 10,
            backend: Some("0.9.0".into()),
        };
        let fail = PolicyOutcomeEvidence {
            ok: false,
            ..ok.clone()
        };
        assert_eq!(binary_reward(&ok), 1.0);
        assert_eq!(binary_reward(&fail), 0.0);
    }

    proptest::proptest! {
        /// The learner is deterministic: the same observations seal the same
        /// table identity, regardless of the order they are presented in.
        #[test]
        fn learning_is_order_independent(
            rewards in proptest::collection::vec(0u8..2, 4..24),
        ) {
            let observations: Vec<PolicyObservation> = rewards
                .iter()
                .enumerate()
                .map(|(i, ok)| obs(&format!("d{i}"), feat(18, 0, 0.85), "xy_cut", f64::from(*ok)))
                .collect();
            let forward = learn_decision_table(
                DecisionPoint::GrammarRoute,
                &observations,
                TableLearnConfig { min_support: 2 },
            )
            .expect("covered");
            let mut reversed = observations.clone();
            reversed.reverse();
            let backward = learn_decision_table(
                DecisionPoint::GrammarRoute,
                &reversed,
                TableLearnConfig { min_support: 2 },
            )
            .expect("covered");
            prop_assert_eq!(forward.table_sha256, backward.table_sha256);
        }

        /// Support recorded for a cell equals the number of observations that
        /// landed in that cell, and is never below the support floor. A corpus
        /// below the floor is honestly not-trainable, never a silent empty table.
        #[test]
        fn support_equals_observed_count(
            count in 2usize..40,
            min_support in 2usize..5,
        ) {
            let observations: Vec<PolicyObservation> = (0..count)
                .map(|i| obs(&format!("d{i}"), feat(18, 0, 0.85), "xy_cut", 1.0))
                .collect();
            let key = feature_bucket(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85));
            match learn_decision_table(
                DecisionPoint::GrammarRoute,
                &observations,
                TableLearnConfig { min_support },
            ) {
                Ok(table) => {
                    prop_assert_eq!(table.cell_support.get(&key).copied(), Some(count as u32));
                    prop_assert!(table.validate().is_ok());
                }
                Err(e) => {
                    // The only honest failure here is a corpus below the floor.
                    prop_assert_eq!(e, TableLearnError::NoLearnableCells);
                    prop_assert!(count < min_support);
                }
            }
        }

        /// A learned table always answers its own training buckets at level 0.
        #[test]
        fn learned_table_covers_its_training_buckets(
            quality_milli in 0i32..1000,
            pages in 1u32..500,
        ) {
            let f = feat(pages, 0, quality_milli as f32 / 1000.0);
            let observations: Vec<PolicyObservation> = (0..4)
                .map(|i| obs(&format!("d{i}"), f.clone(), "xy_cut", 1.0))
                .collect();
            let table = learn_decision_table(
                DecisionPoint::GrammarRoute,
                &observations,
                TableLearnConfig { min_support: 2 },
            )
            .expect("covered");
            let cell = table.lookup_with_fallback(&f).expect("covered");
            prop_assert_eq!(cell.code.as_str(), "xy_cut");
        }
    }

    #[test]
    fn rouge_l_reference_values() {
        assert_eq!(rouge_l("", "abc"), 0.0);
        assert_eq!(rouge_l("abc", ""), 0.0);
        assert!((rouge_l("the cat sat", "the cat sat") - 1.0).abs() < 1e-9);
        // Identical tokens in the same order: LCS = 2 of 3,3 -> F1 = 2*2/(3+3) = 0.667.
        let f = rouge_l("the cat runs", "the dog runs");
        assert!((f - 2.0 / 3.0).abs() < 1e-9);
        // Case-insensitive.
        assert!((rouge_l("THE CAT", "the cat") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rouge_l_grows_with_overlap() {
        let a = "alpha beta gamma delta epsilon";
        let more = "alpha beta gamma delta epsilon zeta";
        let less = "alpha beta";
        assert!(rouge_l(a, more) > rouge_l(a, less));
    }
}
