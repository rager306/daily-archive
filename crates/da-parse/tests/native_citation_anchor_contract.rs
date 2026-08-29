use da_parse::{NativeCitationAnchorOutcome, NativeCitationAnchorScanner};

#[test]
fn preserves_targeted_targetless_and_adjacent_occurrences_in_source_order() {
    let input = r##"Intro <ref type="bibr" target="#b1">(A)</ref>; <ref type="bibr">(B)</ref>, <ref type="bibr" target="#b1">(A)</ref> end"##;
    let NativeCitationAnchorOutcome::Recognized {
        anchors,
        adjacent_pairs,
    } = NativeCitationAnchorScanner::new().scan(input)
    else {
        panic!("recognized")
    };
    assert_eq!(anchors.len(), 3);
    assert_eq!(adjacent_pairs, vec![(0, 1), (1, 2)]);
    assert_eq!(
        anchors.iter().map(|a| a.ordinal).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        anchors.iter().map(|a| a.target_present).collect::<Vec<_>>(),
        vec![true, false, true]
    );
    assert!(anchors.windows(2).all(|w| w[0].span.end <= w[1].span.start));
    for anchor in anchors {
        assert_eq!(&input[anchor.span.start..anchor.span.end], anchor.source);
        assert_eq!(anchor.provenance, "preserved_tei_reconstruction");
    }
}

#[test]
fn duplicate_targets_are_not_deduplicated_and_malformed_boundaries_are_closed() {
    let scanner = NativeCitationAnchorScanner::new();
    let input =
        r##"<ref type="bibr" target="#same">one</ref><ref type="bibr" target="#same">two</ref>"##;
    let NativeCitationAnchorOutcome::Recognized { anchors, .. } = scanner.scan(input) else {
        panic!()
    };
    assert_eq!(anchors.len(), 2);
    assert_eq!(anchors[0].target, Some("#same".into()));
    assert_eq!(anchors[1].target, Some("#same".into()));
    assert!(matches!(
        scanner.scan("<ref type=\"bibr\">broken"),
        NativeCitationAnchorOutcome::Malformed { .. }
    ));
}

#[test]
fn empty_and_caps_are_deterministic_without_silent_truncation() {
    let scanner = NativeCitationAnchorScanner::with_limits(da_parse::NativeCitationAnchorLimits {
        total_bytes: 8,
        max_anchors: 1,
        max_anchor_bytes: 64,
    });
    let count_limited =
        NativeCitationAnchorScanner::with_limits(da_parse::NativeCitationAnchorLimits {
            total_bytes: 128,
            max_anchors: 1,
            max_anchor_bytes: 64,
        });
    assert!(matches!(
        scanner.scan(""),
        NativeCitationAnchorOutcome::Empty
    ));
    assert!(matches!(
        scanner.scan("<ref type=\"bibr\">x</ref>"),
        NativeCitationAnchorOutcome::Oversized
    ));
    assert!(matches!(
        count_limited.scan("<ref type=\"bibr\">x</ref><ref type=\"bibr\">y</ref>"),
        NativeCitationAnchorOutcome::Truncated
    ));
}
