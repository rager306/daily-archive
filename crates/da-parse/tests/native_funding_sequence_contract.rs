//! S06/T06 contract for the native GROBID `funding-acknowledgement` sequence
//! runner and its entity lift.
//!
//! Three independent gates live here:
//!
//! 1. the **DATA binding** — `families.funding` in
//!    `data/parse/native_visual_families.yaml` must agree 1:1 with the Rust
//!    dictionary, the base-role table, the declared-but-unmodelled Java labels and
//!    the printed ABI (feature width, label slot, referenced window), so a
//!    transcription edit on either side fails here — and the binding is shown to
//!    be *sensitive* by mutating a copy of it;
//! 2. the **fail-closed behaviour** of the runner — wrong arity, a training-shape
//!    label slot, a foreign label block, a column window that is not `0..=16`, the
//!    `TaggingTokenClusteror` `I-` boundary, the row cap, and the typed refusal of
//!    the four declared-but-unmodelled Java labels;
//! 3. the **lift semantics** — every role to its own field, the Java split rule on
//!    a repeated scalar role, `<infrastructure>` marking its institution,
//!    `<other>` counted rather than dropped, and absence never rendered as "zero
//!    funders" (H12-N1).
//!
//! The decoded model is synthetic and hand-authored in this file: the real GROBID
//! bytes are not redistributable and are never bundled (D358). Binding a real
//! model is the opt-in harness's job (S06/T07). The lexical guard
//! `the_module_carries_no_model_material_or_io` pins that the SUT has no
//! filesystem, environment, logging or model-byte surface at all, and keeps the
//! shared grouping helper and the D359 exclusion visible.

use da_parse::{
    FundingFeatureRow, FundingFeatureToken, FundingLineStatus, MAX_NATIVE_FUNDING_ROWS,
    NATIVE_FUNDING_BASE_LABELS, NATIVE_FUNDING_FEATURE_COLUMNS, NATIVE_FUNDING_LABEL_COLUMN_DUMMY,
    NATIVE_FUNDING_LABEL_COLUMN_INDEX, NATIVE_FUNDING_LABELS,
    NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_ROW_COLUMNS,
    NATIVE_FUNDING_SEQUENCE_SCHEMA_VERSION, NATIVE_FUNDING_UNMODELED_JAVA_LABELS,
    NativeFundingAbsence, NativeFundingDecodeOutcome, NativeFundingEntry,
    NativeFundingGazetteerCoverage, NativeFundingRole, NativeFundingSequenceError,
    check_funding_column_window, check_funding_label_block, decode_funding_sequence,
    funding_entries_from_decoded, funding_label_is_admissible, funding_role_of_label,
    render_funding_features,
};
use da_ports::scalar_crf::{CrfModel, CrfPattern};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DATA: &str = "data/parse/native_visual_families.yaml";
const SOURCE: &str = include_str!("../src/native_funding_sequence.rs");

const FUNDING_MODEL_ID: &str = "funding-acknowledgement";
const FUNDING_MODEL_FILE: &str = "funding-acknowledgement/model.wapiti";
const FUNDING_CHECKPOINT: &str = "#mdl#2#15784";
const FUNDING_RDR: &str = "#rdr#100/16/0";
const FUNDING_GAZETTEER_LANES: [&str; 2] = ["knownFunder", "knownInfrastructure"];

const FUNDING: [&str; 2] = ["families", "funding"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn data() -> serde_yaml::Value {
    let path = root().join(DATA);
    serde_yaml::from_str(&std::fs::read_to_string(&path).expect(DATA)).expect(DATA)
}

fn get<'a>(value: &'a serde_yaml::Value, path: &[&str]) -> &'a serde_yaml::Value {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .unwrap_or_else(|| panic!("DATA key {path:?} is missing"));
    }
    current
}

/// The sequence at `path`, as owned strings.
fn strings(value: &serde_yaml::Value, path: &[&str]) -> Vec<String> {
    get(value, path)
        .as_sequence()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a sequence"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("DATA key {path:?} carries a non-string"))
                .to_string()
        })
        .collect()
}

/// The 1:1 binding predicate, factored out so a mutated copy can be shown to
/// fail it rather than merely asserted to be true.
///
/// Ordered, because `declared_labels` is transcribed verbatim in model order: the
/// label ids index the Wapiti weight blocks, so a reordering is a transcription
/// defect and not a harmless permutation.
fn binding_verdict(declared: &[String], expected: &[&str]) -> Result<(), String> {
    if declared.len() != expected.len() {
        return Err(format!(
            "declared {} entries; the Rust mirror has {}",
            declared.len(),
            expected.len()
        ));
    }
    for (index, (declared, expected)) in declared.iter().zip(expected.iter()).enumerate() {
        if declared != expected {
            return Err(format!(
                "entry {index} is `{declared}`; the Rust mirror has `{expected}`"
            ));
        }
    }
    Ok(())
}

/// One 18-column row whose first column carries the token text and whose label
/// slot carries the inference literal; the middle columns are inert.
fn row(value: &str) -> FundingFeatureRow {
    let columns: Vec<String> = std::iter::once(value.to_string())
        .chain((1..NATIVE_FUNDING_LABEL_COLUMN_INDEX).map(|_| "0".to_string()))
        .chain(std::iter::once(
            NATIVE_FUNDING_LABEL_COLUMN_DUMMY.to_string(),
        ))
        .collect();
    assert_eq!(columns.len(), NATIVE_FUNDING_ROW_COLUMNS);
    FundingFeatureRow { columns }
}

/// The template set of the frozen model: column 0 carries the token, column 16
/// closes the declared window. Without the second template the synthetic model
/// would declare a narrower window and be refused.
fn frozen_templates() -> Vec<CrfPattern> {
    vec![
        CrfPattern {
            source: "u:%x[0,0]".into(),
            kind: 'u',
        },
        CrfPattern {
            source: "u:%x[0,16]".into(),
            kind: 'u',
        },
    ]
}

/// Synthetic model over the closed twenty-label dictionary that forces
/// `labels[row]` at row `row`: every position owns a distinct unigram observation
/// (built from the row's own token text) and that observation weights exactly one
/// label strictly higher than the rest. No `b:` observation is declared, so all
/// transitions are zero and ties always resolve to the forced label.
fn model_for(rows: &[FundingFeatureRow], labels: &[&str]) -> CrfModel {
    assert_eq!(rows.len(), labels.len(), "one forced label per row");
    let vocabulary: Vec<String> = NATIVE_FUNDING_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    let label_count = vocabulary.len();
    let observations: Vec<String> = rows
        .iter()
        .map(|row| format!("u:{}", row.columns[0]))
        .collect();
    let mut u_offsets = Vec::with_capacity(rows.len());
    let mut weights = BTreeMap::new();
    for (position, label) in labels.iter().enumerate() {
        let offset = position * label_count;
        u_offsets.push(Some(offset));
        let target = NATIVE_FUNDING_LABELS
            .iter()
            .position(|known| known == label)
            .expect("the forced label is part of the closed dictionary");
        weights.insert(offset + target, 3.0);
    }
    CrfModel {
        model_type: 2,
        labels: vocabulary,
        observations,
        patterns: frozen_templates(),
        u_offsets,
        b_offsets: vec![None; labels.len()],
        weights,
        nftr: rows.len() * label_count,
    }
}

fn model_emitting(labels: &[&str]) -> (Vec<FundingFeatureRow>, CrfModel) {
    let rows: Vec<FundingFeatureRow> = (0..labels.len())
        .map(|index| row(&format!("t{index}")))
        .collect();
    let model = model_for(&rows, labels);
    (rows, model)
}

fn decoded(labels: &[&str]) -> NativeFundingDecodeOutcome {
    let (rows, model) = model_emitting(labels);
    decode_funding_sequence(&rows, &model).expect("the synthetic sequence must decode")
}

fn entries(labels: &[&str]) -> Vec<NativeFundingEntry> {
    match decoded(labels) {
        NativeFundingDecodeOutcome::Decoded { entries, .. } => entries,
        other => panic!("a funding sequence must decode, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. DATA binding
// ---------------------------------------------------------------------------

#[test]
fn the_rust_dictionary_and_abi_are_the_data_transcription_1_to_1() {
    let data = data();
    binding_verdict(
        &strings(&data, &[FUNDING[0], FUNDING[1], "declared_labels"]),
        &NATIVE_FUNDING_LABELS,
    )
    .expect("declared_labels must mirror NATIVE_FUNDING_LABELS");
    binding_verdict(
        &strings(&data, &[FUNDING[0], FUNDING[1], "base_labels"]),
        &NATIVE_FUNDING_BASE_LABELS,
    )
    .expect("base_labels must mirror NATIVE_FUNDING_BASE_LABELS");
    binding_verdict(
        &strings(
            &data,
            &[FUNDING[0], FUNDING[1], "java_declared_not_in_model"],
        ),
        &NATIVE_FUNDING_UNMODELED_JAVA_LABELS,
    )
    .expect("java_declared_not_in_model must mirror the unmodelled Java labels");

    // The role table is the same ten base labels, in the DATA order.
    assert_eq!(
        NativeFundingRole::ALL
            .iter()
            .map(|role| role.base_label())
            .collect::<Vec<_>>(),
        NATIVE_FUNDING_BASE_LABELS.to_vec()
    );

    // Model identity and the printed ABI.
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "kind"]).as_str(),
        Some("crf_sequence"),
        "the funding family is a CRF sequence, never a locator join"
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "model_id"])
            .as_str()
            .expect("model_id is a string"),
        FUNDING_MODEL_ID
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "model_file"])
            .as_str()
            .expect("model_file is a string"),
        FUNDING_MODEL_FILE
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "declared_checkpoint"])
            .as_str()
            .expect("declared_checkpoint is a string"),
        FUNDING_CHECKPOINT
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "declared_rdr"])
            .as_str()
            .expect("declared_rdr is a string"),
        FUNDING_RDR
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "declared_label_count"])
            .as_u64()
            .expect("declared_label_count is an integer"),
        NATIVE_FUNDING_LABELS.len() as u64
    );

    // Widths: 17 printed feature columns, label slot at 17, printed row 18.
    for key in ["referenced_columns", "emitted_columns"] {
        assert_eq!(
            get(&data, &[FUNDING[0], FUNDING[1], key])
                .as_u64()
                .unwrap_or_else(|| panic!("{key} is an integer")),
            NATIVE_FUNDING_FEATURE_COLUMNS as u64,
            "DATA {key} must equal the printed feature width"
        );
    }
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "label_column_index"])
            .as_u64()
            .expect("label_column_index is an integer"),
        NATIVE_FUNDING_LABEL_COLUMN_INDEX as u64
    );
    assert_eq!(
        get(&data, &[FUNDING[0], FUNDING[1], "label_column_value"])
            .as_str()
            .expect("label_column_value is a string"),
        NATIVE_FUNDING_LABEL_COLUMN_DUMMY
    );
    assert_eq!(
        NATIVE_FUNDING_LABEL_COLUMN_INDEX + 1,
        NATIVE_FUNDING_ROW_COLUMNS,
        "the label slot is the field after the last printed feature column"
    );

    // The declared `#rdr#` window is the window the runner enforces: its second
    // field is the highest referenced column index.
    let window = FUNDING_RDR
        .split('/')
        .nth(1)
        .expect("`#rdr#a/b/c` carries a second field")
        .parse::<usize>()
        .expect("the window is an integer");
    assert_eq!(window, NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX);
    assert_eq!(
        NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX + 1,
        NATIVE_FUNDING_FEATURE_COLUMNS,
        "the window is exactly the printed feature columns"
    );

    // The gazetteer lanes are declared here but never fed from here (D358/D363).
    assert_eq!(
        strings(&data, &[FUNDING[0], FUNDING[1], "gazetteer_lanes"]),
        FUNDING_GAZETTEER_LANES.to_vec()
    );
}

/// The file-level shadow locks, restated for this family's reader.
#[test]
fn the_family_stays_shadow_and_never_writes_a_graph() {
    let data = data();
    for (path, expected) in [
        (vec!["bundled_model_bytes"], serde_yaml::Value::Bool(false)),
        (vec!["import_eligible"], serde_yaml::Value::Bool(false)),
        (vec!["deployed"], serde_yaml::Value::Bool(false)),
    ] {
        assert_eq!(get(&data, &path), &expected, "DATA {path:?}");
    }
    for (path, expected) in [
        (vec!["production_route"], "forbidden"),
        (vec!["graph_write"], "forbidden"),
        (vec!["auto_promote"], "forbidden"),
        (vec!["observation_mode"], "shadow"),
        (vec!["promotion_state"], "candidate"),
        (vec!["model_root_env"], "DA_GROBID_MODEL_ROOT"),
    ] {
        assert_eq!(
            get(&data, &path).as_str(),
            Some(expected),
            "DATA {path:?} must be {expected}"
        );
    }
    assert_eq!(
        get(&data, &["decision_point"]).as_u64(),
        Some(6),
        "the funding band adds no DecisionPoint"
    );
    assert_eq!(
        strings(&data, &["independently_selectable"]),
        ["header", "references"]
    );
}

/// The binding is not decorative: dropping, renaming or reordering one
/// transcription entry must make it fail.
#[test]
fn the_binding_fails_closed_on_a_mutated_transcription() {
    let data = data();
    let declared = strings(&data, &[FUNDING[0], FUNDING[1], "declared_labels"]);
    assert!(binding_verdict(&declared, &NATIVE_FUNDING_LABELS).is_ok());

    let mut dropped = declared.clone();
    dropped.remove(2);
    assert!(binding_verdict(&dropped, &NATIVE_FUNDING_LABELS).is_err());

    let mut reordered = declared.clone();
    reordered.swap(0, 1);
    assert!(
        binding_verdict(&reordered, &NATIVE_FUNDING_LABELS).is_err(),
        "declared_labels is transcribed in model order, so a swap is a defect"
    );

    let mut renamed = declared.clone();
    renamed[3] = "<funderFull>".to_string();
    assert!(binding_verdict(&renamed, &NATIVE_FUNDING_LABELS).is_err());

    let base = strings(&data, &[FUNDING[0], FUNDING[1], "base_labels"]);
    assert!(binding_verdict(&base, &NATIVE_FUNDING_BASE_LABELS).is_ok());
    let mut base_dropped = base.clone();
    base_dropped.pop();
    assert!(binding_verdict(&base_dropped, &NATIVE_FUNDING_BASE_LABELS).is_err());

    let unmodelled = strings(
        &data,
        &[FUNDING[0], FUNDING[1], "java_declared_not_in_model"],
    );
    assert!(binding_verdict(&unmodelled, &NATIVE_FUNDING_UNMODELED_JAVA_LABELS).is_ok());
    let mut unmodelled_dropped = unmodelled.clone();
    unmodelled_dropped.remove(0);
    assert!(binding_verdict(&unmodelled_dropped, &NATIVE_FUNDING_UNMODELED_JAVA_LABELS).is_err());

    // The dictionary itself is a closed set: a label the model cannot emit is not
    // admissible, and no role maps it.
    for label in NATIVE_FUNDING_UNMODELED_JAVA_LABELS {
        assert!(!funding_label_is_admissible(label));
        assert_eq!(funding_role_of_label(label), None);
    }
    assert_eq!(
        NATIVE_FUNDING_LABELS
            .iter()
            .filter(|label| !funding_label_is_admissible(label))
            .count(),
        0
    );
}

// ---------------------------------------------------------------------------
// 2. Decode and fail-closed refusals
// ---------------------------------------------------------------------------

#[test]
fn a_synthetic_model_decodes_the_forced_labels_in_order() {
    let forced = ["<funderName>", "I-<grantNumber>", "<person>"];
    let (rows, model) = model_emitting(&forced);
    let outcome = decode_funding_sequence(&rows, &model).expect("the sequence decodes");

    // Equal inputs produce an equal decode, byte for byte.
    let again = decode_funding_sequence(&rows, &model).expect("the sequence decodes");
    assert_eq!(
        serde_json::to_string(&outcome).unwrap(),
        serde_json::to_string(&again).unwrap()
    );

    match outcome {
        NativeFundingDecodeOutcome::Decoded {
            labels,
            entries,
            other_count,
        } => {
            assert_eq!(labels, forced, "the decode preserves the forced labels");
            assert_eq!(other_count, 0);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].funder_name.as_deref(), Some("t0"));
            assert_eq!(entries[0].grant_number.as_deref(), Some("t1"));
            assert_eq!(entries[0].persons, ["t2"]);
        }
        other => panic!("expected a decode, got {other:?}"),
    }
}

/// The T05 renderer's row is the T06 decoder's row: the decoder accepts a row the
/// feature builder actually built, including the dummy label slot, so the two
/// halves of the funding band meet on one ABI.
#[test]
fn the_decoder_accepts_rows_the_t05_renderer_built() {
    let tokens: Vec<FundingFeatureToken> = ["t0", "t1"]
        .iter()
        .map(|text| FundingFeatureToken {
            text: (*text).to_string(),
            line_status: FundingLineStatus::Linestart,
        })
        .collect();
    let rows = render_funding_features(&tokens, &NativeFundingGazetteerCoverage::default())
        .expect("the T05 renderer must build the rows");
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row.width(), NATIVE_FUNDING_ROW_COLUMNS);
        assert_eq!(
            row.label_column(),
            Some(NATIVE_FUNDING_LABEL_COLUMN_DUMMY),
            "the inference ABI puts the literal 0 in the label slot"
        );
    }

    let model = model_for(&rows, &["<funderName>", "<person>"]);
    let outcome = decode_funding_sequence(&rows, &model).expect("a rendered row must decode");
    match outcome {
        NativeFundingDecodeOutcome::Decoded { entries, .. } => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].funder_name.as_deref(), Some("t0"));
            assert_eq!(entries[0].persons, ["t1"]);
        }
        other => panic!("expected a decode, got {other:?}"),
    }
}

#[test]
fn refuses_rows_that_do_not_carry_the_18_column_abi() {
    let (_, model) = model_emitting(&["<other>"]);
    assert_eq!(
        decode_funding_sequence(&[], &model),
        Err(NativeFundingSequenceError::EmptySequence)
    );
    for width in [17usize, 19] {
        let mut bad = row("t0");
        while bad.width() < width {
            bad.columns.push("0".into());
        }
        bad.columns.truncate(width);
        assert_eq!(
            decode_funding_sequence(&[bad], &model),
            Err(NativeFundingSequenceError::WrongArity {
                row: 0,
                actual: width,
                expected: NATIVE_FUNDING_ROW_COLUMNS,
            })
        );
    }
}

/// A label slot carrying a gold tag is a training-shape row, which this inference
/// band must refuse rather than decode.
#[test]
fn refuses_a_training_shape_label_slot() {
    let (_, model) = model_emitting(&["<other>"]);
    let mut bad = row("t0");
    bad.columns[NATIVE_FUNDING_LABEL_COLUMN_INDEX] = "<grantName>".into();
    assert_eq!(
        decode_funding_sequence(&[bad], &model),
        Err(NativeFundingSequenceError::WrongLabelSlot {
            row: 0,
            actual: "<grantName>".to_string(),
        })
    );
}

#[test]
fn refuses_a_foreign_label_block_by_count_and_by_set() {
    let (rows, mut model) = model_emitting(&["<other>"]);

    let mut short = model.clone();
    short.labels.pop();
    assert_eq!(
        check_funding_label_block(&short),
        Err(NativeFundingSequenceError::ModelLabelCount {
            expected: NATIVE_FUNDING_LABELS.len(),
            actual: NATIVE_FUNDING_LABELS.len() - 1,
        })
    );
    assert_eq!(
        decode_funding_sequence(&rows, &short),
        Err(NativeFundingSequenceError::ModelLabelCount {
            expected: NATIVE_FUNDING_LABELS.len(),
            actual: NATIVE_FUNDING_LABELS.len() - 1,
        })
    );

    let index = model
        .labels
        .iter()
        .position(|label| label == "<person>")
        .expect("the dictionary carries <person>");
    model.labels[index] = "<funderAbbrv>".into();
    assert_eq!(
        check_funding_label_block(&model),
        Err(NativeFundingSequenceError::ModelLabelMismatch {
            missing: vec!["<person>".to_string()],
            unexpected: vec!["<funderAbbrv>".to_string()],
        })
    );
    assert_eq!(model.labels.len(), NATIVE_FUNDING_LABELS.len());

    // An accepted block is accepted whatever order it lists the spellings in.
    let (_, mut reordered) = model_emitting(&["<other>"]);
    reordered.labels.reverse();
    assert_eq!(check_funding_label_block(&reordered), Ok(()));
}

/// The frozen window is `0..=16`: a narrower model and a model that reaches the
/// label slot are both refused, and the refusal happens *before* the decode — the
/// 17-case would otherwise expand cleanly, because an 18-column row really does
/// carry column 17.
#[test]
fn refuses_a_model_whose_referenced_window_is_not_the_frozen_one() {
    for declared in [15usize, 17, 26] {
        let (rows, mut model) = model_emitting(&["<other>"]);
        model.patterns[1] = CrfPattern {
            source: format!("u:%x[0,{declared}]"),
            kind: 'u',
        };
        assert_eq!(
            check_funding_column_window(&model),
            Err(NativeFundingSequenceError::DeclaredColumnMismatch {
                declared: Some(declared),
                expected: NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
            })
        );
        assert!(matches!(
            decode_funding_sequence(&rows, &model),
            Err(NativeFundingSequenceError::DeclaredColumnMismatch { .. })
        ));
    }

    // The guard is load-bearing: the decoder would expand column 17 cleanly.
    let (rows, mut model) = model_emitting(&["<other>"]);
    model.patterns[1] = CrfPattern {
        source: "u:%x[0,17]".into(),
        kind: 'u',
    };
    let rendered = da_parse::render_funding_feature_text(&rows).unwrap();
    assert_eq!(da_ports::map_input(&model, &rendered).unwrap().len(), 1);

    // A model that references no column at all cannot be the frozen model, and a
    // malformed template is a typed model refusal rather than a skipped reference.
    let (_, mut bare) = model_emitting(&["<other>"]);
    bare.patterns = vec![CrfPattern {
        source: "u:literal".into(),
        kind: 'u',
    }];
    assert_eq!(
        check_funding_column_window(&bare),
        Err(NativeFundingSequenceError::DeclaredColumnMismatch {
            declared: None,
            expected: NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
        })
    );
    for source in ["u:%x[0,16", "u:%x[0]", "u:%x[0,1,6]", "u:%x[0,x]"] {
        let (_, mut malformed) = model_emitting(&["<other>"]);
        malformed.patterns[1] = CrfPattern {
            source: source.into(),
            kind: 'u',
        };
        assert!(
            matches!(
                check_funding_column_window(&malformed),
                Err(NativeFundingSequenceError::Model { .. })
            ),
            "{source} must be a typed model refusal"
        );
    }
}

#[test]
fn the_row_cap_refuses_rather_than_truncating() {
    let (_, model) = model_emitting(&["<other>"]);
    let over = vec![row("t0"); MAX_NATIVE_FUNDING_ROWS + 1];
    assert_eq!(
        decode_funding_sequence(&over, &model),
        Err(NativeFundingSequenceError::TooManyRows {
            actual: MAX_NATIVE_FUNDING_ROWS + 1,
            maximum: MAX_NATIVE_FUNDING_ROWS,
        })
    );
}

/// The four labels the Java enum declares but the frozen model block does not
/// contain are refused *by name*, so a consumer can tell "a foreign label arrived"
/// from "the model block drifted".
#[test]
fn the_unmodelled_java_labels_refuse_by_name_and_are_never_mapped() {
    let rows = [row("t0"), row("t1"), row("t2"), row("t3")];
    for label in NATIVE_FUNDING_UNMODELED_JAVA_LABELS {
        assert_eq!(
            funding_entries_from_decoded(&[label.to_string()], &rows[..1]),
            Err(NativeFundingSequenceError::UnmodeledJavaLabel {
                label: (*label).to_string(),
                row: 0,
            }),
            "{label} must refuse by name"
        );
        // The classification looks through the spelling prefix while the
        // diagnostic keeps the label exactly as it was observed.
        assert_eq!(
            funding_entries_from_decoded(&[format!("I-{label}")], &rows[..1]),
            Err(NativeFundingSequenceError::UnmodeledJavaLabel {
                label: format!("I-{label}"),
                row: 0,
            })
        );
        // …and it is not admissible and carries no role either.
        assert!(!funding_label_is_admissible(label));
        assert_eq!(funding_role_of_label(label), None);
    }

    // A label that is neither in the dictionary nor declared by Java is a plain
    // unmodelled label, reported with its own spelling.
    for label in ["<not-a-role>", "I-<funderFull>", "<grantFull>"] {
        assert_eq!(
            funding_entries_from_decoded(&[label.to_string()], &rows[..1]),
            Err(NativeFundingSequenceError::UnmodeledLabel {
                label: label.to_string(),
                row: 0,
            }),
            "{label} must refuse as an unmodelled label"
        );
    }

    // Labels and rows must stay parallel.
    assert_eq!(
        funding_entries_from_decoded(&["<other>".to_string()], &[]),
        Err(NativeFundingSequenceError::CountMismatch {
            expected: 1,
            actual: 0,
        })
    );
}

// ---------------------------------------------------------------------------
// 3. The lift
// ---------------------------------------------------------------------------

/// Every one of the ten roles reaches its own destination, in one record when the
/// model emits each of them once.
#[test]
fn every_role_lifts_to_its_own_field() {
    let sequence: Vec<&str> = vec![
        "<other>",
        "<funderName>",
        "<grantNumber>",
        "<person>",
        "<programName>",
        "<grantName>",
        "<projectName>",
        "<affiliation>",
        "<institution>",
        "<infrastructure>",
    ];
    // Coverage guard: the sequence must actually exercise the whole role table,
    // so a role added to the dictionary without a destination fails here.
    let mut covered: Vec<NativeFundingRole> = sequence
        .iter()
        .map(|label| funding_role_of_label(label).expect("every label classifies"))
        .collect();
    covered.sort_unstable();
    covered.dedup();
    let mut all = NativeFundingRole::ALL.to_vec();
    all.sort_unstable();
    assert_eq!(covered, all);

    let (rows, model) = model_emitting(&sequence);
    let outcome = decode_funding_sequence(&rows, &model).expect("the sequence decodes");
    match outcome {
        NativeFundingDecodeOutcome::Decoded {
            entries,
            other_count,
            ..
        } => {
            assert_eq!(other_count, 1, "<other> is counted, not placed");
            assert_eq!(entries.len(), 1);
            let entry = &entries[0];
            assert_eq!(entry.funder_name.as_deref(), Some("t1"));
            assert_eq!(entry.grant_number.as_deref(), Some("t2"));
            assert_eq!(entry.program_name.as_deref(), Some("t4"));
            assert_eq!(entry.grant_name.as_deref(), Some("t5"));
            assert_eq!(entry.project_name.as_deref(), Some("t6"));
            assert_eq!(entry.persons, ["t3"]);
            assert_eq!(entry.affiliations, ["t7"]);
            assert_eq!(entry.institutions, ["t8", "t9"]);
            assert_eq!(entry.infrastructures, ["t9"]);
            assert!(entry.is_valid());
            assert!(entry.carries_any_observation());
            assert_eq!(entry.digest().len(), 71);
        }
        other => panic!("expected a decode, got {other:?}"),
    }
}

/// `<infrastructure>` is Java's `Affiliation.setInfrastructure(true)`: the text
/// lands in the institution list and is addressable as the infrastructure subset.
#[test]
fn infrastructure_marks_the_institution_it_projects() {
    let entry = entries(&["<institution>", "<infrastructure>"]);
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0].institutions, ["t0", "t1"]);
    assert_eq!(
        entry[0].infrastructures,
        ["t1"],
        "only the infrastructure cluster is marked"
    );

    // Both spellings of the infrastructure role mark their institution.
    let marked = entries(&["I-<infrastructure>"]);
    assert_eq!(marked[0].institutions, ["t0"]);
    assert_eq!(marked[0].infrastructures, ["t0"]);
}

/// The Java split rule: a repeat of a scalar role whose own slot already carries
/// text closes the record and opens the next one; a filled *list* slot does not.
#[test]
fn a_repeated_grant_number_at_a_valid_record_opens_a_new_record() {
    let split = entries(&["<grantNumber>", "I-<grantNumber>"]);
    assert_eq!(split.len(), 2);
    assert_eq!(split[0].grant_number.as_deref(), Some("t0"));
    assert_eq!(split[1].grant_number.as_deref(), Some("t1"));
    // The new record copied no funder: Java's grant-number "factorization" copy is
    // a named residual of this band, pinned here so it cannot drift silently.
    assert_eq!(split[1].funder_name, None);

    // Java's condition is the repeated slot, not "any field is set".
    let after_other = entries(&["<grantNumber>", "<grantName>", "I-<grantNumber>"]);
    assert_eq!(after_other.len(), 2);
    assert_eq!(after_other[0].grant_name.as_deref(), Some("t1"));
    assert_eq!(after_other[1].grant_name, None);

    // A filled list slot never splits the record.
    let list_first = entries(&["<person>", "I-<grantNumber>"]);
    assert_eq!(list_first.len(), 1);
    assert_eq!(list_first[0].persons, ["t0"]);
    assert_eq!(list_first[0].grant_number.as_deref(), Some("t1"));

    // The same rule holds for every scalar role.
    for (first, repeat) in [
        ("<funderName>", "I-<funderName>"),
        ("<grantName>", "I-<grantName>"),
        ("<programName>", "I-<programName>"),
        ("<projectName>", "I-<projectName>"),
    ] {
        let split = entries(&[first, repeat]);
        assert_eq!(split.len(), 2, "{first} repeats must split the record");
        assert!(split[0].is_valid());
        assert!(split[1].is_valid());
    }

    // An identical bare repeat is one cluster under the shared clusteror rule, so
    // it joins into a single value instead of splitting.
    let merged = entries(&["<grantNumber>", "<grantNumber>", "<programName>"]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].grant_number.as_deref(), Some("t0 t1"));
    assert_eq!(merged[0].program_name.as_deref(), Some("t2"));
}

/// `<other>` is not a field, but it is not dropped either: the counter is what
/// keeps the observation visible.
#[test]
fn other_is_counted_and_never_becomes_a_field() {
    let outcome = decoded(&["<other>", "<funderName>", "<other>"]);
    match outcome {
        NativeFundingDecodeOutcome::Decoded {
            entries,
            other_count,
            ..
        } => {
            assert_eq!(other_count, 2);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].funder_name.as_deref(), Some("t1"));
        }
        other => panic!("expected a decode, got {other:?}"),
    }

    // A paragraph whose only label is `<other>` is a *decode*: the family was
    // positively observed, and the counter is the only place that can say so.
    assert_eq!(
        decoded(&["<other>"]),
        NativeFundingDecodeOutcome::Decoded {
            labels: vec!["<other>".to_string()],
            entries: Vec::new(),
            other_count: 1,
        }
    );
}

/// H12-N1: an observation that produced nothing is an absence with a name, never a
/// successful empty entity list.
#[test]
fn absence_is_not_zero_funders() {
    assert_eq!(
        funding_entries_from_decoded(&[], &[]),
        Ok(NativeFundingDecodeOutcome::NotObserved {
            reason: NativeFundingAbsence::NoFundingLabels,
        })
    );

    // The decoded labels must span the whole role table for the two absence
    // reasons to be exhaustive: an empty entity list can only come from the
    // all-blank case, and it is typed.
    let (mut rows, model) = model_emitting(&["<funderName>", "<other>"]);
    for row in &mut rows {
        row.columns[0] = String::new();
    }
    assert_eq!(
        decode_funding_sequence(&rows, &model),
        Ok(NativeFundingDecodeOutcome::NotObserved {
            reason: NativeFundingAbsence::NoLiftableContent,
        })
    );

    // A person-only paragraph is *not* an absence: it is an observation the
    // flattened entry must conserve, even though it is not a valid Funding record.
    let person_only = entries(&["<person>"]);
    assert_eq!(person_only.len(), 1);
    assert_eq!(person_only[0].persons, ["t0"]);
    assert!(!person_only[0].is_valid());
    assert!(person_only[0].carries_any_observation());

    // Every decode either carries an entry or counts an `<other>`: the outcome
    // shape cannot express "I observed the family and found nothing".
    for sequence in [
        vec!["<other>"],
        vec!["<person>"],
        vec!["<funderName>", "I-<grantNumber>"],
        vec!["<other>", "<other>"],
    ] {
        match decoded(&sequence) {
            NativeFundingDecodeOutcome::Decoded {
                entries,
                other_count,
                ..
            } => assert!(
                !entries.is_empty() || other_count > 0,
                "{sequence:?} decoded to neither an entry nor a counted <other>"
            ),
            other => panic!("{sequence:?} must decode, got {other:?}"),
        }
    }
}

/// The outcome wire shape is the machine-readable contract a consumer reads.
#[test]
fn the_outcome_projection_is_stable() {
    let absent = funding_entries_from_decoded(&[], &[]).unwrap();
    assert_eq!(
        serde_json::to_string(&absent).unwrap(),
        r#"{"outcome":"not_observed","reason":"no_funding_labels"}"#
    );
    let round_tripped: NativeFundingDecodeOutcome =
        serde_json::from_str(&serde_json::to_string(&absent).unwrap())
            .expect("the outcome round-trips");
    assert_eq!(round_tripped, absent);

    let outcome = decoded(&["<funderName>", "<person>"]);
    assert_eq!(
        serde_json::to_string(&outcome).unwrap(),
        concat!(
            r#"{"outcome":"decoded","labels":["<funderName>","<person>"],"entries":["#,
            r#"{"funder_name":"t0","grant_name":null,"grant_number":null,"program_name":null,"#,
            r#""project_name":null,"persons":["t1"],"affiliations":[],"institutions":[],"#,
            r#""infrastructures":[]}],"other_count":0}"#
        )
    );
    let entry: NativeFundingEntry = match &outcome {
        NativeFundingDecodeOutcome::Decoded { entries, .. } => entries[0].clone(),
        other => panic!("expected a decode, got {other:?}"),
    };
    let entry_round_tripped: NativeFundingEntry =
        serde_json::from_str(&serde_json::to_string(&entry).unwrap())
            .expect("the entry projection round-trips");
    assert_eq!(entry_round_tripped, entry);
}

/// The band's boundary: no filesystem, environment, logging, YAML or model bytes,
/// and no second grouping rule (D359 keeps `base_label_runs` out of these
/// families).
#[test]
fn the_module_carries_no_model_material_or_io() {
    for forbidden in [
        "std::fs",
        "std::env",
        "include_bytes",
        "File::open",
        "read_to_string",
        "serde_yaml",
        "tracing::",
        "HashMap",
        "base_label_runs(",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "the funding sequence band must not carry `{forbidden}`"
        );
    }
    for required in [
        "crate::native_funding_features",
        "crate::native_visual_family_cluster",
        "cluster_family_label_runs",
        "NATIVE_FUNDING_LABEL_COLUMN_DUMMY",
        "NATIVE_FUNDING_UNMODELED_JAVA_LABELS",
        "%x[",
    ] {
        assert!(
            SOURCE.contains(required),
            "the funding sequence band must keep `{required}`"
        );
    }
    assert_eq!(
        NATIVE_FUNDING_SEQUENCE_SCHEMA_VERSION,
        "da-parse.native-funding-sequence/v1"
    );
}
