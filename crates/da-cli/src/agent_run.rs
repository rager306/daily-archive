//! `da agent-run` — durable read-only agent CLI journey (R095, D272–D274).
//!
//! This module is the operator composition for the S03 verified agent use
//! case against a **live read-only graph daemon** (`da serve-graph`, S01):
//!
//! 1. Parse a plan file: JSON array of `{query, purpose}` (structural parse
//!    failure is a *configuration* error → exit 2 with **no report**;
//!    semantic plan problems are handed to the use case → typed
//!    `agent_failure:invalid_plan` **inside** the report, R087).
//! 2. Fingerprint the expected `.sgsnap` artifact via
//!    [`fingerprint_snapshot`] → [`SnapshotIdentity`] → domain
//!    [`GraphIdentity`] (semantic hex through `ContentDigest::try_from`,
//!    never `of_text` — R091/D269).
//! 3. Run `AgentRunUseCase` through `RemoteAgentReadPlane` (read-only
//!    plane, zero write authority, D127 remains locked).
//! 4. **Post-run raw capture** — re-read every plan query that produced
//!    evidence through the same `RemoteGraphStore`, embed the raw
//!    `{graph, query, columns, records}` rows into the CLI-owned envelope
//!    (D272) and **fail closed** if the recomputed `sha256:result:<hex>`
//!    disagrees with the recorded `AgentQueryEvidence::result_ref`
//!    (MEM874; never write an internally inconsistent report).
//! 5. **Atomic write** (tmp + rename in the same directory) with
//!    `tracing::info!` on write (path, terminal_state, error_class, bytes)
//!    and a stdout one-liner `report: <path> terminal_state: <state>`.
//!
//! ## Exit codes
//!
//! - `0` — the report was written atomically. `FailedVisible` is a **typed
//!   result inside the report**, not a process error (R087 philosophy).
//! - `2` — configuration error (missing endpoint, malformed plan/snapshot,
//!   `max_rows_per_query == 0`, bad report path).
//! - `3` — runtime error (transport failure during run/capture, capture
//!   disagrees with the run, atomic write failed).
//!
//! ## Env contract (D274)
//!
//! Endpoint comes from `--endpoint` or [`RemoteGraphConfig::ENV_ENDPOINT`]
//! (`DA_REMOTE_GRAPH_ENDPOINT`) only — no new env keys, no daemon
//! auto-start (missing endpoint is a configuration error, mirroring
//! `graph_query`). The historical `DA_GRAPH_ENDPOINT` wart is deliberately
//! **not** honored here.
//!
//! `--verify` (T03) is an **offline** independent report check: it re-reads
//! a written envelope from disk and recomputes the full durable evidence
//! chain (plan → identity → evidence → claims → verification →
//! `result_ref`) without touching the network or the graph store. It never
//! re-hashes canonical serde JSON — it reuses the same public digest
//! functions and `result_ref` the run path used (T01), so capture and
//! verify cannot drift (D273: S05 Governor shells out to this verifier
//! instead of re-hashing in Python).
//!
//! This module ships the run path (T02) and `--verify` (T03). Wiring into
//! `main` (T04) and the cross-process journey test (T05) are sibling tasks
//! in the same slice.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, Subcommand};
use da_adapters::{RemoteAgentReadPlane, result_ref};
use da_application::{
    AgentPlanQuery, AgentRunReport, AgentRunRequest, AgentRunUseCase, DeterministicSynthesizer,
    verify_run_evidence_chain,
};
use da_domain::{AgentBudget, AgentState, ContextFreeze, GraphIdentity};
use da_ports::{AgentReadPlane, Clock, GraphStore, SystemClock};
use kg_storage::snapshot_identity::{SnapshotIdentity, fingerprint_snapshot};
use kg_storage::{QueryResult, RemoteGraphConfig, RemoteGraphStore};
use serde::{Deserialize, Serialize};

/// CLI-owned durable report format (D272). Bumped only on breaking field
/// changes — additive fields go behind `#[serde(default)]`.
pub const REPORT_FORMAT: &str = "da.agent-run.report.v1";
/// Default read-plane graph name when `--graph` is omitted.
pub const DEFAULT_GRAPH: &str = "daily_archive";
/// Default per-query row bound (D274). Zero is rejected by clap fail-closed.
pub const DEFAULT_MAX_ROWS_PER_QUERY: u64 = 50;
/// Default synthesis chars budget for the use case.
pub const DEFAULT_MAX_SYNTHESIS_CHARS: u64 = 4000;

/// `da agent-run` top-level clap subcommand (T04 wiring). Keeps the run
/// path (`Run`) and the offline `--verify` report check (`Verify`) as two
/// sibling variants so `main` can reach both `run` and `verify_report`
/// (T02/T03) from one CLI entry point.
#[derive(Debug, Clone, Subcommand)]
pub enum AgentRunCommand {
    /// Run one durable agent-run against a live read-only serve-graph (T02).
    Run(AgentRunArgs),
    /// Offline --verify of a written v1 envelope (T03; D273 S05 Governor
    /// contract). Recomputes the evidence chain from disk without touching
    /// the network or graph store. Prints PASS or FAIL: <first check>.
    Verify(AgentRunVerifyArgs),
}

/// Arguments for the offline `--verify` subcommand (T03). Only a report
/// path is needed — the check never touches the network or graph store.
#[derive(Debug, Clone, Args)]
pub struct AgentRunVerifyArgs {
    /// Path to a written v1 envelope report to check offline.
    #[arg(long)]
    pub report: PathBuf,
}

/// `da agent-run` clap arguments (T02 run path).
#[derive(Debug, Clone, Args)]
pub struct AgentRunArgs {
    /// Path to the plan file: a JSON array of `[{ "query": "<read-only cypher>", "purpose": "<why>" }]`.
    #[arg(long)]
    pub plan: PathBuf,
    /// Read-plane graph name.
    #[arg(long, default_value = DEFAULT_GRAPH)]
    pub graph: String,
    /// Remote host:port. Falls back to DA_REMOTE_GRAPH_ENDPOINT. Missing
    /// endpoint is a configuration error (no daemon auto-start).
    #[arg(long)]
    pub endpoint: Option<String>,
    /// The `.sgsnap` artifact the daemon was preloaded with (R091 lineage).
    /// Fingerprinted to the expected graph identity the run plans against.
    #[arg(long)]
    pub expected_snapshot: PathBuf,
    /// Durable run-entry evidence ref. Defaults to
    /// `sha256:snapshot:<semantic_hex>` (deterministically anchors the run
    /// to the expected artifact). Override for downstream chains.
    #[arg(long)]
    pub entry_evidence_ref: Option<String>,
    /// Report output path (atomic tmp+rename in the same directory).
    #[arg(long)]
    pub output: PathBuf,
    /// Adapter-side row cut per query. Zero fails closed
    /// (`agent_failure:invalid_plan`), never serves past the bound.
    #[arg(long, default_value_t = DEFAULT_MAX_ROWS_PER_QUERY, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_rows_per_query: u64,
    /// Max queries for the run budget. Defaults to plan length (fail-closed
    /// floor: a longer plan is `agent_failure:invalid_plan`).
    #[arg(long)]
    pub max_queries: Option<u64>,
    /// Max evidence entries for the run budget. Defaults to plan length.
    #[arg(long)]
    pub max_evidence_entries: Option<u64>,
    /// Max synthesis chars for the run budget.
    #[arg(long, default_value_t = DEFAULT_MAX_SYNTHESIS_CHARS)]
    pub max_synthesis_chars: u64,
}

/// Typed CLI error with the documented exit-code contract (0/2/3; see
/// module doc). `FailedVisible` never maps to an error — it is a typed
/// result inside the written report.
#[derive(Debug)]
pub enum AgentRunCliError {
    /// Configuration error → exit 2 (no report written / verify could not
    /// read a valid v1 envelope).
    Configuration(String),
    /// Runtime error → exit 3 (report could not be guaranteed consistent).
    Runtime(String),
    /// `--verify` chain mismatch → exit 3 (the first failing check is the
    /// message; D273 Governor shell-out contract).
    VerifyFailed(String),
}

impl std::fmt::Display for AgentRunCliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) => {
                write!(f, "invalid agent-run configuration: {message}")
            }
            Self::Runtime(message) => write!(f, "agent-run failed: {message}"),
            Self::VerifyFailed(check) => write!(f, "agent-run verify failed: {check}"),
        }
    }
}

impl std::error::Error for AgentRunCliError {}

impl AgentRunCliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Configuration(_) => 2,
            Self::Runtime(_) | Self::VerifyFailed(_) => 3,
        }
    }

    /// Expose the first failing check for a `--verify` mismatch so `main`
    /// can print the documented `FAIL: <check>` contract (D273 Governor
    /// shell-out surface). `None` for config/runtime errors, which `main`
    /// reports via the normal error path instead.
    pub fn verify_failed_check(&self) -> Option<&str> {
        match self {
            Self::VerifyFailed(check) => Some(check),
            Self::Configuration(_) | Self::Runtime(_) => None,
        }
    }
}

/// Result of a successful `--verify` (T03): the envelope's evidence chain
/// recomputes cleanly from disk. A verify **failure** is returned as
/// [`AgentRunCliError::VerifyFailed`] whose message is the first failing
/// check (so the CLI prints `PASS` on `Ok` and `FAIL: <check>` on that
/// error; D273). `terminal_state` is the snake-case FSM state for the
/// one-liner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    /// Terminal FSM state of the verified report, snake-case (matches
    /// [`terminal_state_str`] / the serde wire form).
    pub terminal_state: &'static str,
}

/// Operator-owned `expected_snapshot` block in the envelope: physical path
/// plus the semantic identity the run plans against (R091 as-of context).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedSnapshot {
    /// Operator-supplied artifact path, for provenance.
    pub path: String,
    /// Semantic SHA-256 hex (byte-identical to `SnapshotIdentity.semantic_sha256`).
    pub semantic_hex: String,
    /// Tenant scope of the expected graph.
    pub tenant: String,
    /// Node count of the expected graph.
    pub nodes: u64,
    /// Edge count of the expected graph.
    pub edges: u64,
}

/// One captured raw query result embedded in the envelope (D272). Kept so
/// `sha256:result:<hex>` is independently recomputable from disk and claims
/// can be re-grounded offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResultRecord {
    pub graph: String,
    pub query: String,
    pub columns: Vec<String>,
    pub records: Vec<Vec<serde_json::Value>>,
}

/// CLI-owned durable agent-run envelope (D272).
///
/// Additive surface: the pinned S03 [`AgentRunReport`] is embedded verbatim
/// under `run` (never re-shaped), plus plan, expected identity and raw
/// result records. `import_eligible` never appears (D127 / MEM642).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunEnvelope {
    pub format: String,
    pub tool: String,
    pub version: String,
    /// RFC3339 UTC as-of timestamp (as-of context for the run).
    pub timestamp_utc: String,
    pub endpoint: String,
    pub graph: String,
    pub plan: Vec<AgentPlanQuery>,
    pub expected_snapshot: ExpectedSnapshot,
    /// Pinned S03 report: terminal FSM state, decisions, bundle, budget,
    /// identities, typed error.
    pub run: AgentRunReport,
    /// Raw result records captured post-run, 1:1 with `run.bundle.evidence`.
    #[serde(default)]
    pub result_records: Vec<QueryResultRecord>,
}

/// Execute one `da agent-run` (T02).
///
/// Orchestrates plan parse → expected fingerprint → use-case run →
/// post-run raw capture (fail-closed ref check) → atomic write.
pub async fn run(args: AgentRunArgs) -> Result<(), AgentRunCliError> {
    let plan = parse_plan(&args.plan)?;
    let (snapshot, expected_identity) = fingerprint_expected(&args.expected_snapshot)?;
    let endpoint = resolve_endpoint(&args)?;

    let config = RemoteGraphConfig::new(endpoint.clone()).map_err(|error| {
        AgentRunCliError::Configuration(format!("invalid endpoint {endpoint:?}: {error}"))
    })?;
    let store = RemoteGraphStore::new(config).map_err(|error| {
        AgentRunCliError::Configuration(format!("invalid remote graph config: {error}"))
    })?;

    let plane = RemoteAgentReadPlane::new(store.clone());
    let plane_arc: Arc<dyn AgentReadPlane> = Arc::new(plane);
    let use_case = AgentRunUseCase::new(plane_arc, Arc::new(DeterministicSynthesizer));

    let max_queries = args.max_queries.unwrap_or(plan.len() as u64);
    let max_evidence_entries = args.max_evidence_entries.unwrap_or(plan.len() as u64);
    let entry_evidence_ref = args
        .entry_evidence_ref
        .clone()
        .unwrap_or_else(|| format!("sha256:snapshot:{}", snapshot.semantic_sha256));

    let request = AgentRunRequest {
        entry_evidence_ref,
        expected_identity: expected_identity.clone(),
        graph: args.graph.clone(),
        plan: plan.clone(),
        budget: AgentBudget::new(max_queries, max_evidence_entries, args.max_synthesis_chars),
        max_rows_per_query: args.max_rows_per_query,
        freeze: context_freeze_from_env(),
    };

    let report = use_case.run(&request).await;

    // Post-run raw capture. Fail closed if any recomputed result ref
    // disagrees with the recorded evidence ref (README: never write an
    // internally inconsistent report).
    let result_records = capture_result_records(&store, &args.graph, &report).await?;

    let expected_snapshot = ExpectedSnapshot {
        path: args.expected_snapshot.display().to_string(),
        semantic_hex: expected_identity.semantic_digest.as_str().to_string(),
        tenant: expected_identity.tenant.clone(),
        nodes: expected_identity.nodes,
        edges: expected_identity.edges,
    };

    let envelope = AgentRunEnvelope {
        format: REPORT_FORMAT.into(),
        tool: "da".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        endpoint,
        graph: args.graph.clone(),
        plan,
        expected_snapshot,
        run: report,
        result_records,
    };

    write_envelope_atomic(&args.output, &envelope)
}

/// Independent `--verify` report check (T03): re-read a written envelope
/// from disk and recompute the full durable evidence chain **offline** —
/// plan → identity → evidence → claims → verification → `result_ref` —
/// without touching the network or the graph store.
///
/// This is the S05 Governor contract surface (D273): it never re-hashes
/// canonical serde JSON in Python, and it cannot drift from the run path
/// because it reuses the same public digest functions and `result_ref`
/// (T01). Fails closed:
///
/// - a missing/unreadable/malformed report, or a `format` that is not
///   [`REPORT_FORMAT`], is a **configuration** error (exit 2) — `--verify`
///   cannot check an envelope that isn't a valid v1 report;
/// - any evidence-chain or `result_ref` mismatch is a
///   [`AgentRunCliError::VerifyFailed`] (exit 3) whose message is the
///   **first failing check** (so `main` prints `PASS` on `Ok` and
///   `FAIL: <check>` on that error).
///
/// A `failed_visible` report is still a valid report: `--verify` checks
/// internal consistency, not run success, so a correctly-recorded typed
/// failure PASSes (R087 — `FailedVisible` is a persisted typed result,
/// not a process error).
pub fn verify_report(path: &Path) -> Result<VerifyOutcome, AgentRunCliError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        AgentRunCliError::Configuration(format!("read report {}: {error}", path.display()))
    })?;
    let envelope: AgentRunEnvelope = serde_json::from_str(&text).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "parse report {}: {error}; expected a {} envelope",
            path.display(),
            REPORT_FORMAT
        ))
    })?;
    if envelope.format != REPORT_FORMAT {
        return Err(AgentRunCliError::Configuration(format!(
            "report {} has format {:?}, expected {}",
            path.display(),
            envelope.format,
            REPORT_FORMAT
        )));
    }

    // 1. As-of context consistency: the envelope's expected-snapshot block
    //    must agree with the run's planned identity (R091).
    let expected = &envelope.run.expected_identity;
    if envelope.expected_snapshot.semantic_hex != expected.semantic_digest.as_str()
        || envelope.expected_snapshot.tenant != expected.tenant
        || envelope.expected_snapshot.nodes != expected.nodes
        || envelope.expected_snapshot.edges != expected.edges
    {
        return Err(AgentRunCliError::VerifyFailed(format!(
            "expected_snapshot: envelope identity block ({} / {:?} / {} / {}) disagrees with the run expected_identity ({} / {:?} / {} / {})",
            envelope.expected_snapshot.semantic_hex,
            envelope.expected_snapshot.tenant,
            envelope.expected_snapshot.nodes,
            envelope.expected_snapshot.edges,
            expected.semantic_digest.as_str(),
            expected.tenant,
            expected.nodes,
            expected.edges,
        )));
    }

    // 2. `result_ref` recompute (MEM874): every persisted raw row must
    //    re-hash to the recorded evidence `result_ref`, 1:1.
    if envelope.result_records.len() != envelope.run.bundle.evidence.len() {
        return Err(AgentRunCliError::VerifyFailed(format!(
            "evidence/result_records: {} evidence entries but {} captured records",
            envelope.run.bundle.evidence.len(),
            envelope.result_records.len()
        )));
    }
    for (record, evidence) in envelope
        .result_records
        .iter()
        .zip(envelope.run.bundle.evidence.iter())
    {
        let result = QueryResult {
            columns: record.columns.clone(),
            records: record.records.clone(),
        };
        let recomputed = result_ref(&record.graph, &record.query, &result);
        if recomputed != evidence.result_ref {
            return Err(AgentRunCliError::VerifyFailed(format!(
                "result_ref: recomputed {recomputed} != recorded {} for query {:?}",
                evidence.result_ref, record.query
            )));
        }
    }

    // 3. Full decision/evidence chain recomputation (plan → identity →
    //    evidence → claims → verification) against the recorded FSM history.
    if let Err(check) = verify_run_evidence_chain(&envelope.plan, &envelope.run) {
        return Err(AgentRunCliError::VerifyFailed(check));
    }

    Ok(VerifyOutcome {
        terminal_state: terminal_state_str(envelope.run.terminal_state),
    })
}

/// Parse the plan file: a JSON array of `{query, purpose}`. Structural
/// failure is a **configuration** error (exit 2, no report); semantic plan
/// validity is the use case's job (`agent_failure:invalid_plan` in-report).
fn parse_plan(path: &Path) -> Result<Vec<AgentPlanQuery>, AgentRunCliError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "read plan {}: {error}; expected a JSON array of {{\"query\",\"purpose\"}}",
            path.display()
        ))
    })?;
    serde_json::from_str::<Vec<AgentPlanQuery>>(&text).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "parse plan {}: {error}; expected a JSON array of {{\"query\",\"purpose\"}}",
            path.display()
        ))
    })
}

/// Fingerprint the expected `.sgsnap` into a [`SnapshotIdentity`] and the
/// domain [`GraphIdentity`] the run plans against (R091/D269). Any failure
/// is a configuration error.
fn fingerprint_expected(
    path: &Path,
) -> Result<(SnapshotIdentity, GraphIdentity), AgentRunCliError> {
    let bytes = std::fs::read(path).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "read expected snapshot {}: {error}",
            path.display()
        ))
    })?;
    let snapshot = fingerprint_snapshot(&bytes).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "fingerprint expected snapshot {}: {error}",
            path.display()
        ))
    })?;
    let identity = da_ports::snapshot_to_graph_identity(&snapshot).map_err(|error| {
        AgentRunCliError::Configuration(format!(
            "map expected snapshot {} to graph identity: {error}",
            path.display()
        ))
    })?;
    Ok((snapshot, identity))
}

/// Resolve the read-plane endpoint from `--endpoint` or
/// `DA_REMOTE_GRAPH_ENDPOINT` only (D274). A missing endpoint is a
/// configuration error — daemon auto-start is forbidden.
fn resolve_endpoint(args: &AgentRunArgs) -> Result<String, AgentRunCliError> {
    args.endpoint
        .clone()
        .or_else(|| std::env::var(RemoteGraphConfig::ENV_ENDPOINT).ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AgentRunCliError::Configuration(
                "agent-run requires --endpoint or DA_REMOTE_GRAPH_ENDPOINT; daemon auto-start is forbidden".into(),
            )
        })
}

fn env_or_empty(key: &str) -> String {
    std::env::var(key).unwrap_or_default()
}

/// Layer H freeze from DA_CONTEXT_* env. compiled_at comes from the clock
/// (CLI owns wall time; domain stays clock-free). Empty env = unset freeze.
fn context_freeze_from_env() -> ContextFreeze {
    context_freeze_from_env_with_clock(&SystemClock)
}

fn context_freeze_from_env_with_clock(clock: &dyn Clock) -> ContextFreeze {
    ContextFreeze {
        as_of: env_or_empty("DA_CONTEXT_AS_OF"),
        policy_hash: env_or_empty("DA_CONTEXT_POLICY_HASH"),
        scope: env_or_empty("DA_CONTEXT_SCOPE"),
        task_kind: env_or_empty("DA_CONTEXT_TASK_KIND"),
        compiled_at: clock.now().to_rfc3339(),
    }
}

/// Post-run raw capture: re-read every plan query that produced evidence
/// through the same read-only store, embed the raw rows, and **fail closed**
/// if any recomputed `sha256:result:<hex>` disagrees with the recorded
/// `result_ref` (MEM874 — never write an internally inconsistent report).
async fn capture_result_records(
    store: &RemoteGraphStore,
    graph: &str,
    report: &AgentRunReport,
) -> Result<Vec<QueryResultRecord>, AgentRunCliError> {
    let mut records = Vec::with_capacity(report.bundle.evidence.len());
    for evidence in &report.bundle.evidence {
        let result = store
            .query_readonly(graph, &evidence.query)
            .await
            .map_err(|error| {
                AgentRunCliError::Runtime(format!(
                    "post-run capture failed for query {:?}: {error}",
                    evidence.query
                ))
            })?;
        let recomputed = result_ref(graph, &evidence.query, &result);
        if recomputed != evidence.result_ref {
            return Err(AgentRunCliError::Runtime(format!(
                "capture disagrees with the run: recomputed result_ref {recomputed} != recorded {} for query {:?}; refusing to write an internally inconsistent report",
                evidence.result_ref, evidence.query
            )));
        }
        records.push(QueryResultRecord {
            graph: graph.to_string(),
            query: evidence.query.clone(),
            columns: result.columns.clone(),
            records: result.records.clone(),
        });
    }
    Ok(records)
}

/// Write the envelope atomically (tmp + rename in the same directory,
/// matching the `gold_corpus` manifest pattern), then emit the stdout
/// one-liner and a structured write decision log.
fn write_envelope_atomic(path: &Path, envelope: &AgentRunEnvelope) -> Result<(), AgentRunCliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AgentRunCliError::Runtime(format!("create {}: {error}", parent.display()))
        })?;
    }
    let bytes = serde_json::to_vec_pretty(envelope).map_err(|error| {
        AgentRunCliError::Runtime(format!("serialize agent run report: {error}"))
    })?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, &bytes).map_err(|error| {
        AgentRunCliError::Runtime(format!("write {}: {error}", temporary.display()))
    })?;
    std::fs::rename(&temporary, path).map_err(|error| {
        AgentRunCliError::Runtime(format!(
            "commit agent run report {} -> {}: {error}",
            temporary.display(),
            path.display()
        ))
    })?;

    let terminal_state = terminal_state_str(envelope.run.terminal_state);
    let error_class = envelope.run.error.as_ref().map(|error| error.error_class());
    tracing::info!(
        report_path = %path.display(),
        terminal_state = %terminal_state,
        error_class = ?error_class,
        bytes = bytes.len(),
        "agent run report written atomically"
    );
    println!(
        "report: {} terminal_state: {}",
        path.display(),
        terminal_state
    );
    Ok(())
}

/// Render an `AgentState` as the serde snake-case wire string, for the
/// stdout one-liner / structured logs.
pub fn terminal_state_str(state: AgentState) -> &'static str {
    use AgentState::*;
    match state {
        Created => "created",
        Planning => "planning",
        Searching => "searching",
        Reading => "reading",
        Synthesis => "synthesis",
        Verification => "verification",
        Verified => "verified",
        FailedVisible => "failed_visible",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_application::{claims_digest, evidence_digest, plan_digest, verification_digest};
    use da_domain::{AgentQueryEvidence, ContentDigest};
    use da_ports::FixedClock;
    use serde_json::json;

    fn write_temp(name: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("da-agent-run-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}-{name}", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    /// JSON serde roundtrip, defined locally so this shared test module
    /// compiles in BOTH the lib target (`da_cli`) and the bin target
    /// (`da`, where `crate::roundtrip_json` from lib.rs is not in scope).
    fn roundtrip_json<T>(value: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json1 = serde_json::to_string(value).expect("serialize failed");
        let back: T = serde_json::from_str(&json1).expect("deserialize failed");
        let json2 = serde_json::to_string(&back).expect("re-serialize failed");
        assert_eq!(json1, json2, "roundtrip JSON mismatch");
    }

    fn sample_plan() -> Vec<AgentPlanQuery> {
        vec![
            AgentPlanQuery {
                query: "MATCH (n:Paper) RETURN count(n) as count".into(),
                purpose: "count papers".into(),
            },
            AgentPlanQuery {
                query: "MATCH (n:Paper {year: 2024}) RETURN n.title LIMIT 5".into(),
                purpose: "recent papers".into(),
            },
        ]
    }

    fn sample_envelope() -> AgentRunEnvelope {
        let identity = GraphIdentity::new(ContentDigest::of_text("seed"), "test_tenant", 3, 2);
        AgentRunEnvelope {
            format: REPORT_FORMAT.into(),
            tool: "da".into(),
            version: "test".into(),
            timestamp_utc: "2026-08-17T00:00:00Z".into(),
            endpoint: "127.0.0.1:16379".into(),
            graph: DEFAULT_GRAPH.into(),
            plan: sample_plan(),
            expected_snapshot: ExpectedSnapshot {
                path: "artifacts/test.sgsnap".into(),
                semantic_hex: identity.semantic_digest.as_str().to_string(),
                tenant: identity.tenant.clone(),
                nodes: identity.nodes,
                edges: identity.edges,
            },
            run: AgentRunReport {
                terminal_state: AgentState::Verified,
                decisions: vec![],
                bundle: da_domain::ContextBundle::new(identity.clone()),
                budget: AgentBudget::new(10, 10, 4000),
                expected_identity: identity.clone(),
                actual_identity: Some(identity),
                error: None,
            },
            result_records: vec![QueryResultRecord {
                graph: DEFAULT_GRAPH.into(),
                query: "MATCH (n:Paper) RETURN count(n) as count".into(),
                columns: vec!["count".into()],
                records: vec![vec![json!(42)]],
            }],
        }
    }

    /// Reconstruct a `QueryResult` from a captured record's owned fields so
    /// the capture ref can be recomputed (mirrors `--verify`'s path).
    fn record_to_query_result(record: &QueryResultRecord) -> QueryResult {
        QueryResult {
            columns: record.columns.clone(),
            records: record.records.clone(),
        }
    }

    #[test]
    fn parse_plan_valid_json() {
        let path = write_temp(
            "plan-valid.json",
            r#"[{"query":"MATCH (n) RETURN count(n)","purpose":"scale"}]"#,
        );
        let plan = parse_plan(&path).expect("valid plan parses");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].query, "MATCH (n) RETURN count(n)");
        assert_eq!(plan[0].purpose, "scale");
    }

    #[test]
    fn parse_plan_malformed_is_config_error() {
        let path = write_temp("plan-bad.json", r#"{"query":"not-an-array"}"#);
        match parse_plan(&path) {
            Err(AgentRunCliError::Configuration(_)) => {}
            other => panic!("malformed plan must be a Configuration error, got {other:?}"),
        }
    }

    #[test]
    fn parse_plan_empty_array_is_valid() {
        let path = write_temp("plan-empty.json", "[]");
        assert_eq!(parse_plan(&path).expect("empty plan parses").len(), 0);
    }

    #[test]
    fn parse_plan_missing_file_is_config_error() {
        match parse_plan(Path::new("/nonexistent/plan.json")) {
            Err(AgentRunCliError::Configuration(_)) => {}
            other => panic!("missing plan must be Configuration, got {other:?}"),
        }
    }

    #[test]
    fn envelope_serde_roundtrip() {
        roundtrip_json(&sample_envelope());
    }

    #[test]
    fn envelope_never_contains_import_eligible() {
        // D127 / MEM642 guard: no nested field is named import_eligible.
        let json = serde_json::to_string(&sample_envelope()).unwrap();
        assert!(!json.contains("import_eligible"));
        assert!(!json.contains("import_eligible\":true"));
    }

    #[test]
    fn report_format_is_stable() {
        assert_eq!(sample_envelope().format, REPORT_FORMAT);
        assert_eq!(REPORT_FORMAT, "da.agent-run.report.v1");
    }

    #[test]
    fn atomic_write_commits_and_removes_tmp() {
        let dir = std::env::temp_dir()
            .join("da-agent-run-tests")
            .join(format!("out-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.json");
        let _ = std::fs::remove_file(&path);
        let tmp = path.with_extension("tmp");
        let _ = std::fs::remove_file(&tmp);

        write_envelope_atomic(&path, &sample_envelope()).expect("atomic write succeeds");

        assert!(path.exists(), "report committed");
        assert!(!tmp.exists(), "temp file removed after rename");
        let bytes = std::fs::read(&path).unwrap();
        let back: AgentRunEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, sample_envelope());
        assert_eq!(back.run.terminal_state, AgentState::Verified);
    }

    #[test]
    fn atomic_write_creates_parent_dir() {
        let dir = std::env::temp_dir()
            .join("da-agent-run-tests")
            .join(format!("nested-{}", std::process::id()));
        let path = dir.join("deep").join("sub").join("report.json");
        write_envelope_atomic(&path, &sample_envelope()).expect("creates parents");
        assert!(path.exists());
    }

    #[test]
    fn terminal_state_str_matches_serde_snake_case() {
        for (state, expected) in [
            (AgentState::Verified, "verified"),
            (AgentState::FailedVisible, "failed_visible"),
            (AgentState::Created, "created"),
            (AgentState::Planning, "planning"),
            (AgentState::Searching, "searching"),
            (AgentState::Reading, "reading"),
            (AgentState::Synthesis, "synthesis"),
            (AgentState::Verification, "verification"),
        ] {
            assert_eq!(terminal_state_str(state), expected, "state {state:?}");
            let wire = serde_json::to_string(&state).unwrap();
            assert_eq!(wire.trim_matches('"'), expected, "serde wire for {state:?}");
        }
    }

    #[test]
    fn captured_result_ref_is_deterministic_and_recomputable() {
        // The ref over the full record (graph, query, columns, records) is
        // stable across the run path and the recompute path (MEM874).
        let graph = DEFAULT_GRAPH;
        let query = "MATCH (n:Paper) RETURN count(n) as count";
        let result = QueryResult {
            columns: vec!["count".into()],
            records: vec![vec![json!(42)]],
        };
        let ref_a = result_ref(graph, query, &result);
        let record = QueryResultRecord {
            graph: graph.into(),
            query: query.into(),
            columns: result.columns.clone(),
            records: result.records.clone(),
        };
        let ref_b = result_ref(graph, query, &record_to_query_result(&record));
        assert_eq!(ref_a, ref_b);
        assert!(ref_a.starts_with("sha256:result:"), "ref prefix: {ref_a}");

        // The `AgentQueryEvidence` recorded by the run must ground onto that
        // same ref (the capture comparison is against this exact value).
        let evidence = AgentQueryEvidence::new(query, ref_a.clone(), 1, false, 1);
        assert_eq!(evidence.result_ref, ref_a);
    }

    #[test]
    fn resolve_endpoint_missing_is_config_error() {
        // Safely guard against a real env var leaking into the unit test.
        let prior = std::env::var(RemoteGraphConfig::ENV_ENDPOINT).ok();
        unsafe { std::env::remove_var(RemoteGraphConfig::ENV_ENDPOINT) };
        let args = AgentRunArgs {
            plan: PathBuf::from("p.json"),
            graph: DEFAULT_GRAPH.into(),
            endpoint: None,
            expected_snapshot: PathBuf::from("s.sgsnap"),
            entry_evidence_ref: None,
            output: PathBuf::from("out.json"),
            max_rows_per_query: 50,
            max_queries: None,
            max_evidence_entries: None,
            max_synthesis_chars: 4000,
        };
        let r = resolve_endpoint(&args);
        match r {
            Err(AgentRunCliError::Configuration(_)) => {}
            other => panic!("missing endpoint must be Configuration, got {other:?}"),
        }
        if let Some(value) = prior {
            unsafe { std::env::set_var(RemoteGraphConfig::ENV_ENDPOINT, value) };
        }
    }

    #[test]
    fn context_freeze_from_env_reads_da_context_keys() {
        let keys = [
            "DA_CONTEXT_AS_OF",
            "DA_CONTEXT_POLICY_HASH",
            "DA_CONTEXT_SCOPE",
            "DA_CONTEXT_TASK_KIND",
        ];
        let prior: Vec<(String, Option<String>)> = keys
            .iter()
            .map(|k| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        unsafe {
            std::env::set_var("DA_CONTEXT_AS_OF", "2026-08-25T00:00:00Z");
            std::env::set_var("DA_CONTEXT_POLICY_HASH", "deadbeef");
            std::env::set_var("DA_CONTEXT_SCOPE", "MATCH (p:Paper) RETURN p");
            std::env::set_var("DA_CONTEXT_TASK_KIND", "critical_reading");
        }
        let clock = FixedClock::from_timestamp(1_777_000_000);
        let freeze = context_freeze_from_env_with_clock(&clock);
        assert_eq!(freeze.as_of, "2026-08-25T00:00:00Z");
        assert_eq!(freeze.policy_hash, "deadbeef");
        assert_eq!(freeze.scope, "MATCH (p:Paper) RETURN p");
        assert_eq!(freeze.task_kind, "critical_reading");
        assert_eq!(freeze.compiled_at, clock.now().to_rfc3339());
        for (key, value) in prior {
            unsafe {
                match value {
                    Some(v) => std::env::set_var(&key, v),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }

    #[test]
    fn agent_run_cli_error_exit_codes() {
        assert_eq!(AgentRunCliError::Configuration("x".into()).exit_code(), 2);
        assert_eq!(AgentRunCliError::Runtime("x".into()).exit_code(), 3);
        assert_eq!(AgentRunCliError::VerifyFailed("x".into()).exit_code(), 3);
    }

    /// A single-query plan + result pair, aligned so plan / evidence /
    /// claims / records all reference the same query. Mirrors the FSM a
    /// real Verified run records (T03 fixture).
    fn verified_pair() -> (Vec<AgentPlanQuery>, QueryResult) {
        let plan = vec![AgentPlanQuery {
            query: "MATCH (n:Paper) RETURN count(n) as count".into(),
            purpose: "count papers".into(),
        }];
        let result = QueryResult {
            columns: vec!["count".into()],
            records: vec![vec![json!(42)]],
        };
        (plan, result)
    }

    /// Build a fully-consistent Verified envelope with the complete
    /// decision history, so `verify_report` recomputes cleanly.
    fn verified_envelope() -> AgentRunEnvelope {
        let (plan, result) = verified_pair();
        let identity = GraphIdentity::new(ContentDigest::of_text("seed"), "test_tenant", 3, 2);
        let query = plan[0].query.clone();
        let result_ref_val = result_ref(DEFAULT_GRAPH, &query, &result);

        let mut bundle = da_domain::ContextBundle::new(identity.clone());
        bundle.add_evidence(AgentQueryEvidence::new(
            query.clone(),
            result_ref_val.clone(),
            1,
            false,
            1,
        ));
        bundle.add_claim(da_domain::AgentClaim::new(
            "42 papers",
            result_ref_val.clone(),
        ));

        let actual = identity.clone();
        let decisions = vec![
            da_domain::AgentAdmissionDecision::admitted("sha256:snapshot:abc"),
            da_domain::AgentAdmissionDecision::admitted(plan_digest(&plan)),
            da_domain::AgentAdmissionDecision::admitted(format!(
                "da.identity:{}",
                identity.semantic_digest.as_str()
            )),
            da_domain::AgentAdmissionDecision::admitted(evidence_digest(&bundle)),
            da_domain::AgentAdmissionDecision::admitted(claims_digest(&bundle)),
            da_domain::AgentAdmissionDecision::admitted(verification_digest(&bundle, &actual)),
        ];

        AgentRunEnvelope {
            format: REPORT_FORMAT.into(),
            tool: "da".into(),
            version: "test".into(),
            timestamp_utc: "2026-08-17T00:00:00Z".into(),
            endpoint: "127.0.0.1:16379".into(),
            graph: DEFAULT_GRAPH.into(),
            plan,
            expected_snapshot: ExpectedSnapshot {
                path: "artifacts/test.sgsnap".into(),
                semantic_hex: identity.semantic_digest.as_str().to_string(),
                tenant: identity.tenant.clone(),
                nodes: identity.nodes,
                edges: identity.edges,
            },
            run: AgentRunReport {
                terminal_state: AgentState::Verified,
                decisions,
                bundle,
                budget: AgentBudget::new(10, 10, 4000),
                expected_identity: identity.clone(),
                actual_identity: Some(actual),
                error: None,
            },
            result_records: vec![QueryResultRecord {
                graph: DEFAULT_GRAPH.into(),
                query: query.clone(),
                columns: result.columns.clone(),
                records: result.records.clone(),
            }],
        }
    }

    fn write_envelope_to(path: &Path, envelope: &AgentRunEnvelope) {
        let bytes = serde_json::to_vec(envelope).unwrap();
        std::fs::write(path, &bytes).unwrap();
    }

    #[test]
    fn verify_passes_on_consistent_verified_envelope() {
        let path = write_temp("verify-pass.json", "{}");
        write_envelope_to(&path, &verified_envelope());
        let outcome = verify_report(&path).expect("consistent envelope verifies");
        assert_eq!(outcome.terminal_state, "verified");
    }

    #[test]
    fn verify_fails_on_tampered_result_ref() {
        // The da-cli-specific check: a persisted raw row that no longer
        // re-hashes to the recorded evidence result_ref must fail closed
        // (MEM874).
        let mut envelope = verified_envelope();
        envelope.result_records[0].records = vec![vec![json!(99)]];
        let path = write_temp("verify-result-tamper.json", "{}");
        write_envelope_to(&path, &envelope);
        match verify_report(&path) {
            Err(AgentRunCliError::VerifyFailed(message)) => {
                assert!(
                    message.starts_with("result_ref"),
                    "first failing check: {message}"
                );
            }
            other => panic!("tampered result_ref must be VerifyFailed, got {other:?}"),
        }
    }

    #[test]
    fn verify_fails_on_tampered_plan() {
        // The evidence-chain check: a plan that no longer matches the
        // recorded plan digest surfaces the first failing check.
        let mut envelope = verified_envelope();
        envelope.plan[0].query = "MATCH (x:X) RETURN x".into();
        let path = write_temp("verify-plan-tamper.json", "{}");
        write_envelope_to(&path, &envelope);
        match verify_report(&path) {
            Err(AgentRunCliError::VerifyFailed(message)) => {
                assert!(
                    message.starts_with("plan digest"),
                    "first failing check: {message}"
                );
            }
            other => panic!("tampered plan must be VerifyFailed, got {other:?}"),
        }
    }

    #[test]
    fn verify_fails_on_expected_snapshot_identity_mismatch() {
        // Envelope as-of block must agree with the run's planned identity.
        let mut envelope = verified_envelope();
        envelope.expected_snapshot.nodes += 1;
        let path = write_temp("verify-snapshot-tamper.json", "{}");
        write_envelope_to(&path, &envelope);
        match verify_report(&path) {
            Err(AgentRunCliError::VerifyFailed(message)) => {
                assert!(
                    message.starts_with("expected_snapshot"),
                    "first failing check: {message}"
                );
            }
            other => panic!("snapshot mismatch must be VerifyFailed, got {other:?}"),
        }
    }

    #[test]
    fn verify_fails_on_evidence_record_count_mismatch() {
        let mut envelope = verified_envelope();
        envelope.result_records.clear();
        let path = write_temp("verify-count-tamper.json", "{}");
        write_envelope_to(&path, &envelope);
        match verify_report(&path) {
            Err(AgentRunCliError::VerifyFailed(message)) => {
                assert!(
                    message.starts_with("evidence/result_records"),
                    "first failing check: {message}"
                );
            }
            other => panic!("count mismatch must be VerifyFailed, got {other:?}"),
        }
    }

    #[test]
    fn verify_missing_report_is_config_error() {
        match verify_report(Path::new("/nonexistent/report.json")) {
            Err(AgentRunCliError::Configuration(_)) => {}
            other => panic!("missing report must be Configuration, got {other:?}"),
        }
    }

    #[test]
    fn verify_malformed_json_is_config_error() {
        let path = write_temp("verify-bad-json.json", r#"{not: json"#);
        match verify_report(&path) {
            Err(AgentRunCliError::Configuration(_)) => {}
            other => panic!("malformed report must be Configuration, got {other:?}"),
        }
    }

    #[test]
    fn verify_wrong_format_is_config_error() {
        let path = write_temp("verify-wrong-format.json", "{}");
        write_envelope_to(&path, &{
            let mut envelope = verified_envelope();
            envelope.format = "da.agent-run.report.v2".into();
            envelope
        });
        match verify_report(&path) {
            Err(AgentRunCliError::Configuration(message)) => {
                assert!(
                    message.contains("has format"),
                    "format check message: {message}"
                );
            }
            other => panic!("wrong format must be Configuration, got {other:?}"),
        }
    }
}
