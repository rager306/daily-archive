//! Extraction evaluation metrics (D136).
//!
//! GSD memory: "do not enable DSPy until evaluation metrics and benchmark
//! fixtures are designed and verified. S07 metrics/ablations must come
//! before DSPy claims or optimizer use."
//!
//! This module provides precision/recall/F1 evaluation for extraction
//! against gold-standard fixtures.

use serde::{Deserialize, Serialize};

/// A gold-standard entity for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldEntity {
    pub label: String,
    pub entity_type: String,
    pub section: Option<String>,
}

/// A predicted entity from extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedEntity {
    pub label: String,
    pub entity_type: String,
}

/// Evaluation metrics for one extraction run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetrics {
    pub gold_count: usize,
    pub predicted_count: usize,
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

impl ExtractionMetrics {
    /// Evaluate predicted entities against gold standard.
    /// Match is case-insensitive on label + type.
    pub fn evaluate(gold: &[GoldEntity], predicted: &[PredictedEntity]) -> Self {
        let mut tp = 0usize;
        let mut fp = 0usize;

        for pred in predicted {
            let pred_label = pred.label.to_lowercase();
            let pred_type = pred.entity_type.to_lowercase();
            let matched = gold.iter().any(|g| {
                g.label.to_lowercase() == pred_label && g.entity_type.to_lowercase() == pred_type
            });
            if matched {
                tp += 1;
            } else {
                fp += 1;
            }
        }

        let fn_count = gold
            .iter()
            .filter(|g| {
                let g_label = g.label.to_lowercase();
                let g_type = g.entity_type.to_lowercase();
                !predicted.iter().any(|p| {
                    p.label.to_lowercase() == g_label && p.entity_type.to_lowercase() == g_type
                })
            })
            .count();

        let precision = if predicted.is_empty() {
            0.0
        } else {
            tp as f64 / predicted.len() as f64
        };
        let recall = if gold.is_empty() {
            1.0
        } else {
            tp as f64 / gold.len() as f64
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };

        Self {
            gold_count: gold.len(),
            predicted_count: predicted.len(),
            true_positives: tp,
            false_positives: fp,
            false_negatives: fn_count,
            precision,
            recall,
            f1,
        }
    }

    /// Print metrics in human-readable format.
    pub fn report(&self) -> String {
        format!(
            "Gold: {} | Predicted: {} | TP: {} | FP: {} | FN: {}\nP={:.3} R={:.3} F1={:.3}",
            self.gold_count,
            self.predicted_count,
            self.true_positives,
            self.false_positives,
            self.false_negatives,
            self.precision,
            self.recall,
            self.f1
        )
    }
}

/// Partial-match evaluation: label substring match counts as TP.
/// Useful for comparing "GPT-4" (gold) vs "gpt-4" (predicted).
impl ExtractionMetrics {
    pub fn evaluate_fuzzy(gold: &[GoldEntity], predicted: &[PredictedEntity]) -> Self {
        // One-to-one matching: each gold entity is matched by at most one
        // predicted entity, and each predicted entity matches at most one gold.
        // This prevents recall > 1.0 when multiple predicted variants (GPT-4,
        // gpt-4, Gpt-4) all match the same gold entity.
        //
        // Greedy strategy: for each gold entity, find the first unused predicted
        // entity that fuzzy-matches (substring either direction, case-insensitive).
        let mut matched_gold = vec![false; gold.len()];
        let mut matched_pred = vec![false; predicted.len()];
        let mut tp = 0usize;

        for (gi, g) in gold.iter().enumerate() {
            let g_label = g.label.to_lowercase();
            // Find best predicted match: prefer exact, then substring.
            // First pass: exact label match (case-insensitive).
            for (pi, p) in predicted.iter().enumerate() {
                if matched_pred[pi] {
                    continue;
                }
                if p.label.to_lowercase() == g_label {
                    matched_gold[gi] = true;
                    matched_pred[pi] = true;
                    tp += 1;
                    break;
                }
            }
            if matched_gold[gi] {
                continue;
            }
            // Second pass: fuzzy substring match.
            for (pi, p) in predicted.iter().enumerate() {
                if matched_pred[pi] {
                    continue;
                }
                let p_label = p.label.to_lowercase();
                if g_label.contains(&p_label) || p_label.contains(&g_label) {
                    matched_gold[gi] = true;
                    matched_pred[pi] = true;
                    tp += 1;
                    break;
                }
            }
        }

        let fp = predicted.len() - tp;
        let fn_count = gold.len() - tp;

        let precision = if predicted.is_empty() {
            0.0
        } else {
            tp as f64 / predicted.len() as f64
        };
        let recall = if gold.is_empty() {
            1.0
        } else {
            tp as f64 / gold.len() as f64
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };

        Self {
            gold_count: gold.len(),
            predicted_count: predicted.len(),
            true_positives: tp,
            false_positives: fp,
            false_negatives: fn_count,
            precision,
            recall,
            f1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_match() {
        let gold = vec![
            GoldEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
            GoldEntity {
                label: "WMT".to_string(),
                entity_type: "Dataset".to_string(),
                section: None,
            },
        ];
        let predicted = vec![
            PredictedEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "WMT".to_string(),
                entity_type: "Dataset".to_string(),
            },
        ];
        let m = ExtractionMetrics::evaluate(&gold, &predicted);
        assert_eq!(m.true_positives, 2);
        assert_eq!(m.precision, 1.0);
        assert_eq!(m.recall, 1.0);
        assert_eq!(m.f1, 1.0);
    }

    #[test]
    fn test_partial_recall() {
        let gold = vec![
            GoldEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
            GoldEntity {
                label: "BERT".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
            GoldEntity {
                label: "WMT".to_string(),
                entity_type: "Dataset".to_string(),
                section: None,
            },
        ];
        let predicted = vec![PredictedEntity {
            label: "GPT-4".to_string(),
            entity_type: "Model".to_string(),
        }];
        let m = ExtractionMetrics::evaluate(&gold, &predicted);
        assert_eq!(m.true_positives, 1);
        assert_eq!(m.false_negatives, 2);
        assert!((m.recall - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_false_positive() {
        let gold = vec![GoldEntity {
            label: "GPT-4".to_string(),
            entity_type: "Model".to_string(),
            section: None,
        }];
        let predicted = vec![
            PredictedEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "NonExistent".to_string(),
                entity_type: "Dataset".to_string(),
            },
        ];
        let m = ExtractionMetrics::evaluate(&gold, &predicted);
        assert_eq!(m.false_positives, 1);
        assert!((m.precision - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_empty_predicted() {
        let gold = vec![GoldEntity {
            label: "GPT-4".to_string(),
            entity_type: "Model".to_string(),
            section: None,
        }];
        let m = ExtractionMetrics::evaluate(&gold, &[]);
        assert_eq!(m.precision, 0.0);
        assert_eq!(m.recall, 0.0);
        assert_eq!(m.f1, 0.0);
    }

    #[test]
    fn test_fuzzy_match() {
        let gold = vec![GoldEntity {
            label: "GPT-4".to_string(),
            entity_type: "Model".to_string(),
            section: None,
        }];
        let predicted = vec![PredictedEntity {
            label: "gpt-4".to_string(),
            entity_type: "Model".to_string(),
        }];
        let m = ExtractionMetrics::evaluate_fuzzy(&gold, &predicted);
        assert_eq!(m.true_positives, 1);
        assert_eq!(m.precision, 1.0);
    }

    #[test]
    fn test_fuzzy_one_to_one_no_duplicate_tp() {
        // Multiple predicted entities matching the SAME gold entity must NOT
        // all count as TP. One-to-one matching: each gold entity is matched
        // by at most one predicted entity.
        // Before fix: 3 predicted "gpt-4" variants → tp=3, recall=3.0 (impossible).
        // After fix: 3 predicted → tp=1, fp=2, recall=1.0.
        let gold = vec![GoldEntity {
            label: "GPT-4".to_string(),
            entity_type: "Model".to_string(),
            section: None,
        }];
        let predicted = vec![
            PredictedEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "gpt-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "Gpt-4".to_string(),
                entity_type: "Model".to_string(),
            },
        ];
        let m = ExtractionMetrics::evaluate_fuzzy(&gold, &predicted);
        assert_eq!(m.true_positives, 1, "only one TP for one gold entity");
        assert_eq!(m.false_positives, 2, "remaining 2 are FP");
        assert_eq!(m.false_negatives, 0);
        assert_eq!(m.recall, 1.0, "recall must not exceed 1.0");
        assert!((m.precision - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_fuzzy_recall_never_exceeds_one() {
        // Recall > 1.0 is mathematically impossible. Verify the metric
        // enforces this invariant even with many overlapping predictions.
        let gold = vec![
            GoldEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
            GoldEntity {
                label: "BERT".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
        ];
        let predicted = vec![
            PredictedEntity {
                label: "gpt-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "bert".to_string(),
                entity_type: "Model".to_string(),
            },
            PredictedEntity {
                label: "BERT".to_string(),
                entity_type: "Model".to_string(),
            },
        ];
        let m = ExtractionMetrics::evaluate_fuzzy(&gold, &predicted);
        assert!(m.recall <= 1.0, "recall {} must be <= 1.0", m.recall);
        assert_eq!(m.true_positives, 2, "one TP per gold entity");
        assert_eq!(m.false_positives, 2);
    }

    #[test]
    fn test_report_format() {
        let m = ExtractionMetrics {
            gold_count: 10,
            predicted_count: 8,
            true_positives: 6,
            false_positives: 2,
            false_negatives: 4,
            precision: 0.75,
            recall: 0.6,
            f1: 0.667,
        };
        let r = m.report();
        assert!(r.contains("P=0.750"));
        assert!(r.contains("R=0.600"));
        assert!(r.contains("F1=0.667"));
    }
}
