//! Contract test for the frozen-threshold evaluator and its mandatory-gate
//! ledger (M514-1lss2s/S08/T02).
//!
//! The positive control runs the evaluator over the committed pins, the
//! committed schema and the committed T01 measurement. The mutation matrix
//! asserts that every shape which would let a false pass through is refused by
//! name. Nothing in this test reads a git-ignored path: every carrier is a
//! tracked repository file.

use da_parse::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "data/parse/absorption_progress.schema.yaml";
const PINS: &str = "artifacts/parse_compat/absorption_progress_reference_pins.json";
const PROBE: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
const T01: &str = "artifacts/parse_compat/native_corpus_acceptance.json";
const BODY: &str = "artifacts/parse_compat/native_body_uncertainty_split.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read_bytes(rel: &str) -> Vec<u8> {
    std::fs::read(repo_root().join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn read_text(rel: &str) -> String {
    String::from_utf8(read_bytes(rel)).unwrap_or_else(|err| panic!("utf8 {rel}: {err}"))
}

fn read_json(rel: &str) -> Value {
    serde_json::from_str(&read_text(rel)).unwrap_or_else(|err| panic!("json {rel}: {err}"))
}

fn read_yaml_json(rel: &str) -> Value {
    serde_yaml::from_str(&read_text(rel)).unwrap_or_else(|err| panic!("yaml {rel}: {err}"))
}

/// The frozen reference series, derived from committed carriers rather than
/// restated. This is the identity used to keep the candidate and the reference
/// inside one series; it is not a claim that the probe emits these four keys as
/// a block.
fn committed_series(contract: &Value, probe: &Value) -> Series {
    Series {
        corpus_id: contract["assigned_population"]["census_id"]
            .as_str()
            .expect("census id")
            .to_string(),
        profile: probe["executed_population"]["execution_route_profile"]
            .as_str()
            .expect("execution route profile")
            .to_string(),
        oracle: probe["evaluated_population"]["oracle"]
            .as_str()
            .expect("evaluated oracle")
            .to_string(),
        source_revision: contract["threshold_policy"]["freeze_revision"]
            .as_str()
            .expect("freeze revision")
            .to_string(),
    }
}

fn committed_reference_set(contract: &Value) -> ReferenceSet {
    ReferenceSet {
        reference_series: Some(committed_series(contract, &read_json(PROBE))),
        documents: vec![ReferenceDocument {
            path: PINS.to_string(),
            sha256: contract["threshold_policy"]["reference_pins"]["sha256"]
                .as_str()
                .map(str::to_string),
            bytes: Some(read_bytes(PINS)),
            document: read_json(PINS),
        }],
    }
}

fn closed_locks() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("import_eligible".to_string(), "false".to_string()),
        ("deployed".to_string(), "false".to_string()),
        ("graph_write".to_string(), "forbidden".to_string()),
        ("auto_promote".to_string(), "forbidden".to_string()),
        ("production_route".to_string(), "forbidden".to_string()),
    ])
}

fn committed_body_axis() -> BodyAxisResidual {
    // The artifact carries the residual; assert it is present and keep the name
    // verbatim (never renamed).
    let body = read_json(BODY);
    assert!(
        find_key(&body, "p1_localization_uncertain"),
        "body uncertainty split lost the named p1 residual"
    );
    BodyAxisResidual {
        verdict: "not_assessable".to_string(),
        residual: vec!["p1_localization_uncertain".to_string()],
        evidence_path: BODY.to_string(),
    }
}

fn find_key(value: &Value, wanted: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.contains_key(wanted) || map.values().any(|child| find_key(child, wanted))
        }
        Value::Array(items) => items.iter().any(|child| find_key(child, wanted)),
        _ => false,
    }
}

fn threshold_ref_value(contract: &Value, pins: &Value, index: usize) -> f64 {
    let entry = &contract["threshold_policy"]["thresholds"][index];
    let pointer = entry["reference"]["pointer"]
        .as_str()
        .expect("reference pointer");
    score_number(resolve_pointer(pins, pointer).expect("pointer resolves"))
        .expect("reference score")
}

/// The committed denominator partition, with the typed exclusion reasons
/// recomputed from the per-document rows (never a restated count).
fn committed_denominator(t01: &Value) -> PopulationDenominator {
    let populations = &t01["populations"];
    let mut exclusion_reasons: BTreeMap<String, u64> = BTreeMap::new();
    for document in t01["documents"].as_array().expect("documents") {
        if let Some(reason) = document.get("exclusion_reason").and_then(Value::as_str) {
            *exclusion_reasons.entry(reason.to_string()).or_insert(0) += 1;
        }
    }
    PopulationDenominator {
        assigned: populations["assigned"].as_u64().expect("assigned"),
        executed: populations["measurable"].as_u64().expect("measurable"),
        refused: populations["refused"].as_u64().expect("refused"),
        excluded: populations["excluded"].as_u64().expect("excluded"),
        exclusion_reasons,
    }
}

/// A candidate that satisfies every frozen threshold under the committed
/// contract, used as the positive control for the comparison itself. `factor`
/// scales each measured value against its reference (< 1.0 is unmet).
fn satisfied_candidate(
    contract: &Value,
    pins: &Value,
    factor: f64,
    source_path: &str,
) -> CandidateMeasurement {
    let thresholds = contract["threshold_policy"]["thresholds"]
        .as_array()
        .expect("thresholds");
    let mut metrics = Vec::new();
    for (index, entry) in thresholds.iter().enumerate() {
        let reference = threshold_ref_value(contract, pins, index);
        metrics.push(MeasuredMetric {
            metric_id: entry["metric_id"].as_str().expect("metric id").to_string(),
            numerator: Some(reference * factor),
            denominator: Some(1.0),
            stratum: entry["stratum"].as_str().map(str::to_string),
            series: None,
        });
    }
    CandidateMeasurement {
        source_path: source_path.to_string(),
        series: Some(committed_series(contract, &read_json(PROBE))),
        metrics,
        denominator: committed_denominator(&read_json(T01)),
    }
}

fn good_input() -> NativeAcceptanceInput {
    let contract = read_yaml_json(SCHEMA);
    let pins = read_json(PINS);
    NativeAcceptanceInput {
        candidate: satisfied_candidate(&contract, &pins, 1.0, T01),
        references: committed_reference_set(&contract),
        contract,
        locks: closed_locks(),
        body_axis: committed_body_axis(),
    }
}

fn evaluate_ok(input: &NativeAcceptanceInput) -> NativeAcceptanceEvaluation {
    evaluate_native_acceptance(input).unwrap_or_else(|err| panic!("evaluation refused: {err}"))
}

fn assert_refusal(input: &NativeAcceptanceInput, code: &str) {
    match evaluate_native_acceptance(input) {
        Err(refusal) => assert_eq!(refusal.code, code, "unexpected refusal: {}", refusal.detail),
        Ok(evaluation) => panic!(
            "expected refusal `{code}`, got verdicts {:?}",
            evaluation
                .thresholds
                .iter()
                .map(|t| &t.verdict)
                .collect::<Vec<_>>()
        ),
    }
}

#[test]
fn committed_pins_evaluate_without_a_manufactured_pass() {
    let contract = read_yaml_json(SCHEMA);
    let t01 = read_json(T01);
    let input = NativeAcceptanceInput {
        candidate: CandidateMeasurement {
            source_path: T01.to_string(),
            series: Some(committed_series(&contract, &read_json(PROBE))),
            metrics: serde_json::from_value(t01["metrics"].clone()).expect("T01 metric rows"),
            denominator: committed_denominator(&t01),
        },
        references: committed_reference_set(&contract),
        contract,
        locks: closed_locks(),
        body_axis: committed_body_axis(),
    };
    let evaluation = evaluate_ok(&input);

    assert_eq!(
        evaluation.schema_version,
        NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION
    );
    assert_eq!(evaluation.claim_scope, NATIVE_ACCEPTANCE_CLAIM_SCOPE);
    assert_eq!(
        evaluation.product_coverage_effect,
        NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT
    );
    assert_eq!(evaluation.threshold_policy_state, "frozen");
    assert_eq!(evaluation.candidate_source_path, T01);

    // The committed T01 runner measures all three frozen thresholds over the
    // frozen reference TEI: preservation and both extraction strata (micro and
    // weakest) compare inside the committed series and resolve `met` with a
    // positive candidate denominator — measured values reproducing the pinned
    // fractions, never a manufactured pass. Nothing stays `not_comparable`;
    // `REASON_METRIC_ABSENT` is absent from every verdict.
    assert_eq!(evaluation.thresholds.len(), 3);
    for verdict in &evaluation.thresholds {
        assert_eq!(
            verdict.verdict, "met",
            "threshold {} is measured by the committed T01 runner",
            verdict.threshold_id
        );
        assert_ne!(verdict.reason.as_deref(), Some(REASON_METRIC_ABSENT));
        assert_eq!(verdict.comparability, "matched");
        assert!(
            verdict.candidate_denominator.unwrap_or(0.0) > 0.0,
            "a measured threshold needs a positive candidate denominator"
        );
        assert!(verdict.frozen_reference_value.is_some());
    }
    // The two extraction thresholds share one metric id and are distinguished
    // only by stratum: each must resolve against its own stratum, never by a
    // fallback to the other.
    let mut extraction_strata: Vec<Option<&str>> = evaluation
        .thresholds
        .iter()
        .filter(|verdict| verdict.metric_id == "extraction.native_capability_recall")
        .map(|verdict| verdict.stratum.as_deref())
        .collect();
    assert_eq!(extraction_strata.len(), 2);
    extraction_strata.sort_unstable();
    assert_ne!(
        extraction_strata[0], extraction_strata[1],
        "the two extraction thresholds must resolve distinct strata"
    );

    // The ledger names all sixteen mandatory gates: three thresholds, the body
    // axis and every schema-declared `threshold_pending` family.
    let expected_pending = read_yaml_json(SCHEMA)["required_scope"]["families"]
        .as_array()
        .expect("families")
        .iter()
        .filter(|entry| entry["state"] == "threshold_pending")
        .count();
    assert_eq!(
        expected_pending, 12,
        "committed schema declares twelve pending families"
    );
    let pending_gates: Vec<_> = evaluation
        .gates
        .iter()
        .filter(|gate| gate.kind == "threshold_pending_family")
        .collect();
    assert_eq!(pending_gates.len(), expected_pending);
    assert!(
        pending_gates
            .iter()
            .all(|gate| gate.state == "threshold_pending")
    );
    assert_eq!(evaluation.gates.len(), 3 + 1 + expected_pending);

    let body_gate = evaluation
        .gates
        .iter()
        .find(|gate| gate.kind == "body_axis")
        .expect("body axis gate");
    assert_eq!(body_gate.state, "not_assessable");
    assert_eq!(body_gate.gate_id, NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID);
    assert_eq!(
        body_gate.reason.as_deref(),
        Some("p1_localization_uncertain")
    );
}

#[test]
fn satisfied_and_unsatisfied_candidates_compare_against_the_frozen_pins() {
    let contract = read_yaml_json(SCHEMA);
    let pins = read_json(PINS);

    let satisfied = NativeAcceptanceInput {
        candidate: satisfied_candidate(&contract, &pins, 1.0, T01),
        references: committed_reference_set(&contract),
        contract: contract.clone(),
        locks: closed_locks(),
        body_axis: committed_body_axis(),
    };
    let evaluation = evaluate_ok(&satisfied);
    assert_eq!(evaluation.thresholds.len(), 3);
    for verdict in &evaluation.thresholds {
        assert_eq!(
            verdict.verdict, "met",
            "threshold {} should be met",
            verdict.threshold_id
        );
        assert_eq!(verdict.comparability, "matched");
        assert!(verdict.frozen_reference_value.is_some());
    }

    let unsatisfied = NativeAcceptanceInput {
        candidate: satisfied_candidate(&contract, &pins, 0.5, T01),
        references: committed_reference_set(&contract),
        contract,
        locks: closed_locks(),
        body_axis: committed_body_axis(),
    };
    let evaluation = evaluate_ok(&unsatisfied);
    assert!(
        evaluation
            .thresholds
            .iter()
            .all(|verdict| verdict.verdict == "unmet")
    );
}

#[test]
fn named_mutations_are_refused_instead_of_passing() {
    // 1. Substituted reference: not the independently pinned artifact.
    let mut input = good_input();
    input.contract["threshold_policy"]["thresholds"][0]["reference"]["path"] =
        json!("artifacts/parse_compat/another_pins.json");
    assert_refusal(&input, REFUSAL_REFERENCE_NOT_INDEPENDENTLY_PINNED);

    // 2. Candidate used as its own reference.
    let mut input = good_input();
    input.contract["threshold_policy"]["thresholds"][0]["reference"]["path"] = json!(T01);
    assert_refusal(&input, REFUSAL_SELF_REFERENCE);

    // 3. A threshold literal instead of a pointer-resolved reference.
    let mut input = good_input();
    input.contract["threshold_policy"]["thresholds"][0]["value"] = json!(0.9);
    assert_refusal(&input, REFUSAL_THRESHOLD_LITERAL);

    // 4. A threshold placed on a `threshold_pending` family.
    let mut input = good_input();
    let pending_family = input.contract["required_scope"]["families"]
        .as_array()
        .expect("families")
        .iter()
        .find(|entry| entry["state"] == "threshold_pending")
        .expect("pending family")["family_id"]
        .as_str()
        .expect("family id")
        .to_string();
    input.contract["threshold_policy"]["thresholds"][0]["family_id"] = json!(pending_family);
    assert_refusal(&input, REFUSAL_THRESHOLD_ON_PENDING_FAMILY);

    // 5. A dropped exclusion: the denominator partition must still sum.
    let mut input = good_input();
    input.candidate.denominator.excluded = 0;
    assert_refusal(&input, REFUSAL_DROPPED_EXCLUSION);

    // 6. An unlabeled gate.
    let mut input = good_input();
    input.contract["threshold_policy"]["thresholds"][0]["threshold_id"] = json!("");
    assert_refusal(&input, REFUSAL_UNNAMED_GATE);

    // 7. An open lock.
    let mut input = good_input();
    input
        .locks
        .insert("import_eligible".to_string(), "true".to_string());
    assert_refusal(&input, REFUSAL_OPEN_LOCK);

    // 8. A reference document whose bytes do not match its declared digest.
    let mut input = good_input();
    input.references.documents[0].sha256 =
        Some("sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string());
    assert_refusal(&input, REFUSAL_REFERENCE_DIGEST_MISMATCH);
}

#[test]
fn cross_series_and_missing_denominator_are_not_comparable_never_a_pass() {
    // Cross-series: same metric present, different series -> not_comparable.
    let mut input = good_input();
    let series = input.candidate.series.as_mut().expect("series");
    series.corpus_id = "some-other-corpus".to_string();
    let evaluation = evaluate_ok(&input);
    let verdict = &evaluation.thresholds[0];
    assert_eq!(verdict.verdict, "not_comparable");
    assert_eq!(
        verdict.reason.as_deref(),
        Some(REASON_MIXED_SCOPE_COMPARISON)
    );
    assert_eq!(verdict.comparability, "mixed");

    // Missing series entirely -> not_comparable, never assumed equal.
    let mut input = good_input();
    input.candidate.series = None;
    for metric in &mut input.candidate.metrics {
        metric.series = None;
    }
    let evaluation = evaluate_ok(&input);
    assert_eq!(evaluation.thresholds[0].verdict, "not_comparable");
    assert_eq!(
        evaluation.thresholds[0].reason.as_deref(),
        Some(REASON_COMPARABILITY_FIELDS_MISSING)
    );

    // A metric without a positive denominator is not a number.
    let mut input = good_input();
    input.candidate.metrics[0].denominator = Some(0.0);
    let evaluation = evaluate_ok(&input);
    assert_eq!(evaluation.thresholds[0].verdict, "not_comparable");
    assert_eq!(
        evaluation.thresholds[0].reason.as_deref(),
        Some(REASON_NO_DENOMINATOR)
    );
    assert_eq!(evaluation.thresholds[0].candidate_value, None);
}

#[test]
fn score_number_and_pointer_resolution_mirror_the_domain_rule() {
    assert_eq!(score_number(&json!(0.75)), Some(0.75));
    assert_eq!(
        score_number(&json!({ "numerator": 3, "denominator": 4 })),
        Some(0.75)
    );
    assert_eq!(
        score_number(&json!({ "numerator": 3, "denominator": 0 })),
        None
    );
    assert_eq!(score_number(&json!({ "numerator": 3 })), None);
    assert_eq!(score_number(&json!(true)), None);

    let document = json!({
        "values": [
            { "threshold_id": "a", "fraction": { "numerator": 1, "denominator": 2 } },
            { "threshold_id": "b", "fraction": { "numerator": 3, "denominator": 4 } }
        ]
    });
    let pointer = "values[threshold_id=b]/fraction";
    let resolved = resolve_pointer(&document, pointer).expect("selector pointer resolves");
    assert_eq!(score_number(resolved), Some(0.75));
    assert!(resolve_pointer(&document, "values[threshold_id=missing]/fraction").is_none());
    assert!(resolve_pointer(&document, "values[threshold_id=b").is_none());
}

#[test]
fn the_evaluator_carries_no_threshold_literal_and_no_runtime_side_channel() {
    let source = read_text("crates/da-parse/src/native_acceptance.rs");
    for frozen in ["74910", "79621", "945771", "1000000", "3055", "7077"] {
        assert!(
            !source.contains(frozen),
            "evaluator source carries frozen threshold literal `{frozen}`"
        );
    }
    for forbidden in [
        "tracing::",
        "tokio::",
        "reqwest",
        "std::fs",
        "std::env",
        "std::net",
        "std::process",
        "build_native_route_result",
    ] {
        assert!(
            !source.contains(forbidden),
            "evaluator acquired a runtime side channel or route coupling: `{forbidden}`"
        );
    }
    assert!(source.contains("pub fn score_number"));
    assert!(source.contains("pub fn evaluate_native_acceptance"));
    assert!(source.contains("required_scope"));
}
