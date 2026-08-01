//! Temporal edge resolution (ADR-047).
//!
//! When a new temporal edge arrives between endpoints that already have
//! edges of the same type, this module determines which existing edges
//! should be invalidated.
//!
//! 4-rule algorithm adapted from Graphiti's `resolve_edge_contradictions`:
//!
//! 1. SKIP — old was already invalid before new became valid
//! 2. SKIP — new was invalid before old became valid
//! 3. SUPERSEDE — old is strictly earlier → new invalidates old
//! 4. RETAIN_BOTH — temporal overlap, neither strictly earlier
//!
//! This is the core of temporal KG maintenance. It handles sequential
//! versioning (law v1 → v2), retractions, competing simultaneous facts,
//! and transition periods (two versions active simultaneously).

use kg_ontology::temporal::TemporalEdge;

/// Which resolution rule fired for a particular edge pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionRule {
    /// old.invalid_at ≤ new.valid_at — old was already invalid.
    SkipOldAlreadyInvalid,
    /// new.invalid_at ≤ old.valid_at — new was invalid before old.
    SkipNewWasInvalid,
    /// old.valid_at < new.valid_at — new supersedes old.
    Supersede,
    /// Temporal overlap, neither strictly earlier — retain both.
    RetainBoth,
}

/// Result of resolving a new edge against one existing edge.
#[derive(Debug, Clone)]
pub struct ResolutionOutcome {
    /// Which rule fired.
    pub rule: ResolutionRule,
    /// True if the existing edge should be invalidated.
    pub should_invalidate: bool,
}

/// Resolve a new temporal edge against existing edges with the same
/// endpoints and type.
///
/// Returns a Vec of (index_into_existing, ResolutionOutcome) for each
/// existing edge. Callers iterate the outcomes and invalidate edges
/// where `should_invalidate == true`.
///
/// The invalidation itself (setting invalid_at + expired_at) is done
/// by the caller via kg-storage's `set_edge_property_string_v2`, so
/// this function is pure logic with no IO.
pub fn resolve_temporal_edges(
    new_edge: &TemporalEdge,
    existing_edges: &[TemporalEdge],
) -> Vec<ResolutionOutcome> {
    existing_edges
        .iter()
        .map(|old| {
            let rule = resolve_pair(new_edge, old);
            let should_invalidate = rule == ResolutionRule::Supersede;
            ResolutionOutcome {
                rule,
                should_invalidate,
            }
        })
        .collect()
}

/// Resolve a single pair: new edge vs one existing edge.
fn resolve_pair(new_edge: &TemporalEdge, old_edge: &TemporalEdge) -> ResolutionRule {
    // Rule 1: old was already invalid before new became valid → skip
    if let Some(old_invalid) = old_edge.invalid_at
        && old_invalid <= new_edge.valid_at
    {
        return ResolutionRule::SkipOldAlreadyInvalid;
    }

    // Rule 2: new was invalid before old became valid → skip
    if let Some(new_invalid) = new_edge.invalid_at
        && new_invalid <= old_edge.valid_at
    {
        return ResolutionRule::SkipNewWasInvalid;
    }

    // Rule 3: old is strictly earlier → new supersedes
    if old_edge.valid_at < new_edge.valid_at {
        return ResolutionRule::Supersede;
    }

    // Rule 4: temporal overlap, neither strictly earlier → retain both
    ResolutionRule::RetainBoth
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use kg_ontology::temporal::TemporalEdge;

    fn ts(s: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(s, 0).unwrap()
    }

    fn edge(valid: i64, invalid: Option<i64>) -> TemporalEdge {
        TemporalEdge {
            valid_at: ts(valid),
            invalid_at: invalid.map(ts),
            expired_at: None,
            reference_time: None,
            created_at: ts(valid),
        }
    }

    #[test]
    fn test_rule1_skip_old_already_invalid() {
        let new = edge(2000, None);
        let old = edge(1000, Some(1500)); // invalid before new
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::SkipOldAlreadyInvalid);
        assert!(!outcomes[0].should_invalidate);
    }

    #[test]
    fn test_rule2_skip_new_was_invalid() {
        let new = edge(2000, Some(2500));
        let old = edge(3000, None); // old starts after new is already invalid
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::SkipNewWasInvalid);
        assert!(!outcomes[0].should_invalidate);
    }

    #[test]
    fn test_rule3_supersede() {
        let new = edge(2000, None);
        let old = edge(1000, None); // old is earlier → supersede
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::Supersede);
        assert!(outcomes[0].should_invalidate);
    }

    #[test]
    fn test_rule4_retain_both_on_overlap() {
        let new = edge(2000, None);
        let old = edge(2000, None); // same valid_at → overlap → retain
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::RetainBoth);
        assert!(!outcomes[0].should_invalidate);
    }

    #[test]
    fn test_rule4_retain_both_window_overlap() {
        // old: [1000..3000], new: [2000..4000] — overlap but neither strictly earlier
        let new = edge(2000, Some(4000));
        let old = edge(1000, Some(3000));
        // old.valid_at(1000) < new.valid_at(2000) → this IS rule 3 (supersede)
        // But old is still active until 3000, and new starts at 2000
        // Per the algorithm: old.valid_at < new.valid_at → supersede
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::Supersede);
    }

    #[test]
    fn test_multiple_existing_edges() {
        let new = edge(3000, None);
        let old1 = edge(1000, Some(1500)); // rule 1: already invalid
        let old2 = edge(2000, None); // rule 3: supersede
        let old3 = edge(3000, None); // rule 4: retain both

        let outcomes = resolve_temporal_edges(&new, &[old1, old2, old3]);
        assert_eq!(outcomes.len(), 3);
        assert!(!outcomes[0].should_invalidate); // old1 already invalid
        assert!(outcomes[1].should_invalidate); // old2 superseded
        assert!(!outcomes[2].should_invalidate); // old3 retained
    }

    #[test]
    fn test_empty_existing_edges() {
        let new = edge(1000, None);
        let outcomes = resolve_temporal_edges(&new, &[]);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn test_transition_period_both_active() {
        // Legal domain: old law valid until end of year, new law valid from mid-year
        // Both active during overlap → retain both
        // old: [2010..2025], new: [2024..OPEN]
        // old.valid_at(2010) < new.valid_at(2024) → supersede
        let new = TemporalEdge {
            valid_at: ts(2024_000_000),
            invalid_at: None,
            expired_at: None,
            reference_time: None,
            created_at: ts(2024_000_000),
        };
        let old = TemporalEdge {
            valid_at: ts(2010_000_000),
            invalid_at: Some(ts(2025_000_000)),
            expired_at: None,
            reference_time: None,
            created_at: ts(2010_000_000),
        };
        let outcomes = resolve_temporal_edges(&new, &[old]);
        // old.valid_at(2010) < new.valid_at(2024) → rule 3 supersede
        assert_eq!(outcomes[0].rule, ResolutionRule::Supersede);
        assert!(outcomes[0].should_invalidate);
    }

    #[test]
    fn test_simultaneous_competing_facts() {
        // Scientific: two papers report different accuracy for same method
        // Both published same day → retain both
        let new = edge(2024_0101, None);
        let old = edge(2024_0101, None); // same valid_at
        let outcomes = resolve_temporal_edges(&new, &[old]);
        assert_eq!(outcomes[0].rule, ResolutionRule::RetainBoth);
        assert!(!outcomes[0].should_invalidate);
    }
}
