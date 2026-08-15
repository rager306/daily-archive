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

/// Canonical DateTime wire validator (PIR-008 / M398/S04).
///
/// Re-exported from [`crate::temporal`]: new writes must carry RFC3339
/// UTC strings; integer timestamps are rejected for writes while the
/// read paths below (`read_datetime`, `read_bound`) remain the bounded
/// compatibility decoder for legacy snapshots.
pub use crate::temporal::validate_datetime_wire;

/// A property map — generic JSON-like key-value store.
/// This is the universal equivalent of da-domain's `PropertySnapshot`.
pub type PropertyMap = HashMap<String, serde_json::Value>;

/// Parse a DateTime from a property map field. Returns None if the
/// field is missing, null, empty, or the OPEN sentinel.
///
/// READ path (bounded compatibility decoder): accepts both the canonical
/// RFC3339 UTC string form and legacy integer Unix timestamps (i64
/// seconds since epoch) so old snapshots stay readable. New writes are
/// validated by [`validate_datetime_wire`], which rejects the integer
/// form.
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

    // ── PIR-008 canonical wire (re-exported from temporal) ──────────────

    #[test]
    fn test_validate_datetime_wire_reexport() {
        // Canonical RFC3339 UTC string passes.
        assert!(validate_datetime_wire(&json!("2024-01-15T10:30:00Z"), "valid_from").is_ok());
        // Legacy integer form is rejected for NEW writes...
        let err = validate_datetime_wire(&json!(1_700_000_000_i64), "valid_from").unwrap_err();
        assert!(err.contains("legacy"), "unexpected message: {err}");
        // ...while OPEN stays the accepted open-bound sentinel.
        assert!(validate_datetime_wire(&json!(OPEN), "valid_to").is_ok());
    }

    #[test]
    fn test_read_path_still_accepts_legacy_integer() {
        // The compatibility decoder must keep reading legacy integers.
        let props = map(&[("valid_from", json!(1_700_000_000_i64))]);
        assert_eq!(
            read_datetime(&props, "valid_from"),
            DateTime::from_timestamp(1_700_000_000, 0)
        );
    }

    #[test]
    fn test_mixed_wire_values_compare_parsed_instants_not_lexical_forms() {
        // PIR-008 (M398/S04/T03): a snapshot may mix wire forms across
        // fields — legacy integer `valid_from` (2023-11-14T22:13:20Z) with a
        // canonical RFC3339 UTC `valid_to` (2024-01-15T10:30:00Z, unix
        // 1705314600). `is_active_at` must compare parsed instants, never raw
        // lexical integer/string forms: a lexical compare of "1700000000" vs
        // "2024-01-15T10:30:00Z" would order them wrong ("1" < "2").
        let props = map(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!("2024-01-15T10:30:00Z")),
        ]);
        let within = DateTime::from_timestamp(1_702_000_000, 0).unwrap(); // 2023-12-08
        let before = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let after = DateTime::from_timestamp(1_706_000_000, 0).unwrap(); // 2024-01-23
        assert!(is_active_at(&props, within));
        assert!(!is_active_at(&props, before));
        assert!(!is_active_at(&props, after));
    }
}
