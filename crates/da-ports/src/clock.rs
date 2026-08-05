//! Clock port — ADR-051 Contour 1 Determinism Infrastructure (M330).
//!
//! Application code must not call `chrono::Utc::now()` directly when writing
//! TemporalEdge fields or other time-stamped domain events. Inject [`Clock`]
//! so tests can freeze time with [`FixedClock`].

use chrono::{DateTime, Utc};

/// Port for wall-clock time (inject in use cases; never call Utc::now in domain logic).
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Production clock — real UTC wall time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Test clock — fixed instant (deterministic TemporalEdge stamps).
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub DateTime<Utc>);

impl FixedClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self(now)
    }

    /// Unix seconds convenience for tests.
    pub fn from_timestamp(secs: i64) -> Self {
        Self(DateTime::from_timestamp(secs, 0).expect("valid unix timestamp"))
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_returns_same_instant() {
        let t = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let clock = FixedClock::new(t);
        assert_eq!(clock.now(), t);
        assert_eq!(clock.now(), clock.now());
    }

    #[test]
    fn system_clock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SystemClock>();
        assert_send_sync::<FixedClock>();
        assert_send_sync::<Box<dyn Clock>>();
    }
}
