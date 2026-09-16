//! (R131) Contract for the native GROBID body/fulltext 27-column feature ABI.
//!
//! This file owns the *ABI* section of the S05 body band: the printed column
//! count, the exact `FeaturesVectorFulltext.printVector` column order, the two
//! stateful font lanes, the `ALLDIGIT → NOCAPS` printed override, the typed
//! refusals, and the fail-closed model-declaration check.
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
