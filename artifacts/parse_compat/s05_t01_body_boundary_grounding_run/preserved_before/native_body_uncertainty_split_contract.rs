//! Additive uncertainty-split contract for the native body text (S05/T01).
//!
//! The accepted P1/P2 localization carries a single bucket,
//! `uncertain_present_in_token`: oracle occurrences that are present inside one
//! native token but were not localized at a token boundary. That bucket merges
//! two different facts — an occurrence *equal* to one side token (present, but
//! unbound by the ordered anchors) and an occurrence that is only a *proper
//! substring* of one (`he` inside `the`). This test pins the additive split that
//! separates them, with equality tested first, and pins that the split is a fact
//! count only: it conserves the bucket it refines, and it never becomes a
//! verdict, a threshold, a ratio or a preservation claim.

use da_parse::{
    BodyCasePolicy, BodyCoverageVerdict, BodyGapReason, BodyJoinPolicy, BodyNormalization,
    BodyUnitInventory, BodyUnitRung, NativeBodyText, NativeBodyTextError, align_body_content,
    classify_body_plane, localize_body_plane_checked, split_body_uncertainty,
    split_plane_uncertainty,
};

/// Raw normalization keeps the fixtures literal; the case policy is explicit, so
/// the test never relies on an implicit folding rung.
fn rung() -> BodyUnitRung {
    BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseFolded)
}

fn side(fragments: &[&str]) -> BodyUnitInventory {
    BodyUnitInventory::from_fragments(fragments.iter().copied(), rung())
}

fn oracle(text: &str) -> BodyUnitInventory {
    BodyUnitInventory::from_text(text, rung())
}

#[test]
fn an_equal_token_occurrence_is_split_out_as_equal_not_proper() {
    // `the` appears twice in the oracle and once in the side: the anchor consumes
    // the side occurrence once, and the second oracle occurrence is present as a
    // whole token but unbound. That is the equality sub-class.
    let plane = side(&["the"]);
    let oracle = oracle("the the");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    assert_eq!(localization.exact_in_plane, 1);
    assert_eq!(localization.uncertain_present_in_token_from_plane, 1);

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split.equal_in_token, 1);
    assert_eq!(split.proper_substring_in_token, 0);
    assert_eq!(
        split.total(),
        localization.uncertain_present_in_token_from_plane
    );
}

#[test]
fn a_proper_substring_only_is_split_out_as_proper_not_equal() {
    // `he` sits inside `the` and equals no side token: proper substring only.
    let plane = side(&["the"]);
    let oracle = oracle("he");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    assert_eq!(localization.uncertain_present_in_token_from_plane, 1);

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split.equal_in_token, 0);
    assert_eq!(split.proper_substring_in_token, 1);
    assert_eq!(
        split.total(),
        localization.uncertain_present_in_token_from_plane
    );
}

#[test]
fn equality_takes_precedence_over_proper_substring() {
    // `ab` equals the first side token and is a proper substring of `abc`.
    // Equality is tested first, so an unbound `ab` lands in `equal`, not
    // `proper`; a split that tested containment first would report `proper`.
    let plane = side(&["ab", "abc"]);
    let oracle = oracle("ab ab");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    assert_eq!(localization.uncertain_present_in_token_from_plane, 1);

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split.equal_in_token, 1);
    assert_eq!(split.proper_substring_in_token, 0);
}

#[test]
fn repeated_occurrences_are_counted_once_per_occurrence() {
    // One side occurrence cannot excuse three residual oracle occurrences.
    let plane = side(&["the"]);
    let oracle = oracle("the the the the");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    assert_eq!(localization.exact_in_plane, 1);
    assert_eq!(localization.uncertain_present_in_token_from_plane, 3);

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split.equal_in_token, 3);
    assert_eq!(split.proper_substring_in_token, 0);
}

#[test]
fn an_occurrence_in_no_token_lands_in_no_sub_class() {
    // `cat` shares no characters with `the`: neither equal nor proper, so the
    // split stays zero and the occurrence is absent, never silently matched.
    let plane = side(&["the"]);
    let oracle = oracle("cat");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    assert_eq!(localization.absent_from_plane, 1);
    assert_eq!(localization.uncertain_present_in_token_from_plane, 0);

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split, Default::default());
}

/// The split must sum to the accepted bucket — but this is a **by-construction
/// reflex, not independent evidence**: the accepted `uncertain_present_in_token`
/// count is itself the stored sum of the two fields the split returns, so this
/// test can only fail if that shared computation is changed, never because the
/// measurement drifted. It establishes no preservation claim.
#[test]
fn the_split_conserves_the_in_token_bucket() {
    // A mixed plane: unbound equal tokens, a proper substring, a character-only
    // occurrence and an exact occurrence. The two sub-classes must sum to the
    // bucket the accepted localization reports — the conservation invariant.
    let plane = side(&["the", "ab", "abc", "gamma"]);
    let oracle = oracle("the the he ab ab cat gamma");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");

    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(
        split.total(),
        localization.uncertain_present_in_token_from_plane,
        "equal + proper must equal the uncertain_present_in_token bucket"
    );
    assert_eq!(
        split.equal_in_token, 2,
        "the second `the` and the second `ab`"
    );
    assert_eq!(split.proper_substring_in_token, 1, "`he` inside `the`");
    assert_eq!(localization.exact_in_plane, 3, "the, the first `ab`, gamma");
    assert_eq!(
        localization.uncertain_character_material_from_plane, 1,
        "cat"
    );
}

#[test]
fn the_split_is_a_fact_count_never_a_verdict() {
    let plane = side(&["the"]);
    let oracle = oracle("the the");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert!(split.equal_in_token > 0);

    // A nonzero equal count does not clear the plane: presence is not a
    // boundary, so the verdict stays NotAssessable and D498 stays untouched.
    let verdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(!verdict.d498_reopen_required);

    // The counter cross-serializes as counts only: no verdict, ratio, threshold
    // or match field can be read out of it.
    let json = serde_json::to_value(split).expect("serialize");
    let object = json.as_object().expect("object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["equal_in_token", "proper_substring_in_token"]);
    assert!(!object.contains_key("verdict"));
    assert!(!object.contains_key("ratio"));
    assert!(!object.contains_key("threshold"));
    assert!(!object.contains_key("match"));
}

#[test]
fn uncertainty_is_global_not_a_proof_even_when_every_occurrence_is_equal() {
    // Every oracle occurrence is present as an exact side token, yet none of the
    // residual ones is bound in order. The split names the fact; it does not make
    // the plane granularity-only, and equal>0 is not a proof of anything.
    let plane = side(&["the"]);
    let oracle = oracle("the the");
    let localization = localize_body_plane_checked(&plane, &oracle).expect("same rung");
    let split = split_plane_uncertainty(&plane, &oracle).expect("same rung");
    assert_eq!(split.proper_substring_in_token, 0);
    assert!(split.equal_in_token > 0);

    let verdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::NotAssessable);
    assert!(matches!(
        verdict.reason,
        BodyGapReason::P1LocalizationUncertain { .. }
    ));
}

#[test]
fn a_rung_mismatch_is_refused_by_both_split_entry_points() {
    let raw = side(&["the"]);
    let normalized = BodyUnitInventory::from_text(
        "the the",
        BodyUnitRung::new(
            BodyNormalization::ZeroWidthAndLigatureNormalized,
            BodyCasePolicy::CaseFolded,
        ),
    );
    assert!(matches!(
        split_plane_uncertainty(&raw, &normalized),
        Err(NativeBodyTextError::InventoryRungMismatch {
            side: "native plane"
        })
    ));
    assert!(matches!(
        split_body_uncertainty(&raw, &normalized),
        Err(NativeBodyTextError::InventoryRungMismatch {
            side: "native body"
        })
    ));
}

#[test]
fn an_empty_oracle_yields_the_zero_split_not_a_pass() {
    let plane = side(&["the"]);
    let empty = oracle("");
    let localization = localize_body_plane_checked(&plane, &empty).expect("same rung");
    assert_eq!(localization.oracle_units, 0);

    let split = split_plane_uncertainty(&plane, &empty).expect("same rung");
    assert_eq!(split, Default::default());

    let verdict = classify_body_plane(&localization);
    assert_eq!(verdict.verdict, BodyCoverageVerdict::NotAssessable);
}

#[test]
fn the_body_entry_point_splits_the_body_bucket() {
    // The body entry point reads the same `NativeBodyText::units()` the accepted
    // body classification reads, so its total equals the P2 body bucket.
    let native = NativeBodyText::from_fragments(
        "doc",
        "a".repeat(64),
        1,
        None,
        BodyJoinPolicy::VerbatimTokenConcat,
        ["the"],
        rung(),
    )
    .expect("non-empty body");
    let plane = side(&["the"]);
    let oracle = oracle("the the");
    let alignment = align_body_content(native.units(), &plane, &oracle);
    assert_eq!(alignment.uncertain_present_in_token_from_body, 1);

    let split = split_body_uncertainty(native.units(), &oracle).expect("same rung");
    assert_eq!(split.equal_in_token, 1);
    assert_eq!(split.proper_substring_in_token, 0);
    assert_eq!(
        split.total(),
        alignment.uncertain_present_in_token_from_body
    );
}
