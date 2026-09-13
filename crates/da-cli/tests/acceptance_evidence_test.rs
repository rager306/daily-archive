use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn runner() -> PathBuf {
    std::env::var_os("ACCEPTANCE_EVIDENCE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("examples/acceptance_evidence")
        })
}
fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn invoke(manifest: &Path, report: &Path) -> std::process::Output {
    Command::new(runner())
        .current_dir(repo())
        .args([
            "--manifest",
            manifest.to_str().unwrap(),
            "--report",
            report.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}
fn relative(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn canonical_replay_is_byte_stable_redacted_and_remote_missing_is_visible() {
    let root = repo();
    let dir = tempdir_in_repo(&root);
    let manifest = dir.join("manifest.yaml");
    fs::copy(root.join("data/parse/acceptance_manifest.yaml"), &manifest).unwrap();
    let first = dir.join("first.json");
    let second = dir.join("second.json");
    let a = invoke(
        Path::new(&relative(&root, &manifest)),
        Path::new(&relative(&root, &first)),
    );
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    let b = invoke(
        Path::new(&relative(&root, &manifest)),
        Path::new(&relative(&root, &second)),
    );
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let report: Value = serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert_eq!(report["schema_version"], "parse-acceptance-evidence.v1");
    assert_eq!(report["verdict"], "PASS");
    assert_eq!(report["classes"]["publisher_acceptance"]["executed_n"], 3);
    assert_eq!(
        report["classes"]["publisher_acceptance"]["bytes_absent_n"],
        4
    );
    assert_eq!(report["classes"]["layout_omnidocbench"]["executed_n"], 3);
    assert_eq!(report["layout"]["pages"], 202);
    assert_eq!(report["redaction"]["raw_pdf"], false);
    assert_eq!(report["redaction"]["raw_tei"], false);
    let text = fs::read_to_string(first).unwrap();
    for forbidden in [
        "https://",
        "http://",
        "/root/",
        "data/sources",
        ".pdf",
        "<TEI",
    ] {
        assert!(!text.contains(forbidden), "forbidden marker {forbidden}");
    }
}

#[test]
fn isolated_manifest_mutations_fail_with_stable_machine_codes() {
    let root = repo();
    let source = fs::read_to_string(root.join("data/parse/acceptance_manifest.yaml")).unwrap();
    let mutations: [(&str, &str, &str); 6] = [
        (
            "missing_bound_bytes",
            "pdf_path: data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf",
            "pdf_path: data/sources/arxiv/cs-cl/2507.19457/source/missing.pdf",
        ),
        (
            "checksum_drift",
            "pdf_sha256: ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
            "pdf_sha256: 0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "same_n_mismatch",
            "proxy_of: scholarly_pdf_plus_java_tei\n      same_n: true",
            "proxy_of: scholarly_pdf_plus_java_tei\n      same_n: false",
        ),
        (
            "role_mismatch",
            "comparison_kind: layout_markdown",
            "comparison_kind: tei_family",
        ),
        (
            "lock_erosion",
            "  import_eligible: false",
            "  import_eligible: true",
        ),
        (
            "silent_skip",
            "      - id: hub-2507.19457\n        state: bound",
            "      - id: hub-2507.19457\n        state: bytes_absent",
        ),
    ];
    for (code, old, new) in mutations {
        assert!(source.contains(old), "mutation anchor absent for {code}");
        let dir = tempdir_in_repo(&root);
        let manifest = dir.join("manifest.yaml");
        let report = dir.join("report.json");
        fs::write(&manifest, source.replacen(old, new, 1)).unwrap();
        let output = invoke(
            Path::new(&relative(&root, &manifest)),
            Path::new(&relative(&root, &report)),
        );
        assert!(
            !output.status.success(),
            "mutation {code} unexpectedly passed"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(code),
            "mutation {code} missing code in stderr: {stderr}"
        );
        assert!(
            !report.exists(),
            "mutation {code} emitted partial green report"
        );
    }
}

fn tempdir_in_repo(root: &Path) -> PathBuf {
    let base = root.join("target/acceptance-evidence-test");
    fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("case-")
        .tempdir_in(&base)
        .unwrap()
        .keep()
}
