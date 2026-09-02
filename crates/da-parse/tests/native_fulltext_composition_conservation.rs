//! Downstream conservation checks for the frozen S18 candidate.
//!
//! The expected source inventory is built from the fixture's lexical XML
//! facts, while TeiLifter and ConservationEngine are used only as downstream
//! verifiers.  No composer output is used as an oracle.

use da_parse::{
    ConservationEngine, ConservationFamily, ConservationReasonCode, ConservationVerdict,
    TeiEnvelope, TeiIdentity, TeiLifter,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/native_fulltext_composition_oracle.yaml");

#[derive(Debug, Deserialize)]
struct Fixture {
    source_sha256: String,
    source_xml: String,
    profile_fingerprint: String,
}
fn fixture() -> Fixture {
    serde_yaml::from_str(FIXTURE).expect("fixture parses")
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn envelope(f: &Fixture, xml: &str) -> TeiEnvelope {
    TeiEnvelope::new(
        xml.as_bytes(),
        TeiIdentity::new("grobid-http", &f.profile_fingerprint),
    )
}

#[test]
fn frozen_source_and_lifted_candidate_have_explicit_family_conservation() {
    let f = fixture();
    assert_eq!(
        digest(f.source_xml.as_bytes()),
        format!("sha256:{}", f.source_sha256)
    );
    let source = envelope(&f, &f.source_xml);
    let ir = TeiLifter::new().lift(&source, "oracle-paper", f.source_sha256.clone());
    let engine = ConservationEngine::new();
    let inventory = engine.source_inventory(&source);
    assert_eq!(inventory.count(ConservationFamily::Title), 1);
    assert_eq!(inventory.count(ConservationFamily::Section), 2);
    assert_eq!(inventory.count(ConservationFamily::Figure), 1);
    assert_eq!(inventory.count(ConservationFamily::Coordinate), 1);
    assert_eq!(inventory.count(ConservationFamily::CitationEntry), 1);
    assert_eq!(inventory.count(ConservationFamily::CrossReference), 1);
    assert_eq!(inventory.total(), 7);
    let delta = engine.compare(&source, &ir);
    assert_eq!(delta.identity.raw_tei_sha256, f.source_sha256);
    assert!(matches!(
        delta.verdict,
        ConservationVerdict::Conserved | ConservationVerdict::LossDetected
    ));
    assert!(delta.entries.iter().all(|entry| {
        entry
            .byte_range
            .is_none_or(|range| range.is_valid(f.source_xml.len()))
    }));
}

#[test]
fn replay_is_byte_stable_and_missing_bibliography_is_typed() {
    let f = fixture();
    let source = envelope(&f, &f.source_xml);
    let engine = ConservationEngine::new();
    let ir = TeiLifter::new().lift(&source, "oracle-paper", f.source_sha256.clone());
    let first = engine.compare(&source, &ir);
    let replay = engine.compare(&source, &ir);
    assert_eq!(first, replay);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&replay).unwrap()
    );
    assert_eq!(first.delta_digest, replay.delta_digest);

    let removed = envelope(&f, &f.source_xml);
    let mut removed_ir = TeiLifter::new().lift(&removed, "oracle-paper", f.source_sha256.clone());
    removed_ir.citations.clear();
    let delta = engine.compare(&removed, &removed_ir);
    assert!(delta.entries.iter().any(|entry| entry.reason
        == ConservationReasonCode::CountMismatch
        || entry.reason == ConservationReasonCode::ObservedButMissing));
}

#[test]
fn reordered_sections_and_damaged_coordinate_are_visible_not_silent() {
    let f = fixture();
    let engine = ConservationEngine::new();
    let reordered_xml = f.source_xml.replace("First section</head><p>body bytes <ref type=\"figure\" target=\"#fig-1\">Fig. 1</ref></p></div><div><head>Second section", "Second section</head><p>body bytes <ref type=\"figure\" target=\"#fig-1\">Fig. 1</ref></p></div><div><head>First section");
    let reordered = envelope(&f, &reordered_xml);
    let reordered_ir = TeiLifter::new().lift(&reordered, "oracle-paper", "reordered");
    let reordered_delta = engine.compare(&reordered, &reordered_ir);
    assert!(
        reordered_delta
            .entries
            .iter()
            .any(|entry| entry.reason == ConservationReasonCode::Reordered
                || entry.reason == ConservationReasonCode::ObservedButMissing)
    );

    let damaged_xml = f
        .source_xml
        .replace("coords=\"1,2,3,4\"", "coords=\"9,9,9,9\"");
    let damaged = envelope(&f, &damaged_xml);
    let damaged_ir = TeiLifter::new().lift(&damaged, "oracle-paper", "damaged");
    let damaged_delta = engine.compare(&damaged, &damaged_ir);
    assert!(damaged_delta.entries.iter().any(|entry| entry.reason
        == ConservationReasonCode::CoordinateDropped
        || entry.reason == ConservationReasonCode::ObservedButMissing));
}
