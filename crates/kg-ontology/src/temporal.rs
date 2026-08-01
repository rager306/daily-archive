//! Temporal edge model (ADR-046 revised, GRAPH-CORE-TEMPORAL-DESIGN.md).
//!
//! Entities persist. Facts change. Temporality lives on edges.
//!
//! Every temporal edge carries a 5-field model:
//!   valid_at      — when the fact became true in the real world
//!   invalid_at    — when the fact stopped being true (OPEN = still true)
//!   expired_at    — when the system invalidated this edge (OPEN = active)
//!   reference_time — timestamp from the source episode
//!   created_at    — when the system first wrote this edge
//!
//! Optional extension fields for retroactivity-aware domains (law, accounting):
//!   retroactive_to — fact applies retroactively from this date
//!   retroactivity_basis — legal/operational basis for retroactivity
//!
//! Two temporal axes:
//!   valid time (valid_at → invalid_at): when the fact was true in the world
//!   transaction time (created_at → expired_at): when the system knew it

use chrono::{DateTime, Utc};

/// Sentinel string value for open temporal bounds (invalid_at, expired_at).
/// Matches Semantica's TemporalBound::OPEN convention; serializable in
/// Samyama's schemaless store.
pub const OPEN: &str = "OPEN";

/// The 5-field temporal edge model. Every temporal edge carries these
/// fields. Non-temporal edges (HAS_PART, FROM_SOURCE) carry only
/// `created_at`.
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalEdge {
    /// When the fact became true in the real world.
    pub valid_at: DateTime<Utc>,
    /// When the fact stopped being true. None = still true.
    pub invalid_at: Option<DateTime<Utc>>,
    /// When the system invalidated this edge. None = active.
    pub expired_at: Option<DateTime<Utc>>,
    /// Timestamp from the source episode that produced this edge.
    pub reference_time: Option<DateTime<Utc>>,
    /// When the system first wrote this edge (auto-set at creation).
    pub created_at: DateTime<Utc>,
}

/// Optional retroactivity extension for domains where facts can have
/// retroactive effect (law, accounting, regulatory compliance).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RetroactiveExtension {
    /// Fact applies retroactively from this date (earlier than valid_at).
    pub retroactive_to: Option<DateTime<Utc>>,
    /// Legal/operational basis for retroactivity.
    pub retroactivity_basis: Option<String>,
}

impl TemporalEdge {
    /// Create a new temporal edge starting now. `valid_at` defaults to
    /// the current time; callers should override with the source's
    /// actual `valid_at` when known.
    pub fn new_now() -> Self {
        let now = Utc::now();
        Self {
            valid_at: now,
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: now,
        }
    }

    /// True if the fact was true at the given instant.
    /// Checks the valid-time axis: valid_at <= t < invalid_at.
    /// If `retroactive_to` is provided, uses it instead of valid_at.
    pub fn is_active_at(&self, at: DateTime<Utc>, retro: Option<&RetroactiveExtension>) -> bool {
        let effective_start = retro
            .and_then(|r| r.retroactive_to)
            .unwrap_or(self.valid_at);
        let effective_end = self.invalid_at;
        match effective_end {
            Some(end) => effective_start <= at && at < end,
            None => effective_start <= at,
        }
    }

    /// True if the system knew about this fact at the given instant.
    /// Checks the transaction-time axis: created_at <= t < expired_at.
    pub fn was_known_at(&self, at: DateTime<Utc>) -> bool {
        match self.expired_at {
            Some(exp) => self.created_at <= at && at < exp,
            None => self.created_at <= at,
        }
    }

    /// True if the edge is currently active (valid time) and known
    /// (transaction time). Convenience combining both axes at now().
    pub fn is_current(&self, retro: Option<&RetroactiveExtension>) -> bool {
        let now = Utc::now();
        self.is_active_at(now, retro) && self.was_known_at(now)
    }

    /// True if the edge has been superseded (expired_at is set).
    pub fn is_expired(&self) -> bool {
        self.expired_at.is_some()
    }

    /// Invalidate this edge at the given time. Sets expired_at and,
    /// if invalid_at is not already set, sets invalid_at to the same
    /// timestamp. Used by `resolve_temporal_edges` when a newer edge
    /// supersedes this one.
    pub fn invalidate(&mut self, at: DateTime<Utc>) {
        if self.invalid_at.is_none() {
            self.invalid_at = Some(at);
        }
        self.expired_at = Some(at);
    }
}

/// Validate temporal consistency on a TemporalEdge.
/// Returns a list of human-readable issues (empty = consistent).
///
/// Rules:
///   1. valid_at <= invalid_at (if both set)
///   2. created_at <= expired_at (if both set)
///   3. created_at >= valid_at is NOT required — extraction can lag
///      publication (soft, not flagged)
///   4. retroactive_to <= valid_at (if both set)
pub fn validate_temporal_edge(
    edge: &TemporalEdge,
    retro: Option<&RetroactiveExtension>,
) -> Vec<String> {
    let mut issues = Vec::new();

    if let Some(invalid) = edge.invalid_at
        && edge.valid_at > invalid
    {
        issues.push(format!(
            "valid_at ({}) is later than invalid_at ({})",
            edge.valid_at, invalid
        ));
    }

    if let Some(expired) = edge.expired_at
        && edge.created_at > expired
    {
        issues.push(format!(
            "created_at ({}) is later than expired_at ({})",
            edge.created_at, expired
        ));
    }

    if let Some(r) = retro
        && let Some(retro_to) = r.retroactive_to
        && retro_to > edge.valid_at
    {
        issues.push(format!(
            "retroactive_to ({}) is later than valid_at ({}) — retroactivity must point backwards",
            retro_to, edge.valid_at
        ));
    }

    issues
}

/// Parse a DateTime from a string (ISO-8601) or integer (Unix seconds).
/// Returns None for empty strings, null, or the OPEN sentinel.
pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == OPEN {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| s.parse::<i64>().ok().and_then(|secs| DateTime::from_timestamp(secs, 0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    #[test]
    fn test_new_now_has_no_gaps() {
        let edge = TemporalEdge::new_now();
        assert!(edge.invalid_at.is_none());
        assert!(edge.expired_at.is_none());
        assert!(edge.is_current(None));
    }

    #[test]
    fn test_is_active_at_within_range() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: Some(ts(2000)),
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        assert!(edge.is_active_at(ts(1500), None));
        assert!(!edge.is_active_at(ts(500), None));
        assert!(!edge.is_active_at(ts(2500), None));
    }

    #[test]
    fn test_is_active_at_open_ended() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        assert!(edge.is_active_at(ts(999_999_999), None));
    }

    #[test]
    fn test_was_known_at_transaction_range() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: None,
            expired_at: Some(ts(3000)),
            reference_time: None,
            created_at: ts(2000),
        };
        assert!(!edge.was_known_at(ts(1500))); // not yet written
        assert!(edge.was_known_at(ts(2500))); // written, not expired
        assert!(!edge.was_known_at(ts(3500))); // expired
    }

    #[test]
    fn test_retroactive_effective_start() {
        let edge = TemporalEdge {
            valid_at: ts(2000),
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: ts(2000),
        };
        let retro = RetroactiveExtension {
            retroactive_to: Some(ts(1000)),
            retroactivity_basis: Some("ФЗ-xxx ст.4".to_string()),
        };
        // Before valid_at but after retroactive_to → active
        assert!(edge.is_active_at(ts(1500), Some(&retro)));
        // Before retroactive_to → not active
        assert!(!edge.is_active_at(ts(500), Some(&retro)));
    }

    #[test]
    fn test_invalidate_sets_both_fields() {
        let mut edge = TemporalEdge::new_now();
        edge.invalidate(ts(5000));
        assert_eq!(edge.invalid_at, Some(ts(5000)));
        assert_eq!(edge.expired_at, Some(ts(5000)));
        assert!(edge.is_expired());
    }

    #[test]
    fn test_invalidate_preserves_existing_invalid_at() {
        let mut edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: Some(ts(1500)),
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        edge.invalidate(ts(5000));
        // invalid_at was already set → preserved
        assert_eq!(edge.invalid_at, Some(ts(1500)));
        // expired_at gets the invalidation timestamp
        assert_eq!(edge.expired_at, Some(ts(5000)));
    }

    #[test]
    fn test_validate_temporal_edge_consistent() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: Some(ts(2000)),
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        assert!(validate_temporal_edge(&edge, None).is_empty());
    }

    #[test]
    fn test_validate_temporal_edge_inverted_valid_range() {
        let edge = TemporalEdge {
            valid_at: ts(2000),
            invalid_at: Some(ts(1000)),
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        let issues = validate_temporal_edge(&edge, None);
        assert!(issues.iter().any(|s| s.contains("later than invalid_at")));
    }

    #[test]
    fn test_validate_temporal_edge_inverted_transaction_range() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: None,
            expired_at: Some(ts(500)),
            reference_time: None,
            created_at: ts(1000),
        };
        let issues = validate_temporal_edge(&edge, None);
        assert!(issues.iter().any(|s| s.contains("later than expired_at")));
    }

    #[test]
    fn test_validate_retroactive_to_must_be_before_valid_at() {
        let edge = TemporalEdge {
            valid_at: ts(1000),
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: ts(1000),
        };
        let retro = RetroactiveExtension {
            retroactive_to: Some(ts(2000)), // wrong: retroactive should be earlier
            retroactivity_basis: None,
        };
        let issues = validate_temporal_edge(&edge, Some(&retro));
        assert!(issues.iter().any(|s| s.contains("retroactivity must point backwards")));
    }

    #[test]
    fn test_parse_datetime_iso8601() {
        let dt = parse_datetime("2024-01-15T10:30:00Z");
        assert!(dt.is_some());
    }

    #[test]
    fn test_parse_datetime_unix_seconds() {
        let dt = parse_datetime("1700000000");
        assert!(dt.is_some());
    }

    #[test]
    fn test_parse_datetime_open_returns_none() {
        assert!(parse_datetime(OPEN).is_none());
        assert!(parse_datetime("").is_none());
    }
}
