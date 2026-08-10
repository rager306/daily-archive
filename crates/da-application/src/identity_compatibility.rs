//! Read-only identity index compatibility audit.
//!
//! The audit proves whether currently materialized values can support the
//! data-driven identity lookup policy. It never creates indexes, changes VIDs,
//! or mutates graph state.

use std::collections::BTreeMap;

use da_domain::ContentDigest;
use da_ports::DirectGraphStore;
use kg_ontology::IdentityIndexPolicy;
use serde::{Deserialize, Serialize};

pub const IDENTITY_COMPATIBILITY_SCHEMA_VERSION: &str = "ontology-v2-identity-compatibility.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityIndexStatus {
    Compatible,
    Unobserved,
    Drift,
}

impl IdentityIndexStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Unobserved => "unobserved",
            Self::Drift => "drift",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityIndexCompatibility {
    pub label: String,
    pub property: String,
    pub required: bool,
    pub node_count: usize,
    pub present_values: usize,
    pub unique_values: usize,
    pub missing_values: usize,
    pub duplicate_groups: usize,
    pub duplicate_nodes: usize,
    pub lookup_mismatches: usize,
    pub status: IdentityIndexStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityCompatibilityVerdict {
    Compatible,
    Incomplete,
    Drift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityCompatibilityReport {
    pub schema_version: String,
    pub policy_sha256: String,
    pub graph_source_sha256: String,
    pub indexes: Vec<IdentityIndexCompatibility>,
    pub observed_unique: bool,
    pub verdict: IdentityCompatibilityVerdict,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl IdentityCompatibilityReport {
    pub fn validate(&self) -> Result<(), IdentityCompatibilityError> {
        if self.schema_version != IDENTITY_COMPATIBILITY_SCHEMA_VERSION
            || self.import_eligible
            || !valid_digest(&self.policy_sha256)
            || !valid_digest(&self.graph_source_sha256)
        {
            return Err(IdentityCompatibilityError::InvalidReport(
                "schema, digest, or D127 authority drift".into(),
            ));
        }
        for index in &self.indexes {
            let expected_status = if index.node_count == 0 {
                IdentityIndexStatus::Unobserved
            } else if (index.required && index.missing_values > 0)
                || index.duplicate_groups > 0
                || index.lookup_mismatches > 0
            {
                IdentityIndexStatus::Drift
            } else {
                IdentityIndexStatus::Compatible
            };
            if index.status != expected_status
                || index.present_values + index.missing_values != index.node_count
                || index.unique_values > index.present_values
                || index.duplicate_nodes != index.present_values - index.unique_values
                || index.duplicate_groups > index.duplicate_nodes
                || index.lookup_mismatches > index.present_values
            {
                return Err(IdentityCompatibilityError::InvalidReport(format!(
                    "inconsistent metrics for {}.{}",
                    index.label, index.property
                )));
            }
        }
        let expected_observed_unique = self
            .indexes
            .iter()
            .filter(|index| index.node_count > 0)
            .all(|index| index.duplicate_groups == 0);
        if self.observed_unique != expected_observed_unique {
            return Err(IdentityCompatibilityError::InvalidReport(
                "observed_unique does not match index outcomes".into(),
            ));
        }
        if self.report_sha256 != report_digest(self) {
            return Err(IdentityCompatibilityError::InvalidReport(
                "report digest mismatch".into(),
            ));
        }
        if self.verdict != verdict_for(&self.indexes) {
            return Err(IdentityCompatibilityError::InvalidReport(
                "verdict does not match index outcomes".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityCompatibilityError {
    #[error("invalid identity compatibility input: {0}")]
    InvalidInput(String),
    #[error("invalid identity compatibility report: {0}")]
    InvalidReport(String),
}

pub struct IdentityCompatibilityUseCase<'a, T: DirectGraphStore + ?Sized> {
    store: &'a T,
}

impl<'a, T: DirectGraphStore + ?Sized> IdentityCompatibilityUseCase<'a, T> {
    pub fn new(store: &'a T) -> Self {
        Self { store }
    }

    pub async fn audit(
        &self,
        policy: &IdentityIndexPolicy,
        graph_source_sha256: &str,
    ) -> Result<IdentityCompatibilityReport, IdentityCompatibilityError> {
        if !valid_digest(graph_source_sha256) {
            return Err(IdentityCompatibilityError::InvalidInput(
                "graph source digest is malformed".into(),
            ));
        }
        let mut indexes = Vec::with_capacity(policy.indexes().len());
        for spec in policy.indexes() {
            let node_ids = self.store.get_nodes_by_label(&spec.label).await;
            let node_count = node_ids.len();
            let mut counts = BTreeMap::<String, usize>::new();
            let mut missing_values = 0;
            let mut lookup_mismatches = 0;
            for node_id in node_ids {
                match self
                    .store
                    .get_node_property_string(node_id, &spec.property)
                    .await
                    .filter(|value| !value.trim().is_empty())
                {
                    Some(value) => {
                        if self
                            .store
                            .find_node_by_string_property(&spec.label, &spec.property, &value)
                            .await
                            != Some(node_id)
                        {
                            lookup_mismatches += 1;
                        }
                        *counts.entry(value).or_default() += 1;
                    }
                    None => missing_values += 1,
                }
            }
            let present_values = counts.values().sum();
            let duplicate_groups = counts.values().filter(|count| **count > 1).count();
            let duplicate_nodes = counts
                .values()
                .filter(|count| **count > 1)
                .map(|count| count - 1)
                .sum();
            let status = if node_count == 0 {
                IdentityIndexStatus::Unobserved
            } else if (spec.required && missing_values > 0)
                || duplicate_groups > 0
                || lookup_mismatches > 0
            {
                IdentityIndexStatus::Drift
            } else {
                IdentityIndexStatus::Compatible
            };
            indexes.push(IdentityIndexCompatibility {
                label: spec.label.clone(),
                property: spec.property.clone(),
                required: spec.required,
                node_count,
                present_values,
                unique_values: counts.len(),
                missing_values,
                duplicate_groups,
                duplicate_nodes,
                lookup_mismatches,
                status,
            });
        }
        let mut report = IdentityCompatibilityReport {
            schema_version: IDENTITY_COMPATIBILITY_SCHEMA_VERSION.into(),
            policy_sha256: policy.sha256().into(),
            graph_source_sha256: graph_source_sha256.into(),
            observed_unique: indexes
                .iter()
                .filter(|index| index.node_count > 0)
                .all(|index| index.duplicate_groups == 0),
            verdict: verdict_for(&indexes),
            indexes,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_digest(&report);
        report.validate()?;
        Ok(report)
    }
}

fn verdict_for(indexes: &[IdentityIndexCompatibility]) -> IdentityCompatibilityVerdict {
    if indexes
        .iter()
        .any(|index| index.status == IdentityIndexStatus::Drift)
    {
        IdentityCompatibilityVerdict::Drift
    } else if indexes
        .iter()
        .any(|index| index.status == IdentityIndexStatus::Unobserved)
    {
        IdentityCompatibilityVerdict::Incomplete
    } else {
        IdentityCompatibilityVerdict::Compatible
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn report_digest(report: &IdentityCompatibilityReport) -> String {
    let mut copy = report.clone();
    copy.report_sha256.clear();
    let bytes = serde_json::to_vec(&copy).expect("identity report is serializable");
    format!("sha256:{}", ContentDigest::of_bytes(&bytes).as_str())
}
