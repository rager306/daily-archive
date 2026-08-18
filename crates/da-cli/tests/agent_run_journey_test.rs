//! T05 — cross-process `da agent-run` journey (M401/S04, R095/D272–D274).
//!
//! Process A is the real `da serve-graph` binary (built via
//! `CARGO_BIN_EXE_da`) started with a generated durable `.sgsnap` preload on
//! an ephemeral port. Process B is a second OS process running the real `da
//! agent-run` binary (same `CARGO_BIN_EXE_da`) over TCP against that live
//! read-only daemon. This is the only place that exercises clap, plan
//! parsing, fingerprinting, use-case execution, post-run raw capture, atomic
//! write and offline `--verify` together on the wire.
//!
//! The journeys covered:
//! - **happy path**: a valid read-only plan against a matching preload →
//!   terminal `verified`, atomic report written, raw rows embedded,
//!   `--verify` PASS (exit 0). A tampered copy of that report → `--verify`
//!   FAIL (exit 3).
//! - **tampered expected snapshot**: a *different* valid preload fingerprint
//!   than the daemon serves → typed `identity_mismatch` at pre-flight with
//!   zero queries executed, report still written (`failed_visible`), verify
//!   PASS (a correctly-recorded typed failure is a valid report).
//! - **unreachable plane**: an endpoint with no listener → typed
//!   `read_plane_unavailable`, report still written, verify PASS.
//! - **malformed plan**: structurally-invalid JSON → exit 2 with **no**
//!   report; a semantically-mutating plan (valid JSON) → the use case sinks
//!   it as `failed_visible`/`invalid_plan` **inside** the report (exit 0),
//!   proving semantic plan problems never reach the read plane.
//!
//! Report wire assertions use raw `serde_json::Value` (not the `da_cli`
//! envelope type) so the format contract is checked independently of the
//! code that writes it (ADR-051: no oracle through the same code).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use kg_storage::{GraphStore, RemoteGraphConfig, RemoteGraphStore};

/// The real `da` binary under test (set by cargo for integration tests).
const DA_BIN: &str = env!("CARGO_BIN_EXE_da");

/// Envelope wire format (must match `da_cli::agent_run::REPORT_FORMAT`).
const REPORT_FORMAT: &str = "da.agent-run.report.v1";

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Reserve a concrete free TCP port. The daemon refuses port 0 by design, so
/// the test binds `:0`, reads the assigned port and drops the listener before
/// handing it to the child.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local addr").port()
}

/// Per-test scratch directory (label + pid + sequence so parallel tests and
/// repeated runs never collide).
fn scratch_dir(label: &str) -> PathBuf {
    let seq = DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("m401-s04-{label}-{}-{seq}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Generate a real `.sgsnap` with `count` Paper nodes, each carrying a unique
/// string `vid`. Writes the artifact to `path` and returns the exact bytes so
/// the test can fingerprint the preload artifact independently of the served
/// export.
fn write_sgsnap(path: &Path, count: usize) -> Vec<u8> {
    let mut store = samyama::GraphStore::new();
    for index in 0..count {
        let node = store.create_node(samyama::graph::types::Label::new("Paper"));
        store
            .set_node_property(
                "default",
                node,
                "vid",
                samyama::graph::property::PropertyValue::String(format!(
                    "vid:paper:journey-{index}"
                )),
            )
            .expect("set vid");
    }
    let mut buf = Vec::new();
    samyama::snapshot::export_tenant(&store, &mut buf).expect("export snapshot");
    std::fs::write(path, &buf).expect("write snapshot artifact");
    buf
}

/// Owns the `da serve-graph` child process for one test. Kills and reaps the
/// child on drop even when the test panics.
struct DaemonGuard {
    child: Option<Child>,
    log_dir: PathBuf,
}

impl DaemonGuard {
    /// Spawn `da serve-graph --host 127.0.0.1 --port <port> --snapshot <snap>`
    /// with a scrubbed environment (no `DA_SERVE_GRAPH_*`, no graph data path
    /// leaking from the test host). stdout/stderr go to files so a startup
    /// failure is diagnosable from the logs.
    fn start(port: u16, snapshot: &Path, label: &str) -> Self {
        let log_dir = scratch_dir(label);
        let stdout_path = log_dir.join("daemon.stdout.log");
        let stderr_path = log_dir.join("daemon.stderr.log");
        let stdout = std::fs::File::create(&stdout_path).expect("create stdout log");
        let stderr = std::fs::File::create(&stderr_path).expect("create stderr log");

        let mut command = Command::new(DA_BIN);
        command
            .args([
                "serve-graph",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--snapshot",
            ])
            .arg(snapshot)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        for (key, _) in std::env::vars() {
            if key.starts_with("DA_SERVE_GRAPH_") || key == kg_storage::GraphMode::ENV_DATA_PATH {
                command.env_remove(key);
            }
        }
        let child = command.spawn().expect("spawn da serve-graph");
        Self {
            child: Some(child),
            log_dir,
        }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Connect with retry until the daemon answers health PING. On failure, dumps
/// the daemon's captured logs for diagnosis.
async fn wait_ready(port: u16, guard: &DaemonGuard) -> RemoteGraphStore {
    let endpoint = format!("127.0.0.1:{port}");
    let config = RemoteGraphConfig::new(endpoint.clone()).expect("valid endpoint");
    let store = RemoteGraphStore::new(config).expect("valid config");
    for _ in 0..200 {
        if let Ok(true) = store.health().await {
            return store;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let stdout =
        std::fs::read_to_string(guard.log_dir.join("daemon.stdout.log")).unwrap_or_default();
    let stderr =
        std::fs::read_to_string(guard.log_dir.join("daemon.stderr.log")).unwrap_or_default();
    panic!(
        "da serve-graph never became ready on {endpoint}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}

/// Run the real `da` binary with the given args and a scrubbed agent env
/// (defence in depth — the tests always pass `--endpoint` explicitly, but a
/// leaked `DA_REMOTE_GRAPH_ENDPOINT` must not interfere). Returns
/// (exit code, stdout, stderr).
fn run_da(args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(DA_BIN);
    command.args(args);
    for key in ["DA_REMOTE_GRAPH_ENDPOINT", "DA_GRAPH_ENDPOINT"] {
        command.env_remove(key);
    }
    let output = command.output().expect("spawn da binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Write a plan file (a JSON array of `{query, purpose}`) to the scratch dir.
fn write_plan(dir: &Path, label: &str, contents: &str) -> PathBuf {
    let path = dir.join(format!("{label}.json"));
    std::fs::write(&path, contents).expect("write plan file");
    path
}

/// Read a written report envelope as raw JSON for independent wire assertions.
fn read_report_json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("read report");
    serde_json::from_str(&text).expect("report is valid JSON")
}

#[tokio::test]
async fn happy_path_reaches_verified_and_verify_passes_then_fails_on_tamper() {
    let dir = scratch_dir("happy");
    let snap_path = dir.join("preload.sgsnap");
    write_sgsnap(&snap_path, 2);
    let port = free_port();
    let guard = DaemonGuard::start(port, &snap_path, "happy");
    let _ready = wait_ready(port, &guard).await;
    let endpoint = format!("127.0.0.1:{port}");

    let plan_path = write_plan(
        &dir,
        "plan",
        r#"[{"query":"MATCH (p:Paper) RETURN count(p) AS papers","purpose":"count papers"}]"#,
    );
    let report_path = dir.join("report.json");
    let _ = std::fs::remove_file(&report_path);

    let (code, stdout, stderr) = run_da(&[
        "agent-run",
        "run",
        "--plan",
        plan_path.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--expected-snapshot",
        snap_path.to_str().unwrap(),
        "--output",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "happy run must exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stdout one-liner names the report and the terminal FSM state.
    assert!(
        stdout.contains("report:") && stdout.contains("terminal_state: verified"),
        "stdout one-liner missing, got:\n{stdout}"
    );

    // Atomic write: no temp file left behind.
    assert!(report_path.exists(), "report must exist");
    assert!(
        !report_path.with_extension("tmp").exists(),
        "temp file must be renamed away"
    );

    // Independent wire check (raw JSON, not the da_cli type).
    let envelope = read_report_json(&report_path);
    assert_eq!(envelope["format"], REPORT_FORMAT, "{envelope}");
    assert_eq!(envelope["run"]["terminal_state"], "verified", "{envelope}");
    assert_eq!(envelope["graph"], "daily_archive", "{envelope}");
    // The run itself carries no typed error on the happy path.
    assert!(
        envelope["run"]["error"].is_null(),
        "happy run has no error: {envelope}"
    );
    // One plan query → one captured raw record, 1:1 with evidence.
    assert_eq!(envelope["plan"].as_array().unwrap().len(), 1);
    assert_eq!(envelope["result_records"].as_array().unwrap().len(), 1);
    assert_eq!(
        envelope["result_records"][0]["query"],
        "MATCH (p:Paper) RETURN count(p) AS papers"
    );
    // Expected identity block is populated as-of context.
    let semantic_hex = envelope["expected_snapshot"]["semantic_hex"]
        .as_str()
        .expect("semantic_hex");
    assert!(!semantic_hex.is_empty(), "{envelope}");
    assert!(envelope["expected_snapshot"]["nodes"] == 2, "{envelope}");

    // Offline --verify on the valid report: PASS, exit 0.
    let (vcode, vout, _verr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        vcode, 0,
        "verify of a valid report must exit 0\nstdout:\n{vout}"
    );
    assert!(
        vout.contains("PASS"),
        "--verify must print PASS, got:\n{vout}"
    );
    assert!(
        vout.contains("verified"),
        "--verify names the terminal state:\n{vout}"
    );

    // Tamper a *copy* of the report (raw row value) → --verify FAIL exit 3.
    let tampered_path = dir.join("report-tampered.json");
    let mut tampered = envelope;
    tampered["result_records"][0]["records"][0][0] = serde_json::json!(99);
    std::fs::write(&tampered_path, serde_json::to_vec(&tampered).unwrap()).expect("write tampered");
    let (tcode, tout, terr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        tampered_path.to_str().unwrap(),
    ]);
    assert_eq!(
        tcode, 3,
        "verify of a tampered report must exit 3\nstdout:\n{tout}\nstderr:\n{terr}"
    );
    assert!(
        tout.contains("FAIL") || terr.contains("FAIL"),
        "--verify must print FAIL on a tampered report\nstdout:\n{tout}\nstderr:\n{terr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn tampered_expected_snapshot_writes_failed_visible_identity_mismatch() {
    let dir = scratch_dir("mismatch");
    let served_path = dir.join("served.sgsnap");
    let expected_path = dir.join("expected.sgsnap");
    // Daemon serves a 2-node graph; the operator's expected snapshot is a
    // *different* valid artifact (3 nodes → different semantic fingerprint).
    write_sgsnap(&served_path, 2);
    write_sgsnap(&expected_path, 3);
    let port = free_port();
    let guard = DaemonGuard::start(port, &served_path, "mismatch");
    let _ready = wait_ready(port, &guard).await;
    let endpoint = format!("127.0.0.1:{port}");

    let plan_path = write_plan(
        &dir,
        "plan",
        r#"[{"query":"MATCH (p:Paper) RETURN count(p) AS papers","purpose":"count papers"}]"#,
    );
    let report_path = dir.join("report.json");

    let (code, stdout, stderr) = run_da(&[
        "agent-run",
        "run",
        "--plan",
        plan_path.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--expected-snapshot",
        expected_path.to_str().unwrap(),
        "--output",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "a FailedVisible result is still exit 0 (typed result in the report)\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("terminal_state: failed_visible"),
        "stdout one-liner must name failed_visible, got:\n{stdout}"
    );

    let envelope = read_report_json(&report_path);
    assert_eq!(envelope["format"], REPORT_FORMAT, "{envelope}");
    assert_eq!(
        envelope["run"]["terminal_state"], "failed_visible",
        "{envelope}"
    );
    let error = &envelope["run"]["error"];
    assert_eq!(error["code"], "identity_mismatch", "{envelope}");
    // Zero queries were executed against the plane (pre-flight identity gate).
    assert_eq!(
        envelope["result_records"].as_array().unwrap().len(),
        0,
        "{envelope}"
    );

    // A correctly-recorded typed failure is a valid report → verify PASS.
    let (vcode, vout, _verr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        vcode, 0,
        "verify of a valid failed_visible report must exit 0\n{vout}"
    );
    assert!(
        vout.contains("PASS"),
        "--verify must PASS a typed failure:\n{vout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn unreachable_plane_writes_failed_visible_read_plane_unavailable() {
    let dir = scratch_dir("unreachable");
    // A dead endpoint: bind a port then drop it so nothing is listening.
    let dead_port = free_port();
    let endpoint = format!("127.0.0.1:{dead_port}");

    let snap_path = dir.join("preload.sgsnap");
    write_sgsnap(&snap_path, 2);
    let plan_path = write_plan(
        &dir,
        "plan",
        r#"[{"query":"MATCH (p:Paper) RETURN count(p) AS papers","purpose":"count papers"}]"#,
    );
    let report_path = dir.join("report.json");

    let (code, stdout, stderr) = run_da(&[
        "agent-run",
        "run",
        "--plan",
        plan_path.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--expected-snapshot",
        snap_path.to_str().unwrap(),
        "--output",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "an unreachable plane still writes a failed_visible report (exit 0)\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("terminal_state: failed_visible"),
        "stdout one-liner must name failed_visible, got:\n{stdout}"
    );

    let envelope = read_report_json(&report_path);
    assert_eq!(
        envelope["run"]["terminal_state"], "failed_visible",
        "{envelope}"
    );
    assert_eq!(
        envelope["run"]["error"]["code"], "read_plane_unavailable",
        "{envelope}"
    );
    assert_eq!(
        envelope["result_records"].as_array().unwrap().len(),
        0,
        "{envelope}"
    );

    // verify PASS (a correctly-recorded read_plane_unavailable is valid).
    let (vcode, vout, _verr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        report_path.to_str().unwrap(),
    ]);
    assert_eq!(
        vcode, 0,
        "--verify must PASS an unreachable-plane report:\n{vout}"
    );
    assert!(vout.contains("PASS"), "{vout}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn malformed_plan_exits_2_with_no_report_and_mutating_plan_is_in_report() {
    let dir = scratch_dir("plan-errors");
    let snap_path = dir.join("preload.sgsnap");
    write_sgsnap(&snap_path, 2);
    let port = free_port();
    let guard = DaemonGuard::start(port, &snap_path, "plan-errors");
    let _ready = wait_ready(port, &guard).await;
    let endpoint = format!("127.0.0.1:{port}");

    // 1) Structurally-invalid JSON → configuration error, exit 2, no report.
    let bad_plan = write_plan(&dir, "bad", r#"{"query":"not-an-array"}"#);
    let bad_report = dir.join("bad-report.json");
    let (code, _stdout, stderr) = run_da(&[
        "agent-run",
        "run",
        "--plan",
        bad_plan.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--expected-snapshot",
        snap_path.to_str().unwrap(),
        "--output",
        bad_report.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 2,
        "malformed plan is a config error, exit 2\nstderr:\n{stderr}"
    );
    assert!(
        !bad_report.exists(),
        "no report must be written on a config error"
    );

    // 2) Semantically-mutating plan (valid JSON) → the use case sinks it as
    //    failed_visible/invalid_plan INSIDE the report (exit 0) — never
    //    reaching the read plane (R087). This is the documented contract:
    //    structural parse failures exit 2 with no report; semantic plan
    //    problems are typed results in the report.
    let mutating_plan = write_plan(
        &dir,
        "mutating",
        r#"[{"query":"CREATE (n:Paper {vid:'vid:paper:evil'}) RETURN n","purpose":"try write"}]"#,
    );
    let mut_report = dir.join("mutating-report.json");
    let (mcode, mout, merr) = run_da(&[
        "agent-run",
        "run",
        "--plan",
        mutating_plan.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--expected-snapshot",
        snap_path.to_str().unwrap(),
        "--output",
        mut_report.to_str().unwrap(),
    ]);
    assert_eq!(
        mcode, 0,
        "invalid_plan is a typed result in the report (exit 0)\nstdout:\n{mout}\nstderr:\n{merr}"
    );
    let menv = read_report_json(&mut_report);
    assert_eq!(menv["run"]["terminal_state"], "failed_visible", "{menv}");
    assert_eq!(menv["run"]["error"]["code"], "invalid_plan", "{menv}");
    assert_eq!(
        menv["result_records"].as_array().unwrap().len(),
        0,
        "invalid_plan must not reach the read plane: {menv}"
    );
    // The served graph is untouched: still exactly the preloaded 2 nodes.
    let identity = _ready
        .identity()
        .await
        .expect("DA.IDENTITY after invalid plan");
    assert_eq!(
        identity.node_count, 2,
        "graph counts unchanged by invalid plan"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
