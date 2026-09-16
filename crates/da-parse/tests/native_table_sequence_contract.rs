//! S06/T03 contract for the native GROBID `table` sequence runner.
//!
//! Three independent gates live here:
//!
//! 1. the **DATA binding** — the Rust dictionary, the printed width and the
//!    referenced-column window of `data/parse/native_visual_families.yaml` must
//!    agree 1:1 with `native_table_sequence`, so a transcription edit on either
//!    side fails here;
//! 2. the **fail-closed behaviour** of the runner itself — wrong arity, a foreign
//!    label block, a column window that is not `0..=25`, the `TaggingTokenClusteror`
//!    `I-` boundary, the `TableParser.getExtractionResult` split, the Java
//!    `Table.validateTable` rejection contract, typed cell and caption absence,
//!    and the row cap;
//! 3. the **H12-N1 witnesses** — an absent cell inventory is never a table with
//!    zero rows or cells, and an empty caption inventory is never "zero links".
//!
//! The decoded model is synthetic and hand-authored in this file: the real GROBID
//! bytes are not redistributable and are never bundled (D358). Binding a real
//! model is the opt-in harness's job (S06/T07).

use da_parse::{
    FulltextFeatureRow, MAX_NATIVE_TABLE_ROWS, NATIVE_FULLTEXT_FEATURE_COLUMNS,
    NATIVE_TABLE_BASE_LABELS, NATIVE_TABLE_EMITTED_COLUMNS, NATIVE_TABLE_LABELS,
    NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, NATIVE_TABLE_SEQUENCE_SCHEMA_VERSION,
    NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker, NativeTableAbsence,
    NativeTableCell, NativeTableCells, NativeTableDecodeOutcome, NativeTableRevertReason,
    NativeTableSequenceError, NativeTableSpan, check_table_column_window, check_table_label_block,
    decode_table_sequence, table_cells_from_inventory, table_label_is_admissible,
    table_revert_reason, table_spans_from_decoded,
};
use da_ports::scalar_crf::{CrfModel, CrfPattern};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DATA: &str = "data/parse/native_visual_families.yaml";
const TABLE_CHECKPOINT: &str = "#mdl#2#1391";
const TABLE_RDR: &str = "#rdr#85/25/0";
const TABLE_TEMPLATE_EXCLUDES: [u64; 1] = [26];

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
            .unwrap_or_else(|| panic!("missing DATA key {key}"));
    }
    current
}

fn text(value: &serde_yaml::Value, path: &[&str]) -> String {
    get(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a string"))
        .to_string()
}

fn strings(value: &serde_yaml::Value, path: &[&str]) -> Vec<String> {
    get(value, path)
        .as_sequence()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not a sequence"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("DATA key {path:?} has a non-string entry"))
                .to_string()
        })
        .collect()
}

fn table(path: &[&str]) -> serde_yaml::Value {
    let mut full = vec!["families", "table"];
    full.extend_from_slice(path);
    get(&data(), &full).clone()
}

/// One 27-column row whose first column carries the token text; the remaining
/// columns are inert.
fn row(value: &str) -> FulltextFeatureRow {
    FulltextFeatureRow {
        columns: std::iter::once(value.to_string())
            .chain((1..NATIVE_FULLTEXT_FEATURE_COLUMNS).map(|_| "0".to_string()))
            .collect(),
    }
}

/// The template window of the frozen model: column 0 carries the token, column
/// 25 closes the declared window.
fn frozen_templates() -> Vec<CrfPattern> {
    vec![
        CrfPattern {
            source: "u:%x[0,0]".into(),
            kind: 'u',
        },
        CrfPattern {
            source: "u:%x[0,25]".into(),
            kind: 'u',
        },
    ]
}

/// Synthetic one-token-per-row model over the closed ten-label dictionary that
/// forces `labels[row]` at row `row` and renders `texts[row]` as that row's
/// token, so a fixture can present a Java-valid table (numeric label, head
/// starting with `tab`).
fn model_emitting_with(labels: &[(&str, &str)]) -> (Vec<FulltextFeatureRow>, CrfModel) {
    // The fulltext ABI is a whitespace-separated grid: a token carrying a space
    // would shift every column and leave this synthetic model matching nothing
    // (the decode would silently fall back to label 0). Fail loudly instead.
    for (_, value) in labels {
        assert!(
            !value.chars().any(char::is_whitespace),
            "fixture token {value:?} carries whitespace, which the 27-column ABI forbids"
        );
    }
    let rows = labels
        .iter()
        .map(|(_, value)| row(value))
        .collect::<Vec<_>>();
    let vocabulary: Vec<String> = NATIVE_TABLE_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    let label_count = vocabulary.len();
    let observations = labels
        .iter()
        .map(|(_, value)| format!("u:{value}"))
        .collect::<Vec<_>>();
    let mut u_offsets = Vec::with_capacity(observations.len());
    let mut weights = BTreeMap::new();
    for (position, (label, _)) in labels.iter().enumerate() {
        let offset = position * label_count;
        u_offsets.push(Some(offset));
        let target = NATIVE_TABLE_LABELS
            .iter()
            .position(|known| known == label)
            .unwrap_or_else(|| panic!("{label} is not part of the closed dictionary"));
        weights.insert(offset + target, 3.0);
    }
    let model = CrfModel {
        model_type: 2,
        labels: vocabulary,
        observations,
        patterns: frozen_templates(),
        u_offsets,
        b_offsets: vec![None; labels.len()],
        weights,
        nftr: labels.len() * label_count,
    };
    (rows, model)
}

/// The default fixture texts: `t0`, `t1`, ... in row order.
fn model_emitting(labels: &[&str]) -> (Vec<FulltextFeatureRow>, CrfModel) {
    let labelled: Vec<(&str, String)> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| (*label, format!("t{index}")))
        .collect();
    let pairs: Vec<(&str, &str)> = labelled
        .iter()
        .map(|(label, value)| (*label, value.as_str()))
        .collect();
    model_emitting_with(&pairs)
}

fn decode_with(
    labels: &[(&str, &str)],
    markers: &[NativeCaptionMarker],
    cells: NativeTableCells,
) -> NativeTableDecodeOutcome {
    let (rows, model) = model_emitting_with(labels);
    decode_table_sequence(&rows, &model, markers, cells).expect("synthetic sequence must decode")
}

fn decode(
    labels: &[&str],
    markers: &[NativeCaptionMarker],
    cells: NativeTableCells,
) -> NativeTableDecodeOutcome {
    let (rows, model) = model_emitting(labels);
    decode_table_sequence(&rows, &model, markers, cells).expect("synthetic sequence must decode")
}

fn tables(labels: &[&str]) -> Vec<NativeTableSpan> {
    match decode(labels, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { tables, .. } => tables,
        other => panic!("a table sequence must decode, got {other:?}"),
    }
}

fn table_of(labels: &[&str]) -> NativeTableSpan {
    tables(labels).into_iter().next().expect("one table")
}

fn no_cells() -> NativeTableCells {
    table_cells_from_inventory(Vec::new()).expect("an empty inventory is a typed absence")
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}

fn marker(character: char, cluster_index: usize) -> NativeCaptionMarker {
    NativeCaptionMarker {
        digest: digest(character),
        cluster_index,
    }
}

fn cell(row: u32, column: u32, character: char) -> NativeTableCell {
    NativeTableCell {
        row,
        column,
        content_digest: digest(character),
    }
}

/// A Java-valid table: head first (`tab...`), numeric label, non-empty content.
fn valid_table() -> Vec<(&'static str, &'static str)> {
    vec![
        ("<figure_head>", "tab.1-data"),
        ("I-<label>", "1"),
        ("<content>", "body"),
    ]
}

// ─────────────────── DATA ↔ Rust binding (1:1, no drift) ───────────────────

#[test]
fn the_rust_dictionary_is_the_data_transcription() {
    let declared = strings(&data(), &["families", "table", "declared_labels"]);
    assert_eq!(
        declared, NATIVE_TABLE_LABELS,
        "families.table.declared_labels must mirror NATIVE_TABLE_LABELS 1:1, in model order"
    );
    assert_eq!(
        table(&["declared_label_count"]).as_u64(),
        Some(NATIVE_TABLE_LABELS.len() as u64)
    );
    assert_eq!(
        NATIVE_TABLE_LABELS.len(),
        10,
        "the frozen model declares 10 labels"
    );

    let mut expected = NATIVE_TABLE_BASE_LABELS.to_vec();
    expected.sort_unstable();
    let mut observed = strings(&data(), &["families", "table", "base_labels"]);
    observed.sort_unstable();
    assert_eq!(observed, expected, "the six base roles must mirror 1:1");

    for label in NATIVE_TABLE_LABELS {
        assert!(table_label_is_admissible(label));
    }
    // `I-<label>` is the only spelling of the label role in the frozen block,
    // and `<table>` is a fulltext cluster label, not a label of this model.
    assert!(!table_label_is_admissible("<label>"));
    assert!(!table_label_is_admissible("<table>"));
    assert!(
        !NATIVE_TABLE_LABELS.contains(&"<other>"),
        "the frozen block declares only I-<other>, never the bare spelling"
    );
}

#[test]
fn the_printed_width_and_the_referenced_window_mirror_the_data() {
    assert_eq!(
        text(&data(), &["families", "table", "model_file"]),
        "table/model.wapiti"
    );
    assert_eq!(
        text(&data(), &["families", "table", "declared_checkpoint"]),
        TABLE_CHECKPOINT
    );
    assert_eq!(
        text(&data(), &["families", "table", "declared_rdr"]),
        TABLE_RDR
    );
    assert_eq!(
        text(&data(), &["families", "table", "kind"]),
        "crf_sequence"
    );

    let emitted = table(&["emitted_columns"])
        .as_u64()
        .expect("emitted_columns");
    assert_eq!(emitted as usize, NATIVE_TABLE_EMITTED_COLUMNS);
    assert_eq!(
        NATIVE_TABLE_EMITTED_COLUMNS,
        NATIVE_FULLTEXT_FEATURE_COLUMNS
    );
    assert_eq!(
        emitted as usize, 27,
        "the table family reads the S05 fulltext 27-column ABI, not a second ABI"
    );

    let referenced = table(&["referenced_columns"])
        .as_u64()
        .expect("referenced_columns");
    assert_eq!(
        referenced as usize,
        NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX + 1,
        "the Rust window constant must be the DATA referenced-column count minus one"
    );
    let excludes: Vec<u64> = table(&["template_excludes"])
        .as_sequence()
        .expect("template_excludes")
        .iter()
        .map(|item| item.as_u64().expect("a column index"))
        .collect();
    assert_eq!(excludes, TABLE_TEMPLATE_EXCLUDES);
    assert!(
        excludes.contains(&referenced),
        "the excluded column is the one past the window"
    );

    // `#rdr#85/25/0` — the second field is the highest referenced column index.
    let window = TABLE_RDR
        .trim_start_matches("#rdr#")
        .split('/')
        .nth(1)
        .and_then(|field| field.parse::<usize>().ok());
    assert_eq!(window, Some(NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX));

    let not_in_model = strings(
        &data(),
        &["families", "table", "java_declared_not_in_model"],
    );
    assert!(
        not_in_model.is_empty(),
        "the table family has no declared-but-unmodeled label to refuse: {not_in_model:?}"
    );

    assert_eq!(
        NATIVE_TABLE_SEQUENCE_SCHEMA_VERSION,
        "da-parse.native-table-sequence/v1"
    );
}

#[test]
fn the_table_lane_stays_shadow_locked() {
    let root_data = data();
    assert_eq!(text(&root_data, &["observation_mode"]), "shadow");
    assert_eq!(text(&root_data, &["promotion_state"]), "candidate");
    assert_eq!(text(&root_data, &["production_route"]), "forbidden");
    assert_eq!(text(&root_data, &["graph_write"]), "forbidden");
    assert_eq!(text(&root_data, &["auto_promote"]), "forbidden");
    assert_eq!(
        get(&root_data, &["import_eligible"]).as_bool(),
        Some(false),
        "D127 stays closed"
    );
    assert_eq!(
        get(&root_data, &["bundled_model_bytes"]).as_bool(),
        Some(false),
        "D358: no model bytes in the tree"
    );
    assert_eq!(get(&root_data, &["deployed"]).as_bool(), Some(false));
    assert_eq!(get(&root_data, &["decision_point"]).as_u64(), Some(6));
    assert_eq!(
        strings(&root_data, &["independently_selectable"]),
        ["header", "references"]
    );
    // The grouping rule this runner uses is the clusteror, not D359.
    assert_eq!(
        text(&root_data, &["grouping", "rule"]),
        "tagging_token_clusteror_is_beginning"
    );
    assert_eq!(text(&root_data, &["grouping", "name_prefix"]), "I-");
    assert_eq!(
        text(&root_data, &["grouping", "diverges_from"]),
        "base_label_runs"
    );
}

// ───────────────────────── fail-closed runner behaviour ─────────────────────────

#[test]
fn refuses_rows_that_do_not_carry_the_27_column_abi() {
    let (_, model) = model_emitting(&["<content>"]);
    for width in [26usize, 28] {
        let mut bad = row("t0");
        while bad.columns.len() < width {
            bad.columns.push("0".into());
        }
        bad.columns.truncate(width);
        assert_eq!(
            decode_table_sequence(&[bad], &model, &[], no_cells()),
            Err(NativeTableSequenceError::WrongArity {
                row: 0,
                actual: width,
                expected: NATIVE_TABLE_EMITTED_COLUMNS,
            })
        );
    }

    assert_eq!(
        decode_table_sequence(&[], &model, &[], no_cells()),
        Err(NativeTableSequenceError::EmptySequence)
    );
}

#[test]
fn refuses_a_foreign_label_block_with_both_directions_reported() {
    let (rows, mut model) = model_emitting(&["<content>"]);
    model.labels.pop();
    assert_eq!(
        check_table_label_block(&model),
        Err(NativeTableSequenceError::ModelLabelCount {
            expected: 10,
            actual: 9,
        })
    );
    assert_eq!(
        decode_table_sequence(&rows, &model, &[], no_cells()),
        Err(NativeTableSequenceError::ModelLabelCount {
            expected: 10,
            actual: 9,
        })
    );

    let (rows, mut model) = model_emitting(&["<content>"]);
    let index = model
        .labels
        .iter()
        .position(|label| label == "<note>")
        .expect("the dictionary carries <note>");
    model.labels[index] = "<table>".into();
    assert_eq!(model.labels.len(), 10);
    assert_eq!(
        check_table_label_block(&model),
        Err(NativeTableSequenceError::ModelLabelMismatch {
            missing: vec!["<note>".to_string()],
            unexpected: vec!["<table>".to_string()],
        })
    );
    assert_eq!(
        decode_table_sequence(&rows, &model, &[], no_cells()),
        Err(NativeTableSequenceError::ModelLabelMismatch {
            missing: vec!["<note>".to_string()],
            unexpected: vec!["<table>".to_string()],
        })
    );

    // A same-sized block in another order still declares the same set: the check
    // is order-independent because the loaded model owns the label ids.
    let (_, mut reordered) = model_emitting(&["<content>"]);
    reordered.labels.reverse();
    assert_eq!(check_table_label_block(&reordered), Ok(()));
}

#[test]
fn refuses_a_referenced_window_that_is_not_the_frozen_one() {
    for declared in [24usize, 26] {
        let (rows, mut model) = model_emitting(&["<content>"]);
        model.patterns[1] = CrfPattern {
            source: format!("u:%x[0,{declared}]"),
            kind: 'u',
        };
        assert_eq!(
            check_table_column_window(&model),
            Err(NativeTableSequenceError::DeclaredColumnMismatch {
                declared: Some(declared),
                expected: NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
            })
        );
        assert_eq!(
            decode_table_sequence(&rows, &model, &[], no_cells()),
            Err(NativeTableSequenceError::DeclaredColumnMismatch {
                declared: Some(declared),
                expected: NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
            })
        );
    }

    // The guard is load-bearing rather than decorative: the 27-column ABI really
    // does carry column 26, so the decode path would expand `%x[0,26]` cleanly.
    let (rows, mut model) = model_emitting(&["<content>"]);
    model.patterns[1] = CrfPattern {
        source: "u:%x[0,26]".into(),
        kind: 'u',
    };
    let rendered = da_parse::render_fulltext_feature_text(&rows).unwrap();
    assert_eq!(da_ports::map_input(&model, &rendered).unwrap().len(), 1);
    assert!(check_table_column_window(&model).is_err());

    // A model that references no column at all cannot be the frozen model.
    let (_, mut model) = model_emitting(&["<content>"]);
    model.patterns = vec![CrfPattern {
        source: "u:literal".into(),
        kind: 'u',
    }];
    assert_eq!(
        check_table_column_window(&model),
        Err(NativeTableSequenceError::DeclaredColumnMismatch {
            declared: None,
            expected: NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
        })
    );

    // A malformed template is typed, never skipped: skipping it could hide the
    // very reference the window check exists to catch.
    for source in ["u:%x[0,25", "u:%x[0]", "u:%x[0,2,5]", "u:%x[0,x]"] {
        let (_, mut model) = model_emitting(&["<content>"]);
        model.patterns[1] = CrfPattern {
            source: source.into(),
            kind: 'u',
        };
        assert!(
            matches!(
                check_table_column_window(&model),
                Err(NativeTableSequenceError::Model { .. })
            ),
            "{source} must be a typed model refusal"
        );
    }
}

/// `TableParser.getExtractionResult`: a `<figure_head>` cluster starts a new
/// table exactly when the open table already has a header and the previous
/// cluster was a body (content/note/desc).
#[test]
fn one_decode_splits_into_every_table_it_contains() {
    assert_eq!(tables(&["<content>"]).len(), 1);

    let merged = tables(&["<content>", "<content>"]);
    assert_eq!(
        merged.len(),
        1,
        "a bare repeat is one cluster, hence one table"
    );
    assert_eq!(
        merged[0].content.as_deref(),
        Some("t0 t1"),
        "both tokens of the merged cluster are carried exactly once"
    );

    // An `I-` repeat is a *second cluster* of the same base, but the split
    // trigger is the head, not the body — so this is still one table and the two
    // clusters join into one content role.
    let inside_repeat = tables(&["<content>", "I-<content>"]);
    assert_eq!(inside_repeat.len(), 1);
    assert_eq!(inside_repeat[0].content.as_deref(), Some("t0 t1"));

    let split = tables(&["<figure_head>", "<content>", "I-<figure_head>"]);
    assert_eq!(split.len(), 2);
    assert_eq!(split[0].cluster_index, 0);
    assert_eq!(split[1].cluster_index, 2);
    assert_eq!(split[0].head.as_deref(), Some("t0"));
    assert_eq!(split[0].content.as_deref(), Some("t1"));
    assert_eq!(split[1].head.as_deref(), Some("t2"));
    assert_eq!(split[1].content, None);

    // The bare spelling of the second head splits identically: the comparison is
    // on the plain label, not on the `I-` prefix.
    assert_eq!(
        tables(&["<figure_head>", "<content>", "<figure_head>"]).len(),
        2
    );
    // A note or a caption also counts as the "previous" body label.
    assert_eq!(
        tables(&["<figure_head>", "<note>", "I-<figure_head>"]).len(),
        2
    );
    assert_eq!(
        tables(&["<figure_head>", "<figDesc>", "I-<figure_head>"]).len(),
        2
    );
    // A `<label>` also fills the header, so a head after it still splits... only
    // when a body intervened: without one the previous label is the label itself.
    assert_eq!(
        tables(&["<figure_head>", "I-<label>", "<content>"]).len(),
        1
    );
    assert_eq!(
        tables(&["<figure_head>", "I-<label>", "<content>", "I-<figure_head>"]).len(),
        2
    );
    // Two consecutive head clusters merge into one table (previous is a head).
    assert_eq!(tables(&["<figure_head>", "I-<figure_head>"]).len(), 1);
    // Without a header there is nothing to close.
    assert_eq!(tables(&["<content>", "I-<figure_head>"]).len(), 1);
}

#[test]
fn every_role_maps_to_its_own_field_and_absent_roles_stay_none() {
    let full = table_of(&[
        "I-<label>",
        "<figure_head>",
        "<figDesc>",
        "<content>",
        "<note>",
        "I-<other>",
    ]);
    assert_eq!(full.label.as_deref(), Some("t0"));
    assert_eq!(full.head.as_deref(), Some("t1"));
    assert_eq!(full.caption.as_deref(), Some("t2"));
    assert_eq!(full.content.as_deref(), Some("t3"));
    assert_eq!(full.note.as_deref(), Some("t4"));
    assert_eq!(full.other.as_deref(), Some("t5"));
    assert_eq!(full.cluster_index, 0);

    let partial = table_of(&["<note>"]);
    assert_eq!(partial.note.as_deref(), Some("t0"));
    assert_eq!(partial.label, None);
    assert_eq!(partial.head, None);
    assert_eq!(partial.caption, None);
    assert_eq!(partial.content, None);
    assert_eq!(partial.other, None);
}

/// `Table.validateTable`: a reverted table is flagged and kept, never silently
/// dropped and never reported as complete.
#[test]
fn an_invalid_table_is_reverted_not_dropped() {
    let valid = match decode_with(&valid_table(), &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { mut tables, .. } => tables.remove(0),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(valid.label.as_deref(), Some("1"));
    assert_eq!(valid.head.as_deref(), Some("tab.1-data"));
    assert_eq!(valid.content.as_deref(), Some("body"));
    assert_eq!(
        valid.reverted, None,
        "a tab-prefixed numeric table is valid"
    );

    // A non-integer label, with a valid header otherwise.
    let bad_label = vec![
        ("<figure_head>", "tab.1-data"),
        ("I-<label>", "Figure1"),
        ("<content>", "body"),
    ];
    let reverted = match decode_with(&bad_label, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { mut tables, .. } => tables.remove(0),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(
        reverted.reverted,
        Some(NativeTableRevertReason::LabelNotInteger)
    );
    assert_eq!(
        reverted.content.as_deref(),
        Some("body"),
        "the rejected table keeps its assembled fields instead of vanishing"
    );

    // A head that does not start with `tab`. Note that `Table 1: data` would
    // pass: the Java test is case-insensitive and covers `table`, `tabelle`,
    // `tabella` and friends via the shared `tab` prefix.
    let bad_head = vec![
        ("<figure_head>", "Figure1-data"),
        ("I-<label>", "1"),
        ("<content>", "body"),
    ];
    let reverted = match decode_with(&bad_head, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { mut tables, .. } => tables.remove(0),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(
        reverted.reverted,
        Some(NativeTableRevertReason::HeadNotTabPrefixed)
    );
    // Java compares the prefix ignoring case.
    let shouted = vec![
        ("<figure_head>", "TABELLE1"),
        ("I-<label>", "1"),
        ("<content>", "body"),
    ];
    match decode_with(&shouted, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { tables, .. } => assert_eq!(tables[0].reverted, None),
        other => panic!("expected a decode, got {other:?}"),
    }

    // No content at all.
    let empty_body = vec![("<figure_head>", "tab.1-data"), ("I-<label>", "1")];
    let reverted = match decode_with(&empty_body, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { mut tables, .. } => tables.remove(0),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(
        reverted.reverted,
        Some(NativeTableRevertReason::EmptyContent)
    );

    // An absent label reverts too: Java's blank guard covers `label` as well.
    let unlabelled = vec![("<figure_head>", "tab.1-data"), ("<content>", "body")];
    match decode_with(&unlabelled, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { tables, .. } => assert_eq!(
            tables[0].reverted,
            Some(NativeTableRevertReason::LabelNotInteger)
        ),
        other => panic!("expected a decode, got {other:?}"),
    }

    // The flag is one reason, not a bitmask: label → header → content order.
    assert_eq!(
        table_revert_reason(Some("x"), Some("y"), None),
        Some(NativeTableRevertReason::LabelNotInteger)
    );
}

/// Two transcription details that are Java's behaviour, not local choices, are
/// pinned here so a future edit cannot silently "fix" them.
#[test]
fn the_assembled_header_is_what_validate_table_sees() {
    // `TableParser` appends `" " + content + " "` for a label cluster, so a table
    // whose label precedes its head starts with a space and cannot match `tab`.
    let label_first = vec![
        ("I-<label>", "1"),
        ("<figure_head>", "tab.1-data"),
        ("<content>", "body"),
    ];
    let reverted = match decode_with(&label_first, &[], no_cells()) {
        NativeTableDecodeOutcome::Decoded { mut tables, .. } => tables.remove(0),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(
        reverted.reverted,
        Some(NativeTableRevertReason::HeadNotTabPrefixed),
        "the label branch prepends a space and the prefix test does not trim"
    );

    // The label itself is trimmed before parsing, exactly as Java trims it.
    assert_eq!(
        table_revert_reason(Some(" 1 "), Some("tab. 1"), Some("body")),
        None
    );
    // A blank (whitespace-only) field reverts, as `isAnyBlank` requires.
    assert_eq!(
        table_revert_reason(Some("1"), Some("tab. 1"), Some("   ")),
        Some(NativeTableRevertReason::EmptyContent)
    );
}

// ───────────────────────────── cells (H12-N1) ─────────────────────────────

#[test]
fn an_absent_cell_inventory_is_not_a_table_with_zero_cells() {
    let span = table_of(&["<content>"]);
    assert_eq!(
        span.cells,
        NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        }
    );
    assert_ne!(
        span.cells,
        NativeTableCells::Provided { cells: Vec::new() },
        "absence must never be encoded as an empty success"
    );
    assert!(matches!(span.cells, NativeTableCells::NotObserved { .. }));

    // The constructor applies the same rule, so a caller cannot build the failure
    // mode by hand either.
    assert_eq!(
        table_cells_from_inventory(Vec::new()),
        Ok(NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        })
    );

    // H12-N1 defensively as well: a hand-built provided-but-empty inventory is
    // normalized to the typed absence rather than accepted as a success.
    let (rows, model) = model_emitting(&["<content>"]);
    let outcome = decode_table_sequence(
        &rows,
        &model,
        &[],
        NativeTableCells::Provided { cells: Vec::new() },
    )
    .expect("an empty provided inventory decodes");
    match outcome {
        NativeTableDecodeOutcome::Decoded { tables, .. } => assert_eq!(
            tables[0].cells,
            NativeTableCells::NotObserved {
                reason: NativeTableAbsence::NoCellInventory,
            }
        ),
        other => panic!("expected a decode, got {other:?}"),
    }
}

#[test]
fn a_provided_cell_inventory_is_carried_once_with_its_units_intact() {
    let inventory = vec![cell(0, 0, 'a'), cell(0, 1, 'b')];
    let decoded = decode(
        &["<content>"],
        &[],
        table_cells_from_inventory(inventory.clone()).unwrap(),
    );
    let single = match decoded {
        NativeTableDecodeOutcome::Decoded { tables, .. } => tables[0].clone(),
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(
        single.cells,
        NativeTableCells::Provided {
            cells: inventory.clone()
        },
        "row/column units survive verbatim"
    );

    // A split decode attributes the one inventory to exactly one table: the cell
    // count is preserved, never duplicated across tables.
    let (rows, model) = model_emitting(&["<figure_head>", "<content>", "I-<figure_head>"]);
    let split = decode_table_sequence(
        &rows,
        &model,
        &[],
        table_cells_from_inventory(inventory.clone()).unwrap(),
    )
    .expect("the split decode succeeds");
    let split_tables = match split {
        NativeTableDecodeOutcome::Decoded { tables, .. } => tables,
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(split_tables.len(), 2);
    assert_eq!(
        split_tables[0].cells,
        NativeTableCells::Provided {
            cells: inventory.clone()
        },
        "the inventory is attributed once, to the first table"
    );
    assert_eq!(
        split_tables[1].cells,
        NativeTableCells::NotObserved {
            reason: NativeTableAbsence::AmbiguousInventory,
        }
    );
    let carried: usize = split_tables
        .iter()
        .map(|table| match &table.cells {
            NativeTableCells::Provided { cells } => cells.len(),
            NativeTableCells::NotObserved { .. } => 0,
        })
        .sum();
    assert_eq!(
        carried,
        inventory.len(),
        "the cells appear exactly once in the whole decode"
    );
}

#[test]
fn a_malformed_cell_inventory_is_refused() {
    // A digest outside the shared lower-hex-64 dialect.
    for bad_digest in ["abc".to_string(), "A".repeat(64), "z".repeat(64)] {
        assert_eq!(
            table_cells_from_inventory(vec![NativeTableCell {
                row: 0,
                column: 0,
                content_digest: bad_digest.clone(),
            }]),
            Err(NativeTableSequenceError::InvalidCellInventory),
            "{bad_digest} must be refused"
        );
    }

    // Two cells cannot claim the same position.
    assert_eq!(
        table_cells_from_inventory(vec![cell(1, 1, 'a'), cell(1, 1, 'b')]),
        Err(NativeTableSequenceError::InvalidCellInventory)
    );
    // A different row or column at another digest is a different cell.
    assert!(table_cells_from_inventory(vec![cell(1, 1, 'a'), cell(1, 2, 'b')]).is_ok());
    assert!(table_cells_from_inventory(vec![cell(1, 1, 'a'), cell(2, 1, 'b')]).is_ok());

    // The decode validates a hand-built `Provided` too, not only the constructor.
    let (rows, model) = model_emitting(&["<content>"]);
    assert_eq!(
        decode_table_sequence(
            &rows,
            &model,
            &[],
            NativeTableCells::Provided {
                cells: vec![cell(0, 0, 'a'), cell(0, 0, 'b')],
            },
        ),
        Err(NativeTableSequenceError::InvalidCellInventory)
    );
}

// ───────────────────────────── caption lane ─────────────────────────────

#[test]
fn caption_links_are_provided_per_table_and_never_silently_invalid() {
    // H12-N1: an empty inventory is an absence, never "zero links".
    assert_eq!(
        table_of(&["<content>"]).caption_links,
        NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(NativeCaptionLinkAbsence::NotObserved),
        }
    );

    // Markers are distributed by the cluster they address, so a split keeps each
    // marker exactly once on the table that owns its cluster.
    let inventory = [marker('a', 0), marker('b', 2)];
    let split = decode(
        &["<figure_head>", "<content>", "I-<figure_head>"],
        &inventory,
        no_cells(),
    );
    let split_tables = match split {
        NativeTableDecodeOutcome::Decoded { tables, .. } => tables,
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(split_tables.len(), 2);
    assert_eq!(split_tables[0].caption_links.absence, None);
    assert_eq!(
        split_tables[0]
            .caption_links
            .markers
            .iter()
            .map(|marker| marker.digest.clone())
            .collect::<Vec<_>>(),
        [digest('a')]
    );
    assert_eq!(split_tables[1].caption_links.absence, None);
    assert_eq!(
        split_tables[1]
            .caption_links
            .markers
            .iter()
            .map(|marker| marker.digest.clone())
            .collect::<Vec<_>>(),
        [digest('b')]
    );

    // A table whose range holds no marker reports an absence, not zero links.
    let (rows, model) = model_emitting(&["<figure_head>", "<content>", "I-<figure_head>"]);
    let outcome = decode_table_sequence(&rows, &model, &[marker('c', 1)], no_cells()).unwrap();
    match outcome {
        NativeTableDecodeOutcome::Decoded { tables, .. } => {
            assert_eq!(
                tables[0].caption_links,
                NativeCaptionLinks {
                    markers: vec![marker('c', 1)],
                    absence: None,
                }
            );
            assert_eq!(
                tables[1].caption_links,
                NativeCaptionLinks {
                    markers: Vec::new(),
                    absence: Some(NativeCaptionLinkAbsence::NotObserved),
                }
            );
        }
        other => panic!("expected a decode, got {other:?}"),
    }

    // A present-but-malformed inventory must not decay into an absence.
    let (rows, model) = model_emitting(&["<content>", "<figDesc>"]);
    for bad in [
        NativeCaptionMarker {
            digest: "abc".into(),
            cluster_index: 0,
        },
        NativeCaptionMarker {
            digest: "A".repeat(64),
            cluster_index: 0,
        },
    ] {
        assert_eq!(
            decode_table_sequence(&rows, &model, &[bad], no_cells()),
            Err(NativeTableSequenceError::InvalidCaptionInventory)
        );
    }
    let duplicated = [marker('c', 0), marker('c', 1)];
    assert_eq!(
        decode_table_sequence(&rows, &model, &duplicated, no_cells()),
        Err(NativeTableSequenceError::InvalidCaptionInventory)
    );

    // A marker addressing a cluster this decode does not contain is refused
    // rather than silently dropped.
    assert_eq!(
        decode_table_sequence(&rows, &model, &[marker('d', 2)], no_cells()),
        Err(NativeTableSequenceError::DanglingCaptionCluster {
            cluster_index: 2,
            clusters: 2,
        })
    );
    assert!(decode_table_sequence(&rows, &model, &[marker('d', 1)], no_cells()).is_ok());
}

// ───────────────────── absence, cap, determinism, projection ─────────────────────

#[test]
fn an_empty_decode_is_not_observed_rather_than_zero_tables() {
    assert_eq!(
        table_spans_from_decoded(&[], &[], &[], no_cells()),
        Ok(NativeTableDecodeOutcome::NotObserved {
            reason: NativeTableAbsence::NoTableClusters,
        })
    );

    let rows = vec![row("t0")];
    assert_eq!(
        table_spans_from_decoded(&["<table>".to_string()], &rows, &[], no_cells()),
        Err(NativeTableSequenceError::UnmodeledLabel {
            label: "<table>".to_string(),
            row: 0,
        }),
        "a fulltext cluster label is not a label of this model"
    );
    assert_eq!(
        table_spans_from_decoded(&["<content>".to_string()], &[], &[], no_cells()),
        Err(NativeTableSequenceError::CountMismatch {
            expected: 1,
            actual: 0,
        })
    );
}

/// The cap is inclusive and refuses rather than truncating: a reader must never
/// mistake a silently partial decode for a complete one.
#[test]
fn the_row_cap_refuses_rather_than_truncating() {
    let (_, model) = model_emitting(&["<content>"]);

    let at_cap = vec![row("t0"); MAX_NATIVE_TABLE_ROWS];
    match decode_table_sequence(&at_cap, &model, &[], no_cells()) {
        Ok(NativeTableDecodeOutcome::Decoded { labels, tables }) => {
            assert_eq!(labels.len(), MAX_NATIVE_TABLE_ROWS);
            assert_eq!(tables.len(), 1);
        }
        other => panic!("exactly the cap must decode, got {other:?}"),
    }

    let over_cap = vec![row("t0"); MAX_NATIVE_TABLE_ROWS + 1];
    assert_eq!(
        decode_table_sequence(&over_cap, &model, &[], no_cells()),
        Err(NativeTableSequenceError::TooManyRows {
            actual: MAX_NATIVE_TABLE_ROWS + 1,
            maximum: MAX_NATIVE_TABLE_ROWS,
        })
    );
}

#[test]
fn the_rerun_is_byte_identical() {
    let (rows, model) = model_emitting(&["<figure_head>", "<content>", "I-<figure_head>"]);
    let inventory = [marker('e', 0)];
    let cells = table_cells_from_inventory(vec![cell(0, 0, 'f')]).unwrap();
    let first = decode_table_sequence(&rows, &model, &inventory, cells.clone()).unwrap();
    let second = decode_table_sequence(&rows, &model, &inventory, cells).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn the_span_projection_is_stable() {
    let absent = table_of(&["<content>"]);
    assert_eq!(
        serde_json::to_string(&absent).unwrap(),
        concat!(
            r#"{"label":null,"head":null,"caption":null,"content":"t0","note":null,"#,
            r#""other":null,"reverted":"label_not_integer","#,
            r#""cells":{"state":"not_observed","reason":"no_cell_inventory"},"#,
            r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}"#
        )
    );
    let round_tripped: NativeTableSpan =
        serde_json::from_str(&serde_json::to_string(&absent).unwrap()).expect("round-trips");
    assert_eq!(round_tripped, absent);

    let decoded = decode(&["<content>"], &[], no_cells());
    assert_eq!(
        serde_json::to_string(&decoded).unwrap(),
        concat!(
            r#"{"outcome":"decoded","labels":["<content>"],"tables":["#,
            r#"{"label":null,"head":null,"caption":null,"content":"t0","note":null,"#,
            r#""other":null,"reverted":"label_not_integer","#,
            r#""cells":{"state":"not_observed","reason":"no_cell_inventory"},"#,
            r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}]}"#
        )
    );

    // A valid table projects no revert reason; a body-less one projects the
    // content reason.
    let valid = decode_with(&valid_table(), &[], no_cells());
    let json = serde_json::to_string(&valid).unwrap();
    assert!(json.contains(r#""reverted":null"#), "{json}");
    let empty_body = decode_with(
        &[("<figure_head>", "tab.1-data"), ("I-<label>", "1")],
        &[],
        no_cells(),
    );
    let json = serde_json::to_string(&empty_body).unwrap();
    assert!(
        json.contains(r#""reverted":"empty_content""#),
        "a table without content must project its revert reason: {json}"
    );
}
