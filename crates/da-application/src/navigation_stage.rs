//! Navigation stage use case (M451 phase-2): build navigation.json for
//! one article object from its context projection + DATA policies.
//!
//! Returns the artifacts to write; the CLI owns the bounded filesystem
//! write-back (governor/application boundary from memory). The manifest
//! patch and lifecycle event are produced here as data — the caller
//! applies them append-only.

use da_domain::navigation::{
    build_navigation, stats, tree_cost, FurnitureDetectors, NavNode, NavigationStats,
    ProjectionSection,
};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum NavigationStageError {
    #[error("navigation stage: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("navigation stage: projection unreadable for {0}")]
    NoProjection(String),
    #[error("navigation stage: DATA registry unreadable/invalid: {0}")]
    Registry(String),
    #[error("navigation stage: policy unreadable: {0}")]
    Policy(String),
}

/// Result of the stage: everything the CLI needs to persist.
#[derive(Debug, Clone)]
pub struct NavigationStageOutput {
    pub paper_id: String,
    pub nodes: Vec<NavNode>,
    pub navigation_json: String,
    pub stats: NavigationStats,
    pub furniture_share_verdict: &'static str,
    pub tree_cost_verdict: &'static str,
    /// append-only lifecycle event (added when `structured` not reached yet)
    pub new_event: Option<serde_yaml::Mapping>,
}

pub fn build_navigation_stage(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
    detectors_yaml: &str,
    policy_yaml: &str,
) -> Result<NavigationStageOutput, NavigationStageError> {
    let detectors = FurnitureDetectors::from_yaml_str(detectors_yaml)
        .map_err(|e| NavigationStageError::Registry(e.to_string()))?;
    let policy: serde_yaml::Value = serde_yaml::from_str(policy_yaml)
        .map_err(|e| NavigationStageError::Policy(e.to_string()))?;
    let routing = policy
        .get("model")
        .and_then(|m| m.get("routing_cost"))
        .and_then(|v| v.as_u64())
        .unwrap_or(250);

    // projection: read via store (JSON object with `sections`)
    let raw = store
        .read_raw(paper_id, "context/projection.json")?
        .ok_or_else(|| NavigationStageError::NoProjection(paper_id.to_string()))?;
    let proj: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| NavigationStageError::NoProjection(format!("{paper_id}: {e}")))?;
    let sections: Vec<ProjectionSection> = serde_json::from_value(
        proj.get("sections").cloned().unwrap_or_default(),
    )
    .map_err(|e| NavigationStageError::NoProjection(format!("{paper_id}: sections: {e}")))?;

    let nodes = build_navigation(&sections, &detectors);
    let cost = tree_cost(&nodes, routing);
    let st = stats(&nodes, cost);

    // verdicts from policy thresholds (DATA)
    let nav_pol = policy.get("signal").cloned().unwrap_or_default();
    let (warn, alarm) = thresholds_u64(&nav_pol, "warn_above", "alarm_above");
    let tc_verdict = da_domain::navigation::verdict_above(cost, warn, alarm);
    let fs_pol = policy.get("furniture_cleanliness").cloned().unwrap_or_default();
    let (fw, fa) = thresholds_f64(&fs_pol);
    let share = st.furniture_share;
    let fs_verdict = if fa.map(|a| share > a).unwrap_or(false) {
        "alarm"
    } else if fw.map(|w| share > w).unwrap_or(false) {
        "warn"
    } else {
        "ok"
    };

    let navigation_json = serde_json::to_string_pretty(&nodes)
        .map_err(|e| NavigationStageError::NoProjection(format!("{paper_id}: serialize: {e}")))?;

    // lifecycle event: only if structured not yet reached (append-only;
    // the caller decides by inspecting the manifest events)
    let mut event = serde_yaml::Mapping::new();
    event.insert("stage".into(), "structured".into());
    Ok(NavigationStageOutput {
        paper_id: paper_id.to_string(),
        nodes,
        navigation_json,
        stats: st,
        furniture_share_verdict: fs_verdict,
        tree_cost_verdict: tc_verdict,
        new_event: Some(event),
    })
}

fn thresholds_u64(v: &serde_yaml::Value, warn_k: &str, alarm_k: &str) -> (Option<u64>, Option<u64>) {
    (
        v.get(warn_k).and_then(|x| x.as_f64()).map(|x| x as u64),
        v.get(alarm_k).and_then(|x| x.as_f64()).map(|x| x as u64),
    )
}

fn thresholds_f64(v: &serde_yaml::Value) -> (Option<f64>, Option<f64>) {
    (
        v.get("warn_above").and_then(|x| x.as_f64()),
        v.get("alarm_above").and_then(|x| x.as_f64()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct FakeStore {
        files: BTreeMap<(String, String), String>,
    }

    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str {
            "article"
        }
        fn norm(&self) -> Result<da_domain::schema_invariants::ObjectNorm, SchemaObjectStoreError> {
            unimplemented!()
        }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> {
            Ok(vec![])
        }
        fn read_object(
            &self,
            _id: &str,
        ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn read_collection(
            &self,
            _id: &str,
            _rel: &str,
        ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> {
            Ok(true)
        }
        fn file_backed_checks(
            &self,
        ) -> Box<dyn da_domain::schema_invariants::FileBackedChecks + '_> {
            unimplemented!()
        }
        fn read_raw(&self, id: &str, rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(self.files.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    const DETECTORS: &str = "detectors:\n  - {id: under_review, role: furniture, confidence: deterministic, title_pattern: '^Under review\\.?$'}\n";
    const POLICY: &str = "model: {routing_cost: 250}\nsignal: {warn_above: 12000, alarm_above: 30000}\nfurniture_cleanliness: {warn_above: 0.25, alarm_above: 0.5}\n";

    #[test]
    fn stage_builds_navigation_from_projection() {
        let store = FakeStore {
            files: BTreeMap::from([(
                ("p1".into(), "context/projection.json".into()),
                r#"{"paper_id":"p1","sections":[
                    {"section_id":"a","path_id":"1","order":0,"level":1,"title":"Introduction","char_count":100},
                    {"section_id":"b","path_id":"1.1","order":1,"level":2,"title":"Setup","char_count":200},
                    {"section_id":"c","path_id":"2","order":2,"level":1,"title":"Under review.","char_count":10}
                ]}"#
                    .to_string(),
            )]),
        };
        let out = build_navigation_stage(&store, "p1", DETECTORS, POLICY).unwrap();
        assert_eq!(out.nodes.len(), 3);
        assert_eq!(out.stats.furniture, 1);
        assert_eq!(out.stats.nodes, 3);
        assert!(out.navigation_json.contains("\"section_id\": \"a\""));
        assert_eq!(out.tree_cost_verdict, "ok");
        // furniture share 1/3 > 0.25 -> warn
        assert_eq!(out.furniture_share_verdict, "warn");
        assert_eq!(
            out.new_event.as_ref().unwrap().get("stage"),
            Some(&serde_yaml::Value::from("structured"))
        );
    }

    #[test]
    fn missing_projection_is_honest() {
        let store = FakeStore { files: BTreeMap::new() };
        let r = build_navigation_stage(&store, "ghost", DETECTORS, POLICY);
        assert!(matches!(r, Err(NavigationStageError::NoProjection(_))));
    }
}
