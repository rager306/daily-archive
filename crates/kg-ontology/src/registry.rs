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

/// Whether an edge type is a temporal fact or structural composition (ADR-046 / M326).
///
/// - [`Structural`]: composition/provenance; only `created_at` expected.
/// - [`Fact`]: assertion that can become false; full 5-field [`crate::temporal::TemporalEdge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TemporalClass {
    /// Composition / classification / provenance — not invalidated by newer facts.
    #[default]
    Structural,
    /// Fact assertion — carries valid_at/invalid_at/expired_at/reference_time/created_at.
    Fact,
}

impl TemporalClass {
    pub fn is_fact(self) -> bool {
        matches!(self, Self::Fact)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Fact => "fact",
        }
    }
}

/// Runtime readiness of a data-driven edge contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Active,
    Planned,
}

/// One edge endpoint contract loaded from YAML (owned strings — runtime meta-ontology).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YamlEdgeContract {
    /// Wire string stored on graph edges (e.g. `hasPart`, `CITES`).
    pub edge_constant: String,
    pub source_label: String,
    pub target_labels: Vec<String>,
    /// ADR-046: fact edges get TemporalEdge fields; structural do not.
    #[serde(default)]
    pub temporal_class: TemporalClass,
    #[serde(default)]
    pub rationale: String,
    /// Planned contracts remain visible for roadmap/ADR checks but cannot be
    /// treated as active graph capability until endpoint schemas materialize.
    #[serde(default)]
    pub implementation_status: ContractStatus,
}

impl YamlEdgeContract {
    /// Check whether a (source, target) pair satisfies this contract.
    pub fn matches(&self, source: &str, target: &str) -> bool {
        self.source_label == source && self.target_labels.iter().any(|t| t == target)
    }

    /// True if this edge type should carry the 5-field temporal model.
    pub fn is_temporal_fact(&self) -> bool {
        self.temporal_class.is_fact()
    }
}

/// Canonical temporal wire contract (PIR-008 / M398/S04).
///
/// Encoded in `data/ontology/node_schemas.yaml` under the top-level
/// `temporal_wire_contract` key so the decision lives in ontology DATA
/// (ADR-050). [`TemporalWireContract::default`] carries the same
/// canonical values so a regenerated YAML (`da ontology-export`) always
/// re-emits the contract even when the input file lacks the block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalWireContract {
    /// Wire format for DateTime properties: `rfc3339_utc`.
    pub datetime_format: String,
    /// The only open-bound sentinel.
    pub open_sentinel: String,
    /// New-write policy for integer timestamps: `rejected`.
    pub integer_write: String,
    /// Read policy for legacy integer timestamps: `compatibility_decoder_only`.
    pub legacy_integer_read: String,
    /// Human-readable statement of the contract.
    pub description: String,
}

impl Default for TemporalWireContract {
    fn default() -> Self {
        Self {
            datetime_format: "rfc3339_utc".into(),
            open_sentinel: "OPEN".into(),
            integer_write: "rejected".into(),
            legacy_integer_read: "compatibility_decoder_only".into(),
            description: concat!(
                "DateTime node/edge properties are written as canonical RFC3339 UTC strings ",
                "(chrono DateTime<Utc>::to_rfc3339(), e.g. 2024-01-15T10:30:00Z). Integer Unix ",
                "timestamps are the legacy read-only wire form and are rejected for new writes. ",
                "OPEN is the only open-bound sentinel."
            )
            .into(),
        }
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
    /// Canonical temporal wire contract parsed from YAML (default when absent).
    temporal_wire_contract: TemporalWireContract,
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
    #[error("edge contract {edge} references unregistered {role} label {label}")]
    UnregisteredEdgeLabel {
        edge: String,
        role: &'static str,
        label: String,
    },
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
            reg.validate_edge_labels()?;
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
            #[serde(default)]
            temporal_wire_contract: Option<TemporalWireContract>,
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
            temporal_wire_contract: root.temporal_wire_contract.unwrap_or_default(),
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

    /// Try `data/ontology/` relative to cwd, then env, then workspace-relative.
    pub fn load_project_meta_ontology() -> Result<Self, RegistryError> {
        let primary = std::env::var("DA_ONTOLOGY_DIR")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from);
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = &primary {
            candidates.push(p.clone());
        }
        candidates.push(PathBuf::from("data/ontology"));
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("data/ontology"),
        );
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

    fn validate_edge_labels(&self) -> Result<(), RegistryError> {
        for contract in &self.edge_contracts {
            if contract.implementation_status == ContractStatus::Planned {
                continue;
            }
            if !self.schemas.contains_key(&contract.source_label) {
                return Err(RegistryError::UnregisteredEdgeLabel {
                    edge: contract.edge_constant.clone(),
                    role: "source",
                    label: contract.source_label.clone(),
                });
            }
            for label in &contract.target_labels {
                if !self.schemas.contains_key(label) {
                    return Err(RegistryError::UnregisteredEdgeLabel {
                        edge: contract.edge_constant.clone(),
                        role: "target",
                        label: label.clone(),
                    });
                }
            }
        }
        Ok(())
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

    /// The canonical temporal wire contract (PIR-008): RFC3339 UTC
    /// strings for DateTime writes, `OPEN` as the only open-bound
    /// sentinel, integer timestamps rejected for new writes. Falls back
    /// to the canonical default when the YAML omits the block.
    pub fn temporal_wire_contract(&self) -> &TemporalWireContract {
        &self.temporal_wire_contract
    }

    /// Unique edge wire strings from YAML contracts (sorted).
    pub fn edge_types(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for c in &self.edge_contracts {
            seen.insert(c.edge_constant.clone());
        }
        seen.into_iter().collect()
    }

    /// Wire strings classified as temporal **fact** edges (ADR-046).
    pub fn temporal_fact_edge_types(&self) -> Vec<String> {
        self.edge_contracts
            .iter()
            .filter(|c| c.is_temporal_fact())
            .map(|c| c.edge_constant.clone())
            .collect()
    }

    /// Wire strings classified as **structural** edges.
    pub fn structural_edge_types(&self) -> Vec<String> {
        self.edge_contracts
            .iter()
            .filter(|c| !c.is_temporal_fact())
            .map(|c| c.edge_constant.clone())
            .collect()
    }

    /// Lookup temporal class for a wire string. Unknown → None.
    pub fn temporal_class_of(&self, edge_constant: &str) -> Option<TemporalClass> {
        self.edge_contracts
            .iter()
            .find(|c| c.edge_constant == edge_constant)
            .map(|c| c.temporal_class)
    }

    /// True if the edge wire string is a temporal fact (false if structural or unknown).
    pub fn is_temporal_edge_type(&self, edge_constant: &str) -> bool {
        self.temporal_class_of(edge_constant)
            .is_some_and(TemporalClass::is_fact)
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

    // ── Temporal wire contract (PIR-008 / M398/S04/T01) ────────────────

    #[test]
    fn test_temporal_wire_contract_default_is_canonical() {
        let reg = OntologyRegistry::load_from_str(SAMPLE_YAML, "test").unwrap();
        let contract = reg.temporal_wire_contract();
        assert_eq!(contract.datetime_format, "rfc3339_utc");
        assert_eq!(contract.open_sentinel, "OPEN");
        assert_eq!(contract.integer_write, "rejected");
        assert_eq!(contract.legacy_integer_read, "compatibility_decoder_only");
    }

    #[test]
    fn test_temporal_wire_contract_parsed_from_yaml() {
        let yaml = concat!(
            "temporal_wire_contract:\n",
            "  datetime_format: rfc3339_utc\n",
            "  open_sentinel: OPEN\n",
            "  integer_write: rejected\n",
            "  legacy_integer_read: compatibility_decoder_only\n",
            "  description: test contract\n",
            "node_schemas:\n",
            "  - label: Paper\n    required_fields: []\n    optional_fields: []\n",
        );
        let reg = OntologyRegistry::load_from_str(yaml, "test").unwrap();
        assert_eq!(reg.temporal_wire_contract().description, "test contract");
        assert_eq!(reg.temporal_wire_contract().open_sentinel, "OPEN");
    }

    #[test]
    fn test_default_contract_round_trips_through_serde() {
        let contract = TemporalWireContract::default();
        let yaml = serde_yaml::to_string(&contract).unwrap();
        let parsed: TemporalWireContract = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, contract);
        assert_eq!(parsed.datetime_format, "rfc3339_utc");
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
    fn test_load_from_dir_rejects_edge_endpoint_without_node_schema() {
        let dir = std::env::temp_dir().join(format!(
            "kg-ontology-missing-edge-label-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("node_schemas.yaml"),
            "node_schemas:\n  - label: Paper\n    required_fields: []\n    optional_fields: []\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("edge_contracts.yaml"),
            "edge_contracts:\n  - edge_constant: BAD\n    source_label: Paper\n    target_labels: [Missing]\n",
        )
        .unwrap();
        let error = OntologyRegistry::load_from_dir(&dir).unwrap_err();
        assert!(matches!(
            error,
            RegistryError::UnregisteredEdgeLabel { role: "target", label, .. }
                if label == "Missing"
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_load_from_dir_allows_explicitly_planned_unregistered_endpoint() {
        let dir = std::env::temp_dir().join(format!(
            "kg-ontology-planned-edge-label-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("node_schemas.yaml"),
            "node_schemas:\n  - label: Paper\n    required_fields: []\n    optional_fields: []\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("edge_contracts.yaml"),
            "edge_contracts:\n  - edge_constant: FUTURE\n    source_label: Paper\n    target_labels: [Missing]\n    implementation_status: planned\n",
        )
        .unwrap();
        let registry = OntologyRegistry::load_from_dir(&dir).unwrap();
        assert_eq!(registry.edge_contracts().len(), 1);
        assert_eq!(
            registry.edge_contracts()[0].implementation_status,
            ContractStatus::Planned
        );
        std::fs::remove_dir_all(dir).unwrap();
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

    #[test]
    fn test_temporal_class_from_project_yaml() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/ontology");
        if !dir.join("edge_contracts.yaml").exists() {
            eprintln!("skip: no edge_contracts.yaml");
            return;
        }
        let reg = OntologyRegistry::load_from_dir(&dir).unwrap();
        assert!(reg.is_temporal_edge_type("CITES"));
        assert!(reg.is_temporal_edge_type("MENTIONS"));
        assert!(reg.is_temporal_edge_type("SUPPORTS"));
        assert!(reg.is_temporal_edge_type("SUPERSEDES"));
        assert!(!reg.is_temporal_edge_type("hasPart"));
        assert!(!reg.is_temporal_edge_type("FROM_SOURCE"));
        assert!(!reg.is_temporal_edge_type("inCategory"));
        let facts = reg.temporal_fact_edge_types();
        assert!(facts.iter().any(|e| e == "CITES"));
        assert!(!facts.iter().any(|e| e == "hasPart"));
        let structural = reg.structural_edge_types();
        assert!(structural.iter().any(|e| e == "hasPart"));
        assert_eq!(
            reg.temporal_class_of("hasPart"),
            Some(TemporalClass::Structural)
        );
        assert_eq!(reg.temporal_class_of("CITES"), Some(TemporalClass::Fact));
        assert_eq!(reg.temporal_class_of("NO_SUCH_EDGE"), None);
    }

    #[test]
    fn test_temporal_class_defaults_to_structural() {
        let edges = OntologyRegistry::load_edge_contracts_from_str(
            r#"
edge_contracts:
  - edge_constant: hasPart
    source_label: Paper
    target_labels: [Section]
    rationale: no class field
"#,
            "test",
        )
        .unwrap();
        assert_eq!(edges[0].temporal_class, TemporalClass::Structural);
        assert!(!edges[0].is_temporal_fact());
    }

    #[test]
    fn test_temporal_class_fact_from_str() {
        let edges = OntologyRegistry::load_edge_contracts_from_str(
            r#"
edge_contracts:
  - edge_constant: CITES
    source_label: Paper
    target_labels: [Citation]
    temporal_class: fact
    rationale: citation fact
"#,
            "test",
        )
        .unwrap();
        assert_eq!(edges[0].temporal_class, TemporalClass::Fact);
        assert!(edges[0].is_temporal_fact());
    }

    /// ADR-057 drafts live under data/ontology/drafts/ and must not be auto-loaded
    /// by load_from_dir (only node_schemas.yaml + edge_contracts.yaml).
    #[test]
    fn test_load_from_dir_ignores_drafts_subdirectory() {
        let dir =
            std::env::temp_dir().join(format!("kg-ontology-drafts-ignored-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("node_schemas.yaml"),
            concat!(
                "node_schemas:\n",
                "  - label: Paper\n    required_fields: []\n    optional_fields: []\n",
                "  - label: Section\n    required_fields: []\n    optional_fields: []\n",
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("edge_contracts.yaml"),
            "edge_contracts:\n  - edge_constant: hasPart\n    source_label: Paper\n    target_labels: [Section]\n    rationale: structural\n",
        )
        .unwrap();
        let drafts = dir.join("drafts");
        std::fs::create_dir_all(&drafts).unwrap();
        std::fs::write(
            drafts.join("non_article_structure_contracts.yaml"),
            concat!(
                "node_schemas:\n  - label: KnowledgeUnit\n    required_fields: []\n    optional_fields: []\n",
                "edge_contracts:\n  - edge_constant: DISCUSSES_PAPER\n    source_label: KnowledgeUnit\n    target_labels: [Paper]\n",
            ),
        )
        .unwrap();

        let reg = OntologyRegistry::load_from_dir(&dir).unwrap();
        assert!(reg.get_schema("Paper").is_some());
        assert!(
            reg.get_schema("KnowledgeUnit").is_none(),
            "drafts/ must not pollute production registry"
        );
        assert!(!reg.edge_types().iter().any(|e| e == "DISCUSSES_PAPER"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
