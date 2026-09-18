//! Closed, offline prompt-task contract derived from injected evidence metadata.
//!
//! This module owns DTO shape only in T01. It deliberately has no runtime, parse,
//! graph, promotion, import, or model execution capability.

use crate::derived_evidence_maturity::{
    BlockedReason, FamilyEvidenceSet, FamilyId, HumanAuthorization, MaturityDerivation,
    MaturityState, ReceiptKind, can_advance, derive_gate_maturity,
};
use serde::{Deserialize, Serialize};

/// Complete, injected family-specific metadata required to render one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyPromptSpec {
    pub family: FamilyId,
    pub candidate_id: String,
    pub java_source_refs: Vec<String>,
    pub candidate_symbols: Vec<String>,
    pub allowed_files: Vec<String>,
    pub source_sha256: String,
    pub schema_sha256: String,
    pub config_sha256: String,
    pub artifact_sha256: String,
    pub oracle_location: String,
    pub forbidden_oracle_imports: Vec<String>,
    pub named_denominators: Vec<String>,
    pub negatives: Vec<String>,
    pub property_cases: Vec<String>,
    pub metamorphic_cases: Vec<String>,
    pub verification_commands: Vec<String>,
    pub production_authority: String,
    pub rollback_target: String,
    pub forbidden_crossings: Vec<String>,
}

/// One bounded proof task. Its evidence references are existing receipt refs;
/// the task itself is never a receipt and has no singular evidence identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtomicPromptContract {
    pub family: FamilyId,
    pub paper_id: String,
    pub candidate_id: String,
    pub current_state: Option<MaturityState>,
    pub next_allowed_state: MaturityState,
    pub goal_kind: ReceiptKind,
    pub java_source_refs: Vec<String>,
    pub candidate_symbols: Vec<String>,
    pub allowed_files: Vec<String>,
    pub source_sha256: String,
    pub schema_sha256: String,
    pub config_sha256: String,
    pub artifact_sha256: String,
    pub oracle_location: String,
    pub forbidden_oracle_imports: Vec<String>,
    pub named_denominators: Vec<String>,
    pub negatives: Vec<String>,
    pub property_cases: Vec<String>,
    pub metamorphic_cases: Vec<String>,
    pub verification_commands: Vec<String>,
    pub production_authority: String,
    pub rollback_target: String,
    pub forbidden_crossings: Vec<String>,
    pub evidence_refs: Vec<String>,
}

impl AtomicPromptContract {
    /// Render structured verification commands without changing their source form.
    pub fn verification_block(&self) -> String {
        self.verification_commands.join("\n")
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn non_empty_vec(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| non_empty(value))
}

fn relative_repo_path(value: &str) -> bool {
    non_empty(value)
        && !value.starts_with('/')
        && !value.split('/').any(|component| component == "..")
}

fn malformed(field: impl Into<String>) -> PromptRender {
    PromptRender::Blocked {
        reason: BlockedReason::MalformedPrompt,
        missing_or_offending: Some(field.into()),
        evidence_refs: Vec::new(),
    }
}

fn validate_spec(set: &FamilyEvidenceSet, spec: &FamilyPromptSpec) -> Option<String> {
    if spec.family != set.family {
        return Some("family".into());
    }
    if !non_empty(&spec.candidate_id) {
        return Some("candidate_id".into());
    }
    if !non_empty_vec(&spec.java_source_refs) {
        return Some("java_source_refs".into());
    }
    if !non_empty_vec(&spec.candidate_symbols) {
        return Some("candidate_symbols".into());
    }
    if !spec
        .allowed_files
        .iter()
        .all(|path| relative_repo_path(path))
        || spec.allowed_files.is_empty()
    {
        return Some("allowed_files".into());
    }
    for (field, value) in [
        ("source_sha256", &spec.source_sha256),
        ("schema_sha256", &spec.schema_sha256),
        ("config_sha256", &spec.config_sha256),
        ("artifact_sha256", &spec.artifact_sha256),
    ] {
        if !valid_digest(value) {
            return Some(field.into());
        }
    }
    if !non_empty(&spec.oracle_location) {
        return Some("oracle_location".into());
    }
    for (field, values) in [
        ("forbidden_oracle_imports", &spec.forbidden_oracle_imports),
        ("named_denominators", &spec.named_denominators),
        ("negatives", &spec.negatives),
        ("property_cases", &spec.property_cases),
        ("metamorphic_cases", &spec.metamorphic_cases),
        ("verification_commands", &spec.verification_commands),
    ] {
        if !non_empty_vec(values) {
            return Some(field.into());
        }
    }
    if spec.production_authority != "java_http" {
        return Some("production_authority".into());
    }
    if spec.rollback_target != "java-grobid-http@0.9.0-crf" {
        return Some("rollback_target".into());
    }
    for required in [
        "parse-pdf",
        "ParserPort",
        "source_cli",
        "GraphStore",
        "import",
        "promoted",
    ] {
        if !spec
            .forbidden_crossings
            .iter()
            .any(|value| value == required)
        {
            return Some("forbidden_crossings".into());
        }
    }
    None
}

fn contract(
    set: &FamilyEvidenceSet,
    spec: &FamilyPromptSpec,
    current_state: Option<MaturityState>,
    next_allowed_state: MaturityState,
    goal_kind: ReceiptKind,
    evidence_refs: Vec<String>,
) -> PromptRender {
    PromptRender::Contract {
        contract: Box::new(AtomicPromptContract {
            family: set.family,
            paper_id: set.paper_id.clone(),
            candidate_id: spec.candidate_id.clone(),
            current_state,
            next_allowed_state,
            goal_kind,
            java_source_refs: spec.java_source_refs.clone(),
            candidate_symbols: spec.candidate_symbols.clone(),
            allowed_files: spec.allowed_files.clone(),
            source_sha256: spec.source_sha256.clone(),
            schema_sha256: spec.schema_sha256.clone(),
            config_sha256: spec.config_sha256.clone(),
            artifact_sha256: spec.artifact_sha256.clone(),
            oracle_location: spec.oracle_location.clone(),
            forbidden_oracle_imports: spec.forbidden_oracle_imports.clone(),
            named_denominators: spec.named_denominators.clone(),
            negatives: spec.negatives.clone(),
            property_cases: spec.property_cases.clone(),
            metamorphic_cases: spec.metamorphic_cases.clone(),
            verification_commands: spec.verification_commands.clone(),
            production_authority: spec.production_authority.clone(),
            rollback_target: spec.rollback_target.clone(),
            forbidden_crossings: spec.forbidden_crossings.clone(),
            evidence_refs,
        }),
    }
}

/// Render exactly one bounded next-proof task, or a typed non-action outcome.
///
/// The renderer is deterministic and only copies evidence references produced by
/// the composite maturity derivation; the prompt contract itself is never
/// evidence. The renderer has no approval input, so it evaluates the evidence
/// dimension against the unresolved default authorization: complete evidence
/// means the next action is a human gate, not an automated proof task.
pub fn render_prompt(set: &FamilyEvidenceSet, spec: &FamilyPromptSpec) -> PromptRender {
    if let Some(field) = validate_spec(set, spec) {
        return malformed(field);
    }

    match derive_gate_maturity(set, &HumanAuthorization::Pending) {
        MaturityDerivation::Blocked {
            reason,
            missing_or_offending,
            evidence_refs,
        } => {
            if reason == BlockedReason::MissingEvidence
                && missing_or_offending.as_deref() == Some("inventory")
                && evidence_refs.is_empty()
                && set.inventory.is_none()
            {
                return contract(
                    set,
                    spec,
                    None,
                    MaturityState::InventoryBound,
                    ReceiptKind::Inventory,
                    evidence_refs,
                );
            }
            PromptRender::Blocked {
                reason,
                missing_or_offending,
                evidence_refs,
            }
        }
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } => {
            if state == MaturityState::HumanGateRequired {
                return PromptRender::HumanGate { evidence_refs };
            }
            let Some(next) = state.successor() else {
                return malformed("next_allowed_state");
            };
            let Some(goal_kind) = (match state {
                MaturityState::InventoryBound => Some(ReceiptKind::Oracle),
                MaturityState::OracleBound => Some(ReceiptKind::Atom),
                MaturityState::AtomProven => Some(ReceiptKind::Sequence),
                MaturityState::SequenceProven => Some(ReceiptKind::Shadow),
                MaturityState::ShadowObserved => Some(ReceiptKind::Review),
                MaturityState::ReviewReady | MaturityState::HumanGateRequired => None,
            }) else {
                return malformed("goal_kind");
            };
            if !can_advance(state, next) {
                return malformed("next_allowed_state");
            }
            contract(set, spec, Some(state), next, goal_kind, evidence_refs)
        }
    }
}

/// Closed result of prompt rendering; only `Contract` contains executable work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptRender {
    Contract {
        contract: Box<AtomicPromptContract>,
    },
    Blocked {
        reason: crate::derived_evidence_maturity::BlockedReason,
        missing_or_offending: Option<String>,
        evidence_refs: Vec<String>,
    },
    HumanGate {
        evidence_refs: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derived_evidence_maturity::BoundReceipt;

    #[test]
    fn closed_prompt_schema_exposes_exact_wire_shape() {
        let spec = FamilyPromptSpec {
            family: FamilyId::Body,
            candidate_id: "candidate-1".into(),
            java_source_refs: vec!["src/Main.java".into()],
            candidate_symbols: vec!["BodyLifter".into()],
            allowed_files: vec!["src/Main.java".into()],
            source_sha256: "a".repeat(64),
            schema_sha256: "b".repeat(64),
            config_sha256: "c".repeat(64),
            artifact_sha256: "d".repeat(64),
            oracle_location: "oracle/MainTest.java".into(),
            forbidden_oracle_imports: vec!["TeiLifter".into()],
            named_denominators: vec!["body_sections".into()],
            negatives: vec!["empty_body".into()],
            property_cases: vec!["idempotent".into()],
            metamorphic_cases: vec!["reorder".into()],
            verification_commands: vec!["cargo test".into(), "cargo check".into()],
            production_authority: "java_http".into(),
            rollback_target: "java-grobid-http@0.9.0-crf".into(),
            forbidden_crossings: vec![
                "parse-pdf".into(),
                "ParserPort".into(),
                "source_cli".into(),
                "GraphStore".into(),
                "import".into(),
                "promoted".into(),
            ],
        };
        let contract = AtomicPromptContract {
            family: FamilyId::Body,
            paper_id: "paper-1".into(),
            candidate_id: spec.candidate_id.clone(),
            current_state: Some(MaturityState::ShadowObserved),
            next_allowed_state: MaturityState::ReviewReady,
            goal_kind: ReceiptKind::Review,
            java_source_refs: spec.java_source_refs.clone(),
            candidate_symbols: spec.candidate_symbols.clone(),
            allowed_files: spec.allowed_files.clone(),
            source_sha256: spec.source_sha256.clone(),
            schema_sha256: spec.schema_sha256.clone(),
            config_sha256: spec.config_sha256.clone(),
            artifact_sha256: spec.artifact_sha256.clone(),
            oracle_location: spec.oracle_location.clone(),
            forbidden_oracle_imports: spec.forbidden_oracle_imports.clone(),
            named_denominators: spec.named_denominators.clone(),
            negatives: spec.negatives.clone(),
            property_cases: spec.property_cases.clone(),
            metamorphic_cases: spec.metamorphic_cases.clone(),
            verification_commands: spec.verification_commands.clone(),
            production_authority: spec.production_authority.clone(),
            rollback_target: spec.rollback_target.clone(),
            forbidden_crossings: spec.forbidden_crossings.clone(),
            evidence_refs: vec!["receipt-1".into()],
        };
        assert_eq!(contract.verification_block(), "cargo test\ncargo check");
        let value = serde_json::to_value(&contract).unwrap();
        assert_eq!(value["current_state"], "shadow_observed");
        assert_eq!(value["next_allowed_state"], "review_ready");
        assert_eq!(value["goal_kind"], "review");
        assert!(value.get("evidence_ref").is_none());
        assert!(
            serde_json::from_value::<AtomicPromptContract>({
                let mut value = value.clone();
                value["unknown"] = true.into();
                value
            })
            .is_err()
        );
        assert!(
            serde_json::from_value::<PromptRender>(serde_json::json!({
                "status": "contract",
                "contract": value
            }))
            .is_ok()
        );
    }

    #[test]
    fn six_receipt_kinds_are_the_complete_goal_vocabulary() {
        assert_eq!(ReceiptKind::ALL.len(), 6);
        assert_eq!(
            ReceiptKind::ALL.map(ReceiptKind::as_str),
            [
                "inventory",
                "oracle",
                "atom",
                "sequence",
                "shadow",
                "review"
            ]
        );
        assert!(ReceiptKind::parse("promoted").is_none());
        assert!(MaturityState::parse("promoted").is_none());
    }

    fn receipt(kind: ReceiptKind, evidence_ref: &str) -> BoundReceipt {
        BoundReceipt {
            kind,
            evidence_ref: evidence_ref.into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
        }
    }

    fn evidence_set() -> FamilyEvidenceSet {
        FamilyEvidenceSet {
            family: FamilyId::Body,
            paper_id: "paper-1".into(),
            inventory: None,
            oracle: None,
            atom: None,
            sequence: None,
            shadow: None,
            review: None,
            human_decision_ref: None,
            freshness: None,
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

    fn prompt_spec() -> FamilyPromptSpec {
        FamilyPromptSpec {
            family: FamilyId::Body,
            candidate_id: "candidate-1".into(),
            java_source_refs: vec!["java/Body.java".into()],
            candidate_symbols: vec!["BodyLifter".into()],
            allowed_files: vec!["crates/da-application/src/lib.rs".into()],
            source_sha256: "a".repeat(64),
            schema_sha256: "b".repeat(64),
            config_sha256: "c".repeat(64),
            artifact_sha256: "d".repeat(64),
            oracle_location: "oracle/BodyTest.java".into(),
            forbidden_oracle_imports: vec!["TeiLifter".into()],
            named_denominators: vec!["body_sections".into()],
            negatives: vec!["empty_body".into()],
            property_cases: vec!["idempotent".into()],
            metamorphic_cases: vec!["reorder".into()],
            verification_commands: vec!["cargo test".into(), "cargo check".into()],
            production_authority: "java_http".into(),
            rollback_target: "java-grobid-http@0.9.0-crf".into(),
            forbidden_crossings: vec![
                "parse-pdf".into(),
                "ParserPort".into(),
                "source_cli".into(),
                "GraphStore".into(),
                "import".into(),
                "promoted".into(),
            ],
        }
    }

    fn set_prefix(prefix: usize) -> FamilyEvidenceSet {
        let mut set = evidence_set();
        for (index, kind) in ReceiptKind::ALL.into_iter().enumerate().take(prefix) {
            let value = Some(receipt(kind, &format!("ref-{index}")));
            match kind {
                ReceiptKind::Inventory => set.inventory = value,
                ReceiptKind::Oracle => set.oracle = value,
                ReceiptKind::Atom => set.atom = value,
                ReceiptKind::Sequence => set.sequence = value,
                ReceiptKind::Shadow => set.shadow = value,
                ReceiptKind::Review => set.review = value,
            }
        }
        set
    }

    fn contract_of(render: PromptRender) -> AtomicPromptContract {
        match render {
            PromptRender::Contract { contract } => *contract,
            other => panic!("expected contract, got {other:?}"),
        }
    }

    fn blocked_of(render: PromptRender) -> (BlockedReason, Option<String>, Vec<String>) {
        match render {
            PromptRender::Blocked {
                reason,
                missing_or_offending,
                evidence_refs,
            } => (reason, missing_or_offending, evidence_refs),
            other => panic!("expected block, got {other:?}"),
        }
    }

    #[test]
    fn renders_one_immediate_goal_for_each_nonterminal_prefix() {
        let expected = [
            (None, MaturityState::InventoryBound, ReceiptKind::Inventory),
            (
                Some(MaturityState::InventoryBound),
                MaturityState::OracleBound,
                ReceiptKind::Oracle,
            ),
            (
                Some(MaturityState::OracleBound),
                MaturityState::AtomProven,
                ReceiptKind::Atom,
            ),
            (
                Some(MaturityState::AtomProven),
                MaturityState::SequenceProven,
                ReceiptKind::Sequence,
            ),
            (
                Some(MaturityState::SequenceProven),
                MaturityState::ShadowObserved,
                ReceiptKind::Shadow,
            ),
            (
                Some(MaturityState::ShadowObserved),
                MaturityState::ReviewReady,
                ReceiptKind::Review,
            ),
        ];
        for (prefix, (current, next, goal)) in expected.into_iter().enumerate() {
            let contract = contract_of(render_prompt(&set_prefix(prefix), &prompt_spec()));
            assert_eq!(contract.current_state, current);
            assert_eq!(contract.next_allowed_state, next);
            assert_eq!(contract.goal_kind, goal);
            if let Some(current) = current {
                assert!(can_advance(current, next));
            }
        }
    }

    #[test]
    fn six_receipts_require_human_gate_and_never_emit_a_task() {
        let render = render_prompt(&set_prefix(ReceiptKind::ALL.len()), &prompt_spec());
        assert_eq!(
            render,
            PromptRender::HumanGate {
                evidence_refs: (0..6).map(|index| format!("ref-{index}")).collect(),
            }
        );
    }

    #[test]
    fn bootstrap_is_only_empty_inventory_block_that_becomes_a_contract() {
        let mut occupied = evidence_set();
        occupied.inventory = Some(receipt(ReceiptKind::Inventory, ""));
        assert_eq!(
            blocked_of(render_prompt(&occupied, &prompt_spec())),
            (
                BlockedReason::MissingEvidence,
                Some("inventory.evidence_ref".into()),
                vec![]
            )
        );

        let sparse = {
            let mut value = evidence_set();
            value.shadow = Some(receipt(ReceiptKind::Shadow, "shadow-ref"));
            value
        };
        assert_eq!(
            blocked_of(render_prompt(&sparse, &prompt_spec())).0,
            BlockedReason::LadderGap
        );
    }

    #[test]
    fn blocks_are_passed_through_and_malformed_spec_wins_first() {
        let mut flagged = set_prefix(2);
        flagged.pooling = true;
        assert_eq!(
            blocked_of(render_prompt(&flagged, &prompt_spec())).0,
            BlockedReason::Pooling
        );

        let mut malformed_spec = prompt_spec();
        malformed_spec.candidate_id.clear();
        let render = render_prompt(&set_prefix(2), &malformed_spec);
        assert_eq!(
            blocked_of(render),
            (
                BlockedReason::MalformedPrompt,
                Some("candidate_id".into()),
                vec![]
            )
        );

        let mut malformed_path = prompt_spec();
        malformed_path.allowed_files = vec!["/tmp/outside.rs".into()];
        assert_eq!(
            blocked_of(render_prompt(&set_prefix(1), &malformed_path)).0,
            BlockedReason::MalformedPrompt
        );
    }

    #[test]
    fn copies_only_derived_refs_and_is_deterministic() {
        let set = set_prefix(3);
        let spec = prompt_spec();
        let first = render_prompt(&set, &spec);
        let second = render_prompt(&set, &spec);
        assert_eq!(first, second);
        let contract = contract_of(first);
        assert_eq!(contract.evidence_refs, vec!["ref-0", "ref-1", "ref-2"]);
        assert_eq!(contract.candidate_id, "candidate-1");
        assert_eq!(contract.verification_block(), "cargo test\ncargo check");
    }

    #[test]
    fn authority_and_integrity_failures_never_become_tasks() {
        for flag in [
            "claims_promoted",
            "deployed",
            "import_eligible",
            "failed_evidence",
            "contamination",
        ] {
            let mut set = set_prefix(1);
            match flag {
                "claims_promoted" => set.claims_promoted = true,
                "deployed" => set.deployed = true,
                "import_eligible" => set.import_eligible = !false,
                "failed_evidence" => set.failed_evidence = true,
                "contamination" => set.contamination = true,
                _ => unreachable!(),
            }
            assert!(matches!(
                render_prompt(&set, &prompt_spec()),
                PromptRender::Blocked { .. }
            ));
        }
    }

    #[test]
    fn tagged_render_rejects_unknown_status_and_fields() {
        let unknown = serde_json::json!({"status":"not_a_status"});
        assert!(serde_json::from_value::<PromptRender>(unknown).is_err());
        let unknown_field = serde_json::json!({
            "status":"human_gate",
            "evidence_refs":[],
            "unexpected":true
        });
        assert!(serde_json::from_value::<PromptRender>(unknown_field).is_err());
    }
}
