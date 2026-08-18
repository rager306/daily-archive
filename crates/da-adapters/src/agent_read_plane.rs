//! Remote agent read-plane adapter (R095/R091, S03 T04).
//!
//! Implements [`da_ports::AgentReadPlane`] over the universal kg-storage
//! [`RemoteGraphStore`] (RESP over TCP + `DA.IDENTITY`). The adapter is the
//! only place that talks to the remote plane: the use case never sees
//! `SnapshotIdentity` or `QueryResult` (R040 onion) — those kg-storage types
//! are mapped here onto the domain [`GraphIdentity`] / [`AgentQueryEvidence`]
//! the verified agent runner reasons about.
//!
//! # Adapter-side row truncation (R087)
//!
//! Row truncation is **not** a server capability: `ReadOnlyServer` returns
//! every row and its 8 MB `max_response_bytes` cap is an error, not a row
//! cut. `max_rows` is enforced here so the universal kg-storage server stays
//! agent-agnostic (ADR-050). The cut is explicit via the
//! `truncated: bool` flag (R087) — never implied by `rows == 0`: an empty
//! result with `truncated: false` is a genuine empty answer, and
//! `truncated: true` records that rows were cut off at the bound.
//!
//! # Result references
//!
//! `AgentQueryEvidence::result_ref` is a deterministic content digest
//! (`sha256:result:<hex>`) over the canonical JSON of the **full** result
//! record (graph, query, columns, records) — truncation never changes the
//! record's identity, so a recomputation (S04) reproduces the same ref under
//! unchanged data, and claims ground onto the durable record hash.
//!
//! # Env contract
//!
//! Reuses the documented `DA_REMOTE_GRAPH_*` keys of
//! [`RemoteGraphConfig::from_env`] — S03 adds **no new env keys**.

use std::time::Instant;

use async_trait::async_trait;

use da_domain::{AgentQueryEvidence, ContentDigest, GraphIdentity};
use da_ports::{
    AgentReadPlane, AgentReadPlaneError, AgentReadPlaneResult, snapshot_to_graph_identity,
};
use kg_storage::error::EnvParseError;
use kg_storage::traits::QueryResult;
use kg_storage::{GraphStore, RemoteGraphConfig, RemoteGraphStore};

/// Remote read-plane adapter (R095/R091, S03 T04).
///
/// Wraps the universal [`RemoteGraphStore`] and maps its kg-storage types
/// onto domain values. Identity comes from the `DA.IDENTITY` handshake;
/// queries run through the read-only command (`GRAPH.RO_QUERY`, mutating
/// Cypher is rejected client-side before any connection — D127 discipline);
/// `max_rows` truncation is enforced here with the explicit `truncated`
/// flag (R087).
#[derive(Debug, Clone)]
pub struct RemoteAgentReadPlane {
    store: RemoteGraphStore,
}

impl RemoteAgentReadPlane {
    /// Wrap an already-configured remote graph store.
    pub fn new(store: RemoteGraphStore) -> Self {
        Self { store }
    }

    /// Build from the documented `DA_REMOTE_GRAPH_*` env keys (reused from
    /// [`RemoteGraphConfig::from_env`] — no new env keys). The config
    /// contract is parsed strictly fail-closed: blank, non-numeric, zero and
    /// overflow values are rejected with a typed [`EnvParseError`].
    pub fn from_env() -> Result<Self, EnvParseError> {
        let config = RemoteGraphConfig::from_env()?;
        // from_env already validated the endpoint, so this cannot fail;
        // map defensively so the error stays typed and names the key.
        let store = RemoteGraphStore::new(config).map_err(|error| EnvParseError::Invalid {
            key: RemoteGraphConfig::ENV_ENDPOINT,
            value: String::new(),
            reason: error.to_string(),
        })?;
        Ok(Self { store })
    }

    /// The wrapped remote graph store (config introspection, health checks).
    pub fn store(&self) -> &RemoteGraphStore {
        &self.store
    }
}

#[async_trait]
impl AgentReadPlane for RemoteAgentReadPlane {
    async fn identity(&self) -> AgentReadPlaneResult<GraphIdentity> {
        let snapshot = self
            .store
            .identity()
            .await
            .map_err(AgentReadPlaneError::from)?;
        snapshot_to_graph_identity(&snapshot)
    }

    async fn query(
        &self,
        graph: &str,
        cypher: &str,
        max_rows: u64,
    ) -> AgentReadPlaneResult<AgentQueryEvidence> {
        if max_rows == 0 {
            // Fail closed (port contract): zero rows are never served. An
            // empty `truncated: false` would read as a genuine empty answer
            // (R087) — the explicit refusal keeps the bound honest.
            return Err(AgentReadPlaneError::Query(
                "max_rows must be > 0 (fail-closed row bound)".into(),
            ));
        }
        let started = Instant::now();
        let result = self
            .store
            .query_readonly(graph, cypher)
            .await
            .map_err(AgentReadPlaneError::from)?;
        let duration_ms = started.elapsed().as_millis() as u64;
        let (rows, truncated) = truncate_rows(&result.records, max_rows);
        if truncated {
            tracing::debug!(
                graph,
                served = result.records.len(),
                kept = rows,
                max_rows,
                "agent read plane truncated query rows adapter-side"
            );
        }
        Ok(AgentQueryEvidence::new(
            cypher,
            result_ref(graph, cypher, &result),
            rows as u64,
            truncated,
            duration_ms,
        ))
    }
}

/// Adapter-side row cut (R087): the server returns every row; this is the
/// only place a row bound is applied. Returns `(rows_kept, truncated)`.
fn truncate_rows(records: &[Vec<serde_json::Value>], max_rows: u64) -> (usize, bool) {
    let limit = usize::try_from(max_rows).unwrap_or(usize::MAX);
    if records.len() > limit {
        (limit, true)
    } else {
        (records.len(), false)
    }
}

/// Durable content address for the **full** result record (graph, query,
/// columns, records). Truncation never changes the record's identity, so a
/// recomputation reproduces the same ref under unchanged data, and claims
/// ground onto the durable record hash — not the adapter-side cut.
///
/// Public (S04 T01): the CLI uses this **both** at capture (recompute from
/// raw envelope rows and compare with `AgentQueryEvidence::result_ref`) and
/// at `--verify`, so the canonical form can never drift between the two
/// paths (D273: Governor shells out instead of re-hashing serde JSON).
pub fn result_ref(graph: &str, cypher: &str, result: &QueryResult) -> String {
    let canonical = serde_json::json!({
        "graph": graph,
        "query": cypher,
        "columns": result.columns,
        "records": result.records,
    });
    format!(
        "sha256:result:{}",
        ContentDigest::of_text(&canonical.to_string()).as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::AgentReadPlane;
    use kg_storage::error::{GraphResult, GraphStoreError};
    use kg_storage::read_only_server::{ReadOnlyServer, ReadOnlyServerConfig};
    use kg_storage::snapshot_identity::SnapshotIdentity;
    use kg_storage::traits::{GraphStore, VectorMetric, VectorSearchResult};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinHandle;

    /// Test read-only store: returns `row_count` canned rows per query and
    /// records how many rows it served plus how many read calls happened.
    /// The mutable query path is refused so tests prove the adapter (and
    /// server) never reach it.
    #[derive(Clone)]
    struct TestStore {
        row_count: usize,
        read_calls: Arc<AtomicUsize>,
        rows_served: Arc<AtomicUsize>,
        fail_on: Option<String>,
    }

    impl TestStore {
        fn new(row_count: usize) -> Self {
            Self {
                row_count,
                read_calls: Arc::new(AtomicUsize::new(0)),
                rows_served: Arc::new(AtomicUsize::new(0)),
                fail_on: None,
            }
        }

        /// Fail any read-only query whose text contains `needle` with a
        /// server-side query error (tests the typed Query failure path over
        /// the real wire).
        fn failing_on(needle: &str) -> Self {
            Self {
                row_count: 0,
                read_calls: Arc::new(AtomicUsize::new(0)),
                rows_served: Arc::new(AtomicUsize::new(0)),
                fail_on: Some(needle.to_string()),
            }
        }

        fn read_calls(&self) -> usize {
            self.read_calls.load(Ordering::SeqCst)
        }

        fn rows_served(&self) -> usize {
            self.rows_served.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl GraphStore for TestStore {
        async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
            Err(GraphStoreError::NotImplemented(
                "test store has no mutable query path; the read plane must never call it".into(),
            ))
        }

        async fn query_readonly(&self, _graph: &str, cypher: &str) -> GraphResult<QueryResult> {
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(needle) = &self.fail_on
                && cypher.contains(needle)
            {
                return Err(GraphStoreError::Query(format!(
                    "test query failure for {cypher}"
                )));
            }
            let records = (0..self.row_count)
                .map(|index| {
                    vec![
                        serde_json::json!(format!("row-{index}")),
                        serde_json::json!("Paper"),
                    ]
                })
                .collect::<Vec<_>>();
            self.rows_served.store(records.len(), Ordering::SeqCst);
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

    /// SnapshotIdentity JSON pre-loaded as `DA.IDENTITY`. The semantic hex
    /// is `"a" * 64`; the R091 seam must preserve it byte-for-byte (never
    /// re-hash via of_text).
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

    /// In-process real read plane: the universal `ReadOnlyServer` over real
    /// TCP plus the real `RemoteGraphStore` client (the T05 integration uses
    /// the same wiring through the use case; here the adapter is exercised
    /// directly). `identity` controls whether `DA.IDENTITY` is pre-set.
    async fn start_server(store: TestStore, identity: Option<String>) -> (String, JoinHandle<()>) {
        let server =
            ReadOnlyServer::new(ReadOnlyServerConfig::new("127.0.0.1", 0), Arc::new(store))
                .expect("server config valid");
        let server = match identity {
            Some(json) => server.with_identity(json),
            None => server,
        };
        let listener = server.bind().await.expect("bind");
        let endpoint = listener.local_addr().expect("local addr").to_string();
        let task = tokio::spawn(async move {
            let _ = server.serve_listener(listener).await;
        });
        (endpoint, task)
    }

    fn plane_for(endpoint: &str) -> RemoteAgentReadPlane {
        let config = RemoteGraphConfig::new(endpoint).unwrap();
        RemoteAgentReadPlane::new(RemoteGraphStore::new(config).unwrap())
    }

    #[tokio::test]
    async fn identity_maps_da_identity_to_domain_value_preserving_semantic_hex() {
        let (endpoint, task) = start_server(TestStore::new(0), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let identity = plane.identity().await.expect("DA.IDENTITY handshake");
        assert_eq!(identity.semantic_digest.as_str(), "a".repeat(64));
        assert_eq!(identity.tenant, "tenant-a");
        assert_eq!(identity.nodes, 10);
        assert_eq!(identity.edges, 4);

        // R091 lineage: an expected identity constructed directly over the
        // semantic hex verifies equal to the wire identity (no re-hash).
        let expected = GraphIdentity::new(
            ContentDigest::try_from("a".repeat(64)).unwrap(),
            "tenant-a",
            10,
            4,
        );
        assert!(
            da_domain::agent_verify_identity(&expected, &identity).is_ok(),
            "expected == actual must hold over the wire (R091)"
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn query_below_max_rows_returns_all_rows_without_truncation() {
        let (endpoint, task) = start_server(TestStore::new(3), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let evidence = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 5)
            .await
            .expect("query");
        assert_eq!(evidence.query, "MATCH (p:Paper) RETURN p.vid");
        assert_eq!(evidence.rows, 3);
        assert!(
            !evidence.truncated,
            "no cut below the bound; the flag is explicit, never derived from rows (R087)"
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn query_truncates_adapter_side_with_explicit_flag() {
        let store = TestStore::new(10);
        let (endpoint, task) = start_server(store.clone(), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let evidence = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 3)
            .await
            .expect("query");
        // The server really served 10 rows — truncation is adapter-side.
        assert_eq!(store.rows_served(), 10, "server returns every row");
        assert_eq!(evidence.rows, 3, "adapter keeps only max_rows rows");
        assert!(evidence.truncated, "explicit cut flag (R087)");
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn zero_rows_is_a_genuine_empty_answer_not_truncation() {
        let (endpoint, task) = start_server(TestStore::new(0), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let evidence = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 5)
            .await
            .expect("query");
        assert_eq!(evidence.rows, 0);
        assert!(
            !evidence.truncated,
            "empty with truncated=false is a genuine empty answer (R087)"
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn zero_max_rows_fails_closed_before_any_query() {
        let store = TestStore::new(3);
        let (endpoint, task) = start_server(store.clone(), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let error = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 0)
            .await
            .expect_err("fail-closed bound");
        assert!(
            matches!(error, AgentReadPlaneError::Query(ref message) if message.contains("max_rows")),
            "{error}"
        );
        // Fail-closed: the bound is refused before the server is touched.
        assert_eq!(store.read_calls(), 0, "no query executed past the bound");
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn result_ref_is_a_deterministic_content_digest_over_the_result_record() {
        let (endpoint, task) = start_server(TestStore::new(2), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let first = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 5)
            .await
            .expect("query");
        let second = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 5)
            .await
            .expect("query");
        assert_eq!(
            first.result_ref, second.result_ref,
            "identical result -> identical durable ref"
        );
        assert!(
            first.result_ref.starts_with("sha256:result:"),
            "{}",
            first.result_ref
        );
        let hex = first.result_ref.trim_start_matches("sha256:result:");
        assert_eq!(hex.len(), 64, "content digest hex");
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );

        // Truncation does not change the record's identity: the ref names
        // the full result record, the entry reports the adapter-side cut.
        let truncated = plane
            .query("hub", "MATCH (p:Paper) RETURN p.vid", 1)
            .await
            .expect("query");
        assert_eq!(truncated.result_ref, first.result_ref);
        assert!(truncated.truncated);
        assert_eq!(truncated.rows, 1);
        task.abort();
        let _ = task.await;
    }

    #[test]
    fn result_ref_is_recomputable_from_persisted_envelope_rows() {
        // S04 T01: the CLI envelope persists exactly {graph, query, columns,
        // records}; rebuilding the QueryResult from those JSON pieces and
        // recomputing through the public `result_ref` must reproduce the
        // same durable ref (capture and --verify share one canonical form).
        let result = QueryResult {
            columns: vec!["p.vid".into(), "p.title".into()],
            records: vec![
                vec![serde_json::json!("2507.19457"), serde_json::json!("GEPA")],
                vec![serde_json::json!("2507.19458"), serde_json::json!("Other")],
            ],
        };
        let graph = "hub";
        let cypher = "MATCH (p:Paper) RETURN p.vid, p.title";
        let expected = result_ref(graph, cypher, &result);

        let persisted = serde_json::json!({
            "graph": graph,
            "query": cypher,
            "columns": result.columns,
            "records": result.records,
        });
        let rebuilt = QueryResult {
            columns: serde_json::from_value(persisted["columns"].clone()).unwrap(),
            records: serde_json::from_value(persisted["records"].clone()).unwrap(),
        };
        assert_eq!(
            expected,
            result_ref(graph, cypher, &rebuilt),
            "recompute from persisted rows reproduces the capture ref"
        );
        assert!(expected.starts_with("sha256:result:"));
        // A single changed row changes the ref (tamper sensitivity).
        let mut tampered = rebuilt;
        tampered.records[0][1] = serde_json::json!("forged");
        assert_ne!(expected, result_ref(graph, cypher, &tampered));
    }

    #[tokio::test]
    async fn query_failure_surfaces_as_typed_query_error() {
        let (endpoint, task) =
            start_server(TestStore::failing_on("boom"), Some(test_identity_json())).await;
        let plane = plane_for(&endpoint);

        let error = plane
            .query("hub", "MATCH (p:Paper {id:'boom'}) RETURN p.vid", 5)
            .await
            .expect_err("server-side query failure");
        assert!(
            matches!(error, AgentReadPlaneError::Query(ref message) if message.contains("test query failure")),
            "{error}"
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn mutating_cypher_is_rejected_before_any_connection() {
        // Port 9 (discard) has no listener: a connect attempt would fail.
        // The client-side read-only validator must refuse the mutating plan
        // query first (D127 discipline), so this never reaches the wire.
        let config = RemoteGraphConfig::new("127.0.0.1:9").unwrap();
        let plane = RemoteAgentReadPlane::new(RemoteGraphStore::new(config).unwrap());

        let error = plane
            .query("hub", "CREATE (n:Paper {title:'x'})", 5)
            .await
            .expect_err("mutating cypher");
        assert!(
            matches!(error, AgentReadPlaneError::Query(ref message) if message.contains("CREATE")),
            "{error}"
        );
        assert!(
            !error.to_string().contains("connect"),
            "rejected before any connection attempt"
        );
    }

    #[tokio::test]
    async fn unreachable_plane_is_a_transport_error() {
        let config = RemoteGraphConfig::new("127.0.0.1:9").unwrap();
        let plane = RemoteAgentReadPlane::new(RemoteGraphStore::new(config).unwrap());

        let error = plane.identity().await.expect_err("connect refused");
        assert!(
            matches!(error, AgentReadPlaneError::Transport(ref message) if message.contains("connect")),
            "{error}"
        );
    }

    #[tokio::test]
    async fn unconfigured_da_identity_surfaces_as_plane_error() {
        let (endpoint, task) = start_server(TestStore::new(0), None).await;
        let plane = plane_for(&endpoint);

        assert!(
            plane.identity().await.is_err(),
            "a server without DA.IDENTITY cannot serve identity"
        );
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn malformed_semantic_hex_on_the_wire_is_a_typed_identity_error() {
        let mut identity: SnapshotIdentity =
            serde_json::from_str(&test_identity_json()).expect("identity JSON");
        identity.semantic_sha256 = "not-a-64-char-hex".into();
        let (endpoint, task) =
            start_server(TestStore::new(0), Some(identity.to_json().unwrap())).await;
        let plane = plane_for(&endpoint);

        let error = plane.identity().await.expect_err("malformed semantic hex");
        assert!(
            matches!(error, AgentReadPlaneError::Identity(ref message) if message.contains("semantic_sha256")),
            "{error}"
        );
        task.abort();
        let _ = task.await;
    }
}
