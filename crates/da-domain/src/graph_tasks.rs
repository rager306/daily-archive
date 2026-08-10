//! Typed semantic graph task classes (ADR-051 §8.1 / M371/S07/T04).
//!
//! Task classes keep structural, fact, summary, resolution, cluster and
//! ontology-endpoint evidence from collapsing into one boolean. Domain only —
//! no filesystem, no graph I/O.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Product task class for graph showcase / readiness suites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphTaskClass {
    /// Topology / counts / composition (hasPart, FROM_SOURCE, inCategory, scale).
    Structural,
    /// Fact edges and bibliography-style evidence (CITES, quality signals).
    Fact,
    /// Path-linked Summary presence (ADR-053 Layer B).
    Summary,
    /// Reviewed resolution / Decision audit (ADR-053 Layer G).
    Resolution,
    /// ConceptCluster membership / semantic convergence.
    Cluster,
    /// Ontology endpoint topology canaries (allowed + forbidden wires).
    OntologyEndpoint,
}

impl GraphTaskClass {
    pub const ALL: [Self; 6] = [
        Self::Structural,
        Self::Fact,
        Self::Summary,
        Self::Resolution,
        Self::Cluster,
        Self::OntologyEndpoint,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Fact => "fact",
            Self::Summary => "summary",
            Self::Resolution => "resolution",
            Self::Cluster => "cluster",
            Self::OntologyEndpoint => "ontology_endpoint",
        }
    }

    /// Default for legacy task files that omit `// @class`.
    pub fn default_for_legacy() -> Self {
        Self::Structural
    }
}

impl fmt::Display for GraphTaskClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GraphTaskClass {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "structural" | "structure" => Ok(Self::Structural),
            "fact" | "facts" => Ok(Self::Fact),
            "summary" | "summaries" => Ok(Self::Summary),
            "resolution" | "resolve" | "resolved" => Ok(Self::Resolution),
            "cluster" | "clusters" => Ok(Self::Cluster),
            "ontology_endpoint" | "ontology-endpoint" | "endpoint" | "ontology" => {
                Ok(Self::OntologyEndpoint)
            }
            other => Err(format!(
                "unknown graph task class '{other}'; expected one of: {}",
                Self::ALL
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

/// Per-class gate status for finalize / ACAP projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClassStatus {
    /// At least one task in the class executed and the class gate passed.
    Passed,
    /// Class executed and failed (EMPTY without allow_empty, or ERROR).
    Failed,
    /// Class was required/expected but no tasks were executed.
    Absent,
    /// Class not required and not executed — typed missingness, not a hard fail.
    NoClaim,
}

/// One class row in a classified suite report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskClassResult {
    pub class: GraphTaskClass,
    pub status: TaskClassStatus,
    pub task_count: usize,
    pub pass: usize,
    pub empty: usize,
    pub error: usize,
}

impl TaskClassResult {
    pub fn gate_failed(&self) -> bool {
        matches!(
            self.status,
            TaskClassStatus::Failed | TaskClassStatus::Absent
        )
    }
}

/// Class-qualified suite evidence for finalize and ACAP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClassifiedTaskEvidence {
    /// Legacy overall boolean (suite-level gate across all tasks).
    pub overall_passed: bool,
    pub by_class: BTreeMap<GraphTaskClass, TaskClassResult>,
    /// Classes that must pass for finalize when policy requires class gates.
    pub required_classes: Vec<GraphTaskClass>,
}

impl ClassifiedTaskEvidence {
    pub fn overall_only(overall_passed: bool) -> Self {
        Self {
            overall_passed,
            by_class: BTreeMap::new(),
            required_classes: Vec::new(),
        }
    }

    /// Required classes must be present and not failed.
    pub fn required_classes_satisfied(&self) -> bool {
        self.required_classes.iter().all(|class| {
            self.by_class
                .get(class)
                .map(|row| row.status == TaskClassStatus::Passed)
                .unwrap_or(false)
        })
    }

    pub fn failed_required_classes(&self) -> Vec<GraphTaskClass> {
        self.required_classes
            .iter()
            .copied()
            .filter(|class| {
                !self
                    .by_class
                    .get(class)
                    .map(|row| row.status == TaskClassStatus::Passed)
                    .unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_parse_is_fail_closed_and_aliases_work() {
        assert_eq!(
            "structural".parse::<GraphTaskClass>().unwrap(),
            GraphTaskClass::Structural
        );
        assert_eq!(
            "ontology_endpoint".parse::<GraphTaskClass>().unwrap(),
            GraphTaskClass::OntologyEndpoint
        );
        assert_eq!(
            "endpoint".parse::<GraphTaskClass>().unwrap(),
            GraphTaskClass::OntologyEndpoint
        );
        assert!("mystery".parse::<GraphTaskClass>().is_err());
    }

    #[test]
    fn required_classes_do_not_collapse_to_overall_boolean() {
        let mut by_class = BTreeMap::new();
        by_class.insert(
            GraphTaskClass::Structural,
            TaskClassResult {
                class: GraphTaskClass::Structural,
                status: TaskClassStatus::Passed,
                task_count: 3,
                pass: 3,
                empty: 0,
                error: 0,
            },
        );
        by_class.insert(
            GraphTaskClass::Summary,
            TaskClassResult {
                class: GraphTaskClass::Summary,
                status: TaskClassStatus::Failed,
                task_count: 1,
                pass: 0,
                empty: 1,
                error: 0,
            },
        );
        let evidence = ClassifiedTaskEvidence {
            overall_passed: false,
            by_class,
            required_classes: vec![GraphTaskClass::Structural, GraphTaskClass::Summary],
        };
        assert!(!evidence.required_classes_satisfied());
        assert_eq!(
            evidence.failed_required_classes(),
            vec![GraphTaskClass::Summary]
        );
        // Structural alone would satisfy if it were the only required class.
        let structural_only = ClassifiedTaskEvidence {
            overall_passed: true,
            by_class: evidence.by_class.clone(),
            required_classes: vec![GraphTaskClass::Structural],
        };
        assert!(structural_only.required_classes_satisfied());
    }

    #[test]
    fn serde_roundtrip_class_evidence() {
        let evidence = ClassifiedTaskEvidence::overall_only(true);
        let json = serde_json::to_vec(&evidence).unwrap();
        let back: ClassifiedTaskEvidence = serde_json::from_slice(&json).unwrap();
        assert_eq!(evidence, back);
    }
}
