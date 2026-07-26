//! Relation types (ADR-038 Module E — 18 types from Agents-K1).
//!
//! 7 causal types deferred (need GRPO/causal models, no GPU).
//! 18 adopted across 4 groups.

use crate::evidence::SourceSpan;
use crate::vid::Vid;
use serde::{Deserialize, Serialize};

/// Relation types adopted from Agents-K1 (18 of 25).
/// 7 causal types deferred: MOTIVATED_BY, HAS_PROPERTY, SUBSET_OF,
/// CAUSES, ENABLES, INHIBITS, MODULATES, CORRELATED_WITH.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationType {
    // Controlled (6 — GLiNER/header-detectable)
    BuildsOn,
    UsesComponent,
    AlternativeTo,
    Solves,
    AppliedTo,
    Targets,

    // Composition (5 — structural)
    UsesTechnique,
    ConsistsOf,
    Implements,
    Combines,
    Requires,

    // Methodological comparison (4 — upgrade-mode LLM)
    DerivedFrom,
    DiffersFrom,
    HasLimitation,
    AddressesProblem,

    // Citation argumentative (3 — from Module D CitationContext)
    Supports,
    Contrasts,
    Extends,
}

impl RelationType {
    pub fn as_str(&self) -> &'static str {
        use RelationType::*;
        match self {
            BuildsOn => "BUILDS_ON",
            UsesComponent => "USES_COMPONENT",
            AlternativeTo => "ALTERNATIVE_TO",
            Solves => "SOLVES",
            AppliedTo => "APPLIED_TO",
            Targets => "TARGETS",
            UsesTechnique => "USES_TECHNIQUE",
            ConsistsOf => "CONSISTS_OF",
            Implements => "IMPLEMENTS",
            Combines => "COMBINES",
            Requires => "REQUIRES",
            DerivedFrom => "DERIVED_FROM",
            DiffersFrom => "DIFFERS_FROM",
            HasLimitation => "HAS_LIMITATION",
            AddressesProblem => "ADDRESSES_PROBLEM",
            Supports => "SUPPORTS",
            Contrasts => "CONTRASTS",
            Extends => "EXTENDS",
        }
    }

    pub fn all() -> &'static [RelationType] {
        &RELATION_TYPES
    }
}

/// All 18 adopted relation types.
pub const RELATION_TYPES: [RelationType; 18] = [
    RelationType::BuildsOn,
    RelationType::UsesComponent,
    RelationType::AlternativeTo,
    RelationType::Solves,
    RelationType::AppliedTo,
    RelationType::Targets,
    RelationType::UsesTechnique,
    RelationType::ConsistsOf,
    RelationType::Implements,
    RelationType::Combines,
    RelationType::Requires,
    RelationType::DerivedFrom,
    RelationType::DiffersFrom,
    RelationType::HasLimitation,
    RelationType::AddressesProblem,
    RelationType::Supports,
    RelationType::Contrasts,
    RelationType::Extends,
];

/// A directed relation between two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub source_vid: Vid,
    pub target_vid: Vid,
    pub relation_type: RelationType,
    pub source_spans: Vec<SourceSpan>,
    pub confidence: f32,
    pub schema_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_18_relation_types() {
        assert_eq!(RELATION_TYPES.len(), 18);
    }

    #[test]
    fn test_relation_type_serialization() {
        let r = RelationType::AppliedTo;
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"APPLIED_TO\"");
        let back: RelationType = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn test_all_types_have_str() {
        for t in RelationType::all() {
            assert!(!t.as_str().is_empty());
        }
    }
}
