use da_parse::{
    NativeReferenceListOutcome, NativeReferenceSegmenter, NativeReferenceSegmenterLimits,
};

#[test]
fn preserves_each_ordered_entry_and_literal_span() {
    let input = [
        "(Alpha, 2020). First.",
        "(Beta, 2021). Second.",
        "(Alpha, 2020). First.",
    ];
    let result = NativeReferenceSegmenter::new().segment(&input);
    let NativeReferenceListOutcome::Recognized { references } = result else {
        panic!("recognized")
    };
    assert_eq!(references.len(), 3);
    for (ordinal, reference) in references.iter().enumerate() {
        assert_eq!(reference.ordinal, ordinal);
        assert_eq!(reference.source, input[ordinal]);
        assert_eq!(reference.span.start, 0);
        assert_eq!(reference.span.end, input[ordinal].len());
        assert_eq!(reference.provenance, "preserved_tei_reconstruction");
    }
}

#[test]
fn closed_outcomes_cover_empty_malformed_and_caps_without_repair() {
    let parser = NativeReferenceSegmenter::new();
    assert!(matches!(
        parser.segment(&[]),
        NativeReferenceListOutcome::Empty
    ));
    assert!(matches!(
        parser.segment(&["", "  "]),
        NativeReferenceListOutcome::Empty
    ));
    assert!(matches!(
        parser.segment(&["ok", ""]),
        NativeReferenceListOutcome::Malformed { .. }
    ));

    let limits = NativeReferenceSegmenterLimits {
        total_bytes: 4,
        max_references: 2,
        max_segment_bytes: 3,
    };
    let bounded = NativeReferenceSegmenter::with_limits(limits);
    assert!(matches!(
        bounded.segment(&["abcd"]),
        NativeReferenceListOutcome::Oversized
    ));
    assert!(matches!(
        bounded.segment(&["one", "two", "three"]),
        NativeReferenceListOutcome::Oversized
    ));
    let count_limited = NativeReferenceSegmenter::with_limits(NativeReferenceSegmenterLimits {
        total_bytes: 32,
        max_references: 2,
        max_segment_bytes: 8,
    });
    assert!(matches!(
        count_limited.segment(&["one", "two", "three"]),
        NativeReferenceListOutcome::Truncated
    ));
    assert!(matches!(
        bounded.segment(&["four"]),
        NativeReferenceListOutcome::Oversized
    ));
}

#[test]
fn duplicates_punctuation_and_bracket_spanning_text_are_not_normalized() {
    let input = ["[A; B] (2024).", "[A; B] (2024)."];
    let NativeReferenceListOutcome::Recognized { references } =
        NativeReferenceSegmenter::new().segment(&input)
    else {
        panic!()
    };
    assert_eq!(references[0].source, "[A; B] (2024).");
    assert_eq!(references[1].source, "[A; B] (2024).");
}
