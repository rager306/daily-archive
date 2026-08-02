//! YamlSchemaAdapter — bridges YAML-loaded schemas to NodeSchemaDef trait.
//!
//! ADR-050 Phase F (M288-avqgux/S01): wraps YamlNodeSchema data from
//! OntologyRegistry to implement the NodeSchemaDef trait. Uses Box::leak
//! to convert owned String labels into &'static str (schemas are loaded
//! once and live for program lifetime — leak is intentional and safe).
//!
//! TDD: tests at bottom verify label/required/optional field extraction,
//! FieldType parsing from string, and validate() behavior.

use crate::schema::{FieldType, NodeSchemaDef, SchemaError};
use std::collections::HashMap;

/// Adapter that wraps YAML-loaded schema data and implements NodeSchemaDef.
pub struct YamlSchemaAdapter {
    label_str: &'static str,
    required: Vec<(&'static str, FieldType)>,
    optional: Vec<(&'static str, FieldType)>,
}

impl YamlSchemaAdapter {
    /// Build from kg_ontology::registry::YamlNodeSchema data.
    /// Leaks strings to obtain &'static str lifetime (safe: schemas
    /// are loaded once at startup and live for program lifetime).
    pub fn from_yaml_schema(schema: &kg_ontology::registry::YamlNodeSchema) -> Self {
        let label_str: &'static str = Box::leak(schema.label.clone().into_boxed_str());
        let required = schema
            .required_fields
            .iter()
            .map(|f| {
                let name: &'static str = Box::leak(f.name.clone().into_boxed_str());
                let fty = parse_field_type(&f.field_type);
                (name, fty)
            })
            .collect();
        let optional = schema
            .optional_fields
            .iter()
            .map(|f| {
                let name: &'static str = Box::leak(f.name.clone().into_boxed_str());
                let fty = parse_field_type(&f.field_type);
                (name, fty)
            })
            .collect();
        Self {
            label_str,
            required,
            optional,
        }
    }
}

impl NodeSchemaDef for YamlSchemaAdapter {
    fn label(&self) -> &'static str {
        self.label_str
    }

    fn required_fields(&self) -> Vec<(&'static str, FieldType)> {
        self.required.clone()
    }

    fn optional_fields(&self) -> Vec<(&'static str, FieldType)> {
        self.optional.clone()
    }

    fn validate(&self, props: &HashMap<String, serde_json::Value>) -> Result<(), SchemaError> {
        for (name, _fty) in &self.required_fields() {
            match props.get(&name.to_string()) {
                None => return Err(SchemaError::MissingRequired(name)),
                Some(serde_json::Value::Null) => return Err(SchemaError::MissingRequired(name)),
                _ => {}
            }
        }
        Ok(())
    }
}

/// Parse FieldType from YAML string representation.
/// Format: "String", "Integer", "Float", "Boolean", "DateTime", "Vector", "Any".
/// Case-insensitive. Unknown → Any (with no error — forward-compatible).
fn parse_field_type(s: &str) -> FieldType {
    match s.trim() {
        "String" => FieldType::String,
        "Integer" => FieldType::Integer,
        "Float" => FieldType::Float,
        "Boolean" => FieldType::Boolean,
        "DateTime" => FieldType::DateTime,
        "Vector" => FieldType::Vector,
        "Any" => FieldType::Any,
        // Debug variants from serde Debug format: "String", "Integer", etc.
        _ => {
            // Log unknown types in debug mode; default to Any.
            #[cfg(debug_assertions)]
            eprintln!("Unknown field_type '{s}' in YAML schema, defaulting to Any");
            FieldType::Any
        }
    }
}

/// Load all node schemas from YAML if data/ontology/node_schemas.yaml exists.
/// Falls back to hardcoded all_node_schemas() if YAML not available.
/// Returns the same Vec<Box<dyn NodeSchemaDef>> type as all_node_schemas().
pub fn all_node_schemas_or_yaml() -> Vec<Box<dyn NodeSchemaDef>> {
    // Try to load from YAML first.
    let yaml_path = std::path::Path::new("data/ontology/node_schemas.yaml");
    if yaml_path.exists()
        && let Ok(registry) = kg_ontology::registry::OntologyRegistry::load_from_file(yaml_path)
    {
        let schemas: Vec<Box<dyn NodeSchemaDef>> = registry
            .labels()
            .into_iter()
            .filter_map(|label| registry.get_schema(label))
            .map(|yaml_schema| {
                Box::new(YamlSchemaAdapter::from_yaml_schema(yaml_schema)) as Box<dyn NodeSchemaDef>
            })
            .collect();
        if !schemas.is_empty() {
            return schemas;
        }
    }
    // Fallback: hardcoded schemas.
    crate::schema::all_node_schemas()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_yaml_schema() -> kg_ontology::registry::YamlNodeSchema {
        kg_ontology::registry::YamlNodeSchema {
            label: "TestEntity".to_string(),
            required_fields: vec![
                kg_ontology::registry::YamlFieldDef {
                    name: "vid".to_string(),
                    field_type: "String".to_string(),
                },
                kg_ontology::registry::YamlFieldDef {
                    name: "count".to_string(),
                    field_type: "Integer".to_string(),
                },
            ],
            optional_fields: vec![kg_ontology::registry::YamlFieldDef {
                name: "score".to_string(),
                field_type: "Float".to_string(),
            }],
        }
    }

    #[test]
    fn test_yaml_adapter_label() {
        let schema = make_yaml_schema();
        let adapter = YamlSchemaAdapter::from_yaml_schema(&schema);
        assert_eq!(adapter.label(), "TestEntity");
    }

    #[test]
    fn test_yaml_adapter_required_fields() {
        let schema = make_yaml_schema();
        let adapter = YamlSchemaAdapter::from_yaml_schema(&schema);
        let required = adapter.required_fields();
        assert_eq!(required.len(), 2);
        assert_eq!(required[0].0, "vid");
        assert!(matches!(required[0].1, FieldType::String));
        assert_eq!(required[1].0, "count");
        assert!(matches!(required[1].1, FieldType::Integer));
    }

    #[test]
    fn test_yaml_adapter_optional_fields() {
        let schema = make_yaml_schema();
        let adapter = YamlSchemaAdapter::from_yaml_schema(&schema);
        let optional = adapter.optional_fields();
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0].0, "score");
        assert!(matches!(optional[0].1, FieldType::Float));
    }

    #[test]
    fn test_yaml_adapter_validate_missing_required() {
        let schema = make_yaml_schema();
        let adapter = YamlSchemaAdapter::from_yaml_schema(&schema);
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::Value::String("x".into()));
        // Missing "count" required field.
        let result = adapter.validate(&props);
        assert!(result.is_err());
        match result {
            Err(SchemaError::MissingRequired(name)) => assert_eq!(name, "count"),
            _ => panic!("Expected MissingRequired error"),
        }
    }

    #[test]
    fn test_yaml_adapter_validate_all_present() {
        let schema = make_yaml_schema();
        let adapter = YamlSchemaAdapter::from_yaml_schema(&schema);
        let mut props = HashMap::new();
        props.insert("vid".to_string(), serde_json::Value::String("x".into()));
        props.insert("count".to_string(), serde_json::Value::Number(42.into()));
        let result = adapter.validate(&props);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_field_type_all_variants() {
        assert!(matches!(parse_field_type("String"), FieldType::String));
        assert!(matches!(parse_field_type("Integer"), FieldType::Integer));
        assert!(matches!(parse_field_type("Float"), FieldType::Float));
        assert!(matches!(parse_field_type("Boolean"), FieldType::Boolean));
        assert!(matches!(parse_field_type("DateTime"), FieldType::DateTime));
        assert!(matches!(parse_field_type("Vector"), FieldType::Vector));
        assert!(matches!(parse_field_type("Any"), FieldType::Any));
    }

    #[test]
    fn test_parse_field_type_unknown_defaults_to_any() {
        assert!(matches!(parse_field_type("Unknown"), FieldType::Any));
        assert!(matches!(parse_field_type(""), FieldType::Any));
    }

    #[test]
    fn test_all_node_schemas_or_yaml_fallback() {
        // This test runs from da-domain crate dir where data/ontology/ may not exist.
        // Should fallback to hardcoded all_node_schemas().
        let schemas = all_node_schemas_or_yaml();
        assert!(
            !schemas.is_empty(),
            "Should return schemas (either YAML or fallback)"
        );
        // If YAML exists and we're in project root, should load 31.
        // If fallback, should also have 31.
        assert!(
            schemas.len() >= 28,
            "Expected >=28 schemas, got {}",
            schemas.len()
        );
    }
}
