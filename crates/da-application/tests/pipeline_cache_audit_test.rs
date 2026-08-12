use da_application::pipeline_cache_audit::audit_cache_success;
use da_ports::pipeline_cache::{
    SourceFormat, SourceIdentity, StageCacheKey, StageManifestEntry, StageOutcome,
};
use std::collections::BTreeMap;

fn key(config: &str) -> StageCacheKey {
    StageCacheKey {
        source: SourceIdentity {
            article_id: "x".into(),
            selected_format: SourceFormat::Pdf,
            pdf_sha256: Some("a".repeat(64)),
            html_sha256: None,
            acquisition_fingerprint: "catalog".into(),
            normalization_fingerprint: "norm".into(),
        },
        stage_id: "summary".into(),
        dependency_output_sha256: BTreeMap::from([("parse".into(), "b".repeat(64))]),
        implementation_fingerprint: "impl".into(),
        configuration_fingerprint: config.into(),
        prompt_fingerprint: Some("prompt-v1".into()),
        model_fingerprint: Some("model-v1".into()),
        output_schema_fingerprint: "schema-v1".into(),
    }
}
fn manifest(k: &StageCacheKey, payload: &str) -> StageManifestEntry {
    StageManifestEntry {
        key: k.clone(),
        key_sha256: k.key_sha256().unwrap(),
        recorded_at: "2026-08-12T00:00:00Z".into(),
        outcome: StageOutcome::Success {
            payload_sha256: payload.into(),
            payload_size: 7,
        },
        metadata: BTreeMap::new(),
    }
}
#[test]
fn exact_key_and_payload_are_both_bound() {
    let k = key("a");
    let x = audit_cache_success(&k, &manifest(&k, &"c".repeat(64)), true, false).unwrap();
    assert_eq!(x.key_sha256.as_str(), k.key_sha256().unwrap());
    assert_eq!(x.payload_sha256.as_str(), "c".repeat(64));
    assert!(!x.import_eligible);
}
#[test]
fn identical_output_under_different_config_is_not_same_execution() {
    let a = key("a");
    let b = key("b");
    let payload = "c".repeat(64);
    let x = audit_cache_success(&a, &manifest(&a, &payload), true, false).unwrap();
    let y = audit_cache_success(&b, &manifest(&b, &payload), true, false).unwrap();
    assert_eq!(x.payload_sha256, y.payload_sha256);
    assert_ne!(x.key_sha256, y.key_sha256);
}
#[test]
fn mismatch_failure_and_skipped_materialization_fail_closed() {
    let k = key("a");
    let mut m = manifest(&k, &"c".repeat(64));
    m.key.configuration_fingerprint = "drift".into();
    assert!(audit_cache_success(&k, &m, true, false).is_err());
    assert!(audit_cache_success(&k, &manifest(&k, &"c".repeat(64)), true, true).is_err());
}
