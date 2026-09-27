//! S08/T02 — the frozen Java reference is preflight-gated, byte-pinned and
//! verifiable (M511-j7wz2f).
//!
//! The runner under test is `crates/da-cli/examples/external_holdout_capture.rs`:
//! `--preflight` resolves every declared input, `--capture` froze the reference
//! (already committed under `crates/da-parse/tests/fixtures/holdout/`) and
//! `--verify` re-checks the frozen bytes against both their provenance and their
//! DATA pins.
//!
//! Why this test exists rather than prose: the slice's central promise is that
//! *unavailable inputs block the claim by name*. That promise is only real if
//! the "inputs unavailable" branch is executable, so this test drives the
//! hermetic `--self-check` predicate matrix (each declared failure kind must be
//! rejected by its own label, with positive controls proving the guard does not
//! simply refuse everything), the read-only `--preflight` input checks, and the
//! `--verify` integrity path against deliberately corrupted, missing and
//! pin-drifted copies of the committed reference.
//!
//! Everything here is offline on purpose: the measurement path reads only
//! frozen bytes. `--verify` re-probes the live release by default, so the
//! integrity cases run `--offline` and stay hermetic — the live re-probe is
//! covered by the hermetic `live_release_drift` / `endpoint_drift` self-check
//! cases plus the recorded run of `--verify` against the live service.
//!
//! `cargo test --test NAME` does not build examples, so this test rebuilds the
//! runner through cargo with the same target directory before every invocation
//! (cargo is incremental, so a fresh target is a no-op, and it releases the
//! build lock while tests run — this cannot deadlock). Rebuilding rather than
//! trusting an existing binary matters: a stale example silently re-tests the
//! previous source.

use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Committed frozen-reference directory (the DATA pins point here).
const FIXTURE_DIR: &str = "crates/da-parse/tests/fixtures/holdout";

/// Read the acceptance manifest (DATA lane + frozen pins).
fn acceptance_manifest() -> Yaml {
    serde_yaml::from_str(
        &fs::read_to_string(repo_root().join("data/parse/acceptance_manifest.yaml"))
            .expect("read acceptance manifest"),
    )
    .expect("acceptance manifest is valid YAML")
}

/// The `external_holdout_contract.release_probe` lane.
fn release_probe_lane(manifest: &Yaml) -> &Yaml {
    manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .expect("release_probe lane")
}

/// The `article_id`s of one lane partition list.
fn lane_ids(lane: &Yaml, key: &str) -> BTreeSet<String> {
    lane.get(key)
        .and_then(Yaml::as_sequence)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get("article_id").and_then(Yaml::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The full frozen holdout population, from the holdout manifest DATA.
fn holdout_ids() -> BTreeSet<String> {
    let manifest: Yaml = serde_yaml::from_str(
        &fs::read_to_string(repo_root().join("data/parse/holdout_manifests.yaml"))
            .expect("read holdout manifest"),
    )
    .expect("holdout manifest is valid YAML");
    manifest
        .get("articles")
        .and_then(Yaml::as_sequence)
        .expect("holdout articles")
        .iter()
        .filter_map(|row| row.get("article_id").and_then(Yaml::as_str))
        .map(str::to_string)
        .collect()
}

/// How many declared holdout ids are also production-materialized
/// (`data/processed`). Computed, never hardcoded: growing the population must
/// not leave a stale literal behind.
fn production_overlap(ids: &BTreeSet<String>) -> usize {
    ids.iter()
        .filter(|id| repo_root().join("data/processed").join(id).is_dir())
        .count()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Path to the runner, rebuilt once per test process.
///
/// The build is unconditional rather than "only when the file is missing": a
/// stale example binary silently re-tests the previous source, which is exactly
/// how a green suite can describe code that no longer exists. `cargo build` is
/// incremental, so a fresh target is a no-op, and `OnceLock` keeps the parallel
/// test threads from contending on the cargo build lock.
fn example_binary() -> PathBuf {
    static BINARY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BINARY
        .get_or_init(|| {
            let binary = repo_root().join("target/debug/examples/external_holdout_capture");
            let build = Command::new("cargo")
                .args([
                    "build",
                    "-q",
                    "-p",
                    "da-cli",
                    "--example",
                    "external_holdout_capture",
                ])
                .current_dir(repo_root())
                .output()
                .expect("build the external holdout capture example");
            assert!(
                build.status.success(),
                "example build failed: {}",
                String::from_utf8_lossy(&build.stderr)
            );
            assert!(
                binary.is_file(),
                "the example binary must exist after build"
            );
            binary
        })
        .clone()
}

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(example_binary());
    command.args(args).current_dir(repo_root());
    command.output().expect("run the capture example")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// `PREFIX <key> -> <value>` lines, collected as a map.
fn labelled_lines(stdout: &str, prefix: &str) -> BTreeMap<String, String> {
    stdout
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(prefix)?;
            let (key, value) = rest.split_once(" -> ")?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

/// Copy the committed reference directory into a scratch directory so a case
/// can mutate it without touching the committed bytes.
fn copy_reference() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("scratch directory");
    let source = repo_root().join(FIXTURE_DIR);
    for entry in fs::read_dir(&source).expect("read committed reference directory") {
        let entry = entry.expect("directory entry");
        let target = temp.path().join(entry.file_name());
        fs::copy(entry.path(), &target).expect("copy fixture");
    }
    temp
}

/// File name -> (length, sha256) for every entry in a directory. Used to prove
/// a run wrote nothing.
fn directory_fingerprint(dir: &Path) -> BTreeMap<String, (u64, String)> {
    let mut fingerprint = BTreeMap::new();
    for entry in fs::read_dir(dir).expect("read directory") {
        let entry = entry.expect("directory entry");
        let bytes = fs::read(entry.path()).expect("read directory entry");
        fingerprint.insert(
            entry.file_name().to_string_lossy().to_string(),
            (bytes.len() as u64, sha256_prefixed(&bytes)),
        );
    }
    fingerprint
}

fn provenance_path(dir: &Path) -> PathBuf {
    dir.join("provenance.yaml")
}

fn read_provenance(dir: &Path) -> Yaml {
    let text = fs::read_to_string(provenance_path(dir)).expect("read provenance");
    serde_yaml::from_str(&text).expect("provenance is valid YAML")
}

fn write_provenance(dir: &Path, value: &Yaml) {
    let rendered = serde_yaml::to_string(value).expect("render provenance");
    fs::write(provenance_path(dir), rendered).expect("write provenance");
}

/// Set `documents[<article_id>].sha256` in a provenance document.
fn set_provenance_document_sha(provenance: &mut Yaml, article_id: &str, sha256: &str) {
    let documents = provenance
        .get_mut("documents")
        .and_then(Yaml::as_sequence_mut)
        .expect("provenance.documents");
    let row = documents
        .iter_mut()
        .find(|row| {
            row.get("article_id")
                .and_then(Yaml::as_str)
                .is_some_and(|value| value == article_id)
        })
        .unwrap_or_else(|| panic!("provenance row `{article_id}`"));
    let row = row.as_mapping_mut().expect("provenance row mapping");
    row.insert(
        Yaml::String("sha256".to_string()),
        Yaml::String(sha256.to_string()),
    );
}

#[test]
fn self_check_rejects_every_declared_failure_kind_by_name() {
    let output = run(&["--self-check"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "--self-check must pass on the synthetic predicate matrix:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let cases = labelled_lines(&stdout, "SELFCHECK ");

    // Every declared failure kind is rejected by its own label.
    let expected_failures: [(&str, &str); 14] = [
        ("article_absent", "bytes_absent:synthetic-ref"),
        ("article_sha_drift", "bytes_drift:synthetic-ref"),
        ("article_size_drift", "bytes_size_drift:synthetic-ref"),
        ("endpoint_unreachable", "java_endpoint_unavailable"),
        ("release_undeclared", "java_release_undeclared"),
        ("release_mismatch", "java_version_mismatch:0.8.1"),
        (
            "release_pin_drift",
            "java_release_pin_drift:synthetic/repository:sha256:otherdigest",
        ),
        ("release_pin_missing", "java_release_pin_missing"),
        (
            "external_bytes_present",
            "external_dataset_bytes_present:synthetic-external",
        ),
        ("reference_absent", "reference_absent:synthetic-ref"),
        ("reference_drift", "reference_drift:synthetic-ref"),
        ("data_pin_drift", "data_pin_drift:synthetic-ref"),
        ("live_release_drift", "live_release_drift:0.9.2"),
        ("endpoint_drift", "endpoint_drift:http://127.0.0.1:9999"),
    ];
    for (case, expected) in expected_failures {
        assert_eq!(
            cases.get(case).map(String::as_str),
            Some(expected),
            "self-check case `{case}` must produce `{expected}`; saw {:?}",
            cases.get(case)
        );
    }

    // Positive controls: the guard must admit correct inputs too, otherwise a
    // refusal proves nothing.
    let expected_admissions: [&str; 6] = [
        "article_present",
        "endpoint_reachable",
        "release_match",
        "release_pin_match",
        "external_absent",
        "reference_present",
    ];
    for case in expected_admissions {
        assert_eq!(
            cases.get(case).map(String::as_str),
            Some("ok"),
            "self-check positive control `{case}` must be admitted; saw {:?}",
            cases.get(case)
        );
    }

    assert_eq!(
        cases.get("summary").map(String::as_str),
        Some("ok"),
        "the synthetic matrix must have zero survivors"
    );
}

#[test]
fn preflight_checks_the_committed_inputs_and_writes_nothing() {
    let fixture_dir = repo_root().join(FIXTURE_DIR);
    let before = directory_fingerprint(&fixture_dir);

    // The population is read from DATA, so the test grows with the manifest
    // instead of pinning a fixed literal count. Preflight is read-only by construction
    // and probes whatever endpoint the environment provides, so this asserts
    // the hermetic half (the declared PDF bytes, the DATA release pin, and the
    // no-write property) and accepts either outcome for the live half.
    let output = run(&["--preflight"]);
    let stdout = stdout_of(&output);
    let lines = labelled_lines(&stdout, "PREFLIGHT ");

    let frozen = holdout_ids();
    assert!(
        !frozen.is_empty(),
        "the frozen population must be declared in DATA, not an empty list"
    );
    for article_id in &frozen {
        assert_eq!(
            lines
                .get(&format!("article:{article_id}"))
                .map(String::as_str),
            Some("ok"),
            "the declared holdout PDF `{article_id}` must match its sha256 pin:\n{stdout}"
        );
    }
    assert_eq!(
        lines.get("java_release_pin").map(String::as_str),
        Some("ok"),
        "the declared release must still agree with the repo release pin:\n{stdout}"
    );
    assert_eq!(
        lines.get("external_dataset_bytes").map(String::as_str),
        Some("ok"),
        "the external dataset rows must stay declared bytes_absent:\n{stdout}"
    );
    assert!(
        stdout.contains("PREFLIGHT summary -> "),
        "preflight must print a summary line:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "CENSUS production_materialization_overlap -> {}/{}",
            production_overlap(&frozen),
            frozen.len()
        )),
        "the production overlap census is measured, never asserted:\n{stdout}"
    );

    assert_eq!(
        directory_fingerprint(&fixture_dir),
        before,
        "preflight must write nothing"
    );
}

#[test]
fn verify_is_green_on_the_committed_reference_offline() {
    let output = run(&["--verify", "--offline"]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "--verify must pass on the committed reference:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let checks = labelled_lines(&stdout, "VERIFY ");
    // `--verify` re-hashes the measurable subset AND the typed exceptions, so a
    // document excluded from measurement is still byte-verified rather than
    // skipped. Read the expectation from the lane partition DATA.
    let manifest = acceptance_manifest();
    let lane = release_probe_lane(&manifest);
    let mut verified = lane_ids(lane, "reference_tei");
    verified.extend(lane_ids(lane, "reference_exceptions"));
    assert_eq!(
        verified,
        holdout_ids(),
        "the measurable subset plus the typed exceptions must cover the frozen population"
    );
    for article_id in &verified {
        assert_eq!(
            checks
                .get(&format!("reference:{article_id}"))
                .map(String::as_str),
            Some("ok"),
            "the committed reference `{article_id}` must match its provenance and DATA pin:\n{stdout}"
        );
    }
    assert_eq!(checks.get("live_release").map(String::as_str), Some("ok"));
    assert_eq!(
        checks.get("source_manifest").map(String::as_str),
        Some("ok")
    );
    assert_eq!(
        checks.get("provenance_release").map(String::as_str),
        Some("ok")
    );
    assert_eq!(checks.get("summary").map(String::as_str), Some("ok"));
}

#[test]
fn verify_rejects_a_corrupted_fixture_by_name_without_writing() {
    let temp = copy_reference();
    let target = temp.path().join("1203.2295.tei.xml");
    let mut bytes = fs::read(&target).expect("read fixture");
    // Flip a byte in the document body, keeping the XML well formed enough that
    // only the byte identity can catch it.
    let position = bytes.len() / 2;
    bytes[position] ^= 0x01;
    fs::write(&target, &bytes).expect("write corrupted fixture");
    let before = directory_fingerprint(temp.path());

    let output = run(&[
        "--verify",
        "--offline",
        "--in",
        temp.path().to_str().expect("temp path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        !output.status.success(),
        "a corrupted fixture must fail verification:\n{stdout}"
    );
    let checks = labelled_lines(&stdout, "VERIFY ");
    assert_eq!(
        checks.get("reference:1203.2295").map(String::as_str),
        Some("reference_drift:1203.2295"),
        "the corrupted fixture must be refused by name:\n{stdout}"
    );
    // The other four documents stay green: the failure is local, not a refusal
    // of everything.
    assert_eq!(
        checks.get("reference:2401.17167").map(String::as_str),
        Some("ok"),
        "an untouched document must stay admitted:\n{stdout}"
    );
    assert!(
        stdout.contains("VERIFY summary -> blocked:reference_drift:1203.2295"),
        "the summary must name the exact blocker:\n{stdout}"
    );
    assert_eq!(
        directory_fingerprint(temp.path()),
        before,
        "verify must write nothing, even on failure"
    );
}

#[test]
fn verify_rejects_a_missing_fixture_by_name() {
    let temp = copy_reference();
    fs::remove_file(temp.path().join("2411.07228.tei.xml")).expect("remove fixture");

    let output = run(&[
        "--verify",
        "--offline",
        "--in",
        temp.path().to_str().expect("temp path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        !output.status.success(),
        "a missing fixture must fail verification:\n{stdout}"
    );
    let checks = labelled_lines(&stdout, "VERIFY ");
    assert_eq!(
        checks.get("reference:2411.07228").map(String::as_str),
        Some("reference_absent:2411.07228"),
        "the missing fixture must be refused by name:\n{stdout}"
    );
}

#[test]
fn verify_enforces_the_data_pin_not_only_the_provenance() {
    let temp = copy_reference();
    let target = temp.path().join("2505.16270.tei.xml");
    let mut bytes = fs::read(&target).expect("read fixture");
    bytes.extend_from_slice(b"<!-- substituted -->");
    fs::write(&target, &bytes).expect("write substituted fixture");

    // Make the local provenance agree with the substituted bytes: now only the
    // DATA pin can catch the substitution, which is exactly the property the
    // frozen lane exists for.
    let mut provenance = read_provenance(temp.path());
    set_provenance_document_sha(&mut provenance, "2505.16270", &sha256_prefixed(&bytes));
    write_provenance(temp.path(), &provenance);

    let output = run(&[
        "--verify",
        "--offline",
        "--in",
        temp.path().to_str().expect("temp path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        !output.status.success(),
        "a DATA-pin drift must fail verification:\n{stdout}"
    );
    let checks = labelled_lines(&stdout, "VERIFY ");
    assert_eq!(
        checks.get("reference:2505.16270").map(String::as_str),
        Some("data_pin_drift:2505.16270"),
        "the DATA pin, not just the provenance, must bind the frozen bytes:\n{stdout}"
    );
}

#[test]
fn verify_fails_closed_without_a_provenance_record() {
    let temp = copy_reference();
    fs::remove_file(provenance_path(temp.path())).expect("remove provenance");

    let output = run(&[
        "--verify",
        "--offline",
        "--in",
        temp.path().to_str().expect("temp path"),
    ]);
    assert!(
        !output.status.success(),
        "an unrecorded reference must fail verification"
    );
    let combined = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        combined.contains("provenance_absent"),
        "the failure must name the missing provenance record:\n{combined}"
    );
}

#[test]
fn frozen_reference_matches_its_declared_pins_on_disk() {
    // An independent reading of the committed artefacts: the DATA pins, the
    // provenance record and the files on disk must all agree for BOTH the
    // measurable subset and the typed exceptions, and the two halves must
    // partition the declared frozen population exactly (no overlap, no gaps, no
    // document read as a measured zero).
    let manifest = acceptance_manifest();
    let lane = release_probe_lane(&manifest);
    let measurable = lane_ids(lane, "reference_tei");
    let exceptions = lane_ids(lane, "reference_exceptions");
    let frozen = holdout_ids();
    assert_eq!(
        measurable.len() + exceptions.len(),
        frozen.len(),
        "measurable + exceptions must cover the frozen population"
    );
    assert!(
        measurable.is_disjoint(&exceptions),
        "the two partition halves must not overlap"
    );
    let mut union = measurable.clone();
    union.extend(exceptions.iter().cloned());
    assert_eq!(
        union, frozen,
        "the partition must cover exactly the declared frozen documents"
    );

    let provenance = read_provenance(&repo_root().join(FIXTURE_DIR));
    let provenance_hashes: BTreeSet<String> = provenance
        .get("documents")
        .and_then(Yaml::as_sequence)
        .expect("provenance documents")
        .iter()
        .filter_map(|row| row.get("sha256").and_then(Yaml::as_str))
        .map(str::to_string)
        .collect();

    for key in ["reference_tei", "reference_exceptions"] {
        let rows = lane
            .get(key)
            .and_then(Yaml::as_sequence)
            .unwrap_or_else(|| panic!("lane partition `{key}`"));
        for row in rows {
            let article_id = row
                .get("article_id")
                .and_then(Yaml::as_str)
                .expect("article id");
            let path = row.get("path").and_then(Yaml::as_str).expect("pin path");
            let pin = row
                .get("sha256")
                .and_then(Yaml::as_str)
                .expect("pin sha256");
            let bytes = fs::read(repo_root().join(path))
                .unwrap_or_else(|error| panic!("frozen reference `{path}` unreadable: {error}"));
            assert_eq!(
                sha256_prefixed(&bytes),
                pin,
                "the committed fixture `{article_id}` must match its DATA pin"
            );
            assert_eq!(
                bytes.len() as u64,
                row.get("size_bytes").and_then(Yaml::as_u64).unwrap_or(0),
                "the committed fixture `{article_id}` must match its declared size"
            );
            assert!(
                provenance_hashes.contains(pin),
                "the provenance record must carry the same pin for `{article_id}`"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// `--rederive`: the declared selection rule is executable (S03/T02, D453)
//
// The census is only a derivation if the declared population can be reproduced
// from the corpus by the declared rule. The corpus under `data/sources/arxiv`
// is gitignored (`data/**/*.pdf`), so the corpus walk cannot be a hermetic test
// subject; the rule is therefore covered against purpose-built temp corpora and
// driven through the real `--rederive` mode with `--corpus`/`--manifest`
// overrides. The production corpus run is operator evidence, not a test asset.
// ---------------------------------------------------------------------------

/// Write a corpus asset. The bytes are irrelevant to identity selection (only
/// the path shape matters), so the fixture stays tiny.
fn touch_pdf(path: &Path) {
    fs::create_dir_all(path.parent().expect("asset parent")).expect("create corpus directory");
    fs::write(path, b"%PDF-1.4\n% census fixture\n").expect("write corpus asset");
}

/// Minimal `selection` manifest for hermetic `--rederive` runs. The corpus root
/// and the gold glob are absolute so a temp corpus never depends on the repo
/// checkout and gold exclusion is testable without touching `data/gold_standard`.
fn write_selection_manifest(
    path: &Path,
    corpus_root: &Path,
    gold_glob: &Path,
    excluded: &[&str],
    take_first: Option<u64>,
    articles: &[&str],
) {
    let mut selection = serde_yaml::Mapping::new();
    selection.insert(
        Yaml::String("rule_version".into()),
        Yaml::String("external-holdout-selection.v1".into()),
    );
    selection.insert(
        Yaml::String("corpus_root".into()),
        Yaml::String(corpus_root.to_string_lossy().into_owned()),
    );
    selection.insert(
        Yaml::String("ordering".into()),
        Yaml::String("ascending_article_id".into()),
    );
    if let Some(limit) = take_first {
        selection.insert(
            Yaml::String("take_first".into()),
            Yaml::Number(limit.into()),
        );
    }
    let mut exclude = serde_yaml::Mapping::new();
    exclude.insert(
        Yaml::String("gold_standard_glob".into()),
        Yaml::String(gold_glob.to_string_lossy().into_owned()),
    );
    exclude.insert(
        Yaml::String("identities".into()),
        Yaml::Sequence(
            excluded
                .iter()
                .map(|id| Yaml::String((*id).to_string()))
                .collect(),
        ),
    );
    selection.insert(Yaml::String("exclude".into()), Yaml::Mapping(exclude));

    let mut manifest = serde_yaml::Mapping::new();
    manifest.insert(
        Yaml::String("schema_version".into()),
        Yaml::String("external-holdout-manifest.v1".into()),
    );
    manifest.insert(Yaml::String("selection".into()), Yaml::Mapping(selection));
    manifest.insert(
        Yaml::String("articles".into()),
        Yaml::Sequence(
            articles
                .iter()
                .map(|id| {
                    let mut row = serde_yaml::Mapping::new();
                    row.insert(
                        Yaml::String("article_id".into()),
                        Yaml::String((*id).to_string()),
                    );
                    Yaml::Mapping(row)
                })
                .collect(),
        ),
    );
    let rendered = serde_yaml::to_string(&Yaml::Mapping(manifest)).expect("render manifest");
    fs::write(path, rendered).expect("write manifest");
}

/// `REDERIVE article -> <id> <path>` rows in printed order.
fn rederive_articles(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("REDERIVE article -> ")?;
            let (article_id, path) = rest.split_once(' ')?;
            Some((article_id.to_string(), path.to_string()))
        })
        .collect()
}

/// The winning asset path printed for one derived identity.
fn rederive_path_for<'a>(rows: &'a [(String, String)], article_id: &str) -> Option<&'a str> {
    rows.iter()
        .find(|(id, _)| id == article_id)
        .map(|(_, path)| path.as_str())
}

/// Recursive (relative path -> sha256) fingerprint, used to prove a read-only
/// mode wrote nothing anywhere in a tree (the flat `directory_fingerprint`
/// helper cannot walk the nested corpus).
fn tree_fingerprint(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).expect("read tree directory") {
            let entry = entry.expect("tree entry");
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .expect("relative path")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    sha256_prefixed(&fs::read(&path).expect("read tree file")),
                );
            }
        }
    }
    let mut fingerprint = BTreeMap::new();
    walk(root, root, &mut fingerprint);
    fingerprint
}

#[test]
fn rederive_collapses_versions_and_orders_ascending() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-cl/2501.00001/source/2501.00001.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00002v1/source/2501.00002v1.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00002v2/source/2501.00002v2.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00003/source/2501.00003.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2501.00001", "2501.00002", "2501.00003"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "the derivation must match the declaration:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let counters = labelled_lines(&stdout, "REDERIVE ");
    assert_eq!(counters.get("assets").map(String::as_str), Some("4"));
    assert_eq!(counters.get("identities").map(String::as_str), Some("3"));
    assert_eq!(counters.get("declared").map(String::as_str), Some("3"));
    assert_eq!(counters.get("derived").map(String::as_str), Some("3"));
    assert_eq!(counters.get("summary").map(String::as_str), Some("ok"));

    let rows = rederive_articles(&stdout);
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["2501.00001", "2501.00002", "2501.00003"],
        "derived order must be ascending article_id:\n{stdout}"
    );
    // `v1`/`v2` collapse into one identity and the lowest revision wins.
    let winner = rederive_path_for(&rows, "2501.00002").expect("winner for 2501.00002");
    assert!(
        winner.ends_with("2501.00002v1.pdf"),
        "the lowest revision must win, saw `{winner}`"
    );
}

#[test]
fn rederive_inherits_identity_from_the_nearest_matching_ancestor() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    // 2512.24601-like: `original.pdf` is the ONLY asset. The literal "parent
    // directory" rule would resolve to `source` and lose the real article.
    touch_pdf(&corpus.join("cs-ar/2512.24601/source/original.pdf"));
    // A sibling `<id>.pdf` exists; `original.pdf` still inherits the identity.
    touch_pdf(&corpus.join("cs-ai/2503.00003/source/original.pdf"));
    touch_pdf(&corpus.join("cs-ai/2503.00003/source/2503.00003.pdf"));
    // A versioned ancestor directory contributes the identity and its revision.
    touch_pdf(&corpus.join("cs-cv/2504.00004v2/source/original.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2503.00003", "2504.00004", "2512.24601"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "literal `original.pdf` handling must not break the derivation:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let rows = rederive_articles(&stdout);
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["2503.00003", "2504.00004", "2512.24601"],
        "the inherited identities must be derived:\n{stdout}"
    );
    assert!(
        !ids.contains(&"source"),
        "the literal parent-directory rule must not leak a `source` identity"
    );
    let lone = rederive_path_for(&rows, "2512.24601").expect("lone original.pdf");
    assert!(
        lone.ends_with("2512.24601/source/original.pdf"),
        "the sole `original.pdf` must keep the article identity, saw `{lone}`"
    );
    let sibling = rederive_path_for(&rows, "2503.00003").expect("sibling asset");
    assert!(
        sibling.ends_with("2503.00003.pdf"),
        "equal revisions break on the smallest path, saw `{sibling}`"
    );
    let inherited = rederive_path_for(&rows, "2504.00004").expect("versioned ancestor");
    assert!(
        inherited.ends_with("2504.00004v2/source/original.pdf"),
        "a versioned ancestor directory must supply the identity, saw `{inherited}`"
    );
}

#[test]
fn rederive_tie_break_prefers_lowest_revision_then_smallest_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    // Revision beats path: the v3 asset sorts before the versionless one, so a
    // path-only tie-break would pick the wrong revision.
    touch_pdf(&corpus.join("cs-ne/2505.00005/source/2505.00005v3.pdf"));
    touch_pdf(&corpus.join("cs-ne/w/2505.00005/source/2505.00005.pdf"));
    // Equal revisions: the lexicographically smaller path wins.
    touch_pdf(&corpus.join("cs-cv/2504.00004/b/2504.00004.pdf"));
    touch_pdf(&corpus.join("cs-cv/2504.00004/a/2504.00004.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2504.00004", "2505.00005"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "the tie-break must be deterministic:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let rows = rederive_articles(&stdout);
    let by_revision = rederive_path_for(&rows, "2505.00005").expect("winner");
    assert!(
        by_revision.contains("/w/"),
        "the v3 asset must lose to the versionless one despite sorting earlier, saw `{by_revision}`"
    );
    let by_path = rederive_path_for(&rows, "2504.00004").expect("winner");
    assert!(
        by_path.contains("/a/"),
        "equal revisions must break on the lexicographically smaller path, saw `{by_path}`"
    );
}

#[test]
fn rederive_excludes_gold_identities_and_declared_identities() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-lg/2506.00006/source/2506.00006.pdf"));
    touch_pdf(&corpus.join("cs-lg/2507.00007/source/2507.00007.pdf"));
    touch_pdf(&corpus.join("cs-lg/2508.00008/source/2508.00008.pdf"));
    fs::write(gold.join("2506.00006.gold.json"), b"{}").expect("gold fixture");
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &["2507.00007"],
        None,
        &["2508.00008"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "gold and declared exclusions must be applied:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let counters = labelled_lines(&stdout, "REDERIVE ");
    assert_eq!(counters.get("identities").map(String::as_str), Some("3"));
    assert_eq!(counters.get("excluded_gold").map(String::as_str), Some("1"));
    assert_eq!(counters.get("derived").map(String::as_str), Some("1"));
    let rows = rederive_articles(&stdout);
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["2508.00008"],
        "the gold identity and the declared exclusion must be removed"
    );
}

#[test]
fn rederive_honours_the_declared_take_first_cap() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-cl/2501.00001/source/2501.00001.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00002/source/2501.00002.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00003/source/2501.00003.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        Some(2),
        &["2501.00001", "2501.00002"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "a declared partial population must be reproduced:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let counters = labelled_lines(&stdout, "REDERIVE ");
    assert_eq!(counters.get("derived").map(String::as_str), Some("2"));
    let rows = rederive_articles(&stdout);
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        ["2501.00001", "2501.00002"],
        "the cap must be applied after the ascending ordering"
    );
}

#[test]
fn rederive_reports_selection_rule_drift_by_name_without_writing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-cl/2501.00001/source/2501.00001.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00002/source/2501.00002.pdf"));
    touch_pdf(&corpus.join("cs-cl/2501.00003/source/2501.00003.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    // One declared id is absent from the corpus and one does not exist at all.
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2501.00001", "2501.00002", "2509.99999"],
    );
    let corpus_before = tree_fingerprint(&corpus);
    let manifest_before = fs::read(&manifest).expect("read manifest");

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let combined = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        !output.status.success(),
        "a drifted declaration must fail:\n{combined}"
    );
    assert!(
        combined.contains("selection_rule_drift"),
        "the drift must be named:\n{combined}"
    );
    // `missing` is declared-but-not-derived (the bogus declaration); `extra`
    // is derived-but-not-declared (the real, undeclared corpus article).
    assert!(
        combined.contains("missing=[2509.99999]"),
        "the declared-but-absent id must be named first:\n{combined}"
    );
    assert!(
        combined.contains("extra=[2501.00003]"),
        "the derived-but-undeclared id must be named first:\n{combined}"
    );
    assert_eq!(
        tree_fingerprint(&corpus),
        corpus_before,
        "--rederive must not write to the corpus"
    );
    assert_eq!(
        fs::read(&manifest).expect("read manifest"),
        manifest_before,
        "--rederive must not rewrite the manifest"
    );
}

#[test]
fn rederive_corpus_override_wins_over_the_declared_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("real-corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-cl/2501.00001/source/2501.00001.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    // Declare a corpus root that does not exist: only the override can make the
    // run succeed, which proves the override is honoured.
    write_selection_manifest(
        &manifest,
        &temp.path().join("absent-corpus"),
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2501.00001"],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
        "--corpus",
        corpus.to_str().expect("corpus path"),
    ]);
    let stdout = stdout_of(&output);
    assert!(
        output.status.success(),
        "the `--corpus` override must be used instead of the declared root:\n{stdout}\n{}",
        stderr_of(&output)
    );
    let rows = rederive_articles(&stdout);
    let ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["2501.00001"]);
}

#[test]
fn rederive_fails_closed_on_an_unsupported_rule_version() {
    let temp = tempfile::tempdir().expect("tempdir");
    let corpus = temp.path().join("corpus");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    touch_pdf(&corpus.join("cs-cl/2501.00001/source/2501.00001.pdf"));
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &corpus,
        &gold.join("*.gold.json"),
        &[],
        None,
        &["2501.00001"],
    );
    // Declare a rule the runner does not implement: it must refuse rather than
    // silently apply the v1 algorithm to a v2 declaration.
    let text = fs::read_to_string(&manifest).expect("read manifest");
    fs::write(
        &manifest,
        text.replace(
            "external-holdout-selection.v1",
            "external-holdout-selection.v2",
        ),
    )
    .expect("write manifest");

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let combined = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        !output.status.success(),
        "an unknown rule version must fail closed:\n{combined}"
    );
    assert!(
        combined.contains("selection_rule_version_unsupported:external-holdout-selection.v2"),
        "the unsupported version must be named:\n{combined}"
    );
}

#[test]
fn rederive_fails_closed_when_the_corpus_root_is_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let gold = temp.path().join("gold");
    fs::create_dir_all(&gold).expect("gold dir");
    let manifest = temp.path().join("holdout_manifests.yaml");
    write_selection_manifest(
        &manifest,
        &temp.path().join("absent-corpus"),
        &gold.join("*.gold.json"),
        &[],
        None,
        &[],
    );

    let output = run(&[
        "--rederive",
        "--manifest",
        manifest.to_str().expect("manifest path"),
    ]);
    let combined = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        !output.status.success(),
        "an absent corpus root must fail closed:\n{combined}"
    );
    assert!(
        combined.contains("selection_corpus_unreadable"),
        "the missing corpus must be named:\n{combined}"
    );
}
