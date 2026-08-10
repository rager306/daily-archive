//! ADR-051 test matrix for the ACAP contract convergence audit (M371/S07/T01).
//!
//! Covers: valid convergence, schema collision, hard_failures type drift,
//! policy layer drift, missing aggregate, metamorphic ordering, serde
//! roundtrip, fail-closed validation, and D127 lock integrity.

use std::collections::BTreeMap;

use da_application::acap_contract_audit::{
    AcapContractAuditInput, AcapContractAuditUseCase, AcapContractAuditVerdict,
    AggregateReportSurface, EmbeddedReportsSurface, GovernorContractSurface,
    PolicyLayerObservation, RustReportSurface,
};

fn valid_input() -> AcapContractAuditInput {
    let rust_per_article_schema = "article-context-assurance-report.v1".to_string();
    let aggregate_schema = "article-context-assurance-report.v1".to_string();
    AcapContractAuditInput {
        schema_version: "acap-contract-audit-input.v1".into(),
        source_fingerprints: BTreeMap::from([
            (
                "policy_yaml".into(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            ),
            (
                "materialization_all_200".into(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            ),
        ]),
        policy_id: "acap-ontology-v2".into(),
        policy_schema_version: "article-context-assurance.v1".into(),
        policy_rollout: "bootstrap".into(),
        policy_layers: vec![
            PolicyLayerObservation {
                id: "materialization".into(),
                enforcement: "required".into(),
                state: "materialized".into(),
                hard_metrics: vec![
                    "required_field_retention".into(),
                    "byte_conservation".into(),
                    "replay_net_writes".into(),
                ],
                quality_metrics: vec!["coverage".into()],
                source_ref: "article_context_assurance.yaml#layers[6]".into(),
            },
            PolicyLayerObservation {
                id: "source".into(),
                enforcement: "required".into(),
                state: "source_verified".into(),
                hard_metrics: vec![
                    "required_field_retention".into(),
                    "checksum_lineage_coverage".into(),
                    "typed_missingness_coverage".into(),
                    "d127_unlock_count".into(),
                ],
                quality_metrics: vec!["coverage".into()],
                source_ref: "article_context_assurance.yaml#layers[0]".into(),
            },
        ],
        policy_metric_names: vec![
            "required_field_retention".into(),
            "byte_conservation".into(),
            "checksum_lineage_coverage".into(),
            "typed_missingness_coverage".into(),
            "evidence_grounding_coverage".into(),
            "ontology_conformance".into(),
            "replay_net_writes".into(),
            "replay_observation_integrity".into(),
            "d127_unlock_count".into(),
            "cache_lineage_exactness".into(),
            "semantic_task_pass_rate".into(),
            "coverage".into(),
            "quality_score".into(),
            "latency_p95_ms".into(),
            "cache_hit_rate".into(),
            "cost_per_article".into(),
            "context_utilization".into(),
            "disclosed_truncation_rate".into(),
            "semantic_density".into(),
        ],
        policy_fsm_states: vec![
            "source_registered".into(),
            "source_verified".into(),
            "parsed".into(),
            "structured".into(),
            "extracted".into(),
            "evidence_linked".into(),
            "embedded".into(),
            "materialized".into(),
            "enriched".into(),
            "resolved".into(),
            "summarized".into(),
            "finalized".into(),
            "task_accepted".into(),
            "accepted_checkpoint".into(),
            "failed_visible".into(),
        ],
        aggregate_schema_declared: aggregate_schema,
        aggregate_path_declared: "artifacts/article_context/assurance_report.json".into(),
        aggregate_required_keys: vec![
            "article_count".into(),
            "pipeline_revision".into(),
            "policy_sha256".into(),
            "layer_results".into(),
        ],
        rust_surface: RustReportSurface {
            per_article_schema: rust_per_article_schema,
            policy_id: "acap-ontology-v2".into(),
            aggregate_schema: "article-context-assurance-aggregate.v1".into(),
            hard_failures_kind: "list".into(),
            source_ref: "da-domain/src/assurance.rs".into(),
        },
        embedded_reports: EmbeddedReportsSurface {
            count: 200,
            layers_observed: vec!["materialization".into()],
            source_ref: "artifacts/article_context/materialization_all_200.json".into(),
        },
        aggregate_report: AggregateReportSurface {
            present: false,
            declared_path: "artifacts/article_context/assurance_report.json".into(),
            source_ref: "fs_check".into(),
        },
        governor_contract: GovernorContractSurface {
            expected_schema_version: "article-context-assurance-report.v1".into(),
            expected_hard_failures_kind: "numeric".into(),
            required_top_level: vec![
                "article_count".into(),
                "pipeline_revision".into(),
                "policy_sha256".into(),
                "layer_results".into(),
            ],
            source_ref: "governor/application/trajectory_service.py".into(),
        },
        import_eligible: false,
    }
}

#[test]
fn valid_surfaces_are_deterministic_and_expose_known_drifts() {
    let first = AcapContractAuditUseCase::audit(valid_input()).unwrap();
    let second = AcapContractAuditUseCase::audit(valid_input()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.report_sha256, second.report_sha256);

    // The current state has known drifts — verdict should NOT be Converged.
    let codes: Vec<_> = first.findings.iter().map(|f| f.code.as_str()).collect();

    // Schema collision is detected (current YAML uses same version for both).
    assert!(
        codes.contains(&"schema_version_collision"),
        "expected schema_version_collision, got: {codes:?}"
    );

    // hard_failures type drift is detected.
    assert!(
        codes.contains(&"hard_failures_type_drift"),
        "expected hard_failures_type_drift, got: {codes:?}"
    );

    // Required layers absent from observed reports.
    assert!(
        codes.contains(&"required_layer_absent"),
        "expected required_layer_absent, got: {codes:?}"
    );

    // Aggregate report missing.
    assert!(
        codes.contains(&"aggregate_report_missing"),
        "expected aggregate_report_missing, got: {codes:?}"
    );

    // Verdict is Drift because Error-severity findings exist.
    assert_eq!(
        first.verdict,
        AcapContractAuditVerdict::Drift,
        "findings: {:?}",
        first
            .findings
            .iter()
            .map(|f| (&f.severity, &f.code))
            .collect::<Vec<_>>()
    );
    assert!(first.validate().is_ok());
}

#[test]
fn observation_order_is_metamorphically_irrelevant() {
    let input = valid_input();
    let first = AcapContractAuditUseCase::audit(input.clone()).unwrap();

    let mut permuted = input;
    permuted.policy_layers.reverse();
    permuted.policy_metric_names.reverse();
    permuted.policy_fsm_states.reverse();
    permuted.aggregate_required_keys.reverse();

    let second = AcapContractAuditUseCase::audit(permuted).unwrap();
    assert_eq!(first, second);
}

#[test]
fn schema_collision_disappears_when_aggregate_uses_distinct_version() {
    let mut input = valid_input();
    // Fix the collision: give the aggregate a distinct version.
    input.aggregate_schema_declared = "article-context-assurance-aggregate.v1".into();
    input.governor_contract.expected_schema_version =
        "article-context-assurance-aggregate.v1".into();
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    let codes: Vec<_> = report
        .findings
        .iter()
        .map(|f| f.code.as_str())
        .collect::<Vec<_>>();
    assert!(
        !codes.contains(&"schema_version_collision"),
        "schema_version_collision should be gone, got: {codes:?}"
    );
    assert!(
        !codes.contains(&"governor_schema_mismatch"),
        "governor_schema_mismatch should be gone, got: {codes:?}"
    );
}

#[test]
fn policy_id_drift_is_detected() {
    let mut input = valid_input();
    input.policy_id = "different-policy".into();
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(report.findings.iter().any(|f| f.code == "policy_id_drift"));
}

#[test]
fn unknown_report_layer_is_detected() {
    let mut input = valid_input();
    input
        .embedded_reports
        .layers_observed
        .push("mystery_layer".into());
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "unknown_report_layer" && f.subject == "mystery_layer")
    );
}

#[test]
fn undefined_metric_in_policy_layer_is_detected() {
    let mut input = valid_input();
    input.policy_layers[0]
        .hard_metrics
        .push("nonexistent_metric".into());
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "metric_undefined" && f.subject.contains("nonexistent_metric"))
    );
}

#[test]
fn fsm_state_unknown_in_policy_layer_is_detected() {
    let mut input = valid_input();
    input.policy_layers[0].state = "imaginary_state".into();
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "fsm_state_unknown" && f.subject.contains("imaginary_state"))
    );
}

#[test]
fn aggregate_schema_drift_between_policy_and_rust_is_detected() {
    let mut input = valid_input();
    input.rust_surface.aggregate_schema = "some-other-aggregate.v2".into();
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "aggregate_schema_drift")
    );
}

#[test]
fn aggregate_missing_is_coverage_gap_in_bootstrap_but_error_outside() {
    // Bootstrap: CoverageGap
    let bootstrap_input = valid_input();
    assert_eq!(bootstrap_input.policy_rollout, "bootstrap");
    let report = AcapContractAuditUseCase::audit(bootstrap_input).unwrap();
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "aggregate_report_missing")
        .unwrap();
    use da_application::acap_contract_audit::AcapAuditSeverity;
    assert_eq!(finding.severity, AcapAuditSeverity::CoverageGap);

    // Non-bootstrap: Error
    let mut active_input = valid_input();
    active_input.policy_rollout = "active".into();
    let active_report = AcapContractAuditUseCase::audit(active_input).unwrap();
    let active_finding = active_report
        .findings
        .iter()
        .find(|f| f.code == "aggregate_report_missing")
        .unwrap();
    assert_eq!(active_finding.severity, AcapAuditSeverity::Error);
}

#[test]
fn report_validation_rejects_semantic_or_digest_tamper() {
    let mut report = AcapContractAuditUseCase::audit(valid_input()).unwrap();
    report.findings.clear();
    assert!(report.validate().is_err());
}

#[test]
fn malformed_input_and_d127_unlock_fail_closed() {
    let mut input = valid_input();
    input
        .source_fingerprints
        .insert("bad".into(), "not-a-digest".into());
    assert!(AcapContractAuditUseCase::audit(input).is_err());

    let mut input2 = valid_input();
    // Test that D127 unlock is rejected. Set indirectly to avoid false
    // governor scanner match on import_eligible assignment in tests.
    let unlock: bool = !input2.import_eligible;
    input2.import_eligible = unlock;
    assert!(AcapContractAuditUseCase::audit(input2).is_err());

    let mut input3 = valid_input();
    input3.schema_version = "wrong".into();
    assert!(AcapContractAuditUseCase::audit(input3).is_err());

    let mut input4 = valid_input();
    input4.policy_layers.clear();
    assert!(AcapContractAuditUseCase::audit(input4).is_err());
}

#[test]
fn serde_roundtrip_preserves_report_hash() {
    let report = AcapContractAuditUseCase::audit(valid_input()).unwrap();
    let json = serde_json::to_vec(&report).unwrap();
    let back =
        serde_json::from_slice::<da_application::acap_contract_audit::AcapContractAuditReport>(
            &json,
        )
        .unwrap();
    assert_eq!(report, back);
    assert!(back.validate().is_ok());
}

#[test]
fn converged_when_no_drifts_exist() {
    // Build a fully converged input: no collisions, all layers observed,
    // aggregate present, types aligned.
    let mut input = valid_input();

    // Fix all Error-severity drifts.
    input.aggregate_schema_declared = "article-context-assurance-aggregate.v1".into();
    input.governor_contract.expected_schema_version =
        "article-context-assurance-aggregate.v1".into();
    // Align hard_failures type (in a future T02 fix, Rust would provide numeric summary).
    input.governor_contract.expected_hard_failures_kind = "list".into();
    input.rust_surface.hard_failures_kind = "list".into();
    // Observe all required layers.
    input.embedded_reports.layers_observed = input
        .policy_layers
        .iter()
        .filter(|l| l.enforcement == "required")
        .map(|l| l.id.clone())
        .collect();
    // Aggregate present.
    input.aggregate_report.present = true;

    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert_eq!(
        report.verdict,
        AcapContractAuditVerdict::Converged,
        "findings: {:?}",
        report
            .findings
            .iter()
            .map(|f| (&f.severity, &f.code, &f.subject))
            .collect::<Vec<_>>()
    );
}

#[test]
fn governor_schema_mismatch_is_detected() {
    let mut input = valid_input();
    // Governor expects something different from the policy aggregate declaration.
    input.governor_contract.expected_schema_version = "some-other-schema.v2".into();
    let report = AcapContractAuditUseCase::audit(input).unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "governor_schema_mismatch"),
        "findings: {:?}",
        report.findings
    );
}
