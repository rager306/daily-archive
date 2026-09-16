#![cfg(feature = "learning-evaluation")]

//! S07/T04 — the redacted policy-variant ablation artifact is reproducible,
//! candidate-only and closure-bounded (M507-4fk6m6).
//!
//! The example (`examples/parse_policy_ablation.rs`) is run twice from two
//! different temporary working directories and the two byte streams must be
//! identical; the committed artifact must equal a fresh run. The JSON is then
//! audited for the promotion locks, the exact same-n denominators, the observed
//! closure sets and redaction.
//!
//! `cargo test --test NAME` does not build examples, so when the runner is
//! missing this test builds it through cargo with the same target directory
//! (cargo releases the build lock while tests run, so this cannot deadlock).

use serde_json::Value;
use sha2::Digest;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// The only parsed-stage CLI actions the closed policy may ever select.
const CLOSED_ACTIONS: [&str; 2] = ["parse-pdf", "parse-pdf-toc"];
/// The only grammar routes the closed `GrammarRoute` enum carries.
const CLOSED_ROUTES: [&str; 3] = ["page_outline", "xy_cut", "grobid_structural"];
/// Committed redacted artifact under test.
const ARTIFACT_PATH: &str = "artifacts/parse_learning/policy_variant_ablation.json";
/// sha256 of the committed policy registry: the closure DATA is not rewritten.
const COMMITTED_REGISTRY_SHA256: &str =
    "300b1c2107cfcf683a17ced994535667ef49b416e54759f54bc9d61fcc376f14";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must resolve")
}

fn cases_path() -> PathBuf {
    repo_root().join("crates/da-cli/tests/fixtures/policy_ablation_cases.json")
}

/// `<target>/debug` for the test binary that is currently running, so a custom
/// `CARGO_TARGET_DIR` is honoured without guessing.
fn debug_dir() -> PathBuf {
    std::env::current_exe()
        .expect("test binary path")
        .parent()
        .expect("deps directory")
        .parent()
        .expect("profile directory")
        .to_path_buf()
}

fn discover_example(profile_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> =
        std::fs::read_dir(profile_dir.join("examples"))
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                name.starts_with("parse_policy_ablation-")
                    && !name.ends_with(".d")
                    && path.is_file()
                    && std::fs::metadata(path)
                        .map(|meta| {
                            std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111
                                != 0
                        })
                        .unwrap_or(false)
            })
            .map(|path| {
                let modified = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                (modified, path)
            })
            .collect();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates.pop().map(|(_, path)| path)
}

/// Locate the example binary, building it once when the test target selection
/// did not build examples.
fn example_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("PARSE_POLICY_ABLATION_BIN") {
        return PathBuf::from(path);
    }
    let profile_dir = debug_dir();
    if let Some(path) = discover_example(&profile_dir) {
        return path;
    }
    let target_dir = profile_dir.parent().expect("target directory");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(repo_root())
        .args([
            "build",
            "-p",
            "da-cli",
            "--features",
            "learning-evaluation",
            "--example",
            "parse_policy_ablation",
            "--target-dir",
        ])
        .arg(target_dir)
        .status()
        .expect("cargo must be runnable from the test");
    assert!(status.success(), "building the ablation example failed");
    discover_example(&profile_dir).expect("the example binary must exist after a successful build")
}

/// Run the example in `cwd`, writing its report to `report`, and return the
/// report bytes.
fn run_example(binary: &Path, cwd: &Path, report: &Path) -> Vec<u8> {
    let root = repo_root();
    let output = Command::new(binary)
        .current_dir(cwd)
        .arg("--cases")
        .arg(root.join("crates/da-cli/tests/fixtures/policy_ablation_cases.json"))
        .arg("--report")
        .arg(report)
        .arg("--baseline-wat")
        .arg(root.join("data/policies/fixtures/grammar_route.wat"))
        .arg("--candidate-wat")
        .arg(root.join("data/policies/fixtures/grammar_route_always_page_outline.wat"))
        .arg("--registry")
        .arg(root.join("data/policies/registry.yaml"))
        .output()
        .expect("the ablation example must run");
    assert!(
        output.status.success(),
        "the ablation example failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read(report).expect("the example must write its report")
}

/// Two runs in two different temporary CWDs must be byte-identical.
fn fresh_artifact_bytes() -> Vec<u8> {
    let binary = example_binary();
    let first = tempdir().expect("first temp dir");
    let second = tempdir().expect("second temp dir");
    let first_bytes = run_example(&binary, first.path(), &first.path().join("report.json"));
    let second_bytes = run_example(&binary, second.path(), &second.path().join("report.json"));
    assert_eq!(
        first_bytes, second_bytes,
        "the artifact must be byte-identical across working directories"
    );
    first_bytes
}

fn frozen_case_count() -> usize {
    let json = std::fs::read_to_string(cases_path()).expect("frozen case DATA must be readable");
    da_cli::policy_ablation_reference::parse_cases(&json)
        .expect("frozen case DATA must validate")
        .len()
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("`{key}` entries must be strings"))
                .to_string()
        })
        .collect()
}

#[test]
fn ablation_artifact_is_byte_stable_candidate_only_and_redacted() {
    let bytes = fresh_artifact_bytes();
    let committed =
        std::fs::read(repo_root().join(ARTIFACT_PATH)).expect("the committed artifact must exist");
    assert_eq!(
        bytes, committed,
        "the committed artifact must equal a fresh deterministic run"
    );

    let text = String::from_utf8(bytes.clone()).expect("the artifact is UTF-8");
    for marker in ["/root/", "http", ".pdf", "data/", "artifacts/", "file://"] {
        assert!(
            !text.contains(marker),
            "redaction breach: `{marker}` leaked into the artifact"
        );
    }

    let value: Value = serde_json::from_slice(&bytes).expect("the artifact is JSON");
    let cases = frozen_case_count() as u64;
    assert_eq!(value["baseline_n"].as_u64(), Some(cases));
    assert_eq!(value["candidate_n"].as_u64(), Some(cases));
    assert_eq!(value["declared_matches"].as_u64(), Some(cases));

    let report = &value["report"];
    assert_eq!(report["schema_version"], "parse-policy-ablation.v1");
    assert_eq!(report["n"].as_u64(), Some(cases));
    assert_eq!(report["baseline_n"], Value::Null);
    assert_eq!(report["promotion_state"], "candidate");
    assert_eq!(report["auto_promote"], "forbidden");
    assert_eq!(report["deployed"], serde_json::json!(false));
    assert_eq!(report["import_eligible"], serde_json::json!(false));
    assert_eq!(report["producer_independent"], serde_json::json!(true));
    assert_eq!(report["conservation_veto"], serde_json::json!(false));
    assert_eq!(report["critic_failures"].as_u64(), Some(0));
    assert_eq!(report["n"].as_u64(), Some(cases));
    assert!(
        report["report_digest"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64),
        "report_digest must be a sha256"
    );
    assert!(
        value["artifact_digest"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64),
        "artifact_digest must be a sha256"
    );

    // Policy -> dispatch: the variant is a different artifact from the baseline
    // and it shifts the non-fallback action rate, while BOTH arms keep perfect
    // agreement with the frozen declared actions.
    assert_ne!(
        report["candidate_policy_hash"],
        report["baseline_policy_hash"]
    );
    assert_eq!(report["baseline_metric_one"].as_f64(), Some(1.0));
    assert_eq!(report["candidate_metric_one"].as_f64(), Some(1.0));
    let baseline_two = report["baseline_metric_two"].as_f64().unwrap_or_default();
    let candidate_two = report["candidate_metric_two"].as_f64().unwrap_or_default();
    assert!(
        candidate_two > baseline_two,
        "the variant must change the observed dispatch mix ({candidate_two} vs {baseline_two})"
    );
    assert_eq!(report["quality_qualification"], "qualified");

    // Closure by OBSERVATION, not by declaration.
    let routes = strings(&value, "observed_routes");
    assert!(!routes.is_empty(), "the artifact must observe real routes");
    for route in &routes {
        assert!(
            CLOSED_ROUTES.contains(&route.as_str()),
            "route `{route}` escaped the closed set"
        );
    }
    let actions = strings(&value, "observed_actions");
    assert!(
        !actions.is_empty(),
        "the artifact must observe real actions"
    );
    for action in &actions {
        assert!(
            CLOSED_ACTIONS.contains(&action.as_str()),
            "action `{action}` escaped the closed set"
        );
    }

    // The executed policy bytes are the declared variant, and exactly one
    // artifact decided every dispatch.
    let hashes = strings(&value, "observed_policy_hashes");
    assert_eq!(hashes.len(), 1, "one variant artifact must decide the arm");
    assert_eq!(
        hashes[0],
        report["candidate_policy_hash"].as_str().unwrap_or_default()
    );

    // The pinned DATA registry is unchanged, read back from the committed file.
    let registry = std::fs::read(repo_root().join("data/policies/registry.yaml"))
        .expect("committed registry must be readable");
    let registry_sha = hex::encode(sha2::Sha256::digest(&registry));
    assert_eq!(
        value["registry_digest"].as_str(),
        Some(COMMITTED_REGISTRY_SHA256),
        "the artifact must pin the committed registry digest"
    );
    assert_eq!(
        registry_sha, COMMITTED_REGISTRY_SHA256,
        "the committed registry must still hash to the pinned digest"
    );
}

/// Negative control at the binary boundary: case DATA outside the closed
/// contract must make the example fail closed and write no report at all.
#[test]
fn example_refuses_case_data_outside_the_closed_contract() {
    let binary = example_binary();
    let dir = tempdir().expect("temp dir");
    let bad_cases = dir.path().join("bad-cases.json");

    for (name, cases) in [
        (
            "unsafe-paper-id",
            serde_json::json!({
                "schema_version": "parse-policy-ablation-cases.v1",
                "cases": [
                    {"case_id": "t1", "split": "train", "paper_id": "../../etc/passwd",
                     "features": {"pages": 10, "bookmarks": 0, "quality": 0.9}, "toc_cache": false,
                     "expect": {"baseline": "parse-pdf", "candidate": "parse-pdf"}},
                    {"case_id": "v1", "split": "validation", "paper_id": "5070.20002",
                     "features": {"pages": 80, "bookmarks": 4, "quality": 0.9}, "toc_cache": true,
                     "expect": {"baseline": "parse-pdf-toc", "candidate": "parse-pdf-toc"}}
                ]
            }),
        ),
        (
            "open-action-set",
            serde_json::json!({
                "schema_version": "parse-policy-ablation-cases.v1",
                "cases": [
                    {"case_id": "t1", "split": "train", "paper_id": "5070.20003",
                     "features": {"pages": 10, "bookmarks": 0, "quality": 0.9}, "toc_cache": false,
                     "expect": {"baseline": "parse-pdf", "candidate": "native-fulltext-compose"}},
                    {"case_id": "v1", "split": "validation", "paper_id": "5070.20004",
                     "features": {"pages": 80, "bookmarks": 4, "quality": 0.9}, "toc_cache": true,
                     "expect": {"baseline": "parse-pdf-toc", "candidate": "parse-pdf-toc"}}
                ]
            }),
        ),
    ] {
        std::fs::write(&bad_cases, serde_json::to_vec(&cases).unwrap()).unwrap();
        let report = dir.path().join(format!("{name}.json"));
        let output = Command::new(&binary)
            .current_dir(dir.path())
            .arg("--cases")
            .arg(&bad_cases)
            .arg("--report")
            .arg(&report)
            .output()
            .expect("the ablation example must run");
        assert!(
            !output.status.success(),
            "`{name}` case DATA must be refused, got success"
        );
        assert!(
            !report.exists(),
            "a refused run must not leave a report behind ({name})"
        );
    }
}
