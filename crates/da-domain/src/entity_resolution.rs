//! Ontology-driven entity-resolution blocking (ADR-053 Layer G).
//!
//! This module is pure candidate generation. It never writes graph state,
//! promotes candidates, or decides equivalence.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const BUNDLED_PROFILE: &str = include_str!("../../../data/ontology/entity_resolution.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationStep {
    Trim,
    CollapseWhitespace,
    Lowercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockingKeyKind {
    Exact,
    AlphanumericFold,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolutionDecisionProfile {
    pub category: String,
    pub scenario: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityResolutionProfile {
    pub version: String,
    pub auto_merge: bool,
    pub auto_promote: bool,
    pub same_type_only: bool,
    pub max_block_size: usize,
    pub normalization_steps: Vec<NormalizationStep>,
    pub blocking_keys: Vec<BlockingKeyKind>,
    pub decision: ResolutionDecisionProfile,
}

impl EntityResolutionProfile {
    pub fn load_project() -> Result<Self, String> {
        match std::fs::read_to_string("data/ontology/entity_resolution.yaml") {
            Ok(yaml) => Self::from_yaml(&yaml),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::bundled(),
            Err(error) => Err(format!("read entity resolution profile: {error}")),
        }
    }

    pub fn bundled() -> Result<Self, String> {
        Self::from_yaml(BUNDLED_PROFILE)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        let profile: Self = serde_yaml::from_str(yaml).map_err(|error| error.to_string())?;
        if profile.version != "1" {
            return Err(format!(
                "unsupported entity resolution profile version '{}'",
                profile.version
            ));
        }
        if profile.auto_merge || profile.auto_promote {
            return Err(
                "entity resolution profile must keep auto_merge and auto_promote false".into(),
            );
        }
        if !profile.same_type_only {
            return Err("entity resolution profile must keep same_type_only true".into());
        }
        if profile.max_block_size < 2 || profile.blocking_keys.is_empty() {
            return Err(
                "entity resolution profile needs max_block_size >= 2 and blocking keys".into(),
            );
        }
        if profile.decision.category.trim().is_empty()
            || profile.decision.scenario.trim().is_empty()
            || profile.decision.outcome.trim().is_empty()
        {
            return Err("entity resolution decision profile fields must be non-empty".into());
        }
        if !crate::process::is_known_decision_category(&profile.decision.category)
            || !crate::process::is_valid_decision_outcome(
                &profile.decision.category,
                &profile.decision.outcome,
            )
        {
            return Err("entity resolution decision profile violates decision vocabulary".into());
        }
        Ok(profile)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityResolutionInput {
    pub vid: String,
    pub entity_type: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockingKey {
    pub entity_type: String,
    pub kind: BlockingKeyKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionCandidate {
    pub left_vid: String,
    pub right_vid: String,
    pub shared_keys: Vec<BlockingKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OversizedBlock {
    pub key: BlockingKey,
    pub member_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockingResult {
    pub candidates: Vec<ResolutionCandidate>,
    pub oversized_blocks: Vec<OversizedBlock>,
}

pub fn normalize_label(label: &str, steps: &[NormalizationStep]) -> String {
    let mut value = label.to_string();
    for step in steps {
        value = match step {
            NormalizationStep::Trim => value.trim().to_string(),
            NormalizationStep::CollapseWhitespace => {
                value.split_whitespace().collect::<Vec<_>>().join(" ")
            }
            NormalizationStep::Lowercase => value.to_lowercase(),
        };
    }
    value
}

pub fn blocking_keys(
    entity: &EntityResolutionInput,
    profile: &EntityResolutionProfile,
) -> Vec<BlockingKey> {
    let normalized = normalize_label(&entity.label, &profile.normalization_steps);
    if entity.vid.trim().is_empty() || entity.entity_type.trim().is_empty() || normalized.is_empty()
    {
        return Vec::new();
    }
    profile
        .blocking_keys
        .iter()
        .filter_map(|kind| {
            let value = match kind {
                BlockingKeyKind::Exact => normalized.clone(),
                BlockingKeyKind::AlphanumericFold => normalized
                    .chars()
                    .filter(|character| character.is_alphanumeric())
                    .collect(),
            };
            (!value.is_empty()).then(|| BlockingKey {
                entity_type: entity.entity_type.clone(),
                kind: *kind,
                value,
            })
        })
        .collect()
}

pub fn pair_is_candidate(
    left: &EntityResolutionInput,
    right: &EntityResolutionInput,
    profile: &EntityResolutionProfile,
) -> bool {
    if left.vid == right.vid || left.entity_type != right.entity_type {
        return false;
    }
    let left_keys: BTreeSet<_> = blocking_keys(left, profile).into_iter().collect();
    blocking_keys(right, profile)
        .into_iter()
        .any(|key| left_keys.contains(&key))
}

pub fn block_candidates(
    entities: &[EntityResolutionInput],
    profile: &EntityResolutionProfile,
) -> BlockingResult {
    let mut blocks: BTreeMap<BlockingKey, BTreeSet<String>> = BTreeMap::new();
    for entity in entities {
        for key in blocking_keys(entity, profile) {
            blocks.entry(key).or_default().insert(entity.vid.clone());
        }
    }

    let mut pair_keys: BTreeMap<(String, String), BTreeSet<BlockingKey>> = BTreeMap::new();
    let mut oversized_blocks = Vec::new();
    for (key, members) in blocks {
        if members.len() > profile.max_block_size {
            oversized_blocks.push(OversizedBlock {
                key,
                member_count: members.len(),
            });
            continue;
        }
        let members: Vec<_> = members.into_iter().collect();
        for left in 0..members.len() {
            for right in (left + 1)..members.len() {
                pair_keys
                    .entry((members[left].clone(), members[right].clone()))
                    .or_default()
                    .insert(key.clone());
            }
        }
    }
    BlockingResult {
        candidates: pair_keys
            .into_iter()
            .map(|((left_vid, right_vid), shared_keys)| ResolutionCandidate {
                left_vid,
                right_vid,
                shared_keys: shared_keys.into_iter().collect(),
            })
            .collect(),
        oversized_blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(vid: &str, entity_type: &str, label: &str) -> EntityResolutionInput {
        EntityResolutionInput {
            vid: vid.into(),
            entity_type: entity_type.into(),
            label: label.into(),
        }
    }

    #[test]
    fn arbitrary_ontology_types_use_the_same_profile_without_hardcoding() {
        let profile = EntityResolutionProfile::bundled().unwrap();
        let result = block_candidates(
            &[
                entity("a", "Theorem", " Bayes'   Rule "),
                entity("b", "Theorem", "bayes' rule"),
                entity("c", "Method", "bayes' rule"),
            ],
            &profile,
        );
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].left_vid, "a");
        assert_eq!(result.candidates[0].right_vid, "b");
        assert_eq!(result.candidates[0].shared_keys.len(), 2);
    }

    #[test]
    fn alphanumeric_key_blocks_punctuation_variants() {
        let profile = EntityResolutionProfile::bundled().unwrap();
        let result = block_candidates(
            &[entity("a", "Model", "GPT-4"), entity("b", "Model", "gpt4")],
            &profile,
        );
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].shared_keys.len(), 1);
        assert_eq!(
            result.candidates[0].shared_keys[0].kind,
            BlockingKeyKind::AlphanumericFold
        );
    }

    #[test]
    fn unsafe_policy_and_oversized_blocks_fail_closed() {
        let unsafe_yaml = BUNDLED_PROFILE.replace("auto_merge: false", "auto_merge: true");
        assert!(EntityResolutionProfile::from_yaml(&unsafe_yaml).is_err());
        let mut profile = EntityResolutionProfile::bundled().unwrap();
        profile.max_block_size = 2;
        let result = block_candidates(
            &[
                entity("a", "Method", "x"),
                entity("b", "Method", "X"),
                entity("c", "Method", " x "),
            ],
            &profile,
        );
        assert!(result.candidates.is_empty());
        assert_eq!(result.oversized_blocks.len(), 2);
    }
}
