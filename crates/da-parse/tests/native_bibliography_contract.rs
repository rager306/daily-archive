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
    FactsBlockStatus, FactsLineStatus, MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS,
    NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_CITATION_LABEL_COUNT, NATIVE_CITATION_LABELS,
    NATIVE_CITATION_SEQUENCE_FAMILY, NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
    NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT, NATIVE_REFERENCE_SEGMENTER_LABELS,
    NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY, NativeBibliographySequenceError,
    ReferenceSegmentKind, ReferenceSegmenterFeatureRow, citation_row, decode_bibliography_labels,
    decode_citation_sequence, decode_reference_sequence, reference_segmenter_row,
};
use da_ports::{CrfModel, FeatureTemplate, LexiconBundle, LexiconChannel};
use std::path::{Path, PathBuf};

const REFERENCE_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti";
const CITATION_FIXTURE: &str =
    "crates/da-parse/tests/fixtures/native_bibliography_citation_synthetic.wapiti";

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
