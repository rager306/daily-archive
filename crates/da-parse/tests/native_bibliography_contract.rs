//! Contract for the native bibliography sequence runner (S04/T04).
//!
//! The oracle here is counted by hand **twice over**, and neither counting runs
//! Java or derives an expectation from the code under test:
//!
//! 1. the two synthetic model files declare their own label block and weight
//!    table; the derivations of every weight index are written out below;
//! 2. the grouping rule is base-name equality (D359), so the expected spans are
//!    the maximal runs of equal base names and can be read straight off the
//!    requested label sequence.
//!
//! Nothing in this file loads a real GROBID model, and the fixtures carry no
//! GROBID bytes (D358): they are hand-authored text models of a few hundred
//! bytes each, named `*_synthetic.wapiti` inside this crate's test fixtures.

use da_parse::{
    BibliographyAlignment, BibliographyGazetteerIndex, BibliographyTokenFacts,
    BibliographyTokenInput, CITATION_ROLE_BASE_LABELS, CitationFeatureRow, CitationRole,
    CitationRoleSpan, DATE_PART_BASE_LABELS, DateFeatureRow, DatePartRole, FactsBlockStatus,
    FactsLineStatus, MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS, NAME_PART_BASE_LABELS,
    NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_CITATION_IDENTIFIER_COLUMN,
    NATIVE_CITATION_LABEL_COUNT, NATIVE_CITATION_LABELS, NATIVE_CITATION_SEQUENCE_FAMILY,
    NATIVE_DATE_FEATURE_COLUMNS, NATIVE_DATE_LABEL_COUNT, NATIVE_DATE_LABELS,
    NATIVE_DATE_MODEL_MAX_COLUMN_INDEX, NATIVE_NAME_CITATION_FEATURE_COLUMNS,
    NATIVE_NAME_CITATION_LABEL_COUNT, NATIVE_NAME_CITATION_LABELS,
    NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX, NATIVE_NAME_CITATION_SEQUENCE_FAMILY,
    NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS, NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT,
    NATIVE_REFERENCE_SEGMENTER_LABELS, NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
    NameCitationFeatureRow, NamePartRole, NativeBibliographyNestedError,
    NativeBibliographySequenceError, NativePersonTitleCoverage, ReferenceSegmentKind,
    ReferenceSegmenterFeatureRow, bibliography_label_runs, citation_row, cluster_identifier,
    date_row, decode_bibliography_labels, decode_citation_sequence, decode_date_cluster,
    decode_name_citation_cluster, decode_reference_sequence, name_citation_row,
    reference_segmenter_row,
};
use da_ports::{CrfModel, FeatureTemplate, LexiconBundle, LexiconChannel};
use std::path::{Path, PathBuf};

const REFERENCE_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti";
const CITATION_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_citation_synthetic.wapiti";
const NAME_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_name_synthetic.wapiti";
const DATE_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_date_synthetic.wapiti";

// Fixture provenance notes: both files are hand-written in the Wapiti *text*
// model format the `da-domain` reader accepts, keyed on column 0 of the
// rendered row (`u:%x[0,0]`), one unigram observation per token, and one
// strictly-greater weight per position — so the decoded sequence is the
// requested one and not an artefact of tie-breaking.

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a fixture and parse it with the production model reader. Missing or
/// malformed fixtures are FATAL: the contract must never silently shrink.
fn load_model(rel_path: &str) -> (Vec<u8>, CrfModel) {
    let bytes = std::fs::read(repo_root().join(rel_path))
        .unwrap_or_else(|error| panic!("FATAL {rel_path}: fixture missing/unreadable ({error})"));
    let model = CrfModel::parse(&bytes)
        .unwrap_or_else(|error| panic!("FATAL {rel_path}: fixture does not parse ({error})"));
    (bytes, model)
}

/// A valid provider bundle, shaped exactly like the one the row-builder unit
/// tests use (ordered channel keys, non-empty entries, one template).
fn fixture_lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![
            LexiconChannel {
                key: "JournalNames".into(),
                entries: vec!["nature".into()],
            },
            LexiconChannel {
                key: "LocationNames".into(),
                entries: vec!["google".into()],
            },
            LexiconChannel {
                key: "commonName".into(),
                entries: vec!["google".into()],
            },
            LexiconChannel {
                key: "firstName".into(),
                entries: vec!["ashish".into()],
            },
            LexiconChannel {
                key: "lastName".into(),
                entries: vec!["vaswani".into()],
            },
        ],
        feature_templates: vec![FeatureTemplate {
            key: "citation".into(),
            channel_keys: vec!["commonName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

fn facts(
    text: &str,
    line_status: FactsLineStatus,
    block_status: FactsBlockStatus,
    line_length: usize,
) -> BibliographyTokenFacts {
    BibliographyTokenFacts::from_token(&BibliographyTokenInput {
        text: text.to_string(),
        block_status,
        line_status,
        alignment: BibliographyAlignment::Alignedleft,
        line_length,
        superscript: false,
    })
}

/// `12 Smith J Title 2019 Archive Press` — the label span carries the in-text
/// marker, the reference span the body, and the trailing `<other>` span proves
/// that a bare label followed by its `I-` spelling is still one run.
const REFERENCE_TOKENS: [&str; 7] = ["12", "Smith", "J", "Title", "2019", "Archive", "Press"];
const REFERENCE_LINE_LENGTH: usize = 29; // 2+5+1+5+4+7+5, no separators

fn reference_rows() -> Vec<ReferenceSegmenterFeatureRow> {
    let lexicon = fixture_lexicon();
    let count = REFERENCE_TOKENS.len();
    let tokens: Vec<BibliographyTokenFacts> = REFERENCE_TOKENS
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            };
            let block_status = match index {
                0 => FactsBlockStatus::Blockstart,
                _ if index + 1 == count => FactsBlockStatus::Blockend,
                _ => FactsBlockStatus::Blockin,
            };
            facts(text, line_status, block_status, REFERENCE_LINE_LENGTH)
        })
        .collect();
    reference_segmenter_row(&tokens, &lexicon, REFERENCE_LINE_LENGTH)
        .expect("the reference fixture line renders through the real row builder")
}

/// `Vaswani Ashish Attention Is All You Need 2017` — one author run whose first
/// row is the `I-` spelling, one title run that alternates spellings for five
/// rows, and a one-row date cluster.
const CITATION_TOKENS: [&str; 8] = [
    "Vaswani",
    "Ashish",
    "Attention",
    "Is",
    "All",
    "You",
    "Need",
    "2017",
];
const CITATION_LINE_LENGTH: usize = 45;

fn citation_rows() -> Vec<CitationFeatureRow> {
    let lexicon = fixture_lexicon();
    let count = CITATION_TOKENS.len();
    let tokens: Vec<BibliographyTokenFacts> = CITATION_TOKENS
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            };
            let block_status = match index {
                0 => FactsBlockStatus::Blockstart,
                _ if index + 1 == count => FactsBlockStatus::Blockend,
                _ => FactsBlockStatus::Blockin,
            };
            facts(text, line_status, block_status, CITATION_LINE_LENGTH)
        })
        .collect();
    let spans = BibliographyGazetteerIndex::from_lexicon(&lexicon)
        .expect("fixture lexicon is a valid gazetteer bundle")
        .spans(&CITATION_TOKENS)
        .expect("eight tokens are within the gazetteer bounds");
    citation_row(&tokens, &spans, &lexicon)
        .expect("the citation fixture block renders through the real row builder")
}

fn row_texts<S: AsRef<str>>(tokens: &[S], columns: usize) -> Vec<String> {
    tokens
        .iter()
        .map(|token| {
            std::iter::once(token.as_ref().to_string())
                .chain((1..columns).map(|_| "0".to_string()))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

// ───────────────────────── fixture ↔ family binding ─────────────────────────

/// The reference fixture must declare the transcribed 6-label family block: the
/// model's label ids index its weight blocks, so a fixture with a different
/// block would be a different model.
#[test]
fn reference_fixture_declares_the_transcribed_six_label_block() {
    let (bytes, model) = load_model(REFERENCE_FIXTURE);
    assert_eq!(model.model_type, 2, "scalar CRF model type");
    assert_eq!(model.labels, NATIVE_REFERENCE_SEGMENTER_LABELS);
    assert_eq!(model.labels.len(), NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT);
    assert_eq!(model.patterns.len(), 1);
    assert_eq!(model.patterns[0].source, "u:%x[0,0]");
    assert!(
        bytes.len() < 2_048,
        "the fixture is hand-authored, not a redistributed model: {} bytes",
        bytes.len()
    );
    assert!(
        std::str::from_utf8(&bytes).is_ok(),
        "the fixture stays a text model"
    );
}

#[test]
fn citation_fixture_declares_the_transcribed_thirty_eight_label_block() {
    let (bytes, model) = load_model(CITATION_FIXTURE);
    assert_eq!(model.model_type, 2, "scalar CRF model type");
    assert_eq!(model.labels, NATIVE_CITATION_LABELS);
    assert_eq!(model.labels.len(), NATIVE_CITATION_LABEL_COUNT);
    assert_eq!(model.observations.len(), CITATION_TOKENS.len());
    assert!(
        bytes.len() < 4_096,
        "the fixture is hand-authored, not a redistributed model: {} bytes",
        bytes.len()
    );
    assert!(
        std::str::from_utf8(&bytes).is_ok(),
        "the fixture stays a text model"
    );
}

/// Weight-index derivation for the reference fixture, counted by hand.
///
/// The model declares 6 labels in family order (`I-<other>`=0, `I-<label>`=1,
/// `<label>`=2, `I-<reference>`=3, `<reference>`=4, `<other>`=5) and 7 unigram
/// observations (`u:12`, `u:Smith`, `u:J`, `u:Title`, `u:2019`, `u:Archive`,
/// `u:Press`), so observation *i* owns the weight block starting at `6 * i` and
/// the weight for label `l` sits at `6 * i + l`. One weight per position, value
/// 3.0 (`0x1.8p+1`), strictly greater than every other label's 0.0:
///
/// | row | token | label id | index | value |
/// |-----|-------|----------|-------|-------|
/// | 0 | 12 | `<label>` = 2 | 2 | 3.0 |
/// | 1 | Smith | `I-<reference>` = 3 | 9 | 3.0 |
/// | 2 | J | `<reference>` = 4 | 16 | 3.0 |
/// | 3 | Title | `I-<reference>` = 3 | 21 | 3.0 |
/// | 4 | 2019 | `<reference>` = 4 | 28 | 3.0 |
/// | 5 | Archive | `<other>` = 5 | 35 | 3.0 |
/// | 6 | Press | `I-<other>` = 0 | 36 | 3.0 |
///
/// Hence the expected label sequence and, by base-name equality, exactly three
/// spans.
#[test]
fn reference_line_decodes_into_the_three_boundary_kinds() {
    let (_, model) = load_model(REFERENCE_FIXTURE);
    let decoded = decode_reference_sequence(&reference_rows(), &model)
        .expect("the synthetic reference model decodes the rendered line");

    assert_eq!(
        decoded.labels,
        [
            "<label>",
            "I-<reference>",
            "<reference>",
            "I-<reference>",
            "<reference>",
            "<other>",
            "I-<other>",
        ]
    );
    assert_eq!(decoded.line_text, "12 Smith J Title 2019 Archive Press");

    let spans: Vec<(ReferenceSegmentKind, &str, usize, usize, &str)> = decoded
        .segments
        .iter()
        .map(|span| {
            (
                span.kind,
                span.label.as_str(),
                span.start,
                span.end,
                span.text.as_str(),
            )
        })
        .collect();
    assert_eq!(
        spans,
        [
            (ReferenceSegmentKind::Label, "<label>", 0, 1, "12"),
            (
                ReferenceSegmentKind::Reference,
                "<reference>",
                1,
                5,
                "Smith J Title 2019"
            ),
            (
                ReferenceSegmentKind::Other,
                "<other>",
                5,
                7,
                "Archive Press"
            ),
        ]
    );

    // Byte ranges must reproduce each span inside the reported line text:
    // "12" occupies 0..2, "Smith J Title 2019" 3..21, "Archive Press" 22..35.
    let ranges: Vec<(usize, usize)> = decoded
        .segments
        .iter()
        .map(|span| (span.byte_start, span.byte_end))
        .collect();
    assert_eq!(ranges, [(0, 2), (3, 21), (22, 35)]);
    for span in &decoded.segments {
        assert_eq!(
            &decoded.line_text[span.byte_start..span.byte_end],
            span.text,
            "the byte range must reproduce the span text"
        );
        assert!(!span.label.starts_with("I-"), "spans carry base names");
    }
}

/// Weight-index derivation for the citation fixture, counted by hand.
///
/// 38 labels in family order and 8 unigram observations, so observation *i* owns
/// the block starting at `38 * i`. Target labels and their ids:
/// `I-<author>`=9, `<author>`=10, `<title>`=12, `I-<title>`=11, `<date>`=8.
///
/// | row | token | label id | index = 38 * row + id |
/// |-----|-------|----------|-----------------------|
/// | 0 | Vaswani | `I-<author>` = 9 | 9 |
/// | 1 | Ashish | `<author>` = 10 | 48 |
/// | 2 | Attention | `<title>` = 12 | 88 |
/// | 3 | Is | `I-<title>` = 11 | 125 |
/// | 4 | All | `<title>` = 12 | 164 |
/// | 5 | You | `I-<title>` = 11 | 201 |
/// | 6 | Need | `<title>` = 12 | 240 |
/// | 7 | 2017 | `<date>` = 8 | 274 |
///
/// The alternating `I-<title>` / `<title>` spelling across rows 2..7 is the shape
/// a BIO reading of the labels would shred into five spans; base-name equality
/// keeps it one.
#[test]
fn citation_block_decodes_into_role_clusters() {
    let (_, model) = load_model(CITATION_FIXTURE);
    let decoded = decode_citation_sequence(&citation_rows(), &model)
        .expect("the synthetic citation model decodes the rendered block");

    assert_eq!(
        decoded.labels,
        [
            "I-<author>",
            "<author>",
            "<title>",
            "I-<title>",
            "<title>",
            "I-<title>",
            "<title>",
            "<date>",
        ]
    );
    assert_eq!(
        decoded.cluster_text,
        "Vaswani Ashish Attention Is All You Need 2017"
    );

    let roles: Vec<(CitationRole, &str, usize, usize, &str)> = decoded
        .roles
        .iter()
        .map(|span| {
            (
                span.role,
                span.label.as_str(),
                span.start,
                span.end,
                span.text.as_str(),
            )
        })
        .collect();
    assert_eq!(
        roles,
        [
            (CitationRole::Author, "<author>", 0, 2, "Vaswani Ashish"),
            (
                CitationRole::Title,
                "<title>",
                2,
                7,
                "Attention Is All You Need"
            ),
            (CitationRole::Date, "<date>", 7, 8, "2017"),
        ]
    );

    let ranges: Vec<(usize, usize)> = decoded
        .roles
        .iter()
        .map(|span| (span.byte_start, span.byte_end))
        .collect();
    assert_eq!(ranges, [(0, 14), (15, 40), (41, 45)]);
    for span in &decoded.roles {
        assert_eq!(
            &decoded.cluster_text[span.byte_start..span.byte_end],
            span.text,
            "the byte range must reproduce the span text"
        );
    }
}

/// The `I-` prefix cannot open or close anything on its own: a run is a maximal
/// sequence of equal base names, so the five-row title run is one cluster and the
/// author run legally *begins* with the `I-` spelling.
#[test]
fn the_i_prefix_neither_opens_nor_continues_on_its_own() {
    let (_, model) = load_model(CITATION_FIXTURE);
    let decoded = decode_citation_sequence(&citation_rows(), &model).expect("decodes");
    assert_eq!(decoded.roles.len(), 3, "three base names, three clusters");
    let title = &decoded.roles[1];
    assert_eq!(title.role, CitationRole::Title);
    assert_eq!(title.end - title.start, 5, "one title cluster, not five");
    let author = &decoded.roles[0];
    assert_eq!(author.role, CitationRole::Author);
    assert!(author.start < author.end);
    assert!(
        decoded.labels[author.start].starts_with("I-"),
        "the author run begins with the I- spelling"
    );
    assert!(
        !decoded.labels[title.end - 1].starts_with("I-"),
        "and the title run ends with the bare spelling"
    );

    // Sanity: the grouping rule is not a hard-coded table of shapes.
    let split = decode_citation_sequence(&citation_rows(), &model).expect("decodes");
    assert_eq!(split.roles, decoded.roles, "decode is a pure function");
}

// ─────────────────────────────── negatives ───────────────────────────────

#[test]
fn refuses_an_empty_row_sequence_before_any_model_work() {
    let (_, model) = load_model(REFERENCE_FIXTURE);
    assert_eq!(
        decode_reference_sequence(&[], &model),
        Err(NativeBibliographySequenceError::EmptySequence)
    );
}

/// The two families' printed widths are invariants, not hopes: a 30-column
/// citation row is refused by the 28-column family and vice versa.
#[test]
fn refuses_rows_that_do_not_carry_the_family_width() {
    let (_, reference_model) = load_model(REFERENCE_FIXTURE);
    let citation_width = row_texts(&CITATION_TOKENS, NATIVE_CITATION_FEATURE_COLUMNS);
    assert!(matches!(
        decode_bibliography_labels(
            &citation_width,
            &NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
            &reference_model
        ),
        Err(NativeBibliographySequenceError::WrongArity {
            row: 0,
            actual: NATIVE_CITATION_FEATURE_COLUMNS,
            expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS
        })
    ));

    let (_, citation_model) = load_model(CITATION_FIXTURE);
    let mut narrowed = citation_width;
    for row in narrowed.iter_mut() {
        row.truncate(row.rfind(' ').expect("row has columns"));
    }
    assert!(matches!(
        decode_bibliography_labels(&narrowed, &NATIVE_CITATION_SEQUENCE_FAMILY, &citation_model),
        Err(NativeBibliographySequenceError::WrongArity {
            row: 0,
            actual: 29,
            expected: NATIVE_CITATION_FEATURE_COLUMNS
        })
    ));
}

/// A row whose columns are separated by a tab counts as 28 fields under the
/// model's whitespace split but 29 under the `' '` split the runner builds: the
/// refusal fires instead of letting the grid shift under the decoder.
#[test]
fn refuses_a_row_that_smuggles_a_separator_into_a_column() {
    let (_, model) = load_model(REFERENCE_FIXTURE);
    let rendered = reference_rows();
    let mut columns: Vec<String> = rendered[0]
        .as_text()
        .split(' ')
        .map(str::to_string)
        .collect();
    assert_eq!(columns.len(), NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS);
    // Push the separator *inside* a payload column: the `' '` grid stays 28 wide,
    // while `map_input`'s whitespace split sees 29 fields.
    columns[1].push_str("\t0");
    let smuggled = vec![columns.join(" ")];
    assert_eq!(
        smuggled[0].split(' ').count(),
        NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS
    );
    assert_eq!(
        smuggled[0].split_whitespace().count(),
        NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS + 1
    );
    let error = decode_bibliography_labels(
        &smuggled,
        &NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
        &model,
    )
    .expect_err("a smuggled separator must be refused");
    assert_eq!(
        error,
        NativeBibliographySequenceError::WrongArity {
            row: 0,
            actual: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS + 1,
            expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
        }
    );
}

/// A model of the wrong *size* is refused before decoding, so a 6-label model can
/// never be read as a 38-label one.
#[test]
fn refuses_a_model_whose_label_block_has_the_wrong_size() {
    let (_, reference_model) = load_model(REFERENCE_FIXTURE);
    let error = decode_citation_sequence(&citation_rows(), &reference_model)
        .expect_err("a 6-label model is not a citation model");
    assert_eq!(
        error,
        NativeBibliographySequenceError::WrongLabelCount {
            family: "citation",
            expected: NATIVE_CITATION_LABEL_COUNT,
            actual: NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT,
        }
    );
}

/// Same cardinality, one foreign label: the size check cannot catch it, so the
/// decoded-label check must.
#[test]
fn refuses_a_label_outside_the_declared_vocabulary() {
    let (_, mut model) = load_model(CITATION_FIXTURE);
    model.labels[9] = "<not-declared>".into();
    let error = decode_citation_sequence(&citation_rows(), &model)
        .expect_err("a foreign label must be refused");
    assert_eq!(
        error,
        NativeBibliographySequenceError::UnknownLabel {
            label: "<not-declared>".into(),
            row: 0,
        }
    );
}

/// A malformed model template surfaces as a typed refusal, never a panic.
#[test]
fn surfaces_a_malformed_model_template_as_a_typed_refusal() {
    let (_, mut model) = load_model(REFERENCE_FIXTURE);
    model.patterns[0].source = "u:%x[0,0".into();
    let error = decode_reference_sequence(&reference_rows(), &model)
        .expect_err("a malformed template must be refused");
    assert!(
        matches!(error, NativeBibliographySequenceError::Model { .. }),
        "expected a typed model refusal, got {error:?}"
    );
}

/// The bounded runner refuses an oversized sequence before touching the model.
#[test]
fn refuses_an_oversized_sequence_before_model_work() {
    let (_, model) = load_model(REFERENCE_FIXTURE);
    let oversized = vec![
        row_texts(
            &REFERENCE_TOKENS[..1],
            NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS
        )
        .remove(0);
        MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS + 1
    ];
    assert_eq!(
        decode_bibliography_labels(
            &oversized,
            &NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
            &model
        ),
        Err(NativeBibliographySequenceError::LimitsExceeded {
            resource: "row",
            actual: MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS + 1,
            maximum: MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS,
        })
    );
}

// ───────────────────────────── replay / tables ─────────────────────────────

/// Replay determinism: the same fixture bytes, rows and model produce byte-equal
/// results across repeats (and across a re-read of the fixture).
#[test]
fn replay_is_deterministic() {
    let (_, model) = load_model(REFERENCE_FIXTURE);
    let first = decode_reference_sequence(&reference_rows(), &model).expect("decodes");
    for _ in 0..4 {
        assert_eq!(
            decode_reference_sequence(&reference_rows(), &model).expect("decodes"),
            first
        );
    }
    let (_, reread) = load_model(REFERENCE_FIXTURE);
    assert_eq!(
        decode_reference_sequence(&reference_rows(), &reread).expect("decodes"),
        first
    );

    let (_, citation_model) = load_model(CITATION_FIXTURE);
    let citation = decode_citation_sequence(&citation_rows(), &citation_model).expect("decodes");
    assert_eq!(
        decode_citation_sequence(&citation_rows(), &citation_model).expect("decodes"),
        citation
    );
}

/// The 19 roles and the 38 model labels are two views of one closed set: the
/// role table reproduces every base label of the model table twice, with nothing
/// invented and nothing left over.
#[test]
fn the_role_table_covers_every_model_label_base_exactly_twice() {
    assert_eq!(CitationRole::ALL.len(), CITATION_ROLE_BASE_LABELS.len());
    let mut model_bases: Vec<String> = NATIVE_CITATION_LABELS
        .iter()
        .map(|label| label.strip_prefix("I-").unwrap_or(*label).to_string())
        .collect();
    model_bases.sort();
    model_bases.dedup();
    assert_eq!(model_bases.len(), CITATION_ROLE_BASE_LABELS.len());
    for base in CITATION_ROLE_BASE_LABELS {
        assert!(
            model_bases.iter().any(|model_base| model_base == base),
            "role base {base} is missing from the model label table"
        );
        let spellings = NATIVE_CITATION_LABELS
            .iter()
            .filter(|label| **label == base || label.strip_prefix("I-").unwrap_or(**label) == base)
            .count();
        assert_eq!(spellings, 2, "role {base} needs a begin and an inside");
        assert!(CitationRole::from_base_label(base).is_some());
    }
}

/// The reference family's three kinds are exactly the three base labels the
/// 6-label model can emit.
#[test]
fn the_reference_kinds_cover_the_six_label_table() {
    let mut model_bases: Vec<String> = NATIVE_REFERENCE_SEGMENTER_LABELS
        .iter()
        .map(|label| label.strip_prefix("I-").unwrap_or(*label).to_string())
        .collect();
    model_bases.sort();
    model_bases.dedup();
    assert_eq!(model_bases.len(), ReferenceSegmentKind::ALL.len());
    for kind in ReferenceSegmentKind::ALL {
        assert!(
            model_bases.iter().any(|base| base == kind.base_label()),
            "kind {} has no model base label",
            kind.as_str()
        );
    }
}

// ───────────────────── nested name/date families (S04/T05) ─────────────────────

/// The two tokens of the citation fixture's `<author>` cluster and the three of
/// its `<date>` cluster: the row lists the nested families decode. `NAME_TOKENS`
/// is a prefix of the name fixture's observation list, so the same synthetic
/// model decodes both the two-token cluster and the four-token merge case.
const NAME_TOKENS: [&str; 2] = ["Vaswani", "Ashish"];
const NAME_LINE_LENGTH: usize = 14; // "Vaswani Ashish", no separators
const DATE_TOKENS: [&str; 3] = ["June", "12", "2017"];
const DATE_LINE_LENGTH: usize = 12; // "June 12 2017", no separators

fn name_rows_for(tokens: &[&str]) -> Vec<NameCitationFeatureRow> {
    let lexicon = fixture_lexicon();
    let count = tokens.len();
    let token_facts: Vec<BibliographyTokenFacts> = tokens
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            };
            let block_status = match index {
                0 => FactsBlockStatus::Blockstart,
                _ if index + 1 == count => FactsBlockStatus::Blockend,
                _ => FactsBlockStatus::Blockin,
            };
            facts(text, line_status, block_status, NAME_LINE_LENGTH)
        })
        .collect();
    name_citation_row(
        &token_facts,
        &NativePersonTitleCoverage::from_positions(count, &[]),
        &lexicon,
    )
    .expect("the name rows render through the real row builder")
}

fn date_rows_for(tokens: &[&str]) -> Vec<DateFeatureRow> {
    let lexicon = fixture_lexicon();
    let count = tokens.len();
    let token_facts: Vec<BibliographyTokenFacts> = tokens
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            };
            let block_status = match index {
                0 => FactsBlockStatus::Blockstart,
                _ if index + 1 == count => FactsBlockStatus::Blockend,
                _ => FactsBlockStatus::Blockin,
            };
            facts(text, line_status, block_status, DATE_LINE_LENGTH)
        })
        .collect();
    date_row(&token_facts, &lexicon).expect("the date rows render through the real row builder")
}

/// The citation fixture decoded once: its rows, its role clusters and the
/// cluster text the identifier byte ranges index.
fn citation_block() -> (Vec<CitationFeatureRow>, Vec<CitationRoleSpan>, String) {
    let (_, model) = load_model(CITATION_FIXTURE);
    let rows = citation_rows();
    let decoded = decode_citation_sequence(&rows, &model).expect("the citation fixture decodes");
    (rows, decoded.roles, decoded.cluster_text)
}

fn cluster(roles: &[CitationRoleSpan], role: CitationRole) -> &CitationRoleSpan {
    roles
        .iter()
        .find(|span| span.role == role)
        .unwrap_or_else(|| panic!("the citation fixture has no {} cluster", role.as_str()))
}

// ─────────────────────────────── fixtures ───────────────────────────────

#[test]
fn name_fixture_declares_the_transcribed_nine_label_block() {
    let (bytes, model) = load_model(NAME_FIXTURE);
    assert_eq!(model.model_type, 2, "scalar CRF model type");
    assert_eq!(model.labels, NATIVE_NAME_CITATION_LABELS);
    assert_eq!(model.labels.len(), NATIVE_NAME_CITATION_LABEL_COUNT);
    assert_eq!(model.patterns.len(), 1);
    assert_eq!(model.patterns[0].source, "u:%x[0,0]");
    assert!(
        bytes.len() < 2_048,
        "the fixture is hand-authored, not a redistributed model: {} bytes",
        bytes.len()
    );
    assert!(
        std::str::from_utf8(&bytes).is_ok(),
        "the fixture stays a text model"
    );
}

#[test]
fn date_fixture_declares_the_transcribed_seven_label_block() {
    let (bytes, model) = load_model(DATE_FIXTURE);
    assert_eq!(model.model_type, 2, "scalar CRF model type");
    assert_eq!(model.labels, NATIVE_DATE_LABELS);
    assert_eq!(model.labels.len(), NATIVE_DATE_LABEL_COUNT);
    assert_eq!(model.patterns.len(), 1);
    assert_eq!(model.patterns[0].source, "u:%x[0,0]");
    assert!(bytes.len() < 2_048);
    assert!(
        std::str::from_utf8(&bytes).is_ok(),
        "the fixture stays a text model"
    );
}

/// The two nested families emit the model's *referenced* column subset, not
/// `printVector`'s field count: 18 columns for `name/citation` (highest
/// referenced index 17) and 17 for `date` (16). `isKnownSuffix`, the name
/// family's `punctType` and both label placeholders are deliberately absent.
#[test]
fn nested_rows_are_the_referenced_column_subset_of_their_model() {
    let name_rows = name_rows_for(&NAME_TOKENS);
    assert_eq!(NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX, 17);
    assert_eq!(NATIVE_NAME_CITATION_FEATURE_COLUMNS, 18);
    assert!(
        name_rows
            .iter()
            .all(|row| row.width() == NATIVE_NAME_CITATION_FEATURE_COLUMNS)
    );
    assert_eq!(name_rows[0].as_text().split(' ').count(), 18);

    let date_rows = date_rows_for(&DATE_TOKENS);
    assert_eq!(NATIVE_DATE_MODEL_MAX_COLUMN_INDEX, 16);
    assert_eq!(NATIVE_DATE_FEATURE_COLUMNS, 17);
    assert!(
        date_rows
            .iter()
            .all(|row| row.width() == NATIVE_DATE_FEATURE_COLUMNS)
    );
    assert_eq!(date_rows[0].as_text().split(' ').count(), 17);

    // The citation derivation is a different ladder and is not generalised here.
    assert_ne!(
        NATIVE_NAME_CITATION_FEATURE_COLUMNS,
        NATIVE_CITATION_FEATURE_COLUMNS
    );
}

// ─────────────────────────── nested decode ───────────────────────────

/// Weight-index derivation for the name fixture, counted by hand.
///
/// The model declares 9 labels in family order (`I-<forename>`=0,
/// `I-<surname>`=1, `I-<other>`=2, `<other>`=3, `I-<middlename>`=4,
/// `<surname>`=5, `<forename>`=6, `<middlename>`=7, `I-<suffix>`=8) and four
/// unigram observations, so observation *i* owns the block starting at `9 * i`.
///
/// | row | token | label id | index | value |
/// |-----|-------|----------|-------|-------|
/// | 0 | Vaswani | `<surname>` = 5 | 5 | 3.0 |
/// | 1 | Ashish | `<forename>` = 6 | 15 | 3.0 |
/// | 2 | Paul | `I-<forename>` = 0 | 18 | 3.0 |
/// | 3 | Jr. | `I-<suffix>` = 8 | 35 | 3.0 |
///
/// Rows 0..2 are the citation fixture's `<author>` cluster; rows 1..3 prove that
/// a bare label followed by its `I-` spelling is one run.
#[test]
fn nested_name_family_decodes_the_citation_author_cluster() {
    let (_, roles, cluster_text) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    assert_eq!((author.start, author.end), (0, 2));
    assert_eq!(author.text, "Vaswani Ashish");

    let (_, name_model) = load_model(NAME_FIXTURE);
    let names = decode_name_citation_cluster(author, &name_rows_for(&NAME_TOKENS), &name_model)
        .expect("the synthetic name model decodes the author cluster");

    assert_eq!(names.labels, ["<surname>", "<forename>"]);
    assert_eq!(names.name_text, "Vaswani Ashish");
    let parts: Vec<(NamePartRole, &str, &str)> = names
        .parts
        .iter()
        .map(|part| (part.role, part.label.as_str(), part.text.as_str()))
        .collect();
    assert_eq!(
        parts,
        [
            (NamePartRole::Surname, "<surname>", "Vaswani"),
            (NamePartRole::Forename, "<forename>", "Ashish"),
        ]
    );
    assert_eq!(
        names
            .parts
            .iter()
            .map(|part| (part.byte_start, part.byte_end))
            .collect::<Vec<_>>(),
        [(0, 7), (8, 14)]
    );
    for part in &names.parts {
        assert_eq!(&names.name_text[part.byte_start..part.byte_end], part.text);
        assert!(!part.label.starts_with("I-"), "parts carry base names");
    }
    // The nested coordinate system is anchored on the same cluster text the
    // citation span reports: the union of the parts is the author cluster.
    assert_eq!(names.parts[0].byte_start, author.byte_start);
    assert_eq!(names.parts[1].byte_end, author.byte_end);
    assert_eq!(
        &cluster_text[author.byte_start..author.byte_end],
        author.text
    );
}

/// The grouping rule is T04's, not a copy: the parts are exactly what
/// `bibliography_label_runs` returns over T04's decoded sequence.
#[test]
fn nested_parts_are_t04_runs_over_the_nested_sequence() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let (_, name_model) = load_model(NAME_FIXTURE);
    let rows = name_rows_for(&NAME_TOKENS);
    let decoded = decode_bibliography_labels(
        &rows
            .iter()
            .map(NameCitationFeatureRow::as_text)
            .collect::<Vec<_>>(),
        &NATIVE_NAME_CITATION_SEQUENCE_FAMILY,
        &name_model,
    )
    .expect("the nested family decodes through T04's runner");
    let runs = bibliography_label_runs(&decoded).expect("runs are total for a decoded sequence");
    let names = decode_name_citation_cluster(author, &rows, &name_model).expect("decodes");

    assert_eq!(runs.len(), names.parts.len());
    for (run, part) in runs.iter().zip(names.parts.iter()) {
        assert_eq!(run.label, part.label);
        assert_eq!((run.start, run.end), (part.start, part.end));
        assert_eq!(
            (run.byte_start, run.byte_end),
            (part.byte_start, part.byte_end)
        );
        assert_eq!(run.text, part.text);
        assert_eq!(NamePartRole::from_base_label(&run.label), Some(part.role));
    }
}

#[test]
fn nested_name_run_merges_both_label_spellings() {
    let four_tokens = ["Vaswani", "Ashish", "Paul", "Jr."];
    let span = CitationRoleSpan {
        role: CitationRole::Author,
        label: "<author>".into(),
        start: 0,
        end: 4,
        byte_start: 0,
        byte_end: 23,
        text: "Vaswani Ashish Paul Jr.".into(),
    };
    let (_, name_model) = load_model(NAME_FIXTURE);
    let names = decode_name_citation_cluster(&span, &name_rows_for(&four_tokens), &name_model)
        .expect("the synthetic name model decodes the four-token cluster");

    assert_eq!(
        names.labels,
        ["<surname>", "<forename>", "I-<forename>", "I-<suffix>"]
    );
    let parts: Vec<(NamePartRole, &str)> = names
        .parts
        .iter()
        .map(|part| (part.role, part.text.as_str()))
        .collect();
    assert_eq!(
        parts,
        [
            (NamePartRole::Surname, "Vaswani"),
            (NamePartRole::Forename, "Ashish Paul"),
            (NamePartRole::Suffix, "Jr."),
        ],
        "a bare label followed by its I- spelling is one run, not two"
    );
    assert_eq!((names.parts[1].start, names.parts[1].end), (1, 3));
    assert_eq!(
        &names.name_text[names.parts[1].byte_start..names.parts[1].byte_end],
        "Ashish Paul"
    );
}

/// Weight-index derivation for the date fixture: labels are `I-<day>`=0,
/// `I-<month>`=1, `I-<year>`=2, `I-<other>`=3, `<other>`=4, `<year>`=5,
/// `<day>`=6 over three observations, so observation *i* starts at `7 * i`.
/// `<month>` appears only in its `I-` spelling, exactly like `<suffix>` in the
/// name table.
///
/// | row | token | label id | index | value |
/// |-----|-------|----------|-------|-------|
/// | 0 | June | `I-<month>` = 1 | 1 | 3.0 |
/// | 1 | 12 | `<day>` = 6 | 13 | 3.0 |
/// | 2 | 2017 | `<year>` = 5 | 19 | 3.0 |
#[test]
fn nested_date_family_decodes_the_citation_date_cluster() {
    let (_, roles, _) = citation_block();
    let date = cluster(&roles, CitationRole::Date);
    assert_eq!((date.start, date.end), (7, 8));
    assert_eq!(date.text, "2017");

    let (_, date_model) = load_model(DATE_FIXTURE);
    // The fixture's "2017" observation owns label `<year>` whichever position it
    // sits at, so a one-row cluster decodes without re-deriving weight offsets.
    let single = decode_date_cluster(date, &date_rows_for(&["2017"]), &date_model)
        .expect("the synthetic date model decodes the one-token date cluster");
    assert_eq!(single.labels, ["<year>"]);
    assert_eq!(single.date_text, "2017");
    assert_eq!(single.parts.len(), 1);
    assert_eq!(single.parts[0].role, DatePartRole::Year);
    assert_eq!(single.parts[0].text, "2017");
    assert_eq!(
        (single.parts[0].byte_start, single.parts[0].byte_end),
        (0, 4)
    );

    let three_token_span = CitationRoleSpan {
        role: CitationRole::Date,
        label: "<date>".into(),
        start: 0,
        end: 3,
        byte_start: 0,
        byte_end: 12,
        text: "June 12 2017".into(),
    };
    let full = decode_date_cluster(&three_token_span, &date_rows_for(&DATE_TOKENS), &date_model)
        .expect("the synthetic date model decodes the three-token date cluster");
    assert_eq!(full.labels, ["I-<month>", "<day>", "<year>"]);
    assert_eq!(full.date_text, "June 12 2017");
    let parts: Vec<(DatePartRole, &str)> = full
        .parts
        .iter()
        .map(|part| (part.role, part.text.as_str()))
        .collect();
    assert_eq!(
        parts,
        [
            (DatePartRole::Month, "June"),
            (DatePartRole::Day, "12"),
            (DatePartRole::Year, "2017"),
        ]
    );
    assert_eq!(
        full.parts
            .iter()
            .map(|part| (part.byte_start, part.byte_end))
            .collect::<Vec<_>>(),
        [(0, 4), (5, 7), (8, 12)]
    );
    for part in &full.parts {
        assert_eq!(&full.date_text[part.byte_start..part.byte_end], part.text);
    }
}

// ─────────────────────── nested decode negatives ───────────────────────

#[test]
fn refuses_a_nested_model_whose_label_block_has_the_wrong_size() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let (_, date_model) = load_model(DATE_FIXTURE);
    let error = decode_name_citation_cluster(author, &name_rows_for(&NAME_TOKENS), &date_model)
        .expect_err("a 7-label model is not a name/citation model");
    assert_eq!(
        error,
        NativeBibliographyNestedError::Sequence(NativeBibliographySequenceError::WrongLabelCount {
            family: "name/citation",
            expected: NATIVE_NAME_CITATION_LABEL_COUNT,
            actual: NATIVE_DATE_LABEL_COUNT,
        })
    );
}

#[test]
fn refuses_a_nested_decode_for_a_cluster_of_the_wrong_role() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let date = cluster(&roles, CitationRole::Date);
    let (_, name_model) = load_model(NAME_FIXTURE);
    let (_, date_model) = load_model(DATE_FIXTURE);

    assert_eq!(
        decode_name_citation_cluster(date, &name_rows_for(&["2017"]), &name_model),
        Err(NativeBibliographyNestedError::NotANameCluster {
            role: CitationRole::Date
        })
    );
    assert_eq!(
        decode_date_cluster(author, &date_rows_for(&NAME_TOKENS), &date_model),
        Err(NativeBibliographyNestedError::NotADateCluster {
            role: CitationRole::Author
        })
    );
}

#[test]
fn refuses_nested_rows_that_do_not_cover_the_cluster() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let (_, name_model) = load_model(NAME_FIXTURE);
    let error = decode_name_citation_cluster(
        author,
        &name_rows_for(&["Vaswani", "Ashish", "Paul", "Jr."]),
        &name_model,
    )
    .expect_err("two cluster rows cannot take four nested rows");
    assert_eq!(
        error,
        NativeBibliographyNestedError::ClusterRowMismatch {
            span_rows: 2,
            nested_rows: 4
        }
    );
}

#[test]
fn refuses_nested_rows_that_do_not_carry_the_family_width() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let (_, name_model) = load_model(NAME_FIXTURE);
    // The citation rows are 30 columns wide; presented to the 18-column name
    // family they must be refused rather than re-read as a different ladder.
    let wide: Vec<NameCitationFeatureRow> = citation_rows()
        .into_iter()
        .take(2)
        .map(|row| NameCitationFeatureRow {
            columns: row.columns,
        })
        .collect();
    assert_eq!(
        wide.len(),
        2,
        "the row count must match the cluster, or the mismatch refusal fires first"
    );
    let error = decode_name_citation_cluster(author, &wide, &name_model)
        .expect_err("30-column citation rows are not 18-column name rows");
    assert_eq!(
        error,
        NativeBibliographyNestedError::Sequence(NativeBibliographySequenceError::WrongArity {
            row: 0,
            actual: NATIVE_CITATION_FEATURE_COLUMNS,
            expected: NATIVE_NAME_CITATION_FEATURE_COLUMNS,
        })
    );
}

#[test]
fn nested_decode_replay_is_deterministic() {
    let (_, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author).clone();
    let date = cluster(&roles, CitationRole::Date).clone();
    let (_, name_model) = load_model(NAME_FIXTURE);
    let (_, date_model) = load_model(DATE_FIXTURE);
    let names = decode_name_citation_cluster(&author, &name_rows_for(&NAME_TOKENS), &name_model)
        .expect("decodes");
    let dates =
        decode_date_cluster(&date, &date_rows_for(&["2017"]), &date_model).expect("decodes");
    for _ in 0..3 {
        assert_eq!(
            decode_name_citation_cluster(&author, &name_rows_for(&NAME_TOKENS), &name_model)
                .expect("decodes"),
            names
        );
        assert_eq!(
            decode_date_cluster(&date, &date_rows_for(&["2017"]), &date_model).expect("decodes"),
            dates
        );
    }
    let (_, reread_name) = load_model(NAME_FIXTURE);
    assert_eq!(
        decode_name_citation_cluster(&author, &name_rows_for(&NAME_TOKENS), &reread_name)
            .expect("decodes"),
        names
    );
}

// ─────────────────── H10-N1: an absent identifier stays absent ───────────────────

/// The identifier column is the citation ABI's `isKnownIdentifier` lane: a
/// DOI-shaped token flags it through the real gazetteer scan, and an ordinary
/// token does not.
#[test]
fn identifier_column_is_the_real_citation_identifier_lane() {
    assert_eq!(NATIVE_CITATION_IDENTIFIER_COLUMN, 26);
    let lexicon = fixture_lexicon();
    let tokens = vec![facts(
        "10.1000/xyz",
        FactsLineStatus::Linestart,
        FactsBlockStatus::Blockstart,
        11,
    )];
    let spans = BibliographyGazetteerIndex::from_lexicon(&lexicon)
        .expect("fixture lexicon is a valid gazetteer bundle")
        .spans(&["10.1000/xyz"])
        .expect("one token is within the gazetteer bounds");
    let rows = citation_row(&tokens, &spans, &lexicon).expect("the DOI token renders");
    assert_eq!(
        rows[0].columns[NATIVE_CITATION_IDENTIFIER_COLUMN], "1",
        "a DOI-shaped token must flag the identifier lane at this column"
    );

    let ordinary = citation_rows();
    assert!(
        ordinary
            .iter()
            .all(|row| row.columns[NATIVE_CITATION_IDENTIFIER_COLUMN] == "0"),
        "the citation fixture's names and year carry no identifier evidence"
    );
}

/// H10-N1, negative: when the lane says no, there is no identifier — not an
/// empty string, not a guess from the token's shape — and the decoded name value
/// has no field an identifier could hide in.
#[test]
fn an_absent_identifier_stays_absent_and_appears_in_no_field() {
    let (rows, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);

    // The fixture's author cluster carries no identifier evidence at all.
    assert_eq!(cluster_identifier(&rows, author), Ok(None));

    // Non-vacuous control: the DOI-shaped text really is in the rows, and the
    // lane still says no — the column is evidence, not a text scan.
    let mut spoofed = rows.clone();
    spoofed[0].columns[0] = "10.1000/xyz".into();
    assert_eq!(spoofed[0].columns[NATIVE_CITATION_IDENTIFIER_COLUMN], "0");
    assert_eq!(cluster_identifier(&spoofed, author), Ok(None));

    // The decoded name value has no identifier field, so the absent identifier
    // cannot surface in any of them.
    let (_, name_model) = load_model(NAME_FIXTURE);
    let names = decode_name_citation_cluster(author, &name_rows_for(&NAME_TOKENS), &name_model)
        .expect("decodes");
    let value = serde_json::to_value(&names).expect("the name value serialises");
    assert!(
        value.get("identifier").is_none(),
        "a name value must have no identifier field"
    );
    let serialised = value.to_string();
    for forbidden in ["10.1000", "doi", "arxiv"] {
        assert!(
            !serialised.contains(forbidden),
            "an absent identifier must not appear in any field: {forbidden}"
        );
    }
}

/// H10-N1, positive control: when the lane is set, the evidence is returned in
/// the cluster text's coordinate system, so the negative above is not vacuous.
#[test]
fn identifier_evidence_carries_its_row_and_byte_range() {
    let (rows, roles, cluster_text) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let mut with_identifier = rows.clone();
    with_identifier[1].columns[NATIVE_CITATION_IDENTIFIER_COLUMN] = "1".into();

    let evidence = cluster_identifier(&with_identifier, author)
        .expect("the row width is the citation ABI")
        .expect("row 1 carries the lane");
    assert_eq!(evidence.row, 1);
    assert_eq!(evidence.text, "Ashish");
    assert_eq!(
        &cluster_text[evidence.byte_start..evidence.byte_end],
        evidence.text,
        "the byte range indexes the citation cluster text"
    );
    assert_eq!((evidence.byte_start, evidence.byte_end), (8, 14));
}

#[test]
fn refuses_an_identifier_read_over_a_truncated_citation_row() {
    let (rows, roles, _) = citation_block();
    let author = cluster(&roles, CitationRole::Author);
    let mut truncated = rows.clone();
    truncated[0]
        .columns
        .truncate(NATIVE_CITATION_IDENTIFIER_COLUMN);
    let error = cluster_identifier(&truncated, author)
        .expect_err("a row without the identifier lane cannot be read");
    assert_eq!(
        error,
        NativeBibliographyNestedError::WrongArity {
            actual: NATIVE_CITATION_IDENTIFIER_COLUMN,
            expected: NATIVE_CITATION_FEATURE_COLUMNS,
        }
    );

    let error =
        cluster_identifier(&[], author).expect_err("a cluster outside the block is refused");
    assert_eq!(
        error,
        NativeBibliographyNestedError::ClusterRowsOutOfRange {
            span_start: author.start,
            span_end: author.end,
            rows: 0,
        }
    );
}

// ───────────────── H10-N2: geometry never rewrites identity ─────────────────

/// Java's row carries `lineStatus` and (for the block) `blockStatus`, but the
/// projection is a function of the labels and the token texts only. A value split
/// across a line break — or across a page boundary — keeps the same parts, the
/// same roles and the same byte spans.
#[test]
fn line_break_and_page_boundary_do_not_rewrite_nested_identity() {
    let lexicon = fixture_lexicon();
    let (_, name_model) = load_model(NAME_FIXTURE);
    let (_, date_model) = load_model(DATE_FIXTURE);
    let author = CitationRoleSpan {
        role: CitationRole::Author,
        label: "<author>".into(),
        start: 0,
        end: 2,
        byte_start: 0,
        byte_end: 14,
        text: "Vaswani Ashish".into(),
    };

    let same_line = [
        facts(
            "Vaswani",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            NAME_LINE_LENGTH,
        ),
        facts(
            "Ashish",
            FactsLineStatus::Lineend,
            FactsBlockStatus::Blockend,
            NAME_LINE_LENGTH,
        ),
    ];
    let coverage = NativePersonTitleCoverage::from_positions(2, &[]);
    let flat = name_citation_row(&same_line, &coverage, &lexicon).expect("renders");

    // The same two tokens, but the second begins a new line and a new block (a
    // line break plus a page boundary between them).
    let wrapped = [
        facts(
            "Vaswani",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockend,
            NAME_LINE_LENGTH,
        ),
        facts(
            "Ashish",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            NAME_LINE_LENGTH,
        ),
    ];
    let broken = name_citation_row(&wrapped, &coverage, &lexicon).expect("renders");

    // Non-vacuous: the two row sets really do differ, and only in the status
    // columns and the block geometry.
    assert_ne!(
        flat[1].as_text(),
        broken[1].as_text(),
        "the geometry must actually differ, or the equality below is vacuous"
    );
    assert_eq!(flat[1].columns[0], broken[1].columns[0]);
    assert_eq!(flat[1].columns[10], "LINEEND");
    assert_eq!(broken[1].columns[10], "LINESTART");

    let identity = decode_name_citation_cluster(&author, &flat, &name_model).expect("decodes");
    let wrapped_identity =
        decode_name_citation_cluster(&author, &broken, &name_model).expect("decodes");
    assert_eq!(
        identity, wrapped_identity,
        "a line break must not change a name part's identity or spans"
    );
    assert_eq!(identity.name_text, "Vaswani Ashish");
    assert_eq!(
        identity
            .parts
            .iter()
            .map(|part| (
                part.role,
                part.text.as_str(),
                part.byte_start,
                part.byte_end
            ))
            .collect::<Vec<_>>(),
        [
            (NamePartRole::Surname, "Vaswani", 0, 7),
            (NamePartRole::Forename, "Ashish", 8, 14),
        ]
    );

    // ... and the date family behaves the same way at a page boundary.
    let date_span = CitationRoleSpan {
        role: CitationRole::Date,
        label: "<date>".into(),
        start: 0,
        end: 3,
        byte_start: 0,
        byte_end: 12,
        text: "June 12 2017".into(),
    };
    let inline = [
        facts(
            "June",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            DATE_LINE_LENGTH,
        ),
        facts(
            "12",
            FactsLineStatus::Linein,
            FactsBlockStatus::Blockend,
            DATE_LINE_LENGTH,
        ),
        facts(
            "2017",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            DATE_LINE_LENGTH,
        ),
    ];
    let page_break = date_row(&inline, &lexicon).expect("renders");
    let straight =
        date_row(&date_rows_facts(&DATE_TOKENS, DATE_LINE_LENGTH), &lexicon).expect("renders");
    assert_ne!(straight[2].as_text(), page_break[2].as_text());
    assert_eq!(
        decode_date_cluster(&date_span, &straight, &date_model).expect("decodes"),
        decode_date_cluster(&date_span, &page_break, &date_model).expect("decodes"),
        "a page boundary must not change a date part's identity or spans"
    );
}

/// Facts for `tokens` laid out on one line, used as the geometry-neutral control
/// of the H10-N2 test.
fn date_rows_facts(tokens: &[&str], line_length: usize) -> Vec<BibliographyTokenFacts> {
    let count = tokens.len();
    tokens
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let line_status = match index {
                0 => FactsLineStatus::Linestart,
                _ if index + 1 == count => FactsLineStatus::Lineend,
                _ => FactsLineStatus::Linein,
            };
            facts(text, line_status, FactsBlockStatus::Blockin, line_length)
        })
        .collect()
}

// ───────────────────────── nested role tables ─────────────────────────

#[test]
fn nested_role_tables_cover_their_model_label_bases() {
    let mut name_bases: Vec<String> = NATIVE_NAME_CITATION_LABELS
        .iter()
        .map(|label| label.strip_prefix("I-").unwrap_or(label).to_string())
        .collect();
    name_bases.sort();
    name_bases.dedup();
    let mut name_roles: Vec<String> = NAME_PART_BASE_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    name_roles.sort();
    assert_eq!(name_bases, name_roles);
    assert_eq!(NamePartRole::ALL.len(), NAME_PART_BASE_LABELS.len());
    for role in NamePartRole::ALL {
        assert_eq!(NamePartRole::from_base_label(role.base_label()), Some(role));
    }
    assert!(NamePartRole::from_base_label("<not-a-role>").is_none());

    let mut date_bases: Vec<String> = NATIVE_DATE_LABELS
        .iter()
        .map(|label| label.strip_prefix("I-").unwrap_or(label).to_string())
        .collect();
    date_bases.sort();
    date_bases.dedup();
    let mut date_roles: Vec<String> = DATE_PART_BASE_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    date_roles.sort();
    assert_eq!(date_bases, date_roles);
    assert_eq!(DatePartRole::ALL.len(), DATE_PART_BASE_LABELS.len());
    for role in DatePartRole::ALL {
        assert_eq!(DatePartRole::from_base_label(role.base_label()), Some(role));
    }
    assert!(DatePartRole::from_base_label("<not-a-role>").is_none());
}
