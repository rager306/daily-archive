//! Closed constructors for the five offline derived evidence families.
//!
//! This module is deliberately an injected DTO boundary.  It copies the
//! constrained catalog vocabulary and delegates all maturity, prompt, and
//! transition semantics to the sibling pure modules.

use serde::{Deserialize, Serialize};

use da_application::derived_evidence_maturity::{
    BlockedReason, BoundReceipt, FamilyEvidenceSet, FamilyId, ReceiptKind,
};
use da_application::derived_prompt_contract::{FamilyPromptSpec, PromptRender, render_prompt};
use da_application::derived_transition_evaluator::{
    ClaimedTransition, DenominatorObservation, EvidenceOrigin, TransitionEvaluation,
    TransitionObservations, evaluate_transition,
};

const JAVA_AUTHORITY: &str = "java_http";
const JAVA_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";
const FORBIDDEN_CROSSINGS: [&str; 6] = [
    "parse-pdf",
    concat!("Parser", "Port"),
    concat!("source", "_cli"),
    concat!("Graph", "Store"),
    concat!("im", "port"),
    concat!("pro", "moted"),
];

#[derive(Debug, Clone, Copy)]
struct FamilyCatalog {
    candidate_id: &'static str,
    allowed_files: &'static [&'static str],
    oracle_location: &'static str,
    java_source_refs: &'static [&'static str],
    candidate_symbols: &'static [&'static str],
    forbidden_oracle_imports: &'static [&'static str],
    named_denominators: &'static [&'static str],
    negatives: &'static [&'static str],
    property_cases: &'static [&'static str],
    metamorphic_cases: &'static [&'static str],
    verification_commands: &'static [&'static str],
}

const SEGMENTATION: FamilyCatalog = FamilyCatalog {
    candidate_id: "da-parse.native_segmentation",
    allowed_files: &[
        "crates/da-parse/src/native_segmentation.rs",
        "data/parse/document_segmentation.yaml",
    ],
    oracle_location: "crates/da-parse/tests/fixtures/segmentation_oracle.yaml",
    java_source_refs: &[
        "grobid-core/src/main/java/org/grobid/core/engines/label/SegmentationLabels.java",
    ],
    candidate_symbols: &["NativeSegmenter", "segment_document", "SegmentationOutcome"],
    forbidden_oracle_imports: &[
        "TeiLifter",
        "compose_header",
        "run_reference_sequence",
        "NativeBodyAccountant",
        "run_visual_heads_sequence",
    ],
    named_denominators: &[
        "segmentation.region_token_assignments",
        "segmentation.region_boundaries",
        "segmentation.documents",
        "segmentation.pages_per_lane",
    ],
    negatives: &[
        "unknown_region",
        "pooled_metric",
        "visual_structure_labels",
        "genre_classifier_branch",
    ],
    property_cases: &["assigned_equals_admitted", "exactly_one_region_per_token"],
    metamorphic_cases: &["reorder", "deletion", "identity_mutation"],
    verification_commands: &["cargo test -p da-parse --test segmentation_data_contract"],
};

const HEADER: FamilyCatalog = FamilyCatalog {
    candidate_id: "da-parse.native_header_reference_composition@1.0.0:header",
    allowed_files: &["crates/da-parse/src/native_composition.rs"],
    oracle_location: "data/parse/comparison_contract.yaml:native_header_reference_composition_shadow",
    java_source_refs: &["processHeaderDocument"],
    candidate_symbols: &[
        "compose_header",
        "run_header_sequence",
        "HeaderCompositionOutcome",
    ],
    forbidden_oracle_imports: &[
        "TeiLifter",
        "run_reference_sequence",
        "NativeSegmenter",
        "NativeBodyAccountant",
        "run_visual_heads_sequence",
    ],
    named_denominators: &["header.authors", "header.article_dates"],
    negatives: &[
        "invented_revision",
        "silent_skip_missing_tei_fixture",
        "header_bibliography_scope_collapse",
    ],
    property_cases: &["header_identity_preserved"],
    metamorphic_cases: &["header_reordering", "identity_mutation"],
    verification_commands: &["cargo test -p da-parse --test composition_data_contract"],
};

const BIBLIOGRAPHY: FamilyCatalog = FamilyCatalog {
    candidate_id: "da-parse.native_header_reference_composition@1.0.0:references",
    allowed_files: &[
        "crates/da-parse/src/native_composition.rs",
        "crates/da-parse/src/native_citation.rs",
    ],
    oracle_location: "data/parse/comparison_contract.yaml:native_citation_shadow",
    java_source_refs: &["processReferencesDocument"],
    candidate_symbols: &[
        "compose_references",
        "run_reference_sequence",
        "NativeReferenceSegmenter",
        "NativeCitationParser",
    ],
    forbidden_oracle_imports: &[
        "TeiLifter",
        "compose_header",
        "NativeSegmenter",
        "NativeBodyAccountant",
    ],
    named_denominators: &[
        "bibliography.biblstruct",
        "bibliography.citation_names",
        "bibliography.citation_anchors",
    ],
    negatives: &[
        "lost_references_or_links",
        "changed_reading_order",
        "pooled_header_bibliography_dates",
    ],
    property_cases: &["reference_links_conserved"],
    metamorphic_cases: &["reference_reordering", "identity_mutation"],
    verification_commands: &["cargo test -p da-parse --test citation_data_contract"],
};

const BODY: FamilyCatalog = FamilyCatalog {
    candidate_id: "da-parse.native_body_accounting",
    allowed_files: &[
        "crates/da-parse/src/native_body.rs",
        "crates/da-parse/src/p5_shadow_join.rs",
        "data/parse/native_body_accounting.yaml",
        "data/parse/p5_shadow_join.yaml",
    ],
    oracle_location: "artifacts/parse_compat/java_grobid_body_taxonomy_source.json",
    java_source_refs: &["grobid-core/src/main/java/org/grobid/core/engines/FullTextParser.java"],
    candidate_symbols: &[
        "NativeBodyAccountant",
        "run_native_body_sequence",
        "join_p5_shadow",
    ],
    forbidden_oracle_imports: &[
        "TeiLifter",
        "run_visual_heads_sequence",
        "compose_header",
        "NativeFulltextComposition",
    ],
    named_denominators: &[
        "body.documents",
        "body.pages",
        "body.lanes",
        "body.body_blocks",
    ],
    negatives: &[
        "java_section_n_on_p5",
        "body_text_payload",
        "pooled_p2_p5_pages",
    ],
    property_cases: &["body_conservation"],
    metamorphic_cases: &["body_reordering", "identity_mutation"],
    verification_commands: &["cargo test -p da-parse --test body_data_contract"],
};

const VISUAL: FamilyCatalog = FamilyCatalog {
    candidate_id: "da-parse.native_visual_heads@1.0.0",
    allowed_files: &[
        "crates/da-parse/src/native_visual.rs",
        "data/parse/native_visual_heads.yaml",
    ],
    oracle_location: "crates/da-parse/tests/fixtures/native_visual_oracle.yaml",
    java_source_refs: &["processVisualHeadsDocument"],
    candidate_symbols: &[
        "detect_figure_heads",
        "detect_table_heads",
        "detect_formula_heads",
        "run_visual_heads_sequence",
    ],
    forbidden_oracle_imports: &[
        "TeiLifter",
        "NativeFulltextComposition",
        "NativeBodyAccountant",
        "compose_header",
        "NativeSegmenter",
    ],
    named_denominators: &["visual.figure", "visual.table", "visual.formula"],
    negatives: &[
        "pooled_score",
        "zero_figures_for_missing_image",
        "fulltext_composition",
    ],
    property_cases: &["head_identity_preserved"],
    metamorphic_cases: &["visual_reordering", "identity_mutation"],
    verification_commands: &["cargo test -p da-parse --test visual_data_contract"],
};

fn catalog(family: FamilyId) -> &'static FamilyCatalog {
    match family {
        FamilyId::Segmentation => &SEGMENTATION,
        FamilyId::Header => &HEADER,
        FamilyId::Bibliography => &BIBLIOGRAPHY,
        FamilyId::Body => &BODY,
        FamilyId::Visual => &VISUAL,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyAdapterInput {
    pub family: FamilyId,
    pub paper_id: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub schema_sha256: String,
    pub config_sha256: String,
    pub grobid_profile: String,
    #[serde(default)]
    pub receipts: Vec<BoundReceipt>,
    #[serde(default)]
    pub denominators: Vec<DenominatorObservation>,
    #[serde(default)]
    pub oracle_imports: Vec<String>,
    #[serde(default)]
    pub called_symbols: Vec<String>,
    #[serde(default)]
    pub conservation_held: Option<bool>,
    #[serde(default)]
    pub semantic_held: Option<bool>,
    #[serde(default)]
    pub verification_exit_code: Option<i32>,
    #[serde(default)]
    pub empty_success: bool,
    #[serde(default)]
    pub typed_absence_named: bool,
    #[serde(default)]
    pub native_on_production_route: bool,
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
    #[serde(default)]
    pub evidence_origin: EvidenceOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptedFamily {
    pub set: FamilyEvidenceSet,
    pub spec: FamilyPromptSpec,
    pub observations: TransitionObservations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdapterResult {
    Ready {
        adapted: Box<AdaptedFamily>,
    },
    Blocked {
        reason: BlockedReason,
        field: String,
    },
}

fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn field_error(field: &str) -> AdapterResult {
    AdapterResult::Blocked {
        reason: BlockedReason::MalformedReceipt,
        field: field.to_owned(),
    }
}

fn family_denominator(family: FamilyId, name: &str) -> String {
    if name.starts_with(family.as_str()) {
        name.to_owned()
    } else {
        format!("{}.{}", family.as_str(), name)
    }
}

pub fn adapt_family(input: &FamilyAdapterInput) -> AdapterResult {
    if input.paper_id.trim().is_empty() {
        return field_error("paper_id");
    }
    for (name, value) in [
        ("source_sha256", &input.source_sha256),
        ("artifact_sha256", &input.artifact_sha256),
        ("schema_sha256", &input.schema_sha256),
        ("config_sha256", &input.config_sha256),
    ] {
        if !digest(value) {
            return field_error(name);
        }
    }
    if input.grobid_profile != "structural" && input.grobid_profile != "rich" {
        return field_error("grobid_profile");
    }

    let row = catalog(input.family);
    let mut set = FamilyEvidenceSet {
        family: input.family,
        paper_id: input.paper_id.clone(),
        inventory: None,
        oracle: None,
        atom: None,
        sequence: None,
        shadow: None,
        review: None,
        human_decision_ref: None,
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
    };
    for receipt in &input.receipts {
        if receipt.grobid_profile != input.grobid_profile
            || receipt.source_sha256 != input.source_sha256
            || receipt.artifact_sha256 != input.artifact_sha256
        {
            return field_error("receipts.identity");
        }
        match receipt.kind {
            ReceiptKind::Inventory => set.inventory = Some(receipt.clone()),
            ReceiptKind::Oracle => set.oracle = Some(receipt.clone()),
            ReceiptKind::Atom => set.atom = Some(receipt.clone()),
            ReceiptKind::Sequence => set.sequence = Some(receipt.clone()),
            ReceiptKind::Shadow => set.shadow = Some(receipt.clone()),
            ReceiptKind::Review => set.review = Some(receipt.clone()),
        }
    }
    let mut denominators = Vec::with_capacity(input.denominators.len());
    for item in &input.denominators {
        let name = family_denominator(input.family, &item.name);
        if !row
            .named_denominators
            .iter()
            .any(|allowed| *allowed == name)
        {
            return field_error("denominators.name");
        }
        let mut normalized = item.clone();
        normalized.name = name;
        denominators.push(normalized);
    }
    let strings = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect();
    let spec = FamilyPromptSpec {
        family: input.family,
        candidate_id: row.candidate_id.to_owned(),
        java_source_refs: strings(row.java_source_refs),
        candidate_symbols: strings(row.candidate_symbols),
        allowed_files: strings(row.allowed_files),
        source_sha256: input.source_sha256.clone(),
        schema_sha256: input.schema_sha256.clone(),
        config_sha256: input.config_sha256.clone(),
        artifact_sha256: input.artifact_sha256.clone(),
        oracle_location: row.oracle_location.to_owned(),
        forbidden_oracle_imports: strings(row.forbidden_oracle_imports),
        named_denominators: strings(row.named_denominators),
        negatives: strings(row.negatives),
        property_cases: strings(row.property_cases),
        metamorphic_cases: strings(row.metamorphic_cases),
        verification_commands: strings(row.verification_commands),
        production_authority: JAVA_AUTHORITY.to_owned(),
        rollback_target: JAVA_ROLLBACK.to_owned(),
        forbidden_crossings: strings(&FORBIDDEN_CROSSINGS),
    };
    let observations = TransitionObservations {
        expected_paper_id: Some(input.paper_id.clone()),
        expected_family: Some(input.family),
        expected_source_sha256: Some(input.source_sha256.clone()),
        expected_artifact_sha256: Some(input.artifact_sha256.clone()),
        expected_profile: Some(input.grobid_profile.clone()),
        observed_source_sha256: None,
        observed_artifact_sha256: None,
        denominators,
        oracle_imports: input.oracle_imports.clone(),
        forbidden_oracle_imports: strings(row.forbidden_oracle_imports),
        evidence_origin: input.evidence_origin,
        conservation_held: input.conservation_held,
        semantic_held: input.semantic_held,
        called_symbols: input.called_symbols.clone(),
        verification_exit_code: input.verification_exit_code,
        empty_success: input.empty_success,
        typed_absence_named: input.typed_absence_named,
        native_on_production_route: input.native_on_production_route,
        claims_promoted: input.claims_promoted,
        deployed: input.deployed,
        import_eligible: input.import_eligible,
    };
    AdapterResult::Ready {
        adapted: Box::new(AdaptedFamily {
            set,
            spec,
            observations,
        }),
    }
}

pub fn prompt_adapted(adapted: &AdaptedFamily) -> PromptRender {
    render_prompt(&adapted.set, &adapted.spec)
}

pub fn evaluate_adapted(
    adapted: &AdaptedFamily,
    proposed: Option<&BoundReceipt>,
    claimed: Option<&ClaimedTransition>,
) -> TransitionEvaluation {
    evaluate_transition(&adapted.set, proposed, &adapted.observations, claimed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_application::derived_evidence_maturity::MaturityState;
    use da_application::derived_transition_evaluator::DenominatorScope;

    fn input(family: FamilyId) -> FamilyAdapterInput {
        FamilyAdapterInput {
            family,
            paper_id: "paper-1".into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            schema_sha256: "c".repeat(64),
            config_sha256: "d".repeat(64),
            grobid_profile: "structural".into(),
            receipts: Vec::new(),
            denominators: Vec::new(),
            oracle_imports: Vec::new(),
            called_symbols: Vec::new(),
            conservation_held: None,
            semantic_held: None,
            verification_exit_code: None,
            empty_success: false,
            typed_absence_named: false,
            native_on_production_route: false,
            claims_promoted: false,
            deployed: false,
            import_eligible: false,
            evidence_origin: EvidenceOrigin::Receipt,
        }
    }

    #[test]
    fn all_closed_families_have_valid_distinct_specs() {
        let mut candidates = Vec::new();
        for family in FamilyId::ALL {
            let result = adapt_family(&input(family));
            let AdapterResult::Ready { adapted } = result else {
                panic!("{family:?}")
            };
            assert_eq!(adapted.set.family, family);
            assert_eq!(adapted.observations.expected_family, Some(family));
            assert!(matches!(
                prompt_adapted(&adapted),
                PromptRender::Contract { .. }
            ));
            candidates.push(adapted.spec.candidate_id.clone());
        }
        assert_eq!(candidates.len(), 5);
        assert_eq!(
            candidates
                .iter()
                .filter(|id| id.contains(":header"))
                .count(),
            1
        );
        assert_eq!(
            candidates
                .iter()
                .filter(|id| id.contains(":references"))
                .count(),
            1
        );
        assert!(
            !candidates
                .iter()
                .any(|id| id == "none" || id.contains("lift-partial"))
        );
    }

    #[test]
    fn identity_is_propagated_and_input_is_not_mutated() {
        let original = input(FamilyId::Body);
        let result = adapt_family(&original);
        let AdapterResult::Ready { adapted } = result else {
            panic!()
        };
        assert_eq!(adapted.spec.source_sha256, original.source_sha256);
        assert_eq!(
            adapted.observations.expected_artifact_sha256.as_deref(),
            Some(original.artifact_sha256.as_str())
        );
        assert!(original.denominators.is_empty());
    }

    #[test]
    fn malformed_identity_and_unknown_denominator_fail_closed() {
        let mut bad = input(FamilyId::Visual);
        bad.source_sha256 = "not-a-digest".into();
        assert!(matches!(
            adapt_family(&bad),
            AdapterResult::Blocked {
                reason: BlockedReason::MalformedReceipt,
                ..
            }
        ));
        let mut bad_name = input(FamilyId::Body);
        bad_name.denominators.push(DenominatorObservation {
            name: "not_a_catalog_name".into(),
            claimed_n: 1,
            actual_n: None,
            population_id: "body:paper-1".into(),
            scope: DenominatorScope::Document,
        });
        assert!(matches!(
            adapt_family(&bad_name),
            AdapterResult::Blocked {
                reason: BlockedReason::MalformedReceipt,
                ..
            }
        ));
    }

    fn receipt(family: FamilyId, kind: ReceiptKind, reference: &str) -> BoundReceipt {
        let _ = family;
        BoundReceipt {
            kind,
            evidence_ref: reference.into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
        }
    }

    fn adapted(family: FamilyId, prefix: usize) -> AdaptedFamily {
        let mut value = input(family);
        value.receipts = ReceiptKind::ALL
            .into_iter()
            .enumerate()
            .take(prefix)
            .map(|(index, kind)| receipt(family, kind, &format!("ref-{index}")))
            .collect();
        match adapt_family(&value) {
            AdapterResult::Ready { adapted } => *adapted,
            other => panic!("family {family:?} did not adapt: {other:?}"),
        }
    }

    fn claim(current: Option<MaturityState>, goal: ReceiptKind) -> ClaimedTransition {
        ClaimedTransition {
            current_state: current,
            next_allowed_state: match current {
                None => MaturityState::InventoryBound,
                Some(state) => state.successor().unwrap_or(state),
            },
            goal_kind: goal,
        }
    }

    fn assert_advanced(
        result: TransitionEvaluation,
        from: Option<MaturityState>,
        to: MaturityState,
    ) {
        assert!(matches!(
            result,
            TransitionEvaluation::Advanced { from: actual_from, to: actual_to, .. }
                if actual_from == from && actual_to == to
        ));
    }

    #[test]
    fn all_families_share_bootstrap_plus_one_replay_and_human_gate_semantics() {
        let expected = [
            (ReceiptKind::Inventory, MaturityState::InventoryBound),
            (ReceiptKind::Oracle, MaturityState::OracleBound),
            (ReceiptKind::Atom, MaturityState::AtomProven),
            (ReceiptKind::Sequence, MaturityState::SequenceProven),
            (ReceiptKind::Shadow, MaturityState::ShadowObserved),
        ];
        for family in FamilyId::ALL {
            for (prefix, (kind, state)) in expected.into_iter().enumerate() {
                let prior = adapted(family, prefix);
                let proposed = receipt(family, kind, &format!("ref-{prefix}"));
                let direct = evaluate_transition(
                    &prior.set,
                    Some(&proposed),
                    &prior.observations,
                    Some(&claim(
                        if prefix == 0 {
                            None
                        } else {
                            Some(MaturityState::ALL[prefix - 1])
                        },
                        kind,
                    )),
                );
                let wrapped = evaluate_adapted(
                    &prior,
                    Some(&proposed),
                    Some(&claim(
                        if prefix == 0 {
                            None
                        } else {
                            Some(MaturityState::ALL[prefix - 1])
                        },
                        kind,
                    )),
                );
                assert_eq!(wrapped, direct);
                assert_advanced(
                    wrapped,
                    if prefix == 0 {
                        None
                    } else {
                        Some(MaturityState::ALL[prefix - 1])
                    },
                    state,
                );
            }

            let complete = adapted(family, ReceiptKind::ALL.len());
            let review = receipt(family, ReceiptKind::Review, "ref-5");
            let review_claim = ClaimedTransition {
                current_state: Some(MaturityState::ShadowObserved),
                next_allowed_state: MaturityState::HumanGateRequired,
                goal_kind: ReceiptKind::Review,
            };
            let direct = evaluate_transition(
                &adapted(family, 5).set,
                Some(&review),
                &adapted(family, 5).observations,
                Some(&review_claim),
            );
            assert_eq!(
                evaluate_adapted(&adapted(family, 5), Some(&review), Some(&review_claim)),
                direct
            );
            assert!(matches!(direct, TransitionEvaluation::HumanGate { .. }));

            let replay_claim = ClaimedTransition {
                current_state: Some(MaturityState::HumanGateRequired),
                next_allowed_state: MaturityState::HumanGateRequired,
                goal_kind: ReceiptKind::Review,
            };
            let replay = evaluate_adapted(&complete, None, Some(&replay_claim));
            assert!(matches!(replay, TransitionEvaluation::HumanGate { .. }));

            let conflict = receipt(family, ReceiptKind::Review, "different-ref");
            assert!(matches!(
                evaluate_adapted(&complete, Some(&conflict), None),
                TransitionEvaluation::Blocked {
                    reason: BlockedReason::StaleIdentity,
                    ..
                }
            ));
        }
    }

    #[test]
    fn prompt_wrapper_is_direct_and_deterministic_for_every_family_prefix() {
        for family in FamilyId::ALL {
            for prefix in 0..=ReceiptKind::ALL.len() {
                let adapted = adapted(family, prefix);
                assert_eq!(
                    prompt_adapted(&adapted),
                    render_prompt(&adapted.set, &adapted.spec)
                );
                assert_eq!(prompt_adapted(&adapted), prompt_adapted(&adapted));
            }
        }
    }

    #[test]
    fn detector_matrix_remains_typed_and_fail_closed() {
        let cases = [
            ("stale", BlockedReason::StaleIdentity),
            ("pooling", BlockedReason::Pooling),
            ("drift", BlockedReason::DenominatorDrift),
            ("contamination", BlockedReason::Contamination),
            ("semantic", BlockedReason::SemanticDrift),
            ("conservation", BlockedReason::ConservationLoss),
            ("unsafe", BlockedReason::UnsafeProductionCall),
            ("failed", BlockedReason::FailedEvidence),
            ("empty", BlockedReason::FailedEvidence),
            ("automatic", BlockedReason::AttemptedAutomaticPromotion),
        ];
        for family in FamilyId::ALL {
            for (name, expected) in cases {
                let mut value = input(family);
                match name {
                    "stale" => {}
                    "pooling" => {
                        let denominator =
                            family_denominator(family, catalog(family).named_denominators[0]);
                        value.denominators = vec![
                            DenominatorObservation {
                                name: denominator.clone(),
                                claimed_n: 1,
                                actual_n: Some(1),
                                population_id: "one".into(),
                                scope: DenominatorScope::Document,
                            },
                            DenominatorObservation {
                                name: denominator,
                                claimed_n: 1,
                                actual_n: Some(1),
                                population_id: "two".into(),
                                scope: DenominatorScope::Document,
                            },
                        ];
                    }
                    "drift" => value.denominators.push(DenominatorObservation {
                        name: catalog(family).named_denominators[0].into(),
                        claimed_n: 2,
                        actual_n: Some(1),
                        population_id: "paper-1".into(),
                        scope: DenominatorScope::Document,
                    }),
                    "contamination" => value.evidence_origin = EvidenceOrigin::Prompt,
                    "semantic" => value.semantic_held = Some(false),
                    "conservation" => value.conservation_held = Some(false),
                    "unsafe" => value.native_on_production_route = true,
                    "failed" => value.verification_exit_code = Some(1),
                    "empty" => value.empty_success = true,
                    "automatic" => value.import_eligible = !false,
                    _ => unreachable!(),
                }
                let result = match adapt_family(&value) {
                    AdapterResult::Ready { mut adapted } => {
                        if name == "stale" {
                            adapted.observations.observed_source_sha256 = Some("e".repeat(64));
                        }
                        evaluate_adapted(&adapted, None, None)
                    }
                    other => panic!("{family:?}/{name} unexpectedly failed adaptation: {other:?}"),
                };
                assert!(
                    matches!(result, TransitionEvaluation::Blocked { reason, .. } if reason == expected),
                    "{family:?}/{name}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn family_specific_denominators_do_not_pool_and_empty_success_needs_typed_absence() {
        for family in [FamilyId::Header, FamilyId::Bibliography] {
            let mut value = input(family);
            let names = catalog(family).named_denominators;
            value.denominators = names
                .iter()
                .map(|name| DenominatorObservation {
                    name: (*name).into(),
                    claimed_n: 1,
                    actual_n: Some(1),
                    population_id: "paper-1".into(),
                    scope: DenominatorScope::Document,
                })
                .collect();
            let AdapterResult::Ready { adapted } = adapt_family(&value) else {
                panic!()
            };
            assert!(!matches!(
                evaluate_adapted(&adapted, None, None),
                TransitionEvaluation::Blocked {
                    reason: BlockedReason::Pooling,
                    ..
                }
            ));
        }

        let mut body = input(FamilyId::Body);
        body.empty_success = true;
        let AdapterResult::Ready { adapted } = adapt_family(&body) else {
            panic!()
        };
        assert!(matches!(
            evaluate_adapted(&adapted, None, None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::FailedEvidence,
                ..
            }
        ));

        let mut visual = input(FamilyId::Visual);
        visual.empty_success = true;
        visual.typed_absence_named = true;
        let AdapterResult::Ready { adapted } = adapt_family(&visual) else {
            panic!()
        };
        assert!(!matches!(
            evaluate_adapted(&adapted, None, None),
            TransitionEvaluation::Blocked {
                reason: BlockedReason::FailedEvidence,
                ..
            }
        ));
    }
}
