//! Contract tests for the S06/T01 typed native route result aggregate.
//!
//! The aggregate is the missing carrier that folds the S03 layout plane with
//! the S04 header/bibliography bands and the S05 body/section/visual/callout
//! bands into one typed document result. These tests pin the total/disjoint
//! stage matrix over the declared cascade, exactly one closed disposition per
//! declared family, the reproduced S05 body residual, serde round-trip,
//! determinism, the onion (imports + no foreign envelope/transport type), and
//! one named negative mutation per closed-vocabulary member.
//!
//! The stage ids and the slice membership are resolved by pointer from the
//! committed DATA (`data/parse/grobid_compatibility.yaml#defaults.cascade_stages`
//! and `data/parse/layout_bakeoff.yaml#java_free_vertical_slice`) — this test
//! never restates them. It reads no gitignored `data/sources/**`.

use da_parse::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

const GROBID_COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
const LAYOUT_BAKEOFF: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const S05_RECEIPT: &str =
    include_str!("../../../artifacts/parse_compat/native_content_receipt.json");
const ROUTE_RESULT_SURFACE: &str = include_str!("../src/native_route_result.rs");

/// The declared tokenization rung (normalization ladder + explicit case
/// policy), matching the S05 body band.
const RUNG: BodyUnitRung = BodyUnitRung::new(
    BodyNormalization::ZeroWidthAndLigatureNormalized,
    BodyCasePolicy::CaseFolded,
);

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

// ─────────────────────────── DATA pointer resolution ───────────────────────

/// Cascade stage ids, resolved by pointer and ordered by their numeric suffix
/// so the matrix is deterministic without restating any id.
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
    assert!(
        ids.len() >= 10,
        "the declared cascade must hold at least the P0..P9 stages, found {ids:?}"
    );
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

// ──────────────────────────── fixture plane (S03) ──────────────────────────

fn provider() -> LayoutProvider {
    LayoutProvider::new("fixture-donor", "1", "a".repeat(64)).unwrap()
}

fn header_style() -> LayoutStyleHints {
    LayoutStyleHints {
        bold: true,
        italic: false,
        monospace: false,
        font_size_milli_pt: Some(14_000),
    }
}

/// Header tokens, a body marker, and a references block — the same shape the
/// S04 carrier contract uses, so the S03 plane here is a proven join.
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

fn plane() -> (LayoutDocument, LayoutLattice, Vec<LexemeRow>) {
    let tokens: Vec<LayoutToken> = WORDS
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
                    header_style()
                } else {
                    LayoutStyleHints::default()
                },
                provider(),
            )
            .unwrap()
        })
        .collect();
    let document = LayoutDocument::new(
        "fixture-doc",
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
    let lexemes: Vec<LexemeRow> = WORDS
        .iter()
        .enumerate()
        .map(|(i, text)| {
            LexemeRow::new(
                "fixture-doc",
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

/// The lexical bundle the header/bibliography bands consume. Built from a
/// literal JSON shape so this test depends only on the re-exported type.
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

/// A recorded body oracle that reproduces the S05 residual shape: one oracle
/// unit is only a proper substring of a plane token, so the plane presence is
/// proven while localization is not, and nothing is absent.
fn body_oracle() -> NativeRouteBodyOracle {
    let plane_units = BodyUnitInventory::from_fragments(WORDS.iter().copied(), RUNG);
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
    let oracle_units = BodyUnitInventory::from_fragments(oracle_fragments.iter().copied(), RUNG);
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
        document_id: "fixture-doc".into(),
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
            document_id: "fixture-doc".into(),
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
            document_id: "fixture-doc".into(),
            sha256: "b".repeat(64),
        }],
        rows: vec![NativeCalloutOracleRow {
            document_id: "fixture-doc".into(),
            family: CalloutTargetClass::Citation,
            ordinal: 0,
            content_digest: format!("sha256:{}", "d".repeat(64)),
        }],
    }
}

/// Everything the aggregate needs, owned so the borrow lives long enough.
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
    fn new() -> Self {
        let (document, lattice, lexemes) = plane();
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

    fn build(&self) -> NativeRouteResult {
        let input = NativeRouteInput {
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
        };
        build_native_route_result(&input).expect("the fixture route result builds")
    }
}

// ─────────────────────────────── stage matrix ──────────────────────────────

#[test]
fn the_stage_matrix_is_total_and_disjoint_over_the_declared_cascade() {
    let fixture = Fixture::new();
    let result = fixture.build();
    fixture
        .boundary
        .validate(&fixture.cascade)
        .expect("the declared boundary classifies the whole cascade");

    let expected = fixture.boundary.stances(&fixture.cascade).unwrap();
    assert_eq!(result.stages, expected, "one stance per cascade stage");

    // Total: every cascade stage appears exactly once. Disjoint: no repeats and
    // no stage outside the cascade.
    let mut seen = std::collections::BTreeSet::new();
    for stage in &result.stages {
        assert!(
            seen.insert(stage.stage_id.clone()),
            "stage {} appears twice",
            stage.stage_id
        );
    }
    for id in &fixture.cascade {
        assert!(
            seen.contains(id),
            "cascade stage {id} was left unclassified"
        );
    }
    assert_eq!(seen.len(), fixture.cascade.len());

    // The declared in-slice stages are measured; the out-of-slice ones carry a
    // typed reason; the admission stage is the boundary.
    let in_slice: std::collections::BTreeSet<&str> = fixture
        .boundary
        .in_slice_stage_ids
        .iter()
        .map(String::as_str)
        .collect();
    for stage in &result.stages {
        if in_slice.contains(stage.stage_id.as_str()) {
            assert_eq!(stage.stance, NativeRouteStageStance::NativeMeasured);
        }
        if stage.stage_id == fixture.boundary.admission_stage_id {
            assert_eq!(stage.stance, NativeRouteStageStance::AdmissionBoundary);
        }
        if let NativeRouteStageStance::OutOfScope { reason } = stage.stance {
            assert!(
                NATIVE_ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.contains(&reason.token()),
                "{} carried an out-of-vocabulary reason",
                stage.stage_id
            );
        }
    }

    // The declared reason vocabulary in the module matches the DATA vocabulary
    // token-for-token, so neither can drift silently.
    let yaml: serde_yaml::Value = serde_yaml::from_str(LAYOUT_BAKEOFF).unwrap();
    let declared: Vec<String> = yaml["java_free_vertical_slice"]["out_of_slice_reason_vocabulary"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(declared, NATIVE_ROUTE_OUT_OF_SLICE_REASON_VOCABULARY);
}

#[test]
fn every_family_carries_exactly_one_in_vocabulary_disposition() {
    let fixture = Fixture::new();
    let result = fixture.build();

    let families: Vec<NativeRouteFamily> = result.families.iter().map(|cell| cell.family).collect();
    assert_eq!(
        families, NATIVE_ROUTE_FAMILY_ORDER,
        "one disposition per declared family, in declared order"
    );

    for cell in &result.families {
        let token = cell.disposition.token();
        assert!(
            NATIVE_ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&token),
            "family {} carried an unlabeled disposition token {token}",
            cell.family.token()
        );
        assert_ne!(
            token,
            "out_of_scope",
            "a required family {} was marked out of scope",
            cell.family.token()
        );
    }
    // The vocabulary deliberately excludes `out_of_scope` for a required family.
    assert!(!NATIVE_ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&"out_of_scope"));

    // No family token is unlabeled either.
    for family in NATIVE_ROUTE_FAMILY_ORDER {
        assert!(
            result.families.iter().any(|cell| cell.family == family),
            "family {} is unlabeled",
            family.token()
        );
        assert!(!family.token().is_empty());
    }
}

// ────────────────────────────── body residual ──────────────────────────────

#[test]
fn body_axis_reproduces_the_recorded_s05_residual() {
    // The aggregate's body axis on the fixture inventories is the S05 residual
    // shape: presence proven, localization not, nothing absent.
    let fixture = Fixture::new();
    let result = fixture.build();
    assert!(
        result.body_axis.is_frozen_s05_residual(),
        "expected the frozen S05 residual, got {:?}",
        result.body_axis
    );
    match result.body_axis {
        NativeRouteBodyAxis::Measured {
            verdict,
            reason,
            plane_absent_units,
            d498_reopen_required,
            ..
        } => {
            assert_eq!(verdict.token(), "not_assessable");
            assert_eq!(reason.token(), "p1_localization_uncertain");
            assert_eq!(plane_absent_units, 0);
            assert!(!d498_reopen_required);
        }
        other => panic!("expected a measured body axis, got {other:?}"),
    }

    // Replay the recorded S05 bytes: feeding the recorded counts back through
    // the same classifier reproduces the recorded verdict tokens byte-for-byte.
    let recorded: Value = serde_json::from_str(S05_RECEIPT).expect("the S05 receipt parses");
    let matrix = recorded["populations"][0]["matrix"]
        .as_array()
        .expect("the S05 receipt carries a matrix");
    assert_eq!(matrix.len(), 3, "the frozen slice holds three documents");
    for row in matrix {
        let body = &row["body"];
        let localization = BodyPlaneLocalization {
            method: BODY_ALIGNMENT_METHOD.to_owned(),
            oracle_units: body["oracle_units"].as_u64().unwrap() as usize,
            exact_in_plane: 0,
            boundary_variant_from_plane: 0,
            uncertain_present_in_token_from_plane: body["plane_present_in_token_units"]
                .as_u64()
                .unwrap() as usize,
            uncertain_spanning_tokens_from_plane: body["plane_spanning_token_units"]
                .as_u64()
                .unwrap() as usize,
            uncertain_character_material_from_plane: body["plane_character_material_units"]
                .as_u64()
                .unwrap() as usize,
            absent_from_plane: body["plane_absent_units"].as_u64().unwrap() as usize,
        };
        let verdict = classify_body_plane(&localization);
        let route_verdict: NativeRouteBodyVerdict = verdict.verdict.into();
        let route_reason = NativeRouteBodyGapReason::from(&verdict.reason);
        assert_eq!(
            route_verdict.token(),
            body["plane_preservation"].as_str().unwrap(),
            "recorded plane verdict differs"
        );
        assert_eq!(
            route_reason.token(),
            body["body_reason"].as_str().unwrap(),
            "recorded body reason differs"
        );
        assert_eq!(
            localization.absent_from_plane,
            body["plane_absent_units"].as_u64().unwrap() as usize
        );
    }
}

// ─────────────────────────── serde and determinism ─────────────────────────

#[test]
fn result_round_trips_and_is_deterministic() {
    let fixture = Fixture::new();
    let first = fixture.build();
    let second = fixture.build();

    assert_eq!(first.digest, second.digest, "digest must be deterministic");
    assert_eq!(first, second, "the aggregate must be deterministic");

    let first_bytes = serde_json::to_vec(&first).unwrap();
    let second_bytes = serde_json::to_vec(&second).unwrap();
    assert_eq!(first_bytes, second_bytes, "canonical bytes must be stable");

    let reparsed: NativeRouteResult = serde_json::from_slice(&first_bytes).unwrap();
    assert_eq!(reparsed, first, "serde round-trip must preserve the result");
    assert_eq!(reparsed.digest, first.compute_digest());
}

// ──────────────────────────────── onion ────────────────────────────────────

/// Import roots (`use <root>::…`) of a comment-stripped text.
fn import_roots_of(text: &str) -> std::collections::BTreeSet<String> {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix("use ")?;
            rest.split([':', ' ', '{', ';'])
                .find(|token| !token.is_empty())
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn the_module_imports_only_crate_serde_sha2_and_std() {
    assert!(
        ROUTE_RESULT_SURFACE.len() > 10_000,
        "the route result source looks truncated"
    );
    assert_eq!(
        import_roots_of(ROUTE_RESULT_SURFACE),
        std::collections::BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string(),
        ]),
        "the aggregate may import only typed leaves, hashing, serde and formatting"
    );

    // It names no foreign envelope and no transport type, and reaches no side
    // channel: no path, filesystem, environment, process, clock or network.
    let forbidden = [
        "TeiEnvelope",
        "TeiParseIr",
        "TeiIdentity",
        "TeiResultIdentity",
        "TeiLifter",
        "GrobidParser",
        "GrobidHttpBackend",
        "GrobidProfile",
        "JavaInvocationCounter",
        "JavaInvocationObservation",
        "reqwest",
        "std::path",
        "std::fs",
        "std::env",
        "std::process",
        "std::time",
        "PathBuf",
        "SystemTime",
    ];
    let code: String = ROUTE_RESULT_SURFACE
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    for marker in forbidden {
        assert!(
            !code.contains(marker),
            "the route result acquired a forbidden dependency marker: {marker}"
        );
    }
}

// ───────────────────────── negative mutation matrix ────────────────────────

fn mutate(result: &NativeRouteResult, edit: impl FnOnce(&mut Value)) -> Value {
    let mut value = serde_json::to_value(result).unwrap();
    edit(&mut value);
    value
}

fn assert_rejected(value: &Value, needle: &str) {
    let error = match serde_json::from_value::<NativeRouteResult>(value.clone()) {
        Ok(parsed) => {
            let fixture = Fixture::new();
            match parsed.validate(&fixture.boundary, &fixture.cascade) {
                Ok(()) => panic!("the mutation was accepted"),
                Err(error) => format!("{error:?}"),
            }
        }
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(needle),
        "expected the rejection to name {needle}, got: {error}"
    );
}

#[test]
fn stage_stance_mutations_are_refused_by_name() {
    let fixture = Fixture::new();
    let result = fixture.build();
    let admission = fixture.boundary.admission_stage_id.clone();
    let out_of_slice = fixture.boundary.out_of_slice[0].stage_id.clone();
    let in_slice = fixture.boundary.in_slice_stage_ids[0].clone();

    let stage_index = |stage_id: &str| {
        result
            .stages
            .iter()
            .position(|stage| stage.stage_id == stage_id)
            .expect("stage is present")
    };

    // One mutation per closed stance token, each misused in a real slot.
    // `native_measured` may not be claimed for an out-of-slice stage.
    let value = mutate(&result, |value| {
        value["stages"][stage_index(&out_of_slice)]["stance"] =
            Value::String("native_measured".into());
    });
    assert_rejected(&value, &out_of_slice);

    // `not_qualified` may not be claimed for a declared in-slice stage.
    let value = mutate(&result, |value| {
        value["stages"][stage_index(&in_slice)]["stance"] = Value::String("not_qualified".into());
    });
    assert_rejected(&value, &in_slice);

    // `out_of_scope` may not be claimed for a declared in-slice stage.
    let value = mutate(&result, |value| {
        value["stages"][stage_index(&in_slice)]["stance"] = serde_json::json!({
            "out_of_scope": {"reason": "java_authority"}
        });
    });
    assert_rejected(&value, &in_slice);

    // `admission_boundary` may not be claimed for another stage.
    let value = mutate(&result, |value| {
        value["stages"][stage_index(&in_slice)]["stance"] =
            Value::String("admission_boundary".into());
    });
    assert_rejected(&value, &in_slice);

    // The admission stage itself cannot be re-labelled.
    let value = mutate(&result, |value| {
        value["stages"][stage_index(&admission)]["stance"] =
            Value::String("native_measured".into());
    });
    assert_rejected(&value, &admission);

    // Missing, duplicated, outside-cascade and unknown-token stages.
    let value = mutate(&result, |value| {
        value["stages"].as_array_mut().unwrap().pop();
    });
    assert_rejected(&value, &fixture.cascade[fixture.cascade.len() - 1]);

    let value = mutate(&result, |value| {
        let stages = value["stages"].as_array_mut().unwrap();
        let first = stages[0].clone();
        stages.push(first);
    });
    assert_rejected(&value, &admission);

    let value = mutate(&result, |value| {
        value["stages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "stage_id": "P99",
                "stance": "native_measured"
            }));
    });
    assert_rejected(&value, "P99");

    let value = mutate(&result, |value| {
        value["stages"][0]["stance"] = Value::String("bogus_stance".into());
    });
    assert_rejected(&value, "bogus_stance");
}

#[test]
fn family_mutations_are_refused_by_name() {
    let fixture = Fixture::new();
    let result = fixture.build();

    // A family cannot be out of scope: the token is not in the vocabulary and
    // serde refuses it by name.
    let value = mutate(&result, |value| {
        value["families"][0]["disposition"] = Value::String("out_of_scope".into());
    });
    assert_rejected(&value, "out_of_scope");

    // An unknown disposition token is refused by name.
    let value = mutate(&result, |value| {
        value["families"][0]["disposition"] = Value::String("bogus_disposition".into());
    });
    assert_rejected(&value, "bogus_disposition");

    // A missing family is refused by its own token.
    let value = mutate(&result, |value| {
        value["families"].as_array_mut().unwrap().remove(0);
    });
    assert_rejected(&value, NATIVE_ROUTE_FAMILY_ORDER[0].token());

    // A duplicated family is refused by its own token.
    let value = mutate(&result, |value| {
        let families = value["families"].as_array_mut().unwrap();
        let first = families[0].clone();
        families.push(first);
    });
    assert_rejected(&value, NATIVE_ROUTE_FAMILY_ORDER[0].token());

    // An unknown family name is refused by name.
    let value = mutate(&result, |value| {
        value["families"][0]["family"] = Value::String("bogus_family".into());
    });
    assert_rejected(&value, "bogus_family");
}

#[test]
fn refusal_and_reason_tokens_are_refused_by_name() {
    let fixture = Fixture::new();
    let result = fixture.build();

    // An unknown refusal reason token is refused by name.
    let value = mutate(&result, |value| {
        value["families"][0]["disposition"] = serde_json::json!({
            "refused": {"reason": "bogus_refusal"}
        });
    });
    assert_rejected(&value, "bogus_refusal");

    // An unknown unmet reason variant is refused by name.
    let value = mutate(&result, |value| {
        value["families"][0]["disposition"] = serde_json::json!({
            "unmet": {"reason": {"bogus_reason": {"disposition": "empty"}}}
        });
    });
    assert_rejected(&value, "bogus_reason");

    // An out-of-slice reason outside the closed vocabulary is refused by name.
    let value = mutate(&result, |value| {
        let stages = value["stages"].as_array_mut().unwrap();
        for stage in stages {
            if stage["stance"].get("out_of_scope").is_some() {
                stage["stance"] = serde_json::json!({
                    "out_of_scope": {"reason": "bogus_slice_reason"}
                });
                return;
            }
        }
        panic!("the fixture carries an out-of-slice stage");
    });
    assert_rejected(&value, "bogus_slice_reason");

    // An unknown top-level field is refused by name.
    let value = mutate(&result, |value| {
        value["unexpected_field"] = Value::Bool(true);
    });
    assert_rejected(&value, "unexpected_field");

    // Every refusal-reason member in the module vocabulary is a real mapping.
    for token in NATIVE_ROUTE_REFUSAL_REASON_VOCABULARY {
        assert!(!token.is_empty());
    }
    assert_eq!(NATIVE_ROUTE_REFUSAL_REASON_VOCABULARY.len(), 7);
}

#[test]
fn boundary_mutations_are_refused_by_name() {
    let fixture = Fixture::new();
    let cascade = fixture.cascade.clone();
    let last_out_of_slice = fixture
        .boundary
        .out_of_slice
        .last()
        .unwrap()
        .stage_id
        .clone();

    // A cascade stage the boundary does not classify is refused by name.
    let mut boundary = fixture.boundary.clone();
    boundary.out_of_slice.pop();
    let error = boundary.validate(&cascade).unwrap_err();
    assert!(
        format!("{error:?}").contains(&last_out_of_slice),
        "expected the unclassified stage to be named, got {error:?}"
    );

    // A duplicated cascade id is refused by name.
    let mut duplicated = cascade.clone();
    duplicated.push(cascade[0].clone());
    let error = fixture.boundary.validate(&duplicated).unwrap_err();
    assert!(format!("{error:?}").contains(&cascade[0]));

    // A stage classified twice is refused by name.
    let mut boundary = fixture.boundary.clone();
    let first = boundary.in_slice_stage_ids[0].clone();
    boundary.out_of_slice.push(NativeRouteOutOfSliceStage {
        stage_id: first.clone(),
        reason: NativeRouteOutOfSliceReason::JavaAuthority,
    });
    let error = boundary.validate(&cascade).unwrap_err();
    assert!(format!("{error:?}").contains(&first));

    // An empty cascade, an empty in-slice set, empty lanes, an admission stage
    // outside the cascade, and a classified stage outside the cascade are all
    // refused.
    assert_eq!(
        fixture.boundary.validate(&[]).unwrap_err(),
        NativeRouteBoundaryError::EmptyCascade
    );
    let mut boundary = fixture.boundary.clone();
    boundary.in_slice_stage_ids.clear();
    assert_eq!(
        boundary.validate(&cascade).unwrap_err(),
        NativeRouteBoundaryError::EmptyInSlice
    );
    let mut boundary = fixture.boundary.clone();
    boundary.lane_ids.clear();
    assert_eq!(
        boundary.validate(&cascade).unwrap_err(),
        NativeRouteBoundaryError::EmptyLanes
    );
    let mut boundary = fixture.boundary.clone();
    boundary.admission_stage_id = "P99".into();
    assert_eq!(
        boundary.validate(&cascade).unwrap_err(),
        NativeRouteBoundaryError::AdmissionStageNotInCascade {
            stage_id: "P99".into()
        }
    );
    let mut boundary = fixture.boundary.clone();
    boundary.out_of_slice.push(NativeRouteOutOfSliceStage {
        stage_id: "P98".into(),
        reason: NativeRouteOutOfSliceReason::JavaTeiLift,
    });
    assert_eq!(
        boundary.validate(&cascade).unwrap_err(),
        NativeRouteBoundaryError::UnknownClassifiedStage {
            stage_id: "P98".into()
        }
    );

    // A mutated result cannot be validated against a boundary that refuses.
    let result = fixture.build();
    let mut boundary = fixture.boundary.clone();
    boundary.out_of_slice.pop();
    let error = result.validate(&boundary, &cascade).unwrap_err();
    assert!(matches!(
        error,
        NativeRouteValidationError::BoundaryInvalid(_)
    ));
}

#[test]
fn an_untyped_body_gap_is_refused_by_name() {
    let fixture = Fixture::new();
    let result = fixture.build();
    let value = mutate(&result, |value| {
        value["body_axis"] = serde_json::json!({
            "outcome": "measured",
            "verdict": "content_loss",
            "reason": {"p1_native_text_absent": {"missing_units": 3}},
            "d498_reopen_required": false,
            "plane_absent_units": 3,
            "plane_present_in_token_units": 0,
            "plane_spanning_token_units": 0,
            "plane_character_material_units": 0,
            "oracle_units": 4,
            "native_plane_units": 1
        });
    });
    assert_rejected(&value, "content_loss");
}
