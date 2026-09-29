//! Fail-closed contract for the human-readable GROBID-retirement release-decision
//! package document.
//!
//! Slice: M513-r7dg4z/S07 (task T02). The package a human reads is
//! `doc/review/M513-S07-release-decision-package.md`; the measured values live in
//! the frozen carriers listed by
//! `artifacts/parse_compat/grobid_retirement_release_decision_package.json`.
//!
//! A prose package rots silently: a sentence can be edited while the carrier still
//! holds the measured value, and no reviewer recomputes the numbers by hand. This
//! target makes the document a *checked projection* of the carriers instead of a
//! restatement:
//!
//!   * every number the document presents lives in a marker-delimited JSON block
//!     and is compared to its carrier by integer identity (closed key set
//!     included), so a re-typed value is refused by name;
//!   * the remainders block must equal `artifact.remainders` as a set (id/state/
//!     pointer, no extras and no omissions), so a dropped or invented remainder
//!     fails;
//!   * the human record is re-read from `data/parse/acceptance_manifest.yaml`, so
//!     the document cannot declare a filled-in review while the live record is
//!     still `pending_human_review` with `reviewer: null`;
//!   * the required tokens must be present and decision-claiming tokens must be
//!     absent, because "the package prepares the decision, it does not make it" is
//!     the whole point of the artifact.
//!
//! The negative tests clone the committed document string in memory, mutate one
//! thing, and assert the named refusal, so the contract cannot pass vacuously.
//! Nothing here writes to disk: the document is a projection of the index, never
//! the other way round.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test grobid_retirement_package_doc_contract
//! cargo test -p da-parse --test grobid_retirement_package_doc_contract -- --nocapture
//! ```

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const DOC: &str = "doc/review/M513-S07-release-decision-package.md";
const PACKAGE: &str = "artifacts/parse_compat/grobid_retirement_release_decision_package.json";
const PROBE: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
const BUDGET: &str = "artifacts/parse_compat/parse_batch_budget_live.json";
const REHEARSAL: &str = "artifacts/parse_compat/native_family_promotion_rehearsal.json";
const MANIFEST: &str = "data/parse/acceptance_manifest.yaml";
const LEDGER: &str = "data/parse/grobid_compatibility.yaml";

const PARITY_AGGREGATE_BEGIN: &str = "<!-- parity-aggregate:begin -->";
const PARITY_AGGREGATE_END: &str = "<!-- parity-aggregate:end -->";
const PARITY_POPULATION_BEGIN: &str = "<!-- parity-population:begin -->";
const PARITY_POPULATION_END: &str = "<!-- parity-population:end -->";
const REMAINDERS_BEGIN: &str = "<!-- remainders:begin -->";
const REMAINDERS_END: &str = "<!-- remainders:end -->";
const ROLLBACK_REHEARSAL_BEGIN: &str = "<!-- rollback-rehearsal:begin -->";
const ROLLBACK_REHEARSAL_END: &str = "<!-- rollback-rehearsal:end -->";
const ROLLBACK_RELEASE_BEGIN: &str = "<!-- rollback-release:begin -->";
const ROLLBACK_RELEASE_END: &str = "<!-- rollback-release:end -->";
const RESOURCE_CEILINGS_BEGIN: &str = "<!-- resource-ceilings:begin -->";
const RESOURCE_CEILINGS_END: &str = "<!-- resource-ceilings:end -->";
const PACKAGE_STATE_BEGIN: &str = "<!-- package-state:begin -->";
const PACKAGE_STATE_END: &str = "<!-- package-state:end -->";

/// Closed key set of `evaluated_population.class_parity.aggregate`.
const AGGREGATE_KEYS: [&str; 8] = [
    "covered_union_recall_micros",
    "denominator",
    "f1_micros",
    "fn",
    "fp",
    "native_capability_recall_micros",
    "precision_micros",
    "tp",
];

/// Closed key set of the document's parity-population block.
const POPULATION_KEYS: [&str; 3] = ["declared", "measurable", "excluded"];

/// Closed key set of a remainders block entry (`decision_ref` is optional).
const REMAINDER_KEYS: [&str; 4] = ["id", "state", "pointer", "decision_ref"];

/// Closed key set of the document's rollback-rehearsal block.
const REHEARSAL_KEYS: [&str; 8] = [
    "documents_total",
    "documents_switched",
    "documents_refused",
    "rollback_byte_identical_all",
    "references_never_moved",
    "switch_and_return_non_vacuous",
    "carriers_unchanged",
    "claim_scope",
];

/// Closed key set of the document's rollback-release block.
const RELEASE_KEYS: [&str; 2] = ["readiness_doc", "readiness_doc_sha256"];

/// Closed key set of the document's resource-ceilings block.
const CEILING_KEYS: [&str; 8] = [
    "java_cpu_wall_clock_ns",
    "java_cpu_remainder",
    "latency_p50_ms",
    "latency_p95_ms",
    "latency_p99_ms",
    "max_reorder_depth",
    "peak_retained_bytes",
    "budget_waits",
];

/// Closed key set of the document's package-state block.
const PACKAGE_STATE_KEYS: [&str; 4] = [
    "package_status",
    "reviewer",
    "decision",
    "prose_alone_authorizes_release",
];

/// Tokens the document must carry verbatim, because dropping any one of them is
/// exactly the overclaim this contract exists to refuse.
const REQUIRED_TOKENS: [&str; 14] = [
    "pending_human_review",
    "reviewer=null",
    "decision=pending",
    "prose_alone_authorizes_release=false",
    "GAR-015",
    "H20",
    "rehearsal_only",
    "pooled_quality_claim: forbidden",
    "named_reviewer",
    "explicit_decision",
    "passing_quality_gate",
    "passing_cost_gate",
    "passing_rights_gate",
    "PENDING",
];

/// Tokens a document may not carry: they would claim a decision the human has not
/// made.
const FORBIDDEN_TOKENS: [&str; 3] = ["approved", "decision is recorded", "release authorized"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/da-parse")
        .to_path_buf()
}

fn read_text(root: &Path, rel: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read `{rel}`: {e}"))
}

fn read_json(root: &Path, rel: &str) -> Result<Value, String> {
    let text = read_text(root, rel)?;
    serde_json::from_str(&text).map_err(|e| format!("`{rel}` is not valid JSON: {e}"))
}

fn read_yaml(root: &Path, rel: &str) -> Result<Value, String> {
    let text = read_text(root, rel)?;
    serde_yaml::from_str(&text).map_err(|e| format!("`{rel}` is not valid YAML: {e}"))
}

fn dig<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

// ─────────────────────────── marker plumbing ───────────────────────────

/// The region between two markers, or a named refusal if either is absent.
fn region<'a>(text: &'a str, begin: &str, end: &str) -> Result<&'a str, String> {
    let start = text
        .find(begin)
        .ok_or_else(|| format!("document_marker_missing: {begin}"))?
        + begin.len();
    let stop = text[start..]
        .find(end)
        .ok_or_else(|| format!("document_marker_missing: {end}"))?
        + start;
    Ok(&text[start..stop])
}

/// Parse the single JSON value enclosed by two markers. The document wraps it in
/// a fenced code block, so the region is trimmed to the first opening bracket and
/// the last closing bracket; anything else is a named refusal, never a silent
/// `null`.
fn parse_marked_json(text: &str, begin: &str, end: &str) -> Result<Value, String> {
    let body = region(text, begin, end)?;
    let open = body
        .find(['{', '['])
        .ok_or_else(|| format!("document_block_has_no_json: {begin}"))?;
    let close = body
        .rfind(['}', ']'])
        .ok_or_else(|| format!("document_block_has_no_json: {begin}"))?;
    if close < open {
        return Err(format!("document_block_brackets_out_of_order: {begin}"));
    }
    serde_json::from_str(&body[open..=close])
        .map_err(|e| format!("document_block_json_does_not_parse: {begin}: {e}"))
}

fn marked_json(text: &str, begin: &str, end: &str, errors: &mut Vec<String>) -> Option<Value> {
    match parse_marked_json(text, begin, end) {
        Ok(value) => Some(value),
        Err(error) => {
            errors.push(error);
            None
        }
    }
}

// ─────────────────────────── value comparison ──────────────────────────

/// Compare a document block to its carrier as an exact, closed-key projection.
fn check_closed_object(
    actual: &Value,
    expected: &Value,
    allowed: &[&str],
    label: &str,
    errors: &mut Vec<String>,
) {
    let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) else {
        errors.push(format!("{label}_not_an_object"));
        return;
    };
    for key in allowed {
        match (actual.get(*key), expected.get(*key)) {
            (Some(a), Some(e)) if a == e => {}
            (Some(_), Some(_)) => errors.push(format!("{label}_value_drift: key={key}")),
            (None, Some(_)) => errors.push(format!("{label}_key_missing: key={key}")),
            (_, None) => errors.push(format!("carrier_key_missing: {label}.{key}")),
        }
    }
    for key in actual.keys() {
        if !allowed.contains(&key.as_str()) {
            errors.push(format!("{label}_key_unknown: key={key}"));
        }
    }
}

/// Build an expected object from carrier nodes, refusing a missing carrier field
/// rather than defaulting it to `null`.
fn build_object(spec: &[(&str, Option<&Value>)], label: &str, errors: &mut Vec<String>) -> Value {
    let mut map = Map::new();
    for (key, value) in spec {
        match value {
            Some(value) => {
                map.insert((*key).to_string(), (*value).clone());
            }
            None => errors.push(format!("carrier_key_missing: {label}.{key}")),
        }
    }
    Value::Object(map)
}

fn derived_population(probe: &Value, errors: &mut Vec<String>) -> Value {
    let rp = probe.get("reference_population");
    build_object(
        &[
            ("declared", rp.and_then(|v| v.get("documents"))),
            ("measurable", rp.and_then(|v| v.get("measurable_documents"))),
            ("excluded", rp.and_then(|v| v.get("excluded_documents"))),
        ],
        "reference_population",
        errors,
    )
}

fn derived_rehearsal(rehearsal: &Value, errors: &mut Vec<String>) -> Value {
    let r = rehearsal.get("rehearsal");
    build_object(
        &[
            ("documents_total", r.and_then(|v| v.get("documents_total"))),
            (
                "documents_switched",
                r.and_then(|v| v.get("documents_switched")),
            ),
            (
                "documents_refused",
                r.and_then(|v| v.get("documents_refused")),
            ),
            (
                "rollback_byte_identical_all",
                r.and_then(|v| v.get("rollback_byte_identical_all")),
            ),
            (
                "references_never_moved",
                r.and_then(|v| v.get("references_never_moved")),
            ),
            (
                "switch_and_return_non_vacuous",
                r.and_then(|v| v.get("switch_and_return_non_vacuous")),
            ),
            (
                "carriers_unchanged",
                r.and_then(|v| v.get("carriers_unchanged")),
            ),
            ("claim_scope", rehearsal.get("claim_scope")),
        ],
        "rehearsal",
        errors,
    )
}

fn derived_ceilings(budget: &Value, errors: &mut Vec<String>) -> Value {
    let obs = budget.get("observations");
    let observed = |key: &str| obs.and_then(|o| o.get(key)).and_then(|o| o.get("value"));
    build_object(
        &[
            ("java_cpu_wall_clock_ns", observed("java_cpu_wall_clock_ns")),
            ("java_cpu_remainder", budget.get("java_cpu_remainder")),
            ("latency_p50_ms", observed("latency_p50_ms")),
            ("latency_p95_ms", observed("latency_p95_ms")),
            ("latency_p99_ms", observed("latency_p99_ms")),
            ("max_reorder_depth", observed("max_reorder_depth")),
            ("peak_retained_bytes", observed("peak_retained_bytes")),
            ("budget_waits", observed("budget_waits")),
        ],
        "observations",
        errors,
    )
}

fn derived_package_state(manifest: &Value, errors: &mut Vec<String>) -> Value {
    let rdp = manifest.get("release_decision_package");
    let hrr = rdp.and_then(|v| v.get("human_release_review"));
    let (Some(rdp), Some(hrr)) = (rdp, hrr) else {
        errors.push("carrier_key_missing: release_decision_package.human_release_review".into());
        return Value::Object(Map::new());
    };
    build_object(
        &[
            ("package_status", rdp.get("package_status")),
            ("reviewer", hrr.get("reviewer")),
            ("decision", hrr.get("decision")),
            (
                "prose_alone_authorizes_release",
                hrr.get("prose_alone_authorizes_release"),
            ),
        ],
        "human_release_review",
        errors,
    )
}

// ─────────────────────────── document checks ───────────────────────────

fn check_remainders(text: &str, package: &Value, errors: &mut Vec<String>) {
    let Some(actual) = marked_json(text, REMAINDERS_BEGIN, REMAINDERS_END, errors) else {
        return;
    };
    let Some(expected) = package.get("remainders").and_then(Value::as_array) else {
        errors.push("carrier_key_missing: package.remainders".into());
        return;
    };
    let Some(actual) = actual.as_array() else {
        errors.push("remainders_not_an_array".into());
        return;
    };
    let mut expected_by_id: BTreeMap<String, Value> = BTreeMap::new();
    for entry in expected {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            expected_by_id.insert(id.to_string(), entry.clone());
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for entry in actual {
        if let Some(map) = entry.as_object() {
            for key in map.keys() {
                if !REMAINDER_KEYS.contains(&key.as_str()) {
                    errors.push(format!("remainders_key_unknown: key={key}"));
                }
            }
        }
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            errors.push("remainders_id_missing".into());
            continue;
        };
        if !seen.insert(id.to_string()) {
            errors.push(format!("remainders_duplicate_id: id={id}"));
        }
        let Some(expected_entry) = expected_by_id.get(id) else {
            errors.push(format!("remainders_unknown_in_document: id={id}"));
            continue;
        };
        for key in ["state", "pointer"] {
            match (entry.get(key), expected_entry.get(key)) {
                (Some(a), Some(e)) if a == e => {}
                (Some(_), Some(_)) => {
                    errors.push(format!("remainders_value_drift: id={id} key={key}"));
                }
                (None, Some(_)) => {
                    errors.push(format!("remainders_key_missing: id={id} key={key}"));
                }
                (_, None) => {
                    errors.push(format!("carrier_key_missing: remainder.{id}.{key}"));
                }
            }
        }
        match (
            entry.get("decision_ref"),
            expected_entry.get("decision_ref"),
        ) {
            (None, None) => {}
            (Some(a), Some(e)) if a == e => {}
            _ => errors.push(format!("remainders_value_drift: id={id} key=decision_ref")),
        }
    }
    for id in expected_by_id.keys() {
        if !seen.contains(id) {
            errors.push(format!("remainders_missing_in_document: id={id}"));
        }
    }
}

fn check_rollback_release(text: &str, package: &Value, errors: &mut Vec<String>) {
    let Some(actual) = marked_json(text, ROLLBACK_RELEASE_BEGIN, ROLLBACK_RELEASE_END, errors)
    else {
        return;
    };
    let binding = package
        .get("source_bindings")
        .and_then(Value::as_array)
        .and_then(|bindings| {
            bindings.iter().find(|b| {
                b.get("role").and_then(Value::as_str) == Some("rollback_release_readiness")
            })
        });
    let (Some(path), Some(sha)) = (
        binding.and_then(|b| b.get("path")),
        binding.and_then(|b| b.get("sha256")),
    ) else {
        errors
            .push("carrier_key_missing: package.source_bindings.rollback_release_readiness".into());
        return;
    };
    let expected = build_object(
        &[
            ("readiness_doc", Some(path)),
            ("readiness_doc_sha256", Some(sha)),
        ],
        "rollback_release_readiness",
        errors,
    );
    check_closed_object(
        &actual,
        &expected,
        &RELEASE_KEYS,
        "rollback_release",
        errors,
    );
}

fn check_source_table(text: &str, package: &Value, errors: &mut Vec<String>) {
    let Some(bindings) = package.get("source_bindings").and_then(Value::as_array) else {
        errors.push("carrier_key_missing: package.source_bindings".into());
        return;
    };
    for binding in bindings {
        let path = binding.get("path").and_then(Value::as_str).unwrap_or("");
        let sha = binding.get("sha256").and_then(Value::as_str).unwrap_or("");
        if path.is_empty() || sha.is_empty() {
            errors.push("carrier_key_missing: package.source_bindings[]".into());
            continue;
        }
        if !text.contains(path) {
            errors.push(format!("source_binding_path_missing: {path}"));
        }
        if !text.contains(sha) {
            errors.push(format!("source_binding_sha_missing: {path}"));
        }
    }
}

fn check_tokens(text: &str, errors: &mut Vec<String>) {
    for token in REQUIRED_TOKENS {
        if !text.contains(token) {
            errors.push(format!("document_required_token_missing: {token}"));
        }
    }
    for token in FORBIDDEN_TOKENS {
        if text.contains(token) {
            errors.push(format!("document_forbidden_token_present: {token}"));
        }
    }
}

/// Counts of `parity_ledger.absent_dispositions` by disposition.
fn absent_disposition_counts(ledger: &Value) -> Option<(usize, usize, usize)> {
    let dispositions = dig(ledger, &["parity_ledger", "absent_dispositions"])?.as_object()?;
    let mut out_of_product_scope = 0usize;
    let mut in_horizon_project_native = 0usize;
    for entry in dispositions.values() {
        match entry.get("disposition").and_then(Value::as_str) {
            Some("out_of_product_scope") => out_of_product_scope += 1,
            Some("in_horizon_project_native") => in_horizon_project_native += 1,
            _ => {}
        }
    }
    Some((
        dispositions.len(),
        out_of_product_scope,
        in_horizon_project_native,
    ))
}

/// The full document contract. Empty result means the document is a clean
/// projection of the committed carriers.
fn validate_document(text: &str, root: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    let (package, probe, budget, rehearsal, manifest, ledger) = match (
        read_json(root, PACKAGE),
        read_json(root, PROBE),
        read_json(root, BUDGET),
        read_json(root, REHEARSAL),
        read_yaml(root, MANIFEST),
        read_yaml(root, LEDGER),
    ) {
        (Ok(package), Ok(probe), Ok(budget), Ok(rehearsal), Ok(manifest), Ok(ledger)) => {
            (package, probe, budget, rehearsal, manifest, ledger)
        }
        (package, probe, budget, rehearsal, manifest, ledger) => {
            for carrier in [package, probe, budget, rehearsal, manifest, ledger] {
                if let Err(error) = carrier {
                    errors.push(error);
                }
            }
            return errors;
        }
    };

    // (a) parity aggregate and population, exact integer identity.
    if let Some(actual) = marked_json(
        text,
        PARITY_AGGREGATE_BEGIN,
        PARITY_AGGREGATE_END,
        &mut errors,
    ) {
        let expected = dig(
            &probe,
            &["evaluated_population", "class_parity", "aggregate"],
        );
        match expected {
            Some(expected) => check_closed_object(
                &actual,
                expected,
                &AGGREGATE_KEYS,
                "parity_aggregate",
                &mut errors,
            ),
            None => errors.push("carrier_key_missing: class_parity.aggregate".into()),
        }
    }
    if let Some(actual) = marked_json(
        text,
        PARITY_POPULATION_BEGIN,
        PARITY_POPULATION_END,
        &mut errors,
    ) {
        let expected = derived_population(&probe, &mut errors);
        check_closed_object(
            &actual,
            &expected,
            &POPULATION_KEYS,
            "parity_population",
            &mut errors,
        );
    }
    // The declared measurement scope travels with the numbers, not by memory.
    let scope_missing = dig(
        &probe,
        &["evaluated_population", "class_parity", "claim_scope"],
    )
    .and_then(Value::as_str)
    .filter(|scope| !text.contains(*scope));
    if let Some(scope) = scope_missing {
        errors.push(format!("document_claim_scope_missing: {scope}"));
    }
    // The population is also named in prose; the two must agree.
    if let (Some(declared), Some(measurable)) = (
        dig(&probe, &["reference_population", "documents"]).and_then(Value::as_u64),
        dig(&probe, &["reference_population", "measurable_documents"]).and_then(Value::as_u64),
    ) {
        let needle = format!("объявлено {declared}, измеримо {measurable}");
        if !text.contains(&needle) {
            errors.push(format!("parity_population_prose_missing: {needle}"));
        }
    }

    // (b) remainders.
    check_remainders(text, &package, &mut errors);
    if let Some((total, out_of_product_scope, in_horizon_project_native)) =
        absent_disposition_counts(&ledger)
    {
        for needle in [
            format!("{total} записей"),
            format!("{out_of_product_scope} `out_of_product_scope`"),
            format!("{in_horizon_project_native} `in_horizon_project_native`"),
        ] {
            if !text.contains(&needle) {
                errors.push(format!("absent_dispositions_prose_missing: {needle}"));
            }
        }
    } else {
        errors.push("carrier_key_missing: parity_ledger.absent_dispositions".into());
    }

    // (c) proven rollback.
    if let Some(actual) = marked_json(
        text,
        ROLLBACK_REHEARSAL_BEGIN,
        ROLLBACK_REHEARSAL_END,
        &mut errors,
    ) {
        let expected = derived_rehearsal(&rehearsal, &mut errors);
        check_closed_object(
            &actual,
            &expected,
            &REHEARSAL_KEYS,
            "rollback_rehearsal",
            &mut errors,
        );
    }
    check_rollback_release(text, &package, &mut errors);

    // (d) resource ceilings and the Java-CPU remainder.
    if let Some(actual) = marked_json(
        text,
        RESOURCE_CEILINGS_BEGIN,
        RESOURCE_CEILINGS_END,
        &mut errors,
    ) {
        let expected = derived_ceilings(&budget, &mut errors);
        check_closed_object(
            &actual,
            &expected,
            &CEILING_KEYS,
            "resource_ceilings",
            &mut errors,
        );
    }
    let counter_scope_missing = dig(&budget, &["java_cpu_remainder", "counter_scope"])
        .and_then(Value::as_str)
        .filter(|scope| !text.contains(*scope));
    if let Some(counter_scope) = counter_scope_missing {
        errors.push(format!(
            "resource_ceilings_counter_scope_missing: {counter_scope}"
        ));
    }

    // (e) the human record is a pointer to authority.
    if let Some(actual) = marked_json(text, PACKAGE_STATE_BEGIN, PACKAGE_STATE_END, &mut errors) {
        let expected = derived_package_state(&manifest, &mut errors);
        check_closed_object(
            &actual,
            &expected,
            &PACKAGE_STATE_KEYS,
            "package_state",
            &mut errors,
        );
    }

    // Binding table, mandatory tokens, and the absence of a decision claim.
    check_source_table(text, &package, &mut errors);
    check_tokens(text, &mut errors);

    errors
}

/// Human-readable parse-out of the projection (visible with `--nocapture`).
fn print_breakdown(text: &str, root: &Path) {
    let package = read_json(root, PACKAGE).expect("package readable");
    println!("[package-doc] {DOC}");
    if let Some(bindings) = package.get("source_bindings").and_then(Value::as_array) {
        for binding in bindings {
            let role = binding.get("role").and_then(Value::as_str).unwrap_or("?");
            let path = binding.get("path").and_then(Value::as_str).unwrap_or("?");
            let sha = binding.get("sha256").and_then(Value::as_str).unwrap_or("?");
            let prefix = &sha[..sha.len().min(12)];
            println!("  binding {role} -> {path} sha={prefix}");
        }
    }
    if let Some(rows) = parse_marked_json(text, REMAINDERS_BEGIN, REMAINDERS_END)
        .ok()
        .and_then(|value| value.as_array().cloned())
    {
        println!("  remainders: {}", rows.len());
        for row in &rows {
            println!(
                "    {} state={}",
                row.get("id").and_then(Value::as_str).unwrap_or("?"),
                row.get("state").and_then(Value::as_str).unwrap_or("?"),
            );
        }
    }
    println!(
        "  required tokens present: {}",
        REQUIRED_TOKENS
            .iter()
            .filter(|token| text.contains(**token))
            .count()
    );
}

// ─────────────────────────────── tests ─────────────────────────────────

/// Positive control: the committed document is a clean projection, and the
/// projection is non-vacuous (it really carries the carriers' values).
#[test]
fn committed_document_is_a_clean_projection_of_the_carriers() {
    let root = root();
    let text = read_text(&root, DOC).expect("document readable");
    print_breakdown(&text, &root);

    let errors = validate_document(&text, &root);
    assert!(
        errors.is_empty(),
        "document drift:\n- {}",
        errors.join("\n- ")
    );

    // Non-vacuity: the expected values are not trivially empty.
    let aggregate =
        parse_marked_json(&text, PARITY_AGGREGATE_BEGIN, PARITY_AGGREGATE_END).expect("aggregate");
    assert_eq!(
        aggregate.as_object().map(Map::len),
        Some(AGGREGATE_KEYS.len()),
        "the aggregate block must carry the closed eight-key set"
    );
    let remainders =
        parse_marked_json(&text, REMAINDERS_BEGIN, REMAINDERS_END).expect("remainders");
    let package = read_json(&root, PACKAGE).expect("package readable");
    assert_eq!(
        remainders.as_array().map(Vec::len),
        package
            .get("remainders")
            .and_then(Value::as_array)
            .map(Vec::len),
        "the remainders block must carry exactly the package's remainders"
    );
    assert!(
        text.contains("0.9.0-crf"),
        "the accepted H21 residual must name the rollback label"
    );
}

/// The document is a projection of the index, never the other way round: the
/// index does not cite the document, and validation does not touch the index.
#[test]
fn document_is_a_projection_and_the_index_does_not_cite_it() {
    let root = root();
    let package_text = read_text(&root, PACKAGE).expect("package readable");
    let before = std::fs::read_to_string(root.join(PACKAGE)).expect("package readable");

    let text = read_text(&root, DOC).expect("document readable");
    let _ = validate_document(&text, &root);

    let after = std::fs::read_to_string(root.join(PACKAGE)).expect("package readable");
    assert_eq!(before, after, "validation must not rewrite the index");
    assert!(
        !package_text.contains("M513-S07-release-decision-package.md"),
        "the index must not cite the document it indexes: no circular projection"
    );
}

/// Negative: each in-memory mutation of the committed document is refused by
/// name. The positive control above proves a zero here would be a real zero.
#[test]
fn mutated_documents_are_refused_by_name() {
    let root = root();
    let committed = read_text(&root, DOC).expect("document readable");
    let probe = read_json(&root, PROBE).expect("probe readable");
    let f1 = dig(
        &probe,
        &[
            "evaluated_population",
            "class_parity",
            "aggregate",
            "f1_micros",
        ],
    )
    .and_then(Value::as_u64)
    .expect("carrier f1_micros");
    let measurable = dig(&probe, &["reference_population", "measurable_documents"])
        .and_then(Value::as_u64)
        .expect("carrier measurable_documents");
    let package = read_json(&root, PACKAGE).expect("package readable");
    let probe_sha = package
        .get("source_bindings")
        .and_then(Value::as_array)
        .and_then(|bindings| {
            bindings
                .iter()
                .find(|b| b.get("role").and_then(Value::as_str) == Some("parity_measurement"))
        })
        .and_then(|b| b.get("sha256"))
        .and_then(Value::as_str)
        .expect("carrier parity_measurement binding")
        .to_string();
    let mut mutated_sha = probe_sha.clone();
    mutated_sha.pop();
    mutated_sha.push(if probe_sha.ends_with('5') { '6' } else { '5' });

    let cases: Vec<(&str, String, String, String)> = vec![
        (
            "parity aggregate number",
            format!("\"f1_micros\": {f1}"),
            format!("\"f1_micros\": {}", f1 + 1),
            "parity_aggregate_value_drift: key=f1_micros".into(),
        ),
        (
            "parity population number",
            format!("\"measurable\": {measurable}"),
            format!("\"measurable\": {}", measurable + 1),
            "parity_population_value_drift: key=measurable".into(),
        ),
        (
            "remainders marker",
            REMAINDERS_BEGIN.into(),
            String::new(),
            format!("document_marker_missing: {REMAINDERS_BEGIN}"),
        ),
        (
            "remainders entry state",
            "\"id\": \"h20_frozen_external_holdout_decision\",\n    \"state\": \"pending\"".into(),
            "\"id\": \"h20_frozen_external_holdout_decision\",\n    \"state\": \"resolved\"".into(),
            "remainders_value_drift: id=h20_frozen_external_holdout_decision key=state".into(),
        ),
        (
            "rehearsal scope",
            "\"claim_scope\": \"rehearsal_only\"".into(),
            "\"claim_scope\": \"production\"".into(),
            "rollback_rehearsal_value_drift: key=claim_scope".into(),
        ),
        (
            "ceilings budget_waits",
            "\"budget_waits\": 2".into(),
            "\"budget_waits\": 3".into(),
            "resource_ceilings_value_drift: key=budget_waits".into(),
        ),
        (
            "package-state reviewer",
            "\"reviewer\": null".into(),
            "\"reviewer\": \"M. Reviewer\"".into(),
            "package_state_value_drift: key=reviewer".into(),
        ),
        (
            "required reviewer token",
            "reviewer=null".into(),
            "reviewer=Jane Doe".into(),
            "document_required_token_missing: reviewer=null".into(),
        ),
        (
            "pooled quality claim token",
            "pooled_quality_claim: forbidden".into(),
            "pooled_quality_claim: allowed".into(),
            "document_required_token_missing: pooled_quality_claim: forbidden".into(),
        ),
        (
            "forbidden decision claim",
            "## (e) Что заполняет ЧЕЛОВЕК".into(),
            "release authorized\n\n## (e) Что заполняет ЧЕЛОВЕК".into(),
            "document_forbidden_token_present: release authorized".into(),
        ),
        (
            "binding sha",
            probe_sha,
            mutated_sha,
            "source_binding_sha_missing: artifacts/parse_compat/external_holdout_release_probe.json"
                .into(),
        ),
        (
            "aggregate unknown key",
            format!("\"f1_micros\": {f1},"),
            format!("\"f1_micros\": {f1},\n  \"totally_new_metric_micros\": 1,"),
            "parity_aggregate_key_unknown: key=totally_new_metric_micros".into(),
        ),
        (
            "aggregate dropped key",
            "\"fp\": 0,\n".into(),
            String::new(),
            "parity_aggregate_key_missing: key=fp".into(),
        ),
        (
            "package-state dropped key",
            "\"decision\": \"pending\",".into(),
            String::new(),
            "package_state_key_missing: key=decision".into(),
        ),
    ];

    for (label, old, new, needle) in cases {
        // Replace every occurrence: a token written twice (e.g. `reviewer=null`
        // in the header and again in the closing paragraph) must not survive a
        // single-shot mutation and make the negative vacuous.
        let mutated = committed.replace(&old, &new);
        assert_ne!(
            mutated, committed,
            "mutation `{label}` did not change the document"
        );
        let errors = validate_document(&mutated, &root);
        assert!(
            errors.iter().any(|error| error.contains(&needle)),
            "mutation `{label}` must be refused by `{needle}`; got {errors:?}"
        );
    }

    // Removing every PENDING marker must fail too: a package that hides which
    // fields a human still has to fill in is the failure this contract forbids.
    let no_pending = committed.replace("PENDING", "");
    let errors = validate_document(&no_pending, &root);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("document_required_token_missing: PENDING")),
        "removing the PENDING rows must be refused; got {errors:?}"
    );
}

/// Negative: dropping a whole section removes its marker and must be refused.
#[test]
fn section_removal_is_refused() {
    let root = root();
    let committed = read_text(&root, DOC).expect("document readable");
    let start = committed
        .find("## (e) Что заполняет ЧЕЛОВЕК")
        .expect("the human section is present");
    let truncated = committed[..start].to_string();
    let errors = validate_document(&truncated, &root);
    assert!(
        errors
            .iter()
            .any(|error| error.contains(&format!("document_marker_missing: {PACKAGE_STATE_BEGIN}"))),
        "a removed section must be refused by its missing marker; got {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("document_required_token_missing: PENDING")),
        "a removed section must be refused by its missing PENDING token; got {errors:?}"
    );
}
