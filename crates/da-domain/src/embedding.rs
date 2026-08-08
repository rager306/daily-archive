//! Data-driven embedding input projections.
//!
//! Backend limits are explicit policy, never hidden truncation. Paper vectors
//! use title then abstract; full body/section vectors belong to chunk stages.

use serde::{Deserialize, Serialize};

const BUNDLED_PARAMS_YAML: &str = include_str!("../../../data/algorithm_params.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingInputPolicy {
    pub max_input_chars: usize,
    pub separator: String,
    pub paper_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AlgorithmParams {
    embedding_input: EmbeddingInputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingProjection {
    pub text: String,
    pub source_chars: usize,
    pub emitted_chars: usize,
    pub truncated: bool,
}

impl EmbeddingInputPolicy {
    pub fn load() -> Result<Self, String> {
        let source = std::fs::read_to_string("data/algorithm_params.yaml")
            .unwrap_or_else(|_| BUNDLED_PARAMS_YAML.to_string());
        let mut policy = Self::from_yaml(&source)?;
        if let Ok(value) = std::env::var("FD_MAX_INPUT_CHARS") {
            policy.max_input_chars = value
                .parse::<usize>()
                .map_err(|_| "FD_MAX_INPUT_CHARS must be a positive integer".to_string())?;
            if policy.max_input_chars == 0 {
                return Err("FD_MAX_INPUT_CHARS must be positive".into());
            }
        }
        Ok(policy)
    }

    pub fn from_yaml(source: &str) -> Result<Self, String> {
        let root: AlgorithmParams =
            serde_yaml::from_str(source).map_err(|error| error.to_string())?;
        let policy = root.embedding_input;
        if policy.max_input_chars == 0 {
            return Err("embedding_input.max_input_chars must be positive".into());
        }
        if policy.paper_fields != ["title", "abstract"] {
            return Err(
                "embedding_input.paper_fields must be [title, abstract] for Paper vectors".into(),
            );
        }
        Ok(policy)
    }

    pub fn project_paper(&self, title: &str, abstract_text: &str) -> EmbeddingProjection {
        let source = format!("{}{}{}", title, self.separator, abstract_text);
        let source_chars = source.chars().count();
        let text = source
            .chars()
            .take(self.max_input_chars)
            .collect::<String>();
        let emitted_chars = text.chars().count();
        EmbeddingProjection {
            text,
            source_chars,
            emitted_chars,
            truncated: emitted_chars < source_chars,
        }
    }

    pub fn fingerprint(&self) -> String {
        format!(
            "paper_fields={};separator={:?};max_input_chars={}",
            self.paper_fields.join(","),
            self.separator,
            self.max_input_chars
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_projection_is_utf8_safe_and_reports_truncation() {
        let policy = EmbeddingInputPolicy {
            max_input_chars: 5,
            separator: "\n".into(),
            paper_fields: vec!["title".into(), "abstract".into()],
        };
        let projection = policy.project_paper("Граф", "данные");
        assert_eq!(projection.text, "Граф\n");
        assert_eq!(projection.emitted_chars, 5);
        assert!(projection.truncated);
        assert!(projection.source_chars > projection.emitted_chars);
    }

    #[test]
    fn invalid_or_zero_policy_fails_closed() {
        let yaml = r#"
embedding_input:
  max_input_chars: 0
  separator: "\\n"
  paper_fields: [title, abstract]
"#;
        assert!(EmbeddingInputPolicy::from_yaml(yaml).is_err());
    }

    #[test]
    fn bundled_policy_loads() {
        let policy = EmbeddingInputPolicy::from_yaml(BUNDLED_PARAMS_YAML).unwrap();
        assert_eq!(policy.paper_fields, ["title", "abstract"]);
        assert!(policy.max_input_chars >= 1024);
    }
}
