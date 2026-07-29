//! Article structure types — ontology-aligned (D133).
//!
//! FaBiO + OpenAlex aligned types for the graph metadata and structure layers.
//! These replace the ad-hoc Keyword/Topic from v1 with curated OpenAlex concepts.

use crate::schema::{FieldType, NodeSchemaDef};
use serde::{Deserialize, Serialize};

// ─── Section (FaBiO: DocumentObject) ───

/// A structural section of a work (from GROBID TEI `<div><head>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub vid: crate::vid::Vid,
    pub title: String,
    pub level: u32,
    pub order: u32,
    pub text: String,
    pub work_vid: String,
}

pub struct SectionSchema;

impl NodeSchemaDef for SectionSchema {
    fn label(&self) -> &'static str {
        "Section"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("title", FieldType::String),
            ("level", FieldType::Integer),
            ("order", FieldType::Integer),
            ("work_vid", FieldType::String),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("text", FieldType::String),
            ("char_count", FieldType::Integer),
            // Phase 3 GNN readiness — section text embedding
            ("embedding", FieldType::Vector),
        ]
    }
}

// ─── Concept (DEPRECATED — OpenAlex Concepts replaced by Topics) ───

/// A deprecated OpenAlex Concept. Kept for historical audit but NOT used for
/// retrieval (retrieval_eligible=false). Replaced by Topic system.
/// See doc/ADHD-ONTOLOGY-RESEARCH.md (D134 epigenetic deprecation pattern).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub vid: crate::vid::Vid,
    pub label: String,
    pub level: u32,
    pub wikidata: Option<String>,
    pub openalex_id: Option<String>,
    pub works_count: u64,
    pub retrieval_eligible: bool, // always false for deprecated Concepts
}

pub struct ConceptSchema;

impl NodeSchemaDef for ConceptSchema {
    fn label(&self) -> &'static str {
        "Concept"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("label", FieldType::String),
            ("level", FieldType::Integer),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("wikidata", FieldType::String),
            ("openalex_id", FieldType::String),
            ("works_count", FieldType::Integer),
        ]
    }
}

// ─── Topic (OpenAlex Topic — live taxonomy, 4-level hierarchy) ───
//
// OpenAlex Topics replace deprecated Concepts. Hierarchy:
//   4 domains → 26 fields → 254 subfields → ~4,500 topics
// Each Work is assigned topics via citation clustering + LLM labeling.

/// An OpenAlex topic — grouped concept cluster (domain → field → subfield → topic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub vid: crate::vid::Vid,
    pub label: String,
    pub domain: Option<String>,
    pub field: Option<String>,
    pub subfield: Option<String>,
    pub openalex_id: Option<String>,
}

pub struct TopicSchema;

impl NodeSchemaDef for TopicSchema {
    fn label(&self) -> &'static str {
        "Topic"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("label", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("domain", FieldType::String),
            ("field", FieldType::String),
            ("subfield", FieldType::String),
            ("openalex_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

// ─── Category (arXiv category) ───

/// An arXiv category (cs.CL, cs.CV, stat.ML, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub vid: crate::vid::Vid,
    pub code: String,
    pub name: String,
    pub is_primary: bool,
}

pub struct CategorySchema;

impl NodeSchemaDef for CategorySchema {
    fn label(&self) -> &'static str {
        "Category"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("vid", FieldType::String),
            ("code", FieldType::String),
            ("is_primary", FieldType::Boolean),
        ]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("name", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

// ─── Author (FOAF: Person + PRO: author role) ───

/// A paper author (disambiguated via OpenAlex/ORCID when available).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub vid: crate::vid::Vid,
    pub name: String,
    pub orcid: Option<String>,
    pub openalex_id: Option<String>,
    pub works_count: u64,
}

pub struct AuthorSchema;

impl NodeSchemaDef for AuthorSchema {
    fn label(&self) -> &'static str {
        "Author"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("name", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("orcid", FieldType::String),
            ("openalex_id", FieldType::String),
            ("works_count", FieldType::Integer),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

// ─── Institution (FOAF: Organization) ───

/// A research institution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Institution {
    pub vid: crate::vid::Vid,
    pub name: String,
    pub country: Option<String>,
    pub ror: Option<String>,
    pub openalex_id: Option<String>,
}

pub struct InstitutionSchema;

impl NodeSchemaDef for InstitutionSchema {
    fn label(&self) -> &'static str {
        "Institution"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("name", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("country", FieldType::String),
            ("ror", FieldType::String),
            ("openalex_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
        ]
    }
}

// ─── Reference (FaBiO: BibliographicReference) ───

/// A citation entry from the reference list. May resolve to a Work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub vid: crate::vid::Vid,
    pub raw_text: String,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub resolved_work_vid: Option<String>,
}

pub struct ReferenceSchema;

impl NodeSchemaDef for ReferenceSchema {
    fn label(&self) -> &'static str {
        "Reference"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("raw_text", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("arxiv_id", FieldType::String),
            ("doi", FieldType::String),
            ("title", FieldType::String),
            ("resolved_work_vid", FieldType::String),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_schema() {
        let s = SectionSchema;
        assert_eq!(s.label(), "Section");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "work_vid"));
    }

    #[test]
    fn test_section_schema_has_embedding_field() {
        // Phase 3 GNN readiness: Section nodes must support vector embeddings
        // for section-level similarity search.
        let s = SectionSchema;
        let optional = s.optional_fields();
        let names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"embedding"),
            "SectionSchema must have 'embedding' field for GNN readiness, got: {names:?}"
        );
    }

    #[test]
    fn test_concept_schema() {
        let s = ConceptSchema;
        assert_eq!(s.label(), "Concept");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "level"));
    }

    #[test]
    fn test_topic_schema() {
        let s = TopicSchema;
        assert_eq!(s.label(), "Topic");
    }

    #[test]
    fn test_category_schema() {
        let s = CategorySchema;
        assert_eq!(s.label(), "Category");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "code"));
    }

    #[test]
    fn test_author_schema() {
        let s = AuthorSchema;
        assert_eq!(s.label(), "Author");
        assert!(s.optional_fields().iter().any(|(n, _)| *n == "orcid"));
    }

    #[test]
    fn test_institution_schema() {
        let s = InstitutionSchema;
        assert_eq!(s.label(), "Institution");
        assert!(s.optional_fields().iter().any(|(n, _)| *n == "ror"));
    }

    #[test]
    fn test_reference_schema() {
        let s = ReferenceSchema;
        assert_eq!(s.label(), "Reference");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "raw_text"));
    }
}
