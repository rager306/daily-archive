//! Scientific domain registry (ADR-043 + DOMAIN-REFERENCE-ARXIV.md).
//!
//! Reference data (arXiv categories + extension domains) is loaded from YAML:
//!   data/arxiv_categories.yaml — 148+ official arXiv categories
//!   data/extension_domains.yaml — da.* extension namespace + aliases
//!
//! This module provides logic only (canonicalization, validation, lookup).
//! No hardcoded category codes — all data comes from configuration.
//!
//! To update the registry: edit the YAML files. No recompilation needed.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

// ─── YAML schema types ───

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ArxivCategoriesYaml {
    version: String,
    source: String,
    total_categories: usize,
    groups: HashMap<String, GroupCodes>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GroupCodes {
    codes: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ExtensionDomainsYaml {
    version: String,
    namespace: String,
    domains: Vec<ExtensionDomain>,
    aliases: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ExtensionDomain {
    code: String,
    name: String,
    #[serde(rename = "typical_sources")]
    typical_sources: Option<Vec<String>>,
}

// ─── Bundled fallback (minimal, for first boot without YAML files) ───

/// Minimal bundled arXiv categories for bootstrapping.
/// Full registry is in data/arxiv_categories.yaml.
const BUNDLED_FALLBACK_YAML: &str = include_str!("../../../data/arxiv_categories.yaml");

/// Minimal bundled extension domains for bootstrapping.
const BUNDLED_EXTENSION_YAML: &str = include_str!("../../../data/extension_domains.yaml");

// ─── Registry (loaded once, cached) ───

/// Domain registry — holds arXiv categories + extension domains + aliases.
/// Loaded from YAML at first access (OnceLock for thread-safe init).
pub struct DomainRegistry {
    arxiv_codes: HashSet<String>,
    extension_codes: HashSet<String>,
    aliases: HashMap<String, String>,
}

impl DomainRegistry {
    fn load() -> Self {
        // Try YAML files from disk first, fall back to bundled
        let arxiv_yaml = Self::read_yaml("data/arxiv_categories.yaml", BUNDLED_FALLBACK_YAML);
        let ext_yaml = Self::read_yaml("data/extension_domains.yaml", BUNDLED_EXTENSION_YAML);

        let arxiv_data: ArxivCategoriesYaml =
            serde_yaml::from_str(&arxiv_yaml).expect("arxiv_categories.yaml must be valid YAML");
        let ext_data: ExtensionDomainsYaml =
            serde_yaml::from_str(&ext_yaml).expect("extension_domains.yaml must be valid YAML");

        let mut arxiv_codes = HashSet::new();
        for group in arxiv_data.groups.values() {
            for code in &group.codes {
                arxiv_codes.insert(code.clone());
            }
        }

        let extension_codes: HashSet<String> =
            ext_data.domains.iter().map(|d| d.code.clone()).collect();

        Self {
            arxiv_codes,
            extension_codes,
            aliases: ext_data.aliases,
        }
    }

    fn read_yaml(path: &str, fallback: &str) -> String {
        match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => fallback.to_string(),
        }
    }

    fn instance() -> &'static DomainRegistry {
        static REGISTRY: OnceLock<DomainRegistry> = OnceLock::new();
        REGISTRY.get_or_init(DomainRegistry::load)
    }
}

// ─── Public API (same interface as before, data from YAML) ───

/// Check if a domain code is a known arXiv code.
pub fn is_known_arxiv(code: &str) -> bool {
    DomainRegistry::instance().arxiv_codes.contains(code)
}

/// Check if a domain code is a known extension (da.*) code.
pub fn is_known_extension(code: &str) -> bool {
    DomainRegistry::instance().extension_codes.contains(code)
}

/// Check if a domain code is recognized (arXiv or extension).
pub fn is_known(code: &str) -> bool {
    is_known_arxiv(code) || is_known_extension(code)
}

/// Canonicalize an informal domain label.
/// Returns the canonical code if recognized, or the input as-is if unknown.
pub fn canonicalize(input: &str) -> &str {
    let lower = input.to_lowercase();
    let registry = DomainRegistry::instance();
    if let Some(canonical) = registry.aliases.get(&lower) {
        return canonical.as_str();
    }
    input
}

/// Get total count of known arXiv categories (from loaded YAML).
pub fn arxiv_category_count() -> usize {
    DomainRegistry::instance().arxiv_codes.len()
}

/// Get total count of extension domains (from loaded YAML).
pub fn extension_domain_count() -> usize {
    DomainRegistry::instance().extension_codes.len()
}

/// Force reload registry from disk (useful after YAML updates in long-running processes).
/// In short-lived CLI processes, this is never needed (OnceLock initializes once).
/// For server mode: restart the process to pick up YAML changes.
pub fn reload() {
    // OnceLock doesn't support clearing. No-op placeholder.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cs_lg_is_known() {
        assert!(is_known("cs.LG"));
        assert!(is_known_arxiv("cs.LG"));
    }

    #[test]
    fn test_da_medicine_is_known() {
        assert!(is_known("da.medicine"));
        assert!(is_known_extension("da.medicine"));
        assert!(!is_known_arxiv("da.medicine"));
    }

    #[test]
    fn test_unknown_not_known() {
        assert!(!is_known("xx.YY"));
    }

    #[test]
    fn test_canonicalize_cs_ml_to_cs_lg() {
        assert_eq!(canonicalize("cs.ml"), "cs.LG");
        assert_eq!(canonicalize("machine-learning"), "cs.LG");
    }

    #[test]
    fn test_canonicalize_nlp() {
        assert_eq!(canonicalize("nlp"), "cs.CL");
    }

    #[test]
    fn test_canonicalize_gnn() {
        assert_eq!(canonicalize("gnn"), "cs.LG");
    }

    #[test]
    fn test_canonicalize_medicine() {
        assert_eq!(canonicalize("medicine"), "da.medicine");
        assert_eq!(canonicalize("clinical"), "da.medicine");
    }

    #[test]
    fn test_canonicalize_unknown_passthrough() {
        assert_eq!(canonicalize("xx.YY"), "xx.YY");
    }

    #[test]
    fn test_arxiv_category_count_loaded() {
        // Should have loaded 100+ categories from YAML
        assert!(
            arxiv_category_count() >= 100,
            "got {}",
            arxiv_category_count()
        );
    }

    #[test]
    fn test_extension_domain_count_loaded() {
        // Should have loaded 10 extension domains from YAML
        assert_eq!(extension_domain_count(), 10);
    }

    #[test]
    fn test_qbio_codes_exist() {
        assert!(is_known("q-bio.GN"));
        assert!(is_known("q-bio.QM"));
        assert!(is_known("q-bio.OT"));
    }

    #[test]
    fn test_fin_replaced_qfin() {
        assert!(is_known("fin.CP"));
        assert!(is_known("fin.ST"));
        assert!(!is_known("q-fin.CP")); // old code should NOT be valid
    }

    #[test]
    fn test_da_codes_not_in_arxiv() {
        for code in [
            "da.medicine",
            "da.microbiome",
            "da.metabolism",
            "da.biohacking",
        ] {
            assert!(!is_known_arxiv(code));
            assert!(is_known_extension(code));
        }
    }

    #[test]
    fn test_qfin_legacy_migration() {
        assert_eq!(canonicalize("q-fin.CP"), "fin.CP");
    }
}
