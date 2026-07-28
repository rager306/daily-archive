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
    /// e.g. "Datasets" → Dataset, "Methods" → Method, "Evaluation Setup" → Metric.
    fn classify_section(title: &str) -> Option<EntityType> {
        let lower = title.to_lowercase();
        if lower.contains("dataset") || lower.contains("corpus") || lower.contains("benchmark") {
            Some(EntityType::Dataset)
        } else if lower.contains("baseline") {
            Some(EntityType::Baseline)
        } else if lower.contains("method")
            || lower.contains("approach")
            || lower.contains("algorithm")
            || lower.contains("methodology")
        {
            Some(EntityType::Method)
        } else if lower.contains("model")
            || lower.contains("inference")
            || lower.contains("parameter")
        {
            Some(EntityType::Model)
        } else if lower.contains("metric")
            || lower.contains("evaluation")
            || lower.contains("result")
            || lower.contains("experiment")
            || lower.contains("setup")
            || lower.contains("analysis")
        {
            Some(EntityType::Metric)
        } else if lower.contains("task") || lower.contains("problem") {
            Some(EntityType::Task)
        } else {
            None
        }
    }

    /// Global Method acronym extractor — scans text for KNOWN method acronyms.
    /// Used to find method acronyms (GSEM, GEPA, GRPO, RLVR) that appear in
    /// Abstract/Introduction (unclassified sections) rather than Method sections.
    /// Restricted to a whitelist to avoid false positives from all-caps scan.
    fn extract_method_acronyms_global(text: &str) -> Vec<(usize, usize, String)> {
        let mut results = Vec::new();
        // Known method acronyms in RL/optimization/prompt-engineering literature.
        // Whitelist-based to keep precision high (global scan of all all-caps words
        // produced 20+ false positives per paper).
        let known_methods = [
            // RL / optimization methods
            "GEPA", "GRPO", "RLVR", "PPO", "DPO", "KTO", "SILVER",
            // Prompt optimization / memory
            "GSEM", "PEM", "OPRO", "PRO", "EoT", // Reasoning
            "CoT", "ToT",
        ];
        for method in &known_methods {
            let mut start = 0;
            while let Some(pos) = text[start..].find(method) {
                let abs = start + pos;
                // Check word boundary: previous and next chars must not be alphanumeric
                let before_ok = abs == 0
                    || !text
                        .as_bytes()
                        .get(abs - 1)
                        .is_some_and(|b| (*b as char).is_alphanumeric());
                let after_pos = abs + method.len();
                let after_ok = after_pos >= text.len()
                    || !text
                        .as_bytes()
                        .get(after_pos)
                        .is_some_and(|b| (*b as char).is_alphanumeric());
                if before_ok && after_ok {
                    results.push((abs, abs + method.len(), method.to_string()));
                }
                start = abs + method.len();
            }
        }
        results
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
                // Pattern: well-known dataset names (direct match)
                let known_datasets = [
                    "HotpotQA",
                    "LiveBench",
                    "MATH",
                    "GSM8K",
                    "MBPP",
                    "HumanEval",
                    "SQuAD",
                    "WMT",
                    "ImageNet",
                    "CIFAR",
                    "MNIST",
                    "AGNews",
                    "TriviaQA",
                    "NaturalQuestions",
                    "DROP",
                    "BoolQ",
                    // LLM evaluation benchmarks
                    "MMLU",
                    "MMLU-Pro",
                    "BBH",
                    "ARC",
                    "HellaSwag",
                    "TruthfulQA",
                    "AGIEval",
                    "WinoGrande",
                    "PIQA",
                    "OpenBookQA",
                ];
                for ds in &known_datasets {
                    let mut start = 0;
                    while let Some(pos) = text[start..].find(ds) {
                        let abs = start + pos;
                        results.push((abs, abs + ds.len(), ds.to_string()));
                        start = abs + ds.len();
                    }
                }
                // Pattern: capitalized word before "dataset/benchmark/evaluated on"
                for pattern in &["dataset", "corpus", "benchmark", "evaluated on"] {
                    let lower = text.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(pattern) {
                        let abs = start + pos;
                        let before_start = abs.saturating_sub(30);
                        let before = text.get(before_start..abs).unwrap_or("");
                        let caps: Vec<&str> = before
                            .split_whitespace()
                            .filter(|w| {
                                w.chars()
                                    .next()
                                    .map(|c| c.is_uppercase() && w.len() > 2)
                                    .unwrap_or(false)
                            })
                            .collect();
                        if let Some(name) = caps.last() {
                            let name_start =
                                before.rfind(name).map(|p| before_start + p).unwrap_or(abs);
                            let name_end = name_start + name.len();
                            results.push((name_start, name_end, name.to_string()));
                        }
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
            EntityType::Model => {
                // Named models: GPT-4, LLaMA, Claude, Gemini, etc.
                let model_patterns = [
                    "gpt-4", "gpt-3.5", "gpt-4o", "llama", "claude", "gemini", "mistral", "qwen",
                    "deepseek", "glm", "bert", "t5", "bloom",
                ];
                let lower = text.to_lowercase();
                for pattern in &model_patterns {
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(pattern) {
                        let abs = start + pos;
                        // Extract the model name (up to next space or 15 chars)
                        let remainder = &text[abs..];
                        let end = remainder
                            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '.')
                            .unwrap_or(remainder.len())
                            .min(15);
                        let label = &remainder[..end];
                        if !label.is_empty() {
                            results.push((abs, abs + end, label.to_string()));
                        }
                        start = abs + pattern.len();
                    }
                }
            }
            EntityType::Method => {
                // Use the same known-methods whitelist as the global pass.
                // Blind all-caps scan produced too many false positives (87 Methods
                // in one paper). Whitelist keeps precision high.
                let known_methods = [
                    "GEPA", "GRPO", "RLVR", "PPO", "DPO", "KTO", "SILVER", "GSEM", "PEM", "OPRO",
                    "PRO", "EoT", "BERT", "GPT", "T5", "BART", "CoT", "ToT",
                ];
                for method in &known_methods {
                    let mut start = 0;
                    while let Some(pos) = text[start..].find(method) {
                        let abs = start + pos;
                        let before_ok = abs == 0
                            || !text
                                .as_bytes()
                                .get(abs - 1)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        let after_pos = abs + method.len();
                        let after_ok = after_pos >= text.len()
                            || !text
                                .as_bytes()
                                .get(after_pos)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        if before_ok && after_ok {
                            results.push((abs, abs + method.len(), method.to_string()));
                        }
                        start = abs + method.len();
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

        // Global Method acronym pass: scan ALL sections for all-caps method acronyms.
        // Methods are often introduced in Abstract/Introduction (unclassified sections)
        // but not repeated in Method sections. Without this pass, GSEM/GEPA/GRPO-like
        // names in Abstract would be missed.
        let mut seen_lower: std::collections::HashSet<String> =
            entities.iter().map(|e| e.label.to_lowercase()).collect();
        for (title, text) in sections {
            let acronyms = Self::extract_method_acronyms_global(text);
            for (char_start, char_end, label) in acronyms {
                let key = label.to_lowercase();
                if !seen_lower.contains(&key) {
                    seen_lower.insert(key);
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: EntityType::Method,
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
    fn test_classify_section_extended() {
        // Real GROBID section titles from paper 2507.19457
        assert_eq!(
            RuleBasedExtractor::classify_section("EVALUATION SETUP"),
            Some(EntityType::Metric)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("RESULTS AND ANALYSIS"),
            Some(EntityType::Metric)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("MODELS AND INFERENCE PARAMETERS"),
            Some(EntityType::Model)
        );
        assert_eq!(
            RuleBasedExtractor::classify_section("ALGORITHM AND METHODOLOGY DETAILS"),
            Some(EntityType::Method)
        );
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
        // Should find "PubMed" from capitalized word before "dataset"
        assert!(candidates.iter().any(|(_, _, l)| l.contains("PubMed")));
    }

    #[test]
    fn test_extract_candidates_known_datasets() {
        // Known dataset names should be extracted directly
        let text = "We evaluate on HotpotQA, LiveBench, and MATH benchmarks.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Dataset);
        assert!(!candidates.is_empty());
        let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
        assert!(labels.contains(&"HotpotQA"), "got: {labels:?}");
        assert!(labels.contains(&"LiveBench"), "got: {labels:?}");
        assert!(labels.contains(&"MATH"), "got: {labels:?}");
    }

    #[test]
    fn test_extract_candidates_models() {
        let text = "We compare GPT-4, Llama-3, Claude-3, and Gemini across tasks.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Model);
        assert!(!candidates.is_empty());
        let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
        assert!(
            labels.iter().any(|l| l.to_lowercase().contains("gpt-4")),
            "got: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.to_lowercase().contains("llama")),
            "got: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.to_lowercase().contains("claude")),
            "got: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.to_lowercase().contains("gemini")),
            "got: {labels:?}"
        );
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

#[test]
fn test_extract_method_acronyms() {
    let text = "We propose GEPA and compare with GRPO and RLVR baselines.";
    let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Method);
    assert!(!candidates.is_empty());
    let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
    assert!(labels.contains(&"GEPA"), "got: {labels:?}");
    assert!(labels.contains(&"GRPO"), "got: {labels:?}");
    assert!(labels.contains(&"RLVR"), "got: {labels:?}");
}

#[test]
fn test_extract_known_llm_benchmarks() {
    // LLM evaluation benchmarks should be extracted directly
    let text = "We evaluate on MMLU, MMLU-Pro, BBH, ARC, HellaSwag, and TruthfulQA.";
    let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Dataset);
    assert!(!candidates.is_empty());
    let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
    assert!(labels.contains(&"MMLU"), "got: {labels:?}");
    assert!(labels.contains(&"BBH"), "got: {labels:?}");
    assert!(labels.contains(&"ARC"), "got: {labels:?}");
    assert!(labels.contains(&"HellaSwag"), "got: {labels:?}");
    assert!(labels.contains(&"TruthfulQA"), "got: {labels:?}");
}

#[tokio::test]
async fn test_extract_global_acronym_from_abstract() {
    // GSEM is a method acronym that appears in Abstract (unclassified section).
    // Without global acronym pass, it would be missed.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![
        (
            "Abstract".to_string(),
            "We propose GSEM, a generalizable self-evolving memory system.".to_string(),
        ),
        (
            "Introduction".to_string(),
            "Prompt optimization is a challenging task. LLM and NLP are hot topics.".to_string(),
        ),
    ];
    let entities = extractor.extract(&sections).await.unwrap();
    let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
    assert!(
        labels.contains(&"GSEM"),
        "GSEM should be found via global acronym pass, got: {labels:?}"
    );
    // LLM and NLP should be filtered as tech stopwords
    assert!(
        !labels.contains(&"LLM"),
        "LLM should be filtered, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"NLP"),
        "NLP should be filtered, got: {labels:?}"
    );
}
