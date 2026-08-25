//! Source-import FSM (M451): explicit states, transitions, guards.
//!
//! The orchestrating agent/CLI reads the stage->tool registry from
//! data/ontology/source_import_tools.yaml (DATA) and walks an article
//! object through the import pipeline. This module owns the FSM itself:
//! stage order, guard rules (grammar-profile optional stages, lazy
//! stages), and deriving the current stage from a manifest's
//! append-only lifecycle events (events are the truth, the stored
//! `lifecycle` field is derived and validated elsewhere).
//!
//! Meta/prompt: no tool knowledge here — the registry is DATA; the FSM
//! only reasons about stages and guards.

use serde::{Deserialize, Serialize};

/// Ordered pipeline stages (the canonical order).
pub const STAGE_ORDER: [&str; 7] = [
    "raw",
    "parsed",
    "structured",
    "extracted",
    "enriched",
    "summarized",
    "validated",
];

/// One tool able to execute a transition (from the DATA registry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageTool {
    pub id: String,
    /// rust | reference
    pub r#impl: String,
    pub cli: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// action-level retry policy (DATA; GROBID warm-up, OpenAlex 504)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    /// pre-stage hook command (DATA; path never hardcoded in Rust)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_hook: Option<PreHook>,
}

/// Retry policy for one tool's CLI action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default = "default_retry_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_retry_backoff")]
    pub backoff_ms: u64,
    #[serde(default = "default_retry_factor")]
    pub factor: f32,
    /// retry when any of these substrings appears in the failed output
    #[serde(default)]
    pub retry_on: Vec<String>,
}

fn default_retry_attempts() -> u32 { 2 }
fn default_retry_backoff() -> u64 { 400 }
fn default_retry_factor() -> f32 { 2.0 }

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: default_retry_attempts(), backoff_ms: default_retry_backoff(), factor: default_retry_factor(), retry_on: Vec::new() }
    }
}

/// Pre-stage hook: external command run before the stage's main action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreHook {
    pub command: String,
    pub script: String,
    #[serde(default)]
    pub cache_dir: String,
}

/// A stage entry of the toolchain registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageSpec {
    pub stage: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lazy: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_for: Vec<String>,
    pub tools: Vec<StageTool>,
}

/// The registry: stages + guards (parsed from source_import_tools.yaml).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toolchain {
    pub stages: Vec<StageSpec>,
    #[serde(default)]
    pub guards: ToolchainGuards,
    /// stage → rust CLI actions (single source; was hardcoded in CLI)
    #[serde(default)]
    pub rust_actions: std::collections::BTreeMap<String, Vec<String>>,
    /// action → retry policy (defaults.retry when absent)
    #[serde(default)]
    pub action_retry: std::collections::BTreeMap<String, RetryPolicy>,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ToolchainGuards {
    #[serde(default)]
    pub grammar_optional_stages: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub lazy_stages: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
    #[error("toolchain registry: YAML parse failed: {0}")]
    Parse(String),
}

impl Toolchain {
    /// Parse the registry (pure; IO belongs to adapters/CLI).
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ToolchainError> {
        serde_yaml::from_str(yaml).map_err(|e| ToolchainError::Parse(e.to_string()))
    }

    pub fn stage(&self, name: &str) -> Option<&StageSpec> {
        self.stages.iter().find(|s| s.stage == name)
    }

    /// Ordered stage names as declared (must cover STAGE_ORDER).
    /// Rust CLI actions for a stage (from the registry, not code).
    pub fn rust_actions(&self, stage: &str) -> Vec<&str> {
        self.rust_actions
            .get(stage)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Retry policy for a CLI action (registry-specific or default).
    pub fn retry_for(&self, action: &str) -> RetryPolicy {
        self.action_retry
            .get(action)
            .cloned()
            .or_else(|| self.defaults.retry.clone())
            .unwrap_or_default()
    }

    pub fn stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.stage.as_str()).collect()
    }
}

/// The import FSM state of one article object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportFsmState {
    /// grammar profile of the object (arxiv_pdf_dot | html_headings | page_outline | …)
    pub grammar_profile: String,
    /// stages already reached, in event order (deduped, first-win).
    pub reached: Vec<String>,
}

impl ImportFsmState {
    /// Derive from manifest lifecycle events (events are the truth).
    /// Stage vocabulary == article lifecycle_order (raw, parsed, …).
    pub fn from_events(grammar_profile: &str, event_stages: &[String]) -> Self {
        let mut reached: Vec<String> = Vec::new();
        for s in event_stages {
            if !reached.contains(s) {
                reached.push(s.clone());
            }
        }
        Self {
            grammar_profile: grammar_profile.to_string(),
            reached,
        }
    }

    /// The furthest pipeline stage reached (by canonical order).
    pub fn current_stage(&self) -> &str {
        let mut best = 0;
        for r in &self.reached {
            if let Some(i) = STAGE_ORDER.iter().position(|s| s == r) && i > best {
                best = i;
            }
        }
        STAGE_ORDER[best]
    }

    /// Object class derived from the grammar profile (the registry's
    /// optional_for/guards speak in classes, manifests in profiles).
    pub fn object_class(&self) -> &'static str {
        match self.grammar_profile.as_str() {
            "html_headings" => "chapter",
            "page_outline" => "monograph",
            _ => "article",
        }
    }

    fn is_optional(&self, stage: &str, tc: &Toolchain) -> bool {
        let class = self.object_class();
        tc.stage(stage)
            .map(|s| {
                s.optional_for.iter().any(|g| g == class)
                    || tc.guards
                        .grammar_optional_stages
                        .get(class)
                        .map(|stages| stages.iter().any(|x| x == stage))
                        .unwrap_or(false)
                    || tc.guards.lazy_stages.iter().any(|l| l == stage)
            })
            .unwrap_or(false)
    }

    /// Implicit implication: any reached stage beyond `raw` proves the
    /// source existed (legacy manifests from the M450 migration record
    /// only completed stages). Never fabricates late stages.
    fn has_reached(&self, stage: &str) -> bool {
        if self.reached.iter().any(|r| r == stage) {
            return true;
        }
        if stage == "raw" {
            return self
                .reached
                .iter()
                .any(|r| STAGE_ORDER.iter().position(|s| s == r).unwrap_or(0) > 0);
        }
        false
    }

    /// The next stage to execute and whether it is required for this
    /// object (optional stages are skippable but still listed).
    pub fn next_stage(&self, tc: &Toolchain) -> Option<(&str, bool)> {
        for stage in &STAGE_ORDER {
            if self.has_reached(stage) {
                continue;
            }
            let optional = self.is_optional(stage, tc);
            return Some((stage, !optional));
        }
        None
    }

    /// Remaining stages until validated, annotated (stage, required?).
    pub fn plan(&self, tc: &Toolchain) -> Vec<(&str, bool)> {
        STAGE_ORDER
            .iter()
            .filter(|s| !self.has_reached(s))
            .map(|s| (*s, !self.is_optional(s, tc)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = "stages:\n  - {stage: raw, tools: []}\n  - {stage: parsed, tools: []}\n  - {stage: structured, tools: []}\n  - {stage: extracted, optional_for: [chapter, monograph], tools: []}\n  - {stage: enriched, tools: []}\n  - {stage: summarized, lazy: true, tools: []}\n  - {stage: validated, tools: []}\nguards:\n  grammar_optional_stages:\n    chapter: [extracted]\n    monograph: [extracted]\n  lazy_stages: [summarized]\n";

    fn tc() -> Toolchain {
        Toolchain::from_yaml_str(REGISTRY).unwrap()
    }

    #[test]
    fn derives_current_stage_from_events() {
        let s = ImportFsmState::from_events(
            "arxiv_pdf_dot",
            &["raw".into(), "parsed".into(), "structured".into(), "extracted".into()],
        );
        assert_eq!(s.current_stage(), "extracted");
        // raw counts as registered-reached in the manifest event stream
        let s2 = ImportFsmState::from_events("arxiv_pdf_dot", &["raw".into()]);
        assert_eq!(s2.current_stage(), "raw");
    }

    #[test]
    fn article_requires_extracted_chapter_skips() {
        let t = tc();
        let art = ImportFsmState::from_events(
            "arxiv_pdf_dot", &["raw".into(), "parsed".into(), "structured".into()],
        );
        assert_eq!(art.next_stage(&t), Some(("extracted", true)));

        let ch = ImportFsmState::from_events("html_headings", &["raw".into(), "parsed".into(), "structured".into()]);
        assert_eq!(ch.next_stage(&t), Some(("extracted", false)));
    }

    #[test]
    fn lazy_stage_is_optional_but_planned() {
        let t = tc();
        let s = ImportFsmState::from_events(
            "arxiv_pdf_dot",
            &["raw".into(), "parsed".into(), "structured".into(), "extracted".into(), "enriched".into()],
        );
        assert_eq!(s.next_stage(&t), Some(("summarized", false)));
        let plan = s.plan(&t);
        assert_eq!(plan, vec![("summarized", false), ("validated", true)]);
    }

    #[test]
    fn legacy_manifest_without_raw_implies_source() {
        let t = tc();
        // M450-migrated articles: first recorded event is `parsed`
        let s = ImportFsmState::from_events(
            "arxiv_pdf_dot",
            &["parsed".into(), "structured".into(), "extracted".into(), "enriched".into()],
        );
        assert_eq!(s.next_stage(&t), Some(("summarized", false)));
    }

    #[test]
    fn complete_object_has_empty_plan() {
        let t = tc();
        let s = ImportFsmState::from_events(
            "arxiv_pdf_dot",
            &["raw".into(), "parsed".into(), "structured".into(), "extracted".into(),
              "enriched".into(), "summarized".into(), "validated".into()],
        );
        assert!(s.next_stage(&t).is_none());
        assert!(s.plan(&t).is_empty());
    }

    #[test]
    fn registry_from_real_file_shape() {
        // structural sanity: stages cover the canonical order
        let t = tc();
        assert_eq!(t.stage_names(), STAGE_ORDER.to_vec());
    }
}
