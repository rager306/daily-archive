use da_parse::{
    CompositionSpan, HeaderCompositionInput, NativeCitationAnchorOutcome, NativeDateOutcome,
    NativeReferenceListOutcome, compose_header, compose_references,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn arbitrary_bounded_header_inputs_never_panic(
        date_year in 0i32..12000,
        author_count in 0usize..8,
        span_start in 0usize..100,
    ) {
        let date = da_parse::RecognizedDate::new(date_year, None, None)
            .map(|date| NativeDateOutcome::Recognized { date })
            .unwrap_or(NativeDateOutcome::Empty);
        let authors = (0..author_count).map(|ordinal| da_parse::HeaderAuthorInput {
            ordinal,
            span: CompositionSpan { start: span_start + ordinal * 10, end: span_start + ordinal * 10 + 1 },
            name: da_parse::NativePersonalNameOutcome::Empty,
            affiliations: da_parse::NativeAffiliationLinkOutcome::Error {
                error: da_parse::NativeAffiliationLinkError::MissingKey { ordinal },
            },
        }).collect();
        let outcome = compose_header(HeaderCompositionInput { date, authors, affiliation_components: vec![] });
        prop_assert!(
            matches!(outcome, da_parse::HeaderCompositionOutcome::Rejected { .. }),
            "header must fail closed"
        );
    }

    #[test]
    fn arbitrary_reference_ordinals_are_closed_without_panic(
        ordinals in prop::collection::vec(0usize..32, 0..16),
    ) {
        let references = ordinals.into_iter().map(|ordinal| da_parse::NativeReference {
            ordinal,
            source: "r".to_owned(),
            span: da_parse::CitationSpan { start: 0, end: 1 },
            provenance: "test".to_owned(),
        }).collect();
        let outcome = compose_references(da_parse::ReferenceCompositionInput {
            references: NativeReferenceListOutcome::Recognized { references },
            fields: vec![],
            anchors: NativeCitationAnchorOutcome::Empty,
        });
        prop_assert!(
            matches!(outcome,
                da_parse::ReferenceCompositionOutcome::Accepted { .. }
                    | da_parse::ReferenceCompositionOutcome::Rejected { .. }),
            "reference outcome must be closed"
        );
    }

    #[test]
    fn serde_replay_is_byte_stable_for_closed_reference_outcomes(
        n in 0usize..8,
    ) {
        let refs = (0..n).map(|ordinal| da_parse::NativeReference {
            ordinal,
            source: "r".to_owned(),
            span: da_parse::CitationSpan { start: 0, end: 1 },
            provenance: "test".to_owned(),
        }).collect();
        let outcome = compose_references(da_parse::ReferenceCompositionInput {
            references: NativeReferenceListOutcome::Recognized { references: refs },
            fields: vec![],
            anchors: NativeCitationAnchorOutcome::Empty,
        });
        let bytes = serde_json::to_vec(&outcome).unwrap();
        let replay: da_parse::ReferenceCompositionOutcome = serde_json::from_slice(&bytes).unwrap();
        prop_assert_eq!(bytes, serde_json::to_vec(&replay).unwrap());
    }
}
