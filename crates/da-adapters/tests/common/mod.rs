// Shared test support. Every integration test target that declares `mod common;`
// compiles this whole module tree, so helpers used by one target are "never used"
// from the point of view of the others. The pre-commit hook runs
// `cargo clippy --lib --tests --no-deps -- -D warnings`, which would promote those
// per-target dead-code warnings into hard errors (M430 class). Allowing dead_code
// here is the standard treatment for a shared test-helper module; it never applies
// to production code.
#![allow(dead_code)]

pub mod lexicon_contract;
pub mod parse_job_contract;
