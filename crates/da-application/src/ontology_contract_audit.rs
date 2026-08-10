//! Detect-only ontology v2 contract audit (M371/S06/T01).
//!
//! The ontology registry remains the sole semantic authority. This application
//! use case compares injected project observations against that authority; it
//! does not load files, mutate ontology data, or write the graph.

use std::collections::{BTreeMap, BTreeSet};

use da_domain::ContentDigest;
use da_ports::DirectGraphStore;
use kg_ontology::registry::{ContractStatus, OntologyRegistry};
use serde::{Deserialize, Serialize};

pub const AUDIT_INPUT_SCHEMA_VERSION: &str = "ontology-contract-audit-input.v1";
pub const AUDIT_REPORT_SCHEMA_VERSION: &str = "ontology-contract-audit-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct IdentityIndexObservation {
    pub label: String,
    pub property: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SurfaceObservation {
    pub value: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TaskWireObservation {
    pub edge_type: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RuntimeEdgeObservation {
    pub edge_type: String,
    pub source_label: String,
    pub target_label: String,
    pub count: usize,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RuntimeNodeObservation {
    pub label: String,
    pub count: usize,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeGraphAuditObservation {
    pub nodes: Vec<RuntimeNodeObservation>,
    pub edges: Vec<RuntimeEdgeObservation>,
    pub coverage_gaps: Vec<SurfaceObservation>,
}

pub async fn observe_runtime_graph<T>(store: &T, source_ref: &str) -> RuntimeGraphAuditObservation
where
    T: DirectGraphStore + ?Sized,
{
    let node_ids = store.get_all_node_ids().await;
    let mut labels = BTreeMap::new();
    let mut coverage_gaps = Vec::new();
    for node_id in &node_ids {
        let mut node_labels = store.get_node_labels(*node_id).await;
        node_labels.sort();
        node_labels.dedup();
        if node_labels.is_empty() {
            coverage_gaps.push(SurfaceObservation {
                value: format!("unlabeled_node:{node_id}"),
                source_ref: source_ref.into(),
            });
        } else if node_labels.len() > 1 {
            coverage_gaps.push(SurfaceObservation {
                value: format!("ambiguous_node_labels:{node_id}:{}", node_labels.join(",")),
                source_ref: source_ref.into(),
            });
        }
        labels.insert(*node_id, node_labels);
    }

    let mut node_counts = BTreeMap::<String, usize>::new();
    for node_labels in labels.values().filter(|labels| labels.len() == 1) {
        *node_counts.entry(node_labels[0].clone()).or_default() += 1;
    }
    let nodes = node_counts
        .into_iter()
        .map(|(label, count)| RuntimeNodeObservation {
            label,
            count,
            source_ref: source_ref.into(),
        })
        .collect();

    let mut counts = BTreeMap::<(String, String, String), usize>::new();
    for source_id in node_ids {
        let source_labels = labels.get(&source_id).cloned().unwrap_or_default();
        for (target_id, edge_type) in store.get_outgoing_edges(source_id).await {
            let target_labels = labels.get(&target_id).cloned().unwrap_or_default();
            if source_labels.len() != 1 || target_labels.len() != 1 {
                coverage_gaps.push(SurfaceObservation {
                    value: format!(
                        "edge_endpoint_label_ambiguous:{source_id}->{target_id}:{edge_type}"
                    ),
                    source_ref: source_ref.into(),
                });
                continue;
            }
            *counts
                .entry((
                    edge_type,
                    source_labels[0].clone(),
                    target_labels[0].clone(),
                ))
                .or_default() += 1;
        }
    }
    let edges = counts
        .into_iter()
        .map(
            |((edge_type, source_label, target_label), count)| RuntimeEdgeObservation {
                edge_type,
                source_label,
                target_label,
                count,
                source_ref: source_ref.into(),
            },
        )
        .collect();
    coverage_gaps.sort();
    coverage_gaps.dedup();
    RuntimeGraphAuditObservation {
        nodes,
        edges,
        coverage_gaps,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractAuditInput {
    pub schema_version: String,
    pub source_fingerprints: BTreeMap<String, String>,
    pub identity_indexes: Vec<IdentityIndexObservation>,
    pub finalize_required_labels: Vec<SurfaceObservation>,
    pub task_wires: Vec<TaskWireObservation>,
    pub runtime_nodes: Vec<RuntimeNodeObservation>,
    pub runtime_edges: Vec<RuntimeEdgeObservation>,
    pub consumer_labels: Vec<SurfaceObservation>,
    pub coverage_gaps: Vec<SurfaceObservation>,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Error,
    Warning,
    CoverageGap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ContractAuditFinding {
    pub severity: AuditSeverity,
    pub code: String,
    pub subject: String,
    pub detail: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractAuditVerdict {
    Converged,
    Incomplete,
    Drift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyContractAuditReport {
    pub schema_version: String,
    pub input: ContractAuditInput,
    pub ontology_labels: usize,
    pub ontology_edges: usize,
    pub active_edges: usize,
    pub planned_edges: usize,
    pub observed_runtime_edge_types: usize,
    pub verdict: ContractAuditVerdict,
    pub findings: Vec<ContractAuditFinding>,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl OntologyContractAuditReport {
    pub fn validate(&self) -> Result<(), ContractAuditError> {
        if self.schema_version != AUDIT_REPORT_SCHEMA_VERSION || self.import_eligible {
            return Err(ContractAuditError::InvalidReport(
                "schema or D127 authority drift".into(),
            ));
        }
        if self.report_sha256 != report_digest(self) {
            return Err(ContractAuditError::InvalidReport(
                "report digest mismatch".into(),
            ));
        }
        let expected = verdict_for(&self.findings);
        if self.verdict != expected {
            return Err(ContractAuditError::InvalidReport(
                "verdict does not match findings".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContractAuditError {
    #[error("invalid ontology audit input: {0}")]
    InvalidInput(String),
    #[error("invalid ontology audit report: {0}")]
    InvalidReport(String),
}

pub struct OntologyContractAuditUseCase<'a> {
    registry: &'a OntologyRegistry,
}

impl<'a> OntologyContractAuditUseCase<'a> {
    pub fn new(registry: &'a OntologyRegistry) -> Self {
        Self { registry }
    }

    pub fn audit(
        &self,
        mut input: ContractAuditInput,
    ) -> Result<OntologyContractAuditReport, ContractAuditError> {
        validate_input(&input)?;
        normalize_input(&mut input);

        let labels: BTreeSet<String> = self.registry.labels_owned().into_iter().collect();
        let contracts = self.registry.edge_contracts();
        let mut by_edge = BTreeMap::<&str, Vec<_>>::new();
        for contract in contracts {
            by_edge
                .entry(contract.edge_constant.as_str())
                .or_default()
                .push(contract);
        }
        let mut findings = Vec::new();

        let mut seen_indexes = BTreeMap::<(String, String), String>::new();
        for index in &input.identity_indexes {
            if !labels.contains(&index.label) {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "identity_label_unknown",
                    &index.label,
                    "identity index references an unregistered label",
                    vec![index.source_ref.clone()],
                );
                continue;
            }
            let schema = self
                .registry
                .get_schema(&index.label)
                .expect("label indexed above");
            let property_exists = schema
                .required_fields
                .iter()
                .chain(&schema.optional_fields)
                .any(|field| field.name == index.property);
            if !property_exists {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "identity_property_unknown",
                    &format!("{}.{}", index.label, index.property),
                    "identity property is absent from the node schema",
                    vec![index.source_ref.clone()],
                );
            }
            let key = (index.label.clone(), index.property.clone());
            if let Some(first) = seen_indexes.insert(key, index.source_ref.clone()) {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "identity_index_duplicate",
                    &format!("{}.{}", index.label, index.property),
                    "identity index is declared more than once",
                    vec![first, index.source_ref.clone()],
                );
            }
        }

        for surface in input
            .finalize_required_labels
            .iter()
            .chain(&input.consumer_labels)
        {
            if !labels.contains(&surface.value) {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "consumer_label_unknown",
                    &surface.value,
                    "consumer references an unregistered node label",
                    vec![surface.source_ref.clone()],
                );
            }
        }

        for node in &input.runtime_nodes {
            if !labels.contains(&node.label) {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "runtime_label_unknown",
                    &node.label,
                    "runtime graph contains a node label absent from the ontology registry",
                    vec![node.source_ref.clone()],
                );
            }
        }

        for gap in &input.coverage_gaps {
            finding(
                &mut findings,
                AuditSeverity::CoverageGap,
                "observation_coverage_gap",
                &gap.value,
                "a project surface could not be observed by this audit",
                vec![gap.source_ref.clone()],
            );
        }

        for task in &input.task_wires {
            match by_edge.get(task.edge_type.as_str()) {
                None => finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "task_edge_unknown",
                    &task.edge_type,
                    "task uses an edge wire absent from the ontology registry",
                    vec![task.source_ref.clone()],
                ),
                Some(candidates)
                    if candidates.iter().all(|contract| {
                        contract.implementation_status == ContractStatus::Planned
                    }) =>
                {
                    finding(
                        &mut findings,
                        AuditSeverity::Warning,
                        "task_edge_planned",
                        &task.edge_type,
                        "task uses only planned edge contracts",
                        vec![task.source_ref.clone()],
                    );
                }
                Some(_) => {}
            }
        }

        let mut observed_edge_types = BTreeSet::new();
        for edge in &input.runtime_edges {
            observed_edge_types.insert(edge.edge_type.clone());
            let Some(candidates) = by_edge.get(edge.edge_type.as_str()) else {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "runtime_edge_unknown",
                    &edge.edge_type,
                    "runtime graph contains an unregistered edge wire",
                    vec![edge.source_ref.clone()],
                );
                continue;
            };
            let active_match = candidates.iter().any(|contract| {
                contract.implementation_status == ContractStatus::Active
                    && contract.matches(&edge.source_label, &edge.target_label)
            });
            if active_match {
                continue;
            }
            let planned_match = candidates.iter().any(|contract| {
                contract.implementation_status == ContractStatus::Planned
                    && contract.matches(&edge.source_label, &edge.target_label)
            });
            if planned_match {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "planned_edge_materialized",
                    &format!(
                        "{}:{}->{}",
                        edge.edge_type, edge.source_label, edge.target_label
                    ),
                    "runtime graph contains an edge declared only planned for these endpoints",
                    vec![edge.source_ref.clone()],
                );
            } else {
                finding(
                    &mut findings,
                    AuditSeverity::Error,
                    "runtime_endpoint_mismatch",
                    &format!(
                        "{}:{}->{}",
                        edge.edge_type, edge.source_label, edge.target_label
                    ),
                    "runtime edge endpoints violate every ontology contract for this wire",
                    vec![edge.source_ref.clone()],
                );
            }
        }

        let runtime_source = input
            .runtime_edges
            .first()
            .map(|edge| edge.source_ref.clone())
            .unwrap_or_else(|| "runtime-edge-observation-missing".into());
        for contract in contracts
            .iter()
            .filter(|contract| contract.implementation_status == ContractStatus::Active)
        {
            let observed = input.runtime_edges.iter().any(|edge| {
                edge.edge_type == contract.edge_constant
                    && contract.matches(&edge.source_label, &edge.target_label)
            });
            if !observed {
                finding(
                    &mut findings,
                    AuditSeverity::CoverageGap,
                    "active_edge_unobserved",
                    &format!(
                        "{}:{}->[{}]",
                        contract.edge_constant,
                        contract.source_label,
                        contract.target_labels.join(",")
                    ),
                    "active edge contract has no runtime observation in this audit",
                    vec![runtime_source.clone()],
                );
            }
        }

        let mut topology = BTreeSet::new();
        for contract in contracts
            .iter()
            .filter(|contract| contract.implementation_status == ContractStatus::Active)
        {
            topology.insert(contract.source_label.clone());
            topology.extend(contract.target_labels.iter().cloned());
        }
        for label in labels.iter().filter(|label| !topology.contains(*label)) {
            finding(
                &mut findings,
                AuditSeverity::CoverageGap,
                "orphan_node_topology",
                label,
                "registered node label has no active edge topology",
                vec!["ontology-registry".into()],
            );
        }

        findings.sort();
        findings.dedup();
        let active_edges = contracts
            .iter()
            .filter(|contract| contract.implementation_status == ContractStatus::Active)
            .count();
        let planned_edges = contracts.len() - active_edges;
        let mut report = OntologyContractAuditReport {
            schema_version: AUDIT_REPORT_SCHEMA_VERSION.into(),
            input,
            ontology_labels: labels.len(),
            ontology_edges: contracts.len(),
            active_edges,
            planned_edges,
            observed_runtime_edge_types: observed_edge_types.len(),
            verdict: verdict_for(&findings),
            findings,
            import_eligible: false,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_digest(&report);
        report.validate()?;
        Ok(report)
    }
}

fn validate_input(input: &ContractAuditInput) -> Result<(), ContractAuditError> {
    if input.schema_version != AUDIT_INPUT_SCHEMA_VERSION || input.import_eligible {
        return Err(ContractAuditError::InvalidInput(
            "schema or D127 authority drift".into(),
        ));
    }
    if input.source_fingerprints.is_empty()
        || input
            .source_fingerprints
            .values()
            .any(|digest| !valid_digest(digest))
    {
        return Err(ContractAuditError::InvalidInput(
            "source fingerprints are missing or malformed".into(),
        ));
    }
    let refs_valid = input
        .identity_indexes
        .iter()
        .map(|item| item.source_ref.as_str())
        .chain(
            input
                .finalize_required_labels
                .iter()
                .map(|item| item.source_ref.as_str()),
        )
        .chain(input.task_wires.iter().map(|item| item.source_ref.as_str()))
        .chain(
            input
                .runtime_nodes
                .iter()
                .map(|item| item.source_ref.as_str()),
        )
        .chain(
            input
                .runtime_edges
                .iter()
                .map(|item| item.source_ref.as_str()),
        )
        .chain(
            input
                .consumer_labels
                .iter()
                .map(|item| item.source_ref.as_str()),
        )
        .chain(
            input
                .coverage_gaps
                .iter()
                .map(|item| item.source_ref.as_str()),
        )
        .all(|source| !source.trim().is_empty());
    let subjects_valid = input
        .identity_indexes
        .iter()
        .all(|item| !item.label.trim().is_empty() && !item.property.trim().is_empty())
        && input
            .task_wires
            .iter()
            .all(|item| !item.edge_type.trim().is_empty())
        && input
            .runtime_nodes
            .iter()
            .all(|item| !item.label.trim().is_empty())
        && input.runtime_edges.iter().all(|item| {
            !item.edge_type.trim().is_empty()
                && !item.source_label.trim().is_empty()
                && !item.target_label.trim().is_empty()
        });
    if !refs_valid
        || !subjects_valid
        || input.runtime_nodes.iter().any(|node| node.count == 0)
        || input.runtime_edges.iter().any(|edge| edge.count == 0)
    {
        return Err(ContractAuditError::InvalidInput(
            "observation source or runtime count is invalid".into(),
        ));
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn normalize_input(input: &mut ContractAuditInput) {
    input.identity_indexes.sort();
    input.finalize_required_labels.sort();
    input.task_wires.sort();
    input.runtime_nodes.sort();
    input.runtime_edges.sort();
    input.consumer_labels.sort();
    input.coverage_gaps.sort();
}

fn finding(
    findings: &mut Vec<ContractAuditFinding>,
    severity: AuditSeverity,
    code: &str,
    subject: &str,
    detail: &str,
    mut source_refs: Vec<String>,
) {
    source_refs.sort();
    source_refs.dedup();
    findings.push(ContractAuditFinding {
        severity,
        code: code.into(),
        subject: subject.into(),
        detail: detail.into(),
        source_refs,
    });
}

fn verdict_for(findings: &[ContractAuditFinding]) -> ContractAuditVerdict {
    if findings
        .iter()
        .any(|finding| finding.severity == AuditSeverity::Error)
    {
        ContractAuditVerdict::Drift
    } else if findings.is_empty() {
        ContractAuditVerdict::Converged
    } else {
        ContractAuditVerdict::Incomplete
    }
}

fn report_digest(report: &OntologyContractAuditReport) -> String {
    let mut copy = report.clone();
    copy.report_sha256.clear();
    let bytes = serde_json::to_vec(&copy).expect("ontology audit report is serializable");
    format!("sha256:{}", ContentDigest::of_bytes(&bytes).as_str())
}
