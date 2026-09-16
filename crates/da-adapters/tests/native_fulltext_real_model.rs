//! Opt-in harness for the real GROBID 0.9.0 fulltext (body) CRF model (S05/T05).
//!
//! Model bytes are never committed and never discovered implicitly (D358 —
//! GROBID model assets are not redistributable): the harness reads a
//! caller-owned local resource **root** from the environment and resolves one
//! relative family path below it. A recorded real-model sequence is *not* an
//! independent oracle, so this harness asserts structural invariants only —
//! the declared observation width, the label vocabulary against the native
//! tables, the resource budget, and that the resource passes
//! `validate_loaded_crf_model` — and never a golden label string.
//!
//! # Environment
//!
//! ```text
//! DA_GROBID_MODEL_ROOT=<directory that contains the family directories>
//! ```
//!
//! Relative to that root the harness expects GROBID's own layout
//! (`fulltext/model.wapiti`). No absolute path is baked into this file, no
//! default root exists, and no model byte is copied into the tree. Unset
//! environment is reported as `MISSING`, which is not a pass: it makes no claim
//! about the real model at all.
//!
//! # Order of work, and why it is this order
//!
//! 1. hard guards — the resource must resolve outside the workspace (D358) and
//!    stay inside the declared 128 MiB budget;
//! 2. a **lexical read of the declared header** (`#mdl#`, `#rdr#`, the
//!    `%x[<offset>,<column>]` templates, the label block and both `#qrk#`
//!    counts) taken from a bounded prefix of the file. This runs *before* the
//!    production reader so every divergence is reported even when that reader
//!    refuses the artifact outright, which is the observed state of the shipped
//!    0.9.0 fulltext model;
//! 3. fail closed on a divergence — the declared observation width must be
//!    covered by the printed 27-column ABI, the label base set must equal the
//!    native dictionary, and the declared observation count must fit the
//!    production reader's budget. Any violation is a panic naming the offending
//!    values, never a warning and never a silently narrower dictionary;
//! 4. on a conforming artifact, load it through `validate_loaded_crf_model`,
//!    drive the printed ABI into the real observation table with `map_input`,
//!    and report the decode outcome.
//!
//! # What the shipped artifact is observed to declare
//!
//! The GROBID 0.9.0 fulltext model declares 24 labels over twelve bases, and
//! that base set contains `<section_marker>` where the native dictionary
//! transcribed from the vendored Java source (`data/parse/native_body_accounting.yaml`,
//! `fulltext_labels`) has `<item>`. It also declares 2 066 170 observations,
//! above the production reader's `MAX_CRF_OBSERVATIONS`. Both divergences are
//! reported and fail the opt-in run by design: the band must not absorb a
//! dictionary or a reader-budget change silently.

use da_adapters::FilesystemCrfModelLoader;
use da_domain::scalar_crf::{MAX_CRF_MODEL_BYTES, MAX_CRF_OBSERVATIONS};
use da_parse::{
    FulltextCalloutType, NATIVE_FULLTEXT_BASE_LABELS, NATIVE_FULLTEXT_FEATURE_COLUMNS,
    NATIVE_FULLTEXT_LABELS, NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX, NativeFulltextDecodeOutcome,
    NativeFulltextPageToken, NativeFulltextTokenText, check_fulltext_feature_columns,
    decode_fulltext_sequence, glue_fulltext_reading_order,
};
use da_ports::{map_input, validate_loaded_crf_model};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Caller-owned resource root; the same opt-in convention as the scalar CRF,
/// header and bibliography harnesses (a directory of family directories).
const MODEL_ROOT_ENV: &str = "DA_GROBID_MODEL_ROOT";

/// GROBID's own relative resource path for the body model.
const FULLTEXT_RELATIVE_PATH: &str = "fulltext/model.wapiti";

/// The printed ABI width, restated as an independent expectation so a silent
/// edit of the shared constant cannot move the denominator.
const DECLARED_ROW_WIDTH: usize = 27;

/// The frozen model's declared label count (twelve bases × two spellings).
const DECLARED_LABEL_COUNT: usize = 24;

/// The frozen model's declared base-label count.
const DECLARED_BASE_LABEL_COUNT: usize = 12;

/// The model resource budget, restated independently of the loader constant.
const DECLARED_MODEL_BYTES_LIMIT: usize = 128 * 1024 * 1024;

/// How much of the file the lexical header read is allowed to touch. The
/// shipped artifact's header is 118 lines; the guard fails rather than silently
/// truncating a header.
const HEADER_PROBE_BYTES: usize = 64 * 1024;

/// The structural facts a Wapiti model file declares in its header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DeclaredHeader {
    model_type: Option<u32>,
    /// `#mdl#`'s third field — the reader's `nftr`.
    feature_count: Option<usize>,
    /// `#rdr#`'s first field — the declared pattern count.
    declared_patterns: Option<usize>,
    patterns: Vec<String>,
    /// The label block count, then the labels themselves.
    declared_labels: Option<usize>,
    labels: Vec<String>,
    /// The second `#qrk#` count — the observation table size.
    declared_observations: Option<usize>,
    /// True when the second `#qrk#` line was reached inside the probe budget.
    complete: bool,
}

/// Lexically read the declared header from a bounded prefix.
///
/// Only the declaration is read: no weight is touched, so the parse stays valid
/// even for an artifact the production reader refuses.
fn parse_declared_header(bytes: &[u8]) -> DeclaredHeader {
    #[derive(PartialEq)]
    enum Mode {
        Patterns,
        Labels,
    }

    let mut header = DeclaredHeader::default();
    let mut mode = Mode::Patterns;
    for raw in bytes.split(|byte| *byte == b'\n') {
        let line = String::from_utf8_lossy(raw);
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("#mdl#") {
            let mut fields = rest.split('#');
            header.model_type = fields.next().and_then(|value| value.parse().ok());
            header.feature_count = fields.next().and_then(|value| value.parse().ok());
            continue;
        }
        if let Some(rest) = line.strip_prefix("#rdr#") {
            header.declared_patterns = rest.split('/').next().and_then(|value| value.parse().ok());
            continue;
        }
        if let Some(rest) = line.strip_prefix("#qrk#") {
            let count = rest.trim().parse::<usize>().ok();
            match mode {
                Mode::Patterns => {
                    header.declared_labels = count;
                    mode = Mode::Labels;
                }
                Mode::Labels => {
                    header.declared_observations = count;
                    header.complete = true;
                    break;
                }
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let payload = declaration_payload(line);
        match mode {
            Mode::Patterns => header.patterns.push(payload.to_owned()),
            Mode::Labels => header.labels.push(payload.to_owned()),
        }
    }
    header
}

/// The payload of one Wapiti declaration line.
///
/// `12:u00:%x[-4,0],` → `%x[-4,0]` and `1:b,` → `b`: the whole line is trimmed
/// of its length prefix (and of the pattern name when it carries one) and of the
/// trailing comma.
fn declaration_payload(line: &str) -> &str {
    let rest = match line.split_once(':') {
        Some((_, rest)) => rest,
        None => line,
    };
    let payload = match rest.split_once(':') {
        Some((_, payload)) => payload,
        None => rest,
    };
    payload.trim_end_matches(',')
}

/// Read at most `limit` bytes from the head of the resource.
fn read_header_prefix(path: &Path, limit: usize) -> Vec<u8> {
    let file = fs::File::open(path)
        .unwrap_or_else(|error| panic!("the fulltext model resource must open: {error}"));
    let mut buffer = Vec::new();
    file.take(limit as u64)
        .read_to_end(&mut buffer)
        .unwrap_or_else(|error| panic!("the fulltext model header must read: {error}"));
    buffer
}

/// Every column index the model's `%x[<offset>,<column>]` templates reference.
///
/// The parser is deliberately a plain scan over the template sources: it reads
/// the declaration the artifact carries rather than a guess about it, and it is
/// unit-tested below without any model bytes.
fn referenced_columns(sources: &[String]) -> BTreeSet<usize> {
    let mut columns = BTreeSet::new();
    for source in sources {
        let mut rest = source.as_str();
        while let Some(at) = rest.find("%x[") {
            let after = &rest[at + 3..];
            let Some(close) = after.find(']') else {
                break;
            };
            if let Some((_, column)) = after[..close].split_once(',')
                && let Ok(index) = column.trim().parse::<usize>()
            {
                columns.insert(index);
            }
            rest = &after[close..];
        }
    }
    columns
}

/// Base label of one spelling (`I-` is a spelling prefix, never a base).
fn base_of(label: &str) -> &str {
    label.strip_prefix("I-").unwrap_or(label)
}

/// The divergence between a model's label vocabulary and the native base set.
///
/// Returns `(missing, unexpected)` over *base* labels, so a model that declares
/// only one spelling of a base is not reported as diverging: the spelling mix is
/// an artifact of the training run, while a base label that appears or vanishes
/// is a change of dictionary.
fn base_set_divergence(model_labels: &[String]) -> (Vec<String>, Vec<String>) {
    let native: BTreeSet<&str> = NATIVE_FULLTEXT_BASE_LABELS.iter().copied().collect();
    let declared: BTreeSet<&str> = model_labels.iter().map(|label| base_of(label)).collect();
    let mut missing: Vec<String> = native
        .difference(&declared)
        .map(|label| (*label).to_owned())
        .collect();
    let mut unexpected: Vec<String> = declared
        .difference(&native)
        .map(|label| (*label).to_owned())
        .collect();
    missing.sort_unstable();
    unexpected.sort_unstable();
    (missing, unexpected)
}

/// The path below the caller's root must be relative, parent-free and named.
fn assert_relative_model_path() {
    let path = Path::new(FULLTEXT_RELATIVE_PATH);
    assert!(
        !path.is_absolute(),
        "the harness must resolve a relative path below {MODEL_ROOT_ENV}: {FULLTEXT_RELATIVE_PATH}"
    );
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "the relative model path must not contain a root or parent component: {FULLTEXT_RELATIVE_PATH}"
    );
    assert!(path.ends_with("model.wapiti"));
}

/// A four-block body: head, paragraph, figure caption and a citation marker.
///
/// Built through the glue so the rows the model sees are the printed ABI in
/// document order, exactly as the production band would present them.
fn probe_tokens() -> Vec<NativeFulltextPageToken> {
    let blocks: [(&str, &[&str]); 4] = [
        ("INTRODUCTION", &[]),
        (
            "Large",
            &["language", "models", "have", "enabled", "development"],
        ),
        ("Figure", &["1", ":", "Overview", "of", "the", "band"]),
        ("[12]", &[]),
    ];
    let mut words: Vec<Vec<&str>> = Vec::new();
    for (first, rest) in blocks {
        let mut block = vec![first];
        block.extend_from_slice(rest);
        words.push(block);
    }

    let heights: Vec<f64> = words
        .iter()
        .map(|block| 12.0 * block.len() as f64)
        .collect();
    let mut block_y = Vec::new();
    let mut cursor = 72.0;
    for height in &heights {
        block_y.push(cursor);
        cursor += height + 12.0;
    }
    let page_height =
        block_y.last().copied().unwrap_or(72.0) + heights.last().copied().unwrap_or(0.0) + 72.0;

    let mut out = Vec::new();
    for (ordinal, block) in words.iter().enumerate() {
        let count = block.len() as u32;
        for (within, text) in block.iter().enumerate() {
            out.push(NativeFulltextPageToken {
                page: 1,
                page_height,
                block: ordinal as u32,
                block_y: block_y[ordinal],
                block_height: heights[ordinal],
                within_block: within as u32,
                block_token_count: count,
                x: 72.0,
                width: 8.0,
                y: block_y[ordinal] + 12.0 * within as f64,
                font: "Times-Roman".into(),
                font_size: 10.0,
                bold: false,
                italic: false,
                bitmap_around: false,
                callout_type: FulltextCalloutType::Unknown,
                callout_known: false,
                superscript: false,
                text: NativeFulltextTokenText::Word {
                    text: (*text).to_owned(),
                },
            });
        }
    }
    out
}

fn rows_text(rows: &[da_parse::FulltextFeatureRow]) -> String {
    rows.iter()
        .map(da_parse::FulltextFeatureRow::as_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The declared surface is pinned without any resource, so a silent move of the
/// constants is caught even when no model is configured.
#[test]
fn declared_fulltext_surface_stays_frozen() {
    assert_eq!(DECLARED_ROW_WIDTH, NATIVE_FULLTEXT_FEATURE_COLUMNS);
    assert_eq!(NATIVE_FULLTEXT_FEATURE_COLUMNS, 27);
    assert_eq!(NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX, 26);
    assert_eq!(NATIVE_FULLTEXT_LABELS.len(), DECLARED_LABEL_COUNT);
    assert_eq!(NATIVE_FULLTEXT_BASE_LABELS.len(), DECLARED_BASE_LABEL_COUNT);
    assert_eq!(DECLARED_MODEL_BYTES_LIMIT, MAX_CRF_MODEL_BYTES);
    assert_eq!(DECLARED_MODEL_BYTES_LIMIT, 128 * 1024 * 1024);
    assert_eq!(check_fulltext_feature_columns(DECLARED_ROW_WIDTH), Ok(()));
    assert!(check_fulltext_feature_columns(DECLARED_ROW_WIDTH + 1).is_err());
    assert_relative_model_path();
    assert_eq!(
        Path::new(FULLTEXT_RELATIVE_PATH).components().count(),
        2,
        "the family path is one directory component plus the resource name"
    );
}

/// The divergence detector and the header reader are the invariants the opt-in
/// path enforces, so their polarity is pinned here on synthetic bytes and
/// synthetic vocabularies (no model resource involved).
#[test]
fn label_divergence_columns_and_header_reading_are_detected_without_model_bytes() {
    // The native dictionary agrees with itself.
    let native: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    assert_eq!(base_set_divergence(&native), (Vec::new(), Vec::new()));

    // A bare-only spelling of every base is the *same* dictionary: the spelling
    // is an artifact, not a label.
    let bare: Vec<String> = NATIVE_FULLTEXT_BASE_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    assert_eq!(base_set_divergence(&bare), (Vec::new(), Vec::new()));

    // Swapping one base for another is a divergence, and it is named.
    let swapped: Vec<String> = NATIVE_FULLTEXT_BASE_LABELS
        .iter()
        .map(|label| {
            if *label == "<item>" {
                "<section_marker>".to_owned()
            } else {
                (*label).to_owned()
            }
        })
        .collect();
    let (missing, unexpected) = base_set_divergence(&swapped);
    assert_eq!(missing, vec!["<item>".to_owned()]);
    assert_eq!(unexpected, vec!["<section_marker>".to_owned()]);

    // A vocabulary that drops a base still reports the gap.
    let reduced: Vec<String> = NATIVE_FULLTEXT_BASE_LABELS[1..]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let (missing, unexpected) = base_set_divergence(&reduced);
    assert_eq!(missing, vec!["<paragraph>".to_owned()]);
    assert!(unexpected.is_empty());

    // Column span: the templates declare the columns the model reads, and a
    // template that reaches past the printed ABI is visible before any load.
    let sources: Vec<String> = vec![
        "u00:%x[-4,0],".to_owned(),
        "u04:%x[0,0],".to_owned(),
        "uN5:%x[2,26],".to_owned(),
        "b".to_owned(),
    ];
    let columns = referenced_columns(&sources);
    assert_eq!(columns.iter().copied().next(), Some(0));
    assert_eq!(columns.iter().copied().last(), Some(26));
    assert_eq!(
        columns.len(),
        2,
        "column 0 is referenced by two templates but is one column"
    );
    assert!(columns.iter().copied().max().unwrap() <= NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX);

    let too_wide: Vec<String> = vec!["uX:%x[0,27],".to_owned()];
    let columns = referenced_columns(&too_wide);
    assert_eq!(columns.iter().copied().max(), Some(27));
    assert!(columns.iter().copied().max().unwrap() > NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX);

    // A malformed or absent template contributes no column rather than a panic.
    let degenerate: Vec<String> = vec!["uY:%x[0],".to_owned(), "uZ:%x[0,abc],".to_owned()];
    assert!(referenced_columns(&degenerate).is_empty());

    // The header reader, on a hand-written Wapiti header of the shipped shape.
    let header = b"#mdl#2#19376\n#rdr#4/26/0\n\
                   12:u00:%x[-4,0],\n11:u04:%x[0,0],\n12:uN5:%x[2,26],\n1:b,\n\
                   #qrk#24\n11:I-<section>,\n10:<section>,\n\
                   #qrk#2066170\n";
    let declared = parse_declared_header(header);
    assert_eq!(declared.model_type, Some(2));
    assert_eq!(declared.feature_count, Some(19376));
    assert_eq!(declared.declared_patterns, Some(4));
    assert_eq!(declared.patterns.len(), 4);
    assert_eq!(declared.declared_labels, Some(24));
    assert_eq!(declared.labels, vec!["I-<section>", "<section>"]);
    assert_eq!(declared.declared_observations, Some(2_066_170));
    assert!(declared.complete);
    assert_eq!(referenced_columns(&declared.patterns).len(), 2);

    // A header cut off before its second `#qrk#` is incomplete rather than
    // silently reported with a missing observation count.
    let truncated = b"#mdl#2#1\n#rdr#1/0/0\n1:b,\n#qrk#1\n9:<other>,\n";
    let declared = parse_declared_header(truncated);
    assert!(!declared.complete);
    assert_eq!(declared.declared_observations, None);
    assert_eq!(declared.labels, vec!["<other>"]);
}

#[test]
fn optional_real_fulltext_model_is_explicitly_missing_or_structurally_validated() {
    let Some(root) = std::env::var_os(MODEL_ROOT_ENV).filter(|value| !value.is_empty()) else {
        eprintln!(
            "MISSING: {MODEL_ROOT_ENV} is not configured; no real GROBID fulltext model claim was made"
        );
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.is_dir(),
        "{MODEL_ROOT_ENV} must name a directory of GROBID family directories"
    );
    assert_relative_model_path();

    let resolved = root.join(FULLTEXT_RELATIVE_PATH);
    assert!(
        resolved.exists(),
        "no {} below {MODEL_ROOT_ENV}",
        FULLTEXT_RELATIVE_PATH
    );
    let model_path = resolved
        .canonicalize()
        .unwrap_or_else(|error| panic!("{FULLTEXT_RELATIVE_PATH} must resolve: {error}"));

    // D358: the resource is read from the caller's root and never lives in the
    // tree. A root pointing back into the workspace is refused rather than
    // silently treated as a tracked fixture.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root resolves");
    assert!(
        !model_path.starts_with(&repo),
        "D358: the fulltext model resource must not resolve inside the workspace ({} resolves to {})",
        MODEL_ROOT_ENV,
        model_path.display()
    );

    // The declared model budget is enforced before the bytes are read.
    let metadata = fs::metadata(&model_path).expect("the model resource is readable");
    assert!(
        metadata.len() <= DECLARED_MODEL_BYTES_LIMIT as u64,
        "the fulltext model resource is {} bytes, over the declared {} byte budget",
        metadata.len(),
        DECLARED_MODEL_BYTES_LIMIT
    );

    // (2) Lexical read of the declared header, before the production reader.
    let header_bytes = read_header_prefix(&model_path, HEADER_PROBE_BYTES);
    let declared = parse_declared_header(&header_bytes);
    assert!(
        declared.complete,
        "the fulltext model header did not declare its observation table inside the {HEADER_PROBE_BYTES} byte probe"
    );
    let columns = referenced_columns(&declared.patterns);
    let lowest = columns.iter().copied().next();
    let highest = columns.iter().copied().last();
    let (missing, unexpected) = base_set_divergence(&declared.labels);
    let over_budget = declared
        .declared_observations
        .filter(|count| *count > MAX_CRF_OBSERVATIONS);
    let spelling_histogram: BTreeMap<&str, usize> =
        declared
            .labels
            .iter()
            .fold(BTreeMap::new(), |mut acc, label| {
                *acc.entry(base_of(label)).or_insert(0) += 1;
                acc
            });

    eprintln!(
        "declared header: type={:?} features={:?} patterns={:?} ({} parsed) labels={:?} ({} parsed) observations={:?}",
        declared.model_type,
        declared.feature_count,
        declared.declared_patterns,
        declared.patterns.len(),
        declared.declared_labels,
        declared.labels.len(),
        declared.declared_observations
    );
    eprintln!(
        "declared observation span: columns {lowest:?}..={highest:?} ({} distinct)",
        columns.len()
    );
    eprintln!("declared label spellings per base: {spelling_histogram:?}");
    eprintln!(
        "native dictionary divergence: missing={missing:?} unexpected={unexpected:?}; \
         production reader observation budget={MAX_CRF_OBSERVATIONS}, over budget={over_budget:?}"
    );

    // The header parse must be self-consistent before its content is trusted.
    if let Some(count) = declared.declared_patterns {
        assert_eq!(
            declared.patterns.len(),
            count,
            "the declared pattern count must equal the parsed pattern lines"
        );
    }
    if let Some(count) = declared.declared_labels {
        assert_eq!(
            declared.labels.len(),
            count,
            "the declared label count must equal the parsed label lines"
        );
    }

    // (3) Fail closed. Every divergence is named at once, so one run of the
    // harness is enough to act on.
    let (Some(lowest), Some(highest)) = (lowest, highest) else {
        panic!("the real fulltext model declares no %x[<offset>,<column>] template");
    };
    assert!(
        highest <= NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX,
        "the real fulltext model references column {highest} but the printed ABI is only {} columns wide \
         (max index {}); the row builder must not be widened to satisfy a model",
        NATIVE_FULLTEXT_FEATURE_COLUMNS,
        NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX
    );
    check_fulltext_feature_columns(highest + 1).unwrap_or_else(|error| {
        panic!("the printed ABI must cover the model's declared observation width: {error}")
    });

    if !missing.is_empty() || !unexpected.is_empty() || over_budget.is_some() {
        panic!(
            "the real fulltext model diverges from the native band and must not be absorbed:\n\
             - label dictionary: missing {missing:?}, unexpected {unexpected:?}\n\
               (the native dictionary is transcribed from data/parse/native_body_accounting.yaml)\n\
             - observation budget: declared {:?}, production reader admits {MAX_CRF_OBSERVATIONS}\n\
             The band stays shadow-only and candidate-only; reconcile the artifact with the ledger \
             before any adoption claim.",
            declared.declared_observations
        );
    }

    // (4) A conforming artifact is then loaded and driven through the real
    // observation table.
    let loader = FilesystemCrfModelLoader::new(&model_path);
    let model = validate_loaded_crf_model(&loader).unwrap_or_else(|error| {
        panic!("configured real fulltext model failed validation: {error}")
    });
    assert_eq!(model.model_type, 2, "resource must be a scalar CRF model");
    assert_eq!(model.labels.len(), DECLARED_LABEL_COUNT);

    let tokens = probe_tokens();
    let order = glue_fulltext_reading_order(&tokens)
        .unwrap_or_else(|error| panic!("the probe body must glue: {error}"));
    assert_eq!(order.rows.len(), tokens.len());
    assert!(
        order
            .rows
            .iter()
            .all(|row| row.width() == NATIVE_FULLTEXT_FEATURE_COLUMNS),
        "the probe rows must carry the printed {NATIVE_FULLTEXT_FEATURE_COLUMNS}-column ABI"
    );
    let input = rows_text(&order.rows);
    let positions = map_input(&model, &input).unwrap_or_else(|error| {
        panic!(
            "the printed {NATIVE_FULLTEXT_FEATURE_COLUMNS}-column ABI was refused by the real fulltext model: {error}"
        )
    });
    assert_eq!(positions.len(), tokens.len(), "one mapped position per row");

    match decode_fulltext_sequence(&order.rows, &model) {
        NativeFulltextDecodeOutcome::Accepted { decode } => {
            assert_eq!(decode.labels.len(), tokens.len());
            let unknown: Vec<&str> = decode
                .labels
                .iter()
                .map(String::as_str)
                .filter(|label| !NATIVE_FULLTEXT_LABELS.contains(label))
                .collect();
            assert!(
                unknown.is_empty(),
                "the real fulltext model emitted labels outside the closed vocabulary: {unknown:?}"
            );
            eprintln!(
                "decoded probe labels: {:?} ({} runs)",
                decode.labels,
                decode.runs.len()
            );
        }
        NativeFulltextDecodeOutcome::Rejected { error } => {
            panic!("the native runner rejected the real fulltext model sequence: {error}");
        }
    }

    eprintln!(
        "PASS: real fulltext model validated (labels={}, bases={}, columns={lowest}..={highest}, tokens={})",
        model.labels.len(),
        spelling_histogram.len(),
        tokens.len()
    );
}
