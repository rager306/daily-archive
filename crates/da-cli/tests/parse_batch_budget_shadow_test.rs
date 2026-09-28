//! S06/T05 (M511-j7wz2f) — form and invariant pin for the measured batch-budget
//! shadow artifact `artifacts/parse_compat/parse_batch_budget_shadow.json`.
//!
//! The artifact is MEASURED: it carries real wall-clock durations, so it cannot
//! be byte-stable and this test deliberately does not pin its bytes. That is a
//! named, honest boundary (S04/T05 lesson). What is pinned is the SHAPE and the
//! INVARIANTS, bound to the live `da_parse` DATA vocabulary
//! (`BATCH_BUDGET_METRICS`, `BATCH_OBSERVATION_STATES`, `BATCH_BUDGET_VERDICTS`,
//! `BATCH_BUDGET_H05_CLOSURE`) so a drift cannot pass silently:
//!
//! * the GAR-006 reference/executed/evaluated split is present, distinguishable
//!   and free of the forbidden legacy `denominators`/`corpus` blocks;
//! * every metric of the closed DATA set is present exactly once, and every
//!   observation is typed: `observed` carries an integer and no reason,
//!   `not_observed` carries a non-empty reason and NO numeric field;
//! * a zero is never published as a process measurement (H19-N1);
//! * retained bytes are additive: the window peak can never fall below the sum
//!   of the batch's known successful outcome sizes (H05-N2);
//! * H05 is never `complete` without measured RSS + Java CPU + latency tails;
//! * every policy lock holds;
//! * the serialized report leaks no raw payload, physical path, host identity or
//!   executable location.
//!
//! Three tests: the committed artifact, a freshly regenerated artifact (so a
//! stale committed copy is caught), and the example's own fail-closed
//! self-check (so the guards are proven to FIRE, not merely to exist).

use da_parse::{
    BATCH_BUDGET_H05_CLOSURE, BATCH_BUDGET_METRICS, BATCH_BUDGET_VERDICTS, BATCH_OBSERVATION_STATES,
};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// The committed runtime artifact under test.
const ARTIFACT: &str = "artifacts/parse_compat/parse_batch_budget_shadow.json";
const SCHEMA_VERSION: &str = "parse-batch-budget-shadow.v1";
const LIVE_SCHEMA_VERSION: &str = "parse-batch-budget-live.v1";
const STUB_SUBSTRATE_KIND: &str = "local_deterministic_loopback_stub";
const LIVE_SUBSTRATE_KIND: &str = "live_grobid_http";
const REFERENCE_POPULATION_ROLE: &str = "identity_only";
const EXECUTION_EVIDENCE_KIND: &str = "offline_loopback_stub_existing_typed_transport";
const LIVE_EXECUTION_EVIDENCE_KIND: &str = "live_grobid_http_structural";
const PROCESS_METRICS: [&str; 2] = ["peak_rss_bytes", "java_cpu_wall_clock_ns"];
const H05_REQUIRED_MEASUREMENTS: [&str; 3] =
    ["peak_rss_bytes", "java_cpu_wall_clock_ns", "latency_tails"];
const LATENCY_TAIL_METRICS: [&str; 3] = ["latency_p50_ms", "latency_p95_ms", "latency_p99_ms"];
const FORBIDDEN_REPORT_TOKENS: [&str; 15] = [
    "<TEI",
    "<tei",
    "%PDF-",
    "data/sources",
    "file://",
    "/root/",
    "/home/",
    "target/debug",
    "target/release",
    "executable",
    "hostname",
    "/sys/",
    "docker-",
    ".scope",
    "daily-archive-grobid",
];
/// Mutations the example's `--self-check` must prove it rejects.
const REQUIRED_SELF_CHECK_GUARDS: [&str; 14] = [
    "zero-rss-published-as-measurement",
    "not-observed-without-reason",
    "not-observed-with-numeric-field",
    "non-additive-retained-bytes",
    "incomplete-metric-set",
    "missing-gar006-split",
    "forbidden-token-in-report",
    "h05-complete-without-java-cpu",
    "policy-lock-lifted",
    "physical-path-in-report",
    "live-grobid-participation-without-observed-java-cpu",
    "java-cpu-remainder-observed-without-numbers",
    "java-cpu-remainder-ratio-not-positive",
    "java-cpu-remainder-not-observed-with-numeric-field",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn run_example(arguments: &[&str]) -> std::process::Output {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "-q",
            "-p",
            "da-cli",
            "--example",
            "parse_batch_budget_shadow",
            "--",
        ])
        .args(arguments)
        .output()
        .expect("run parse_batch_budget_shadow example")
}

fn object<'a>(report: &'a Value, key: &str) -> Result<&'a Map<String, Value>, String> {
    report
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("`{key}` block is missing or not an object"))
}

fn non_empty_string(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("`{key}` must be a non-empty string"))
}

fn count(object: &Map<String, Value>, key: &str) -> Result<u64, String> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("`{key}` must be an integer count"))
}

fn state_of(report: &Value, metric: &str) -> Option<String> {
    report
        .get("observations")?
        .get(metric)?
        .get("state")?
        .as_str()
        .map(str::to_string)
}

fn push(result: Result<(), String>, findings: &mut Vec<String>) {
    if let Err(finding) = result {
        findings.push(finding);
    }
}

fn host_tokens() -> Vec<String> {
    let mut tokens = Vec::new();
    for key in ["HOSTNAME", "HOST"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() && trimmed != "localhost" {
                tokens.push(trimmed.to_string());
            }
        }
    }
    for path in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let trimmed = text.trim();
            if !trimmed.is_empty()
                && trimmed != "localhost"
                && !tokens.contains(&trimmed.to_string())
            {
                tokens.push(trimmed.to_string());
            }
        }
    }
    tokens
}

/// Independent audit of one report: every returned string is a violation, so an
/// empty vector means the artifact holds every invariant this task claims.
fn audit(report: &Value) -> Vec<String> {
    let mut findings = Vec::new();

    // ── substrate + schema, checked before anything substrate-shaped ────
    let kind = report
        .get("substrate")
        .and_then(|substrate| substrate.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let live = kind.as_deref() == Some(LIVE_SUBSTRATE_KIND);
    match (&kind, report.get("schema_version").and_then(Value::as_str)) {
        (Some(kind), Some(schema)) if kind == STUB_SUBSTRATE_KIND && schema == SCHEMA_VERSION => {}
        (Some(kind), Some(schema))
            if kind == LIVE_SUBSTRATE_KIND && schema == LIVE_SCHEMA_VERSION => {}
        (None, _) => findings.push("`substrate.kind` is missing".to_string()),
        (Some(kind), other) => findings.push(format!(
            "schema_version {other:?} does not match the `{kind}` substrate"
        )),
    }

    // ── GAR-006 split: present, distinguishable, no legacy blocks ────────
    let (reference, executed, evaluated) = match (
        object(report, "reference_population"),
        object(report, "executed_population"),
        object(report, "evaluated_population"),
    ) {
        (Ok(reference), Ok(executed), Ok(evaluated)) => (reference, executed, evaluated),
        _ => {
            findings
                .push("GAR-006 split is incomplete (a population block is missing)".to_string());
            return findings;
        }
    };
    if !report
        .get("outcomes")
        .is_some_and(|value| value.is_array() || value.is_object())
    {
        findings.push("`outcomes` block is missing or not an array/object".to_string());
    }
    let reference_value = &report["reference_population"];
    let executed_value = &report["executed_population"];
    let evaluated_value = &report["evaluated_population"];
    if reference_value == executed_value
        || reference_value == evaluated_value
        || executed_value == evaluated_value
    {
        findings.push("GAR-006 populations are not distinguishable objects".to_string());
    }
    if report.get("denominators").is_some() || report.get("corpus").is_some() {
        findings.push("a forbidden legacy `denominators`/`corpus` block is present".to_string());
    }

    match non_empty_string(reference, "role") {
        Ok(role) if role == REFERENCE_POPULATION_ROLE => {}
        other => findings.push(format!(
            "reference_population.role must be `{REFERENCE_POPULATION_ROLE}`, got {other:?}"
        )),
    }
    push(
        non_empty_string(reference, "identity").map(|_| ()),
        &mut findings,
    );
    push(count(reference, "documents").map(|_| ()), &mut findings);
    match executed.get("corpus_execution").and_then(Value::as_bool) {
        Some(value) if value == live => {}
        other => findings.push(format!(
            "executed_population.corpus_execution must be `{live}` for this substrate, got {other:?}"
        )),
    }
    match non_empty_string(executed, "execution_evidence") {
        Ok(evidence) => {
            let expected_kind = if live {
                LIVE_EXECUTION_EVIDENCE_KIND
            } else {
                EXECUTION_EVIDENCE_KIND
            };
            if !evidence.starts_with(expected_kind) {
                findings.push(format!(
                    "execution_evidence must name the declared kind `{expected_kind}`, got `{evidence}`"
                ));
            }
        }
        Err(finding) => findings.push(finding),
    }
    push(count(executed, "documents").map(|_| ()), &mut findings);
    push(
        non_empty_string(evaluated, "status").map(|_| ()),
        &mut findings,
    );
    push(count(evaluated, "documents").map(|_| ()), &mut findings);

    // ── population relations (executed ⊆ reference, evaluated ⊆ executed) ─
    let reference_documents = count(reference, "documents").unwrap_or(0);
    let executed_documents = count(executed, "documents").unwrap_or(0);
    let evaluated_documents = count(evaluated, "documents").unwrap_or(0);
    if executed_documents > reference_documents {
        findings.push(format!(
            "executed documents {executed_documents} exceed reference documents {reference_documents}"
        ));
    }
    if evaluated_documents > executed_documents {
        findings.push(format!(
            "evaluated documents {evaluated_documents} exceed executed documents {executed_documents}"
        ));
    }

    // ── the closed DATA metric set, every observation typed ─────────────
    let observations = match object(report, "observations") {
        Ok(observations) => observations,
        Err(finding) => {
            findings.push(finding);
            return findings;
        }
    };
    if observations.is_empty() {
        findings.push("the observation metric set is empty".to_string());
    }
    let declared: Vec<&str> = observations.keys().map(String::as_str).collect();
    for metric in BATCH_BUDGET_METRICS {
        if !declared.contains(&metric) {
            findings.push(format!("metric `{metric}` is missing from `observations`"));
        }
    }
    for metric in &declared {
        if !BATCH_BUDGET_METRICS.contains(metric) {
            findings.push(format!(
                "`observations` publishes undeclared metric `{metric}`"
            ));
        }
    }
    for (metric, observation) in observations {
        let Some(entry) = observation.as_object() else {
            findings.push(format!("observation `{metric}` is not an object"));
            continue;
        };
        let Some(state) = entry.get("state").and_then(Value::as_str) else {
            findings.push(format!("observation `{metric}` has no `state`"));
            continue;
        };
        if !BATCH_OBSERVATION_STATES.contains(&state) {
            findings.push(format!(
                "observation `{metric}` has unknown state `{state}`"
            ));
            continue;
        }
        match state {
            "observed" => {
                match entry.get("value").and_then(Value::as_u64) {
                    Some(0) if PROCESS_METRICS.contains(&metric.as_str()) => findings.push(
                        format!("process metric `{metric}` publishes a zero as a measurement"),
                    ),
                    Some(_) => {}
                    None => findings.push(format!(
                        "observation `{metric}` is `observed` without an integer `value`"
                    )),
                }
                if entry.contains_key("reason") {
                    findings.push(format!(
                        "observation `{metric}` is `observed` and carries a reason"
                    ));
                }
            }
            "not_observed" => {
                if entry.contains_key("value") {
                    findings.push(format!(
                        "observation `{metric}` is `not_observed` and carries a numeric field"
                    ));
                }
                match entry.get("reason").and_then(Value::as_str) {
                    Some(reason) if !reason.trim().is_empty() => {}
                    _ => findings.push(format!(
                        "observation `{metric}` is `not_observed` without a named reason"
                    )),
                }
            }
            _ => {}
        }
    }

    // ── additivity: peak >= the known successful window sizes ───────────
    let batches = match report.get("batches").and_then(Value::as_array) {
        Some(batches) if !batches.is_empty() => batches,
        _ => {
            findings.push("`batches` is missing or empty".to_string());
            return findings;
        }
    };
    let empty = Vec::new();
    let outcomes = report
        .get("outcomes")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut aggregate_peak: Option<u64> = None;
    for batch in batches {
        let Some(entry) = batch.as_object() else {
            findings.push("a batch entry is not an object".to_string());
            continue;
        };
        let id = non_empty_string(entry, "id").unwrap_or_else(|_| "<unnamed>".to_string());
        let peak = entry
            .get("budget_observation")
            .and_then(|observation| observation.get("peak_retained_bytes"))
            .filter(|observation| {
                observation.get("state").and_then(Value::as_str) == Some("observed")
            })
            .and_then(|observation| observation.get("value"))
            .and_then(Value::as_u64);
        let known = entry
            .get("retained_window_success_bytes")
            .and_then(Value::as_u64);
        let mut recomputed = 0u64;
        for outcome in outcomes {
            let Some(outcome) = outcome.as_object() else {
                continue;
            };
            if outcome.get("batch").and_then(Value::as_str) != Some(id.as_str()) {
                continue;
            }
            if outcome.get("status").and_then(Value::as_str) == Some("succeeded") {
                recomputed += outcome
                    .get("outcome_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
        }
        let Some(peak) = peak else {
            findings.push(format!(
                "batch `{id}` did not observe `peak_retained_bytes`"
            ));
            continue;
        };
        match known {
            Some(known) if known != recomputed => findings.push(format!(
                "batch `{id}` recorded retained_window_success_bytes {known} but its outcomes sum to {recomputed}"
            )),
            None => findings.push(format!("batch `{id}` has no retained_window_success_bytes")),
            _ => {}
        }
        if peak < recomputed {
            findings.push(format!(
                "batch `{id}` non-additive accounting: window peak {peak} < known window success bytes {recomputed}"
            ));
        }
        aggregate_peak = Some(aggregate_peak.map_or(peak, |current| current.max(peak)));
    }
    let declared_peak = observations
        .get("peak_retained_bytes")
        .and_then(|observation| observation.get("value"))
        .and_then(Value::as_u64);
    if declared_peak != aggregate_peak {
        findings.push(format!(
            "aggregate peak_retained_bytes {declared_peak:?} is not the max across batches {aggregate_peak:?}"
        ));
    }

    // ── H05 honesty, bound to the DATA closure constant ─────────────────
    let closure = report.get("h05_closure").and_then(Value::as_str);
    match report.get("h05_design_closure").and_then(Value::as_str) {
        Some(design) if design == BATCH_BUDGET_H05_CLOSURE => {}
        other => findings.push(format!(
            "h05_design_closure must stay bound to the DATA constant `{BATCH_BUDGET_H05_CLOSURE}`, got {other:?}"
        )),
    }
    match report
        .get("h05_required_measurements")
        .and_then(Value::as_object)
    {
        Some(required) => {
            for name in H05_REQUIRED_MEASUREMENTS {
                if !required.contains_key(name) {
                    findings.push(format!("h05_required_measurements lacks `{name}`"));
                }
            }
        }
        None => findings.push("h05_required_measurements is missing".to_string()),
    }
    match closure {
        Some("complete") => {
            for metric in ["peak_rss_bytes", "java_cpu_wall_clock_ns"] {
                if state_of(report, metric).as_deref() != Some("observed") {
                    findings.push(format!(
                        "h05_closure is `complete` without an observed `{metric}`"
                    ));
                }
            }
            for metric in LATENCY_TAIL_METRICS {
                if state_of(report, metric).as_deref() != Some("observed") {
                    findings.push(format!(
                        "h05_closure is `complete` without an observed `{metric}`"
                    ));
                }
            }
        }
        Some("in_progress") => {
            if report
                .get("h05_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                findings.push("h05_closure is `in_progress` without a named reason".to_string());
            }
        }
        other => findings.push(format!("h05_closure has an unknown value {other:?}")),
    }
    // The measured fact: with Java CPU not observed, closure may not be complete.
    if state_of(report, "java_cpu_wall_clock_ns").as_deref() != Some("observed")
        && closure == Some("complete")
    {
        findings.push(
            "h05_closure claims `complete` while the Java CPU ceiling is not observed".to_string(),
        );
    }

    // ── verdict, locks, redaction ──────────────────────────────────────
    match report.get("verdict").and_then(Value::as_str) {
        Some(verdict) if BATCH_BUDGET_VERDICTS.contains(&verdict) => {}
        other => findings.push(format!("verdict {other:?} is outside the DATA vocabulary")),
    }
    match object(report, "policy") {
        Ok(policy) => {
            for (key, expected) in [
                ("import_eligible", serde_json::json!(false)),
                ("deployed", serde_json::json!(false)),
                ("production_route", serde_json::json!("forbidden")),
                ("graph_write", serde_json::json!("forbidden")),
                ("auto_promote", serde_json::json!("forbidden")),
                ("observation_mode", serde_json::json!("shadow")),
                ("promotion_state", serde_json::json!("candidate_only")),
            ] {
                if policy.get(key) != Some(&expected) {
                    findings.push(format!(
                        "policy lock `{key}` drifted: {:?}",
                        policy.get(key)
                    ));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }
    match object(report, "redaction") {
        Ok(redaction) if redaction.is_empty() => {
            findings.push("`redaction` marker block is empty".to_string());
        }
        Ok(redaction) => {
            for (key, value) in redaction {
                if value != &serde_json::json!(false) {
                    findings.push(format!("redaction marker `{key}` is not `false`"));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }
    // ── java_cpu_remainder: one unified typed form for both substrates ──
    match object(report, "java_cpu_remainder") {
        Ok(remainder) => {
            let state = remainder.get("state").and_then(Value::as_str);
            let java_observed =
                state_of(report, "java_cpu_wall_clock_ns").as_deref() == Some("observed");
            match state {
                Some("observed") if java_observed => {
                    let field = |name: &str| remainder.get(name).and_then(Value::as_u64);
                    let java_cpu_ns = field("java_cpu_ns");
                    let wall_clock_ns = field("wall_clock_ns");
                    let ratio = field("java_cpu_to_wall_ratio_micros");
                    for (name, value) in [
                        ("java_cpu_ns", java_cpu_ns),
                        ("wall_clock_ns", wall_clock_ns),
                        ("java_cpu_to_wall_ratio_micros", ratio),
                    ] {
                        if value.unwrap_or(0) == 0 {
                            findings.push(format!(
                                "java_cpu_remainder.`{name}` must be a positive integer when observed"
                            ));
                        }
                    }
                    match (java_cpu_ns, wall_clock_ns, ratio) {
                        (Some(java_cpu_ns), Some(wall_clock_ns), Some(ratio))
                            if wall_clock_ns > 0 =>
                        {
                            let expected = (u128::from(java_cpu_ns) * 1_000_000
                                / u128::from(wall_clock_ns))
                                as u64;
                            if ratio != expected {
                                findings.push(format!(
                                    "java_cpu_remainder ratio {ratio} disagrees with java_cpu_ns * 1_000_000 / wall_clock_ns = {expected}"
                                ));
                            }
                        }
                        _ => {}
                    }
                    if remainder.contains_key("reason") {
                        findings.push(
                            "java_cpu_remainder is observed and carries a reason".to_string(),
                        );
                    }
                }
                Some("not_observed") if !java_observed => {
                    for name in [
                        "java_cpu_ns",
                        "wall_clock_ns",
                        "java_cpu_to_wall_ratio_micros",
                    ] {
                        if remainder.contains_key(name) {
                            findings.push(format!(
                                "java_cpu_remainder is not_observed and carries a numeric field `{name}`"
                            ));
                        }
                    }
                    match remainder.get("reason").and_then(Value::as_str) {
                        Some(reason) if !reason.trim().is_empty() => {}
                        _ => findings.push(
                            "java_cpu_remainder is not_observed without a named reason".to_string(),
                        ),
                    }
                }
                Some(other) => findings.push(format!(
                    "java_cpu_remainder state `{other}` disagrees with the Java-CPU observation"
                )),
                None => findings.push("java_cpu_remainder has no `state`".to_string()),
            }
            for key in ["counter_scope", "basis"] {
                match remainder.get(key).and_then(Value::as_str) {
                    Some(value) if !value.trim().is_empty() => {}
                    _ => findings.push(format!(
                        "java_cpu_remainder.`{key}` must be a non-empty string"
                    )),
                }
            }
        }
        Err(finding) => findings.push(finding),
    }

    match object(report, "substrate") {
        Ok(substrate) => {
            let java_observed =
                state_of(report, "java_cpu_wall_clock_ns").as_deref() == Some("observed");
            match substrate.get("kind").and_then(Value::as_str) {
                Some(STUB_SUBSTRATE_KIND) => {
                    if substrate
                        .get("live_grobid_participated")
                        .and_then(Value::as_bool)
                        != Some(false)
                    {
                        findings.push(
                            "the stub substrate must not claim a live GROBID participated"
                                .to_string(),
                        );
                    }
                    if substrate
                        .get("deterministic_substrate")
                        .and_then(Value::as_bool)
                        != Some(true)
                    {
                        findings.push(
                            "the stub substrate must declare itself deterministic".to_string(),
                        );
                    }
                }
                Some(LIVE_SUBSTRATE_KIND) => {
                    let live_claimed = substrate
                        .get("live_grobid_participated")
                        .and_then(Value::as_bool);
                    if live_claimed != Some(java_observed) {
                        findings.push(
                            "live_grobid_participated must equal the Java-CPU observation state"
                                .to_string(),
                        );
                    }
                    match substrate.get("profile").and_then(Value::as_str) {
                        Some("structural") => {}
                        other => findings.push(format!(
                            "the live substrate must declare the `structural` profile, got {other:?}"
                        )),
                    }
                    if substrate
                        .get("declared_release")
                        .and_then(Value::as_object)
                        .is_none()
                    {
                        findings
                            .push("the live substrate must carry its declared_release".to_string());
                    }
                }
                other => findings.push(format!("substrate.kind is unknown: {other:?}")),
            }
            let participation = substrate
                .get("java_process_participation_observed")
                .and_then(Value::as_bool);
            if participation != Some(java_observed) {
                findings.push(
                    "Java process participation must equal the Java-CPU observation state"
                        .to_string(),
                );
            }
            match substrate
                .get("reader_invocations")
                .and_then(Value::as_object)
            {
                Some(invocations) => {
                    for meter in ["peak_rss_bytes", "java_cpu_wall_clock_ns"] {
                        if invocations.get(meter).and_then(Value::as_u64).unwrap_or(0) == 0 {
                            findings.push(format!("injected reader `{meter}` was never invoked"));
                        }
                    }
                }
                None => findings.push("reader_invocations is missing".to_string()),
            }
        }
        Err(finding) => findings.push(finding),
    }

    let text = match serde_json::to_string(report) {
        Ok(text) => text,
        Err(error) => {
            findings.push(format!("report does not serialize: {error}"));
            return findings;
        }
    };
    for token in FORBIDDEN_REPORT_TOKENS {
        if text.contains(token) {
            findings.push(format!("serialized report leaks forbidden token `{token}`"));
        }
    }
    for token in host_tokens() {
        if text.contains(&token) {
            findings.push(format!(
                "serialized report leaks the host identity `{token}`"
            ));
        }
    }

    findings
}

fn read_artifact(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read artifact {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("artifact {} is not JSON: {error}", path.display()))
}

fn assert_clean(report: &Value, label: &str) {
    let findings = audit(report);
    assert!(
        findings.is_empty(),
        "{label} violates {} invariant(s):\n- {}",
        findings.len(),
        findings.join("\n- ")
    );
}

#[test]
fn committed_artifact_publishes_the_measured_budget_and_the_gar006_split() {
    let path = repo_root().join(ARTIFACT);
    let report = read_artifact(&path);
    assert_clean(&report, "the committed shadow artifact");

    // Measured, not assumed: the load really was two concurrent batches with a
    // deterministic slow ordinal-0, and the closed metric set really is typed.
    assert_eq!(report["schema_version"], SCHEMA_VERSION);
    assert_eq!(
        report["reference_population"]["role"],
        REFERENCE_POPULATION_ROLE
    );
    assert_eq!(report["executed_population"]["corpus_execution"], false);
    assert_eq!(report["executed_population"]["documents"], 6);
    assert_eq!(report["executed_population"]["failed"], 0);
    assert_eq!(
        report["substrate"]["load"]["concurrent_batches"], 2,
        "the artifact must be measured under concurrent batches"
    );
    assert_eq!(
        report["substrate"]["load"]["slow_ordinal_zero_gate"], "sibling_responses_answered",
        "ordinal-0 must be held by a deterministic gate, not a race"
    );
    assert_eq!(report["outcomes"].as_array().expect("outcomes").len(), 6);
    assert_eq!(
        report["observations"]
            .as_object()
            .expect("observations")
            .len(),
        BATCH_BUDGET_METRICS.len(),
        "exactly the closed DATA metric set must be published"
    );
    for metric in LATENCY_TAIL_METRICS {
        assert_eq!(
            state_of(&report, metric).as_deref(),
            Some("observed"),
            "latency tail `{metric}` must be measured under the concurrent load"
        );
    }
    assert_eq!(
        state_of(&report, "peak_retained_bytes").as_deref(),
        Some("observed")
    );
    assert_eq!(
        state_of(&report, "peak_rss_bytes").as_deref(),
        Some("observed")
    );
    // The honest absence: no Java process participated, so the Java-CPU ceiling
    // is a typed `not_observed` with a named reason and no numeric field.
    assert_eq!(
        state_of(&report, "java_cpu_wall_clock_ns").as_deref(),
        Some("not_observed")
    );
    assert!(
        report["observations"]["java_cpu_wall_clock_ns"]
            .get("value")
            .is_none(),
        "a `not_observed` observation must carry no numeric field"
    );
    assert!(
        report["observations"]["java_cpu_wall_clock_ns"]["reason"]
            .as_str()
            .is_some_and(|reason| !reason.trim().is_empty())
    );
    assert_eq!(report["h05_closure"], "in_progress");
    assert_eq!(report["verdict"], "within_ceiling");
    // The declared shared execution bound is genuinely TIGHTER than the load, so
    // the two batches really contend for the shared slots (whether an individual
    // run happened to observe a wait is reported by `budget_waits`, which the
    // audit already pins as an observed integer of the closed metric set).
    let execution_capacity = report["substrate"]["load"]["shared_budget"]["execution_capacity"]
        .as_u64()
        .expect("shared execution capacity");
    let documents = report["executed_population"]["documents"]
        .as_u64()
        .expect("executed documents");
    assert!(
        execution_capacity < documents,
        "the shared execution bound ({execution_capacity}) must be tighter than the load ({documents})"
    );
    assert!(
        report["observations"]["max_reorder_depth"]["value"]
            .as_u64()
            .is_some_and(|depth| depth > 0)
    );
}

#[test]
fn regenerated_artifact_matches_the_committed_form() {
    let committed = read_artifact(&repo_root().join(ARTIFACT));
    let directory = tempdir().expect("temporary report directory");
    let regenerated_path = directory.path().join("parse_batch_budget_shadow.json");
    let output = run_example(&["--report", regenerated_path.to_str().expect("report path")]);
    assert!(
        output.status.success(),
        "canonical regeneration failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let regenerated = read_artifact(&regenerated_path);
    assert_clean(&regenerated, "the regenerated shadow artifact");

    let keys = |value: &Value| -> Vec<String> {
        value
            .as_object()
            .expect("report object")
            .keys()
            .cloned()
            .collect()
    };
    assert_eq!(
        keys(&regenerated),
        keys(&committed),
        "a regenerated report must publish the same top-level shape as the committed artifact"
    );
    for key in [
        "observations",
        "policy",
        "redaction",
        "substrate",
        "h05_required_measurements",
    ] {
        assert_eq!(
            regenerated[key].as_object().map(|object| {
                let mut keys: Vec<&String> = object.keys().collect();
                keys.sort();
                keys
            }),
            committed[key].as_object().map(|object| {
                let mut keys: Vec<&String> = object.keys().collect();
                keys.sort();
                keys
            }),
            "`{key}` keys drifted between the committed and the regenerated report"
        );
    }
    let batch_ids = |value: &Value| {
        let mut ids: Vec<String> = value["batches"]
            .as_array()
            .expect("batches")
            .iter()
            .map(|batch| batch["id"].as_str().expect("batch id").to_string())
            .collect();
        ids.sort();
        ids
    };
    assert_eq!(batch_ids(&regenerated), batch_ids(&committed));
    assert_eq!(
        regenerated["outcomes"].as_array().map(Vec::len),
        committed["outcomes"].as_array().map(Vec::len),
        "the regenerated run must produce the same terminal outcome count"
    );
}

#[test]
fn self_check_proves_every_fail_closed_guard_fires() {
    let output = run_example(&["--self-check"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the fail-closed self-check failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("SELF-CHECK OK"),
        "the self-check must report its verdict:\n{stdout}"
    );
    for guard in REQUIRED_SELF_CHECK_GUARDS {
        assert!(
            stdout.contains(guard),
            "the self-check must prove the `{guard}` guard fires:\n{stdout}"
        );
    }
    // `--self-check` is hermetic: it writes no report.
    assert!(
        !stdout.contains("wrote "),
        "--self-check must not write a report artifact:\n{stdout}"
    );
}
