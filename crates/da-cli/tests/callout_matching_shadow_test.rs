#![cfg(feature = "pdf-layout")]

use da_cli::uat_verdict_lineage::{
    BoundArtifact, CommandIdentity, ParsedAssertion, SourceRevision, VerdictBinding, VerdictClaim,
    VerdictLineage, VerdictStatus,
};
use da_domain::ContentDigest;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::{fs, process::Command};
use tempfile::tempdir;

/// GAR-016: the tracked sources the emitted verdict is executed against. The
/// test itself is included, so a changed assertion or command shape changes the
/// bound source revision instead of silently reusing an old PASS.
const VERDICT_SOURCE_FILES: [&str; 3] = [
    "crates/da-cli/examples/callout_matching_shadow.rs",
    "crates/da-cli/tests/callout_matching_shadow_test.rs",
    "crates/da-cli/src/uat_verdict_lineage.rs",
];

const EXAMPLE_ARGV: [&str; 8] = [
    "run",
    "--offline",
    "--features",
    "pdf-layout",
    "--example",
    "callout_matching_shadow",
    "--",
    "--report",
];

fn example_argv(report: &Path) -> Vec<String> {
    let mut argv = EXAMPLE_ARGV
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect::<Vec<_>>();
    argv.push(report.to_string_lossy().into_owned());
    argv
}

fn run_example(report: &Path) -> (std::process::Output, Vec<String>) {
    let argv = example_argv(report);
    let output = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(&argv)
        .output()
        .expect("run offline example");
    (output, argv)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn input_artifact(repo_root: &Path, relative: &str) -> BoundArtifact {
    let bytes = fs::read(repo_root.join(relative))
        .unwrap_or_else(|error| panic!("verdict input `{relative}`: {error}"));
    BoundArtifact::of_bytes(relative, &bytes).expect("host-free input name")
}

fn source_revision(repo_root: &Path) -> SourceRevision {
    let mut material = String::new();
    for relative in VERDICT_SOURCE_FILES {
        let bytes = fs::read(repo_root.join(relative))
            .unwrap_or_else(|error| panic!("source revision input `{relative}`: {error}"));
        material.push_str(relative);
        material.push('\n');
        material.push_str(ContentDigest::of_bytes(&bytes).as_str());
        material.push('\n');
    }
    SourceRevision::new(&format!(
        "files:sha256:{}",
        ContentDigest::of_text(&material).as_str()
    ))
    .expect("host-free source revision")
}

fn resolve<'a>(value: &'a Value, dotted_path: &str) -> &'a Value {
    dotted_path.split('.').fold(value, |current, segment| {
        current
            .get(segment)
            .unwrap_or_else(|| panic!("report is missing `{dotted_path}` at `{segment}`"))
    })
}

fn observed_assertion(value: &Value, dotted_path: &str, expected: Value) -> ParsedAssertion {
    assert_eq!(
        resolve(value, dotted_path),
        &expected,
        "report assertion drifted at `{dotted_path}`"
    );
    ParsedAssertion::new(dotted_path, expected).expect("host-free assertion path")
}

/// The invocation identity with the ephemeral report path and the checkout root
/// replaced by stable placeholders, so the verdict is reproducible across
/// checkouts while still changing when the command or any argument changes.
fn invocation_argv(argv: &[String], repo_root: &Path, report: &Path) -> Vec<String> {
    let report_text = report.to_string_lossy().into_owned();
    let root_prefix = format!("{}/", repo_root.to_string_lossy());
    argv.iter()
        .map(|argument| {
            if argument.as_str() == report_text.as_str() {
                "<report>".to_owned()
            } else {
                argument.replace(&root_prefix, "")
            }
        })
        .collect()
}

/// Emit the bounded verdict to disk, reload it and recompute its status from the
/// observed binding. A tampered file fails to load; a drifted binding fails to
/// verify; only an unchanged execution binding yields the execution PASS.
fn emit_and_reload(dir: &Path, file_name: &str, binding: VerdictBinding) -> VerdictStatus {
    let lineage = VerdictLineage::new(binding.clone()).expect("bounded verdict");
    let path = dir.join(file_name);
    fs::write(&path, lineage.to_json_pretty().expect("serialize verdict")).expect("write verdict");
    let durable = fs::read_to_string(&path).expect("durable verdict file");
    assert!(
        !durable.contains(&dir.to_string_lossy().to_string()),
        "the durable verdict must not carry the report host path"
    );
    let restored = VerdictLineage::from_json(&durable)
        .expect("the durable verdict reloads with an intact binding");
    println!(
        "{}",
        serde_json::to_string(&restored).expect("emit bounded verdict")
    );
    restored.verify(&binding)
}

fn assert_redacted(value: &Value) {
    const KEYS: &[&str] = &[
        "raw_payload",
        "raw_tei",
        "raw_pdf",
        "raw_text",
        "images",
        "physical_paths",
        "host",
        "executable",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if KEYS.contains(&key.as_str()) {
                    assert_eq!(child, &Value::Bool(false), "redaction lock: {key}");
                }
                assert_redacted(child);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_redacted),
        Value::String(text) => {
            for forbidden in [
                "<TEI",
                "raw_text",
                "Reference one",
                "/root/",
                "file://",
                ".pdf",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "forbidden report value: {forbidden}"
                );
            }
        }
        _ => {}
    }
}

#[test]
fn callout_matching_shadow_is_composed_terminal_and_quarantined() {
    let dir = tempdir().expect("temporary output");
    let report = dir.path().join("callout_matching_shadow.json");
    let (output, argv) = run_example(&report);
    assert!(
        output.status.success(),
        "example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(&report).expect("report");
    let value: Value = serde_json::from_str(&text).expect("JSON report");
    assert_eq!(value["schema_version"], "callout-matching-shadow.v1");
    assert!(
        value["source_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["data_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["oracle_sha256"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(
        value["reference_population"],
        serde_json::json!({"documents":3,"pages":202,"lane_pages":606,"document_lane_outcomes":9,"oracle_cells":54,"identity":"reviewed_three_document_layout_reference","role":"identity_only"})
    );
    assert_eq!(value["executed_population"]["corpus_execution"], false);
    assert_eq!(value["evaluated_population"]["status"], "not_evaluated");
    assert_eq!(value["outcomes"]["quality_claims"], serde_json::json!([]));
    assert_eq!(
        value["accounting"]["admitted"],
        value["accounting"]["conserved"]
    );
    assert_eq!(value["accounting"]["loss"], 0);
    assert_eq!(value["matching"]["admitted"], 4);
    assert_eq!(value["matching"]["matched"], 3);
    assert_eq!(value["matching"]["conserved"], 4);
    assert_eq!(value["matching"]["loss"], 0);
    assert_eq!(value["matching"]["unresolved"]["sourcenotestablished"], 1);
    assert_eq!(value["replay_equal"], true);
    assert_eq!(value["worker_one_equal"], true);
    assert_eq!(
        value["policy_locks"]["production_authority"],
        "java-grobid-http@0.9.0-crf"
    );
    assert_eq!(value["policy_locks"]["import_eligible"], false);
    assert_eq!(value["policy_locks"]["graph_write"], "forbidden");
    assert_eq!(value["policy_locks"]["production_route"], "forbidden");
    assert_eq!(value["policy_locks"]["auto_promote"], "forbidden");
    assert_eq!(value["policy_locks"]["decision_point"], 6);
    assert_eq!(
        value["policy_locks"]["independently_selectable"],
        serde_json::json!(["header", "references"])
    );
    assert_redacted(&value);

    // GAR-016: the PASS is emitted as a bounded verdict bound to the exact
    // command, inputs, source revision, produced artifact and parsed
    // assertions, not asserted from the report fields alone. The report's
    // published input digests are re-derived from the tracked files on disk, so
    // the verdict and the report cannot disagree about what was read.
    let repo_root = repo_root();
    let source_input = input_artifact(
        &repo_root,
        "artifacts/parse_compat/java_grobid_callout_matching_source.json",
    );
    let data_input = input_artifact(&repo_root, "data/parse/callout_matching_semantics.yaml");
    let oracle_input = input_artifact(
        &repo_root,
        "crates/da-parse/tests/fixtures/callout_matching_oracle.yaml",
    );
    for (field, artifact) in [
        ("source_sha256", &source_input),
        ("data_sha256", &data_input),
        ("oracle_sha256", &oracle_input),
    ] {
        assert_eq!(
            value[field].as_str().expect("published input digest"),
            format!("sha256:{}", artifact.sha256.as_str()),
            "report {field} must be the digest of the tracked input it compiled in"
        );
    }
    let observed = VerdictBinding::new(
        VerdictClaim::Execution,
        CommandIdentity::new(
            "da-cli:shadow:callout_matching",
            "cargo",
            &invocation_argv(&argv, &repo_root, &report),
        )
        .expect("host-free command identity"),
        vec![source_input, data_input, oracle_input],
        source_revision(&repo_root),
        vec![
            BoundArtifact::of_bytes("callout_matching_shadow.json", text.as_bytes())
                .expect("host-free artifact name"),
        ],
        vec![
            observed_assertion(
                &value,
                "schema_version",
                Value::String("callout-matching-shadow.v1".to_owned()),
            ),
            observed_assertion(&value, "policy_locks.import_eligible", Value::Bool(false)),
            observed_assertion(
                &value,
                "policy_locks.graph_write",
                Value::String("forbidden".to_owned()),
            ),
            observed_assertion(
                &value,
                "policy_locks.production_route",
                Value::String("forbidden".to_owned()),
            ),
            observed_assertion(
                &value,
                "policy_locks.auto_promote",
                Value::String("forbidden".to_owned()),
            ),
            observed_assertion(
                &value,
                "reference_population.role",
                Value::String("identity_only".to_owned()),
            ),
            observed_assertion(
                &value,
                "evaluated_population.status",
                Value::String("not_evaluated".to_owned()),
            ),
            observed_assertion(&value, "outcomes.quality_claims", serde_json::json!([])),
            observed_assertion(&value, "replay_equal", Value::Bool(true)),
            observed_assertion(&value, "worker_one_equal", Value::Bool(true)),
        ],
    )
    .expect("non-empty verdict binding");
    let status = emit_and_reload(dir.path(), "callout_matching_shadow.verdict.json", observed);
    assert!(status.is_execution_pass(), "{status:?}");
    assert_eq!(status.claim(), Some(VerdictClaim::Execution));
}
