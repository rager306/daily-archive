//! Edge contract matrix types (ADR-045, ADR-050).
//!
//! Universal struct for declaring which edges are legal between which
//! node labels. Project-specific contracts (CITES, AUTHORED_BY, etc.)
//! live in da-domain.

#![cfg_attr(rustfmt, rustfmt_skip)]
// Universal struct — uses &'static str for compile-time contracts.
// serde Deserialize removed: static slices don't impl Deserialize.

use serde::Serialize;

/// One row of the edge-endpoint contract matrix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EdgeContract {
    /// Edge constant (e.g. "CITES", "AUTHORED_BY").
    pub edge_constant: &'static str,
    /// Source node label.
    pub source_label: &'static str,
    /// Allowed target node label(s).
    pub target_labels: &'static [&'static str],
    /// Short rationale for documentation.
    pub rationale: &'static str,
}

impl EdgeContract {
    /// Check whether a (source, target) pair satisfies this contract.
    pub fn matches(&self, source: &str, target: &str) -> bool {
        self.source_label == source && self.target_labels.contains(&target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_monomorphic() {
        let c = EdgeContract {
            edge_constant: "CITES",
            source_label: "Paper",
            target_labels: &["Paper"],
            rationale: "Paper cites another paper",
        };
        assert!(c.matches("Paper", "Paper"));
        assert!(!c.matches("Paper", "Author"));
        assert!(!c.matches("Author", "Paper"));
    }

    #[test]
    fn test_matches_polymorphic() {
        let c = EdgeContract {
            edge_constant: "MENTIONS",
            source_label: "Paper",
            target_labels: &["Method", "Model", "Dataset"],
            rationale: "Polymorphic",
        };
        assert!(c.matches("Paper", "Method"));
        assert!(c.matches("Paper", "Dataset"));
        assert!(!c.matches("Paper", "Author"));
    }

    #[test]
    fn test_serde_serialize() {
        let c = EdgeContract {
            edge_constant: "CITES",
            source_label: "Paper",
            target_labels: &["Paper"],
            rationale: "test",
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("CITES"));
        assert!(json.contains("Paper"));
    }
}
