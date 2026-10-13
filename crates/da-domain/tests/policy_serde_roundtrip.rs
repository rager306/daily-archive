//! Serde roundtrip tests for the policy types (ADR-051 §4.4 Contour 2).
//!
//! Every public Serialize/Deserialize type introduced by ADR-078/ADR-079 must
//! survive a JSON cycle without loss and keep its `deny_unknown_fields`
//! refusal of foreign keys.

use da_domain::fsm_policy::{
    Admission, ArticleFeatures, BatchAdmissionNative, BucketLevel, DecisionPoint, GrammarRoute,
    GrammarRouteNative, LearnedDecisionTable, PolicyDecision, PolicyId, PolicyRegistry,
    PolicySource, ProfileRoute, ProfileRouteNative, TableCell, feature_bucket,
    feature_bucket_at_level,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

fn assert_json_roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize must succeed");
    let back: T = serde_json::from_str(&json1).expect("deserialize must succeed");
    let json2 = serde_json::to_string(&back).expect("re-serialize must succeed");
    assert_eq!(json1, json2, "roundtrip must be lossless");
}

fn features() -> ArticleFeatures {
    ArticleFeatures {
        page_count: 18,
        toc_bookmarks: 0,
        text_layer_quality: 0.85,
        domain: "computer_science".into(),
    }
}

#[test]
fn article_features_roundtrip() {
    assert_json_roundtrip(&features());
}

#[test]
fn decision_point_roundtrip_all_variants() {
    for point in [
        DecisionPoint::GrammarRoute,
        DecisionPoint::BatchAdmission,
        DecisionPoint::ConcurrencyTune,
        DecisionPoint::RetryPolicy,
        DecisionPoint::SummaryDepth,
        DecisionPoint::ProfileRoute,
    ] {
        assert_json_roundtrip(&point);
        assert_eq!(DecisionPoint::parse(point.as_str()), Some(point));
    }
}

#[test]
fn closed_enums_roundtrip() {
    for r in [
        GrammarRoute::PageOutline,
        GrammarRoute::XyCut,
        GrammarRoute::GrobidStructural,
    ] {
        assert_json_roundtrip(&r);
    }
    for p in [ProfileRoute::Structural, ProfileRoute::Rich] {
        assert_json_roundtrip(&p);
    }
    for a in [Admission::Accept, Admission::Defer, Admission::Skip] {
        assert_json_roundtrip(&a);
    }
    for l in BucketLevel::CHAIN {
        assert_json_roundtrip(&l);
    }
}

#[test]
fn policy_source_roundtrip_all_variants() {
    assert_json_roundtrip(&PolicySource::Native);
    assert_json_roundtrip(&PolicySource::Wasm {
        sha256: "abc".into(),
    });
    assert_json_roundtrip(&PolicySource::Learned {
        table_sha256: "0".repeat(64),
    });
}

#[test]
fn policy_decision_and_id_roundtrip() {
    let d = PolicyDecision::grammar(
        PolicySource::Learned {
            table_sha256: "1".repeat(64),
        },
        GrammarRoute::XyCut,
    );
    assert_json_roundtrip(&d);
    assert_json_roundtrip(&d.id);
    let id = PolicyId {
        decision_point: "grammar_route".into(),
        source: PolicySource::Native,
    };
    assert_json_roundtrip(&id);
}

#[test]
fn native_threshold_structs_roundtrip() {
    assert_json_roundtrip(&GrammarRouteNative::default());
    assert_json_roundtrip(&BatchAdmissionNative::default());
    assert_json_roundtrip(&ProfileRouteNative::default());
}

#[test]
fn registry_roundtrips_via_yaml_and_json() {
    let yaml = "schema_version: v2\nadr_ref: ADR-074\ndecision_points: {}\n";
    let reg = PolicyRegistry::from_yaml_str(yaml).expect("yaml parses");
    assert_json_roundtrip(&reg);
}

#[test]
fn learned_table_and_cell_roundtrip() {
    let mut cells = BTreeMap::new();
    let key = feature_bucket(DecisionPoint::GrammarRoute, &features());
    cells.insert(key.clone(), "xy_cut".to_string());
    let mut support = BTreeMap::new();
    support.insert(key, 7u32);
    let table =
        LearnedDecisionTable::seal_with_support(DecisionPoint::GrammarRoute, cells, support);
    assert_json_roundtrip(&table);
    let cell = table.lookup_with_fallback(&features()).expect("covered");
    assert_json_roundtrip(&cell);
    let _ = TableCell {
        code: "xy_cut".into(),
        bucket_key_used: feature_bucket_at_level(DecisionPoint::GrammarRoute, &features(), 0),
        support: Some(1),
        level: BucketLevel::Exact,
    };
}

#[test]
fn deny_unknown_fields_is_enforced_on_policy_types() {
    // A foreign key on the table must be refused, not silently ignored.
    let mut cells = BTreeMap::new();
    cells.insert(
        feature_bucket(DecisionPoint::GrammarRoute, &features()),
        "xy_cut".to_string(),
    );
    let table = LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells);
    let mut value = serde_json::to_value(&table).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("invented".into(), serde_json::json!(1));
    assert!(
        serde_json::from_value::<LearnedDecisionTable>(value).is_err(),
        "deny_unknown_fields must refuse a foreign key"
    );

    // Same discipline on ArticleFeatures.
    let mut f = serde_json::to_value(features()).unwrap();
    f.as_object_mut()
        .unwrap()
        .insert("rogue".into(), serde_json::json!("x"));
    assert!(serde_json::from_value::<ArticleFeatures>(f).is_err());
}
