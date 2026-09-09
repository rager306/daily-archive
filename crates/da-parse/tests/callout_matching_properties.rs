use da_parse::*;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
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
fn blocks(token: u32) -> NativeFulltextBlocksOutcome {
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
            marker_subtype: Some(MarkerSubtype::Citation),
            parent_annex: None,
        }],
        unsupported: 0,
    }
}
fn inventory(token: u32) -> DigestOnlyTargetInventory {
    let mut value = DigestOnlyTargetInventory {
        document_id: "doc".into(),
        lane_id: "lane".into(),
        source_sha256: "b".repeat(64),
        inventory_digest: String::new(),
        targets: vec![DigestOnlyCalloutTarget {
            class: CalloutTargetClass::Citation,
            target_digest: digest("marker"),
            position: CalloutPosition {
                page_ordinal: 0,
                block_ordinal: 0,
                line_ordinal: 0,
                token_ordinal: token,
                annex_parent: None,
            },
            style: ObservedCalloutStyle::Numeric,
        }],
    };
    value.inventory_digest = inventory_digest(&value);
    value
}

proptest! {
    #[test]
    fn replay_is_byte_identical(token in 1u32..10_000) {
        let result = match_native_callouts(&accounting(), &blocks(token), NativeCalloutUpstreamStatus::Pass, &inventory(token + 1)).unwrap();
        let replay = match_native_callouts(&accounting(), &blocks(token), NativeCalloutUpstreamStatus::Pass, &inventory(token + 1)).unwrap();
        prop_assert_eq!(serde_json::to_vec(&result).unwrap(), serde_json::to_vec(&replay).unwrap());
    }

    #[test]
    fn exact_target_distance_does_not_change_semantic_result(token in 1u32..10_000, distance in 1u32..10_000) {
        let result = match_native_callouts(&accounting(), &blocks(token), NativeCalloutUpstreamStatus::Pass, &inventory(token.saturating_add(distance))).unwrap();
        prop_assert_eq!(result.matched.len(), 1);
        prop_assert_eq!(result.unresolved.len(), 0);
    }
}

#[test]
fn upstream_terminals_are_never_coerced_to_empty_success() {
    for status in [
        NativeCalloutUpstreamStatus::Empty,
        NativeCalloutUpstreamStatus::SourceNotEstablished,
        NativeCalloutUpstreamStatus::Malformed,
        NativeCalloutUpstreamStatus::Timeout,
        NativeCalloutUpstreamStatus::Oversize,
    ] {
        let result = match_native_callouts(&accounting(), &blocks(1), status, &inventory(2));
        assert!(
            matches!(result, Err(NativeCalloutMatchingError::UpstreamTerminal(actual)) if actual == status)
        );
    }
}

#[test]
fn max_bound_is_explicit() {
    let mut value = DigestOnlyTargetInventory {
        document_id: "doc".into(),
        lane_id: "lane".into(),
        source_sha256: "b".repeat(64),
        inventory_digest: String::new(),
        targets: Vec::new(),
    };
    value.targets = (0..=MAX_CALLOUT_OBSERVATIONS)
        .map(|n| DigestOnlyCalloutTarget {
            class: CalloutTargetClass::Citation,
            target_digest: format!("{n:064x}"),
            position: CalloutPosition {
                page_ordinal: 0,
                block_ordinal: 0,
                line_ordinal: 0,
                token_ordinal: n as u32,
                annex_parent: None,
            },
            style: ObservedCalloutStyle::Numeric,
        })
        .collect();
    value.inventory_digest = inventory_digest(&value);
    assert!(matches!(
        match_native_callouts(
            &accounting(),
            &blocks(1),
            NativeCalloutUpstreamStatus::Pass,
            &value
        ),
        Err(NativeCalloutMatchingError::Oversize { .. })
    ));
}
