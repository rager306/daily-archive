//! Offline, redacted composition-root rehearsal for the native family gate.
//! The only application operation here is the public decision-set assembler.

use da_application::derived_evidence_maturity::FamilyId;
use da_application::native_family_decision_packages::{
    FamilyDecisionSetInput, FamilyDecisionSetResult, FamilyPackageOutcome,
    assemble_family_decision_set,
};
use da_application::native_family_promotion_gate::PromotionGateBlockReason;
use da_application::native_family_same_n_adapter::{
    FamilyIntegrityClaims, FamilySameNInput, InjectedProofRefs, NamedFamilyGap,
};
use da_application::{CriticObservation, CriticStatus, FrozenEvalCase};
use da_ports::{
    CandidateManifest, CandidateReceipt, LearningLineage, PromotionState, Sha256Digest,
};
use serde::{Deserialize, Serialize};
const PAPER_ID: &str = "synthetic-hub-family";
const JAVA_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDigest {
    pub schema_version: String,
    pub authority: AuthorityDigest,
    pub composition: CompositionDigest,
    pub families: Vec<FamilyDigest>,
    pub blocked_rehearsals: Vec<BlockedDigest>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityDigest {
    pub deployed: bool,
    pub import_eligible: bool,
    pub can_auto_promote: bool,
    pub this_wave_promotes: Option<String>,
    pub java_rollback_target: String,
    pub pooled_winner: String,
    pub independently_selectable: Vec<String>,
    pub decision_point_consults: Vec<String>,
    pub java_production_unchanged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionDigest {
    pub native_nodes: u64,
    pub java_nodes: u64,
    pub citation_names_total: u64,
    pub references_total: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyDigest {
    pub family: String,
    pub outcome: String,
    pub gate: String,
    pub binding_present: bool,
    pub visible_gap_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockedDigest {
    pub id: String,
    pub reason: PromotionGateBlockReason,
    pub field: String,
}

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).expect("fixed synthetic digest")
}

fn required_gap(family: FamilyId) -> &'static str {
    match family {
        FamilyId::Segmentation => "segmentation_not_independently_selectable",
        FamilyId::Header => "hub_native_accepted_header_vs_java_raw",
        FamilyId::Bibliography => "citation_names_not_biblstruct",
        FamilyId::Body => "body_shadow_java_fallback",
        FamilyId::Visual => "visual_unpooled_heads",
    }
}

fn family_input(family: FamilyId) -> FamilySameNInput {
    let name = family.as_str();
    let cases = (0..2)
        .map(|order| FrozenEvalCase {
            case_id: format!("{name}-case-{order}"),
            corpus: digest('c'),
            order,
            expected_digest: digest('e'),
        })
        .collect::<Vec<_>>();
    let rows = |producer: &str| {
        (0..2)
            .map(|order| CriticObservation {
                schema_version: 1,
                case_id: format!("{name}-case-{order}"),
                corpus: digest('c'),
                order,
                producer: producer.to_owned(),
                metric_one: 0.8,
                metric_two: 0.6,
                status: CriticStatus::Ok,
            })
            .collect::<Vec<_>>()
    };
    FamilySameNInput {
        family,
        paper_id: PAPER_ID.to_owned(),
        source_sha256: "a".repeat(64),
        artifact_sha256: "b".repeat(64),
        grobid_profile: "structural".to_owned(),
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
        trainer_producer: "sona-trainer".to_owned(),
        proof_refs: InjectedProofRefs {
            java_rollback_target: JAVA_ROLLBACK.to_owned(),
            java_rollback_proof_ref: format!("java-rollback-{name}"),
            architecture_quarantine_proof_ref: format!("architecture-{name}"),
        },
        integrity: FamilyIntegrityClaims::default(),
        named_gaps: vec![NamedFamilyGap {
            id: required_gap(family).to_owned(),
            family,
            summary: "synthetic bounded reviewer gap".to_owned(),
            must_remain_visible: true,
        }],
    }
}

fn input() -> FamilyDecisionSetInput {
    FamilyDecisionSetInput {
        paper_id: PAPER_ID.to_owned(),
        families: FamilyId::ALL
            .into_iter()
            .map(|family| (family, family_input(family)))
            .collect(),
        independently_selectable_widened: false,
        claimed_pooled_winner: None,
    }
}

fn expect_blocked(id: &str, mutate: impl FnOnce(&mut FamilyDecisionSetInput)) -> BlockedDigest {
    let mut value = input();
    mutate(&mut value);
    match assemble_family_decision_set(value) {
        FamilyDecisionSetResult::Blocked { reason, field } => BlockedDigest {
            id: id.to_owned(),
            reason,
            field,
        },
        FamilyDecisionSetResult::Ready { .. } => panic!("{id} unexpectedly assembled"),
    }
}

fn project(
    set: &da_application::native_family_decision_packages::FamilyDecisionSet,
) -> Vec<FamilyDigest> {
    FamilyId::ALL
        .into_iter()
        .map(|family| {
            let package = &set.packages[&family];
            let (outcome, gate) = match &package.outcome {
                FamilyPackageOutcome::ReadyReview { .. } => (
                    "ready_review".to_owned(),
                    package
                        .binding
                        .as_ref()
                        .map(|binding| match binding.gate {
                            da_application::native_family_promotion_gate::PromotionGateDerivation::HumanGateRequired { .. } => "human_gate_required",
                            _ => "unexpected",
                        })
                        .unwrap_or("unexpected"),
                ),
                FamilyPackageOutcome::Incomplete { .. } => ("incomplete".to_owned(), "not_applicable"),
                FamilyPackageOutcome::Blocked { .. } => ("blocked".to_owned(), "blocked"),
            };
            FamilyDigest {
                family: family.as_str().to_owned(),
                outcome,
                gate: gate.to_owned(),
                binding_present: package.binding.is_some(),
                visible_gap_ids: package.named_gaps.iter().map(|gap| gap.id.clone()).collect(),
            }
        })
        .collect()
}

/// Build the deterministic, redacted review digest without I/O.
///
/// This is public so an independent consumer can include this example as a
/// module without launching a nested Cargo process.
pub fn build_digest() -> ReviewDigest {
    let FamilyDecisionSetResult::Ready { set } = assemble_family_decision_set(input()) else {
        panic!("happy rehearsal did not assemble")
    };
    let mut blocked = vec![
        expect_blocked("S05-R01", |value| {
            value.claimed_pooled_winner = Some("header".to_owned())
        }),
        expect_blocked("S05-R02", |value| {
            value.independently_selectable_widened = true
        }),
        expect_blocked("S05-R03", |value| {
            value
                .families
                .get_mut(&FamilyId::Header)
                .unwrap()
                .named_gaps
                .clear()
        }),
        expect_blocked("S05-R04", |value| {
            value
                .families
                .get_mut(&FamilyId::Header)
                .unwrap()
                .integrity
                .unsafe_production_call = true
        }),
        expect_blocked("S05-R05", |value| {
            value
                .families
                .get_mut(&FamilyId::Visual)
                .unwrap()
                .integrity
                .claims_promoted = true
        }),
        expect_blocked("S05-R06", |value| {
            value.families.remove(&FamilyId::Visual);
        }),
    ];
    blocked.shrink_to_fit();
    assert_eq!(project(&set).len(), 5);
    assert_eq!(set.independently_selectable, ["header", "references"]);
    assert_eq!(
        set.pooled_winner,
        da_application::native_family_decision_packages::PooledWinner::Forbidden
    );
    ReviewDigest {
        schema_version: "native-family-review-digest.v1".to_owned(),
        authority: AuthorityDigest {
            deployed: set.deployed,
            import_eligible: set.import_eligible,
            can_auto_promote: da_application::native_family_promotion_gate::can_auto_promote(),
            this_wave_promotes: set.this_wave_promotes.clone(),
            java_rollback_target: set.java_rollback_target.clone(),
            pooled_winner: "forbidden".to_owned(),
            independently_selectable: set.independently_selectable.clone(),
            decision_point_consults: Vec::new(),
            java_production_unchanged: true,
        },
        composition: CompositionDigest {
            native_nodes: 20,
            java_nodes: 21,
            citation_names_total: 2198,
            references_total: 207,
        },
        families: project(&set),
        blocked_rehearsals: blocked,
    }
}

/// Render and round-trip the closed wire DTO before emitting it.
pub fn render_digest(digest: &ReviewDigest) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(digest)?;
    let _: ReviewDigest = serde_json::from_str(&json)?;
    Ok(json)
}

fn main() -> anyhow::Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("native_family_gate_rehearsal");
    println!("{}", render_digest(&build_digest())?);
    Ok(())
}
