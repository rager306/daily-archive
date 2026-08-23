//! Centralized path configuration — reads data/path_config.yaml.
//!
//! Meta-prompt principle: ALL paths are DATA, not code. This port defines
//! the interface; the adapter reads the YAML. No crate hardcodes paths.
//!
//! Env override: `DA_PATH_<KEY>` overrides any individual path
//! (e.g., DA_PATH_CATALOG_ROOT=data/sources/arxiv).

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Central path configuration loaded from data/path_config.yaml.
///
/// Every path constant in the pipeline is defined here — no crate
/// should have hardcoded `"data/..."` strings.
/// Binding of one schema-manifest object class (ADR-068): where its
/// instances live, which manifest file describes each instance, and
/// which schema (norm) defines the class. All DATA — no code defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaObjectBinding {
    pub root: PathBuf,
    pub manifest: String,
    pub norm: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct PathConfig {
    paths: BTreeMap<String, PathBuf>,
    schema_objects: BTreeMap<String, SchemaObjectBinding>,
}

impl PathConfig {
    /// Default config file location (overridable via DA_PATH_CONFIG).
    pub const CONFIG_FILE: &'static str = "data/path_config.yaml";

    /// Load from the default YAML file. Falls back to empty config
    /// (paths return None → caller uses its own default or errors).
    pub fn load() -> Self {
        let config_path = std::env::var("DA_PATH_CONFIG")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| Self::CONFIG_FILE.to_string());
        Self::load_from(&config_path)
    }

    /// Load from a specific file path.
    pub fn load_from(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Self::parse(&text)
    }

    /// Parse YAML text into a PathConfig.
    pub fn parse(text: &str) -> Self {
        let mut paths = BTreeMap::new();

        let mut schema_objects = BTreeMap::new();

        if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(text)
            && let Some(map) = value.get("paths").and_then(|v| v.as_mapping())
        {
            for (key, val) in map {
                if let (Some(k), Some(v)) = (key.as_str(), val.as_str()) {
                    paths.insert(k.to_string(), PathBuf::from(v));
                }
                // nested class table: schema_objects.<class> = {root, manifest, norm}
                if key.as_str() == Some("schema_objects")
                    && let Some(classes) = val.as_mapping()
                {
                    for (class, binding) in classes {
                        let Some(class) = class.as_str() else { continue };
                        let (Some(root), Some(manifest), Some(norm)) = (
                            binding.get("root").and_then(|v| v.as_str()),
                            binding.get("manifest").and_then(|v| v.as_str()),
                            binding.get("norm").and_then(|v| v.as_str()),
                        ) else {
                            continue;
                        };
                        schema_objects.insert(
                            class.to_string(),
                            SchemaObjectBinding {
                                root: PathBuf::from(root),
                                manifest: manifest.to_string(),
                                norm: PathBuf::from(norm),
                            },
                        );
                    }
                }
            }
        }

        Self { paths, schema_objects }
    }

    /// Binding for one schema-manifest object class (e.g. "article").
    pub fn schema_object(&self, class: &str) -> Option<&SchemaObjectBinding> {
        self.schema_objects.get(class)
    }

    /// All registered object classes.
    pub fn schema_object_classes(&self) -> Vec<&str> {
        self.schema_objects.keys().map(String::as_str).collect()
    }

    /// Get a path by key. Checks env override first (DA_PATH_<KEY>),
    /// then the YAML config, then returns None.
    ///
    /// # Example
    /// ```ignore
    /// let config = PathConfig::load();
    /// let catalog = config.get("catalog_root")
    ///     .unwrap_or_else(|| PathBuf::from("data/sources/arxiv"));
    /// ```
    pub fn get(&self, key: &str) -> Option<PathBuf> {
        // Env override: DA_PATH_CATALOG_ROOT → catalog_root
        let env_key = format!("DA_PATH_{}", key.to_uppercase());
        if let Ok(val) = std::env::var(&env_key)
            && !val.trim().is_empty()
        {
            return Some(PathBuf::from(val));
        }
        self.paths.get(key).cloned()
    }

    /// Get a path or panic with a clear message (for paths that MUST exist).
    pub fn require(&self, key: &str) -> PathBuf {
        self.get(key).unwrap_or_else(|| {
            panic!(
                "path_config: required path '{}' not found in {} (or DA_PATH_{} env)",
                key,
                Self::CONFIG_FILE,
                key.to_uppercase()
            )
        })
    }

    /// Get a path with a fallback default.
    pub fn get_or(&self, key: &str, default: &str) -> PathBuf {
        self.get(key).unwrap_or_else(|| PathBuf::from(default))
    }

    /// List all configured path keys.
    pub fn keys(&self) -> Vec<&str> {
        self.paths.keys().map(String::as_str).collect()
    }

    /// Merge another config on top (for testing / overrides).
    pub fn merge(mut self, other: &Self) -> Self {
        for (k, v) in &other.paths {
            self.paths.insert(k.clone(), v.clone());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_get() {
        let config = PathConfig::parse(
            "paths:\n  catalog_root: data/sources/arxiv\n  gold_dir: data/gold\n",
        );
        assert_eq!(
            config.get("catalog_root"),
            Some(PathBuf::from("data/sources/arxiv"))
        );
        assert_eq!(config.get("gold_dir"), Some(PathBuf::from("data/gold")));
        assert_eq!(config.get("missing"), None);
    }

    #[test]
    fn env_override() {
        // Rust 2024: env mutation is unsafe in multi-threaded tests.
        unsafe {
            std::env::set_var("DA_PATH_TEST_OVERRIDE_KEY", "/custom/path");
        }
        let config = PathConfig::parse("paths:\n  test_override_key: data/original\n");
        assert_eq!(
            config.get("test_override_key"),
            Some(PathBuf::from("/custom/path"))
        );
        unsafe {
            std::env::remove_var("DA_PATH_TEST_OVERRIDE_KEY");
        }
    }

    #[test]
    fn get_or_fallback() {
        let config = PathConfig::default();
        assert_eq!(
            config.get_or("missing", "default/path"),
            PathBuf::from("default/path")
        );
    }

    #[test]
    fn empty_yaml_returns_empty_config() {
        let config = PathConfig::parse("");
        assert_eq!(config.get("anything"), None);
    }
}
