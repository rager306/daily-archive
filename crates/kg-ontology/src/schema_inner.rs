//! Schema types for the universal ontology.
//!
//! Phase A: EpisodicNode definition (provenance ground truth) and
//! edge-type temporal classification. These are universal types that
//! work across all domains (scientific, legal, enterprise, code).
//!
//! Full YAML-driven schema loading lands in Phase D (ADR-050).

use std::collections::HashSet;

/// Source type for an EpisodicNode. Domain-agnostic — each project
/// maps its own source categories to these generic types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EpisodeSourceType {
    /// Unstructured or semi-structured text (paper, article, memo).
    Message,
    /// Plain text document.
    Text,
    /// JSON document (API response, structured record).
    Json,
    /// XML document (TEI, legal act, regulatory filing).
    Xml,
    /// Legal act (statute, regulation, decree).
    LegalAct,
    /// Court decision (ruling, judgment, order).
    CourtDecision,
    /// Custom source type for domain-specific extensions.
    Custom(String),
}

impl EpisodeSourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Message => "message",
            Self::Text => "text",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::LegalAct => "legal_act",
            Self::CourtDecision => "court_decision",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// EpisodicNode — provenance ground truth.
///
/// Every raw data source (paper, law, court decision, log entry) enters
/// the graph as an EpisodicNode. Temporal edges trace back to episodes
/// via `reference_time`, creating a bi-temporal provenance chain:
///   edge.valid_at → EpisodicNode.valid_at → source creation date.
///
/// This is the Graphiti EpisodicNode pattern adapted for kg-ontology.
#[derive(Debug, Clone)]
pub struct EpisodicNodeSchema {
    /// Stable identifier for the episode.
    pub vid: String,
    /// Source type (message, text, json, legal_act, court_decision).
    pub source_type: EpisodeSourceType,
    /// Human-readable source label.
    pub source_description: String,
    /// Raw content — never modified after creation.
    pub content: String,
    /// When the source was originally created (publication/decision date).
    pub valid_at: chrono::DateTime<chrono::Utc>,
    /// When the system ingested this episode.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Classification of edge types: temporal vs structural.
///
/// Temporal edges carry the 5-field TemporalEdge model and participate
/// in temporal resolution (invalidation). Structural edges describe
/// composition/provenance and carry only `created_at`.
///
/// This registry is data-driven (Phase D will load from YAML). Phase A
/// provides the type + a default registry matching common edge types.
#[derive(Debug, Clone, Default)]
pub struct EdgeTypeRegistry {
    temporal_edges: HashSet<String>,
}

impl EdgeTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry pre-populated with the default temporal edge
    /// types. Projects can add their own via `register_temporal()`.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        // Evidence plane
        reg.register_temporal("SUPPORTS");
        reg.register_temporal("REFUTES");
        reg.register_temporal("QUALIFIES");
        // Bibliographic
        reg.register_temporal("CITES");
        reg.register_temporal("MENTIONS");
        reg.register_temporal("REFERENCES");
        // Versioning / amendment
        reg.register_temporal("AMENDS");
        reg.register_temporal("REPEALS");
        reg.register_temporal("SUPERSEDES");
        reg.register_temporal("INVALIDATED_BY");
        // Decision plane (ADR-048)
        reg.register_temporal("CAUSED");
        reg.register_temporal("INFLUENCED");
        reg.register_temporal("PRECEDENT_FOR");
        reg
    }

    /// Register a new edge type as temporal.
    pub fn register_temporal(&mut self, edge_type: &str) {
        self.temporal_edges.insert(edge_type.to_string());
    }

    /// Is this edge type temporal (carries the 5-field model)?
    pub fn is_temporal(&self, edge_type: &str) -> bool {
        self.temporal_edges.contains(edge_type)
    }

    /// All registered temporal edge types.
    pub fn temporal_types(&self) -> &HashSet<String> {
        &self.temporal_edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_source_type_roundtrip() {
        assert_eq!(EpisodeSourceType::Message.as_str(), "message");
        assert_eq!(EpisodeSourceType::LegalAct.as_str(), "legal_act");
        assert_eq!(
            EpisodeSourceType::Custom("podcast".to_string()).as_str(),
            "podcast"
        );
    }

    #[test]
    fn test_edge_registry_defaults() {
        let reg = EdgeTypeRegistry::with_defaults();
        assert!(reg.is_temporal("SUPPORTS"));
        assert!(reg.is_temporal("CITES"));
        assert!(reg.is_temporal("AMENDS"));
        assert!(reg.is_temporal("CAUSED"));
        // Structural edges are NOT temporal
        assert!(!reg.is_temporal("HAS_PART"));
        assert!(!reg.is_temporal("FROM_SOURCE"));
        assert!(!reg.is_temporal("AUTHORED_BY"));
    }

    #[test]
    fn test_edge_registry_custom() {
        let mut reg = EdgeTypeRegistry::new();
        reg.register_temporal("CONTRACTED_WITH");
        assert!(reg.is_temporal("CONTRACTED_WITH"));
        assert!(!reg.is_temporal("SUPPORTS")); // not in custom registry
    }
}
