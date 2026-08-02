//! Universal retry policy for pipeline scheduling and healing.
//!
//! Any system that retries failed operations with exponential backoff
//! can use this type. Extracted from da-domain in M289-u4fzx5/S05.

use serde::{Deserialize, Serialize};

/// Retry policy with exponential backoff.
///
/// Default: 1 day initial delay, 3x backoff, 30-day cap, 10 retries.
/// This matches the daily-archive paper-ingestion cadence but the
/// struct is generic — any pipeline can customise the fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Initial delay in seconds before the first retry.
    pub initial_delay: i64,
    /// Backoff multiplier applied after each failure.
    pub backoff_multiplier: f64,
    /// Maximum delay in seconds between retries.
    pub max_delay: i64,
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            initial_delay: 86400,    // 1 day
            backoff_multiplier: 3.0, // 1d → 3d → 9d → 27d
            max_delay: 30 * 86400,   // 30 days cap
            max_retries: 10,
        }
    }
}

impl RetryPolicy {
    /// Compute the delay for the n-th retry (0-indexed).
    /// Clamped to `max_delay`.
    pub fn delay_for_retry(&self, retry_index: u32) -> i64 {
        if retry_index >= self.max_retries {
            return self.max_delay;
        }
        let factor = self.backoff_multiplier.powi(retry_index as i32);
        let delay = (self.initial_delay as f64 * factor) as i64;
        delay.min(self.max_delay)
    }

    /// True if the retry index is within the allowed count.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Create a fast policy for testing: 1s initial, 2x backoff, 10s cap.
    pub fn fast() -> Self {
        Self {
            initial_delay: 1,
            backoff_multiplier: 2.0,
            max_delay: 10,
            max_retries: 5,
        }
    }

    /// Calculate next retry timestamp given current retry count.
    pub fn next_retry_ts(&self, retry_count: u32, now: i64) -> i64 {
        now + self.delay_for_retry(retry_count)
    }

    /// Should we give up on this task? (inverse of should_retry)
    pub fn should_give_up(&self, retry_count: u32) -> bool {
        !self.should_retry(retry_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let p = RetryPolicy::default();
        assert_eq!(p.initial_delay, 86400);
        assert_eq!(p.backoff_multiplier, 3.0);
        assert_eq!(p.max_delay, 30 * 86400);
        assert_eq!(p.max_retries, 10);
    }

    #[test]
    fn test_delay_for_retry_exponential() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_for_retry(0), 86400);  // 1 day
        assert_eq!(p.delay_for_retry(1), 259200); // 3 days
        assert_eq!(p.delay_for_retry(2), 777600); // 9 days
    }

    #[test]
    fn test_delay_clamped_to_max() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_for_retry(10), p.max_delay);
        assert_eq!(p.delay_for_retry(100), p.max_delay);
    }

    #[test]
    fn test_should_retry() {
        let p = RetryPolicy::default();
        assert!(p.should_retry(0));
        assert!(p.should_retry(9));
        assert!(!p.should_retry(10));
    }

    #[test]
    fn test_fast_policy() {
        let p = RetryPolicy::fast();
        assert_eq!(p.initial_delay, 1);
        assert_eq!(p.delay_for_retry(0), 1);
        assert_eq!(p.delay_for_retry(1), 2);
        assert_eq!(p.delay_for_retry(2), 4);
        assert_eq!(p.max_delay, 10);
    }

    #[test]
    fn test_serde_round_trip() {
        let p = RetryPolicy::fast();
        let json = serde_json::to_string(&p).unwrap();
        let back: RetryPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(p.initial_delay, back.initial_delay);
        assert_eq!(p.max_retries, back.max_retries);
    }
}
