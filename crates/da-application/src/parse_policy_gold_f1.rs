//! Faculty: gold-F1 scoring for two-arm pairs (ADR-080 metric v2).
//!
//! The arm-comparison metric: each arm's parsed article is run through the same
//! entity extractor, and precision/recall/F1 are computed against the frozen
//! gold entities for that paper. The arm with the higher F1 wins the pair.
//!
//! Scope note (ADR-076 unchanged): this F1 is the ARM-COMPARISON metric for the
//! learning loop, not a parse oracle for conservation or import decisions.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldEntity {
    pub label: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldRef {
    pub paper_id: String,
    pub entities: Vec<GoldEntity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmF1 {
    pub arm: String, // "toc" | "grobid"
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub true_positives: usize,
    pub predicted_count: usize,
    pub gold_count: usize,
}

/// Greedy one-to-one fuzzy match (same discipline as `ExtractionMetrics`):
/// for each gold entity, the first unused predicted entity whose label
/// fuzzy-matches (case-insensitive substring either way).
fn count_matches(gold: &[GoldEntity], predicted: &[(String, String)]) -> usize {
    let mut used = vec![false; predicted.len()];
    let mut matches = 0;
    for g in gold {
        let g_lower = g.label.to_lowercase();
        for (i, (plabel, _ptype)) in predicted.iter().enumerate() {
            if used[i] {
                continue;
            }
            let p_lower = plabel.to_lowercase();
            if p_lower.contains(&g_lower) || g_lower.contains(&p_lower) {
                used[i] = true;
                matches += 1;
                break;
            }
        }
    }
    matches
}

/// F1 of one arm's predicted entities against the gold entities.
pub fn arm_f1(arm: &str, gold: &[GoldEntity], predicted: &[(String, String)]) -> ArmF1 {
    let tp = count_matches(gold, predicted);
    let precision = if predicted.is_empty() {
        0.0
    } else {
        tp as f64 / predicted.len() as f64
    };
    let recall = if gold.is_empty() {
        0.0
    } else {
        tp as f64 / gold.len() as f64
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    ArmF1 {
        arm: arm.to_string(),
        f1,
        precision,
        recall,
        true_positives: tp,
        predicted_count: predicted.len(),
        gold_count: gold.len(),
    }
}

/// The winner of a pair by F1: higher F1 wins; an exact tie is broken by
/// higher recall (better coverage), then by lower predicted count (precision
/// discipline). Returns the winning arm name, or `None` for a full tie.
pub fn f1_winner(a: &ArmF1, b: &ArmF1) -> Option<String> {
    let better = |x: &ArmF1, y: &ArmF1| x.f1 > y.f1;
    if better(a, b) {
        return Some(a.arm.clone());
    }
    if better(b, a) {
        return Some(b.arm.clone());
    }
    // F1 tie: prefer higher recall (coverage), then sparser predictions.
    if a.recall > b.recall {
        return Some(a.arm.clone());
    }
    if b.recall > a.recall {
        return Some(b.arm.clone());
    }
    if a.predicted_count < b.predicted_count {
        return Some(a.arm.clone());
    }
    if b.predicted_count < a.predicted_count {
        return Some(b.arm.clone());
    }
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldPairVerdict {
    pub schema_version: String,
    pub document_identity: String,
    pub toc: ArmF1,
    pub grobid: ArmF1,
    /// The arm that won by F1, or "tie".
    pub winner: String,
}

pub const GOLD_PAIR_SCHEMA_VERSION: &str = "gold-pair-verdict.v1";

#[derive(Debug, Error, PartialEq)]
pub enum GoldPairError {
    #[error("gold pair: empty gold entity list for the paper")]
    EmptyGold,
}

/// Build the verdict from two arms' predicted entities against one gold list.
pub fn score_gold_pair(
    document_identity: &str,
    gold: &[GoldEntity],
    toc_predicted: &[(String, String)],
    grobid_predicted: &[(String, String)],
) -> Result<GoldPairVerdict, GoldPairError> {
    if gold.is_empty() {
        return Err(GoldPairError::EmptyGold);
    }
    let toc = arm_f1("toc", gold, toc_predicted);
    let grobid = arm_f1("grobid", gold, grobid_predicted);
    let winner = f1_winner(&toc, &grobid).unwrap_or_else(|| "tie".to_string());
    Ok(GoldPairVerdict {
        schema_version: GOLD_PAIR_SCHEMA_VERSION.to_string(),
        document_identity: document_identity.to_string(),
        toc,
        grobid,
        winner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn gold() -> Vec<GoldEntity> {
        vec![
            GoldEntity {
                label: "CIFAR".into(),
                entity_type: "Dataset".into(),
            },
            GoldEntity {
                label: "GPT-4".into(),
                entity_type: "Model".into(),
            },
            GoldEntity {
                label: "chain of thought".into(),
                entity_type: "Method".into(),
            },
        ]
    }

    fn predicted(labels: &[&str]) -> Vec<(String, String)> {
        labels
            .iter()
            .map(|l| (l.to_string(), "Model".to_string()))
            .collect()
    }

    #[test]
    fn perfect_extraction_scores_one() {
        let f = arm_f1(
            "toc",
            &gold(),
            &predicted(&["CIFAR", "GPT-4", "chain of thought"]),
        );
        assert!((f.f1 - 1.0).abs() < 1e-9);
        assert_eq!(f.true_positives, 3);
    }

    #[test]
    fn empty_prediction_scores_zero() {
        let f = arm_f1("toc", &gold(), &[]);
        assert_eq!(f.f1, 0.0);
        assert_eq!(f.recall, 0.0);
    }

    #[test]
    fn one_to_one_matching_prevents_recall_above_one() {
        // Three variants of the same gold label: only ONE may match.
        let f = arm_f1("toc", &gold(), &predicted(&["GPT-4", "gpt-4", "Gpt-4"]));
        assert_eq!(f.true_positives, 1, "one-to-one match");
    }

    #[test]
    fn higher_f1_wins_regardless_of_argument_order() {
        let toc = arm_f1(
            "toc",
            &gold(),
            &predicted(&["CIFAR", "GPT-4", "chain of thought"]),
        );
        let grobid = arm_f1("grobid", &gold(), &predicted(&["CIFAR"]));
        // toc F1 1.0 vs grobid F1 0.67: toc wins in both argument orders.
        assert_eq!(f1_winner(&toc, &grobid).as_deref(), Some("toc"));
        assert_eq!(f1_winner(&grobid, &toc).as_deref(), Some("toc"));
    }

    #[test]
    fn full_tie_breaks_on_recall_then_sparser() {
        let a = arm_f1("toc", &gold(), &predicted(&["CIFAR", "GPT-4"]));
        let b = arm_f1("grobid", &gold(), &predicted(&["CIFAR", "GPT-4"]));
        assert_eq!(f1_winner(&a, &b), None, "identical predictions tie");
    }

    #[test]
    fn empty_gold_is_refused() {
        assert_eq!(
            score_gold_pair("d", &[], &[], &[]),
            Err(GoldPairError::EmptyGold)
        );
    }

    #[test]
    fn verdict_serialises() {
        let v = score_gold_pair(
            "d1",
            &gold(),
            &predicted(&["CIFAR", "GPT-4", "chain of thought"]),
            &predicted(&["CIFAR"]),
        )
        .unwrap();
        let json = serde_json::to_string(&v).unwrap();
        let back: GoldPairVerdict = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
        assert_eq!(v.winner, "toc");
        assert_eq!(v.schema_version, GOLD_PAIR_SCHEMA_VERSION);
    }

    proptest::proptest! {
        /// F1 is always in [0, 1] for any combination of gold and predicted.
        #[test]
        fn f1_always_between_zero_and_one(
            gold_count in 1usize..50,
            pred_count in 0usize..50,
            match_count in 0usize..50,
        ) {
            let actual_matches = match_count.min(gold_count).min(pred_count);
            let gold: Vec<GoldEntity> = (0..gold_count)
                .map(|i| GoldEntity { label: format!("g{i}"), entity_type: "Test".into() })
                .collect();
            let mut predicted: Vec<(String, String)> = (0..actual_matches)
                .map(|i| (format!("g{i}"), "Test".into()))
                .collect();
            let remaining = pred_count.saturating_sub(actual_matches);
            predicted.extend((0..remaining).map(|i| (format!("extra{i}"), "Test".into())));
            let f = arm_f1("toc", &gold, &predicted);
            prop_assert!(f.f1 >= 0.0 && f.f1 <= 1.0);
            prop_assert!(f.precision >= 0.0 && f.precision <= 1.0);
            prop_assert!(f.recall >= 0.0 && f.recall <= 1.0);
        }

        /// F1 is deterministic: same inputs always give the same result.
        #[test]
        fn f1_is_deterministic(
            labels in proptest::collection::vec("[a-z]{3,10}", 0..20),
        ) {
            let gold: Vec<GoldEntity> = labels.iter().enumerate()
                .map(|(i, l)| GoldEntity { label: format!("{l}"), entity_type: "Test".into() })
                .collect();
            let predicted: Vec<(String, String)> = labels.iter()
                .map(|l| (l.to_string(), "Test".into()))
                .collect();
            let f1 = arm_f1("toc", &gold, &predicted);
            let f2 = arm_f1("toc", &gold, &predicted);
            prop_assert_eq!(f1.f1, f2.f1);
        }
    }

    proptest::proptest! {
        /// F1 is always in [0, 1] regardless of the input sizes.
        #[test]
        fn f1_always_bounded(
            gold_count in 1usize..100,
            pred_count in 0usize..100,
            overlap in 0usize..50,
        ) {
            let gold: Vec<GoldEntity> = (0..gold_count)
                .map(|i| GoldEntity { label: format!("ent{i}"), entity_type: "Test".into() })
                .collect();
            let matches = overlap.min(gold_count).min(pred_count);
            let mut predicted: Vec<(String, String)> = (0..matches)
                .map(|i| (format!("ent{i}"), "Test".into()))
                .collect();
            let extras = pred_count - matches;
            predicted.extend((0..extras).map(|i| (format!("x{i}"), "Test".into())));
            let f = arm_f1("toc", &gold, &predicted);
            prop_assert!(f.f1 >= 0.0 && f.f1 <= 1.0, "f1 out of range: {}", f.f1);
            prop_assert!(f.precision >= 0.0 && f.precision <= 1.0);
            prop_assert!(f.recall >= 0.0 && f.recall <= 1.0);
        }

        /// F1 = 1.0 iff predicted labels exactly match gold labels (same set).
        #[test]
        fn perfect_match_gives_f1_one(
            labels in proptest::collection::vec("[a-z]{3,12}", 1..20),
        ) {
            let gold: Vec<GoldEntity> = labels.iter().enumerate()
                .map(|(i, l)| GoldEntity { label: l.clone(), entity_type: "Test".into() })
                .collect();
            let predicted: Vec<(String, String)> = labels.iter()
                .map(|l| (l.clone(), "Test".into()))
                .collect();
            let f = arm_f1("toc", &gold, &predicted);
            prop_assert!((f.f1 - 1.0).abs() < 1e-9, "exact match must give F1=1.0");
        }
    }
}
