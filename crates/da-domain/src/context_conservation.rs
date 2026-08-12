//! Diagnostic stage-context projection for ADR-055 ACAP.
//!
//! This module has no admission verdict. Hard assurance remains owned by ACAP;
//! quality thresholds may only annotate these observations.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::assurance::{BoundaryMode, ContentDigest, MissingnessReason};

pub const BASIS_POINTS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextConservationPolicy {
    pub schema_version: String,
    pub policy_id: String,
    pub minimum_text_retention_bps: BTreeMap<String, u32>,
    pub minimum_section_grounding_bps: u32,
    pub maximum_navigation_noise_bps: u32,
    pub navigation_noise_titles: BTreeSet<String>,
}

impl ContextConservationPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "context-conservation-policy.v1"
            || self.policy_id.trim().is_empty()
            || self.minimum_text_retention_bps.is_empty()
            || self.navigation_noise_titles.is_empty()
        {
            return Err("context conservation policy is incomplete".into());
        }
        if self.minimum_section_grounding_bps > BASIS_POINTS
            || self.maximum_navigation_noise_bps > BASIS_POINTS
            || self
                .minimum_text_retention_bps
                .values()
                .any(|value| *value > BASIS_POINTS)
        {
            return Err("context conservation policy basis points exceed 10000".into());
        }
        if self
            .navigation_noise_titles
            .iter()
            .any(|value| value.trim().is_empty() || value != &value.to_ascii_lowercase())
        {
            return Err("navigation noise titles must be normalized lowercase".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageContextSnapshot {
    pub stage_id: String,
    pub mode: BoundaryMode,
    pub artifact_sha256: ContentDigest,
    pub source_bytes: u64,
    pub text_chars: u64,
    pub token_count: Option<u64>,
    pub tokenizer_lineage: Option<ContentDigest>,
    pub sections: u64,
    pub citations: u64,
    pub figures: u64,
    pub tables: u64,
    pub grounded_spans: u64,
    pub grounded_section_vids: u64,
    pub entities: u64,
    pub evidence_items: u64,
    pub summaries: u64,
    pub manifestations: u64,
    pub duplicate_section_titles: u64,
    pub navigation_noise_sections: u64,
    pub missingness: BTreeMap<String, MissingnessReason>,
    pub no_claim: Option<MissingnessReason>,
    pub import_eligible: bool,
}

impl StageContextSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.stage_id.trim().is_empty() {
            return Err("context snapshot requires stage_id".into());
        }
        if self.import_eligible {
            return Err("context observation cannot authorize import".into());
        }
        if self.token_count.is_some() != self.tokenizer_lineage.is_some() {
            return Err("token count requires exact tokenizer lineage".into());
        }
        if self.grounded_spans > self.entities || self.grounded_section_vids > self.sections {
            return Err("grounded context exceeds observed eligible context".into());
        }
        if self.missingness.keys().any(|code| code.trim().is_empty()) {
            return Err("missingness entries require nonempty codes".into());
        }
        if self.no_claim.is_some()
            && (self.text_chars > 0 || self.sections > 0 || self.entities > 0 || self.summaries > 0)
        {
            return Err("no_claim stage cannot publish observed semantic counts".into());
        }
        Ok(())
    }

    pub fn digest_json<T: Serialize>(value: &T) -> Result<ContentDigest, String> {
        serde_json::to_vec(value)
            .map(|bytes| ContentDigest::of_bytes(&bytes))
            .map_err(|error| format!("serialize context artifact: {error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatioObservation {
    pub name: String,
    pub numerator: u64,
    pub denominator: u64,
    pub basis_points: u32,
    pub mode: BoundaryMode,
    pub no_claim: Option<MissingnessReason>,
}

impl RatioObservation {
    pub fn measured(
        name: &str,
        numerator: u64,
        denominator: u64,
        mode: BoundaryMode,
    ) -> Result<Self, String> {
        if denominator == 0 {
            return Err(format!("ratio {name} cannot claim a zero denominator"));
        }
        let bounded = numerator.min(denominator);
        Ok(Self {
            name: name.into(),
            numerator,
            denominator,
            basis_points: ((bounded as u128 * u128::from(BASIS_POINTS)) / denominator as u128)
                as u32,
            mode,
            no_claim: None,
        })
    }

    pub fn no_claim(name: &str, mode: BoundaryMode, reason: MissingnessReason) -> Self {
        Self {
            name: name.into(),
            numerator: 0,
            denominator: 0,
            basis_points: 0,
            mode,
            no_claim: Some(reason),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self.no_claim {
            Some(_) if self.numerator == 0 && self.denominator == 0 && self.basis_points == 0 => {
                Ok(())
            }
            None if self.denominator > 0 && self.basis_points <= BASIS_POINTS => Ok(()),
            _ => Err(format!("ratio {} has invalid claim/denominator", self.name)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextTransition {
    pub from_stage: String,
    pub to_stage: String,
    pub text_retention: RatioObservation,
    pub section_retention: RatioObservation,
    pub citation_retention: RatioObservation,
    pub grounded_entities: RatioObservation,
    pub candidate_added: BTreeMap<String, u64>,
    pub authoritative_lost: BTreeMap<String, u64>,
    pub duplicate_or_navigation: BTreeMap<String, u64>,
}

impl ContextTransition {
    pub fn between(from: &StageContextSnapshot, to: &StageContextSnapshot) -> Result<Self, String> {
        from.validate()?;
        to.validate()?;
        let ratio = |name, numerator, denominator, mode| {
            if denominator == 0 {
                Ok(RatioObservation::no_claim(
                    name,
                    mode,
                    MissingnessReason::GenuinelyAbsent,
                ))
            } else {
                RatioObservation::measured(name, numerator, denominator, mode)
            }
        };
        let mut candidate_added = BTreeMap::new();
        if to.entities > from.entities {
            candidate_added.insert("entities".into(), to.entities - from.entities);
        }
        if to.evidence_items > from.evidence_items {
            candidate_added.insert(
                "evidence_items".into(),
                to.evidence_items - from.evidence_items,
            );
        }
        let mut authoritative_lost = BTreeMap::new();
        for (name, left, right) in [
            ("text_chars", from.text_chars, to.text_chars),
            ("sections", from.sections, to.sections),
            ("citations", from.citations, to.citations),
        ] {
            if left > right {
                authoritative_lost.insert(name.into(), left - right);
            }
        }
        let duplicate_or_navigation = BTreeMap::from([
            (
                "duplicate_section_titles".into(),
                to.duplicate_section_titles,
            ),
            (
                "navigation_noise_sections".into(),
                to.navigation_noise_sections,
            ),
        ]);
        Ok(Self {
            from_stage: from.stage_id.clone(),
            to_stage: to.stage_id.clone(),
            text_retention: ratio("text_retention", to.text_chars, from.text_chars, to.mode)?,
            section_retention: ratio("section_retention", to.sections, from.sections, to.mode)?,
            citation_retention: ratio("citation_retention", to.citations, from.citations, to.mode)?,
            grounded_entities: ratio("grounded_entities", to.grounded_spans, to.entities, to.mode)?,
            candidate_added,
            authoritative_lost,
            duplicate_or_navigation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(stage: &str, text: u64, sections: u64) -> StageContextSnapshot {
        StageContextSnapshot {
            stage_id: stage.into(),
            mode: BoundaryMode::NonAuthoritativeDiagnostic,
            artifact_sha256: ContentDigest::of_text(stage),
            source_bytes: 1,
            text_chars: text,
            token_count: None,
            tokenizer_lineage: None,
            sections,
            citations: 4,
            figures: 1,
            tables: 1,
            grounded_spans: 0,
            grounded_section_vids: 0,
            entities: 0,
            evidence_items: 0,
            summaries: 0,
            manifestations: 0,
            duplicate_section_titles: 0,
            navigation_noise_sections: 0,
            missingness: BTreeMap::new(),
            no_claim: None,
            import_eligible: false,
        }
    }
    #[test]
    fn transition_partitions_candidates_from_authoritative_loss() {
        let from = snapshot("parse", 100, 4);
        let mut to = snapshot("extract", 25, 3);
        to.entities = 3;
        to.grounded_spans = 2;
        let x = ContextTransition::between(&from, &to).unwrap();
        assert_eq!(x.text_retention.basis_points, 2500);
        assert_eq!(x.authoritative_lost["text_chars"], 75);
        assert_eq!(x.candidate_added["entities"], 3);
    }
    #[test]
    fn zero_denominator_is_no_claim_not_perfect_retention() {
        assert!(
            RatioObservation::measured("x", 0, 0, BoundaryMode::NonAuthoritativeDiagnostic)
                .is_err()
        );
        let x = RatioObservation::no_claim(
            "x",
            BoundaryMode::NonAuthoritativeDiagnostic,
            MissingnessReason::GenuinelyAbsent,
        );
        x.validate().unwrap();
    }
    #[test]
    fn tokens_require_lineage() {
        let mut x = snapshot("parse", 10, 1);
        x.token_count = Some(3);
        assert!(x.validate().is_err());
        x.tokenizer_lineage = Some(ContentDigest::of_text("tokenizer-v1"));
        x.validate().unwrap();
    }
    #[test]
    fn digest_and_serde_roundtrip_are_stable() {
        let x = snapshot("parse", 100, 4);
        assert_eq!(
            StageContextSnapshot::digest_json(&x).unwrap(),
            StageContextSnapshot::digest_json(&x).unwrap()
        );
        let y: StageContextSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&x).unwrap()).unwrap();
        assert_eq!(x, y);
    }
}
