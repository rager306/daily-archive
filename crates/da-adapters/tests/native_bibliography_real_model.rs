//! Opt-in harness for the real GROBID bibliography CRF models (S04/T07).
//!
//! Model bytes are never committed and never discovered implicitly (D358 —
//! GROBID model assets are not redistributable): the harness reads a
//! caller-owned local resource **root** from the environment and resolves four
//! relative family paths below it. A recorded real-model sequence is *not* an
//! independent oracle, so this harness asserts structural invariants only —
//! the declared label counts, the row arities, vocabulary agreement with the
//! native label tables, and that the artifact passes
//! `validate_loaded_crf_model` — and never a golden label string.
//!
//! # Environment
//!
//! ```text
//! DA_GROBID_MODEL_ROOT=<directory that contains the four family directories>
//! ```
//!
//! Relative to that root the harness expects GROBID's own layout:
//!
//! | family | relative path | labels | row width |
//! |---|---|---|---|
//! | `reference-segmenter` | `reference-segmenter/model.wapiti` | 6 | 28 |
//! | `citation` | `citation/model.wapiti` | 38 | 30 |
//! | `name/citation` | `name/citation/model.wapiti` | 9 | 18 |
//! | `date` | `date/model.wapiti` | 7 | 17 |
//!
//! Unset environment is reported as `MISSING`, which is not a pass: it makes no
//! claim about the real models at all. No absolute path is baked into this file
//! and no model byte is copied into the tree.

use da_adapters::FilesystemCrfModelLoader;
use da_parse::{
    BibliographyAlignment, BibliographySequenceFamily, BibliographyTokenFacts,
    BibliographyTokenInput, FactsBlockStatus, FactsLineStatus, NATIVE_CITATION_FEATURE_COLUMNS,
    NATIVE_CITATION_LABELS, NATIVE_CITATION_SEQUENCE_FAMILY, NATIVE_DATE_FEATURE_COLUMNS,
    NATIVE_DATE_LABELS, NATIVE_DATE_SEQUENCE_FAMILY, NATIVE_NAME_CITATION_FEATURE_COLUMNS,
    NATIVE_NAME_CITATION_LABELS, NATIVE_NAME_CITATION_SEQUENCE_FAMILY,
    NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS, NATIVE_REFERENCE_SEGMENTER_LABELS,
    NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY, NativePersonTitleCoverage, citation_row, date_row,
    decode_bibliography_labels, gazetteer_spans, name_citation_row, reference_segmenter_row,
    render_citation_feature_text, render_date_feature_text, render_name_citation_feature_text,
    render_reference_segmenter_feature_text,
};
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel, validate_loaded_crf_model};
use std::path::PathBuf;

/// Caller-owned resource root; the same opt-in convention as the scalar CRF and
/// header harnesses, one level up (a directory rather than a single file).
const MODEL_ROOT_ENV: &str = "DA_GROBID_MODEL_ROOT";

/// One real family's declared structure.
struct RealModelFamily {
    /// Family name, matching `BibliographySequenceFamily::name`.
    name: &'static str,
    /// Path below [`MODEL_ROOT_ENV`], in GROBID's own resource layout.
    relative_path: &'static str,
    /// Labels the audited artifact declares.
    declared_labels: usize,
    /// Feature-row width the family's ABI must present.
    declared_row_width: usize,
    /// The native family contract (label table and width) under test.
    family: BibliographySequenceFamily,
}

const FAMILIES: [RealModelFamily; 4] = [
    RealModelFamily {
        name: "reference-segmenter",
        relative_path: "reference-segmenter/model.wapiti",
        declared_labels: 6,
        declared_row_width: 28,
        family: NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
    },
    RealModelFamily {
        name: "citation",
        relative_path: "citation/model.wapiti",
        declared_labels: 38,
        declared_row_width: 30,
        family: NATIVE_CITATION_SEQUENCE_FAMILY,
    },
    RealModelFamily {
        name: "name/citation",
        relative_path: "name/citation/model.wapiti",
        declared_labels: 9,
        declared_row_width: 18,
        family: NATIVE_NAME_CITATION_SEQUENCE_FAMILY,
    },
    RealModelFamily {
        name: "date",
        relative_path: "date/model.wapiti",
        declared_labels: 7,
        declared_row_width: 17,
        family: NATIVE_DATE_SEQUENCE_FAMILY,
    },
];

#[test]
fn optional_real_bibliography_models_are_explicitly_missing_or_structurally_validated() {
    let Some(root) = std::env::var_os(MODEL_ROOT_ENV).filter(|value| !value.is_empty()) else {
        eprintln!(
            "MISSING: {MODEL_ROOT_ENV} is not configured; no real GROBID bibliography model claim was made"
        );
        return;
    };
    let root = PathBuf::from(root);

    // The four families are four distinct contracts: identical widths or label
    // counts would mean the family table had been collapsed.
    let mut widths: Vec<usize> = FAMILIES
        .iter()
        .map(|spec| spec.declared_row_width)
        .collect();
    widths.sort_unstable();
    widths.dedup();
    assert_eq!(
        widths.len(),
        FAMILIES.len(),
        "the four bibliography families must declare distinct row widths"
    );
    let mut counts: Vec<usize> = FAMILIES.iter().map(|spec| spec.declared_labels).collect();
    counts.sort_unstable();
    counts.dedup();
    assert_eq!(
        counts.len(),
        FAMILIES.len(),
        "the four bibliography families must declare distinct label counts"
    );

    let lexicon = harness_lexicon();

    for spec in FAMILIES {
        // Declared structure, before touching any bytes: the native contract and
        // this harness's independent expectation must agree.
        assert_eq!(
            spec.family.name, spec.name,
            "native family name drifted for {}",
            spec.relative_path
        );
        assert_eq!(
            spec.family.admissible_labels.len(),
            spec.declared_labels,
            "native label table for {} drifted from the declared {} labels",
            spec.relative_path,
            spec.declared_labels
        );
        assert_eq!(
            spec.family.expected_columns, spec.declared_row_width,
            "native row width for {} drifted from the declared {} columns",
            spec.relative_path, spec.declared_row_width
        );

        let loader = FilesystemCrfModelLoader::new(root.join(spec.relative_path));
        let model = validate_loaded_crf_model(&loader).unwrap_or_else(|error| {
            panic!(
                "configured real {} model ({}) failed validation: {error}",
                spec.name, spec.relative_path
            )
        });
        assert_eq!(
            model.model_type, 2,
            "{} must be a scalar CRF model",
            spec.relative_path
        );
        assert_eq!(
            model.labels.len(),
            spec.declared_labels,
            "real {} model must declare {} labels",
            spec.name,
            spec.declared_labels
        );

        // Vocabulary agreement, both directions: the artifact may not declare a
        // label the native table does not know, and the native table may not
        // declare a label the artifact omits.
        let undeclared = model
            .labels
            .iter()
            .map(String::as_str)
            .filter(|label| !spec.family.admissible_labels.contains(label))
            .collect::<Vec<_>>();
        assert!(
            undeclared.is_empty(),
            "real {} model declares labels the native vocabulary does not know: {undeclared:?}",
            spec.name
        );
        let missing = spec
            .family
            .admissible_labels
            .iter()
            .copied()
            .filter(|label| !model.labels.iter().any(|model_label| model_label == label))
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "real {} model is missing declared labels: {missing:?}",
            spec.name
        );

        // The hand-built ABI must reach the real observation table with the
        // declared width and decode one label per token.
        let tokens = tiny_tokens();
        let row_texts = family_row_texts(spec.name, &tokens, &lexicon)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            row_texts.len(),
            tokens.len(),
            "one {} row per token",
            spec.name
        );
        assert!(
            row_texts
                .iter()
                .all(|row| row.split(' ').count() == spec.declared_row_width),
            "every {} row must carry the declared {} columns",
            spec.name,
            spec.declared_row_width
        );

        let decode =
            decode_bibliography_labels(&row_texts, &spec.family, &model).unwrap_or_else(|error| {
                panic!(
                    "native runner rejected the real {} model: {error}",
                    spec.name
                )
            });
        assert_eq!(
            decode.labels.len(),
            tokens.len(),
            "label sequence length must equal the token count for {}",
            spec.name
        );
        let unknown = decode
            .labels
            .iter()
            .map(String::as_str)
            .filter(|label| !spec.family.admissible_labels.contains(label))
            .collect::<Vec<_>>();
        assert!(
            unknown.is_empty(),
            "real {} model emitted labels outside the declared set: {unknown:?}",
            spec.name
        );

        eprintln!(
            "PASS: real {} model validated (labels={}, observations={}, nftr={}, row_width={}, decoded={:?})",
            spec.name,
            model.labels.len(),
            model.observations.len(),
            model.nftr,
            spec.declared_row_width,
            decode.labels
        );
    }

    eprintln!(
        "PASS: four real bibliography families structurally validated under {MODEL_ROOT_ENV}"
    );
}

/// A three-token block: a surname, a forename and a year. Enough for every
/// family to produce a decodable row.
fn tiny_tokens() -> Vec<BibliographyTokenFacts> {
    let words = ["Vaswani", "Ashish", "2017"];
    let last = words.len() - 1;
    words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let line_status = if index == 0 {
                FactsLineStatus::Linestart
            } else if index == last {
                FactsLineStatus::Lineend
            } else {
                FactsLineStatus::Linein
            };
            let block_status = if index == 0 {
                FactsBlockStatus::Blockstart
            } else if index == last {
                FactsBlockStatus::Blockend
            } else {
                FactsBlockStatus::Blockin
            };
            BibliographyTokenFacts::from_token(&BibliographyTokenInput {
                text: (*word).into(),
                block_status,
                line_status,
                alignment: BibliographyAlignment::Alignedleft,
                line_length: words.len(),
                superscript: false,
            })
        })
        .collect()
}

/// Minimal project-owned bundle; the harness reads no GROBID lexicon bytes.
fn harness_lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["Vaswani".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "bibliography".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

/// Render the exact row payloads `map_input` consumes for one family.
fn family_row_texts(
    family: &str,
    tokens: &[BibliographyTokenFacts],
    lexicon: &LexiconBundle,
) -> Result<Vec<String>, String> {
    let lines = match family {
        "reference-segmenter" => {
            let rows = reference_segmenter_row(tokens, lexicon, tokens.len()).map_err(|error| {
                format!("reference-segmenter ABI refused the harness tokens: {error}")
            })?;
            render_reference_segmenter_feature_text(&rows)
                .map_err(|error| format!("reference-segmenter render refused: {error}"))?
        }
        "citation" => {
            let texts: Vec<String> = tokens.iter().map(|token| token.text.clone()).collect();
            let spans = gazetteer_spans(lexicon, &texts)
                .map_err(|error| format!("gazetteer pass failed on the harness tokens: {error}"))?;
            let rows = citation_row(tokens, &spans, lexicon)
                .map_err(|error| format!("citation ABI refused the harness tokens: {error}"))?;
            render_citation_feature_text(&rows)
                .map_err(|error| format!("citation render refused: {error}"))?
        }
        "name/citation" => {
            let coverage = NativePersonTitleCoverage::from_positions(tokens.len(), &[]);
            let rows = name_citation_row(tokens, &coverage, lexicon).map_err(|error| {
                format!("name/citation ABI refused the harness tokens: {error}")
            })?;
            render_name_citation_feature_text(&rows)
                .map_err(|error| format!("name/citation render refused: {error}"))?
        }
        "date" => {
            let rows = date_row(tokens, lexicon)
                .map_err(|error| format!("date ABI refused the harness tokens: {error}"))?;
            render_date_feature_text(&rows)
                .map_err(|error| format!("date render refused: {error}"))?
        }
        other => {
            return Err(format!(
                "unknown bibliography family in the harness: {other}"
            ));
        }
    };
    let texts: Vec<String> = lines.lines().map(str::to_owned).collect();
    assert_eq!(
        texts.len(),
        tokens.len(),
        "rendered row count must equal the token count"
    );
    Ok(texts)
}

/// The declared widths, restated as an independent expectation so a silent edit
/// of a shared constant cannot move the denominator.
#[test]
fn declared_bibliography_widths_stay_frozen() {
    assert_eq!(NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS, 28);
    assert_eq!(NATIVE_CITATION_FEATURE_COLUMNS, 30);
    assert_eq!(NATIVE_NAME_CITATION_FEATURE_COLUMNS, 18);
    assert_eq!(NATIVE_DATE_FEATURE_COLUMNS, 17);
    assert_eq!(NATIVE_REFERENCE_SEGMENTER_LABELS.len(), 6);
    assert_eq!(NATIVE_CITATION_LABELS.len(), 38);
    assert_eq!(NATIVE_NAME_CITATION_LABELS.len(), 9);
    assert_eq!(NATIVE_DATE_LABELS.len(), 7);
    for spec in FAMILIES {
        assert_eq!(spec.declared_labels, spec.family.admissible_labels.len());
        assert_eq!(spec.declared_row_width, spec.family.expected_columns);
    }
}

/// An unknown family is a typed refusal, never a silently mis-shaped decode.
#[test]
fn unknown_family_is_refused_rather_than_decoded() {
    let tokens = tiny_tokens();
    let lexicon = harness_lexicon();
    let error = family_row_texts("figure", &tokens, &lexicon)
        .expect_err("an unknown family must be a typed refusal");
    assert!(
        error.contains("figure"),
        "the refusal must name the family it refused: {error}"
    );
    let error = family_row_texts("", &tokens, &lexicon)
        .expect_err("an empty family name must be a typed refusal");
    assert!(error.contains("unknown bibliography family"));

    // Positive control: the four known families render at exactly their
    // declared widths without any model resource.
    for spec in FAMILIES {
        let rows = family_row_texts(spec.name, &tokens, &lexicon).expect(spec.name);
        assert_eq!(rows.len(), tokens.len());
        assert!(
            rows.iter()
                .all(|row| row.split(' ').count() == spec.declared_row_width),
            "{} must render {} columns",
            spec.name,
            spec.declared_row_width
        );
    }
}
