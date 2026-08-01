//! da-domain — pure domain types for daily-archive v2.
//!
//! Zero infrastructure dependencies. Only serde + chrono + sha2.
//! This is the single source of truth for all data shapes.
//!
//! ADR-037 §2: Domain layer depends on nothing (only std + serde).
//! ADR-038 §2: 5-module schema (A-E), 18 relation types.
//! ADR-040 §11: Schema enforcement lives here, not in DDL.

pub mod article;
pub mod cluster;
pub mod domain;
pub mod entity;
pub mod eval;
pub mod evidence;
pub mod evidence_bundle;
pub mod healing;
pub mod hypergraph;
pub mod paper;
pub mod process;
pub mod relation;
pub mod scheduler;
pub mod schema;
pub mod source;
pub mod validator;
pub mod vid;
pub mod vocabulary;

pub use article::{
    Author, AuthorSchema, Category, CategorySchema, Concept, ConceptSchema, Institution,
    InstitutionSchema, Reference, ReferenceSchema, Section, SectionSchema, Topic, TopicSchema,
};
pub use entity::{Entity, EntitySchema, EntityType};
pub use eval::{ExtractionMetrics, GoldEntity, PredictedEntity};
pub use evidence::{EpistemicStatus, EvidenceAssertion, EvidenceId, SourceSpan, SpanType};
pub use healing::{
    CorrectResult, HealingActor, HealingOperation, MergeResult, ProvenanceEvent, SilenceResult,
};
pub use hypergraph::ConceptClusterSchema;
pub use paper::{Paper, PaperSchema, PaperStatus};
pub use relation::hypergraph::{
    CONTRADICTS, MEMBER_OF_CLUSTER, PARTICIPATES_IN, QUALIFIES, SUPPORTS,
};
pub use relation::{CitationSchema, RELATION_TYPES, Relation, RelationType, bibliographic};
pub use scheduler::{
    PendingTask, RetryPolicy, SchedulerTaskSchema, TaskPriority, TaskStatus, TaskType,
};
pub use schema::{
    Field, FieldType, NodeSchemaDef, SchemaError, all_node_schemas, schema_for_label,
};
pub use source::{
    SourceSchema, is_known_source_code, is_known_source_profile, is_known_source_type,
};
pub use vid::{Vid, entity_vid, paper_vid, reference_vid};
