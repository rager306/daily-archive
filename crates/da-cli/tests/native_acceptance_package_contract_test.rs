//! S08/T05 (M514-1lss2s) — contract test for the GROBID retirement acceptance
//! package.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it
//! rather than compiling an empty target. It re-derives the evidence sha256 from
//! the one committed carrier table, independently re-runs the T02 evaluator over
//! the committed carriers, binds the committed artifact, asserts its canonical
//! byte re-serialization, and rejects each named mutation.

use da_cli::native_acceptance::NativeAcceptanceReceipt;
use da_cli::native_acceptance_package::{
    AcceptanceGate, AcceptancePackageExpectation, DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL,
    GATE_STATE_VOCABULARY, GrobidRetirementAcceptancePackage, JAVA_CPU_GATE_ID,
    LEARNING_DISPOSITION_GATE_ID, NAMED_GAP_H20, NAMED_GAP_H21, NAMED_GAP_LEARNING_RUVLLM_ABLATION,
    PACKAGE_BODY_RESIDUAL_REL, PACKAGE_COMPATIBILITY_REL, PACKAGE_DB_CANONICAL_REQUIREMENT_IDS,
    PACKAGE_EVIDENCE_CARRIERS, PACKAGE_NATIVE_MEASUREMENT_REL, PACKAGE_REFERENCE_PINS_REL,
    PACKAGE_REFERENCE_PROBE_REL, PACKAGE_RESOURCE_RECEIPT_REL, PACKAGE_SCHEMA_REL,
    RESOURCE_PAIR_GATE_KIND, gate_blocks_s09, validate_acceptance_package,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_resource_same_n::SAME_N_ABSENCE_REASON_VOCABULARY;
use da_parse::{
    BodyAxisResidual, CandidateMeasurement, MeasuredMetric, NativeAcceptanceInput,
    PopulationDenominator, ReferenceDocument, ReferenceSet, Series, evaluate_native_acceptance,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

fn read_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(read(root, rel)?)?)
}

fn expected_evidence(root: &Path) -> Result<Vec<CarrierBinding>, Box<dyn Error>> {
    PACKAGE_EVIDENCE_CARRIERS
        .iter()
        .map(|(rel, pointer)| {
            Ok(CarrierBinding {
                manifest: (*rel).to_owned(),
                pointer: (*pointer).to_owned(),
                revision_sha256: sha256_prefixed(&read(root, rel)?),
            })
        })
        .collect()
}

fn find_key(value: &serde_json::Value, wanted: &str) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            map.contains_key(wanted) || map.values().any(|child| find_key(child, wanted))
        }
        serde_json::Value::Array(items) => items.iter().any(|child| find_key(child, wanted)),
        _ => false,
    }
}

fn closed_locks() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("deployed".to_owned(), "false".to_owned()),
        ("import_eligible".to_owned(), "false".to_owned()),
        ("auto_promote".to_owned(), "forbidden".to_owned()),
        ("production_route".to_owned(), "forbidden".to_owned()),
        ("graph_write".to_owned(), "forbidden".to_owned()),
    ])
}

/// Independently re-run the T02 evaluator over the committed carriers. This is
/// what makes the package's carried evaluation a re-derivation, not a restated
/// claim.
fn re_run_evaluator(root: &Path) -> Result<da_parse::NativeAcceptanceEvaluation, Box<dyn Error>> {
    let t01 =
        NativeAcceptanceReceipt::parse_json_bytes(&read(root, PACKAGE_NATIVE_MEASUREMENT_REL)?)?;
    let contract: serde_json::Value = serde_yaml::from_slice(&read(root, PACKAGE_SCHEMA_REL)?)?;
    let pins_bytes = read(root, PACKAGE_REFERENCE_PINS_REL)?;
    let pins: serde_json::Value = serde_json::from_slice(&pins_bytes)?;
    let probe: serde_json::Value =
        serde_json::from_slice(&read(root, PACKAGE_REFERENCE_PROBE_REL)?)?;

    let series = Series {
        corpus_id: contract["assigned_population"]["census_id"]
            .as_str()
            .ok_or("census id")?
            .to_owned(),
        profile: probe["executed_population"]["execution_route_profile"]
            .as_str()
            .ok_or("execution route profile")?
            .to_owned(),
        oracle: probe["evaluated_population"]["oracle"]
            .as_str()
            .ok_or("evaluated oracle")?
            .to_owned(),
        source_revision: contract["threshold_policy"]["freeze_revision"]
            .as_str()
            .ok_or("freeze revision")?
            .to_owned(),
    };

    let mut exclusion_reasons: BTreeMap<String, u64> = BTreeMap::new();
    for document in &t01.documents {
        if let Some(reason) = &document.exclusion_reason {
            *exclusion_reasons.entry(reason.clone()).or_insert(0) += 1;
        }
    }
    let candidate = CandidateMeasurement {
        source_path: PACKAGE_NATIVE_MEASUREMENT_REL.to_owned(),
        series: Some(series.clone()),
        metrics: t01
            .metrics
            .iter()
            .map(|row| MeasuredMetric {
                metric_id: row.metric_id.clone(),
                numerator: Some(row.numerator as f64),
                denominator: Some(row.denominator as f64),
                stratum: row.stratum.clone(),
                series: None,
            })
            .collect(),
        denominator: PopulationDenominator {
            assigned: t01.populations.assigned,
            executed: t01.populations.measurable,
            refused: t01.populations.refused,
            excluded: t01.populations.excluded,
            exclusion_reasons,
        },
    };
    let body: serde_json::Value = serde_json::from_slice(&read(root, PACKAGE_BODY_RESIDUAL_REL)?)?;
    assert!(
        find_key(&body, "p1_localization_uncertain"),
        "the body-residual carrier lost the named p1 residual"
    );
    let evaluation = evaluate_native_acceptance(&NativeAcceptanceInput {
        candidate,
        contract,
        references: ReferenceSet {
            reference_series: Some(series),
            documents: vec![ReferenceDocument {
                path: PACKAGE_REFERENCE_PINS_REL.to_owned(),
                sha256: Some(sha256_prefixed(&pins_bytes)),
                bytes: Some(pins_bytes),
                document: pins,
            }],
        },
        locks: closed_locks(),
        body_axis: BodyAxisResidual {
            verdict: "not_assessable".to_owned(),
            residual: vec!["p1_localization_uncertain".to_owned()],
            evidence_path: PACKAGE_BODY_RESIDUAL_REL.to_owned(),
        },
    })?;
    Ok(evaluation)
}

fn expect_refusal(
    label: &str,
    package: &GrobidRetirementAcceptancePackage,
    expectation: &AcceptancePackageExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_acceptance_package(package, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

#[test]
fn acceptance_package_is_canonical_and_names_every_gate() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let evidence = expected_evidence(&root)?;
    assert_eq!(evidence.len(), 21, "the evidence carrier table drifted");

    let schema: serde_json::Value = serde_yaml::from_slice(&read(&root, PACKAGE_SCHEMA_REL)?)?;
    let threshold_gate_count = schema["threshold_policy"]["thresholds"]
        .as_array()
        .ok_or("thresholds")?
        .len() as u64;
    let pending_family_gate_count = schema["required_scope"]["families"]
        .as_array()
        .ok_or("families")?
        .iter()
        .filter(|family| family["state"].as_str() == Some("threshold_pending"))
        .count() as u64;
    assert_eq!(
        threshold_gate_count, 3,
        "the contract declares three thresholds"
    );
    assert_eq!(
        pending_family_gate_count, 12,
        "the contract declares twelve pending families"
    );

    // S05 (M516-th0oby): every declared pending family is resolved by a recorded
    // `not_applicable` downgrade in the parity ledger, so no
    // `threshold_pending_family` gate survives: the twelve emit as
    // `required_family_downgrade`/`not_applicable` and no longer block S09.
    let compatibility: serde_yaml::Value =
        serde_yaml::from_slice(&read(&root, PACKAGE_COMPATIBILITY_REL)?)?;
    let downgrade_gate_count =
        compatibility["parity_ledger"]["required_scope_downgrades"]["records"]
            .as_mapping()
            .ok_or("parity_ledger.required_scope_downgrades.records")?
            .len() as u64;
    assert_eq!(
        downgrade_gate_count, 12,
        "the parity ledger records twelve required-scope downgrades"
    );
    let pending_family_gate_count = 0u64;

    let expectation = AcceptancePackageExpectation {
        evidence: evidence.clone(),
        threshold_gate_count,
        pending_family_gate_count,
        downgrade_gate_count,
        body_residual: vec!["p1_localization_uncertain".to_owned()],
        db_canonical_requirement_ids: PACKAGE_DB_CANONICAL_REQUIREMENT_IDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    };

    let artifact_bytes = read(&root, DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL)?;
    let package = GrobidRetirementAcceptancePackage::parse_json_bytes(&artifact_bytes)?;
    validate_acceptance_package(&package, &expectation)?;
    assert_eq!(
        package.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the two R134 statement blocks round-trip with the committed bytes ──
    assert_eq!(
        package.reviewer_recomputation.carrier_count,
        package.evidence.len() as u64,
        "the carried carrier_count is not the bound carrier count"
    );
    assert_eq!(
        package.reviewer_recomputation.carrier_count, 21,
        "the package must carry the twenty-one bound carriers"
    );
    assert_eq!(
        package.carrier_independence.candidate_manifest, PACKAGE_NATIVE_MEASUREMENT_REL,
        "the candidate manifest is not the T01 native measurement carrier"
    );
    assert!(
        package
            .carrier_independence
            .reference_manifests
            .contains(&PACKAGE_REFERENCE_PINS_REL.to_owned())
    );
    assert!(
        package
            .carrier_independence
            .reference_manifests
            .contains(&PACKAGE_REFERENCE_PROBE_REL.to_owned())
    );
    let round_trip: serde_json::Value = serde_json::from_slice(&artifact_bytes)?;
    assert!(
        round_trip.get("reviewer_recomputation").is_some()
            && round_trip.get("carrier_independence").is_some(),
        "a committed R134 statement block is absent from the artifact bytes"
    );

    // ── every gate is typed and every type is present ─────────────────────
    let kinds: BTreeSet<&str> = package
        .mandatory_gates
        .iter()
        .map(|gate| gate.kind.as_str())
        .collect();
    for kind in [
        "threshold",
        "body_axis",
        "required_family_downgrade",
        "named_gap",
        "learning_disposition",
        "resource_pair",
        "human_release",
    ] {
        assert!(kinds.contains(kind), "gate kind `{kind}` is missing");
    }
    for gate in &package.mandatory_gates {
        assert!(
            GATE_STATE_VOCABULARY.contains(&gate.state.as_str()),
            "gate `{}` has state `{}` outside the vocabulary",
            gate.gate_id,
            gate.state
        );
        assert_eq!(
            gate.blocks_s09,
            gate_blocks_s09(gate.required, &gate.state),
            "gate `{}` blocks_s09 disagrees with its state",
            gate.gate_id
        );
        assert!(
            !gate.detail.trim().is_empty(),
            "gate `{}` has no detail",
            gate.gate_id
        );
    }
    for gate_id in [
        NAMED_GAP_LEARNING_RUVLLM_ABLATION,
        NAMED_GAP_H20,
        NAMED_GAP_H21,
        LEARNING_DISPOSITION_GATE_ID,
        JAVA_CPU_GATE_ID,
    ] {
        assert!(
            package
                .mandatory_gates
                .iter()
                .any(|gate| gate.gate_id == gate_id),
            "mandatory gate `{gate_id}` is missing"
        );
    }

    // The resource-axis gate is the measured same-n pair, not the typed absence.
    let resource_gate = package
        .mandatory_gates
        .iter()
        .find(|gate| gate.gate_id == JAVA_CPU_GATE_ID)
        .ok_or("the resource gate is missing")?;
    assert_eq!(
        resource_gate.kind, RESOURCE_PAIR_GATE_KIND,
        "the resource gate is not the measured same-n pair"
    );
    assert_eq!(resource_gate.state, "met");
    assert!(!resource_gate.blocks_s09);

    // ── the body residual survives verbatim ───────────────────────────────
    let body = package
        .mandatory_gates
        .iter()
        .find(|gate| gate.kind == "body_axis")
        .ok_or("body gate missing")?;
    assert_eq!(body.state, "not_assessable");
    assert_eq!(body.reason.as_deref(), Some("p1_localization_uncertain"));

    // ── the blocking set is non-empty and S09 is unauthorized ─────────────
    assert!(
        !package.blocking_gates.is_empty(),
        "the frozen corpus cannot satisfy the gates; the blocking set must not be empty"
    );
    assert!(
        !package.authorization.s09_authorized,
        "S09 must stay unauthorized while a gate blocks"
    );
    assert!(!package.authorization.gsd_lifecycle_authorizes);
    assert_eq!(
        package.authorization.basis,
        "human_release_decision_required"
    );
    assert_eq!(
        package.blocking_gates.len(),
        package
            .mandatory_gates
            .iter()
            .filter(|gate| gate.blocks_s09)
            .count()
    );

    // ── the carried evaluation is the evaluator's own re-derivation ───────
    let evaluation = re_run_evaluator(&root)?;
    assert_eq!(evaluation.schema_version, package.evaluation.schema_version);
    assert_eq!(
        evaluation.threshold_policy_state,
        package.evaluation.threshold_policy_state
    );
    assert_eq!(
        evaluation.candidate_source_path,
        package.evaluation.candidate_source_path
    );
    assert_eq!(
        evaluation.frozen_reference_path,
        package.evaluation.frozen_reference_path
    );
    assert_eq!(
        serde_json::to_value(&evaluation.thresholds)?,
        serde_json::to_value(&package.evaluation.thresholds)?,
        "the carried threshold verdicts drifted from a fresh evaluation"
    );
    // T04 (M515/S02): both extraction thresholds now resolve `met`. The
    // candidate receipt carries the class-parity rows projected from the frozen
    // reference probe, so each extraction threshold matches the pins with a
    // positive denominator and `comparability: matched`. No manufactured pass:
    // the values are read by pointer from the pins and folded through the shared
    // class-parity module.
    let threshold = |id: &str| {
        package
            .evaluation
            .thresholds
            .iter()
            .find(|verdict| verdict.threshold_id == id)
            .unwrap_or_else(|| panic!("the package carries no `{id}` threshold verdict"))
    };
    let preservation = threshold("preservation_micro_occurrence");
    assert_ne!(
        preservation.verdict, "not_comparable",
        "the preservation threshold must be measured, not metric_absent"
    );
    assert_eq!(
        preservation.verdict, "met",
        "the preservation threshold must resolve met against the frozen pins"
    );
    assert_ne!(
        preservation.reason.as_deref(),
        Some("metric_absent"),
        "the preservation threshold must not report metric_absent"
    );
    for id in ["extraction_micro_occurrence", "extraction_weakest_family"] {
        let verdict = threshold(id);
        assert_eq!(verdict.verdict, "met");
        assert_eq!(verdict.comparability, "matched");
        assert!(
            verdict.candidate_denominator.unwrap_or(0.0) > 0.0,
            "`{id}` must resolve with a non-empty denominator"
        );
        assert!(
            verdict.reason.is_none(),
            "a met extraction threshold must not carry a refusal reason"
        );
    }
    let evaluator_gates = evaluation.gates.len() as u64;
    assert_eq!(
        package.mandatory_gates.len() as u64,
        evaluator_gates + 5,
        "the package must carry the evaluator gates plus the five named/disposition/resource gates"
    );

    // ── the attribution reconciliation is recorded, not closed ────────────
    assert!(package.attribution.declared_not_in_db.is_empty());
    assert_eq!(
        package.attribution.db_only,
        vec!["R133".to_owned(), "R134".to_owned()]
    );
    assert_eq!(
        package.attribution.binding_only,
        vec!["R139".to_owned(), "R123".to_owned(), "R124".to_owned()]
    );
    assert_eq!(
        package.attribution.db_linkage_status,
        "artifact_internal_only"
    );
    let contract_requirement_ids: Vec<String> = schema["requirement_ids"]
        .as_array()
        .ok_or("requirement_ids")?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        package.attribution.contract_requirement_ids,
        contract_requirement_ids
    );

    // ── the resource receipt records a same-n pair or a typed absence ─────
    let resource: serde_json::Value =
        serde_json::from_slice(&read(&root, PACKAGE_RESOURCE_RECEIPT_REL)?)?;
    let reference_rows = resource["reference_metrics"]
        .as_array()
        .ok_or("reference_metrics")?;
    let mut observed_rows = 0usize;
    for row in reference_rows {
        match row["state"].as_str() {
            Some("observed") => {
                let source = row
                    .get("source")
                    .filter(|value| !value.is_null())
                    .ok_or("an observed resource reference row carries no same-n source")?;
                assert_eq!(
                    source["kind"].as_str(),
                    Some("measured_reference"),
                    "an observed resource reference row is not a measured reference"
                );
                observed_rows += 1;
            }
            Some("not_observed") => {
                let reason = row["reason"].as_str().unwrap_or_default();
                assert!(
                    SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason),
                    "a not_observed resource reference row carries an untyped reason `{reason}`"
                );
            }
            other => panic!(
                "a resource reference row has state `{other:?}`, expected observed or not_observed"
            ),
        }
    }
    assert!(
        observed_rows > 0,
        "the resource receipt carries no observed same-n reference row"
    );
    assert!(
        reference_rows
            .iter()
            .any(|row| row["id"].as_str() == Some("java_latency_ms")
                && row["state"].as_str() == Some("observed")),
        "the measured java_latency_ms same-n reference row is missing"
    );

    // ── named mutations are all refused ───────────────────────────────────
    let mut m = package.clone();
    m.claim_scope = "product_progress".to_owned();
    expect_refusal("non_diagnostic_claim", &m, &expectation, "ClaimScopeDrift")?;

    let mut m = package.clone();
    for gate in &mut m.mandatory_gates {
        if gate.kind == "body_axis" {
            gate.reason = Some("p1_localization_unknown".to_owned());
        }
    }
    expect_refusal(
        "renamed_body_residual",
        &m,
        &expectation,
        "BodyResidualRenamed",
    )?;

    let mut m = package.clone();
    m.mandatory_gates[0].evidence.manifest = String::new();
    expect_refusal(
        "gate_without_evidence",
        &m,
        &expectation,
        "GateWithoutEvidence",
    )?;

    let mut m = package.clone();
    m.mandatory_gates[0].evidence = CarrierBinding {
        manifest: "unbound.json".to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(b"unbound"),
    };
    expect_refusal("unbound_carrier", &m, &expectation, "UnboundCarrier")?;

    let mut m = package.clone();
    m.authorization.s09_authorized = true;
    expect_refusal(
        "unmet_gate_authorized",
        &m,
        &expectation,
        "UnmetGateAuthorized",
    )?;

    let mut m = package.clone();
    m.authorization.gsd_lifecycle_authorizes = true;
    expect_refusal(
        "gsd_lifecycle_authorization",
        &m,
        &expectation,
        "GsdLifecycleAuthorization",
    )?;

    let mut m = package.clone();
    m.blocking_gates.pop();
    expect_refusal("blocking_set_drift", &m, &expectation, "BlockingSetDrift")?;

    let mut m = package.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut m = package.clone();
    m.mandatory_gates.push(AcceptanceGate {
        gate_id: "synthetic.duplicate".to_owned(),
        kind: "named_gap".to_owned(),
        state: "pending".to_owned(),
        required: true,
        evidence: package.evidence[0].clone(),
        detail: "synthetic".to_owned(),
        family_id: None,
        metric_id: None,
        reason: Some("synthetic".to_owned()),
        evidence_needed: "synthetic".to_owned(),
        blocks_s09: true,
    });
    expect_refusal(
        "blocking_set_after_gate_add",
        &m,
        &expectation,
        "BlockingSetDrift",
    )?;

    let mut m = package.clone();
    m.attribution
        .declared_requirement_ids
        .push("R999".to_owned());
    expect_refusal(
        "declared_requirement_not_canonical",
        &m,
        &expectation,
        "DeclaredRequirementNotCanonical",
    )?;

    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &package, &drifted, "EvidenceDrift")?;

    let mut drifted = expectation.clone();
    drifted.threshold_gate_count = threshold_gate_count + 1;
    expect_refusal(
        "threshold_gate_count_drift",
        &package,
        &drifted,
        "ThresholdCountDrift",
    )?;

    let mut drifted = expectation.clone();
    drifted.pending_family_gate_count = pending_family_gate_count + 1;
    expect_refusal(
        "pending_family_count_drift",
        &package,
        &drifted,
        "PendingFamilyCountDrift",
    )?;

    let mut drifted = expectation.clone();
    drifted.downgrade_gate_count = downgrade_gate_count - 1;
    expect_refusal(
        "downgrade_count_drift",
        &package,
        &drifted,
        "DowngradeCountDrift",
    )?;

    // A redaction leak must be refused by the serializer itself.
    let mut m = package.clone();
    m.mandatory_gates[0].detail = "http://leak".to_owned();
    assert!(
        matches!(
            m.artifact_json_bytes(),
            Err(da_cli::native_acceptance_package::AcceptancePackageError::RedactionLeak { .. })
        ),
        "a payload leak was not refused"
    );

    Ok(())
}

#[test]
fn the_package_module_carries_no_threshold_literal_and_no_side_channel() {
    let root = repo_root();
    let source = read_text(&root, "crates/da-cli/src/native_acceptance_package.rs")
        .expect("the package module is readable");
    for frozen in ["74910", "79621", "945771", "1000000", "3055", "7077"] {
        assert!(
            !source.contains(frozen),
            "the package module carries frozen threshold literal `{frozen}`"
        );
    }
    for carrier_literal in [
        "carrier_count: 21",
        "carrier_count: 21u64",
        "carrier_count:21",
    ] {
        assert!(
            !source.contains(carrier_literal),
            "the package module hardcodes the carrier count literal `{carrier_literal}`"
        );
    }
    assert!(
        source.contains("carrier_count: input.evidence.len() as u64"),
        "the package module must derive carrier_count from the bound evidence"
    );
    for forbidden in [
        "tracing::",
        "tokio::",
        "reqwest",
        "std::fs",
        "std::env",
        "std::net",
        "std::process",
        "evaluate_native_acceptance(",
    ] {
        assert!(
            !source.contains(forbidden),
            "the package module acquired a runtime side channel: `{forbidden}`"
        );
    }
    assert!(source.contains("pub fn validate_acceptance_package"));
    assert!(source.contains("pub fn build_acceptance_package"));
    assert!(source.contains("p1_localization_uncertain"));
}
