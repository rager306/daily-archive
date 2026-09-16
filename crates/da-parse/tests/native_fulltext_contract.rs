//! (R131) Contract for the native GROBID body/fulltext 27-column feature ABI.
//!
//! This file owns the *ABI* section of the S05 body band: the printed column
//! count, the exact `FeaturesVectorFulltext.printVector` column order, the two
//! stateful font lanes, the `ALLDIGIT → NOCAPS` printed override, the typed
//! refusals, and the fail-closed model-declaration check. Since S05/T02 it also
//! owns the *sequence* section: the closed 24-label dictionary, the D359
//! base-name grouping and the synthetic-model decode fixture.
//!
//! Expectations here are hand-derived from the vendored GROBID source
//! (`FeaturesVectorFulltext.printVector` plus the `features.*` assignment block
//! in `FullTextParser.getBodyTextFeatured`), never from the module under test:
//! the row is reconstructed field by field in Java order and compared by index,
//! so a re-ordered or widened row fails rather than passing by self-agreement.
//! The independent same-n oracle and the opt-in real-model harness are separate
//! artifacts.
//!
//! Boundary statement: every symbol exercised here is a pure renderer. No
//! filesystem, environment, clock, model bytes, or production route is touched,
//! and no atom of this band is spliced into the Java-authoritative parse path.

use da_parse::*;
use da_ports::CrfModel;
use std::path::{Path, PathBuf};

fn token(text: &str) -> FulltextFeatureToken {
    FulltextFeatureToken {
        text: text.into(),
        block_status: FactsBlockStatus::Blockin,
        line_status: FactsLineStatus::Linein,
        alignment: FulltextAlignment::Alignedleft,
        font: "Times-Roman".into(),
        font_size: 10.0,
        bold: false,
        italic: false,
        document_position: 0,
        document_length: 8,
        page_line_position: 0.0,
        page_height: 792.0,
        bitmap_around: false,
        callout_type: FulltextCalloutType::Unknown,
        callout_known: false,
        superscript: false,
    }
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

/// The printed row width is the number of fields `printVector` appends, and the
/// highest reachable column index is that width minus one.
#[test]
fn abi_surface_is_exactly_27_columns() {
    assert_eq!(NATIVE_FULLTEXT_FEATURE_COLUMNS, 27);
    assert_eq!(NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX, 26);
    assert_eq!(
        NATIVE_FULLTEXT_FEATURE_SCHEMA_VERSION,
        "da-parse.native-fulltext-features/v1"
    );
    assert_eq!(NATIVE_FULLTEXT_RELATIVE_POSITION_BINS, 12);
    assert_eq!(check_fulltext_feature_columns(27), Ok(()));
}

/// Reconstruct the Java row by hand, in `printVector` order, and compare every
/// one of the 27 indices.
///
/// The token is chosen so that every lane carries a *distinct*, non-default
/// value: a transposition, an inserted lane, or a dropped lane cannot pass.
#[test]
fn abi_column_order_is_the_print_vector_order() {
    let mut input = token("Vaswani");
    input.block_status = FactsBlockStatus::Blockstart;
    input.line_status = FactsLineStatus::Lineend;
    input.alignment = FulltextAlignment::Lineindent;
    input.font = "Nimbus".into();
    input.font_size = 14.0;
    input.bold = true;
    input.italic = true;
    input.document_position = 4;
    input.document_length = 8;
    input.page_line_position = 396.0;
    input.page_height = 792.0;
    input.bitmap_around = true;
    input.callout_type = FulltextCalloutType::Number;
    input.callout_known = true;
    input.superscript = true;

    let rows = render_fulltext_features(&[input.clone()]).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.width(), 27);

    // Hand-built Java expectation, index by index.
    let expected = vec![
        "Vaswani",    // 0  string
        "vaswani",    // 1  lower
        "V",          // 2  prefix 1
        "Va",         // 3  prefix 2
        "Vas",        // 4  prefix 3
        "Vasw",       // 5  prefix 4
        "i",          // 6  suffix 1
        "ni",         // 7  suffix 2
        "ani",        // 8  suffix 3
        "wani",       // 9  suffix 4
        "BLOCKSTART", // 10 blockStatus
        "LINEEND",    // 11 lineStatus
        "LINEINDENT", // 12 alignmentStatus
        "NEWFONT",    // 13 fontStatus
        "HIGHERFONT", // 14 fontSize
        "1",          // 15 bold
        "1",          // 16 italic
        "INITCAP",    // 17 capitalisation
        "NODIGIT",    // 18 digit
        "0",          // 19 singleChar
        "NOPUNCT",    // 20 punctType
        "6",          // 21 relativeDocumentPosition (4/8 * 12)
        "6",          // 22 relativePagePosition (396/792 * 12)
        "1",          // 23 bitmapAround
        "NUMBER",     // 24 calloutType
        "1",          // 25 calloutKnown
        "1",          // 26 superscript
    ];
    assert_eq!(row.columns, expected);
    roundtrip(&input);
    roundtrip(row);
}

/// `vectorAround`, `spacingWithPreviousBlock`, `characterDensity`, the
/// training-label placeholder and `relativePagePositionChar` are not emitted.
///
/// The assertion is structural: if any of them had been inserted, the last four
/// lanes would not sit at exactly `23..=26`, and the row would be wider than 27.
#[test]
fn abi_omits_the_commented_out_and_unprinted_lanes() {
    let mut input = token("x");
    input.bitmap_around = true;
    input.callout_type = FulltextCalloutType::Author;
    input.callout_known = true;
    input.superscript = true;
    let row = render_fulltext_features(&[input]).unwrap().remove(0);
    assert_eq!(row.columns[23..].to_vec(), vec!["1", "AUTHOR", "1", "1"]);
    assert_eq!(row.width(), 27);
    // The default callout lane prints the Java default, never a null.
    let unknown = render_fulltext_features(&[token("x")]).unwrap().remove(0);
    assert_eq!(unknown.columns[24], "UNKNOWN");
    assert_eq!(unknown.columns[23], "0");
}

/// `digit == ALLDIGIT` overrides the printed capitalisation with `NOCAPS`,
/// while the `digit` lane itself stays `ALLDIGIT`.
#[test]
fn alldigit_prints_nocaps_and_keeps_the_alldigit_lane() {
    let rows = render_fulltext_features(&[token("2017"), token("2nd"), token("Word")]).unwrap();
    assert_eq!(rows[0].columns[17], "NOCAPS");
    assert_eq!(rows[0].columns[18], "ALLDIGIT");
    assert_eq!(rows[1].columns[17], "NOCAPS");
    assert_eq!(rows[1].columns[18], "CONTAINSDIGITS");
    assert_eq!(rows[2].columns[17], "INITCAP");
    assert_eq!(rows[2].columns[18], "NODIGIT");
    // Pure punctuation has no lowercase character, so the ladder says ALLCAP and
    // the printed value is not overridden because its digit lane is NODIGIT.
    let dotted = render_fulltext_features(&[token(".")]).unwrap().remove(0);
    assert_eq!(dotted.columns[17], "ALLCAP");
    assert_eq!(dotted.columns[20], "DOT");
}

/// The two font lanes are stateful: they compare against the previous admitted
/// token, and `fontSize` compares the *truncated* size.
#[test]
fn font_lanes_track_the_previous_token() {
    let mut first = token("A");
    first.font = "Times".into();
    first.font_size = 10.0;
    let mut second = token("B");
    second.font = "Times".into();
    second.font_size = 10.4; // (int) 10.4 == 10
    let mut third = token("C");
    third.font = "Helvetica".into();
    third.font_size = 8.0;
    let mut fourth = token("D");
    fourth.font = "Helvetica".into();
    fourth.font_size = 12.0;
    let rows = render_fulltext_features(&[first, second, third, fourth]).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.columns[13].as_str())
            .collect::<Vec<_>>(),
        ["NEWFONT", "SAMEFONT", "NEWFONT", "SAMEFONT"]
    );
    assert_eq!(
        rows.iter()
            .map(|r| r.columns[14].as_str())
            .collect::<Vec<_>>(),
        ["HIGHERFONT", "SAMEFONTSIZE", "LOWERFONT", "HIGHERFONT"]
    );
}

/// Multi-byte and astral-plane tokens render without panicking, and the
/// character-based prefix/suffix lanes stay inside the token.
#[test]
fn multibyte_tokens_render_without_panicking() {
    let rows = render_fulltext_features(&[token("日本語"), token("🙂"), token("٣٤")]).unwrap();
    assert_eq!(rows[0].columns[1], "日本語");
    assert_eq!(rows[0].columns[2], "日");
    // `TextUtilities.suffix` returns the whole string when it is shorter than
    // the count, so the 4-character lane carries the 3-character token.
    assert_eq!(rows[0].columns[9], "日本語");
    assert_eq!(rows[0].columns[19], "0");
    // One astral-plane scalar: the single-character lane fires.
    assert_eq!(rows[1].columns[19], "1");
    assert_eq!(rows[2].columns[18], "CONTAINSDIGITS");
    for row in &rows {
        assert_eq!(row.width(), 27);
    }
    // Serialization keeps one line per row and exactly 27 fields per line.
    let text = render_fulltext_feature_text(&rows).unwrap();
    assert_eq!(text.lines().count(), 3);
    for line in text.lines() {
        assert_eq!(line.split(' ').count(), 27);
    }
}

/// Every refusal is typed, not a panic and not a silent approximation.
#[test]
fn malformed_inputs_are_refused_with_typed_errors() {
    assert!(matches!(
        render_fulltext_features(&[]),
        Err(NativeFulltextFeatureError::EmptyInput)
    ));
    assert!(matches!(
        render_fulltext_features(&[token("")]),
        Err(NativeFulltextFeatureError::EmptyToken { token: 0 })
    ));
    // A smuggled separator would split one field into two and shift the grid.
    for smuggled in [" ", "\t", "\n", "a b", "x\ry", "\u{a0}"] {
        assert!(
            matches!(
                render_fulltext_features(&[token(smuggled)]),
                Err(NativeFulltextFeatureError::TokenContainsWhitespace { token: 0 })
            ),
            "token {smuggled:?} must be refused"
        );
    }
    let mut bad_font = token("x");
    bad_font.font_size = f64::NAN;
    assert!(matches!(
        render_fulltext_features(&[bad_font]),
        Err(NativeFulltextFeatureError::NonFiniteFont { token: 0 })
    ));
    let mut bad_page = token("x");
    bad_page.page_height = f64::INFINITY;
    assert!(matches!(
        render_fulltext_features(&[bad_page]),
        Err(NativeFulltextFeatureError::NonFinitePageGeometry { token: 0 })
    ));
    let mut bad_line = token("x");
    bad_line.page_line_position = f64::NAN;
    assert!(matches!(
        render_fulltext_features(&[bad_line]),
        Err(NativeFulltextFeatureError::NonFinitePageGeometry { token: 0 })
    ));
    for field in ["document_position", "document_length"] {
        let mut negative = token("x");
        if field == "document_position" {
            negative.document_position = -1;
        } else {
            negative.document_length = -1;
        }
        assert!(matches!(
            render_fulltext_features(&[negative]),
            Err(NativeFulltextFeatureError::NegativePosition { token: 0, field: _ })
        ));
    }
    // A row below the printed width is refused by the row serializer.
    let short = FulltextFeatureRow {
        columns: vec!["a".into(); 26],
    };
    assert!(matches!(
        render_fulltext_feature_text(&[short]),
        Err(NativeFulltextFeatureError::WrongArity {
            actual: 26,
            expected: 27
        })
    ));
    // A model declaring a different observation width is fail-closed.
    for declared in [0, 26, 28, 29] {
        assert!(
            matches!(
                check_fulltext_feature_columns(declared),
                Err(NativeFulltextFeatureError::DeclaredColumnMismatch {
                    declared: d,
                    expected: 27
                }) if d == declared
            ),
            "declared {declared} must fail closed"
        );
    }
}

/// The row bound is fail-closed: an over-long input is refused, not truncated.
#[test]
fn oversized_input_is_refused_rather_than_truncated() {
    let admitted = vec![token("a"); MAX_NATIVE_FULLTEXT_FEATURE_ROWS];
    let rows = render_fulltext_features(&admitted).expect("the exact bound is admitted");
    assert_eq!(rows.len(), MAX_NATIVE_FULLTEXT_FEATURE_ROWS);
    let over = vec![token("a"); MAX_NATIVE_FULLTEXT_FEATURE_ROWS + 1];
    assert!(matches!(
        render_fulltext_features(&over),
        Err(NativeFulltextFeatureError::TooManyRows { rows: r, limit: l })
            if r == MAX_NATIVE_FULLTEXT_FEATURE_ROWS + 1 && l == MAX_NATIVE_FULLTEXT_FEATURE_ROWS
    ));
}

/// The position helpers are the fulltext scale (`NBBINS_POSITION = 12`) and are
/// bounded and monotonic; the page lane resolves the `double` overload.
#[test]
fn position_helpers_stay_inside_the_declared_scale() {
    assert_eq!(relative_position_fulltext_document(4, 8), 6);
    assert_eq!(relative_position_fulltext_document(8, 8), 12);
    assert_eq!(relative_position_fulltext_document(0, 0), 12);
    assert_eq!(relative_position_fulltext_page(396.0, 792.0), 6);
    assert_eq!(relative_position_fulltext_page(792.0, 792.0), 12);
    assert_eq!(relative_position_fulltext_page(0.0, 792.0), 0);
    // Out-of-range geometry saturates rather than escaping the scale.
    assert_eq!(relative_position_fulltext_page(1.0e9, 792.0), 12);
    assert_eq!(relative_position_fulltext_page(-5.0, 792.0), 0);
    for position in 0..64i32 {
        for total in 0..64i32 {
            let value = relative_position_fulltext_document(position, total);
            assert!((0..=NATIVE_FULLTEXT_RELATIVE_POSITION_BINS).contains(&value));
            assert!(value <= relative_position_fulltext_document(position + 1, total));
        }
        for step in 0..64i32 {
            let y = step as f64 * 16.0;
            let value = relative_position_fulltext_page(y, 792.0);
            assert!((0..=NATIVE_FULLTEXT_RELATIVE_POSITION_BINS).contains(&value));
        }
    }
}

/// The typed inputs serialize with the exact spellings the row prints.
///
/// The model consumes these strings, so a serde rename would silently change
/// the ABI without changing the Rust type; the spelling is pinned here.
#[test]
fn typed_inputs_keep_their_wire_spelling() {
    for (alignment, expected) in [
        (FulltextAlignment::Alignedleft, "ALIGNEDLEFT"),
        (FulltextAlignment::Lineindent, "LINEINDENT"),
        (FulltextAlignment::Centered, "CENTERED"),
    ] {
        assert_eq!(alignment.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&alignment).unwrap(),
            format!("\"{expected}\"")
        );
        roundtrip(&alignment);
    }
    for (status, expected) in [
        (FulltextFontStatus::Newfont, "NEWFONT"),
        (FulltextFontStatus::Samefont, "SAMEFONT"),
    ] {
        assert_eq!(status.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{expected}\"")
        );
        roundtrip(&status);
    }
    for (size, expected) in [
        (FulltextFontSize::Higherfont, "HIGHERFONT"),
        (FulltextFontSize::Samefontsize, "SAMEFONTSIZE"),
        (FulltextFontSize::Lowerfont, "LOWERFONT"),
    ] {
        assert_eq!(size.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&size).unwrap(),
            format!("\"{expected}\"")
        );
        roundtrip(&size);
    }
    for (callout, expected) in [
        (FulltextCalloutType::Unknown, "UNKNOWN"),
        (FulltextCalloutType::Number, "NUMBER"),
        (FulltextCalloutType::Author, "AUTHOR"),
    ] {
        assert_eq!(callout.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&callout).unwrap(),
            format!("\"{expected}\"")
        );
        roundtrip(&callout);
    }
    assert_eq!(FulltextCalloutType::default(), FulltextCalloutType::Unknown);
}

/// Rendering is a pure function of the admitted sequence.
#[test]
fn rendering_is_deterministic_for_the_same_input() {
    let tokens = [token("Body"), token("text"), token(",")];
    let first = render_fulltext_features(&tokens).unwrap();
    let second = render_fulltext_features(&tokens).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        render_fulltext_feature_text(&first).unwrap(),
        render_fulltext_feature_text(&second).unwrap()
    );
    let text = render_fulltext_feature_text(&first).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(!text.ends_with('\n'));
}

// ───────────────────────── body sequence (T02) ─────────────────────────
//
// The synthetic model fixture lives beside the rows it decodes, is hand-written
// in the Wapiti text format the production reader accepts, and carries no GROBID
// bytes (D358). Every weight index in it is derived by hand below; the decode
// itself only groups base names, and that grouping is the shared D359 primitive
// the header, bibliography and nested families already use.

const FULLTEXT_SEQUENCE_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_fulltext_synthetic.wapiti";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read the synthetic model fixture and parse it with the production model
/// reader. A missing or malformed fixture is FATAL: the contract must never
/// silently shrink.
fn load_fulltext_model() -> (Vec<u8>, CrfModel) {
    let bytes = std::fs::read(repo_root().join(FULLTEXT_SEQUENCE_FIXTURE))
        .unwrap_or_else(|error| panic!("FATAL {FULLTEXT_SEQUENCE_FIXTURE}: {error}"));
    let model = CrfModel::parse(&bytes)
        .unwrap_or_else(|error| panic!("FATAL {FULLTEXT_SEQUENCE_FIXTURE}: {error}"));
    (bytes, model)
}

/// Render one body token per text through the T01 row builder, so the sequence
/// section consumes exactly the 27-column ABI the row builder prints rather than
/// a hand-made row that could agree with the decoder while disagreeing with the
/// ABI.
fn body_rows(tokens: &[&str]) -> Vec<FulltextFeatureRow> {
    let inputs: Vec<FulltextFeatureToken> = tokens.iter().map(|text| token(text)).collect();
    render_fulltext_features(&inputs).expect("the T01 row builder renders body tokens")
}

fn decode_body(tokens: &[&str]) -> NativeFulltextDecodeCandidate {
    let (_, model) = load_fulltext_model();
    match decode_fulltext_sequence(&body_rows(tokens), &model) {
        NativeFulltextDecodeOutcome::Accepted { decode } => decode,
        NativeFulltextDecodeOutcome::Rejected { error } => {
            panic!("body tokens {tokens:?} must decode, got {error}")
        }
    }
}

fn body_runs(tokens: &[&str]) -> Vec<(String, usize, usize)> {
    decode_body(tokens)
        .runs
        .into_iter()
        .map(|run| (run.label, run.start, run.end))
        .collect()
}

fn run(label: &str, start: usize, end: usize) -> (String, usize, usize) {
    (label.to_owned(), start, end)
}

/// The fixture declares the transcribed 24-label block — both spellings of the
/// twelve ledger base labels — over the same 27-column ABI column 0 the row
/// builder prints, and stays a small hand-authored text model.
#[test]
fn fulltext_sequence_fixture_declares_the_closed_24_label_block() {
    let (bytes, model) = load_fulltext_model();
    assert_eq!(model.model_type, 2, "scalar CRF model type");
    assert_eq!(model.labels, NATIVE_FULLTEXT_LABELS);
    assert_eq!(model.labels.len(), 24);
    assert_eq!(NATIVE_FULLTEXT_BASE_LABELS.len(), 12);
    assert_eq!(model.patterns.len(), 1);
    assert_eq!(model.patterns[0].source, "u:%x[0,0]");
    assert_eq!(check_fulltext_label_block(&model), Ok(()));
    assert!(
        bytes.len() < 2_048,
        "hand-authored, not a redistributed model"
    );
    assert!(
        std::str::from_utf8(&bytes).is_ok(),
        "the fixture stays text"
    );
    assert_eq!(
        NATIVE_FULLTEXT_SEQUENCE_SCHEMA_VERSION,
        "da-parse.native-fulltext-sequence/v1"
    );
    assert_eq!(MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS, 32_768);
}

/// The dictionary is derived from the DATA ledger, not from the Rust table: the
/// twelve base labels are exactly `fulltext_labels.labels` and the closed
/// vocabulary is exactly both spellings of each, with no extra and no missing
/// label.
#[test]
fn fulltext_sequence_dictionary_is_the_data_ledger() {
    let raw = std::fs::read_to_string(repo_root().join("data/parse/native_body_accounting.yaml"))
        .expect("FATAL the body accounting ledger must be readable");
    let ledger: serde_yaml::Value =
        serde_yaml::from_str(&raw).expect("FATAL the body accounting ledger must parse");
    let fulltext = &ledger["fulltext_labels"];
    assert_eq!(fulltext["closed"].as_bool(), Some(true));
    assert_eq!(
        fulltext["unknown_label_behavior"].as_str(),
        Some("fail_closed")
    );
    let mut declared: Vec<String> = fulltext["labels"]
        .as_mapping()
        .expect("fulltext_labels.labels must be a mapping")
        .values()
        .map(|value| {
            value
                .as_str()
                .expect("every ledger label must be a string")
                .to_owned()
        })
        .collect();
    declared.sort_unstable();
    let mut native: Vec<String> = NATIVE_FULLTEXT_BASE_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    native.sort_unstable();
    assert_eq!(declared, native, "base labels drifted from the ledger");
    assert_eq!(declared.len(), 12);

    // Both spellings of the ledger's bases are exactly the closed vocabulary.
    let mut expected: Vec<String> = NATIVE_FULLTEXT_BASE_LABELS
        .iter()
        .flat_map(|base| [(*base).to_owned(), format!("I-{base}")])
        .collect();
    expected.sort_unstable();
    let mut closed: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    closed.sort_unstable();
    assert_eq!(closed, expected);
    assert_eq!(closed.len(), 24);
}

/// Hand-counted weight derivation for the synthetic fixture.
///
/// The fixture declares 24 labels in `NATIVE_FULLTEXT_LABELS` order, so
/// observation *i* owns the block starting at `24 * i`, and the weight index is
/// `24 * i + label_id`:
///
/// | label | id | | label | id |
/// |-------|----|-|-------|----|
/// | `<paragraph>` | 0 | | `<figure>` | 14 |
/// | `I-<paragraph>` | 1 | | `I-<figure>` | 15 |
/// | `<item>` | 2 | | `<table>` | 16 |
/// | `I-<item>` | 3 | | `I-<table>` | 17 |
/// | `<section>` | 4 | | `<equation>` | 18 |
/// | `I-<section>` | 5 | | `I-<equation>` | 19 |
/// | `<citation_marker>` | 6 | | `<equation_label>` | 20 |
/// | `I-<citation_marker>` | 7 | | `I-<equation_label>` | 21 |
/// | `<figure_marker>` | 8 | | `<other>` | 22 |
/// | `I-<figure_marker>` | 9 | | `I-<other>` | 23 |
/// | `<table_marker>` | 10 | | | |
/// | `I-<table_marker>` | 11 | | | |
/// | `<equation_marker>` | 12 | | | |
/// | `I-<equation_marker>` | 13 | | | |
///
/// | row | token | label | index |
/// |-----|-------|-------|-------|
/// | 0 | alpha | `I-<paragraph>` = 1 | 1 |
/// | 1 | beta | `<paragraph>` = 0 | 24 |
/// | 2 | beta | `<paragraph>` = 0 | 48 |
/// | 1 | gamma | `I-<section>` = 5 | 53 |
/// | 2 | delta | `<section>` = 4 | 76 |
/// | 0 | zeta | `I-<other>` = 23 | 143 |
/// | 1 | eta | `<other>` = 22 | 166 |
#[test]
fn fulltext_sequence_groups_both_spellings_into_one_base_run() {
    // `I-` first, then two bare rows: one paragraph, exactly the shape the real
    // 0.9.0 family emits and the shape the mirror-image BIO rule rejected.
    let decode = decode_body(&["alpha", "beta", "beta"]);
    assert_eq!(
        decode.labels,
        ["I-<paragraph>", "<paragraph>", "<paragraph>"]
    );
    assert_eq!(
        decode.runs,
        [FulltextLabelRun {
            label: "<paragraph>".into(),
            start: 0,
            end: 3,
        }]
    );
    assert_eq!(
        body_runs(&["alpha", "beta", "beta"]),
        [run("<paragraph>", 0, 3)]
    );

    // Bare first, `I-` second: the same run, not a new one.
    let decode = decode_body(&["beta", "alpha"]);
    assert_eq!(decode.labels, ["<paragraph>", "I-<paragraph>"]);
    assert_eq!(body_runs(&["beta", "alpha"]), [run("<paragraph>", 0, 2)]);
}

#[test]
fn fulltext_sequence_base_change_closes_a_run_and_opens_the_next() {
    let decode = decode_body(&["beta", "gamma", "delta"]);
    assert_eq!(decode.labels, ["<paragraph>", "I-<section>", "<section>"]);
    assert_eq!(
        body_runs(&["beta", "gamma", "delta"]),
        [run("<paragraph>", 0, 1), run("<section>", 1, 3)]
    );
    // Runs tile the rows in reading order and carry the base name of their first
    // row, never the `I-` spelling.
    let mut cursor = 0usize;
    for run in &decode.runs {
        assert_eq!(run.start, cursor, "runs tile the rows in reading order");
        assert!(run.start < run.end);
        let first = decode.labels[run.start].as_str();
        assert_eq!(run.label, first.strip_prefix("I-").unwrap_or(first));
        assert!(!run.label.starts_with("I-"));
        cursor = run.end;
    }
    assert_eq!(cursor, decode.labels.len(), "the last run reaches the end");
}

/// The `<other>` family groups by the same rule: a base-name change is the only
/// thing that closes a run.
#[test]
fn fulltext_sequence_other_family_runs_group_by_base_name() {
    let decode = decode_body(&["zeta", "eta"]);
    assert_eq!(decode.labels, ["I-<other>", "<other>"]);
    assert_eq!(body_runs(&["zeta", "eta"]), [run("<other>", 0, 2)]);
}

/// Every refusal is typed, and the ones that must precede model work do: an
/// empty sequence and an over-wide row never reach the decoder.
#[test]
fn fulltext_sequence_refusals_are_typed() {
    let (_, model) = load_fulltext_model();
    assert!(matches!(
        decode_fulltext_sequence(&[], &model),
        NativeFulltextDecodeOutcome::Rejected {
            error: NativeFulltextSequenceError::EmptySequence
        }
    ));
    let short = FulltextFeatureRow {
        columns: vec!["a".into(); 26],
    };
    assert!(matches!(
        decode_fulltext_sequence(&[short], &model),
        NativeFulltextDecodeOutcome::Rejected {
            error: NativeFulltextSequenceError::WrongArity {
                row: 0,
                actual: 26,
                expected: 27
            }
        }
    ));
    // A column that smuggles a separator keeps the `' '`-field count but gives
    // the model's whitespace split one field more: refused, not shifted.
    let mut smuggled = body_rows(&["beta"]).remove(0);
    smuggled.columns[1] = "0\t0".into();
    assert!(matches!(
        decode_fulltext_sequence(&[smuggled], &model),
        NativeFulltextDecodeOutcome::Rejected {
            error: NativeFulltextSequenceError::WrongArity {
                row: 0,
                actual: 28,
                expected: 27
            }
        }
    ));
    // A model whose label block is not the native dictionary is fail-closed.
    let mut diverging = model.clone();
    diverging.labels.pop();
    assert!(matches!(
        decode_fulltext_sequence(&body_rows(&["beta"]), &diverging),
        NativeFulltextDecodeOutcome::Rejected {
            error: NativeFulltextSequenceError::ModelLabelCount {
                expected: 24,
                actual: 23
            }
        }
    ));
    let mut renamed = model.clone();
    let index = renamed
        .labels
        .iter()
        .position(|label| label == "<other>")
        .expect("the dictionary carries <other>");
    renamed.labels[index] = "<not-declared>".into();
    match decode_fulltext_sequence(&body_rows(&["beta"]), &renamed) {
        NativeFulltextDecodeOutcome::Rejected {
            error:
                NativeFulltextSequenceError::ModelLabelMismatch {
                    missing,
                    unexpected,
                },
        } => {
            assert_eq!(missing, ["<other>"]);
            assert_eq!(unexpected, ["<not-declared>"]);
        }
        other => panic!("a diverging label block must be refused, got {other:?}"),
    }
}

#[test]
fn fulltext_sequence_decode_is_deterministic() {
    let (_, model) = load_fulltext_model();
    let rows = body_rows(&["alpha", "beta", "gamma", "delta"]);
    assert_eq!(
        decode_fulltext_sequence(&rows, &model),
        decode_fulltext_sequence(&rows, &model)
    );
    assert_eq!(
        body_runs(&["alpha", "beta", "gamma"]),
        [run("<paragraph>", 0, 2), run("<section>", 2, 3),]
    );
}
