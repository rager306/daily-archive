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
            ("text_sha256", FieldType::String),
            // Phase 3 GNN readiness — section text embedding and replay identity
            ("embedding", FieldType::Vector),
            ("embedding_sha256", FieldType::String),
            // ADR-053 Layer A navigation fields (M365 S01 drift closure)
            ("path_id", FieldType::String),
            ("parent_section_id", FieldType::String),
            ("order_in_parent", FieldType::Integer),
            ("section_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
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
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
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
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
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
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("citation_count", FieldType::Integer),
            ("section_count", FieldType::Integer),
        ]
    }
}

// ─── Layer C: Typed entities (ADR-053, D194 Wave 1) ───
/// A method, technique, or approach mentioned in a paper.
/// First-class node label (not just Entity.type) so graph algorithms can
/// operate on type-homogeneous subgraphs (PageRank on Method graph, etc.).
pub struct MethodSchema;

impl NodeSchemaDef for MethodSchema {
    fn label(&self) -> &'static str {
        "Method"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("name", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("description", FieldType::String),
            ("paper_arxiv_id", FieldType::String),
            ("section_path_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

/// A dataset used or introduced in a paper.
pub struct DatasetSchema;

impl NodeSchemaDef for DatasetSchema {
    fn label(&self) -> &'static str {
        "Dataset"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("name", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("url", FieldType::String),
            ("description", FieldType::String),
            ("paper_arxiv_id", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

/// A metric or evaluation measure used in a paper.
pub struct MetricSchema;

impl NodeSchemaDef for MetricSchema {
    fn label(&self) -> &'static str {
        "Metric"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("name", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("description", FieldType::String),
            ("paper_arxiv_id", FieldType::String),
            ("value", FieldType::String),
            ("baseline_for", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

// ─── Layer B: Summary node (ADR-053, D194 Wave 1) ───
/// Multi-granularity digest of a Paper or Section.
/// Linked via hasSummary edge. Enables MCP get_summary without LLM calls.
pub struct SummarySchema;

impl NodeSchemaDef for SummarySchema {
    fn label(&self) -> &'static str {
        "Summary"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("text", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("granularity", FieldType::String),
            ("char_count", FieldType::Integer),
            ("source_chars", FieldType::Integer),
            ("source_text_sha256", FieldType::String),
            ("source_label", FieldType::String),
            ("source_id", FieldType::String),
            // PIR-007 (S02): durable dotted AST key on the Summary node so
            // downstream joins and S05 honesty resolve the source Section by
            // path instead of an invented ordinal. source_path_missingness
            // records the typed fallback reason when path_id was derived from
            // order/source_vid (empty = real dotted path present). Kept in
            // sync with data/ontology/node_schemas.yaml (YamlSchemaAdapter
            // DATA source).
            ("source_path_id", FieldType::String),
            ("source_path_missingness", FieldType::String),
            // PIR-012 / D243 (S02): Summary nodes are versioned, not
            // overwritten in place. `vid` is the content-addressed version VID
            // (`revision_summary_version_vid`); `logical_vid` is the stable
            // cross-stage join key (`revision_summary_vid`) shared by every
            // version of the same abstract/section digest. `valid_from` /
            // `valid_to` bound the version's valid-time interval, `recorded_at`
            // is when this version was written, `superseded_at` /
            // `superseded_by` close the interval when a rerun with different
            // content or policy replaced it (retrieval_eligible=false). Kept in
            // sync with data/ontology/node_schemas.yaml (YamlSchemaAdapter
            // DATA source). DateTime encoding stays the already-accepted
            // i64/RFC3339 pair — no validator tightening (S04 owns the wire).
            ("logical_vid", FieldType::String),
            ("valid_to", FieldType::DateTime),
            ("recorded_at", FieldType::DateTime),
            ("superseded_at", FieldType::DateTime),
            ("superseded_by", FieldType::String),
            ("revision_vid", FieldType::String),
            ("summary_type", FieldType::String),
            ("role_policy_schema_version", FieldType::String),
            ("role_policy_sha256", FieldType::String),
            ("method", FieldType::String),
            ("generated_at", FieldType::DateTime),
            ("prompt_task_id", FieldType::String),
            ("prompt_run_id", FieldType::String),
            ("prompt_task_sha256", FieldType::String),
            ("prompt_sha256", FieldType::String),
            ("output_schema_sha256", FieldType::String),
            ("route_policy_sha256", FieldType::String),
            ("model_binding_sha256", FieldType::String),
            ("model", FieldType::String),
            ("input_sha256", FieldType::String),
            ("output_sha256", FieldType::String),
            ("authoritative", FieldType::Boolean),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
        ]
    }
}

/// Structured quality evidence generated for a paper summary.
/// Quantitative results and reproducibility assessments remain typed signals,
/// rather than being hidden inside an unregistered ad-hoc node label.
pub struct QualitySignalSchema;

impl NodeSchemaDef for QualitySignalSchema {
    fn label(&self) -> &'static str {
        "QualitySignal"
    }
    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![("vid", FieldType::String), ("text", FieldType::String)]
    }
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![
            ("signal_type", FieldType::String),
            ("score", FieldType::Integer),
            ("source_label", FieldType::String),
            ("source_id", FieldType::String),
            ("revision_vid", FieldType::String),
            ("state", FieldType::String),
            ("blocker_codes", FieldType::String),
            ("diagnostic_codes", FieldType::String),
            ("section_count", FieldType::Integer),
            ("grounded_summary_count", FieldType::Integer),
            ("manifestation_count", FieldType::Integer),
            ("summary_coverage_bps", FieldType::Integer),
            ("summary_type_counts", FieldType::String),
            ("evidence_sha256", FieldType::String),
            ("policy_schema_version", FieldType::String),
            ("policy_sha256", FieldType::String),
            ("is_current", FieldType::Boolean),
            ("method", FieldType::String),
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
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
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
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
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("openalex_pending", FieldType::Boolean),
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
            ("retrieval_eligible", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
            ("valid_from", FieldType::DateTime),
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
    fn test_section_schema_has_nav_fields() {
        // ADR-053 Layer A (M365 S01): SectionSchema must expose nav fields.
        let s = SectionSchema;
        let optional = s.optional_fields();
        let names: Vec<&str> = optional.iter().map(|(n, _)| *n).collect();
        for f in [
            "path_id",
            "parent_section_id",
            "order_in_parent",
            "section_id",
        ] {
            assert!(
                names.contains(&f),
                "SectionSchema must have '{f}' field for ADR-053 Layer A, got: {names:?}"
            );
        }
    }

    #[test]
    fn test_concept_schema() {
        let s = ConceptSchema;
        assert_eq!(s.label(), "Concept");
        assert!(s.required_fields().iter().any(|(n, _)| *n == "level"));
    }

    #[test]
    fn test_typed_entity_schemas_exist() {
        // ADR-053 Layer C (D194 Wave 1): Method, Dataset, Metric must be
        // first-class node labels, not just Entity.type values.
        let method = MethodSchema;
        assert_eq!(method.label(), "Method");
        let dataset = DatasetSchema;
        assert_eq!(dataset.label(), "Dataset");
        let metric = MetricSchema;
        assert_eq!(metric.label(), "Metric");
        // Typed entities must have name required field
        assert!(
            MethodSchema
                .required_fields()
                .iter()
                .any(|(n, _)| *n == "name")
        );
        assert!(
            DatasetSchema
                .required_fields()
                .iter()
                .any(|(n, _)| *n == "name")
        );
        assert!(
            MetricSchema
                .required_fields()
                .iter()
                .any(|(n, _)| *n == "name")
        );
    }

    #[test]
    fn test_summary_schema_exists() {
        // ADR-053 Layer B (D194 Wave 1): Summary must be a first-class node
        // label so MCP get_summary can return multi-granularity digests.
        let s = SummarySchema;
        assert_eq!(s.label(), "Summary");
        let required: Vec<&str> = s.required_fields().iter().map(|(n, _)| *n).collect();
        assert!(
            required.contains(&"text"),
            "Summary must have 'text' required, got: {required:?}"
        );
        let optional: Vec<&str> = s.optional_fields().iter().map(|(n, _)| *n).collect();
        assert!(
            optional.contains(&"granularity"),
            "Summary must have 'granularity' optional, got: {optional:?}"
        );
        assert!(
            optional.contains(&"char_count"),
            "Summary must have 'char_count' optional"
        );
    }

    #[test]
    fn test_summary_schema_has_path_identity_fields() {
        // PIR-007 (S02): Summary carries a durable dotted source_path_id so
        // downstream joins and S05 honesty resolve the source Section by AST
        // path, plus the typed source_path_missingness reason when the path
        // was derived from order/source_vid fallback.
        let s = SummarySchema;
        let optional: Vec<&str> = s.optional_fields().iter().map(|(n, _)| *n).collect();
        for field in ["source_path_id", "source_path_missingness"] {
            assert!(
                optional.contains(&field),
                "Summary must have '{field}' optional, got: {optional:?}"
            );
        }
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
