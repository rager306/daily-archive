//! S03/T05 (M514-1lss2s) — executable audit of the Java-free slice receipt (R140).
//!
//! T04 emits `artifacts/parse_compat/java_free_vertical_slice.json`. A receipt is
//! a self-declaration until something independent re-reads it, so this file is
//! that independent read. It is gated on `pdf-layout` because the receipt it
//! audits is produced by the `pdf-layout`-only runner
//! (`examples/java_free_vertical_slice.rs`): producer and audit ride one feature
//! lane, and a default `cargo test --tests` does not audit an artifact whose
//! producer it never builds (the sibling convention in
//! `native_fulltext_corpus_shadow_test.rs`).
//!
//! The audit itself reads only git-tracked DATA, the git-tracked artifact and the
//! `da-parse` transport seam. It never reads the gitignored `data/sources/**/*.pdf`
//! corpus (which a clean checkout does not have) and never writes the committed
//! artifact: every mutation below is an in-memory clone.
//!
//! Four disciplines are proven, each with its own negative:
//!
//! 1. **Replay from the carrier, not from the receipt.** The stage matrix, slice
//!    id, typed out-of-slice reasons, document sha256 and the two carrier pin
//!    revisions are re-derived from `data/parse/layout_bakeoff.yaml` and
//!    `data/parse/grobid_compatibility.yaml`, then compared to the artifact — and
//!    the typed receipt must canonicalize back to the committed bytes.
//! 2. **A counted zero, not an absent measurement.** Every in-slice stage reads
//!    `Observed(0)` with `guard.installed == true` on the real
//!    `ParseBackend::fetch_tei` seam; the same receipt with the guard withdrawn is
//!    refused (`ObservedWithoutGuard`), while a typed `NotObserved` absence is
//!    accepted. Absence is typed; a zero is not fabricated.
//! 3. **The counter is non-vacuous.** Consulting Java through the real
//!    `GrobidHttpBackend::fetch_tei` on a guaranteed-unreachable endpoint yields
//!    `Observed(> 0)` on the same observer, and feeding that reading into
//!    `build_receipt` is refused (`ObservedJavaOnSlice`). The observer is
//!    transport tuning: identities with and without it are byte-identical.
//! 4. **The boundary is typed.** An unknown stage, a stage dropped from the
//!    matrix, a document outside the pin, a missing typed disposition and a
//!    numeric count on an out-of-slice stage each produce their own named refusal
//!    rather than a silent skip.
//!
//! Determinism (ADR-051): no network, no clock, no retry. The single transport
//! call uses a malformed `http://` base URL — it fails at URL parsing, so there
//! is no socket, no DNS and no bind/drop race.

#![cfg(feature = "pdf-layout")]

use da_cli::java_free_slice::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, COMPATIBILITY_DATA_REL, DEFAULT_ARTIFACT_REL, DocumentIdentity,
    GuardProvenance, JavaFreeSliceError, JavaFreeSliceReceipt, JavaStageCount, LEAK_MARKERS,
    NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED, NOT_OBSERVED_GUARD_NOT_INSTALLED, OBSERVER_KIND,
    ObservedDocument, ObservedLane, PRODUCT_COVERAGE_EFFECT_NONE, RECEIPT_SCHEMA_VERSION,
    Redaction, SEGMENTATION_NOT_OBSERVED_REJECTED, SLICE_DECLARATION_DATA_REL,
    SLICE_DECLARATION_POINTER, SegmentationReceipt, SliceDeclaration, StageReceipt, StageScope,
    assert_redacted, build_receipt, load_declaration, sha256_hex, validate_receipt,
};
use da_parse::{
    FailureCode, GrobidHttpBackend, GrobidProfile, JavaInvocationCounter,
    JavaInvocationObservation, ParseBackend, ParseBackendRequest,
};

/// The declaration carrier, as committed.
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
/// The cascade stage-id carrier, as committed.
const COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
/// The receipt under audit, as committed.
const ARTIFACT: &str =
    include_str!("../../../artifacts/parse_compat/java_free_vertical_slice.json");
/// The committed artifact path the runner's default must point at.
const ARTIFACT_REL: &str = "artifacts/parse_compat/java_free_vertical_slice.json";
/// The slice identity the declaration and the receipt must agree on.
const SLICE_ID: &str = "pdf-layout-p1-p2";

fn declaration() -> SliceDeclaration {
    load_declaration(LAYOUT, COMPATIBILITY).expect("the committed slice declaration must load")
}

fn receipt() -> JavaFreeSliceReceipt {
    serde_json::from_str(ARTIFACT).expect("the committed receipt must deserialize")
}

fn stage_mut<'a>(receipt: &'a mut JavaFreeSliceReceipt, stage_id: &str) -> &'a mut StageReceipt {
    receipt
        .stages
        .iter_mut()
        .find(|stage| stage.stage_id == stage_id)
        .expect("the declared stage exists")
}

/// Synthetic P1/P2 observation that already satisfies the corpus binding, used
/// only to drive `build_receipt` in the non-vacuity control (never emitted).
fn synthetic_observed(declaration: &SliceDeclaration) -> Vec<ObservedDocument> {
    declaration
        .documents
        .iter()
        .map(|document| ObservedDocument {
            id: document.id.clone(),
            sha256: document.sha256.clone(),
            observed_pages: document.pages,
            lanes: declaration
                .lane_ids
                .iter()
                .map(|lane| ObservedLane {
                    lane_id: lane.clone(),
                    observed_pages: document.pages,
                    admitted_lexical_tokens: 10,
                    lattice_fingerprint: "sha256:lattice".to_owned(),
                    segmentation: SegmentationReceipt::Observed {
                        assigned_tokens: 10,
                        order_digest: "sha256:order".to_owned(),
                    },
                })
                .collect(),
        })
        .collect()
}

#[test]
fn committed_receipt_replays_the_declared_stage_matrix() {
    let declaration = declaration();
    let receipt = receipt();

    // The runner's default artifact path is the committed artifact: a receipt
    // written anywhere else is not this slice's evidence.
    assert_eq!(DEFAULT_ARTIFACT_REL, ARTIFACT_REL);

    // Schema, claim scope and the two carrier bindings, re-derived from the
    // committed bytes rather than trusted from the receipt.
    assert_eq!(declaration.slice_id, SLICE_ID);
    assert_eq!(receipt.schema_version, RECEIPT_SCHEMA_VERSION);
    assert_eq!(receipt.slice_id, declaration.slice_id);
    assert_eq!(receipt.adr_ref, declaration.adr_ref);
    assert_eq!(receipt.requirement_ids, declaration.requirement_ids);
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.declaration.manifest, SLICE_DECLARATION_DATA_REL);
    assert_eq!(receipt.declaration.pointer, SLICE_DECLARATION_POINTER);
    assert_eq!(
        receipt.declaration.revision_sha256,
        format!("sha256:{}", sha256_hex(LAYOUT.as_bytes())),
        "the receipt must be pinned to the committed declaration bytes"
    );
    assert_eq!(receipt.stage_id_source.manifest, COMPATIBILITY_DATA_REL);
    assert_eq!(
        receipt.stage_id_source.revision_sha256,
        format!("sha256:{}", sha256_hex(COMPATIBILITY.as_bytes())),
        "the receipt must be pinned to the committed cascade bytes"
    );

    // Stage matrix: same ids in cascade order, same scope/java_required/name,
    // and exactly one typed disposition per stage.
    let declared_ids: Vec<&str> = declaration
        .stages
        .iter()
        .map(|stage| stage.stage_id.as_str())
        .collect();
    let receipt_ids: Vec<&str> = receipt
        .stages
        .iter()
        .map(|stage| stage.stage_id.as_str())
        .collect();
    assert_eq!(receipt_ids, declared_ids);
    for row in &receipt.stages {
        let declared = declaration
            .stage(&row.stage_id)
            .expect("the receipt names only declared stages");
        assert_eq!(row.scope, declared.scope);
        assert_eq!(row.java_required, declared.java_required);
        assert_eq!(row.stage_name, declared.stage_name);
        match (declared.scope, &row.count) {
            (StageScope::InSlice, JavaStageCount::Observed { attempts }) => {
                assert_eq!(
                    *attempts, 0,
                    "in-slice stage {} observed Java",
                    row.stage_id
                );
            }
            (StageScope::OutOfSlice, JavaStageCount::OutOfSlice { reason }) => {
                assert_eq!(
                    Some(reason.as_str()),
                    declared.out_of_slice_reason.as_deref(),
                    "stage {} carries the wrong out-of-slice reason",
                    row.stage_id
                );
                assert!(declaration.out_of_slice_reasons.contains(reason));
            }
            (StageScope::IdentityAdmission, JavaStageCount::NotObserved { reason }) => {
                assert_eq!(reason, NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED);
            }
            (scope, count) => panic!(
                "stage {} carries {count:?}, which is not a typed disposition for {}",
                row.stage_id,
                scope.as_str()
            ),
        }
    }
    assert_eq!(declaration.in_slice_stage_ids(), vec!["P1", "P2"]);
    assert_eq!(
        declaration.out_of_slice_stage_ids(),
        vec!["P3", "P4", "P5", "P6", "P7", "P8", "P9"]
    );
    assert_eq!(declaration.stages.len(), 10);

    // Corpus identities: same ids, same sha256, same pages, repository-relative
    // paths only. The receipt restates the declaration; it never widens it.
    assert_eq!(receipt.corpus.document_count, declaration.documents.len());
    assert_eq!(receipt.corpus.identities.len(), declaration.documents.len());
    assert_eq!(receipt.corpus.corpus_id, declaration.corpus_id);
    assert_eq!(receipt.corpus.lane_ids, declaration.lane_ids);
    assert_eq!(receipt.corpus.pages_per_lane, declaration.page_denominator);
    assert_eq!(
        receipt.corpus.lane_page_denominator,
        declaration.lane_page_denominator()
    );
    assert_eq!(
        receipt.corpus.observed_lane_pages,
        declaration.lane_page_denominator(),
        "every declared lane-page must have been observed"
    );
    for identity in &receipt.corpus.identities {
        let declared = declaration
            .document(&identity.id)
            .expect("the receipt names only pinned documents");
        assert_eq!(identity.sha256, declared.sha256);
        assert_eq!(identity.pages, declared.pages);
        assert_eq!(identity.source_pdf, declared.source_pdf);
        assert!(!identity.source_pdf.starts_with('/'));
        assert!(!std::path::Path::new(&identity.source_pdf).is_absolute());
    }
    let mut expected_lanes = declaration.lane_ids.clone();
    expected_lanes.sort();
    for document in &receipt.corpus.documents {
        let declared = declaration
            .document(&document.id)
            .expect("the receipt names only pinned documents");
        assert_eq!(document.observed_pages, declared.pages);
        let mut observed_lanes: Vec<&str> = document
            .lanes
            .iter()
            .map(|lane| lane.lane_id.as_str())
            .collect();
        observed_lanes.sort();
        assert_eq!(
            observed_lanes,
            expected_lanes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        for lane in &document.lanes {
            assert_eq!(lane.observed_pages, declared.pages);
            assert!(!lane.lattice_fingerprint.trim().is_empty());
        }
    }

    // Guard provenance: the zero is a measurement, not an absence.
    assert_eq!(receipt.guard.observer, OBSERVER_KIND);
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(receipt.guard.observed_java_calls, 0);

    // Redaction contract, and the artifact must be exactly what the typed
    // receipt canonicalizes to (no hand edit, no foreign field).
    assert_eq!(receipt.redaction, Redaction::clean());
    assert_redacted(ARTIFACT).expect("the committed artifact is redacted");
    for marker in LEAK_MARKERS {
        assert!(
            !ARTIFACT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        ARTIFACT,
        "the committed artifact must be exactly the canonical serialization"
    );

    // And the whole thing re-validates against the declaration it claims.
    validate_receipt(&receipt, &declaration)
        .expect("the committed receipt must re-validate against the committed declaration");
}

#[test]
fn a_counted_zero_requires_an_installed_guard() {
    let declaration = declaration();
    let committed = receipt();

    // Positive: every in-slice stage is a measured zero under the guard.
    for stage in &committed.stages {
        if stage.scope == StageScope::InSlice {
            assert_eq!(stage.count, JavaStageCount::Observed { attempts: 0 });
        }
    }

    // Negative 1: the very same zero without an installed guard is refused.
    let mut unguarded = committed.clone();
    unguarded.guard.installed = false;
    assert_eq!(
        validate_receipt(&unguarded, &declaration).unwrap_err(),
        JavaFreeSliceError::ObservedWithoutGuard {
            stage: "P1".to_owned()
        }
    );

    // Negative 2: a guard that rotates the cache identity is refused the same
    // way — its zero cannot be attributed to a stable transport.
    let mut rotated = committed.clone();
    rotated.guard.identity_isolated = false;
    assert!(matches!(
        validate_receipt(&rotated, &declaration).unwrap_err(),
        JavaFreeSliceError::ObservedWithoutGuard { .. }
    ));

    // The typed alternative is allowed: withdrawing the guard and declaring a
    // typed NotObserved absence is accepted. Absence is typed; a zero is not
    // fabricated from it.
    let mut typed_absence = committed.clone();
    typed_absence.guard.installed = false;
    for stage in typed_absence.stages.iter_mut() {
        if stage.scope == StageScope::InSlice {
            stage.count = JavaStageCount::NotObserved {
                reason: NOT_OBSERVED_GUARD_NOT_INSTALLED.to_owned(),
            };
        }
    }
    validate_receipt(&typed_absence, &declaration)
        .expect("a typed absence must be accepted, never silently zeroed");
}

#[tokio::test]
async fn the_observer_counts_a_real_java_consult() {
    let declaration = declaration();

    // The same observer the receipt installs, driven through the real seam.
    let counter = JavaInvocationCounter::new();
    let plain = GrobidHttpBackend::new(Some("http://")).with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());

    // An unobserved seam is a typed absence, never a fabricated zero.
    assert_eq!(
        plain.java_invocation_observation(),
        JavaInvocationObservation::NotObserved
    );
    assert_eq!(
        counter.observation(),
        JavaInvocationObservation::Observed { attempts: 0 }
    );

    // Consulting Java on a guaranteed-unreachable endpoint counts the attempt
    // even though no wire exchange completes. `http://` carries an empty
    // authority, so this fails at URL parsing: no socket, no DNS, no race.
    let error = watched
        .fetch_tei(ParseBackendRequest::new(
            "m514-s03-t05",
            b"%PDF-1.4 java-free-slice fixture".to_vec(),
        ))
        .await
        .expect_err("an unroutable endpoint must fail");
    assert_eq!(error.code(), FailureCode::Transport);

    let measured = counter.observation();
    assert_eq!(
        measured,
        JavaInvocationObservation::Observed { attempts: 1 },
        "the attempt is counted before any wire bytes exist"
    );
    assert_eq!(watched.java_invocation_observation(), measured);
    let attempts = measured.attempts().expect("an installed guard is observed");
    assert!(attempts > 0, "the counter must be able to grow");

    // And the slice refuses that measurement: the same reading fed to the
    // receipt builder is ObservedJavaOnSlice, not a zero.
    assert_eq!(
        build_receipt(
            &declaration,
            GuardProvenance {
                installed: true,
                identity_isolated: true,
                observed_java_calls: attempts,
            },
            "sha256:declaration",
            "sha256:compatibility",
            &[],
        )
        .unwrap_err(),
        JavaFreeSliceError::ObservedJavaOnSlice { attempts: 1 }
    );

    // With a fresh guard that never consulted Java, the same construction
    // succeeds: the zero is precisely the measured absence of a consult.
    let clean = JavaInvocationCounter::new();
    build_receipt(
        &declaration,
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: clean
                .observation()
                .attempts()
                .expect("an installed guard is observed"),
        },
        "sha256:declaration",
        "sha256:compatibility",
        &synthetic_observed(&declaration),
    )
    .expect("an installed guard that never consulted Java builds a receipt");
}

#[test]
fn the_observer_is_not_part_of_the_tei_identity() {
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural());
    let watched = plain
        .clone()
        .with_invocation_observer(JavaInvocationCounter::new());

    // Installing the observer must not rotate the cache key: same identity,
    // field for field and byte for byte.
    let plain_identity = plain.identity();
    let watched_identity = watched.identity();
    assert_eq!(plain_identity, watched_identity);
    assert_eq!(plain_identity.backend_id(), watched_identity.backend_id());
    assert_eq!(
        plain_identity.profile_fingerprint(),
        watched_identity.profile_fingerprint()
    );
    assert_eq!(
        plain_identity.backend_version(),
        watched_identity.backend_version()
    );
    assert_eq!(
        format!("{plain_identity:?}"),
        format!("{watched_identity:?}"),
        "identity with and without the observer must be byte-identical"
    );
    assert_eq!(plain.profile(), watched.profile());

    // The equality above is not vacuous: identity does rotate on a real input.
    assert_ne!(
        plain_identity,
        plain.clone().with_backend_version("0.9.0-crf").identity(),
        "a real identity input must change the identity, or the isolation is vacuous"
    );
}

#[test]
fn every_receipt_mutation_fails_by_name() {
    let declaration = declaration();
    let committed = receipt();

    // An unknown stage named by the receipt.
    let mut unknown_stage = committed.clone();
    unknown_stage.stages.push(StageReceipt {
        stage_id: "P99".to_owned(),
        stage_name: "ghost".to_owned(),
        scope: StageScope::OutOfSlice,
        java_required: true,
        count: JavaStageCount::OutOfSlice {
            reason: "java_authority".to_owned(),
        },
    });
    assert_eq!(
        validate_receipt(&unknown_stage, &declaration).unwrap_err(),
        JavaFreeSliceError::UnknownStage {
            stage: "P99".to_owned()
        }
    );

    // A stage dropped from the matrix.
    let mut dropped = committed.clone();
    dropped.stages.pop();
    assert!(matches!(
        validate_receipt(&dropped, &declaration).unwrap_err(),
        JavaFreeSliceError::UnclassifiedStage { .. }
    ));

    // A document outside the pinned identities.
    let mut unpinned = committed.clone();
    unpinned.corpus.identities.push(DocumentIdentity {
        id: "not-pinned".to_owned(),
        source_pdf: "data/sources/arxiv/xx/0/source/0.pdf".to_owned(),
        sha256: "0".repeat(64),
        pages: 1,
    });
    assert_eq!(
        validate_receipt(&unpinned, &declaration).unwrap_err(),
        JavaFreeSliceError::UnknownDocument {
            document: "not-pinned".to_owned()
        }
    );

    // A pinned document whose sha256 drifted.
    let mut drifted = committed.clone();
    drifted.corpus.identities[0].sha256 = "0".repeat(64);
    assert!(matches!(
        validate_receipt(&drifted, &declaration).unwrap_err(),
        JavaFreeSliceError::CorpusBindingMismatch { .. }
    ));

    // A document count that drifted from the pinned corpus.
    let mut count_drift = committed.clone();
    count_drift.corpus.document_count = 99;
    assert!(matches!(
        validate_receipt(&count_drift, &declaration).unwrap_err(),
        JavaFreeSliceError::CorpusBindingMismatch { .. }
    ));

    // An out-of-slice stage with no typed count.
    let mut untyped = committed.clone();
    stage_mut(&mut untyped, "P3").count = JavaStageCount::NotObserved {
        reason: "whatever".to_owned(),
    };
    assert_eq!(
        validate_receipt(&untyped, &declaration).unwrap_err(),
        JavaFreeSliceError::OutOfSliceStageCounted {
            stage: "P3".to_owned()
        }
    );

    // An out-of-slice reason outside the closed vocabulary.
    let mut bad_reason = committed.clone();
    stage_mut(&mut bad_reason, "P3").count = JavaStageCount::OutOfSlice {
        reason: "made_up".to_owned(),
    };
    assert_eq!(
        validate_receipt(&bad_reason, &declaration).unwrap_err(),
        JavaFreeSliceError::VocabularyDrift {
            reason: "made_up".to_owned()
        }
    );

    // An in-slice stage with a disposition that is neither the counted zero
    // nor the typed guard absence.
    let mut in_slice_typo = committed.clone();
    stage_mut(&mut in_slice_typo, "P2").count = JavaStageCount::NotObserved {
        reason: SEGMENTATION_NOT_OBSERVED_REJECTED.to_owned(),
    };
    assert!(matches!(
        validate_receipt(&in_slice_typo, &declaration).unwrap_err(),
        JavaFreeSliceError::StageMatrixMismatch { .. }
    ));

    // An in-slice stage that observed Java.
    let mut observed_java = committed.clone();
    stage_mut(&mut observed_java, "P1").count = JavaStageCount::Observed { attempts: 1 };
    assert_eq!(
        validate_receipt(&observed_java, &declaration).unwrap_err(),
        JavaFreeSliceError::ObservedJavaOnSlice { attempts: 1 }
    );

    // An admission boundary that is not the typed not-counted disposition.
    let mut admission = committed.clone();
    stage_mut(&mut admission, "P0").count = JavaStageCount::Observed { attempts: 0 };
    assert!(matches!(
        validate_receipt(&admission, &declaration).unwrap_err(),
        JavaFreeSliceError::StageMatrixMismatch { .. }
    ));

    // A scope, java_required or name that drifted from the cascade pointer.
    let mut scope_drift = committed.clone();
    stage_mut(&mut scope_drift, "P3").scope = StageScope::InSlice;
    assert!(matches!(
        validate_receipt(&scope_drift, &declaration).unwrap_err(),
        JavaFreeSliceError::StageMatrixMismatch { .. }
    ));
    let mut java_required_drift = committed.clone();
    stage_mut(&mut java_required_drift, "P3").java_required = false;
    assert!(matches!(
        validate_receipt(&java_required_drift, &declaration).unwrap_err(),
        JavaFreeSliceError::StageMatrixMismatch { .. }
    ));
    let mut name_drift = committed.clone();
    stage_mut(&mut name_drift, "P1").stage_name = "ghost".to_owned();
    assert!(matches!(
        validate_receipt(&name_drift, &declaration).unwrap_err(),
        JavaFreeSliceError::StageMatrixMismatch { .. }
    ));

    // A widened claim or a product-coverage claim.
    let mut widened = committed.clone();
    widened.claim_scope = "product_ready".to_owned();
    assert_eq!(
        validate_receipt(&widened, &declaration).unwrap_err(),
        JavaFreeSliceError::ClaimScopeDrift {
            found: "product_ready".to_owned()
        }
    );
    let mut product = committed.clone();
    product.product_coverage_effect = "full".to_owned();
    assert_eq!(
        validate_receipt(&product, &declaration).unwrap_err(),
        JavaFreeSliceError::ProductCoverageClaimed {
            found: "full".to_owned()
        }
    );

    // A drifted schema version or slice id.
    let mut schema = committed.clone();
    schema.schema_version = "java-free-vertical-slice-receipt.v0".to_owned();
    assert!(matches!(
        validate_receipt(&schema, &declaration).unwrap_err(),
        JavaFreeSliceError::SchemaDrift { .. }
    ));
    let mut slice = committed.clone();
    slice.slice_id = "some-other-slice".to_owned();
    assert!(matches!(
        validate_receipt(&slice, &declaration).unwrap_err(),
        JavaFreeSliceError::SchemaDrift { .. }
    ));

    // A raised redaction flag.
    let mut leaked = committed.clone();
    leaked.redaction.raw_text = true;
    assert_eq!(
        validate_receipt(&leaked, &declaration).unwrap_err(),
        JavaFreeSliceError::RedactionViolation {
            marker: "redaction flag set".to_owned()
        }
    );

    // The committed artifact is untouched: every mutation above was an
    // in-memory clone, and the tracked bytes still canonicalize.
    let reserialized = receipt()
        .artifact_json_bytes()
        .expect("the committed receipt still serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        ARTIFACT,
        "the audit must not have modified the committed artifact"
    );
}
