//! Pure DATA-driven semantic role policy for grounded Section summaries.
//!
//! The domain validates and executes loaded policy values; filesystem/YAML and
//! policy fingerprinting remain adapter concerns. No title vocabulary is
//! compiled into Rust.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SummaryType(String);

impl SummaryType {
    pub fn new(value: impl Into<String>) -> Result<Self, SectionRolePolicyError> {
        let value = value.into().trim().to_ascii_lowercase();
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err(SectionRolePolicyError::InvalidSummaryType(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRoleRule {
    pub summary_type: SummaryType,
    #[serde(default)]
    pub allow_suffix_match: bool,
    pub title_aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRolePolicy {
    pub schema_version: String,
    pub max_title_chars: usize,
    pub rules: Vec<SectionRoleRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRoleMatch {
    pub summary_type: SummaryType,
    pub matched_alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SectionRolePolicyError {
    #[error("section summary policy schema_version must not be empty")]
    EmptySchemaVersion,
    #[error("section summary policy must contain at least one rule")]
    EmptyRules,
    #[error("section summary policy max_title_chars must be greater than zero")]
    ZeroMaxTitleChars,
    #[error("invalid summary type {0:?}; use lowercase snake_case")]
    InvalidSummaryType(String),
    #[error("duplicate summary type {0}")]
    DuplicateSummaryType(String),
    #[error("summary type {0} has no title aliases")]
    EmptyAliases(String),
    #[error("duplicate normalized title alias {0:?}")]
    DuplicateAlias(String),
}

impl SectionRolePolicy {
    pub fn validate(&self) -> Result<(), SectionRolePolicyError> {
        if self.schema_version.trim().is_empty() {
            return Err(SectionRolePolicyError::EmptySchemaVersion);
        }
        if self.rules.is_empty() {
            return Err(SectionRolePolicyError::EmptyRules);
        }
        if self.max_title_chars == 0 {
            return Err(SectionRolePolicyError::ZeroMaxTitleChars);
        }
        let mut types = BTreeSet::new();
        let mut aliases = BTreeSet::new();
        for rule in &self.rules {
            if !types.insert(rule.summary_type.clone()) {
                return Err(SectionRolePolicyError::DuplicateSummaryType(
                    rule.summary_type.as_str().into(),
                ));
            }
            if rule.title_aliases.is_empty() {
                return Err(SectionRolePolicyError::EmptyAliases(
                    rule.summary_type.as_str().into(),
                ));
            }
            for alias in &rule.title_aliases {
                let alias = normalize_title(alias);
                if alias.is_empty() || !aliases.insert(alias.clone()) {
                    return Err(SectionRolePolicyError::DuplicateAlias(alias));
                }
            }
        }
        Ok(())
    }

    /// Deterministic ordered match. An alias matches a complete normalized
    /// title token sequence, optionally with numeric heading prefixes removed.
    /// This avoids broad substring matches such as `method` in `methodologyx`.
    pub fn classify(&self, title: &str) -> Option<SectionRoleMatch> {
        if title.chars().count() > self.max_title_chars {
            return None;
        }
        let title = normalize_title(title);
        for rule in &self.rules {
            for alias in &rule.title_aliases {
                let normalized_alias = normalize_title(alias);
                if title == normalized_alias
                    || (rule.allow_suffix_match
                        && title
                            .strip_suffix(&normalized_alias)
                            .is_some_and(|prefix| prefix.ends_with(' ')))
                {
                    return Some(SectionRoleMatch {
                        summary_type: rule.summary_type.clone(),
                        matched_alias: normalized_alias,
                    });
                }
            }
        }
        None
    }
}

fn normalize_title(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    let normalized = cleaned.split_whitespace().collect::<Vec<_>>();
    let first_text = normalized
        .iter()
        .position(|token| !token.chars().all(|character| character.is_ascii_digit()))
        .unwrap_or(normalized.len());
    normalized[first_text..].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SectionRolePolicy {
        SectionRolePolicy {
            schema_version: "section-summary-roles.v1".into(),
            max_title_chars: 160,
            rules: vec![
                SectionRoleRule {
                    summary_type: SummaryType::new("methods").unwrap(),
                    allow_suffix_match: true,
                    title_aliases: vec!["Methods".into(), "Methodology".into()],
                },
                SectionRoleRule {
                    summary_type: SummaryType::new("results").unwrap(),
                    allow_suffix_match: true,
                    title_aliases: vec!["Results".into(), "Experimental Results".into()],
                },
            ],
        }
    }

    #[test]
    fn classifies_normalized_exact_titles_without_substring_guessing() {
        let policy = policy();
        policy.validate().unwrap();
        assert_eq!(
            policy.classify("2. METHODS").unwrap().summary_type.as_str(),
            "methods"
        );
        assert_eq!(
            policy
                .classify("Experimental Results")
                .unwrap()
                .summary_type
                .as_str(),
            "results"
        );
        assert_eq!(
            policy
                .classify("2.2 Akoma Ntoso Methodology")
                .unwrap()
                .summary_type
                .as_str(),
            "methods"
        );
        assert!(policy.classify("MethodologyX").is_none());
        assert!(policy.classify("Discussion").is_none());
        assert!(policy.classify(&"Methods paragraph ".repeat(20)).is_none());
    }

    #[test]
    fn rejects_duplicate_types_and_aliases() {
        let mut duplicate_type = policy();
        duplicate_type.rules[1].summary_type = SummaryType::new("methods").unwrap();
        assert!(matches!(
            duplicate_type.validate(),
            Err(SectionRolePolicyError::DuplicateSummaryType(_))
        ));
        let mut duplicate_alias = policy();
        duplicate_alias.rules[1].title_aliases[0] = " methods ".into();
        assert!(matches!(
            duplicate_alias.validate(),
            Err(SectionRolePolicyError::DuplicateAlias(_))
        ));
    }

    #[test]
    fn serde_roundtrip_preserves_ordered_policy() {
        let policy = policy();
        let bytes = serde_json::to_vec(&policy).unwrap();
        let roundtrip: SectionRolePolicy = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(roundtrip, policy);
    }
}
