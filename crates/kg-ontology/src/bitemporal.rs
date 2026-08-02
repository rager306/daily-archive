//! Node-level bi-temporal helpers (ADR-046, GRAPH-CORE-TEMPORAL-DESIGN.md).
//!
//! Fact-bearing nodes carry four temporal fields:
//!   valid_from    — when the fact became true in the real world
//!   valid_to      — when the fact stopped being true (OPEN = still true)
//!   recorded_at   — when our system first wrote this fact
//!   superseded_at — when our system stopped using this fact (OPEN = current)
//!
//! Together they answer two distinct time-travel questions:
//!   is_active_at(snapshot, t)  — was the fact true at time t?
//!   was_known_at(snapshot, t)  — did our system know it at time t?
//!
//! Universal: operates on generic `PropertyMap` = `HashMap<String, serde_json::Value>`.
//! No project-specific dependencies. Extracted from da-domain in M289-u4fzx5/S01.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Re-export OPEN sentinel for node-level code that imports from here.
pub use crate::temporal::OPEN;

/// A property map — generic JSON-like key-value store.
/// This is the universal equivalent of da-domain's `PropertySnapshot`.
pub type PropertyMap = HashMap<String, serde_json::Value>;

/// Parse a DateTime from a property map field. Returns None if the
/// field is missing, null, empty, or the OPEN sentinel.
///
/// Accepts Unix timestamps (i64 seconds since epoch) or ISO-8601 strings.
/// The two encodings coexist because graph stores may write either shape.
pub fn read_datetime(props: &PropertyMap, key: &str) -> Option<DateTime<Utc>> {
    let val = props.get(key)?;
    if val.is_null() {
        return None;
    }
    if let Some(s) = val.as_str() {
        if s.is_empty() || s == OPEN {
            return None;
        }
        return DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
    }
    if let Some(i) = val.as_i64() {
        return DateTime::from_timestamp(i, 0);
    }
    None
}

/// Read a temporal bound — either a DateTime or the OPEN sentinel.
/// Returns Ok(Some(dt)) for a concrete timestamp, Ok(None) for OPEN or
/// missing, Err for a malformed value.
pub fn read_bound(props: &PropertyMap, key: &str) -> Result<Option<DateTime<Utc>>, String> {
    let val = props.get(key);
    let Some(val) = val else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    if let Some(s) = val.as_str() {
        if s.is_empty() || s == OPEN {
            return Ok(None);
        }
        return DateTime::parse_from_rfc3339(s)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(|e| format!("invalid datetime at {key}: {e}"));
    }
    if let Some(i) = val.as_i64() {
        return DateTime::from_timestamp(i, 0)
            .map(Some)
            .ok_or_else(|| format!("out-of-range timestamp at {key}: {i}"));
    }
    Err(format!("expected datetime or OPEN at {key}, got {val}"))
}

/// True if the fact was true in the real world at the given instant.
/// Checks the valid-time axis: valid_from <= t < valid_to.
pub fn is_active_at(props: &PropertyMap, at: DateTime<Utc>) -> bool {
    let from = read_datetime(props, "valid_from");
    let to = read_bound(props, "valid_to").ok().flatten();
    match (from, to) {
        (Some(f), Some(t)) => f <= at && at < t,
        (Some(f), None) => f <= at,
        (None, Some(t)) => at < t,
        (None, None) => true,
    }
}

/// True if our system knew about the fact at the given instant.
/// Checks the transaction-time axis: recorded_at <= t < superseded_at.
pub fn was_known_at(props: &PropertyMap, at: DateTime<Utc>) -> bool {
    let recorded = read_datetime(props, "recorded_at");
    let superseded = read_bound(props, "superseded_at").ok().flatten();
    match (recorded, superseded) {
        (Some(r), Some(s)) => r <= at && at < s,
        (Some(r), None) => r <= at,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

/// True if the fact is currently active (valid time) AND currently known
/// (transaction time). Convenience combining is_active_at(now) and
/// was_known_at(now).
pub fn is_current(props: &PropertyMap) -> bool {
    let now = Utc::now();
    is_active_at(props, now) && was_known_at(props, now)
}

/// Validate bi-temporal consistency on a fact-bearing node snapshot.
/// Returns a list of human-readable issues (empty = consistent).
///
/// Rules:
///   1. If both valid_from and valid_to are set, valid_from <= valid_to.
///   2. If both recorded_at and superseded_at are set,
///      recorded_at <= superseded_at.
///   3. If recorded_at is set and valid_from is set, recorded_at >= valid_from
///      (we cannot record a fact before it became true). This is a soft
///      warning for paper-publication facts where extraction timestamp
///      naturally lags publication.
pub fn validate_bitemporal(props: &PropertyMap) -> Vec<String> {
    let mut issues = Vec::new();
    let from = read_datetime(props, "valid_from");
    let to = read_bound(props, "valid_to").ok().flatten();
    let recorded = read_datetime(props, "recorded_at");
    let superseded = read_bound(props, "superseded_at").ok().flatten();

    if let (Some(f), Some(t)) = (from, to)
        && f > t
    {
        issues.push(format!("valid_from ({f}) is later than valid_to ({t})"));
    }
    if let (Some(r), Some(s)) = (recorded, superseded)
        && r > s
    {
        issues.push(format!(
            "recorded_at ({r}) is later than superseded_at ({s})"
        ));
    }
    if let (Some(r), Some(f)) = (recorded, from)
        && r < f
    {
        issues.push(format!(
            "recorded_at ({r}) is earlier than valid_from ({f}) — extraction before publication (unusual but allowed)"
        ));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(pairs: &[(&str, serde_json::Value)]) -> PropertyMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_open_sentinel_recognized() {
        let props = map(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!(OPEN)),
        ]);
        assert!(read_bound(&props, "valid_to").unwrap().is_none());
    }

    #[test]
    fn test_is_active_at_within_range() {
        let props = map(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!(1_800_000_000_i64)),
        ]);
        let within = DateTime::from_timestamp(1_750_000_000, 0).unwrap();
        let before = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let after = DateTime::from_timestamp(1_900_000_000, 0).unwrap();
        assert!(is_active_at(&props, within));
        assert!(!is_active_at(&props, before));
        assert!(!is_active_at(&props, after));
    }

    #[test]
    fn test_is_active_at_open_ended() {
        let props = map(&[("valid_from", json!(1_700_000_000_i64))]);
        let now = Utc::now();
        assert!(is_active_at(&props, now));
    }

    #[test]
    fn test_was_known_at_within_transaction_range() {
        let props = map(&[
            ("recorded_at", json!(1_700_000_000_i64)),
            ("superseded_at", json!(1_800_000_000_i64)),
        ]);
        let within = DateTime::from_timestamp(1_750_000_000, 0).unwrap();
        let before = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let after = DateTime::from_timestamp(1_900_000_000, 0).unwrap();
        assert!(was_known_at(&props, within));
        assert!(!was_known_at(&props, before));
        assert!(!was_known_at(&props, after));
    }

    #[test]
    fn test_validate_bitemporal_consistent() {
        let props = map(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!(1_800_000_000_i64)),
            ("recorded_at", json!(1_700_500_000_i64)),
            ("superseded_at", json!(OPEN)),
        ]);
        let issues = validate_bitemporal(&props);
        assert!(issues.is_empty(), "expected no issues, got: {issues:?}");
    }

    #[test]
    fn test_validate_bitemporal_inverted_valid_range() {
        let props = map(&[
            ("valid_from", json!(1_800_000_000_i64)),
            ("valid_to", json!(1_700_000_000_i64)),
        ]);
        let issues = validate_bitemporal(&props);
        assert!(
            issues
                .iter()
                .any(|s| s.contains("valid_from") && s.contains("valid_to"))
        );
    }

    #[test]
    fn test_no_temporal_info_treated_as_active_and_known() {
        let props = PropertyMap::new();
        let now = Utc::now();
        assert!(is_active_at(&props, now));
        assert!(was_known_at(&props, now));
    }
}
