//! S05/T01 — pinned offline `da agent-run --verify` over committed fixtures.
//!
//! The novel S05 proof is D273/MEM885: the digest math (`plan → identity →
//! evidence → claims → verification → result_ref`) MUST stay in the Rust
//! verifier (`da agent-run verify`), never recomputed as canonical serde JSON
//! in Python. This test pins that behaviour against **committed genuine
//! envelopes** checked into `governor/fixtures/read_only_agent_contract/`:
//!
//! - `agent_run_verified.positive.json` — a real v1 envelope produced by a
//!   live `da agent-run run` (terminal `verified`). Offline `--verify` must
//!   exit **0** and print **PASS**.
//! - `agent_run_tampered.negative.json` — the same envelope with a raw row
//!   value mutated so its recomputed `result_ref` disagrees with the recorded
//!   evidence. Offline `--verify` must exit **3** and print **FAIL**.
//!
//! Both are run purely offline (no daemon, no network, no graph store): the
//! verifier recomputes the whole chain from the report file on disk. The test
//! also asserts the committed positive envelope carries **no** `import_eligible`
//! key (D127 stays locked) and that the negative fixture is genuinely tampered
//! (not an identical copy of the positive).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The real `da` binary under test (set by cargo for integration tests).
const DA_BIN: &str = env!("CARGO_BIN_EXE_da");

/// Envelope wire format (must match `da_cli::agent_run::REPORT_FORMAT`).
const REPORT_FORMAT: &str = "da.agent-run.report.v1";

/// Repo-relative contract fixture directory (repo root is two levels above
/// the `crates/da-cli` crate dir).
fn contract_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("governor/fixtures/read_only_agent_contract")
}

/// Run the real `da` binary with a scrubbed agent env (defence in depth —
/// a leaked `DA_REMOTE_GRAPH_ENDPOINT` must not interfere). Returns
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

/// Read a committed envelope as raw JSON for independent wire assertions.
fn read_envelope(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("read committed envelope");
    serde_json::from_str(&text).expect("committed envelope is valid JSON")
}

#[test]
fn committed_positive_envelope_verifies_offline_pass() {
    let positive = contract_dir().join("agent_run_verified.positive.json");
    assert!(
        positive.exists(),
        "missing committed positive fixture: {positive:?}"
    );

    // Independent wire check (raw JSON, not the da_cli type).
    let envelope = read_envelope(&positive);
    assert_eq!(envelope["format"], REPORT_FORMAT, "{envelope}");
    assert_eq!(envelope["run"]["terminal_state"], "verified", "{envelope}");
    assert_eq!(envelope["graph"], "daily_archive", "{envelope}");
    assert!(
        envelope["result_records"].as_array().unwrap().len() == 1,
        "{envelope}"
    );

    // D127 guard: the committed positive envelope carries no import_eligible.
    let text = std::fs::read_to_string(&positive).unwrap();
    assert!(
        !text.contains("import_eligible"),
        "committed positive envelope must carry no import_eligible (D127)"
    );

    // Offline --verify → PASS, exit 0.
    let (code, stdout, stderr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        positive.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "verify of committed positive fixture must exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("PASS"),
        "committed positive fixture must print PASS:\n{stdout}"
    );
    assert!(
        stdout.contains("verified"),
        "committed positive fixture names terminal_state verified:\n{stdout}"
    );
}

#[test]
fn committed_negative_envelope_verifies_offline_fail() {
    let negative = contract_dir().join("agent_run_tampered.negative.json");
    let positive = contract_dir().join("agent_run_verified.positive.json");
    assert!(
        negative.exists(),
        "missing committed negative fixture: {negative:?}"
    );

    // The negative fixture must be a *real* tamper (differ from the positive),
    // otherwise the FAIL assertion is vacuous.
    let neg = read_envelope(&negative);
    let pos = read_envelope(&positive);
    assert_ne!(
        neg, pos,
        "negative fixture must not be an identical copy of the positive"
    );
    assert_eq!(neg["format"], REPORT_FORMAT, "{neg}");
    assert_eq!(neg["run"]["terminal_state"], "verified", "{neg}");

    // Offline --verify → FAIL, exit 3 (the first failing check names the cause).
    let (code, stdout, stderr) = run_da(&[
        "agent-run",
        "verify",
        "--report",
        negative.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 3,
        "verify of committed tampered fixture must exit 3\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("FAIL") || stderr.contains("FAIL"),
        "committed tampered fixture must print FAIL\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
