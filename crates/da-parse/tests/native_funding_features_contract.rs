//! S06/T05 contract for the native GROBID `funding` feature ABI.
//!
//! Three independent gates live here:
//!
//! 1. the **DATA binding** — `families.funding` in
//!    `data/parse/native_visual_families.yaml` must agree 1:1 with the Rust ABI
//!    constants (feature width, label slot index, label literal, lane names), so
//!    a transcription edit on either side fails here;
//! 2. the **ABI shape** — exactly 18 fields per row (17 printed features plus the
//!    dummy label slot), the column order of `FeaturesVectorFunding.printVector`
//!    pinned by index, and the `ALLDIGIT ⇒ NOCAPS` override;
//! 3. the **coverage semantics** — caller-supplied inclusive spans (D363), an
//!    empty coverage printing `0` without ever being presented as an observed
//!    negative, and typed refusals for an inverted span, an unbounded inventory,
//!    a blank token, a wrong declared width and an over-long sequence.
//!
//! The renderer needs no model and no lexicon: the two real gazetteer files stay
//! opt-in and unbundled (D358), and `the_module_carries_no_model_material_or_io`
//! pins that this file's SUT has no filesystem, environment, or model byte
//! surface at all.

use da_parse::{
    FundingCapitalisation, FundingDigit, FundingFeatureRow, FundingFeatureToken, FundingLineStatus,
    FundingPunctType, MAX_NATIVE_FUNDING_FEATURE_ROWS, MAX_NATIVE_FUNDING_GAZETTEER_SPANS,
    NATIVE_FUNDING_FEATURE_COLUMNS, NATIVE_FUNDING_FEATURES_SCHEMA_VERSION,
    NATIVE_FUNDING_LABEL_COLUMN_DUMMY, NATIVE_FUNDING_LABEL_COLUMN_INDEX,
    NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_ROW_COLUMNS, NativeFundingFeatureError,
    NativeFundingGazetteerCoverage, NativeFundingGazetteerLane, NativeFundingGazetteerSpan,
    check_funding_feature_columns, funding_span_covers, render_funding_feature_text,
    render_funding_features, validate_funding_gazetteer_coverage,
};
use std::path::PathBuf;

const DATA: &str = "data/parse/native_visual_families.yaml";
const SOURCE: &str = include_str!("../src/native_funding_features.rs");

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

fn number(value: &serde_yaml::Value, path: &[&str]) -> u64 {
    get(value, path)
        .as_u64()
        .unwrap_or_else(|| panic!("DATA key {path:?} is not an unsigned integer"))
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
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("DATA key {path:?} carries a non-string"))
                .to_string()
        })
        .collect()
}

fn token(text: &str) -> FundingFeatureToken {
    FundingFeatureToken {
        text: text.into(),
        line_status: FundingLineStatus::Linestart,
    }
}

fn coverage(
    funders: Vec<NativeFundingGazetteerSpan>,
    infrastructures: Vec<NativeFundingGazetteerSpan>,
) -> NativeFundingGazetteerCoverage {
    NativeFundingGazetteerCoverage {
        funders,
        infrastructures,
    }
}

fn empty_coverage() -> NativeFundingGazetteerCoverage {
    NativeFundingGazetteerCoverage::default()
}

fn rows(
    tokens: &[FundingFeatureToken],
    coverage: &NativeFundingGazetteerCoverage,
) -> Vec<FundingFeatureRow> {
    render_funding_features(tokens, coverage).expect("the fixture must render")
}

fn column(row: &FundingFeatureRow, index: usize) -> &str {
    row.columns
        .get(index)
        .unwrap_or_else(|| panic!("row has no column {index}"))
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

/// The Rust ABI constants are the `families.funding` transcription, and the
/// label slot is the slot *after* the last referenced feature column.
#[test]
fn the_rust_abi_constants_are_the_data_transcription() {
    let value = data();
    assert_eq!(
        number(&value, &["families", "funding", "referenced_columns"]) as usize,
        NATIVE_FUNDING_FEATURE_COLUMNS
    );
    assert_eq!(
        number(&value, &["families", "funding", "emitted_columns"]) as usize,
        NATIVE_FUNDING_FEATURE_COLUMNS
    );
    assert_eq!(
        number(&value, &["families", "funding", "label_column_index"]) as usize,
        NATIVE_FUNDING_LABEL_COLUMN_INDEX
    );
    assert_eq!(
        text(&value, &["families", "funding", "label_column_value"]),
        NATIVE_FUNDING_LABEL_COLUMN_DUMMY
    );
    assert_eq!(
        strings(&value, &["families", "funding", "gazetteer_lanes"]),
        NativeFundingGazetteerLane::ALL.map(NativeFundingGazetteerLane::as_str)
    );
    assert_eq!(
        NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX + 1,
        NATIVE_FUNDING_FEATURE_COLUMNS
    );
    assert_eq!(
        NATIVE_FUNDING_LABEL_COLUMN_INDEX,
        NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX + 1
    );
    assert_eq!(
        NATIVE_FUNDING_ROW_COLUMNS,
        NATIVE_FUNDING_FEATURE_COLUMNS + 1
    );
    assert_eq!(NativeFundingGazetteerLane::KnownFunder.column_index(), 14);
    assert_eq!(
        NativeFundingGazetteerLane::KnownInfrastructure.column_index(),
        15
    );
}

#[test]
fn the_schema_version_names_the_surface() {
    assert_eq!(
        NATIVE_FUNDING_FEATURES_SCHEMA_VERSION,
        "da-parse.native-funding-features/v1"
    );
}

/// The DATA declares two caller-supplied lanes whose real lexicons stay
/// unbundled (D358): no lane spelling may be a bundled resource path.
#[test]
fn the_lane_spellings_are_the_data_spellings() {
    assert_eq!(
        NativeFundingGazetteerLane::KnownFunder.as_str(),
        "knownFunder"
    );
    assert_eq!(
        NativeFundingGazetteerLane::KnownInfrastructure.as_str(),
        "knownInfrastructure"
    );
    assert_eq!(
        serde_json::to_string(&NativeFundingGazetteerLane::KnownFunder).expect("serializes"),
        "\"knownFunder\""
    );
}

/// One row is the 17 printed features plus the label slot, in
/// `printVector` order, and every lane lands at its own index.
#[test]
fn the_row_is_the_print_vector_column_order() {
    let rendered = rows(&[token("Cambridge")], &empty_coverage());
    assert_eq!(rendered.len(), 1);
    let row = &rendered[0];
    assert_eq!(row.width(), 18);
    assert_eq!(
        row.columns,
        [
            "Cambridge", // 0  string
            "cambridge", // 1  lowercase
            "C",         // 2  prefix 1
            "Ca",        // 3  prefix 2
            "Cam",       // 4  prefix 3
            "Camb",      // 5  prefix 4
            "e",         // 6  suffix 1
            "ge",        // 7  suffix 2
            "dge",       // 8  suffix 3
            "idge",      // 9  suffix 4
            "LINESTART", // 10 lineStatus
            "INITCAP",   // 11 capitalisation
            "NODIGIT",   // 12 digit
            "0",         // 13 singleChar
            "0",         // 14 knownFunder
            "0",         // 15 knownInfrastructure
            "NOPUNCT",   // 16 punctType
            "0",         // 17 label slot
        ]
        .map(String::from)
    );
}

/// Every `punctType` branch of the Java ladder reaches its own spelling, and a
/// digit-only token is printed `NOCAPS` although the ladder says `ALLCAP`.
#[test]
fn the_ladder_lanes_print_their_own_spellings() {
    let rendered = rows(
        &[
            token("("),
            token(")"),
            token("."),
            token(","),
            token("-"),
            token("\""),
            token(";"),
            token("!"),
            token("2017"),
            token("ABC1"),
            token("x"),
        ],
        &empty_coverage(),
    );
    let punct: Vec<&str> = rendered.iter().map(|row| column(row, 16)).collect();
    assert_eq!(
        punct,
        [
            "OPENBRACKET",
            "ENDBRACKET",
            "DOT",
            "COMMA",
            "HYPHEN",
            "QUOTE",
            "PUNCT",
            "NOPUNCT",
            "NOPUNCT",
            "NOPUNCT",
            "NOPUNCT"
        ]
    );
    assert_eq!(column(&rendered[8], 11), "NOCAPS");
    assert_eq!(column(&rendered[8], 12), "ALLDIGIT");
    assert_eq!(column(&rendered[9], 11), "ALLCAP");
    assert_eq!(column(&rendered[9], 12), "CONTAINSDIGITS");
    assert_eq!(column(&rendered[10], 13), "1");
    for row in &rendered {
        assert_eq!(row.width(), NATIVE_FUNDING_ROW_COLUMNS);
    }
}

/// The label slot is a literal, never a gold tag — even when the caller hands in
/// tokens whose text *is* a model label spelling.
#[test]
fn the_label_slot_is_the_literal_zero_even_when_a_gold_label_exists() {
    let rendered = rows(
        &[
            token("I-<funderName>"),
            token("<grantNumber>"),
            token("<other>"),
        ],
        &empty_coverage(),
    );
    for (index, row) in rendered.iter().enumerate() {
        assert_eq!(row.width(), NATIVE_FUNDING_ROW_COLUMNS);
        assert_eq!(
            row.label_column(),
            Some(NATIVE_FUNDING_LABEL_COLUMN_DUMMY),
            "row {index} must carry the inference literal"
        );
        assert_eq!(column(row, NATIVE_FUNDING_LABEL_COLUMN_INDEX), "0");
    }
    // Java's *training* path prints the tag in the slot (`features.label = tag`).
    // This ABI is the inference ABI, so that row shape can never be produced by
    // the renderer.
    let mut training_shaped = rendered[0].columns[..NATIVE_FUNDING_LABEL_COLUMN_INDEX].to_vec();
    training_shaped.push("I-<funderName>".to_string());
    assert_ne!(rendered[0].as_text(), training_shaped.join(" "));
}

/// A multi-word span flags exactly the tokens inside it, both ends inclusive.
#[test]
fn a_multi_word_span_covers_only_its_inclusive_window() {
    let rendered = rows(
        &["alpha", "beta", "gamma", "delta", "epsilon"].map(token),
        &coverage(
            vec![NativeFundingGazetteerSpan { start: 2, end: 4 }],
            vec![NativeFundingGazetteerSpan { start: 0, end: 0 }],
        ),
    );
    let funders: Vec<&str> = rendered.iter().map(|row| column(row, 14)).collect();
    let infrastructures: Vec<&str> = rendered.iter().map(|row| column(row, 15)).collect();
    assert_eq!(funders, ["0", "0", "1", "1", "1"]);
    assert_eq!(infrastructures, ["1", "0", "0", "0", "0"]);
    for (n, row) in rendered.iter().enumerate() {
        assert_eq!(
            column(row, 14) == "1",
            funding_span_covers(&[NativeFundingGazetteerSpan { start: 2, end: 4 }], n)
        );
    }
}

/// Empty coverage prints `0` in both lanes, and the `0` is the absence of an
/// observation: it is never a claim that the token was checked and rejected.
#[test]
fn an_empty_coverage_is_the_offline_limit_and_not_an_observed_negative() {
    let empty = empty_coverage();
    assert!(empty.is_empty());
    assert_eq!(empty.span_count(), 0);
    assert!(!empty.covers_funder(0));
    assert!(!empty.covers_infrastructure(0));
    let rendered = rows(&[token("Cambridge")], &empty);
    assert_eq!(column(&rendered[0], 14), "0");
    assert_eq!(column(&rendered[0], 15), "0");

    // The same token with coverage supplied differs *only* in the two lane
    // columns, so the empty-coverage `0` cannot be read as a checked negative.
    let supplied = rows(
        &[token("Cambridge")],
        &coverage(
            vec![NativeFundingGazetteerSpan { start: 0, end: 0 }],
            vec![NativeFundingGazetteerSpan { start: 0, end: 0 }],
        ),
    );
    assert_eq!(column(&supplied[0], 14), "1");
    assert_eq!(column(&supplied[0], 15), "1");
    let differing: Vec<usize> = rendered[0]
        .columns
        .iter()
        .zip(&supplied[0].columns)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(differing, [14, 15]);
}

/// An inverted span is not an empty span: it is refused, so coverage cannot be
/// silently dropped (fail-open).
#[test]
fn an_inverted_span_is_refused_before_any_flag_is_built() {
    let inverted = coverage(
        vec![NativeFundingGazetteerSpan { start: 7, end: 3 }],
        vec![],
    );
    assert_eq!(
        validate_funding_gazetteer_coverage(&inverted),
        Err(NativeFundingFeatureError::InvertedGazetteerSpan {
            lane: NativeFundingGazetteerLane::KnownFunder,
            index: 0,
            start: 7,
            end: 3,
        })
    );
    assert_eq!(
        render_funding_features(&[token("Cambridge")], &inverted),
        Err(NativeFundingFeatureError::InvertedGazetteerSpan {
            lane: NativeFundingGazetteerLane::KnownFunder,
            index: 0,
            start: 7,
            end: 3,
        })
    );
    let inverted_infrastructure = coverage(
        vec![],
        vec![NativeFundingGazetteerSpan { start: 2, end: 1 }],
    );
    assert_eq!(
        validate_funding_gazetteer_coverage(&inverted_infrastructure),
        Err(NativeFundingFeatureError::InvertedGazetteerSpan {
            lane: NativeFundingGazetteerLane::KnownInfrastructure,
            index: 0,
            start: 2,
            end: 1,
        })
    );
}

/// An unbounded inventory is refused rather than swept.
#[test]
fn an_over_long_coverage_is_refused() {
    let spans = vec![
        NativeFundingGazetteerSpan { start: 0, end: 0 };
        MAX_NATIVE_FUNDING_GAZETTEER_SPANS + 1
    ];
    let oversized = coverage(spans, vec![]);
    assert_eq!(
        render_funding_features(&[token("Cambridge")], &oversized),
        Err(NativeFundingFeatureError::TooManyGazetteerSpans {
            spans: MAX_NATIVE_FUNDING_GAZETTEER_SPANS + 1,
            maximum: MAX_NATIVE_FUNDING_GAZETTEER_SPANS,
        })
    );
}

/// Blank tokens are refused instead of skipped: skipping would shift the token
/// indices the coverage spans address.
#[test]
fn a_blank_token_is_refused_instead_of_skipped() {
    for (token_text, expected) in [
        ("", NativeFundingFeatureError::EmptyToken { token: 0 }),
        (
            "two words",
            NativeFundingFeatureError::TokenContainsWhitespace { token: 0 },
        ),
        (
            "tab\there",
            NativeFundingFeatureError::TokenContainsWhitespace { token: 0 },
        ),
    ] {
        assert_eq!(
            render_funding_features(&[token(token_text)], &empty_coverage()),
            Err(expected)
        );
    }
    assert_eq!(
        render_funding_features(&[], &empty_coverage()),
        Err(NativeFundingFeatureError::EmptyInput)
    );
    assert_eq!(
        render_funding_features(&[token("ok"), token("")], &empty_coverage()),
        Err(NativeFundingFeatureError::EmptyToken { token: 1 })
    );
}

/// A model that declares 16 or 18 observation columns is refused: the dummy
/// label slot is not an observation column, and the printed features are not a
/// subset to be widened.
#[test]
fn the_declared_observation_width_is_refused_when_it_is_not_seventeen() {
    assert_eq!(check_funding_feature_columns(17), Ok(()));
    for declared in [16, 18, MAX_NATIVE_FUNDING_FEATURE_ROWS] {
        assert_eq!(
            check_funding_feature_columns(declared),
            Err(NativeFundingFeatureError::DeclaredColumnMismatch { declared })
        );
    }
}

/// A row of the wrong printed width is refused by the text renderer, and the
/// admitted width serializes one row per line with no trailing terminator.
#[test]
fn a_wrong_printed_arity_is_refused_by_the_text_renderer() {
    for width in [16, 19] {
        let row = FundingFeatureRow {
            columns: vec!["0".into(); width],
        };
        assert_eq!(
            render_funding_feature_text(&[row]),
            Err(NativeFundingFeatureError::WrongArity {
                row: 0,
                expected: NATIVE_FUNDING_ROW_COLUMNS,
                actual: width,
            })
        );
    }
    let rendered = rows(&[token("a"), token("b")], &empty_coverage());
    let text = render_funding_feature_text(&rendered).expect("rows are the declared width");
    assert_eq!(text.lines().count(), 2);
    assert!(!text.ends_with('\n'));
    assert_eq!(
        text.lines().next().expect("two lines").split(' ').count(),
        18
    );
}

/// The bounded admission refuses instead of truncating.
#[test]
fn an_over_long_sequence_is_refused_instead_of_truncated() {
    let tokens: Vec<FundingFeatureToken> =
        std::iter::repeat_n(token("token"), MAX_NATIVE_FUNDING_FEATURE_ROWS + 1).collect();
    assert_eq!(
        render_funding_features(&tokens, &empty_coverage()),
        Err(NativeFundingFeatureError::TooManyRows {
            rows: MAX_NATIVE_FUNDING_FEATURE_ROWS + 1,
            maximum: MAX_NATIVE_FUNDING_FEATURE_ROWS,
        })
    );
}

/// A multibyte token still yields exactly one full row, with scalar-value
/// prefix/suffix columns.
#[test]
fn a_multibyte_token_yields_a_full_row() {
    let rendered = rows(&[token("Ünïcödé")], &empty_coverage());
    assert_eq!(rendered[0].width(), 18);
    assert_eq!(column(&rendered[0], 0), "Ünïcödé");
    assert_eq!(column(&rendered[0], 1), "ünïcödé");
    assert_eq!(column(&rendered[0], 2), "Ü");
    assert_eq!(column(&rendered[0], 6), "é");
    assert_eq!(column(&rendered[0], 7), "dé");
    assert_eq!(column(&rendered[0], 17), NATIVE_FUNDING_LABEL_COLUMN_DUMMY);
}

/// Rendering is deterministic byte-for-byte, and the caller's span order does not
/// move the flags: the lane sweep sorts its own copy.
#[test]
fn rendering_is_deterministic_and_span_order_independent() {
    let tokens: Vec<FundingFeatureToken> = ["Known", "Unknown", "Other"].map(token).to_vec();
    let ordered = coverage(
        vec![
            NativeFundingGazetteerSpan { start: 0, end: 0 },
            NativeFundingGazetteerSpan { start: 2, end: 2 },
        ],
        vec![NativeFundingGazetteerSpan { start: 1, end: 1 }],
    );
    let shuffled = coverage(
        vec![
            NativeFundingGazetteerSpan { start: 2, end: 2 },
            NativeFundingGazetteerSpan { start: 0, end: 0 },
        ],
        vec![NativeFundingGazetteerSpan { start: 1, end: 1 }],
    );
    let first = render_funding_feature_text(&rows(&tokens, &ordered)).expect("renders");
    let second = render_funding_feature_text(&rows(&tokens, &ordered)).expect("renders");
    let third = render_funding_feature_text(&rows(&tokens, &shuffled)).expect("renders");
    assert_eq!(first, second);
    assert_eq!(first, third);
}

/// Overlapping spans are a union, not a last-writer-wins, and a span may reach
/// past the admitted token list without inventing flags.
#[test]
fn overlapping_and_out_of_range_spans_are_a_union() {
    let rendered = rows(
        &["a", "b", "c"].map(token),
        &coverage(
            vec![
                NativeFundingGazetteerSpan { start: 1, end: 4 },
                NativeFundingGazetteerSpan { start: 0, end: 2 },
            ],
            vec![],
        ),
    );
    let funders: Vec<&str> = rendered.iter().map(|row| column(row, 14)).collect();
    assert_eq!(funders, ["1", "1", "1"]);
}

/// The typed projections are serde-stable in both directions.
#[test]
fn the_typed_surface_roundtrips() {
    roundtrip(&token("Cambridge"));
    roundtrip(&FundingLineStatus::Lineend);
    roundtrip(&FundingCapitalisation::Allcap);
    roundtrip(&FundingDigit::Alldigit);
    roundtrip(&FundingPunctType::Openbracket);
    roundtrip(&NativeFundingGazetteerLane::KnownInfrastructure);
    roundtrip(&NativeFundingGazetteerSpan { start: 1, end: 3 });
    roundtrip(&coverage(
        vec![NativeFundingGazetteerSpan { start: 0, end: 1 }],
        vec![],
    ));
    let rendered = rows(&[token("Cambridge")], &empty_coverage());
    roundtrip(&rendered[0]);
    roundtrip(&NativeFundingFeatureError::InvertedGazetteerSpan {
        lane: NativeFundingGazetteerLane::KnownFunder,
        index: 0,
        start: 4,
        end: 2,
    });
    assert_eq!(
        serde_json::to_string(&NativeFundingFeatureError::EmptyInput).expect("serializes"),
        "\"empty_input\""
    );
}

/// The row digest is the redaction boundary: stable, prefixed, and free of the
/// raw text it identifies.
#[test]
fn the_row_digest_is_a_stable_opaque_identity() {
    let rendered = rows(&[token("Forschungsgemeinschaft")], &empty_coverage());
    let digest = rendered[0].digest();
    assert!(digest.starts_with("sha256:"));
    assert_eq!(digest.len(), 71);
    assert!(!digest.contains("Forschung"));
    let again = rows(&[token("Forschungsgemeinschaft")], &empty_coverage());
    assert_eq!(again[0].digest(), digest);
    let other = rows(&[token("Deutsche")], &empty_coverage());
    assert_ne!(other[0].digest(), digest);
}

/// Quarantine: the renderer carries no filesystem, environment, logging or model
/// material, and never reaches the production route.
///
/// The lexicon *paths* may appear in the module documentation (the band names
/// the feeds it deliberately does not read); what must not appear is a read, a
/// model byte, or a log call.
#[test]
fn the_module_carries_no_model_material_or_io() {
    for forbidden in [
        "std::fs",
        "std::env",
        "include_bytes",
        "include_str",
        "File::open",
        "read_to_string",
        "tracing::",
        "#qrk#",
        "%x[",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "the funding feature band must not carry `{forbidden}`"
        );
    }
    for required in [
        "crate::native_feature_facts",
        "NATIVE_FUNDING_LABEL_COLUMN_DUMMY",
        "self.start <= n && n <= self.end",
    ] {
        assert!(
            SOURCE.contains(required),
            "the funding feature band must keep `{required}`"
        );
    }
}
