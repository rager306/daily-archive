//! Cross-language ACAP admission parity, Rust half (S07/T03).
//!
//! The runtime must read the single ACAP admission declaration, not a
//! hardcoded copy. This test loads
//! `scripts/governor/checks/article_context_assurance.yaml` through the strict
//! `da_adapters::load_acap_admission_policy` loader and drives the shared parity
//! oracle `governor/fixtures/acap_runtime_control_plane/admission_parity.vectors.json`
//! through the production `admit_stage_transition_with_policy`. The Python half
//! (T04) drives the same fixture through the governor rule; every case must
//! resolve to the same admit/block decision and the same typed
//! `admission:<code>`.
//!
//! The loader-built policy is also asserted equal to
//! `AcapAdmissionPolicy::canonical()` and to agree with it on every vector, so
//! the DATA declaration and the pinned default cannot drift.

use std::path::{Path, PathBuf};

use da_adapters::load_acap_admission_policy;
use da_domain::{AcapAdmissionPolicy, ArticleContextState, admit_stage_transition_with_policy};

const POLICY_REL: &str = "scripts/governor/checks/article_context_assurance.yaml";
const VECTORS_REL: &str =
    "governor/fixtures/acap_runtime_control_plane/admission_parity.vectors.json";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One row of the shared parity oracle. Field names match the fixture exactly.
#[derive(Debug, serde::Deserialize)]
struct ParityVector {
    case: String,
    prior: Option<ArticleContextState>,
    target: ArticleContextState,
    evidence_ref: String,
    skip_reason: Option<String>,
    admitted: bool,
    /// `null` means "admitted with no typed error code".
    code: Option<String>,
}

fn parity_vectors() -> Vec<ParityVector> {
    let bytes = std::fs::read(repo_root().join(VECTORS_REL)).expect("read parity vectors");
    serde_json::from_slice(&bytes).expect("parse parity vectors")
}

fn assert_agrees(case: &ParityVector, policy: &AcapAdmissionPolicy, label: &str) {
    let decision = admit_stage_transition_with_policy(
        policy,
        case.prior,
        case.target,
        &case.evidence_ref,
        case.skip_reason.as_deref(),
    );
    assert_eq!(
        decision.admitted, case.admitted,
        "[{label}] parity case {} admitted",
        case.case
    );
    assert_eq!(
        decision.error.as_ref().map(|error| error.code.as_str()),
        case.code.as_deref(),
        "[{label}] parity case {} code",
        case.case
    );
    // A blocked decision must carry the typed error; an admitted one must not.
    assert_eq!(
        decision.admitted,
        decision.error.is_none(),
        "[{label}] parity case {} error presence",
        case.case
    );
}

#[test]
fn data_loaded_policy_matches_canonical_default() {
    let loaded = load_acap_admission_policy(&repo_root().join(POLICY_REL))
        .expect("load the declaration through the loader");
    let canonical = AcapAdmissionPolicy::canonical();
    // Same order, entry set, sink and rules: the DATA read and the pinned
    // default are one policy, so a decision cannot come from two truths.
    assert_eq!(loaded, canonical, "loaded policy must equal canonical()");
}

#[test]
fn loaded_and_canonical_policies_agree_on_every_parity_vector() {
    let loaded = load_acap_admission_policy(&repo_root().join(POLICY_REL))
        .expect("load the declaration through the loader");
    let canonical = AcapAdmissionPolicy::canonical();
    let vectors = parity_vectors();
    assert_eq!(vectors.len(), 10, "the parity oracle carries ten cases");
    for case in &vectors {
        assert_agrees(case, &loaded, "loaded");
        assert_agrees(case, &canonical, "canonical");
    }
}

#[test]
fn forward_layer_skip_is_admitted_with_a_typed_reason() {
    // The divergence this slice closes: `parsed -> summarized` skips ACAP
    // layers. Both sides must admit it and expose the typed skip reason the run
    // recorder needs.
    let policy = load_acap_admission_policy(&repo_root().join(POLICY_REL)).unwrap();
    let decision = admit_stage_transition_with_policy(
        &policy,
        Some(ArticleContextState::Parsed),
        ArticleContextState::Summarized,
        "test-key-parse",
        Some("acap_skip:Parsed->Summarized"),
    );
    assert!(decision.admitted);
    assert_eq!(
        decision.skip_reason.as_deref(),
        Some("acap_skip:Parsed->Summarized")
    );
}

// ── negative loader surface (fail closed, never defaulted) ──────────────────

const VALID_FSM_BLOCK: &str = "fsm:\n  initial_state: source_registered\n  terminal_states: [accepted_checkpoint, failed_visible]\n  states: [source_registered, source_verified, parsed, structured, extracted, evidence_linked, embedded, materialized, enriched, resolved, summarized, finalized, task_accepted, accepted_checkpoint, failed_visible]\n  transitions: []\n  failure_transition_from: \"*\"\n  failure_transition_to: failed_visible\n  retry_requires_same_semantic_key: true\n";

fn write_temp(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join(name);
    std::fs::write(&path, body).expect("write temp policy");
    (directory, path)
}

#[test]
fn loader_rejects_missing_admission_block() {
    let (_directory, path) = write_temp("missing-admission.yaml", VALID_FSM_BLOCK);
    let error = load_acap_admission_policy(&path).expect_err("no admission block must fail closed");
    assert!(error.contains("has no admission block"), "{error}");
}

#[test]
fn loader_rejects_unknown_forward_rule() {
    let body = format!(
        "{VALID_FSM_BLOCK}admission:\n  entry_states: [parsed]\n  forward_rule: strict_adjacent\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n"
    );
    let (_directory, path) = write_temp("unknown-forward-rule.yaml", &body);
    let error =
        load_acap_admission_policy(&path).expect_err("unknown forward_rule must fail closed");
    assert!(error.contains("unknown admission.forward_rule"), "{error}");
}

#[test]
fn loader_rejects_unknown_same_layer_repeat() {
    let body = format!(
        "{VALID_FSM_BLOCK}admission:\n  entry_states: [parsed]\n  forward_rule: monotone_subset\n  same_layer_repeat: sometimes\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n"
    );
    let (_directory, path) = write_temp("unknown-repeat.yaml", &body);
    let error =
        load_acap_admission_policy(&path).expect_err("unknown same_layer_repeat must fail closed");
    assert!(
        error.contains("unknown admission.same_layer_repeat"),
        "{error}"
    );
}

#[test]
fn loader_rejects_weakened_evidence_requirement() {
    let body = format!(
        "{VALID_FSM_BLOCK}admission:\n  entry_states: [parsed]\n  forward_rule: monotone_subset\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: false\n  skip_requires_typed_reason: true\n"
    );
    let (_directory, path) = write_temp("weak-evidence.yaml", &body);
    let error =
        load_acap_admission_policy(&path).expect_err("evidence_required != true must fail closed");
    assert!(error.contains("evidence_required != true"), "{error}");
}

#[test]
fn loader_rejects_failure_sink_disagreement() {
    // `admission.failure_sink` must be the fsm-declared sink; otherwise the two
    // sides could disagree about where failure lands.
    let body = format!(
        "{VALID_FSM_BLOCK}admission:\n  entry_states: [parsed]\n  forward_rule: monotone_subset\n  same_layer_repeat: allowed\n  failure_sink: accepted_checkpoint\n  evidence_required: true\n  skip_requires_typed_reason: true\n"
    );
    let (_directory, path) = write_temp("sink-disagreement.yaml", &body);
    let error = load_acap_admission_policy(&path).expect_err("sink disagreement must fail closed");
    assert!(error.contains("failure_sink"), "{error}");
}

#[test]
fn loader_rejects_entry_state_not_a_declared_state() {
    // Drop `resolved` from the states list and offer it as an entry: the loader
    // must refuse (it is not a declared fsm state).
    let states = "source_registered, source_verified, parsed, structured, extracted, evidence_linked, embedded, materialized, enriched, summarized, finalized, task_accepted, accepted_checkpoint, failed_visible";
    let fsm = format!(
        "fsm:\n  initial_state: source_registered\n  terminal_states: [accepted_checkpoint, failed_visible]\n  states: [{states}]\n  transitions: []\n  failure_transition_from: \"*\"\n  failure_transition_to: failed_visible\n  retry_requires_same_semantic_key: true\n"
    );
    let body = format!(
        "{fsm}admission:\n  entry_states: [resolved]\n  forward_rule: monotone_subset\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n"
    );
    let (_directory, path) = write_temp("undeclared-entry.yaml", &body);
    let error =
        load_acap_admission_policy(&path).expect_err("undeclared entry state must fail closed");
    assert!(error.contains("is not a declared fsm state"), "{error}");
}

#[test]
fn loader_rejects_malformed_yaml() {
    // A truncated declaration must surface the parse error, not a defaulted
    // policy: the runtime refuses to admit on an unreadable declaration.
    let (_directory, path) = write_temp("malformed.yaml", "fsm: [unterminated\n");
    let error = load_acap_admission_policy(&path).expect_err("malformed YAML must fail closed");
    assert!(error.contains("parse ACAP admission policy"), "{error}");
}

#[test]
fn loader_rejects_missing_fsm_block() {
    let body = "admission:\n  entry_states: [parsed]\n  forward_rule: monotone_subset\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n";
    let (_directory, path) = write_temp("missing-fsm.yaml", body);
    let error = load_acap_admission_policy(&path).expect_err("no fsm block must fail closed");
    assert!(error.contains("has no fsm block"), "{error}");
}

#[test]
fn loader_rejects_unknown_field_in_admission_block() {
    let body = format!(
        "{VALID_FSM_BLOCK}admission:\n  entry_states: [parsed]\n  forward_rule: monotone_subset\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n  surprise: true\n"
    );
    let (_directory, path) = write_temp("unknown-admission-field.yaml", &body);
    let error =
        load_acap_admission_policy(&path).expect_err("unknown admission field must fail closed");
    assert!(error.contains("unknown field"), "{error}");
}

#[test]
fn loader_rejects_oversized_or_missing_file() {
    let error = load_acap_admission_policy(Path::new("/nonexistent/acap.yaml"))
        .expect_err("a missing file must fail closed");
    assert!(error.contains("stat ACAP admission policy"), "{error}");

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("big.yaml");
    std::fs::write(&path, vec![b'#'; 2 * 1024 * 1024]).unwrap();
    let error = load_acap_admission_policy(&path).expect_err("an oversized file must fail closed");
    assert!(error.contains("is not a bounded file"), "{error}");
}
