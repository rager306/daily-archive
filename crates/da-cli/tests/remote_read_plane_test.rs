//! T04 — cross-process read-plane contract (ADR-061).
//!
//! Process A is the real `da serve-graph` binary (built via
//! `CARGO_BIN_EXE_da`) started with a generated durable `.sgsnap` preload on
//! an ephemeral port. Process B is this test process: it talks to the daemon
//! through `RemoteGraphStore` (health, semantic identity, read query) and
//! through raw TCP RESP frames (fail-closed rejection of mutating/admin
//! commands, unchanged graph counts, typed identity mismatch).
//!
//! The daemon is a separate OS process, so this is a genuine cross-process
//! contract test — not an in-crate loopback. The preload artifact is
//! fingerprinted in-process (`fingerprint_snapshot`) and the second process
//! verifies the served `DA.IDENTITY` matches it semantically, proving the
//! read plane is bound to the actual preloaded graph content.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use kg_storage::resp::{RespValue, encode_command, parse_resp};
use kg_storage::{
    GraphStore, GraphStoreError, RemoteGraphConfig, RemoteGraphStore, SnapshotIdentity,
    fingerprint_snapshot,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The real `da` binary under test (set by cargo for integration tests).
const DA_BIN: &str = env!("CARGO_BIN_EXE_da");

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Reserve a concrete free TCP port. The daemon refuses port 0 by design
/// (typed env contract), so the test binds `:0`, reads the assigned port and
/// drops the listener before handing it to the child.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    listener.local_addr().expect("local addr").port()
}

/// Per-test scratch directory (label + pid + sequence so parallel tests and
/// repeated runs never collide).
fn scratch_dir(label: &str) -> PathBuf {
    let seq = DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("m401-t04-{label}-{}-{seq}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Generate a real `.sgsnap` with `count` Paper nodes, each carrying a unique
/// string `vid` (the semantic fingerprint `sgsnap-vid-canonical-json.v1`
/// requires a unique string vid per node). Writes the artifact to `path` and
/// returns the exact bytes so the test can fingerprint the preload artifact
/// independently of the served export.
fn write_sgsnap(path: &Path, count: usize) -> Vec<u8> {
    let mut store = samyama::GraphStore::new();
    for index in 0..count {
        let node = store.create_node(samyama::graph::types::Label::new("Paper"));
        store
            .set_node_property(
                "default",
                node,
                "vid",
                samyama::graph::property::PropertyValue::String(format!("vid:paper:cross-{index}")),
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
    /// with a scrubbed environment: no `DA_SERVE_GRAPH_*` and no
    /// `DA_GRAPH_DATA_PATH` leak from the test host (explicit CLI flags are
    /// authoritative anyway; the scrub is defence in depth). stdout/stderr go
    /// to files so a startup failure is diagnosable from the logs.
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

    /// Best-effort banner check: the daemon must print the operator banner
    /// naming the preload snapshot before it listens. Rust stdout is
    /// line-buffered, so by the time a connection succeeds the banner lines
    /// are already in the log file.
    fn assert_banner_names_snapshot(&self) {
        let stdout = std::fs::read_to_string(self.log_dir.join("daemon.stdout.log"))
            .expect("read daemon stdout log");
        assert!(
            stdout.contains("snapshot:"),
            "daemon banner must state the snapshot preload, got:\n{stdout}"
        );
        assert!(
            stdout.contains("NON-PERSISTENT"),
            "daemon banner must stay honest about memory-only preload, got:\n{stdout}"
        );
        assert!(
            stdout.contains("import_eligible: false") && stdout.contains("D127"),
            "daemon banner must keep the D127 lock visible, got:\n{stdout}"
        );
        assert!(
            stdout.contains("read-only"),
            "daemon banner must state the read-only data plane, got:\n{stdout}"
        );
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

/// Connect with retry until the daemon answers PING. Returns a ready
/// `RemoteGraphStore` (process B's client surface). On failure, dumps the
/// daemon's captured logs for diagnosis.
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

/// Raw TCP RESP round-trip: send a command with the shared codec and parse
/// exactly one complete response value. This is the hostile-client surface —
/// the same bytes an operator would send with `redis-cli`/nc.
async fn raw_command(addr: &str, args: &[&str]) -> RespValue {
    let mut stream = TcpStream::connect(addr).await.expect("raw tcp connect");
    stream
        .write_all(&encode_command(args))
        .await
        .expect("raw write");
    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match parse_resp(&response) {
            Ok(Some((value, consumed))) => {
                assert_eq!(
                    consumed,
                    response.len(),
                    "raw response carried trailing bytes for {args:?}"
                );
                return value;
            }
            Ok(None) => {}
            Err(error) => panic!("raw response for {args:?} is not valid RESP: {error}"),
        }
        let n = stream.read(&mut chunk).await.expect("raw read");
        assert!(n > 0, "raw connection closed mid-response for {args:?}");
        response.extend_from_slice(&chunk[..n]);
    }
}

/// Shared setup: scratch dir, snapshot artifact, spawned daemon.
fn spawn_with_snapshot(label: &str, nodes: usize) -> (u16, PathBuf, Vec<u8>, DaemonGuard) {
    let dir = scratch_dir(label);
    let snap_path = dir.join("preload.sgsnap");
    let bytes = write_sgsnap(&snap_path, nodes);
    let port = free_port();
    let guard = DaemonGuard::start(port, &snap_path, label);
    (port, dir, bytes, guard)
}

#[tokio::test]
async fn second_process_reads_health_semantic_identity_and_read_query() {
    let (port, dir, artifact_bytes, guard) = spawn_with_snapshot("health", 2);
    let artifact = fingerprint_snapshot(&artifact_bytes).expect("artifact fingerprint");
    let client = wait_ready(port, &guard).await;

    guard.assert_banner_names_snapshot();

    // Process B health: PING → PONG through the real daemon.
    assert!(client.health().await.expect("health"), "PING must answer");

    // Semantic identity: DA.IDENTITY matches the preload artifact's semantic
    // fingerprint (SHA-256 of the canonicalised exported graph bytes) and the
    // node count of the preloaded graph.
    let identity = client.identity().await.expect("DA.IDENTITY");
    assert_eq!(
        identity.semantic_sha256, artifact.semantic_sha256,
        "served semantic identity must match the preload artifact"
    );
    assert_eq!(identity.node_count, 2, "{identity:?}");
    assert_eq!(identity.tenant, artifact.tenant, "{identity:?}");

    // Process B read query: scalar aggregation through the read plane.
    let result = client
        .query_readonly("g", "MATCH (p:Paper) RETURN count(p) AS papers")
        .await
        .expect("read query");
    assert_eq!(result.columns, ["papers"], "{result:?}");
    assert_eq!(
        result.records,
        vec![vec![serde_json::json!(2)]],
        "{result:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn raw_write_and_admin_commands_are_rejected_and_counts_unchanged() {
    let (port, dir, _, guard) = spawn_with_snapshot("reject", 2);
    let client = wait_ready(port, &guard).await;
    let addr = format!("127.0.0.1:{port}");

    // Hostile raw frames: mutating queries, delete, and admin/index/vector/
    // import commands must all fail closed server-side with a visible error —
    // never a silent OK.
    for cmd in [
        // raw GRAPH.QUERY CREATE — the exact T01 threat-surface exploit
        &[
            "GRAPH.QUERY",
            "g",
            "CREATE (n:Paper {vid: 'vid:paper:evil'}) RETURN n",
        ][..],
        &["GRAPH.QUERY", "g", "MATCH (n) DELETE n"][..],
        &["GRAPH.DELETE", "g", "1"][..],
        &["GRAPH.LIST"][..],
        &["INDEX.CREATE", "Paper", "title"][..],
        &["VECTOR.ADD", "Paper", "v"][..],
        &["IMPORT", "SNAPSHOT", "x"][..],
        &["SNAPSHOT.SAVE", "x.sgsnap"][..],
        &["ADMIN", "SHUTDOWN"][..],
        &["CONFIG", "SET", "save", "900 1"][..],
        &["FLUSHALL"][..],
        &["AUTH", "hunter2"][..],
        &["KEYS", "*"][..],
    ] {
        match raw_command(&addr, cmd).await {
            RespValue::Error(message) => {
                assert!(
                    message.starts_with("ERR"),
                    "rejection for {cmd:?} must be a typed error, got {message:?}"
                );
            }
            other => panic!("expected -ERR rejection for {cmd:?}, got {other:?}"),
        }
    }

    // Graph counts are unchanged after every rejected write: DA.IDENTITY
    // still reports the preloaded node count and the read query still counts
    // exactly the preloaded Paper nodes.
    let identity = client
        .identity()
        .await
        .expect("DA.IDENTITY after rejections");
    assert_eq!(
        identity.node_count, 2,
        "rejected writes must not change graph counts, got {identity:?}"
    );
    let result = client
        .query_readonly("g", "MATCH (p:Paper) RETURN count(p) AS papers")
        .await
        .expect("read query after rejections");
    assert_eq!(
        result.records,
        vec![vec![serde_json::json!(2)]],
        "{result:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn wrong_expected_identity_returns_typed_visible_mismatch() {
    let (port, dir, _, guard) = spawn_with_snapshot("mismatch", 2);
    let client = wait_ready(port, &guard).await;

    // Fetch the real served identity, then verify against a wrong expected
    // semantic SHA: the client must surface a typed IdentityMismatch carrying
    // BOTH sides — never an empty result or a silent pass.
    let actual = client.identity().await.expect("DA.IDENTITY");
    let wrong = SnapshotIdentity {
        semantic_sha256: "deadbeef".into(),
        ..actual.clone()
    };
    let error = client
        .verify_identity(&wrong)
        .await
        .expect_err("wrong expected identity must fail");
    match &error {
        GraphStoreError::IdentityMismatch(detail) => {
            assert_eq!(detail.expected_semantic_sha256, "deadbeef");
            assert_eq!(detail.actual_semantic_sha256, actual.semantic_sha256);
            assert_eq!(detail.expected_nodes, 2);
            assert_eq!(detail.actual_nodes, 2);
        }
        other => panic!("expected IdentityMismatch, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("identity mismatch"), "{message}");
    assert!(message.contains("deadbeef"), "{message}");
    assert!(
        message.contains(&actual.semantic_sha256),
        "mismatch must show the actual side, got {message}"
    );

    // The daemon stays healthy and the mismatch never mutated anything.
    assert!(client.health().await.expect("health after mismatch"));
    let identity = client.identity().await.expect("DA.IDENTITY after mismatch");
    assert_eq!(identity.node_count, 2);

    let _ = std::fs::remove_dir_all(&dir);
}
