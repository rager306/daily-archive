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
            let g_type = g.entity_type.to_lowercase();
            // Find best predicted match: prefer exact, then substring.
            // Both passes require entity_type to match — a Method prediction
            // must not match a Task/Model gold even if labels overlap.
            // First pass: exact label match (case-insensitive) + type match.
            for (pi, p) in predicted.iter().enumerate() {
                if matched_pred[pi] {
                    continue;
                }
                if p.label.to_lowercase() == g_label && p.entity_type.to_lowercase() == g_type {
                    matched_gold[gi] = true;
                    matched_pred[pi] = true;
                    tp += 1;
                    break;
                }
            }
            if matched_gold[gi] {
                continue;
            }
            // Second pass: fuzzy substring match + type match.
            for (pi, p) in predicted.iter().enumerate() {
                if matched_pred[pi] {
                    continue;
                }
                if p.entity_type.to_lowercase() != g_type {
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

// ---------------------------------------------------------------------------
// E2 gold hygiene (M315): evidence-based promotion — no free invent
// ---------------------------------------------------------------------------

/// Key for case-insensitive (label, type) identity in gold inventories.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey {
    pub label_lower: String,
    pub type_lower: String,
}

impl EntityKey {
    pub fn new(label: &str, entity_type: &str) -> Self {
        Self {
            label_lower: label.to_lowercase(),
            type_lower: entity_type.to_lowercase(),
        }
    }
}

/// Labels that appear as gold in at least `min_papers` distinct papers.
/// Used as the only allowed source for gold promotion (constrained select).
#[derive(Debug, Clone, Default)]
pub struct MultiPaperGoldIndex {
    /// key → number of distinct papers where this (label,type) is gold
    pub paper_counts: std::collections::HashMap<EntityKey, usize>,
}

impl MultiPaperGoldIndex {
    /// Build from an iterator of (paper_id, gold entities).
    pub fn from_papers<'a, I>(papers: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a [GoldEntity])>,
    {
        use std::collections::{HashMap, HashSet};
        let mut papers_by_key: HashMap<EntityKey, HashSet<String>> = HashMap::new();
        for (paper_id, entities) in papers {
            for e in entities {
                let key = EntityKey::new(&e.label, &e.entity_type);
                papers_by_key
                    .entry(key)
                    .or_default()
                    .insert(paper_id.to_string());
            }
        }
        let paper_counts = papers_by_key
            .into_iter()
            .map(|(k, set)| (k, set.len()))
            .collect();
        Self { paper_counts }
    }

    pub fn paper_count(&self, label: &str, entity_type: &str) -> usize {
        self.paper_counts
            .get(&EntityKey::new(label, entity_type))
            .copied()
            .unwrap_or(0)
    }

    pub fn is_multi_paper(&self, label: &str, entity_type: &str, min_papers: usize) -> bool {
        self.paper_count(label, entity_type) >= min_papers
    }
}

/// Fuzzy FP list: predicted entities not matched to any gold (same rules as evaluate_fuzzy).
pub fn list_false_positives(
    gold: &[GoldEntity],
    predicted: &[PredictedEntity],
) -> Vec<PredictedEntity> {
    let m = ExtractionMetrics::evaluate_fuzzy(gold, predicted);
    // Recompute unmatched preds with same greedy matching as evaluate_fuzzy.
    let mut matched_pred = vec![false; predicted.len()];
    let mut matched_gold = vec![false; gold.len()];
    for (gi, g) in gold.iter().enumerate() {
        let g_label = g.label.to_lowercase();
        let g_type = g.entity_type.to_lowercase();
        for (pi, p) in predicted.iter().enumerate() {
            if matched_pred[pi] {
                continue;
            }
            if p.label.to_lowercase() == g_label && p.entity_type.to_lowercase() == g_type {
                matched_gold[gi] = true;
                matched_pred[pi] = true;
                break;
            }
        }
        if matched_gold[gi] {
            continue;
        }
        for (pi, p) in predicted.iter().enumerate() {
            if matched_pred[pi] {
                continue;
            }
            if p.entity_type.to_lowercase() != g_type {
                continue;
            }
            let p_label = p.label.to_lowercase();
            if g_label.contains(&p_label) || p_label.contains(&g_label) {
                matched_gold[gi] = true;
                matched_pred[pi] = true;
                break;
            }
        }
    }
    let _ = m;
    predicted
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_pred[*i])
        .map(|(_, p)| p.clone())
        .collect()
}

/// Evidence-based gold promotion candidates from FP list.
///
/// A predicted FP becomes a promote candidate only if:
/// 1. `(label, type)` is already gold in at least `min_papers` other papers
/// 2. It is not already present in this paper's gold (case-insensitive)
///
/// Never invents free labels (D127 / constrained select).
pub fn promote_candidates(
    paper_gold: &[GoldEntity],
    false_positives: &[PredictedEntity],
    index: &MultiPaperGoldIndex,
    min_papers: usize,
) -> Vec<GoldEntity> {
    let mut out = Vec::new();
    let mut seen_local = std::collections::HashSet::new();
    for g in paper_gold {
        seen_local.insert(EntityKey::new(&g.label, &g.entity_type));
    }
    for fp in false_positives {
        let key = EntityKey::new(&fp.label, &fp.entity_type);
        if seen_local.contains(&key) {
            continue;
        }
        if !index.is_multi_paper(&fp.label, &fp.entity_type, min_papers) {
            continue;
        }
        seen_local.insert(key);
        out.push(GoldEntity {
            label: fp.label.clone(),
            entity_type: fp.entity_type.clone(),
            section: None,
        });
    }
    out
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
    fn test_fuzzy_matches_type_only() {
        // Fuzzy matching must respect entity_type. A Method prediction must not
        // match a Task gold, even if its label is a substring of the gold label.
        let gold = vec![
            GoldEntity {
                label: "prompt optimization".to_string(),
                entity_type: "Task".to_string(),
                section: None,
            },
            GoldEntity {
                label: "GPT-4".to_string(),
                entity_type: "Model".to_string(),
                section: None,
            },
        ];
        let predicted = vec![
            PredictedEntity {
                label: "PRO".to_string(),
                entity_type: "Method".to_string(),
            },
            PredictedEntity {
                label: "GPT".to_string(),
                entity_type: "Method".to_string(),
            },
        ];
        let m = ExtractionMetrics::evaluate_fuzzy(&gold, &predicted);
        assert_eq!(m.true_positives, 0, "type mismatch must not count as TP");
        assert_eq!(m.false_negatives, 2);
        assert_eq!(m.recall, 0.0);
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

    // --- M315 E2 evidence-based gold promote ---

    #[test]
    fn test_list_false_positives_fuzzy() {
        let gold = vec![GoldEntity {
            label: "GPT-4".into(),
            entity_type: "Model".into(),
            section: None,
        }];
        let predicted = vec![
            PredictedEntity {
                label: "gpt-4".into(),
                entity_type: "Model".into(),
            },
            PredictedEntity {
                label: "Qwen".into(),
                entity_type: "Model".into(),
            },
        ];
        let fps = list_false_positives(&gold, &predicted);
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].label, "Qwen");
    }

    #[test]
    fn test_promote_only_multi_paper_gold_labels() {
        // Corpus: CoT is gold in 2 papers; AGIEval is never gold.
        let p1 = vec![GoldEntity {
            label: "CoT".into(),
            entity_type: "Method".into(),
            section: None,
        }];
        let p2 = vec![GoldEntity {
            label: "CoT".into(),
            entity_type: "Method".into(),
            section: None,
        }];
        let index =
            MultiPaperGoldIndex::from_papers([("p1", p1.as_slice()), ("p2", p2.as_slice())]);
        assert!(index.is_multi_paper("CoT", "Method", 2));
        assert!(!index.is_multi_paper("AGIEval", "Dataset", 2));

        let paper_gold = vec![GoldEntity {
            label: "prompt optimization".into(),
            entity_type: "Task".into(),
            section: None,
        }];
        let fps = vec![
            PredictedEntity {
                label: "CoT".into(),
                entity_type: "Method".into(),
            },
            PredictedEntity {
                label: "AGIEval".into(),
                entity_type: "Dataset".into(),
            },
        ];
        let promoted = promote_candidates(&paper_gold, &fps, &index, 2);
        assert_eq!(promoted.len(), 1, "only multi-paper gold labels promote");
        assert_eq!(promoted[0].label, "CoT");
        assert_eq!(promoted[0].entity_type, "Method");
    }

    #[test]
    fn test_promote_rejects_already_in_paper_gold() {
        let p1 = vec![GoldEntity {
            label: "CoT".into(),
            entity_type: "Method".into(),
            section: None,
        }];
        let p2 = vec![GoldEntity {
            label: "CoT".into(),
            entity_type: "Method".into(),
            section: None,
        }];
        let index =
            MultiPaperGoldIndex::from_papers([("p1", p1.as_slice()), ("p2", p2.as_slice())]);
        let paper_gold = vec![GoldEntity {
            label: "cot".into(), // case-insensitive already present
            entity_type: "Method".into(),
            section: None,
        }];
        let fps = vec![PredictedEntity {
            label: "CoT".into(),
            entity_type: "Method".into(),
        }];
        let promoted = promote_candidates(&paper_gold, &fps, &index, 2);
        assert!(promoted.is_empty(), "must not duplicate existing gold");
    }

    #[test]
    fn test_promote_never_free_invents() {
        let index = MultiPaperGoldIndex::default();
        let fps = vec![PredictedEntity {
            label: "brand-new-widget".into(),
            entity_type: "Method".into(),
        }];
        let promoted = promote_candidates(&[], &fps, &index, 2);
        assert!(promoted.is_empty(), "free invent forbidden");
    }
}
