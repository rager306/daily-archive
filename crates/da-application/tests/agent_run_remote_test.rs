//! S03 T05 — real remote read-plane integration (R095/R091, ADR-061).
//!
//! Walks the *same* [`AgentRunUseCase`] (da-application) over the **real
//! remote read plane**: an in-process [`ReadOnlyServer`] (kg-storage) over
//! real TCP plus a pre-set `DA.IDENTITY`, consumed through the
//! [`RemoteAgentReadPlane`] adapter (da-adapters) over [`RemoteGraphStore`].
//! No fake plane, no `CARGO_BIN_EXE_da` (S04 owns the cross-process binary
//! journey) — this is the real in-process server + real TCP + `DA.IDENTITY`
//! slice demo:
//!
//! - happy path: expected identity equals the served identity → legal FSM
//!   walk to `Verified`, every plan query executed, deterministic grounded
//!   synthesis, budget spent within bounds;
//! - tampered expected identity → pre-flight `identity_mismatch` with
//!   **zero** queries executed (R091 lineage checked before any read);
//! - mutating plan cypher → `invalid_plan` before any server interaction
//!   (client-side `validate_readonly_cypher`, D127 discipline).
//!
//! The use case never sees `SnapshotIdentity` or `QueryResult` (R040 onion):
//! the adapter maps them onto domain [`GraphIdentity`] / [`AgentQueryEvidence`]
//! (R091 seam), and the assertions below check domain types only.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use da_adapters::RemoteAgentReadPlane;
use da_application::{AgentPlanQuery, AgentRunRequest, AgentRunUseCase, DeterministicSynthesizer};
use da_domain::{AgentBudget, AgentFailureCode, AgentState, ContentDigest, GraphIdentity};
use da_ports::AgentReadPlane;
use kg_storage::error::{GraphResult, GraphStoreError};
use kg_storage::read_only_server::{ReadOnlyServer, ReadOnlyServerConfig};
use kg_storage::snapshot_identity::SnapshotIdentity;
use kg_storage::traits::{QueryResult, VectorMetric, VectorSearchResult};
use kg_storage::{GraphStore, RemoteGraphConfig, RemoteGraphStore};
use tokio::task::JoinHandle;

/// Canned read-only store: serves `row_count` rows per read-only query and
/// counts how many read calls happened (so tests can assert zero-queries-
/// executed guarantees). The mutable query path is refused so the whole
/// stack proves the agent never reaches a write handle (D127).
#[derive(Clone)]
struct TestStore {
    row_count: usize,
    read_calls: Arc<AtomicUsize>,
}

impl TestStore {
    fn new(row_count: usize) -> Self {
        Self {
            row_count,
            read_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn read_calls(&self) -> usize {
        self.read_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl GraphStore for TestStore {
    async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Err(GraphStoreError::NotImplemented(
            "test store has no mutable query path; the agent read plane must never call it".into(),
        ))
    }

    async fn query_readonly(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        self.read_calls.fetch_add(1, Ordering::SeqCst);
        let records = (0..self.row_count)
            .map(|index| {
                vec![
                    serde_json::json!(format!("vid:paper:remote-{index}")),
                    serde_json::json!("Paper"),
                ]
            })
            .collect::<Vec<_>>();
        Ok(QueryResult {
            columns: vec!["vid".into(), "label".into()],
            records,
        })
    }

    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Err(GraphStoreError::NotImplemented(
            "test store has no vector index".into(),
        ))
    }

    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
        Err(GraphStoreError::NotImplemented(
            "test store has no property index".into(),
        ))
    }

    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Err(GraphStoreError::NotImplemented(
            "test store has no vector search".into(),
        ))
    }

    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Err(GraphStoreError::NotImplemented(
            "test store has no snapshot export".into(),
        ))
    }

    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        Err(GraphStoreError::NotImplemented(
            "read-only plane never imports".into(),
        ))
    }

    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

/// `DA.IDENTITY` JSON pre-set on the server. The semantic hex is `"a" * 64`;
/// the R091 seam must preserve it byte-for-byte over the wire (never re-hash
/// via `ContentDigest::of_text`).
fn test_identity_json() -> String {
    SnapshotIdentity {
        physical_sha256: "0".repeat(64),
        semantic_sha256: "a".repeat(64),
        semantic_algorithm: "sgsnap-vid-canonical-json.v1".to_string(),
        snapshot_format_version: 2,
        tenant: "tenant-a".to_string(),
        node_count: 10,
        edge_count: 4,
    }
    .to_json()
    .unwrap()
}

/// Domain identity matching the served `DA.IDENTITY` (R091: constructed
/// directly over the semantic hex — `ContentDigest::try_from`, not
/// `of_text`).
fn served_identity() -> GraphIdentity {
    GraphIdentity::new(
        ContentDigest::try_from("a".repeat(64)).unwrap(),
        "tenant-a",
        10,
        4,
    )
}

/// In-process real read plane: the universal `ReadOnlyServer` over real TCP
/// (ephemeral port) with the pre-set `DA.IDENTITY`, served on a background
/// task. Returns the endpoint string plus the server task (aborted on test
/// teardown).
async fn start_server(store: TestStore) -> (String, JoinHandle<()>) {
    let server = ReadOnlyServer::new(ReadOnlyServerConfig::new("127.0.0.1", 0), Arc::new(store))
        .expect("server config valid");
    let server = server.with_identity(test_identity_json());
    let listener = server.bind().await.expect("bind");
    let endpoint = listener.local_addr().expect("local addr").to_string();
    let task = tokio::spawn(async move {
        let _ = server.serve_listener(listener).await;
    });
    (endpoint, task)
}

/// Remote read-plane adapter over the real TCP endpoint (da-adapters).
fn plane_for(endpoint: &str) -> RemoteAgentReadPlane {
    let config = RemoteGraphConfig::new(endpoint).unwrap();
    RemoteAgentReadPlane::new(RemoteGraphStore::new(config).unwrap())
}

/// Sample run request: two read-only plan queries, budget for both, the
/// served identity as expected (R091), and an adapter-side row bound.
fn sample_request() -> AgentRunRequest {
    AgentRunRequest {
        entry_evidence_ref: "sha256:entry-remote-v1".into(),
        expected_identity: served_identity(),
        graph: "hub".into(),
        plan: vec![
            AgentPlanQuery {
                query: "MATCH (p:Paper) RETURN p.vid LIMIT 5".into(),
                purpose: "paper vids".into(),
            },
            AgentPlanQuery {
                query: "MATCH (e:Entity) WHERE e.name = 'GRPO' RETURN e.name".into(),
                purpose: "GRPO entity".into(),
            },
        ],
        budget: AgentBudget::new(2, 4, 1000),
        max_rows_per_query: 50,
        freeze: da_domain::ContextFreeze::default(),
    }
}

fn use_case(plane: Arc<dyn AgentReadPlane>) -> AgentRunUseCase {
    AgentRunUseCase::new(plane, Arc::new(DeterministicSynthesizer))
}

#[tokio::test]
async fn happy_path_walks_fsm_to_verified_over_real_tcp_with_served_identity() {
    let store = TestStore::new(3);
    let (endpoint, task) = start_server(store.clone()).await;
    let plane = plane_for(&endpoint);

    // The served identity over the wire equals the domain expected identity
    // (R091: no re-hash — as_str == semantic hex).
    let served = plane.identity().await.expect("DA.IDENTITY handshake");
    assert_eq!(
        served,
        served_identity(),
        "wire identity == domain identity"
    );

    let report = use_case(Arc::new(plane)).run(&sample_request()).await;

    // Legal walk: Verified with no typed error, full six-step FSM history.
    assert_eq!(report.terminal_state, AgentState::Verified);
    assert!(report.error.is_none(), "happy path carries no error");
    assert_eq!(report.decisions.len(), 6, "six forward admissions");
    assert!(
        report.decisions.iter().all(|decision| decision.admitted),
        "every step admitted on the legal walk"
    );

    // Evidence-ref scheme over the real wire (entry -> plan -> identity ->
    // evidence -> claims -> verify).
    assert_eq!(report.decisions[0].evidence_ref, "sha256:entry-remote-v1");
    assert!(report.decisions[1].evidence_ref.starts_with("sha256:plan:"));
    assert!(
        report.decisions[2].evidence_ref.starts_with("da.identity:"),
        "{}",
        report.decisions[2].evidence_ref
    );
    assert!(
        report.decisions[3]
            .evidence_ref
            .starts_with("sha256:evidence:")
    );
    assert!(
        report.decisions[4]
            .evidence_ref
            .starts_with("sha256:claims:")
    );
    assert!(
        report.decisions[5]
            .evidence_ref
            .starts_with("sha256:verify:")
    );

    // Every plan query executed over real TCP; evidence + grounded claims.
    assert_eq!(store.read_calls(), 2, "both plan queries hit the wire");
    assert_eq!(report.bundle.evidence.len(), 2);
    assert_eq!(report.bundle.claims.len(), 2);
    assert!(report.bundle.all_claims_grounded());
    assert!(
        report.bundle.evidence[0]
            .result_ref
            .starts_with("sha256:result:"),
        "{}",
        report.bundle.evidence[0].result_ref
    );

    // Budget spent within bounds; identity lineage closed (expected == actual).
    assert_eq!(report.budget.spent_queries, 2);
    assert_eq!(report.budget.spent_evidence_entries, 2);
    assert!(report.budget.spent_synthesis_chars > 0);
    assert_eq!(report.actual_identity, Some(served_identity()));
    assert_eq!(report.expected_identity, served_identity());

    // Serde-stable wire shape for S04's atomic JSON report.
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"terminal_state\":\"verified\""));
    assert!(json.contains("\"spent_queries\":2"));
    assert!(json.contains("\"actual_identity\""));

    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn tampered_expected_identity_fails_preflight_with_zero_queries() {
    let store = TestStore::new(3);
    let (endpoint, task) = start_server(store.clone()).await;
    let plane = plane_for(&endpoint);

    // Tamper the expected identity: same digest shape, wrong semantic hex
    // (and tenant/counts) — the run plans against a different graph (R091).
    let mut request = sample_request();
    request.expected_identity = GraphIdentity::new(
        ContentDigest::try_from("b".repeat(64)).unwrap(),
        "tenant-b",
        12,
        5,
    );

    let report = use_case(Arc::new(plane)).run(&request).await;

    // Typed visible failure: agent_failure:identity_mismatch, pre-flight.
    assert_eq!(report.terminal_state, AgentState::FailedVisible);
    let error = report.error.expect("typed visible failure");
    assert_eq!(error.code, AgentFailureCode::IdentityMismatch);
    assert_eq!(error.error_class(), "agent_failure:identity_mismatch");
    assert_eq!(error.from, AgentState::Searching);
    assert_eq!(error.to, AgentState::Reading);

    // R091: zero queries executed over the wire, zero budget spent, no
    // evidence — the identity gate fires before any read.
    assert_eq!(store.read_calls(), 0, "zero queries executed");
    assert_eq!(report.bundle.evidence.len(), 0);
    assert_eq!(report.budget.spent_queries, 0);

    // The report names both sides (R087): the actual served identity is
    // recorded and both tenants appear in the reason.
    assert_eq!(report.actual_identity, Some(served_identity()));
    assert!(error.reason.contains("tenant-a"), "actual side named");
    assert!(error.reason.contains("tenant-b"), "expected side named");

    // History: entry + plan admissions, blocked Searching->Reading, sink.
    assert_eq!(report.decisions.len(), 4);
    assert!(report.decisions[2].error.is_some(), "blocked step recorded");
    assert!(report.decisions[3].admitted, "sink admission recorded");

    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn mutating_plan_query_is_invalid_plan_before_any_server_interaction() {
    let store = TestStore::new(3);
    let (endpoint, task) = start_server(store.clone()).await;
    let plane = plane_for(&endpoint);

    // A mutating cypher in the plan: the use case refuses it client-side
    // (validate_readonly_cypher inside validate_plan) before the server is
    // ever contacted (D127 discipline over the real remote plane).
    let mut request = sample_request();
    request.plan[0].query = "CREATE (n:Paper {title: 'x'})".into();

    let report = use_case(Arc::new(plane)).run(&request).await;

    assert_eq!(report.terminal_state, AgentState::FailedVisible);
    let error = report.error.expect("typed visible failure");
    assert_eq!(error.code, AgentFailureCode::InvalidPlan);
    assert_eq!(error.error_class(), "agent_failure:invalid_plan");
    assert_eq!(error.from, AgentState::Planning);
    assert_eq!(error.to, AgentState::Searching);
    assert!(error.reason.contains("read-only"), "{}", error.reason);

    // Zero wire interaction: no identity handshake reached the server
    // (the identity call would also count as a read call — it does not).
    assert_eq!(store.read_calls(), 0, "no server interaction at all");
    assert_eq!(report.budget.spent_queries, 0);
    assert!(report.actual_identity.is_none(), "plane never handshaken");

    task.abort();
    let _ = task.await;
}
