//! Rule-based entity extractor — implements Extractor port.
//!
//! ADR-038 Module B Phase 3 Slice 1: deterministic rules, no ML.
//! Detects entity mentions via section heuristics + keyword patterns.
//! GLiNER 2 replaces this later (Slice 3) for higher recall.

use async_trait::async_trait;
use da_domain::entity::EntityType;
use da_ports::extractor::{ExtractResult, ExtractedEntity, Extractor};

// ============================================================================
// Canonical whitelists — single source of truth for entity-type patterns.
// Used by both extract_candidates (section-classified) and the global passes
// in extract(). Adding a new entity = add it here once.
// ============================================================================
// Entity whitelists moved to data/extraction_patterns.yaml.
// RuleBasedExtractor loads config from YAML at construction time.
// No hardcoded KNOWN_* arrays — single source of truth is YAML.
// ============================================================================

// ============================================================================
// Declarative extraction patterns (Wave 3).
// Patterns loaded from YAML: data/extraction_patterns.yaml (bundled fallback)
// embedded defaults. Governor CLI can update patterns without recompiling.
// ============================================================================

/// Declarative extraction config (Wave 3: JSON-driven whitelists).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExtractionConfig {
    pub methods: MethodConfig,
    pub models: Vec<String>,
    pub datasets: Vec<String>,
    pub metrics: Vec<String>,
    pub tasks: TaskConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MethodConfig {
    pub acronyms: Vec<String>,
    pub phrases: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TaskConfig {
    pub phrases: Vec<String>,
    pub acronyms: Vec<String>,
}

impl ExtractionConfig {
    /// Load from YAML file.
    pub fn from_yaml_file(path: &str) -> Result<Self, std::io::Error> {
        let yaml = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&yaml)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Bundled defaults from YAML (single source of truth).
    /// data/extraction_patterns.yaml is the canonical reference.
    pub fn bundled() -> Self {
        serde_yaml::from_str(BUNDLED_EXTRACTION_YAML)
            .expect("bundled extraction_patterns.yaml must be valid")
    }

    /// Load from YAML file, falling back to bundled defaults.
    pub fn load() -> Self {
        for path in [
            "data/extraction_patterns.yaml",
            "../../data/extraction_patterns.yaml",
            "../../../data/extraction_patterns.yaml",
        ] {
            if std::path::Path::new(path).exists() {
                if let Ok(config) = Self::from_yaml_file(path) {
                    return config;
                }
            }
        }
        Self::bundled()
    }
}

/// Bundled extraction patterns YAML (canonical source of truth).
/// data/extraction_patterns.yaml can override at runtime.
const BUNDLED_EXTRACTION_YAML: &str = include_str!("../../../data/extraction_patterns.yaml");

/// Rule-based extractor using section titles + keyword heuristics.
/// Holds an ExtractionConfig loaded from YAML (no hardcoded whitelists).
///
/// Performance: pre-computes lowercase versions of config lists once
/// at construction time to avoid repeated `.to_lowercase()` allocations
/// in the extraction hot path (Rust 2026 best practice: reuse allocations).
pub struct RuleBasedExtractor {
    config: ExtractionConfig,
    /// Pre-lowercased config data for O(1) hot-path lookups.
    lowered: LoweredConfig,
}

/// Pre-lowercased extraction config data.
/// Built once at construction; avoids `.to_lowercase()` in inner loops.
struct LoweredConfig {
    method_acronyms_lower: Vec<String>,
    method_acronyms_canonical: Vec<String>,
    models_lower: Vec<String>,
    models_prefix_lower: Vec<String>, // first segment before '-', lowercased
    datasets_lower: Vec<String>,
    metrics_lower: Vec<String>,
    task_phrases_lower: Vec<String>,
    task_acronyms_lower: Vec<String>,
    method_phrases_lower: Vec<String>,
}

impl LoweredConfig {
    fn from_config(config: &ExtractionConfig) -> Self {
        Self {
            method_acronyms_lower: config
                .methods
                .acronyms
                .iter()
                .map(|s| s.to_lowercase())
                .collect(),
            method_acronyms_canonical: config.methods.acronyms.clone(),
            models_lower: config.models.iter().map(|s| s.to_lowercase()).collect(),
            models_prefix_lower: config
                .models
                .iter()
                .map(|s| s.split('-').next().unwrap_or(s).to_lowercase())
                .collect(),
            datasets_lower: config.datasets.iter().map(|s| s.to_lowercase()).collect(),
            metrics_lower: config.metrics.iter().map(|s| s.to_lowercase()).collect(),
            task_phrases_lower: config
                .tasks
                .phrases
                .iter()
                .map(|s| s.to_lowercase())
                .collect(),
            task_acronyms_lower: config
                .tasks
                .acronyms
                .iter()
                .map(|s| s.to_lowercase())
                .collect(),
            method_phrases_lower: config
                .methods
                .phrases
                .iter()
                .map(|s| s.to_lowercase())
                .collect(),
        }
    }
}

impl RuleBasedExtractor {
    pub fn new() -> Self {
        let config = ExtractionConfig::load();
        let lowered = LoweredConfig::from_config(&config);
        Self { config, lowered }
    }

    /// Create with explicit config (for testing or custom whitelists).
    pub fn with_config(config: ExtractionConfig) -> Self {
        let lowered = LoweredConfig::from_config(&config);
        Self { config, lowered }
    }

    /// Access the loaded config.
    pub fn config(&self) -> &ExtractionConfig {
        &self.config
    }

    /// Classify a section title to an entity type.
    /// e.g. "Datasets" → Dataset, "Methods" → Method, "Evaluation Setup" → Metric.
    fn classify_section(title: &str) -> Option<EntityType> {
        let lower = title.to_lowercase();
        if lower.contains("dataset") || lower.contains("corpus") || lower.contains("benchmark") {
            Some(EntityType::Dataset)
        } else if lower.contains("method")
            || lower.contains("approach")
            || lower.contains("algorithm")
            || lower.contains("methodology")
        {
            Some(EntityType::Method)
        } else if (lower.contains("model") && !lower.contains("world model"))
            || lower.contains("inference")
        {
            Some(EntityType::Model)
        } else if lower.contains("metric")
            || lower.contains("evaluation")
            || lower.contains("result")
            || lower.contains("experiment")
        {
            Some(EntityType::Metric)
        } else if lower.contains("task") || lower.contains("problem") {
            Some(EntityType::Task)
        } else {
            None
        }
    }

    /// Check that [start, end) in text is bounded by non-alphanumeric chars
    /// (or string edges). Prevents substring false positives like "arc" in
    /// "architecture", "ppo" in "support", "drop" in "dropout".
    fn word_boundary(text: &str, start: usize, end: usize) -> bool {
        let before_ok = start == 0
            || !text
                .as_bytes()
                .get(start - 1)
                .is_some_and(|b| (*b as char).is_alphanumeric());
        let after_ok = end >= text.len()
            || !text
                .as_bytes()
                .get(end)
                .is_some_and(|b| (*b as char).is_alphanumeric());
        before_ok && after_ok
    }

    /// Global Method acronym extractor — scans text for KNOWN method acronyms.
    /// Used to find method acronyms (GSEM, GEPA, GRPO, RLVR) that appear in
    /// Abstract/Introduction (unclassified sections) rather than Method sections.
    /// Restricted to a whitelist to avoid false positives from all-caps scan.
    fn extract_method_acronyms_global(&self, text: &str) -> Vec<(usize, usize, String)> {
        let mut results = Vec::with_capacity(self.lowered.method_acronyms_lower.len());
        let lower_text = text.to_lowercase();
        for (method_lower, method_canonical) in self
            .lowered
            .method_acronyms_lower
            .iter()
            .zip(self.lowered.method_acronyms_canonical.iter())
        {
            let mut start = 0;
            while let Some(pos) = lower_text[start..].find(method_lower.as_str()) {
                let abs = start + pos;
                let end = abs + method_lower.len();
                if Self::word_boundary(text, abs, end) {
                    results.push((abs, end, method_canonical.clone()));
                }
                start = end;
            }
        }
        results
    }

    /// Extract candidate entity labels from section text using heuristics.
    /// Looks for capitalized phrases, quoted terms, and "we propose X" patterns.
    fn extract_candidates(
        &self,
        text: &str,
        entity_type: &EntityType,
    ) -> Vec<(usize, usize, String)> {
        let mut results = Vec::new();

        // Pattern 1: "we propose X" / "we use X" / "we present X" (case-insensitive)
        // Extract only the first capitalized word after the pattern — not the
        // full sentence (which creates noisy long phrase labels).
        //
        // TYPE RESTRICTION: only fire for Method and Task entity types. For
        // Dataset/Metric/Model sections, "we use X" does not introduce a new
        // entity of that type (e.g., "we use GRPO" in a Benchmarks section
        // must NOT add GRPO as a Dataset — the global Method pass will add it
        // with its canonical Method type).
        if matches!(entity_type, EntityType::Method | EntityType::Task) {
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
                        // CROSS-WHITELIST SUPPRESSION: if the candidate is a known
                        // model name (GPT-4, Claude, etc.) or known dataset/metric,
                        // do NOT claim it as a Method/Task — the global
                        // Model/Dataset/Metric pass will type it correctly.
                        let cand_lower = candidate.to_lowercase();
                        let is_known_non_method = self
                            .lowered
                            .models_prefix_lower
                            .iter()
                            .any(|mp| cand_lower.starts_with(mp.as_str()))
                            || self
                                .lowered
                                .datasets_lower
                                .iter()
                                .any(|d| d.eq_ignore_ascii_case(&cand_lower))
                            || self
                                .lowered
                                .metrics_lower
                                .iter()
                                .any(|m| m.eq_ignore_ascii_case(&cand_lower));
                        if !is_known_non_method {
                            let label_end = abs + end;
                            results.push((abs, label_end, candidate.to_string()));
                        }
                    }
                    start = abs + 1;
                }
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
                let lower_text_ds = text.to_lowercase();
                for (ds_lower, ds_canonical) in self
                    .lowered
                    .datasets_lower
                    .iter()
                    .zip(self.config.datasets.iter())
                {
                    let mut start = 0;
                    while let Some(pos) = lower_text_ds[start..].find(ds_lower.as_str()) {
                        let abs = start + pos;
                        results.push((abs, abs + ds_lower.len(), ds_canonical.clone()));
                        start = abs + ds_lower.len();
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
                let lower_text = text.to_lowercase();
                for (method_lower, method_canonical) in self
                    .lowered
                    .method_acronyms_lower
                    .iter()
                    .zip(self.lowered.method_acronyms_canonical.iter())
                {
                    let mut start = 0;
                    while let Some(pos) = lower_text[start..].find(method_lower.as_str()) {
                        let abs = start + pos;
                        let end = abs + method_lower.len();
                        if Self::word_boundary(text, abs, end) {
                            results.push((abs, end, method_canonical.clone()));
                        }
                        start = end;
                    }
                }
            }
            EntityType::Task => {
                // Known task phrases + acronyms. Case-insensitive with word
                // boundary to avoid substring false positives.
                let task_phrases = &self.config.tasks.phrases;
                let task_acronyms = &self.config.tasks.acronyms;
                let lower = text.to_lowercase();
                for phrase in task_phrases {
                    let p_lower = phrase.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(&p_lower) {
                        let abs = start + pos;
                        let end = abs + phrase.len();
                        if Self::word_boundary(text, abs, end) {
                            results.push((abs, end, phrase.to_string()));
                        }
                        start = end;
                    }
                }
                for acr in task_acronyms {
                    let a_lower = acr.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(&a_lower) {
                        let abs = start + pos;
                        let end = abs + acr.len();
                        if Self::word_boundary(text, abs, end) {
                            results.push((abs, end, acr.to_string()));
                        }
                        start = end;
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
                let candidates = self.extract_candidates(text, &entity_type);
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
            let scan_text = format!("{} {}", title, text);
            let acronyms = self.extract_method_acronyms_global(&scan_text);
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
                        surface: scan_text
                            [char_start.min(scan_text.len())..char_end.min(scan_text.len())]
                            .to_string(),
                    });
                }
            }
        }

        // Global Method phrase pass: scan ALL sections for known multi-word
        // method phrases (self-evolving memory, chain-of-thought reasoning).
        // These are compound method names that cannot be captured by the
        // all-caps acronym pass — they appear as lowercase/titlecase phrases.
        for (title, text) in sections {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for (p_lower, phrase_canonical) in self
                .lowered
                .method_phrases_lower
                .iter()
                .zip(self.config.methods.phrases.iter())
            {
                let mut start = 0;
                while let Some(pos) = lower[start..].find(p_lower.as_str()) {
                    let abs = start + pos;
                    let end = abs + phrase_canonical.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (p_lower.clone(), "method".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: phrase_canonical.to_string(),
                                entity_type: EntityType::Method,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: end,
                                surface: phrase_canonical.to_string(),
                            });
                        }
                    }
                    start = end;
                }
            }
        }

        // Global Dataset pass: scan ALL sections for known dataset names.
        // Datasets are mentioned throughout the paper, not just in Dataset sections.
        for (title, text) in sections {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for ds in &self.config.datasets {
                let ds_lower = ds.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&ds_lower) {
                    let abs = start + pos;
                    let end = abs + ds.len();
                    if Self::word_boundary(&scan_text, abs, end) {
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
        for (title, text) in sections {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for canonical in &self.config.models {
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
        for (title, text) in sections {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for metric in &self.config.metrics {
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
        let task_phrases = &self.config.tasks.phrases;
        let task_acronyms = &self.config.tasks.acronyms;
        for (title, text) in sections {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for phrase in task_phrases {
                let p_lower = phrase.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&p_lower) {
                    let abs = start + pos;
                    let end = abs + phrase.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (p_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: phrase.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: end,
                                surface: phrase.to_string(),
                            });
                        }
                    }
                    start = end;
                }
            }
            for acr in task_acronyms {
                let a_lower = acr.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&a_lower) {
                    let abs = start + pos;
                    let end = abs + acr.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (a_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: acr.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                char_start: abs,
                                char_end: end,
                                surface: acr.to_string(),
                            });
                        }
                    }
                    start = end;
                }
            }
        }

        // Final entity-level dedup: collapse any same (label, type) pairs
        // that may have been added by multiple section-classified passes
        // before `seen` was initialized. Without this pass, HotpotQA appearing
        // in two Dataset-classified sections would yield two [Dataset] HotpotQA
        // entities — inflating predicted count and FP without changing TP.
        let mut seen_final: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        entities.retain(|e| {
            let key = (
                e.label.to_lowercase(),
                format!("{:?}", e.entity_type).to_lowercase(),
            );
            seen_final.insert(key)
        });

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
    fn test_word_boundary_standalone() {
        // Standalone words: word_boundary returns true.
        assert!(RuleBasedExtractor::word_boundary("hello ARC world", 6, 9));
        assert!(RuleBasedExtractor::word_boundary("ARC world", 0, 3));
        assert!(RuleBasedExtractor::word_boundary("hello ARC", 6, 9));
    }

    #[test]
    fn test_word_boundary_substring_rejected() {
        // Substring matches (arc in architecture, drop in dropout) → false.
        assert!(!RuleBasedExtractor::word_boundary("architecture", 0, 3));
        assert!(!RuleBasedExtractor::word_boundary("dropout", 0, 4));
        assert!(!RuleBasedExtractor::word_boundary("support", 3, 6));
    }

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
            None,
            "'baseline' has no extraction branch — must return None"
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
    fn test_classify_section_noise_reduction() {
        // Regression: "parameter" used to classify as Model, but in LLM papers
        // "parameter" usually means training hyperparameters, not an LLM model.
        assert_eq!(
            RuleBasedExtractor::classify_section("Parameter Change Analysis"),
            None,
            "'parameter' must not classify as Model (hyperparameters, not LLM)"
        );
        // "World Model Learning" is an architecture concept, not an LLM model
        // entity section — must not classify as Model.
        assert_eq!(
            RuleBasedExtractor::classify_section("Reinforcement World Model Learning"),
            None,
            "'world model' section must not classify as Model"
        );
        // "analysis" alone is too noisy to imply a Metrics section.
        assert_eq!(
            RuleBasedExtractor::classify_section("Weight Change Analysis"),
            None,
            "'analysis' alone must not classify as Metric"
        );
    }

    #[test]
    fn test_extract_candidates_propose() {
        let extractor = RuleBasedExtractor::new();
        let text = "In this paper we propose GeoRLE, a novel approach for layout analysis.";
        let candidates = extractor.extract_candidates(text, &EntityType::Method);
        assert!(!candidates.is_empty());
        assert!(candidates[0].2.contains("GeoRLE"));
    }

    #[test]
    fn test_propose_pattern_type_restricted() {
        let extractor = RuleBasedExtractor::new();
        // Pattern 1 ("we use X") should NOT fire for Dataset/Metric entity types.
        // "we use GRPO" in a Benchmarks section must not add GRPO as a Dataset.
        // Previously, Pattern 1 was type-agnostic, causing GRPO/GPT-4/etc to be
        // mistyped when they appeared in non-Method sections with "we use".
        let text = "Policy RL training, we use GRPO to let the model learn.";
        let ds_candidates = extractor.extract_candidates(text, &EntityType::Dataset);
        let metric_candidates = extractor.extract_candidates(text, &EntityType::Metric);
        assert!(
            !ds_candidates.iter().any(|(_, _, l)| l.contains("GRPO")),
            "GRPO should NOT be a Dataset candidate, got: {ds_candidates:?}"
        );
        assert!(
            !metric_candidates.iter().any(|(_, _, l)| l.contains("GRPO")),
            "GRPO should NOT be a Metric candidate, got: {metric_candidates:?}"
        );
        // But Method type should still extract it
        let method_candidates = extractor.extract_candidates(text, &EntityType::Method);
        assert!(
            method_candidates.iter().any(|(_, _, l)| l.contains("GRPO")),
            "GRPO should be a Method candidate, got: {method_candidates:?}"
        );
    }

    #[test]
    fn test_pattern1_suppresses_known_model_names() {
        let extractor = RuleBasedExtractor::new();
        // Regression: Pattern 1 ("we use X") in a Method-classified section
        // extracts the first capitalized word after "we use". For model names
        // like GPT-4, this would incorrectly classify them as Method entities.
        // Known models (GPT-4, Claude, Gemini, etc.) must NOT be extracted as
        // Method/Task candidates — they belong to the global Model pass.
        let text = "In our method, we use GPT-4 to evaluate the outputs.";
        let method_candidates = extractor.extract_candidates(text, &EntityType::Method);
        assert!(
            !method_candidates
                .iter()
                .any(|(_, _, l)| l.to_lowercase().contains("gpt-4")),
            "GPT-4 should NOT be a Method candidate (it's a known model), got: {method_candidates:?}"
        );
        // But real method names should still be extracted
        let text2 = "In our method, we use GEPA to optimize prompts.";
        let method_candidates2 = extractor.extract_candidates(text2, &EntityType::Method);
        assert!(
            method_candidates2
                .iter()
                .any(|(_, _, l)| l.contains("GEPA")),
            "GEPA should still be a Method candidate, got: {method_candidates2:?}"
        );
    }

    #[test]
    fn test_extract_candidates_dataset() {
        let extractor = RuleBasedExtractor::new();
        // Direct match of known datasets is the primary pattern.
        // The heuristic "capitalized word before dataset keyword" was removed
        // (too many false positives — long phrase labels).
        let text = "We evaluate on the PubMed dataset and the HotpotQA benchmark.";
        let candidates = extractor.extract_candidates(text, &EntityType::Dataset);
        assert!(!candidates.is_empty());
        // HotpotQA is in known_datasets
        assert!(candidates.iter().any(|(_, _, l)| l == "HotpotQA"));
    }

    #[test]
    fn test_extract_candidates_known_datasets() {
        let extractor = RuleBasedExtractor::new();
        // Known dataset names should be extracted directly
        let text = "We evaluate on HotpotQA, LiveBench, and MATH benchmarks.";
        let candidates = extractor.extract_candidates(text, &EntityType::Dataset);
        assert!(!candidates.is_empty());
        let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
        assert!(labels.contains(&"HotpotQA"), "got: {labels:?}");
        assert!(labels.contains(&"LiveBench"), "got: {labels:?}");
        assert!(labels.contains(&"MATH"), "got: {labels:?}");
    }

    #[test]
    fn test_extract_candidates_models() {
        let extractor = RuleBasedExtractor::new();
        // Model extraction is now handled by the global pass in extract(),
        // not by section-classified extract_candidates. This test verifies
        // that extract_candidates for Model returns empty (no false positives).
        let text = "We compare GPT-4, Llama-3, Claude-3, and Gemini across tasks.";
        let candidates = extractor.extract_candidates(text, &EntityType::Model);
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
        assert!(
            entities
                .iter()
                .any(|e| e.entity_type == EntityType::Dataset)
        );
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

    #[tokio::test]
    async fn test_global_entity_dedup_cross_type_paths() {
        // Regression: global Metric pass must deduplicate across sections via
        // the shared `seen` HashSet. F1 appearing in two Metric-titled sections
        // should produce ONE [Metric] F1 entity, not two.
        let extractor = RuleBasedExtractor::new();
        let sections = vec![
            (
                "Results".to_string(),
                "F1 score reached 0.87, accuracy 92%.".to_string(),
            ),
            (
                "Appendix B Evaluation".to_string(),
                "We also report F1 = 0.86 and accuracy = 91%.".to_string(),
            ),
        ];
        let entities = extractor.extract(&sections).await.unwrap();
        let metrics: Vec<&str> = entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Metric)
            .map(|e| e.label.as_str())
            .collect();
        assert_eq!(
            metrics.iter().filter(|&&m| m == "F1").count(),
            1,
            "F1 duplicated, got: {metrics:?}"
        );
        assert_eq!(
            metrics.iter().filter(|&&m| m == "accuracy").count(),
            1,
            "accuracy duplicated, got: {metrics:?}"
        );
    }

    #[test]
    fn test_name() {
        let extractor = RuleBasedExtractor::new();
        assert_eq!(extractor.name(), "rule-based");
    }
}

#[test]
fn test_extract_method_acronyms() {
    let extractor = RuleBasedExtractor::new();
    let text = "We propose GEPA and compare with GRPO and RLVR baselines.";
    let candidates = extractor.extract_candidates(text, &EntityType::Method);
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
async fn test_generic_acronyms_not_extracted_as_methods() {
    // Regression: "PRO methodology" and "silver bullet" were previously
    // extracted as Method entities because PRO and SILVER were on the
    // KNOWN_METHODS whitelist despite being ordinary English words.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "Method".to_string(),
        "We propose a PRO methodology with silver-standard annotations.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let methods: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        !methods.contains(&"PRO"),
        "'PRO' must not be extracted as Method (generic English word), got: {methods:?}"
    );
    assert!(
        !methods.contains(&"SILVER"),
        "'SILVER' must not be extracted as Method (common English adjective), got: {methods:?}"
    );
}

#[tokio::test]
async fn test_multi_word_method_phrase_extraction() {
    // Wave 1: multi-word method phrases like "self-evolving memory" are
    // compound names that cannot be captured by the all-caps acronym
    // whitelist. They appear as lowercase/titlecase phrases in body text.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "Method".to_string(),
        "We propose a self-evolving memory system for agents.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let methods: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        methods.contains(&"self-evolving memory"),
        "'self-evolving memory' should be extracted as Method, got: {methods:?}"
    );
    // Also test case-insensitive: "Self-Evolving Memory" (title case)
    let sections2 = vec![(
        "Abstract".to_string(),
        "Self-Evolving Memory enables agents to learn from experience.".to_string(),
    )];
    let entities2 = extractor.extract(&sections2).await.unwrap();
    let methods2: Vec<&str> = entities2
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        methods2.contains(&"self-evolving memory"),
        "'Self-Evolving Memory' (title case) should be extracted, got: {methods2:?}"
    );
}

#[tokio::test]
async fn test_entity_in_section_title_is_extracted() {
    // Regression: entity mentions in section TITLES were not scanned by
    // global passes — only body text was. Section "In-Context Learning and
    // Case-Based Reasoning" has "in-context learning" only in the title,
    // not in body text, causing FN.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "In-Context Learning and Case-Based Reasoning".to_string(),
        "We compare different approaches.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let methods: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        methods.contains(&"in-context learning"),
        "'in-context learning' should be found in section title, got: {methods:?}"
    );
}

#[test]
fn test_no_cross_type_conflict_between_acronyms_and_phrases() {
    // Regression: "retrieval-augmented generation" was in method phrases
    // while "RAG" is in task acronyms. Same concept, different entity types →
    // graph confusion. Verify no acronym in task acronyms has a corresponding
    // phrase in method phrases (and vice versa for method acronyms).
    let config = ExtractionConfig::bundled();
    let method_acronyms: std::collections::HashSet<String> =
        config.methods.acronyms.iter().cloned().collect();
    let task_acronyms: std::collections::HashSet<String> =
        config.tasks.acronyms.iter().cloned().collect();

    // No method phrase should be a task acronym expanded (or vice versa).
    // Check: task acronyms should not appear as substrings of method phrases.
    for phrase in &config.methods.phrases {
        let p_lower = phrase.to_lowercase();
        for acr in &task_acronyms {
            let a_lower = acr.to_lowercase();
            // RAG = Retrieval-Augmented Generation — check if phrase starts
            // with the expansion. This is a heuristic guard, not exhaustive.
            if p_lower.starts_with(&a_lower) {
                panic!("Cross-type conflict: phrase '{phrase}' starts with task acronym '{acr}'");
            }
        }
    }

    // Also verify method acronyms and task acronyms don't overlap.
    let overlap: Vec<&String> = method_acronyms.intersection(&task_acronyms).collect();
    assert!(
        overlap.is_empty(),
        "Method and Task acronyms overlap: {overlap:?}"
    );
}

#[test]
fn test_extraction_config_bundled() {
    // ExtractionConfig::bundled() loads from bundled YAML.
    let config = ExtractionConfig::bundled();
    assert!(!config.methods.acronyms.is_empty());
    assert!(config.methods.acronyms.iter().any(|m| m == "GEPA"));
    assert!(config.methods.acronyms.iter().any(|m| m == "GCN")); // cross-domain
    assert!(config.models.iter().any(|m| m == "GPT-4"));
    assert!(config.datasets.iter().any(|d| d == "MMLU"));
    assert!(config.metrics.iter().any(|m| m == "accuracy"));
    assert!(
        config
            .tasks
            .phrases
            .iter()
            .any(|t| t == "prompt optimization")
    );
    assert!(config.tasks.acronyms.iter().any(|t| t == "RLHF"));
}

#[test]
fn test_extraction_config_from_yaml_file() {
    // Load patterns from YAML file (canonical format).
    let candidates = [
        "../../../data/extraction_patterns.yaml",
        "../../data/extraction_patterns.yaml",
        "data/extraction_patterns.yaml",
    ];
    let path = candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .copied()
        .expect("extraction_patterns.yaml should exist");
    let config = ExtractionConfig::from_yaml_file(path).unwrap_or_else(|e| {
        panic!("Failed to load config: {e}");
    });
    assert!(!config.methods.acronyms.is_empty());
    assert!(config.methods.acronyms.iter().any(|m| m == "GEPA"));
    assert!(config.models.iter().any(|m| m == "GPT-4"));
}

#[test]
fn test_rule_based_extractor_loads_config() {
    // RuleBasedExtractor::new() should load config from YAML automatically.
    let extractor = RuleBasedExtractor::new();
    let config = extractor.config();
    assert!(!config.methods.acronyms.is_empty());
    assert!(config.methods.acronyms.iter().any(|m| m == "GRPO"));
}

#[tokio::test]
async fn test_gnn_entities_extracted_from_textbook_content() {
    // Cross-domain validation: GNN-specific methods (GCN, GAT, GraphSAGE)
    // should be extractable from textbook HTML content.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![(
        "Graph Neural Networks".to_string(),
        "We compare GCN, GAT, and GraphSAGE architectures. GNN models use message passing."
            .to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let methods: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        methods.contains(&"GCN"),
        "GCN should be Method, got: {methods:?}"
    );
    assert!(
        methods.contains(&"GAT"),
        "GAT should be Method, got: {methods:?}"
    );
    assert!(
        methods.contains(&"GNN"),
        "GNN should be Method, got: {methods:?}"
    );
    assert!(
        methods.contains(&"GraphSAGE"),
        "GraphSAGE should be Method, got: {methods:?}"
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
    let extractor = RuleBasedExtractor::new();
    // LLM evaluation benchmarks should be extracted directly
    let text = "We evaluate on MMLU, MMLU-Pro, BBH, ARC, HellaSwag, and TruthfulQA.";
    let candidates = extractor.extract_candidates(text, &EntityType::Dataset);
    assert!(!candidates.is_empty());
    let labels: Vec<&str> = candidates.iter().map(|(_, _, l)| l.as_str()).collect();
    assert!(labels.contains(&"MMLU"), "got: {labels:?}");
    assert!(labels.contains(&"BBH"), "got: {labels:?}");
    assert!(labels.contains(&"ARC"), "got: {labels:?}");
    assert!(labels.contains(&"HellaSwag"), "got: {labels:?}");
    assert!(labels.contains(&"TruthfulQA"), "got: {labels:?}");
}

#[tokio::test]
async fn test_duplicate_entity_across_sections_deduped() {
    // Regression: section-classified extraction can add the SAME (label, type)
    // pair from MULTIPLE sections BEFORE `seen` is initialized. Example:
    // HotpotQA appears in both "E.1 Benchmarks" AND "G Datasets" → two
    // [Dataset] HotpotQA entities. Final post-pass dedup collapses them.
    let extractor = RuleBasedExtractor::new();
    let sections = vec![
        (
            "E.1 Benchmarks".to_string(),
            "We evaluate on HotpotQA.".to_string(),
        ),
        (
            "G Datasets".to_string(),
            "HotpotQA is a benchmark dataset.".to_string(),
        ),
    ];
    let entities = extractor.extract(&sections).await.unwrap();
    let datasets: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Dataset)
        .map(|e| e.label.as_str())
        .collect();
    assert_eq!(
        datasets.iter().filter(|&&d| d == "HotpotQA").count(),
        1,
        "HotpotQA must not appear twice, got datasets: {datasets:?}"
    );
    // No extra entities — "best" and bare "dataset" prose are not in whitelist.
    assert_eq!(entities.len(), 1);
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
