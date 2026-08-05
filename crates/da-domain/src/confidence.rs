//! Confidence unit-interval helpers (ADR-051 P1 / M355).
//!
//! Entity.confidence, Relation.confidence, and PdfClassification.confidence
//! are documented as scores in [0, 1]. These free helpers validate/clamp
//! without changing public field types (non-breaking).

/// True if `c` is a finite value in the closed unit interval [0, 1].
pub fn is_unit_interval(c: f32) -> bool {
    c.is_finite() && (0.0..=1.0).contains(&c)
}

/// Clamp a confidence-like score into [0, 1].
///
/// Non-finite values (NaN, ±inf) map to `0.0` (conservative).
pub fn clamp_confidence(c: f32) -> f32 {
    if !c.is_finite() {
        return 0.0;
    }
    c.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_interval_bounds() {
        assert!(is_unit_interval(0.0));
        assert!(is_unit_interval(1.0));
        assert!(is_unit_interval(0.5));
        assert!(!is_unit_interval(-0.01));
        assert!(!is_unit_interval(1.01));
        assert!(!is_unit_interval(f32::NAN));
        assert!(!is_unit_interval(f32::INFINITY));
    }

    #[test]
    fn clamp_edges() {
        assert_eq!(clamp_confidence(-1.0), 0.0);
        assert_eq!(clamp_confidence(2.0), 1.0);
        assert_eq!(clamp_confidence(0.3), 0.3);
        assert_eq!(clamp_confidence(f32::NAN), 0.0);
        assert_eq!(clamp_confidence(f32::NEG_INFINITY), 0.0);
        assert_eq!(clamp_confidence(f32::INFINITY), 0.0);
    }
}

// ─── ADR-051 P1: Property-based tests (M355) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Property 1: clamp_confidence always yields a unit-interval value.
        #[test]
        fn prop_clamp_always_unit(c in any::<f32>()) {
            let out = clamp_confidence(c);
            prop_assert!(is_unit_interval(out), "out={out} from c={c}");
        }

        /// Property 2: values already in [0,1] are unchanged by clamp.
        #[test]
        fn prop_clamp_idempotent_on_unit(c in 0.0f32..=1.0) {
            prop_assume!(c.is_finite());
            prop_assert_eq!(clamp_confidence(c), c);
        }

        /// Property 3: is_unit_interval is true for all clamped outputs.
        #[test]
        fn prop_is_unit_after_clamp(c in prop::num::f32::ANY) {
            prop_assert!(is_unit_interval(clamp_confidence(c)));
        }
    }
}
