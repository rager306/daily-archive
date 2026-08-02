//! Serde roundtrip tests for da-domain types (ADR-051 §4.4 Contour 2).
//!
//! Paper is the primary persisted type; it must survive JSON
//! serialization without loss.

use da_domain::paper::Paper;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn assert_json_roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize must succeed");
    let back: T = serde_json::from_str(&json1).expect("deserialize must succeed");
    let json2 = serde_json::to_string(&back).expect("re-serialize must succeed");
    assert_eq!(
        json1, json2,
        "roundtrip must be lossless (json changed after cycle)"
    );
}

#[test]
fn test_paper_roundtrip_new() {
    let p = Paper::new("2507.19457", "Self-evolving memory");
    assert_json_roundtrip(&p);
}

#[test]
fn test_paper_roundtrip_with_optional_fields() {
    let mut p = Paper::new("2501.00001", "Generalization in RL");
    p.abstract_text = Some("We study generalization...".into());
    p.doi = Some("10.1000/test".into());
    p.pdf_hash = Some("sha256:abc123".into());
    assert_json_roundtrip(&p);
}

#[test]
fn test_paper_roundtrip_superseded() {
    let mut p = Paper::new("2401.00001", "Old version");
    p.superseded_by = Some("2401.00001v2".into());
    p.valid_to = Some(p.valid_from + 86400);
    assert_json_roundtrip(&p);
}
