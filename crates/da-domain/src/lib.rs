//! da-domain — pure domain types for daily-archive v2.
//!
//! Zero infrastructure dependencies. Only serde + chrono + sha2.
//! This is the single source of truth for all data shapes.
//!
//! ADR-037 §2: Domain layer depends on nothing (only std + serde).
//! ADR-038 §2: 5-module schema (A-E), 18 relation types.
//! ADR-040 §11: Schema enforcement lives here, not in DDL.

pub mod article;
pub mod entity;
pub mod evidence;
pub mod paper;
pub mod relation;
pub mod schema;
pub mod versioning;
pub mod vid;

pub use article::{
    Category, CategorySchema, Keyword, KeywordSchema, Section, SectionSchema, Topic, TopicSchema,
};
pub use entity::{Entity, EntitySchema, EntityType};
pub use evidence::{EpistemicStatus, EvidenceAssertion, EvidenceId, SourceSpan, SpanType};
pub use paper::{Paper, PaperSchema, PaperStatus};
pub use relation::{bibliographic, CitationSchema, Relation, RelationType, RELATION_TYPES};
pub use schema::{
    all_node_schemas, schema_for_label, Field, FieldType, NodeSchemaDef, SchemaError,
};
pub use versioning::{TemporalRecord, Versioned};
pub use vid::{entity_vid, paper_vid, Vid};
