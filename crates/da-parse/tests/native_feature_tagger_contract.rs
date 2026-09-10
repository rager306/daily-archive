use da_parse::*;
use sha2::{Digest, Sha256};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture", "1", "a".repeat(64)).unwrap()
}

fn fixture() -> (
    LayoutDocument,
    LayoutLattice,
    FullSegmentationOutcome,
    NativeFullRegionAccounting,
) {
    let texts = ["Introduction", "prose"];
    let tokens = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(10.0, 20.0 + i as f64 * 20.0, 80.0, 30.0 + i as f64 * 20.0)
                    .unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::Word,
                LayoutStyleHints::default(),
                provider(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let document = LayoutDocument::new(
        "fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 200.0, 300.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let lattice_tokens = tokens
        .iter()
        .map(|t| LatticeToken {
            source_ordinal: t.source_ordinal,
            token_ordinal: t.token_ordinal,
            bbox: t.bbox,
            text_digest: t.text_digest.clone(),
            source_span: t.source_span.clone(),
            kind: t.kind,
        })
        .collect();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 200.0,
            height: 300.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, 2).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, 2).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 100.0, 300.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes = texts
        .iter()
        .enumerate()
        .map(|(i, text)| LexemeRow::new("fixture", "fixture", 0, i as u32, digest(text), *text))
        .collect::<Vec<_>>();
    let segmentation =
        segment_document_full(&document, &lattice, &lexemes, SegmentationConfig::default())
            .unwrap();
    let accounting = account_full_regions(
        &document,
        &lattice,
        &segmentation,
        NativeBodyLimits::default(),
    )
    .unwrap();
    (document, lattice, segmentation, accounting)
}

#[test]
fn renders_only_structural_atoms_from_identity_matched_conserved_units() {
    let (_document, lattice, segmentation, accounting) = fixture();
    let output = NativeFeatureTagger::render(NativeFeatureInput {
        lattice: &lattice,
        segmentation: &segmentation,
        accounting: &accounting,
        upstream: FeatureUpstreamStatus::Pass,
    })
    .unwrap();
    assert_eq!(output.columns(), NATIVE_FEATURE_COLUMNS);
    assert_eq!(output.rows.len(), accounting.admitted);
    assert_eq!(output.admitted, output.conserved);
    assert_eq!(output.loss, 0);
    assert!(
        output
            .rows
            .iter()
            .all(|row| row.lexicon_flags == FeatureDisposition::UnsupportedLexiconBacked)
    );
    assert_eq!(
        output,
        NativeFeatureTagger::render(NativeFeatureInput {
            lattice: &lattice,
            segmentation: &segmentation,
            accounting: &accounting,
            upstream: FeatureUpstreamStatus::Pass,
        })
        .unwrap()
    );
}

#[test]
fn visual_units_are_conserved_but_never_rendered_as_lexical_rows() {
    let (_document, lattice, mut segmentation, mut accounting) = fixture();
    let visual = FullRegionAccountingUnit {
        page_ordinal: 0,
        source_ordinal: 9,
        token_ordinal: 9,
        text_digest: digest("visual"),
        kind: FullRegionUnitKind::Image,
        region: None,
    };
    accounting.visual_evidence.push(visual);
    accounting.admitted += 1;
    accounting.conserved += 1;
    // The lattice-bound segmentation has no matching visual token: fail closed,
    // rather than inventing a lexical feature row.
    segmentation.identity.lattice_fingerprint = lattice.digest().unwrap();
    let error = NativeFeatureTagger::render(NativeFeatureInput {
        lattice: &lattice,
        segmentation: &segmentation,
        accounting: &accounting,
        upstream: FeatureUpstreamStatus::Pass,
    })
    .unwrap_err();
    assert!(matches!(error, NativeFeatureError::AccountingMismatch));
}

#[test]
fn upstream_terminal_status_propagates_without_zero_or_pass_rows() {
    let (_document, lattice, segmentation, accounting) = fixture();
    let output = NativeFeatureTagger::render(NativeFeatureInput {
        lattice: &lattice,
        segmentation: &segmentation,
        accounting: &accounting,
        upstream: FeatureUpstreamStatus::Terminal {
            code: "p2_failed".into(),
        },
    })
    .unwrap();
    assert!(output.rows.is_empty());
    assert_eq!(
        output.disposition,
        FeatureDisposition::PropagatedTerminal {
            code: "p2_failed".into()
        }
    );
    assert_eq!(output.admitted, 0);
    assert_eq!(output.loss, accounting.admitted);
}

#[test]
fn public_surface_has_no_tagger_or_model_lifecycle() {
    let names = [
        "GenericTagger",
        "TaggerFactory",
        "Trainer",
        "train",
        "evaluate",
        "model_load",
    ];
    let source = include_str!("../src/native_feature_tagger.rs");
    for forbidden in names {
        assert!(
            !source.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
}
