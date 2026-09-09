#![cfg(feature = "pdf-layout")]

use da_adapters::{
    LayoutDonorError, LayoutDonorLimits, PdfaltoExitDisposition, PdfaltoInvocationConfig,
    classify_pdfalto_exit, parse_pdfalto_xml_shadow,
};
use da_parse::{CoordinateDialect, FurnitureEvidence, ReadingOrder};

const NESTED: &[u8] = include_bytes!("fixtures/layout_lattice_nested.alto.xml");
const MALFORMED: &[u8] = include_bytes!("fixtures/layout_lattice_malformed.alto.xml");

#[test]
fn audited_invocation_defaults_and_exit_taxonomy_are_closed() {
    let config = PdfaltoInvocationConfig::default();
    assert_eq!(config.files_limit, Some(5000));
    assert!(config.full_font_name && config.no_line_numbers);
    assert!(config.annotation && config.only_graphs_coord);
    assert_eq!(config.fingerprint().len(), 64);
    assert_eq!(
        classify_pdfalto_exit(Some(0)),
        PdfaltoExitDisposition::Success
    );
    assert_eq!(
        classify_pdfalto_exit(Some(5)),
        PdfaltoExitDisposition::SuccessWithWarning
    );
    assert_eq!(
        classify_pdfalto_exit(Some(4)),
        PdfaltoExitDisposition::WriteFailed
    );
    assert_eq!(
        classify_pdfalto_exit(Some(98)),
        PdfaltoExitDisposition::OutOfMemory
    );
    assert_eq!(
        classify_pdfalto_exit(Some(126)),
        PdfaltoExitDisposition::MissingPdfalto
    );
    assert_eq!(
        classify_pdfalto_exit(Some(127)),
        PdfaltoExitDisposition::MissingLibxml2
    );
    assert_eq!(
        classify_pdfalto_exit(Some(143)),
        PdfaltoExitDisposition::TimeoutOrKilled
    );
    assert_eq!(
        classify_pdfalto_exit(Some(17)),
        PdfaltoExitDisposition::UnknownExitCode
    );
}

#[test]
fn nested_alto_emits_conserved_page_block_line_lattice() {
    let shadow = parse_pdfalto_xml_shadow(
        NESTED,
        b"fixture-pdf",
        "nested",
        LayoutDonorLimits::default(),
    )
    .unwrap();
    assert_eq!(shadow.document.pages.len(), 2);
    assert_eq!(shadow.lattice.pages.len(), 2);
    assert_eq!(
        shadow.lattice.coordinate_dialect,
        CoordinateDialect::AltoTopLeft72Dpi
    );
    assert_eq!(shadow.lattice.source_order, ReadingOrder::XmlSource);
    assert_eq!(shadow.lattice.pages[0].java_page_number, 1);
    assert_eq!(shadow.lattice.pages[0].page_ordinal, 0);
    assert_eq!(shadow.lattice.pages[1].java_page_number, 2);
    assert_eq!(shadow.lattice.pages[1].page_ordinal, 1);
    assert_eq!(shadow.lattice.pages[0].blocks.len(), 2);
    assert_eq!(shadow.lattice.pages[0].blocks[0].lines.len(), 2);
    assert_eq!(shadow.lattice.pages[0].blocks[0].lines[0].tokens.len(), 2);
    assert_eq!(shadow.lattice.pages[0].blocks[1].lines[0].tokens.len(), 1);
    assert_eq!(
        shadow.lattice.pages[0].blocks[0].lines[0].newline_boundary_count,
        1
    );
    assert_eq!(
        shadow.lattice.pages[0].blocks[0].furniture,
        FurnitureEvidence::Unknown
    );
    assert_eq!(
        shadow.lattice.pages[0].blocks[0].lines[0].tokens[0]
            .bbox
            .y_min,
        30.0
    );
    assert_eq!(
        shadow.lattice.pages[0].blocks[0].lines[0].tokens[0]
            .bbox
            .y_max,
        42.0
    );
    assert_eq!(shadow.lexemes.len(), 5);
    assert_eq!(
        shadow.lattice.digest().unwrap(),
        shadow.lattice.digest().unwrap()
    );
}

#[test]
fn malformed_zero_area_string_is_accounted_as_terminal_failure() {
    let result = parse_pdfalto_xml_shadow(
        MALFORMED,
        b"fixture-pdf",
        "malformed",
        LayoutDonorLimits::default(),
    );
    assert!(
        matches!(result, Err(LayoutDonorError::Malformed { detail }) if detail.contains("geometry"))
    );
}

#[test]
fn incomplete_block_topology_is_not_silently_flattened() {
    let xml = br#"<alto><Layout><Page WIDTH="600" HEIGHT="800"><PrintSpace><TextLine><String CONTENT="orphan" HPOS="20" VPOS="30" WIDTH="40" HEIGHT="12"/></TextLine></PrintSpace></Page></Layout></alto>"#;
    let result =
        parse_pdfalto_xml_shadow(xml, b"fixture-pdf", "orphan", LayoutDonorLimits::default());
    assert!(
        matches!(result, Err(LayoutDonorError::Malformed { detail }) if detail.contains("outside TextBlock"))
    );
}

#[test]
fn malformed_utf8_attribute_is_typed_not_lossy() {
    let xml = b"<alto><Layout><Page WIDTH=\"600\" HEIGHT=\"800\"><String CONTENT=\"x\" HPOS=\"\xff\" VPOS=\"30\" WIDTH=\"40\" HEIGHT=\"12\"/></Page></Layout></alto>";
    let result =
        parse_pdfalto_xml_shadow(xml, b"fixture-pdf", "utf8", LayoutDonorLimits::default());
    assert!(matches!(result, Err(LayoutDonorError::Malformed { .. })));
}
