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
        let lower_text = text.to_lowercase();
        for method in &known_methods {
            let method_lower = method.to_lowercase();
            let mut start = 0;
            while let Some(pos) = lower_text[start..].find(&method_lower) {
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
                    // Use canonical uppercase form as the label
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
        // Extract only the first capitalized word after the pattern — not the
        // full sentence (which creates noisy long phrase labels).
        for pattern in &["we propose ", "we present ", "we introduce ", "we use "] {
            let lower_text = text.to_lowercase();
            let mut start = 0;
            while let Some(pos) = lower_text[start..].find(pattern) {
                let abs = start + pos + pattern.len();
                if abs >= text.len() {
                    break;
                }
                // Extract the first word (up to next space, comma, or 30 chars)
                let remainder = &text[abs..];
                let end = remainder
                    .find(|c: char| c.is_whitespace() || c == ',' || c == '.')
                    .unwrap_or(remainder.len().min(30));
                let candidate = remainder[..end].trim();
                // Only accept if it starts with uppercase (proper noun / method name)
                if !candidate.is_empty()
                    && candidate.len() > 2
                    && candidate
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                {
                    let label_end = abs + end;
                    results.push((abs, label_end, candidate.to_string()));
                }
                start = abs + 1;
            }
        }

        // Pattern 2: Section-type-specific keywords
        match entity_type {
            EntityType::Dataset => {
                // Pattern: well-known dataset names (direct match only).
                // The previous "capitalized word before dataset/benchmark" heuristic
                // produced high false positive rates (long phrase labels like
                // "150 examples for training" classified as Dataset entities).
                // Unknown dataset discovery requires GLiNER or curated lists.
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
            }
            EntityType::Metric => {
                // Metric extraction is handled by the global pass in extract().
                // No section-specific extraction needed here.
            }
            EntityType::Model => {
                // Model extraction is handled by the global model pass in extract().
                // No section-specific extraction needed here.
            }
            EntityType::Method => {
                // Use the same known-methods whitelist as the global pass.
                // Blind all-caps scan produced too many false positives (87 Methods
                // in one paper). Whitelist keeps precision high.
                // Case-insensitive search: GROBID may normalize casing (ppo, Cot).
                let known_methods = [
                    "GEPA", "GRPO", "RLVR", "PPO", "DPO", "KTO", "SILVER", "GSEM", "PEM", "OPRO",
                    "PRO", "EoT", "BERT", "GPT", "T5", "BART", "CoT", "ToT",
                ];
                let lower_text = text.to_lowercase();
                for method in &known_methods {
                    let method_lower = method.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower_text[start..].find(&method_lower) {
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
            EntityType::Task => {
                // Known task phrases + acronyms. Case-insensitive with word
                // boundary to avoid substring false positives.
                let task_phrases = ["prompt optimization", "preference optimization"];
                let task_acronyms = ["RLHF", "RAG"];
                let lower = text.to_lowercase();
                for phrase in &task_phrases {
                    let p_lower = phrase.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(&p_lower) {
                        let abs = start + pos;
                        let before_ok = abs == 0
                            || !text
                                .as_bytes()
                                .get(abs - 1)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        let after_pos = abs + phrase.len();
                        let after_ok = after_pos >= text.len()
                            || !text
                                .as_bytes()
                                .get(after_pos)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        if before_ok && after_ok {
                            results.push((abs, after_pos, phrase.to_string()));
                        }
                        start = after_pos;
                    }
                }
                for acr in &task_acronyms {
                    let a_lower = acr.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(&a_lower) {
                        let abs = start + pos;
                        let before_ok = abs == 0
                            || !text
                                .as_bytes()
                                .get(abs - 1)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        let after_pos = abs + acr.len();
                        let after_ok = after_pos >= text.len()
                            || !text
                                .as_bytes()
                                .get(after_pos)
                                .is_some_and(|b| (*b as char).is_alphanumeric());
                        if before_ok && after_ok {
                            results.push((abs, after_pos, acr.to_string()));
                        }
                        start = after_pos;
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

        // Global Method acronym pass: scan ALL sections for known method acronyms.
        // Methods are often introduced in Abstract/Introduction (unclassified sections)
        // but not repeated in Method sections. Without this pass, GSEM/GEPA/GRPO-like
        // names in Abstract would be missed.
        // Dedup key is (label_lowercase, type_lowercase) — the same surface label
        // can legitimately appear under different types (e.g. "GRPO" as Method
        // via whitelist vs as Dataset via section-title heuristic). Using a
        // composite key lets the global passes add the canonical-type version
        // even when a section pass already added a wrong-type version.
        let mut seen: std::collections::HashSet<(String, String)> = entities
            .iter()
            .map(|e| {
                (
                    e.label.to_lowercase(),
                    format!("{:?}", e.entity_type).to_lowercase(),
                )
            })
            .collect();
        for (title, text) in sections {
            let acronyms = Self::extract_method_acronyms_global(text);
            for (char_start, char_end, label) in acronyms {
                let key = (label.to_lowercase(), "method".to_string());
                if !seen.contains(&key) {
                    seen.insert(key);
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

        // Global Dataset pass: scan ALL sections for known dataset names.
        // Datasets are mentioned throughout the paper, not just in Dataset sections.
        let known_dataset_names = [
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
        for (title, text) in sections {
            let lower = text.to_lowercase();
            for ds in &known_dataset_names {
                let ds_lower = ds.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&ds_lower) {
                    let abs = start + pos;
                    // Word boundary check: prevent substring matches like
                    // "arc" in "architecture", "drop" in "dropout".
                    let before_ok = abs == 0
                        || !text
                            .as_bytes()
                            .get(abs - 1)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    let after_pos = abs + ds.len();
                    let after_ok = after_pos >= text.len()
                        || !text
                            .as_bytes()
                            .get(after_pos)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    if before_ok && after_ok {
                        let key = (ds_lower.clone(), "dataset".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: ds.to_string(),
                                entity_type: EntityType::Dataset,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: abs + ds.len(),
                                surface: ds.to_string(),
                            });
                        }
                    }
                    start = abs + ds.len();
                }
            }
        }

        // Global Model pass: scan ALL sections for known model names.
        // Models are mentioned in Related Work, Experiments, etc.
        // Uses canonical names (not greedy extraction) to avoid duplicate
        // variants like GPT-4.1, GPT-4.1-Mini, GPT-4.1-Mini.
        let known_models = [
            "GPT-4", "GPT-3.5", "GPT-4o", "LLaMA", "Claude", "Gemini", "Mistral", "Qwen",
            "DeepSeek", "GLM", "BERT", "T5", "BLOOM",
        ];
        for (title, text) in sections {
            let lower = text.to_lowercase();
            for canonical in &known_models {
                let pattern = canonical.to_lowercase();
                if lower.contains(&pattern) {
                    let key = (pattern.clone(), "model".to_string());
                    if !seen.contains(&key) {
                        let pos = lower.find(&pattern).unwrap_or(0);
                        seen.insert(key);
                        entities.push(ExtractedEntity {
                            label: canonical.to_string(),
                            entity_type: EntityType::Model,
                            section_title: title.clone(),
                            char_start: pos,
                            char_end: pos + canonical.len(),
                            surface: canonical.to_string(),
                        });
                    }
                }
            }
        }

        // Global Metric pass: scan ALL sections for known metric names.
        // Metrics (accuracy, F1, etc.) appear in various sections, not just
        // Evaluation/Results.
        let known_metrics = [
            "accuracy",
            "precision",
            "recall",
            "F1",
            "BLEU",
            "ROUGE",
            "AUC",
            "MSE",
            "RMSE",
            "MAE",
        ];
        for (title, text) in sections {
            let lower = text.to_lowercase();
            for metric in &known_metrics {
                let metric_lower = metric.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&metric_lower) {
                    let abs = start + pos;
                    let key = (metric_lower.clone(), "metric".to_string());
                    if !seen.contains(&key) {
                        seen.insert(key);
                        entities.push(ExtractedEntity {
                            label: metric.to_string(),
                            entity_type: EntityType::Metric,
                            section_title: title.clone(),
                            char_start: abs,
                            char_end: abs + metric.len(),
                            surface: metric.to_string(),
                        });
                    }
                    start = abs + metric.len();
                }
            }
        }

        // Global Task pass: scan ALL sections for known task phrases/acronyms.
        // Tasks are mentioned throughout the paper, not just in Task sections.
        let task_phrases = ["prompt optimization", "preference optimization"];
        let task_acronyms = ["RLHF", "RAG"];
        for (title, text) in sections {
            let lower = text.to_lowercase();
            for phrase in &task_phrases {
                let p_lower = phrase.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&p_lower) {
                    let abs = start + pos;
                    let before_ok = abs == 0
                        || !text
                            .as_bytes()
                            .get(abs - 1)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    let after_pos = abs + phrase.len();
                    let after_ok = after_pos >= text.len()
                        || !text
                            .as_bytes()
                            .get(after_pos)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    if before_ok && after_ok {
                        let key = (p_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: phrase.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: after_pos,
                                surface: phrase.to_string(),
                            });
                        }
                    }
                    start = after_pos;
                }
            }
            for acr in &task_acronyms {
                let a_lower = acr.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&a_lower) {
                    let abs = start + pos;
                    let before_ok = abs == 0
                        || !text
                            .as_bytes()
                            .get(abs - 1)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    let after_pos = abs + acr.len();
                    let after_ok = after_pos >= text.len()
                        || !text
                            .as_bytes()
                            .get(after_pos)
                            .is_some_and(|b| (*b as char).is_alphanumeric());
                    if before_ok && after_ok {
                        let key = (a_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: acr.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: after_pos,
                                surface: acr.to_string(),
                            });
                        }
                    }
                    start = after_pos;
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
        // Direct match of known datasets is the primary pattern.
        // The heuristic "capitalized word before dataset keyword" was removed
        // (too many false positives — long phrase labels).
        let text = "We evaluate on the PubMed dataset and the HotpotQA benchmark.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Dataset);
        assert!(!candidates.is_empty());
        // HotpotQA is in known_datasets
        assert!(candidates.iter().any(|(_, _, l)| l == "HotpotQA"));
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
        // Model extraction is now handled by the global pass in extract(),
        // not by section-classified extract_candidates. This test verifies
        // that extract_candidates for Model returns empty (no false positives).
        let text = "We compare GPT-4, Llama-3, Claude-3, and Gemini across tasks.";
        let candidates = RuleBasedExtractor::extract_candidates(text, &EntityType::Model);
        assert!(
            candidates.is_empty(),
            "Model extraction should be global, not section-classified"
        );
    }

    #[tokio::test]
    async fn test_extract_models_global() {
        // Models should be extracted via global pass regardless of section type.
        let extractor = RuleBasedExtractor::new();
        let sections = vec![(
            "Related Work".to_string(),
            "We compare GPT-4, LLaMA-3, Claude-3, and Gemini across tasks.".to_string(),
        )];
        let entities = extractor.extract(&sections).await.unwrap();
        let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
        assert!(
            labels.contains(&"GPT-4"),
            "GPT-4 should be found, got: {labels:?}"
        );
        assert!(
            labels.contains(&"LLaMA"),
            "LLaMA should be found, got: {labels:?}"
        );
        assert!(
            labels.contains(&"Claude"),
            "Claude should be found, got: {labels:?}"
        );
        assert!(
            labels.contains(&"Gemini"),
            "Gemini should be found, got: {labels:?}"
        );
    }

    #[tokio::test]
    async fn test_known_method_overrides_section_classification() {
        // Regression test: GRPO is a known Method. Even if it appears inside a
        // section titled "Benchmarks" (classified as Dataset), the extractor
        // must label it as Method — whitelist canonical types override noisy
        // section-title heuristics.
        let extractor = RuleBasedExtractor::new();
        let sections = vec![
            (
                "Experiment Setup Benchmarks".to_string(),
                "We compare GRPO, PPO, and DPO baselines.".to_string(),
            ),
            (
                "Experiments".to_string(),
                "GPT-4 and Qwen are used for evaluation.".to_string(),
            ),
        ];
        let entities = extractor.extract(&sections).await.unwrap();
        let methods: Vec<&str> = entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Method)
            .map(|e| e.label.as_str())
            .collect();
        let datasets: Vec<&str> = entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Dataset)
            .map(|e| e.label.as_str())
            .collect();
        // Known methods should appear as Method, not Dataset
        assert!(
            methods.contains(&"GRPO"),
            "GRPO should be Method, got methods: {methods:?}"
        );
        assert!(
            methods.contains(&"PPO"),
            "PPO should be Method, got methods: {methods:?}"
        );
        assert!(
            methods.contains(&"DPO"),
            "DPO should be Method, got methods: {methods:?}"
        );
        // And NOT duplicated/retyped as Dataset
        assert!(
            !datasets.contains(&"GRPO"),
            "GRPO should not be Dataset, got datasets: {datasets:?}"
        );
        assert!(
            !datasets.contains(&"PPO"),
            "PPO should not be Dataset, got datasets: {datasets:?}"
        );
        // Known models should appear as Model regardless of section title
        let models: Vec<&str> = entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Model)
            .map(|e| e.label.as_str())
            .collect();
        assert!(
            models.contains(&"GPT-4"),
            "GPT-4 should be Model, got models: {models:?}"
        );
        assert!(
            models.contains(&"Qwen"),
            "Qwen should be Model, got models: {models:?}"
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

#[tokio::test]
async fn test_extract_method_case_insensitive() {
    // GROBID may lowercase or mix-case method acronyms (ppo, Cot, Gpt-4).
    // The extractor must find them regardless of casing and canonicalize
    // to uppercase form in the label.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "Abstract".to_string(),
        "We compare ppo and dpo baselines with our cot approach.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
    assert!(
        labels.contains(&"PPO"),
        "PPO should be found case-insensitively, got: {labels:?}"
    );
    assert!(
        labels.contains(&"DPO"),
        "DPO should be found case-insensitively, got: {labels:?}"
    );
    assert!(
        labels.contains(&"CoT"),
        "CoT should be found case-insensitively, got: {labels:?}"
    );
}

#[tokio::test]
async fn test_extract_task_phrases_global() {
    // Task entities (prompt optimization, RLHF, RAG) should be extracted via
    // global pass regardless of section type. "prompt optimization" is a gold
    // entity in 2507.19457 but was never extracted (no Task branch existed).
    let extractor = RuleBasedExtractor::new();
    let sections = vec![
        (
            "Abstract".to_string(),
            "We study prompt optimization and compare with RLHF baselines.".to_string(),
        ),
        (
            "Introduction".to_string(),
            "RAG and question answering are common tasks.".to_string(),
        ),
    ];
    let entities = extractor.extract(&sections).await.unwrap();
    let tasks: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Task)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        tasks.contains(&"prompt optimization"),
        "prompt optimization should be Task, got: {tasks:?}"
    );
    assert!(
        tasks.contains(&"RLHF"),
        "RLHF should be Task, got: {tasks:?}"
    );
    assert!(tasks.contains(&"RAG"), "RAG should be Task, got: {tasks:?}");
    // "question answering" is a generic NLP term, not in the narrow task
    // whitelist (it caused too many false positives). Verify it is NOT
    // extracted as a Task entity.
    assert!(
        !tasks.contains(&"question answering"),
        "question answering should NOT be extracted (too generic), got: {tasks:?}"
    );
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
async fn test_dataset_word_boundary_no_substring_match() {
    // Datasets like ARC, DROP must NOT match as substrings of other words.
    // "arc" in "architecture", "drop" in "dropout".
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "Introduction".to_string(),
        "We use an agentic architecture with dropout layers.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
    assert!(
        !labels.contains(&"ARC"),
        "ARC must not match 'arc' in 'architecture', got: {labels:?}"
    );
    assert!(
        !labels.contains(&"DROP"),
        "DROP must not match 'drop' in 'dropout', got: {labels:?}"
    );
    // But real standalone mentions should still match
    let sections2 = vec![(
        "Experiments".to_string(),
        "We evaluate on ARC and DROP benchmarks.".to_string(),
    )];
    let entities2 = extractor.extract(&sections2).await.unwrap();
    let labels2: Vec<&str> = entities2.iter().map(|e| e.label.as_str()).collect();
    assert!(
        labels2.contains(&"ARC"),
        "standalone ARC should match, got: {labels2:?}"
    );
    assert!(
        labels2.contains(&"DROP"),
        "standalone DROP should match, got: {labels2:?}"
    );
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
