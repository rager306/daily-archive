//! Contract tests for the S06/T04 typed native-route failure recovery layer.
//!
//! The layer is the one T01's module doc hands the *fine* per-band failure
//! class to. These tests pin: the closed recovery-disposition vocabulary; a
//! deterministic failure injection for absent bytes, truncated/non-PDF bytes,
//! a named band refusal on the pinned fixture, an unknown document id and an
//! exceeded ceiling; that a refusal is never `measured`; that the Java observer
//! installed on the seam stays at `observed_java_calls=0` on every failure path
//! (with a non-vacuous positive control); and that the same bytes always yield
//! the same disposition set.
//!
//! Stage ids, slice membership and the pinned corpus are resolved *by pointer*
//! from the committed DATA. The test reads no gitignored `data/sources/**`.

use da_parse::*;
use sha2::{Digest, Sha256};

const GROBID_COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
const LAYOUT_BAKEOFF: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");

/// The pinned fixture bytes the injected cases are derived from. They are a
/// literal, so the failure injection is a function of bytes the test owns.
const PDF_BYTES: &[u8] = b"%PDF-1.4\nfixture payload\n%%EOF\n";

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

// ─────────────────────────── DATA pointer resolution ───────────────────────

fn cascade_stage_ids() -> Vec<String> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(GROBID_COMPATIBILITY).unwrap();
    let stages = yaml["defaults"]["cascade_stages"]
        .as_mapping()
        .expect("defaults.cascade_stages is a mapping");
    let mut ids: Vec<String> = stages
        .keys()
        .map(|key| key.as_str().expect("stage id is a string").to_owned())
        .collect();
    ids.sort_by_key(|id| {
        id.trim_start_matches('P')
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    });
    assert!(ids.len() >= 10, "the declared cascade must hold P0..P9");
    ids
}

fn out_of_slice_reason_from_token(token: &str) -> NativeRouteOutOfSliceReason {
    match token {
        "java_authority" => NativeRouteOutOfSliceReason::JavaAuthority,
        "visual_heads_not_measured" => NativeRouteOutOfSliceReason::VisualHeadsNotMeasured,
        "java_tei_coordinates" => NativeRouteOutOfSliceReason::JavaTeiCoordinates,
        "profile_dependent_consolidation" => {
            NativeRouteOutOfSliceReason::ProfileDependentConsolidation
        }
        "java_tei_lift" => NativeRouteOutOfSliceReason::JavaTeiLift,
        other => panic!("unknown out-of-slice reason token in DATA: {other}"),
    }
}

/// The slice boundary, resolved by pointer from the declaration DATA.
fn boundary() -> NativeRouteBoundary {
    let yaml: serde_yaml::Value = serde_yaml::from_str(LAYOUT_BAKEOFF).unwrap();
    let slice = &yaml["java_free_vertical_slice"];
    let admission_stage_id = slice["identity_admission_stage"]["stage_id"]
        .as_str()
        .expect("identity_admission_stage.stage_id")
        .to_owned();
    let in_slice_stage_ids: Vec<String> = slice["stages_in_slice"]
        .as_sequence()
        .expect("stages_in_slice")
        .iter()
        .map(|stage| stage["stage_id"].as_str().unwrap().to_owned())
        .collect();
    let out_of_slice: Vec<NativeRouteOutOfSliceStage> = slice["stages_out_of_slice"]
        .as_sequence()
        .expect("stages_out_of_slice")
        .iter()
        .map(|stage| NativeRouteOutOfSliceStage {
            stage_id: stage["stage_id"].as_str().unwrap().to_owned(),
            reason: out_of_slice_reason_from_token(stage["reason"].as_str().unwrap()),
        })
        .collect();
    let lane_ids: Vec<String> = slice["lanes_in_slice"]
        .as_sequence()
        .expect("lanes_in_slice")
        .iter()
        .map(|lane| lane["lane_id"].as_str().unwrap().to_owned())
        .collect();
    NativeRouteBoundary {
        admission_stage_id,
        in_slice_stage_ids,
        out_of_slice,
        lane_ids,
    }
}

/// The corpus pin pointer, resolved from the same declaration carrier.
/// The declaration spells each pinned identity as `id`.
fn declared_document_ids() -> Vec<String> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(LAYOUT_BAKEOFF).unwrap();
    yaml["corpus"]["documents"]
        .as_sequence()
        .expect("corpus.documents")
        .iter()
        .map(|document| document["id"].as_str().unwrap().to_owned())
        .collect()
}

// ──────────────────────────── fixture plane (S03) ──────────────────────────

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

/// The fixture document id, independent of the pinned corpus.
const FIXTURE_DOC: &str = "fixture-doc";

const WORDS: [&str; 8] = [
    "Title",
    "Author",
    "introduction",
    "body",
    "references",
    "[1]",
    "Smith",
    "2020",
];

/// Build the S03 plane from `words`. `header_font` is the first
/// (header-styled) token's declared font size, so a case can inject the header
/// band's font refusal.
fn plane(
    words: &[&str],
    header_font: Option<u32>,
) -> (LayoutDocument, LayoutLattice, Vec<LexemeRow>) {
    let header_style = LayoutStyleHints {
        bold: true,
        italic: false,
        monospace: false,
        font_size_milli_pt: header_font,
    };
    let tokens: Vec<LayoutToken> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let y = 10.0 + i as f64 * 12.0;
            LayoutToken::new(
                0,
                i as u32,
                i as u32,
                LayoutBBox::new(20.0, y, 80.0, y + 10.0).unwrap(),
                digest(text),
                LayoutSpan::present(i as u32, i as u32 + 1).unwrap(),
                LayoutTokenKind::from_lexical_text(text),
                if i == 0 {
                    header_style
                } else {
                    LayoutStyleHints {
                        bold: false,
                        italic: false,
                        monospace: false,
                        font_size_milli_pt: Some(10_000),
                    }
                },
                provider(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        FIXTURE_DOC,
        "b".repeat(64),
        vec![LayoutPage::new(0, 300.0, 400.0, 0, None, tokens.clone()).unwrap()],
    )
    .unwrap();
    let count = tokens.len() as u32;
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
    let lattice = LayoutLattice::new(
        &document,
        CoordinateDialect::AltoTopLeft72Dpi,
        ReadingOrder::XmlSource,
        Some(ReadingOrder::XyCutDerived),
        vec![LatticePage {
            page_ordinal: 0,
            java_page_number: 1,
            width: 300.0,
            height: 400.0,
            rotation: 0,
            crop_box: None,
            blocks: vec![LatticeBlock {
                source_ordinal: 0,
                source_span: LayoutSpan::present(0, count).unwrap(),
                bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                source_order: 0,
                derived_order: Some(0),
                furniture: FurnitureEvidence::Absent,
                lines: vec![LatticeLine {
                    source_ordinal: 0,
                    source_span: LayoutSpan::present(0, count).unwrap(),
                    bbox: LayoutBBox::new(0.0, 0.0, 300.0, 400.0).unwrap(),
                    newline_boundary_count: 1,
                    tokens: lattice_tokens,
                }],
            }],
        }],
    )
    .unwrap();
    let lexemes: Vec<LexemeRow> = words
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                FIXTURE_DOC,
                "fixture-donor",
                0,
                i as u32,
                digest(text),
                *text,
            )
        })
        .collect();
    (document, lattice, lexemes)
}

fn lexicon() -> LexiconBundle {
    serde_json::from_str(
        r#"{
            "schema_version": "grobid-lexicon.v1",
            "resource_version": "0.9.0-crf",
            "channels": [{"key": "properName", "entries": ["smith"]}],
            "feature_templates": [{
                "key": "header",
                "channel_keys": ["properName"],
                "feature_inputs": ["token.text"]
            }]
        }"#,
    )
    .expect("the fixture lexicon parses")
}

/// The S05 body-oracle residual shape, reproduced so the body axis stays the
/// frozen `not_assessable` / `p1_localization_uncertain` the route must carry.
fn body_oracle() -> NativeRouteBodyOracle {
    let plane_units = BodyUnitInventory::from_fragments(
        WORDS.iter().copied(),
        BodyUnitRung::new(
            BodyNormalization::ZeroWidthAndLigatureNormalized,
            BodyCasePolicy::CaseFolded,
        ),
    );
    let oracle_fragments = [
        "Title",
        "Author",
        "introduction",
        "body",
        "reference",
        "[1]",
        "Smith",
        "2020",
    ];
    let oracle_units = BodyUnitInventory::from_fragments(
        oracle_fragments.iter().copied(),
        BodyUnitRung::new(
            BodyNormalization::ZeroWidthAndLigatureNormalized,
            BodyCasePolicy::CaseFolded,
        ),
    );
    NativeRouteBodyOracle {
        native_plane_units: plane_units,
        oracle_units,
    }
}

fn visual_oracle() -> NativeVisualOracle {
    let rows = [
        NativeVisualFamily::Figure,
        NativeVisualFamily::Table,
        NativeVisualFamily::Formula,
    ]
    .into_iter()
    .map(|family| NativeVisualOracleRow {
        document_id: FIXTURE_DOC.into(),
        family,
        ordinal: 0,
        page_ordinal: Some(0),
        coords: None,
        caption_digest: None,
        content_digest: format!("sha256:{}", "c".repeat(64)),
    })
    .collect();
    NativeVisualOracle {
        schema_version: NATIVE_VISUAL_ORACLE_SCHEMA_VERSION.into(),
        generator_id: "fixture-independent-visual-scanner".into(),
        backend: "fixture".into(),
        pins: vec![NativeVisualOraclePin {
            document_id: FIXTURE_DOC.into(),
            sha256: "b".repeat(64),
        }],
        rows,
    }
}

fn callout_oracle() -> NativeCalloutOracle {
    NativeCalloutOracle {
        schema_version: NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION.into(),
        generator_id: "fixture-independent-callout-scanner".into(),
        backend: "fixture".into(),
        pins: vec![NativeCalloutOraclePin {
            document_id: FIXTURE_DOC.into(),
            sha256: "b".repeat(64),
        }],
        rows: vec![NativeCalloutOracleRow {
            document_id: FIXTURE_DOC.into(),
            family: CalloutTargetClass::Citation,
            ordinal: 0,
            content_digest: format!("sha256:{}", "d".repeat(64)),
        }],
    }
}

struct Fixture {
    document: LayoutDocument,
    lattice: LayoutLattice,
    lexemes: Vec<LexemeRow>,
    lexicon: LexiconBundle,
    boundary: NativeRouteBoundary,
    cascade: Vec<String>,
    body: NativeRouteBodyOracle,
    sections: Vec<NativeSectionOracleRow>,
    visual: NativeVisualOracle,
    callout: NativeCalloutOracle,
}

impl Fixture {
    fn new(header_font: Option<u32>) -> Self {
        Self::with_words(&WORDS, header_font)
    }

    fn with_words(words: &[&str], header_font: Option<u32>) -> Self {
        let (document, lattice, lexemes) = plane(words, header_font);
        Self {
            document,
            lattice,
            lexemes,
            lexicon: lexicon(),
            boundary: boundary(),
            cascade: cascade_stage_ids(),
            body: body_oracle(),
            sections: Vec::new(),
            visual: visual_oracle(),
            callout: callout_oracle(),
        }
    }

    fn input(&self) -> NativeRouteInput<'_> {
        NativeRouteInput {
            document: &self.document,
            lattice: &self.lattice,
            lexemes: &self.lexemes,
            lexicon: &self.lexicon,
            boundary: &self.boundary,
            cascade_stage_ids: &self.cascade,
            body_oracle: &self.body,
            section_lane_id: NATIVE_MEASURED_LANE_ID,
            section_oracle_independent: true,
            section_oracle: &self.sections,
            visual_oracle: &self.visual,
            callout_oracle: &self.callout,
        }
    }

    fn build(&self) -> Result<NativeRouteResult, NativeRouteResultError> {
        build_native_route_result(&self.input())
    }

    /// The S03 region carrier the header band runs on, built through the same
    /// public plane calls the aggregate uses.
    fn carrier(&self) -> NativeRegionCarrier {
        let projection =
            project_native_words(&self.document, &self.lexemes).expect("word projection");
        // The aggregate validates the plane through the same call; the
        // carrier itself is built from the projection and the heading regions.
        let _segmentation = segment_document_full(
            &self.document,
            &self.lattice,
            &self.lexemes,
            SegmentationConfig::default(),
        )
        .expect("full segmentation");
        let heading = assign_heading_regions(
            &self.document,
            &self.lattice,
            &projection,
            HeadingRegionConfig::default(),
        )
        .expect("heading regions");
        build_native_word_region_carrier(
            &self.document,
            &self.lattice,
            &projection,
            &heading,
            &native_section_regions(),
        )
        .expect("region carrier")
    }
}

// ─────────────────────────────── helpers ───────────────────────────────────

fn spec<'a>(
    bytes: Option<&'a [u8]>,
    declared: &'a [String],
    document_id: &'a str,
) -> RouteInputSpec<'a> {
    RouteInputSpec {
        declared_document_ids: declared,
        document_id,
        input_bytes: bytes,
    }
}

fn fixture_ids() -> Vec<String> {
    vec![FIXTURE_DOC.to_owned()]
}

/// Every failure path the route can take, each as one attempt report with the
/// supplied Java observation recorded verbatim.
fn all_failure_reports(java: JavaInvocationObservation) -> Vec<RouteAttemptReport> {
    let fixture = Fixture::new(Some(14_000));
    let build = || fixture.build();
    let declared = fixture_ids();
    let mut reports = Vec::new();

    // (a) absent bytes: no carrier entry at all.
    reports.push(attempt_native_route(
        &spec(None, &declared, FIXTURE_DOC),
        java,
        build,
    ));
    // (a') present but empty bytes: still a typed absence, never a zero.
    reports.push(attempt_native_route(
        &spec(Some(b""), &declared, FIXTURE_DOC),
        java,
        build,
    ));
    // (b) truncated bytes: the `%PDF-` magic without the trailer.
    reports.push(attempt_native_route(
        &spec(Some(b"%PDF-1.4\nno trailer here"), &declared, FIXTURE_DOC),
        java,
        build,
    ));
    // (b') non-PDF bytes.
    reports.push(attempt_native_route(
        &spec(Some(b"this is not a pdf at all"), &declared, FIXTURE_DOC),
        java,
        build,
    ));
    // (d) an unknown document id.
    reports.push(attempt_native_route(
        &spec(Some(PDF_BYTES), &declared, "not-a-declared-document"),
        java,
        build,
    ));
    // (d') an empty declared list admits nothing.
    reports.push(attempt_native_route(
        &spec(Some(PDF_BYTES), &[], FIXTURE_DOC),
        java,
        build,
    ));
    // (c) the pinned fixture's own aggregate: every band refusal and unmet
    // family it records must surface as a typed failure.
    reports.push(attempt_native_route(
        &spec(Some(PDF_BYTES), &declared, FIXTURE_DOC),
        java,
        build,
    ));
    // A hard plane refusal is a route failure too, not a silent zero.
    reports.push(attempt_native_route(
        &spec(Some(PDF_BYTES), &declared, FIXTURE_DOC),
        java,
        || {
            Err(NativeRouteResultError::Plane {
                stage: NativeRoutePlaneFailure::Segmentation,
            })
        },
    ));
    reports
}

fn assert_in_vocabulary(failure: &RouteFailure) {
    assert!(
        ROUTE_RECOVERY_DISPOSITION_VOCABULARY.contains(&failure.disposition_token()),
        "disposition {} is outside the closed vocabulary",
        failure.disposition_token()
    );
    let token = failure.reason_token();
    assert!(
        ROUTE_FAILURE_REASON_PREFIX_VOCABULARY
            .iter()
            .any(|prefix| token == *prefix || token.starts_with(&format!("{prefix}:"))),
        "reason token {token} carries no closed prefix"
    );
    assert!(!failure.is_measured(), "a refusal must never be measured");
    assert_ne!(
        failure.disposition,
        RouteRecoveryDisposition::NoRecoveryRequired,
        "a failure must never claim that no recovery is required"
    );
    assert_eq!(
        failure.termination_class(),
        classify_termination(failure.failure_class, failure.failure_phase),
        "the failure must reuse the closed termination classification"
    );
}

// ─────────────────────────── vocabulary closure ────────────────────────────

#[test]
fn the_recovery_disposition_vocabulary_is_closed_total_and_unique() {
    assert_eq!(
        ROUTE_RECOVERY_DISPOSITION_VOCABULARY.as_slice(),
        RouteRecoveryDisposition::ALL
            .map(|disposition| disposition.token())
            .as_slice(),
        "the declared vocabulary must match the enum in declaration order"
    );
    let mut seen = std::collections::BTreeSet::new();
    for token in ROUTE_RECOVERY_DISPOSITION_VOCABULARY {
        assert!(!token.is_empty());
        assert!(seen.insert(token), "duplicate disposition token {token}");
    }
    assert_eq!(
        ROUTE_FAILURE_RETRY, "forbidden",
        "no retry is part of this layer"
    );
    assert!(!ROUTE_RECOVERY_DISPOSITION_VOCABULARY.contains(&"measured"));
}

#[test]
fn the_route_declares_no_retry_and_keeps_the_reason_namespace_closed() {
    // Every prefix is non-empty and unique: the reason namespace is a closed
    // dictionary rather than free text.
    let mut seen = std::collections::BTreeSet::new();
    for prefix in ROUTE_FAILURE_REASON_PREFIX_VOCABULARY {
        assert!(!prefix.is_empty());
        assert!(seen.insert(prefix), "duplicate reason prefix {prefix}");
    }
    assert_eq!(
        NATIVE_ROUTE_FAILURE_SCHEMA_VERSION,
        "da-parse.native-route-failure/v1"
    );
}

// ───────────────────────────── input injection ─────────────────────────────

#[test]
fn absent_empty_truncated_and_non_pdf_bytes_are_distinct_named_dispositions() {
    let declared = fixture_ids();

    let well_formed = classify_route_input(&spec(Some(PDF_BYTES), &declared, FIXTURE_DOC));
    assert!(well_formed.is_ok(), "a well-formed PDF is admitted");
    assert!(is_well_formed_pdf(PDF_BYTES));
    assert!(!is_well_formed_pdf(b"%PDF-1.4\nno trailer"));
    assert!(!is_well_formed_pdf(b"not a pdf"));
    assert!(!is_well_formed_pdf(b""));

    let absent = classify_route_input(&spec(None, &declared, FIXTURE_DOC)).unwrap_err();
    assert_eq!(
        absent.disposition,
        RouteRecoveryDisposition::InputBytesAbsent
    );
    assert_eq!(absent.reason_token(), "input_absent");
    assert_eq!(absent.stage_or_family, ROUTE_SCOPE_DOCUMENT);
    assert_in_vocabulary(&absent);

    let empty = classify_route_input(&spec(Some(b""), &declared, FIXTURE_DOC)).unwrap_err();
    assert_eq!(
        empty.disposition,
        RouteRecoveryDisposition::InputBytesAbsent
    );
    assert_eq!(empty.reason_token(), "input_absent");

    let truncated =
        classify_route_input(&spec(Some(b"%PDF-1.4\ntrunc"), &declared, FIXTURE_DOC)).unwrap_err();
    assert_eq!(
        truncated.disposition,
        RouteRecoveryDisposition::InputBytesMalformed
    );
    assert_eq!(truncated.reason_token(), "input_malformed");
    assert_in_vocabulary(&truncated);

    let non_pdf = classify_route_input(&spec(Some(b"nope"), &declared, FIXTURE_DOC)).unwrap_err();
    assert_eq!(
        non_pdf.disposition,
        RouteRecoveryDisposition::InputBytesMalformed
    );
    assert_eq!(non_pdf.reason_token(), "input_malformed");

    assert_ne!(
        absent.disposition, truncated.disposition,
        "a missing file is not a corrupt one"
    );
}

#[test]
fn an_unknown_document_id_is_refused_by_name_and_an_empty_list_admits_nothing() {
    let declared = fixture_ids();
    let unknown =
        classify_route_input(&spec(Some(PDF_BYTES), &declared, "some-other-document")).unwrap_err();
    assert_eq!(
        unknown.disposition,
        RouteRecoveryDisposition::UnknownDocument
    );
    assert_eq!(unknown.reason_token(), "unknown_document");
    assert_eq!(unknown.document_id, "some-other-document");
    assert_in_vocabulary(&unknown);

    // Boundary case: an empty declared list admits nothing.
    let none_declared = classify_route_input(&spec(Some(PDF_BYTES), &[], FIXTURE_DOC)).unwrap_err();
    assert_eq!(
        none_declared.disposition,
        RouteRecoveryDisposition::UnknownDocument
    );
    assert_in_vocabulary(&none_declared);

    // The pinned corpus pointer resolves and every declared id is admitted.
    let pinned = declared_document_ids();
    assert_eq!(pinned.len(), 3, "the declaration pins three documents");
    for id in &pinned {
        assert!(
            classify_route_input(&spec(Some(PDF_BYTES), &pinned, id)).is_ok(),
            "declared document {id} must be admitted"
        );
    }
}

// ───────────────────────────── band refusals ───────────────────────────────

#[test]
fn a_refused_header_band_is_named_by_its_own_closed_cause() {
    // A header token with no declared font size cannot be projected into the
    // CRF font column: the band refuses with a named cause, not a zero.
    let fixture = Fixture::new(None);
    let carrier = fixture.carrier();
    let error = build_native_header_input(&carrier, &fixture.lexicon)
        .expect_err("a header token without a font size must refuse the band");
    let failure = route_failure_from_header_band(FIXTURE_DOC, &error);
    assert_eq!(
        failure.disposition,
        RouteRecoveryDisposition::TypedRefusalRecorded
    );
    assert_eq!(failure.stage_or_family, ROUTE_SCOPE_HEADER_BAND);
    assert!(
        failure.reason_token() == "header:missing_font_size"
            || failure.reason_token() == "header:no_admitted_tokens",
        "expected a named header cause, got {}",
        failure.reason_token()
    );
    assert_in_vocabulary(&failure);

    // A zero font size is the header band's `InvalidFontSize` cause.
    let zero = Fixture::new(Some(0));
    let zero_carrier = zero.carrier();
    let zero_error = build_native_header_input(&zero_carrier, &zero.lexicon)
        .expect_err("a zero font size must refuse the band");
    let zero_failure = route_failure_from_header_band(FIXTURE_DOC, &zero_error);
    assert!(
        zero_failure.reason_token() == "header:invalid_font_size"
            || zero_failure.reason_token() == "header:no_admitted_tokens",
        "expected a named header cause, got {}",
        zero_failure.reason_token()
    );
    assert_eq!(zero_failure.failure_class, FailureCode::ConversionFailed);
    assert_in_vocabulary(&zero_failure);

    // A region that admitted nothing is `no_content`, not a conversion failure.
    let empty_region = NativeHeaderInputError::NoAdmittedTokens {
        region: FullSemanticRegion::Header,
    };
    let empty_failure = route_failure_from_header_band(FIXTURE_DOC, &empty_region);
    assert_eq!(empty_failure.failure_class, FailureCode::NoContent);
    assert_eq!(empty_failure.reason_token(), "header:no_admitted_tokens");
}

#[test]
fn a_refused_bibliography_band_is_named_by_its_own_closed_cause() {
    // Every closed bibliography cause is nameable, including the anchor scan
    // that refuses a native text walking to something other than `Empty`.
    let cases: [NativeBibliographyInputError; 6] = [
        NativeBibliographyInputError::NoAdmittedTokens {
            region: FullSemanticRegion::References,
        },
        NativeBibliographyInputError::RegionNotRequested {
            region: FullSemanticRegion::References,
        },
        NativeBibliographyInputError::NoFeatureTokens,
        NativeBibliographyInputError::TokenContainsWhitespace { page: 0, source: 3 },
        NativeBibliographyInputError::AnchorScanNotNative {
            outcome: "found",
            anchors: 2,
        },
        NativeBibliographyInputError::Serialization,
    ];
    let expected = [
        "bibliography:no_admitted_tokens",
        "bibliography:region_not_requested",
        "bibliography:no_feature_tokens",
        "bibliography:token_contains_whitespace",
        "bibliography:anchor_scan_not_native",
        "bibliography:serialization",
    ];
    for (error, token) in cases.iter().zip(expected) {
        let failure = route_failure_from_bibliography_band(FIXTURE_DOC, error);
        assert_eq!(failure.reason_token(), token);
        assert_eq!(failure.stage_or_family, ROUTE_SCOPE_BIBLIOGRAPHY_BAND);
        assert_in_vocabulary(&failure);
    }
    // An admitted-nothing region is `no_content`; the anchor refusal is not.
    let absent = route_failure_from_bibliography_band(
        FIXTURE_DOC,
        &NativeBibliographyInputError::NoAdmittedTokens {
            region: FullSemanticRegion::References,
        },
    );
    assert_eq!(absent.failure_class, FailureCode::NoContent);
    let anchor = route_failure_from_bibliography_band(
        FIXTURE_DOC,
        &NativeBibliographyInputError::AnchorScanNotNative {
            outcome: "found",
            anchors: 2,
        },
    );
    assert_eq!(anchor.failure_class, FailureCode::ConversionFailed);
}

// ──────────────────────────── aggregate mapping ────────────────────────────

#[test]
fn the_pinned_fixture_s_aggregate_refusals_become_typed_failures() {
    let fixture = Fixture::new(Some(14_000));
    let result = fixture.build().expect("the fixture aggregate builds");

    let failures = route_failures_from_result(&result);
    let refused: Vec<RouteFailure> = failures
        .iter()
        .filter(|failure| failure.reason_token().starts_with("band:"))
        .cloned()
        .collect();
    let unmet: Vec<RouteFailure> = failures
        .iter()
        .filter(|failure| failure.reason_token().starts_with("unmet:"))
        .cloned()
        .collect();

    // Every non-measurement in the aggregate surfaces, and nothing else does.
    let expected = result
        .families
        .iter()
        .filter(|cell| {
            matches!(
                cell.disposition,
                NativeRouteFamilyDisposition::Refused { .. }
                    | NativeRouteFamilyDisposition::Unmet { .. }
            )
        })
        .count()
        + result
            .stages
            .iter()
            .filter(|stage| stage.stance == NativeRouteStageStance::NotQualified)
            .count();
    assert_eq!(failures.len(), expected);

    for failure in &failures {
        assert_in_vocabulary(failure);
        // The failing family's own cell is never `measured`.
        if let Some(cell) = result
            .families
            .iter()
            .find(|cell| cell.family.token() == failure.stage_or_family)
        {
            assert_ne!(
                cell.disposition.token(),
                "measured",
                "a refused family must never carry a measured disposition"
            );
        }
    }
    // The pinned fixture's disposition set is deterministic, and on these bytes
    // every negative is a typed `unmet`: the aggregate never refused a band.
    // (The `refused` arm is pinned separately through a typed injection below.)
    assert!(
        !unmet.is_empty(),
        "the pinned fixture must record typed negatives"
    );
    assert!(
        refused.is_empty(),
        "the pinned fixture is not expected to refuse a band, got {refused:?}"
    );
    assert_eq!(
        unmet
            .iter()
            .map(RouteFailure::reason_token)
            .collect::<Vec<String>>(),
        vec![
            "unmet:header_atom",
            "unmet:header_atom",
            "unmet:header_atom",
            "unmet:header_atom",
            "unmet:bibliography_atom",
            "unmet:section",
            "unmet:section",
            "unmet:visual",
            "unmet:visual",
            "unmet:visual",
            "unmet:callout",
        ],
        "the pinned fixture's unmet set must stay frozen"
    );
    for failure in refused.iter().chain(unmet.iter()) {
        assert_eq!(
            failure.disposition,
            RouteRecoveryDisposition::TypedRefusalRecorded
        );
    }
}

/// One closed band-refusal token → its typed member. Panics on a token the
/// module vocabulary does not declare, so the DATA and the enum cannot drift.
fn band_refusal_from_token(token: &str) -> NativeRouteRefusalReason {
    match token {
        "header_band_refused" => NativeRouteRefusalReason::HeaderBandRefused,
        "bibliography_band_refused" => NativeRouteRefusalReason::BibliographyBandRefused,
        "body_text_band_refused" => NativeRouteRefusalReason::BodyTextBandRefused,
        "section_band_refused" => NativeRouteRefusalReason::SectionBandRefused,
        "visual_band_refused" => NativeRouteRefusalReason::VisualBandRefused,
        "callout_band_refused" => NativeRouteRefusalReason::CalloutBandRefused,
        "oracle_refused" => NativeRouteRefusalReason::OracleRefused,
        other => panic!("unknown band-refusal token: {other}"),
    }
}

#[test]
fn a_refused_family_cell_becomes_a_band_failure_by_name() {
    // Every closed band-refusal member is reachable through the aggregate
    // mapping, named, and never `measured`.
    for token in NATIVE_ROUTE_REFUSAL_REASON_VOCABULARY {
        let fixture = Fixture::new(Some(14_000));
        let mut result = fixture.build().expect("the fixture aggregate builds");
        let family = result.families[0].family;
        result.families[0].disposition = NativeRouteFamilyDisposition::Refused {
            reason: band_refusal_from_token(token),
        };

        let failures = route_failures_from_result(&result);
        let failure = failures
            .iter()
            .find(|failure| failure.stage_or_family == family.token())
            .unwrap_or_else(|| panic!("the refused family {token} was not recorded"));
        assert_eq!(failure.reason_token(), format!("band:{token}"));
        assert_eq!(
            failure.disposition,
            RouteRecoveryDisposition::TypedRefusalRecorded
        );
        assert_in_vocabulary(failure);
        // The refused family's cell is not a measurement.
        assert_ne!(result.families[0].disposition.token(), "measured");
    }
}

#[test]
fn an_unqualified_stage_is_a_stage_not_qualified_failure() {
    let fixture = Fixture::new(Some(14_000));
    let mut result = fixture.build().expect("the fixture aggregate builds");
    let in_slice = fixture.boundary.in_slice_stage_ids[0].clone();
    let index = result
        .stages
        .iter()
        .position(|stage| stage.stage_id == in_slice)
        .expect("the in-slice stage is present");
    result.stages[index].stance = NativeRouteStageStance::NotQualified;

    let failures = route_failures_from_result(&result);
    let failure = failures
        .iter()
        .find(|failure| failure.stage_or_family == in_slice)
        .expect("the unqualified stage is recorded");
    assert_eq!(
        failure.disposition,
        RouteRecoveryDisposition::StageNotQualified
    );
    assert_eq!(failure.reason_token(), "stage_not_qualified");
    assert_in_vocabulary(failure);
}

#[test]
fn a_refused_body_axis_is_a_typed_failure() {
    // The body axis is the one aggregate-level refusal outside the family and
    // stage axes; it must surface rather than disappear.
    let fixture = Fixture::new(Some(14_000));
    let mut result = fixture.build().expect("the fixture aggregate builds");
    result.body_axis = NativeRouteBodyAxis::Refused {
        reason: NativeRouteRefusalReason::OracleRefused,
    };

    let failures = route_failures_from_result(&result);
    let failure = failures
        .iter()
        .find(|failure| failure.stage_or_family == ROUTE_SCOPE_BODY_AXIS)
        .expect("the body-axis refusal must be recorded");
    assert_eq!(failure.reason_token(), "body_axis:oracle_refused");
    assert_eq!(
        failure.disposition,
        RouteRecoveryDisposition::TypedRefusalRecorded
    );
    assert_in_vocabulary(failure);
}

#[test]
fn aggregate_errors_are_mapped_by_name() {
    let plane = route_failure_from_aggregate_error(
        FIXTURE_DOC,
        &NativeRouteResultError::Plane {
            stage: NativeRoutePlaneFailure::RegionCarrier,
        },
    );
    assert_eq!(plane.reason_token(), "plane:region_carrier");
    assert_eq!(plane.stage_or_family, "region_carrier");
    assert_eq!(
        plane.disposition,
        RouteRecoveryDisposition::TypedRefusalRecorded
    );
    assert_in_vocabulary(&plane);

    let boundary = route_failure_from_aggregate_error(
        FIXTURE_DOC,
        &NativeRouteResultError::Boundary(NativeRouteBoundaryError::EmptyCascade),
    );
    assert_eq!(boundary.reason_token(), "boundary_refused");
    assert_eq!(boundary.stage_or_family, ROUTE_SCOPE_CASCADE);
    assert_in_vocabulary(&boundary);

    let invalid = route_failure_from_aggregate_error(
        FIXTURE_DOC,
        &NativeRouteResultError::Invalid(NativeRouteValidationError::FamilyDuplicated {
            family: "figure",
        }),
    );
    assert_eq!(invalid.reason_token(), "invalid_aggregate");
    assert_eq!(invalid.stage_or_family, "figure");
    assert_in_vocabulary(&invalid);
}

#[test]
fn an_exceeded_ceiling_is_a_typed_failure() {
    let outbound = route_failure_from_ceiling(FIXTURE_DOC, "route_bytes", LimitKind::RequestBytes);
    assert_eq!(
        outbound.disposition,
        RouteRecoveryDisposition::CeilingExceeded
    );
    assert_eq!(outbound.reason_token(), "ceiling_exceeded");
    assert_eq!(outbound.failure_class, FailureCode::LimitExceeded);
    assert_eq!(outbound.failure_phase, FailurePhase::Send);
    assert_in_vocabulary(&outbound);

    let inbound = route_failure_from_ceiling(FIXTURE_DOC, "route_pages", LimitKind::ResponseBytes);
    assert_eq!(inbound.failure_phase, FailurePhase::ReadBody);
    assert_in_vocabulary(&inbound);
}

// ─────────────────────── attempts, determinism, Java ───────────────────────

#[test]
fn every_failure_path_is_unmeasured_consistent_and_deterministic() {
    let first = all_failure_reports(JavaInvocationObservation::NotObserved);
    let second = all_failure_reports(JavaInvocationObservation::NotObserved);
    assert_eq!(
        first, second,
        "the same bytes must always yield the same disposition set"
    );

    for report in &first {
        assert!(
            report.is_consistent(),
            "measured must agree with the recorded failures"
        );
        assert!(
            !report.is_measured(),
            "a failure path must never report a measurement"
        );
        assert!(!report.failures.is_empty());
        assert_ne!(
            report.recovery_disposition(),
            RouteRecoveryDisposition::NoRecoveryRequired
        );
        for failure in &report.failures {
            assert_in_vocabulary(failure);
            assert_eq!(failure.document_id, report.document_id);
        }
    }

    // The specific named dispositions of the input cases.
    let tokens: Vec<Vec<String>> = first
        .iter()
        .map(|report| {
            report
                .failures
                .iter()
                .map(RouteFailure::reason_token)
                .collect()
        })
        .collect();
    assert_eq!(tokens[0], vec!["input_absent"]);
    assert_eq!(tokens[1], vec!["input_absent"]);
    assert_eq!(tokens[2], vec!["input_malformed"]);
    assert_eq!(tokens[3], vec!["input_malformed"]);
    assert_eq!(tokens[4], vec!["unknown_document"]);
    assert_eq!(tokens[5], vec!["unknown_document"]);
    assert_eq!(tokens[7], vec!["plane:segmentation"]);
}

#[test]
fn a_single_token_plane_is_total_and_deterministic() {
    // Boundary case: the smallest non-empty plane. Whatever it yields, the
    // mapping stays total and every disposition stays in the closed vocabulary.
    let fixture = Fixture::with_words(&["Title"], Some(14_000));
    let declared = fixture_ids();
    let run = || {
        attempt_native_route(
            &spec(Some(PDF_BYTES), &declared, FIXTURE_DOC),
            JavaInvocationObservation::NotObserved,
            || fixture.build(),
        )
    };
    let first = run();
    let second = run();
    assert_eq!(first, second, "a deterministic mapping over one token");
    assert!(first.is_consistent());
    for failure in &first.failures {
        assert_in_vocabulary(failure);
    }
    // The route either measured the one-token plane or recorded a typed
    // refusal; there is no third, silent outcome.
    assert!(first.is_measured() || !first.failures.is_empty());
    if first.is_measured() {
        assert!(first.failures.is_empty());
        assert_eq!(
            first.recovery_disposition(),
            RouteRecoveryDisposition::NoRecoveryRequired
        );
    }
}

#[test]
fn a_measured_attempt_needs_no_recovery() {
    // With no recorded failure the attempt is measured and its recovery
    // disposition is the closed `no_recovery_required` member.
    let report = RouteAttemptReport {
        document_id: FIXTURE_DOC.to_owned(),
        failures: Vec::new(),
        measured: true,
        java: RouteJavaObservation::Observed { attempts: 0 },
    };
    assert_eq!(
        report.recovery_disposition(),
        RouteRecoveryDisposition::NoRecoveryRequired
    );
    assert!(report.is_measured());
    assert!(report.is_consistent());
}

#[tokio::test]
async fn the_java_observer_stays_at_zero_on_every_failure_path() {
    // Install the observer FIRST, on the seam the route could reach, and prove
    // installing it does not rotate the cache identity.
    let counter = JavaInvocationCounter::new();
    let watched = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural())
        .with_invocation_observer(counter.clone());
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural());
    assert_eq!(
        plain.identity(),
        watched.identity(),
        "the observer is transport tuning, never identity"
    );
    assert_eq!(
        watched.java_invocation_observation(),
        JavaInvocationObservation::Observed { attempts: 0 },
        "an installed observer reads a measured zero"
    );

    let reports = all_failure_reports(watched.java_invocation_observation());
    for report in &reports {
        assert_eq!(
            report.java,
            RouteJavaObservation::Observed { attempts: 0 },
            "the refusal path must record observed_java_calls=0"
        );
        assert!(
            report.java.installed(),
            "the zero must be measured, not absent"
        );
        assert_eq!(report.java.attempts(), Some(0));
        assert!(report.java.is_observed_zero());
    }
    assert_eq!(
        watched.java_invocation_observation(),
        JavaInvocationObservation::Observed { attempts: 0 },
        "no failure path may consult Java"
    );

    // A typed absence is never a zero.
    let declared = fixture_ids();
    let unobserved = attempt_native_route(
        &spec(None, &declared, FIXTURE_DOC),
        JavaInvocationObservation::NotObserved,
        || unreachable!("the input is refused before any build"),
    );
    assert_eq!(unobserved.java, RouteJavaObservation::NotObserved);
    assert!(!unobserved.java.installed());
    assert_eq!(unobserved.java.attempts(), None);
    assert!(!unobserved.java.is_observed_zero());

    // Non-vacuous positive control: the observed zero can rise.
    let _ = watched
        .fetch_tei(ParseBackendRequest::new("m514-t04-doc", PDF_BYTES.to_vec()))
        .await;
    assert_eq!(
        watched.java_invocation_observation(),
        JavaInvocationObservation::Observed { attempts: 1 },
        "the guard is non-vacuous"
    );
}
