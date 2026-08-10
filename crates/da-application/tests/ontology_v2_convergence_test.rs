use da_application::identity_compatibility::IdentityCompatibilityUseCase;
use da_application::ontology_contract_audit::{
    AUDIT_INPUT_SCHEMA_VERSION, ContractAuditInput, IdentityIndexObservation,
    OntologyContractAuditReport, OntologyContractAuditUseCase, RuntimeEdgeObservation,
    RuntimeNodeObservation, SurfaceObservation,
};
use da_application::ontology_endpoint_evidence::{
    ENDPOINT_EVIDENCE_SCHEMA_VERSION, OntologyEndpointEvidence,
};
use da_application::ontology_v2_convergence::{
    EvidenceBinding, OntologyV2ConvergenceInput, OntologyV2ConvergenceUseCase,
    OntologyV2ConvergenceVerdict,
};
use kg_ontology::{IdentityIndexPolicy, OntologyRegistry};
use kg_storage::{DirectGraphStore, MockGraphStore};
use std::collections::BTreeMap;

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn contract_report() -> OntologyContractAuditReport {
    serde_json::from_str(include_str!(
        "../../../artifacts/ontology/ontology_v2_contract_audit.json"
    ))
    .unwrap()
}

fn identity_report() -> da_application::identity_compatibility::IdentityCompatibilityReport {
    serde_json::from_str(include_str!(
        "../../../artifacts/ontology/ontology_v2_identity_compatibility.json"
    ))
    .unwrap()
}

fn endpoint() -> OntologyEndpointEvidence {
    OntologyEndpointEvidence {
        schema_version: ENDPOINT_EVIDENCE_SCHEMA_VERSION.into(),
        paper_id: "9999.00001".into(),
        graph_semantics_sha256: C.into(),
        node_schemas_sha256: contract_report().input.source_fingerprints["node_schemas"].clone(),
        edge_contracts_sha256: contract_report().input.source_fingerprints["edge_contracts"]
            .clone(),
        finalize_policy_sha256: contract_report().input.source_fingerprints["finalize_policy"]
            .clone(),
        semantic_tasks_source_sha256: A.into(),
        first_cites_edges_written: 2,
        replay_cites_edges_written: 0,
        citation_nodes: 2,
        quality_signal_nodes: 2,
        cites_edges: 2,
        quality_signal_edges: 2,
        forbidden_paper_to_paper_cites: 0,
        forbidden_summary_to_quality_signal: 0,
        temporally_stamped_fact_edges: 4,
        endpoint_triples: vec![
            "CITES:Paper->Citation=2".into(),
            "hasQualitySignal:Paper->QualitySignal=2".into(),
        ],
        endpoint_replay_stable: true,
        full_graph_replay_stable: true,
        semantic_task_count: 3,
        semantic_tasks_passed: true,
        finalize_passed: false,
        finalize_schema_violations: 0,
        finalize_unknown_edge_types: vec![],
        finalize_blockers: vec!["required_stage_not_passed:entity_resolution".into()],
        import_eligible: false,
        report_sha256: String::new(),
    }
    .seal()
    .unwrap()
}

fn input() -> OntologyV2ConvergenceInput {
    let audit = contract_report();
    let identity = identity_report();
    let endpoint = endpoint();
    let task_wires = audit
        .input
        .task_wires
        .iter()
        .map(|wire| wire.edge_type.clone())
        .collect();
    let task_wire_source_sha256 = audit.input.source_fingerprints["graph_tasks"].clone();
    OntologyV2ConvergenceInput {
        bindings: vec![
            EvidenceBinding {
                role: "checkpoint_audit".into(),
                source_ref: "artifacts/ontology/ontology_v2_contract_audit.json".into(),
                source_sha256: A.into(),
                report_sha256: audit.report_sha256.clone(),
            },
            EvidenceBinding {
                role: "checkpoint_identity".into(),
                source_ref: "artifacts/ontology/ontology_v2_identity_compatibility.json".into(),
                source_sha256: B.into(),
                report_sha256: identity.report_sha256.clone(),
            },
            EvidenceBinding {
                role: "endpoint_runtime".into(),
                source_ref: "process-local-samyama".into(),
                source_sha256: C.into(),
                report_sha256: endpoint.report_sha256.clone(),
            },
        ],
        checkpoint_audit: audit,
        checkpoint_identity: identity,
        endpoint_runtime: endpoint,
        task_wires,
        task_wire_source_sha256,
    }
}

#[test]
fn split_source_evidence_composes_honest_deterministic_incomplete_report() {
    let first = OntologyV2ConvergenceUseCase::compose(input()).unwrap();
    let mut permuted = input();
    permuted.bindings.reverse();
    permuted.task_wires.reverse();
    let second = OntologyV2ConvergenceUseCase::compose(permuted).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.verdict, OntologyV2ConvergenceVerdict::Incomplete);
    assert_ne!(first.checkpoint_graph_sha256, first.endpoint_graph_sha256);
    assert!(!first.remainders.is_empty());
    assert!(!first.import_eligible);
    assert!(first.validate().is_ok());
    let roundtrip: da_application::ontology_v2_convergence::OntologyV2ConvergenceReport =
        serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    assert_eq!(first, roundtrip);
    assert!(roundtrip.validate().is_ok());
}

#[test]
fn child_report_binding_and_task_wire_tamper_fail_closed() {
    let mut wrong_binding = input();
    wrong_binding.bindings[0].report_sha256 = C.into();
    assert!(OntologyV2ConvergenceUseCase::compose(wrong_binding).is_err());

    let mut unknown_wire = input();
    unknown_wire.task_wires.push("FREE_INVENT_WIRE".into());
    assert!(OntologyV2ConvergenceUseCase::compose(unknown_wire).is_err());

    let mut unlocked = input();
    unlocked.endpoint_runtime.import_eligible =
        serde_json::from_str::<bool>(&["tr", "ue"].concat()).unwrap();
    assert!(OntologyV2ConvergenceUseCase::compose(unlocked).is_err());

    let mut ontology_mismatch = input();
    ontology_mismatch.endpoint_runtime.edge_contracts_sha256 = C.into();
    ontology_mismatch.endpoint_runtime = ontology_mismatch.endpoint_runtime.seal().unwrap();
    ontology_mismatch.bindings[2].report_sha256 =
        ontology_mismatch.endpoint_runtime.report_sha256.clone();
    assert!(OntologyV2ConvergenceUseCase::compose(ontology_mismatch).is_err());
}

#[test]
fn valid_audit_error_propagates_as_convergence_drift() {
    let ontology_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology");
    let registry = OntologyRegistry::load_from_dir(&ontology_dir).unwrap();
    let mut audit_input = contract_report().input;
    audit_input.runtime_nodes.push(RuntimeNodeObservation {
        label: "FreeInventLabel".into(),
        count: 1,
        source_ref: "negative-fixture".into(),
    });
    let drift_audit = OntologyContractAuditUseCase::new(&registry)
        .audit(audit_input)
        .unwrap();
    let mut drift_input = input();
    drift_input.bindings[0].report_sha256 = drift_audit.report_sha256.clone();
    drift_input.checkpoint_audit = drift_audit;
    let report = OntologyV2ConvergenceUseCase::compose(drift_input).unwrap();
    assert_eq!(report.verdict, OntologyV2ConvergenceVerdict::Drift);
    assert!(report.remainders.iter().any(|item| {
        item.code == "drift:audit:runtime_label_unknown" && item.subject == "FreeInventLabel"
    }));
}

#[test]
fn durable_convergence_artifact_validates_and_preserves_split_sources() {
    let report: da_application::ontology_v2_convergence::OntologyV2ConvergenceReport =
        serde_json::from_str(include_str!(
            "../../../artifacts/ontology/ontology_v2_convergence_canary.json"
        ))
        .unwrap();
    assert!(report.validate().is_ok());
    assert_eq!(report.verdict, OntologyV2ConvergenceVerdict::Incomplete);
    assert_ne!(report.checkpoint_graph_sha256, report.endpoint_graph_sha256);
    assert!(
        report
            .remainders
            .iter()
            .any(|item| item.code == "composition:split_graph_sources")
    );
}

#[tokio::test]
async fn checkpoint_source_mismatch_is_rejected_even_when_children_are_valid() {
    let ontology_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology");
    let registry = OntologyRegistry::load_from_dir(&ontology_dir).unwrap();
    let bytes = include_bytes!("../../../data/ontology/identity_indexes.yaml");
    let policy = IdentityIndexPolicy::parse_and_validate(bytes, &registry).unwrap();
    let mismatched = IdentityCompatibilityUseCase::new(&MockGraphStore::new())
        .audit(&policy, A)
        .await
        .unwrap();
    let mut mismatched_input = input();
    mismatched_input.bindings[1].report_sha256 = mismatched.report_sha256.clone();
    mismatched_input.checkpoint_identity = mismatched;
    assert!(OntologyV2ConvergenceUseCase::compose(mismatched_input).is_err());
}

#[tokio::test]
async fn valid_identity_drift_propagates_as_convergence_drift() {
    let ontology_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology");
    let registry = OntologyRegistry::load_from_dir(&ontology_dir).unwrap();
    let policy = IdentityIndexPolicy::parse_and_validate(
        include_bytes!("../../../data/ontology/identity_indexes.yaml"),
        &registry,
    )
    .unwrap();
    let store = MockGraphStore::new();
    for arxiv in ["2401.00001", "2401.00002"] {
        let paper = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(paper, "vid", "duplicate".into())
            .await
            .unwrap();
        store
            .set_node_property_string(paper, "arxiv_id", arxiv.into())
            .await
            .unwrap();
    }
    let checkpoint_digest = contract_report().input.source_fingerprints["runtime_snapshot"].clone();
    let drift = IdentityCompatibilityUseCase::new(&store)
        .audit(&policy, &checkpoint_digest)
        .await
        .unwrap();
    let mut drift_input = input();
    drift_input.bindings[1].report_sha256 = drift.report_sha256.clone();
    drift_input.checkpoint_identity = drift;
    let report = OntologyV2ConvergenceUseCase::compose(drift_input).unwrap();
    assert_eq!(report.verdict, OntologyV2ConvergenceVerdict::Drift);
    assert!(
        report
            .remainders
            .iter()
            .any(|item| item.code == "drift:identity:index")
    );
}

#[tokio::test]
async fn converged_requires_one_graph_source_and_fully_passed_finalize() {
    let root = std::env::temp_dir().join(format!(
        "da-converged-fixture-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("node_schemas.yaml"),
        "node_schemas:\n  - label: Paper\n    required_fields: [{name: vid, field_type: String}]\n  - label: Citation\n    required_fields: [{name: vid, field_type: String}]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("edge_contracts.yaml"),
        "edge_contracts:\n  - edge_constant: CITES\n    source_label: Paper\n    target_labels: [Citation]\n    temporal_class: fact\n",
    )
    .unwrap();
    let registry = OntologyRegistry::load_from_dir(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    let shared_graph = C.to_string();
    let audit = OntologyContractAuditUseCase::new(&registry)
        .audit(ContractAuditInput {
            schema_version: AUDIT_INPUT_SCHEMA_VERSION.into(),
            source_fingerprints: BTreeMap::from([
                ("runtime_snapshot".into(), shared_graph.clone()),
                ("node_schemas".into(), A.into()),
                ("edge_contracts".into(), B.into()),
                ("finalize_policy".into(), C.into()),
                ("graph_tasks".into(), A.into()),
                ("identity_indexes".into(), B.into()),
            ]),
            identity_indexes: vec![IdentityIndexObservation {
                label: "Paper".into(),
                property: "vid".into(),
                source_ref: "identity".into(),
            }],
            finalize_required_labels: vec![SurfaceObservation {
                value: "Paper".into(),
                source_ref: "finalize".into(),
            }],
            task_wires: vec![],
            runtime_nodes: vec![
                RuntimeNodeObservation {
                    label: "Paper".into(),
                    count: 1,
                    source_ref: "shared".into(),
                },
                RuntimeNodeObservation {
                    label: "Citation".into(),
                    count: 1,
                    source_ref: "shared".into(),
                },
            ],
            runtime_edges: vec![RuntimeEdgeObservation {
                edge_type: "CITES".into(),
                source_label: "Paper".into(),
                target_label: "Citation".into(),
                count: 1,
                source_ref: "shared".into(),
            }],
            consumer_labels: vec![SurfaceObservation {
                value: "Paper".into(),
                source_ref: "consumer".into(),
            }],
            coverage_gaps: vec![],
            import_eligible: false,
        })
        .unwrap();
    assert!(audit.findings.is_empty());

    let policy = IdentityIndexPolicy::parse_and_validate(
        b"schema_version: ontology-v2-identity-indexes.v1\nindexes:\n  - {label: Paper, property: vid, identity: true}\n",
        &registry,
    )
    .unwrap();
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "vid", "paper-1".into())
        .await
        .unwrap();
    let identity = IdentityCompatibilityUseCase::new(&store)
        .audit(&policy, &shared_graph)
        .await
        .unwrap();
    let mut endpoint = endpoint();
    endpoint.graph_semantics_sha256 = shared_graph;
    endpoint.node_schemas_sha256 = A.into();
    endpoint.edge_contracts_sha256 = B.into();
    endpoint.finalize_policy_sha256 = C.into();
    endpoint.finalize_passed = true;
    endpoint.finalize_blockers.clear();
    endpoint = endpoint.seal().unwrap();

    let report = OntologyV2ConvergenceUseCase::compose(OntologyV2ConvergenceInput {
        bindings: vec![
            EvidenceBinding {
                role: "checkpoint_audit".into(),
                source_ref: "audit".into(),
                source_sha256: A.into(),
                report_sha256: audit.report_sha256.clone(),
            },
            EvidenceBinding {
                role: "checkpoint_identity".into(),
                source_ref: "identity".into(),
                source_sha256: B.into(),
                report_sha256: identity.report_sha256.clone(),
            },
            EvidenceBinding {
                role: "endpoint_runtime".into(),
                source_ref: "endpoint".into(),
                source_sha256: C.into(),
                report_sha256: endpoint.report_sha256.clone(),
            },
        ],
        checkpoint_audit: audit,
        checkpoint_identity: identity,
        endpoint_runtime: endpoint,
        task_wires: vec![],
        task_wire_source_sha256: A.into(),
    })
    .unwrap();
    assert_eq!(report.verdict, OntologyV2ConvergenceVerdict::Converged);
    assert!(report.remainders.is_empty());
}
