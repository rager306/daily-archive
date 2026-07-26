//! da-graph — Samyama graph operations (queries, schema, algorithms).
//!
//! ADR-040 §8: da-graph provides Cypher query builders and schema helpers
//! for the Samyama knowledge graph. Uses da-ports::GraphStore (WARM path).
//!
//! This crate is the "graph domain logic" layer — it knows about Paper,
//! Author, Entity, Relation types and generates Cypher queries for them.
//! It does NOT depend on da-adapters (no Samyama SDK directly).

pub mod queries;
pub mod schema;

pub use queries::{PaperQueries, EntityQueries, RelationQueries};
pub use schema::{SchemaInitializer, SchemaVersion};
