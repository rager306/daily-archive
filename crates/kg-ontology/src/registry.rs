//! OntologyRegistry — loads schema definitions from YAML files.
//!
//! ADR-050 Phase D: schema definitions live as DATA in YAML, not
//! hardcoded in Rust. This module reads data/ontology/node_schemas.yaml
//! and provides lookup by label.
//!
//! Created in M285-z09px0/S07.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// One field definition loaded from YAML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YamlFieldDef {
    pub name: String,
    pub field_type: String,
}

/// One node schema loaded from YAML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YamlNodeSchema {
    pub label: String,
    #[serde(default)]
    pub required_fields: Vec<YamlFieldDef>,
    #[serde(default)]
    pub optional_fields: Vec<YamlFieldDef>,
}

/// OntologyRegistry — in-memory index of node schemas loaded from YAML.
///
/// # Example
/// ```no_run
/// use kg_ontology::registry::OntologyRegistry;
/// let reg = OntologyRegistry::load_from_dir("data/ontology/").unwrap();
/// assert!(reg.get_schema("Paper").is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct OntologyRegistry {
    schemas: HashMap<String, YamlNodeSchema>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("YAML parse error in {file}: {source}")]
    YamlParse {
        file: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("IO error reading {file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
    #[error("no node_schemas.yaml found in {dir}")]
    NotFound { dir: String },
}

impl OntologyRegistry {
    /// Load node schemas from a directory containing node_schemas.yaml.
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let dir = dir.as_ref();
        let yaml_path = dir.join("node_schemas.yaml");
        if !yaml_path.exists() {
            return Err(RegistryError::NotFound {
                dir: dir.display().to_string(),
            });
        }
        Self::load_from_file(&yaml_path)
    }

    /// Load node schemas from a specific YAML file.
    pub fn load_from_file(path: &Path) -> Result<Self, RegistryError> {
        let text = std::fs::read_to_string(path).map_err(|e| RegistryError::Io {
            file: path.display().to_string(),
            source: e,
        })?;
        Self::load_from_str(&text, &path.display().to_string())
    }

    /// Load from YAML string (for tests and embedded resources).
    pub fn load_from_str(text: &str, file_id: &str) -> Result<Self, RegistryError> {
        #[derive(Deserialize)]
        struct Root {
            node_schemas: Vec<YamlNodeSchema>,
        }
        let root: Root = serde_yaml::from_str(text).map_err(|e| RegistryError::YamlParse {
            file: file_id.to_string(),
            source: e,
        })?;
        let mut schemas = HashMap::new();
        for s in root.node_schemas {
            schemas.insert(s.label.clone(), s);
        }
        Ok(Self { schemas })
    }

    /// Lookup a node schema by label (e.g. "Paper", "Author").
    pub fn get_schema(&self, label: &str) -> Option<&YamlNodeSchema> {
        self.schemas.get(label)
    }

    /// All labels known to the registry.
    pub fn labels(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// Number of schemas loaded.
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = "\
node_schemas:
  - label: Paper
    required_fields:
      - name: vid
        field_type: String
      - name: arxiv_id
        field_type: String
    optional_fields:
      - name: doi
        field_type: String
  - label: Author
    required_fields:
      - name: vid
        field_type: String
    optional_fields: []
";

    #[test]
    fn test_load_from_str() {
        let reg = OntologyRegistry::load_from_str(SAMPLE_YAML, "test").unwrap();
        assert_eq!(reg.len(), 2);
        assert!(reg.get_schema("Paper").is_some());
        assert!(reg.get_schema("Author").is_some());
        assert!(reg.get_schema("Nonexistent").is_none());
    }

    #[test]
    fn test_field_defs_preserved() {
        let reg = OntologyRegistry::load_from_str(SAMPLE_YAML, "test").unwrap();
        let paper = reg.get_schema("Paper").unwrap();
        assert_eq!(paper.required_fields.len(), 2);
        assert_eq!(paper.required_fields[0].name, "vid");
        assert_eq!(paper.required_fields[0].field_type, "String");
        assert_eq!(paper.optional_fields.len(), 1);
        assert_eq!(paper.optional_fields[0].name, "doi");
    }

    #[test]
    fn test_labels_returns_all() {
        let reg = OntologyRegistry::load_from_str(SAMPLE_YAML, "test").unwrap();
        let mut labels = reg.labels();
        labels.sort();
        assert_eq!(labels, vec!["Author", "Paper"]);
    }

    #[test]
    fn test_load_empty_yaml_errors() {
        let result = OntologyRegistry::load_from_str("not: valid\n", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_file_errors() {
        let result = OntologyRegistry::load_from_dir("/nonexistent/path/");
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn test_round_trip_with_generated_yaml() {
        // Load the actual generated YAML from data/ontology/ if it exists.
        let yaml_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/ontology/node_schemas.yaml");
        if !yaml_path.exists() {
            eprintln!(
                "Skipping round-trip test: {} not found",
                yaml_path.display()
            );
            return;
        }
        let reg = OntologyRegistry::load_from_file(&yaml_path).unwrap();
        assert!(reg.len() >= 28, "Expected >=28 schemas, got {}", reg.len());
        assert!(reg.get_schema("Paper").is_some());
        // Verify required_fields are populated.
        let paper = reg.get_schema("Paper").unwrap();
        assert!(!paper.required_fields.is_empty());
        assert!(paper.required_fields.iter().any(|f| f.name == "vid"));
    }
}
