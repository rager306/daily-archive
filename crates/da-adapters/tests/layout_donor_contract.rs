#![cfg(feature = "pdf-layout")]

use da_adapters::{
    LayoutDonor, LayoutDonorError, LayoutDonorLimits, PdfInspectorLayoutDonor, PdfaltoLayoutDonor,
    parse_pdfalto_xml, reorder_with_xy_cut,
};

#[test]
fn pdfalto_xml_and_pdf_inspector_share_neutral_identity() {
    let pdf = include_bytes!("../../../data/sources/arxiv/cs-cl/1702.01806/source/1702.01806.pdf");
    let rust = PdfInspectorLayoutDonor.extract(pdf, "1702.01806", LayoutDonorLimits::default());
    let rust_identity = match &rust {
        Ok(rust) => {
            assert_eq!(rust.document_id, "1702.01806");
            assert_eq!(rust.source_pdf_sha256, sha256(pdf));
            assert!(!rust.pages.is_empty());
            assert!(
                rust.pages
                    .iter()
                    .flat_map(|p| &p.tokens)
                    .all(|t| t.bbox.x_min.is_finite())
            );
            (rust.document_id.clone(), rust.source_pdf_sha256.clone())
        }
        Err(LayoutDonorError::Malformed { detail }) => {
            assert!(detail.contains("geometry"));
            ("1702.01806".to_owned(), sha256(pdf))
        }
        Err(_) => panic!("unexpected positioned donor outcome"),
    };

    let xml = br#"<alto><page width="612" height="792" label="nonnumeric-is-ignored"><text x="10" y="20" width="30" height="8">Hello</text><text x="50" y="20" width="20" height="8">world</text></page></alto>"#;
    let alto = parse_pdfalto_xml(xml, pdf, "1702.01806", LayoutDonorLimits::default()).unwrap();
    assert_eq!(alto.document_id, rust_identity.0);
    assert_eq!(alto.source_pdf_sha256, rust_identity.1);
    assert_eq!(alto.pages[0].tokens.len(), 2);
    assert_eq!((alto.pages[0].width, alto.pages[0].height), (612.0, 792.0));
    assert_eq!(alto.pages[0].tokens[0].bbox.y_min, 764.0);

    let real_alto_shape = br#"<alto><Layout><Page ID="Page1" WIDTH="595" HEIGHT="842"><PrintSpace><TextBlock><TextLine><String ID="p1_w1" CONTENT="word" HPOS="18.34" VPOS="423.9" WIDTH="18" HEIGHT="16.1" STYLEREFS="font0"/></TextLine></TextBlock></PrintSpace></Page></Layout></alto>"#;
    let real = parse_pdfalto_xml(
        real_alto_shape,
        pdf,
        "1702.01806",
        LayoutDonorLimits::default(),
    )
    .unwrap();
    assert_eq!(real.pages[0].tokens.len(), 1);
    assert_eq!(real.pages[0].tokens[0].bbox.y_min, 423.9);
    assert_eq!(real.pages[0].tokens[0].bbox.y_max, 440.0);

    let with_empty_page = br#"<alto><Layout><Page WIDTH="595" HEIGHT="842"></Page><Page WIDTH="612" HEIGHT="792"><String CONTENT="x" HPOS="10" VPOS="20" WIDTH="5" HEIGHT="7"/></Page></Layout></alto>"#;
    let pages = parse_pdfalto_xml(
        with_empty_page,
        pdf,
        "1702.01806",
        LayoutDonorLimits::default(),
    )
    .unwrap();
    assert_eq!(pages.pages.len(), 2, "empty pdfalto pages stay visible");
    assert!(pages.pages[0].tokens.is_empty());
    assert_eq!(pages.pages[1].tokens.len(), 1);
}

#[test]
fn xy_cut_is_lossless_and_keeps_source_order_separate() {
    let xml = br#"<doc><page width="612" height="792"><text x="10" y="20" width="30" height="8">left</text><text x="300" y="20" width="30" height="8">right</text><text x="10" y="40" width="30" height="8">next</text></page></doc>"#;
    let before = parse_pdfalto_xml(xml, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    let source: Vec<u32> = before.pages[0]
        .tokens
        .iter()
        .map(|t| t.source_ordinal)
        .collect();
    let after = reorder_with_xy_cut(before).unwrap();
    let after_source: Vec<u32> = after.pages[0]
        .tokens
        .iter()
        .map(|t| t.source_ordinal)
        .collect();
    assert_eq!(after.pages[0].tokens.len(), 3);
    assert_eq!(source.len(), after_source.len());
    assert_eq!(
        source
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        after_source.iter().copied().collect()
    );
    assert_eq!(
        after.pages[0]
            .tokens
            .iter()
            .map(|t| t.token_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn malformed_and_limits_are_typed_and_external_donor_does_not_fallback() {
    let bad = parse_pdfalto_xml(
        br#"<page width="612" height="792"><text x="nan" y="1" width="2" height="3">x</text>"#,
        b"pdf",
        "p",
        LayoutDonorLimits::default(),
    );
    assert!(matches!(bad, Err(LayoutDonorError::Malformed { .. })));

    let missing_page_geometry = parse_pdfalto_xml(
        br#"<doc><page><text x="1" y="1" width="2" height="3">x</text></page></doc>"#,
        b"pdf",
        "p",
        LayoutDonorLimits::default(),
    );
    assert!(matches!(
        missing_page_geometry,
        Err(LayoutDonorError::Malformed { .. })
    ));

    let limited = parse_pdfalto_xml(
        br#"<doc><page width="612" height="792"><text x="1" y="1" width="2" height="3">x</text><text x="4" y="1" width="2" height="3">y</text></page></doc>"#,
        b"pdf", "p", LayoutDonorLimits { max_tokens: 1, ..Default::default() },
    );
    assert!(matches!(
        limited,
        Err(LayoutDonorError::Oversize {
            resource: "tokens",
            ..
        })
    ));

    let unavailable = PdfaltoLayoutDonor::new("definitely-not-a-pdfalto-binary").extract(
        b"pdf",
        "p",
        LayoutDonorLimits::default(),
    );
    assert!(matches!(
        unavailable,
        Err(LayoutDonorError::Unavailable { .. })
    ));
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
