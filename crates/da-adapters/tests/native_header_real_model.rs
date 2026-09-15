//! Opt-in harness for the real GROBID 0.9.0 header CRF model.
//!
//! Model bytes are never committed and never discovered implicitly (D358 —
//! GROBID model assets are not redistributable): the harness reads a
//! caller-owned local resource path from the environment and reports explicit
//! missingness when it is absent. A recorded real-model sequence is *not* an
//! independent oracle, so this harness asserts structural invariants only —
//! frozen vocabulary size, vocabulary membership, and sequence/token arity —
//! and never a golden label string (507-03-RESEARCH.md §6).
//!
//! The harness also probes the shipped sequence runner on the real model and
//! requires it to tile the rows into exactly the runs computed here. Runs are
//! grouped by *base* field name, the way GROBID's `writeField` does; the `I-`
//! spelling is another spelling of the same field, not a boundary marker.
//! `507-03-RESEARCH.md` §3 asserted the inverse (a BIO-like scheme), which the
//! runner and this harness both followed until T03 corrected the rule against
//! the primary source; the harness expectation is corrected here for the same
//! reason.
//!
//! Unset environment is reported as `MISSING`, which is not a pass: it makes no
//! claim about the real model at all.

use da_adapters::FilesystemCrfModelLoader;
use da_parse::{
    BlockStatus, HeaderAlignment, HeaderFeatureRow, HeaderFeatureToken, HeaderFontStatistics,
    LineStatus, NATIVE_HEADER_FEATURE_COLUMNS, NATIVE_HEADER_LABELS, NativeHeaderDecodeOutcome,
    decode_header_sequence, render_header_features,
};
use da_ports::{
    FeatureTemplate, LexiconBundle, LexiconChannel, map_input, validate_loaded_crf_model,
    viterbi_decode,
};

/// Same opt-in key as the scalar CRF smoke harness (S02 convention). Paths are
/// always supplied by the caller; none are baked into this file.
const MODEL_PATH_ENV: &str = "DA_GROBID_CRF_MODEL_PATH";

/// Frozen GROBID 0.9.0 header vocabulary: 22 fields × {plain, `I-`} = 44 labels
/// (507-03-RESEARCH.md §3). Duplicated here as an independent expectation so a
/// silent edit of the shared constant cannot move the denominator.
const DECLARED_HEADER_LABEL_COUNT: usize = 44;

#[test]
fn optional_real_header_model_is_explicitly_missing_or_structurally_validated() {
    let Some(path) = std::env::var_os(MODEL_PATH_ENV).filter(|value| !value.is_empty()) else {
        eprintln!(
            "MISSING: {MODEL_PATH_ENV} is not configured; no real GROBID header model claim was made"
        );
        return;
    };

    let loader = FilesystemCrfModelLoader::new(path);
    let model = validate_loaded_crf_model(&loader)
        .unwrap_or_else(|error| panic!("configured real header model failed validation: {error}"));

    assert_eq!(model.model_type, 2, "resource must be a scalar CRF model");

    // Invariant: the loaded vocabulary is exactly the declared header set.
    assert_eq!(
        NATIVE_HEADER_LABELS.len(),
        DECLARED_HEADER_LABEL_COUNT,
        "declared native header vocabulary drifted from the frozen 44 labels"
    );
    assert_eq!(
        model.labels.len(),
        DECLARED_HEADER_LABEL_COUNT,
        "real header model must declare {DECLARED_HEADER_LABEL_COUNT} labels"
    );
    let undeclared = model
        .labels
        .iter()
        .map(String::as_str)
        .filter(|label| !NATIVE_HEADER_LABELS.contains(label))
        .collect::<Vec<_>>();
    assert!(
        undeclared.is_empty(),
        "real header model declares labels the native vocabulary does not know: {undeclared:?}"
    );
    let missing = NATIVE_HEADER_LABELS
        .iter()
        .copied()
        .filter(|label| {
            !model
                .labels
                .iter()
                .any(|model_label| model_label.as_str() == *label)
        })
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "real header model is missing declared labels: {missing:?}"
    );

    // The hand-built 29-column ABI must reach the real observation table and
    // decode one label per token.
    let tokens = tiny_header_tokens();
    let rows = render_header_features(&tokens, header_fonts(), &header_lexicon())
        .unwrap_or_else(|error| panic!("hand-built header rows must render: {error}"));
    assert_eq!(rows.len(), tokens.len());
    assert!(
        rows.iter()
            .all(|row| row.width() == NATIVE_HEADER_FEATURE_COLUMNS),
        "every rendered row must carry the 29-column ABI"
    );

    let input = header_input(&rows);
    let positions = map_input(&model, &input)
        .unwrap_or_else(|error| panic!("real header model rejected the rendered ABI: {error}"));
    let decoded = viterbi_decode(&model, &positions)
        .unwrap_or_else(|error| panic!("real header model could not decode the ABI: {error}"));
    assert_eq!(
        decoded.labels.len(),
        tokens.len(),
        "label sequence length must equal the decoded token count"
    );
    let unknown = decoded
        .labels
        .iter()
        .map(String::as_str)
        .filter(|label| !NATIVE_HEADER_LABELS.contains(label))
        .collect::<Vec<_>>();
    assert!(
        unknown.is_empty(),
        "real header model emitted labels outside the declared set: {unknown:?}"
    );

    // Runs are grouped by base field name, exactly as GROBID's `writeField`
    // does: `base(current) == base(previous)` continues the run, any other base
    // opens a new one, and the `I-` prefix itself neither opens nor continues.
    // Whether the pinned 0.9.0 artifact happens to spell run starts with `I-`
    // is an observation about that artifact, not the grouping rule, so it is
    // reported rather than asserted.
    let runs = grobid_runs(&decoded.labels);
    eprintln!(
        "run spellings on header-like rows: {} of {} runs start with I-",
        runs.iter()
            .filter(|run| run.start_label.starts_with("I-"))
            .count(),
        runs.len()
    );

    // The shipped native runner is probed on the real model as an integration
    // check: it must accept the sequence and tile the rows into exactly the
    // runs grouped independently here (T03's corrected rule).
    let decode = match decode_header_sequence(&rows, &model) {
        NativeHeaderDecodeOutcome::Accepted { decode } => decode,
        NativeHeaderDecodeOutcome::Rejected { error } => {
            panic!("the native runner rejected the real header model sequence: {error}")
        }
    };
    assert_eq!(decode.labels, decoded.labels);
    assert!(
        decode
            .labels
            .iter()
            .all(|label| NATIVE_HEADER_LABELS.contains(&label.as_str()))
    );
    assert_eq!(
        decode
            .fields
            .iter()
            .map(|field| (field.label.as_str(), field.start, field.end))
            .collect::<Vec<_>>(),
        runs.iter()
            .map(|run| (run.base, run.start, run.end))
            .collect::<Vec<_>>(),
        "runner spans must match the independently grouped real-model runs"
    );
    eprintln!(
        "native runner accepted the real model sequence ({} field spans)",
        decode.fields.len()
    );

    eprintln!(
        "PASS: real header model validated (labels={}, observations={}, nftr={}, tokens={}, runs={})",
        model.labels.len(),
        model.observations.len(),
        model.nftr,
        tokens.len(),
        runs.len()
    );
    eprintln!("observed labels: {:?}", decoded.labels);
    eprintln!(
        "observed runs (grouped by base name): {:?}",
        runs.iter()
            .map(|run| (run.base, run.start, run.end, run.start_label))
            .collect::<Vec<_>>()
    );
}

/// A four-line header: title / author / affiliation / date.
fn tiny_header_tokens() -> Vec<HeaderFeatureToken> {
    let mut tokens = Vec::new();
    push_line(
        &mut tokens,
        &["Attention", "Is", "All", "You", "Need"],
        14.0,
        BlockStatus::Blockstart,
    );
    push_line(
        &mut tokens,
        &["Ashish", "Vaswani"],
        11.0,
        BlockStatus::Blockin,
    );
    push_line(&mut tokens, &["Google", "Brain"], 9.0, BlockStatus::Blockin);
    push_line(&mut tokens, &["2017"], 9.0, BlockStatus::Blockend);
    tokens
}

fn push_line(
    out: &mut Vec<HeaderFeatureToken>,
    words: &[&str],
    font_size: f32,
    block_status: BlockStatus,
) {
    let last = words.len().saturating_sub(1);
    for (index, word) in words.iter().enumerate() {
        let line_status = if index == 0 {
            LineStatus::Linestart
        } else if index == last {
            LineStatus::Lineend
        } else {
            LineStatus::Linein
        };
        out.push(HeaderFeatureToken {
            text: (*word).into(),
            block_status,
            line_status,
            alignment: HeaderAlignment::LineIndent,
            font_size,
            bold: font_size >= 14.0,
            italic: false,
        });
    }
}

/// Aggregate font facts over the piece above; must satisfy
/// `smallest <= average <= largest` and stay strictly positive.
fn header_fonts() -> HeaderFontStatistics {
    HeaderFontStatistics {
        largest_font: 14.0,
        average_font: 11.5,
        smallest_font: 9.0,
    }
}

/// Minimal project-owned bundle. Lexicon contents only flip eight binary
/// columns of the ABI; they cannot change the structural invariants here, and
/// no GROBID lexicon bytes are read or committed.
fn header_lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["Ashish".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

/// The exact text the decoder consumes: one row per line (see `HeaderFeatureRow::as_text`).
fn header_input(rows: &[HeaderFeatureRow]) -> String {
    rows.iter()
        .map(HeaderFeatureRow::as_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// One observed field run grouped the way GROBID's `writeField` groups them:
/// rows continue the open run while their *base* name equals the run's base
/// name, and any base-name change opens a new run. `I-<field>` and `<field>`
/// are spellings of one field, so a run's start label may be either.
struct ObservedRun<'a> {
    base: &'a str,
    start: usize,
    end: usize,
    start_label: &'a str,
}

fn grobid_runs(labels: &[String]) -> Vec<ObservedRun<'_>> {
    let mut open: Vec<ObservedRun<'_>> = Vec::new();
    for (row, label) in labels.iter().enumerate() {
        let base = label.strip_prefix("I-").unwrap_or(label.as_str());
        let continues = open.last().is_some_and(|run| run.base == base);
        if continues {
            continue;
        }
        if let Some(run) = open.last_mut() {
            run.end = row;
        }
        open.push(ObservedRun {
            base,
            start: row,
            end: labels.len(),
            start_label: label.as_str(),
        });
    }
    open
}

/// The grouping rule needs no model, so its polarity is pinned here rather than
/// only on the opt-in path. These expectations are derived from GROBID's
/// `writeField` (base-name equality), not from any decoder in this workspace.
#[cfg(test)]
mod grouping_rule {
    use super::{ObservedRun, grobid_runs};

    fn labels(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|label| (*label).to_string()).collect()
    }

    fn spans<'a>(runs: &'a [ObservedRun<'a>]) -> Vec<(&'a str, usize, usize)> {
        runs.iter()
            .map(|run| (run.base, run.start, run.end))
            .collect()
    }

    /// The premise research §3 asserted (bare opens, `I-` continues) is not
    /// GROBID's: two `I-<field>` rows of one field are one run.
    #[test]
    fn repeated_i_prefixed_rows_of_one_field_form_a_single_run() {
        let labels = labels(&["I-<author>", "I-<author>", "<author>"]);
        let runs = grobid_runs(&labels);
        assert_eq!(spans(&runs), [("<author>", 0, 3)]);
    }

    /// One run may begin with a bare label while another begins with `I-`; the
    /// prefix is a spelling, never a boundary.
    #[test]
    fn a_base_name_change_opens_the_next_run_whatever_spelling_it_uses() {
        let labels = labels(&["<title>", "I-<title>", "I-<author>", "<affiliation>"]);
        let runs = grobid_runs(&labels);
        assert_eq!(
            spans(&runs),
            [
                ("<title>", 0, 2),
                ("<author>", 2, 3),
                ("<affiliation>", 3, 4),
            ]
        );
    }

    /// The real 0.9.0 author run observed by T06: one `I-<author>` opening
    /// followed by nine bare rows is a single span.
    #[test]
    fn the_observed_real_model_author_run_is_one_span() {
        let labels = labels(&[
            "I-<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
            "<author>",
        ]);
        let runs = grobid_runs(&labels);
        assert_eq!(spans(&runs), [("<author>", 0, 10)]);
        assert_eq!(runs[0].start_label, "I-<author>");
    }

    /// Grouping depends on base names only, so rewriting every label to its
    /// base spelling must not change the tiling. The inverted rule this file
    /// used to encode fails exactly this property.
    #[test]
    fn grouping_is_invariant_under_stripping_every_i_prefix() {
        let emitted = labels(&[
            "I-<author>",
            "I-<author>",
            "<title>",
            "I-<title>",
            "<author>",
        ]);
        let stripped = emitted
            .iter()
            .map(|label| label.strip_prefix("I-").unwrap_or(label).to_string())
            .collect::<Vec<_>>();

        let from_emitted = grobid_runs(&emitted);
        let from_stripped = grobid_runs(&stripped);
        assert_eq!(spans(&from_emitted), spans(&from_stripped));
        assert_eq!(
            spans(&from_emitted),
            [("<author>", 0, 2), ("<title>", 2, 4), ("<author>", 4, 5)]
        );
    }
}
