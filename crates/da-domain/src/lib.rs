//! da-domain — pure domain types for daily-archive v2.
//!
//! Zero infrastructure dependencies. Only serde + chrono + sha2.
//! This is the single source of truth for all data shapes.
//!
//! ADR-037 §2: Domain layer depends on nothing (only std + serde).
//! ADR-038 §2: 5-module schema (A-E), 18 relation types.
//! ADR-040 §11: Schema enforcement lives here, not in DDL.

pub mod vid;
pub mod entity;
pub mod evidence;
pub mod paper;
pub mod relation;
pub mod versioning;
pub mod schema;

pub use vid::{paper_vid, entity_vid, Vid};
pub use entity::{Entity, EntityType, EntitySchema};
pub use evidence::{EvidenceAssertion, SourceSpan, SpanType, EpistemicStatus, EvidenceId};
pub use paper::{Paper, PaperSchema, PaperStatus};
pub use relation::{Relation, RelationType, RELATION_TYPES};
pub use versioning::{Versioned, TemporalRecord};
pub use schema::{SchemaError, NodeSchemaDef, Field, FieldType};
