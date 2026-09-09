//! Closed DTO boundary for one native scholarly-parse family same-n dry-run.
//!
//! This module freezes injected metadata and failure vocabulary only.  It does
//! not evaluate rows, perform I/O, promote candidates, or write graph state.

use crate::derived_evidence_maturity::FamilyId;
use crate::native_family_promotion_gate::PromotionGateBlockReason;
use crate::parse_learning_evaluation::{CriticObservation, EvaluationError, FrozenEvalCase};
use da_ports::CandidateManifest;
use serde::{Deserialize, Serialize};

/// The operational Java rollback identity is deliberately separate from the
/// evaluator's prior-candidate digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InjectedProofRefs {
    pub java_rollback_target: String,
    pub java_rollback_proof_ref: String,
    pub architecture_quarantine_proof_ref: String,
}

/// Authority and integrity claims are copied, never healed, by later binding.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyIntegrityClaims {
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub pooling: bool,
    #[serde(default)]
    pub contamination: bool,
    #[serde(default)]
    pub known_gaps_elided: bool,
}

/// One bounded, source-family-bound reviewer-visible gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedFamilyGap {
    pub id: String,
    pub family: FamilyId,
    pub summary: String,
    pub must_remain_visible: bool,
}

pub const MAX_NAMED_GAP_ID_BYTES: usize = 128;
pub const MAX_NAMED_GAP_SUMMARY_BYTES: usize = 256;

/// Inputs for exactly one family. Rows are intentionally retained verbatim;
/// the canonical evaluator owns denominator, ordering, and metric semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilySameNInput {
    pub family: FamilyId,
    pub paper_id: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub grobid_profile: String,
    pub candidate: CandidateManifest,
    pub frozen_cases: Vec<FrozenEvalCase>,
    pub baseline_rows: Vec<CriticObservation>,
    pub candidate_rows: Vec<CriticObservation>,
    pub conservation_ok: bool,
    pub trainer_producer: String,
    pub proof_refs: InjectedProofRefs,
    #[serde(default)]
    pub integrity: FamilyIntegrityClaims,
    #[serde(default)]
    pub named_gaps: Vec<NamedFamilyGap>,
}

/// Closed validation result. There is no promoted/accepted terminal variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FamilySameNResult {
    Ready {
        family: FamilyId,
        paper_id: String,
    },
    Blocked {
        reason: PromotionGateBlockReason,
        field: String,
    },
}

/// T01's frozen canonical failure table. No evaluator error is silently lost.
pub const fn evaluation_error_reason(error: &EvaluationError) -> PromotionGateBlockReason {
    match error {
        EvaluationError::EmptyEvaluation => PromotionGateBlockReason::FailedEvidence,
        EvaluationError::InvalidOrder => PromotionGateBlockReason::InvalidOrder,
        EvaluationError::DenominatorMismatch => PromotionGateBlockReason::DenominatorMismatch,
        EvaluationError::MalformedRow => PromotionGateBlockReason::MalformedPackage,
        EvaluationError::NonFiniteMetric => PromotionGateBlockReason::NonfiniteMetric,
        EvaluationError::ProducerNotIndependent => PromotionGateBlockReason::ProducerNotIndependent,
        EvaluationError::CorpusMismatch => PromotionGateBlockReason::StaleIdentity,
        EvaluationError::ConservationVeto => PromotionGateBlockReason::ConservationVeto,
        EvaluationError::CandidateLockViolation => PromotionGateBlockReason::CandidateLockViolation,
    }
}

/// Validate one injected gap without inspecting or retaining raw source data.
pub fn validate_named_gap(family: FamilyId, gap: &NamedFamilyGap) -> Result<(), String> {
    if gap.id.trim().is_empty() {
        return Err("named_gaps.id".into());
    }
    if gap.id.len() > MAX_NAMED_GAP_ID_BYTES || has_forbidden_text(&gap.id) {
        return Err("named_gaps.id".into());
    }
    if gap.family != family {
        return Err("named_gaps.family".into());
    }
    if !gap.must_remain_visible {
        return Err("named_gaps.must_remain_visible".into());
    }
    if gap.summary.trim().is_empty() || gap.summary.len() > MAX_NAMED_GAP_SUMMARY_BYTES {
        return Err("named_gaps.summary".into());
    }
    if has_forbidden_text(&gap.summary) {
        return Err("named_gaps.summary".into());
    }
    Ok(())
}

/// Validate all gaps; an empty list is valid and no supplied gap is dropped.
pub fn validate_named_gaps(family: FamilyId, gaps: &[NamedFamilyGap]) -> Result<(), String> {
    for gap in gaps {
        validate_named_gap(family, gap)?;
    }
    Ok(())
}

fn has_forbidden_text(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    [
        "<tei",
        "</tei",
        "<xml",
        "</xml",
        "body_text",
        "pdf_bytes",
        "secret",
        "api_key",
        "apikey",
        "token",
        "password",
        "authorization",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || value.starts_with('/')
        || value.starts_with('\\')
        || lower.contains("://")
        || lower.contains("@localhost")
        || lower.contains("@127.0.0.1")
        || lower.contains("www.")
        || lower.split_whitespace().any(|word| {
            let candidate =
                word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-');
            candidate.contains('.')
                && candidate.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gap(family: FamilyId, summary: &str) -> NamedFamilyGap {
        NamedFamilyGap {
            id: "gap-1".into(),
            family,
            summary: summary.into(),
            must_remain_visible: true,
        }
    }

    #[test]
    fn dto_result_is_tagged_and_roundtrips() {
        let gap_value = gap(FamilyId::Header, "visible gap");
        let gap_json = serde_json::to_vec(&gap_value).unwrap();
        assert_eq!(
            serde_json::from_slice::<NamedFamilyGap>(&gap_json).unwrap(),
            gap_value
        );

        let values = [
            FamilySameNResult::Ready {
                family: FamilyId::Header,
                paper_id: "paper-1".into(),
            },
            FamilySameNResult::Blocked {
                reason: PromotionGateBlockReason::MalformedPackage,
                field: "named_gaps.summary".into(),
            },
        ];
        for value in values {
            let json = serde_json::to_value(&value).unwrap();
            assert!(json.get("status").is_some());
            assert_eq!(
                serde_json::from_value::<FamilySameNResult>(json).unwrap(),
                value
            );
        }
    }

    #[test]
    fn every_evaluation_error_has_a_stable_typed_mapping() {
        let errors = [
            (
                EvaluationError::EmptyEvaluation,
                PromotionGateBlockReason::FailedEvidence,
            ),
            (
                EvaluationError::InvalidOrder,
                PromotionGateBlockReason::InvalidOrder,
            ),
            (
                EvaluationError::DenominatorMismatch,
                PromotionGateBlockReason::DenominatorMismatch,
            ),
            (
                EvaluationError::MalformedRow,
                PromotionGateBlockReason::MalformedPackage,
            ),
            (
                EvaluationError::NonFiniteMetric,
                PromotionGateBlockReason::NonfiniteMetric,
            ),
            (
                EvaluationError::ProducerNotIndependent,
                PromotionGateBlockReason::ProducerNotIndependent,
            ),
            (
                EvaluationError::CorpusMismatch,
                PromotionGateBlockReason::StaleIdentity,
            ),
            (
                EvaluationError::ConservationVeto,
                PromotionGateBlockReason::ConservationVeto,
            ),
            (
                EvaluationError::CandidateLockViolation,
                PromotionGateBlockReason::CandidateLockViolation,
            ),
        ];
        for (error, expected) in errors {
            assert_eq!(evaluation_error_reason(&error), expected);
        }
    }

    #[test]
    fn gaps_allow_empty_and_valid_utf8_but_reject_untrusted_boundaries() {
        assert!(validate_named_gaps(FamilyId::Body, &[]).is_ok());
        assert!(validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, "visible gap")).is_ok());
        for bad in [
            "",
            "line\nbreak",
            "<TEI payload>",
            "/absolute/path",
            "https://example.invalid/secret",
            "api_key=hidden",
        ] {
            assert!(
                validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, bad)).is_err(),
                "{bad:?}"
            );
        }
        let mut too_long = "x".repeat(MAX_NAMED_GAP_SUMMARY_BYTES);
        too_long.push('x');
        assert!(validate_named_gap(FamilyId::Body, &gap(FamilyId::Body, &too_long)).is_err());
    }

    #[test]
    fn gaps_require_family_and_visibility() {
        let mut wrong_family = gap(FamilyId::Header, "gap");
        assert!(validate_named_gap(FamilyId::Body, &wrong_family).is_err());
        wrong_family.family = FamilyId::Body;
        wrong_family.must_remain_visible = false;
        assert!(validate_named_gap(FamilyId::Body, &wrong_family).is_err());
    }
}
