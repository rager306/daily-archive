//! Rule-based entity extractor — implements Extractor port.
//!
//! ADR-038 Module B Phase 3 Slice 1: deterministic rules, no ML.
//! Detects entity mentions via section heuristics + keyword patterns.
//! GLiNER 2 replaces this later (Slice 3) for higher recall.

use async_trait::async_trait;
use da_domain::entity::EntityType;
use da_ports::extractor::{ExtractResult, ExtractedEntity, Extractor};

/// Rule-based extractor using section titles + keyword heuristics.
pub struct RuleBasedExtractor;

impl RuleBasedExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Classify a section title to an entity type.
    /// e.g. "Datasets" → Dataset, "Methods" → Method.
    fn classify_section(title: &str) -> Option<EntityType> {
        let lower = title.to_lowercase();
        if lower.contains("dataset") || lower.contains("corpus") {
            Some(EntityType::Dataset)
        } else if lower.contains("method") || lower.contains("approach") || lower.contains("model")
        {
            if lower.contains("model") {
                Some(EntityType::Model)
            } else {
                Some(EntityType::Method)
            }
        } else if lower.contains("baseline") {
            Some(EntityType::Baseline)
        } else if lower.contains("metric") || lower.contains("evaluation") {
            Some(EntityType::Metric)
        } else if lower.contains("task") || lower.contains("problem") {
            Some(EntityType::Task)
        } else {
            None
        }
    }

    /// Extract candidate entity labels from section text using heuristics.
    /// Looks for capitalized phrases, quoted terms, and "we propose X" patterns.
    fn extract_candidates(text: &str, entity_type: &EntityType) -> Vec<(usize, usize, String)> {
        let mut results = Vec::new();

        // Pattern 1: "we propose X" / "we use X" / "we present X" (case-insensitive)
        for pattern in &["we propose ", "we present ", "we introduce ", "we use "] {
            let lower_text = text.to_lowercase();
            let mut start = 0;
            while let Some(pos) = lower_text[start..].find(pattern) {
                let abs = start + pos + pattern.len();
                if abs >= text.len() {
                    break;
                }
                // Extract the next ~60 chars or until period
                let remainder = &text[abs..];
                let end = remainder.find('.').unwrap_or(remainder.len()).min(60);
                let candidate = remainder[..end].trim();
                if !candidate.is_empty() && candidate.len() > 2 {
                    let label_end = abs + end;
                    results.push((abs, label_end, candidate.to_string()));
                }
                start = abs + 1;
            }
        }

        // Pattern 2: Section-type-specific keywords
        match entity_type {
            EntityType::Dataset => {
                // "on the X dataset" / "using X"
                for pattern in &["dataset", "corpus", "benchmark"] {
                    let lower = text.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(pattern) {
                        let abs = start + pos;
                        // Take 40 chars around the keyword
                        let s = abs.saturating_sub(20);
                        let e = (abs + 20).min(text.len());
                        let surface = &text[s..e];
                        results.push((s, e, surface.trim().to_string()));
                        start = abs + pattern.len();
                    }
                }
            }
            EntityType::Metric => {
                for pattern in &["accuracy", "precision", "recall", "f1", "bleu", "rouge"] {
                    let lower = text.to_lowercase();
                    if let Some(pos) = lower.find(pattern) {
                        let s = pos.saturating_sub(10);
                        let e = (pos + pattern.len() + 10).min(text.len());
                        results.push((s, e, text[s..e].trim().to_string()));
                    }
                }
            }
            _ => {}
        }

        // Deduplicate by label
        let mut seen = std::collections::HashSet::new();
        results.retain(|(_, _, label)| {
            let key = label.to_lowercase();
            if seen.contains(&key) {
                false
            } else {
                seen.insert(key);
                true
            }
        });

        results
    }
}

impl Default for RuleBasedExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Extractor for RuleBasedExtractor {
    async fn extract(&self, sections: &[(String, String)]) -> ExtractResult<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        for (title, text) in sections {
            if let Some(entity_type) = Self::classify_section(title) {
                let candidates = Self::extract_candidates(text, &entity_type);
                for (char_start, char_end, label) in candidates {
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: entity_type.clone(),
                        section_title: title.clone(),
                        char_start,
                        char_end,
                        surface: text[char_start.min(text.len())..char_end.min(text.len())]
                            .to_string(),
                    });
                }
            }
        }

        Ok(entities)
    }

    fn name(&self) -> &str {
        "rule-based"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_section() {
        assert_eq!(
            RuleBasedExtractor::classify_section("Datasets"),
            Some(EntityType::Dataset)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("Method"),
            Some(EntityType::Method)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("Our Model"),
            Some(EntityType::Model)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("Baselines"),
            Some(EntityType::Baseline)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("Evaluation Metrics"),
            Some(EntityType::Metric)
        );
        assert_eq!(RuleBasedExtractor::classify_section("Introduction"), None);
    }

    #[test]
    fn test_extract_candidates_propose() {
        let text = "In this paper we propose GeoRLE, a novel approach for layout analysis.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Method);
        assert!(!candidates.is_empty());
        assert!(candidates[0].2.contains("GeoRLE"));
    }

    #[test]
    fn test_extract_candidates_dataset() {
        let text = "We evaluate on the PubMed dataset and the arXiv corpus.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Dataset);
        assert!(!candidates.is_empty());
        // Should find "dataset" and "corpus" mentions
        assert!(candidates.iter().any(|(_, _, l)| l.contains("PubMed")));
    }

    #[tokio::test]
    async fn test_extract_from_sections() {
        let extractor = RuleBasedExtractor::new();
        let sections = vec![
            (
                "Method".to_string(),
                "We propose TransformerX for sequence modeling.".to_string(),
            ),
            (
                "Datasets".to_string(),
                "We evaluate on the WMT dataset.".to_string(),
            ),
            (
                "Introduction".to_string(),
                "This paper has no extractable entities.".to_string(),
            ),
        ];
        let entities = extractor.extract(&sections).await.unwrap();
        assert!(!entities.is_empty());
        assert!(entities.iter().any(|e| e.label.contains("TransformerX")));
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Dataset));
        // Introduction should produce no entities
        assert!(!entities.iter().any(|e| e.section_title == "Introduction"));
    }

    #[tokio::test]
    async fn test_extract_empty_sections() {
        let extractor = RuleBasedExtractor::new();
        let sections: Vec<(String, String)> = vec![];
        let entities = extractor.extract(&sections).await.unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn test_name() {
        let extractor = RuleBasedExtractor::new();
        assert_eq!(extractor.name(), "rule-based");
    }
}
