//! Integration test: the learned-table opt-in gate (ADR-078 R144/R145,
//! ADR-051 Contour 5/6).
//!
//! `DA_LEARNED_POLICY_TABLE` is the ONLY way a learned table reaches a consult.
//! Unset, missing, or invalid it must fail open to native; set and valid it must
//! answer covered buckets with a `Learned` stamp and leave uncovered ones to
//! native. This test drives the real consult helpers, not a re-implementation.

use da_cli::parse_profile::resolve_parse_profile_with_learned;
use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, LearnedDecisionTable, PolicySource, ProfileRoute,
    feature_bucket,
};
use da_ports::fsm_policy::FsmPolicyPort;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes every test in this binary that mutates process env or cwd.
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}
impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: ENV_LOCK serializes every env-mutating test in this binary.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
    fn clear(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

const PROFILE_YAML: &str = r#"
decision_points:
  profile_route:
    native:
      default: structural
"#;

fn features() -> ArticleFeatures {
    ArticleFeatures {
        page_count: 18,
        toc_bookmarks: 0,
        text_layer_quality: 0.85,
        domain: "computer_science".into(),
    }
}

fn rich_table() -> LearnedDecisionTable {
    // Learn the exact bucket -> rich, so the answer differs from the native
    // structural default and the test can tell who answered.
    let mut cells = BTreeMap::new();
    cells.insert(
        feature_bucket(DecisionPoint::ProfileRoute, &features()),
        "rich".to_string(),
    );
    LearnedDecisionTable::seal(DecisionPoint::ProfileRoute, cells)
}

fn write_table(dir: &std::path::Path, name: &str, table: &LearnedDecisionTable) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_vec_pretty(table).unwrap()).unwrap();
    path
}

#[test]
fn no_env_answers_native() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _guard = EnvGuard::clear("DA_LEARNED_POLICY_TABLE");
    let decision =
        resolve_parse_profile_with_learned(PROFILE_YAML, None, &features(), None).unwrap();
    assert_eq!(decision.source, PolicySource::Native);
    assert_eq!(decision.profile, ProfileRoute::Structural);
}

#[test]
fn valid_env_answers_learned_on_a_covered_bucket() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_table(dir.path(), "t.json", &rich_table());
    let loaded = da_application::parse_policy_artifact::load_learned_table(&path).unwrap();
    let decision =
        resolve_parse_profile_with_learned(PROFILE_YAML, None, &features(), Some(loaded)).unwrap();
    assert!(matches!(decision.source, PolicySource::Learned { .. }));
    assert_eq!(
        decision.profile,
        ProfileRoute::Rich,
        "the learned code answered"
    );
}

#[test]
fn uncovered_bucket_falls_open_to_native() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_table(dir.path(), "t.json", &rich_table());
    let loaded = da_application::parse_policy_artifact::load_learned_table(&path).unwrap();
    // A different quality band is not covered -> native structural answers.
    let other = ArticleFeatures {
        page_count: 18,
        toc_bookmarks: 0,
        text_layer_quality: 0.10,
        domain: "computer_science".into(),
    };
    let decision =
        resolve_parse_profile_with_learned(PROFILE_YAML, None, &other, Some(loaded)).unwrap();
    assert_eq!(decision.source, PolicySource::Native);
    assert_eq!(decision.profile, ProfileRoute::Structural);
}

#[test]
fn invalid_artifact_is_refused_at_load() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    // A tampered digest must fail to load, so the consult never sees it.
    let table = rich_table();
    let mut value = serde_json::to_value(&table).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("table_sha256".into(), serde_json::json!("0".repeat(64)));
    let path = dir.path().join("bad.json");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(da_application::parse_policy_artifact::load_learned_table(&path).is_err());
}

#[test]
fn wasm_host_takes_precedence_over_the_learned_table() {
    let _lock = ENV_LOCK.lock().unwrap();
    // A host that answers profile_route with a distinct source must win.
    struct AlwaysWasm;
    impl FsmPolicyPort for AlwaysWasm {
        fn consult(
            &self,
            _point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> da_ports::fsm_policy::FsmPolicyResult<da_domain::fsm_policy::PolicyDecision> {
            Ok(da_domain::fsm_policy::PolicyDecision::profile(
                PolicySource::Wasm {
                    sha256: "deadbeef".into(),
                },
                ProfileRoute::Structural,
            ))
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = write_table(dir.path(), "t.json", &rich_table());
    let loaded = da_application::parse_policy_artifact::load_learned_table(&path).unwrap();
    let host = AlwaysWasm;
    let decision =
        resolve_parse_profile_with_learned(PROFILE_YAML, Some(&host), &features(), Some(loaded))
            .unwrap();
    assert!(
        matches!(decision.source, PolicySource::Wasm { .. }),
        "the WASM host must be consulted before the learned table (R145)"
    );
}
