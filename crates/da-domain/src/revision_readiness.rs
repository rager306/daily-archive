//! Pure revision-scoped agent-readiness observation and policy evaluation.
//! Readiness is derived evidence, never graph truth and never an LLM score.

use serde::{Deserialize, Serialize};

pub const BASIS_POINTS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessObservation {
    pub revision_vid: String,
    pub section_count: u32,
    pub grounded_summary_count: u32,
    pub summary_type_counts: std::collections::BTreeMap<String, u32>,
    pub manifestation_count: u32,
}

impl RevisionReadinessObservation {
    pub fn summary_coverage_bps(&self) -> u32 {
        if self.section_count == 0 {
            return 0;
        }
        let numerator = u64::from(self.grounded_summary_count.min(self.section_count))
            * u64::from(BASIS_POINTS);
        (numerator / u64::from(self.section_count)) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessPolicy {
    pub schema_version: String,
    pub min_sections: u32,
    pub min_manifestations: u32,
    pub min_summary_coverage_bps: u32,
    pub expected_summary_types: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionReadinessState {
    Ready,
    Partial,
    Blocked,
}

impl RevisionReadinessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Partial => "partial",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessAssessment {
    pub state: RevisionReadinessState,
    pub summary_coverage_bps: u32,
    pub blocker_codes: Vec<String>,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RevisionReadinessPolicyError {
    #[error("readiness policy schema_version must not be empty")]
    EmptySchemaVersion,
    #[error("min_sections must be greater than zero")]
    ZeroMinSections,
    #[error("min_manifestations must be greater than zero")]
    ZeroMinManifestations,
    #[error("min_summary_coverage_bps must be within 0..={BASIS_POINTS}")]
    InvalidCoverage,
    #[error("expected summary type must be lowercase snake_case: {0:?}")]
    InvalidSummaryType(String),
    #[error("duplicate expected summary type {0:?}")]
    DuplicateSummaryType(String),
}

impl RevisionReadinessPolicy {
    pub fn validate(&self) -> Result<(), RevisionReadinessPolicyError> {
        if self.schema_version.trim().is_empty() {
            return Err(RevisionReadinessPolicyError::EmptySchemaVersion);
        }
        if self.min_sections == 0 {
            return Err(RevisionReadinessPolicyError::ZeroMinSections);
        }
        if self.min_manifestations == 0 {
            return Err(RevisionReadinessPolicyError::ZeroMinManifestations);
        }
        if self.min_summary_coverage_bps > BASIS_POINTS {
            return Err(RevisionReadinessPolicyError::InvalidCoverage);
        }
        let mut seen = std::collections::BTreeSet::new();
        for summary_type in &self.expected_summary_types {
            if summary_type.is_empty()
                || !summary_type
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            {
                return Err(RevisionReadinessPolicyError::InvalidSummaryType(
                    summary_type.clone(),
                ));
            }
            if !seen.insert(summary_type.clone()) {
                return Err(RevisionReadinessPolicyError::DuplicateSummaryType(
                    summary_type.clone(),
                ));
            }
        }
        Ok(())
    }

    pub fn evaluate(
        &self,
        observation: &RevisionReadinessObservation,
    ) -> Result<RevisionReadinessAssessment, RevisionReadinessPolicyError> {
        self.validate()?;
        let mut blockers = Vec::new();
        if observation.section_count < self.min_sections {
            blockers.push("insufficient_sections".into());
        }
        if observation.manifestation_count < self.min_manifestations {
            blockers.push("missing_manifestation".into());
        }
        let coverage = observation.summary_coverage_bps();
        let mut diagnostics = Vec::new();
        if coverage < self.min_summary_coverage_bps {
            diagnostics.push("low_summary_coverage".into());
        }
        for summary_type in &self.expected_summary_types {
            if observation
                .summary_type_counts
                .get(summary_type)
                .copied()
                .unwrap_or(0)
                == 0
            {
                diagnostics.push(format!("missing_{summary_type}_summary"));
            }
        }
        let state = if !blockers.is_empty() {
            RevisionReadinessState::Blocked
        } else if !diagnostics.is_empty() {
            RevisionReadinessState::Partial
        } else {
            RevisionReadinessState::Ready
        };
        Ok(RevisionReadinessAssessment {
            state,
            summary_coverage_bps: coverage,
            blocker_codes: blockers,
            diagnostic_codes: diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn policy() -> RevisionReadinessPolicy {
        RevisionReadinessPolicy {
            schema_version: "revision-readiness.v1".into(),
            min_sections: 1,
            min_manifestations: 1,
            min_summary_coverage_bps: 8_000,
            expected_summary_types: vec!["methods".into(), "results".into()],
        }
    }

    fn observation() -> RevisionReadinessObservation {
        RevisionReadinessObservation {
            revision_vid: "a".repeat(64),
            section_count: 10,
            grounded_summary_count: 10,
            summary_type_counts: [("methods".into(), 1), ("results".into(), 1)]
                .into_iter()
                .collect(),
            manifestation_count: 2,
        }
    }

    #[test]
    fn ready_requires_hard_invariants_and_all_diagnostics() {
        let assessment = policy().evaluate(&observation()).unwrap();
        assert_eq!(assessment.state, RevisionReadinessState::Ready);
        assert_eq!(assessment.summary_coverage_bps, BASIS_POINTS);
        assert!(assessment.blocker_codes.is_empty());
        assert!(assessment.diagnostic_codes.is_empty());
    }

    #[test]
    fn missing_results_is_partial_not_invented_or_blocked() {
        let mut observation = observation();
        observation.summary_type_counts.remove("results");
        let assessment = policy().evaluate(&observation).unwrap();
        assert_eq!(assessment.state, RevisionReadinessState::Partial);
        assert_eq!(assessment.diagnostic_codes, ["missing_results_summary"]);
    }

    #[test]
    fn zero_evidence_is_blocked_with_explicit_diagnostics() {
        let observation = RevisionReadinessObservation {
            revision_vid: "a".repeat(64),
            section_count: 0,
            grounded_summary_count: 0,
            summary_type_counts: Default::default(),
            manifestation_count: 0,
        };
        let assessment = policy().evaluate(&observation).unwrap();
        assert_eq!(assessment.state, RevisionReadinessState::Blocked);
        assert_eq!(assessment.summary_coverage_bps, 0);
        assert_eq!(
            assessment.blocker_codes,
            ["insufficient_sections", "missing_manifestation"]
        );
        assert_eq!(
            assessment.diagnostic_codes,
            [
                "low_summary_coverage",
                "missing_methods_summary",
                "missing_results_summary"
            ]
        );
    }

    #[test]
    fn serde_roundtrip_preserves_contract() {
        let value = policy().evaluate(&observation()).unwrap();
        let bytes = serde_json::to_vec(&value).unwrap();
        let roundtrip: RevisionReadinessAssessment = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(roundtrip, value);
    }

    proptest! {
        #[test]
        fn coverage_is_bounded_and_never_panics(sections in any::<u32>(), summaries in any::<u32>()) {
            let observation = RevisionReadinessObservation {
                revision_vid: "a".repeat(64),
                section_count: sections,
                grounded_summary_count: summaries,
                summary_type_counts: Default::default(),
                manifestation_count: 0,
            };
            prop_assert!(observation.summary_coverage_bps() <= BASIS_POINTS);
        }
    }
}
