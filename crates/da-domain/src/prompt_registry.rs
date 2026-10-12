//! Unified prompt/meta-prompt registry value type (ADR-055).
//!
//! Declares a prompt by `(prompt_id, prompt_version)` and binds it to its
//! fingerprint, output schema and provenance. The registry is DATA-declared
//! (`data/ontology/prompt_registry.yaml`); this module performs no I/O and
//! never writes graph truth.
//!
//! Two entry kinds share one resolution surface:
//! - [`PromptEntryKind::Task`] — an inline-template prompt task registered in
//!   `data/ontology/prompt_tasks.yaml`; `schema_ref` is an output-schema id.
//! - [`PromptEntryKind::MetaPrompt`] — a file-based meta-prompt agent; both
//!   `schema_ref` and `source_ref` are repo-relative paths.
//!
//! [`bind_lineage_to_registry`] is the fail-closed refusal: a call record whose
//! recorded `(prompt_id, prompt_version)` does not resolve, or whose template or
//! schema fingerprint disagrees with the resolved entry, returns a typed
//! [`PromptLineageBindingError`] rather than passing silently.

use crate::assurance::ContentDigest;
use crate::prompt_optimization::PromptExecutionLineage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Schema version accepted by [`PromptRegistry::validate`].
pub const PROMPT_REGISTRY_SCHEMA_VERSION: &str = "prompt-registry.v1";

/// The surface a registry entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryKind {
    /// An inline-template prompt task (system + user template pair).
    Task,
    /// A file-based meta-prompt agent (prompt file + agent manifest).
    MetaPrompt,
}

/// One declared prompt, bound to its fingerprint, schema and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRegistryEntry {
    pub prompt_id: String,
    pub prompt_version: String,
    pub kind: PromptEntryKind,
    /// Fingerprint of the prompt template (task) or prompt file (meta-prompt).
    pub prompt_sha256: ContentDigest,
    /// Output-schema id for [`PromptEntryKind::Task`], repo-relative path for
    /// [`PromptEntryKind::MetaPrompt`].
    pub schema_ref: String,
    pub schema_sha256: ContentDigest,
    /// `prompt_tasks.yaml#<task_id>` for [`PromptEntryKind::Task`], repo-relative
    /// prompt file for [`PromptEntryKind::MetaPrompt`].
    pub source_ref: String,
}

/// The unified prompt/meta-prompt registry (DATA-declared, candidate-only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRegistry {
    pub schema_version: String,
    pub registry_id: String,
    pub d127_import_eligible: bool,
    pub entries: Vec<PromptRegistryEntry>,
}

impl PromptRegistry {
    /// Structural validation. Any unlock attempt (setting `d127_import_eligible` to true)
    /// or identity drift is refused.
    pub fn validate(&self) -> Result<(), PromptRegistryError> {
        if self.schema_version != PROMPT_REGISTRY_SCHEMA_VERSION {
            return Err(PromptRegistryError::Invalid(format!(
                "prompt registry schema_version must be {PROMPT_REGISTRY_SCHEMA_VERSION}"
            )));
        }
        if self.registry_id.trim().is_empty() {
            return Err(PromptRegistryError::Invalid(
                "prompt registry id must not be empty".into(),
            ));
        }
        if self.d127_import_eligible {
            return Err(PromptRegistryError::Invalid(
                "prompt registry must keep d127_import_eligible=false".into(),
            ));
        }
        if self.entries.is_empty() {
            return Err(PromptRegistryError::Invalid(
                "prompt registry must declare at least one entry".into(),
            ));
        }
        let mut identities = BTreeSet::new();
        for entry in &self.entries {
            if [
                entry.prompt_id.as_str(),
                entry.prompt_version.as_str(),
                entry.schema_ref.as_str(),
                entry.source_ref.as_str(),
            ]
            .into_iter()
            .any(|value| value.trim().is_empty())
            {
                return Err(PromptRegistryError::Invalid(
                    "prompt registry entry identity is incomplete".into(),
                ));
            }
            if !identities.insert((entry.prompt_id.as_str(), entry.prompt_version.as_str())) {
                return Err(PromptRegistryError::Invalid(format!(
                    "duplicate prompt registry entry {}@{}",
                    entry.prompt_id, entry.prompt_version
                )));
            }
        }
        Ok(())
    }

    /// Resolve an entry by `(prompt_id, prompt_version)`. This is a pure
    /// lookup: an absent pair returns [`PromptRegistryError::Unresolved`].
    pub fn resolve(
        &self,
        prompt_id: &str,
        prompt_version: &str,
    ) -> Result<&PromptRegistryEntry, PromptRegistryError> {
        self.entries
            .iter()
            .find(|entry| entry.prompt_id == prompt_id && entry.prompt_version == prompt_version)
            .ok_or_else(|| PromptRegistryError::Unresolved {
                prompt_id: prompt_id.to_string(),
                prompt_version: prompt_version.to_string(),
            })
    }

    /// Canonical digest over the validated registry serialization.
    pub fn sha256(&self) -> Result<ContentDigest, String> {
        self.validate().map_err(|error| error.to_string())?;
        serde_json::to_vec(self)
            .map(|bytes| ContentDigest::of_bytes(&bytes))
            .map_err(|error| error.to_string())
    }
}

/// Registry resolution failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PromptRegistryError {
    #[error("prompt registry entry {prompt_id}@{prompt_version} is unresolved")]
    Unresolved {
        prompt_id: String,
        prompt_version: String,
    },
    #[error("prompt registry invalid: {0}")]
    Invalid(String),
}

/// Fail-closed refusal produced when a call record disagrees with the registry.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PromptLineageBindingError {
    #[error("prompt lineage {prompt_id}@{prompt_version} is unresolved in the prompt registry")]
    Unresolved {
        prompt_id: String,
        prompt_version: String,
    },
    #[error(
        "prompt lineage version mismatch for {prompt_id}: registry has {registered}, record has {recorded}"
    )]
    VersionMismatch {
        prompt_id: String,
        registered: String,
        recorded: String,
    },
    #[error(
        "prompt lineage fingerprint mismatch for {prompt_id}: registry has {expected}, record has {recorded}"
    )]
    FingerprintMismatch {
        prompt_id: String,
        expected: String,
        recorded: String,
    },
    #[error(
        "prompt lineage schema mismatch for {prompt_id}: registry has {expected}, record has {recorded}"
    )]
    SchemaMismatch {
        prompt_id: String,
        expected: String,
        recorded: String,
    },
}

/// Bind a call record to the registry, fail-closed.
///
/// Resolves `(lineage.prompt_id, lineage.prompt_version)` and requires the
/// recorded template fingerprint to equal the registered one. For
/// [`PromptEntryKind::Task`] entries the recorded output-schema fingerprint
/// must also match the registered schema; meta-prompt entries carry no
/// machine-checked output schema here.
pub fn bind_lineage_to_registry(
    lineage: &PromptExecutionLineage,
    registry: &PromptRegistry,
) -> Result<(), PromptLineageBindingError> {
    let prompt_id = lineage.prompt_id.as_str();
    let prompt_version = lineage.prompt_version.as_str();
    let entry = match registry.resolve(prompt_id, prompt_version) {
        Ok(entry) => entry,
        Err(_) => {
            // `resolve` is a lookup only: any miss means the pair is not
            // registered. Distinguish an id registered at another version from
            // an id the registry never declares.
            return Err(
                match registry
                    .entries
                    .iter()
                    .find(|entry| entry.prompt_id == prompt_id)
                {
                    Some(registered) => PromptLineageBindingError::VersionMismatch {
                        prompt_id: prompt_id.to_string(),
                        registered: registered.prompt_version.clone(),
                        recorded: prompt_version.to_string(),
                    },
                    None => PromptLineageBindingError::Unresolved {
                        prompt_id: prompt_id.to_string(),
                        prompt_version: prompt_version.to_string(),
                    },
                },
            );
        }
    };
    if lineage.prompt_sha256 != entry.prompt_sha256 {
        return Err(PromptLineageBindingError::FingerprintMismatch {
            prompt_id: prompt_id.to_string(),
            expected: entry.prompt_sha256.as_str().to_string(),
            recorded: lineage.prompt_sha256.as_str().to_string(),
        });
    }
    if entry.kind == PromptEntryKind::Task && lineage.output_schema_sha256 != entry.schema_sha256 {
        return Err(PromptLineageBindingError::SchemaMismatch {
            prompt_id: prompt_id.to_string(),
            expected: entry.schema_sha256.as_str().to_string(),
            recorded: lineage.output_schema_sha256.as_str().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        prompt_id: &str,
        prompt_version: &str,
        kind: PromptEntryKind,
        prompt_sha: &str,
        schema_sha: &str,
    ) -> PromptRegistryEntry {
        PromptRegistryEntry {
            prompt_id: prompt_id.into(),
            prompt_version: prompt_version.into(),
            kind,
            prompt_sha256: ContentDigest::of_text(prompt_sha),
            schema_ref: "schema".into(),
            schema_sha256: ContentDigest::of_text(schema_sha),
            source_ref: "source".into(),
        }
    }

    fn registry(entries: Vec<PromptRegistryEntry>) -> PromptRegistry {
        PromptRegistry {
            schema_version: PROMPT_REGISTRY_SCHEMA_VERSION.into(),
            registry_id: "prompt-registry".into(),
            d127_import_eligible: false,
            entries,
        }
    }

    fn task_registry() -> PromptRegistry {
        registry(vec![entry(
            "article_summary",
            "v1",
            PromptEntryKind::Task,
            "template",
            "schema",
        )])
    }

    fn lineage(
        prompt_id: &str,
        prompt_version: &str,
        prompt_sha: &str,
        schema_sha: &str,
    ) -> PromptExecutionLineage {
        PromptExecutionLineage {
            run_id: "run-1".into(),
            prompt_id: prompt_id.into(),
            prompt_version: prompt_version.into(),
            prompt_sha256: ContentDigest::of_text(prompt_sha),
            task_spec_sha256: ContentDigest::of_text("spec"),
            rendered_prompt_sha256: ContentDigest::of_text("rendered"),
            output_schema_sha256: ContentDigest::of_text(schema_sha),
            route_policy_sha256: ContentDigest::of_text("route"),
            endpoint_sha256: ContentDigest::of_text("endpoint"),
            model_binding_sha256: ContentDigest::of_text("model"),
            inference_config_sha256: ContentDigest::of_text("inference"),
            context_bundle_sha256: ContentDigest::of_text("context"),
            provider: "provider".into(),
            model: "model".into(),
            input_sha256: ContentDigest::of_text("input"),
            output_sha256: ContentDigest::of_text("output"),
            input_tokens: 1,
            output_tokens: 1,
            latency_ms: 1,
            temperature_milli: 0,
            max_output_tokens: 16,
            estimated_cost_microunits: None,
            authoritative: false,
            import_eligible: false,
        }
    }

    #[test]
    fn prompt_registry_resolves_and_binds_matching_task_lineage() {
        let registry = task_registry();
        registry.validate().unwrap();
        let resolved = registry.resolve("article_summary", "v1").unwrap();
        assert_eq!(resolved.kind, PromptEntryKind::Task);
        assert_eq!(resolved.prompt_sha256, ContentDigest::of_text("template"));
        bind_lineage_to_registry(
            &lineage("article_summary", "v1", "template", "schema"),
            &registry,
        )
        .unwrap();
    }

    #[test]
    fn prompt_registry_unresolved_miss_is_typed() {
        let registry = task_registry();
        assert_eq!(
            registry.resolve("summary", "v1").unwrap_err(),
            PromptRegistryError::Unresolved {
                prompt_id: "summary".into(),
                prompt_version: "v1".into(),
            }
        );
        assert_eq!(
            bind_lineage_to_registry(&lineage("summary", "v1", "template", "schema"), &registry)
                .unwrap_err(),
            PromptLineageBindingError::Unresolved {
                prompt_id: "summary".into(),
                prompt_version: "v1".into(),
            }
        );
    }

    #[test]
    fn prompt_registry_version_mismatch_is_typed() {
        let registry = task_registry();
        let error = bind_lineage_to_registry(
            &lineage("article_summary", "v2", "template", "schema"),
            &registry,
        )
        .unwrap_err();
        assert_eq!(
            error,
            PromptLineageBindingError::VersionMismatch {
                prompt_id: "article_summary".into(),
                registered: "v1".into(),
                recorded: "v2".into(),
            }
        );
        assert!(error.to_string().contains("article_summary"));
    }

    #[test]
    fn prompt_registry_fingerprint_mismatch_is_typed() {
        let registry = task_registry();
        let error = bind_lineage_to_registry(
            &lineage("article_summary", "v1", "tampered", "schema"),
            &registry,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            PromptLineageBindingError::FingerprintMismatch { .. }
        ));
        assert!(error.to_string().contains("fingerprint mismatch"));
    }

    #[test]
    fn prompt_registry_schema_mismatch_is_typed() {
        let registry = task_registry();
        let error = bind_lineage_to_registry(
            &lineage("article_summary", "v1", "template", "other-schema"),
            &registry,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            PromptLineageBindingError::SchemaMismatch { .. }
        ));
    }

    #[test]
    fn prompt_registry_meta_prompt_binds_without_schema_gate() {
        let registry = registry(vec![entry(
            "critic",
            "1.0.0",
            PromptEntryKind::MetaPrompt,
            "critic-prompt",
            "agent-schema",
        )]);
        // A meta-prompt entry carries no machine-checked output schema, so a
        // schema digest disagreement is not a refusal; the fingerprint still is.
        bind_lineage_to_registry(
            &lineage("critic", "1.0.0", "critic-prompt", "anything"),
            &registry,
        )
        .unwrap();
        assert!(matches!(
            bind_lineage_to_registry(
                &lineage("critic", "1.0.0", "drifted", "anything"),
                &registry
            )
            .unwrap_err(),
            PromptLineageBindingError::FingerprintMismatch { .. }
        ));
    }

    #[test]
    fn prompt_registry_rejects_d127_unlock() {
        let mut registry = task_registry();
        registry.d127_import_eligible = true;
        assert!(matches!(
            registry.validate().unwrap_err(),
            PromptRegistryError::Invalid(message) if message.contains("d127")
        ));
        assert!(registry.sha256().is_err());
    }

    #[test]
    fn prompt_registry_rejects_duplicate_and_empty_entries() {
        let duplicate = registry(vec![
            entry("a", "v1", PromptEntryKind::Task, "t", "s"),
            entry("a", "v1", PromptEntryKind::Task, "t2", "s"),
        ]);
        assert!(duplicate.validate().is_err());
        let mut empty = task_registry();
        empty.entries.clear();
        assert!(empty.validate().is_err());
        let mut blank = task_registry();
        blank.entries[0].prompt_version = "  ".into();
        assert!(blank.validate().is_err());
    }

    #[test]
    fn prompt_registry_rejects_unknown_serde_field() {
        let yaml = r#"
schema_version: prompt-registry.v1
registry_id: prompt-registry
d127_import_eligible: false
entries: []
extra: 1
"#;
        assert!(serde_yaml::from_str::<PromptRegistry>(yaml).is_err());
    }

    #[test]
    fn prompt_lineage_validate_requires_version_and_stays_candidate_only() {
        let mut record = lineage("article_summary", "v1", "template", "schema");
        record.validate().unwrap();
        record.prompt_version = "  ".into();
        assert!(record.validate().is_err());
        let mut unlocked = lineage("article_summary", "v1", "template", "schema");
        unlocked.import_eligible = true;
        assert!(unlocked.validate().is_err());
        let mut authoritative = lineage("article_summary", "v1", "template", "schema");
        authoritative.authoritative = true;
        assert!(authoritative.validate().is_err());
    }

    #[test]
    fn prompt_registry_sha256_is_deterministic_and_sensitive() {
        let registry = task_registry();
        assert_eq!(registry.sha256().unwrap(), registry.sha256().unwrap());
        let mut changed = registry.clone();
        changed.entries[0].prompt_version = "v2".into();
        assert_ne!(registry.sha256().unwrap(), changed.sha256().unwrap());
    }
}
