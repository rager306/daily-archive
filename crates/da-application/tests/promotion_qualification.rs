//! M509-yzblqx S03 T02 — review-package completeness and technical promotion
//! qualification are two independent machine-checked dimensions.
//!
//! Qualification is computed only from measured values: tri-state
//! contamination, measured producer provenance, and measured candidate quality
//! with explicit thresholds. The tests drive the public derivation surfaces
//! only — no fixture files, no store, no graph, no parse route. They pin that a
//! shared producer digest, weakly established independence and unknown
//! contamination are *unqualified*, never clean.

use da_application::derived_evidence_maturity::FamilyId;
use da_application::native_family_promotion_gate::{
    FamilyPromotionEvidence, NamedGap, PromotionDisqualifier, PromotionGateDerivation,
    PromotionProofKind, PromotionQualificationDecision, PromotionQualificationPolicy, ProofSlot,
    can_auto_promote, derive_promotion_gate, derive_promotion_gate_report,
    derive_promotion_qualification,
};
use da_application::parse_learning_evaluation::{
    ContaminationAssessment, ProducerProvenance, PromotionQualityPolicy,
};
use da_ports::Sha256Digest;

const SOURCE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ARTIFACT_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const JAVA_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).expect("fixed synthetic digest")
}

fn producer(code: char) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest(code),
        input_digest: digest('c'),
        method: "same-n-critic".into(),
        authority: "fixture-authority".into(),
    }
}

fn slot(index: usize) -> ProofSlot {
    ProofSlot {
        evidence_ref: format!("evidence-{index}"),
        source_sha256: SOURCE_SHA256.into(),
        artifact_sha256: Some(ARTIFACT_SHA256.into()),
    }
}

/// A package whose *completeness* is full and whose *measurements* are clean.
fn measured_package() -> FamilyPromotionEvidence {
    let mut package = FamilyPromotionEvidence::new(FamilyId::Body, "paper-1");
    package.source_sha256 = SOURCE_SHA256.into();
    package.artifact_sha256 = ARTIFACT_SHA256.into();
    package.grobid_profile = "structural".into();
    for (index, kind) in PromotionProofKind::ALL.into_iter().enumerate() {
        package.set_proof(kind, slot(index));
    }
    package.named_gaps.push(NamedGap {
        id: "body-shadow".into(),
        family: FamilyId::Body,
        summary: "body remains shadow-only".into(),
        must_remain_visible: true,
    });
    package.rollback_target = Some(JAVA_ROLLBACK.into());
    package.contamination = ContaminationAssessment::NotDetected;
    package.producer_provenance = vec![producer('1'), producer('2'), producer('3')];
    package.measured_train_quality = Some(0.8);
    package.measured_validation_quality = Some(0.7);
    package
}

fn qualification(package: &FamilyPromotionEvidence) -> PromotionQualificationDecision {
    derive_promotion_qualification(package, PromotionQualificationPolicy::default())
}

fn assert_unqualified(
    package: &FamilyPromotionEvidence,
    expected: PromotionDisqualifier,
) -> String {
    match qualification(package) {
        PromotionQualificationDecision::Unqualified { reason, measured } => {
            assert_eq!(reason, expected, "unexpected disqualifier: {package:?}");
            measured
        }
        decision @ PromotionQualificationDecision::Qualified { .. } => {
            panic!("expected {expected:?}, got {decision:?}")
        }
    }
}

#[test]
fn measured_clean_values_qualify_and_never_grant_promotion() {
    let package = measured_package();
    match qualification(&package) {
        PromotionQualificationDecision::Qualified {
            independent_producers,
            contamination,
            validation_gap,
        } => {
            assert_eq!(independent_producers, 3);
            assert_eq!(contamination, ContaminationAssessment::NotDetected);
            assert!(
                (validation_gap - 0.1).abs() < 1e-9,
                "measured gap must be train - validation"
            );
        }
        decision => panic!("expected qualified, got {decision:?}"),
    }
    // Qualification is measured-value policy only; completeness still stops at
    // the human gate and the gate can never auto-promote.
    assert!(matches!(
        derive_promotion_gate(&package),
        PromotionGateDerivation::HumanGateRequired { .. }
    ));
    assert!(!can_auto_promote());
}

#[test]
fn completeness_and_qualification_are_independent_dimensions() {
    let mut package = measured_package();
    package.clear_proof(PromotionProofKind::DualMetrics);
    // Incomplete review package, but its measured values satisfy the policy.
    assert!(matches!(
        derive_promotion_gate(&package),
        PromotionGateDerivation::Incomplete { ref missing, .. }
            if missing == &vec![PromotionProofKind::DualMetrics]
    ));
    assert!(matches!(
        qualification(&package),
        PromotionQualificationDecision::Qualified { .. }
    ));

    let report = derive_promotion_gate_report(&package, PromotionQualificationPolicy::default());
    assert!(matches!(
        report.completeness,
        PromotionGateDerivation::Incomplete { .. }
    ));
    assert!(matches!(
        report.qualification,
        PromotionQualificationDecision::Qualified { .. }
    ));
}

#[test]
fn shared_producer_digest_is_unqualified() {
    let mut package = measured_package();
    package.producer_provenance = vec![producer('1'), producer('1'), producer('2')];
    let measured = assert_unqualified(&package, PromotionDisqualifier::SharedProducerDigest);
    assert_eq!(measured, digest('1').as_str());
    // The disqualifier lives in the qualification dimension; completeness is
    // untouched by it.
    assert!(matches!(
        derive_promotion_gate(&package),
        PromotionGateDerivation::HumanGateRequired { .. }
    ));
}

#[test]
fn weak_producer_independence_is_unqualified_and_thresholds_are_explicit() {
    let mut package = measured_package();
    package.producer_provenance = vec![producer('1'), producer('2')];
    let measured = assert_unqualified(&package, PromotionDisqualifier::WeakProducerIndependence);
    assert_eq!(measured, "2 < 3");

    // A producer with an incomplete measured identity does not count as
    // independence.
    package.producer_provenance = vec![producer('1'), producer('2'), producer('3')];
    package.producer_provenance[2].method = "   ".into();
    assert_unqualified(&package, PromotionDisqualifier::WeakProducerIndependence);

    // Lowering the explicit cardinality threshold admits the same measured set.
    let policy = PromotionQualificationPolicy {
        minimum_independent_producers: 2,
        ..PromotionQualificationPolicy::default()
    };
    assert!(matches!(
        derive_promotion_qualification(&package, policy),
        PromotionQualificationDecision::Qualified {
            independent_producers: 2,
            ..
        }
    ));
}

#[test]
fn unknown_contamination_surfaces_as_unknown_and_is_never_clean() {
    let mut package = measured_package();
    package.contamination = ContaminationAssessment::NotAssessed;
    assert_unqualified(&package, PromotionDisqualifier::ContaminationNotAssessed);

    // A brand-new package carries no measured contamination at all.
    let bare = FamilyPromotionEvidence::new(FamilyId::Body, "paper-1");
    assert_eq!(bare.contamination, ContaminationAssessment::NotAssessed);
    assert_unqualified(&bare, PromotionDisqualifier::ContaminationNotAssessed);

    // An artifact that omits the field deserializes to NotAssessed, never clean.
    let mut json = serde_json::to_value(measured_package()).unwrap();
    json.as_object_mut().unwrap().remove("contamination");
    let omitted: FamilyPromotionEvidence = serde_json::from_value(json).unwrap();
    assert_eq!(omitted.contamination, ContaminationAssessment::NotAssessed);
    assert_unqualified(&omitted, PromotionDisqualifier::ContaminationNotAssessed);

    package.contamination = ContaminationAssessment::Detected;
    assert_unqualified(&package, PromotionDisqualifier::ContaminationDetected);
}

#[test]
fn unmeasured_or_failing_quality_is_disqualifying() {
    let mut package = measured_package();
    package.measured_train_quality = None;
    assert_unqualified(&package, PromotionDisqualifier::UnmeasuredQuality);

    package.measured_train_quality = Some(0.8);
    package.measured_validation_quality = Some(f64::NAN);
    assert_unqualified(&package, PromotionDisqualifier::UnmeasuredQuality);

    package.measured_validation_quality = Some(0.2);
    let policy = PromotionQualificationPolicy {
        quality: PromotionQualityPolicy {
            minimum_validation_quality: 0.65,
            maximum_validation_gap: 0.100001,
        },
        ..PromotionQualificationPolicy::default()
    };
    assert!(matches!(
        derive_promotion_qualification(&package, policy),
        PromotionQualificationDecision::Unqualified {
            reason: PromotionDisqualifier::ValidationQualityBelowThreshold,
            ..
        }
    ));

    package.measured_train_quality = Some(0.9);
    package.measured_validation_quality = Some(0.7);
    match derive_promotion_qualification(&package, policy) {
        PromotionQualificationDecision::Unqualified { reason, measured } => {
            assert_eq!(reason, PromotionDisqualifier::ValidationGapExceeded);
            assert!(measured.contains('>'), "measured detail: {measured}");
        }
        decision => panic!("expected gap disqualifier, got {decision:?}"),
    }
}

#[test]
fn report_exposes_both_dimensions_without_promotion_authority() {
    let report =
        derive_promotion_gate_report(&measured_package(), PromotionQualificationPolicy::default());
    let json = serde_json::to_value(&report).unwrap();
    assert!(json.get("completeness").is_some());
    assert!(json.get("qualification").is_some());
    let roundtrip: da_application::native_family_promotion_gate::PromotionGateReport =
        serde_json::from_value(json.clone()).unwrap();
    assert_eq!(roundtrip, report);
    // A report is not an admission: no promoted/authorized state exists here.
    assert!(!json.to_string().contains(concat!("promo", "ted")));
    assert!(!can_auto_promote());
}
