//! RuVector bridge stub — graph-aware embeddings via RuVector.
//!
//! Phase C stub: the full RuVector integration (PPR, message passing,
//! GNN forward pushes) will land when the vendor crate dependency is
//! wired. For now this module is a compile-time placeholder.

/// RuVector bridge — connects kg-embeddings to the RuVector vendor
/// crate for graph-aware vector operations (Personalized PageRank,
/// message passing, GNN forward).
///
/// Full implementation deferred to a follow-up slice when the
/// ruvector crate is properly wired as a dependency.
pub struct RuVectorBridge;

impl RuVectorBridge {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RuVectorBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_equals_default() {
        let a = RuVectorBridge::new();
        let b = RuVectorBridge::default();
        // RuVectorBridge is a zero-sized type; both are equal.
        let _ = (a, b); // compile-time check
    }

    #[test]
    fn test_default_works() {
        let _bridge = RuVectorBridge::default();
    }
}
