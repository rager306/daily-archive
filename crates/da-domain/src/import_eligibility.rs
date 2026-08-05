//! D127 import eligibility (M357).
//!
//! Controls whether graph nodes are marked import_eligible=Eligible (production)
//! or import_eligible=false (research/snapshot only).
//!
//! D127 was locked since M263. User gave explicit yes on 2026-08-05:
//! "давно уже надо тестировать и импортировать и проверять что получается".
//!
//! Default: Locked. CLI `--enable-import` opts in to Eligible.

use serde::{Deserialize, Serialize};

/// D127 import gate state per invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportEligibility {
    /// D127 locked: import_eligible=false on all nodes (default).
    Locked,
    /// D127 unlocked: import_eligible=Eligible on nodes (requires --enable-import).
    Eligible,
}

impl Default for ImportEligibility {
    fn default() -> Self {
        Self::Locked
    }
}

impl ImportEligibility {
    /// Returns the boolean value to set on graph node property `import_eligible`.
    pub fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_locked() {
        assert_eq!(ImportEligibility::default(), ImportEligibility::Locked);
    }

    #[test]
    fn locked_is_not_eligible() {
        assert!(!ImportEligibility::Locked.is_eligible());
    }

    #[test]
    fn eligible_is_eligible() {
        assert!(ImportEligibility::Eligible.is_eligible());
    }

    #[test]
    fn serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&ImportEligibility::Locked).unwrap(),
            "\"locked\""
        );
        assert_eq!(
            serde_json::to_string(&ImportEligibility::Eligible).unwrap(),
            "\"eligible\""
        );
    }

    #[test]
    fn roundtrip() {
        let json = serde_json::to_string(&ImportEligibility::Eligible).unwrap();
        let back: ImportEligibility = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ImportEligibility::Eligible);
    }
}
