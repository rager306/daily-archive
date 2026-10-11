//! Governed-process record value (S10, ADR-055).
//!
//! One typed, inspectable artifact binding a single driven request's
//! **DATA route decision**, its **per-stage runtime admission codes** and its
//! **registry-bound prompt lineage**. The governor reads the same record shape
//! against the same live DATA and must agree (see the `governed_process_record`
//! governor check).
//!
//! Pure domain: serializable, no I/O, never writes graph truth. The record is
//! fail-closed candidate evidence — [`GovernedProcessRecord::validate`] refuses
//! any shape a reader could not resolve or any attempt to unlock
//! `import_eligible`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema id for [`GovernedProcessRecord`] artifacts.
pub const GOVERNED_PROCESS_RECORD_SCHEMA_VERSION: &str = "governed-process-record.v1";

/// Closed set of coarse admission codes a stage record may carry.
///
/// Derived from the S07 typed [`crate::assurance::AdmissionDecision`]:
/// - `admitted` — admitted and adjacent to the prior stage;
/// - `skipped` — admitted but carrying a typed skip reason for a forward skip;
/// - `blocked` — refused; the executor did not run.
///
/// Later/decorative names (`batch_admission`, `concurrency_tune`, ...) are not
/// admission codes and must never appear here.
pub const GOVERNED_ADMISSION_CODES: [&str; 3] = ["admitted", "blocked", "skipped"];

/// The DATA policy decision consulted for this request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyDecisionRecord {
    /// FSM `DecisionPoint` name (e.g. `profile_route`, `grammar_route`).
    pub decision_point: String,
    /// The resolved value the decision produced (e.g. `structural`).
    pub resolved_value: String,
    /// Rendering of `PolicySource`: `native` or `wasm:<sha256>`.
    pub policy_source: String,
    /// Fingerprint of the policy that produced the decision (may be empty for
    /// native fallback, matching the S07 policy stamp surface).
    pub policy_hash: String,
}

/// One per-stage runtime admission, carrying the S07 typed code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRecord {
    /// Stage identity the admission applies to.
    pub stage_id: String,
    /// Mirrors [`crate::assurance::AdmissionDecision::admitted`].
    pub admitted: bool,
    /// One of [`GOVERNED_ADMISSION_CODES`].
    pub code: String,
    /// Typed skip reason carried by an admitted forward layer skip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// Registry-bound prompt lineage for one prompt-bearing stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptLineageRecord {
    pub prompt_id: String,
    pub prompt_version: String,
    /// 64 lowercase hex fingerprint of the prompt template.
    pub prompt_sha256: String,
    /// 64 lowercase hex fingerprint of the output schema.
    pub schema_sha256: String,
}

/// One driven request's governed process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernedProcessRecord {
    pub article_id: String,
    pub policy_decisions: Vec<PolicyDecisionRecord>,
    pub admissions: Vec<AdmissionRecord>,
    pub prompt_lineages: Vec<PromptLineageRecord>,
    /// Candidate-only; an unlock attempt is refused by [`Self::validate`].
    pub import_eligible: bool,
}

/// Fail-closed validation refusal for [`GovernedProcessRecord`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GovernedProcessRecordError {
    #[error("governed process record article_id must not be empty")]
    EmptyArticleId,
    #[error("governed process record must carry at least one policy decision")]
    NoPolicyDecisions,
    #[error("governed process record admission {stage_id} carries unknown code {code}")]
    UnknownAdmissionCode { stage_id: String, code: String },
    #[error("governed process record prompt lineage identity is incomplete")]
    IncompleteLineage,
    #[error("governed process record prompt lineage fingerprint must be 64 lowercase hex")]
    InvalidFingerprint,
    #[error("governed process record must keep import_eligible=false")]
    ImportEligibleUnlock,
}

/// True when `value` is exactly 64 lowercase hexadecimal characters.
fn is_lowercase_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl GovernedProcessRecord {
    /// Structural, fail-closed validation.
    ///
    /// Refuses: an empty `article_id`; zero policy decisions; an admission code
    /// outside [`GOVERNED_ADMISSION_CODES`]; a prompt lineage with an empty
    /// identity or a non-64-lowercase-hex fingerprint; and any attempt to set
    /// `import_eligible = true`.
    pub fn validate(&self) -> Result<(), GovernedProcessRecordError> {
        if self.article_id.trim().is_empty() {
            return Err(GovernedProcessRecordError::EmptyArticleId);
        }
        if self.policy_decisions.is_empty() {
            return Err(GovernedProcessRecordError::NoPolicyDecisions);
        }
        for admission in &self.admissions {
            if !GOVERNED_ADMISSION_CODES.contains(&admission.code.as_str()) {
                return Err(GovernedProcessRecordError::UnknownAdmissionCode {
                    stage_id: admission.stage_id.clone(),
                    code: admission.code.clone(),
                });
            }
        }
        for lineage in &self.prompt_lineages {
            if lineage.prompt_id.trim().is_empty() || lineage.prompt_version.trim().is_empty() {
                return Err(GovernedProcessRecordError::IncompleteLineage);
            }
            if !is_lowercase_hex64(&lineage.prompt_sha256)
                || !is_lowercase_hex64(&lineage.schema_sha256)
            {
                return Err(GovernedProcessRecordError::InvalidFingerprint);
            }
        }
        if self.import_eligible {
            return Err(GovernedProcessRecordError::ImportEligibleUnlock);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex64(fill: char) -> String {
        fill.to_string().repeat(64)
    }

    fn policy_decision() -> PolicyDecisionRecord {
        PolicyDecisionRecord {
            decision_point: "profile_route".into(),
            resolved_value: "structural".into(),
            policy_source: "native".into(),
            policy_hash: hex64('a'),
        }
    }

    fn admission(code: &str) -> AdmissionRecord {
        AdmissionRecord {
            stage_id: "parse".into(),
            admitted: code != "blocked",
            code: code.into(),
            skip_reason: None,
        }
    }

    fn lineage() -> PromptLineageRecord {
        PromptLineageRecord {
            prompt_id: "figure_analysis".into(),
            prompt_version: "figure-analysis.v3".into(),
            prompt_sha256: hex64('b'),
            schema_sha256: hex64('c'),
        }
    }

    fn record() -> GovernedProcessRecord {
        GovernedProcessRecord {
            article_id: "2507.19457".into(),
            policy_decisions: vec![policy_decision()],
            admissions: vec![admission("admitted"), admission("skipped")],
            prompt_lineages: vec![lineage()],
            import_eligible: false,
        }
    }

    #[test]
    fn governed_process_record_accepts_closed_shape() {
        record().validate().unwrap();
    }

    #[test]
    fn governed_process_record_refuses_empty_article_id() {
        let mut r = record();
        r.article_id = "  ".into();
        assert_eq!(
            r.validate().unwrap_err(),
            GovernedProcessRecordError::EmptyArticleId
        );
    }

    #[test]
    fn governed_process_record_refuses_zero_policy_decisions() {
        let mut r = record();
        r.policy_decisions.clear();
        assert_eq!(
            r.validate().unwrap_err(),
            GovernedProcessRecordError::NoPolicyDecisions
        );
    }

    #[test]
    fn governed_process_record_closes_admission_codes() {
        for code in GOVERNED_ADMISSION_CODES {
            let mut r = record();
            r.admissions = vec![admission(code)];
            r.validate().unwrap();
        }
        let mut bad = record();
        bad.admissions = vec![admission("visual_bundle")];
        assert!(matches!(
            bad.validate().unwrap_err(),
            GovernedProcessRecordError::UnknownAdmissionCode { .. }
        ));
        // `head_fallback` is an unregistered later name, never an admission code.
        let mut later = record();
        later.admissions = vec![admission("head_fallback")];
        assert!(later.validate().is_err());
    }

    #[test]
    fn governed_process_record_refuses_incomplete_and_bad_lineage() {
        let mut empty_id = record();
        empty_id.prompt_lineages = vec![PromptLineageRecord {
            prompt_id: String::new(),
            ..lineage()
        }];
        assert_eq!(
            empty_id.validate().unwrap_err(),
            GovernedProcessRecordError::IncompleteLineage
        );

        let mut short = record();
        short.prompt_lineages = vec![PromptLineageRecord {
            prompt_sha256: "abc".into(),
            ..lineage()
        }];
        assert_eq!(
            short.validate().unwrap_err(),
            GovernedProcessRecordError::InvalidFingerprint
        );

        // Uppercase hex is refused, matching ContentDigest's lowercase contract.
        let mut upper = record();
        upper.prompt_lineages = vec![PromptLineageRecord {
            schema_sha256: "A".repeat(64),
            ..lineage()
        }];
        assert_eq!(
            upper.validate().unwrap_err(),
            GovernedProcessRecordError::InvalidFingerprint
        );
    }

    #[test]
    fn governed_process_record_lock_stays_false() {
        let mut unlocked = record();
        unlocked.import_eligible = true;
        assert_eq!(
            unlocked.validate().unwrap_err(),
            GovernedProcessRecordError::ImportEligibleUnlock
        );
    }

    #[test]
    fn governed_process_record_serde_round_trip() {
        use crate::roundtrip_json;
        roundtrip_json(&record());
        roundtrip_json(&GovernedProcessRecord {
            article_id: "2507.19457".into(),
            policy_decisions: vec![policy_decision()],
            admissions: vec![],
            prompt_lineages: vec![],
            import_eligible: false,
        });
    }

    #[test]
    fn governed_process_record_rejects_unknown_serde_field() {
        let yaml = r#"
article_id: "2507.19457"
policy_decisions: []
admissions: []
prompt_lineages: []
import_eligible: false
extra: 1
"#;
        assert!(serde_yaml::from_str::<GovernedProcessRecord>(yaml).is_err());
    }
}
