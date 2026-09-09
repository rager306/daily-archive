use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn position(token: u32, annex_parent: Option<usize>) -> CalloutPosition {
    CalloutPosition {
        page_ordinal: 0,
        block_ordinal: 0,
        line_ordinal: 0,
        token_ordinal: token,
        annex_parent,
    }
}
fn inventory(
    document_id: &str,
    targets: Vec<DigestOnlyCalloutTarget>,
) -> DigestOnlyTargetInventory {
    let mut value = DigestOnlyTargetInventory {
        document_id: document_id.into(),
        lane_id: "fixture-lane".into(),
        source_sha256: "b".repeat(64),
        inventory_digest: String::new(),
        targets,
    };
    value.inventory_digest = inventory_digest(&value);
    value
}
fn accounting() -> NativeFullRegionAccounting {
    NativeFullRegionAccounting {
        schema_version: NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION.into(),
        document_id: "doc".into(),
        provider_id: "provider".into(),
        source_pdf_sha256: "b".repeat(64),
        lattice_fingerprint: "sha256:lattice".into(),
        segmentation_config_fingerprint: "sha256:config".into(),
        admitted: 1,
        conserved: 1,
        loss: 0,
        visual_evidence: vec![],
        lexical_units: vec![],
    }
}
fn blocks(
    subtype: MarkerSubtype,
    token: u32,
    annex_parent: Option<usize>,
) -> NativeFulltextBlocksOutcome {
    NativeFulltextBlocksOutcome {
        schema_version: NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION.into(),
        document_id: "doc".into(),
        lattice_fingerprint: "sha256:lattice".into(),
        config_fingerprint: "sha256:config".into(),
        annex_runs: vec![],
        leaves: vec![FulltextLeafAssignment {
            page_ordinal: 0,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: token,
            source_ordinal: token,
            text_digest: digest("marker"),
            role: FulltextLeafRole::Marker,
            marker_subtype: Some(subtype),
            parent_annex: annex_parent,
        }],
        unsupported: 0,
    }
}
fn target(
    class: CalloutTargetClass,
    token: u32,
    style: ObservedCalloutStyle,
    annex_parent: Option<usize>,
) -> DigestOnlyCalloutTarget {
    DigestOnlyCalloutTarget {
        class,
        target_digest: digest("marker"),
        position: position(token, annex_parent),
        style,
    }
}

#[test]
fn exact_match_is_conserved_and_digest_only() {
    let result = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Citation, 1, None),
        NativeCalloutUpstreamStatus::Pass,
        &inventory(
            "doc",
            vec![target(
                CalloutTargetClass::Citation,
                2,
                ObservedCalloutStyle::Numeric,
                None,
            )],
        ),
    )
    .unwrap();
    assert_eq!(result.matched.len(), 1);
    assert_eq!((result.admitted, result.conserved, result.loss), (1, 1, 0));
    let encoded = serde_json::to_string(&result).unwrap();
    assert!(encoded.contains("sha256"));
    assert!(!encoded.contains("\"text\"") && !encoded.contains("\"raw_text\""));
}

#[test]
fn typed_unresolved_and_equation_are_fail_closed() {
    let no_target = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Figure, 1, None),
        NativeCalloutUpstreamStatus::Pass,
        &inventory("doc", vec![]),
    )
    .unwrap();
    assert_eq!(
        no_target.unresolved[0].reason,
        CalloutUnresolvedReason::NoTarget
    );
    let equation = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Equation, 1, None),
        NativeCalloutUpstreamStatus::Pass,
        &inventory(
            "doc",
            vec![target(
                CalloutTargetClass::Equation,
                2,
                ObservedCalloutStyle::Label,
                None,
            )],
        ),
    )
    .unwrap();
    assert_eq!(
        equation.unresolved[0].reason,
        CalloutUnresolvedReason::SourceNotEstablished
    );
    let terminal = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Citation, 1, None),
        NativeCalloutUpstreamStatus::Unsupported,
        &inventory("doc", vec![]),
    );
    assert!(matches!(
        terminal,
        Err(NativeCalloutMatchingError::UpstreamTerminal(
            NativeCalloutUpstreamStatus::Unsupported
        ))
    ));
}

#[test]
fn tie_and_annex_crossing_never_guess() {
    let tie_inventory = inventory(
        "doc",
        vec![
            target(
                CalloutTargetClass::Citation,
                2,
                ObservedCalloutStyle::Numeric,
                None,
            ),
            target(
                CalloutTargetClass::Citation,
                3,
                ObservedCalloutStyle::AuthorYear,
                None,
            ),
        ],
    );
    let tie = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Citation, 1, None),
        NativeCalloutUpstreamStatus::Pass,
        &tie_inventory,
    )
    .unwrap();
    assert_eq!(tie.unresolved[0].reason, CalloutUnresolvedReason::Ambiguous);
    let crossing = match_native_callouts(
        &accounting(),
        &blocks(MarkerSubtype::Citation, 1, Some(0)),
        NativeCalloutUpstreamStatus::Pass,
        &inventory(
            "doc",
            vec![target(
                CalloutTargetClass::Citation,
                2,
                ObservedCalloutStyle::Numeric,
                Some(1),
            )],
        ),
    )
    .unwrap();
    assert_eq!(
        crossing.unresolved[0].reason,
        CalloutUnresolvedReason::IllegalAdjacency
    );
}

#[test]
fn malformed_inventory_and_identity_are_rejected() {
    let mut bad = inventory(
        "doc",
        vec![target(
            CalloutTargetClass::Citation,
            2,
            ObservedCalloutStyle::Numeric,
            None,
        )],
    );
    bad.inventory_digest = "sha256:wrong".into();
    assert!(matches!(
        match_native_callouts(
            &accounting(),
            &blocks(MarkerSubtype::Citation, 1, None),
            NativeCalloutUpstreamStatus::Pass,
            &bad
        ),
        Err(NativeCalloutMatchingError::InvalidInventory(
            "inventory_digest"
        ))
    ));
    let mut altered = accounting();
    altered.loss = 1;
    assert!(matches!(
        match_native_callouts(
            &altered,
            &blocks(MarkerSubtype::Citation, 1, None),
            NativeCalloutUpstreamStatus::Pass,
            &inventory("doc", vec![])
        ),
        Err(NativeCalloutMatchingError::IdentityMismatch(
            "accounting_conservation"
        ))
    ));
}
