//! Validator types (Phase A — universal, schema-agnostic).
//!
//! This module holds the *types* the validator produces: `Severity`,
//! `Violation`, `PropertySnapshot`, and the `format_violations` helper.
//! The schema-aware `validate_node_properties` function stays in
//! `da-domain::validator` during Phase A because it needs access to
//! `schema_for_label()` which has not moved to kg-ontology yet (that
//! lands in Phase D when the YAML loader ships).
//!
//! Once Phase D lands, `validate_node_properties` moves here as a method
//! on `OntologyRegistry` and this module becomes the single home for
//! all validation types and logic.

use serde_json::Value;
use std::collections::HashMap;

/// Severity of a schema violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Missing required field, broken architectural invariant (D127/D134),
    /// or unknown node label. Must be fixed.
    Critical,
    /// Type mismatch, unknown optional field, or suspicious pattern.
    /// May indicate drift between schema and pipeline.
    Warning,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::Warning => "WARNING",
        }
    }
}

/// One rule violation found by the validator.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub severity: Severity,
    pub rule: String,
    pub field: Option<String>,
    pub message: String,
}

impl Violation {
    pub fn critical(
        rule: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Critical,
            rule: rule.into(),
            field: Some(field.into()),
            message: message.into(),
        }
    }

    pub fn warn(
        rule: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            rule: rule.into(),
            field: Some(field.into()),
            message: message.into(),
        }
    }

    pub fn critical_no_field(
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Critical,
            rule: rule.into(),
            field: None,
            message: message.into(),
        }
    }
}

/// A snapshot of one node's properties for validation. Generic JSON map
/// so the validator works against any source (live graph read, test
/// fixture, CLI stdin input, pipeline capture).
pub type PropertySnapshot = HashMap<String, Value>;

/// Format a list of violations for human-readable output.
pub fn format_violations(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return "OK — no violations".to_string();
    }
    let mut s = String::new();
    for v in violations {
        let field = v.field.as_deref().unwrap_or("-");
        s.push_str(&format!(
            "[{}] rule={} field={} :: {}\n",
            v.severity.as_str(),
            v.rule,
            field,
            v.message
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Critical.as_str(), "CRITICAL");
        assert_eq!(Severity::Warning.as_str(), "WARNING");
    }

    #[test]
    fn test_violation_constructors() {
        let c = Violation::critical("required-field", "vid", "missing");
        assert_eq!(c.severity, Severity::Critical);
        assert_eq!(c.field.as_deref(), Some("vid"));

        let w = Violation::warn("type-mismatch", "value", "got string");
        assert_eq!(w.severity, Severity::Warning);

        let n = Violation::critical_no_field("schema-registry", "unknown");
        assert!(n.field.is_none());
    }

    #[test]
    fn test_format_violations_empty() {
        assert_eq!(format_violations(&[]), "OK — no violations");
    }

    #[test]
    fn test_format_violations_non_empty() {
        let v = vec![
            Violation::critical("required-field", "vid", "missing vid"),
            Violation::warn("type-mismatch", "value", "got bool"),
        ];
        let s = format_violations(&v);
        assert!(s.contains("[CRITICAL] rule=required-field field=vid"));
        assert!(s.contains("[WARNING] rule=type-mismatch field=value"));
    }
}
