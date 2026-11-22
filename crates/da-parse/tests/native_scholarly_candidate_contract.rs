//! Synthetic-model wiring contracts, not real-model or PDF accuracy evidence.

use da_parse::*;
use da_ports::scalar_crf::CrfPattern;
use da_ports::{CrfModel, FeatureTemplate, LexiconBundle, LexiconChannel};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Explicit region assignments are fixture data. The public carrier builder
/// still checks layout/lexeme digests and order; segmentation is not under test.
fn carrier(
    lines: &[(FullSemanticRegion, &[&str])],
    requested: &[FullSemanticRegion],
) -> NativeRegionCarrier {
    let provider = LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap();
    let mut tokens = Vec::new();
    let mut lexemes = Vec::new();
    let mut lattice_lines = Vec::new();
    let mut assignments = Vec::new();
    for (line, (region, words)) in lines.iter().enumerate() {
        let start = tokens.len() as u32;
        let y = 20.0 + line as f64 * 20.0;
        for (column, word) in words.iter().enumerate() {
            let ordinal = tokens.len() as u32;
            let x = 20.0 + column as f64 * 90.0;
            let bbox = LayoutBBox::new(x, y, x + 80.0, y + 10.0).unwrap();
            tokens.push(
                LayoutToken::new(
                    0,
                    ordinal,
                    ordinal,
                    bbox,
                    digest(word),
                    LayoutSpan::present(ordinal, ordinal + 1).unwrap(),
                    LayoutTokenKind::from_lexical_text(word),
                    LayoutStyleHints {
                        font_size_milli_pt: Some(10_000),
                        ..LayoutStyleHints::default()
                    },
                    provider.clone(),
                )
                .unwrap(),
            );
            lexemes.push(LexemeRow::new(
                "candidate-fixture",
                "fixture-donor",
                0,
                ordinal,
                digest(word),
                *word,
            ));
            assignments.push(FullTokenAssignment {
                location: FullLocation {
                    page_ordinal: 0,
                    block_ordinal: 0,
                    line_ordinal: line as u32,
                    token_ordinal: ordinal,
                },
                source_ordinal: ordinal,
                text_digest: digest(word),
                region: *region,
            });
        }
        let end = tokens.len() as u32;
        lattice_lines.push(LatticeLine {
            source_ordinal: line as u32,
            source_span: LayoutSpan::present(start, end).unwrap(),
            bbox: LayoutBBox::new(0.0, y, 4000.0, y + 10.0).unwrap(),
            newline_boundary_count: 1,
            tokens: tokens[start as usize..end as usize]
                .iter()
                .map(|token| LatticeToken {
                    source_ordinal: token.source_ordinal,
                    token_ordinal: token.token_ordinal,
                    bbox: token.bbox,
                    text_digest: token.text_digest.clone(),
                    source_span: token.source_span.clone(),
                    kind: token.kind,
                })
                .collect(),
        });
    }
    let count = tokens.len() as u32;
    let document = LayoutDocument::new(
        "candidate-fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 4000.0, 4000.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 4000.0,
            height: 4000.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 4000.0, 4000.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: lattice_lines,
            }],
        }],
    )
    .unwrap();
    let segmentation = FullSegmentationOutcome {
        schema_version: FULL_SEGMENTATION_SCHEMA_VERSION.into(),
        identity: FullSegmentationIdentity {
            document_id: document.document_id.clone(),
            provider_id: "fixture-donor".into(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            lattice_fingerprint: lattice.digest().unwrap(),
            config_fingerprint: "c".repeat(64),
        },
        order_digest: format!("sha256:{}", digest(&format!("{assignments:?}"))),
        assignments,
        runs: Vec::new(),
        furniture: Vec::new(),
        dispositions: Vec::new(),
    };
    build_native_region_carrier(&document, &lattice, &lexemes, &segmentation, requested).unwrap()
}

fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["Ada".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "candidate-fixture".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

/// Hand-authored token/label pairs, independent of the feature builders.
fn token_model(vocabulary: &[&str], token_labels: &[(&str, &str)]) -> CrfModel {
    let count = vocabulary.len();
    let mut observations = Vec::new();
    let mut weights = BTreeMap::new();
    for (position, (token, label)) in token_labels.iter().enumerate() {
        observations.push(format!("u:{token}"));
        let target = vocabulary.iter().position(|value| value == label).unwrap();
        weights.insert(position * count + target, 10.0);
    }
    CrfModel {
        model_type: 2,
        labels: vocabulary.iter().map(|value| (*value).into()).collect(),
        patterns: vec![CrfPattern {
            source: "u:%x[0,0]".into(),
            kind: 'u',
        }],
        u_offsets: (0..observations.len())
            .map(|index| Some(index * count))
            .collect(),
        b_offsets: vec![None; observations.len()],
        nftr: observations.len() * count,
        observations,
        weights,
    }
}

#[test]
fn missing_date_does_not_erase_other_decoded_header_fields() {
    let input = carrier(
        &[
            (FullSemanticRegion::Header, &["A", "Study"]),
            (FullSemanticRegion::Header, &["Ada", "Lovelace"]),
            (FullSemanticRegion::Header, &["Example", "University,"]),
            (FullSemanticRegion::Header, &["a@example.test"]),
            (FullSemanticRegion::Header, &["arXiv:2401.00001"]),
        ],
        &[FullSemanticRegion::Header],
    );
    let model = token_model(
        NATIVE_HEADER_LABELS,
        &[
            ("A", "I-<title>"),
            ("Study", "<title>"),
            ("Ada", "I-<author>"),
            ("Lovelace", "<author>"),
            ("Example", "I-<affiliation>"),
            ("University,", "<affiliation>"),
            ("a@example.test", "<email>"),
            ("arXiv:2401.00001", "<pubnum>"),
        ],
    );
    let result = build_native_header_candidate(&input, &lexicon(), &model).unwrap();
    assert_eq!(result.fields.title, ["A Study"]);
    assert_eq!(result.fields.email, ["a@example.test"]);
    assert_eq!(result.fields.ids, ["arXiv:2401.00001"]);
    assert!(result.dates.is_empty());
    assert_eq!(result.recognized_date(), None);
    assert_eq!(result.authors.len(), 1);
    assert_eq!(result.authors[0].source_text, "Ada Lovelace");
    assert_eq!(
        result.authors[0]
            .source_rows
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert!(
        matches!(&result.authors[0].outcome, NativePersonalNameOutcome::Recognized { name } if name.surname == "Lovelace")
    );
    assert_eq!(result.affiliations[0].source_text, "Example University,");
    assert_eq!(result.rows.len(), 8);
    assert_eq!(
        result,
        build_native_header_candidate(&input, &lexicon(), &model).unwrap()
    );
}

#[test]
fn decoded_date_and_author_spans_get_one_leaf_outcome_each() {
    let input = carrier(
        &[
            (FullSemanticRegion::Header, &["Study"]),
            (
                FullSemanticRegion::Header,
                &["Ada", "Lovelace", "and", "Alan", "Turing"],
            ),
            (FullSemanticRegion::Header, &["Tomorrow"]),
        ],
        &[FullSemanticRegion::Header],
    );
    let model = token_model(
        NATIVE_HEADER_LABELS,
        &[
            ("Study", "<title>"),
            ("Ada", "I-<author>"),
            ("Lovelace", "<author>"),
            ("and", "I-<author>"),
            ("Alan", "<author>"),
            ("Turing", "<author>"),
            ("Tomorrow", "<date>"),
        ],
    );
    let result = build_native_header_candidate(&input, &lexicon(), &model).unwrap();
    assert_eq!(result.authors.len(), 1);
    assert_eq!(
        result.authors[0].source_text,
        "Ada Lovelace and Alan Turing"
    );
    assert_eq!(result.authors[0].source_rows.len(), 5);
    assert_eq!(result.dates.len(), 1);
    assert_eq!(result.dates[0].source_text, "Tomorrow");
    assert!(!matches!(
        result.dates[0].outcome,
        NativeDateOutcome::Recognized { .. }
    ));
    assert_eq!(result.recognized_date(), None);
    assert_eq!(result.fields.title, ["Study"]);
}

#[test]
fn recognized_date_keeps_only_the_precision_declared_by_its_decoded_span() {
    let input = carrier(
        &[
            (FullSemanticRegion::Header, &["Study"]),
            (FullSemanticRegion::Header, &["Feb", "2026"]),
        ],
        &[FullSemanticRegion::Header],
    );
    let model = token_model(
        NATIVE_HEADER_LABELS,
        &[
            ("Study", "<title>"),
            ("Feb", "I-<date>"),
            ("2026", "<date>"),
        ],
    );
    let result = build_native_header_candidate(&input, &lexicon(), &model).unwrap();
    assert_eq!(
        result.recognized_date(),
        Some(RecognizedDate {
            year: 2026,
            month: Some(2),
            day: None,
            precision: DatePrecision::YearMonth,
        })
    );
    assert_eq!(
        result.dates[0]
            .source_rows
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        [1, 2]
    );
}

fn reference_models() -> (CrfModel, CrfModel) {
    let boundary = token_model(
        &NATIVE_REFERENCE_SEGMENTER_LABELS,
        &[
            ("[7]", "<label>"),
            ("[2]", "<label>"),
            ("Ada", "I-<reference>"),
            ("First", "<reference>"),
            ("Second", "<reference>"),
            ("2024", "<reference>"),
            ("2025", "<reference>"),
            ("continuation", "<reference>"),
        ],
    );
    let citation = token_model(
        &NATIVE_CITATION_LABELS,
        &[
            ("Ada", "<author>"),
            ("First", "<title>"),
            ("Second", "<title>"),
            ("2024", "<date>"),
            ("2025", "<date>"),
        ],
    );
    (boundary, citation)
}

#[test]
fn structured_fields_and_source_rows_remain_owned_by_each_printed_reference() {
    let input = carrier(
        &[
            (
                FullSemanticRegion::References,
                &["[7]", "Ada", "First", "2024"],
            ),
            (
                FullSemanticRegion::References,
                &["[2]", "Ada", "Second", "2025"],
            ),
        ],
        &[FullSemanticRegion::References],
    );
    let (boundary, citation) = reference_models();
    let result =
        build_native_reference_candidate(&input, &lexicon(), &boundary, &citation).unwrap();
    assert!(result.has_complete_labelled_line_inventory());
    assert_eq!(result.records.len(), 2);
    let first = &result.records[0];
    let second = &result.records[1];
    assert_eq!(first.ordinal, 0);
    assert_eq!(second.ordinal, 1);
    assert_eq!(first.printed_label.arabic_key.as_deref(), Some("7"));
    assert_eq!(second.printed_label.arabic_key.as_deref(), Some("2"));
    assert_eq!(first.published_key(), "b0");
    assert_eq!(second.published_key(), "b1");
    assert_eq!(first.citation.cluster_text, "Ada First 2024");
    assert_eq!(second.citation.cluster_text, "Ada Second 2025");
    assert_eq!(
        first
            .citation
            .roles
            .iter()
            .map(|span| (span.role, span.text.as_str()))
            .collect::<Vec<_>>(),
        [
            (CitationRole::Author, "Ada"),
            (CitationRole::Title, "First"),
            (CitationRole::Date, "2024")
        ]
    );
    assert_eq!(second.citation.roles[1].text, "Second");
    assert_eq!(
        first
            .citation_source_rows
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        second
            .citation_source_rows
            .iter()
            .map(|row| row.row)
            .collect::<Vec<_>>(),
        [5, 6, 7]
    );
    assert_ne!(
        first.citation_source_rows[0].location,
        second.citation_source_rows[0].location
    );
    let links = link_native_reference_markers("Résumé [2] then [7] and [1]", &result).unwrap();
    assert_eq!(links.report.resolved, 2);
    assert_eq!(links.report.unresolved, 1);
    assert!(
        matches!(&links.report.links[0].target, CalloutLinkTarget::Resolved { target_id, basis: CalloutLabelBasis::Printed, .. } if target_id == "b1")
    );
    assert!(
        matches!(&links.report.links[1].target, CalloutLinkTarget::Resolved { target_id, .. } if target_id == "b0")
    );
    let marker = &links.report.markers[0];
    assert_eq!(&"Résumé [2] then [7] and [1]"[marker.span.clone()], "[2]");
}

#[test]
fn duplicate_labels_are_ambiguous_not_linked_by_record_order() {
    let input = carrier(
        &[
            (
                FullSemanticRegion::References,
                &["[7]", "Ada", "First", "2024"],
            ),
            (
                FullSemanticRegion::References,
                &["[7]", "Ada", "Second", "2025"],
            ),
        ],
        &[FullSemanticRegion::References],
    );
    let (boundary, citation) = reference_models();
    let result =
        build_native_reference_candidate(&input, &lexicon(), &boundary, &citation).unwrap();
    let links = link_native_reference_markers("[7]", &result).unwrap();
    assert_eq!(links.report.resolved, 0);
    assert_eq!(
        links.report.links[0].target.reason(),
        Some(CalloutLinkUnresolved::Ambiguous)
    );
}

#[test]
fn unlabelled_continuation_is_visible_and_prevents_target_inventory_claim() {
    let input = carrier(
        &[
            (
                FullSemanticRegion::References,
                &["[7]", "Ada", "First", "2024"],
            ),
            (FullSemanticRegion::References, &["continuation"]),
        ],
        &[FullSemanticRegion::References],
    );
    let (boundary, citation) = reference_models();
    let result =
        build_native_reference_candidate(&input, &lexicon(), &boundary, &citation).unwrap();
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.lines.len(), 2);
    assert!(matches!(
        result.lines[1].disposition,
        NativeReferenceLineDisposition::Unsupported {
            reason: NativeReferenceLineUnsupported::NoPrintedLabel,
        }
    ));
    assert_eq!(
        result.lines[1].decoded.as_ref().unwrap().line_text,
        "continuation"
    );
    assert!(!result.has_complete_labelled_line_inventory());
    assert_eq!(
        link_native_reference_markers("[7]", &result),
        Err(NativeScholarlyCandidateError::ReferenceInventoryIncomplete)
    );
}

/// Real heading assignment and word-carrier chain, with synthetic layout only.
fn heading_carrier(
    spans: &[(&str, f64, f64, bool)],
) -> (NativeRegionCarrier, NativeHeadingRegionOutcome) {
    let provider = LayoutProvider::new("heading-fixture", "1", "a".repeat(64)).unwrap();
    let tokens = spans
        .iter()
        .enumerate()
        .map(|(index, (text, x, y, prominent))| {
            LayoutToken::new(
                0,
                index as u32,
                index as u32,
                LayoutBBox::new(*x, *y, x + 260.0, y + 10.0).unwrap(),
                digest(text),
                LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                LayoutTokenKind::from_lexical_text(text),
                LayoutStyleHints {
                    bold: *prominent,
                    font_size_milli_pt: Some(if *prominent { 12_000 } else { 10_000 }),
                    ..LayoutStyleHints::default()
                },
                provider.clone(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let lattice_tokens = tokens
        .iter()
        .map(|token| LatticeToken {
            source_ordinal: token.source_ordinal,
            token_ordinal: token.token_ordinal,
            bbox: token.bbox,
            text_digest: token.text_digest.clone(),
            source_span: token.source_span.clone(),
            kind: token.kind,
        })
        .collect();
    let document = LayoutDocument::new(
        "heading-fixture",
        "b".repeat(64),
        vec![LayoutPage::new(0, 1000.0, 1000.0, 0, None, tokens).unwrap()],
    )
    .unwrap();
    let bounds = LayoutBBox::new(0.0, 0.0, 1000.0, 1000.0).unwrap();
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 1000.0,
            height: 1000.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                bbox: bounds,
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::absent(LayoutSpanAbsence::NotAvailable),
                    bbox: bounds,
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes = spans
        .iter()
        .enumerate()
        .map(|(index, (text, _, _, _))| {
            LexemeRow::new(
                "heading-fixture",
                "heading-fixture",
                0,
                index as u32,
                digest(text),
                *text,
            )
        })
        .collect::<Vec<_>>();
    let projection = project_native_words(&document, &lexemes).unwrap();
    let headings = assign_heading_regions(
        &document,
        &lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .unwrap();
    let carrier = build_native_word_region_carrier(
        &document,
        &lattice,
        &projection,
        &headings,
        &[FullSemanticRegion::References],
    )
    .unwrap();
    (carrier, headings)
}

#[test]
fn source_bound_whole_reference_headings_are_retained_without_blocking_links() {
    for (text, matched_by) in [
        ("References", HeadingMatch::NormalizedLine),
        ("4. References", HeadingMatch::OrdinalStrippedLine),
    ] {
        let (input, headings) = heading_carrier(&[
            ("Study", 20.0, 10.0, false),
            (text, 20.0, 40.0, true),
            ("[7] Ada First 2024", 20.0, 60.0, false),
        ]);
        let (boundary, citation) = reference_models();
        let legacy =
            build_native_reference_candidate(&input, &lexicon(), &boundary, &citation).unwrap();
        assert!(!legacy.has_complete_labelled_line_inventory());
        let result = build_native_reference_candidate_with_headings(
            &input,
            &lexicon(),
            &boundary,
            &citation,
            &headings,
        )
        .unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].decoded.as_ref().unwrap().line_text, text);
        assert_eq!(
            result.lines[0].source_rows.len(),
            text.split_whitespace().count()
        );
        assert_eq!(
            result.lines[0].disposition,
            NativeReferenceLineDisposition::SectionHeading {
                matched_by,
                text_digest: headings.lines[1].text_digest.clone(),
            }
        );
        assert_eq!(
            result.reference_feature_digest,
            legacy.reference_feature_digest
        );
        assert_eq!(result.records, legacy.records);
        assert!(result.has_complete_labelled_line_inventory());
        let links = link_native_reference_markers("[7]", &result).unwrap();
        assert_eq!(links.report.resolved, 1);
        assert!(
            matches!(&links.report.links[0].target, CalloutLinkTarget::Resolved { target_id, .. } if target_id == "b0")
        );
    }
}

#[test]
fn heading_context_does_not_excuse_unlabelled_continuations_or_mixed_heading_lines() {
    let (input, headings) = heading_carrier(&[
        ("Study", 20.0, 10.0, false),
        ("References", 20.0, 40.0, true),
        ("[7] Ada First 2024", 20.0, 60.0, false),
        ("continuation", 20.0, 80.0, false),
    ]);
    let (boundary, citation) = reference_models();
    let result = build_native_reference_candidate_with_headings(
        &input,
        &lexicon(),
        &boundary,
        &citation,
        &headings,
    )
    .unwrap();
    assert!(matches!(
        result.lines[0].disposition,
        NativeReferenceLineDisposition::SectionHeading { .. }
    ));
    assert!(matches!(
        result.lines[2].disposition,
        NativeReferenceLineDisposition::Unsupported {
            reason: NativeReferenceLineUnsupported::NoPrintedLabel
        }
    ));
    assert!(!result.has_complete_labelled_line_inventory());
    assert_eq!(
        link_native_reference_markers("[7]", &result),
        Err(NativeScholarlyCandidateError::ReferenceInventoryIncomplete)
    );

    let (mixed, mixed_headings) = heading_carrier(&[
        ("Study", 20.0, 10.0, false),
        ("References", 20.0, 40.0, true),
        ("[7] Ada First 2024", 400.0, 40.0, false),
    ]);
    assert_eq!(
        mixed_headings.lines[1].matched_by,
        Some(HeadingMatch::LeadingSpanLine)
    );
    let result = build_native_reference_candidate_with_headings(
        &mixed,
        &lexicon(),
        &boundary,
        &citation,
        &mixed_headings,
    )
    .unwrap();
    assert_eq!(result.lines.len(), 1);
    assert!(matches!(
        result.lines[0].disposition,
        NativeReferenceLineDisposition::Unsupported {
            reason: NativeReferenceLineUnsupported::NonLeadingLabel
        }
    ));
    assert!(result.records.is_empty());
    assert_eq!(
        result.lines[0].decoded.as_ref().unwrap().line_text,
        "References [7] Ada First 2024"
    );
    assert!(!result.has_complete_labelled_line_inventory());
}

#[test]
fn foreign_or_mutated_heading_assignments_cannot_exempt_source_rows() {
    let (input, mut headings) = heading_carrier(&[
        ("Study", 20.0, 10.0, false),
        ("References", 20.0, 40.0, true),
        ("[7] Ada First 2024", 20.0, 60.0, false),
    ]);
    let (boundary, citation) = reference_models();
    headings.identity.document_id = "another-document".into();
    assert!(matches!(
        build_native_reference_candidate_with_headings(
            &input,
            &lexicon(),
            &boundary,
            &citation,
            &headings
        ),
        Err(NativeScholarlyCandidateError::HeadingEvidenceMismatch { .. })
    ));
    headings.identity.document_id = input.identity.document_id.clone();
    headings.assignments[1].word_ordinal += 100;
    assert!(matches!(
        build_native_reference_candidate_with_headings(
            &input,
            &lexicon(),
            &boundary,
            &citation,
            &headings
        ),
        Err(NativeScholarlyCandidateError::HeadingEvidenceMismatch { .. })
    ));
}

#[test]
fn missing_header_and_reference_regions_are_independent_typed_results() {
    let input = carrier(
        &[(FullSemanticRegion::Header, &["Study"])],
        &[FullSemanticRegion::Header, FullSemanticRegion::References],
    );
    let header = token_model(NATIVE_HEADER_LABELS, &[("Study", "<title>")]);
    let (boundary, citation) = reference_models();
    let result = build_native_scholarly_candidate(
        &input,
        &lexicon(),
        NativeScholarlyModels {
            header: &header,
            reference_segmenter: &boundary,
            citation: &citation,
        },
    );
    assert_eq!(result.header.unwrap().fields.title, ["Study"]);
    assert_eq!(
        result.references,
        Err(NativeScholarlyCandidateError::RegionAbsent {
            region: FullSemanticRegion::References,
            requested: true,
        })
    );
    let unrequested = carrier(
        &[(FullSemanticRegion::Header, &["Study"])],
        &[FullSemanticRegion::Header],
    );
    assert_eq!(
        build_native_reference_candidate(&unrequested, &lexicon(), &boundary, &citation),
        Err(NativeScholarlyCandidateError::RegionAbsent {
            region: FullSemanticRegion::References,
            requested: false
        })
    );
}
