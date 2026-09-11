//! Closed in-memory decision packages for the five native evidence families.
//!
//! This is a decision-log boundary only. It deliberately has no parser, I/O,
//! graph, deployment, import, or promotion capability. Four review families
//! are bound independently; segmentation remains a catalogued observation.

use crate::derived_evidence_maturity::FamilyId;
use crate::native_family_promotion_gate::{
    NamedGap, PromotionGateBlockReason, PromotionGateDerivation, PromotionProofKind,
};
use crate::native_family_same_n_adapter::{
    FamilySameNBinding, FamilySameNBindingResult, FamilySameNInput, bind_family_same_n,
    validate_named_gap,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const JAVA_ROLLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";
const INDEPENDENTLY_SELECTABLE: [&str; 2] = ["header", "references"];
const REVIEW_FAMILIES: [FamilyId; 4] = [
    FamilyId::Header,
    FamilyId::Bibliography,
    FamilyId::Body,
    FamilyId::Visual,
];

const REQUIRED_GAPS: [(FamilyId, &str); 5] = [
    (FamilyId::Header, "hub_native_accepted_header_vs_java_raw"),
    (FamilyId::Bibliography, "citation_names_not_biblstruct"),
    (FamilyId::Body, "body_shadow_java_fallback"),
    (FamilyId::Visual, "visual_unpooled_heads"),
    (
        FamilyId::Segmentation,
        "segmentation_not_independently_selectable",
    ),
];

/// Inputs for a complete, closed five-family decision set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyDecisionSetInput {
    pub paper_id: String,
    pub families: BTreeMap<FamilyId, FamilySameNInput>,
    #[serde(default)]
    pub independently_selectable_widened: bool,
    #[serde(default)]
    pub claimed_pooled_winner: Option<String>,
}

/// The outcome of one family in the assembled decision log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FamilyPackageOutcome {
    ReadyReview {
        family: FamilyId,
    },
    Incomplete {
        family: FamilyId,
        missing: Vec<PromotionProofKind>,
    },
    Blocked {
        family: FamilyId,
        reason: PromotionGateBlockReason,
        field: String,
    },
}

/// A family package retains the canonical binding and named gaps verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyDecisionPackage {
    pub family: FamilyId,
    pub outcome: FamilyPackageOutcome,
    pub named_gaps: Vec<NamedGap>,
    pub binding: Option<Box<FamilySameNBinding>>,
}

/// There is no winner variant: this enum is intentionally one-valued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PooledWinner {
    Forbidden,
}

/// Closed set-level pins consumed by the later rehearsal slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyDecisionSet {
    pub paper_id: String,
    pub packages: BTreeMap<FamilyId, FamilyDecisionPackage>,
    pub independently_selectable: Vec<String>,
    pub pooled_winner: PooledWinner,
    pub deployed: bool,
    pub import_eligible: bool,
    pub this_wave_promotes: Option<String>,
    pub java_rollback_target: String,
}

/// Assembly never exposes a partially admitted set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FamilyDecisionSetResult {
    Ready {
        set: Box<FamilyDecisionSet>,
    },
    Blocked {
        reason: PromotionGateBlockReason,
        field: String,
    },
}

fn blocked(reason: PromotionGateBlockReason, field: &str) -> FamilyDecisionSetResult {
    FamilyDecisionSetResult::Blocked {
        reason,
        field: field.to_owned(),
    }
}

fn required_gap(family: FamilyId) -> &'static str {
    REQUIRED_GAPS
        .iter()
        .find_map(|(known, id)| (*known == family).then_some(*id))
        .expect("all five FamilyId variants have a required gap")
}

fn canonical_gap(family: FamilyId, gaps: &[NamedGap]) -> Option<NamedGap> {
    gaps.iter()
        .find(|gap| {
            gap.family == family && gap.id == required_gap(family) && gap.must_remain_visible
        })
        .cloned()
}

fn binding_gap(binding: &FamilySameNBinding, family: FamilyId) -> Option<NamedGap> {
    match &binding.gate {
        PromotionGateDerivation::HumanGateRequired { named_gaps, .. } => {
            canonical_gap(family, named_gaps)
        }
        _ => None,
    }
}

/// Assemble four independent review packages and one non-admitted observation.
///
/// The only evaluator call is the single call site inside the review-family
/// loop. Rows, denominators, proof references, reports, and gaps are never
/// merged or recomputed here.
pub fn assemble_family_decision_set(input: FamilyDecisionSetInput) -> FamilyDecisionSetResult {
    if input.families.len() != FamilyId::ALL.len()
        || FamilyId::ALL
            .iter()
            .any(|family| !input.families.contains_key(family))
    {
        return blocked(PromotionGateBlockReason::MalformedPackage, "families");
    }
    if input.independently_selectable_widened {
        return blocked(
            PromotionGateBlockReason::AttemptedAutomaticPromotion,
            "admission",
        );
    }
    if input.claimed_pooled_winner.is_some() {
        return blocked(PromotionGateBlockReason::Pooling, "pooled_winner");
    }
    if input.paper_id.trim().is_empty() {
        return blocked(PromotionGateBlockReason::MalformedPackage, "paper_id");
    }
    if input
        .families
        .iter()
        .any(|(key, family)| key != &family.family || family.paper_id != input.paper_id)
    {
        return blocked(
            PromotionGateBlockReason::MalformedPackage,
            "families.identity",
        );
    }

    let mut packages = BTreeMap::new();
    for family in REVIEW_FAMILIES {
        let family_input = input
            .families
            .get(&family)
            .expect("validated FamilyId::ALL contains every review family")
            .clone();
        let result = bind_family_same_n(family_input);
        let FamilySameNBindingResult::Ready { binding } = result else {
            let FamilySameNBindingResult::Blocked { reason, field } = result else {
                unreachable!("binding has only Ready and Blocked variants")
            };
            return blocked(reason, &format!("{family:?}.{field}"));
        };
        if !matches!(
            binding.gate,
            PromotionGateDerivation::HumanGateRequired {
                deployed: false,
                import_eligible: false,
                this_wave_promotes: None,
                ..
            }
        ) {
            return blocked(
                PromotionGateBlockReason::AttemptedAutomaticPromotion,
                "integrity",
            );
        }
        let Some(_required_gap) = binding_gap(&binding, family) else {
            return blocked(PromotionGateBlockReason::NamedGapHidden, "named_gaps");
        };
        let named_gaps = match &binding.gate {
            PromotionGateDerivation::HumanGateRequired { named_gaps, .. } => named_gaps.clone(),
            _ => unreachable!("authority validation above requires the human gate"),
        };
        packages.insert(
            family,
            FamilyDecisionPackage {
                family,
                outcome: FamilyPackageOutcome::ReadyReview { family },
                named_gaps,
                binding: Some(binding),
            },
        );
    }

    // Segmentation is catalogued fallback evidence, not an independently
    // selectable family and therefore never enters the canonical binder.
    let segmentation = FamilyId::Segmentation;
    let segmentation_input = input
        .families
        .get(&segmentation)
        .expect("validated FamilyId::ALL contains segmentation");
    let Some(gap) = segmentation_input
        .named_gaps
        .iter()
        .find(|gap| gap.id == required_gap(segmentation) && gap.family == segmentation)
    else {
        return blocked(PromotionGateBlockReason::NamedGapHidden, "named_gaps");
    };
    if !gap.must_remain_visible {
        return blocked(PromotionGateBlockReason::NamedGapHidden, "named_gaps");
    }
    if let Err(field) = validate_named_gap(segmentation, gap) {
        return blocked(PromotionGateBlockReason::MalformedPackage, &field);
    }
    let gap = NamedGap {
        id: gap.id.clone(),
        family: gap.family,
        summary: gap.summary.clone(),
        must_remain_visible: gap.must_remain_visible,
    };
    packages.insert(
        segmentation,
        FamilyDecisionPackage {
            family: segmentation,
            outcome: FamilyPackageOutcome::Incomplete {
                family: segmentation,
                missing: PromotionProofKind::ALL.to_vec(),
            },
            named_gaps: vec![gap],
            binding: None,
        },
    );

    FamilyDecisionSetResult::Ready {
        set: Box::new(FamilyDecisionSet {
            paper_id: input.paper_id,
            packages,
            independently_selectable: INDEPENDENTLY_SELECTABLE
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            pooled_winner: PooledWinner::Forbidden,
            deployed: false,
            import_eligible: false,
            this_wave_promotes: None,
            java_rollback_target: JAVA_ROLLBACK_TARGET.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_family_same_n_adapter::{
        FamilyIntegrityClaims, InjectedProofRefs, NamedFamilyGap,
    };
    use crate::parse_learning_evaluation::{CriticObservation, CriticStatus, FrozenEvalCase};
    use da_ports::{
        CandidateManifest, CandidateReceipt, LearningLineage, PromotionState, Sha256Digest,
    };

    fn digest(ch: char) -> Sha256Digest {
        Sha256Digest::new(ch.to_string().repeat(64)).unwrap()
    }

    fn input(family: FamilyId) -> FamilySameNInput {
        let suffix = family.as_str();
        let cases = (0..2)
            .map(|order| FrozenEvalCase {
                case_id: format!("{suffix}-case-{order}"),
                corpus: digest('c'),
                order,
                expected_digest: digest('e'),
            })
            .collect::<Vec<_>>();
        let rows = |producer: &str| {
            (0..2)
                .map(|order| CriticObservation {
                    schema_version: 1,
                    case_id: format!("{suffix}-case-{order}"),
                    corpus: digest('c'),
                    order,
                    producer: producer.into(),
                    metric_one: 0.8,
                    metric_two: 0.6,
                    status: CriticStatus::Ok,
                })
                .collect()
        };
        FamilySameNInput {
            family,
            paper_id: "paper-1".into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
            candidate: CandidateManifest {
                receipt: CandidateReceipt {
                    schema_version: 1,
                    candidate_id: digest('a'),
                    lineage: LearningLineage {
                        source: digest('b'),
                        corpus: digest('c'),
                        config: digest('d'),
                        prior_policy: digest('e'),
                        conservation: digest('f'),
                        split: digest('1'),
                    },
                    prior_candidate: Some(digest('9')),
                    conservation_ok: true,
                    import_eligible: false,
                    promotion_state: PromotionState::Candidate,
                    deployed: false,
                },
                rank: 1,
                hidden_dim: 8,
                train_count: 2,
                validation_count: 1,
                train_quality: 0.8,
                validation_quality: 0.7,
                validation_gap: 0.1,
                weights_digest: digest('2'),
            },
            frozen_cases: cases,
            baseline_rows: rows("critic-a"),
            candidate_rows: rows("critic-b"),
            conservation_ok: true,
            trainer_producer: "sona-trainer".into(),
            proof_refs: InjectedProofRefs {
                java_rollback_target: JAVA_ROLLBACK_TARGET.into(),
                java_rollback_proof_ref: format!("java-rollback-{suffix}"),
                architecture_quarantine_proof_ref: format!("architecture-{suffix}"),
            },
            integrity: FamilyIntegrityClaims::default(),
            named_gaps: vec![NamedFamilyGap {
                id: required_gap(family).into(),
                family,
                summary: format!("visible bounded gap for {suffix}"),
                must_remain_visible: true,
            }],
        }
    }

    fn set_input() -> FamilyDecisionSetInput {
        FamilyDecisionSetInput {
            paper_id: "paper-1".into(),
            families: FamilyId::ALL
                .into_iter()
                .map(|family| (family, input(family)))
                .collect(),
            independently_selectable_widened: false,
            claimed_pooled_winner: None,
        }
    }

    #[test]
    fn happy_set_has_four_independent_review_packages_and_segmentation_observation() {
        let FamilyDecisionSetResult::Ready { set } = assemble_family_decision_set(set_input())
        else {
            panic!("expected ready set")
        };
        assert_eq!(set.packages.len(), 5);
        assert_eq!(set.independently_selectable, ["header", "references"]);
        assert_eq!(set.pooled_winner, PooledWinner::Forbidden);
        assert!(!set.deployed && !set.import_eligible && set.this_wave_promotes.is_none());
        assert_eq!(set.java_rollback_target, JAVA_ROLLBACK_TARGET);
        for family in REVIEW_FAMILIES {
            let package = &set.packages[&family];
            assert!(matches!(
                package.outcome,
                FamilyPackageOutcome::ReadyReview { family: actual } if actual == family
            ));
            assert_eq!(package.named_gaps.len(), 1);
            assert!(package.named_gaps[0].must_remain_visible);
            assert!(matches!(
                package.binding.as_ref().unwrap().gate,
                PromotionGateDerivation::HumanGateRequired { .. }
            ));
        }
        let segmentation = &set.packages[&FamilyId::Segmentation];
        assert!(matches!(
            segmentation.outcome,
            FamilyPackageOutcome::Incomplete { family: FamilyId::Segmentation, ref missing }
                if missing == &PromotionProofKind::ALL.to_vec()
        ));
        assert!(segmentation.binding.is_none());
    }

    #[test]
    fn set_authority_and_gap_mutations_fail_closed() {
        let mut pooled = set_input();
        pooled.claimed_pooled_winner = Some("header".into());
        assert!(matches!(
            assemble_family_decision_set(pooled),
            FamilyDecisionSetResult::Blocked { reason: PromotionGateBlockReason::Pooling, field }
                if field == "pooled_winner"
        ));

        let mut widened = set_input();
        widened.independently_selectable_widened = true;
        assert!(matches!(
            assemble_family_decision_set(widened),
            FamilyDecisionSetResult::Blocked { reason: PromotionGateBlockReason::AttemptedAutomaticPromotion, field }
                if field == "admission"
        ));

        let mut missing = set_input();
        missing
            .families
            .get_mut(&FamilyId::Header)
            .unwrap()
            .named_gaps
            .clear();
        assert!(matches!(
            assemble_family_decision_set(missing),
            FamilyDecisionSetResult::Blocked { reason: PromotionGateBlockReason::NamedGapHidden, field }
                if field == "named_gaps"
        ));

        let mut absent = set_input();
        absent.families.remove(&FamilyId::Visual);
        assert!(matches!(
            assemble_family_decision_set(absent),
            FamilyDecisionSetResult::Blocked { reason: PromotionGateBlockReason::MalformedPackage, field }
                if field == "families"
        ));
    }

    /// ADR-051: f32-derived values shift one ULP through a JSON roundtrip, so
    /// DTO equality is compared float-tolerantly instead of exactly.
    fn assert_json_roundtrip_close(a: &serde_json::Value, b: &serde_json::Value) {
        match (a, b) {
            (serde_json::Value::Number(x), serde_json::Value::Number(y)) => {
                let xf = x.as_f64().unwrap_or(f64::NAN);
                let yf = y.as_f64().unwrap_or(f64::NAN);
                assert!(
                    (xf - yf).abs() <= 1e-9 * xf.abs().max(1.0),
                    "float drift beyond tolerance: {x} vs {y}"
                );
            }
            (serde_json::Value::Array(x), serde_json::Value::Array(y)) => {
                assert_eq!(x.len(), y.len(), "array length drift");
                for (xi, yi) in x.iter().zip(y.iter()) {
                    assert_json_roundtrip_close(xi, yi);
                }
            }
            (serde_json::Value::Object(x), serde_json::Value::Object(y)) => {
                assert_eq!(x.len(), y.len(), "object key drift");
                for (k, xv) in x {
                    let yv = y.get(k).unwrap_or_else(|| panic!("missing key {k}"));
                    assert_json_roundtrip_close(yv, xv);
                }
            }
            _ => assert_eq!(a, b, "scalar drift"),
        }
    }

    #[test]
    fn public_dtos_roundtrip_and_reject_unknown_or_promoted_fields() {
        let result = assemble_family_decision_set(set_input());
        let json = serde_json::to_vec(&result).unwrap();
        let back: FamilyDecisionSetResult = serde_json::from_slice(&json).unwrap();
        // f32-derived gap values shift one ULP through the JSON roundtrip, so the
        // DTO comparison is float-tolerant instead of exact (ADR-051: approximate
        // output must not be asserted exactly).
        assert_json_roundtrip_close(
            &serde_json::to_value(&result).unwrap(),
            &serde_json::to_value(&back).unwrap(),
        );
        let mut value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        value["unexpected"] = true.into();
        assert!(serde_json::from_value::<FamilyDecisionSetResult>(value).is_err());
        assert!(serde_json::from_str::<PooledWinner>(r#""header""#).is_err());
    }
}
