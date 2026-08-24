//! Driven port: storage of schema-manifest objects (ADR-068, M451).
//!
//! GENERIC over object class: articles today, textbook chapters and
//! other schema-manifest objects later. The store answers three
//! questions the generic validation engine needs: list instances,
//! read one manifest, read a collection file / check file existence.

use da_domain::schema_invariants::{FileBackedChecks, ObjectNorm};

#[derive(Debug, thiserror::Error)]
pub enum SchemaObjectStoreError {
    #[error("schema-object store: IO failed: {0}")]
    Io(String),
    #[error("schema-object store: norm unparseable: {0}")]
    Norm(String),
    #[error("schema-object store: unknown object class {0:?}")]
    UnknownClass(String),
}

/// Read access to one class of schema-manifest objects.
pub trait SchemaObjectStore: Send + Sync {
    /// Class name, e.g. "article" — matches a path_config binding.
    fn object_class(&self) -> &str;

    /// The norm (parsed object schema) for this class.
    fn norm(&self) -> Result<ObjectNorm, SchemaObjectStoreError>;

    /// All instance ids (sorted; ids are directory names).
    fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError>;

    /// One manifest as raw YAML value (validator is schema-driven).
    fn read_object(
        &self,
        id: &str,
    ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError>;

    /// A collection file (JSON array of records) as YAML values.
    fn read_collection(
        &self,
        id: &str,
        rel_path: &str,
    ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError>;

    /// Does a file (relative to the object directory) exist?
    fn file_exists(&self, id: &str, rel_path: &str) -> Result<bool, SchemaObjectStoreError>;

    /// Raw file content (text) — for stages that consume layer files
    /// verbatim (projection, structural elements, …).
    fn read_raw(&self, id: &str, rel_path: &str) -> Result<Option<String>, SchemaObjectStoreError>;

    /// Bridge to the engine's file-backed pattern context. Adapters
    /// return their disk-backed implementation; the use case passes it
    /// to `validate_file_backed` (keeps the engine pure and the onion
    /// intact: application never touches the filesystem directly).
    fn file_backed_checks(&self) -> Box<dyn FileBackedChecks + '_>;
}
