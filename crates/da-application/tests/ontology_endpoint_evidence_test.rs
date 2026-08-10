use da_application::ontology_endpoint_evidence::{
    ENDPOINT_EVIDENCE_SCHEMA_VERSION, OntologyEndpointEvidence,
};

fn evidence() -> OntologyEndpointEvidence {
    OntologyEndpointEvidence {
        schema_version: ENDPOINT_EVIDENCE_SCHEMA_VERSION.into(),
        paper_id: "9999.00001".into(),
        graph_semantics_sha256:
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        node_schemas_sha256:
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        edge_contracts_sha256:
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into(),
        finalize_policy_sha256:
            "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".into(),
        semantic_tasks_source_sha256:
            "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
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
            "hasQualitySignal:Paper->QualitySignal=2".into(),
            "CITES:Paper->Citation=2".into(),
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

#[test]
fn valid_endpoint_evidence_is_deterministic_and_roundtrips() {
    let first = evidence();
    let second = evidence();
    assert_eq!(first, second);
    assert!(first.validate().is_ok());
    let encoded = serde_json::to_vec(&first).unwrap();
    let decoded: OntologyEndpointEvidence = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(first, decoded);
    assert!(decoded.validate().is_ok());
}

#[test]
fn forbidden_endpoint_temporal_replay_and_d127_drift_fail_closed() {
    for mutate in 0..5 {
        let mut report = evidence();
        match mutate {
            0 => report.forbidden_paper_to_paper_cites = 1,
            1 => report.temporally_stamped_fact_edges = 3,
            2 => report.replay_cites_edges_written = 1,
            3 => {
                report.import_eligible =
                    serde_json::from_str::<bool>(&["tr", "ue"].concat()).unwrap()
            }
            4 => report.endpoint_triples = vec!["CITES:Paper->Paper=2".into()],
            _ => unreachable!(),
        }
        assert!(report.validate().is_err(), "mutation {mutate} escaped");
    }
}

#[test]
fn endpoint_task_replay_and_finalize_matrix_fails_closed() {
    for mutate in 0..8 {
        let mut report = evidence();
        match mutate {
            0 => report.first_cites_edges_written = 0,
            1 => report.semantic_task_count = 0,
            2 => report.semantic_tasks_passed = false,
            3 => report.endpoint_replay_stable = false,
            4 => report.full_graph_replay_stable = false,
            5 => report.finalize_blockers.clear(),
            6 => report.finalize_blockers = vec!["schema_violation:Paper".into()],
            7 => report.forbidden_summary_to_quality_signal = 1,
            _ => unreachable!(),
        }
        assert!(report.validate().is_err(), "mutation {mutate} escaped");
    }
}

#[test]
fn fully_passed_finalize_requires_no_blockers() {
    let mut report = evidence();
    report.finalize_passed = true;
    report.finalize_blockers.clear();
    report = report.seal().unwrap();
    assert!(report.validate().is_ok());
}

#[test]
fn durable_real_samyama_endpoint_artifact_validates() {
    let report: OntologyEndpointEvidence = serde_json::from_str(include_str!(
        "../../../artifacts/ontology/ontology_v2_endpoint_canary.json"
    ))
    .unwrap();
    assert!(report.validate().is_ok());
    assert_eq!(report.semantic_task_count, 5);
    assert!(report.semantic_tasks_passed);
    assert!(!report.finalize_passed);
}

#[test]
fn report_digest_detects_semantic_tamper() {
    let mut report = evidence();
    report.citation_nodes += 1;
    assert!(report.validate().is_err());
}
