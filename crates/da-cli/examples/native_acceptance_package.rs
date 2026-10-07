//! S08/T05 (M514-1lss2s) — GROBID retirement acceptance-package runner.
//!
//! This is the executable half of the acceptance-package contract. It reads the
//! committed S08 carriers, invokes the T02 frozen-threshold evaluator and writes
//! `artifacts/parse_compat/grobid_retirement_acceptance_package.json` atomically
//! (tmp + rename). It reads and binds, by sha256 + pointer:
//!
//! * the T01 corpus-bound native measurement (610 assigned = 531 measurable +
//!   79 typed exclusions);
//! * the T03 same-n resource receipt and the T04 real-corpus rollback rehearsal;
//! * the frozen reference pins, the executed reference probe and the closed
//!   absorption-progress schema;
//! * the T02 evaluator source, the frozen M513 release decision package, the
//!   parity-ledger named gaps, the horizon followup, the S07 learning cycle, the
//!   S05 body residual and the container-wide Java batch-budget carrier.
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No carrier is read and nothing is written.
//!   Rejects each named mutation (non-diagnostic claim, renamed body residual,
//!   gate without evidence, unbound carrier, drifted blocking set,
//!   `s09_authorized=true` under an unmet gate, GSD-lifecycle authorization,
//!   opened lock, non-canonical declared requirement).
//! * default — canonical run over the committed carriers.
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, all five locks closed, no graph/import/
//! promotion, no production route (D127 / ADR-040 / ADR-076 §14).

use da_cli::native_acceptance::NativeAcceptanceReceipt;
use da_cli::native_acceptance_package::{
    AcceptanceGate, AcceptancePackageExpectation, AcceptancePackageInput,
    DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL, EvaluationBinding, GateSources,
    GrobidRetirementAcceptancePackage, JAVA_CPU_GATE_ID, LEARNING_DISPOSITION_GATE_ID,
    NAMED_GAP_H20, NAMED_GAP_H21, NAMED_GAP_LEARNING_RUVLLM_ABLATION,
    PACKAGE_BINDING_REQUIREMENT_IDS, PACKAGE_BODY_RESIDUAL_REL, PACKAGE_COMPATIBILITY_REL,
    PACKAGE_DB_CANONICAL_REQUIREMENT_IDS, PACKAGE_DECLARED_REQUIREMENT_IDS,
    PACKAGE_EVIDENCE_CARRIERS, PACKAGE_FOLLOWUP_REL, PACKAGE_LEARNING_CYCLE_REL,
    PACKAGE_M513_RELEASE_PACKAGE_REL, PACKAGE_NATIVE_MEASUREMENT_REL, PACKAGE_REFERENCE_PINS_REL,
    PACKAGE_REFERENCE_PROBE_REL, PACKAGE_RESOURCE_RECEIPT_REL, PACKAGE_SCHEMA_REL,
    POINTER_BODY_RESIDUAL_AGGREGATE, POINTER_COMPATIBILITY_NAMED_GAPS, POINTER_FOLLOWUP_STAGES,
    POINTER_LEARNING_OUTCOME, POINTER_M513_REMAINDERS, POINTER_MEASUREMENT_DOCUMENTS,
    POINTER_PINS_VALUES, POINTER_RESOURCE_REFERENCE_METRICS, POINTER_SCHEMA_REQUIRED_SCOPE,
    POINTER_SCHEMA_REQUIREMENT_IDS, RESOURCE_PAIR_GATE_KIND, build_acceptance_package,
    build_attribution, gate_blocks_s09, validate_acceptance_package,
};
use da_cli::native_corpus_resource_same_n::{
    CORPUS_RESOURCE_REFERENCE_NOT_SAME_N, CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use da_cli::native_resource_same_n::SAME_N_ABSENCE_REASON_VOCABULARY;
use da_parse::{
    BodyAxisResidual, CandidateMeasurement, MandatoryGate, MeasuredMetric,
    NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID, NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION,
    NativeAcceptanceInput, PopulationDenominator, ReferenceDocument, ReferenceSet, Series,
    ThresholdVerdict, evaluate_native_acceptance,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const BODY_TOKEN: &str = "p1_localization_uncertain";

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!("native_acceptance_package [--root DIR] [--report FILE] [--self-check]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args {
        root,
        report,
        self_check,
    }
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_acceptance_package");
    let args = parse_args();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_acceptance_package: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────────── carrier loading ─────────────────────────

fn read_bytes(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

/// Build the evidence list from the one shared carrier table, so the producing
/// example and the contract test cannot keep different sets.
fn evidence_from_root(root: &Path) -> Result<Vec<CarrierBinding>, Box<dyn Error>> {
    PACKAGE_EVIDENCE_CARRIERS
        .iter()
        .map(|(rel, pointer)| {
            let bytes = read_bytes(root, rel)?;
            Ok(CarrierBinding {
                manifest: (*rel).to_owned(),
                pointer: (*pointer).to_owned(),
                revision_sha256: sha256_prefixed(&bytes),
            })
        })
        .collect()
}

fn binding_for<'a>(
    evidence: &'a [CarrierBinding],
    rel: &str,
    pointer: &str,
) -> Result<&'a CarrierBinding, Box<dyn Error>> {
    evidence
        .iter()
        .find(|binding| binding.manifest == rel && binding.pointer == pointer)
        .ok_or_else(|| format!("carrier `{rel}#{pointer}` is not in the evidence list").into())
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

// ───────────────────────── resource axis ─────────────────────────

/// What the same-n resource receipt's reference rows report, derived rather
/// than restated: the observed-row count, the measured Java latency, the
/// measurable population the pair covers and the typed absence reason the
/// pair would otherwise carry.
struct ResourceAxis {
    observed_rows: u64,
    observed_java_latency_ms: Option<u64>,
    measurable_documents: u64,
    absence_reason: String,
}

/// Derive the resource axis from the corpus same-n receipt.
///
/// Every reference row is either `observed` (it must carry a pinned
/// `measured_reference` source) or `not_observed` (it must carry a reason from
/// the closed vocabulary); any other state is refused. The returned axis drives
/// the Java resource gate instead of the old hardcoded typed absence.
fn derive_resource_axis(resource: &serde_json::Value) -> Result<ResourceAxis, Box<dyn Error>> {
    let rows = resource["reference_metrics"]
        .as_array()
        .ok_or("the resource receipt carries no reference_metrics")?;
    if rows.is_empty() {
        return Err("the resource receipt carries no reference metric rows".into());
    }
    let measurable_documents = resource["populations"]["measurable"]
        .as_u64()
        .ok_or("the resource receipt carries no `populations.measurable`")?;
    let mut observed_rows = 0u64;
    let mut observed_java_latency_ms: Option<u64> = None;
    let mut absence_reason: Option<String> = None;
    for row in rows {
        let id = row["id"]
            .as_str()
            .ok_or("every resource reference row must carry an `id`")?;
        match row["state"].as_str() {
            Some("observed") => {
                let source = row
                    .get("source")
                    .filter(|value| !value.is_null())
                    .ok_or_else(|| {
                        format!("observed resource reference row `{id}` has no source")
                    })?;
                if source["kind"].as_str() != Some("measured_reference") {
                    return Err(format!(
                        "observed resource reference row `{id}` is not a `measured_reference`"
                    )
                    .into());
                }
                observed_rows += 1;
                if id == "java_latency_ms" {
                    let value = row["value"].as_u64().ok_or(
                        "the observed java_latency_ms resource reference row carries no integer value",
                    )?;
                    observed_java_latency_ms =
                        Some(observed_java_latency_ms.map_or(value, |known| known.max(value)));
                }
            }
            Some("not_observed") => {
                let reason = row["reason"].as_str().unwrap_or_default();
                if !SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason) {
                    return Err(format!(
                        "not_observed resource reference row `{id}` carries an untyped reason"
                    )
                    .into());
                }
                if absence_reason.is_none() {
                    absence_reason = Some(reason.to_owned());
                }
            }
            other => {
                return Err(format!(
                    "resource reference row `{id}` has state `{}`, expected `observed` or `not_observed`",
                    other.unwrap_or("<absent>")
                )
                .into());
            }
        }
    }
    Ok(ResourceAxis {
        observed_rows,
        observed_java_latency_ms,
        measurable_documents,
        absence_reason: absence_reason
            .unwrap_or_else(|| CORPUS_RESOURCE_REFERENCE_NOT_SAME_N.to_owned()),
    })
}

/// Build the Java resource-axis gate from the derived axis: a measured same-n
/// pair is `resource_pair`/`met` (non-blocking); an all-typed-absence receipt
/// keeps the `resource_absence`/`not_observed` form and blocks S09.
fn resource_axis_gate(axis: &ResourceAxis, evidence: CarrierBinding) -> AcceptanceGate {
    if axis.observed_rows > 0 {
        let latency = axis.observed_java_latency_ms.unwrap_or_default();
        make_gate(
            JAVA_CPU_GATE_ID,
            RESOURCE_PAIR_GATE_KIND,
            "met",
            evidence,
            format!(
                "the Java resource axis is a measured same-n pair: java_latency_ms {latency} ms \
                 (maximum over the {} measurable documents) is bound to the same-n carrier \
                 {CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL}; D510 container-wide Java CPU is \
                 re-scoped by D531 to this same-n reference pair",
                axis.measurable_documents
            ),
            None,
            String::new(),
        )
    } else {
        make_gate(
            JAVA_CPU_GATE_ID,
            "resource_absence",
            "not_observed",
            evidence,
            "container-wide Java CPU is not same-n to the 610 assigned corpus (D510)".to_owned(),
            Some(axis.absence_reason.clone()),
            "a pinned same-n Java CPU reference over the 610 assigned corpus (D510)".to_owned(),
        )
    }
}

// ───────────────────────── full run ─────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }
    let root = args.root.as_path();
    let evidence = evidence_from_root(root)?;

    // ── T01: the candidate measurement ─────────────────────────────────────
    let t01_bytes = read_bytes(root, PACKAGE_NATIVE_MEASUREMENT_REL)?;
    let t01 = NativeAcceptanceReceipt::parse_json_bytes(&t01_bytes)?;
    let mut exclusion_reasons: BTreeMap<String, u64> = BTreeMap::new();
    for document in &t01.documents {
        if let Some(reason) = &document.exclusion_reason {
            *exclusion_reasons.entry(reason.clone()).or_insert(0) += 1;
        }
    }
    let candidate = CandidateMeasurement {
        source_path: PACKAGE_NATIVE_MEASUREMENT_REL.to_owned(),
        series: None,
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

    // ── frozen reference: contract, pins and the executed probe ────────────
    let schema_bytes = read_bytes(root, PACKAGE_SCHEMA_REL)?;
    let mut contract: serde_json::Value = serde_yaml::from_slice(&schema_bytes)?;
    // ── S05: bind the recorded required-scope downgrades into the contract ──
    // The frozen schema declares the required families; the parity ledger records
    // which of them cannot be measured on a frozen corpus. Merge that registry into
    // the contract the evaluator reads, so the T02 evaluator and the governor read
    // one registry rather than two. A missing registry is fatal: the family would
    // silently stay `threshold_pending` and the downgrade would be lost.
    let compatibility_bytes = read_bytes(root, PACKAGE_COMPATIBILITY_REL)?;
    let compatibility: serde_yaml::Value = serde_yaml::from_slice(&compatibility_bytes)?;
    let downgrades = compatibility["parity_ledger"]["required_scope_downgrades"].clone();
    if !downgrades.is_mapping() {
        return Err(
            "the parity ledger lost the required_scope_downgrades registry; the recorded \
             downgrades cannot be merged into the contract"
                .into(),
        );
    }
    let downgrades_json = serde_json::to_value(&downgrades)?;
    contract["required_scope"]
        .as_object_mut()
        .ok_or("the contract declares no required_scope object")?
        .insert("downgrades".to_owned(), downgrades_json);
    let pins_bytes = read_bytes(root, PACKAGE_REFERENCE_PINS_REL)?;
    let pins_value: serde_json::Value = serde_json::from_slice(&pins_bytes)?;
    let probe_bytes = read_bytes(root, PACKAGE_REFERENCE_PROBE_REL)?;
    let probe_value: serde_json::Value = serde_json::from_slice(&probe_bytes)?;

    let series = Series {
        corpus_id: contract["assigned_population"]["census_id"]
            .as_str()
            .ok_or("the schema carries no census id")?
            .to_owned(),
        profile: probe_value["executed_population"]["execution_route_profile"]
            .as_str()
            .ok_or("the probe carries no execution route profile")?
            .to_owned(),
        oracle: probe_value["evaluated_population"]["oracle"]
            .as_str()
            .ok_or("the probe carries no evaluated oracle")?
            .to_owned(),
        source_revision: contract["threshold_policy"]["freeze_revision"]
            .as_str()
            .ok_or("the schema carries no freeze revision")?
            .to_owned(),
    };
    let candidate = CandidateMeasurement {
        series: Some(series.clone()),
        ..candidate
    };

    let pins_binding = binding_for(&evidence, PACKAGE_REFERENCE_PINS_REL, POINTER_PINS_VALUES)?;
    let references = ReferenceSet {
        reference_series: Some(series.clone()),
        documents: vec![ReferenceDocument {
            path: PACKAGE_REFERENCE_PINS_REL.to_owned(),
            sha256: Some(pins_binding.revision_sha256.clone()),
            bytes: Some(pins_bytes),
            document: pins_value,
        }],
    };

    // ── the body-axis residual stays named ─────────────────────────────────
    let body_bytes = read_bytes(root, PACKAGE_BODY_RESIDUAL_REL)?;
    let body_value: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    if !find_key(&body_value, BODY_TOKEN) {
        return Err(format!(
            "the body-residual carrier {PACKAGE_BODY_RESIDUAL_REL} lost the named `{BODY_TOKEN}` residual"
        )
        .into());
    }
    let body_axis = BodyAxisResidual {
        verdict: "not_assessable".to_owned(),
        residual: vec![BODY_TOKEN.to_owned()],
        evidence_path: PACKAGE_BODY_RESIDUAL_REL.to_owned(),
    };

    // ── T02: evaluate the candidate against the frozen pins ────────────────
    let evaluation = evaluate_native_acceptance(&NativeAcceptanceInput {
        candidate,
        contract: contract.clone(),
        references,
        locks: closed_locks(),
        body_axis,
    })
    .map_err(|refusal| format!("the T02 evaluator refused: {refusal}"))?;

    let evaluation_binding = EvaluationBinding {
        schema_version: evaluation.schema_version.clone(),
        threshold_policy_state: evaluation.threshold_policy_state.clone(),
        candidate_source_path: evaluation.candidate_source_path.clone(),
        frozen_reference_path: evaluation.frozen_reference_path.clone(),
        thresholds: evaluation.thresholds.clone(),
    };

    // ── extra gates: named gaps, learning disposition, Java CPU absence ────
    let m513_bytes = read_bytes(root, PACKAGE_M513_RELEASE_PACKAGE_REL)?;
    let m513_value: serde_json::Value = serde_json::from_slice(&m513_bytes)?;
    let remainders = m513_value["remainders"]
        .as_array()
        .ok_or("the M513 package carries no remainders")?;
    let remainder_state = |id: &str| -> Result<String, Box<dyn Error>> {
        let entry = remainders
            .iter()
            .find(|entry| entry["id"].as_str() == Some(id))
            .ok_or_else(|| format!("the M513 package has no remainder `{id}`"))?;
        let state = entry["state"]
            .as_str()
            .ok_or_else(|| format!("M513 remainder `{id}` carries no state"))?;
        match state {
            "pending" | "typed_refusal" | "accepted_residual" => Ok(state.to_owned()),
            other => Err(format!(
                "M513 remainder `{id}` state `{other}` is outside the package mapping"
            )
            .into()),
        }
    };
    let ruvllm_state = remainder_state(NAMED_GAP_LEARNING_RUVLLM_ABLATION)?;
    let h20_state = remainder_state(NAMED_GAP_H20)?;
    let h21_state = remainder_state(NAMED_GAP_H21)?;

    let named_gaps = compatibility["parity_ledger"]["named_gaps"]
        .as_sequence()
        .ok_or("the parity ledger carries no named_gaps")?;
    let ruvllm_reason = named_gaps
        .iter()
        .find(|gap| gap["capability"].as_str() == Some("learning.ruvllm_ablation"))
        .and_then(|gap| gap["reason"].as_str())
        .ok_or("the parity ledger lost the learning.ruvllm_ablation named gap")?;

    let followup_bytes = read_bytes(root, PACKAGE_FOLLOWUP_REL)?;
    let followup: serde_json::Value = serde_json::from_slice(&followup_bytes)?;
    let stages = followup["stages"]
        .as_array()
        .ok_or("the followup carries no stages")?;
    for stage_id in ["H20", "H21"] {
        if !stages
            .iter()
            .any(|stage| stage["stage_id"].as_str() == Some(stage_id))
        {
            return Err(format!("the followup lost stage {stage_id}").into());
        }
    }

    let cycle_bytes = read_bytes(root, PACKAGE_LEARNING_CYCLE_REL)?;
    let cycle: serde_json::Value = serde_json::from_slice(&cycle_bytes)?;
    let outcome_kind = cycle["outcome"]["kind"]
        .as_str()
        .ok_or("the learning cycle carries no outcome")?;
    if outcome_kind != "measured_no_gain" {
        return Err(format!(
            "the S07 learning cycle outcome is `{outcome_kind}`, not `measured_no_gain`"
        )
        .into());
    }

    let resource_bytes = read_bytes(root, PACKAGE_RESOURCE_RECEIPT_REL)?;
    let resource: serde_json::Value = serde_json::from_slice(&resource_bytes)?;
    let resource_axis = derive_resource_axis(&resource)?;

    let m513_binding = binding_for(
        &evidence,
        PACKAGE_M513_RELEASE_PACKAGE_REL,
        POINTER_M513_REMAINDERS,
    )?
    .clone();
    let cycle_binding = binding_for(
        &evidence,
        PACKAGE_LEARNING_CYCLE_REL,
        POINTER_LEARNING_OUTCOME,
    )?
    .clone();
    let resource_binding = binding_for(
        &evidence,
        PACKAGE_RESOURCE_RECEIPT_REL,
        POINTER_RESOURCE_REFERENCE_METRICS,
    )?
    .clone();
    let compatibility_binding = binding_for(
        &evidence,
        PACKAGE_COMPATIBILITY_REL,
        POINTER_COMPATIBILITY_NAMED_GAPS,
    )?
    .clone();
    let followup_binding =
        binding_for(&evidence, PACKAGE_FOLLOWUP_REL, POINTER_FOLLOWUP_STAGES)?.clone();

    let extra_gates = vec![
        make_gate(
            NAMED_GAP_LEARNING_RUVLLM_ABLATION,
            "named_gap",
            &ruvllm_state,
            compatibility_binding,
            format!(
                "capability `learning.ruvllm_ablation` has no Java donor (`{ruvllm_reason}`); decision_ref D471"
            ),
            Some("D471:no_java_donor".to_owned()),
            "a Java donor for `learning.ruvllm_ablation` or an explicit scope decision (D471)"
                .to_owned(),
        ),
        make_gate(
            NAMED_GAP_H20,
            "human_release",
            &h20_state,
            followup_binding,
            "the frozen external holdout package and the human release decision remain pending (H20)"
                .to_owned(),
            Some("H20".to_owned()),
            "a recorded human release decision: named reviewer, explicit decision and passing \
             quality/cost/rights gates"
                .to_owned(),
        ),
        make_gate(
            NAMED_GAP_H21,
            "named_gap",
            &h21_state,
            m513_binding,
            "the revision-label residual is an accepted residual, not a closed gate (H21)"
                .to_owned(),
            Some("H21".to_owned()),
            "close the revision-label residual or record owner acceptance".to_owned(),
        ),
        make_gate(
            LEARNING_DISPOSITION_GATE_ID,
            "learning_disposition",
            "unmet",
            cycle_binding,
            "the S07 learning candidate returned `measured_no_gain` over the frozen corpus"
                .to_owned(),
            Some("measured_no_gain".to_owned()),
            "a learning candidate that achieves a same-n gain over the frozen corpus".to_owned(),
        ),
        resource_axis_gate(&resource_axis, resource_binding),
    ];

    // ── attribution: declared vs DB-canonical vs carrier bindings ──────────
    let contract_requirement_ids: Vec<String> = contract["requirement_ids"]
        .as_array()
        .ok_or("the schema carries no requirement_ids")?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    let strings = |values: &[&str]| -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    };
    let attribution = build_attribution(
        &strings(&PACKAGE_DECLARED_REQUIREMENT_IDS),
        &strings(&PACKAGE_DB_CANONICAL_REQUIREMENT_IDS),
        &strings(&PACKAGE_BINDING_REQUIREMENT_IDS),
        &contract_requirement_ids,
        binding_for(
            &evidence,
            PACKAGE_SCHEMA_REL,
            POINTER_SCHEMA_REQUIREMENT_IDS,
        )?
        .clone(),
    )?;

    let gate_sources = GateSources {
        candidate_measurement: binding_for(
            &evidence,
            PACKAGE_NATIVE_MEASUREMENT_REL,
            POINTER_MEASUREMENT_DOCUMENTS,
        )?
        .clone(),
        pins: binding_for(&evidence, PACKAGE_REFERENCE_PINS_REL, POINTER_PINS_VALUES)?.clone(),
        schema: binding_for(&evidence, PACKAGE_SCHEMA_REL, POINTER_SCHEMA_REQUIRED_SCOPE)?.clone(),
        body_residual: binding_for(
            &evidence,
            PACKAGE_BODY_RESIDUAL_REL,
            POINTER_BODY_RESIDUAL_AGGREGATE,
        )?
        .clone(),
    };

    let package = build_acceptance_package(&AcceptancePackageInput {
        claim_scope: "diagnostic_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
        locks: closed_locks(),
        evaluation: evaluation_binding,
        candidate_series: Some(series.clone()),
        reference_series: Some(series),
        evaluation_gates: evaluation.gates.clone(),
        gate_sources,
        extra_gates,
        evidence: evidence.clone(),
        attribution,
        human_release_recorded: false,
    })?;

    let threshold_gate_count = contract["threshold_policy"]["thresholds"]
        .as_array()
        .map(|values| values.len() as u64)
        .unwrap_or(0);
    // A family resolved by a recorded downgrade emits `required_family_downgrade`,
    // not `threshold_pending_family`; only an unresolved family stays pending.
    let downgrade_records = contract["required_scope"]["downgrades"]["records"]
        .as_object()
        .ok_or("the merged contract carries no downgrade records")?;
    let downgrade_gate_count = downgrade_records.len() as u64;
    let pending_family_gate_count = contract["required_scope"]["families"]
        .as_array()
        .map(|families| {
            families
                .iter()
                .filter(|family| {
                    family["state"].as_str() == Some("threshold_pending")
                        && !family["family_id"]
                            .as_str()
                            .map(|family_id| downgrade_records.contains_key(family_id))
                            .unwrap_or(false)
                })
                .count() as u64
        })
        .unwrap_or(0);
    let expectation = AcceptancePackageExpectation {
        evidence: evidence.clone(),
        threshold_gate_count,
        pending_family_gate_count,
        downgrade_gate_count,
        body_residual: vec![BODY_TOKEN.to_owned()],
        db_canonical_requirement_ids: strings(&PACKAGE_DB_CANONICAL_REQUIREMENT_IDS),
    };
    validate_acceptance_package(&package, &expectation)?;

    let bytes = package.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "acceptance package `{}`: {} evidence carriers bound, {} mandatory gates ({} threshold, \
         {} body-axis, {} threshold_pending family, {} downgraded family, {} named/extra), \
         {} blocking gates, \
         s09_authorized={} basis={} gsd_lifecycle_authorizes={} human_release_recorded={}; \
         attribution declared={:?} db_only={:?} binding_only={:?} status={}; canonical run",
        package.schema_version,
        package.evidence.len(),
        package.mandatory_gates.len(),
        threshold_gate_count,
        1,
        pending_family_gate_count,
        downgrade_gate_count,
        package.mandatory_gates.len() as u64
            - threshold_gate_count
            - 1
            - pending_family_gate_count
            - downgrade_gate_count,
        package.blocking_gates.len(),
        package.authorization.s09_authorized,
        package.authorization.basis,
        package.authorization.gsd_lifecycle_authorizes,
        package.authorization.human_release_recorded,
        package.attribution.declared_requirement_ids,
        package.attribution.db_only,
        package.attribution.binding_only,
        package.attribution.db_linkage_status,
    ))
}

#[allow(clippy::too_many_arguments)]
fn make_gate(
    gate_id: &str,
    kind: &str,
    state: &str,
    evidence: CarrierBinding,
    detail: String,
    reason: Option<String>,
    evidence_needed: String,
) -> AcceptanceGate {
    AcceptanceGate {
        gate_id: gate_id.to_owned(),
        kind: kind.to_owned(),
        state: state.to_owned(),
        required: true,
        evidence,
        detail,
        family_id: None,
        metric_id: None,
        reason,
        evidence_needed,
        blocks_s09: gate_blocks_s09(true, state),
    }
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ──────────────────────────────── self-check ────────────────────────────────

/// Deterministic negative controls, all hermetic (no carrier, no write). Each
/// mutation must be refused **by name**; any mutation that slips through fails.
fn run_self_check() -> Result<String, Box<dyn Error>> {
    let (input, expectation) = synthetic_fixtures();
    let base = build_acceptance_package(&input)?;
    validate_acceptance_package(&base, &expectation)?;

    let mut checked = 0usize;

    // (1) a non-diagnostic claim.
    let mut m = base.clone();
    m.claim_scope = "product_progress".to_owned();
    expect_refusal("non_diagnostic_claim", &m, &expectation, "ClaimScopeDrift")?;
    checked += 1;

    // (2) a renamed body residual.
    let mut m = base.clone();
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
    checked += 1;

    // (3) a gate without usable evidence.
    let mut m = base.clone();
    m.mandatory_gates[0].evidence.manifest = String::new();
    expect_refusal(
        "gate_without_evidence",
        &m,
        &expectation,
        "GateWithoutEvidence",
    )?;
    checked += 1;

    // (4) a gate whose evidence carrier is not bound.
    let mut m = base.clone();
    m.mandatory_gates[0].evidence = CarrierBinding {
        manifest: "unbound.json".to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(b"unbound"),
    };
    expect_refusal("unbound_carrier", &m, &expectation, "UnboundCarrier")?;
    checked += 1;

    // (5) an authorization that the blocking set does not support.
    let mut m = base.clone();
    m.authorization.s09_authorized = true;
    expect_refusal(
        "unmet_gate_authorized",
        &m,
        &expectation,
        "UnmetGateAuthorized",
    )?;
    checked += 1;

    // (6) authorization derived from the harness lifecycle.
    let mut m = base.clone();
    m.authorization.gsd_lifecycle_authorizes = true;
    expect_refusal(
        "gsd_lifecycle_authorization",
        &m,
        &expectation,
        "GsdLifecycleAuthorization",
    )?;
    checked += 1;

    // (7) a blocking set that drops a blocking gate.
    let mut m = base.clone();
    m.blocking_gates.pop();
    expect_refusal("blocking_set_drift", &m, &expectation, "BlockingSetDrift")?;
    checked += 1;

    // (8) an opened lock.
    let mut m = base.clone();
    m.locks.insert("deployed".to_owned(), "true".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;
    checked += 1;

    // (9) a declared requirement outside the DB-canonical set.
    let mut m = base.clone();
    m.attribution
        .declared_requirement_ids
        .push("R999".to_owned());
    expect_refusal(
        "declared_requirement_not_canonical",
        &m,
        &expectation,
        "DeclaredRequirementNotCanonical",
    )?;
    checked += 1;

    // (10) a threshold gate dropped from a package that claims three.
    let mut m = base.clone();
    m.mandatory_gates.retain(|gate| gate.kind != "threshold");
    expect_refusal(
        "threshold_gate_count_drift",
        &m,
        &expectation,
        "ThresholdCountDrift",
    )?;
    checked += 1;

    // (11) a drifted evidence binding.
    let mut drifted = expectation.clone();
    drifted.evidence[0].revision_sha256 = sha256_prefixed(b"drifted");
    expect_refusal("evidence_drift", &base, &drifted, "EvidenceDrift")?;
    checked += 1;

    // (12) a reviewer-recomputation block that admits corpus-derived pins and
    // names no recompute surface.
    let mut m = base.clone();
    m.reviewer_recomputation.corpus_derived_pins = true;
    m.reviewer_recomputation.recompute_surfaces.clear();
    expect_refusal(
        "reviewer_recomputation_drift",
        &m,
        &expectation,
        "ReviewerRecomputationDrift",
    )?;
    checked += 1;

    // (13) an independence block with no judges whose candidate is named as its
    // own reference.
    let mut m = base.clone();
    m.carrier_independence.judges.clear();
    m.carrier_independence.candidate_manifest =
        m.carrier_independence.reference_manifests[0].clone();
    expect_refusal(
        "carrier_independence_drift",
        &m,
        &expectation,
        "CarrierIndependenceDrift",
    )?;
    checked += 1;

    // (14) a `same_series` verdict whose two fingerprints differ.
    let mut m = base.clone();
    assert_eq!(
        m.carrier_independence.series_identity.verdict, "same_series",
        "the synthetic fixture must derive a same_series verdict"
    );
    m.carrier_independence.series_identity.candidate_fingerprint =
        sha256_prefixed(b"a-different-candidate-series");
    expect_refusal(
        "series_identity_same_series_fingerprints_differ",
        &m,
        &expectation,
        "CarrierIndependenceDrift",
    )?;
    checked += 1;

    // (15) an `independent_series` verdict carrying two equal fingerprints.
    let mut m = base.clone();
    m.carrier_independence.series_identity.verdict = "independent_series".to_owned();
    expect_refusal(
        "series_identity_independent_series_fingerprints_equal",
        &m,
        &expectation,
        "CarrierIndependenceDrift",
    )?;
    checked += 1;

    Ok(format!(
        "acceptance package self-check: {checked} named refusals executed, all refused"
    ))
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

/// Hermetic fixtures: a synthetic evidence set, evaluation and gate ledger that
/// validate without reading any carrier.
fn synthetic_fixtures() -> (AcceptancePackageInput, AcceptancePackageExpectation) {
    let binding = |manifest: &str, pointer: &str| CarrierBinding {
        manifest: manifest.to_owned(),
        pointer: pointer.to_owned(),
        revision_sha256: sha256_prefixed(manifest.as_bytes()),
    };
    let candidate_binding = binding("candidate.json", "documents");
    let pins_binding = binding("pins.json", "values");
    let schema_binding = binding("schema.yaml", "required_scope.families");
    let schema_requirements_binding = binding("schema.yaml", "requirement_ids");
    let body_binding = binding("body.json", "aggregate");
    let m513_binding = binding("m513.json", "remainders");
    let evidence = vec![
        candidate_binding.clone(),
        pins_binding.clone(),
        schema_binding.clone(),
        schema_requirements_binding.clone(),
        body_binding.clone(),
        m513_binding.clone(),
    ];

    let evaluation_gates = vec![
        MandatoryGate {
            gate_id: "threshold.t1".to_owned(),
            kind: "threshold".to_owned(),
            state: "unmet".to_owned(),
            required: true,
            family_id: Some("family.required".to_owned()),
            metric_id: Some("metric.required".to_owned()),
            reason: None,
            evidence: "measured metric.required on the synthetic candidate".to_owned(),
        },
        MandatoryGate {
            gate_id: NATIVE_ACCEPTANCE_BODY_AXIS_GATE_ID.to_owned(),
            kind: "body_axis".to_owned(),
            state: "not_assessable".to_owned(),
            required: true,
            family_id: None,
            metric_id: None,
            reason: Some(BODY_TOKEN.to_owned()),
            evidence: "body residual is not assessable".to_owned(),
        },
        MandatoryGate {
            gate_id: "required_scope.family.pending".to_owned(),
            kind: "threshold_pending_family".to_owned(),
            state: "threshold_pending".to_owned(),
            required: true,
            family_id: Some("family.pending".to_owned()),
            metric_id: None,
            reason: Some("lane_not_admitted_in_measurement".to_owned()),
            evidence: "family.pending has no measurable mechanism".to_owned(),
        },
    ];
    let thresholds = vec![ThresholdVerdict {
        threshold_id: "t1".to_owned(),
        family_id: "family.required".to_owned(),
        metric_id: "metric.required".to_owned(),
        stratum: None,
        direction: "at_least".to_owned(),
        margin: 0.0,
        verdict: "unmet".to_owned(),
        reason: None,
        frozen_reference_value: Some(1.0),
        candidate_value: Some(0.0),
        candidate_denominator: Some(1.0),
        comparability: "matched".to_owned(),
    }];

    let extra_gates = vec![make_gate(
        "named_gap.synthetic",
        "named_gap",
        "pending",
        m513_binding,
        "synthetic named gap".to_owned(),
        Some("synthetic".to_owned()),
        "synthetic evidence".to_owned(),
    )];

    let strings = |values: &[&str]| -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    };
    let attribution = build_attribution(
        &strings(&["R1"]),
        &strings(&["R1", "R2"]),
        &strings(&["R3"]),
        &strings(&["R2"]),
        schema_requirements_binding,
    )
    .expect("the synthetic attribution reconciles");

    // A complete synthetic series pair: both sides are the same series, so the
    // derived verdict is `same_series` and the base fixture validates.
    let synthetic_series = Series {
        corpus_id: "synthetic-corpus".to_owned(),
        profile: "synthetic-profile".to_owned(),
        oracle: "synthetic-oracle".to_owned(),
        source_revision: "synthetic-revision".to_owned(),
    };

    let input = AcceptancePackageInput {
        claim_scope: "diagnostic_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
        locks: closed_locks(),
        evaluation: EvaluationBinding {
            schema_version: NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_owned(),
            threshold_policy_state: "frozen".to_owned(),
            candidate_source_path: candidate_binding.manifest.clone(),
            frozen_reference_path: pins_binding.manifest.clone(),
            thresholds,
        },
        candidate_series: Some(synthetic_series.clone()),
        reference_series: Some(synthetic_series),
        evaluation_gates,
        gate_sources: GateSources {
            candidate_measurement: candidate_binding,
            pins: pins_binding,
            schema: schema_binding,
            body_residual: body_binding,
        },
        extra_gates,
        evidence: evidence.clone(),
        attribution,
        human_release_recorded: false,
    };
    let expectation = AcceptancePackageExpectation {
        evidence,
        threshold_gate_count: 1,
        pending_family_gate_count: 1,
        downgrade_gate_count: 0,
        body_residual: vec![BODY_TOKEN.to_owned()],
        db_canonical_requirement_ids: strings(&["R1", "R2"]),
    };
    (input, expectation)
}
