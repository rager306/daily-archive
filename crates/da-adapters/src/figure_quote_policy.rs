//! DATA loader for the figure quote policy window (ADR-062).
//!
//! Reads `data/ontology/figure_quote_policy.yaml` (or an explicit path).
//! The soft target is a guideline; the absolute max must stay in sync with
//! the JSON-schema `maxLength` in prompt_tasks.yaml and figure_schemas.yaml.

use da_domain::figure_analysis::FigureQuotePolicy;
use std::path::Path;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FigureQuotePolicyFile {
    schema_version: u32,
    soft_target_chars: u32,
    absolute_max_chars: u32,
}

/// Load and validate the quote policy from a YAML file.
pub fn load_figure_quote_policy(path: &Path) -> Result<FigureQuotePolicy, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("read figure quote policy {}: {error}", path.display()))?;
    let file: FigureQuotePolicyFile = serde_yaml::from_str(&raw)
        .map_err(|error| format!("parse figure quote policy {}: {error}", path.display()))?;
    if file.schema_version != 1 {
        return Err(format!(
            "unsupported figure quote policy schema_version {} in {}",
            file.schema_version,
            path.display()
        ));
    }
    let policy = FigureQuotePolicy {
        soft_target_chars: file.soft_target_chars,
        absolute_max_chars: file.absolute_max_chars,
    };
    policy
        .validate()
        .map_err(|error| format!("invalid figure quote policy {}: {error}", path.display()))?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(content: &str) -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("figure_quote_policy.yaml");
        std::fs::write(&path, content).unwrap();
        // Leak the tempdir path: tests are short-lived and the OS reclaims it.
        dir.keep();
        path
    }

    #[test]
    fn loads_valid_policy() {
        let path =
            write_temp("schema_version: 1\nsoft_target_chars: 1500\nabsolute_max_chars: 4000\n");
        let policy = load_figure_quote_policy(&path).unwrap();
        assert_eq!(policy.soft_target_chars, 1500);
        assert_eq!(policy.absolute_max_chars, 4000);
    }

    #[test]
    fn unknown_field_is_rejected() {
        let path = write_temp(
            "schema_version: 1\nsoft_target_chars: 1500\nabsolute_max_chars: 4000\nbogus: 1\n",
        );
        assert!(load_figure_quote_policy(&path).is_err());
    }

    #[test]
    fn missing_file_errors_without_panicking() {
        let err = load_figure_quote_policy(Path::new("/nonexistent/figure_quote_policy.yaml"))
            .unwrap_err();
        assert!(err.contains("read figure quote policy"), "{err}");
    }

    #[test]
    fn inverted_window_is_rejected() {
        let path =
            write_temp("schema_version: 1\nsoft_target_chars: 5000\nabsolute_max_chars: 4000\n");
        let err = load_figure_quote_policy(&path).unwrap_err();
        assert!(err.contains("soft target"), "{err}");
    }
}
