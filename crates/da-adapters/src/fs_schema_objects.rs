//! Filesystem adapter for schema-manifest objects (ADR-068, M451).
//!
//! Generic over object class: root/manifest/norm come from the
//! `schema_objects` table in data/path_config.yaml (DATA, no code
//! defaults). Bridges the store onto the domain's
//! [`FileBackedChecks`] so the generic engine can run file-backed
//! patterns without knowing about disks.

use std::path::PathBuf;

use da_domain::schema_invariants::{FileBackedChecks, InvariantDeclaration, ObjectNorm};
use da_ports::path_config::PathConfig;
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

/// Filesystem store for one object class bound via path_config.
#[derive(Debug, Clone)]
pub struct FsSchemaObjectStore {
    class: String,
    root: PathBuf,
    manifest: String,
    norm_path: PathBuf,
}

impl FsSchemaObjectStore {
    /// Bind from the loaded PathConfig. Fails for unregistered classes
    /// (a typo must be loud, not silently empty).
    pub fn from_path_config(config: &PathConfig, class: &str) -> Result<Self, SchemaObjectStoreError> {
        let binding = config
            .schema_object(class)
            .ok_or_else(|| SchemaObjectStoreError::UnknownClass(class.to_string()))?;
        Ok(Self {
            class: class.to_string(),
            root: binding.root.clone(),
            manifest: binding.manifest.clone(),
            norm_path: binding.norm.clone(),
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn object_dir(&self, id: &str) -> PathBuf {
        // ids are directory names; reject traversal loudly
        debug_assert!(!id.contains(".."), "object id must not traverse");
        self.root.join(id)
    }
}

impl SchemaObjectStore for FsSchemaObjectStore {
    fn object_class(&self) -> &str {
        &self.class
    }

    fn norm(&self) -> Result<ObjectNorm, SchemaObjectStoreError> {
        let text = std::fs::read_to_string(&self.norm_path)
            .map_err(|e| SchemaObjectStoreError::Norm(format!("{}: {e}", self.norm_path.display())))?;
        ObjectNorm::from_yaml_str(&text)
            .map_err(|e| SchemaObjectStoreError::Norm(e.to_string()))
    }

    fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> {
        if !self.root.is_dir() {
            return Err(SchemaObjectStoreError::Io(format!("root {} missing", self.root.display())));
        }
        let mut ids: Vec<String> = std::fs::read_dir(&self.root)
            .map_err(|e| SchemaObjectStoreError::Io(e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                // only instances that actually carry the manifest file
                e.path().join(&self.manifest).is_file()
            })
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn read_object(
        &self,
        id: &str,
    ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> {
        let path = self.object_dir(id).join(&self.manifest);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| SchemaObjectStoreError::Io(e.to_string()))?;
        serde_yaml::from_str(&text)
            .map(Some)
            .map_err(|e| SchemaObjectStoreError::Io(format!("{}: {e}", path.display())))
    }

    fn read_collection(
        &self,
        id: &str,
        rel_path: &str,
    ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> {
        let path = self.object_dir(id).join(rel_path);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| SchemaObjectStoreError::Io(e.to_string()))?;
        // collections on disk are JSON arrays; normalize into YAML values
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| SchemaObjectStoreError::Io(format!("{}: {e}", path.display())))?;
        let yaml_text = serde_yaml::to_string(&json)
            .map_err(|e| SchemaObjectStoreError::Io(e.to_string()))?;
        let records: Vec<serde_yaml::Value> = serde_yaml::from_str(&yaml_text)
            .map_err(|e| SchemaObjectStoreError::Io(e.to_string()))?;
        Ok(Some(records))
    }

    fn file_exists(&self, id: &str, rel_path: &str) -> Result<bool, SchemaObjectStoreError> {
        Ok(self.object_dir(id).join(rel_path).is_file())
    }

    fn file_backed_checks(&self) -> Box<dyn FileBackedChecks + '_> {
        Box::new(StoreBackedChecks::new(self))
    }
}

/// Bridge: store → domain's file-backed checks (keeps engine pure).
pub struct StoreBackedChecks<'a, S: SchemaObjectStore>(&'a S);

impl<'a, S: SchemaObjectStore> StoreBackedChecks<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self(store)
    }
}

impl<S: SchemaObjectStore> FileBackedChecks for StoreBackedChecks<'_, S> {
    fn collection_records(
        &self,
        object_id: &str,
        decl: &InvariantDeclaration,
    ) -> Result<Option<Vec<serde_yaml::Value>>, String> {
        let rel = decl
            .params
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "collection_records: no `file` param".to_string())?;
        self.0.read_collection(object_id, rel).map_err(|e| e.to_string())
    }

    fn artifact_exists(&self, object_id: &str, rel: &str) -> Result<bool, String> {
        self.0.file_exists(object_id, rel).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tree(root: &std::path::Path) {
        let norm = root.join("norm.yaml");
        std::fs::write(
            &norm,
            "lifecycle_order: [raw, parsed]\nlayers:\n  - {id: parsed}\ninvariants:\n  - pattern: derived_state_from_events\n    state_field: lifecycle\n    events_field: lifecycle_events\n    stage_field: stage\n    default_state: raw\n",
        )
        .unwrap();
        let obj = root.join("obj/2501.00001");
        std::fs::create_dir_all(obj.join("parsed")).unwrap();
        std::fs::write(
            obj.join("manifest.yaml"),
            "schema_version: article-manifest.v1\npaper_id: 2501.00001\nlifecycle: parsed\nlifecycle_events:\n  - {stage: parsed, recorded_at: '1', actor: t}\nartifacts:\n  parsed: {file: parsed/article.json, derived_from: [source]}\n",
        )
        .unwrap();
        std::fs::write(obj.join("parsed/article.json"), "{}").unwrap();
        // sibling without manifest must not be listed
        std::fs::create_dir_all(root.join("obj/no_manifest")).unwrap();
    }

    fn config(root: &std::path::Path) -> PathConfig {
        PathConfig::parse(&format!(
            "paths:\n  schema_objects:\n    article:\n      root: {}\n      manifest: manifest.yaml\n      norm: {}\n",
            root.join("obj").display(),
            root.join("norm.yaml").display()
        ))
    }

    #[test]
    fn binds_reads_and_lists_only_manifest_bearing_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        write_tree(tmp.path());
        let cfg = config(tmp.path());
        let store = FsSchemaObjectStore::from_path_config(&cfg, "article").unwrap();
        assert_eq!(store.list_ids().unwrap(), vec!["2501.00001".to_string()]);

        let norm = store.norm().unwrap();
        assert_eq!(norm.invariants.len(), 1);

        let obj = store.read_object("2501.00001").unwrap().unwrap();
        assert_eq!(obj.get("lifecycle").unwrap().as_str(), Some("parsed"));

        assert!(store.file_exists("2501.00001", "parsed/article.json").unwrap());
        assert!(!store.file_exists("2501.00001", "parsed/nope.json").unwrap());
    }

    #[test]
    fn unknown_class_is_loud() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = config(tmp.path());
        assert!(matches!(
            FsSchemaObjectStore::from_path_config(&cfg, "warp"),
            Err(SchemaObjectStoreError::UnknownClass(_))
        ));
    }

    #[test]
    fn bridge_reads_collection_and_exists() {
        use da_domain::schema_invariants::InvariantDeclaration;
        let tmp = tempfile::tempdir().unwrap();
        write_tree(tmp.path());
        let cfg = config(tmp.path());
        let store = FsSchemaObjectStore::from_path_config(&cfg, "article").unwrap();
        let extracted_dir = tmp.path().join("obj/2501.00001/extracted");
        std::fs::create_dir_all(&extracted_dir).unwrap();
        std::fs::write(
            extracted_dir.join("entities.json"),
            r#"[{"label":"x","section_ordinal":0}]"#,
        )
        .unwrap();

        let decl = InvariantDeclaration {
            pattern: "collection_records".into(),
            params: [
                ("file".to_string(), serde_yaml::Value::from("extracted/entities.json")),
            ]
            .into_iter()
            .collect(),
        };
        let io = StoreBackedChecks::new(&store);
        let records = io.collection_records("2501.00001", &decl).unwrap().unwrap();
        assert_eq!(records.len(), 1);
        assert!(io.artifact_exists("2501.00001", "parsed/article.json").unwrap());
    }
}
