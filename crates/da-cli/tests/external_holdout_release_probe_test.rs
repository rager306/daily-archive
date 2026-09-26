#![cfg(feature = "pdf-layout")]

//! M511-j7wz2f / S08 / T04 — the runtime artifact of the frozen external holdout
//! probe is frozen, non-vacuous, reproducible and redacted (H20 / GAR-015).
//!
//! The structural, independent oracle for this probe is the dev-only
//! `da-parse` test `external_holdout_probe_semantics.rs` (S08/T03) and its DATA
//! carrier `data/parse/acceptance_manifest.yaml#external_holdout_contract.release_probe`
//! (S08/T01/T02).  This test owns the *runtime* half: the example
//! `examples/external_holdout_release_probe.rs` re-measures the same frozen
//! sha256-pinned TEI (the measurable subset of the frozen population; the
//! remainder is the lane's named typed exceptions), fails FATAL on
//! drift/absence/non-UTF-8, and emits
//! `artifacts/parse_compat/external_holdout_release_probe.json`.
//!
//! The artifact is the same-n surface the governor and H20 need, so the audit
//! below pins the identity (one distinct sha256 per frozen document, the probe digest recomputed
//! from the source manifest by the declared canonicalization handles), the
//! GAR-006 population split (a distinguishable reference/executed/evaluated
//! population with no legacy `denominators`/`corpus` block), the measured
//! per-document profile S08/T03 froze, the counts-only aggregate, the margin
//! verdicts recomputed from the DATA thresholds (a re-fitted verdict is a
//! failure), the catalog-overlap census as a measurement, the policy locks and
//! the redaction.  Denominator erosion, identity collapse, a fabricated band, a
//! spliced latency source, a lifted external band and a leaked payload are all
//! rejected by the same audit, which the negative test proves one mutation at a
//! time.
//!
//! `cargo test --test NAME` does not build examples, so when the runner is
//! missing this test builds it through cargo with the same target directory
//! (cargo releases the build lock while tests run, so this cannot deadlock).

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

/// The committed runtime artifact under test.
const ARTIFACT: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
const SCHEMA_VERSION: &str = "external-holdout-release-probe.v1";
/// The committed recorded measurement tables: a projection of `ARTIFACT`, read
/// as DATA rather than compiled as Rust literals.  At census scale the tables are
/// ~17-19k lines of generated Rust across two test targets (D451), so the recorded
/// numbers live in one content-addressed file instead.
const FROZEN_TABLES_REL: &str = "crates/da-parse/tests/fixtures/holdout/frozen_tables.json";
const FROZEN_TABLES_SCHEMA: &str = "external-holdout-frozen-tables.v1";
/// sha256 of the committed recorded tables.  A recorded number cannot change
/// without this literal changing with it: the tables are content-addressed by it.
const FROZEN_TABLES_SHA256: &str =
    "21b63c52739e56b856f52fc071b21f3495f2de1be5e38aa7a421f73382c034f6";
const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const SOURCE_MANIFEST_REL: &str = "data/parse/holdout_manifests.yaml";
const PROVENANCE_REL: &str = "crates/da-parse/tests/fixtures/holdout/provenance.yaml";
const LATENCY_SOURCE_REF: &str =
    "crates/da-parse/tests/fixtures/holdout/provenance.yaml#documents[].latency_ms";
const EXECUTION_EVIDENCE: &str = "tracked_offline_fixture:external_holdout_release_probe_reference";

/// The declared probe population.  DATA is the only carrier of the population
/// size: the source manifest's article list.  No literal governs it, and the
/// frozen per-document table below must keep covering exactly this population.
fn declared_population() -> usize {
    load_yaml(SOURCE_MANIFEST_REL)["articles"]
        .as_sequence()
        .expect("the source manifest must declare the article list")
        .len()
}

/// One recorded per-document row, read from the frozen tables:
/// `(document_id, identity_sha256, source_population, admitted, agreement_micros,
/// uncertainty_low_micros, uncertainty_high_micros, uncertainty_width_micros,
/// latency_ms)`.
type FrozenDocument = (String, String, u64, u64, u64, u64, u64, u64, u64);

fn frozen_tables_path() -> PathBuf {
    repo_root().join(FROZEN_TABLES_REL)
}

fn frozen_tables_bytes() -> Vec<u8> {
    let path = frozen_tables_path();
    fs::read(&path).unwrap_or_else(|error| panic!("FATAL cannot read {}: {error}", path.display()))
}

/// The recorded tables are content-addressed by `FROZEN_TABLES_SHA256`: a file
/// whose bytes do not hash to the pin is a substituted measurement, never a
/// silently newer one.
fn check_frozen_tables_pin(bytes: &[u8]) -> Result<(), String> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest != FROZEN_TABLES_SHA256 {
        return Err(format!(
            "frozen_tables_sha256_mismatch: pinned {FROZEN_TABLES_SHA256}, recorded {digest}"
        ));
    }
    Ok(())
}

fn frozen_tables() -> Value {
    let bytes = frozen_tables_bytes();
    check_frozen_tables_pin(&bytes).unwrap_or_else(|error| panic!("FATAL {error}"));
    let value: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("FATAL the frozen tables are not valid JSON: {error}"));
    assert_eq!(
        value["schema_version"].as_str(),
        Some(FROZEN_TABLES_SCHEMA),
        "the frozen tables must declare the pinned schema"
    );
    value
}

/// The recorded per-document rows.
fn frozen_documents() -> Vec<FrozenDocument> {
    frozen_tables()["documents"]
        .as_array()
        .expect("the frozen tables declare a documents list")
        .iter()
        .map(|document| {
            (
                document["article_id"]
                    .as_str()
                    .expect("article_id")
                    .to_string(),
                document["sha256"].as_str().expect("sha256").to_string(),
                document["source_population"]
                    .as_u64()
                    .expect("source_population"),
                document["admitted"].as_u64().expect("admitted"),
                document["agreement_micros"]
                    .as_u64()
                    .expect("agreement_micros"),
                document["uncertainty_low_micros"]
                    .as_u64()
                    .expect("uncertainty_low_micros"),
                document["uncertainty_high_micros"]
                    .as_u64()
                    .expect("uncertainty_high_micros"),
                document["uncertainty_width_micros"]
                    .as_u64()
                    .expect("uncertainty_width_micros"),
                document["latency_ms"].as_u64().expect("latency_ms"),
            )
        })
        .collect()
}

/// The recorded named admission narrowings: `(document_id, class,
/// source_population, admitted)`.  Every class whose admitted band is smaller
/// than its real carrier population must appear here, and nothing else.
fn frozen_narrowings() -> Vec<(String, String, u64, u64)> {
    frozen_tables()["documents"]
        .as_array()
        .expect("the frozen tables declare a documents list")
        .iter()
        .flat_map(|document| {
            let id = document["article_id"]
                .as_str()
                .expect("article_id")
                .to_string();
            document["narrowings"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(move |narrowing| {
                    (
                        id.clone(),
                        narrowing["class"].as_str().expect("class").to_string(),
                        narrowing["source_population"]
                            .as_u64()
                            .expect("source_population"),
                        narrowing["admitted"].as_u64().expect("admitted"),
                    )
                })
        })
        .collect()
}

/// The recorded counts-only aggregate: `(source_population, admitted, tp, fn, fp,
/// agreeing_documents, margin_pass, margin_fail, no_denominator_documents)`.
/// Every counter is read from the committed recorded tables except
/// `agreeing_documents`, which is recomputed from the recorded per-document rows
/// so a re-measured table cannot silently keep the old population number.
fn frozen_aggregate() -> (u64, u64, u64, u64, u64, u64, u64, u64, u64) {
    let tables = frozen_tables();
    let aggregate = &tables["aggregate"];
    let agreeing_documents = tables["documents"]
        .as_array()
        .expect("documents")
        .iter()
        .filter(|document| document["agreement_micros"].as_u64() == Some(1_000_000))
        .count() as u64;
    (
        aggregate["source_population"]
            .as_u64()
            .expect("source_population"),
        aggregate["admitted"].as_u64().expect("admitted"),
        aggregate["tp"].as_u64().expect("tp"),
        aggregate["fn"].as_u64().expect("fn"),
        aggregate["fp"].as_u64().expect("fp"),
        agreeing_documents,
        aggregate["margin_pass"].as_u64().expect("margin_pass"),
        aggregate["margin_fail"].as_u64().expect("margin_fail"),
        aggregate["no_denominator_documents"]
            .as_u64()
            .expect("no_denominator_documents"),
    )
}

const PINNED_RELEASE_LABELS: [&str; 2] = ["java-grobid-http@0.9.1", "java-grobid-http@0.9.0-crf"];

/// Every redaction flag the artifact must carry, all `false`.
const REDACTION_FLAGS: [&str; 16] = [
    "raw_tei",
    "raw_pdf",
    "raw_text",
    "field_values",
    "names",
    "email_addresses",
    "institution_names",
    "captions",
    "references",
    "physical_paths",
    "hostname",
    "executable",
    "secrets",
    "pdf_bytes",
    "graph_payload",
    "fulltext",
];

/// Payload keys that may only ever appear as redaction *flags*.
const FORBIDDEN_KEYS: [&str; 12] = [
    "raw_tei",
    "tei_xml",
    "pdf_bytes",
    "body_text",
    "text",
    "captions",
    "references",
    "physical_path",
    "path",
    "hostname",
    "executable",
    "secret",
];

/// The guards the runner declares.  The self-check must prove each one rejects
/// its own violating input, so a guard that silently stops firing is visible.
const GUARD_LABELS: [&str; 32] = [
    "schema_version_drift",
    "reference_population_role",
    "reference_documents",
    "reference_identity_missing",
    "reference_identity_digest_drift",
    "source_manifest_sha_drift",
    "reference_byte_pins_drift",
    "outcome_count",
    "identity_sha_collapse",
    "identity_pin_drift",
    "executed_population_not_executed",
    "execution_evidence_unbound",
    "accepted_documents",
    "evaluated_status",
    "aggregate_zero_admitted_population",
    "aggregate_conservation_mismatch",
    "margin_verdict_absent",
    "margin_verdict_refitted",
    "latency_source_substituted",
    "latency_unmeasured",
    "legacy_block_present",
    "external_lane_state_flipped",
    "policy_lock_open",
    "redaction_flag_raised",
    "redaction_scope_replaced",
    "raw_payload_leak",
    "cost_unknown_read_as_zero",
    "production_catalog_overlap_absent",
    "named_limitations_missing",
    "class_narrowings_incomplete",
    "reference_measurable_documents",
    "reference_exceptions_drift",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn artifact_path() -> PathBuf {
    repo_root().join(ARTIFACT)
}

fn load_committed_artifact() -> Value {
    let path = artifact_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the canonical artifact {} must be regenerated by `cargo run -p da-cli --features pdf-layout --example external_holdout_release_probe -- --report {ARTIFACT}` ({error})",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the canonical artifact is valid JSON")
}

fn example_binary() -> PathBuf {
    // Provision the runner once per test process. `cargo test --test NAME` does
    // not build examples, so a stale binary from an earlier run would silently
    // shadow the source under test; an unconditional (incremental) build makes
    // the population proof reproducible. `OnceLock` keeps the parallel tests
    // from racing on cargo's target lock.
    static BINARY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BINARY
        .get_or_init(|| {
            let binary = repo_root().join("target/debug/examples/external_holdout_release_probe");
            let build = Command::new("cargo")
                .args([
                    "build",
                    "-q",
                    "-p",
                    "da-cli",
                    "--features",
                    "pdf-layout",
                    "--example",
                    "external_holdout_release_probe",
                ])
                .current_dir(repo_root())
                .output()
                .expect("build feature-gated release probe example");
            assert!(
                build.status.success(),
                "example build failed: {}",
                String::from_utf8_lossy(&build.stderr)
            );
            assert!(
                binary.is_file(),
                "the example binary must exist after a successful build"
            );
            binary
        })
        .clone()
}

// ─────────────────────── the independent expectations ───────────────────────

fn load_yaml(rel: &str) -> serde_yaml::Value {
    let text = fs::read_to_string(repo_root().join(rel)).expect("DATA file must be readable");
    serde_yaml::from_str(&text).expect("DATA file must be valid YAML")
}

/// A parsed DATA file, cached for the process lifetime.
///
/// The audit resolves one row at a time from the same frozen manifest and
/// provenance on every mutation (41 audits x ~610 pins / ~531 outcomes), so
/// re-reading and re-parsing ~150-190 KB of YAML per row made the census-scale
/// target CPU-bound (~49k parses, measured ~2700 s).  DATA files are read-only
/// within a test process, so a single parse is enough: the audit stays
/// semantically identical and becomes O(rows) instead of O(rows x parses).
fn cached_yaml(rel: &str) -> std::sync::Arc<serde_yaml::Value> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<serde_yaml::Value>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = cache.lock().expect("DATA cache must not be poisoned");
    if let Some(value) = guard.get(rel) {
        return std::sync::Arc::clone(value);
    }
    let value = std::sync::Arc::new(load_yaml(rel));
    guard.insert(rel.to_string(), std::sync::Arc::clone(&value));
    value
}

fn manifest_yaml() -> std::sync::Arc<serde_yaml::Value> {
    cached_yaml(MANIFEST_REL)
}

fn yaml_at<'a>(root: &'a serde_yaml::Value, key: &str) -> &'a serde_yaml::Value {
    root.get(key).expect("DATA key must exist")
}

/// Recompute the probe identity digest from the source manifest using ONLY the
/// declared canonicalization handles.  An independent implementation: the
/// artifact's declared digest is never trusted.
fn recompute_identity_digest(lane: &serde_yaml::Value) -> String {
    let fields: Vec<String> = lane["digest_input_fields"]
        .as_sequence()
        .expect("digest_input_fields")
        .iter()
        .map(|field| field.as_str().expect("field name").to_string())
        .collect();
    assert_eq!(
        lane["digest_input_order"].as_str(),
        Some("as_listed"),
        "the identity handles must be frozen in order"
    );
    let field_separator = lane["digest_field_separator"].as_str().expect("separator");
    let record_separator = lane["digest_record_separator"].as_str().expect("separator");
    let source = load_yaml(SOURCE_MANIFEST_REL);
    let mut records = Vec::new();
    for article in source["articles"].as_sequence().expect("articles") {
        let parts: Vec<String> = fields
            .iter()
            .map(|field| match &article[field.as_str()] {
                serde_yaml::Value::String(text) => text.clone(),
                other => panic!("field `{field}` must be a scalar string, got {other:?}"),
            })
            .collect();
        records.push(parts.join(field_separator));
    }
    format!(
        "sha256:{:x}",
        Sha256::digest(records.join(record_separator).as_bytes())
    )
}

/// The declared margins by metric, read from `acceptance_margins` (the single
/// place thresholds live).
fn declared_margins(manifest: &serde_yaml::Value) -> BTreeMap<String, (String, f64)> {
    let contract = yaml_at(manifest, "external_holdout_contract");
    let mut margins = BTreeMap::new();
    for entry in yaml_at(contract, "acceptance_margins")["margins"]
        .as_sequence()
        .expect("margins")
    {
        margins.insert(
            entry["metric"].as_str().expect("metric").to_string(),
            (
                entry["direction"].as_str().expect("direction").to_string(),
                entry["margin"].as_f64().expect("margin"),
            ),
        );
    }
    margins
}

fn provenance_latency(document_id: &str) -> u64 {
    let provenance = cached_yaml(PROVENANCE_REL);
    provenance["documents"]
        .as_sequence()
        .expect("provenance documents")
        .iter()
        .find(|row| row["article_id"].as_str() == Some(document_id))
        .expect("the provenance declares every frozen document")["latency_ms"]
        .as_u64()
        .expect("latency_ms")
}

// ─────────────────────────────── the audit ──────────────────────────────────

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn collect_strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => out.push(text),
        Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Value::Object(map) => map.values().for_each(|child| collect_strings(child, out)),
        _ => {}
    }
}

/// Payload keys may only appear as redaction flags.
fn raw_key_errors(report: &Value) -> Vec<String> {
    fn visit(value: &Value, inside_redaction: bool, errors: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let in_redaction = inside_redaction || key == "redaction";
                    if !in_redaction && FORBIDDEN_KEYS.contains(&key.as_str()) {
                        errors.push(format!("probe:raw_payload.key:{key}"));
                    }
                    visit(child, in_redaction, errors);
                }
            }
            Value::Array(items) => items
                .iter()
                .for_each(|item| visit(item, inside_redaction, errors)),
            _ => {}
        }
    }
    let mut errors = Vec::new();
    visit(report, false, &mut errors);
    errors
}

/// Every reason the runtime probe artifact is unacceptable.  A pure predicate so
/// the negative test can prove it rejects one violation at a time.
fn artifact_errors(
    report: &Value,
    expected_digest: &str,
    margins: &BTreeMap<String, (String, f64)>,
    declared: usize,
) -> Vec<String> {
    let mut errors = Vec::new();
    // The DATA surfaces this audit compares against, parsed once.
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    let lane = &contract["release_probe"];
    let frozen_documents = frozen_documents();

    // The frozen population is a DATA partition: the measurable `reference_tei`
    // rows plus the typed `reference_exceptions` rows.  The frozen reference
    // surface (`reference_population`, `production_catalog_overlap`) is audited
    // against `declared`; the measured surface (`executed_population`,
    // `evaluated_population`, `outcomes`) is audited against the measurable
    // subset.  If DATA moved the population off the frozen table, or the frozen
    // table no longer covers the measurable subset, the artifact cannot be
    // audited against it: refuse by name rather than compare against a stale
    // table.
    let measurable = lane["reference_tei"]
        .as_sequence()
        .map(Vec::len)
        .unwrap_or(0);
    let exceptions = lane["reference_exceptions"]
        .as_sequence()
        .map(Vec::len)
        .unwrap_or(0);
    if declared != measurable + exceptions || frozen_documents.len() != measurable {
        errors.push("probe:reference_population.frozen_table_drift".into());
        return errors;
    }

    if report["schema_version"].as_str() != Some(SCHEMA_VERSION) {
        errors.push("probe:schema_version".into());
    }

    // ── identity: distinct sha256 per frozen document, recomputed digest ──────
    let reference = &report["reference_population"];
    if reference["role"].as_str() != Some("identity_only") {
        errors.push("probe:reference_population.role".into());
    }
    if reference["identity"]
        .as_str()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        errors.push("probe:reference_population.identity".into());
    }
    if reference["identity_sha256"].as_str() != Some(expected_digest) {
        errors.push("probe:reference_population.identity_digest".into());
    }
    let declared_digest = lane["identity_digest"]
        .as_str()
        .expect("declared identity digest");
    if expected_digest != declared_digest {
        errors.push("probe:identity_digest_not_declared".into());
    }
    if reference["documents"].as_u64() != Some(declared as u64) {
        errors.push("probe:reference_population.documents".into());
    }
    let pins = reference["byte_pins"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if pins.len() != declared {
        errors.push("probe:reference_population.byte_pins.cardinality".into());
    }
    let pin_shas: BTreeSet<String> = pins
        .iter()
        .filter_map(|pin| pin["identity_sha256"].as_str().map(str::to_string))
        .collect();
    if pin_shas.len() != declared || pin_shas.iter().any(|sha| !is_sha256(sha)) {
        errors.push("probe:reference_population.byte_pins.shape".into());
    }
    for (index, pin) in pins.iter().enumerate() {
        let matches_lane =
            reference_pin_from_manifest(pin["document_id"].as_str().unwrap_or_default())
                .map(|(sha, size)| {
                    pin["identity_sha256"].as_str() == Some(sha.as_str())
                        && pin["size_bytes"].as_u64() == Some(size)
                })
                .unwrap_or(false);
        if !matches_lane {
            errors.push(format!("probe:reference_population.byte_pins[{index}]"));
        }
    }
    // The release attribution must be the declared one.
    let release = &reference["reference_release"];
    let declared_release = lane["declared_release"].clone();
    if release["version"].as_str() != declared_release["version"].as_str() {
        errors.push("probe:reference_population.release.version".into());
    }
    if release["profile"].as_str() != lane["reference_profile"].as_str() {
        errors.push("probe:reference_population.release.profile".into());
    }
    if release["captured_at_revision"].as_str()
        != cached_yaml(PROVENANCE_REL)["captured_at_revision"].as_str()
    {
        errors.push("probe:reference_population.release.captured_at_revision".into());
    }

    // ── executed population: really executed, evidence named ───────────────
    let executed = &report["executed_population"];
    if executed["corpus_execution"].as_bool() != Some(true) {
        errors.push("probe:executed_population.corpus_execution".into());
    }
    if executed["execution_evidence"].as_str() != Some(EXECUTION_EVIDENCE) {
        errors.push("probe:executed_population.execution_evidence".into());
    }
    if executed["documents"].as_u64() != Some(measurable as u64)
        || executed["accepted"].as_u64() != Some(measurable as u64)
    {
        errors.push("probe:executed_population.documents".into());
    }

    // ── GAR-006: the split is real, and the legacy blocks are gone ─────────
    for legacy in ["denominators", "corpus"] {
        if report.get(legacy).is_some() {
            errors.push(format!("probe:legacy_blocks.{legacy}"));
        }
    }
    if reference == executed
        || reference == &report["evaluated_population"]
        || executed == &report["evaluated_population"]
    {
        errors.push("probe:populations.indistinguishable".into());
    }

    // ── the external band stays where it is ────────────────────────────────
    let external = &report["external_holdout_band"];
    if external["execution_status"].as_str() != contract["execution_status"].as_str()
        || external["external_bytes"].as_str() != contract["byte_policy"]["external_bytes"].as_str()
    {
        errors.push("probe:external_holdout_band.flipped".into());
    }

    // ── evaluated population: measured status, counts-only aggregate ───────
    let evaluated = &report["evaluated_population"];
    if evaluated["status"].as_str() != Some("evaluated") {
        errors.push("probe:evaluated_population.status".into());
    }
    if evaluated["documents"].as_u64() != Some(measurable as u64) {
        errors.push("probe:evaluated_population.documents".into());
    }
    let aggregate = &evaluated["aggregate"];
    let observed_aggregate = (
        aggregate["source_population"].as_u64().unwrap_or(0),
        aggregate["admitted"].as_u64().unwrap_or(0),
        aggregate["tp"].as_u64().unwrap_or(0),
        aggregate["fn"].as_u64().unwrap_or(0),
        aggregate["fp"].as_u64().unwrap_or(0),
        aggregate["agreeing_documents"].as_u64().unwrap_or(0),
        aggregate["margin_pass"].as_u64().unwrap_or(0),
        aggregate["margin_fail"].as_u64().unwrap_or(0),
        aggregate["no_denominator_documents"].as_u64().unwrap_or(0),
    );
    let frozen = frozen_aggregate();
    if observed_aggregate != frozen {
        errors.push("probe:aggregate.frozen".into());
    }
    if aggregate["counts_only"].as_bool() != Some(true) {
        errors.push("probe:aggregate.counts_only".into());
    }
    if aggregate["admitted"].as_u64().unwrap_or(0) == 0 {
        errors.push("probe:aggregate.zero_denominator".into());
    }
    let aggregated_from_rows: (u64, u64, u64, u64, u64) = report["outcomes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .fold((0, 0, 0, 0, 0), |acc, outcome| {
            (
                acc.0 + outcome["source_population"].as_u64().unwrap_or(0),
                acc.1 + outcome["admitted"].as_u64().unwrap_or(0),
                acc.2 + outcome["conserved"].as_u64().unwrap_or(0),
                acc.3 + outcome["lost"].as_u64().unwrap_or(0),
                acc.4 + outcome["fabricated"].as_u64().unwrap_or(0),
            )
        });
    if aggregated_from_rows != (frozen.0, frozen.1, frozen.2, frozen.3, frozen.4) {
        errors.push("probe:aggregate.row_sum".into());
    }

    // ── outcomes: the frozen per-document measurement ──────────────────────
    let outcomes = report["outcomes"].as_array().cloned().unwrap_or_default();
    if outcomes.len() != measurable {
        errors.push("probe:outcomes.cardinality".into());
    }
    let mut measured_ids: BTreeSet<&str> = BTreeSet::new();
    for (document_id, sha, source_population, admitted, agreement, low, high, width, latency) in
        &frozen_documents
    {
        let row = outcomes
            .iter()
            .find(|row| row["document_id"].as_str() == Some(document_id.as_str()))
            .unwrap_or_else(|| panic!("the artifact must carry the frozen document {document_id}"));
        measured_ids.insert(document_id.as_str());
        if row["identity_sha256"].as_str() != Some(sha.as_str()) {
            errors.push(format!("probe:outcomes.{document_id}.identity"));
        }
        if row["state"].as_str() != Some("evaluated") {
            errors.push(format!("probe:outcomes.{document_id}.state"));
        }
        let observed = (
            row["source_population"].as_u64(),
            row["admitted"].as_u64(),
            row["conserved"].as_u64(),
            row["lost"].as_u64(),
            row["fabricated"].as_u64(),
            row["agreement_ratio_micros"].as_u64(),
            row["uncertainty_low"].as_u64(),
            row["uncertainty_high"].as_u64(),
            row["uncertainty_width_micros"].as_u64(),
        );
        if observed
            != (
                Some(*source_population),
                Some(*admitted),
                Some(*admitted),
                Some(0),
                Some(0),
                Some(*agreement),
                Some(*low),
                Some(*high),
                Some(*width),
            )
        {
            errors.push(format!("probe:outcomes.{document_id}.profile"));
        }
        if *width != high.saturating_sub(*low) {
            errors.push(format!("probe:outcomes.{document_id}.interval"));
        }
        if row["latency_ms"].as_u64() != Some(*latency)
            || row["latency_ms"].as_u64() != Some(provenance_latency(document_id))
        {
            errors.push(format!("probe:outcomes.{document_id}.latency"));
        }
        if row["latency_source"].as_str() != Some(LATENCY_SOURCE_REF) {
            errors.push(format!("probe:outcomes.{document_id}.latency_source"));
        }
        if !row["cost_lower_bound"].is_null() || !row["cost_upper_bound"].is_null() {
            errors.push(format!("probe:outcomes.{document_id}.cost_unknown"));
        }
        // The declared per-document reporting fields must all be present.
        for field in [
            "document_id",
            "state",
            "identity_sha256",
            "uncertainty_low",
            "uncertainty_high",
            "cost_lower_bound",
            "cost_upper_bound",
        ] {
            if row.get(field).is_none() {
                errors.push(format!("probe:outcomes.{document_id}.field:{field}"));
            }
        }
        // ── margin verdicts: present, and not re-fitted ────────────────────
        let margins_rows = row["margins"].as_array().cloned().unwrap_or_default();
        if margins_rows.len() != margins.len() {
            errors.push(format!("probe:outcomes.{document_id}.margins.cardinality"));
        }
        for margin in &margins_rows {
            let metric = margin["metric"].as_str().unwrap_or_default();
            let Some((direction, threshold)) = margins.get(metric) else {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.unknown:{metric}"
                ));
                continue;
            };
            let observed_threshold = margin["threshold_micros"].as_u64();
            if observed_threshold != Some((threshold * 1_000_000.0).round() as u64) {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.threshold:{metric}"
                ));
            }
            let value = margin["value_micros"].as_u64();
            let expected_verdict = match value {
                None => "no_denominator",
                Some(value) => {
                    let value = value as f64 / 1_000_000.0;
                    let satisfied = if direction == "at_least" {
                        value >= *threshold
                    } else {
                        value <= *threshold
                    };
                    if satisfied { "PASS" } else { "FAIL" }
                }
            };
            if margin["verdict"].as_str() != Some(expected_verdict) {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.refitted:{metric}"
                ));
            }
        }
        // ── typed gaps: every narrowing is explained ───────────────────────
        for gap in row["typed_gaps"].as_array().cloned().unwrap_or_default() {
            let admitted = gap["admitted"].as_u64().unwrap_or(u64::MAX);
            let source_population = gap["source_population"].as_u64().unwrap_or(0);
            if admitted >= source_population || source_population == 0 {
                errors.push(format!(
                    "probe:outcomes.{document_id}.typed_gaps.not_a_narrowing"
                ));
            }
            if gap["reason"]
                .as_str()
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .is_none()
            {
                errors.push(format!("probe:outcomes.{document_id}.typed_gaps.reason"));
            }
        }
    }
    if outcomes
        .iter()
        .filter_map(|row| row["document_id"].as_str())
        .any(|document_id| !measured_ids.contains(document_id))
    {
        errors.push("probe:outcomes.unexpected_document".into());
    }

    // ── class narrowings: the frozen table, and nothing silent ─────────────
    let declared_narrowings: BTreeSet<(String, String, u64, u64)> = report["class_narrowings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            Some((
                row["document_id"].as_str()?.to_string(),
                row["class"].as_str()?.to_string(),
                row["source_population"].as_u64()?,
                row["admitted"].as_u64()?,
            ))
        })
        .collect();
    let expected_narrowings: BTreeSet<(String, String, u64, u64)> =
        frozen_narrowings().into_iter().collect();
    if declared_narrowings != expected_narrowings {
        errors.push("probe:class_narrowings.frozen".into());
    }
    // Every class profile must derive exactly the declared narrowing set.
    let mut derived: BTreeSet<(String, String, u64, u64)> = BTreeSet::new();
    for document in report["class_profiles"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let document_id = document["document_id"].as_str().unwrap_or_default();
        for class_row in document["classes"].as_array().cloned().unwrap_or_default() {
            let admitted = class_row["admitted"].as_u64().unwrap_or(0);
            let source_population = class_row["source_population"].as_u64().unwrap_or(0);
            if admitted < source_population {
                derived.insert((
                    document_id.to_string(),
                    class_row["class"].as_str().unwrap_or_default().to_string(),
                    source_population,
                    admitted,
                ));
            }
        }
    }
    if derived != expected_narrowings {
        errors.push("probe:class_profiles.narrowings".into());
    }

    // ── catalog overlap: a real measurement, not an assertion ──────────────
    let census = &report["production_catalog_overlap"];
    let locators = census["locators"].as_array().cloned().unwrap_or_default();
    if census["census_kind"].as_str() != Some("filesystem_presence_of_the_holdout_identities")
        || census["documents"].as_u64() != Some(declared as u64)
        || locators.len() != declared
    {
        errors.push("probe:production_catalog_overlap.shape".into());
    }
    // The non-gold claim is the one this census exists to measure.
    if census["gold_standard_present_count"].as_u64() != Some(0) {
        errors.push("probe:production_catalog_overlap.gold_standard".into());
    }
    if census["canonical_catalog_present_count"].as_u64() != Some(0) {
        errors.push("probe:production_catalog_overlap.canonical_catalog".into());
    }
    if census["production_corpus_identity_overlap"].as_str() != Some("none") {
        errors.push("probe:production_catalog_overlap.overlap_label".into());
    }
    // Each locator row must be a measurement: declared-present must agree with
    // the manifest's declared path, and the flags must be booleans.
    let absent_expected = locators
        .iter()
        .filter(|row| {
            row["declared_pdf_present"].as_bool() != Some(true)
                && row["canonical_catalog_present"].as_bool() != Some(true)
                && row["sources_archive_present"].as_bool() != Some(true)
                && row["gold_standard_present"].as_bool() != Some(true)
        })
        .count() as u64;
    if census["absent_from_every_locator_count"].as_u64() != Some(absent_expected) {
        errors.push("probe:production_catalog_overlap.absent_count".into());
    }
    for row in &locators {
        if row["document_id"]
            .as_str()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            errors.push("probe:production_catalog_overlap.locator".into());
        }
        for flag in [
            "declared_pdf_present",
            "canonical_catalog_present",
            "sources_archive_present",
            "gold_standard_present",
        ] {
            if row[flag].as_bool().is_none() {
                errors.push(format!("probe:production_catalog_overlap.flag:{flag}"));
            }
        }
    }

    // ── cost and latency accounting: bounded, never measured, never pooled ─
    let cost = &report["cost_accounting"];
    if cost["status"].as_str() != Some("bounded_not_measured")
        || cost["unknown_is_not_zero"].as_bool() != Some(true)
        || !cost["cost_lower_bound"].is_null()
        || !cost["cost_upper_bound"].is_null()
        || cost["pooled_cost_claim"].as_str() != Some("forbidden")
    {
        errors.push("probe:cost_accounting".into());
    }
    let latency = &report["latency_accounting"];
    if latency["measurement_source"].as_str() != Some("recorded_wall_clock_per_document")
        || latency["latency_source"].as_str() != Some(LATENCY_SOURCE_REF)
        || latency["pooled_latency_claim"].as_str() != Some("forbidden")
        || latency["wall_clock_at_run_time"].as_bool() != Some(false)
    {
        errors.push("probe:latency_accounting".into());
    }

    // ── policy locks: shadow only, D127 closed ─────────────────────────────
    let policy = &report["policy_locks"];
    for (key, expected) in [
        ("auto_promote", "forbidden"),
        ("production_route", "forbidden"),
        ("graph_write", "forbidden"),
        ("pooled_claim", "forbidden"),
    ] {
        if policy[key].as_str() != Some(expected) {
            errors.push(format!("probe:policy_locks.{key}"));
        }
    }
    if policy["import_eligible"].as_bool() != Some(false)
        || policy["deployed"].as_bool() != Some(false)
    {
        errors.push("probe:policy_locks.bool".into());
    }

    // ── redaction: declared flags only, all false, no payload anywhere ─────
    let redaction = &report["redaction"];
    let declared_flags: BTreeSet<&str> = redaction
        .as_object()
        .map(|flags| flags.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_flags: BTreeSet<&str> = REDACTION_FLAGS.iter().copied().collect();
    if declared_flags != expected_flags {
        errors.push("probe:redaction.scope".into());
    }
    if let Some(flags) = redaction.as_object() {
        for (flag, value) in flags {
            if value.as_bool() != Some(false) {
                errors.push(format!("probe:redaction.{flag}"));
            }
        }
    }
    errors.extend(raw_key_errors(report));
    if serde_json::to_string(report)
        .unwrap_or_default()
        .contains('<')
    {
        errors.push("probe:raw_payload.xml".into());
    }
    let mut strings = Vec::new();
    collect_strings(report, &mut strings);
    for text in strings.into_iter().filter(|text| text.contains('@')) {
        if !PINNED_RELEASE_LABELS.contains(&text) {
            errors.push("probe:raw_payload.at".into());
        }
    }

    // ── the limitations are declared next to the numbers ───────────────────
    if report["named_limitations"]
        .as_array()
        .map(Vec::is_empty)
        .unwrap_or(true)
    {
        errors.push("probe:named_limitations".into());
    }
    errors
}

/// A lane byte pin from the manifest: `(identity sha256 without prefix, size)`.
fn reference_pin_from_manifest(document_id: &str) -> Option<(String, u64)> {
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    let lane = &contract["release_probe"];
    // The frozen population is the measurable pins plus the typed exceptions:
    // the artifact's `byte_pins` covers all 50, so the manifest lookup must too.
    let mut rows: Vec<&serde_yaml::Value> = Vec::new();
    for key in ["reference_tei", "reference_exceptions"] {
        if let Some(entries) = lane[key].as_sequence() {
            rows.extend(entries.iter());
        }
    }
    rows.into_iter()
        .find(|row| row["article_id"].as_str() == Some(document_id))
        .and_then(|row| {
            Some((
                row["sha256"].as_str()?.strip_prefix("sha256:")?.to_string(),
                row["size_bytes"].as_u64()?,
            ))
        })
}

// ──────────────────────────────── positives ─────────────────────────────────

#[test]
fn committed_artifact_is_a_frozen_probe_measurement() {
    let report = load_committed_artifact();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest_yaml(), "external_holdout_contract")["release_probe"],
    );
    let errors = artifact_errors(
        &report,
        &expected_digest,
        &declared_margins(&manifest_yaml()),
        declared_population(),
    );
    assert!(
        errors.is_empty(),
        "the committed {} failed its own audit: {errors:#?}",
        artifact_path().display()
    );
    // Non-vacuity, stated once outside the audit.
    assert!(
        report["evaluated_population"]["aggregate"]["tp"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "the probe metric must be non-vacuous"
    );
}

#[test]
fn external_band_and_human_release_decision_stay_open() {
    // H20 is not closed by this artifact: the external bytes band stays absent
    // and the release decision stays with a real person.
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    assert_eq!(contract["execution_status"].as_str(), Some("not_executed"));
    assert_eq!(
        contract["byte_policy"]["external_bytes"].as_str(),
        Some("bytes_absent")
    );
    assert_eq!(
        contract["byte_policy"]["external_rows_are_evidence"].as_bool(),
        Some(false)
    );
    let review = yaml_at(&manifest, "release_decision_package")["human_release_review"].clone();
    assert_eq!(review["decision"].as_str(), Some("pending"));
    assert!(review["reviewer"].is_null());
    assert_eq!(
        review["prose_alone_authorizes_release"].as_bool(),
        Some(false)
    );
}

// ──────────────────────────────── negatives ────────────────────────────────

#[test]
fn artifact_audit_rejects_every_declared_violation() {
    let valid = load_committed_artifact();
    let manifest = manifest_yaml();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest, "external_holdout_contract")["release_probe"],
    );
    let margins = declared_margins(&manifest);
    assert!(
        artifact_errors(&valid, &expected_digest, &margins, declared_population()).is_empty(),
        "the unmutated artifact must pass the audit"
    );

    let first_sha = valid["outcomes"][0]["identity_sha256"].clone();
    let mut mutations: Vec<(&str, Value)> = Vec::new();

    let mut synthetic = valid.clone();
    synthetic["schema_version"] = json!("external-holdout-release-probe.v0");
    mutations.push(("schema_version_rewritten", synthetic));

    let mut role = valid.clone();
    role["reference_population"]["role"] = json!("identity_and_execution");
    mutations.push(("reference_population_role_rewritten", role));

    let mut digest = valid.clone();
    digest["reference_population"]["identity_sha256"] = json!("sha256:drift");
    mutations.push(("identity_digest_drifted", digest));

    let mut pin_drift = valid.clone();
    pin_drift["reference_population"]["byte_pins"][0]["size_bytes"] = json!(1);
    mutations.push(("byte_pin_drifted", pin_drift));

    let mut missing_pin = valid.clone();
    missing_pin["reference_population"]["byte_pins"]
        .as_array_mut()
        .expect("byte pins")
        .pop();
    mutations.push(("byte_pin_removed", missing_pin));

    let mut lost_documents = valid.clone();
    lost_documents["reference_population"]["documents"] = json!(1);
    mutations.push(("document_denominator_lost", lost_documents));

    let mut denied_execution = valid.clone();
    denied_execution["executed_population"]["corpus_execution"] = json!(false);
    mutations.push(("corpus_execution_denied", denied_execution));

    let mut unbound_evidence = valid.clone();
    unbound_evidence["executed_population"]["execution_evidence"] = json!("n/a");
    mutations.push(("execution_evidence_unbound", unbound_evidence));

    let mut legacy = valid.clone();
    legacy["denominators"] = json!({"documents": declared_population()});
    mutations.push(("legacy_denominators_block_reintroduced", legacy));

    let mut legacy_corpus = valid.clone();
    legacy_corpus["corpus"] = json!({"documents": declared_population()});
    mutations.push(("legacy_corpus_block_reintroduced", legacy_corpus));

    let mut split_collapsed = valid.clone();
    split_collapsed["executed_population"] = valid["reference_population"].clone();
    mutations.push(("populations_collapsed", split_collapsed));

    let mut flipped_band = valid.clone();
    flipped_band["external_holdout_band"]["execution_status"] = json!("executed");
    mutations.push(("external_band_lifted", flipped_band));

    let mut unstated_status = valid.clone();
    unstated_status["evaluated_population"]["status"] = json!("not_evaluated");
    mutations.push(("evaluated_status_removed", unstated_status));

    let mut vacuous = valid.clone();
    vacuous["evaluated_population"]["aggregate"]["admitted"] = json!(0);
    vacuous["evaluated_population"]["aggregate"]["tp"] = json!(0);
    vacuous["evaluated_population"]["aggregate"]["source_population"] = json!(0);
    mutations.push(("vacuous_metric", vacuous));

    let mut fabricated = valid.clone();
    fabricated["evaluated_population"]["aggregate"]["fp"] = json!(7);
    mutations.push(("fabricated_band_admitted", fabricated));

    let mut non_counts = valid.clone();
    non_counts["evaluated_population"]["aggregate"]["counts_only"] = json!(false);
    mutations.push(("counts_only_dropped", non_counts));

    let mut doc_drift = valid.clone();
    doc_drift["outcomes"][0]["admitted"] = json!(1);
    mutations.push(("outcome_admitted_drifted", doc_drift));

    let mut latency_swap = valid.clone();
    latency_swap["outcomes"][0]["latency_source"] = json!("std::time::Instant::now()");
    mutations.push(("latency_source_substituted", latency_swap));

    let mut latency_zero = valid.clone();
    latency_zero["outcomes"][0]["latency_ms"] = json!(0);
    mutations.push(("latency_unmeasured", latency_zero));

    let mut sha_collapse = valid.clone();
    sha_collapse["outcomes"][1]["identity_sha256"] = first_sha;
    mutations.push(("outcome_identity_collapsed", sha_collapse));

    let mut dropped_field = valid.clone();
    dropped_field["outcomes"][0]
        .as_object_mut()
        .expect("outcome")
        .remove("uncertainty_low");
    mutations.push(("declared_reporting_field_dropped", dropped_field));

    let mut dropped_margin = valid.clone();
    dropped_margin["outcomes"][0]["margins"]
        .as_array_mut()
        .expect("margins")
        .pop();
    mutations.push(("margin_verdict_missing", dropped_margin));

    let mut refitted = valid.clone();
    refitted["outcomes"][0]["margins"][0]["verdict"] = json!("FAIL");
    mutations.push(("margin_verdict_refitted", refitted));

    let mut lowered_threshold = valid.clone();
    lowered_threshold["outcomes"][0]["margins"][0]["threshold_micros"] = json!(1);
    mutations.push(("margin_threshold_duplicated", lowered_threshold));

    let mut gap_without_reason = valid.clone();
    gap_without_reason["outcomes"][0]["typed_gaps"] = json!([{
        "class": "SemanticFieldLoss::HeaderTitle",
        "source_population": 2,
        "admitted": 1,
        "reason": "",
    }]);
    mutations.push(("typed_gap_reason_removed", gap_without_reason));

    let mut fake_gap = valid.clone();
    fake_gap["outcomes"][0]["typed_gaps"] = json!([{
        "class": "SemanticFieldLoss::HeaderTitle",
        "source_population": 2,
        "admitted": 2,
        "reason": "not a narrowing",
    }]);
    mutations.push(("non_narrowing_declared_as_gap", fake_gap));

    let mut narrowings_removed = valid.clone();
    narrowings_removed["class_narrowings"] = json!([]);
    mutations.push(("class_narrowings_removed", narrowings_removed));

    let mut narrowing_drift = valid.clone();
    narrowing_drift["class_narrowings"][0]["document_id"] = json!("unknown");
    mutations.push(("class_narrowing_drifted", narrowing_drift));

    let mut census_emptied = valid.clone();
    census_emptied["production_catalog_overlap"]["locators"] = json!([]);
    mutations.push(("census_locators_removed", census_emptied));

    let mut gold_member = valid.clone();
    gold_member["production_catalog_overlap"]["gold_standard_present_count"] = json!(1);
    mutations.push(("gold_standard_membership_claimed", gold_member));

    let mut census_label = valid.clone();
    census_label["production_catalog_overlap"]["production_corpus_identity_overlap"] =
        json!("present");
    mutations.push(("overlap_label_rewritten", census_label));

    let mut cost_zero = valid.clone();
    cost_zero["cost_accounting"]["cost_lower_bound"] = json!(0);
    mutations.push(("unknown_cost_read_as_zero", cost_zero));

    let mut cost_measured = valid.clone();
    cost_measured["cost_accounting"]["status"] = json!("measured");
    mutations.push(("cost_claimed_as_measured", cost_measured));

    let mut latency_clock = valid.clone();
    latency_clock["latency_accounting"]["wall_clock_at_run_time"] = json!(true);
    mutations.push(("run_time_clock_used", latency_clock));

    let mut unlocked_import = valid.clone();
    unlocked_import["policy_locks"]["import_eligible"] = json!(true);
    mutations.push(("import_lock_removed", unlocked_import));

    let mut unlocked_promotion = valid.clone();
    unlocked_promotion["policy_locks"]["auto_promote"] = json!("allowed");
    mutations.push(("auto_promote_unlocked", unlocked_promotion));

    let mut redaction_lie = valid.clone();
    redaction_lie["redaction"]["raw_tei"] = json!(true);
    mutations.push(("redaction_flag_raised", redaction_lie));

    let mut redaction_scope = valid.clone();
    redaction_scope["redaction"]["tei_xml"] = json!(false);
    mutations.push(("redaction_scope_replaced", redaction_scope));

    let mut raw_payload = valid.clone();
    raw_payload["outcomes"][0]["raw_tei"] = json!("<teiHeader/>");
    mutations.push(("raw_payload_injected", raw_payload));

    let mut leaked_email = valid.clone();
    leaked_email["outcomes"][0]["contact"] = json!("someone@example.org");
    mutations.push(("email_injected", leaked_email));

    let mut limitations_removed = valid.clone();
    limitations_removed["named_limitations"] = json!([]);
    mutations.push(("named_limitations_removed", limitations_removed));

    for (name, mutant) in mutations {
        let errors = artifact_errors(&mutant, &expected_digest, &margins, declared_population());
        assert!(
            !errors.is_empty(),
            "the audit accepted the mutation `{name}`"
        );
    }
}

#[test]
fn guard_self_check_rejects_every_declared_violation() {
    let output = Command::new(example_binary())
        .arg("--self-check")
        .current_dir(repo_root())
        .output()
        .expect("run the release probe guard self-check");
    assert!(
        output.status.success(),
        "self-check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for guard in GUARD_LABELS {
        assert!(
            stdout.contains(&format!("guard {guard}: rejected")),
            "the self-check did not prove `{guard}` rejects its violating input:\n{stdout}"
        );
    }
    assert!(
        stdout.contains(&format!(
            "guard self-check: {} guards rejected their violating input",
            GUARD_LABELS.len()
        )),
        "the self-check must report every declared guard:\n{stdout}"
    );
}

/// The lane's guard surface must follow the DATA, not a compiled-in population:
/// one binary, two populations, no code change between the runs.  The synthetic
/// fixture is code-built and lives only for the run, never written into the
/// repository, so it cannot dilute the declared pilot.
#[test]
fn guard_self_check_scales_to_any_population_without_a_code_change() {
    let binary = example_binary();
    let base = declared_population();
    let populations = [base, base + 2]; // N=5 and N=7 on the frozen manifest.
    let mut recorded: BTreeMap<usize, String> = BTreeMap::new();
    for population in populations {
        let output = Command::new(&binary)
            .args(["--self-check", "--documents", &population.to_string()])
            .current_dir(repo_root())
            .output()
            .expect("run the release probe guard self-check");
        assert!(
            output.status.success(),
            "self-check failed at population {population}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(
            stdout.contains(&format!(
                "guard self-check population: {population} documents"
            )),
            "the self-check must record the population it exercised:\n{stdout}"
        );
        for guard in GUARD_LABELS {
            assert!(
                stdout.contains(&format!("guard {guard}: rejected")),
                "population {population} did not prove `{guard}` rejects its violating input:\n{stdout}"
            );
        }
        assert!(
            stdout.contains(&format!(
                "guard self-check: {} guards rejected their violating input",
                GUARD_LABELS.len()
            )),
            "population {population} must report every declared guard:\n{stdout}"
        );
        recorded.insert(population, stdout);
    }

    assert_ne!(populations[0], populations[1]);
    // Strip the population line: the two recorded runs must otherwise be equal,
    // which is what "the same binary, only the data changed" means.
    let fingerprint = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("guard self-check population:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        fingerprint(&recorded[&populations[0]]),
        fingerprint(&recorded[&populations[1]]),
        "the two populations must expose the same guard surface"
    );
}

#[test]
fn documents_flag_is_refused_outside_the_self_check() {
    let output = Command::new(example_binary())
        .args(["--documents", "7"])
        .current_dir(repo_root())
        .output()
        .expect("run the release probe example with --documents but no --self-check");
    assert!(
        !output.status.success(),
        "--documents outside --self-check must fail closed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--documents is only meaningful together with --self-check"),
        "the refusal must name the argument:\n{stderr}"
    );
}

#[test]
fn zero_document_self_check_is_refused() {
    let output = Command::new(example_binary())
        .args(["--self-check", "--documents", "0"])
        .current_dir(repo_root())
        .output()
        .expect("run the release probe guard self-check at population 0");
    assert!(
        !output.status.success(),
        "a zero-document population must be refused, not reported as a vacuous pass"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("a zero-document population is not a probe"),
        "the refusal must name the reason:\n{stdout}"
    );
}

#[test]
fn artifact_audit_refuses_a_data_population_that_left_the_frozen_table() {
    let report = load_committed_artifact();
    let manifest = manifest_yaml();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest, "external_holdout_contract")["release_probe"],
    );
    let margins = declared_margins(&manifest);
    let declared = declared_population();
    assert!(
        artifact_errors(&report, &expected_digest, &margins, declared).is_empty(),
        "the committed artifact must pass at the declared population"
    );
    // DATA moved off the frozen table: the audit refuses by name instead of
    // comparing the artifact against a stale table.
    assert_eq!(
        artifact_errors(&report, &expected_digest, &margins, declared + 2),
        vec!["probe:reference_population.frozen_table_drift".to_string()],
        "a population that left the frozen table must be refused by name"
    );
}

#[test]
fn committed_artifact_equals_two_independent_runs() {
    let binary = example_binary();
    let committed = fs::read(artifact_path()).expect("committed artifact");
    let mut streams: Vec<Vec<u8>> = Vec::new();
    for _ in 0..2 {
        // A different working directory proves the artifact is not a function of
        // the invoking cwd: the runner resolves everything from the repo root.
        let dir = tempdir().expect("temporary run directory");
        let report = dir.path().join("report.json");
        let output = Command::new(&binary)
            .args(["--report", report.to_str().expect("report path")])
            .current_dir(dir.path())
            .output()
            .expect("run the release probe example");
        assert!(
            output.status.success(),
            "runner failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        streams.push(fs::read(&report).expect("generated report bytes"));
    }
    assert_eq!(
        streams[0], streams[1],
        "two independent runs must be byte-identical"
    );
    assert_eq!(
        streams[0], committed,
        "the committed artifact must equal a fresh canonical run"
    );
}

#[test]
fn missing_report_fails_closed_without_writing_an_artifact() {
    let output = Command::new(example_binary())
        .current_dir(repo_root())
        .output()
        .expect("run the release probe example without --report");
    assert!(
        !output.status.success(),
        "the canonical run must refuse to start without --report"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--report <path> is required for the canonical run"),
        "the refusal must name the missing argument:\n{stderr}"
    );

    // The recorded tables share that refusal: a table is never written without
    // the canonical artifact it projects, so the generator cannot record a
    // measurement the artifact does not carry.
    let dir = tempdir().expect("temporary run directory");
    let tables = dir.path().join("frozen_tables.json");
    let output = Command::new(example_binary())
        .args([
            "--emit-frozen-tables",
            tables.to_str().expect("tables path"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("run the release probe generator without --report");
    assert!(
        !output.status.success(),
        "the generator must refuse to emit a recorded table without --report"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--report <path> is required for the canonical run"),
        "the refusal must name the missing argument:\n{stderr}"
    );
    assert!(
        !tables.exists(),
        "a refused generator must not create the recorded tables"
    );
}

/// Compare the recorded tables against the committed artifact row by row, naming
/// every disagreement.  The names are the guard's contract:
/// `frozen_tables_artifact_sha256` (the tables describe a different artifact),
/// `frozen_tables_missing:<id>` (a document present on one side but not the
/// other), and `frozen_tables_row_drift:<id>` (a recorded value the artifact does
/// not carry).  A pure function so the guard can prove its own rejections.
fn frozen_tables_drift(tables: &Value, artifact: &Value, artifact_bytes: &[u8]) -> Vec<String> {
    let mut drift = Vec::new();
    let recorded_sha = format!("{:x}", Sha256::digest(artifact_bytes));
    if tables["artifact_sha256"].as_str() != Some(recorded_sha.as_str()) {
        drift.push("frozen_tables_artifact_sha256".to_string());
    }

    let empty: Vec<Value> = Vec::new();
    let recorded: BTreeMap<&str, &Value> = tables["documents"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|document| Some((document["article_id"].as_str()?, document)))
        .collect();
    let outcomes: BTreeMap<&str, &Value> = artifact["outcomes"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|outcome| Some((outcome["document_id"].as_str()?, outcome)))
        .collect();
    let profiles: BTreeMap<&str, &Value> = artifact["class_profiles"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|profile| Some((profile["document_id"].as_str()?, profile)))
        .collect();

    for (document_id, outcome) in &outcomes {
        let Some(document) = recorded.get(document_id) else {
            drift.push(format!("frozen_tables_missing:{document_id}"));
            continue;
        };
        let mut row_ok = true;
        let observed = (
            outcome["identity_sha256"].as_str(),
            outcome["source_population"].as_u64(),
            outcome["admitted"].as_u64(),
            outcome["conserved"].as_u64(),
            outcome["fabricated"].as_u64(),
            outcome["agreement_ratio_micros"].as_u64(),
            outcome["uncertainty_low"].as_u64(),
            outcome["uncertainty_high"].as_u64(),
            outcome["uncertainty_width_micros"].as_u64(),
            outcome["latency_ms"].as_u64(),
        );
        let expected = (
            document["sha256"].as_str(),
            document["source_population"].as_u64(),
            document["admitted"].as_u64(),
            document["conserved"].as_u64(),
            document["rejected"].as_u64(),
            document["agreement_micros"].as_u64(),
            document["uncertainty_low_micros"].as_u64(),
            document["uncertainty_high_micros"].as_u64(),
            document["uncertainty_width_micros"].as_u64(),
            document["latency_ms"].as_u64(),
        );
        if observed != expected {
            row_ok = false;
        }
        let margins = outcome["margins"].as_array().cloned().unwrap_or_default();
        let margin_pass = margins
            .iter()
            .filter(|margin| margin["verdict"].as_str() == Some("PASS"))
            .count() as u64;
        let margin_fail = margins
            .iter()
            .filter(|margin| margin["verdict"].as_str() == Some("FAIL"))
            .count() as u64;
        if document["margin_pass"].as_u64() != Some(margin_pass)
            || document["margin_fail"].as_u64() != Some(margin_fail)
            || document["no_denominator"].as_bool()
                != Some(outcome["agreement_ratio_micros"].is_null())
        {
            row_ok = false;
        }
        match profiles.get(document_id) {
            Some(profile) => {
                let artifact_classes: BTreeMap<&str, &Value> = profile["classes"]
                    .as_array()
                    .unwrap_or(&empty)
                    .iter()
                    .filter_map(|class| Some((class["class"].as_str()?, class)))
                    .collect();
                let recorded_classes = document["profile"].as_array().cloned().unwrap_or_default();
                if recorded_classes.len() != artifact_classes.len() {
                    row_ok = false;
                }
                for class in &recorded_classes {
                    let Some(artifact_class) =
                        artifact_classes.get(class["class"].as_str().unwrap_or_default())
                    else {
                        row_ok = false;
                        continue;
                    };
                    if class["source_population"].as_u64()
                        != artifact_class["source_population"].as_u64()
                        || class["admitted"].as_u64() != artifact_class["admitted"].as_u64()
                        || class["conserved"].as_u64() != artifact_class["conserved"].as_u64()
                        || class["rejected"].as_u64() != artifact_class["fabricated"].as_u64()
                    {
                        row_ok = false;
                    }
                }
            }
            None => row_ok = false,
        }
        if !row_ok {
            drift.push(format!("frozen_tables_row_drift:{document_id}"));
        }
    }
    for document_id in recorded.keys() {
        if !outcomes.contains_key(document_id) {
            drift.push(format!("frozen_tables_missing:{document_id}"));
        }
    }

    let aggregate = &artifact["evaluated_population"]["aggregate"];
    let recorded_aggregate = &tables["aggregate"];
    let aggregate_ok = recorded_aggregate["documents"].as_u64() == aggregate["documents"].as_u64()
        && recorded_aggregate["source_population"].as_u64()
            == aggregate["source_population"].as_u64()
        && recorded_aggregate["admitted"].as_u64() == aggregate["admitted"].as_u64()
        && recorded_aggregate["tp"].as_u64() == aggregate["tp"].as_u64()
        && recorded_aggregate["fn"].as_u64() == aggregate["fn"].as_u64()
        && recorded_aggregate["fp"].as_u64() == aggregate["fp"].as_u64()
        && recorded_aggregate["agreeing_documents"].as_u64()
            == aggregate["agreeing_documents"].as_u64()
        && recorded_aggregate["margin_pass"].as_u64() == aggregate["margin_pass"].as_u64()
        && recorded_aggregate["margin_fail"].as_u64() == aggregate["margin_fail"].as_u64()
        && recorded_aggregate["no_denominator_documents"].as_u64()
            == aggregate["no_denominator_documents"].as_u64();
    if !aggregate_ok {
        drift.push("frozen_tables_row_drift:aggregate".to_string());
    }

    let mut recorded_narrowings: BTreeSet<(String, String, u64, u64)> = BTreeSet::new();
    for document in tables["documents"].as_array().unwrap_or(&empty) {
        let id = document["article_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        for narrowing in document["narrowings"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            recorded_narrowings.insert((
                id.clone(),
                narrowing["class"].as_str().unwrap_or_default().to_string(),
                narrowing["source_population"].as_u64().unwrap_or(0),
                narrowing["admitted"].as_u64().unwrap_or(0),
            ));
        }
    }
    let artifact_narrowings: BTreeSet<(String, String, u64, u64)> = artifact["class_narrowings"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|row| {
            (
                row["document_id"].as_str().unwrap_or_default().to_string(),
                row["class"].as_str().unwrap_or_default().to_string(),
                row["source_population"].as_u64().unwrap_or(0),
                row["admitted"].as_u64().unwrap_or(0),
            )
        })
        .collect();
    if recorded_narrowings != artifact_narrowings {
        drift.push("frozen_tables_row_drift:narrowings".to_string());
    }

    drift
}

/// The recorded measurement tables are DERIVED FROM the committed artifact, not
/// hand-typed: this guard re-reads
/// `artifacts/parse_compat/external_holdout_release_probe.json` and asserts that
/// every recorded row, the aggregate and the named narrowings are exactly its
/// `outcomes`, aggregate and `class_narrowings`.  A substituted recorded number,
/// a removed document, a stale artifact, or a table regenerated without the other
/// fails here by name (`frozen_tables_artifact_sha256`,
/// `frozen_tables_missing:<id>`, `frozen_tables_row_drift:<id>`).  The audit
/// itself still compares a *mutated* artifact against these recorded tables, so
/// this guard does not make the mutation matrix self-referential.
#[test]
fn frozen_tables_are_derived_from_the_committed_artifact() {
    let tables = frozen_tables();
    let artifact_bytes = fs::read(artifact_path()).expect("committed artifact");
    let artifact: Value =
        serde_json::from_slice(&artifact_bytes).expect("the committed artifact is valid JSON");
    let drift = frozen_tables_drift(&tables, &artifact, &artifact_bytes);
    assert!(
        drift.is_empty(),
        "the recorded tables drifted from the committed artifact: {drift:#?}"
    );

    // The guard's own rejections, one mutation at a time: a substituted row the
    // pin would refuse, a removed document, and a table that describes another
    // artifact.
    let mut substituted = tables.clone();
    let bumped = substituted["documents"][0]["admitted"]
        .as_u64()
        .unwrap_or(0)
        + 1;
    substituted["documents"][0]["admitted"] = json!(bumped);
    let substituted_id = substituted["documents"][0]["article_id"]
        .as_str()
        .expect("article_id")
        .to_string();
    let substituted_bytes =
        serde_json::to_vec(&substituted).expect("serialize the substituted tables");
    assert_ne!(
        format!("{:x}", Sha256::digest(&substituted_bytes)),
        FROZEN_TABLES_SHA256,
        "a substituted recorded row must not keep the pinned digest"
    );
    assert!(
        check_frozen_tables_pin(&substituted_bytes).is_err(),
        "the pin must refuse a substituted recorded row"
    );
    let drift = frozen_tables_drift(&substituted, &artifact, &artifact_bytes);
    assert!(
        drift
            .iter()
            .any(|name| name == &format!("frozen_tables_row_drift:{substituted_id}")),
        "a substituted row must fail by name: {drift:#?}"
    );

    let mut removed = tables.clone();
    let removed_id = removed["documents"]
        .as_array_mut()
        .expect("documents")
        .pop()
        .expect("a recorded document")["article_id"]
        .as_str()
        .expect("article_id")
        .to_string();
    let drift = frozen_tables_drift(&removed, &artifact, &artifact_bytes);
    assert!(
        drift
            .iter()
            .any(|name| name == &format!("frozen_tables_missing:{removed_id}")),
        "a removed document must fail by name: {drift:#?}"
    );

    let mut foreign = tables.clone();
    foreign["artifact_sha256"] = json!("0".repeat(64));
    let drift = frozen_tables_drift(&foreign, &artifact, &artifact_bytes);
    assert!(
        drift
            .iter()
            .any(|name| name == "frozen_tables_artifact_sha256"),
        "a table that describes another artifact must fail by name: {drift:#?}"
    );
}
