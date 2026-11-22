#![cfg(feature = "pdf-layout")]

use da_adapters::layout_donors::{LayoutDonorError, LayoutDonorLimits};
use da_adapters::layout_visual::{VisualAbsenceReason, VisualPrimitiveKind, VisualPrimitiveState};
use da_adapters::native_pdf_visual::{
    NATIVE_PDF_VISUAL_PROVIDER, NATIVE_PDF_VISUAL_VERSION, extract_native_pdf_visual,
};
use da_parse::LayoutBBox;
use lopdf::{Document, Object, Stream, dictionary};
use sha2::{Digest, Sha256};

fn fixture(
    image_bytes: &[u8],
    drawing: &[u8],
    configure: impl FnOnce(&mut lopdf::Dictionary),
) -> Vec<u8> {
    let mut pdf = Document::with_version("1.5");
    let pages_id = pdf.new_object_id();
    let image_id = pdf.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => 1,
            "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
        },
        image_bytes.to_vec(),
    ));
    let font_id = pdf.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding", "FirstChar" => 32, "LastChar" => 126,
        "Widths" => vec![Object::Integer(600); 95],
    });
    let mut content = b"BT /F1 12 Tf 20 700 Td (private-source-text) Tj ET\n".to_vec();
    content.extend_from_slice(drawing);
    let content_id = pdf.add_object(Stream::new(dictionary! {}, content));
    let mut page = dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! {
            "XObject" => dictionary! { "Im0" => image_id },
            "Font" => dictionary! { "F1" => font_id },
        },
    };
    configure(&mut page);
    let page_id = pdf.add_object(page);
    pdf.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
        }
        .into(),
    );
    let catalog_id = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    pdf.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

const IMAGE_PLACEMENT: &[u8] = b"q 30 0 0 40 10 20 cm /Im0 Do Q";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn opt_in_lexical_donor_excludes_images_without_renumbering_surviving_text() {
    let drawing = [
        &b"q 30 0 0 10 10 670 cm /Im0 Do Q"[..],
        b"\nBT /F1 12 Tf 20 650 Td (text-after-image) Tj ET",
    ]
    .concat();
    let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
    let donor = da_adapters::PdfInspectorLayoutDonor;
    let legacy = donor
        .extract_shadow(&pdf, "fixture", LayoutDonorLimits::default())
        .unwrap();
    let lexical = donor
        .extract_lexical_shadow(&pdf, "fixture", LayoutDonorLimits::default())
        .unwrap();
    assert_eq!(
        legacy.document.source_pdf_sha256,
        lexical.document.source_pdf_sha256
    );
    assert_eq!(legacy.document.document_id, lexical.document.document_id);
    assert_eq!(
        legacy
            .lexemes
            .iter()
            .map(|row| row.text())
            .collect::<Vec<_>>(),
        ["private-source-text", "[Image: Im0]", "text-after-image"],
    );
    let expected = legacy
        .lexemes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, row)| {
            (
                row.document_id.clone(),
                row.provider_id.clone(),
                row.page_ordinal,
                row.source_ordinal,
                row.text_digest.clone(),
                row.text().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let observed = lexical
        .lexemes
        .iter()
        .map(|row| {
            (
                row.document_id.clone(),
                row.provider_id.clone(),
                row.page_ordinal,
                row.source_ordinal,
                row.text_digest.clone(),
                row.text().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(observed, expected);
    assert_eq!(
        lexical
            .lexemes
            .iter()
            .map(|row| row.source_ordinal)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    let lexical_tokens = lexical
        .document
        .pages
        .iter()
        .flat_map(|page| &page.tokens)
        .map(|token| (token.source_ordinal, token.text_digest.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        lexical_tokens,
        lexical
            .lexemes
            .iter()
            .map(|row| (row.source_ordinal, row.text_digest.clone()))
            .collect::<Vec<_>>()
    );
    for observation in [&legacy, &lexical] {
        assert!(observation.visual.primitives.is_empty());
        assert_eq!(
            observation.visual.absence,
            Some(VisualAbsenceReason::NotObserved)
        );
    }
    let trait_default = da_adapters::LayoutDonor::extract_shadow(
        &donor,
        &pdf,
        "fixture",
        LayoutDonorLimits::default(),
    )
    .unwrap();
    assert_eq!(trait_default, legacy);
}

#[test]
fn actual_image_source_stream_has_exact_geometry_and_digest_without_text() {
    let image = [17, 35, 71];
    let pdf = fixture(&image, IMAGE_PLACEMENT, |_| {});
    let extracted =
        extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()).unwrap();
    assert_eq!(extracted.document.source_pdf_sha256, sha256(&pdf));
    assert_eq!(extracted.document.pages.len(), 1);
    assert_eq!(
        (
            extracted.document.pages[0].width,
            extracted.document.pages[0].height
        ),
        (612.0, 792.0)
    );
    assert!(extracted.document.pages[0].tokens.is_empty());
    assert_eq!(extracted.visual.provider.id, NATIVE_PDF_VISUAL_PROVIDER);
    assert_eq!(extracted.visual.provider.version, NATIVE_PDF_VISUAL_VERSION);
    assert_eq!(extracted.visual.absence, None);
    assert_eq!(extracted.visual.primitives.len(), 1);
    let primitive = &extracted.visual.primitives[0];
    assert_eq!(primitive.page_ordinal, 0);
    assert_eq!(primitive.kind, VisualPrimitiveKind::Image);
    assert_eq!(
        primitive.bbox,
        Some(LayoutBBox::new(10.0, 732.0, 40.0, 772.0).unwrap())
    );
    assert_eq!(
        primitive.state,
        VisualPrimitiveState::Observed {
            content_digest: sha256(&image)
        }
    );
    extracted.visual.validate(&extracted.document).unwrap();
    let wire = serde_json::to_string(&extracted.visual).unwrap();
    assert!(!wire.contains("private-source-text"));
    assert!(!wire.contains("Im0"));
    assert!(!format!("{extracted:?}").contains("private-source-text"));
}

#[test]
fn inherited_page_geometry_resources_and_nested_axis_transforms_are_supported() {
    let bytes = fixture(
        &[17, 35, 71],
        b"q 2 0 0 2 10 20 cm q 3 0 0 4 5 6 cm /Im0 Do Q Q",
        |page| {
            page.set("CropBox", vec![0.into(), 0.into(), 612.into(), 792.into()]);
        },
    );
    let mut pdf = Document::load_mem(&bytes).unwrap();
    let page_id = pdf.get_pages()[&1];
    let page = pdf.get_dictionary_mut(page_id).unwrap();
    let parent_id = page.get(b"Parent").and_then(Object::as_reference).unwrap();
    let media = page.remove(b"MediaBox").unwrap();
    let resources = page.remove(b"Resources").unwrap();
    let parent = pdf.get_dictionary_mut(parent_id).unwrap();
    parent.set("MediaBox", media);
    parent.set("Resources", resources);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let extracted =
        extract_native_pdf_visual(&bytes, "fixture", LayoutDonorLimits::default()).unwrap();
    assert_eq!(
        extracted.visual.primitives[0].bbox,
        Some(LayoutBBox::new(20.0, 752.0, 26.0, 760.0).unwrap())
    );
    assert_eq!(
        extracted.document.pages[0].crop_box,
        Some(LayoutBBox::new(0.0, 0.0, 612.0, 792.0).unwrap())
    );
}

#[test]
fn image_digest_changes_with_stream_bytes_not_placement_or_resource_placeholder() {
    let first = fixture(&[17, 35, 71], IMAGE_PLACEMENT, |_| {});
    let changed = fixture(&[18, 35, 71], IMAGE_PLACEMENT, |_| {});
    let moved = fixture(&[17, 35, 71], b"q 30 0 0 40 50 60 cm /Im0 Do Q", |_| {});
    let observations = [&first, &changed, &moved].map(|pdf| {
        extract_native_pdf_visual(pdf, "fixture", LayoutDonorLimits::default())
            .unwrap()
            .visual
    });
    assert_ne!(
        observations[0].primitives[0].state,
        observations[1].primitives[0].state
    );
    assert_eq!(
        observations[0].primitives[0].state,
        observations[2].primitives[0].state
    );
    assert_ne!(
        observations[0].primitives[0].bbox,
        observations[2].primitives[0].bbox
    );
    assert_ne!(
        observations[0].source_pdf_sha256,
        observations[2].source_pdf_sha256
    );
}

#[test]
fn text_only_or_uninvoked_image_resource_is_not_observed() {
    let pdf = fixture(&[17, 35, 71], b"", |_| {});
    let extracted =
        extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()).unwrap();
    assert!(extracted.visual.primitives.is_empty());
    assert_eq!(
        extracted.visual.absence,
        Some(VisualAbsenceReason::NotObserved)
    );
}

#[test]
fn unsupported_transforms_and_clipping_are_not_partial_success() {
    for unsupported in [
        "q 0 30 -40 0 10 20 cm /Im0 Do Q",
        "q 30 1 0 40 10 20 cm /Im0 Do Q",
        "q 0 0 10 10 re W n /Im0 Do Q",
        "/GS0 gs /Im0 Do",
        "BI /W 1 /H 1 /BPC 8 /CS /RGB ID abc EI",
    ] {
        let drawing = [IMAGE_PLACEMENT, b"\n", unsupported.as_bytes()].concat();
        let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
        let extracted =
            extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()).unwrap();
        assert!(extracted.visual.primitives.is_empty());
        assert_eq!(
            extracted.visual.absence,
            Some(VisualAbsenceReason::Unsupported)
        );
    }
}

#[test]
fn crop_rotation_and_nonzero_origin_do_not_receive_guessed_coordinates() {
    let cases = [
        fixture(&[17, 35, 71], IMAGE_PLACEMENT, |page| {
            page.set("Rotate", 90);
        }),
        fixture(&[17, 35, 71], IMAGE_PLACEMENT, |page| {
            page.set(
                "CropBox",
                vec![10.into(), 20.into(), 600.into(), 780.into()],
            );
        }),
        fixture(&[17, 35, 71], IMAGE_PLACEMENT, |page| {
            page.set(
                "MediaBox",
                vec![10.into(), 20.into(), 622.into(), 812.into()],
            );
        }),
    ];
    for pdf in cases {
        let extracted =
            extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()).unwrap();
        assert!(extracted.document.pages.is_empty());
        assert!(extracted.visual.primitives.is_empty());
        assert_eq!(
            extracted.visual.absence,
            Some(VisualAbsenceReason::Unsupported)
        );
    }
}

#[test]
fn forms_are_unsupported_instead_of_inventing_nested_image_provenance() {
    let bytes = fixture(&[17, 35, 71], IMAGE_PLACEMENT, |_| {});
    let mut pdf = Document::load_mem(&bytes).unwrap();
    let page_id = pdf.get_pages()[&1];
    let resources = pdf
        .get_dict_in_dict(pdf.get_dictionary(page_id).unwrap(), b"Resources")
        .unwrap();
    let xobjects = pdf.get_dict_in_dict(resources, b"XObject").unwrap();
    let image_id = xobjects.get(b"Im0").and_then(Object::as_reference).unwrap();
    *pdf.get_object_mut(image_id).unwrap() = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
        },
        b"/Nested Do".to_vec(),
    )
    .into();
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let extracted =
        extract_native_pdf_visual(&bytes, "fixture", LayoutDonorLimits::default()).unwrap();
    assert!(extracted.visual.primitives.is_empty());
    assert_eq!(
        extracted.visual.absence,
        Some(VisualAbsenceReason::Unsupported)
    );
}

#[test]
fn malformed_or_dangling_suffix_never_returns_an_observed_prefix_inventory() {
    for suffix in [
        "[",
        "(",
        "<<",
        "123",
        "/Dangling",
        "(dangling)",
        "[]",
        "<< /K 1 >>",
    ] {
        let drawing = [IMAGE_PLACEMENT, b"\n", suffix.as_bytes()].concat();
        let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
        assert!(matches!(
            extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()),
            Err(LayoutDonorError::Malformed { .. })
        ));
    }
}

#[test]
fn valid_trailing_comments_and_strings_do_not_hide_the_content_boundary() {
    for suffix in [
        "\n% trailing comment without a source newline",
        "\n% trailing comment with an existing source newline\n",
        "\n(BI /W 1 /H 1 /BPC 8 ID abc EI) Tj\n% trailing comment",
        "\n[(literal [ << BI) 12 (second)] TJ\n% first comment\n% second comment",
    ] {
        let drawing = [IMAGE_PLACEMENT, suffix.as_bytes()].concat();
        let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
        let extracted = extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default())
            .unwrap_or_else(|error| panic!("valid suffix {suffix:?} was refused: {error}"));
        assert_eq!(extracted.visual.absence, None, "suffix {suffix:?}");
        assert_eq!(extracted.visual.primitives.len(), 1, "suffix {suffix:?}");
        assert_eq!(
            extracted.visual.primitives[0].state,
            VisualPrimitiveState::Observed {
                content_digest: sha256(&[17, 35, 71]),
            },
            "suffix {suffix:?}"
        );
    }
}

#[test]
fn source_blank_lines_after_comments_are_conservatively_refused_not_skipped() {
    let drawing = [IMAGE_PLACEMENT, b"\n% trailing source comment\n\n"].concat();
    let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
    assert!(matches!(
        extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()),
        Err(LayoutDonorError::Malformed { .. })
    ));
}

#[test]
fn source_content_boundary_collision_is_unsupported_including_inside_a_string() {
    for suffix in [
        "\nDANativeVisualContentEnd",
        "\n(DANativeVisualContentEnd) Tj",
        "\n% DANativeVisualContentEnd",
    ] {
        let drawing = [IMAGE_PLACEMENT, suffix.as_bytes()].concat();
        let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
        let extracted =
            extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()).unwrap();
        assert!(extracted.visual.primitives.is_empty());
        assert_eq!(
            extracted.visual.absence,
            Some(VisualAbsenceReason::Unsupported)
        );
    }
}

#[test]
fn inline_image_missing_colorspace_is_a_typed_error_not_a_partial_inventory_or_panic() {
    let drawing = [IMAGE_PLACEMENT, b"\nBI /W 1 /H 1 /BPC 8 ID abc EI"].concat();
    let pdf = fixture(&[17, 35, 71], &drawing, |_| {});
    let result = std::panic::catch_unwind(|| {
        extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default())
    });
    assert!(matches!(
        result,
        Ok(Err(LayoutDonorError::Malformed { .. }))
    ));
}

#[test]
fn malformed_image_geometry_missing_source_and_limits_are_hard_errors() {
    let outside = fixture(&[17, 35, 71], b"q 30 0 0 40 600 20 cm /Im0 Do Q", |_| {});
    let missing = fixture(&[17, 35, 71], b"/Missing Do", |_| {});
    let empty = fixture(&[], IMAGE_PLACEMENT, |_| {});
    for pdf in [outside, missing, empty] {
        assert!(matches!(
            extract_native_pdf_visual(&pdf, "fixture", LayoutDonorLimits::default()),
            Err(LayoutDonorError::Malformed { .. }) | Err(LayoutDonorError::InvalidLayout(_))
        ));
    }
    let pdf = fixture(&[17, 35, 71], IMAGE_PLACEMENT, |_| {});
    let limits = LayoutDonorLimits {
        max_tokens: 1,
        ..LayoutDonorLimits::default()
    };
    assert!(matches!(
        extract_native_pdf_visual(&pdf, "fixture", limits),
        Err(LayoutDonorError::Oversize { .. })
    ));
}
