use da_parse::*;
use sha2::{Digest, Sha256};

fn accounting() -> NativeFullRegionAccounting {
    NativeFullRegionAccounting {
        schema_version: NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION.into(),
        document_id: "doc-1".into(),
        provider_id: "fixture".into(),
        source_pdf_sha256: format!("sha256:{}", "a".repeat(64)),
        lattice_fingerprint: "sha256:lattice".into(),
        segmentation_config_fingerprint: "sha256:seg".into(),
        admitted: 2,
        conserved: 2,
        loss: 0,
        visual_evidence: vec![],
        lexical_units: vec![],
    }
}
fn blocks() -> NativeFulltextBlocksOutcome {
    NativeFulltextBlocksOutcome {
        schema_version: NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION.into(),
        document_id: "doc-1".into(),
        lattice_fingerprint: "sha256:lattice".into(),
        config_fingerprint: "sha256:blocks".into(),
        annex_runs: vec![],
        leaves: vec![],
        unsupported: 0,
    }
}
fn digest(items: &[String]) -> String {
    let mut h = Sha256::new();
    for item in items {
        h.update(item.as_bytes());
        h.update([0]);
    }
    format!("sha256:{:x}", h.finalize())
}
fn rich() -> DigestOnlyEnrichmentReceipt {
    let evidence = vec![format!("sha256:{}", "b".repeat(64))];
    DigestOnlyEnrichmentReceipt {
        profile: "rich".into(),
        receipt_digest: digest(&evidence),
        evidence_digests: evidence,
        timed_out: false,
    }
}

#[test]
fn structural_is_complete_without_enrichment_and_has_six_families() {
    let result = evaluate_native_consolidation_specialized(
        &accounting(),
        &blocks(),
        SpecializedUpstreamStatus::Pass,
        None,
    )
    .unwrap();
    assert_eq!(result.families.len(), 6);
    assert_eq!((result.admitted, result.conserved, result.loss), (2, 2, 0));
    assert_eq!(
        result.families[5].disposition,
        SpecializedDisposition::StructuralDefault
    );
    assert_eq!(
        result.families[0].disposition,
        SpecializedDisposition::SourceNotEstablished
    );
    assert_eq!(
        result.families[2].disposition,
        SpecializedDisposition::Unsupported
    );
}

#[test]
fn rich_is_injected_digest_only_and_replay_is_byte_equal() {
    let receipt = rich();
    let a = evaluate_native_consolidation_specialized(
        &accounting(),
        &blocks(),
        SpecializedUpstreamStatus::Pass,
        Some(&receipt),
    )
    .unwrap();
    let b = evaluate_native_consolidation_specialized(
        &accounting(),
        &blocks(),
        SpecializedUpstreamStatus::Pass,
        Some(&receipt),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.families[5].disposition, SpecializedDisposition::RichOptIn);
    let encoded = serde_json::to_string(&a).unwrap();
    assert!(
        !encoded.contains("raw_pdf")
            && !encoded.contains("raw_text")
            && !encoded.contains("tei_xml")
    );
}

#[test]
fn identity_and_conservation_are_fail_closed() {
    let mut bad = accounting();
    bad.loss = 1;
    assert!(matches!(
        evaluate_native_consolidation_specialized(
            &bad,
            &blocks(),
            SpecializedUpstreamStatus::Pass,
            None
        ),
        Err(NativeConsolidationSpecializedError::InvalidAccountantReceipt)
    ));
    let mut other = blocks();
    other.document_id = "other".into();
    assert!(matches!(
        evaluate_native_consolidation_specialized(
            &accounting(),
            &other,
            SpecializedUpstreamStatus::Pass,
            None
        ),
        Err(NativeConsolidationSpecializedError::IdentityMismatch(
            "document_id"
        ))
    ));
}

#[test]
fn invalid_and_terminal_upstream_propagate_to_every_family() {
    for status in [
        SpecializedUpstreamStatus::InvalidEvidence,
        SpecializedUpstreamStatus::TerminalFailure,
    ] {
        let result =
            evaluate_native_consolidation_specialized(&accounting(), &blocks(), status, None)
                .unwrap();
        assert!(
            result
                .families
                .iter()
                .all(|item| item.disposition == SpecializedDisposition::PropagatedTerminal)
        );
        assert!(result.families.iter().all(|item| item.count != usize::MAX));
    }
}

#[test]
fn malformed_duplicate_and_oversize_enrichment_are_rejected() {
    let mut duplicate = rich();
    duplicate
        .evidence_digests
        .push(duplicate.evidence_digests[0].clone());
    assert!(matches!(
        evaluate_native_consolidation_specialized(
            &accounting(),
            &blocks(),
            SpecializedUpstreamStatus::Pass,
            Some(&duplicate)
        ),
        Err(NativeConsolidationSpecializedError::DuplicateDigest)
    ));
    let mut malformed = rich();
    malformed.receipt_digest = "sha256:wrong".into();
    assert!(matches!(
        evaluate_native_consolidation_specialized(
            &accounting(),
            &blocks(),
            SpecializedUpstreamStatus::Pass,
            Some(&malformed)
        ),
        Err(NativeConsolidationSpecializedError::InvalidEnrichment(
            "receipt_digest"
        ))
    ));
    let mut oversized = rich();
    oversized.evidence_digests =
        vec![format!("sha256:{}", "c".repeat(64)); MAX_SPECIALIZED_DIGESTS + 1];
    assert!(matches!(
        evaluate_native_consolidation_specialized(
            &accounting(),
            &blocks(),
            SpecializedUpstreamStatus::Pass,
            Some(&oversized)
        ),
        Err(NativeConsolidationSpecializedError::Oversize { .. })
    ));
}

#[test]
fn timed_out_rich_receipt_does_not_break_base_parse() {
    let mut timeout = rich();
    timeout.timed_out = true;
    let result = evaluate_native_consolidation_specialized(
        &accounting(),
        &blocks(),
        SpecializedUpstreamStatus::Pass,
        Some(&timeout),
    )
    .unwrap();
    assert_eq!(
        result.families[5].disposition,
        SpecializedDisposition::Unavailable
    );
    assert_eq!(result.admitted, result.conserved);
}
