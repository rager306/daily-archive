//! Ontology-driven graph finalization gate.

use crate::graph_inventory::collect_graph_inventory_for_registry;
use crate::{PipelineRunReport, StageDisposition};
use da_domain::finalize::{FinalizeFinding, FinalizePolicy, FinalizeVerdict, GraphFinalizeReport};
use da_ports::graph_store::DirectGraphStore;
use kg_ontology::registry::{OntologyRegistry, YamlNodeSchema};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};

pub struct GraphFinalizeUseCase {
    policy: FinalizePolicy,
    policy_fingerprint: String,
    registry: OntologyRegistry,
}

impl GraphFinalizeUseCase {
    pub fn load_project() -> Result<Self, String> {
        let (policy, policy_fingerprint) = FinalizePolicy::load_project()?;
        let registry =
            OntologyRegistry::load_project_meta_ontology().map_err(|error| error.to_string())?;
        Ok(Self {
            policy,
            policy_fingerprint,
            registry,
        })
    }

    pub async fn finalize(
        &self,
        store: &dyn DirectGraphStore,
        pipeline_report: &PipelineRunReport,
        semantic_tasks_passed: bool,
    ) -> GraphFinalizeReport {
        let stage_outcomes: BTreeMap<String, bool> = pipeline_report
            .stages
            .iter()
            .map(|record| {
                let passed = matches!(
                    record.disposition,
                    StageDisposition::Executed { .. } | StageDisposition::CacheHit { .. }
                );
                (record.stage_id.clone(), passed)
            })
            .collect();
        let pipeline_material = format!("{:?}", pipeline_report);
        let pipeline_report_fingerprint = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(pipeline_material.as_bytes()))
        );
        let inventory = collect_graph_inventory_for_registry(store, &self.registry).await;
        let mut findings = Vec::new();
        let mut validated_nodes = 0usize;
        let mut schema_violations = 0usize;
        let mut import_eligible_true = 0usize;
        let registered: HashSet<String> = self.registry.labels_owned().into_iter().collect();
        let mut unregistered_node_labels = Vec::new();
        for node in store.get_all_node_ids().await {
            let labels = store.get_node_labels(node).await;
            if labels.is_empty() {
                unregistered_node_labels.push(format!("<unlabeled>:{node}"));
            }
            for label in labels {
                if !registered.contains(&label) {
                    unregistered_node_labels.push(format!("{label}:{node}"));
                }
            }
        }
        unregistered_node_labels.sort();
        if !unregistered_node_labels.is_empty() {
            findings.push(FinalizeFinding {
                code: "unregistered_node_labels".into(),
                evidence: unregistered_node_labels.join(", "),
            });
        }

        for label in self.registry.labels_owned() {
            let Some(schema) = self.registry.get_schema(&label) else {
                continue;
            };
            for node in store.get_nodes_by_label(&label).await {
                let properties = collect_properties(store, node, schema).await;
                validated_nodes += 1;
                let violations =
                    da_domain::validator::validate_node_properties_or_yaml(&label, &properties);
                if !violations.is_empty() {
                    schema_violations += violations.len();
                    findings.push(FinalizeFinding {
                        code: "schema_violation".into(),
                        evidence: format!("{label} node {node}: {} violation(s)", violations.len()),
                    });
                }
                if properties.get("import_eligible").and_then(Value::as_bool) == Some(true) {
                    import_eligible_true += 1;
                }
            }
        }

        let unknown_edge_types = inventory.unknown_edge_types_for(&self.registry);
        if !unknown_edge_types.is_empty() {
            findings.push(FinalizeFinding {
                code: "unknown_edge_types".into(),
                evidence: unknown_edge_types.join(", "),
            });
        }
        if self.policy.require_nonempty_graph && inventory.total_nodes == 0 {
            findings.push(FinalizeFinding {
                code: "empty_graph".into(),
                evidence: "graph has zero ontology-registered nodes".into(),
            });
        }
        for label in &self.policy.required_nonempty_labels {
            if inventory.nodes(label) == 0 {
                findings.push(FinalizeFinding {
                    code: "required_label_empty".into(),
                    evidence: label.clone(),
                });
            }
        }
        for stage in &self.policy.required_stage_outcomes {
            if stage_outcomes.get(stage) != Some(&true) {
                findings.push(FinalizeFinding {
                    code: "required_stage_not_passed".into(),
                    evidence: stage.clone(),
                });
            }
        }
        if self.policy.semantic_tasks_required && !semantic_tasks_passed {
            findings.push(FinalizeFinding {
                code: "semantic_tasks_failed_or_missing".into(),
                evidence: "required semantic task suite did not PASS".into(),
            });
        }
        if import_eligible_true > 0 {
            findings.push(FinalizeFinding {
                code: "d127_import_unlock".into(),
                evidence: format!("{import_eligible_true} node(s) violate the D127 import lock"),
            });
        }

        let fail = (self.policy.fail_on_schema_violations && schema_violations > 0)
            || (self.policy.fail_on_unknown_edge_types && !unknown_edge_types.is_empty())
            || import_eligible_true > 0
            || !unregistered_node_labels.is_empty()
            || !semantic_tasks_passed
            || findings.iter().any(|finding| {
                matches!(
                    finding.code.as_str(),
                    "empty_graph" | "required_label_empty" | "required_stage_not_passed"
                )
            });
        let verdict = if fail {
            FinalizeVerdict::Fail
        } else {
            FinalizeVerdict::Pass
        };
        GraphFinalizeReport {
            policy_id: self.policy.policy_id.clone(),
            policy_fingerprint: self.policy_fingerprint.clone(),
            verdict,
            total_nodes: inventory.total_nodes,
            total_edges: inventory.total_edges,
            validated_nodes,
            schema_violations,
            import_eligible_true,
            unknown_edge_types,
            stage_outcomes,
            pipeline_report_fingerprint,
            semantic_tasks_passed,
            unregistered_node_labels,
            findings,
            snapshot_eligible: verdict == FinalizeVerdict::Pass
                && self.policy.snapshot_on_pass_only,
            import_eligible: false,
        }
    }
}

async fn collect_properties(
    store: &dyn DirectGraphStore,
    node: u64,
    schema: &YamlNodeSchema,
) -> HashMap<String, Value> {
    let mut properties = HashMap::new();
    for field in schema
        .required_fields
        .iter()
        .chain(schema.optional_fields.iter())
    {
        let value = match field.field_type.as_str() {
            "String" => store
                .get_node_property_string(node, &field.name)
                .await
                .map(Value::String),
            "DateTime" => match store.get_node_property_string(node, &field.name).await {
                Some(value) => Some(Value::String(value)),
                None => store
                    .get_node_property_int(node, &field.name)
                    .await
                    .map(serde_json::Number::from)
                    .map(Value::Number),
            },
            "Integer" => store
                .get_node_property_int(node, &field.name)
                .await
                .map(serde_json::Number::from)
                .map(Value::Number),
            "Float" => store
                .get_node_property_float(node, &field.name)
                .await
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number),
            "Boolean" => store
                .get_node_property_bool(node, &field.name)
                .await
                .map(Value::Bool),
            _ => None,
        };
        if let Some(value) = value {
            properties.insert(field.name.clone(), value);
        }
    }
    properties
}
