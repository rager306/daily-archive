//! S03/T03 (M514-1lss2s) — typed Java-invocation observation on the
//! `ParseBackend::fetch_tei` transport seam (R140).
//!
//! R140 requires per-stage Java invocation counters. Before this module the
//! parse plane had no runtime counter of Java consults at all: the only
//! document-level number was `ParseBatchRunSummary::backend_calls` (one call
//! per admitted document), and the per-model counts in
//! [`crate::debug_tape::model_invocation_counts`] come from a *recorded* tape,
//! not from a live attempt. A counter that cannot grow is vacuous, so this
//! observer attaches to the real Java seam — the single
//! [`crate::backend::ParseBackend::fetch_tei`] attempt on
//! [`crate::http::GrobidHttpBackend`] — rather than to a native lane.
//!
//! Two properties are deliberate and load-bearing:
//!
//! - **One consult is recorded per attempt, before any wire bytes exist.**
//!   The count is incremented as the first statement of `fetch_tei`, so an
//!   unreachable endpoint still yields `Observed(> 0)`. This is what makes the
//!   counter non-vacuous.
//! - **Absence of measurement is a typed [`JavaInvocationObservation::NotObserved`],
//!   never a fabricated zero.** A backend with no installed observer reports
//!   `NotObserved`; only an *installed* observer may report
//!   `Observed { attempts: 0 }`. The distinction is the same "a zero is not a
//!   measurement" rule carried by
//!   [`crate::batch_budget::BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT`].
//!
//! The observer is **transport tuning**: it is never an input to
//! [`crate::http::GrobidHttpBackend::identity`] and never a request field, so
//! installing one cannot rotate a cache key. It uses `std` only — no new
//! dependency, no env, no filesystem, no clock.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the transport seam can say about Java consult attempts.
///
/// `NotObserved` is a typed absence, not a zero: an uninstalled observer must
/// never be readable as "0 consults" (the same discipline as
/// [`crate::batch_budget::BatchObservation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaInvocationObservation {
    /// No observer is installed on this seam, so nothing was measured.
    NotObserved,
    /// An observer is installed; `attempts` counts the transport-level Java
    /// consult attempts recorded since installation. Zero here is a real
    /// measurement under an installed guard.
    Observed { attempts: u64 },
}

impl JavaInvocationObservation {
    /// Whether a number was measured at all (i.e. an observer was installed).
    pub fn is_observed(self) -> bool {
        matches!(self, Self::Observed { .. })
    }

    /// The measured attempt count, or `None` for the typed absence.
    pub fn attempts(self) -> Option<u64> {
        match self {
            Self::Observed { attempts } => Some(attempts),
            Self::NotObserved => None,
        }
    }

    /// Stable snake_case name (diagnostics only, never identity).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotObserved => "not_observed",
            Self::Observed { .. } => "observed",
        }
    }
}

/// Shared, monotonic, `std`-only counter of Java consult attempts.
///
/// Cloning shares the same atomic, so a caller keeps one handle while the
/// transport seam owns another. Creating a counter *is* installing a guard:
/// once it is handed to
/// [`crate::http::GrobidHttpBackend::with_invocation_observer`], a zero read
/// back means "measured zero attempts", not "unmeasured".
#[derive(Debug, Clone, Default)]
pub struct JavaInvocationCounter {
    attempts: Arc<AtomicUsize>,
}

impl JavaInvocationCounter {
    /// A fresh counter at zero, not yet attached to any seam.
    pub fn new() -> Self {
        Self {
            attempts: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Record exactly one consult attempt and return the new total.
    ///
    /// Called by the transport seam once per `fetch_tei` attempt.
    pub fn observe_attempt(&self) -> u64 {
        let previous = self.attempts.fetch_add(1, Ordering::SeqCst);
        previous as u64 + 1
    }

    /// This guard's observation. A counter exists, so it is always
    /// `Observed` — its zero is a measurement rather than an absence.
    pub fn observation(&self) -> JavaInvocationObservation {
        JavaInvocationObservation::Observed {
            attempts: self.attempts.load(Ordering::SeqCst) as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{ParseBackend, ParseBackendRequest};
    use crate::http::GrobidHttpBackend;
    use crate::profile::GrobidProfile;
    use crate::transport::FailureCode;

    const PDF_FIXTURE: &[u8] = b"%PDF-1.4 java-invocation observer fixture";

    /// A backend whose request can never reach the wire: `http://` carries an
    /// empty authority, so the request fails at URL parsing and `send()` returns
    /// a typed transport failure. There is no socket, no DNS, and no
    /// bind-then-drop port to race — the observation is deterministic.
    fn unreachable_backend() -> GrobidHttpBackend {
        GrobidHttpBackend::new(Some("http://")).with_profile(GrobidProfile::structural())
    }

    fn request() -> ParseBackendRequest {
        ParseBackendRequest::new("m514-slice-doc", PDF_FIXTURE.to_vec())
    }

    #[test]
    fn counter_is_monotonic_and_clone_shared() {
        let counter = JavaInvocationCounter::new();
        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 0 },
            "an installed guard reads Observed(0), not NotObserved"
        );

        counter.observe_attempt();
        let shared = counter.clone();
        shared.observe_attempt();
        shared.observe_attempt();

        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 3 },
            "clones share one atomic counter"
        );
        assert_eq!(counter.observation().attempts(), Some(3));
        assert!(counter.observation().is_observed());
    }

    #[test]
    fn typed_absence_is_never_a_zero() {
        assert!(!JavaInvocationObservation::NotObserved.is_observed());
        assert_eq!(JavaInvocationObservation::NotObserved.attempts(), None);
        assert_eq!(
            JavaInvocationObservation::NotObserved.as_str(),
            "not_observed"
        );
        assert_eq!(
            JavaInvocationObservation::Observed { attempts: 0 }.as_str(),
            "observed"
        );
    }

    #[tokio::test]
    async fn one_attempt_is_counted_before_any_wire_bytes_on_an_unreachable_endpoint() {
        let counter = JavaInvocationCounter::new();
        let backend = unreachable_backend().with_invocation_observer(counter.clone());

        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 0 }
        );

        let error = backend
            .fetch_tei(request())
            .await
            .expect_err("an unroutable endpoint must fail");

        assert_eq!(
            error.code(),
            FailureCode::Transport,
            "an unroutable request is a transport failure (got {error})"
        );
        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 1 },
            "the attempt is counted even though no wire exchange completed"
        );
        assert_eq!(
            backend.java_invocation_observation(),
            JavaInvocationObservation::Observed { attempts: 1 },
            "the seam reports the installed guard's measurement"
        );
    }

    #[tokio::test]
    async fn without_an_observer_the_attempt_is_not_observed_never_a_zero() {
        let backend = unreachable_backend();

        assert_eq!(
            backend.java_invocation_observation(),
            JavaInvocationObservation::NotObserved,
            "an uninstalled observer must not read as Observed(0)"
        );

        let _ = backend.fetch_tei(request()).await;

        assert_eq!(
            backend.java_invocation_observation(),
            JavaInvocationObservation::NotObserved,
            "a completed attempt on an unwatched seam is still unmeasured"
        );
    }

    #[test]
    fn observer_is_transport_tuning_and_never_enters_identity() {
        let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
            .with_profile(GrobidProfile::structural());
        let watched = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
            .with_profile(GrobidProfile::structural())
            .with_invocation_observer(JavaInvocationCounter::new());

        assert_eq!(
            plain.identity(),
            watched.identity(),
            "installing an observer must not rotate the cache identity"
        );
        assert_eq!(plain.profile(), watched.profile());
    }

    #[tokio::test]
    async fn every_attempt_is_counted_once() {
        let counter = JavaInvocationCounter::new();
        let backend = unreachable_backend().with_invocation_observer(counter.clone());

        for _ in 0..3 {
            let _ = backend.fetch_tei(request()).await;
        }

        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 3 },
            "each fetch_tei attempt increments the counter exactly once"
        );
    }

    /// A bound observer is shared, not moved: cloning the transport and then
    /// dropping the original handle must keep the same count. This is what
    /// makes the seam's observation safe against a bind/drop race — the
    /// `Arc<AtomicUsize>` is the only shared state and nothing re-binds it.
    #[tokio::test]
    async fn observer_survives_a_backend_clone_and_a_dropped_handle() {
        let counter = JavaInvocationCounter::new();
        let backend = unreachable_backend().with_invocation_observer(counter.clone());
        let clone = backend.clone();

        let _ = clone.fetch_tei(request()).await;
        drop(backend);

        assert_eq!(
            counter.observation(),
            JavaInvocationObservation::Observed { attempts: 1 },
            "dropping one bound handle must not lose the shared count"
        );
        assert_eq!(
            clone.java_invocation_observation(),
            JavaInvocationObservation::Observed { attempts: 1 }
        );
    }
}
