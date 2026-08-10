//! Ordered additive mutation executor (M371/S08).
//!
//! Applies a validated domain plan through DirectGraphStore and records physical
//! outcomes via kg-storage InstrumentedGraphStore. Materializer APIs are not
//! modified. D127 remains false.

use std::collections::BTreeMap;
use std::sync::Arc;

use da_domain::ordered_mutation::{
    ORDERED_MUTATION_REPORT_SCHEMA_VERSION, OrderedMutationPlan, OrderedMutationPlanError,
    OrderedMutationStep,
};
use da_ports::DirectGraphStore;
use kg_storage::{InstrumentedGraphStore, MutationCounters};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderedMutationPassKind {
    First,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedMutationPassReport {
    pub kind: OrderedMutationPassKind,
    pub counters: MutationCounters,
    pub binds: BTreeMap<String, u64>,
    pub steps_applied: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderedMutationVerdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedMutationReport {
    pub schema_version: String,
    pub plan_id: String,
    pub plan_sha256: String,
    pub first: OrderedMutationPassReport,
    pub replay: OrderedMutationPassReport,
    pub verdict: OrderedMutationVerdict,
    pub findings: Vec<String>,
    pub import_eligible: bool,
    pub report_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OrderedMutationError {
    #[error(transparent)]
    Plan(#[from] OrderedMutationPlanError),
    #[error("graph store error: {0}")]
    Graph(String),
    #[error("invalid ordered mutation report: {0}")]
    InvalidReport(String),
}

pub struct OrderedMutationUseCase;

impl OrderedMutationUseCase {
    /// Apply plan twice: first pass creates nodes/edges and sets properties;
    /// replay re-applies property steps only against first-pass binds so
    /// compare-before-write can prove Unchanged. CreateNode/CreateEdge are not
    /// re-run on replay (no create-idempotency on the universal port).
    pub async fn execute(
        plan: &OrderedMutationPlan,
        store: Arc<dyn DirectGraphStore>,
    ) -> Result<OrderedMutationReport, OrderedMutationError> {
        plan.validate()?;
        let plan_sha256 = plan.digest()?;
        let instrumented = Arc::new(InstrumentedGraphStore::new(store));

        let first = Self::run_pass(
            plan,
            instrumented.as_ref(),
            OrderedMutationPassKind::First,
            None,
        )
        .await?;
        // Replay re-applies property steps against first-pass binds so
        // compare-before-write can prove Unchanged. Re-running CreateNode would
        // allocate new ids and invent a false applied property partition.
        let replay = Self::run_pass(
            plan,
            instrumented.as_ref(),
            OrderedMutationPassKind::Replay,
            Some(&first.binds),
        )
        .await?;

        let mut findings = Vec::new();
        if first.counters.unobserved_operations != 0 {
            findings.push(format!(
                "first_unobserved:{}",
                first.counters.unobserved_operations
            ));
        }
        if replay.counters.unobserved_operations != 0 {
            findings.push(format!(
                "replay_unobserved:{}",
                replay.counters.unobserved_operations
            ));
        }
        if !first.counters.partition_is_valid() {
            findings.push("first_invalid_partition".into());
        }
        if !replay.counters.partition_is_valid() {
            findings.push("replay_invalid_partition".into());
        }
        if first.counters.rejected() != 0 {
            findings.push(format!("first_rejected:{}", first.counters.rejected()));
        }
        if replay.counters.rejected() != 0 {
            findings.push(format!("replay_rejected:{}", replay.counters.rejected()));
        }
        // Property replay must be pure unchanged (compare-before-write).
        if replay.counters.property.applied != 0 {
            findings.push(format!(
                "replay_property_applied:{}",
                replay.counters.property.applied
            ));
        }
        if first.counters.attempted() == 0 {
            findings.push("first_empty_attempt".into());
        }
        // First pass must apply something for create steps to be meaningful.
        if first.counters.applied() == 0 && first.counters.property.unchanged == 0 {
            findings.push("first_no_physical_effect".into());
        }

        let verdict = if findings.is_empty() {
            OrderedMutationVerdict::Pass
        } else {
            OrderedMutationVerdict::Fail
        };

        let mut report = OrderedMutationReport {
            schema_version: ORDERED_MUTATION_REPORT_SCHEMA_VERSION.into(),
            plan_id: plan.plan_id.clone(),
            plan_sha256,
            first,
            replay,
            verdict,
            findings,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_digest(&report);
        Ok(report)
    }

    async fn run_pass(
        plan: &OrderedMutationPlan,
        store: &InstrumentedGraphStore,
        kind: OrderedMutationPassKind,
        prior_binds: Option<&BTreeMap<String, u64>>,
    ) -> Result<OrderedMutationPassReport, OrderedMutationError> {
        store.reset().await;
        let mut binds: BTreeMap<String, u64> = prior_binds.cloned().unwrap_or_default();
        let mut steps_applied = 0_usize;
        for step in &plan.steps {
            match step {
                OrderedMutationStep::CreateNode { label, bind } => {
                    if kind == OrderedMutationPassKind::Replay {
                        // Bind already established on first pass; do not allocate.
                        if !binds.contains_key(bind) {
                            return Err(OrderedMutationError::Graph(format!(
                                "replay missing bind {bind}"
                            )));
                        }
                        continue;
                    }
                    let id = store
                        .create_node(label)
                        .await
                        .map_err(|e| OrderedMutationError::Graph(e.to_string()))?;
                    binds.insert(bind.clone(), id);
                    steps_applied += 1;
                }
                OrderedMutationStep::SetString { bind, key, value } => {
                    let id = *binds.get(bind).ok_or_else(|| {
                        OrderedMutationError::Graph(format!("missing bind {bind}"))
                    })?;
                    store
                        .set_node_property_string(id, key, value.clone())
                        .await
                        .map_err(|e| OrderedMutationError::Graph(e.to_string()))?;
                    steps_applied += 1;
                }
                OrderedMutationStep::SetInt { bind, key, value } => {
                    let id = *binds.get(bind).ok_or_else(|| {
                        OrderedMutationError::Graph(format!("missing bind {bind}"))
                    })?;
                    store
                        .set_node_property_int(id, key, *value)
                        .await
                        .map_err(|e| OrderedMutationError::Graph(e.to_string()))?;
                    steps_applied += 1;
                }
                OrderedMutationStep::SetBool { bind, key, value } => {
                    let id = *binds.get(bind).ok_or_else(|| {
                        OrderedMutationError::Graph(format!("missing bind {bind}"))
                    })?;
                    store
                        .set_node_property_bool(id, key, *value)
                        .await
                        .map_err(|e| OrderedMutationError::Graph(e.to_string()))?;
                    steps_applied += 1;
                }
                OrderedMutationStep::CreateEdge {
                    source_bind,
                    target_bind,
                    edge_type,
                } => {
                    if kind == OrderedMutationPassKind::Replay {
                        // Edge create is not compare-before-write; skip on replay.
                        continue;
                    }
                    let source = *binds.get(source_bind).ok_or_else(|| {
                        OrderedMutationError::Graph(format!("missing bind {source_bind}"))
                    })?;
                    let target = *binds.get(target_bind).ok_or_else(|| {
                        OrderedMutationError::Graph(format!("missing bind {target_bind}"))
                    })?;
                    store
                        .create_edge(source, target, edge_type)
                        .await
                        .map_err(|e| OrderedMutationError::Graph(e.to_string()))?;
                    steps_applied += 1;
                }
            }
        }
        let counters = store.snapshot().await;
        Ok(OrderedMutationPassReport {
            kind,
            counters,
            binds,
            steps_applied,
        })
    }
}

fn report_digest(report: &OrderedMutationReport) -> String {
    let mut clone = report.clone();
    clone.report_sha256 = String::new();
    let json = serde_json::to_string(&clone).expect("report serializes");
    let hash = Sha256::digest(json.as_bytes());
    let mut binder = Vec::with_capacity(hash.len() + ORDERED_MUTATION_REPORT_SCHEMA_VERSION.len());
    binder.extend_from_slice(&hash);
    binder.extend_from_slice(ORDERED_MUTATION_REPORT_SCHEMA_VERSION.as_bytes());
    hex::encode(Sha256::digest(&binder))
}
