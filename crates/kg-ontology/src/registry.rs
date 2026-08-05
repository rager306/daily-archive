//! OntologyRegistry — loads meta-ontology definitions from YAML files.
//!
//! ADR-050 / M324: schema and edge contracts live as **DATA** in YAML, not
//! hardcoded in Rust. This module reads:
//! - `data/ontology/node_schemas.yaml`
//! - `data/ontology/edge_contracts.yaml` (optional; empty if missing)
//!
//! Created in M285-z09px0/S07; edge contracts added M324-v985an.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// One edge endpoint contract loaded from YAML (owned strings — runtime meta-ontology).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YamlEdgeContract {
    /// Wire string stored on graph edges (e.g. `hasPart`, `CITES`).
    pub edge_constant: String,
    pub source_label: String,
    pub target_labels: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

impl YamlEdgeContract {
    /// Check whether a (source, target) pair satisfies this contract.
    pub fn matches(&self, source: &str, target: &str) -> bool {
        self.source_label == source && self.target_labels.iter().any(|t| t == target)
    }
}

/// OntologyRegistry — in-memory index of node schemas + edge contracts from YAML.
///
/// # Example
/// ```no_run
/// use kg_ontology::registry::OntologyRegistry;
/// let reg = OntologyRegistry::load_from_dir("data/ontology/").unwrap();
/// assert!(reg.get_schema("Paper").is_some());
/// assert!(reg.edge_types().iter().any(|e| e == "hasPart"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct OntologyRegistry {
    schemas: HashMap<String, YamlNodeSchema>,
    edge_contracts: Vec<YamlEdgeContract>,
    /// Directory loaded from (if any) — diagnostics only.
    source_dir: Option<PathBuf>,
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
    /// Load meta-ontology from a directory: `node_schemas.yaml` (required)
    /// and `edge_contracts.yaml` (optional).
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let dir = dir.as_ref();
        let yaml_path = dir.join("node_schemas.yaml");
        if !yaml_path.exists() {
            return Err(RegistryError::NotFound {
                dir: dir.display().to_string(),
            });
        }
        let mut reg = Self::load_from_file(&yaml_path)?;
        reg.source_dir = Some(dir.to_path_buf());
        let edges_path = dir.join("edge_contracts.yaml");
        if edges_path.exists() {
            let edges = Self::load_edge_contracts_from_file(&edges_path)?;
            reg.edge_contracts = edges;
        }
        Ok(reg)
    }

    /// Load node schemas from a specific YAML file (edges empty).
    pub fn load_from_file(path: &Path) -> Result<Self, RegistryError> {
        let text = std::fs::read_to_string(path).map_err(|e| RegistryError::Io {
            file: path.display().to_string(),
            source: e,
        })?;
        Self::load_from_str(&text, &path.display().to_string())
    }

    /// Load node schemas from YAML string (for tests and embedded resources).
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
        Ok(Self {
            schemas,
            edge_contracts: Vec::new(),
            source_dir: None,
        })
    }

    /// Load edge contracts from a YAML file.
    pub fn load_edge_contracts_from_file(
        path: &Path,
    ) -> Result<Vec<YamlEdgeContract>, RegistryError> {
        let text = std::fs::read_to_string(path).map_err(|e| RegistryError::Io {
            file: path.display().to_string(),
            source: e,
        })?;
        Self::load_edge_contracts_from_str(&text, &path.display().to_string())
    }

    /// Load edge contracts from YAML string.
    pub fn load_edge_contracts_from_str(
        text: &str,
        file_id: &str,
    ) -> Result<Vec<YamlEdgeContract>, RegistryError> {
        #[derive(Deserialize)]
        struct Root {
            edge_contracts: Vec<YamlEdgeContract>,
        }
        let root: Root = serde_yaml::from_str(text).map_err(|e| RegistryError::YamlParse {
            file: file_id.to_string(),
            source: e,
        })?;
        Ok(root.edge_contracts)
    }

    /// Try `data/ontology/` relative to cwd, then workspace-relative path from crate.
    pub fn load_project_meta_ontology() -> Result<Self, RegistryError> {
        let candidates = [
            PathBuf::from("data/ontology"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("data/ontology"),
        ];
        let mut last = None;
        for dir in &candidates {
            match Self::load_from_dir(dir) {
                Ok(r) => return Ok(r),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or(RegistryError::NotFound {
            dir: "data/ontology".into(),
        }))
    }

    /// Lookup a node schema by label (e.g. "Paper", "Author").
    pub fn get_schema(&self, label: &str) -> Option<&YamlNodeSchema> {
        self.schemas.get(label)
    }

    /// All labels known to the registry.
    pub fn labels(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// Owned copy of all labels (stable for inventory).
    pub fn labels_owned(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    /// Edge contracts loaded from YAML (may be empty).
    pub fn edge_contracts(&self) -> &[YamlEdgeContract] {
        &self.edge_contracts
    }

    /// Unique edge wire strings from YAML contracts (sorted).
    pub fn edge_types(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for c in &self.edge_contracts {
            seen.insert(c.edge_constant.clone());
        }
        seen.into_iter().collect()
    }

    /// Number of node schemas loaded.
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Is the node schema map empty?
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Directory the registry was loaded from, if any.
    pub fn source_dir(&self) -> Option<&Path> {
        self.source_dir.as_deref()
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

    const SAMPLE_EDGES: &str = "\
edge_contracts:
  - edge_constant: hasPart
    source_label: Paper
    target_labels: [Section, Reference]
    rationale: structural part
  - edge_constant: CITES
    source_label: Paper
    target_labels: [Citation]
    rationale: cites
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
    fn test_edge_contracts_from_str() {
        let edges = OntologyRegistry::load_edge_contracts_from_str(SAMPLE_EDGES, "test").unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].edge_constant, "hasPart");
        assert!(edges[0].matches("Paper", "Section"));
        assert!(!edges[0].matches("Paper", "Author"));
        assert_eq!(edges[1].edge_constant, "CITES");
    }

    #[test]
    fn test_load_from_dir_includes_edges() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/ontology");
        if !dir.join("node_schemas.yaml").exists() {
            eprintln!("skip: no data/ontology");
            return;
        }
        let reg = OntologyRegistry::load_from_dir(&dir).unwrap();
        assert!(reg.len() >= 28);
        assert!(
            !reg.edge_contracts().is_empty(),
            "edge_contracts.yaml should load"
        );
        let types = reg.edge_types();
        assert!(
            types.iter().any(|e| e == "hasPart"),
            "expected hasPart in {types:?}"
        );
        assert!(types.iter().any(|e| e == "CITES"));
        assert!(types.iter().any(|e| e == "MENTIONS"));
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
