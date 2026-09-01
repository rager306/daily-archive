#![cfg(feature = "pdf-layout")]

use da_adapters::{
    LayoutDonorError, LayoutDonorLimits, VisualAbsenceReason, VisualError, VisualObservation,
    VisualPrimitive, VisualPrimitiveKind, VisualPrimitiveState, VisualProvider,
    parse_pdfalto_xml_shadow,
};
use da_parse::LayoutBBox;

const XML: &[u8] = br#"<doc><page width="612" height="792"><text x="10" y="20" width="30" height="8">opaque</text></page></doc>"#;

fn observation() -> da_adapters::LayoutShadowObservation {
    parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap()
}

fn digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn provider() -> VisualProvider {
    VisualProvider::new("pdf-inspector-visual", "unavailable").unwrap()
}

#[test]
fn donor_returns_typed_terminal_visual_absence_without_empty_success() {
    let shadow = observation();
    assert_eq!(shadow.visual.primitives.len(), 0);
    assert_eq!(
        shadow.visual.absence,
        Some(VisualAbsenceReason::NotObserved)
    );
    shadow.visual.validate(&shadow.document).unwrap();
}

#[test]
fn observed_primitive_is_source_bound_and_72dpi_top_left() {
    let shadow = observation();
    let bbox = LayoutBBox::new(10.0, 20.0, 40.0, 28.0).unwrap();
    let primitive = VisualPrimitive::observed(
        &shadow.document,
        provider(),
        1,
        7,
        VisualPrimitiveKind::Graphic,
        bbox,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .unwrap();
    let visual = VisualObservation::new(&shadow.document, vec![primitive]).unwrap();
    assert_eq!(visual.document_id, "paper");
    assert_eq!(visual.source_pdf_sha256, shadow.document.source_pdf_sha256);
    assert_eq!(visual.primitives[0].bbox, Some(bbox));
    assert!(matches!(
        visual.primitives[0].state,
        VisualPrimitiveState::Observed { .. }
    ));
}

#[test]
fn malformed_digest_and_out_of_page_geometry_are_rejected() {
    let shadow = observation();
    let bbox = LayoutBBox::new(10.0, 20.0, 40.0, 28.0).unwrap();
    assert!(matches!(
        VisualPrimitive::observed(
            &shadow.document,
            provider(),
            1,
            0,
            VisualPrimitiveKind::Image,
            bbox,
            "bad"
        ),
        Err(VisualError::InvalidDigest)
    ));
    let outside = LayoutBBox::new(600.0, 20.0, 620.0, 28.0).unwrap();
    assert!(matches!(
        VisualPrimitive::observed(
            &shadow.document,
            provider(),
            1,
            0,
            VisualPrimitiveKind::Image,
            outside,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ),
        Err(VisualError::Layout(da_parse::LayoutError::OutOfPage))
    ));
}

#[test]
fn duplicate_identity_and_mixed_absence_are_rejected() {
    let shadow = observation();
    let first = VisualPrimitive::absent(
        &shadow.document,
        provider(),
        1,
        0,
        VisualPrimitiveKind::Table,
        VisualAbsenceReason::Unsupported,
    )
    .unwrap();
    let second = first.clone();
    assert!(matches!(
        VisualObservation::new(&shadow.document, vec![first, second]),
        Err(VisualError::DuplicateIdentity)
    ));
    let mut terminal = VisualObservation::terminal_absence(
        &shadow.document,
        provider(),
        VisualAbsenceReason::Unsupported,
    )
    .unwrap();
    terminal.primitives.push(
        VisualPrimitive::absent(
            &shadow.document,
            provider(),
            1,
            0,
            VisualPrimitiveKind::Formula,
            VisualAbsenceReason::Unsupported,
        )
        .unwrap(),
    );
    assert!(matches!(
        terminal.validate(&shadow.document),
        Err(VisualError::InvalidIdentity("observation"))
    ));
}

#[test]
fn debug_and_json_are_redacted_of_payload_and_paths() {
    let shadow = observation();
    let debug = format!("{:?}", shadow.visual);
    let json = serde_json::to_string(&shadow.visual).unwrap();
    for forbidden in ["opaque", "PDF bytes", "/tmp/", "caption", "pixels"] {
        assert!(!debug.contains(forbidden));
        assert!(!json.contains(forbidden));
    }
    assert!(debug.contains("primitive_count"));
    assert!(json.contains("not_observed"));
}

#[test]
fn xy_cut_preserves_visual_receipt_and_terminal_state() {
    let before = observation();
    let expected = before.visual.clone();
    let after = da_adapters::reorder_shadow_with_xy_cut(before).unwrap();
    assert_eq!(after.visual, expected);
    assert_eq!(after.visual.absence, Some(VisualAbsenceReason::NotObserved));
}

#[test]
fn explicit_visual_xml_is_retained_as_digest_only_primitives() {
    let xml = br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="caption-free-image"/><Table x="50" y="30" width="100" height="60" content_digest="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"/><Formula x="70" y="100" width="25" height="12" content="x plus y"/><Rect x="2" y="3" width="4" height="5" content="stroke"/></page></doc>"#;
    let shadow =
        parse_pdfalto_xml_shadow(xml, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    assert_eq!(shadow.visual.absence, None);
    assert_eq!(shadow.visual.primitives.len(), 4);
    assert_eq!(shadow.visual.primitives[0].kind, VisualPrimitiveKind::Image);
    assert_eq!(shadow.visual.primitives[1].kind, VisualPrimitiveKind::Table);
    assert_eq!(
        shadow.visual.primitives[2].kind,
        VisualPrimitiveKind::Formula
    );
    assert_eq!(shadow.visual.primitives[3].kind, VisualPrimitiveKind::Rect);
    assert_eq!(shadow.visual.primitives[0].bbox.unwrap().y_min, 20.0);
    assert_eq!(shadow.visual.primitives[0].bbox.unwrap().y_max, 60.0);
    assert!(matches!(
        &shadow.visual.primitives[0].state,
        VisualPrimitiveState::Observed { content_digest }
            if content_digest == &digest("caption-free-image")
    ));
    let encoded = serde_json::to_string(&shadow.visual).unwrap();
    assert!(!encoded.contains("caption-free-image"));
    assert!(!encoded.contains("x plus y"));
    assert!(!encoded.contains("stroke"));
}

#[test]
fn bottom_left_visual_geometry_is_normalized_to_top_left() {
    let xml = br#"<doc><page width="612" height="792"><Illustration x="10" y="700" width="30" height="40" origin="bottom_left" digest="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"/></page></doc>"#;
    let shadow =
        parse_pdfalto_xml_shadow(xml, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    let bbox = shadow.visual.primitives[0].bbox.unwrap();
    assert_eq!(
        (bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max),
        (10.0, 52.0, 40.0, 92.0)
    );
}

#[test]
fn malformed_visual_geometry_is_terminal_error_not_empty_success() {
    let xml = br#"<doc><page width="612" height="792"><Formula x="nan" y="1" width="2" height="3" digest="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"/></page></doc>"#;
    let result = parse_pdfalto_xml_shadow(xml, b"pdf", "paper", LayoutDonorLimits::default());
    assert!(matches!(
        result,
        Err(LayoutDonorError::Malformed { .. }) | Err(LayoutDonorError::InvalidLayout(_))
    ));
}
