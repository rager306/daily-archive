//! Closed contract for the offline Java P5/P2 + native body shadow join.

use da_parse::{
    JavaFallbackEvidence, JavaP2Observation, JavaP5Observation, NATIVE_BODY_SCHEMA_VERSION,
    NATIVE_BODY_SEQUENCE_SCHEMA_VERSION, NATIVE_SEGMENTATION_SCHEMA_VERSION,
    NativeBodySequenceCounts, NativeBodySequenceIdentity, NativeBodySequenceReceipt,
    P5ShadowJoinError, P5ShadowJoinIdentity, P5ShadowJoinInput, join_p5_shadow,
};
use std::collections::BTreeMap;

fn input<'a>(native: Option<&'a NativeBodySequenceReceipt>) -> P5ShadowJoinInput<'a> {
    P5ShadowJoinInput {
        identity: P5ShadowJoinIdentity {
            document_id: "contract-document".into(),
            source_sha256: "a".repeat(64),
            grobid_profile: "structural".into(),
            provider_id: "provider-contract".into(),
            segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
            accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
            boundary_digest: "b".repeat(64),
        },
        java_p5: JavaP5Observation {
            paper_id: "contract-document".into(),
            grobid_profile: "structural".into(),
            production_authority: "java_http".into(),
            observe_mode: "production_stamp".into(),
            sections_len: 0,
            body_empty: true,
            empty_count_not_success: true,
            native_surface: "none".into(),
        },
        java_p2: Some(JavaP2Observation {
            paper_id: "contract-document".into(),
            grobid_profile: "structural".into(),
            java_section_n: 0,
        }),
        native_body: native,
    }
}

fn receipt() -> NativeBodySequenceReceipt {
    NativeBodySequenceReceipt {
        schema_version: NATIVE_BODY_SEQUENCE_SCHEMA_VERSION.into(),
        status: "pass".into(),
        identity: NativeBodySequenceIdentity {
            document_id: "contract-document".into(),
            source_sha256: "a".repeat(64),
            provider_id: "provider-contract".into(),
            segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
            accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
            boundary_digest: "b".repeat(64),
        },
        steps: Vec::new(),
        counts: NativeBodySequenceCounts {
            admitted: 2,
            conserved: 2,
            unsupported: 0,
            loss: 0,
        },
        java_fallback: JavaFallbackEvidence {
            disposition: "preserved_evidence_only".into(),
            target: "java_grobid".into(),
            source: "production_fallback".into(),
        },
        redaction: BTreeMap::from([(String::from("raw_text"), false)]),
    }
}

#[test]
fn matching_triple_joins_empty_java_observation_without_calling_accountant() {
    let native = receipt();
    let report = join_p5_shadow(input(Some(&native))).expect("matching identities join");
    assert!(report.identity_match);
    assert!(report.java_p5.body_empty);
    assert_eq!(report.java_p5.sections_len, 0);
    assert_eq!(report.java_p2.java_section_n, 0);
    assert_eq!(report.native_status, "pass");
    assert_eq!(report.native_counts.admitted, 2);
    assert!(report.empty_count_not_success);
    assert_eq!(report.java_fallback.target, "java_grobid");
    assert_eq!(report.redaction.get("body_text"), Some(&false));
    assert_eq!(report.redaction.get("pdf_bytes"), Some(&false));
    let wire = serde_json::to_string(&report).unwrap();
    for forbidden_payload in ["raw body payload", "<tei>", "%PDF-"] {
        assert!(
            !wire.contains(forbidden_payload),
            "report leaked payload marker {forbidden_payload}"
        );
    }
    assert!(!wire.contains("p5_success"));
    assert!(!wire.contains("java_section_n_on_p5"));
}

#[test]
fn every_required_leg_and_identity_family_fails_closed() {
    assert_eq!(
        join_p5_shadow(input(None)),
        Err(P5ShadowJoinError::MissingRequiredLeg {
            leg: "native_body_sequence"
        })
    );
    let native = receipt();
    macro_rules! identity_mutation {
        ($name:literal, $field:ident, $value:expr, $expected:expr) => {{
            let mut candidate = input(Some(&native));
            candidate.identity.$field = $value;
            assert_eq!(
                join_p5_shadow(candidate),
                Err($expected),
                "{} drift must fail",
                $name
            );
        }};
    }
    identity_mutation!(
        "document",
        document_id,
        "other".into(),
        P5ShadowJoinError::DocumentMismatch
    );
    identity_mutation!(
        "source",
        source_sha256,
        "c".repeat(64),
        P5ShadowJoinError::SourceDigestMismatch
    );
    identity_mutation!(
        "profile",
        grobid_profile,
        "rich".into(),
        P5ShadowJoinError::ProfileMismatch
    );
    identity_mutation!(
        "provider",
        provider_id,
        "other".into(),
        P5ShadowJoinError::ProviderMismatch
    );
    identity_mutation!(
        "boundary",
        boundary_digest,
        "c".repeat(64),
        P5ShadowJoinError::BoundaryDigestMismatch
    );
}

#[test]
fn malformed_identity_and_authority_mutations_fail_closed() {
    let native = receipt();
    let mut blank = input(Some(&native));
    blank.identity.document_id.clear();
    assert_eq!(
        join_p5_shadow(blank),
        Err(P5ShadowJoinError::MissingIdentity {
            field: "document_id"
        })
    );
    let mut malformed = input(Some(&native));
    malformed.identity.source_sha256 = "not-a-digest".into();
    assert_eq!(
        join_p5_shadow(malformed),
        Err(P5ShadowJoinError::MalformedIdentity {
            field: "source_sha256"
        })
    );
    let mut authority = input(Some(&native));
    authority.java_p5.production_authority = "native".into();
    assert_eq!(
        join_p5_shadow(authority),
        Err(P5ShadowJoinError::AuthorityDrift)
    );
    let mut mode = input(Some(&native));
    mode.java_p5.observe_mode = "shadow_receipt".into();
    assert_eq!(
        join_p5_shadow(mode),
        Err(P5ShadowJoinError::ObserveModeDrift)
    );
    let mut empty = input(Some(&native));
    empty.java_p5.empty_count_not_success = false;
    assert_eq!(
        join_p5_shadow(empty),
        Err(P5ShadowJoinError::EmptyCountTreatedAsSuccess)
    );
    let mut surface = input(Some(&native));
    surface.java_p5.native_surface = "native".into();
    assert_eq!(
        join_p5_shadow(surface),
        Err(P5ShadowJoinError::NativeSurfaceLive)
    );
}

#[test]
fn native_schema_and_receipt_status_are_not_silently_coerced() {
    let mut native = receipt();
    native.schema_version = "other/v1".into();
    assert!(matches!(
        join_p5_shadow(input(Some(&native))),
        Err(P5ShadowJoinError::SchemaMismatch { .. })
    ));
    let mut native = receipt();
    native.status.clear();
    assert_eq!(
        join_p5_shadow(input(Some(&native))),
        Err(P5ShadowJoinError::NativeReceiptNotPassing)
    );
}
