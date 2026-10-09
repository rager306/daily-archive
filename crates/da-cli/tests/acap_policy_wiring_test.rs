//! S07/T06: the da-cli production composition reads the ACAP admission policy
//! from the single DATA declaration, not the pinned canonical default.
//!
//! Integration tests over the public `da_cli::production` surface: the default
//! load resolves the bundled
//! `scripts/governor/checks/article_context_assurance.yaml` and admits the
//! `parsed -> summarized` forward layer skip with its typed reason; a
//! `DA_ACAP_POLICY` override that omits the `admission` block (or points at a
//! missing file) fails closed with an error instead of falling back to a
//! default policy.

use std::ffi::OsStr;
use std::sync::{Mutex, MutexGuard};

use da_cli::production::ACAP_POLICY_ENV;
use da_domain::{ArticleContextState, admit_stage_transition_with_policy};

/// Serializes the tests that mutate `DA_ACAP_POLICY` so the process-global
/// environment does not race between them.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard: hold the env lock and restore `DA_ACAP_POLICY` on drop, so a
/// mutation leaks neither into another test nor into the rest of the binary.
struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: every test that touches this key holds ENV_LOCK, so no other
        // thread in this binary reads or writes the environment concurrently.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }

    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: see `unset`.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            // SAFETY: see `unset`.
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

#[test]
fn default_env_load_reads_the_data_declaration_and_admits_forward_layer_skip() {
    let _serial = EnvGuard::lock();
    let _guard = EnvGuard::unset(ACAP_POLICY_ENV);

    let policy = da_cli::production::load_acap_admission_policy_from_env()
        .expect("the bundled ACAP declaration loads with no override");

    // The loaded value is the DATA declaration, not a defaulted stub: the
    // ordered state list plus the declared durability rules.
    assert_eq!(policy.order().len(), 15);
    assert!(policy.evidence_required());
    assert!(policy.skip_requires_typed_reason());

    // The divergence this task wires shut: the production runtime admits the
    // `parsed -> summarized` forward layer skip with the typed reason the
    // declaration's `skip_requires_typed_reason` rule requires.
    let decision = admit_stage_transition_with_policy(
        &policy,
        Some(ArticleContextState::Parsed),
        ArticleContextState::Summarized,
        "sha256:journey-parse-evidence",
        Some("acap_skip:Parsed->Summarized"),
    );
    assert!(decision.admitted, "forward layer skip must be admitted");
    assert!(decision.error.is_none());
    assert_eq!(
        decision.skip_reason.as_deref(),
        Some("acap_skip:Parsed->Summarized")
    );
}

#[test]
fn override_without_admission_block_fails_closed() {
    let _serial = EnvGuard::lock();
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("acap-without-admission.yaml");
    std::fs::write(
        &path,
        "fsm:\n  initial_state: source_registered\n  terminal_states: [accepted_checkpoint, failed_visible]\n  states: [source_registered, source_verified, parsed, structured, extracted, evidence_linked, embedded, materialized, enriched, resolved, summarized, finalized, task_accepted, accepted_checkpoint, failed_visible]\n  transitions: []\n  failure_transition_from: \"*\"\n  failure_transition_to: failed_visible\n  retry_requires_same_semantic_key: true\n",
    )
    .expect("write declaration without an admission block");
    let _guard = EnvGuard::set(ACAP_POLICY_ENV, &path);

    let error = da_cli::production::load_acap_admission_policy_from_env()
        .expect_err("a declaration without an admission block must fail closed");
    assert!(error.contains("has no admission block"), "{error}");
}

#[test]
fn override_pointing_at_a_missing_file_fails_closed() {
    let _serial = EnvGuard::lock();
    let _guard = EnvGuard::set(ACAP_POLICY_ENV, "/nonexistent/acap-policy.yaml");

    let error = da_cli::production::load_acap_admission_policy_from_env()
        .expect_err("a missing override path must fail closed");
    assert!(error.contains("stat ACAP admission policy"), "{error}");
}
