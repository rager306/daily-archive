//! SamyamaGraphStore adapter — bridges kg-storage traits to the
//! Samyama embedded graph engine.
//!
//! Phase B stub: the full adapter implementation (moved from
//! da-adapters/src/samyama_graph.rs) will land in a follow-up slice
//! when the dependency on the samyama-sdk crate is properly wired.
//! For now this module is a compile-time placeholder so the public
//! API is visible.

/// Samyama-backed GraphStore + DirectGraphStore implementation.
///
/// Wraps the vendor `samyama-sdk` store behind the universal kg-storage
/// traits. The full impl (GraphStore + DirectGraphStore trait
/// implementations) moves here from `da-adapters/src/samyama_graph.rs`
/// in a follow-up slice.
pub struct SamyamaGraphStore;

impl SamyamaGraphStore {
    /// Create a fresh instance. Full implementation will wrap the
    /// Samyama `EmbeddedClient` (see da-adapters for the reference impl).
    pub fn new() -> Self {
        Self
    }

    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self::new()
    }
}

impl Default for SamyamaGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

// TODO(phase-b-followup): move the full GraphStore + DirectGraphStore
// impl blocks from da-adapters/src/samyama_graph.rs here. The impl
// is ~500 lines and depends on the samyama-sdk crate; moving it
// requires confirming the exact samyama-sdk API surface (the current
// da-adapters code uses samyama::graph::* paths that differ from
// samyama-sdk's top-level re-exports).
