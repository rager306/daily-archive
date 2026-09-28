//! Hermetic contract for the LIVE Java-CPU artifact (M513-r7dg4z/S04/T03).
//!
//! WHY THIS TEST EXISTS: T02 built the declared `live-grobid` substrate but never
//! executed it, so H05's `java_cpu_wall_clock_ns` stayed a typed `not_observed`.
//! A measurement becomes a fact only when its bytes are pinned by a check that
//! needs no GROBID — this test reads the COMMITTED
//! `artifacts/parse_compat/parse_batch_budget_live.json` and holds every
//! invariant the live run claims. It never opens a socket, never starts the
//! sidecar, and never re-runs the harness.
//!
//! Operator command that produced the artifact (run on the host with the
//! sidecar up; the reader takes the cgroup SCOPE DIRECTORY and reads `cpu.stat`
//! itself — it never receives `/sys/` back into the report):
//!
//! ```text
//! GID=$(docker inspect -f '{{.Id}}' daily-archive-grobid)
//! PARSE_BATCH_JAVA_CPUACCT=/sys/fs/cgroup/system.slice/docker-$GID.scope \
//!   cargo run -p da-cli --example parse_batch_budget_shadow -- \
//!   --substrate live-grobid --endpoint http://127.0.0.1:8070 \
//!   --report artifacts/parse_compat/parse_batch_budget_live.json
//! ```
//!
//! Honest semantics of the share (the whole point of the slice):
//!
//! * the numerator `java_cpu_ns` is the GROBID sidecar's CONTAINER-WIDE CPU
//!   total over the window (cgroup v2 `cpu.stat` `usage_usec` delta, all
//!   threads) — NOT a per-document figure;
//! * the denominator `wall_clock_ns` is measured between the SAME before/after
//!   counter reads that produce the delta, so the share is computed over
//!   exactly one window, and that window holds no synthetic delay;
//! * therefore `java_cpu_to_wall_ratio_micros` is a count of busy cores
//!   (`* 1_000_000`), and it may legitimately EXCEED 1_000_000. It is never a
//!   share ≤ 1.
//!
//! The retained-byte additivity reference is SUBSTRATE-AWARE, and this test
//! pins why: the stub's deterministic gate makes every successful outcome of a
//! batch coexist in the retention window, so its peak must cover their sum; the
//! live substrate publishes strictly by ordinal and releases a reservation as
//! soon as the sink consumes it, so successes need not coexist. The live
//! artifact therefore carries a window peak that is LEGITIMATELY below the sum
//! of its successful outcome sizes (batch B measured 1_362_196 against a
//! 2_358_615 sum), which is exactly what releasing memory means. The ceiling
//! guarantee is enforced at reservation time, not by this reference.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The live artifact this test judges. Git-tracked on purpose.
const LIVE_ARTIFACT: &str = "artifacts/parse_compat/parse_batch_budget_live.json";
/// The committed stub artifact whose top-level key set is the single form.
const STUB_ARTIFACT: &str = "artifacts/parse_compat/parse_batch_budget_shadow.json";
/// The acceptance manifest is the source of truth for the declared release.
const MANIFEST: &str = "data/parse/acceptance_manifest.yaml";

const LIVE_SCHEMA_VERSION: &str = "parse-batch-budget-live.v1";
const LIVE_SUBSTRATE_KIND: &str = "live_grobid_http";
const LIVE_PROFILE: &str = "structural";
const JAVA_CPU_COUNTER_SCOPE: &str = "grobid_sidecar_cgroup_cpuacct";
const EXECUTION_EVIDENCE_KIND: &str = "live_grobid_http_structural";
const H05_REQUIRED_MEASUREMENTS: [&str; 3] =
    ["peak_rss_bytes", "java_cpu_wall_clock_ns", "latency_tails"];
const POLICY_LOCKS: [&str; 7] = [
    "import_eligible",
    "deployed",
    "production_route",
    "graph_write",
    "auto_promote",
    "observation_mode",
    "promotion_state",
];

/// Literal tokens that would publish a physical path, a container identity, a
/// host identity, raw payload bytes or an executable path. The same denylist the
/// harness validates against.
const FORBIDDEN_TOKENS: [&str; 15] = [
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn read_json(relative: &str) -> Value {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

fn live_artifact() -> Value {
    read_json(LIVE_ARTIFACT)
}

fn object<'a>(value: &'a Value, key: &str) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("`{key}` block is missing or not an object"))
}

fn nested<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    let mut current = value;
    for key in path {
        current = current
            .get(key)
            .ok_or_else(|| format!("`{}` is missing", path.join(".")))?;
    }
    Ok(current)
}

fn non_empty(value: &Value, path: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("`{path}` must be a non-empty string"))
}

fn positive(value: &Value, path: &str) -> Result<u64, String> {
    match value.as_u64() {
        Some(number) if number > 0 => Ok(number),
        _ => Err(format!("`{path}` must be a positive integer")),
    }
}

/// Host identities that must never appear in a published artifact.
fn host_tokens() -> Vec<String> {
    let mut tokens = Vec::new();
    for key in ["HOSTNAME", "HOST"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty()
                && trimmed != "localhost"
                && !tokens.contains(&trimmed.to_string())
            {
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

/// Any maximal run of exactly 64 hex digits that is NOT immediately preceded by
/// `sha256:` is an unlabelled container id or hash — both are physical identity
/// leaks. Byte comparison avoids char-boundary panics.
fn unlabelled_long_hex_runs(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_hexdigit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_hexdigit() {
            index += 1;
        }
        if index - start == 64 {
            let labelled = start >= 7 && &bytes[start - 7..start] == b"sha256:";
            if !labelled {
                findings.push(text[start..start + 16].to_string());
            }
        }
    }
    findings
}

/// The declared release from `data/parse/acceptance_manifest.yaml`.
#[derive(Debug)]
struct DeclaredRelease {
    version: String,
    digest: String,
}

fn declared_release() -> DeclaredRelease {
    let path = repo_root().join(MANIFEST);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()));
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(&text).expect("the acceptance manifest is YAML");
    let declared = parsed
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .and_then(|probe| probe.get("declared_release"))
        .expect("external_holdout_contract.release_probe.declared_release must exist");
    let field = |name: &str| -> String {
        declared
            .get(name)
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("declared_release.{name} must be a string"))
            .to_string()
    };
    DeclaredRelease {
        version: field("version"),
        digest: field("image_digest"),
    }
}

/// Independent audit of the live artifact: every returned string is a
/// violation, so an empty vector means the artifact holds every invariant this
/// task claims. `stub` is used only to compare the single top-level key form.
fn audit_live(live: &Value, stub: &Value) -> Vec<String> {
    /// Fold one check result into the findings vector without borrowing it for
    /// the whole audit body.
    fn push(result: Result<(), String>, findings: &mut Vec<String>) {
        if let Err(finding) = result {
            findings.push(finding);
        }
    }

    let mut findings = Vec::new();

    // ── one single artifact form: live and stub share the top-level keys ─
    let live_keys: Vec<&str> = live
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let stub_keys: Vec<&str> = stub
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if live_keys.is_empty() || stub_keys.is_empty() {
        findings.push("the live or stub artifact is not a JSON object".to_string());
    }
    let mut live_sorted = live_keys.clone();
    let mut stub_sorted = stub_keys.clone();
    live_sorted.sort_unstable();
    stub_sorted.sort_unstable();
    if live_sorted != stub_sorted {
        findings.push(format!(
            "the live artifact's top-level key set diverges from the committed stub form: live_only={:?} stub_only={:?}",
            live_sorted
                .iter()
                .filter(|key| !stub_sorted.contains(key))
                .collect::<Vec<_>>(),
            stub_sorted
                .iter()
                .filter(|key| !live_sorted.contains(key))
                .collect::<Vec<_>>()
        ));
    }

    // ── substrate identity and schema ───────────────────────────────────
    let kind = nested(live, &["substrate", "kind"])
        .ok()
        .and_then(Value::as_str);
    if kind != Some(LIVE_SUBSTRATE_KIND) {
        findings.push(format!(
            "`substrate.kind` must be `{LIVE_SUBSTRATE_KIND}`, got {kind:?}"
        ));
    }
    if live.get("schema_version").and_then(Value::as_str) != Some(LIVE_SCHEMA_VERSION) {
        findings.push(format!(
            "`schema_version` must be `{LIVE_SCHEMA_VERSION}`, got {:?}",
            live.get("schema_version")
        ));
    }
    if nested(live, &["substrate", "profile"])
        .ok()
        .and_then(Value::as_str)
        != Some(LIVE_PROFILE)
    {
        findings.push(format!(
            "`substrate.profile` must be `{LIVE_PROFILE}` (D204: no Crossref wait in the window)"
        ));
    }

    // ── a real Java process participated, and the claim is earned ───────
    if nested(live, &["substrate", "live_grobid_participated"])
        .ok()
        .and_then(Value::as_bool)
        != Some(true)
    {
        findings.push("`substrate.live_grobid_participated` must be true".to_string());
    }
    if nested(live, &["substrate", "java_process_participation_observed"])
        .ok()
        .and_then(Value::as_bool)
        != Some(true)
    {
        findings.push("`substrate.java_process_participation_observed` must be true".to_string());
    }
    if nested(live, &["substrate", "deterministic_substrate"])
        .ok()
        .and_then(Value::as_bool)
        != Some(false)
    {
        findings
            .push("`substrate.deterministic_substrate` must be false for a live run".to_string());
    }
    if nested(live, &["substrate", "java_cpu_scope"])
        .ok()
        .and_then(Value::as_str)
        != Some(JAVA_CPU_COUNTER_SCOPE)
    {
        findings.push(format!(
            "`substrate.java_cpu_scope` must declare `{JAVA_CPU_COUNTER_SCOPE}`"
        ));
    }
    for invocation in ["java_cpu_wall_clock_ns", "peak_rss_bytes"] {
        match nested(live, &["substrate", "reader_invocations"])
            .ok()
            .and_then(|value| value.get(invocation))
            .and_then(Value::as_u64)
        {
            Some(count) if count >= 1 => {}
            other => findings.push(format!(
                "`substrate.reader_invocations.{invocation}` must be >= 1, got {other:?}"
            )),
        }
    }
    match nested(live, &["substrate", "reader_invocations"])
        .ok()
        .and_then(|value| value.get("java_cpu_wall_clock_ns"))
        .and_then(Value::as_u64)
    {
        Some(count) if count >= 2 => {}
        other => findings.push(format!(
            "the Java-CPU reader must be exercised at least twice (before/after), got {other:?}"
        )),
    }

    // ── the declared release, bound to the manifest by DIGEST, not a tag ─
    let declared = declared_release();
    match nested(live, &["substrate", "declared_release"]) {
        Ok(release) => {
            push(
                non_empty(
                    release.get("version").unwrap_or(&Value::Null),
                    "substrate.declared_release.version",
                )
                .and_then(|version| {
                    if version == declared.version {
                        Ok(())
                    } else {
                        Err(format!(
                            "the measured version `{version}` does not match the declared release `{}`",
                            declared.version
                        ))
                    }
                }),
                &mut findings,
            );
            push(
                non_empty(
                    release.get("image_digest").unwrap_or(&Value::Null),
                    "substrate.declared_release.image_digest",
                )
                .and_then(|digest| {
                    if digest != declared.digest {
                        return Err(format!(
                            "the measured digest `{digest}` does not match the declared release `{}`",
                            declared.digest
                        ));
                    }
                    let hex = digest.strip_prefix("sha256:").unwrap_or("");
                    if hex.len() != 64 || !hex.chars().all(|character| character.is_ascii_hexdigit())
                    {
                        return Err(format!(
                            "`{digest}` must be pinned as sha256:<64 hex>, not a tag"
                        ));
                    }
                    Ok(())
                }),
                &mut findings,
            );
        }
        Err(finding) => findings.push(finding),
    }

    // ── the measured ceiling, observed and positive, without a reason ───
    match nested(live, &["observations", "java_cpu_wall_clock_ns"]) {
        Ok(observation) => {
            if observation.get("state").and_then(Value::as_str) != Some("observed") {
                findings
                    .push("`observations.java_cpu_wall_clock_ns` must be `observed`".to_string());
            }
            push(
                positive(
                    observation.get("value").unwrap_or(&Value::Null),
                    "observations.java_cpu_wall_clock_ns.value",
                )
                .map(|_| ()),
                &mut findings,
            );
            if observation.get("reason").is_some() {
                findings.push("an observed Java-CPU ceiling must not carry a reason".to_string());
            }
        }
        Err(finding) => findings.push(finding),
    }

    // ── H05 fully measured and honestly closed ──────────────────────────
    match object(live, "h05_required_measurements") {
        Ok(required) => {
            for name in H05_REQUIRED_MEASUREMENTS {
                if required.get(name).and_then(Value::as_str) != Some("observed") {
                    findings.push(format!(
                        "`h05_required_measurements.{name}` must be `observed`, got {:?}",
                        required.get(name)
                    ));
                }
            }
            if required.len() != H05_REQUIRED_MEASUREMENTS.len() {
                findings.push(format!(
                    "`h05_required_measurements` must carry exactly the {} declared names, got {}",
                    H05_REQUIRED_MEASUREMENTS.len(),
                    required.len()
                ));
            }
        }
        Err(finding) => findings.push(finding),
    }
    if live.get("h05_closure").and_then(Value::as_str) != Some("complete") {
        findings.push(format!(
            "`h05_closure` must be `complete`, got {:?}",
            live.get("h05_closure")
        ));
    }
    // The design block stays explicitly unclaimed: H05 is closed by MEASUREMENT
    // here, the design claim is a separate DATA constant.
    if live.get("h05_design_closure").and_then(Value::as_str) != Some("not_claimed") {
        findings.push(format!(
            "`h05_design_closure` must stay `not_claimed`, got {:?}",
            live.get("h05_design_closure")
        ));
    }

    // ── the same concurrent load with the same slow ordinal-0 ───────────
    match nested(live, &["substrate", "load"]) {
        Ok(load) => {
            for (path, expected) in [
                ("concurrent_batches", 2u64),
                ("documents_per_batch", 3),
                ("slow_ordinal_zero_settle_ms", 0),
            ] {
                if load.get(path).and_then(Value::as_u64) != Some(expected) {
                    findings.push(format!(
                        "`substrate.load.{path}` must be {expected}, got {:?}",
                        load.get(path)
                    ));
                }
            }
            if load.get("slow_ordinal_zero_gate").and_then(Value::as_str)
                != Some("declared_page_count_disparity")
            {
                findings.push(format!(
                    "the slow ordinal-0 must be named `declared_page_count_disparity`, got {:?}",
                    load.get("slow_ordinal_zero_gate")
                ));
            }
            let budget = load.get("shared_budget");
            for (path, expected) in [("execution_capacity", 4u64), ("publication_capacity", 6)] {
                if budget
                    .and_then(|value| value.get(path))
                    .and_then(Value::as_u64)
                    != Some(expected)
                {
                    findings.push(format!(
                        "`substrate.load.shared_budget.{path}` must be {expected}"
                    ));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }

    // ── the share of time left on Java, with honest semantics ───────────
    match object(live, "java_cpu_remainder") {
        Ok(remainder) => {
            if remainder.get("state").and_then(Value::as_str) != Some("observed") {
                findings.push("`java_cpu_remainder.state` must be `observed`".to_string());
            }
            let java_cpu_ns = positive(
                remainder.get("java_cpu_ns").unwrap_or(&Value::Null),
                "java_cpu_remainder.java_cpu_ns",
            );
            let wall_clock_ns = positive(
                remainder.get("wall_clock_ns").unwrap_or(&Value::Null),
                "java_cpu_remainder.wall_clock_ns",
            );
            let ratio = positive(
                remainder
                    .get("java_cpu_to_wall_ratio_micros")
                    .unwrap_or(&Value::Null),
                "java_cpu_remainder.java_cpu_to_wall_ratio_micros",
            );
            push(java_cpu_ns.clone().map(|_| ()), &mut findings);
            push(wall_clock_ns.clone().map(|_| ()), &mut findings);
            push(ratio.clone().map(|_| ()), &mut findings);
            if let (Ok(java_cpu_ns), Ok(wall_clock_ns), Ok(ratio)) =
                (java_cpu_ns, wall_clock_ns, ratio)
            {
                let expected =
                    (u128::from(java_cpu_ns) * 1_000_000 / u128::from(wall_clock_ns)) as u64;
                if ratio != expected {
                    findings.push(format!(
                        "the share must be java_cpu_ns * 1_000_000 / wall_clock_ns: declared {ratio}, recomputed {expected}"
                    ));
                }
            }
            // The semantics are pinned, not merely present: the numerator is
            // container-wide and the ratio counts cores (it may exceed 1_000_000).
            match non_empty(
                remainder.get("basis").unwrap_or(&Value::Null),
                "java_cpu_remainder.basis",
            ) {
                Ok(basis) => {
                    for needle in ["container-wide", "NOT per-document", "may exceed 1_000_000"] {
                        if !basis.contains(needle) {
                            findings.push(format!(
                                "`java_cpu_remainder.basis` must state `{needle}` so the ratio is not read as a share ≤ 1"
                            ));
                        }
                    }
                }
                Err(finding) => findings.push(finding),
            }
            if remainder.get("counter_scope").and_then(Value::as_str)
                != Some(JAVA_CPU_COUNTER_SCOPE)
            {
                findings.push(format!(
                    "`java_cpu_remainder.counter_scope` must be `{JAVA_CPU_COUNTER_SCOPE}`"
                ));
            }
            if remainder.contains_key("reason") {
                findings
                    .push("an observed `java_cpu_remainder` must not carry a reason".to_string());
            }
        }
        Err(finding) => findings.push(finding),
    }

    // ── the executed population actually ran the pinned corpus ──────────
    match object(live, "executed_population") {
        Ok(executed) => {
            if executed.get("corpus_execution").and_then(Value::as_bool) != Some(true) {
                findings.push("`executed_population.corpus_execution` must be true".to_string());
            }
            match non_empty(
                executed.get("execution_evidence").unwrap_or(&Value::Null),
                "executed_population.execution_evidence",
            ) {
                Ok(evidence) => {
                    if !evidence.starts_with(EXECUTION_EVIDENCE_KIND) {
                        findings.push(format!(
                            "`executed_population.execution_evidence` must name `{EXECUTION_EVIDENCE_KIND}`"
                        ));
                    }
                }
                Err(finding) => findings.push(finding),
            }
            for (path, expected) in [("documents", 6u64), ("succeeded", 6), ("failed", 0)] {
                if executed.get(path).and_then(Value::as_u64) != Some(expected) {
                    findings.push(format!(
                        "`executed_population.{path}` must be {expected}, got {:?}",
                        executed.get(path)
                    ));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }

    // ── the tool does not decide the level, and neither does this artifact ─
    match object(live, "policy") {
        Ok(policy) => {
            for key in POLICY_LOCKS {
                let expected = match key {
                    "import_eligible" | "deployed" => Value::Bool(false),
                    "production_route" | "graph_write" | "auto_promote" => {
                        Value::String("forbidden".to_string())
                    }
                    "observation_mode" => Value::String("shadow".to_string()),
                    "promotion_state" => Value::String("candidate_only".to_string()),
                    _ => unreachable!("declared lock"),
                };
                if policy.get(key) != Some(&expected) {
                    findings.push(format!(
                        "policy lock `{key}` must stay {expected}, got {:?}",
                        policy.get(key)
                    ));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }
    match object(live, "redaction") {
        Ok(redaction) => {
            if redaction.is_empty() {
                findings.push("`redaction` must not be empty".to_string());
            }
            for (key, value) in redaction {
                if value != &Value::Bool(false) {
                    findings.push(format!("redaction marker `{key}` must be false"));
                }
            }
        }
        Err(finding) => findings.push(finding),
    }

    // ── no physical path, container id or host identity was published ────
    let text = serde_json::to_string(live).expect("the artifact serializes");
    for token in FORBIDDEN_TOKENS {
        if text.contains(token) {
            findings.push(format!(
                "the serialized artifact leaks forbidden token `{token}`"
            ));
        }
    }
    for host in host_tokens() {
        if text.contains(&host) {
            findings.push(format!(
                "the serialized artifact leaks the host identity `{host}`"
            ));
        }
    }
    for run in unlabelled_long_hex_runs(&text) {
        findings.push(format!(
            "the serialized artifact carries an unlabelled 64-hex identity `{run}…`"
        ));
    }

    findings
}

#[test]
fn live_artifact_publishes_the_same_top_level_key_set_as_the_stub() {
    let findings: Vec<String> = audit_live(&live_artifact(), &read_json(STUB_ARTIFACT))
        .into_iter()
        .filter(|finding| finding.contains("top-level key set"))
        .collect();
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn live_artifact_reports_an_observed_java_cpu_ceiling_and_the_share_of_time_on_java() {
    let live = live_artifact();
    let findings = audit_live(&live, &read_json(STUB_ARTIFACT));
    assert!(findings.is_empty(), "{}", findings.join("\n"));

    // The measured number must be a real one, not a typed absence.
    let value = live["observations"]["java_cpu_wall_clock_ns"]["value"]
        .as_u64()
        .expect("observed Java-CPU ceiling");
    assert!(value > 0, "the Java-CPU ceiling must be positive");
    let ratio = live["java_cpu_remainder"]["java_cpu_to_wall_ratio_micros"]
        .as_u64()
        .expect("observed share");
    assert!(ratio > 0, "the share must be positive");
    eprintln!(
        "live Java-CPU: {} ns over {} ns of wall clock -> {} micro-cores ({:.3} cores)",
        value,
        live["java_cpu_remainder"]["wall_clock_ns"]
            .as_u64()
            .unwrap_or(0),
        ratio,
        ratio as f64 / 1_000_000.0
    );
}

#[test]
fn live_artifact_is_bound_to_the_declared_release_by_digest() {
    let live = live_artifact();
    let digest = live["substrate"]["declared_release"]["image_digest"]
        .as_str()
        .expect("declared release digest");
    let hex = digest
        .strip_prefix("sha256:")
        .expect("a digest pin, not a tag");
    assert_eq!(hex.len(), 64, "a digest pin must be sha256:<64 hex>");
    assert!(
        hex.chars().all(|character| character.is_ascii_hexdigit()),
        "a digest pin must be hexadecimal"
    );
    assert_eq!(
        digest,
        declared_release().digest,
        "digest must equal the manifest declaration"
    );
}

type Case = (&'static str, Box<dyn Fn(&mut Value)>);

/// Polarity guard: the audit must actually reject corrupted copies, otherwise
/// every assertion above would be vacuous.
#[test]
fn the_audit_rejects_a_corrupted_live_artifact() {
    let good = live_artifact();
    let stub = read_json(STUB_ARTIFACT);
    assert!(
        audit_live(&good, &stub).is_empty(),
        "the committed live artifact must pass its own audit first"
    );

    let mut cases: Vec<Case> = Vec::new();
    cases.push((
        "substrate kind reverted to the stub",
        Box::new(|report: &mut Value| {
            report["substrate"]["kind"] =
                Value::String("local_deterministic_loopback_stub".to_string());
        }),
    ));
    cases.push((
        "Java-CPU ceiling reverted to not_observed",
        Box::new(|report: &mut Value| {
            report["observations"]["java_cpu_wall_clock_ns"] =
                serde_json::json!({"state": "not_observed", "reason": "injected"});
        }),
    ));
    cases.push((
        "share zeroed",
        Box::new(|report: &mut Value| {
            report["java_cpu_remainder"]["java_cpu_to_wall_ratio_micros"] = Value::from(0);
        }),
    ));
    cases.push((
        "physical path injected",
        Box::new(|report: &mut Value| {
            report["substrate"]["measurement_note"] =
                Value::String("/sys/fs/cgroup/system.slice/docker-deadbeef.scope".to_string());
        }),
    ));
    cases.push((
        "unlabelled container id injected",
        Box::new(|report: &mut Value| {
            report["substrate"]["measurement_note"] = Value::String(
                "container 71389b7c891cf95b7a195e88c7feafbd424b2ef5bca1cec3023de26ff7e907ce"
                    .to_string(),
            );
        }),
    ));
    cases.push((
        "top-level key dropped",
        Box::new(|report: &mut Value| {
            report
                .as_object_mut()
                .expect("object")
                .remove("java_cpu_remainder");
        }),
    ));

    for (name, mutate) in cases {
        let mut corrupted = good.clone();
        mutate(&mut corrupted);
        let findings = audit_live(&corrupted, &stub);
        assert!(
            !findings.is_empty(),
            "the audit must reject the corrupted copy `{name}`, but it passed"
        );
    }
}
