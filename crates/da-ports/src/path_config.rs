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
    domain_routing: BTreeMap<String, Vec<String>>,
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
        // domain_routing: coarse_topic_code → Vec<domain> (multi-domain)
        let mut domain_routing: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(text)
            && let Some(dr) = value.get("paths").and_then(|p| p.get("domain_routing")).and_then(|d| d.as_mapping())
        {
            for (code, domain) in dr {
                if let Some(k) = code.as_str() {
                    let domains: Vec<String> = match domain {
                        serde_yaml::Value::String(s) => vec![s.clone()],
                        serde_yaml::Value::Sequence(seq) => seq
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect(),
                        _ => vec![],
                    };
                    if !domains.is_empty() {
                        domain_routing.insert(k.to_string(), domains);
                    }
                }
            }
        }

        Self { paths, schema_objects, domain_routing }
    }

    /// Binding for one schema-manifest object class (e.g. "article").
    pub fn schema_object(&self, class: &str) -> Option<&SchemaObjectBinding> {
        self.schema_objects.get(class)
    }

    /// All registered object classes.
    pub fn schema_object_classes(&self) -> Vec<&str> {
        self.schema_objects.keys().map(String::as_str).collect()
    }

    /// Domains for a coarse_topic_code (multi-domain: stat-ml → [CS, math]).
    /// First = PRIMARY (quality thresholds), rest = SECONDARY (entity types).
    pub fn domains_for_topic(&self, code: &str) -> Option<Vec<String>> {
        self.domain_routing.get(code).cloned()
    }

    /// All registered domains (unique).
    pub fn domains(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for domains in self.domain_routing.values() {
            for d in domains {
                if !seen.contains(d) {
                    seen.push(d.clone());
                }
            }
        }
        seen
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

    // ── domain routing regression tests ──────────────────────────────
    // Regressions caught live (2026-08-24): (1) domain_routing parsing
    // was lost in a scope mistake — table stayed empty; (2) multi-domain
    // lists (stat-ml → [CS, math]) replaced single strings.

    #[test]
    fn domain_routing_single_and_multi_domain() {
        let config = PathConfig::parse(
            "paths:\n  domain_routing:\n    math-ho: mathematics\n    stat-ml:\n      - computer_science\n      - mathematics\n    cs-cl: computer_science\n",
        );
        // single string value
        assert_eq!(
            config.domains_for_topic("math-ho"),
            Some(vec!["mathematics".to_string()])
        );
        // list value = multi-domain (cross-domain paper)
        assert_eq!(
            config.domains_for_topic("stat-ml"),
            Some(vec![
                "computer_science".to_string(),
                "mathematics".to_string()
            ])
        );
        // unknown code
        assert_eq!(config.domains_for_topic("no-such-code"), None);
    }

    #[test]
    fn domain_routing_empty_when_absent() {
        let config = PathConfig::parse("paths:\n  catalog_root: data/x\n");
        assert_eq!(config.domains_for_topic("cs-cl"), None);
        assert!(config.domains().is_empty());
    }

    #[test]
    fn domains_unique_across_codes() {
        let config = PathConfig::parse(
            "paths:\n  domain_routing:\n    cs-cl: computer_science\n    cs-ai: computer_science\n    math-ho: mathematics\n",
        );
        let domains = config.domains();
        assert_eq!(domains.len(), 2, "computer_science counted once: {domains:?}");
        assert!(domains.contains(&"computer_science".to_string()));
        assert!(domains.contains(&"mathematics".to_string()));
    }
}
