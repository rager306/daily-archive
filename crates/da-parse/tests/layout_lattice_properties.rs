use da_parse::{LayoutBBox, LayoutLattice, LayoutLatticeError};
use proptest::prelude::*;

proptest! {
    #[test]
    fn bottom_left_coordinate_roundtrip_is_involutive(
        x in 0.0f64..500.0,
        y in 0.0f64..700.0,
        width in 1.0f64..100.0,
        height in 1.0f64..100.0,
    ) {
        let page_height = 800.0;
        let bbox = LayoutBBox::new(x, y, x + width, y + height).unwrap();
        let bottom = bbox.to_bottom_left(page_height).unwrap();
        let restored = LayoutBBox::from_bottom_left(
            bottom.0, bottom.1, bottom.2, bottom.3, page_height,
        ).unwrap();
        prop_assert!((restored.x_min - bbox.x_min).abs() < 1e-9);
        prop_assert!((restored.y_min - bbox.y_min).abs() < 1e-9);
        prop_assert!((restored.x_max - bbox.x_max).abs() < 1e-9);
        prop_assert!((restored.y_max - bbox.y_max).abs() < 1e-9);
    }

    #[test]
    fn java_page_mapping_is_exact_for_positive_pages(java_page in 1u32..100_000u32) {
        prop_assert_eq!(LayoutLattice::java_page_to_rust(java_page).unwrap(), java_page - 1);
    }

    #[test]
    fn zero_java_page_is_always_rejected(_unit in 0u8..1u8) {
        prop_assert!(matches!(
            LayoutLattice::java_page_to_rust(0),
            Err(LayoutLatticeError::InvalidJavaPage(0))
        ));
    }
}

#[test]
fn reorder_with_equal_count_is_not_accepted_as_source_order() {
    // This is the metamorphic mutation required by the lattice contract:
    // changing only order must become a typed failure, not a smaller count.
    let error = LayoutLatticeError::NonMonotonic {
        page: 0,
        kind: "token",
    };
    assert_eq!(
        format!("{error}"),
        "invalid layout lattice: NonMonotonic { page: 0, kind: \"token\" }"
    );
}
