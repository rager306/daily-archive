//! FSM policy port (ADR-074 Layer 2).
//!
//! Domain types live in da-domain. Implementations: native fallback
//! (da-application) and WASM host (da-adapters). Application never
//! depends on wasmi.

use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint, PolicyDecision};

#[derive(Debug, thiserror::Error)]
pub enum FsmPolicyError {
    #[error("fsm policy: unknown decision point {0}")]
    UnknownPoint(String),
    #[error("fsm policy: module load failed: {0}")]
    Load(String),
    #[error("fsm policy: instantiate failed: {0}")]
    Instantiate(String),
    #[error("fsm policy: trap: {0}")]
    Trap(String),
    #[error("fsm policy: invalid result code {0}")]
    InvalidCode(i32),
    #[error("fsm policy: io: {0}")]
    Io(String),
}

pub type FsmPolicyResult<T> = Result<T, FsmPolicyError>;

/// Consult a policy at a named FSM decision point.
///
/// Implementations MUST stamp [`PolicyDecision::id.source`] with either
/// `Native` or `Wasm { sha256 }` so trajectory records can replay the
/// exact policy artifact.
pub trait FsmPolicyPort: Send + Sync {
    fn consult(
        &self,
        point: DecisionPoint,
        features: &ArticleFeatures,
    ) -> FsmPolicyResult<PolicyDecision>;

    /// Reload artifact from disk (no-op for native). Hash change = hot-swap.
    fn reload(&mut self) -> FsmPolicyResult<String> {
        Ok("native".into())
    }

    fn policy_hash(&self) -> String {
        "native".into()
    }
}
