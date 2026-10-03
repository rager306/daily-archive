//! S05/T01 (M515-v2mvmc) — hermetic R140/R142 evidence-linkage audit.
//!
//! The reviewers recorded that R140 and R142 "does not explicitly show" a
//! field-level evidence pointer. This audit closes that gap over committed
//! bytes: it re-reads the committed acceptance package, proves every pointer in
//! [`REQUIREMENT_EVIDENCE_R140`] and [`REQUIREMENT_EVIDENCE_R142`] is bound in
//! `package.evidence`, recomputes each pin from the manifest bytes and compares
//! it to the bound `revision_sha256`, and asserts the reviewer-named pointee
//! shapes. It never reads the frozen corpus and fails by name when a pointee
//! field or pin drifts.
//!
//! The audit is deliberately ungated (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it
//! rather than compiling an empty target.

use da_cli::native_acceptance_package::{
    DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL, GrobidRetirementAcceptancePackage,
    REQUIREMENT_EVIDENCE_R140, REQUIREMENT_EVIDENCE_R142, requirement_evidence_pointers,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_prefixed};
use serde_json::Value;
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

fn load_package(root: &Path) -> Result<GrobidRetirementAcceptancePackage, Box<dyn Error>> {
    Ok(GrobidRetirementAcceptancePackage::parse_json_bytes(&read(
        root,
        DEFAULT_ACCEPTANCE_PACKAGE_ARTIFACT_REL,
    )?)?)
}

/// Resolve a dotted pointer against a JSON document, so a renamed pointee key
/// fails here instead of being taken on trust.
fn resolve_pointer<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut current = document;
    for segment in pointer.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Prove a requirement's grouped pointers are bound in the package and that
/// every pin re-derives from the committed manifest bytes. Fails with a message
/// naming the requirement id and pointer.
fn assert_requirement_pins(
    root: &Path,
    package: &GrobidRetirementAcceptancePackage,
    requirement_id: &str,
    pointers: &[(&str, &str)],
) -> Result<(), Box<dyn Error>> {
    let group = requirement_evidence_pointers(requirement_id)
        .ok_or_else(|| format!("{requirement_id} has no declared pointer group"))?;
    assert_eq!(
        group.len(),
        pointers.len(),
        "{requirement_id} pointer group length drifted"
    );
    for (rel, pointer) in pointers {
        let binding: &CarrierBinding = package
            .evidence
            .iter()
            .find(|candidate| candidate.manifest == *rel && candidate.pointer == *pointer)
            .ok_or_else(|| {
                format!(
                    "{requirement_id} pointer `{rel}#{pointer}` is not bound in package.evidence"
                )
            })?;
        let recomputed = sha256_prefixed(&read(root, rel)?);
        assert_eq!(
            binding.revision_sha256, recomputed,
            "{requirement_id} pointer `{rel}#{pointer}` pin does not re-derive from the committed bytes"
        );
    }
    Ok(())
}

#[test]
fn r140_and_r142_field_pointers_are_bound_and_pinned() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let package = load_package(&root)?;

    // The accessor names exactly the two requirements this slice links.
    assert!(
        requirement_evidence_pointers("R140").is_some(),
        "R140 has no declared pointer group"
    );
    assert!(
        requirement_evidence_pointers("R142").is_some(),
        "R142 has no declared pointer group"
    );
    assert!(
        requirement_evidence_pointers("R141").is_none(),
        "R141 must not have a declared pointer group"
    );
    assert!(
        requirement_evidence_pointers("R999").is_none(),
        "an unknown requirement must not resolve a pointer group"
    );
    assert_eq!(
        requirement_evidence_pointers("R140"),
        Some(&REQUIREMENT_EVIDENCE_R140[..])
    );
    assert_eq!(
        requirement_evidence_pointers("R142"),
        Some(&REQUIREMENT_EVIDENCE_R142[..])
    );

    // R140: the typed gaps the frozen corpus carries and the Java-free guard.
    assert_requirement_pins(&root, &package, "R140", &REQUIREMENT_EVIDENCE_R140)?;
    // R142: reward version/digest, conservation, contamination, gap, corpus reward.
    assert_requirement_pins(&root, &package, "R142", &REQUIREMENT_EVIDENCE_R142)?;

    // ── reviewer-named pointee shapes ─────────────────────────────────────
    let native: Value = serde_json::from_slice(&read(
        &root,
        "artifacts/parse_compat/native_corpus_acceptance.json",
    )?)?;
    let gaps = resolve_pointer(&native, "gaps")
        .and_then(Value::as_array)
        .ok_or("R140 native_corpus_acceptance.json#gaps is not a list")?;
    assert!(
        !gaps.is_empty(),
        "R140 native_corpus_acceptance.json#gaps is empty"
    );
    for gap in gaps {
        let kind = gap.get("kind").and_then(Value::as_str).unwrap_or_default();
        let reason = gap
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !kind.trim().is_empty() && !reason.trim().is_empty(),
            "R140 native_corpus_acceptance.json#gaps has an untyped entry: {gap}"
        );
    }

    let slice: Value = serde_json::from_slice(&read(
        &root,
        "artifacts/parse_compat/java_free_vertical_slice.json",
    )?)?;
    let guard = resolve_pointer(&slice, "guard")
        .ok_or("R140 java_free_vertical_slice.json#guard is absent")?;
    assert_eq!(
        guard.get("installed").and_then(Value::as_bool),
        Some(true),
        "R140 java_free_vertical_slice.json#guard.installed is not true"
    );
    assert_eq!(
        guard.get("observed_java_calls").and_then(Value::as_u64),
        Some(0),
        "R140 java_free_vertical_slice.json#guard.observed_java_calls is not 0"
    );

    let cycle: Value = serde_json::from_slice(&read(
        &root,
        "artifacts/parse_learning/candidate_cycle.json",
    )?)?;
    let conservation = resolve_pointer(&cycle, "conservation")
        .ok_or("R142 candidate_cycle.json#conservation is absent")?;
    let before = conservation
        .get("corpus_digest_before")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let after = conservation
        .get("corpus_digest_after")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !before.trim().is_empty() && !after.trim().is_empty(),
        "R142 candidate_cycle.json#conservation lacks corpus_digest_before/after"
    );
    assert!(
        conservation.get("veto").and_then(Value::as_bool).is_some(),
        "R142 candidate_cycle.json#conservation lacks a `veto` flag"
    );
    assert_eq!(
        resolve_pointer(&cycle, "contamination_assessment").and_then(Value::as_str),
        Some("not_detected"),
        "R142 candidate_cycle.json#contamination_assessment is not not_detected"
    );
    assert!(
        resolve_pointer(&cycle, "validation_gap")
            .and_then(Value::as_f64)
            .is_some(),
        "R142 candidate_cycle.json#validation_gap is not a number"
    );

    let corpus: Value = serde_json::from_slice(&read(
        &root,
        "artifacts/parse_learning/learning_corpus.json",
    )?)?;
    let reward = resolve_pointer(&corpus, "reward_definition")
        .ok_or("R142 learning_corpus.json#reward_definition is absent")?;
    assert_eq!(
        reward.get("version").and_then(Value::as_str),
        Some("reward-v1"),
        "R142 learning_corpus.json#reward_definition.version is not reward-v1"
    );
    let corpus_digest = reward
        .get("digest")
        .and_then(Value::as_str)
        .ok_or("R142 learning_corpus.json#reward_definition.digest is absent")?;
    assert_eq!(
        resolve_pointer(&cycle, "reward_definition_sha256").and_then(Value::as_str),
        Some(corpus_digest),
        "R142 candidate_cycle.json#reward_definition_sha256 does not equal \
         learning_corpus.json#reward_definition.digest"
    );

    Ok(())
}
