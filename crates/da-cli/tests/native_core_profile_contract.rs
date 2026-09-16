//! S08/T01 contract gate for the native CRF core profile (M507-4fk6m6).
//!
//! Deliberately NOT feature-gated: the profile module, its example, and this
//! gate must compile and run in the default feature set, which is why
//! `layout_runtime` is no longer behind `pdf-layout`.
//!
//! What this gate proves:
//! 1. two independent profiles of the frozen models emit byte-identical
//!    artifacts, and the observational timing stays out of those bytes;
//! 2. every family's scalar oracle digest equals its semantic digest, while a
//!    mutated kernel is reported as a typed semantic mismatch;
//! 3. selection stays forbidden and the H13-N2 gate stays closed (SIMD/PGO only
//!    after profile + scalar equivalence — never here);
//! 4. the DATA threshold is really read: mutating it changes the decision, and
//!    an eligible phase outside the closed vocabulary is a contract violation;
//! 5. the artifact leaks no model token, no `.wapiti` byte, no filesystem path
//!    and no hostname — each family stays bound by digest only;
//! 6. allocator deltas are stability-gated: equal replays yield numbers,
//!    disagreeing replays yield a typed unstable state with no numbers.

use da_cli::layout_runtime::{
    CharacterizationError, OptimizationState, characterize_cpu_kernel, gate_simd_candidate,
};
use da_cli::native_core_profile::{
    AllocatorObservation, AllocatorSnapshot, NATIVE_CORE_PHASES, NativeCoreFamilySpec,
    NativeCoreProfileOptions, PHASE_VITERBI_DECODE, evaluate_dense_numeric_kernel,
    load_qualification_from_manifest, profile_native_core,
};
use da_domain::scalar_crf::{CrfModel, map_input, viterbi_decode};
use std::cell::Cell;
use std::collections::BTreeMap;

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const FULLTEXT: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_fulltext_synthetic.wapiti");
const FIGURE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_figure_synthetic.wapiti");
const REFERENCE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti");
const ROWS: usize = 64;

fn families() -> Vec<NativeCoreFamilySpec> {
    [
        ("fulltext", FULLTEXT),
        ("figure", FIGURE),
        ("bibliography_reference", REFERENCE),
    ]
    .into_iter()
    .map(|(family, bytes)| {
        NativeCoreFamilySpec::from_frozen_model(family, bytes, ROWS)
            .expect("frozen synthetic model is part of the slice contract")
    })
    .collect()
}

/// Equal deltas on both passes: 100 allocations / 3200 bytes each.
fn stable_snapshot() -> impl FnMut() -> AllocatorSnapshot {
    let counter = Cell::new(0u64);
    move || {
        counter.set(counter.get() + 100);
        AllocatorSnapshot {
            allocations: counter.get(),
            allocated_bytes: counter.get() * 32,
        }
    }
}

/// Escalating deltas: 300 allocations on the first pass, 700 on the second.
fn escalating_snapshot() -> impl FnMut() -> AllocatorSnapshot {
    let step = Cell::new(0u64);
    move || {
        step.set(step.get() + 1);
        let value = step.get();
        AllocatorSnapshot {
            allocations: value * value * 100,
            allocated_bytes: value * 1000,
        }
    }
}

fn options(
    data: &da_cli::native_core_profile::NativeCoreQualificationData,
) -> NativeCoreProfileOptions {
    NativeCoreProfileOptions::from_data(data, ROWS)
}

#[test]
fn two_independent_profiles_emit_identical_artifact_bytes() {
    let data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let specs = families();
    let options = options(&data);

    let mut first_snapshot = stable_snapshot();
    let first = profile_native_core(&data, &specs, &options, &mut first_snapshot)
        .expect("first independent profile");
    let mut second_snapshot = stable_snapshot();
    let second = profile_native_core(&data, &specs, &options, &mut second_snapshot)
        .expect("second independent profile");

    let first_bytes = first.artifact_json_bytes().expect("artifact bytes");
    let second_bytes = second.artifact_json_bytes().expect("artifact bytes");
    assert_eq!(
        first_bytes, second_bytes,
        "two independent profiles of the same frozen models must be byte-identical"
    );

    let text = String::from_utf8(first_bytes).expect("artifact is utf8");
    assert!(
        !text.contains("phase_nanos") && !text.contains("phase_share_basis_points"),
        "observational timing must not enter the artifact bytes"
    );
    assert_eq!(first.observations.families.len(), specs.len());
    assert!(
        first
            .observations
            .families
            .iter()
            .all(|family| !family.phase_nanos.is_empty()),
        "the receipt keeps the observational timings the artifact excludes"
    );
    assert!(
        first
            .observations
            .excluded_from
            .iter()
            .any(|field| field == "acceptance_denominators")
    );
}

#[test]
fn scalar_oracle_equals_semantic_digest_per_family() {
    let data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let specs = families();
    let mut snapshot = stable_snapshot();
    let receipt =
        profile_native_core(&data, &specs, &options(&data), &mut snapshot).expect("profile");

    for family in &receipt.families {
        assert_eq!(
            family.characterization.scalar_oracle_digest, family.characterization.semantic_digest,
            "family {} must keep one digest for oracle and semantics",
            family.family
        );
        assert!(
            family
                .characterization
                .scalar_oracle_digest
                .starts_with("sha256:")
        );
        assert!(family.characterization.cpu_only);
        assert!(!family.characterization.subprocess_io_included);
        assert!(
            family.positions > 0 && family.matched_observations > 0,
            "family {} must actually reach the kernel",
            family.family
        );
    }
}

#[test]
fn mutated_kernel_is_reported_as_semantic_mismatch() {
    let model = CrfModel::parse(FULLTEXT).expect("frozen model");
    let rows = da_cli::native_core_profile::derive_rows(&model, ROWS).expect("model rows");
    let mut text = String::new();
    for row in &rows {
        text.push_str(row);
        text.push('\n');
    }

    let mut call = 0usize;
    let result = characterize_cpu_kernel(0, 3, text.len().max(1), rows.len() as u64, |arena| {
        call += 1;
        let buffer = arena.allocate(text.len())?;
        buffer.copy_from_slice(text.as_bytes());
        let positions = map_input(&model, &text).expect("dry run");
        let mut decoded = viterbi_decode(&model, &positions).expect("dry run");
        if call > 1 {
            // Mutate the semantics of the second pass only.
            decoded.labels[0].push_str("_mutated");
        }
        let mut bytes = Vec::new();
        for label in &decoded.labels {
            bytes.extend_from_slice(label.as_bytes());
        }
        Ok(bytes)
    });

    assert!(
        matches!(result, Err(CharacterizationError::SemanticMismatch { .. })),
        "a mutated kernel must fail loudly, got {result:?}"
    );
}

#[test]
fn selection_is_forbidden_and_the_h13n2_gate_is_closed() {
    let data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let specs = families();
    let mut snapshot = stable_snapshot();
    let receipt =
        profile_native_core(&data, &specs, &options(&data), &mut snapshot).expect("profile");

    assert!(receipt.disposition.selection_forbidden);
    for family in &receipt.families {
        assert!(family.characterization.selection_forbidden);
        assert!(family.disposition.selection_forbidden);
        assert!(
            family
                .disposition
                .optimization_reason
                .contains(&data.dense_numeric_kernel_rule.rule_id),
            "the disposition reason must cite the DATA rule"
        );
        assert!(
            family
                .disposition
                .optimization_reason
                .contains("candidate=none_implemented")
        );
        assert!(matches!(
            family.disposition.optimization_state,
            OptimizationState::NotApplicable | OptimizationState::MeasuredRejected
        ));
    }
    assert!(matches!(
        receipt.disposition.optimization_state,
        OptimizationState::NotApplicable | OptimizationState::MeasuredRejected
    ));
    assert_eq!(receipt.disposition.pgo_disposition, "not_adopted");

    // H13-N2: no dense kernel means not applicable; a candidate that was never
    // measured for semantic equivalence can never become retained.
    assert_eq!(
        gate_simd_candidate(false, true, true, false).0,
        OptimizationState::NotApplicable
    );
    assert_eq!(
        gate_simd_candidate(true, true, true, true).0,
        OptimizationState::MeasuredRejected,
        "a full-pipeline p95 regression blocks retention"
    );
    assert_eq!(
        gate_simd_candidate(true, true, false, false).0,
        OptimizationState::MeasuredRejected,
        "a candidate with no repeated CPU improvement is never retained"
    );
    // Retention is reachable only when every proof exists. This slice measures
    // no candidate, so it always feeds `false` for semantic equivalence and
    // repeatability (see the disposition inputs in `gate_disposition`).
    assert_eq!(
        gate_simd_candidate(true, true, true, false).0,
        OptimizationState::MeasuredRetained,
        "retention requires all three proofs at once"
    );
    assert_eq!(
        gate_simd_candidate(true, false, false, true).0,
        OptimizationState::MeasuredRejected,
        "the inputs this slice feeds can never retain anything"
    );
}

#[test]
fn data_threshold_is_read_and_changes_the_decision() {
    let mut data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let rule = data.dense_numeric_kernel_rule.clone();
    assert_eq!(rule.rule_id, "dense-contiguous-numeric-phase");
    assert_eq!(rule.threshold, 0.25);
    assert_eq!(rule.eligible_phases, vec!["viterbi_decode".to_string()]);
    assert_eq!(rule.outcome_when_unmet, "not_applicable");

    let mut shares: BTreeMap<String, f64> = NATIVE_CORE_PHASES
        .iter()
        .map(|phase| (phase.to_string(), 0.0))
        .collect();
    shares.insert(PHASE_VITERBI_DECODE.to_string(), 0.5);

    let generous = evaluate_dense_numeric_kernel(&rule, &shares);
    assert!(
        generous.found,
        "a 0.5 dense-phase share clears the declared 0.25 threshold"
    );
    assert_eq!(generous.phase.as_deref(), Some(PHASE_VITERBI_DECODE));

    data.dense_numeric_kernel_rule.threshold = 0.9;
    let strict = evaluate_dense_numeric_kernel(&data.dense_numeric_kernel_rule, &shares);
    assert!(
        !strict.found,
        "the DATA threshold must be read, never hardcoded"
    );
    assert_eq!(strict.reason_key, "no_eligible_phase_at_or_above_threshold");

    // Negative: an eligible phase outside the closed vocabulary is refused.
    let mut broken_rule = rule.clone();
    broken_rule
        .eligible_phases
        .push("invented_phase".to_string());
    let mut broken_data = data.clone();
    broken_data.dense_numeric_kernel_rule = broken_rule;
    assert!(broken_data.validate().is_err());

    // Negative: a weakened promotion lock is refused.
    let mut weakened = data.clone();
    weakened.dense_numeric_kernel_rule = rule.clone();
    weakened.promotion.auto_promote = "allowed".to_string();
    assert!(weakened.validate().is_err());

    // Negative: a retention without scalar equivalence is refused.
    let mut weakened = data.clone();
    weakened.dense_numeric_kernel_rule = rule.clone();
    weakened
        .optimization
        .retained_thresholds
        .requires_scalar_equivalence = false;
    assert!(weakened.validate().is_err());
}

#[test]
fn artifact_is_redacted() {
    let data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let specs = families();
    let mut snapshot = stable_snapshot();
    let receipt =
        profile_native_core(&data, &specs, &options(&data), &mut snapshot).expect("profile");
    let text = String::from_utf8(receipt.artifact_json_bytes().expect("artifact bytes"))
        .expect("artifact is utf8");

    assert!(!text.contains(".wapiti"), "no model filename may appear");
    assert!(
        !text.contains("#mdl#") && !text.contains("#rdr#"),
        "no model bytes may appear"
    );
    for marker in ["/root/", "/home/", "/tmp/", "/usr/"] {
        assert!(
            !text.contains(marker),
            "no physical path may appear: {marker}"
        );
    }
    for spec in &specs {
        for observation in &spec.model.observations {
            if observation.len() >= 8 {
                assert!(
                    !text.contains(observation.as_str()),
                    "artifact leaked model token `{observation}`"
                );
            }
        }
    }
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        let hostname = hostname.trim();
        if hostname.len() >= 4 {
            assert!(!text.contains(hostname), "artifact leaked the hostname");
        }
    }
    for family in &receipt.families {
        assert!(
            text.contains(&family.model_sha256),
            "family {} must stay bound by model digest",
            family.family
        );
    }
    let debug = format!("{receipt:?}");
    assert!(
        !debug.contains("#mdl#"),
        "the receipt carries no model bytes"
    );
}

#[test]
fn allocator_observation_is_typed_and_stability_gated() {
    let data = load_qualification_from_manifest(MANIFEST).expect("qualification block");
    let specs = families();
    let options = options(&data);

    let mut stable = stable_snapshot();
    let stable_receipt =
        profile_native_core(&data, &specs, &options, &mut stable).expect("stable profile");
    match &stable_receipt.allocator {
        AllocatorObservation::Stable {
            allocations,
            allocated_bytes,
        } => {
            // One increment per snapshot call: 100 allocations and 100*32 bytes.
            assert_eq!(*allocations, 100);
            assert_eq!(*allocated_bytes, 3200);
        }
        other => panic!("equal replays must yield numbers, got {other:?}"),
    }

    let mut escalating = escalating_snapshot();
    let unstable_receipt =
        profile_native_core(&data, &specs, &options, &mut escalating).expect("unstable profile");
    match &unstable_receipt.allocator {
        AllocatorObservation::Unstable { first, second } => assert_ne!(first, second),
        other => panic!("disagreeing replays must stay typed, got {other:?}"),
    }

    let unstable_json: serde_json::Value =
        serde_json::from_slice(&unstable_receipt.artifact_json_bytes().expect("bytes"))
            .expect("artifact json");
    let allocator = unstable_json["allocator"]
        .as_object()
        .expect("allocator object");
    assert_eq!(
        allocator.len(),
        1,
        "an unstable observation carries a state and no numbers: {allocator:?}"
    );
    assert_eq!(allocator["state"], "unstable");

    let stable_json: serde_json::Value =
        serde_json::from_slice(&stable_receipt.artifact_json_bytes().expect("bytes"))
            .expect("artifact json");
    assert_eq!(stable_json["allocator"]["state"], "stable");
}
