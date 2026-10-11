//! Standalone deterministic ruVLLM learning critic (D471
//! `learning.ruvllm_ablation`, ADR-075 §2.7; Э1 of the integration plan).
//!
//! Produces `parse-learning-ablation.v1` critic observations without a GGUF
//! model, network or nondeterminism: the vendored ruvllm reflection
//! perspectives are pure heuristics folded by [`UnifiedCritique::combine`],
//! then calibrated against the structural evidence of the redacted subject
//! (audit Н-1: the vendor perspectives alone scored a degraded, all-zero
//! profile 1.0 because they only detect *absence of badness* markers).
//! This crate is intentionally OUTSIDE the da-* workspace: the ruvllm stack
//! links `hnsw_rs` 0.3 while the samyama chain links 0.2, and the two must
//! never meet in one link unit (producer_independence by construction).
//!
//! Only redacted, bounded subjects cross this boundary.

pub mod critic;
pub mod gguf_critic;
pub mod models;
pub mod subject;

pub use critic::{
    identity_digest, CriticCase, CriticError, CriticObservation, CriticStatus, ProducerProvenance,
    RuvllmReflectionCritic,
};
pub use gguf_critic::{GenerationStats, GgufCritic, ObserveOutput, Verdict, VerdictClass};
pub use models::{resolve as resolve_model, ModelSpec, MODELS};
pub use subject::{derive_subject, StructuralProfile};
