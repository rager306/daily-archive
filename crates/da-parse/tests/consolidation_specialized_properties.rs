use da_parse::*;
use proptest::prelude::*;

fn accounting(admitted: usize) -> NativeFullRegionAccounting {
    NativeFullRegionAccounting {
        schema_version: NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION.into(),
        document_id: "property-doc".into(),
        provider_id: "fixture".into(),
        source_pdf_sha256:
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        lattice_fingerprint: "sha256:lattice".into(),
        segmentation_config_fingerprint: "sha256:seg".into(),
        admitted,
        conserved: admitted,
        loss: 0,
        visual_evidence: vec![],
        lexical_units: vec![],
    }
}
fn blocks() -> NativeFulltextBlocksOutcome {
    NativeFulltextBlocksOutcome {
        schema_version: NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION.into(),
        document_id: "property-doc".into(),
        lattice_fingerprint: "sha256:lattice".into(),
        config_fingerprint: "sha256:blocks".into(),
        annex_runs: vec![],
        leaves: vec![],
        unsupported: 0,
    }
}

proptest! {
    #[test]
    fn replay_is_deterministic_for_every_admitted_count(admitted in 0usize..1000) {
        let first = evaluate_native_consolidation_specialized(
            &accounting(admitted), &blocks(), SpecializedUpstreamStatus::Pass, None,
        ).unwrap();
        let second = evaluate_native_consolidation_specialized(
            &accounting(admitted), &blocks(), SpecializedUpstreamStatus::Pass, None,
        ).unwrap();
        prop_assert_eq!(first.clone(), second);
        prop_assert_eq!((first.admitted, first.conserved, first.loss), (admitted, admitted, 0));
        prop_assert_eq!(first.families.len(), 6);
    }

    #[test]
    fn upstream_failure_never_becomes_positive_or_zero(status in prop_oneof![
        Just(SpecializedUpstreamStatus::InvalidEvidence),
        Just(SpecializedUpstreamStatus::TerminalFailure),
    ]) {
        let result = evaluate_native_consolidation_specialized(
            &accounting(7), &blocks(), status, None,
        ).unwrap();
        prop_assert!(result.families.iter().all(|family|
            family.disposition == SpecializedDisposition::PropagatedTerminal
        ));
    }
}

#[test]
fn unsupported_families_have_no_positive_count() {
    let result = evaluate_native_consolidation_specialized(
        &accounting(1),
        &blocks(),
        SpecializedUpstreamStatus::Pass,
        None,
    )
    .unwrap();
    for family in &result.families {
        if matches!(
            family.family,
            SpecializedFamily::Licence | SpecializedFamily::Copyright | SpecializedFamily::Software
        ) {
            assert_eq!(family.disposition, SpecializedDisposition::Unsupported);
            assert_eq!(family.count, 0);
        }
    }
}
