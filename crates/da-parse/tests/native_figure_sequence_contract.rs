//! S06/T02 contract for the native GROBID `figure` sequence runner.
//!
//! Two independent gates live here:
//!
//! 1. the **DATA binding** — the Rust dictionary, the printed width and the
//!    referenced-column window of `data/parse/native_visual_families.yaml` must
//!    agree 1:1 with `native_figure_sequence`, so a transcription edit on either
//!    side fails here;
//! 2. the **fail-closed behaviour** of the runner itself — wrong arity, a
//!    foreign label block, a column window that is not `0..=25`, the
//!    `TaggingTokenClusteror` `I-` boundary, role mapping, typed caption absence
//!    and the row cap.
//!
//! The decoded model is synthetic and hand-authored in this file: the real
//! GROBID bytes are not redistributable and are never bundled (D358). Binding a
//! real model is the opt-in harness's job (S06/T07).

use da_parse::{
    FigureSpan, FulltextFeatureRow, MAX_NATIVE_FIGURE_ROWS, NATIVE_FIGURE_BASE_LABELS,
    NATIVE_FIGURE_EMITTED_COLUMNS, NATIVE_FIGURE_LABELS, NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
    NATIVE_FIGURE_SEQUENCE_SCHEMA_VERSION, NATIVE_FULLTEXT_FEATURE_COLUMNS,
    NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker, NativeFigureAbsence,
    NativeFigureDecodeOutcome, NativeFigureSequenceError, check_figure_column_window,
    check_figure_label_block, decode_figure_sequence, figure_label_is_admissible,
    figure_spans_from_decoded, render_fulltext_feature_text,
};
use da_ports::scalar_crf::{CrfModel, CrfPattern};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DATA: &str = "data/parse/native_visual_families.yaml";
const FIGURE_CHECKPOINT: &str = "#mdl#2#521";
const FIGURE_RDR: &str = "#rdr#85/25/0";
const FIGURE_TEMPLATE_EXCLUDES: [u64; 1] = [26];

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

fn figure(path: &[&str]) -> serde_yaml::Value {
    let mut full = vec!["families", "figure"];
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

/// Synthetic one-token-per-row model over the closed seven-label dictionary
/// that forces `labels[row]` at row `row`.
fn model_emitting(labels: &[&str]) -> (Vec<FulltextFeatureRow>, CrfModel) {
    let rows = (0..labels.len())
        .map(|index| row(&format!("t{index}")))
        .collect::<Vec<_>>();
    let vocabulary: Vec<String> = NATIVE_FIGURE_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    let label_count = vocabulary.len();
    let observations = (0..labels.len())
        .map(|index| format!("u:t{index}"))
        .collect::<Vec<_>>();
    let mut u_offsets = Vec::with_capacity(observations.len());
    let mut weights = BTreeMap::new();
    for (position, label) in labels.iter().enumerate() {
        let offset = position * label_count;
        u_offsets.push(Some(offset));
        let target = NATIVE_FIGURE_LABELS
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

fn decoded(labels: &[&str]) -> NativeFigureDecodeOutcome {
    let (rows, model) = model_emitting(labels);
    decode_figure_sequence(&rows, &model, &[]).expect("synthetic sequence must decode")
}

fn span(labels: &[&str]) -> FigureSpan {
    match decoded(labels) {
        NativeFigureDecodeOutcome::Decoded { spans, .. } => {
            spans.into_iter().next().expect("one span per decode")
        }
        other => panic!("a figure sequence must decode, got {other:?}"),
    }
}

fn marker(digest_char: char, cluster_index: usize) -> NativeCaptionMarker {
    NativeCaptionMarker {
        digest: digest_char.to_string().repeat(64),
        cluster_index,
    }
}

// ─────────────────── DATA ↔ Rust binding (1:1, no drift) ───────────────────

#[test]
fn the_rust_dictionary_is_the_data_transcription() {
    let declared = strings(&data(), &["families", "figure", "declared_labels"]);
    assert_eq!(
        declared, NATIVE_FIGURE_LABELS,
        "families.figure.declared_labels must mirror NATIVE_FIGURE_LABELS 1:1, in model order"
    );
    assert_eq!(
        figure(&["declared_label_count"]).as_u64(),
        Some(NATIVE_FIGURE_LABELS.len() as u64)
    );
    assert_eq!(
        NATIVE_FIGURE_LABELS.len(),
        7,
        "the frozen model declares 7 labels"
    );

    let bases = strings(&data(), &["families", "figure", "base_labels"]);
    let mut expected = NATIVE_FIGURE_BASE_LABELS.to_vec();
    expected.sort_unstable();
    let mut observed = bases;
    observed.sort_unstable();
    assert_eq!(observed, expected, "the four base roles must mirror 1:1");

    for label in NATIVE_FIGURE_LABELS {
        assert!(figure_label_is_admissible(label));
    }
    assert!(!figure_label_is_admissible("<other>"));
    assert!(
        !NATIVE_FIGURE_LABELS.contains(&"<label>"),
        "the frozen block declares only I-<label>, never the bare spelling"
    );
}

#[test]
fn the_printed_width_and_the_referenced_window_mirror_the_data() {
    assert_eq!(
        text(&data(), &["families", "figure", "model_file"]),
        "figure/model.wapiti"
    );
    assert_eq!(
        text(&data(), &["families", "figure", "declared_checkpoint"]),
        FIGURE_CHECKPOINT
    );
    assert_eq!(
        text(&data(), &["families", "figure", "declared_rdr"]),
        FIGURE_RDR
    );
    assert_eq!(
        text(&data(), &["families", "figure", "kind"]),
        "crf_sequence"
    );

    let emitted = figure(&["emitted_columns"])
        .as_u64()
        .expect("emitted_columns");
    assert_eq!(emitted as usize, NATIVE_FIGURE_EMITTED_COLUMNS);
    assert_eq!(
        NATIVE_FIGURE_EMITTED_COLUMNS,
        NATIVE_FULLTEXT_FEATURE_COLUMNS
    );
    assert_eq!(
        emitted as usize, 27,
        "the figure family reads the S05 fulltext 27-column ABI, not a second ABI"
    );

    let referenced = figure(&["referenced_columns"])
        .as_u64()
        .expect("referenced_columns");
    assert_eq!(
        referenced as usize,
        NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX + 1,
        "the Rust window constant must be the DATA referenced-column count minus one"
    );
    let excludes: Vec<u64> = figure(&["template_excludes"])
        .as_sequence()
        .expect("template_excludes")
        .iter()
        .map(|item| item.as_u64().expect("a column index"))
        .collect();
    assert_eq!(excludes, FIGURE_TEMPLATE_EXCLUDES);
    assert!(
        excludes.contains(&(referenced)),
        "the excluded column is the one past the window"
    );

    // `#rdr#85/25/0` — the second field is the highest referenced column index.
    let window = FIGURE_RDR
        .trim_start_matches("#rdr#")
        .split('/')
        .nth(1)
        .and_then(|field| field.parse::<usize>().ok());
    assert_eq!(window, Some(NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX));

    let not_in_model = strings(
        &data(),
        &["families", "figure", "java_declared_not_in_model"],
    );
    assert_eq!(not_in_model, ["<other>"]);
    for label in not_in_model {
        assert!(
            !figure_label_is_admissible(&label),
            "{label} is declared by Java but absent from the frozen model block"
        );
    }

    assert_eq!(
        NATIVE_FIGURE_SEQUENCE_SCHEMA_VERSION,
        "da-parse.native-figure-sequence/v1"
    );
}

#[test]
fn the_figure_lane_stays_shadow_locked() {
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
            decode_figure_sequence(&[bad], &model, &[]),
            Err(NativeFigureSequenceError::WrongArity {
                row: 0,
                actual: width,
                expected: NATIVE_FIGURE_EMITTED_COLUMNS,
            })
        );
    }

    assert_eq!(
        decode_figure_sequence(&[], &model, &[]),
        Err(NativeFigureSequenceError::EmptySequence)
    );
}

#[test]
fn refuses_a_foreign_label_block_with_both_directions_reported() {
    let (rows, mut model) = model_emitting(&["<content>"]);
    model.labels.pop();
    assert_eq!(
        check_figure_label_block(&model),
        Err(NativeFigureSequenceError::ModelLabelCount {
            expected: 7,
            actual: 6,
        })
    );
    assert_eq!(
        decode_figure_sequence(&rows, &model, &[]),
        Err(NativeFigureSequenceError::ModelLabelCount {
            expected: 7,
            actual: 6,
        })
    );

    let (rows, mut model) = model_emitting(&["<content>"]);
    let index = model
        .labels
        .iter()
        .position(|label| label == "<figDesc>")
        .expect("the dictionary carries <figDesc>");
    model.labels[index] = "<other>".into();
    assert_eq!(model.labels.len(), 7);
    assert_eq!(
        check_figure_label_block(&model),
        Err(NativeFigureSequenceError::ModelLabelMismatch {
            missing: vec!["<figDesc>".to_string()],
            unexpected: vec!["<other>".to_string()],
        })
    );
    assert_eq!(
        decode_figure_sequence(&rows, &model, &[]),
        Err(NativeFigureSequenceError::ModelLabelMismatch {
            missing: vec!["<figDesc>".to_string()],
            unexpected: vec!["<other>".to_string()],
        })
    );

    // A same-sized block in another order still declares the same set: the check
    // is order-independent because the loaded model owns the label ids.
    let (_, mut reordered) = model_emitting(&["<content>"]);
    reordered.labels.reverse();
    assert_eq!(check_figure_label_block(&reordered), Ok(()));
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
            check_figure_column_window(&model),
            Err(NativeFigureSequenceError::DeclaredColumnMismatch {
                declared: Some(declared),
                expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
            })
        );
        assert_eq!(
            decode_figure_sequence(&rows, &model, &[]),
            Err(NativeFigureSequenceError::DeclaredColumnMismatch {
                declared: Some(declared),
                expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
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
    let rendered = render_fulltext_feature_text(&rows).unwrap();
    assert_eq!(da_ports::map_input(&model, &rendered).unwrap().len(), 1);
    assert!(check_figure_column_window(&model).is_err());

    // A model that references no column at all cannot be the frozen model.
    let (_, mut model) = model_emitting(&["<content>"]);
    model.patterns = vec![CrfPattern {
        source: "u:literal".into(),
        kind: 'u',
    }];
    assert_eq!(
        check_figure_column_window(&model),
        Err(NativeFigureSequenceError::DeclaredColumnMismatch {
            declared: None,
            expected: NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
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
                check_figure_column_window(&model),
                Err(NativeFigureSequenceError::Model { .. })
            ),
            "{source} must be a typed model refusal"
        );
    }
}

/// The D359 divergence, witnessed through the public surface: a caption marker
/// can address cluster 1 of `I-<content>,<content>` only because the `I-` prefix
/// opened a second cluster there, while the same marker against
/// `<content>,<content>` addresses a cluster that does not exist.
#[test]
fn the_inside_spelling_opens_a_cluster_and_a_bare_repeat_does_not() {
    let (rows, model) = model_emitting(&["I-<content>", "<content>"]);
    assert!(decode_figure_sequence(&rows, &model, &[marker('a', 1)]).is_ok());
    assert_eq!(
        decode_figure_sequence(&rows, &model, &[marker('a', 2)]),
        Err(NativeFigureSequenceError::DanglingCaptionCluster {
            cluster_index: 2,
            clusters: 2,
        })
    );

    let (rows, model) = model_emitting(&["<content>", "<content>"]);
    assert_eq!(
        decode_figure_sequence(&rows, &model, &[marker('a', 1)]),
        Err(NativeFigureSequenceError::DanglingCaptionCluster {
            cluster_index: 1,
            clusters: 1,
        }),
        "identical bare spellings merge into one cluster"
    );
    // Both tokens of the merged cluster are still carried, exactly once.
    assert_eq!(
        span(&["<content>", "<content>"]).content.as_deref(),
        Some("t0 t1")
    );
}

#[test]
fn every_role_maps_to_its_own_field_and_absent_roles_stay_none() {
    let filled = span(&["I-<label>", "<figure_head>", "<figDesc>", "<content>"]);
    assert_eq!(filled.label.as_deref(), Some("t0"));
    assert_eq!(filled.head.as_deref(), Some("t1"));
    assert_eq!(filled.caption.as_deref(), Some("t2"));
    assert_eq!(filled.content.as_deref(), Some("t3"));
    assert_eq!(filled.cluster_index, 0);

    let partial = span(&["<figDesc>"]);
    assert_eq!(partial.caption.as_deref(), Some("t0"));
    assert_eq!(partial.label, None);
    assert_eq!(partial.head, None);
    assert_eq!(partial.content, None);

    // A role split across two clusters keeps both parts instead of dropping one.
    let rejoined = span(&["<content>", "<figDesc>", "<content>"]);
    assert_eq!(rejoined.content.as_deref(), Some("t0 t2"));
    assert_eq!(rejoined.caption.as_deref(), Some("t1"));
}

#[test]
fn caption_links_are_provided_or_not_observed_and_never_silently_invalid() {
    let inventory = [marker('a', 0), marker('b', 1)];
    let (rows, model) = model_emitting(&["<content>", "<figDesc>"]);
    let provided = match decode_figure_sequence(&rows, &model, &inventory).unwrap() {
        NativeFigureDecodeOutcome::Decoded { mut spans, .. } => {
            assert_eq!(spans.len(), 1, "one span per decode");
            spans.remove(0)
        }
        other => panic!("expected a decode, got {other:?}"),
    };
    assert_eq!(provided.caption_links.absence, None);
    assert_eq!(
        provided
            .caption_links
            .markers
            .iter()
            .map(|marker| marker.digest.clone())
            .collect::<Vec<_>>(),
        [marker('a', 0).digest, marker('b', 1).digest],
        "the caller's marker order is preserved"
    );

    // H12-N1: an empty inventory is an absence, never "zero links" as a success.
    assert_eq!(
        span(&["<content>", "<figDesc>"]).caption_links,
        NativeCaptionLinks {
            markers: Vec::new(),
            absence: Some(NativeCaptionLinkAbsence::NotObserved),
        }
    );

    // A present-but-malformed inventory must not decay into an absence.
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
            decode_figure_sequence(&rows, &model, &[bad]),
            Err(NativeFigureSequenceError::InvalidCaptionInventory)
        );
    }
    let duplicated = [marker('c', 0), marker('c', 1)];
    assert_eq!(
        decode_figure_sequence(&rows, &model, &duplicated),
        Err(NativeFigureSequenceError::InvalidCaptionInventory)
    );
}

#[test]
fn an_empty_decode_is_not_observed_rather_than_zero_figures() {
    assert_eq!(
        figure_spans_from_decoded(&[], &[], &[]),
        Ok(NativeFigureDecodeOutcome::NotObserved {
            reason: NativeFigureAbsence::NoFigureClusters,
        })
    );

    let (rows, _) = model_emitting(&["<content>"]);
    assert_eq!(
        figure_spans_from_decoded(&["<other>".to_string()], &rows, &[]),
        Err(NativeFigureSequenceError::UnmodeledLabel {
            label: "<other>".to_string(),
            row: 0,
        })
    );
}

/// The cap is inclusive and refuses rather than truncating: a reader must never
/// mistake a silently partial decode for a complete one.
#[test]
fn the_row_cap_refuses_rather_than_truncating() {
    let (_, model) = model_emitting(&["<content>"]);

    let at_cap = vec![row("t0"); MAX_NATIVE_FIGURE_ROWS];
    match decode_figure_sequence(&at_cap, &model, &[]) {
        Ok(NativeFigureDecodeOutcome::Decoded { labels, spans }) => {
            assert_eq!(labels.len(), MAX_NATIVE_FIGURE_ROWS);
            assert_eq!(spans.len(), 1);
        }
        other => panic!("exactly the cap must decode, got {other:?}"),
    }

    let over_cap = vec![row("t0"); MAX_NATIVE_FIGURE_ROWS + 1];
    assert_eq!(
        decode_figure_sequence(&over_cap, &model, &[]),
        Err(NativeFigureSequenceError::TooManyRows {
            actual: MAX_NATIVE_FIGURE_ROWS + 1,
            maximum: MAX_NATIVE_FIGURE_ROWS,
        })
    );
}

#[test]
fn the_rerun_is_byte_identical() {
    let (rows, model) = model_emitting(&["<content>", "I-<figDesc>", "I-<label>"]);
    let inventory = [marker('e', 0)];
    let first = decode_figure_sequence(&rows, &model, &inventory).unwrap();
    let second = decode_figure_sequence(&rows, &model, &inventory).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn the_span_projection_is_stable() {
    let absent = span(&["<content>"]);
    assert_eq!(
        serde_json::to_string(&absent).unwrap(),
        concat!(
            r#"{"label":null,"head":null,"caption":null,"content":"t0","#,
            r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}"#
        )
    );
    let round_tripped: FigureSpan =
        serde_json::from_str(&serde_json::to_string(&absent).unwrap()).expect("round-trips");
    assert_eq!(round_tripped, absent);

    assert_eq!(
        serde_json::to_string(&decoded(&["<content>", "<figDesc>"])).unwrap(),
        concat!(
            r#"{"outcome":"decoded","labels":["<content>","<figDesc>"],"spans":["#,
            r#"{"label":null,"head":null,"caption":"t1","content":"t0","#,
            r#""caption_links":{"markers":[],"absence":"not_observed"},"cluster_index":0}]}"#
        )
    );
}
