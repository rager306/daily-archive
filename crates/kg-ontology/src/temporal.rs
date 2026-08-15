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
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Sentinel string value for open temporal bounds (invalid_at, expired_at).
/// Matches Semantica's TemporalBound::OPEN convention; serializable in
/// Samyama's schemaless store.
pub const OPEN: &str = "OPEN";

/// The 5-field temporal edge model. Every temporal edge carries these
/// fields. Non-temporal edges (HAS_PART, FROM_SOURCE) carry only
/// `created_at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
///
/// This is the **read** path: it intentionally keeps accepting the legacy
/// integer string form so old snapshots remain readable. New writes must
/// use the canonical RFC3339 UTC string form and are validated by
/// [`validate_datetime_wire`] (which rejects integers).
pub fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() || s == OPEN {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            s.parse::<i64>()
                .ok()
                .and_then(|secs| DateTime::from_timestamp(secs, 0))
        })
}

/// Validate one DateTime property value against the canonical temporal
/// wire contract (PIR-008; encoded in `data/ontology/node_schemas.yaml`
/// under `temporal_wire_contract`, see
/// [`crate::registry::TemporalWireContract`]).
///
/// New node/edge writes MUST encode DateTime properties as RFC3339 UTC
/// strings — chrono's `DateTime<Utc>::to_rfc3339()` shape, e.g.
/// `2024-01-15T10:30:00Z`. Integer Unix timestamps are the LEGACY wire
/// form: still readable through the bounded compatibility decoders
/// ([`crate::bitemporal::read_datetime`], [`crate::bitemporal::read_bound`],
/// [`parse_datetime`]) but REJECTED for new writes.
///
/// `OPEN` remains the only open-bound sentinel and is accepted here for
/// any DateTime field; bound semantics (OPEN meaningful only on open-bound
/// fields such as `valid_to` / `superseded_at`) are enforced by the readers
/// and `validate_bitemporal`.
///
/// Returns `Ok(())` for a canonical value, `Err(message)` otherwise.
pub fn validate_datetime_wire(value: &Value, key: &str) -> Result<(), String> {
    match value {
        Value::Null => Ok(()),
        Value::String(s) if s == OPEN => Ok(()),
        Value::String(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(dt) if dt.offset().local_minus_utc() == 0 => Ok(()),
            Ok(dt) => Err(format!(
                "DateTime field '{key}': '{s}' is RFC3339 but not UTC (offset {} — canonical wire requires the Z/UTC designator)",
                dt.offset()
            )),
            Err(e) => Err(format!(
                "DateTime field '{key}': '{s}' is not a valid RFC3339 UTC string ({e})"
            )),
        },
        Value::Number(_) => Err(format!(
            "DateTime field '{key}': integer timestamp is the legacy wire form — new writes must use an RFC3339 UTC string"
        )),
        other => Err(format!(
            "DateTime field '{key}': expected RFC3339 UTC string (or OPEN), got {other}"
        )),
    }
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
        assert!(
            issues
                .iter()
                .any(|s| s.contains("retroactivity must point backwards"))
        );
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

// ─── PIR-008: canonical DateTime wire contract (M398/S04/T01) ───

#[cfg(test)]
mod wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_wire_accepts_canonical_rfc3339_utc() {
        assert!(validate_datetime_wire(&json!("2024-01-15T10:30:00Z"), "valid_from").is_ok());
        assert!(
            validate_datetime_wire(&json!("2024-01-15T10:30:00.123456789Z"), "recorded_at").is_ok()
        );
        // +00:00 is an explicit zero offset — same UTC instant, canonical.
        assert!(validate_datetime_wire(&json!("2024-01-15T10:30:00+00:00"), "valid_from").is_ok());
    }

    #[test]
    fn test_wire_rejects_integer_timestamps() {
        let err = validate_datetime_wire(&json!(1_700_000_000_i64), "valid_from").unwrap_err();
        assert!(
            err.contains("legacy"),
            "expected legacy-form rejection message, got: {err}"
        );
    }

    #[test]
    fn test_wire_rejects_non_utc_offset() {
        let err =
            validate_datetime_wire(&json!("2024-01-15T10:30:00+02:00"), "valid_from").unwrap_err();
        assert!(err.contains("UTC"), "unexpected message: {err}");
    }

    #[test]
    fn test_wire_rejects_non_rfc3339_string() {
        let err = validate_datetime_wire(&json!("2024-01-15 10:30:00"), "valid_from").unwrap_err();
        assert!(
            err.contains("not a valid RFC3339"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn test_wire_accepts_open_sentinel_and_null() {
        assert!(validate_datetime_wire(&json!(OPEN), "valid_to").is_ok());
        assert!(validate_datetime_wire(&Value::Null, "valid_to").is_ok());
    }

    #[test]
    fn test_wire_rejects_other_types() {
        let err = validate_datetime_wire(&json!(true), "valid_from").unwrap_err();
        assert!(
            err.contains("expected RFC3339"),
            "unexpected message: {err}"
        );
    }
}

// ─── PIR-008: canonical wire stability + replay proof (M398/S04/T04) ───

#[cfg(test)]
mod wire_stability_tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::path::Path;

    fn ts(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    fn edge_with(dt: DateTime<Utc>) -> TemporalEdge {
        TemporalEdge {
            valid_at: dt,
            invalid_at: Some(dt),
            expired_at: None,
            reference_time: Some(dt),
            created_at: dt,
        }
    }

    /// Serde wire stability: every DateTime field serializes as a canonical
    /// RFC3339 UTC string — never as a JSON number — and the serialized
    /// form passes the write validator (ADR-051 §4.4 wire-drift guard).
    #[test]
    fn test_serde_wire_serializes_datetime_as_rfc3339_utc_strings() {
        let edge = edge_with(ts(1_700_000_000));
        let value: Value = serde_json::to_value(&edge).unwrap();
        for key in ["valid_at", "invalid_at", "reference_time", "created_at"] {
            let s = value.get(key).and_then(Value::as_str).unwrap_or_else(|| {
                panic!("{key} must serialize as a string, got {:?}", value.get(key))
            });
            assert!(
                s.ends_with('Z') || s.ends_with("+00:00"),
                "{key} must carry the UTC designator on the wire, got {s}"
            );
            assert!(
                validate_datetime_wire(&value[key], key).is_ok(),
                "{key} serialized form must pass the write validator"
            );
        }
        let numeric: Vec<&str> = [
            "valid_at",
            "invalid_at",
            "expired_at",
            "reference_time",
            "created_at",
        ]
        .into_iter()
        .filter(|k| value[*k].is_number())
        .collect();
        assert!(
            numeric.is_empty(),
            "numeric temporal wire fields are the legacy form: {numeric:?}"
        );
    }

    /// Replay proof at the wire level: decode → re-encode must be
    /// byte-identical — re-applying the same wire changes nothing.
    /// (The graph-level replay equivalence applied=0 / unchanged>0 for
    /// canonical writers is proven in da-application
    /// `revision_citation_test::replay_stays_applied_zero_and_d127_holds`
    /// and `batch_ingest_test`.)
    #[test]
    fn test_wire_replay_is_byte_stable() {
        let edge = edge_with(ts(1_700_000_000));
        let first = serde_json::to_string(&edge).unwrap();
        for _ in 0..3 {
            let decoded: TemporalEdge = serde_json::from_str(&first).unwrap();
            let re_encoded = serde_json::to_string(&decoded).unwrap();
            assert_eq!(
                first, re_encoded,
                "re-encoding a decoded edge must be byte-identical (replay stability)"
            );
        }
    }

    /// The committed Governor wire fixtures are pinned to the actual
    /// validator: every positive value must be accepted, every negative
    /// value must be rejected. Regenerating either fixture without the
    /// matching validator behavior fails this test (mirrors
    /// `navigation_report_fixtures_match_committed_governor_data` in
    /// `crates/da-application/tests/section_navigation_test.rs`).
    #[test]
    fn temporal_wire_fixtures_match_committed_governor_data() {
        let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data/temporal_wire_contract");
        let positive_path = fixtures_dir.join("canonical_wire.positive.json");
        if !positive_path.exists() {
            eprintln!("skip: {} not found", positive_path.display());
            return;
        }
        let positive: Value = serde_json::from_str(
            &fs::read_to_string(positive_path).expect("read canonical_wire.positive.json"),
        )
        .expect("parse canonical_wire.positive.json");
        let positive = positive
            .as_array()
            .expect("positive fixture must be a JSON array");
        assert!(!positive.is_empty(), "positive fixture must not be empty");
        for (i, v) in positive.iter().enumerate() {
            assert!(
                validate_datetime_wire(v, "valid_from").is_ok(),
                "positive fixture[{i}] must be accepted: {v}"
            );
        }

        let negative_path = fixtures_dir.join("legacy_integer_wire.negative.json");
        if !negative_path.exists() {
            eprintln!("skip: {} not found", negative_path.display());
            return;
        }
        let negative: Value = serde_json::from_str(
            &fs::read_to_string(negative_path).expect("read legacy_integer_wire.negative.json"),
        )
        .expect("parse legacy_integer_wire.negative.json");
        let negative = negative
            .as_array()
            .expect("negative fixture must be a JSON array");
        assert!(!negative.is_empty(), "negative fixture must not be empty");
        for (i, v) in negative.iter().enumerate() {
            assert!(
                validate_datetime_wire(v, "valid_from").is_err(),
                "negative fixture[{i}] must be rejected: {v}"
            );
        }
    }
}

// ─── ADR-051 P1: Property-based tests (M353) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::{Value, json};

    /// Bounded unix seconds (avoids chrono edge overflow).
    fn arb_ts() -> impl Strategy<Value = DateTime<Utc>> {
        (0i64..=2_000_000_000i64).prop_map(|s| DateTime::from_timestamp(s, 0).unwrap())
    }

    fn arb_edge() -> impl Strategy<Value = TemporalEdge> {
        (
            arb_ts(),
            prop::option::of(arb_ts()),
            prop::option::of(arb_ts()),
            prop::option::of(arb_ts()),
            arb_ts(),
        )
            .prop_map(
                |(valid_at, invalid_at, expired_at, reference_time, created_at)| TemporalEdge {
                    valid_at,
                    invalid_at,
                    expired_at,
                    reference_time,
                    created_at,
                },
            )
    }

    /// Consistent edges: valid_at <= invalid_at (if set), created_at <= expired_at (if set).
    fn arb_consistent_edge() -> impl Strategy<Value = TemporalEdge> {
        (
            arb_ts(),
            prop::option::of(0u32..=86_400u32), // invalid after valid by 0..1 day secs offset via map
            prop::option::of(0u32..=86_400u32),
            arb_ts(),
        )
            .prop_map(|(valid_at, inv_off, exp_off, created_at)| {
                let invalid_at = inv_off.map(|o| {
                    DateTime::from_timestamp(valid_at.timestamp().saturating_add(o as i64), 0)
                        .unwrap()
                });
                let expired_at = exp_off.map(|o| {
                    DateTime::from_timestamp(created_at.timestamp().saturating_add(o as i64), 0)
                        .unwrap()
                });
                TemporalEdge {
                    valid_at,
                    invalid_at,
                    expired_at,
                    reference_time: None,
                    created_at,
                }
            })
    }

    proptest! {
        /// Property 1: consistent edges always pass validate_temporal_edge.
        #[test]
        fn prop_consistent_edge_validates_clean(edge in arb_consistent_edge()) {
            let issues = validate_temporal_edge(&edge, None);
            prop_assert!(issues.is_empty(), "issues={issues:?} edge={edge:?}");
        }

        /// Property 2: if invalid_at < valid_at, validate reports inverted valid range.
        #[test]
        fn prop_inverted_valid_range_is_flagged(
            a in arb_ts(),
            b in arb_ts(),
            created in arb_ts(),
        ) {
            prop_assume!(a != b);
            let (valid_at, invalid_at) = if a < b { (b, a) } else { (a, b) };
            // now valid_at > invalid_at
            prop_assume!(valid_at > invalid_at);
            let edge = TemporalEdge {
                valid_at,
                invalid_at: Some(invalid_at),
                expired_at: None,
                reference_time: None,
                created_at: created,
            };
            let issues = validate_temporal_edge(&edge, None);
            prop_assert!(
                issues.iter().any(|s| s.contains("valid_at") && s.contains("invalid_at")),
                "issues={issues:?}"
            );
        }

        /// Property 3: is_active_at is half-open [valid_at, invalid_at).
        #[test]
        fn prop_is_active_at_half_open(
            start in 0i64..=1_000_000i64,
            len in 1i64..=100_000i64,
        ) {
            let valid_at = DateTime::from_timestamp(start, 0).unwrap();
            let invalid_at = DateTime::from_timestamp(start + len, 0).unwrap();
            let edge = TemporalEdge {
                valid_at,
                invalid_at: Some(invalid_at),
                expired_at: None,
                reference_time: None,
                created_at: valid_at,
            };
            // start inclusive
            prop_assert!(edge.is_active_at(valid_at, None));
            // end exclusive
            prop_assert!(!edge.is_active_at(invalid_at, None));
            // midpoint active
            let mid = DateTime::from_timestamp(start + len / 2, 0).unwrap();
            prop_assert!(edge.is_active_at(mid, None));
            // before start inactive
            if start > 0 {
                let before = DateTime::from_timestamp(start - 1, 0).unwrap();
                prop_assert!(!edge.is_active_at(before, None));
            }
        }

        /// Property 4: was_known_at is half-open [created_at, expired_at).
        #[test]
        fn prop_was_known_at_half_open(
            start in 0i64..=1_000_000i64,
            len in 1i64..=100_000i64,
        ) {
            let created_at = DateTime::from_timestamp(start, 0).unwrap();
            let expired_at = DateTime::from_timestamp(start + len, 0).unwrap();
            let edge = TemporalEdge {
                valid_at: created_at,
                invalid_at: None,
                expired_at: Some(expired_at),
                reference_time: None,
                created_at,
            };
            prop_assert!(edge.was_known_at(created_at));
            prop_assert!(!edge.was_known_at(expired_at));
            let mid = DateTime::from_timestamp(start + len / 2, 0).unwrap();
            prop_assert!(edge.was_known_at(mid));
        }

        /// Property 5: open invalid_at means active for all t >= valid_at.
        #[test]
        fn prop_open_invalid_active_from_start(
            start in 0i64..=1_000_000i64,
            delta in 0i64..=100_000i64,
        ) {
            let valid_at = DateTime::from_timestamp(start, 0).unwrap();
            let edge = TemporalEdge {
                valid_at,
                invalid_at: None,
                expired_at: None,
                reference_time: None,
                created_at: valid_at,
            };
            let at = DateTime::from_timestamp(start + delta, 0).unwrap();
            prop_assert!(edge.is_active_at(at, None));
            if start > 0 {
                let before = DateTime::from_timestamp(start - 1, 0).unwrap();
                prop_assert!(!edge.is_active_at(before, None));
            }
        }

        /// Property 6: invalidate sets expired_at and fills invalid_at if open.
        #[test]
        fn prop_invalidate_sets_expired(edge in arb_edge(), at in arb_ts()) {
            let mut e = edge;
            let had_invalid = e.invalid_at.is_some();
            e.invalidate(at);
            prop_assert_eq!(e.expired_at, Some(at));
            prop_assert!(e.is_expired());
            if !had_invalid {
                prop_assert_eq!(e.invalid_at, Some(at));
            }
        }

        /// Property 7: retroactive_to > valid_at is flagged.
        #[test]
        fn prop_retro_after_valid_flagged(
            valid in 1i64..=1_000_000i64,
            delta in 1i64..=100_000i64,
        ) {
            let valid_at = DateTime::from_timestamp(valid, 0).unwrap();
            let retro_to = DateTime::from_timestamp(valid + delta, 0).unwrap();
            let edge = TemporalEdge {
                valid_at,
                invalid_at: None,
                expired_at: None,
                reference_time: None,
                created_at: valid_at,
            };
            let retro = RetroactiveExtension {
                retroactive_to: Some(retro_to),
                retroactivity_basis: Some("test".into()),
            };
            let issues = validate_temporal_edge(&edge, Some(&retro));
            prop_assert!(
                issues.iter().any(|s| s.contains("retroactivity must point backwards")),
                "issues={issues:?}"
            );
        }

        /// Property 8: arbitrary edge never panics validate / is_active / was_known.
        #[test]
        fn prop_never_panic_queries(edge in arb_edge(), at in arb_ts()) {
            let _ = validate_temporal_edge(&edge, None);
            let _ = edge.is_active_at(at, None);
            let _ = edge.was_known_at(at);
            let _ = edge.is_expired();
        }

        /// Property 9 (PIR-008 / M398 S04 T04): the canonical to_rfc3339()
        /// wire of any DateTime always passes the write validator, parses
        /// back to the identical instant (no drift), and re-encodes to the
        /// same bytes (replay stability).
        #[test]
        fn prop_canonical_wire_roundtrips_without_drift(dt in arb_ts()) {
            let wire = dt.to_rfc3339();
            prop_assert!(
                validate_datetime_wire(&Value::String(wire.clone()), "valid_from").is_ok(),
                "canonical wire must be accepted: {wire}"
            );
            let reparsed = DateTime::parse_from_rfc3339(&wire).unwrap().with_timezone(&Utc);
            prop_assert_eq!(
                reparsed, dt,
                "canonical wire must roundtrip to the identical instant"
            );
            prop_assert_eq!(
                reparsed.to_rfc3339(),
                wire,
                "re-encoding must be byte-identical (replay stability)"
            );
        }

        /// Property 10 (PIR-008 / M398 S04 T04): serde roundtrip of an
        /// arbitrary TemporalEdge is identity (wire-stability guard).
        #[test]
        fn prop_edge_serde_roundtrip_identity(edge in arb_edge()) {
            let wire = serde_json::to_string(&edge).unwrap();
            let back: TemporalEdge = serde_json::from_str(&wire).unwrap();
            prop_assert_eq!(edge, back, "serde roundtrip mismatch: {}", wire);
        }

        /// Property 11 (PIR-008 / M398 S04 T04): the legacy integer wire
        /// form is always rejected for new writes — including negative and
        /// extreme values, not just the classic 1.7e9 epoch.
        #[test]
        fn prop_integer_wire_always_rejected(secs in i64::MIN..=2_000_000_000i64) {
            let err = validate_datetime_wire(&json!(secs), "valid_from").unwrap_err();
            prop_assert!(
                err.contains("legacy"),
                "expected legacy-form rejection message, got: {err}"
            );
        }
    }
}
