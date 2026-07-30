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

/// Hypergraph edge types (ONTOLOGY-DESIGN Layer 6, ADR-042 revised).
///
/// IMPORTANT: ConceptCluster is a derived community object, NOT an evidence
/// unit. EvidenceBundle (future) is the source-grounded n-ary evidence node.
/// Do NOT run PPR over MEMBER_OF_CLUSTER edges — wrong semantics.
pub mod hypergraph {
    /// Entity is a member of a ConceptCluster (derived community membership).
    /// This is NOT evidence participation. For evidence, use PARTICIPATES_IN.
    pub const MEMBER_OF_CLUSTER: &str = "MEMBER_OF_CLUSTER";

    /// ConceptCluster A subsumes ConceptCluster B (hierarchical clusters).
    pub const SUBSUMES: &str = "SUBSUMES";

    /// Entity participates in an EvidenceBundle (n-ary evidence, future).
    /// Carries role: method, dataset, metric, baseline, result, etc.
    pub const PARTICIPATES_IN: &str = "PARTICIPATES_IN";

    /// EvidenceBundle supports a Claim (truth-bearing proposition, future).
    /// Target is Claim node, NOT Entity.
    pub const SUPPORTS: &str = "SUPPORTS";

    /// EvidenceBundle contradicts a Claim (future).
    pub const CONTRADICTS: &str = "CONTRADICTS";

    /// EvidenceBundle qualifies a Claim (partial support, future).
    pub const QUALIFIES: &str = "QUALIFIES";
}

/// Research Process Plane edge types (ADR-043, PROCESS-SCHEMA-P0 §15).
///
/// Cross-cutting edges for the execution-grounded research process.
/// These connect process kernel nodes (ResearchProblem, ResearchEnvironment,
/// ResearchIdea, Hypothesis, Intervention, ExperimentRun, etc.).
///
/// Invariants enforced by schema, not by edge strings:
///   - FailureEvent NEVER has REFUTES → Hypothesis
///   - Only ResultComparison → Hypothesis (SUPPORTS/REFUTES/QUALIFIES)
///   - Claim asserting experimental effectiveness requires VALID_UNDER
pub mod process {
    // ─── Environment / problem ───
    pub const SEEKS_SOLUTION_IN: &str = "SEEKS_SOLUTION_IN";
    pub const HAS_SUBPROBLEM: &str = "HAS_SUBPROBLEM";
    pub const DEFINES_BASELINE: &str = "DEFINES_BASELINE";
    pub const USES_DATASET: &str = "USES_DATASET";
    pub const USES_METRIC: &str = "USES_METRIC";
    pub const FOLLOWS_PROTOCOL: &str = "FOLLOWS_PROTOCOL";
    pub const RUNS: &str = "RUNS";
    pub const VALIDATES: &str = "VALIDATES";
    pub const VALID_UNDER: &str = "VALID_UNDER";

    // ─── Idea / hypothesis / lineage ───
    pub const FORMALIZES: &str = "FORMALIZES";
    pub const OPENS: &str = "OPENS";
    pub const VARIANT_OF: &str = "VARIANT_OF";
    pub const REFINES: &str = "REFINES";
    pub const COMBINES: &str = "COMBINES";
    pub const GENERALIZES: &str = "GENERALIZES";
    pub const REJECTS: &str = "REJECTS";
    pub const REDISCOVERS: &str = "REDISCOVERS";
    pub const INSPIRED_BY: &str = "INSPIRED_BY";
    pub const DECOMPOSES_INTO: &str = "DECOMPOSES_INTO";
    pub const HAS_INTERVENTION: &str = "HAS_INTERVENTION";
    pub const TESTED_BY: &str = "TESTED_BY";
    pub const TESTS: &str = "TESTS";
    pub const SUPPORTS: &str = "SUPPORTS";
    pub const REFUTES: &str = "REFUTES";
    pub const QUALIFIES: &str = "QUALIFIES";

    // ─── Intervention / bundle ───
    pub const HAS_COMPONENT: &str = "HAS_COMPONENT";
    pub const TARGETS_COMPONENT: &str = "TARGETS_COMPONENT";
    pub const APPLIES: &str = "APPLIES";
    pub const REMOVES_COMPONENT: &str = "REMOVES_COMPONENT"; // P1 ablation
    pub const ESTIMATES_CONTRIBUTION_OF: &str = "ESTIMATES_CONTRIBUTION_OF"; // P1

    // ─── Attempt / artifact / run ───
    pub const ATTEMPTS: &str = "ATTEMPTS";
    pub const PRODUCES: &str = "PRODUCES";
    pub const FAILED_WITH: &str = "FAILED_WITH";
    pub const IMPLEMENTS: &str = "IMPLEMENTS";
    pub const PARENT_OF: &str = "PARENT_OF";
    pub const EXECUTES: &str = "EXECUTES";
    pub const FROM_ARTIFACT: &str = "FROM_ARTIFACT";

    // ─── Observation / comparison ───
    pub const FROM_DEFINITION: &str = "FROM_DEFINITION";
    pub const MEASURED_BY: &str = "MEASURED_BY";
    pub const OBSERVED_AS: &str = "OBSERVED_AS";
    pub const COMPARES_CANDIDATE: &str = "COMPARES_CANDIDATE";
    pub const COMPARES_BASELINE: &str = "COMPARES_BASELINE";
    pub const GROUNDS: &str = "GROUNDS";
    pub const SUPPORTED_BY: &str = "SUPPORTED_BY";

    // ─── Failure ───
    pub const OCCURRED_DURING: &str = "OCCURRED_DURING";
    pub const LIMITS_EXECUTABILITY_OF: &str = "LIMITS_EXECUTABILITY_OF";

    // ─── Replication ───
    pub const REPLICATES: &str = "REPLICATES";

    // ─── Literature bridge ───
    pub const DESCRIBED_IN: &str = "DESCRIBED_IN";
    pub const APPROXIMATES: &str = "APPROXIMATES"; // ExperimentSetup → ResearchEnvironment
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
