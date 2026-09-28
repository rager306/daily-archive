//! Fail-closed contract for the human-readable GROBID Rust-vs-Java parity report.
//!
//! Slice: M513-r7dg4z/S02 (task T04). The report
//! (`doc/review/grobid-parity-report.md`) is the surface a reviewer reads; the
//! numbers live in `artifacts/parse_compat/external_holdout_release_probe.json`.
//! A readable report decays silently: a sentence can be edited while the machine
//! artifact still holds the measured value, and no reviewer recomputes 4.6 MB by
//! hand. This target makes the report's numbers a checked projection of the
//! artifact instead of a restatement:
//!
//!   * the aggregate micros are compared to
//!     `evaluated_population.class_parity.aggregate` exactly (integer identity,
//!     not a fuzzy float match), key set included;
//!   * the per-class rows are re-derived from the artifact's own
//!     `class_profiles[]` counts and compared exactly;
//!   * the per-class row sums must equal the aggregate counts, so a table that
//!     was edited to be internally consistent but externally wrong still fails;
//!   * the mandatory prose sections and the identity/digest/scope/route-boundary
//!     tokens must be present, because a report that drops the scope or the
//!     route boundary is exactly the overclaim this contract exists to refuse.
//!
//! The negative tests inject a drift into a cloned structure and assert it is
//! refused BY NAME, so the contract cannot pass vacuously: on the committed
//! report it must be clean, and on a mutated report it must name the mutation.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test parity_report_contract
//! cargo test -p da-parse --test parity_report_contract -- --nocapture
//! ```

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const REPORT_PATH: &str = "doc/review/grobid-parity-report.md";
const ARTIFACT_PATH: &str = "artifacts/parse_compat/external_holdout_release_probe.json";

const AGGREGATE_BEGIN: &str = "<!-- parity-aggregate:begin -->";
const AGGREGATE_END: &str = "<!-- parity-aggregate:end -->";
const PER_CLASS_BEGIN: &str = "<!-- parity-per-class:begin -->";
const PER_CLASS_END: &str = "<!-- parity-per-class:end -->";

/// The exact closed key set of `evaluated_population.class_parity.aggregate`.
const AGGREGATE_KEYS: [&str; 8] = [
    "denominator",
    "tp",
    "fn",
    "fp",
    "precision_micros",
    "native_capability_recall_micros",
    "covered_union_recall_micros",
    "f1_micros",
];

/// `(aggregate key, per-class row key)` pairs whose sums must agree.
const ROW_SUM_KEYS: [(&str, &str); 4] = [
    ("denominator", "source_population"),
    ("tp", "conserved"),
    ("fn", "missing"),
    ("fp", "fabricated"),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/da-parse")
        .to_path_buf()
}

fn read_report(root: &Path) -> String {
    std::fs::read_to_string(root.join(REPORT_PATH)).expect("parity report readable")
}

fn read_artifact(root: &Path) -> Value {
    let raw = std::fs::read_to_string(root.join(ARTIFACT_PATH)).expect("parity artifact readable");
    serde_json::from_str(&raw).expect("parity artifact parses")
}

/// The region between two markers, or a named error if either is absent.
fn region<'a>(text: &'a str, begin: &str, end: &str) -> Result<&'a str, String> {
    let start = text
        .find(begin)
        .ok_or_else(|| format!("report marker missing: {begin}"))?
        + begin.len();
    let stop = text[start..]
        .find(end)
        .ok_or_else(|| format!("report marker missing: {end}"))?
        + start;
    Ok(&text[start..stop])
}

/// Parse the single JSON value (object or array) enclosed by two markers. The
/// report wraps it in a fenced code block, so the region is trimmed to the first
/// opening bracket and the last closing bracket; anything else is a named error
/// rather than a silent `null`.
fn parse_marked_json(text: &str, begin: &str, end: &str) -> Result<Value, String> {
    let body = region(text, begin, end)?;
    let open = body
        .find(['{', '['])
        .ok_or_else(|| format!("no JSON opening bracket between {begin} and {end}"))?;
    let close = body
        .rfind(['}', ']'])
        .ok_or_else(|| format!("no JSON closing bracket between {begin} and {end}"))?;
    if close < open {
        return Err(format!(
            "JSON brackets out of order between {begin} and {end}"
        ));
    }
    serde_json::from_str(&body[open..=close])
        .map_err(|error| format!("JSON between {begin} and {end} does not parse: {error}"))
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

/// One re-derived per-class aggregate row (counts only, never a rate).
#[derive(Debug, PartialEq, Eq)]
struct ClassRow {
    class: String,
    source_population: u64,
    admitted: u64,
    conserved: u64,
    missing: u64,
    fabricated: u64,
    source_consumed: bool,
}

/// Re-derive the per-class counts from the artifact's own `class_profiles[]`.
///
/// The report must not be trusted for its own denominators: summing the
/// per-document rows is the independent derivation the artifact itself carries.
fn derive_class_rows(artifact: &Value) -> Result<Vec<ClassRow>, String> {
    let profiles = artifact
        .get("class_profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| "artifact.class_profiles missing".to_string())?;
    let mut order: Vec<String> = Vec::new();
    let mut acc: BTreeMap<String, ClassRow> = BTreeMap::new();
    for profile in profiles {
        let classes = profile
            .get("classes")
            .and_then(Value::as_array)
            .ok_or_else(|| "class_profiles[].classes missing".to_string())?;
        for entry in classes {
            let class = entry
                .get("class")
                .and_then(Value::as_str)
                .ok_or_else(|| "class entry missing `class`".to_string())?
                .to_string();
            let source_population = u64_field(entry, "source_population")
                .ok_or_else(|| format!("{class}: source_population missing"))?;
            let admitted =
                u64_field(entry, "admitted").ok_or_else(|| format!("{class}: admitted missing"))?;
            let conserved = u64_field(entry, "conserved")
                .ok_or_else(|| format!("{class}: conserved missing"))?;
            let fabricated = u64_field(entry, "fabricated")
                .ok_or_else(|| format!("{class}: fabricated missing"))?;
            let source_consumed = entry
                .get("source_consumed")
                .and_then(Value::as_bool)
                .ok_or_else(|| format!("{class}: source_consumed missing"))?;
            let row = acc.entry(class.clone()).or_insert_with(|| {
                order.push(class.clone());
                ClassRow {
                    class: class.clone(),
                    source_population: 0,
                    admitted: 0,
                    conserved: 0,
                    missing: 0,
                    fabricated: 0,
                    source_consumed,
                }
            });
            if row.source_consumed != source_consumed {
                return Err(format!(
                    "{class}: source_consumed mixed within the population"
                ));
            }
            row.source_population += source_population;
            row.admitted += admitted;
            row.conserved += conserved;
            row.fabricated += fabricated;
        }
    }
    for row in acc.values_mut() {
        row.missing = row.source_population - row.conserved;
    }
    Ok(order
        .into_iter()
        .map(|class| acc.remove(&class).expect("class inserted exactly once"))
        .collect())
}

/// Named errors for the report's aggregate block versus the artifact block.
fn aggregate_errors(report: &Value, artifact: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let (Some(report), Some(artifact)) = (report.as_object(), artifact.as_object()) else {
        errors.push("parity_report_aggregate_not_an_object".to_string());
        return errors;
    };
    for key in AGGREGATE_KEYS {
        match artifact.get(key).and_then(Value::as_u64) {
            Some(expected) => match report.get(key).and_then(Value::as_u64) {
                Some(actual) if actual == expected => {}
                Some(actual) => errors.push(format!(
                    "parity_report_aggregate_value_drift: key={key} report={actual} artifact={expected}"
                )),
                None => errors.push(format!(
                    "parity_report_aggregate_key_missing: key={key}"
                )),
            },
            None => errors.push(format!("artifact aggregate missing {key}")),
        }
    }
    for key in report.keys() {
        if !AGGREGATE_KEYS.contains(&key.as_str()) {
            errors.push(format!("parity_report_aggregate_key_unknown: key={key}"));
        }
    }
    errors
}

/// Named errors for the report's per-class rows versus re-derived rows.
fn per_class_errors(report_rows: &[Value], artifact_rows: &[ClassRow]) -> Vec<String> {
    let mut errors = Vec::new();
    if report_rows.len() != artifact_rows.len() {
        errors.push(format!(
            "parity_report_per_class_row_count: report={} artifact={}",
            report_rows.len(),
            artifact_rows.len()
        ));
    }
    for (index, artifact) in artifact_rows.iter().enumerate() {
        match report_rows.get(index) {
            None => errors.push(format!(
                "parity_report_per_class_row_missing: index={index} class={}",
                artifact.class
            )),
            Some(report) => check_class_row(report, artifact, &mut errors),
        }
    }
    errors
}

fn check_class_row(report: &Value, artifact: &ClassRow, errors: &mut Vec<String>) {
    let class = report
        .get("class")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    if class != artifact.class {
        errors.push(format!(
            "parity_report_per_class_class_drift: index report={class} artifact={}",
            artifact.class
        ));
    }
    for (key, expected) in [
        ("source_population", artifact.source_population),
        ("admitted", artifact.admitted),
        ("conserved", artifact.conserved),
        ("missing", artifact.missing),
        ("fabricated", artifact.fabricated),
    ] {
        match report.get(key).and_then(Value::as_u64) {
            Some(actual) if actual == expected => {}
            Some(actual) => errors.push(format!(
                "parity_report_per_class_value_drift: class={} key={key} report={actual} artifact={expected}",
                artifact.class
            )),
            None => errors.push(format!(
                "parity_report_per_class_key_missing: class={} key={key}",
                artifact.class
            )),
        }
    }
    match report.get("source_consumed").and_then(Value::as_bool) {
        Some(actual) if actual == artifact.source_consumed => {}
        Some(actual) => errors.push(format!(
            "parity_report_per_class_source_consumed_drift: class={} report={actual} artifact={}",
            artifact.class, artifact.source_consumed
        )),
        None => errors.push(format!(
            "parity_report_per_class_source_consumed_missing: class={}",
            artifact.class
        )),
    }
}

/// The per-class row sums must equal the aggregate counts, so a table edited to
/// be internally consistent but externally wrong is still refused.
fn row_sum_errors(report_rows: &[Value], artifact_aggregate: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    for (aggregate_key, row_key) in ROW_SUM_KEYS {
        let sum: u64 = report_rows
            .iter()
            .filter_map(|row| u64_field(row, row_key))
            .sum();
        match u64_field(artifact_aggregate, aggregate_key) {
            Some(expected) if expected == sum => {}
            Some(expected) => errors.push(format!(
                "parity_report_aggregate_row_sum_drift: {aggregate_key} rows={sum} aggregate={expected}"
            )),
            None => errors.push(format!(
                "parity_report_aggregate_key_missing: key={aggregate_key}"
            )),
        }
    }
    errors
}

fn missing_tokens(text: &str, tokens: &[&str]) -> Vec<String> {
    let mut missing = Vec::new();
    for token in tokens {
        if !text.contains(*token) {
            missing.push((*token).to_string());
        }
    }
    missing
}

fn report_text(root: &Path) -> String {
    read_report(root)
}

fn report_aggregate(report: &str) -> Value {
    parse_marked_json(report, AGGREGATE_BEGIN, AGGREGATE_END)
        .expect("report aggregate block parses")
}

fn report_rows(report: &str) -> Vec<Value> {
    parse_marked_json(report, PER_CLASS_BEGIN, PER_CLASS_END)
        .expect("report per-class block parses")
        .as_array()
        .expect("report per-class block is an array")
        .clone()
}

/// (a) Population identity and digest binding, not the version label.
#[test]
fn report_declares_population_identity_and_digest_binding() {
    let report = report_text(&workspace_root());
    let missing = missing_tokens(
        &report,
        &[
            "identity_digest",
            "canonical_dataset_id_plus_identity",
            "sha256:eee622ae57286faff567b1793f31ee54c5df3118e544a4bb8edf1c7376955c6d",
            "declared_release",
            "image_digest",
            "sha256:223957791ac2bbe48609dcc58a689b16b60baeae13a8734ef440ae6bfb38f4cd",
            "a937da95ef987dffdcde46b998e98549fe247999e50986e1b0627a6f485d8275",
            "d25a446a2aa1aec8fce09b988c812b95b965e98929147713cd90880fdb734ba2",
        ],
    );
    assert!(
        missing.is_empty(),
        "report missing identity/digest tokens: {missing:?}"
    );
}

/// (b) Declared scope, the independent judge, and the typed route boundary.
#[test]
fn report_declares_scope_oracle_and_route_boundary() {
    let report = report_text(&workspace_root());
    let missing = missing_tokens(
        &report,
        &[
            "local_proxy_only",
            "roxmltree",
            "external_holdout_probe_semantics.rs",
            "compose_native_fulltext",
            "corpus_n = 3",
            "PDF",
            "не измерен",
        ],
    );
    assert!(
        missing.is_empty(),
        "report missing scope/oracle/route-boundary tokens: {missing:?}"
    );
}

/// (c) The typed exception list with its three named reasons and counts, read
/// from the artifact rather than hardcoded in the report.
#[test]
fn report_names_the_typed_exceptions_with_their_reasons() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let exceptions = artifact
        .get("reference_population")
        .and_then(|v| v.get("exceptions"))
        .and_then(Value::as_array)
        .expect("reference_population.exceptions present");
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for exception in exceptions {
        let reason = exception
            .get("reason")
            .and_then(Value::as_str)
            .expect("exception reason")
            .to_string();
        *counts.entry(reason).or_default() += 1;
    }
    assert_eq!(exceptions.len(), 79, "declared exception denominator");
    assert_eq!(counts.get("header_date_absent").copied().unwrap_or(0), 72);
    assert_eq!(counts.get("header_title_absent").copied().unwrap_or(0), 5);
    assert_eq!(counts.get("header_author_absent").copied().unwrap_or(0), 2);

    let report = report_text(&root);
    let mut missing = missing_tokens(
        &report,
        &[
            "header_date_absent",
            "header_title_absent",
            "header_author_absent",
        ],
    );
    // Each reason must carry its measured count in the exception table rather
    // than only appearing as a bare token.
    for (reason, count) in [
        ("header_date_absent", 72usize),
        ("header_title_absent", 5usize),
        ("header_author_absent", 2usize),
    ] {
        let row = format!("| `{reason}` | {count} |");
        if !report.contains(&row) {
            missing.push(row);
        }
    }
    assert!(
        missing.is_empty(),
        "report missing typed exception rows: {missing:?}"
    );
}

/// (d) The per-class table is an exact projection of the artifact's own
/// `class_profiles[]` counts.
#[test]
fn report_per_class_rows_match_the_artifact_exactly() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let derived = derive_class_rows(&artifact).expect("per-class rows derive");
    let report = report_text(&root);
    let rows = report_rows(&report);
    let errors = per_class_errors(&rows, &derived);
    assert!(
        errors.is_empty(),
        "per-class drift:\n- {}",
        errors.join("\n- ")
    );
    assert_eq!(derived.len(), 10, "declared class denominator");
}

/// (e) The aggregate micros are compared to the artifact exactly, and the
/// per-class counts must sum to them.
#[test]
fn report_aggregate_matches_the_artifact_exactly() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let artifact_aggregate = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .and_then(|v| v.get("aggregate"))
        .expect("evaluated_population.class_parity.aggregate present");
    let report = report_text(&root);
    let aggregate = report_aggregate(&report);
    let errors = aggregate_errors(&aggregate, artifact_aggregate);
    assert!(
        errors.is_empty(),
        "aggregate drift:\n- {}",
        errors.join("\n- ")
    );

    let rows = report_rows(&report);
    let sum_errors = row_sum_errors(&rows, artifact_aggregate);
    assert!(
        sum_errors.is_empty(),
        "per-class sums drifted from the aggregate:\n- {}",
        sum_errors.join("\n- ")
    );
}

/// (f) The report must name the source-consumed rule and the H18/H21 open
/// items; a report that silently implies full coverage is refused.
#[test]
fn report_declares_source_consumed_rule_and_open_items() {
    let report = report_text(&workspace_root());
    let missing = missing_tokens(
        &report,
        &[
            "source_consumed",
            "SemanticFieldLoss::HeaderTitle",
            "SemanticFieldLoss::HeaderEmail",
            "SemanticFieldLoss::HeaderIds",
            "0.5000",
            "H18",
            "CitationFieldRole",
            "arxiv_id",
            "H21",
            "0.9.0-crf",
            "0.9.1",
            "recall_java",
            "precision",
            "f1",
            "claim_scope",
            "micro_occurrence",
            "79621",
            "74910",
            "4711",
            "945771",
            "947501",
            "972130",
            "1000000",
        ],
    );
    assert!(
        missing.is_empty(),
        "report missing source-consumed/open-item tokens: {missing:?}"
    );
}

/// Negative: every aggregate key mutation is refused by name. On the committed
/// report the same comparison is clean (proven in the test above), so a zero
/// here is a real zero.
#[test]
fn aggregate_value_mutation_is_rejected_for_every_key() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let artifact_aggregate = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .and_then(|v| v.get("aggregate"))
        .expect("artifact aggregate");
    let report = report_text(&root);
    let committed = report_aggregate(&report);
    assert!(
        aggregate_errors(&committed, artifact_aggregate).is_empty(),
        "non-vacuity: the committed aggregate must be clean"
    );

    for key in AGGREGATE_KEYS {
        let mut mutated = committed.clone();
        let expected = artifact_aggregate
            .get(key)
            .and_then(Value::as_u64)
            .expect("artifact key");
        mutated[key] = Value::from(expected + 1);
        let errors = aggregate_errors(&mutated, artifact_aggregate);
        assert!(
            errors
                .iter()
                .any(|error| error.contains(&format!("key={key}"))),
            "mutation of {key} must be refused by name: {errors:?}"
        );
    }
}

/// Negative: a dropped aggregate key is refused, not silently defaulted.
#[test]
fn dropped_aggregate_key_is_rejected() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let artifact_aggregate = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .and_then(|v| v.get("aggregate"))
        .expect("artifact aggregate");
    let report = report_text(&root);
    let mut mutated = report_aggregate(&report);
    mutated
        .as_object_mut()
        .expect("aggregate object")
        .remove("f1_micros");
    let errors = aggregate_errors(&mutated, artifact_aggregate);
    assert!(
        errors.iter().any(|error| error.contains("key=f1_micros")),
        "a dropped f1_micros key must be refused: {errors:?}"
    );
}

/// Negative: a per-class count mutation is refused by name, and the row-sum
/// consistency check also catches it.
#[test]
fn per_class_count_mutation_is_rejected() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let derived = derive_class_rows(&artifact).expect("per-class rows derive");
    let report = report_text(&root);
    let mut rows = report_rows(&report);

    let victim = rows
        .iter_mut()
        .find(|row| row.get("class").and_then(Value::as_str) == Some("AdmittedAuthorLine"))
        .expect("AdmittedAuthorLine row present");
    let conserved = u64_field(victim, "conserved").expect("conserved");
    let missing = u64_field(victim, "missing").expect("missing");
    victim["conserved"] = Value::from(conserved - 1);
    victim["missing"] = Value::from(missing + 1);

    let errors = per_class_errors(&rows, &derived);
    assert!(
        errors.iter().any(|error| error.contains("key=conserved")),
        "conserved drift must be refused by name: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| error.contains("key=missing")),
        "missing drift must be refused by name: {errors:?}"
    );

    // The same mutation also breaks the row sums against the committed
    // aggregate, so the consistency guard is non-vacuous.
    let artifact_aggregate = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .and_then(|v| v.get("aggregate"))
        .expect("artifact aggregate");
    let sum_errors = row_sum_errors(&rows, artifact_aggregate);
    assert!(
        !sum_errors.is_empty(),
        "the mutated row must also break the row sums"
    );
}

/// Negative: a reordered per-class table is refused, because the class identity
/// is part of the projection, not just the numbers.
#[test]
fn reordered_per_class_table_is_rejected() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let derived = derive_class_rows(&artifact).expect("per-class rows derive");
    let report = report_text(&root);
    let mut rows = report_rows(&report);
    rows.swap(0, 1);
    let errors = per_class_errors(&rows, &derived);
    assert!(
        errors
            .iter()
            .any(|error| error.starts_with("parity_report_per_class_class_drift")),
        "a reordered table must be refused by class identity: {errors:?}"
    );
}

/// Negative: an unknown aggregate key is refused, so the block stays a closed
/// set rather than an open bag.
#[test]
fn unknown_aggregate_key_is_rejected() {
    let root = workspace_root();
    let artifact = read_artifact(&root);
    let artifact_aggregate = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .and_then(|v| v.get("aggregate"))
        .expect("artifact aggregate");
    let report = report_text(&root);
    let mut mutated = report_aggregate(&report);
    mutated["totally_new_metric_micros"] = Value::from(1);
    let errors = aggregate_errors(&mutated, artifact_aggregate);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("key=totally_new_metric_micros")),
        "an unknown aggregate key must be refused: {errors:?}"
    );
}

/// The artifact declares `local_proxy_only` and the parity block is present;
/// the report is checked against that claimed scope rather than an invented one.
#[test]
fn artifact_parity_block_is_declared_local_proxy_only() {
    let artifact = read_artifact(&workspace_root());
    let parity = artifact
        .get("evaluated_population")
        .and_then(|v| v.get("class_parity"))
        .expect("evaluated_population.class_parity present");
    assert_eq!(
        parity.get("claim_scope").and_then(Value::as_str),
        Some("local_proxy_only")
    );
    let declaration = artifact
        .get("class_parity_declaration")
        .expect("class_parity_declaration present");
    assert_eq!(
        declaration.get("denominator").and_then(Value::as_str),
        Some("source_population")
    );
    assert_eq!(
        declaration.get("aggregation").and_then(Value::as_str),
        Some("micro_occurrence")
    );
}
