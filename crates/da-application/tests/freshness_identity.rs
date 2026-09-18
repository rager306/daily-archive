//! GAR-009 (M509/S01/T03): evidence sets must carry a bounded freshness
//! identity, and a superseded input must fail the freshness check even when
//! every digest is byte-identical.
//!
//! The test drives the real revision-materialization path over the canonical
//! storage mock, binds the identity it produces into a [`FamilyEvidenceSet`],
//! and reads currency back through the derived-evidence surface. Nothing here
//! writes a graph on behalf of the assertion: state is read from the store the
//! use case already wrote.

use std::sync::Arc;

use da_application::derived_evidence_maturity::{
    BlockedReason, BoundReceipt, DeclaredRevisionTrust, FamilyEvidenceSet, FamilyId,
    FreshnessAssessment, FreshnessBinding, FreshnessIdentity, FreshnessIdentityError,
    FreshnessOutcome, MaturityDerivation, MaturityState, ReceiptKind, assess_freshness,
    assess_set_freshness, derive_maturity, derive_maturity_against_target,
};
use da_application::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationResult,
    RevisionMaterializationUseCase,
};
use da_application::revision_materialization_stage::{
    REVISION_MATERIALIZATION_IMPL_FINGERPRINT, RevisionMaterializationStage,
};
use da_ports::FixedClock;
use da_ports::parser::ParsedArticle;
use kg_storage::MockGraphStore;

const CANONICAL_ID: &str = "2506.07853";

fn request(revision: u32) -> RevisionMaterializationRequest {
    let digest = format!("{revision:064x}");
    RevisionMaterializationRequest {
        source_code: "arxiv".into(),
        canonical_id: CANONICAL_ID.into(),
        revision,
        parsed: ParsedArticle {
            paper_id: CANONICAL_ID.into(),
            title: format!("Paper {CANONICAL_ID}v{revision}"),
            pdf_hash: digest.clone(),
            ..Default::default()
        },
        manifestations: vec![
            RevisionManifestationInput {
                format: "pdf".into(),
                sha256: digest,
                bytes: 100 + revision as u64,
                uri: format!("https://arxiv.org/pdf/{CANONICAL_ID}v{revision}"),
                path: format!("source/{CANONICAL_ID}v{revision}.pdf"),
                capture_status: "captured_local".into(),
            },
            RevisionManifestationInput {
                format: "html".into(),
                sha256: format!("{:064x}", revision + 100),
                bytes: 200 + revision as u64,
                uri: format!("https://arxiv.org/html/{CANONICAL_ID}v{revision}"),
                path: format!("article.v{revision}.html"),
                capture_status: "captured_local".into(),
            },
        ],
    }
}

fn use_case(store: &MockGraphStore) -> RevisionMaterializationUseCase {
    RevisionMaterializationUseCase::new(
        Arc::new(store.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    )
}

/// Bind an identity into an evidence set whose single inventory receipt carries
/// the identity's own digests. Nothing else distinguishes the resulting sets:
/// freshness can only come from the revision identity.
fn freshness_set(identity: FreshnessIdentity) -> FamilyEvidenceSet {
    let binding = FreshnessBinding::new(
        identity.clone(),
        DeclaredRevisionTrust::TeiDeclared {
            revision: identity.target_revision.clone(),
        },
    )
    .expect("a matching TEI declaration is accepted");
    FamilyEvidenceSet {
        family: FamilyId::Body,
        paper_id: CANONICAL_ID.into(),
        inventory: Some(BoundReceipt {
            kind: ReceiptKind::Inventory,
            evidence_ref: "inventory-ref".into(),
            source_sha256: identity.source_digest.clone(),
            artifact_sha256: identity.representation_digest.clone(),
            grobid_profile: "structural".into(),
        }),
        oracle: None,
        atom: None,
        sequence: None,
        shadow: None,
        review: None,
        human_decision_ref: None,
        freshness: Some(binding),
        stale_identity: false,
        denominator_drift: false,
        pooling: false,
        contamination: false,
        semantic_drift: false,
        conservation_loss: false,
        unsafe_production_call: false,
        failed_evidence: false,
        claims_promoted: false,
        deployed: false,
        import_eligible: false,
    }
}

fn blocked_reason(result: MaturityDerivation) -> BlockedReason {
    match result {
        MaturityDerivation::Blocked { reason, .. } => reason,
        other => panic!("expected a blocked derivation, got {other:?}"),
    }
}

fn reached_state(result: MaturityDerivation) -> MaturityState {
    match result {
        MaturityDerivation::Reached { state, .. } => state,
        other => panic!("expected a reached derivation, got {other:?}"),
    }
}

#[tokio::test]
async fn superseded_revision_fails_freshness_even_with_byte_identical_digests() {
    // Case A: the v3 source materialized onto an empty graph is the current
    // input for target v3.
    let empty = MockGraphStore::new();
    let fresh_result = use_case(&empty).materialize(&request(3)).await.unwrap();
    assert_eq!(fresh_result.current_revision, 3);

    // Case B: the same v3 source, byte-identical, re-materialized onto a graph
    // that already holds v5. The graph observed v5, so the input is superseded.
    let ahead = MockGraphStore::new();
    let uc = use_case(&ahead);
    uc.materialize(&request(5)).await.unwrap();
    let superseded_result = uc.materialize(&request(3)).await.unwrap();
    assert_eq!(superseded_result.current_revision, 5);

    // One observation identity for both cases: only the observed revision may
    // differ, so any freshness difference is attributable to revision ordering
    // alone.
    let observation = uc.observation_identity(
        REVISION_MATERIALIZATION_IMPL_FINGERPRINT,
        "structural-v1",
        "revision-materialization/sha256-index",
    );
    let declared = DeclaredRevisionTrust::TeiDeclared {
        revision: format!("{CANONICAL_ID}v3"),
    };
    let fresh_binding = fresh_result
        .freshness_binding(&request(3), &observation, declared.clone())
        .unwrap();
    // The declaration is recorded, not assumed: it names the target the
    // identity declares.
    assert_eq!(fresh_binding.declared_revision, declared);
    let superseded_binding = superseded_result
        .freshness_binding(&request(3), &observation, declared)
        .unwrap();
    let fresh_identity = fresh_binding.identity;
    let superseded_identity = superseded_binding.identity;

    // The proof obligation: the two identities are byte-identical where a digest
    // heuristic would look, and differ only in the observed revision.
    assert_eq!(
        fresh_identity.source_digest,
        superseded_identity.source_digest
    );
    assert_eq!(
        fresh_identity.representation_digest,
        superseded_identity.representation_digest
    );
    assert_eq!(
        fresh_identity.target_revision,
        superseded_identity.target_revision
    );
    assert_ne!(
        fresh_identity.observed_revision,
        superseded_identity.observed_revision
    );
    assert_eq!(
        fresh_identity.observed_revision,
        format!("{CANONICAL_ID}v3")
    );
    assert_eq!(
        superseded_identity.observed_revision,
        format!("{CANONICAL_ID}v5")
    );

    let target = format!("{CANONICAL_ID}v3");
    assert_eq!(
        assess_freshness(&fresh_identity, &target),
        Ok(FreshnessOutcome::Fresh)
    );
    assert_eq!(
        assess_freshness(&superseded_identity, &target),
        Ok(FreshnessOutcome::Superseded)
    );

    let fresh_set = freshness_set(fresh_identity);
    assert_eq!(
        assess_set_freshness(&fresh_set, None),
        FreshnessAssessment::Fresh {
            observed_revision: target.clone(),
        }
    );
    assert_eq!(
        reached_state(derive_maturity(&fresh_set)),
        MaturityState::InventoryBound
    );
    assert_eq!(
        reached_state(derive_maturity_against_target(&fresh_set, &target)),
        MaturityState::InventoryBound
    );

    let superseded_set = freshness_set(superseded_identity);
    assert_eq!(
        assess_set_freshness(&superseded_set, None).as_str(),
        "superseded"
    );
    assert_eq!(
        blocked_reason(derive_maturity(&superseded_set)),
        BlockedReason::StaleIdentity
    );
    assert_eq!(
        blocked_reason(derive_maturity_against_target(&superseded_set, &target)),
        BlockedReason::StaleIdentity
    );
}

#[tokio::test]
async fn stale_observation_and_explicit_target_selection_both_fail_closed() {
    let store = MockGraphStore::new();
    let uc = use_case(&store);
    let observation = uc.observation_identity(
        REVISION_MATERIALIZATION_IMPL_FINGERPRINT,
        "structural-v1",
        "cache-key-1",
    );

    // A stored observation whose graph only knew v2 when target v3 was selected.
    let stored = RevisionMaterializationResult {
        current_revision: 2,
        ..RevisionMaterializationResult::default()
    };
    let stale = stored
        .freshness_binding(
            &request(3),
            &observation,
            DeclaredRevisionTrust::TeiDeclared {
                revision: format!("{CANONICAL_ID}v3"),
            },
        )
        .unwrap()
        .identity;
    assert_eq!(
        assess_freshness(&stale, &format!("{CANONICAL_ID}v3")),
        Ok(FreshnessOutcome::Stale)
    );
    assert_eq!(
        blocked_reason(derive_maturity(&freshness_set(stale))),
        BlockedReason::StaleIdentity
    );

    // The current set is only current for the target it declares: selecting a
    // different target is not satisfied by the declaration it carries.
    let current = use_case(&store).materialize(&request(3)).await.unwrap();
    let identity = current
        .freshness_binding(
            &request(3),
            &observation,
            DeclaredRevisionTrust::TeiDeclared {
                revision: format!("{CANONICAL_ID}v3"),
            },
        )
        .unwrap()
        .identity;
    let set = freshness_set(identity);
    assert_eq!(
        assess_set_freshness(&set, Some(&format!("{CANONICAL_ID}v5"))).as_str(),
        "mismatched"
    );
    assert_eq!(
        blocked_reason(derive_maturity_against_target(
            &set,
            &format!("{CANONICAL_ID}v5")
        )),
        BlockedReason::StaleIdentity
    );
}

#[test]
fn unattributed_evidence_set_is_not_a_current_measurement() {
    let mut set = freshness_set(FreshnessIdentity {
        target_revision: format!("{CANONICAL_ID}v3"),
        observed_revision: format!("{CANONICAL_ID}v3"),
        source_digest: "a".repeat(64),
        representation_digest: "b".repeat(64),
        parser_identity: "grobid-0.9.0-crf".into(),
        config_identity: "structural-v1".into(),
        cache_key: "cache-key-1".into(),
        observed_at: "2026-09-18T00:00:00Z".into(),
    });
    set.freshness = None;

    assert_eq!(
        assess_set_freshness(&set, None),
        FreshnessAssessment::Unattributed
    );
    // Without a selected target the legacy path reports the receipt ladder and
    // asserts nothing about currency …
    assert_eq!(
        reached_state(derive_maturity(&set)),
        MaturityState::InventoryBound
    );
    // … while a caller that selected a target cannot get a pass by omission.
    assert_eq!(
        blocked_reason(derive_maturity_against_target(
            &set,
            &format!("{CANONICAL_ID}v3")
        )),
        BlockedReason::StaleIdentity
    );
}

#[test]
fn a_declared_revision_is_recorded_and_a_contradiction_is_not_a_measurement() {
    let identity = FreshnessIdentity {
        target_revision: format!("{CANONICAL_ID}v3"),
        observed_revision: format!("{CANONICAL_ID}v3"),
        source_digest: "a".repeat(64),
        representation_digest: "b".repeat(64),
        parser_identity: "grobid-0.9.0-crf".into(),
        config_identity: "structural-v1".into(),
        cache_key: "cache-key-1".into(),
        observed_at: "2026-09-18T00:00:00Z".into(),
    };

    // A TEI declaration that does not name the identity target is rejected …
    assert_eq!(
        FreshnessBinding::new(
            identity.clone(),
            DeclaredRevisionTrust::TeiDeclared {
                revision: format!("{CANONICAL_ID}v4"),
            },
        ),
        Err(FreshnessIdentityError::TargetRevisionMismatch)
    );
    // … and a non-TEI source may only defer to a named cross-check.
    let deferred = FreshnessBinding::new(
        identity.clone(),
        DeclaredRevisionTrust::DeferredCrossCheck {
            receipt_ref: "cross-check-1".into(),
        },
    )
    .expect("deferred cross-check is a recorded declaration");
    assert_eq!(
        assess_set_freshness(&freshness_set_from(deferred, identity.clone()), None).as_str(),
        "fresh"
    );

    // A contradiction that arrives through deserialization (bypassing the
    // constructor) is still not readable as a measurement.
    let mut json = serde_json::to_value(freshness_set(identity)).unwrap();
    json["freshness"]["declared_revision"]["revision"] = format!("{CANONICAL_ID}v4").into();
    let contradiction: FamilyEvidenceSet = serde_json::from_value(json).unwrap();
    assert!(matches!(
        assess_set_freshness(&contradiction, None),
        FreshnessAssessment::Malformed {
            error: FreshnessIdentityError::TargetRevisionMismatch
        }
    ));
    assert_eq!(
        blocked_reason(derive_maturity(&contradiction)),
        BlockedReason::MalformedReceipt
    );
}

fn freshness_set_from(binding: FreshnessBinding, identity: FreshnessIdentity) -> FamilyEvidenceSet {
    let mut set = freshness_set(identity);
    set.freshness = Some(binding);
    set
}

#[tokio::test]
async fn produced_identities_are_labels_or_fingerprints_not_raw_config() {
    let store = MockGraphStore::new();
    let uc = use_case(&store);
    let result = uc.materialize(&request(3)).await.unwrap();
    let stage = RevisionMaterializationStage::new(
        Arc::new(store.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
        "arxiv",
        CANONICAL_ID,
        3,
        request(3).manifestations,
    );
    let observation = uc.observation_identity(
        REVISION_MATERIALIZATION_IMPL_FINGERPRINT,
        stage.configuration_fingerprint(),
        "revision-materialization/sha256-index",
    );
    let binding = result
        .freshness_binding(
            &request(3),
            &observation,
            DeclaredRevisionTrust::TeiDeclared {
                revision: format!("{CANONICAL_ID}v3"),
            },
        )
        .unwrap();
    let identity = binding.identity.clone();

    // The produced configuration identity is a sha256 fingerprint and the
    // parser identity is a bare label: neither embeds a raw configuration value.
    assert_eq!(identity.config_identity.len(), 64);
    assert!(
        identity
            .config_identity
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    for value in [&identity.parser_identity, &identity.config_identity] {
        assert!(
            !value.contains(['=', ':', '/', '?', '&']),
            "identity must not carry a raw config value: {value}"
        );
    }
    assert_eq!(
        assess_freshness(&identity, &format!("{CANONICAL_ID}v3")),
        Ok(FreshnessOutcome::Fresh)
    );

    for raw in [
        "GROBID_ENDPOINT=http://127.0.0.1:8070",
        "/etc/da/config.yaml",
        "{\"token\": \"abc\"}",
        "bearer abcdef",
    ] {
        let mut poisoned = identity.clone();
        poisoned.config_identity = raw.into();
        let mut set = freshness_set(poisoned);
        assert!(matches!(
            assess_set_freshness(&set, None),
            FreshnessAssessment::Malformed {
                error: FreshnessIdentityError::UnsafeIdentityLabel
            }
        ));
        assert_eq!(
            blocked_reason(derive_maturity(&set)),
            BlockedReason::MalformedReceipt
        );
        // An unusable identity is unusable under an explicit target too; it
        // never degrades into "currency unknown, therefore fine".
        assert_eq!(
            blocked_reason(derive_maturity_against_target(
                &set,
                &format!("{CANONICAL_ID}v3")
            )),
            BlockedReason::MalformedReceipt
        );
        set.freshness = None;
        assert_eq!(
            assess_set_freshness(&set, None),
            FreshnessAssessment::Unattributed
        );
    }

    // Boundedness: an oversized field cannot ride along inside artifact data.
    let mut oversized = identity.clone();
    oversized.cache_key = "k".repeat(201);
    assert!(matches!(
        assess_set_freshness(&freshness_set(oversized), None),
        FreshnessAssessment::Malformed {
            error: FreshnessIdentityError::FieldTooLong
        }
    ));
}
