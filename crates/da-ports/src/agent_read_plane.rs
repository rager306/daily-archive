//! Agent read-plane port (R095/R091, S03 T02).
//!
//! The verified agent use case talks to the graph through this port only:
//! identity ([`GraphIdentity`]) plus bounded read-only query
//! ([`AgentQueryEvidence`]). Zero `DirectGraphStore`, zero write method
//! (D127 discipline) — the port is the R091 seam between kg-storage's
//! `SnapshotIdentity`/`QueryResult` and the domain types the use case
//! reasons about, so `da-application` never imports kg-storage identity or
//! query types into its core loop (R040 onion).
//!
//! `snapshot_to_graph_identity` is the R091 mapping seam: it wraps the
//! **existing** semantic hex via `ContentDigest::try_from` — never
//! `ContentDigest::of_text`, which would SHA-256 the hex string again and
//! produce a digest whose `as_str()` differs from
//! `SnapshotIdentity.semantic_sha256`, breaking expected == actual lineage.

use async_trait::async_trait;
use thiserror::Error;

use da_domain::{AgentClaim, AgentQueryEvidence, ContentDigest, ContextBundle, GraphIdentity};
use kg_storage::error::GraphStoreError;
use kg_storage::snapshot_identity::SnapshotIdentity;

/// Read-plane port for the verified agent runner (R095/R091, S03).
///
/// Returns only domain types: [`GraphIdentity`] for identity checks and
/// [`AgentQueryEvidence`] for bounded query results. The use case never
/// imports `SnapshotIdentity` or `QueryResult` into its core loop (R040
/// onion). Adapter-side `max_rows` truncation is explicit via
/// [`AgentQueryEvidence::truncated`] (R087) — a server-side row cut would
/// leak an agent concern into the universal kg-storage plane (ADR-050).
#[async_trait]
pub trait AgentReadPlane: Send + Sync {
    /// Fetch the read-plane identity (DA.IDENTITY handshake mapped to the
    /// domain value). Transport or identity-unavailable failures surface as
    /// [`AgentReadPlaneError`] and become `agent_failure:read_plane_unavailable`
    /// at the use-case boundary.
    async fn identity(&self) -> AgentReadPlaneResult<GraphIdentity>;

    /// Execute one read-only cypher query, truncated adapter-side to
    /// `max_rows` rows with the explicit `truncated` flag (R087). A
    /// `max_rows` of zero fails closed (no rows are ever served past the
    /// bound).
    async fn query(
        &self,
        graph: &str,
        cypher: &str,
        max_rows: u64,
    ) -> AgentReadPlaneResult<AgentQueryEvidence>;
}

/// Typed read-plane result alias (matches the `LLMResult<T>` port
/// convention).
pub type AgentReadPlaneResult<T> = Result<T, AgentReadPlaneError>;

/// Typed read-plane errors (R087, S03).
///
/// No serde: the error is not part of the run report — the use case bridges
/// it to an [`da_domain::AgentAdmissionError`] whose `error_class` renders
/// `agent_failure:read_plane_unavailable`. The `From<GraphStoreError>`
/// classifier keeps kg-storage error variants out of the use-case core loop
/// (R040 onion): the use case matches only these four classes.
#[derive(Debug, Error)]
pub enum AgentReadPlaneError {
    /// Identity unavailable or invalid: malformed semantic digest, DA.IDENTITY
    /// transport failure, or an identity mismatch surfaced by the plane.
    #[error("agent read plane identity unavailable: {0}")]
    Identity(String),
    /// A read-only query failed at the query level (validation, parse,
    /// execution, not-found).
    #[error("agent read plane query failed: {0}")]
    Query(String),
    /// Transport-level failure (connect, timeout, response-size limit). The
    /// universal kg-storage client surfaces these as `Storage`; the
    /// classifier lifts them to `Transport` so the use case can distinguish
    /// reachability from capability.
    #[error("agent read plane transport failure: {0}")]
    Transport(String),
    /// Storage-capability error: the plane does not implement a required
    /// capability (e.g. mutation attempted on a read-only plane).
    #[error("agent read plane storage capability error: {0}")]
    Storage(String),
}

impl From<GraphStoreError> for AgentReadPlaneError {
    fn from(error: GraphStoreError) -> Self {
        match error {
            GraphStoreError::NotFound(message) => Self::Query(message),
            GraphStoreError::Query(message) => Self::Query(message),
            GraphStoreError::Vector(message) => Self::Query(message),
            GraphStoreError::Storage(message) => Self::Transport(message),
            GraphStoreError::NotImplemented(message) => Self::Storage(message),
            GraphStoreError::IdentityMismatch(detail) => Self::Identity(detail.to_string()),
        }
    }
}

/// R091 mapping seam: `kg-storage SnapshotIdentity` → domain
/// [`GraphIdentity`].
///
/// The semantic digest wraps the **existing** `semantic_sha256` hex via
/// [`ContentDigest::try_from`] — never `ContentDigest::of_text`, which would
/// SHA-256 the hex string again and produce a digest whose `as_str()` differs
/// from `SnapshotIdentity.semantic_sha256`, breaking expected == actual
/// lineage (R091). Tenant, node and edge counts map 1:1. Fail-closed: a
/// malformed semantic hex (not exactly 64 lowercase hex chars) is a typed
/// [`AgentReadPlaneError::Identity`], never a panic.
pub fn snapshot_to_graph_identity(
    snapshot: &SnapshotIdentity,
) -> AgentReadPlaneResult<GraphIdentity> {
    let semantic_digest =
        ContentDigest::try_from(snapshot.semantic_sha256.clone()).map_err(|_| {
            AgentReadPlaneError::Identity(format!(
                "snapshot semantic_sha256 {:?} is not a valid 64-char lowercase hex digest",
                snapshot.semantic_sha256
            ))
        })?;
    Ok(GraphIdentity::new(
        semantic_digest,
        snapshot.tenant.clone(),
        snapshot.node_count,
        snapshot.edge_count,
    ))
}

/// Deterministic synthesis policy port (R095, S03).
///
/// Produces grounded claims from the run bundle. Sync and deterministic — no
/// LLM in the loop (the codebase policy-port convention: da-adapters / da-cli
/// implement, the use case consumes). Every returned claim must ground onto
/// evidence already in the bundle (`evidence_ref` matches an
/// [`AgentQueryEvidence::result_ref`]); the verification guard refuses
/// dangling claims as `agent_failure:missing_evidence` (R087).
pub trait AgentSynthesizer: Send + Sync {
    fn synthesize(&self, bundle: &ContextBundle) -> Vec<AgentClaim>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kg_storage::error::IdentityMismatchDetail;
    use sha2::Digest;

    /// Build a SnapshotIdentity whose semantic_sha256 is `semantic_hex`.
    fn snapshot(semantic_hex: &str) -> SnapshotIdentity {
        SnapshotIdentity {
            physical_sha256: "0".repeat(64),
            semantic_sha256: semantic_hex.to_string(),
            semantic_algorithm: "sgsnap-vid-canonical-json.v1".to_string(),
            snapshot_format_version: 2,
            tenant: "tenant-a".to_string(),
            node_count: 10,
            edge_count: 4,
        }
    }

    #[test]
    fn snapshot_to_graph_identity_keeps_semantic_hex_not_rehash() {
        // The R091 seam: as_str() equals the semantic hex exactly — the
        // mapping must never re-hash the hex string via of_text.
        let hex_digest = "a".repeat(64);
        let identity = snapshot_to_graph_identity(&snapshot(&hex_digest)).unwrap();

        assert_eq!(
            identity.semantic_digest.as_str(),
            hex_digest,
            "as_str must equal the semantic hex (lineage value), not a re-hash"
        );
        // of_text would SHA-256 the hex string again: prove the mapping is
        // not that path.
        let rehashed = hex::encode(sha2::Sha256::digest(hex_digest.as_bytes()));
        assert_ne!(
            identity.semantic_digest.as_str(),
            rehashed,
            "the mapping must not re-hash the hex string"
        );
        // 1:1 field mapping for the remaining dimensions.
        assert_eq!(identity.tenant, "tenant-a");
        assert_eq!(identity.nodes, 10);
        assert_eq!(identity.edges, 4);

        // Lineage: two mappings of the same snapshot verify equal, and a
        // caller holding the raw semantic hex constructs the same domain
        // identity directly (expected == actual preserved, R091).
        let again = snapshot_to_graph_identity(&snapshot(&hex_digest)).unwrap();
        assert!(da_domain::agent_verify_identity(&identity, &again).is_ok());

        let direct = GraphIdentity::new(
            ContentDigest::try_from(hex_digest.clone()).unwrap(),
            "tenant-a",
            10,
            4,
        );
        assert!(da_domain::agent_verify_identity(&identity, &direct).is_ok());
    }

    #[test]
    fn snapshot_to_graph_identity_fails_closed_on_malformed_hex() {
        // Fail-closed: a malformed semantic hex is a typed Identity error,
        // never a panic and never a silently wrong digest.
        let bad_hexes = vec![
            String::new(),
            "abc".to_string(),
            "A".repeat(64), // uppercase is not lowercase hex
            "a".repeat(63), // wrong length
        ];
        for bad in &bad_hexes {
            let err = snapshot_to_graph_identity(&snapshot(bad)).unwrap_err();
            assert!(
                matches!(err, AgentReadPlaneError::Identity(_)),
                "bad hex {bad:?} must be a typed Identity error"
            );
        }
    }

    #[test]
    fn graph_store_error_classifier_maps_typed_variants() {
        // Query-level failures stay Query.
        assert!(matches!(
            AgentReadPlaneError::from(GraphStoreError::NotFound("nope".into())),
            AgentReadPlaneError::Query(_)
        ));
        assert!(matches!(
            AgentReadPlaneError::from(GraphStoreError::Query("boom".into())),
            AgentReadPlaneError::Query(_)
        ));
        assert!(matches!(
            AgentReadPlaneError::from(GraphStoreError::Vector("v".into())),
            AgentReadPlaneError::Query(_)
        ));
        // The universal client surfaces connect/timeout/size-limit as
        // Storage; the classifier lifts them to Transport so the use case
        // sees reachability distinctly from capability.
        assert!(matches!(
            AgentReadPlaneError::from(GraphStoreError::Storage("timeout".into())),
            AgentReadPlaneError::Transport(_)
        ));
        // NotImplemented is a storage-capability error (read-only plane).
        assert!(matches!(
            AgentReadPlaneError::from(GraphStoreError::NotImplemented("import".into())),
            AgentReadPlaneError::Storage(_)
        ));
        // Identity mismatches are Identity errors carrying both sides.
        let detail = IdentityMismatchDetail {
            expected_semantic_sha256: "e".repeat(64),
            expected_tenant: "tenant-a".into(),
            expected_nodes: 10,
            expected_edges: 4,
            actual_semantic_sha256: "a".repeat(64),
            actual_tenant: "tenant-b".into(),
            actual_nodes: 12,
            actual_edges: 5,
        };
        let classified =
            AgentReadPlaneError::from(GraphStoreError::IdentityMismatch(Box::new(detail)));
        assert!(matches!(classified, AgentReadPlaneError::Identity(_)));
        // The reason names both sides (R087).
        let reason = classified.to_string();
        assert!(reason.contains("tenant-a"));
        assert!(reason.contains("tenant-b"));
        assert!(reason.contains("10 nodes"));
        assert!(reason.contains("12 nodes"));
    }

    #[test]
    fn agent_read_plane_error_renders_stable_messages() {
        let transport = AgentReadPlaneError::Transport("connect refused".into());
        assert!(transport.to_string().contains("transport"));
        assert!(transport.to_string().contains("connect refused"));

        let identity = AgentReadPlaneError::Identity("DA.IDENTITY timed out".into());
        assert!(identity.to_string().contains("identity"));
    }

    #[test]
    fn synthesizer_trait_is_object_safe_and_sync() {
        // The policy-port convention: a plain struct implements the trait and
        // the use case consumes it behind &dyn AgentSynthesizer. This test
        // pins object safety + Send + Sync (the use case stores it).
        struct GroundedSynthesizer;
        impl AgentSynthesizer for GroundedSynthesizer {
            fn synthesize(&self, bundle: &ContextBundle) -> Vec<AgentClaim> {
                bundle
                    .evidence
                    .iter()
                    .map(|entry| AgentClaim::new("claim", entry.result_ref.clone()))
                    .collect()
            }
        }
        let synthesizer: Box<dyn AgentSynthesizer> = Box::new(GroundedSynthesizer);
        let bundle = ContextBundle::new(GraphIdentity::new(
            ContentDigest::try_from("a".repeat(64)).unwrap(),
            "tenant-a",
            10,
            4,
        ));
        let claims = synthesizer.synthesize(&bundle);
        assert!(claims.is_empty(), "no evidence -> no claims");
    }
}
