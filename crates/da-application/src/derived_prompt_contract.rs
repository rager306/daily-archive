//! Closed, offline prompt-task contract derived from injected evidence metadata.
//!
//! This module owns DTO shape only in T01. It deliberately has no runtime, parse,
//! graph, promotion, import, or model execution capability.

use crate::derived_evidence_maturity::{FamilyId, MaturityState, ReceiptKind};
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
