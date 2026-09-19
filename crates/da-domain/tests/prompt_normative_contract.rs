//! Prompt change-control contract for M510/S02/T01.
//!
//! The summarizer system prompt has a **single authoritative source**:
//! `data/prompts/summarizer/system.v1.md` — the file the executed agent
//! manifest binds (`data/agents/summarizer/manifest.yaml`). The frozen
//! prompt-eval baseline `data/prompts/article-summary/baseline.v1.md` is an
//! explicitly **derived copy**: it is exactly the documented derivation
//! header followed by the authoritative source bytes.
//!
//! These tests fail closed when the derived copy is not exactly
//! `DERIVED_HEADER + source`, so the duplicated prompt source can never
//! silently diverge again. The header is the only tolerated difference;
//! every other byte must come from the source.
//!
//! Lives under `crates/da-domain/tests/` so the governor source scanners
//! (which exclude `/tests/`) do not read the literal markers below as
//! production DATA.

use da_domain::agent_manifest::AgentManifest;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Executed, authoritative prompt source (bound by the agent manifest).
const SOURCE_PATH: &str = "data/prompts/summarizer/system.v1.md";
/// Derived copy kept for the frozen prompt-eval fixture.
const DERIVED_PATH: &str = "data/prompts/article-summary/baseline.v1.md";
/// The manifest that binds the executed prompt for the summarizer agent.
const EXECUTED_MANIFEST_PATH: &str = "data/agents/summarizer/manifest.yaml";
/// Frozen eval fixture that names the derived copy as its baseline prompt.
const FROZEN_EVAL_PATH: &str = "governor/fixtures/prompt_eval/article-summary.eval.v1.json";

/// Documented derivation header. The derived copy must be exactly this header
/// followed by the authoritative source bytes; nothing else is tolerated.
const DERIVED_HEADER: &str = concat!(
    "<!-- DERIVED-FROM: data/prompts/summarizer/system.v1.md -->\n",
    "<!-- PROMPT-CONTRACT: PR-SUM-001, PR-SUM-002 -->\n",
    "<!-- IMMUTABLE: grounding, no_invention, evidence_binding; wording may be optimized only -->\n",
    "<!-- These grounding and no-invention obligations are proof-bound and must survive prompt optimization or editing. -->\n",
);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_repo_file(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("contract input {rel} must be readable: {e}"))
}

fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// The contract as a pure function: derived == documented header + source.
fn expected_derived(source: &str) -> String {
    format!("{DERIVED_HEADER}{source}")
}

/// Fail-closed drift predicate shared by the real-file test and the
/// negative tamper test.
fn derived_matches_source(derived: &str, source: &str) -> bool {
    derived == expected_derived(source)
}

#[test]
fn derived_copy_equals_documented_header_plus_authoritative_source() {
    let source = read_repo_file(SOURCE_PATH);
    let derived = read_repo_file(DERIVED_PATH);

    assert!(
        !source.is_empty(),
        "authoritative source {SOURCE_PATH} must not be empty"
    );
    assert!(
        derived_matches_source(&derived, &source),
        "derived copy {DERIVED_PATH} drifted from {SOURCE_PATH}: expected exactly \
         the documented derivation header followed by the source bytes"
    );
    // The body after the documented header is byte-identical to the source.
    let body = derived
        .strip_prefix(DERIVED_HEADER)
        .expect("derived copy must start with the documented header");
    assert_eq!(
        body, source,
        "derived body must equal the authoritative source"
    );
}

#[test]
fn derivation_header_declares_source_and_immutable_markers() {
    let derived = read_repo_file(DERIVED_PATH);
    assert!(
        derived.starts_with(DERIVED_HEADER),
        "derived copy must begin with the documented derivation header"
    );
    let declared: Vec<&str> = derived
        .lines()
        .filter_map(|l| l.strip_prefix("<!-- DERIVED-FROM: "))
        .map(|l| l.trim_end_matches(" -->"))
        .collect();
    assert_eq!(
        declared,
        vec![SOURCE_PATH],
        "derived copy must declare exactly one authoritative source"
    );
    assert!(
        derived.contains("<!-- PROMPT-CONTRACT: PR-SUM-001, PR-SUM-002 -->"),
        "derived copy must carry the prompt-contract marker"
    );
    assert!(
        derived.contains("<!-- IMMUTABLE: grounding, no_invention, evidence_binding"),
        "derived copy must carry the immutable-obligation marker"
    );
    assert!(
        derived.contains("proof-bound and must survive prompt optimization or editing"),
        "derived copy must keep the proof-bound obligation statement"
    );
}

#[test]
fn authoritative_source_is_not_marked_as_derived() {
    let source = read_repo_file(SOURCE_PATH);
    assert!(
        !source.contains("DERIVED-FROM"),
        "{SOURCE_PATH} is the authoritative source and must not declare itself derived"
    );
    assert!(
        !source.contains("PROMPT-CONTRACT:"),
        "{SOURCE_PATH} must not carry the derived-copy contract header"
    );
}

#[test]
fn executed_summarizer_manifest_binds_the_authoritative_source() {
    let manifest_yaml = read_repo_file(EXECUTED_MANIFEST_PATH);
    let manifest =
        AgentManifest::from_yaml_str(&manifest_yaml).expect("summarizer agent manifest must parse");
    assert_eq!(
        manifest.prompt.system_file, SOURCE_PATH,
        "the executed summarizer prompt must be the authoritative source, not the derived copy"
    );
    assert!(
        manifest.check_locks().is_ok(),
        "optimizer lock must stay closed for the prompt-normative contract"
    );
}

#[test]
fn frozen_eval_baseline_digest_tracks_the_authoritative_source() {
    // The frozen eval fixture names the derived copy as its baseline prompt,
    // but the recorded digest is the digest of the authoritative source body
    // (the exported/executed prompt). Pin both facts so a source edit without
    // re-freezing the eval is a visible failure rather than silent drift.
    let fixture: serde_json::Value = serde_json::from_str(&read_repo_file(FROZEN_EVAL_PATH))
        .expect("frozen prompt-eval fixture must be valid JSON");
    let baseline = fixture
        .get("baseline")
        .expect("frozen fixture must declare a baseline");
    assert_eq!(
        baseline.get("prompt_file").and_then(|v| v.as_str()),
        Some(DERIVED_PATH),
        "frozen baseline must name the derived copy"
    );
    let recorded = baseline
        .get("prompt_sha256")
        .and_then(|v| v.as_str())
        .expect("frozen baseline must record prompt_sha256");
    assert_eq!(
        recorded,
        sha256_hex(&read_repo_file(SOURCE_PATH)),
        "frozen baseline digest must equal the authoritative source digest"
    );
}

#[test]
fn drift_detection_is_fail_closed() {
    let source = read_repo_file(SOURCE_PATH);
    let derived = read_repo_file(DERIVED_PATH);
    assert!(derived_matches_source(&derived, &source));

    // Negative 1: a source edit not mirrored into the derived copy fails.
    let mutated_source = format!("{source}Do not invent.\n");
    assert!(
        !derived_matches_source(&derived, &mutated_source),
        "source drift must be detected"
    );

    // Negative 2: a derived copy that keeps the obligations but weakens them
    // (extra trailing instruction) is still rejected — the check is exact.
    let mutated_derived = format!("{derived}Prefer brevity over grounding.\n");
    assert!(
        !derived_matches_source(&mutated_derived, &source),
        "derived-copy tampering must be detected"
    );

    // Negative 3: dropping the documented header entirely fails.
    assert!(
        !derived_matches_source(&source, &source),
        "a derived copy without the documented header must be rejected"
    );

    // Negative 4: an empty source cannot satisfy the contract.
    assert!(
        !derived_matches_source("", ""),
        "empty inputs must not satisfy the drift predicate"
    );
}
