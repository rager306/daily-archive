//! EvidenceStore port (ADR-040 §13).
//!
//! Links graph nodes to immutable evidence artifacts.

use async_trait::async_trait;
use da_domain::evidence::{EvidenceAssertion, EvidenceId, EvidenceVerification};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EvidenceStoreError {
    #[error("Artifact missing: {0}")]
    ArtifactMissing(String),
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("Storage error: {0}")]
    Storage(String),
}

pub type EvidenceResult<T> = Result<T, EvidenceStoreError>;

/// Links graph nodes to immutable evidence artifacts.
/// Every Entity/Relation must have evidence before import_eligible.
#[async_trait]
pub trait EvidenceStore: Send + Sync {
    async fn store_assertion(
        &self,
        node_vid: &str,
        assertion: &EvidenceAssertion,
    ) -> EvidenceResult<EvidenceId>;

    async fn get_assertions(&self, node_vid: &str) -> EvidenceResult<Vec<EvidenceAssertion>>;

    async fn has_resolvable_evidence(&self, node_vid: &str) -> EvidenceResult<bool>;

    async fn verify_chain(&self, node_vid: &str) -> EvidenceResult<EvidenceVerification>;

    async fn nodes_without_evidence(&self, label: &str) -> EvidenceResult<Vec<String>>;
}
