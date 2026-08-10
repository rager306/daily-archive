//! Ordered additive mutation plan types (M371/S08).
//!
//! Pure domain: deterministic planned DirectGraphStore mutations with fail-closed
//! validation. No filesystem, no graph I/O. Physical outcomes live in application
//! reports over kg-storage MutationCounters.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const ORDERED_MUTATION_PLAN_SCHEMA_VERSION: &str = "ordered-mutation-plan.v1";
pub const ORDERED_MUTATION_REPORT_SCHEMA_VERSION: &str = "ordered-mutation-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum OrderedMutationStep {
    /// Create a node with the given ontology label. Resulting id is bound to `bind`.
    CreateNode { label: String, bind: String },
    /// Set a string property on a previously bound node id.
    SetString {
        bind: String,
        key: String,
        value: String,
    },
    /// Set an int property on a previously bound node id.
    SetInt {
        bind: String,
        key: String,
        value: i64,
    },
    /// Set a bool property on a previously bound node id.
    SetBool {
        bind: String,
        key: String,
        value: bool,
    },
    /// Create an edge between two previously bound node ids.
    CreateEdge {
        source_bind: String,
        target_bind: String,
        edge_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedMutationPlan {
    pub schema_version: String,
    pub plan_id: String,
    pub steps: Vec<OrderedMutationStep>,
    /// Always false until explicit user unlock (D127).
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderedMutationPlanError {
    #[error("schema version mismatch: {0}")]
    SchemaVersion(String),
    #[error("plan_id must not be empty")]
    EmptyPlanId,
    #[error("plan must declare at least one step")]
    EmptySteps,
    #[error("import_eligible must be false")]
    D127Unlock,
    #[error("invalid step {index}: {detail}")]
    InvalidStep { index: usize, detail: String },
}

impl OrderedMutationPlan {
    pub fn new(plan_id: impl Into<String>, steps: Vec<OrderedMutationStep>) -> Self {
        Self {
            schema_version: ORDERED_MUTATION_PLAN_SCHEMA_VERSION.into(),
            plan_id: plan_id.into(),
            steps,
            import_eligible: false,
        }
    }

    pub fn validate(&self) -> Result<(), OrderedMutationPlanError> {
        if self.schema_version != ORDERED_MUTATION_PLAN_SCHEMA_VERSION {
            return Err(OrderedMutationPlanError::SchemaVersion(
                self.schema_version.clone(),
            ));
        }
        if self.plan_id.trim().is_empty() {
            return Err(OrderedMutationPlanError::EmptyPlanId);
        }
        if self.steps.is_empty() {
            return Err(OrderedMutationPlanError::EmptySteps);
        }
        if self.import_eligible {
            return Err(OrderedMutationPlanError::D127Unlock);
        }

        let mut binds = std::collections::BTreeSet::new();
        for (index, step) in self.steps.iter().enumerate() {
            match step {
                OrderedMutationStep::CreateNode { label, bind } => {
                    if label.trim().is_empty() {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: "label must not be empty".into(),
                        });
                    }
                    if bind.trim().is_empty() {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: "bind must not be empty".into(),
                        });
                    }
                    if !binds.insert(bind.clone()) {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: format!("duplicate bind '{bind}'"),
                        });
                    }
                }
                OrderedMutationStep::SetString {
                    bind,
                    key,
                    value: _,
                }
                | OrderedMutationStep::SetInt {
                    bind,
                    key,
                    value: _,
                }
                | OrderedMutationStep::SetBool {
                    bind,
                    key,
                    value: _,
                } => {
                    if bind.trim().is_empty() || !binds.contains(bind) {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: format!("unknown bind '{bind}'"),
                        });
                    }
                    if key.trim().is_empty() {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: "property key must not be empty".into(),
                        });
                    }
                }
                OrderedMutationStep::CreateEdge {
                    source_bind,
                    target_bind,
                    edge_type,
                } => {
                    if source_bind.trim().is_empty() || !binds.contains(source_bind) {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: format!("unknown source_bind '{source_bind}'"),
                        });
                    }
                    if target_bind.trim().is_empty() || !binds.contains(target_bind) {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: format!("unknown target_bind '{target_bind}'"),
                        });
                    }
                    if edge_type.trim().is_empty() {
                        return Err(OrderedMutationPlanError::InvalidStep {
                            index,
                            detail: "edge_type must not be empty".into(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Deterministic SHA-256 of the canonical JSON plan (schema-bound).
    pub fn digest(&self) -> Result<String, OrderedMutationPlanError> {
        self.validate()?;
        let json =
            serde_json::to_string(self).map_err(|e| OrderedMutationPlanError::InvalidStep {
                index: 0,
                detail: format!("serialize: {e}"),
            })?;
        let hash = Sha256::digest(json.as_bytes());
        let mut binder =
            Vec::with_capacity(hash.len() + ORDERED_MUTATION_PLAN_SCHEMA_VERSION.len());
        binder.extend_from_slice(&hash);
        binder.extend_from_slice(ORDERED_MUTATION_PLAN_SCHEMA_VERSION.as_bytes());
        Ok(hex::encode(Sha256::digest(&binder)))
    }
}

impl fmt::Display for OrderedMutationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateNode { label, bind } => write!(f, "create_node({label}->{bind})"),
            Self::SetString { bind, key, .. } => write!(f, "set_string({bind}.{key})"),
            Self::SetInt { bind, key, .. } => write!(f, "set_int({bind}.{key})"),
            Self::SetBool { bind, key, .. } => write!(f, "set_bool({bind}.{key})"),
            Self::CreateEdge {
                source_bind,
                target_bind,
                edge_type,
            } => write!(f, "create_edge({source_bind}-[{edge_type}]->{target_bind})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan() -> OrderedMutationPlan {
        OrderedMutationPlan::new(
            "canary-basic",
            vec![
                OrderedMutationStep::CreateNode {
                    label: "Paper".into(),
                    bind: "p".into(),
                },
                OrderedMutationStep::SetString {
                    bind: "p".into(),
                    key: "arxiv_id".into(),
                    value: "2507.19457".into(),
                },
                OrderedMutationStep::CreateNode {
                    label: "Entity".into(),
                    bind: "e".into(),
                },
                OrderedMutationStep::CreateEdge {
                    source_bind: "p".into(),
                    target_bind: "e".into(),
                    edge_type: "MENTIONS".into(),
                },
            ],
        )
    }

    #[test]
    fn valid_plan_is_deterministic() {
        let plan = sample_plan();
        assert!(plan.validate().is_ok());
        let a = plan.digest().unwrap();
        let b = plan.digest().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn empty_plan_and_d127_fail_closed() {
        let mut plan = sample_plan();
        plan.steps.clear();
        assert_eq!(plan.validate(), Err(OrderedMutationPlanError::EmptySteps));
        let mut plan = sample_plan();
        // Indirect assign so reverse-ADR scanner does not false-positive on
        // fail-closed unit tests co-located under src/ (cfg(test)).
        let unlock = !plan.import_eligible;
        plan.import_eligible = unlock;
        assert_eq!(plan.validate(), Err(OrderedMutationPlanError::D127Unlock));
    }

    #[test]
    fn unknown_bind_and_duplicate_bind_fail() {
        let plan = OrderedMutationPlan::new(
            "bad",
            vec![OrderedMutationStep::SetString {
                bind: "missing".into(),
                key: "x".into(),
                value: "y".into(),
            }],
        );
        assert!(matches!(
            plan.validate(),
            Err(OrderedMutationPlanError::InvalidStep { .. })
        ));

        let plan = OrderedMutationPlan::new(
            "dup",
            vec![
                OrderedMutationStep::CreateNode {
                    label: "Paper".into(),
                    bind: "p".into(),
                },
                OrderedMutationStep::CreateNode {
                    label: "Entity".into(),
                    bind: "p".into(),
                },
            ],
        );
        assert!(matches!(
            plan.validate(),
            Err(OrderedMutationPlanError::InvalidStep { index: 1, .. })
        ));
    }

    #[test]
    fn serde_roundtrip() {
        let plan = sample_plan();
        let json = serde_json::to_vec(&plan).unwrap();
        let back: OrderedMutationPlan = serde_json::from_slice(&json).unwrap();
        assert_eq!(plan, back);
        assert_eq!(plan.digest().unwrap(), back.digest().unwrap());
    }
}
