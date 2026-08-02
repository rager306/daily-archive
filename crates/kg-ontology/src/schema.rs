//! Schema trait and field types (ADR-040 §11.1, ADR-050).
//!
//! Universal schema-as-code framework: every node type implements
//! NodeSchemaDef, generating validate() and field metadata.
//! Extracted from da-domain in M289-u4fzx5/S02.

use std::collections::HashMap;

/// Schema validation error.
#[derive(Debug, thiserror::Error)]
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

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
        Self { name, field_type: FieldType::String, required: true, default: None }
    }
    pub fn integer(name: &'static str) -> Self {
        Self { name, field_type: FieldType::Integer, required: true, default: None }
    }
    pub fn float(name: &'static str) -> Self {
        Self { name, field_type: FieldType::Float, required: true, default: None }
    }
    pub fn boolean(name: &'static str) -> Self {
        Self { name, field_type: FieldType::Boolean, required: true, default: None }
    }
    pub fn datetime(name: &'static str) -> Self {
        Self { name, field_type: FieldType::DateTime, required: true, default: None }
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
        props: &HashMap<String, serde_json::Value>,
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSchema;
    impl NodeSchemaDef for TestSchema {
        fn label(&self) -> &'static str { "Test" }
        fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
            vec![("vid", FieldType::String)]
        }
    }

    #[test]
    fn test_field_type_as_str() {
        assert_eq!(FieldType::String.as_str(), "string");
        assert_eq!(FieldType::Integer.as_str(), "integer");
        assert_eq!(FieldType::Vector.as_str(), "vector");
    }

    #[test]
    fn test_field_type_display() {
        assert_eq!(format!("{}", FieldType::String), "string");
    }

    #[test]
    fn test_field_constructors() {
        assert_eq!(Field::string("name").field_type, FieldType::String);
        assert_eq!(Field::integer("count").field_type, FieldType::Integer);
        assert!(Field::string("x").required);
    }

    #[test]
    fn test_schema_validate_missing_required() {
        let schema = TestSchema;
        let props = HashMap::new();
        let result = schema.validate(&props);
        assert!(result.is_err());
        match result {
            Err(SchemaError::MissingRequired(name)) => assert_eq!(name, "vid"),
            _ => panic!("Expected MissingRequired"),
        }
    }

    #[test]
    fn test_schema_validate_present() {
        let schema = TestSchema;
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::Value::String("x".into()));
        assert!(schema.validate(&props).is_ok());
    }

    #[test]
    fn test_schema_error_display() {
        let e = SchemaError::MissingRequired("field1");
        assert!(format!("{e}").contains("field1"));
    }
}
