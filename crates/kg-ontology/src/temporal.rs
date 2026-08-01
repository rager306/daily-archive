//! BiTemporal Fact helpers (ADR-046).
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
//! Phase 1 (this module): helpers operate on PropertySnapshot from the
//! validator module. No storage changes. Fact-bearing schemas declare
//! the four fields as optional; existing writes are unaffected.

use crate::validator::PropertySnapshot;

/// Sentinel string value for open temporal bounds (valid_to, superseded_at).
/// Matches Semantica's TemporalBound::OPEN convention; serializable in
/// Samyama's schemaless store.
pub const OPEN: &str = "OPEN";

/// Parse a DateTime from a property snapshot field. Returns None if the
/// field is missing, null, empty, or the OPEN sentinel.
///
/// Accepts Unix timestamps (i64 seconds since epoch) or ISO-8601 strings.
/// The two encodings coexist because Samyama stores both shapes depending
/// on which adapter wrote the node.
pub fn read_datetime(props: &PropertySnapshot, key: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let val = props.get(key)?;
    if val.is_null() {
        return None;
    }
    if let Some(s) = val.as_str() {
        if s.is_empty() || s == OPEN {
            return None;
        }
        // Try ISO-8601 first, then fall through to caller.
        return chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc));
    }
    if let Some(i) = val.as_i64() {
        // Treat as Unix seconds. Negative values are pre-1970 — unlikely
        // in our domain but handled defensively.
        return chrono::DateTime::from_timestamp(i, 0);
    }
    None
}

/// Read a temporal bound — either a DateTime or the OPEN sentinel.
/// Returns Ok(Some(dt)) for a concrete timestamp, Ok(None) for OPEN or
/// missing, Err for a malformed value.
pub fn read_bound(
    props: &PropertySnapshot,
    key: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
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
        return chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| Some(dt.with_timezone(&chrono::Utc)))
            .map_err(|e| format!("invalid datetime at {key}: {e}"));
    }
    if let Some(i) = val.as_i64() {
        return chrono::DateTime::from_timestamp(i, 0)
            .map(Some)
            .ok_or_else(|| format!("out-of-range timestamp at {key}: {i}"));
    }
    Err(format!("expected datetime or OPEN at {key}, got {val}"))
}

/// True if the fact was true in the real world at the given instant.
/// Checks the valid-time axis: valid_from <= t < valid_to.
pub fn is_active_at(props: &PropertySnapshot, at: chrono::DateTime<chrono::Utc>) -> bool {
    let from = read_datetime(props, "valid_from");
    let to = read_bound(props, "valid_to").ok().flatten();
    match (from, to) {
        (Some(f), Some(t)) => f <= at && at < t,
        (Some(f), None) => f <= at,
        (None, Some(t)) => at < t,
        (None, None) => true, // no temporal info → treat as always active
    }
}

/// True if our system knew about the fact at the given instant.
/// Checks the transaction-time axis: recorded_at <= t < superseded_at.
pub fn was_known_at(props: &PropertySnapshot, at: chrono::DateTime<chrono::Utc>) -> bool {
    let recorded = read_datetime(props, "recorded_at");
    let superseded = read_bound(props, "superseded_at").ok().flatten();
    match (recorded, superseded) {
        (Some(r), Some(s)) => r <= at && at < s,
        (Some(r), None) => r <= at,
        (None, Some(_)) => false, // superseded before any record — inconsistent
        (None, None) => true, // no transaction info → treat as always known
    }
}

/// True if the fact is currently active (valid time) AND currently known
/// (transaction time). Convenience combining is_active_at(now) and
/// was_known_at(now).
pub fn is_current(props: &PropertySnapshot) -> bool {
    let now = chrono::Utc::now();
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
pub fn validate_bitemporal(props: &PropertySnapshot) -> Vec<String> {
    let mut issues = Vec::new();
    let from = read_datetime(props, "valid_from");
    let to = read_bound(props, "valid_to").ok().flatten();
    let recorded = read_datetime(props, "recorded_at");
    let superseded = read_bound(props, "superserialized_at")
        .ok()
        .flatten()
        .or_else(|| read_bound(props, "superseded_at").ok().flatten());

    if let (Some(f), Some(t)) = (from, to)
        && f > t
    {
        issues.push(format!(
            "valid_from ({f}) is later than valid_to ({t})"
        ));
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

    fn snap(pairs: &[(&str, serde_json::Value)]) -> PropertySnapshot {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_open_sentinel_recognized() {
        let props = snap(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!(OPEN)),
        ]);
        assert!(read_bound(&props, "valid_to").unwrap().is_none());
    }

    #[test]
    fn test_is_active_at_within_range() {
        let props = snap(&[
            ("valid_from", json!(1_700_000_000_i64)), // 2023-11-14
            ("valid_to", json!(1_800_000_000_i64)),   // 2027-01-15
        ]);
        let within = chrono::DateTime::from_timestamp(1_750_000_000, 0).unwrap();
        let before = chrono::DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let after = chrono::DateTime::from_timestamp(1_900_000_000, 0).unwrap();
        assert!(is_active_at(&props, within));
        assert!(!is_active_at(&props, before));
        assert!(!is_active_at(&props, after));
    }

    #[test]
    fn test_is_active_at_open_ended() {
        let props = snap(&[("valid_from", json!(1_700_000_000_i64))]);
        let now = chrono::Utc::now();
        assert!(is_active_at(&props, now));
    }

    #[test]
    fn test_was_known_at_within_transaction_range() {
        let props = snap(&[
            ("recorded_at", json!(1_700_000_000_i64)),
            ("superseded_at", json!(1_800_000_000_i64)),
        ]);
        let within = chrono::DateTime::from_timestamp(1_750_000_000, 0).unwrap();
        let before = chrono::DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let after = chrono::DateTime::from_timestamp(1_900_000_000, 0).unwrap();
        assert!(was_known_at(&props, within));
        assert!(!was_known_at(&props, before));
        assert!(!was_known_at(&props, after));
    }

    #[test]
    fn test_was_known_at_open_ended() {
        let props = snap(&[("recorded_at", json!(1_700_000_000_i64))]);
        let now = chrono::Utc::now();
        assert!(was_known_at(&props, now));
    }

    #[test]
    fn test_validate_bitemporal_consistent() {
        let props = snap(&[
            ("valid_from", json!(1_700_000_000_i64)),
            ("valid_to", json!(1_800_000_000_i64)),
            ("recorded_at", json!(1_700_500_000_i64)),
            ("superseded_at", json!(OPEN)),
        ]);
        let issues = validate_bitemporal(&props);
        assert!(
            issues.is_empty(),
            "expected no issues, got: {issues:?}"
        );
    }

    #[test]
    fn test_validate_bitemporal_inverted_valid_range() {
        let props = snap(&[
            ("valid_from", json!(1_800_000_000_i64)),
            ("valid_to", json!(1_700_000_000_i64)),
        ]);
        let issues = validate_bitemporal(&props);
        assert!(
            issues
                .iter()
                .any(|s| s.contains("valid_from") && s.contains("later than valid_to")),
            "expected valid range inversion issue, got: {issues:?}"
        );
    }

    #[test]
    fn test_validate_bitemporal_inverted_transaction_range() {
        let props = snap(&[
            ("recorded_at", json!(1_800_000_000_i64)),
            ("superseded_at", json!(1_700_000_000_i64)),
        ]);
        let issues = validate_bitemporal(&props);
        assert!(
            issues
                .iter()
                .any(|s| s.contains("recorded_at") && s.contains("later than superseded_at")),
            "expected transaction range inversion issue, got: {issues:?}"
        );
    }

    #[test]
    fn test_validate_bitemporal_extraction_before_publication_warns() {
        let props = snap(&[
            ("valid_from", json!(1_800_000_000_i64)), // future publication
            ("recorded_at", json!(1_700_000_000_i64)), // extracted before
        ]);
        let issues = validate_bitemporal(&props);
        assert!(
            issues
                .iter()
                .any(|s| s.contains("earlier than valid_from")),
            "expected extraction-before-publication warning, got: {issues:?}"
        );
    }

    #[test]
    fn test_no_temporal_info_treated_as_active_and_known() {
        let props = PropertySnapshot::new();
        let now = chrono::Utc::now();
        assert!(is_active_at(&props, now));
        assert!(was_known_at(&props, now));
    }
}
