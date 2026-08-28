//! Article manifest types and invariants (ADR-068, M451).
//!
//! Article manifest WIRE TYPES (ADR-068, M451).
//!
//! The article manifest is ARTIFACT metadata (lowercase `manifest.yaml`
//! inside `processed/<paper_id>/`), distinct from directory-level
//! MANIFEST.yaml caches. Its shape is DEFINED by
//! `data/schemas/article_schema.yaml` (single definition site).
//!
//! INVARIANTS DO NOT LIVE HERE: validation is the GENERIC engine
//! ([`crate::schema_invariants`]) driven by machine-readable pattern
//! declarations in the object schema — articles are the first object
//! type, not a special case.
//!
//! These typed structs exist for ergonomic downstream use (pipeline
//! stages reading manifests); the validator itself works on
//! `serde_yaml::Value` so any schema-manifest object can reuse it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Wire type of a single extracted entity record (`extracted/entities.json`).
///
/// Only the fields needed by invariants are modeled; unknown fields are
/// preserved by serde's default tolerance and ignored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedEntityWire {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_ordinal: Option<i64>,
}

/// One lifecycle transition of an article. Append-only in storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub stage: String,
    /// transaction_time: when our pipeline recorded the transition.
    pub recorded_at: String,
    /// valid_time: when the fact was true in the external world.
    /// Present only when externally anchored (ADR-068 §2.5.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_sha256: Option<String>,
}

/// A layer entry in the artifact graph: built, missing (typed reason),
/// or not reached by the article lifecycle yet (schema v2 contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArtifactEntry {
    /// Layer materialized on disk.
    Built {
        file: String,
        derived_from: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha256: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        built_at: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stats: Option<serde_yaml::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inputs: Option<Vec<String>>,
    },
    /// Layer absent — with a typed, honest reason.
    Missing {
        present: bool,
        missingness: String,
        derived_from: Vec<String>,
    },
    /// Lifecycle of the article has not arrived at this layer yet.
    NotReached { not_reached: bool },
}

impl ArtifactEntry {
    /// `derived_from` for invariant checks; empty for not-reached entries.
    pub fn derived_from(&self) -> &[String] {
        match self {
            ArtifactEntry::Built { derived_from, .. } => derived_from,
            ArtifactEntry::Missing { derived_from, .. } => derived_from,
            ArtifactEntry::NotReached { .. } => &[],
        }
    }

    /// Relative file path when the layer is built.
    pub fn file(&self) -> Option<&str> {
        match self {
            ArtifactEntry::Built { file, .. } => Some(file),
            _ => None,
        }
    }

    /// Integer stat from the optional stats map (e.g. `ast_nodes`).
    pub fn stat_i64(&self, key: &str) -> Option<i64> {
        let stats = match self {
            ArtifactEntry::Built { stats: Some(v), .. } => v,
            _ => return None,
        };
        stats.get(key).and_then(serde_yaml::Value::as_i64)
    }
}

/// Source locator recorded in the manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleSourceRef {
    #[serde(default)]
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_ref: Option<String>,
}

/// The article artifact manifest (article-manifest.v1, ADR-068).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleManifest {
    pub schema_version: String,
    pub paper_id: String,
    #[serde(default)]
    pub source: Option<ArticleSourceRef>,
    /// DERIVED from the last lifecycle event — validated, never trusted.
    pub lifecycle: String,
    #[serde(default)]
    pub lifecycle_events: Vec<LifecycleEvent>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, ArtifactEntry>,
}

impl ArticleManifest {
    /// The lifecycle stage derived from the last event (or `raw` when
    /// no events exist). This — not the stored field — is the truth.
    pub fn derived_lifecycle(&self) -> &str {
        self.lifecycle_events
            .last()
            .map(|e| e.stage.as_str())
            .unwrap_or("raw")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_entry_untagged_roundtrip() {
        let built = r#"file: equations/index.json
derived_from: [parsed]
stats: {count: 68, grounded: 68}"#;
        let e: ArtifactEntry = serde_yaml::from_str(built).unwrap();
        assert_eq!(e.stat_i64("count"), Some(68));

        let missing = r#"present: false
missingness: no_table_elements
derived_from: [parsed]"#;
        let e: ArtifactEntry = serde_yaml::from_str(missing).unwrap();
        assert_eq!(e.file(), None);

        let nr: ArtifactEntry = serde_yaml::from_str("not_reached: true").unwrap();
        assert!(matches!(nr, ArtifactEntry::NotReached { .. }));
    }

    #[test]
    fn manifest_roundtrip_yaml() {
        let m = ArticleManifest {
            schema_version: "article-manifest.v1".into(),
            paper_id: "2501.00001".into(),
            source: Some(ArticleSourceRef {
                code: "arxiv".into(),
                identity_ref: None,
            }),
            lifecycle: "parsed".into(),
            lifecycle_events: vec![LifecycleEvent {
                stage: "parsed".into(),
                recorded_at: "2026-08-23T01:00:00Z".into(),
                observed_at: None,
                actor: "grobid".into(),
                evidence_sha256: None,
            }],
            artifacts: BTreeMap::new(),
        };
        let s = serde_yaml::to_string(&m).unwrap();
        let back: ArticleManifest = serde_yaml::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn derived_lifecycle_is_last_event() {
        let m = ArticleManifest {
            schema_version: "v".into(),
            paper_id: "p".into(),
            source: None,
            lifecycle: "unused".into(),
            lifecycle_events: vec![
                LifecycleEvent {
                    stage: "parsed".into(),
                    recorded_at: "1".into(),
                    observed_at: None,
                    actor: "a".into(),
                    evidence_sha256: None,
                },
                LifecycleEvent {
                    stage: "extracted".into(),
                    recorded_at: "2".into(),
                    observed_at: None,
                    actor: "a".into(),
                    evidence_sha256: None,
                },
            ],
            artifacts: BTreeMap::new(),
        };
        assert_eq!(m.derived_lifecycle(), "extracted");
    }
}
