//! Independent consumer for the closed native-family review digest.
//!
//! This test imports the standalone example as a module: it never registers the
//! example in the library and never launches Cargo or another process.

#[path = "../examples/native_family_gate_rehearsal.rs"]
mod rehearsal;

use rehearsal::{
    FamilyDigest, PooledWinnerDigest, ReviewDigest, ReviewGate, ReviewOutcome, build_digest,
    render_digest,
};
use serde_json::{Value, json};

fn digest_json() -> String {
    render_digest(&build_digest()).expect("producer renders closed digest")
}

fn family<'a>(digest: &'a ReviewDigest, name: &str) -> &'a FamilyDigest {
    digest
        .families
        .iter()
        .find(|row| row.family == name)
        .unwrap_or_else(|| panic!("missing family {name}"))
}

#[test]
fn accepts_exact_digest_and_replays_byte_identically() {
    let first = digest_json();
    let second = digest_json();
    assert_eq!(first, second, "in-memory rendering must be deterministic");

    let digest: ReviewDigest = serde_json::from_str(&first).expect("closed digest schema");
    assert_eq!(digest.schema_version, "native-family-review-digest.v1");
    assert!(!digest.authority.deployed);
    assert!(!digest.authority.import_eligible);
    assert!(!digest.authority.can_auto_promote);
    assert_eq!(digest.authority.this_wave_promotes, None);
    assert_eq!(
        digest.authority.java_rollback_target,
        "java-grobid-http@0.9.0-crf"
    );
    assert_eq!(
        digest.authority.pooled_winner,
        PooledWinnerDigest::Forbidden
    );
    assert_eq!(
        digest.authority.independently_selectable,
        ["header", "references"]
    );
    assert!(digest.authority.decision_point_consults.is_empty());
    assert!(digest.authority.java_production_unchanged);
    assert_eq!(
        (
            digest.composition.native_nodes,
            digest.composition.java_nodes
        ),
        (20, 21)
    );
    assert_eq!(
        (
            digest.composition.citation_names_total,
            digest.composition.references_total
        ),
        (2198, 207)
    );

    let expected = [
        (
            "segmentation",
            ReviewOutcome::Incomplete,
            ReviewGate::NotApplicable,
            false,
            "segmentation_not_independently_selectable",
        ),
        (
            "header",
            ReviewOutcome::ReadyReview,
            ReviewGate::HumanGateRequired,
            true,
            "hub_native_accepted_header_vs_java_raw",
        ),
        (
            "bibliography",
            ReviewOutcome::ReadyReview,
            ReviewGate::HumanGateRequired,
            true,
            "citation_names_not_biblstruct",
        ),
        (
            "body",
            ReviewOutcome::ReadyReview,
            ReviewGate::HumanGateRequired,
            true,
            "body_shadow_java_fallback",
        ),
        (
            "visual",
            ReviewOutcome::ReadyReview,
            ReviewGate::HumanGateRequired,
            true,
            "visual_unpooled_heads",
        ),
    ];
    assert_eq!(digest.families.len(), expected.len());
    for (row, (name, outcome, gate, binding, gap)) in digest.families.iter().zip(expected) {
        assert_eq!(row.family, name);
        assert_eq!(row.outcome, outcome);
        assert_eq!(row.gate, gate);
        assert_eq!(row.binding_present, binding);
        assert_eq!(row.visible_gap_ids, [gap]);
    }

    let ids: Vec<&str> = digest
        .blocked_rehearsals
        .iter()
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "S05-R01", "S05-R02", "S05-R03", "S05-R04", "S05-R05", "S05-R06"
        ]
    );
    let reasons_fields: Vec<(&str, String, &str)> = digest
        .blocked_rehearsals
        .iter()
        .map(|row| {
            (
                row.id.as_str(),
                serde_json::to_value(row.reason)
                    .expect("typed block reason serializes")
                    .as_str()
                    .expect("block reason uses a string wire value")
                    .to_owned(),
                row.field.as_str(),
            )
        })
        .collect();
    assert_eq!(
        reasons_fields,
        vec![
            ("S05-R01", "pooling".to_owned(), "pooled_winner"),
            (
                "S05-R02",
                "attempted_automatic_promotion".to_owned(),
                "admission"
            ),
            ("S05-R03", "named_gap_hidden".to_owned(), "named_gaps"),
            (
                "S05-R04",
                "unsafe_production_call".to_owned(),
                "Header.integrity"
            ),
            (
                "S05-R05",
                "attempted_automatic_promotion".to_owned(),
                "Visual.integrity"
            ),
            ("S05-R06", "malformed_package".to_owned(), "families"),
        ]
    );
    assert!(digest.blocked_rehearsals.iter().all(|row| {
        let object = serde_json::to_value(row).expect("blocked row JSON");
        object.as_object().is_some_and(|map| map.len() == 3)
    }));
}

#[test]
fn rejects_unknown_fields_and_privilege_vocabulary() {
    let mut unknown: Value = serde_json::from_str(&digest_json()).unwrap();
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<ReviewDigest>(unknown).is_err());

    for (path, value) in [
        ("authority.this_wave_promotes", json!("promoted")),
        ("authority.pooled_winner", json!("header")),
    ] {
        let mut tampered: Value = serde_json::from_str(&digest_json()).unwrap();
        let mut cursor = &mut tampered;
        for part in path.split('.').take(1) {
            cursor = &mut cursor[part];
        }
        cursor[path.rsplit('.').next().unwrap()] = value;
        assert!(
            serde_json::from_value::<ReviewDigest>(tampered).is_err(),
            "tampered {path} must not deserialize"
        );
    }

    let mut human_yes: Value = serde_json::from_str(&digest_json()).unwrap();
    human_yes["families"][1]["gate"] = json!("human_yes");
    assert!(serde_json::from_value::<ReviewDigest>(human_yes).is_err());
}

#[test]
fn digest_is_redacted_and_contains_no_hidden_payloads() {
    let text = digest_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    let forbidden = [
        "<TEI",
        "<tei",
        ".pdf",
        "pdf_path",
        "source_path",
        "source text",
        "graph id",
        "graph_id",
        "gap summary",
        "synthetic bounded reviewer gap",
    ];
    for marker in forbidden {
        assert!(
            !text
                .to_ascii_lowercase()
                .contains(&marker.to_ascii_lowercase()),
            "leaked {marker}"
        );
    }
    assert!(
        value["blocked_rehearsals"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row
                .as_object()
                .unwrap()
                .keys()
                .eq(["field", "id", "reason"]))
    );
    assert_eq!(
        family(&serde_json::from_str(&text).unwrap(), "header")
            .visible_gap_ids
            .len(),
        1
    );
}
