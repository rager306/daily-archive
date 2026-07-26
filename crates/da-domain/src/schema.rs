//! Schema-as-code framework (ADR-040 §11.1).
//!
//! Schema enforcement lives in Rust types, not DDL.
//! Every node type implements NodeSchemaDef, generating:
//! - validate(): pre-write check
//! - to_properties(): Rust → PropertyValue map
//! - from_properties(): PropertyValue → Rust (with type checking)

use thiserror::Error;

/// Schema validation error.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Missing required field: {0}")]
    MissingRequired(&'static str),

    #[error("Type mismatch on field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },

    #[error("Invalid value for field '{field}': {reason}")]
    InvalidValue { field: &'static str, reason: String },
}

/// Declared field type for schema validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Vector,
    Any,
}

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Integer => "integer",
            FieldType::Float => "float",
            FieldType::Boolean => "boolean",
            FieldType::DateTime => "datetime",
            FieldType::Vector => "vector",
            FieldType::Any => "any",
        }
    }
}

/// A field in a schema definition.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: &'static str,
    pub field_type: FieldType,
    pub required: bool,
    pub default: Option<&'static str>,
}

impl Field {
    pub fn string(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::String,
            required: true,
            default: None,
        }
    }
    pub fn integer(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Integer,
            required: true,
            default: None,
        }
    }
    pub fn float(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Float,
            required: true,
            default: None,
        }
    }
    pub fn boolean(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::Boolean,
            required: true,
            default: None,
        }
    }
    pub fn datetime(name: &'static str) -> Self {
        Self {
            name,
            field_type: FieldType::DateTime,
            required: true,
            default: None,
        }
    }
}

/// Trait for schema definitions (ADR-040 §11.1).
/// Each node type implements this to define its schema.
pub trait NodeSchemaDef {
    /// The Cypher label for this node type.
    fn label(&self) -> &'static str;

    /// Required fields (must be present and non-null).
    fn required_fields(&self) -> Vec<(&'static str, FieldType)>;

    /// Optional fields (may be absent or null).
    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        vec![]
    }

    /// Validate a property map against this schema.
    fn validate(
        &self,
        props: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), SchemaError> {
        for (name, _fty) in self.required_fields() {
            match props.get(&name.to_string()) {
                None => return Err(SchemaError::MissingRequired(name)),
                Some(serde_json::Value::Null) => return Err(SchemaError::MissingRequired(name)),
                _ => {}
            }
        }
        Ok(())
    }
}

/// Current schema version (ADR-040 §11.2).
/// Incremented when schema changes require migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;
