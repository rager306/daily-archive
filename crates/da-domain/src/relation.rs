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

/// Bibliographic edge types — structural metadata, NOT extracted semantic
/// relations. These are deterministic facts from the source document
/// (paper A cites paper B), not LLM/GLiNER-extracted relationships.
///
/// Separated from RelationType (ADR-038's 18 extracted types) to keep
/// bibliographic metadata distinct from semantic extraction results.
pub mod bibliographic {
    /// Paper A cites paper B (from GROBID parsed references).
    pub const CITES: &str = "CITES";

    /// Reverse of CITES: paper B is cited by paper A.
    pub const CITED_BY: &str = "CITED_BY";

    /// Paper A and paper B share an author.
    pub const CO_AUTHORED: &str = "CO_AUTHORED";

    /// Paper mentions an entity (extracted from paper text).
    /// Links Paper nodes to Entity nodes extracted from their sections.
    pub const MENTIONS: &str = "MENTIONS";

    /// Entity A is superseded by Entity B (merge scenario, D135).
    /// The superseded node gets retrieval_eligible=false.
    pub const SUPERSEDES: &str = "SUPERSEDES";

    /// Entity A was split into Entity B (split scenario, D135).
    pub const SPLITS: &str = "SPLITS";
}

/// Schema definition for Citation nodes (GRAPH-SCHEMA.md).
pub struct CitationSchema;

impl crate::schema::NodeSchemaDef for CitationSchema {
    fn label(&self) -> &'static str {
        "Citation"
    }

    fn required_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("vid", crate::schema::FieldType::String),
            ("valid_from", crate::schema::FieldType::DateTime),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        vec![
            ("arxiv_id", crate::schema::FieldType::String),
            ("title", crate::schema::FieldType::String),
            ("doi", crate::schema::FieldType::String),
            ("schema_version", crate::schema::FieldType::Integer),
            ("retrieval_eligible", crate::schema::FieldType::Boolean),
        ]
    }
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

    #[test]
    fn test_bibliographic_cites_constant() {
        // CITES is a bibliographic metadata edge, not an extracted RelationType.
        // It must be a stable string for graph edge labels.
        assert_eq!(bibliographic::CITES, "CITES");
        assert_eq!(bibliographic::CITED_BY, "CITED_BY");
        assert_eq!(bibliographic::CO_AUTHORED, "CO_AUTHORED");
        assert_eq!(bibliographic::MENTIONS, "MENTIONS");
        assert_eq!(bibliographic::SUPERSEDES, "SUPERSEDES");
        assert_eq!(bibliographic::SPLITS, "SPLITS");
    }
}
