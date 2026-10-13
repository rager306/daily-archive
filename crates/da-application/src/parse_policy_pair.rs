//! Two-arm paired observations for policy learning (ADR-080, F4).
//!
//! Coverage for the learned policy table comes from measuring BOTH parse arms
//! on the SAME document under one frozen metric, then recording the winner and
//! the loser as observations. This module owns the pure part: the metric, the
//! pair record, and the verdicts. The two-arm run itself lives in the CLI.
//!
//! Nothing here touches the production route, the graph, or promotion. A pair
//! is candidate-only evidence.

use crate::parse_policy_table::{PolicyObservation, PolicyOutcomeEvidence, binary_reward};
use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PAIR_SCHEMA_VERSION: &str = "parse-pair.v1";
pub const PAIR_METRIC_SCHEMA_VERSION: &str = "parse-pair-metric.v1";

/// Which parse arm produced an outcome. Closed set (ADR-080).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseArm {
    /// Page-outline arm: PyMuPDF TOC observations -> pdf-toc stage.
    Toc,
    /// GROBID arm: arxiv_pdf_dot via the Java cascade.
    Grobid,
}

impl ParseArm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Toc => "toc",
            Self::Grobid => "grobid",
        }
    }

    /// The `grammar_route` code this arm corresponds to.
    pub fn decision_code(self) -> &'static str {
        match self {
            Self::Toc => "page_outline",
            Self::Grobid => "xy_cut",
        }
    }
}

/// The frozen pair metric (DATA; `data/policies/parse_pair_metric.yaml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairMetric {
    pub schema_version: String,
    pub adr_ref: String,
    /// A conservation loss on an arm vetoes that arm.
    pub conservation_first: bool,
    #[serde(rename = "yield")]
    pub yield_rule: YieldRule,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YieldRule {
    pub metric_one: String,
    pub metric_two: String,
    pub margin: i64,
    /// D511: normalise the yield by parse latency (chars per millisecond).
    /// The arms differ by an order of magnitude in speed; a latency-blind
    /// score would pick the slower arm on equal section counts.
    #[serde(default)]
    pub latency_aware: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum PairError {
    #[error("pair metric: YAML parse failed: {0}")]
    MetricParse(String),
    #[error("pair metric: unsupported schema version {0}")]
    MetricSchema(String),
    #[error("pair: an arm carries no outcome evidence")]
    MissingEvidence,
}

impl PairMetric {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, PairError> {
        let metric: PairMetric =
            serde_yaml::from_str(yaml).map_err(|e| PairError::MetricParse(e.to_string()))?;
        if metric.schema_version != PAIR_METRIC_SCHEMA_VERSION {
            return Err(PairError::MetricSchema(metric.schema_version));
        }
        Ok(metric)
    }

    /// Content digest of the metric; part of the pair identity.
    pub fn digest(&self) -> String {
        use sha2::Digest;
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = sha2::Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }

    /// Yield of one arm under this metric: (metric_one, metric_two).
    /// With `latency_aware` the primary axis is characters per millisecond
    /// (scaled by 1000 to stay integral), comparing throughput, not volume.
    pub fn yield_of(&self, evidence: &PolicyOutcomeEvidence) -> (i64, i64) {
        let latency = evidence.latency_ms.max(1) as i64;
        let scale = |v: i64| -> i64 {
            if self.yield_rule.latency_aware {
                v * 1000 / latency
            } else {
                v
            }
        };
        let one = scale(match self.yield_rule.metric_one.as_str() {
            "section_count" => evidence.section_count as i64,
            "body_chars" => evidence.body_chars as i64,
            _ => 0,
        });
        let two = scale(match self.yield_rule.metric_two.as_str() {
            "section_count" => evidence.section_count as i64,
            "body_chars" => evidence.body_chars as i64,
            _ => 0,
        });
        (one, two)
    }
}

/// The verdict one arm earned in a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairVerdict {
    Winner,
    Loser,
    Tie,
    /// A conservation loss vetoed this arm regardless of yield.
    Vetoed,
    /// The arm cannot run on this document (e.g. the TOC arm needs outline
    /// bookmarks the PDF does not have). Not a loss: the choice was forced.
    NotApplicable,
}

/// One arm's measured outcome inside a pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmOutcome {
    pub arm: ParseArm,
    pub decision_code: String,
    pub verdict: PairVerdict,
    pub evidence: PolicyOutcomeEvidence,
}

/// One document measured on both arms under one frozen metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedObservation {
    pub schema_version: String,
    pub document_identity: String,
    pub features: ArticleFeatures,
    pub metric_digest: String,
    pub arms: [ArmOutcome; 2],
}

/// Score two arm outcomes under a frozen metric and assign verdicts.
///
/// Rules (ADR-080 D509):
/// - conservation first: an arm whose evidence is not `ok` is Vetoed;
/// - among non-vetoed arms, the higher yield wins; a margin of 0 makes equal
///   yields a Tie; both arms lose if both are vetoed.
pub fn score_pair(
    metric: &PairMetric,
    toc: (ParseArm, String, PolicyOutcomeEvidence),
    grobid: (ParseArm, String, PolicyOutcomeEvidence),
) -> PairedObservation {
    let (toc_arm, toc_code, toc_ev) = toc;
    let (gb_arm, gb_code, gb_ev) = grobid;
    let toc_veto = metric.conservation_first && !toc_ev.ok;
    let gb_veto = metric.conservation_first && !gb_ev.ok;
    let toc_yield = metric.yield_of(&toc_ev);
    let gb_yield = metric.yield_of(&gb_ev);
    let margin = metric.yield_rule.margin;

    let (toc_verdict, gb_verdict) = if toc_veto && gb_veto {
        (PairVerdict::Vetoed, PairVerdict::Vetoed)
    } else if toc_veto {
        (PairVerdict::Vetoed, PairVerdict::Winner)
    } else if gb_veto {
        (PairVerdict::Winner, PairVerdict::Vetoed)
    } else {
        let toc_wins = toc_yield.0 > gb_yield.0 + margin
            || (toc_yield.0 == gb_yield.0 && toc_yield.1 > gb_yield.1 + margin);
        let gb_wins = gb_yield.0 > toc_yield.0 + margin
            || (gb_yield.0 == toc_yield.0 && gb_yield.1 > toc_yield.1 + margin);
        match (toc_wins, gb_wins) {
            (true, false) => (PairVerdict::Winner, PairVerdict::Loser),
            (false, true) => (PairVerdict::Loser, PairVerdict::Winner),
            _ => (PairVerdict::Tie, PairVerdict::Tie),
        }
    };

    PairedObservation {
        schema_version: PAIR_SCHEMA_VERSION.to_string(),
        document_identity: String::new(), // filled by the caller (has the id)
        features: ArticleFeatures {
            page_count: 0,
            toc_bookmarks: 0,
            text_layer_quality: 0.0,
            domain: String::new(),
        },
        metric_digest: metric.digest(),
        arms: [
            ArmOutcome {
                arm: toc_arm,
                decision_code: toc_code,
                verdict: toc_verdict,
                evidence: toc_ev,
            },
            ArmOutcome {
                arm: gb_arm,
                decision_code: gb_code,
                verdict: gb_verdict,
                evidence: gb_ev,
            },
        ],
    }
}

/// Score a pair where either arm may be **not applicable** (ADR-080 §5
/// correction). A document with no outline cannot be parsed by the TOC arm, so
/// that arm is `NotApplicable` and the other wins if it produced evidence.
/// Returns `None` when neither arm produced an outcome — there is nothing to
/// pair.
pub fn score_pair_optional(
    metric: &PairMetric,
    toc: Option<(ParseArm, String, PolicyOutcomeEvidence)>,
    grobid: Option<(ParseArm, String, PolicyOutcomeEvidence)>,
) -> Option<PairedObservation> {
    let toc_absent = toc.is_none();
    let gb_absent = grobid.is_none();
    let (toc_arm, toc_code, toc_ev) = toc.unwrap_or((
        ParseArm::Toc,
        ParseArm::Toc.decision_code().to_string(),
        PolicyOutcomeEvidence {
            ok: false,
            section_count: 0,
            body_chars: 0,
            latency_ms: 0,
            backend: None,
        },
    ));
    let (gb_arm, gb_code, gb_ev) = grobid.unwrap_or((
        ParseArm::Grobid,
        ParseArm::Grobid.decision_code().to_string(),
        PolicyOutcomeEvidence {
            ok: false,
            section_count: 0,
            body_chars: 0,
            latency_ms: 0,
            backend: None,
        },
    ));
    // Build the base pair, then override the verdict of an absent arm.
    let mut pair = score_pair(
        metric,
        (toc_arm, toc_code, toc_ev),
        (gb_arm, gb_code, gb_ev),
    );
    if toc_absent {
        pair.arms[0].verdict = PairVerdict::NotApplicable;
    }
    if gb_absent {
        pair.arms[1].verdict = PairVerdict::NotApplicable;
    }
    // If exactly one arm is applicable and it produced evidence, it is the
    // winner; if both are absent there is nothing to pair.
    if toc_absent && gb_absent {
        return None;
    }
    // D510: the absent arm is NotApplicable; the present arm is Winner ONLY if
    // it passed the conservation gate (ok). A vetoed arm is never a Winner.
    if toc_absent && !gb_absent && pair.arms[1].verdict != PairVerdict::Vetoed {
        pair.arms[1].verdict = PairVerdict::Winner;
    }
    if gb_absent && !toc_absent && pair.arms[0].verdict != PairVerdict::Vetoed {
        pair.arms[0].verdict = PairVerdict::Winner;
    }
    Some(pair)
}

/// Project a scored pair into the observations the trainer consumes: the
/// winner earns reward 1.0, the loser 0.0, a tie both 0.5, a veto 0.0. Both
/// arms are emitted so the table sees a genuine counterfactual.
pub fn pair_to_observations(pair: &PairedObservation) -> Vec<PolicyObservation> {
    pair.arms
        .iter()
        .filter(|arm| arm.verdict != PairVerdict::NotApplicable)
        .map(|arm| {
            let reward = match arm.verdict {
                PairVerdict::Winner => binary_reward(&arm.evidence).max(1.0),
                PairVerdict::Loser | PairVerdict::Vetoed => 0.0,
                PairVerdict::Tie => 0.5,
                PairVerdict::NotApplicable => 0.0, // filtered out above
            };
            PolicyObservation {
                document_identity: pair.document_identity.clone(),
                decision_point: DecisionPoint::GrammarRoute,
                features: pair.features.clone(),
                decision_code: arm.decision_code.clone(),
                reward,
                outcome: Some(arm.evidence.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn m_blind() -> PairMetric {
        let mut m = metric();
        m.yield_rule.latency_aware = false;
        m
    }

    fn metric() -> PairMetric {
        PairMetric::from_yaml_str(
            "schema_version: parse-pair-metric.v1\nadr_ref: ADR-080\nconservation_first: true\nyield:\n  latency_aware: true\n  metric_one: section_count\n  metric_two: body_chars\n  margin: 0\n",
        )
        .unwrap()
    }

    fn ev(ok: bool, sections: usize, chars: usize) -> PolicyOutcomeEvidence {
        PolicyOutcomeEvidence {
            ok,
            section_count: sections,
            body_chars: chars,
            latency_ms: 1000,
            backend: Some("test".into()),
        }
    }

    fn pair_with(toc: PolicyOutcomeEvidence, gb: PolicyOutcomeEvidence) -> PairedObservation {
        let m = metric();
        let mut pair = score_pair(
            &m,
            (ParseArm::Toc, "page_outline".into(), toc),
            (ParseArm::Grobid, "xy_cut".into(), gb),
        );
        pair.document_identity = "doc-1".into();
        pair
    }

    #[test]
    fn higher_section_yield_wins() {
        let pair = pair_with(ev(true, 22, 100), ev(true, 15, 100));
        assert_eq!(pair.arms[0].verdict, PairVerdict::Winner);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Loser);
    }

    #[test]
    fn equal_sections_tiebreak_on_chars() {
        let pair = pair_with(ev(true, 15, 100), ev(true, 15, 200));
        assert_eq!(pair.arms[0].verdict, PairVerdict::Loser);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Winner);
    }

    #[test]
    fn equal_on_both_is_a_tie() {
        let pair = pair_with(ev(true, 15, 100), ev(true, 15, 100));
        assert_eq!(pair.arms[0].verdict, PairVerdict::Tie);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Tie);
    }

    #[test]
    fn conservation_loss_vetoes_regardless_of_yield() {
        // TOC has far more sections but failed -> vetoed; GROBID wins.
        let pair = pair_with(ev(false, 999, 9999), ev(true, 5, 100));
        assert_eq!(pair.arms[0].verdict, PairVerdict::Vetoed);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Winner);
    }

    #[test]
    fn both_vetoed_is_not_a_winner() {
        let pair = pair_with(ev(false, 1, 1), ev(false, 2, 2));
        assert_eq!(pair.arms[0].verdict, PairVerdict::Vetoed);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Vetoed);
        assert!(pair_to_observations(&pair).iter().all(|o| o.reward == 0.0));
    }

    #[test]
    fn pair_projects_to_two_counterfactual_observations() {
        let pair = pair_with(ev(true, 22, 100), ev(true, 15, 100));
        let observations = pair_to_observations(&pair);
        assert_eq!(observations.len(), 2);
        // Same document, same features, two different codes: a real pair.
        assert_eq!(
            observations[0].document_identity,
            observations[1].document_identity
        );
        assert_eq!(observations[0].features, observations[1].features);
        assert_ne!(observations[0].decision_code, observations[1].decision_code);
        let winner = observations.iter().find(|o| o.reward == 1.0).unwrap();
        assert_eq!(winner.decision_code, "page_outline");
    }

    #[test]
    fn metric_digest_is_content_derived_and_stable() {
        let a = metric();
        let b = metric();
        assert_eq!(a.digest(), b.digest());
        let mut changed = metric();
        changed.yield_rule.margin = 5;
        assert_ne!(
            a.digest(),
            changed.digest(),
            "a metric edit is a new identity"
        );
    }

    #[test]
    fn metric_rejects_wrong_schema() {
        let err = PairMetric::from_yaml_str(
            "schema_version: nope\nadr_ref: x\nconservation_first: true\nyield:\n  metric_one: section_count\n  metric_two: body_chars\n  margin: 0\n",
        )
        .unwrap_err();
        assert!(matches!(err, PairError::MetricSchema(_)));
    }

    #[test]
    fn optional_pair_marks_absent_toc_and_crowns_grobid() {
        let m = metric();
        let pair = score_pair_optional(
            &m,
            None, // no outline -> TOC arm not applicable
            Some((ParseArm::Grobid, "xy_cut".into(), ev(true, 22, 100))),
        )
        .expect("one applicable arm is a pair");
        assert_eq!(pair.arms[0].verdict, PairVerdict::NotApplicable);
        assert_eq!(pair.arms[1].verdict, PairVerdict::Winner);
        // Only the applicable arm becomes an observation.
        let obs = pair_to_observations(&pair);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].decision_code, "xy_cut");
        assert_eq!(obs[0].reward, 1.0);
    }

    #[test]
    fn optional_pair_marks_absent_grobid_and_crowns_toc() {
        let m = metric();
        let pair = score_pair_optional(
            &m,
            Some((ParseArm::Toc, "page_outline".into(), ev(true, 15, 100))),
            None,
        )
        .unwrap();
        assert_eq!(pair.arms[0].verdict, PairVerdict::Winner);
        assert_eq!(pair.arms[1].verdict, PairVerdict::NotApplicable);
        let obs = pair_to_observations(&pair);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].decision_code, "page_outline");
    }

    #[test]
    fn optional_pair_with_both_absent_is_not_a_pair() {
        let m = metric();
        assert!(score_pair_optional(&m, None, None).is_none());
    }

    proptest::proptest! {
        /// D511: latency-aware yield rewards the faster arm when volumes are
        /// equal, regardless of the absolute latency values.
        #[test]
        fn latency_aware_prefers_the_faster_arm(
            sections in 1usize..500,
            chars in 1000usize..900_000,
            fast_ms in 1u64..500,
            slow_ms in 5000u64..60_000,
        ) {
            let m = metric();
            let fast = ev(true, sections, chars);
            let mut slow = ev(true, sections, chars);
            slow.latency_ms = slow_ms;
            let mut fast = fast;
            fast.latency_ms = fast_ms;
            let pair = score_pair(
                &m,
                (ParseArm::Toc, "page_outline".into(), fast),
                (ParseArm::Grobid, "xy_cut".into(), slow),
            );
            prop_assert_eq!(pair.arms[0].verdict, PairVerdict::Winner);
            // The latency-aware yield of the fast arm strictly exceeds the slow
            // arm's on every axis.
            prop_assert!(m.yield_of(&pair.arms[0].evidence).0 > m.yield_of(&pair.arms[1].evidence).0);
        }

        /// Latency-blind mode is unchanged: equal volumes with margin 0 tie.
        #[test]
        fn latency_blind_mode_unchanged(
            sections in 1usize..500,
            chars in 1000usize..900_000,
        ) {
            let ev = ev(true, sections, chars);
            let pair = score_pair(
                &m_blind(),
                (ParseArm::Toc, "page_outline".into(), ev.clone()),
                (ParseArm::Grobid, "xy_cut".into(), ev),
            );
            prop_assert_eq!(pair.arms[0].verdict, PairVerdict::Tie);
        }
    }
}
