use da_parse::{
    LayoutBBox, LayoutDocument, LayoutPage, LayoutProvider, LayoutSpan, LayoutStyleHints,
    LayoutToken, LayoutTokenKind,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn finite_boxes_round_trip(x in 0.0f64..500.0, y in 0.0f64..700.0, w in 0.01f64..100.0, h in 0.01f64..80.0) {
        let bbox = LayoutBBox::new(x, y, x + w, y + h).unwrap();
        let bottom = bbox.to_bottom_left(800.0).unwrap();
        let round_trip = LayoutBBox::from_bottom_left(bottom.0, bottom.1, bottom.2, bottom.3, 800.0).unwrap();
        prop_assert!((round_trip.x_min - bbox.x_min).abs() < 1e-8);
        prop_assert!((round_trip.y_min - bbox.y_min).abs() < 1e-8);
        prop_assert!((round_trip.x_max - bbox.x_max).abs() < 1e-8);
        prop_assert!((round_trip.y_max - bbox.y_max).abs() < 1e-8);
    }

    #[test]
    fn canonical_digest_is_stable(ordinal in 0u32..1000) {
        let provider = LayoutProvider::new("rust", "1", "a".repeat(64)).unwrap();
        let token = LayoutToken::new(0, ordinal, ordinal, LayoutBBox::new(1.0, 2.0, 3.0, 4.0).unwrap(), "c".repeat(64), LayoutSpan::absent(da_parse::LayoutSpanAbsence::NotAvailable), LayoutTokenKind::Word, LayoutStyleHints::default(), provider).unwrap();
        let doc = LayoutDocument::new("p", "b".repeat(64), vec![LayoutPage::new(0, 10.0, 10.0, 0, None, vec![token]).unwrap()]).unwrap();
        prop_assert_eq!(doc.digest().unwrap(), doc.digest().unwrap());
    }
}
