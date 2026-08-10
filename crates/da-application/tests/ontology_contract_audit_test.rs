use std::collections::BTreeMap;

use da_application::ontology_contract_audit::{
    ContractAuditInput, IdentityIndexObservation, OntologyContractAuditUseCase,
    RuntimeEdgeObservation, RuntimeNodeObservation, SurfaceObservation, TaskWireObservation,
    observe_runtime_graph,
};
use kg_ontology::registry::OntologyRegistry;
use kg_storage::{DirectGraphStore, mock::MockGraphStore};

fn registry() -> OntologyRegistry {
    let root = std::env::temp_dir().join(format!(
        "da-ontology-audit-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("node_schemas.yaml"),
        r#"
node_schemas:
  - label: Root
    required_fields: [{name: vid, field_type: String}]
  - label: Child
    required_fields: [{name: vid, field_type: String}]
    optional_fields: [{name: slug, field_type: String}]
  - label: Orphan
    required_fields: [{name: vid, field_type: String}]
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("edge_contracts.yaml"),
        r#"
edge_contracts:
  - edge_constant: hasChild
    source_label: Root
    target_labels: [Child]
    temporal_class: structural
  - edge_constant: hasChild
    source_label: Child
    target_labels: [Root]
    temporal_class: structural
  - edge_constant: futureEdge
    source_label: Root
    target_labels: [Child]
    implementation_status: planned
  - edge_constant: mixedEdge
    source_label: Root
    target_labels: [Child]
  - edge_constant: mixedEdge
    source_label: Root
    target_labels: [Root]
    implementation_status: planned
"#,
    )
    .unwrap();
    let registry = OntologyRegistry::load_from_dir(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    registry
}

fn valid_input() -> ContractAuditInput {
    ContractAuditInput {
        schema_version: "ontology-contract-audit-input.v1".into(),
        source_fingerprints: BTreeMap::from([
            (
                "nodes".into(),
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            ),
            (
                "edges".into(),
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            ),
        ]),
        identity_indexes: vec![IdentityIndexObservation {
            label: "Child".into(),
            property: "slug".into(),
            source_ref: "identity.yaml#indexes[0]".into(),
        }],
        finalize_required_labels: vec![SurfaceObservation {
            value: "Root".into(),
            source_ref: "finalize.yaml#required_nonempty_labels[0]".into(),
        }],
        task_wires: vec![TaskWireObservation {
            edge_type: "hasChild".into(),
            source_ref: "tasks.cypher#task-1".into(),
        }],
        runtime_nodes: vec![
            RuntimeNodeObservation {
                label: "Root".into(),
                count: 1,
                source_ref: "canary.sgsnap".into(),
            },
            RuntimeNodeObservation {
                label: "Child".into(),
                count: 1,
                source_ref: "canary.sgsnap".into(),
            },
        ],
        runtime_edges: vec![
            RuntimeEdgeObservation {
                edge_type: "hasChild".into(),
                source_label: "Root".into(),
                target_label: "Child".into(),
                count: 1,
                source_ref: "canary.sgsnap".into(),
            },
            RuntimeEdgeObservation {
                edge_type: "mixedEdge".into(),
                source_label: "Root".into(),
                target_label: "Child".into(),
                count: 1,
                source_ref: "canary.sgsnap".into(),
            },
        ],
        consumer_labels: vec![SurfaceObservation {
            value: "Child".into(),
            source_ref: "inventory-reader".into(),
        }],
        coverage_gaps: vec![],
        import_eligible: false,
    }
}

#[test]
fn valid_surfaces_are_deterministic_and_orphans_are_visible() {
    let registry = registry();
    let use_case = OntologyContractAuditUseCase::new(&registry);
    let first = use_case.audit(valid_input()).unwrap();
    let second = use_case.audit(valid_input()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.report_sha256, second.report_sha256);
    assert!(
        first
            .findings
            .iter()
            .any(|f| f.code == "orphan_node_topology")
    );
    assert!(
        !first
            .findings
            .iter()
            .any(|f| f.code == "planned_edge_materialized")
    );
    assert!(
        first.findings.iter().any(|f| {
            f.code == "active_edge_unobserved" && f.subject == "hasChild:Child->[Root]"
        })
    );
    assert!(!first.import_eligible);
    assert!(first.validate().is_ok());
}

#[test]
fn observation_order_is_metamorphically_irrelevant() {
    let registry = registry();
    let use_case = OntologyContractAuditUseCase::new(&registry);
    let input = valid_input();
    let first = use_case.audit(input.clone()).unwrap();
    let mut permuted = input;
    permuted.identity_indexes.reverse();
    permuted.finalize_required_labels.reverse();
    permuted.task_wires.reverse();
    permuted.runtime_nodes.reverse();
    permuted.runtime_edges.reverse();
    permuted.consumer_labels.reverse();
    let second = use_case.audit(permuted).unwrap();
    assert_eq!(first, second);
}

#[test]
fn endpoint_identity_and_task_drift_are_detected_generically() {
    let mut input = valid_input();
    input.runtime_edges[0].target_label = "Root".into();
    input.identity_indexes.push(IdentityIndexObservation {
        label: "Child".into(),
        property: "missing".into(),
        source_ref: "identity.yaml#indexes[1]".into(),
    });
    input
        .identity_indexes
        .push(input.identity_indexes[0].clone());
    input.task_wires.push(TaskWireObservation {
        edge_type: "unknownEdge".into(),
        source_ref: "tasks.cypher#task-2".into(),
    });
    input.task_wires.push(TaskWireObservation {
        edge_type: "futureEdge".into(),
        source_ref: "tasks.cypher#task-3".into(),
    });
    input.runtime_nodes.push(RuntimeNodeObservation {
        label: "Invented".into(),
        count: 1,
        source_ref: "canary.sgsnap".into(),
    });
    input.runtime_edges.push(RuntimeEdgeObservation {
        edge_type: "futureEdge".into(),
        source_label: "Root".into(),
        target_label: "Child".into(),
        count: 1,
        source_ref: "canary.sgsnap".into(),
    });

    let report = OntologyContractAuditUseCase::new(&registry())
        .audit(input)
        .unwrap();
    let codes: Vec<_> = report.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(codes.contains(&"runtime_endpoint_mismatch"));
    assert!(codes.contains(&"identity_property_unknown"));
    assert!(codes.contains(&"identity_index_duplicate"));
    assert!(codes.contains(&"task_edge_unknown"));
    assert!(codes.contains(&"task_edge_planned"));
    assert!(codes.contains(&"runtime_label_unknown"));
    assert!(codes.contains(&"planned_edge_materialized"));
}

#[test]
fn report_validation_rejects_semantic_or_digest_tamper() {
    let registry = registry();
    let mut report = OntologyContractAuditUseCase::new(&registry)
        .audit(valid_input())
        .unwrap();
    report.findings.clear();
    assert!(report.validate().is_err());
}

#[test]
fn malformed_observation_and_d127_unlock_fail_closed() {
    let mut input = valid_input();
    input
        .source_fingerprints
        .insert("bad".into(), "not-a-digest".into());
    input.runtime_edges[0].count = 0;
    input.runtime_nodes[0].label.clear();
    input.import_eligible = !input.import_eligible;
    assert!(
        OntologyContractAuditUseCase::new(&registry())
            .audit(input)
            .is_err()
    );
}

#[tokio::test]
async fn runtime_observer_aggregates_real_endpoints_through_the_port() {
    let store = MockGraphStore::new();
    let root = store.create_node("Root").await.unwrap();
    let child = store.create_node("Child").await.unwrap();
    store.create_edge(root, child, "hasChild").await.unwrap();
    let observed = observe_runtime_graph(&store, "mock-canary").await;
    assert_eq!(
        observed.nodes,
        vec![
            RuntimeNodeObservation {
                label: "Child".into(),
                count: 1,
                source_ref: "mock-canary".into(),
            },
            RuntimeNodeObservation {
                label: "Root".into(),
                count: 1,
                source_ref: "mock-canary".into(),
            },
        ]
    );
    assert_eq!(
        observed.edges,
        vec![RuntimeEdgeObservation {
            edge_type: "hasChild".into(),
            source_label: "Root".into(),
            target_label: "Child".into(),
            count: 1,
            source_ref: "mock-canary".into(),
        }]
    );
    assert!(observed.coverage_gaps.is_empty());
}

#[test]
fn serde_roundtrip_preserves_report_hash() {
    let report = OntologyContractAuditUseCase::new(&registry())
        .audit(valid_input())
        .unwrap();
    let json = serde_json::to_vec(&report).unwrap();
    let back = serde_json::from_slice(&json).unwrap();
    assert_eq!(report, back);
}
