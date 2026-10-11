//! Persist and reload a candidate learned decision table (ADR-074).
//!
//! The table is DATA: a JSON artifact whose content digest is its identity. Load
//! validates the digest, so a truncated or hand-edited file can never silently
//! answer a consult. Nothing here promotes; a reloaded table is still a
//! candidate and its consult carries a [`da_domain::fsm_policy::PolicySource::Learned`]
//! stamp.
//!
//! IO lives here (an application boundary), matching the ADR-074 split where
//! da-domain stays pure and the application owns the bounded use case.

use da_domain::fsm_policy::LearnedDecisionTable;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LearnedArtifactError {
    #[error("learned artifact: io: {0}")]
    Io(String),
    #[error("learned artifact: parse: {0}")]
    Parse(String),
    #[error("learned artifact: invalid table: {0}")]
    Invalid(String),
}

/// Write a sealed table to `path` atomically. The artifact is the table's JSON
/// form; its content digest is `table_sha256`, recomputed on load.
pub fn save_learned_table(
    path: &Path,
    table: &LearnedDecisionTable,
) -> Result<(), LearnedArtifactError> {
    table
        .validate()
        .map_err(|e| LearnedArtifactError::Invalid(e.to_string()))?;
    let bytes = serde_json::to_vec_pretty(table)
        .map_err(|e| LearnedArtifactError::Parse(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| LearnedArtifactError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| LearnedArtifactError::Io(e.to_string()))?;
    Ok(())
}

/// Read and validate a table from `path`. A digest mismatch is refused here, not
/// at consult time, so a corrupt artifact fails loudly and early.
pub fn load_learned_table(path: &Path) -> Result<LearnedDecisionTable, LearnedArtifactError> {
    let bytes = std::fs::read(path).map_err(|e| LearnedArtifactError::Io(e.to_string()))?;
    let table: LearnedDecisionTable = serde_json::from_slice(&bytes)
        .map_err(|e| LearnedArtifactError::Parse(e.to_string()))?;
    table
        .validate()
        .map_err(|e| LearnedArtifactError::Invalid(e.to_string()))?;
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint, feature_bucket};
    use std::collections::BTreeMap;

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: "computer_science".into(),
        }
    }

    fn table() -> LearnedDecisionTable {
        let mut cells = BTreeMap::new();
        cells.insert(
            feature_bucket(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85)),
            "xy_cut".to_string(),
        );
        LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells)
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("table.json");
        let original = table();
        save_learned_table(&path, &original).unwrap();
        let loaded = load_learned_table(&path).unwrap();
        assert_eq!(original, loaded);
        assert_eq!(loaded.grammar_for(&feat(18, 0, 0.85)).unwrap().as_str(), "xy_cut");
    }

    #[test]
    fn refuses_tampered_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("table.json");
        save_learned_table(&path, &table()).unwrap();
        // Corrupt the digest on disk.
        let text = std::fs::read_to_string(&path).unwrap();
        let tampered = text.replace(&table().table_sha256, &"0".repeat(64));
        std::fs::write(&path, tampered).unwrap();
        let err = load_learned_table(&path).unwrap_err();
        assert!(matches!(err, LearnedArtifactError::Invalid(_)));
    }

    #[test]
    fn refuses_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_learned_table(&dir.path().join("absent.json")).unwrap_err();
        assert!(matches!(err, LearnedArtifactError::Io(_)));
    }

    #[test]
    fn full_loop_learn_save_reload_consult() {
        use crate::parse_policy_table::{
            PolicyObservation, TableLearnConfig, learn_decision_table,
        };
        use crate::fsm_policy_fallback::NativeFallbackPolicy;
        use da_domain::fsm_policy::{DecisionPoint, PolicySource};
        use da_ports::fsm_policy::FsmPolicyPort;

        // 1. Observe decisions the pipeline actually took.
        let observations = vec![
            PolicyObservation {
                document_identity: "d1".into(),
                decision_point: DecisionPoint::GrammarRoute,
                features: feat(18, 0, 0.85),
                decision_code: "xy_cut".into(),
                reward: 0.9,
            },
            PolicyObservation {
                document_identity: "d2".into(),
                decision_point: DecisionPoint::GrammarRoute,
                features: feat(20, 0, 0.80),
                decision_code: "xy_cut".into(),
                reward: 0.8,
            },
        ];
        // 2. Learn a sealed candidate table.
        let learned =
            learn_decision_table(DecisionPoint::GrammarRoute, &observations, TableLearnConfig { min_support: 2 })
                .unwrap();
        // 3. Persist and reload it (digest checked on load).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learned.json");
        save_learned_table(&path, &learned).unwrap();
        let reloaded = load_learned_table(&path).unwrap();
        assert_eq!(reloaded.table_sha256, learned.table_sha256);

        // 4. Consult through the fallback policy: covered bucket -> Learned.
        let policy = NativeFallbackPolicy::empty()
            .with_learned_table(reloaded)
            .unwrap();
        let d = policy
            .consult(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert!(matches!(d.id.source, PolicySource::Learned { .. }));
        assert_eq!(d.route.unwrap().as_str(), "xy_cut");

        // 5. An uncovered bucket still fail-opens to native.
        let native = policy
            .consult(DecisionPoint::GrammarRoute, &feat(700, 40, 0.9))
            .unwrap();
        assert_eq!(native.id.source, PolicySource::Native);
    }
}
