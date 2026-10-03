//! S09/T01 (M514-1lss2s) — contract for the fail-closed retirement
//! authorization gate.
//!
//! The gate is a pure predicate: this binary proves it against (a) a fully
//! authorized synthetic record whose gates are all `met`, and (b) a named
//! negative-mutation matrix in which every refusal is a typed code from the
//! closed vocabulary. The live half then reads the committed S08 gate ledger
//! and the committed human release record and asserts the exact typed reasons
//! for the refusal that must exist today (`reviewer=null`, `decision=pending`,
//! twenty blocking gates).
//!
//! The test is deliberately ungated (no `#![cfg(feature = ...)]`): the
//! `da-parse --tests` gate compiles it in the default feature set.

use da_parse::{
    HumanReleaseRecord, MandatoryGate, NativeAcceptanceEvaluation, PackageBinding,
    REFUSAL_FAMILY_GATE_BLOCKS, REFUSAL_HUMAN_DECISION_NOT_RECORDED,
    REFUSAL_HUMAN_REVIEWER_UNNAMED, REFUSAL_LIFECYCLE_STATE_AUTHORIZES, REFUSAL_OPEN_LOCK,
    REFUSAL_PACKAGE_ABSENT, REFUSAL_PACKAGE_PIN_STALE, REFUSAL_PACKAGE_STATUS_UNKNOWN,
    REFUSAL_PROSE_DOES_NOT_AUTHORIZE, REFUSAL_REQUIRED_GATE_BLOCKS,
    RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY, RETIREMENT_AUTHORIZATION_REFUSED,
    RETIREMENT_BASIS_HUMAN_RELEASE_REQUIRED, RETIREMENT_CLAIM_SCOPE_BLOCKED,
    RETIREMENT_HUMAN_DECISION_RECORDED_TOKENS, RETIREMENT_HUMAN_DECISION_VOCABULARY,
    RETIREMENT_PACKAGE_STATUS_AUTHORIZED, RETIREMENT_REQUIRED_LOCKS, RetirementAuthorizationInput,
    authorize_native_retirement, sha256_hex,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const PACKAGE_REL: &str = "artifacts/parse_compat/grobid_retirement_acceptance_package.json";
const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";

const FAMILY_PRESERVATION: &str = "parse.component.native_fulltext_composition_shadow";
const FAMILY_HEADER: &str = "stage.header_composition";

/// A well-formed `sha256:` pin used only by the synthetic positive control.
const SYNTHETIC_PIN: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The nineteen required, non-clear gates the committed S08 ledger carries today.
const LIVE_BLOCKING_GATE_COUNT: usize = 19;
/// The fifteen `required_scope.families` the committed schema declares.
const LIVE_REQUIRED_FAMILY_COUNT: usize = 15;

// ───────────────────────────── helpers ─────────────────────────────

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

fn gate(gate_id: &str, kind: &str, state: &str, family_id: Option<&str>) -> MandatoryGate {
    MandatoryGate {
        gate_id: gate_id.to_string(),
        kind: kind.to_string(),
        state: state.to_string(),
        required: true,
        family_id: family_id.map(str::to_string),
        metric_id: None,
        reason: None,
        evidence: "synthetic contract gate".to_string(),
    }
}

fn closed_locks() -> BTreeMap<String, String> {
    RETIREMENT_REQUIRED_LOCKS
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

fn synthetic_evaluation() -> NativeAcceptanceEvaluation {
    NativeAcceptanceEvaluation {
        schema_version: da_parse::NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_string(),
        claim_scope: da_parse::NATIVE_ACCEPTANCE_CLAIM_SCOPE.to_string(),
        product_coverage_effect: da_parse::NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT.to_string(),
        candidate_source_path: "artifacts/parse_compat/native_corpus_acceptance.json".to_string(),
        candidate_series: None,
        threshold_policy_state: "frozen".to_string(),
        frozen_reference_path: "artifacts/parse_compat/absorption_progress_reference_pins.json"
            .to_string(),
        thresholds: Vec::new(),
        gates: vec![
            gate(
                "threshold.preservation_micro_occurrence",
                "threshold",
                "met",
                Some(FAMILY_PRESERVATION),
            ),
            gate(
                "required_scope.stage.header_composition",
                "threshold_pending_family",
                "met",
                Some(FAMILY_HEADER),
            ),
            gate(
                "h21_revision_label_residual",
                "named_gap",
                "accepted_residual",
                None,
            ),
        ],
    }
}

/// The positive control: a named reviewer, a recorded decision, closed locks, a
/// current package pin and every required gate `met` / `accepted_residual`.
fn authorized_input() -> RetirementAuthorizationInput {
    RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_string(),
            declared_sha256: Some(SYNTHETIC_PIN.to_string()),
            actual_sha256: Some(SYNTHETIC_PIN.to_string()),
        },
        human_release: HumanReleaseRecord {
            reviewer: Some("Jane Reviewer".to_string()),
            decision: "approved".to_string(),
            decision_record: Some("doc/review/HUMAN-REVIEW-TEMPLATE.md".to_string()),
            prose_alone_authorizes_release: false,
            package_status: RETIREMENT_PACKAGE_STATUS_AUTHORIZED.to_string(),
        },
        lifecycle_state_authorizes: false,
        locks: closed_locks(),
        required_families: vec![FAMILY_PRESERVATION.to_string(), FAMILY_HEADER.to_string()],
        evaluation: synthetic_evaluation(),
    }
}

fn codes(out: &da_parse::NativeRetirementAuthorization) -> Vec<&str> {
    out.refusals.iter().map(|r| r.code.as_str()).collect()
}

fn all_typed(codes: &[&str]) -> bool {
    codes
        .iter()
        .all(|code| RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY.contains(code))
}

// ───────────────────────────── authorization ─────────────────────────────

#[test]
fn fully_authorized_record_with_all_gates_met_is_authorized() {
    let out = authorize_native_retirement(&authorized_input());
    assert_eq!(out.verdict, da_parse::RETIREMENT_AUTHORIZATION_AUTHORIZED);
    assert!(out.s09_authorized);
    assert!(out.refusals.is_empty(), "{:?}", out.refusals);
    assert_eq!(out.basis, da_parse::RETIREMENT_BASIS_RECORDED_HUMAN_RELEASE);
    assert_eq!(out.claim_scope, da_parse::RETIREMENT_CLAIM_SCOPE_AUTHORIZED);
    assert!(out.human_reviewer_named);
    assert!(out.human_decision_recorded);
    assert!(out.blocking_gates.is_empty());
    assert_eq!(out.families.len(), 2);
    assert!(out.families.iter().all(|family| family.authorized));
    // Authorization evidence is never product coverage.
    assert_eq!(out.product_coverage_effect, "none");
}

#[test]
fn s09_authorized_is_computed_from_the_refusal_set() {
    let authorized = authorize_native_retirement(&authorized_input());
    assert!(authorized.s09_authorized == authorized.refusals.is_empty());

    let mut input = authorized_input();
    input.human_release.reviewer = None;
    let refused = authorize_native_retirement(&input);
    assert!(!refused.s09_authorized);
    assert!(!refused.refusals.is_empty());
    assert!(refused.s09_authorized == refused.refusals.is_empty());
}

#[test]
fn refusal_vocabulary_is_closed_and_unique() {
    let unique: BTreeSet<&str> = RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY
        .iter()
        .copied()
        .collect();
    assert_eq!(
        unique.len(),
        RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY.len(),
        "the refusal vocabulary must not repeat a code"
    );
    // The open-lock token is reused from the da-parse acceptance vocabulary.
    assert!(RETIREMENT_AUTHORIZATION_REFUSAL_VOCABULARY.contains(&REFUSAL_OPEN_LOCK));
    // Every recorded decision token is part of the closed decision vocabulary.
    for token in RETIREMENT_HUMAN_DECISION_RECORDED_TOKENS {
        assert!(RETIREMENT_HUMAN_DECISION_VOCABULARY.contains(&token));
    }
}

#[test]
fn named_negative_mutation_matrix_is_typed() {
    type Mutation = Box<dyn Fn(&mut RetirementAuthorizationInput)>;
    let cases: Vec<(&str, Mutation, &str)> = vec![
        (
            "reviewer absent",
            Box::new(|input| input.human_release.reviewer = None),
            REFUSAL_HUMAN_REVIEWER_UNNAMED,
        ),
        (
            "reviewer blank",
            Box::new(|input| input.human_release.reviewer = Some("   ".to_string())),
            REFUSAL_HUMAN_REVIEWER_UNNAMED,
        ),
        (
            "decision pending",
            Box::new(|input| input.human_release.decision = "pending".to_string()),
            REFUSAL_HUMAN_DECISION_NOT_RECORDED,
        ),
        (
            "decision rejected",
            Box::new(|input| input.human_release.decision = "rejected".to_string()),
            REFUSAL_HUMAN_DECISION_NOT_RECORDED,
        ),
        (
            "decision unknown token",
            Box::new(|input| input.human_release.decision = "looks_fine".to_string()),
            REFUSAL_HUMAN_DECISION_NOT_RECORDED,
        ),
        (
            "prose authorizes",
            Box::new(|input| input.human_release.prose_alone_authorizes_release = true),
            REFUSAL_PROSE_DOES_NOT_AUTHORIZE,
        ),
        (
            "gsd lifecycle authorizes",
            Box::new(|input| input.lifecycle_state_authorizes = true),
            REFUSAL_LIFECYCLE_STATE_AUTHORIZES,
        ),
        (
            "deployed lock open",
            Box::new(|input| {
                input
                    .locks
                    .insert("deployed".to_string(), "true".to_string());
            }),
            REFUSAL_OPEN_LOCK,
        ),
        (
            "auto_promote lock drifted",
            Box::new(|input| {
                input
                    .locks
                    .insert("auto_promote".to_string(), "allowed".to_string());
            }),
            REFUSAL_OPEN_LOCK,
        ),
        (
            "graph_write lock missing",
            Box::new(|input| {
                input.locks.remove("graph_write");
            }),
            REFUSAL_OPEN_LOCK,
        ),
        (
            "package pin absent",
            Box::new(|input| input.package.declared_sha256 = None),
            REFUSAL_PACKAGE_ABSENT,
        ),
        (
            "package path blank",
            Box::new(|input| input.package.path = String::new()),
            REFUSAL_PACKAGE_ABSENT,
        ),
        (
            "package pin stale",
            Box::new(|input| {
                input.package.actual_sha256 = Some(
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                );
            }),
            REFUSAL_PACKAGE_PIN_STALE,
        ),
        (
            "family gate unmet",
            Box::new(|input| {
                input.evaluation.gates[0].state = "unmet".to_string();
            }),
            REFUSAL_FAMILY_GATE_BLOCKS,
        ),
        (
            "non-family gate pending",
            Box::new(|input| {
                input.evaluation.gates[2].state = "pending".to_string();
            }),
            REFUSAL_REQUIRED_GATE_BLOCKS,
        ),
        (
            "package status outside vocabulary",
            Box::new(|input| input.human_release.package_status = "made_up".to_string()),
            REFUSAL_PACKAGE_STATUS_UNKNOWN,
        ),
        (
            "recorded decision disagrees with package status",
            Box::new(|input| {
                input.human_release.package_status = "pending_human_review".to_string();
            }),
            REFUSAL_PACKAGE_STATUS_UNKNOWN,
        ),
        (
            "required family carries no gate",
            Box::new(|input| {
                input
                    .required_families
                    .push("stage.reference_segmentation".to_string());
            }),
            REFUSAL_FAMILY_GATE_BLOCKS,
        ),
    ];

    for (label, mutate, expected) in cases {
        let mut input = authorized_input();
        mutate(&mut input);
        let out = authorize_native_retirement(&input);
        assert!(!out.s09_authorized, "{label}: must be refused");
        let observed = codes(&out);
        assert!(
            observed.contains(&expected),
            "{label}: got {observed:?}, expected `{expected}`"
        );
        assert!(
            all_typed(&observed),
            "{label}: untyped code in {observed:?}"
        );
        assert!(
            out.families.iter().all(|family| !family.authorized),
            "{label}: no family may switch while refused"
        );
    }
}

// ───────────────────────────── live pending record ─────────────────────────────

fn map_gate(value: &Value) -> Result<MandatoryGate, Box<dyn Error>> {
    let field = |name: &str| -> Result<String, Box<dyn Error>> {
        value
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("mandatory gate is missing `{name}`").into())
    };
    Ok(MandatoryGate {
        gate_id: field("gate_id")?,
        kind: field("kind")?,
        state: field("state")?,
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        family_id: value
            .get("family_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        metric_id: value
            .get("metric_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        reason: value
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        evidence: value
            .get("evidence_needed")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

#[test]
fn live_pending_record_is_refused_with_the_exact_typed_reasons() -> Result<(), Box<dyn Error>> {
    let root = repo_root();

    // The human release record, read from the committed carrier by pointer.
    let manifest: serde_yaml::Value = serde_yaml::from_slice(&read(&root, MANIFEST_REL)?)?;
    let release = manifest
        .get("release_decision_package")
        .ok_or_else(|| -> Box<dyn Error> {
            "missing release_decision_package".to_string().into()
        })?;
    let review = release
        .get("human_release_review")
        .ok_or_else(|| -> Box<dyn Error> { "missing human_release_review".to_string().into() })?;
    let human_release = HumanReleaseRecord {
        reviewer: review
            .get("reviewer")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        decision: review
            .get("decision")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        decision_record: review
            .get("decision_record")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        prose_alone_authorizes_release: review
            .get("prose_alone_authorizes_release")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        package_status: release
            .get("package_status")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
    };

    // The gate must not fill the record for a human: it stays empty today.
    assert!(
        human_release.reviewer.is_none(),
        "the live record must name no reviewer"
    );
    assert_eq!(human_release.decision, "pending");
    assert!(!human_release.prose_alone_authorizes_release);
    assert_eq!(human_release.package_status, "pending_human_review");
    assert!(human_release.decision_record.is_some());

    // The S08 gate ledger, re-read from committed bytes.
    let package_bytes = read(&root, PACKAGE_REL)?;
    let package: Value = serde_json::from_slice(&package_bytes)?;
    let ledger = package
        .get("mandatory_gates")
        .and_then(Value::as_array)
        .ok_or_else(|| -> Box<dyn Error> {
            "mandatory_gates is not an array".to_string().into()
        })?;
    let gates: Vec<MandatoryGate> = ledger
        .iter()
        .map(map_gate)
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    // The contract's required families, read by pointer.
    let schema: serde_yaml::Value = serde_yaml::from_slice(&read(&root, SCHEMA_REL)?)?;
    let family_rows = schema
        .get("required_scope")
        .and_then(|scope| scope.get("families"))
        .and_then(|families| families.as_sequence())
        .ok_or_else(|| -> Box<dyn Error> {
            "required_scope.families is missing".to_string().into()
        })?;
    let required_families: Vec<String> = family_rows
        .iter()
        .filter_map(|row| {
            row.get("family_id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect();
    assert_eq!(required_families.len(), LIVE_REQUIRED_FAMILY_COUNT);

    // The five locks, stringified from the package's declared values.
    let locks: BTreeMap<String, String> = package
        .get("locks")
        .and_then(Value::as_object)
        .map(|locks| {
            locks
                .iter()
                .map(|(key, value)| {
                    let rendered = value
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| value.to_string());
                    (key.clone(), rendered)
                })
                .collect()
        })
        .unwrap_or_default();

    let digest = sha256_hex(&package_bytes);
    let input = RetirementAuthorizationInput {
        package: PackageBinding {
            path: PACKAGE_REL.to_string(),
            declared_sha256: Some(digest.clone()),
            actual_sha256: Some(digest),
        },
        human_release,
        lifecycle_state_authorizes: false,
        locks,
        required_families,
        evaluation: NativeAcceptanceEvaluation {
            schema_version: da_parse::NATIVE_ACCEPTANCE_EVALUATION_SCHEMA_VERSION.to_string(),
            claim_scope: da_parse::NATIVE_ACCEPTANCE_CLAIM_SCOPE.to_string(),
            product_coverage_effect: da_parse::NATIVE_ACCEPTANCE_PRODUCT_COVERAGE_EFFECT
                .to_string(),
            candidate_source_path: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("candidate_source_path"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            candidate_series: None,
            threshold_policy_state: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("threshold_policy_state"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            frozen_reference_path: package
                .get("evaluation")
                .and_then(|evaluation| evaluation.get("frozen_reference_path"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            thresholds: Vec::new(),
            gates,
        },
    };

    let out = authorize_native_retirement(&input);
    let observed = codes(&out);

    assert_eq!(out.verdict, RETIREMENT_AUTHORIZATION_REFUSED);
    assert!(!out.s09_authorized);
    assert_eq!(out.basis, RETIREMENT_BASIS_HUMAN_RELEASE_REQUIRED);
    assert_eq!(out.claim_scope, RETIREMENT_CLAIM_SCOPE_BLOCKED);
    assert_eq!(out.product_coverage_effect, "none");
    assert!(!out.human_reviewer_named);
    assert!(!out.human_decision_recorded);
    assert!(!out.lifecycle_state_authorizes);
    assert_eq!(
        out.blocking_gates.len(),
        LIVE_BLOCKING_GATE_COUNT,
        "{:?}",
        out.blocking_gates
    );
    assert_eq!(out.families.len(), LIVE_REQUIRED_FAMILY_COUNT);
    assert!(out.families.iter().all(|family| !family.authorized));

    for expected in [
        REFUSAL_HUMAN_REVIEWER_UNNAMED,
        REFUSAL_HUMAN_DECISION_NOT_RECORDED,
        REFUSAL_FAMILY_GATE_BLOCKS,
        REFUSAL_REQUIRED_GATE_BLOCKS,
    ] {
        assert!(
            observed.contains(&expected),
            "expected `{expected}` in {observed:?}"
        );
    }
    for unexpected in [
        REFUSAL_PACKAGE_PIN_STALE,
        REFUSAL_PACKAGE_ABSENT,
        REFUSAL_OPEN_LOCK,
    ] {
        assert!(
            !observed.contains(&unexpected),
            "unexpected `{unexpected}` in {observed:?}"
        );
    }
    assert!(all_typed(&observed), "untyped code in {observed:?}");

    Ok(())
}
