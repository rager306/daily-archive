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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExtractionConfig {
    pub methods: MethodConfig,
    pub models: Vec<String>,
    pub datasets: Vec<String>,
    pub metrics: Vec<String>,
    pub tasks: TaskConfig,
    #[serde(default)]
    pub structural: StructuralConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MethodConfig {
    pub acronyms: Vec<String>,
    pub phrases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskConfig {
    pub phrases: Vec<String>,
    pub acronyms: Vec<String>,
}

/// DATA-driven structural patterns for NEW-term discovery (papers whose
/// methods/datasets are absent from whitelists — 2608.20845 lesson: 4
/// entities from 17 sections because "ingest-time semantic compilation
/// (ISC)" matches no list entry).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct StructuralConfig {
    #[serde(default)]
    pub acronym_definitions: AcronymDefinitionConfig,
    #[serde(default)]
    pub proposal_noun_phrases: ProposalNounPhraseConfig,
    #[serde(default)]
    pub prose_benchmark_numbers: ProseBenchmarkConfig,
    #[serde(default)]
    pub recurring_terms: RecurringTermsConfig,
    #[serde(default)]
    pub label_junk_filters: LabelJunkFilters,
    #[serde(default)]
    pub numbered_environments: NumberedEnvironmentsConfig,
    #[serde(default)]
    pub yake_keyphrases: YakeKeyphrasesConfig,
    #[serde(default)]
    pub section_titles: SectionTitlesConfig,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AcronymDefinitionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default = "default_acronym_min")]
    pub acronym_min_len: usize,
    #[serde(default = "default_acronym_max")]
    pub acronym_max_len: usize,
    #[serde(default = "default_min_words")]
    pub min_words_in_phrase: usize,
    #[serde(default = "default_max_words")]
    pub max_words_in_phrase: usize,
}

impl Default for AcronymDefinitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            acronym_min_len: default_acronym_min(),
            acronym_max_len: default_acronym_max(),
            min_words_in_phrase: default_min_words(),
            max_words_in_phrase: default_max_words(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_acronym_entity() -> String {
    "Method".into()
}
fn default_acronym_min() -> usize {
    2
}
fn default_acronym_max() -> usize {
    6
}
fn default_min_words() -> usize {
    2
}
fn default_max_words() -> usize {
    5
}

/// Prose benchmark numbers (ADR-069 §2.5): vision papers carry results in
/// narrative paragraphs without Results sections. Harvest percentages,
/// p-values, fold changes and sample sizes as Metric observations.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProseBenchmarkConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default)]
    pub patterns: std::collections::BTreeMap<String, ProseNumberPattern>,
    #[serde(default = "default_prose_max_per_section")]
    pub max_per_section: usize,
    #[serde(default = "default_prose_global_cap")]
    pub global_cap_per_paper: usize,
}

impl Default for ProseBenchmarkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            patterns: std::collections::BTreeMap::new(),
            max_per_section: default_prose_max_per_section(),
            global_cap_per_paper: default_prose_global_cap(),
        }
    }
}

fn default_prose_max_per_section() -> usize {
    8
}
fn default_prose_global_cap() -> usize {
    12
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProseNumberPattern {
    pub regex: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_suffix: Option<String>,
    /// drop integer captures divisible by this (sweep-grid noise: 0/10/70/75/80)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_int_multiples_of: Option<f64>,
    /// drop captures below this value (zero/leading-comma noise)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_value: Option<f64>,
    /// "2,048" → "048": strip leading zeros from the captured digits
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strip_leading_zeros: bool,
}

/// Recurring dominant terms (survey pattern): a term repeated across the
/// paper ≥ min_occurrences is a core object of the work (centrality in a
/// centrality survey). Generic vocabulary is stoplisted via DATA.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecurringTermsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default = "default_recurring_min_occurrences")]
    pub min_occurrences: usize,
    #[serde(default = "default_max_words")]
    pub max_words: usize,
    #[serde(default = "default_recurring_min_word_len")]
    pub min_word_len: usize,
    #[serde(default)]
    pub generic_stoplist: Vec<String>,
    #[serde(default)]
    pub english_stopwords: Vec<String>,
    #[serde(default = "default_true")]
    pub require_ascii_letters: bool,
    #[serde(default = "default_true")]
    pub drop_plural_when_singular_qualifies: bool,
    /// genre gate: pass fires only when section titles match survey markers
    #[serde(default)]
    pub survey_heading_markers: Vec<String>,
}

impl Default for RecurringTermsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            min_occurrences: default_recurring_min_occurrences(),
            max_words: default_max_words(),
            min_word_len: default_recurring_min_word_len(),
            generic_stoplist: Vec::new(),
            english_stopwords: Vec::new(),
            require_ascii_letters: true,
            drop_plural_when_singular_qualifies: true,
            survey_heading_markers: Vec::new(),
        }
    }
}

fn default_recurring_min_occurrences() -> usize {
    15
}
fn default_recurring_min_word_len() -> usize {
    4
}

/// Numbered environments (survey spine, ADR-071): "M3. Isolating
/// centrality." — a closed index of the survey's objects. Fires on section
/// titles and on the intro slice of section text ("M21." + first sentence).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NumberedEnvironmentsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default = "default_env_prefixes")]
    pub environment_prefixes: Vec<String>,
    #[serde(default = "default_env_max_name_len")]
    pub max_name_len: usize,
    #[serde(default = "default_true")]
    pub scan_title: bool,
    #[serde(default = "default_env_intro_chars")]
    pub scan_text_intro_chars: usize,
}

impl Default for NumberedEnvironmentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            environment_prefixes: default_env_prefixes(),
            max_name_len: default_env_max_name_len(),
            scan_title: true,
            scan_text_intro_chars: default_env_intro_chars(),
        }
    }
}

fn default_env_prefixes() -> Vec<String> {
    vec!["M".into()]
}
fn default_env_max_name_len() -> usize {
    60
}
fn default_env_intro_chars() -> usize {
    500
}

/// YAKE keyphrases (recall layer, ADR-071 companion): statistical
/// single-document extractor — five features (casing, position, normalized
/// TF, co-occurrence diversity TRel, dif-sentence). Untyped candidates
/// under the evidence gate; typed passes keep precision.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct YakeKeyphrasesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_yake_top_n")]
    pub top_n: usize,
    #[serde(default = "default_yake_ngrams")]
    pub ngrams: usize,
    /// статистическим признакам нужен объём: на коротких текстах
    /// (unit-тесты, аннотации) TRel/TFNorm нестабильны
    #[serde(default = "default_yake_min_doc_chars")]
    pub min_doc_chars: usize,
    /// фразы — ценность YAKE; одиночные слова покрыты recurring/whitelist
    #[serde(default = "default_true")]
    pub require_multiword: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default)]
    pub extra_stopwords: Vec<String>,
    /// масштабировать top_n длиной документа (монографии 1M+ символов:
    /// статичный top 15 терял bellman/frontier-search на specimen)
    #[serde(default)]
    pub top_n_per_25k_chars: bool,
    #[serde(default = "default_yake_top_n_cap")]
    pub top_n_cap: usize,
}

impl Default for YakeKeyphrasesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_n: default_yake_top_n(),
            ngrams: default_yake_ngrams(),
            min_doc_chars: default_yake_min_doc_chars(),
            require_multiword: true,
            entity_type: default_acronym_entity(),
            extra_stopwords: Vec::new(),
            top_n_per_25k_chars: false,
            top_n_cap: default_yake_top_n_cap(),
        }
    }
}

fn default_yake_top_n() -> usize {
    15
}
fn default_yake_ngrams() -> usize {
    3
}
fn default_yake_min_doc_chars() -> usize {
    1000
}
fn default_yake_top_n_cap() -> usize {
    150
}

/// Section titles as entities (monograph spine): TOC headings level 2+
/// are authored topic names — the textbook analogue of numbered survey
/// environments. Titlecase multi-word titles become Method candidates.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SectionTitlesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default = "default_min_words")]
    pub min_words: usize,
    #[serde(default = "default_max_words")]
    pub max_words: usize,
    #[serde(default = "default_true")]
    pub require_capitalized: bool,
    #[serde(default)]
    pub generic_headings: Vec<String>,
}

impl Default for SectionTitlesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            min_words: default_min_words(),
            max_words: default_max_words(),
            require_capitalized: true,
            generic_headings: Vec::new(),
        }
    }
}

/// Junk label filters: GROBID page furniture leaking into labels.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LabelJunkFilters {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub junk_patterns: Vec<String>,
}

impl Default for LabelJunkFilters {
    fn default() -> Self {
        Self {
            enabled: true,
            junk_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProposalNounPhraseConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_acronym_entity")]
    pub entity_type: String,
    #[serde(default = "default_proposal_triggers")]
    pub triggers: Vec<String>,
    #[serde(default = "default_third_person_triggers")]
    pub third_person_triggers: Vec<String>,
    #[serde(default = "default_max_phrase_chars")]
    pub max_phrase_chars: usize,
}

impl Default for ProposalNounPhraseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            entity_type: default_acronym_entity(),
            triggers: default_proposal_triggers(),
            third_person_triggers: default_third_person_triggers(),
            max_phrase_chars: default_max_phrase_chars(),
        }
    }
}

fn default_proposal_triggers() -> Vec<String> {
    [
        "we propose",
        "we introduce",
        "we present",
        "we develop",
        "we describe",
        "we build",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
fn default_third_person_triggers() -> Vec<String> {
    ["propose", "proposes", "introduces", "presents"]
        .into_iter()
        .map(String::from)
        .collect()
}
fn default_max_phrase_chars() -> usize {
    40
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
    /// M446: reads from data/extraction_patterns.yaml (env DA_EXTRACTION_PATTERNS).
    pub fn load() -> Self {
        let primary = std::env::var("DA_EXTRACTION_PATTERNS")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "data/extraction_patterns.yaml".to_string());
        let paths = [
            primary,
            "../../data/extraction_patterns.yaml".into(),
            "../../../data/extraction_patterns.yaml".into(),
        ];
        for path in &paths {
            if std::path::Path::new(path).exists()
                && let Ok(config) = Self::from_yaml_file(path)
            {
                return config;
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExtractionCacheIdentity {
    pub implementation_fingerprint: String,
    pub configuration_fingerprint: String,
    pub output_schema_fingerprint: String,
}

/// Last `max_words` words of `text` (for acronym-definition phrase check).
/// Char-boundary-safe: reverse scan counting word starts; never slices on a
/// computed byte index (multibyte regression: 'µRL framework').
fn before_tail_words(text: &str, max_words: usize) -> Option<&str> {
    let trimmed = text.trim_end().trim_end_matches([',', ';', ':']);
    if trimmed.is_empty() {
        return None;
    }
    let mut words_found = 0usize;
    let mut in_word = false;
    let mut start_of_nth = None;
    for (i, c) in trimmed.char_indices().rev() {
        if c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words_found += 1;
            if words_found == max_words {
                start_of_nth = Some(i);
                break;
            }
        }
    }
    match start_of_nth {
        Some(i) => Some(&trimmed[i..]),
        None => Some(trimmed), // fewer words than max_words → whole tail
    }
}

/// True when the acronym's letters match the initials of the phrase words.
/// "Ingest-time Semantic Compilation" vs "ISC" → I,S,C ✓.
/// Lowercase significance: skip pure stop-words in the phrase.
fn initials_match(phrase: &str, acronym: &str) -> bool {
    let skip = ["of", "the", "and", "for", "a", "an", "to", "in", "on"];
    // hyphenated tokens contribute EACH part's initial: "senate-bills" → S,B
    // (specimen Table 2); "Ingest-time" alone contributes I (not I,T) —
    // subsequence matching tolerates extras either way.
    let initials: Vec<char> = phrase
        .split_whitespace()
        .filter(|w| !skip.contains(&w.to_lowercase().as_str()))
        .flat_map(|w| w.split(['-', '/']))
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.chars().next())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let acr: Vec<char> = acronym.chars().map(|c| c.to_ascii_uppercase()).collect();
    // exact initials sequence, or a subsequence (some words may not count)
    if initials == acr {
        return true;
    }
    // subsequence check: acronym letters appear in order among initials
    if initials.len() <= acr.len() {
        return false;
    }
    let mut ai = initials.iter();
    acr.iter().all(|a| ai.any(|i| i == a))
}

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
    models_prefix_lower: Vec<String>, // first segment before '-', lowercased
    datasets_lower: Vec<String>,
    metrics_lower: Vec<String>,
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
            models_prefix_lower: config
                .models
                .iter()
                .map(|s| s.split('-').next().unwrap_or(s).to_lowercase())
                .collect(),
            datasets_lower: config.datasets.iter().map(|s| s.to_lowercase()).collect(),
            metrics_lower: config.metrics.iter().map(|s| s.to_lowercase()).collect(),
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

    /// Cache identity for the actual loaded extraction contract.
    ///
    /// Hashes canonical serialized values rather than a filename, so bundled
    /// and filesystem configurations with identical DATA share a key while any
    /// rule mutation invalidates stale extraction output.
    pub fn cache_identity(&self) -> ExtractionCacheIdentity {
        use sha2::{Digest, Sha256};
        let config_bytes =
            serde_json::to_vec(&self.config).expect("ExtractionConfig serialization is infallible");
        ExtractionCacheIdentity {
            implementation_fingerprint: "da_adapters::RuleBasedExtractor@extract-v2".into(),
            configuration_fingerprint: hex::encode(Sha256::digest(config_bytes)),
            output_schema_fingerprint:
                da_ports::cache_payload::extracted_entity_output_schema_fingerprint(),
        }
    }

    /// Classify a section title to an entity type.
    /// e.g. "Datasets" → Dataset, "Methods" → Method, "Evaluation Setup" → Metric.
    #[inline]
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
    #[inline]
    /// Snap a byte index down to the nearest char boundary at or before it.
    /// Byte indices found in a `to_lowercase()` copy of `text` can point
    /// inside a multi-byte char of the original (lowercasing may change
    /// byte lengths), so slicing the original must snap first.
    fn snap_down(text: &str, index: usize) -> usize {
        let mut i = index.min(text.len());
        while i > 0 && !text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    /// Clamp an externally computed byte span to valid UTF-8 boundaries.
    ///
    /// Several extractors search a lowercased projection. Unicode lowercase
    /// expansion can shift those byte offsets relative to the original text;
    /// callers must never slice the original with the raw projected offsets.
    fn safe_span(text: &str, start: usize, end: usize) -> (usize, usize) {
        let start = Self::snap_down(text, start);
        let mut end = end.min(text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        (start, end.max(start))
    }

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
    /// Structural pattern: `<phrase> (ABBR)` where ABBR initials match the
    /// phrase's significant words. Catches NEW methods absent from
    /// whitelists: "ingest-time semantic compilation (ISC)" → Method[ISC].
    /// Hyphenated words contribute their first letter ("Ingest-time" → I).
    fn extract_acronym_definitions(
        &self,
        text: &str,
        entity_type: &EntityType,
    ) -> Vec<(usize, usize, String)> {
        let cfg = &self.config.structural.acronym_definitions;
        // Dataset включён для табличных псевдо-секций "Datasets" (ADR-071:
        // Table 2 specimen — "senate-bills (SB)" живёт только в ячейках)
        if !cfg.enabled
            || !matches!(
                entity_type,
                EntityType::Method | EntityType::Task | EntityType::Dataset
            )
        {
            return Vec::new();
        }
        let mut results = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while let Some(open) = text[i..].find('(') {
            let open_abs = i + open;
            // acronym: uppercase letters then ')'
            let inner_start = open_abs + 1;
            let mut j = inner_start;
            while j < bytes.len() && bytes[j].is_ascii_uppercase() {
                j += 1;
            }
            if j > inner_start
                && j < bytes.len()
                && bytes[j] == b')'
                && (j - inner_start) >= cfg.acronym_min_len
                && (j - inner_start) <= cfg.acronym_max_len
            {
                let acronym = &text[inner_start..j];
                // phrase: words immediately before '('
                let before = text[..open_abs].trim_end();
                if let Some(phrase) = before_tail_words(before, cfg.max_words_in_phrase) {
                    // hyphenated compounds count per part: "senate-bills" = 2
                    // words (Table 2 specimen datasets are single hyphenated
                    // tokens; min_words_in_phrase=2 must not reject them)
                    let word_count = phrase
                        .split_whitespace()
                        .flat_map(|w| w.split(['-', '/']))
                        .filter(|p| !p.is_empty())
                        .count();
                    if word_count >= cfg.min_words_in_phrase && initials_match(phrase, acronym) {
                        // span: phrase + " (ABBR)"
                        let start = open_abs - phrase.len();
                        results.push((start, j + 1, acronym.to_string()));
                    }
                }
            }
            i = open_abs + 1;
        }
        results
    }

    /// Structural pattern: noun phrase after proposal verbs, lowercase
    /// allowed. "we propose a compiled semantic substrate" → Method.
    /// Articles (a/an/the) are skipped; phrase runs to a stop word.
    fn extract_proposal_noun_phrases(
        &self,
        text: &str,
        entity_type: &EntityType,
    ) -> Vec<(usize, usize, String)> {
        let cfg = &self.config.structural.proposal_noun_phrases;
        if !cfg.enabled || !matches!(entity_type, EntityType::Method | EntityType::Task) {
            return Vec::new();
        }
        let lower = text.to_lowercase();
        const STOP_WORDS: &[&str] = &[
            "with", "that", "to", "of", "for", "on", "in", "by", "which", "and", "where", "while",
            "using", "as", "at", "into", "from", "such", "so",
            // articles survive the article-skip when a space precedes them
            // ("we present the substrate" → skip failed on leading space)
            "the", "a", "an", "our", "its",
        ];
        let mut results = Vec::new();
        // full triggers ("we propose") + bare verbs ("propose") both anchor here
        let mut triggers: Vec<&str> = cfg.triggers.iter().map(String::as_str).collect();
        triggers.extend(cfg.third_person_triggers.iter().map(String::as_str));
        for trigger in triggers {
            let mut start = 0;
            while let Some(pos) = lower[start..].find(trigger) {
                let after = start + pos + trigger.len();
                // word-boundary guard: "propose" inside "proposes" must not
                // fire (leaked "s ingest-time semantic compilation" label)
                if text[after..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric())
                {
                    start = after;
                    continue;
                }
                // skip whitespace, then a leading article
                let mut cursor = after;
                let ws = text[cursor..].len() - text[cursor..].trim_start().len();
                cursor += ws;
                let rest = &text[cursor..];
                for article in ["a ", "an ", "the ", "our ", "its "] {
                    // get(..n) is None when n is not a char boundary
                    // (multibyte regression: ' µRL framework' rest[..2])
                    if rest
                        .get(..article.len())
                        .is_some_and(|s| s.eq_ignore_ascii_case(article))
                    {
                        cursor += article.len();
                        break;
                    }
                }
                let remainder = &text[cursor..];
                // take words until stop word / sentence end / max chars
                let mut phrase_end = 0;
                for word in remainder.split_inclusive(char::is_whitespace) {
                    // split_inclusive yields separator-only chunks (" ") for
                    // leading whitespace — skip them before the first word.
                    let w = word.trim();
                    if w.is_empty() {
                        if phrase_end > 0 {
                            break; // trailing whitespace — phrase complete
                        }
                        continue; // leading whitespace
                    }
                    if w.ends_with(['.', ';', ':', ',']) {
                        break;
                    }
                    if STOP_WORDS.contains(&w.to_lowercase().as_str()) {
                        break;
                    }
                    if phrase_end + word.len() > cfg.max_phrase_chars {
                        break;
                    }
                    if w.chars().any(|c| c.is_ascii_digit()) {
                        break; // numbers signal measurements, not names
                    }
                    phrase_end += word.len();
                }
                let candidate = remainder[..phrase_end].trim();
                let words = candidate.split_whitespace().count();
                if (2..=6).contains(&words) && candidate.chars().any(|c| c.is_alphabetic()) {
                    let abs_start = cursor;
                    results.push((abs_start, cursor + phrase_end, candidate.to_string()));
                }
                start = cursor;
            }
        }
        results
    }

    /// Prose benchmark numbers (ADR-069 §2.5): harvest percentages / p-values /
    /// fold-changes / sample sizes from narrative text as Metric candidates.
    /// Vision papers carry results in prose — no Results section exists.
    fn extract_prose_benchmark_numbers(
        &self,
        text: &str,
        entity_type: &EntityType,
    ) -> Vec<(usize, usize, String)> {
        let cfg = &self.config.structural.prose_benchmark_numbers;
        if !cfg.enabled || !matches!(entity_type, EntityType::Metric) {
            return Vec::new();
        }
        let mut results = Vec::new();
        for (name, pat) in &cfg.patterns {
            let Ok(re) = regex::Regex::new(&pat.regex) else {
                continue;
            };
            for caps in re.captures_iter(text) {
                if results.len() >= cfg.max_per_section {
                    break;
                }
                let Some(m) = caps.get(0) else { continue };
                let Some(val) = caps.get(1).map(|v| v.as_str()) else {
                    continue;
                };
                // DATA-driven noise filters (sweep grids, zero/leading-comma)
                let mut digits: String = val
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if pat.strip_leading_zeros {
                    digits = digits.trim_start_matches('0').to_string();
                }
                if digits.is_empty() || digits == "." {
                    continue; // '2,048' comma-split left nothing
                }
                let numeric: Option<f64> = digits.parse().ok();
                if let Some(min) = pat.min_value
                    && let Some(v) = numeric
                    && v < min
                {
                    continue;
                }
                if let Some(step) = pat.drop_int_multiples_of
                    && let Some(v) = numeric
                    && v.fract() == 0.0
                    && (v / step).fract() == 0.0
                {
                    continue;
                }
                // label: prefix + captured number (+ suffix)
                let mut label = String::new();
                if let Some(p) = &pat.label_prefix {
                    label.push_str(p);
                }
                label.push_str(&digits);
                if let Some(s) = &pat.label_suffix {
                    label.push('_');
                    label.push_str(s);
                }
                let _ = name;
                results.push((m.start(), m.end(), label));
            }
        }
        results.sort_by_key(|(s, _, _)| *s);
        results.truncate(cfg.max_per_section);
        results
    }

    /// Numbered environments (survey spine, ADR-071): title form
    /// "M3. Isolating centrality." and inline form "M21." + name in the
    /// first sentence. Returns (env_id, label, name) triples.
    fn extract_numbered_environments(
        &self,
        sections: &[(String, String)],
    ) -> Vec<(usize, String, String)> {
        let cfg = &self.config.structural.numbered_environments;
        if !cfg.enabled || cfg.environment_prefixes.is_empty() {
            return Vec::new();
        }
        let alt = format!(
            "^({})[0-9]{{1,3}}\\.\\s*(.+)$",
            cfg.environment_prefixes.join("|")
        );
        let inline = format!(
            "\\b({})[0-9]{{1,3}}\\.\\s*([A-Z][^.{{}}]{{2,{}}}?)\\.",
            cfg.environment_prefixes.join("|"),
            cfg.max_name_len
        );
        let (Ok(title_re), Ok(inline_re)) = (regex::Regex::new(&alt), regex::Regex::new(&inline))
        else {
            return Vec::new();
        };
        let mut out: Vec<(usize, String, String)> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (ordinal, (title, text)) in sections.iter().enumerate() {
            if cfg.scan_title {
                if let Some(caps) = title_re.captures(title.trim())
                    && let (Some(env), Some(name)) = (caps.get(1), caps.get(2))
                {
                    let name = name.as_str().trim().trim_end_matches('.').to_string();
                    if !name.is_empty() && seen.insert(name.to_lowercase()) {
                        out.push((ordinal, env.as_str().to_string(), name));
                    }
                } else {
                    // bare environment title (specimen M21.): the name is the
                    // FIRST SENTENCE of the section text
                    let bare = format!(
                        "^({})[0-9]{{1,3}}\\.\\s*$",
                        cfg.environment_prefixes.join("|")
                    );
                    if let Ok(bare_re) = regex::Regex::new(&bare)
                        && bare_re.is_match(title.trim())
                    {
                        // 0 = full scan: не обрезать (take(0) дал бы пустоту —
                        // регрессия bare-title при scan_text_intro_chars=0)
                        let intro: String = if cfg.scan_text_intro_chars == 0 {
                            text.clone()
                        } else {
                            text.chars().take(cfg.scan_text_intro_chars).collect()
                        };
                        if let Some(dot) = intro.find('.') {
                            let name = intro[..dot].trim();
                            if name.len() >= 4
                                && name.chars().next().is_some_and(|c| c.is_uppercase())
                                && seen.insert(name.to_lowercase())
                            {
                                out.push((ordinal, title.trim().to_string(), name.to_string()));
                            }
                        }
                    }
                }
            }
            let scan: String = if cfg.scan_text_intro_chars == 0 {
                text.clone()
            } else {
                text.chars().take(cfg.scan_text_intro_chars).collect()
            };
            for caps in inline_re.captures_iter(&scan) {
                if let (Some(env), Some(name)) = (caps.get(1), caps.get(2)) {
                    let name = name.as_str().trim().to_string();
                    if !name.is_empty() && seen.insert(name.to_lowercase()) {
                        out.push((ordinal, env.as_str().to_string(), name));
                    }
                }
            }
        }
        out
    }

    /// YAKE keyphrases (recall layer): whole-document statistical ranking.
    /// Returns (raw_first_occurrence, keyword_lower, score) — raw is used
    /// for evidence-span location, score for ranking (lower = better).
    fn extract_yake_keyphrases(&self, sections: &[(String, String)]) -> Vec<(String, String, f64)> {
        let cfg = &self.config.structural.yake_keyphrases;
        if !cfg.enabled || sections.is_empty() {
            return Vec::new();
        }
        let doc_len: usize = sections
            .iter()
            .map(|(t, x)| t.chars().count() + x.chars().count())
            .sum();
        if doc_len < cfg.min_doc_chars {
            return Vec::new();
        }
        let top_n = if cfg.top_n_per_25k_chars {
            // монография 1M символов: базовый top 15 калиброван под статью
            let scaled = cfg.top_n.saturating_mul(doc_len / 25_000);
            scaled.clamp(cfg.top_n, cfg.top_n_cap)
        } else {
            cfg.top_n
        };
        let mut stop: std::collections::HashSet<String> = yake_rust::StopWords::predefined("en")
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for w in &cfg.extra_stopwords {
            stop.insert(w.to_lowercase());
        }
        let text = sections
            .iter()
            .map(|(t, x)| format!("{t} {x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let yc = yake_rust::Config {
            ngrams: cfg.ngrams,
            ..yake_rust::Config::default()
        };
        let mut items: Vec<(String, String, f64)> =
            yake_rust::get_n_best(top_n, &text, &yake_rust::StopWords::custom(stop), &yc)
                .into_iter()
                .map(|item| (item.raw, item.keyword, item.score))
                .collect();
        if cfg.require_multiword {
            items.retain(|(raw, _, _)| raw.split_whitespace().count() >= 2);
        }
        // substring dedup: обрезки полного фразы («Ingest-time semantic» при
        // живом «Ingest-time semantic compilation») — держим более длинную
        items.sort_by_key(|(raw, _, _)| std::cmp::Reverse(raw.split_whitespace().count()));
        let mut kept: Vec<(String, String, f64)> = Vec::new();
        for item in items {
            let is_fragment = kept.iter().any(|(kraw, _, _)| {
                let a = kraw.to_lowercase();
                let b = item.0.to_lowercase();
                a != b && (a.contains(&b) || b.contains(&a))
            });
            if !is_fragment {
                kept.push(item);
            }
        }
        kept
    }

    /// Section titles as entities (monograph spine, ADR-071 companion):
    /// titlecase multi-word headings are authored topic names. Generic
    /// headings (Introduction/Exercises/Summary) excluded via DATA.
    fn extract_section_titles(&self, sections: &[(String, String)]) -> Vec<(usize, String)> {
        let cfg = &self.config.structural.section_titles;
        if !cfg.enabled {
            return Vec::new();
        }
        let generic: std::collections::HashSet<String> = cfg
            .generic_headings
            .iter()
            .map(|g| g.to_lowercase())
            .collect();
        let mut out = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (ordinal, (title, _)) in sections.iter().enumerate() {
            let t = title.trim().trim_end_matches(['.', ':']);
            let words: Vec<&str> = t.split_whitespace().collect();
            if words.len() < cfg.min_words || words.len() > cfg.max_words {
                continue;
            }
            // числовые и буквенные префиксы заголовков:
            // "2.3.1 Alpha Vectors" → Alpha Vectors; "G Datasets",
            // "E.1 Benchmarks" (appendix lettering) → отбрасываются
            let strip_prefix = |w: &&str| -> bool {
                let bare = w.trim_end_matches(['.', ':']);
                if bare.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return true; // 2.3.1 / 2.
                }
                // одиночная буква приложения: G / E. / A.2
                let letters: Vec<char> = bare.chars().filter(|c| c.is_ascii_alphabetic()).collect();
                letters.len() == 1
                    && bare.chars().next().is_some_and(|c| c.is_uppercase())
                    && w.len() <= 4
            };
            let content: Vec<&str> = words
                .iter()
                .skip_while(|w| strip_prefix(w))
                .copied()
                .collect();
            if content.len() < cfg.min_words {
                continue;
            }
            if cfg.require_capitalized {
                let caps = content
                    .iter()
                    .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
                    .count();
                if caps < content.len().min(2) {
                    continue;
                }
            }
            let label = content.join(" ");
            // generic-заголовок — после срезки префиксов: 'e.1 benchmarks'
            // → 'benchmarks' → generic (регрессия фикстуры G Datasets)
            let ll = label.to_lowercase();
            if generic.contains(&ll) || generic.iter().any(|g| ll.starts_with(&format!("{g} "))) {
                continue;
            }
            if seen.insert(label.to_lowercase()) {
                out.push((ordinal, label));
            }
        }
        out
    }

    /// Is this label junk? GROBID page furniture leaking into labels
    /// ("resultsManuscript submitted"). DATA-driven junk_patterns.
    fn label_is_junk(&self, label: &str) -> bool {
        let cfg = &self.config.structural.label_junk_filters;
        if !cfg.enabled {
            return false;
        }
        cfg.junk_patterns.iter().any(|pat| {
            regex::Regex::new(pat)
                .map(|re| re.is_match(label))
                .unwrap_or(false)
        })
    }

    /// Recurring dominant terms (survey pattern, ADR-069 §2.4 companion):
    /// words/bigrams repeated ≥ min_occurrences across the whole document are
    /// core objects of the work. Counted over all section texts (title+text),
    /// lowercase-folded, generic vocabulary stoplisted via DATA.
    fn extract_recurring_terms(&self, sections: &[(String, String)]) -> Vec<String> {
        let cfg = &self.config.structural.recurring_terms;
        if !cfg.enabled {
            return Vec::new();
        }
        // genre gate: survey markers in section titles (Related Surveys,
        // Paper Outline, Scope and Contributions, taxonomy) — regular papers
        // skip the pass entirely (noise regression on 2608.19799)
        if !cfg.survey_heading_markers.is_empty() {
            let marker_res: Vec<regex::Regex> = cfg
                .survey_heading_markers
                .iter()
                .filter_map(|m| regex::Regex::new(m).ok())
                .collect();
            let is_survey = sections
                .iter()
                .any(|(title, _)| marker_res.iter().any(|re| re.is_match(title)));
            if !is_survey {
                return Vec::new();
            }
        }
        let mut stop: std::collections::HashSet<String> = cfg
            .generic_stoplist
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        stop.extend(cfg.english_stopwords.iter().map(|s| s.to_lowercase()));
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (title, text) in sections {
            let scan = format!("{title} {text}").to_lowercase();
            for word in scan.split(|c: char| !c.is_alphanumeric()) {
                // char-count (не bytes): math italic 𝑒 = 4 BYTES, 1 char
                if word.chars().count() < cfg.min_word_len || stop.contains(word) {
                    continue;
                }
                // математические курсивные глифы — не термины
                if cfg.require_ascii_letters && !word.chars().any(|c| c.is_ascii_alphabetic()) {
                    continue;
                }
                *counts.entry(word.to_string()).or_insert(0) += 1;
            }
        }
        let qualified: std::collections::HashSet<String> = counts
            .iter()
            .filter(|(_, n)| **n >= cfg.min_occurrences)
            .map(|(w, _)| w.clone())
            .collect();
        let mut terms: Vec<String> = qualified
            .iter()
            .filter(|w| {
                // plural дубли: 'hypergraphs'→'hypergraph', 'centralities'→'centrality'
                if cfg.drop_plural_when_singular_qualifies && w.ends_with('s') && w.len() > 4 {
                    let mut stems: Vec<String> = vec![w[..w.len() - 1].to_string()];
                    if let Some(stripped) = w.strip_suffix("ies") {
                        stems.push(format!("{stripped}y"));
                    }
                    if let Some(stripped) = w.strip_suffix("es") {
                        stems.push(stripped.to_string());
                    }
                    if stems.iter().any(|s| qualified.contains(s)) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        terms.sort();
        terms
    }

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
            // Structural patterns (DATA-driven, new-term discovery):
            // acronym definitions "phrase (ABBR)" + proposal noun phrases.
            // Deduped downstream by label; cheap relative to whitelist scans.
            results.extend(self.extract_acronym_definitions(text, entity_type));
            results.extend(self.extract_proposal_noun_phrases(text, entity_type));

            for pattern in &["we propose ", "we present ", "we introduce ", "we use "] {
                let lower_text = text.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower_text.get(start..).and_then(|s| s.find(pattern)) {
                    let abs = Self::snap_down(text, start + pos + pattern.len());
                    if abs >= text.len() {
                        break;
                    }
                    // Extract the first word (up to next space, comma, or 30 chars).
                    // The terminator search runs on the ORIGINAL text slice, so
                    // every byte index is a char boundary by construction.
                    let remainder = &text[abs..];
                    let end = remainder
                        .find(|c: char| c.is_whitespace() || c == ',' || c == '.')
                        .unwrap_or_else(|| {
                            let mut i = remainder.len().min(30);
                            while i > 0 && !remainder.is_char_boundary(i) {
                                i -= 1;
                            }
                            i
                        });
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
                // Table pseudo-sections (ADR-071): "senate-bills (SB)" lives
                // only in table cells — acronym definitions fire here too.
                results.extend(self.extract_acronym_definitions(text, entity_type));
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
                // Prose benchmark numbers (ADR-069 §2.5) + the global metric
                // whitelist pass in extract().
                results.extend(self.extract_prose_benchmark_numbers(text, entity_type));
            }
            EntityType::Model => {
                // Model extraction is handled by the global model pass in extract().
                // No section-specific extraction needed here.
            }
            EntityType::Method => {
                // Same whitelist as global pass: acronyms + multi-word phrases.
                // Blind all-caps scan produced too many false positives (87 Methods
                // in one paper). Whitelist keeps precision high.
                // Case-insensitive search: GROBID may normalize casing (ppo, Cot).
                // M313: section arm previously scanned acronyms only — phrases
                // lived only in the global pass (logic gap vs Task section).
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
                for (p_lower, phrase_canonical) in self
                    .lowered
                    .method_phrases_lower
                    .iter()
                    .zip(self.config.methods.phrases.iter())
                {
                    let mut start = 0;
                    while let Some(pos) = lower_text[start..].find(p_lower.as_str()) {
                        let abs = start + pos;
                        // Match length must come from the lowercased needle
                        // (same bytes as lower_text), not the canonical label.
                        let end = abs + p_lower.len();
                        if Self::word_boundary(text, abs, end) {
                            results.push((abs, end, phrase_canonical.clone()));
                        }
                        start = end;
                    }
                }
            }
            EntityType::Task => {
                // Known task phrases + acronyms. Case-insensitive with word
                // boundary to avoid substring false positives.
                // M314: end index must use lowercased needle length (same as Method M313).
                let task_phrases = &self.config.tasks.phrases;
                let task_acronyms = &self.config.tasks.acronyms;
                let lower = text.to_lowercase();
                for phrase in task_phrases {
                    let p_lower = phrase.to_lowercase();
                    let mut start = 0;
                    while let Some(pos) = lower[start..].find(&p_lower) {
                        let abs = start + pos;
                        let end = abs + p_lower.len();
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
                        let end = abs + a_lower.len();
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
    /// M446: port-level cache identity bridge — exposes the hashed rule
    /// DATA so RuleExtractionStage contracts invalidate on dictionary
    /// changes even when callers pass a static fingerprint string.
    fn cache_fingerprint(&self) -> Option<String> {
        Some(self.cache_identity().configuration_fingerprint)
    }
    async fn extract(&self, sections: &[(String, String)]) -> ExtractResult<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
            if let Some(entity_type) = Self::classify_section(title) {
                let candidates = self.extract_candidates(text, &entity_type);
                for (char_start, char_end, label) in candidates {
                    if self.label_is_junk(&label) {
                        continue;
                    }
                    let (char_start, char_end) = Self::safe_span(text, char_start, char_end);
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: entity_type.clone(),
                        section_title: title.clone(),
                        section_ordinal: Some(section_ordinal),
                        char_start,
                        char_end,
                        surface: text[char_start..char_end].to_string(),
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
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
            let scan_text = format!("{} {}", title, text);
            let acronyms = self.extract_method_acronyms_global(&scan_text);
            for (char_start, char_end, label) in acronyms {
                let key = (label.to_lowercase(), "method".to_string());
                if !seen.contains(&key) {
                    seen.insert(key);
                    let (char_start, char_end) = Self::safe_span(&scan_text, char_start, char_end);
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: EntityType::Method,
                        section_title: title.clone(),
                        section_ordinal: Some(section_ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                }
            }
        }

        // Global Method phrase pass: scan ALL sections for known multi-word
        // method phrases (self-evolving memory, chain-of-thought reasoning).
        // These are compound method names that cannot be captured by the
        // all-caps acronym pass — they appear as lowercase/titlecase phrases.
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
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
                    // end must use lowercased needle length (indices into `lower` /
                    // same UTF-8 length for ASCII phrases). M313: was phrase_canonical.len().
                    let end = abs + p_lower.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (p_lower.clone(), "method".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: phrase_canonical.to_string(),
                                entity_type: EntityType::Method,
                                section_title: title.clone(),
                                section_ordinal: Some(section_ordinal),
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
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
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
                                section_ordinal: Some(section_ordinal),
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
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
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
                            section_ordinal: Some(section_ordinal),
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
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
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
                            section_ordinal: Some(section_ordinal),
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
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
            let scan_text = format!("{} {}", title, text);
            let lower = scan_text.to_lowercase();
            for phrase in task_phrases {
                let p_lower = phrase.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&p_lower) {
                    let abs = start + pos;
                    // M314: lowercased needle length (indices into `lower`).
                    let end = abs + p_lower.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (p_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: phrase.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                section_ordinal: Some(section_ordinal),
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
                    let end = abs + a_lower.len();
                    if Self::word_boundary(&scan_text, abs, end) {
                        let key = (a_lower.clone(), "task".to_string());
                        if !seen.contains(&key) {
                            seen.insert(key);
                            entities.push(ExtractedEntity {
                                label: acr.to_string(),
                                entity_type: EntityType::Task,
                                section_title: title.clone(),
                                section_ordinal: Some(section_ordinal),
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

        // Global structural pass (2608.20845 lesson): NEW-term discovery must
        // scan ALL sections — definitions ("phrase (ABBR)") and proposals
        // ("we propose X") usually appear in Abstract/Introduction, which
        // classify_section leaves unclassified. Whitelist-based global passes
        // cannot see terms absent from extraction_patterns.yaml.
        let mut prose_total: usize = 0;
        for (section_ordinal, (title, text)) in sections.iter().enumerate() {
            let scan_text = format!("{} {}", title, text);
            let structural_hits: Vec<_> = {
                let mut hits = self.extract_acronym_definitions(&scan_text, &EntityType::Method);
                hits.extend(self.extract_proposal_noun_phrases(&scan_text, &EntityType::Method));
                hits
            };
            // Prose benchmark numbers are Metric-typed (ADR-069 §2.5):
            // vision papers report results in narrative prose — harvest
            // globally, not only in Metric-classified sections.
            let prose_metrics =
                self.extract_prose_benchmark_numbers(&scan_text, &EntityType::Metric);
            for (char_start, char_end, label) in prose_metrics {
                let key = (label.to_lowercase(), "metric".to_string());
                if prose_total
                    >= self
                        .config
                        .structural
                        .prose_benchmark_numbers
                        .global_cap_per_paper
                {
                    break;
                }
                if !seen.contains(&key) {
                    prose_total += 1;
                    seen.insert(key);
                    let (char_start, char_end) = Self::safe_span(&scan_text, char_start, char_end);
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: EntityType::Metric,
                        section_title: title.clone(),
                        section_ordinal: Some(section_ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                }
            }
            for (char_start, char_end, label) in structural_hits {
                if self.label_is_junk(&label) {
                    continue;
                }
                let key = (label.to_lowercase(), "method".to_string());
                if !seen.contains(&key) {
                    seen.insert(key);
                    let (char_start, char_end) = Self::safe_span(&scan_text, char_start, char_end);
                    entities.push(ExtractedEntity {
                        label,
                        entity_type: EntityType::Method,
                        section_title: title.clone(),
                        section_ordinal: Some(section_ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                }
            }
        }

        // Numbered environments (survey spine, ADR-071): closed index of
        // survey objects — "M3. Isolating centrality." in titles and intro
        // slices. 39/39 recall on the specimen vs ~20/39 by frequency.
        for (ordinal, env_id, name) in self.extract_numbered_environments(sections) {
            if self.label_is_junk(&name) {
                continue;
            }
            let key = (name.to_lowercase(), "method".to_string());
            if !seen.contains(&key)
                && let Some((title, text)) = sections.get(ordinal)
            {
                let scan_text = format!("{} {}", title, text);
                let lower = scan_text.to_lowercase();
                if let Some(pos) = lower.find(&name.to_lowercase()) {
                    seen.insert(key);
                    let (char_start, char_end) = Self::safe_span(&scan_text, pos, pos + name.len());
                    entities.push(ExtractedEntity {
                        label: name.clone(),
                        entity_type: EntityType::Method,
                        section_title: title.clone(),
                        section_ordinal: Some(ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                }
                let _ = env_id;
            }
        }

        // YAKE keyphrases (recall layer, statistical): untyped candidates
        // ranked by five local features; evidence-gated like everything
        // else. Complements typed passes — catches multi-word terms the
        // noun-phrase pass misses (proposal verbs) and ranks by position/
        // casing/frequency principled (no hand stoplists).
        for (raw, keyword, _score) in self.extract_yake_keyphrases(sections) {
            if self.label_is_junk(&raw) {
                continue;
            }
            let key = (keyword.to_lowercase(), "method".to_string());
            if seen.contains(&key) {
                continue;
            }
            // evidence span: locate the raw first occurrence in sections
            let mut placed = false;
            for (section_ordinal, (title, text)) in sections.iter().enumerate() {
                let scan_text = format!("{} {}", title, text);
                let lower = scan_text.to_lowercase();
                if let Some(pos) = lower.find(&keyword) {
                    let (char_start, char_end) = Self::safe_span(&scan_text, pos, pos + raw.len());
                    seen.insert(key.clone());
                    entities.push(ExtractedEntity {
                        label: raw.clone(),
                        entity_type: EntityType::Method,
                        section_title: title.clone(),
                        section_ordinal: Some(section_ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                    placed = true;
                    break;
                }
            }
            if !placed {
                continue;
            }
        }

        // Section titles as entities (monograph spine): TOC headings are
        // authored topic inventory — actor-critic/bandit/alpha-vector lived
        // ONLY in headings on the algorithms-for-decision-making specimen.
        for (ordinal, label) in self.extract_section_titles(sections) {
            if self.label_is_junk(&label) {
                continue;
            }
            let key = (label.to_lowercase(), "method".to_string());
            if !seen.contains(&key)
                && let Some((title, text)) = sections.get(ordinal)
            {
                let scan_text = format!("{} {}", title, text);
                let lower = scan_text.to_lowercase();
                if let Some(pos) = lower.find(&label.to_lowercase()) {
                    seen.insert(key);
                    let (char_start, char_end) =
                        Self::safe_span(&scan_text, pos, pos + label.len());
                    entities.push(ExtractedEntity {
                        label: label.clone(),
                        entity_type: EntityType::Method,
                        section_title: title.clone(),
                        section_ordinal: Some(ordinal),
                        char_start,
                        char_end,
                        surface: scan_text[char_start..char_end].to_string(),
                    });
                }
            }
        }

        // Recurring dominant terms (survey pattern): core objects repeated
        // across the whole document (centrality in a centrality survey).
        for term in self.extract_recurring_terms(sections) {
            if self.label_is_junk(&term) {
                continue;
            }
            let key = (term.to_lowercase(), "method".to_string());
            if !seen.contains(&key) && !seen.contains(&(term.to_lowercase(), "task".to_string())) {
                seen.insert(key);
                // first occurrence span for evidence
                let mut placed = false;
                for (section_ordinal, (title, text)) in sections.iter().enumerate() {
                    let scan_text = format!("{} {}", title, text);
                    if let Some(pos) = scan_text.to_lowercase().find(&term) {
                        let (char_start, char_end) =
                            Self::safe_span(&scan_text, pos, pos + term.len());
                        entities.push(ExtractedEntity {
                            label: term.clone(),
                            entity_type: EntityType::Method,
                            section_title: title.clone(),
                            section_ordinal: Some(section_ordinal),
                            char_start,
                            char_end,
                            surface: scan_text[char_start..char_end].to_string(),
                        });
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    continue;
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

    // ── structural patterns: acronym definitions (2608.20845 gap) ──
    // New-paper regression: "ingest-time semantic compilation (ISC)" —
    // method absent from whitelists, lowercase phrase, acronym in parens.

    // ── survey patterns (specimen 2512.00107: 39 centrality measures) ──
    #[test]
    fn test_recurring_terms_core_objects_of_survey() {
        let extractor = RuleBasedExtractor::new();
        let text = "centrality ".repeat(20);
        // survey genre markers in headings gate the pass (2512.00107 pattern)
        let sections = vec![
            ("Related Surveys".to_string(), String::new()),
            ("Introduction".to_string(), format!("Hypergraph {text}")),
        ];
        let terms = extractor.extract_recurring_terms(&sections);
        assert!(
            terms.contains(&"centrality".to_string()),
            "dominant term must surface: {terms:?}"
        );
        // stoplist: structural is vocabulary, not a method
        let text2 = format!("{}{}", "structural ".repeat(30), "centrality ".repeat(20));
        let terms2 =
            extractor.extract_recurring_terms(&[("Scope and Contributions".to_string(), text2)]);
        assert!(
            !terms2.contains(&"structural".to_string()),
            "stoplisted: {terms2:?}"
        );
        // genre gate: regular IMRaD paper without survey markers skips the pass
        let terms3 = extractor.extract_recurring_terms(&[(
            "Introduction".to_string(),
            format!("code {}", "task ".repeat(40)),
        )]);
        assert!(
            terms3.is_empty(),
            "no survey markers → pass must not fire: {terms3:?}"
        );
    }

    // ── numbered environments (survey spine, specimen 2512.00107) ──
    #[test]
    fn test_numbered_environments_title_and_inline() {
        let extractor = RuleBasedExtractor::new();
        let sections = vec![
            // title form
            ("M3. Isolating centrality.".to_string(), "some text".into()),
            // inline form: env id + name as first sentence
            (
                "On measures".to_string(),
                "M21. Core-periphery score. Both works proposed...".into(),
            ),
        ];
        let envs = extractor.extract_numbered_environments(&sections);
        let names: Vec<&str> = envs.iter().map(|(_, _, n)| n.as_str()).collect();
        assert!(
            names.contains(&"Isolating centrality"),
            "title form: {names:?}"
        );
        assert!(
            names.contains(&"Core-periphery score"),
            "inline form: {names:?}"
        );
    }

    // ── table acronym → Dataset (specimen Table 2) ──
    #[test]
    fn test_numbered_environments_bare_title() {
        let extractor = RuleBasedExtractor::new();
        let sections = vec![
            (
                "Previous discussion".to_string(),
                "as noted in M20. Something else here".into(),
            ),
            // bare env title + name as first sentence (specimen M21)
            (
                "M21.".to_string(),
                "Core-periphery score. Both Tudisco and Higham proposed methods.".into(),
            ),
        ];
        let envs = extractor.extract_numbered_environments(&sections);
        let names: Vec<&str> = envs.iter().map(|(_, _, n)| n.as_str()).collect();
        assert!(
            names.contains(&"Core-periphery score"),
            "bare-title M21 must pair with first sentence: {names:?}"
        );
    }

    #[test]
    fn test_table_dataset_acronym_typed_as_dataset() {
        let extractor = RuleBasedExtractor::new();
        let text = "senate-bills (SB) . 294 . 29,157 . email-eu (EEU) . 986";
        let cands = extractor.extract_candidates(text, &EntityType::Dataset);
        assert!(
            cands
                .iter()
                .any(|(_, _, l)| l == "SB" || l.contains("senate")),
            "table dataset acronyms must surface in Dataset arm: {cands:?}"
        );
    }

    #[test]
    fn test_junk_label_filtered() {
        let extractor = RuleBasedExtractor::new();
        assert!(extractor.label_is_junk("resultsManuscript submitted"));
        assert!(extractor.label_is_junk("Preprint Server 2025"));
        assert!(!extractor.label_is_junk("ingest-time semantic compilation"));
    }

    // ── ADR-069 §2.5: prose benchmark numbers (vision papers have no Results) ──
    #[test]
    fn test_prose_percentage_extracted_as_metric() {
        let extractor = RuleBasedExtractor::new();
        let text = "At a 2,048-token budget facts answered 85.2% correctly from roughly 2.2k reader tokens, while the best chunk configuration reached 72.5%.";
        let cands = extractor.extract_candidates(text, &EntityType::Metric);
        assert!(
            cands.iter().any(|(_, _, l)| l.contains("85.2")),
            "85.2% must be harvested from prose, got: {cands:?}"
        );
    }

    #[test]
    fn test_prose_sample_size_extracted() {
        let extractor = RuleBasedExtractor::new();
        let text =
            "Results are reported on a held-out sample of 500 transcripts and 499 questions.";
        let cands = extractor.extract_candidates(text, &EntityType::Metric);
        assert!(
            cands.iter().any(|(_, _, l)| l.starts_with("n_")),
            "sample size must become n_* metric, got: {cands:?}"
        );
    }

    #[test]
    fn test_acronym_definition_catches_new_method() {
        let extractor = RuleBasedExtractor::new();
        let text =
            "Ingest-time semantic compilation (ISC) compiles corpus meaning into validated claims.";
        let cands = extractor.extract_candidates(text, &EntityType::Method);
        assert!(
            cands
                .iter()
                .any(|(_, _, l)| l == "ISC" || l.contains("semantic compilation")),
            "ISC must be discovered via acronym-definition pattern, got: {cands:?}"
        );
    }

    #[test]
    fn test_acronym_definition_initials_must_match() {
        let extractor = RuleBasedExtractor::new();
        // "(RAG)" preceded by unrelated phrase — initials must NOT match
        let text = "The query pipeline reads every document (RAG) at answer time.";
        let cands = extractor.extract_candidates(text, &EntityType::Method);
        assert!(
            !cands.iter().any(|(_, _, l)| l == "RAG" && l.len() == 3),
            "acronym without matching initials must not be discovered: {cands:?}"
        );
    }

    #[test]
    fn test_proposal_noun_phrase_lowercase_allowed() {
        let extractor = RuleBasedExtractor::new();
        let text = "We propose a compiled semantic substrate with maintenance contracts.";
        let cands = extractor.extract_candidates(text, &EntityType::Method);
        assert!(
            cands
                .iter()
                .any(|(_, _, l)| l.to_lowercase().contains("semantic substrate")),
            "lowercase noun phrase after 'we propose a' must be caught, got: {cands:?}"
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

    #[test]
    fn test_safe_span_encloses_multibyte_character() {
        let text = "a•b";
        let inside_bullet = text.find('•').unwrap() + 1;
        let (start, end) = RuleBasedExtractor::safe_span(text, inside_bullet, inside_bullet + 1);
        assert!(text.is_char_boundary(start));
        assert!(text.is_char_boundary(end));
        assert_eq!(&text[start..end], "•");
    }

    #[tokio::test]
    async fn test_extract_unicode_offsets_are_valid_boundaries() {
        let extractor = RuleBasedExtractor::new();
        // Minimal excerpt from 2005.14165: the bullet begins at byte 333 in
        // the full section and previously produced start=334, inside `•`.
        let sections = vec![(
            "Word Scrambling and Manipulation Tasks".to_string(),
            "The five tasks are:• Cycle letters in word (CL) - The model recovers the original word."
                .to_string(),
        )];
        let entities = extractor.extract(&sections).await.unwrap();
        assert!(
            entities.iter().any(|entity| entity.label == "CL"),
            "the acronym definition remains extractable"
        );
        for entity in &entities {
            assert!(entity.char_start <= entity.char_end);
        }
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

/// M313: Method *section* arm must scan phrases (not only acronyms).
/// Body text under a Method-classified section title must hit method_phrases
/// without relying solely on the later global phrase pass.
#[tokio::test]
async fn test_method_section_arm_extracts_phrases() {
    let extractor = RuleBasedExtractor::new();
    // Title "Method" classifies the section as Method → section arm runs.
    let sections = vec![(
        "Method".to_string(),
        "Our approach uses self-evolving memory and tests generalization.".to_string(),
    )];
    let entities = extractor.extract(&sections).await.unwrap();
    let methods: Vec<&str> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Method)
        .map(|e| e.label.as_str())
        .collect();
    assert!(
        methods.contains(&"self-evolving memory"),
        "section Method arm should extract phrase, got: {methods:?}"
    );
    assert!(
        methods.contains(&"generalization"),
        "E1 gold FN phrase 'generalization' should extract, got: {methods:?}"
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
fn cache_identity_is_stable_and_matches_equivalent_config() {
    let bundled = RuleBasedExtractor::with_config(ExtractionConfig::bundled());
    let equivalent = RuleBasedExtractor::with_config(bundled.config().clone());
    let first = bundled.cache_identity();
    let second = bundled.cache_identity();
    assert_eq!(first, second);
    assert_eq!(first, equivalent.cache_identity());
    assert_eq!(first.configuration_fingerprint.len(), 64);
    assert_eq!(
        first.implementation_fingerprint,
        "da_adapters::RuleBasedExtractor@extract-v2"
    );
    assert_eq!(
        first.output_schema_fingerprint,
        da_ports::cache_payload::extracted_entity_output_schema_fingerprint()
    );
    assert_eq!(first.output_schema_fingerprint.len(), 64);
}

#[test]
fn cache_identity_changes_when_actual_rule_data_changes() {
    let original = ExtractionConfig::bundled();
    let mut changed = original.clone();
    changed.methods.phrases.push("new scientific method".into());
    let original_id = RuleBasedExtractor::with_config(original).cache_identity();
    let changed_id = RuleBasedExtractor::with_config(changed).cache_identity();
    assert_ne!(
        original_id.configuration_fingerprint,
        changed_id.configuration_fingerprint
    );
    assert_eq!(
        original_id.implementation_fingerprint,
        changed_id.implementation_fingerprint
    );
    assert_eq!(
        original_id.output_schema_fingerprint,
        changed_id.output_schema_fingerprint
    );
}

#[test]
fn cache_identity_uses_loaded_values_not_path() {
    let candidates = [
        "../../../data/extraction_patterns.yaml",
        "../../data/extraction_patterns.yaml",
        "data/extraction_patterns.yaml",
    ];
    let path = candidates
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .expect("extraction_patterns.yaml should exist");
    let file = ExtractionConfig::from_yaml_file(path).unwrap();
    let bundled = ExtractionConfig::bundled();
    assert_eq!(file, bundled);
    assert_eq!(
        RuleBasedExtractor::with_config(file).cache_identity(),
        RuleBasedExtractor::with_config(bundled).cache_identity()
    );
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

/// M314: Task phrase spans must use lowercased match length (char_end - char_start).
/// Global pass indexes into `title + " " + body`, not body alone.
#[tokio::test]
async fn test_task_phrase_span_uses_lower_needle_len() {
    let extractor = RuleBasedExtractor::new();
    let title = "Abstract";
    let body = "We study prompt optimization carefully.";
    let sections = vec![(title.to_string(), body.to_string())];
    let entities = extractor.extract(&sections).await.unwrap();
    let hit = entities
        .iter()
        .find(|e| e.entity_type == EntityType::Task && e.label == "prompt optimization")
        .expect("prompt optimization Task should extract");
    let expected_len = "prompt optimization".len();
    assert_eq!(
        hit.char_end - hit.char_start,
        expected_len,
        "Task phrase span must equal lowercased needle length, got start={} end={}",
        hit.char_start,
        hit.char_end
    );
    // Spans are into scan_text = "{title} {body}" (global Task pass).
    let scan = format!("{title} {body}");
    let slice = &scan[hit.char_start..hit.char_end];
    assert_eq!(slice.to_lowercase(), "prompt optimization");
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

#[cfg(test)]
mod char_boundary_tests {
    use super::*;

    /// Regression (M439 sweep panic): a multi-byte char ('µ') straddling the
    /// 30-byte candidate cutoff must not panic. The cutoff index comes from a
    /// byte count, so it can land mid-char; the slice must snap to a boundary.
    #[test]
    fn candidate_cutoff_inside_multibyte_char_does_not_panic() {
        let extractor = RuleBasedExtractor::new();
        // "We propose µ" + padding: after "we propose " (11 bytes), remainder
        // starts inside/at 'µ' (2 bytes); min(len,30) can land mid-char when
        // no terminator appears within 30 bytes.
        // Bytes of remainder: 29×'a', then 'µ' (2 bytes: 29..31), then letters.
        // The 30-byte cutoff lands on the continuation byte of 'µ'.
        let text = "We propose aaaaaaaaaaaaaaaaaaaaaaaaaaaaa\u{b5}bcdefghij";
        debug_assert_eq!(text[11..].len(), 40);
        let candidates = extractor.extract_candidates(text, &EntityType::Method);
        // No panic == pass.
        let _ = candidates;
    }

    /// Same class: multibyte char immediately after the pattern boundary.
    #[test]
    fn multibyte_char_immediately_after_pattern_boundary() {
        let extractor = RuleBasedExtractor::new();
        let text = "We propose µRL framework";
        let candidates = extractor.extract_candidates(text, &EntityType::Method);
        let _ = candidates;
    }
}
