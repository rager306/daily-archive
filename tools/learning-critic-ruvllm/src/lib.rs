//! Standalone deterministic ruVLLM learning critic (D471
//! `learning.ruvllm_ablation`, ADR-075 §2.7; Э1 of the integration plan).
//!
//! Produces `parse-learning-ablation.v1` critic observations without a GGUF
//! model, network or nondeterminism: the vendored ruvllm reflection
//! perspectives are pure heuristics folded by [`UnifiedCritique::combine`].
//! This crate is intentionally OUTSIDE the da-* workspace: the ruvllm stack
//! links `hnsw_rs` 0.3 while the samyama chain links 0.2, and the two must
//! never meet in one link unit (producer_independence by construction).
//!
//! Only redacted, bounded subjects cross this boundary.

pub mod critic;
pub use critic::{
    CriticCase, CriticError, CriticObservation, CriticStatus, ProducerProvenance,
    RuvllmReflectionCritic, identity_digest,
};
