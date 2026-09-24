//! S06/T01: the batch budget ceiling dictionary is DATA, bound to LIVE Rust
//! constants.
//!
//! S06 must DESIGN its ceilings before any behaviour change. The only batch
//! budget today counts slots, so the ceiling dictionary, the mandatory metric
//! set and the prohibitions live as DATA in the already-sanctioned
//! `data/parse/comparison_contract.yaml#batch_budget_ceilings` (D377: no new
//! `data/parse/*` file) and are compared here against the LIVE
//! [`da_parse::batch_budget`] constants — never against a second literal copy.
//!
//! * a missing block is `batch-budget-ceilings:missing` — absence, not drift;
//! * a drifted scalar fails by name (`batch-budget-ceilings:<key>`);
//! * negatives are fail-closed: an empty metric set, a `silent_clamp` that is
//!   not `forbidden`, a `not_observed` without a reason, and an `h05_closure`
//!   that is claimed from a design block carrying no measured ceilings;
//! * the carrier is proven structurally: `not_observed` serializes with a named
//!   reason and **no numeric field**, so a ceiling cannot be reported without a
//!   measurement (H19-N1 / H05-N2).
//!
//! Design only: no batch behaviour changes, H05 stays unclaimed (D430), and
//! every lock holds. Run with `-- --nocapture` to print the measured profile.

use da_parse::{
    BATCH_BUDGET_ABSENCE_REQUIRES_REASON, BATCH_BUDGET_CEILINGS_ADR_REF,
    BATCH_BUDGET_CEILINGS_REQUIREMENT_IDS, BATCH_BUDGET_CEILINGS_SCHEMA_VERSION,
    BATCH_BUDGET_COUNT_LIMIT_IS_NOT_A_BYTE_CEILING, BATCH_BUDGET_H05_CLOSURE,
    BATCH_BUDGET_H05_REASON, BATCH_BUDGET_METRICS, BATCH_BUDGET_RETENTION_CEILING_COMPOSES_WITH,
    BATCH_BUDGET_SILENT_CLAMP, BATCH_BUDGET_VERDICTS, BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT,
    BATCH_OBSERVATION_STATES, BatchObservation,
};
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

const CONTRACT: &str = "data/parse/comparison_contract.yaml";
const BLOCK: &str = "batch_budget_ceilings";
/// S08's block: it must not be mutated or pooled with the S06 block.
const NATIVE_CORE: &str = "native_core_qualification";
const ERROR_PREFIX: &str = "batch-budget-ceilings";
const MISSING: &str = "batch-budget-ceilings:missing";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Value {
    let bytes = std::fs::read(root().join(CONTRACT))
        .unwrap_or_else(|error| panic!("{CONTRACT}: unreadable ({error})"));
    serde_yaml::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("{CONTRACT}: unparsable ({error})"))
}

fn block(contract: &Value) -> Option<&Value> {
    contract.get(BLOCK)
}

/// Folded YAML scalars and Rust `concat!` literals may only differ in
/// whitespace, so prose scalars are compared after normalisation.
fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check_str(block: &Value, key: &str, expected: &str, errs: &mut Vec<String>) {
    if block.get(key).and_then(Value::as_str) != Some(expected) {
        errs.push(format!("{ERROR_PREFIX}:{key}"));
    }
}

fn check_bool(block: &Value, key: &str, expected: bool, errs: &mut Vec<String>) {
    if block.get(key).and_then(Value::as_bool) != Some(expected) {
        errs.push(format!("{ERROR_PREFIX}:{key}"));
    }
}

fn check_str_list(block: &Value, key: &str, expected: &[&str], errs: &mut Vec<String>) {
    let observed: Option<Vec<&str>> = block
        .get(key)
        .and_then(Value::as_sequence)
        .and_then(|seq| seq.iter().map(Value::as_str).collect());
    let ok = match observed {
        Some(observed) => {
            observed.len() == expected.len()
                && observed.iter().zip(expected.iter()).all(|(o, e)| o == e)
        }
        None => false,
    };
    if !ok {
        errs.push(format!("{ERROR_PREFIX}:{key}"));
    }
}

/// Validate the DATA block against the LIVE Rust constants. Every finding is
/// named `batch-budget-ceilings:<key>`; a missing block is reported as
/// `batch-budget-ceilings:missing` (absence, not drift).
fn validate(block: Option<&Value>) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(block) = block else {
        errs.push(MISSING.to_string());
        return errs;
    };
    if !block.is_mapping() {
        errs.push(format!("{ERROR_PREFIX}:block"));
        return errs;
    }

    check_str(
        block,
        "schema_version",
        BATCH_BUDGET_CEILINGS_SCHEMA_VERSION,
        &mut errs,
    );
    check_str(block, "adr_ref", BATCH_BUDGET_CEILINGS_ADR_REF, &mut errs);
    check_str_list(
        block,
        "requirement_ids",
        &BATCH_BUDGET_CEILINGS_REQUIREMENT_IDS,
        &mut errs,
    );
    check_str_list(
        block,
        "observation_states",
        &BATCH_OBSERVATION_STATES,
        &mut errs,
    );
    check_bool(
        block,
        "absence_requires_reason",
        BATCH_BUDGET_ABSENCE_REQUIRES_REASON,
        &mut errs,
    );
    check_str_list(block, "metrics", &BATCH_BUDGET_METRICS, &mut errs);
    check_str_list(block, "verdicts", &BATCH_BUDGET_VERDICTS, &mut errs);
    check_bool(
        block,
        "zero_is_not_a_measurement",
        BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT,
        &mut errs,
    );
    check_str(block, "silent_clamp", BATCH_BUDGET_SILENT_CLAMP, &mut errs);
    check_bool(
        block,
        "count_limit_is_not_a_byte_ceiling",
        BATCH_BUDGET_COUNT_LIMIT_IS_NOT_A_BYTE_CEILING,
        &mut errs,
    );
    check_str(
        block,
        "retention_ceiling_composes_with",
        BATCH_BUDGET_RETENTION_CEILING_COMPOSES_WITH,
        &mut errs,
    );

    // H05 closure belongs to the MEASURED artifact, never to the design block
    // (D430). This block carries no `measured_ceilings`, so `not_claimed` is the
    // only honest value and any claim fails by name.
    let closure = block.get("h05_closure").and_then(Value::as_str);
    if closure != Some(BATCH_BUDGET_H05_CLOSURE) {
        errs.push(format!("{ERROR_PREFIX}:h05_closure"));
    }

    let reason = block.get("h05_reason").and_then(Value::as_str);
    if reason.map(normalize_ws) != Some(normalize_ws(BATCH_BUDGET_H05_REASON)) {
        errs.push(format!("{ERROR_PREFIX}:h05_reason"));
    }

    errs
}

/// Apply a mutation to a copy of the block and return the findings.
fn errors_after(mutate: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut contract = contract();
    mutate(contract.get_mut(BLOCK).expect("block present in fixture"));
    validate(contract.get(BLOCK))
}

fn assert_named(errs: &[String], key: &str) {
    let expected = format!("{ERROR_PREFIX}:{key}");
    assert!(
        errs.iter().any(|err| err == &expected),
        "expected `{expected}` among {errs:?}"
    );
}

/// The reason of an unmeasured observation, or `None` when the observation is
/// measured or the reason is blank. A blank reason is not a named reason.
fn named_reason(observation: &BatchObservation<u64>) -> Option<&str> {
    observation
        .reason()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
}

#[test]
fn batch_budget_ceilings_data_contract_holds() {
    let contract = contract();
    let errs = validate(block(&contract));
    assert!(
        errs.is_empty(),
        "batch budget ceiling DATA violations:\n{}",
        errs.join("\n")
    );

    // The block is a TOP-LEVEL sibling: present at the contract root, absent
    // from the S08 qualifier, and the qualifier still carries its own honest
    // `not_claimed`.
    assert!(
        contract.get(BLOCK).is_some(),
        "the block is a top-level sibling"
    );
    assert!(
        contract
            .get(NATIVE_CORE)
            .and_then(|b| b.get(BLOCK))
            .is_none(),
        "the S06 block must not be nested inside native_core_qualification"
    );
    assert_eq!(
        contract[NATIVE_CORE]["h05_closure"].as_str(),
        Some("not_claimed"),
        "S08's qualifier must stay unclaimed and unmutated"
    );
    // A design block measures nothing: `complete` is unreachable from here.
    assert!(
        contract[BLOCK].get("measured_ceilings").is_none(),
        "the design block declares ceilings; it carries no measured ceilings"
    );

    println!(
        "[S06-T01] block={BLOCK} schema={} metrics={} verdicts={} states={:?} h05_closure={}",
        BATCH_BUDGET_CEILINGS_SCHEMA_VERSION,
        BATCH_BUDGET_METRICS.len(),
        BATCH_BUDGET_VERDICTS.len(),
        BATCH_OBSERVATION_STATES,
        BATCH_BUDGET_H05_CLOSURE,
    );
}

#[test]
fn missing_block_is_reported_as_missing_not_drift() {
    assert_eq!(validate(None), vec![MISSING.to_string()]);

    let mut contract = contract();
    contract
        .as_mapping_mut()
        .expect("contract is a mapping")
        .remove(BLOCK);
    let errs = validate(block(&contract));
    assert!(
        errs.iter().any(|err| err == MISSING),
        "removing the block must be reported as missing, got {errs:?}"
    );
}

#[test]
fn scalar_drift_fails_by_name() {
    assert_named(
        &errors_after(|b| b["schema_version"] = Value::String("drift".into())),
        "schema_version",
    );
    assert_named(
        &errors_after(|b| b["adr_ref"] = Value::String("ADR-000".into())),
        "adr_ref",
    );
    assert_named(
        &errors_after(|b| b["requirement_ids"] = serde_yaml::from_str("[R085, R087]").unwrap()),
        "requirement_ids",
    );
    assert_named(
        &errors_after(|b| b["observation_states"] = serde_yaml::from_str("[observed]").unwrap()),
        "observation_states",
    );
    assert_named(
        &errors_after(|b| b["absence_requires_reason"] = Value::Bool(false)),
        "absence_requires_reason",
    );
    assert_named(
        &errors_after(|b| b["metrics"] = serde_yaml::from_str("[]").unwrap()),
        "metrics",
    );
    assert_named(
        &errors_after(|b| b["metrics"] = serde_yaml::from_str("[peak_rss_bytes]").unwrap()),
        "metrics",
    );
    assert_named(
        &errors_after(|b| b["verdicts"] = serde_yaml::from_str("[within_ceiling]").unwrap()),
        "verdicts",
    );
    assert_named(
        &errors_after(|b| b["zero_is_not_a_measurement"] = Value::Bool(false)),
        "zero_is_not_a_measurement",
    );
    assert_named(
        &errors_after(|b| b["silent_clamp"] = Value::String("allowed".into())),
        "silent_clamp",
    );
    assert_named(
        &errors_after(|b| b["count_limit_is_not_a_byte_ceiling"] = Value::Bool(false)),
        "count_limit_is_not_a_byte_ceiling",
    );
    assert_named(
        &errors_after(|b| b["retention_ceiling_composes_with"] = Value::String("nothing".into())),
        "retention_ceiling_composes_with",
    );
    assert_named(
        &errors_after(|b| b["h05_reason"] = Value::String("H05 is done.".into())),
        "h05_reason",
    );
    // Removing a scalar is a named drift too, never a silent absence.
    assert_named(
        &errors_after(|b| {
            b.as_mapping_mut()
                .expect("block is a mapping")
                .remove("silent_clamp");
        }),
        "silent_clamp",
    );
}

#[test]
fn h05_closure_cannot_be_claimed_from_a_design_block_without_measurements() {
    let base = contract();
    assert!(
        base[BLOCK].get("measured_ceilings").is_none(),
        "precondition: the design block measures no ceiling"
    );
    for claimed in ["complete", "closed", "passed", "in_progress"] {
        assert_named(
            &errors_after(|b| b["h05_closure"] = Value::String(claimed.into())),
            "h05_closure",
        );
    }
}

#[test]
fn absence_requires_a_named_reason() {
    // DATA half: the carrier must declare the requirement, so flipping it to
    // `false` fails closed by name.
    assert_named(
        &errors_after(|b| b["absence_requires_reason"] = Value::Bool(false)),
        "absence_requires_reason",
    );

    // Carrier half: serde refuses a `not_observed` that carries no reason at all.
    let missing_reason =
        serde_json::from_str::<BatchObservation<u64>>(r#"{"state":"not_observed"}"#);
    let error = missing_reason.expect_err("a not_observed without a reason must fail closed");
    assert!(
        error.to_string().contains("reason"),
        "the deserialization error must name the missing field: {error}"
    );

    // A blank reason is not a named reason, while a named reason is observable.
    let blank = BatchObservation::<u64>::not_observed("   ");
    assert_eq!(named_reason(&blank), None, "a blank reason names nothing");
    let named = BatchObservation::<u64>::not_observed("rss reader unavailable");
    assert_eq!(named_reason(&named), Some("rss reader unavailable"));
}

#[test]
fn absent_observation_serializes_without_a_numeric_field() {
    let absent = BatchObservation::<u64>::not_observed("java cpu reader absent");
    let wire = serde_json::to_string(&absent).expect("absence serializes");
    println!("[S06-T01] absent wire: {wire}");
    let parsed: serde_json::Value = serde_json::from_str(&wire).expect("absence parses");
    let keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("absence is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["reason", "state"]),
        "an absence carries exactly a state and a named reason: {wire}"
    );
    assert_eq!(parsed["state"].as_str(), Some("not_observed"));
    assert_eq!(parsed["reason"].as_str(), Some("java cpu reader absent"));
    assert!(
        parsed.get("value").is_none(),
        "an absence must not carry a value field: {wire}"
    );

    // Only an observation carries a `value`, so a number can never appear
    // without a measurement.
    let observed = BatchObservation::<u64>::observed(1_500);
    let observed_wire = serde_json::to_string(&observed).expect("observation serializes");
    let observed_parsed: serde_json::Value =
        serde_json::from_str(&observed_wire).expect("observation parses");
    assert_eq!(observed_parsed["state"].as_str(), Some("observed"));
    assert_eq!(observed_parsed["value"].as_u64(), Some(1_500));
    assert!(observed_parsed.get("reason").is_none(), "{observed_wire}");
}

#[test]
fn batch_observation_roundtrip_is_stable() {
    for observation in [
        BatchObservation::<u64>::observed(1_500),
        BatchObservation::<u64>::not_observed("not measured on this host"),
    ] {
        let wire = serde_json::to_vec(&observation).expect("observation serializes");
        let replay: BatchObservation<u64> =
            serde_json::from_slice(&wire).expect("observation deserializes");
        assert_eq!(replay, observation, "serde replay must be byte-stable");
    }
    let observed = BatchObservation::<u64>::observed(7);
    assert!(observed.is_observed());
    assert_eq!(observed.value(), Some(&7_u64));
    assert_eq!(observed.reason(), None);

    let absent = BatchObservation::<u64>::not_observed("unmeasured");
    assert!(!absent.is_observed());
    assert_eq!(absent.value(), None);
    assert_eq!(absent.reason(), Some("unmeasured"));
}

#[test]
fn vocabulary_is_closed_and_matches_the_carrier_wire_tokens() {
    assert_eq!(BATCH_OBSERVATION_STATES.len(), 2);
    assert_eq!(BATCH_BUDGET_METRICS.len(), 10);
    assert_eq!(BATCH_BUDGET_VERDICTS.len(), 3);
    for vocabulary in [
        &BATCH_OBSERVATION_STATES[..],
        &BATCH_BUDGET_METRICS[..],
        &BATCH_BUDGET_VERDICTS[..],
    ] {
        let unique: BTreeSet<&str> = vocabulary.iter().copied().collect();
        assert_eq!(
            unique.len(),
            vocabulary.len(),
            "vocabulary tokens must be unique: {vocabulary:?}"
        );
    }

    // The state vocabulary must be the carrier's serde wire tokens, otherwise
    // DATA and the Rust type could drift apart.
    let observed_wire = serde_json::to_value(BatchObservation::<u64>::observed(1)).unwrap();
    assert_eq!(
        observed_wire["state"].as_str(),
        Some(BATCH_OBSERVATION_STATES[0])
    );
    let absent_wire = serde_json::to_value(BatchObservation::<u64>::not_observed("x")).unwrap();
    assert_eq!(
        absent_wire["state"].as_str(),
        Some(BATCH_OBSERVATION_STATES[1])
    );
}
