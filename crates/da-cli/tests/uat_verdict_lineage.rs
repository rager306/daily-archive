//! GAR-016 contract target: `cargo test -p da-cli --test uat_verdict_lineage`.
//!
//! The shadow-test unit tests exercise the lineage type's internals; this
//! standalone target pins the *contract* through the public API only, using the
//! real default feature set (no `pdf-layout`, no shadow runner). It proves that
//!
//! * a changed command, a changed input digest or a changed artifact digest
//!   each make the verdict fail instead of passing,
//! * unchanged bound inputs keep the execution PASS,
//! * drift in the parsed assertions or the source revision is also fatal,
//! * prose or a description cannot mark a verdict PASS, and
//! * an execution verdict does not imply a parity or quality claim.
//!
//! The tracked input is read from disk, but the test never mutates it: a
//! simulated change appends bytes in memory and recomputes the observed digest,
//! which is exactly the state a re-run would observe after the file changed.
//!
//! Test/library provenance only. `import_eligible` stays false and no graph is
//! written.

use da_cli::uat_verdict_lineage::{
    BoundArtifact, CommandIdentity, LineageError, ParsedAssertion, SourceRevision, VerdictBinding,
    VerdictClaim, VerdictFailure, VerdictLineage, VerdictStatus,
};
use da_domain::ContentDigest;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// A git-tracked input the verdict binds. Read-only: the contract simulates a
/// change rather than editing the working tree.
const TRACKED_INPUT: &str = "data/parse/layout_bakeoff.yaml";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn tracked_bytes(relative: &str) -> Vec<u8> {
    fs::read(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("tracked verdict input `{relative}`: {error}"))
}

/// The canonical produced report. Synthesized in memory so the artifact digest
/// test does not depend on a generated file.
fn artifact_bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": "uat-verdict-lineage.v1",
        "policy": { "import_eligible": false, "auto_promote": "forbidden" },
        "outcome": "pass",
    }))
    .expect("artifact fixture serializes")
}

fn command() -> CommandIdentity {
    CommandIdentity::new(
        "da-cli:test:uat_verdict_lineage",
        "uat_verdict_lineage",
        &["--report".to_owned(), "<report>".to_owned()],
    )
    .expect("host-free command identity")
}

fn bound(input: &[u8], artifact: &[u8]) -> VerdictBinding {
    VerdictBinding::new(
        VerdictClaim::Execution,
        command(),
        vec![BoundArtifact::of_bytes(TRACKED_INPUT, input).expect("host-free input name")],
        SourceRevision::new("files:sha256:uat-verdict-lineage-contract")
            .expect("host-free source revision"),
        vec![
            BoundArtifact::of_bytes("uat-verdict-lineage-contract.json", artifact)
                .expect("host-free artifact name"),
        ],
        vec![
            ParsedAssertion::new("policy.import_eligible", serde_json::json!(false))
                .expect("host-free assertion path"),
            ParsedAssertion::new(
                "schema_version",
                serde_json::json!("uat-verdict-lineage.v1"),
            )
            .expect("host-free assertion path"),
        ],
    )
    .expect("non-empty verdict binding")
}

fn assert_failed_with(status: &VerdictStatus, predicate: impl Fn(&VerdictFailure) -> bool) {
    match status {
        VerdictStatus::ExecutionPass => panic!("expected a failed verdict, got {status:?}"),
        VerdictStatus::Failed { reasons } => {
            assert!(
                reasons.iter().any(predicate),
                "expected a matching failure reason, got {reasons:?}"
            );
        }
    }
}

#[test]
fn unchanged_tracked_input_and_artifact_keep_the_execution_pass() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding.clone()).expect("bounded verdict");

    assert_eq!(lineage.verify(&binding), VerdictStatus::ExecutionPass);
    assert!(lineage.verify(&binding).is_execution_pass());
    assert!(lineage.verify_binding().is_ok());
}

#[test]
fn changed_command_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding).expect("bounded verdict");

    // One extra argument is enough: the argv digest covers the whole command.
    let observed_command = CommandIdentity::new(
        "da-cli:test:uat_verdict_lineage",
        "uat_verdict_lineage",
        &[
            "--report".to_owned(),
            "<report>".to_owned(),
            "--strict".to_owned(),
        ],
    )
    .expect("host-free command identity");
    assert_ne!(
        command().argv_digest,
        observed_command.argv_digest,
        "the argv digest must change when the command changes"
    );

    let mut observed = bound(&input, &artifact);
    observed.command = observed_command;
    let status = lineage.verify(&observed);
    assert!(
        !status.is_execution_pass(),
        "a changed command must not pass"
    );
    assert_failed_with(&status, |reason| {
        matches!(reason, VerdictFailure::CommandMismatch { .. })
    });
}

#[test]
fn changed_input_digest_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding).expect("bounded verdict");

    // Simulate "the tracked input changed after the verdict was recorded": the
    // observed bytes differ, so the recomputed digest differs. The file is not
    // touched, so the pass recorded above stays exactly as it was.
    let mut mutated_input = input.clone();
    mutated_input.extend_from_slice(b"\n# mutated after the verdict was recorded\n");
    assert_ne!(
        ContentDigest::of_bytes(&input).as_str(),
        ContentDigest::of_bytes(&mutated_input).as_str(),
        "mutating the input must change its digest"
    );

    let status = lineage.verify(&bound(&mutated_input, &artifact));
    assert!(!status.is_execution_pass(), "a changed input must not pass");
    assert_failed_with(&status, |reason| {
        matches!(
            reason,
            VerdictFailure::ArtifactDigestMismatch { role, name, .. }
                if role == "input" && name == TRACKED_INPUT
        )
    });
}

#[test]
fn changed_artifact_digest_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding).expect("bounded verdict");

    let mut mutated_artifact = artifact.clone();
    mutated_artifact.extend_from_slice(b"\n");
    let status = lineage.verify(&bound(&input, &mutated_artifact));
    assert!(
        !status.is_execution_pass(),
        "a changed produced artifact must not pass"
    );
    assert_failed_with(&status, |reason| {
        matches!(
            reason,
            VerdictFailure::ArtifactDigestMismatch { role, .. } if role == "artifact"
        )
    });
}

#[test]
fn changed_parsed_assertion_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding).expect("bounded verdict");

    // Re-parsing the same artifact under a different expected value is a drift,
    // not a PASS: the assertion is part of the binding, not commentary about it.
    let mut observed = bound(&input, &artifact);
    observed.assertions[0] =
        ParsedAssertion::new("policy.import_eligible", serde_json::json!(true))
            .expect("host-free assertion path");
    let status = lineage.verify(&observed);
    assert!(!status.is_execution_pass());
    assert_failed_with(&status, |reason| {
        matches!(
            reason,
            VerdictFailure::AssertionMismatch { path, .. } if path == "policy.import_eligible"
        )
    });

    // A missing parsed assertion is equally fatal.
    let mut dropped = bound(&input, &artifact);
    dropped.assertions.remove(1);
    assert_failed_with(
        &lineage.verify(&dropped),
        |reason| matches!(reason, VerdictFailure::AssertionMissing { path } if path == "schema_version"),
    );
}

#[test]
fn removed_or_added_input_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding.clone()).expect("bounded verdict");

    // A bound input disappears.
    let mut replaced = binding.clone();
    replaced.inputs[0] =
        BoundArtifact::of_bytes("data/parse/other.yaml", &input).expect("host-free input name");
    assert_failed_with(&lineage.verify(&replaced), |reason| {
        matches!(
            reason,
            VerdictFailure::ArtifactMissing { role, name }
                if role == "input" && name == TRACKED_INPUT
        )
    });

    // An unexpected input appears (the command grew a dependency).
    let mut extra = binding;
    extra
        .inputs
        .push(BoundArtifact::of_bytes("data/parse/document_segmentation.yaml", b"extra").unwrap());
    assert_failed_with(
        &lineage.verify(&extra),
        |reason| matches!(reason, VerdictFailure::ArtifactUnexpected { role, .. } if role == "input"),
    );
}

#[test]
fn changed_source_revision_fails_verdict() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let binding = bound(&input, &artifact);
    let lineage = VerdictLineage::new(binding).expect("bounded verdict");

    let mut observed = bound(&input, &artifact);
    observed.source_revision =
        SourceRevision::new("files:sha256:different-revision").expect("host-free revision");
    let status = lineage.verify(&observed);
    assert!(!status.is_execution_pass());
    assert_failed_with(&status, |reason| {
        matches!(reason, VerdictFailure::SourceRevisionMismatch { .. })
    });
}

#[test]
fn prose_or_description_cannot_mark_a_verdict_pass() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();
    let lineage = VerdictLineage::new(bound(&input, &artifact)).expect("bounded verdict");

    // Prose attached at either level is an unknown field, not a status. The
    // document is refused at load instead of being read as PASS.
    let mut value: Value =
        serde_json::from_str(&lineage.to_json_pretty().expect("serialize verdict")).unwrap();
    value["description"] = Value::String("PASS: reviewed and approved".to_owned());
    value["prose"] = Value::String("the report looked correct".to_owned());
    value["binding"]["description"] = Value::String("execution PASS".to_owned());
    let error = VerdictLineage::from_json(&value.to_string()).unwrap_err();
    assert!(
        matches!(error, LineageError::SerializationFailed { .. }),
        "prose must not be loadable into a verdict: {error:?}"
    );

    // A prose status is not a verdict variant, and a bare `status` is not read
    // as one either.
    assert!(serde_json::from_str::<VerdictStatus>(r#"{"verdict":"pass"}"#).is_err());
    assert!(serde_json::from_str::<VerdictStatus>(r#"{"status":"PASS"}"#).is_err());
    assert!(
        serde_json::from_str::<VerdictStatus>(r#"{"verdict":"failed","description":"PASS"}"#)
            .is_err()
    );

    // The only thing that loads is the explicitly scoped execution PASS.
    let status: VerdictStatus =
        serde_json::from_str(r#"{"verdict":"execution_pass"}"#).expect("scoped verdict status");
    assert_eq!(status, VerdictStatus::ExecutionPass);
    assert_eq!(status.claim(), Some(VerdictClaim::Execution));
}

#[test]
fn execution_verdict_does_not_imply_parity_or_quality() {
    let input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();

    // An execution PASS reports only the execution claim. It is not a quality
    // or parity claim and cannot be read as one.
    let lineage = VerdictLineage::new(bound(&input, &artifact)).expect("bounded verdict");
    let status = lineage.verify(&bound(&input, &artifact));
    assert!(status.is_execution_pass());
    assert_eq!(status.claim(), Some(VerdictClaim::Execution));
    assert_ne!(status.claim(), Some(VerdictClaim::Quality));
    assert_ne!(status.claim(), Some(VerdictClaim::Parity));

    // The claim vocabulary contains no "pass": a quality or parity claim is
    // declared but never substantiated by an execution verdict.
    assert_eq!(VerdictClaim::Execution.as_str(), "execution");
    assert_eq!(VerdictClaim::Quality.as_str(), "quality");
    assert_eq!(VerdictClaim::Parity.as_str(), "parity");

    for claim in [VerdictClaim::Quality, VerdictClaim::Parity] {
        let mut declared = bound(&input, &artifact);
        declared.claim = claim;
        let declared_lineage =
            VerdictLineage::new(declared.clone()).expect("bounded verdict over a declared claim");
        let declared_status = declared_lineage.verify(&declared);
        assert!(
            !declared_status.is_execution_pass(),
            "{claim:?} must not PASS"
        );
        assert_eq!(declared_status.claim(), None);
        assert_failed_with(
            &declared_status,
            |reason| matches!(reason, VerdictFailure::UnsupportedClaim { claim: observed } if *observed == claim),
        );
    }
}

#[test]
fn durable_verdict_goes_stale_when_a_bound_input_changes() {
    let dir = tempdir().expect("temporary verdict directory");
    let repository_input = tracked_bytes(TRACKED_INPUT);
    let artifact = artifact_bytes();

    // A working copy of the tracked input, so the demonstration can change the
    // bytes on disk without editing the repository.
    let input_path = dir.path().join("layout_bakeoff.yaml");
    fs::write(&input_path, &repository_input).expect("write input copy");
    let initial = fs::read(&input_path).expect("read input copy");
    assert_eq!(initial, repository_input, "the copy starts unchanged");

    let lineage = VerdictLineage::new(bound(&initial, &artifact)).expect("bounded verdict");
    let verdict_path = dir.path().join("uat-verdict-lineage.verdict.json");
    fs::write(
        &verdict_path,
        lineage.to_json_pretty().expect("serialize verdict"),
    )
    .expect("write verdict");

    let durable_text = fs::read_to_string(&verdict_path).expect("durable verdict file");
    assert!(
        !durable_text.contains(&dir.path().to_string_lossy().to_string()),
        "the durable verdict must not carry the host path"
    );
    let restored = VerdictLineage::from_json(&durable_text).expect("intact binding reloads");

    // Re-run before any change: the durable verdict is still a valid execution
    // observation.
    let unchanged = fs::read(&input_path).expect("read input copy");
    assert!(
        restored
            .verify(&bound(&unchanged, &artifact))
            .is_execution_pass()
    );

    // The bound input changes on disk: a re-run reads a different digest and the
    // durable verdict must fail, not report PASS.
    let mut changed = unchanged.clone();
    changed.extend_from_slice(b"\n# changed after the verdict was recorded\n");
    fs::write(&input_path, &changed).expect("mutate input copy");
    let observed = fs::read(&input_path).expect("re-read input copy");
    assert_ne!(
        ContentDigest::of_bytes(&unchanged),
        ContentDigest::of_bytes(&observed),
        "mutating the file must change its digest"
    );
    let status = restored.verify(&bound(&observed, &artifact));
    assert!(matches!(status, VerdictStatus::Failed { .. }), "{status:?}");

    // Editing the stored evidence without re-running does not mint a PASS; the
    // tampered document fails to load at all.
    let mut value: Value = serde_json::from_str(&durable_text).unwrap();
    value["binding"]["inputs"][0]["sha256"] =
        Value::String(ContentDigest::of_bytes(&observed).as_str().to_owned());
    assert!(matches!(
        VerdictLineage::from_json(&value.to_string()),
        Err(LineageError::BindingDigestMismatch { .. })
    ));
}
