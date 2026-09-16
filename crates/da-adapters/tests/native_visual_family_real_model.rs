//! Opt-in harness for the real GROBID 0.9.0 `figure`, `table` and
//! `funding-acknowledgement` CRF models (S06/T07).
//!
//! Model bytes are never committed and never discovered implicitly (D358 —
//! GROBID model assets are not redistributable): the harness reads a
//! caller-owned local resource **root** from the environment and resolves three
//! relative family paths below it. A recorded real-model sequence is *not* an
//! independent oracle, so this harness asserts structural invariants only — the
//! declared label block against the Rust dictionaries and the DATA transcription,
//! the declared `#rdr#` referenced-column window against the printed ABI, the
//! resource budget, and that the artifact passes `validate_loaded_crf_model` —
//! and never a golden label string. The family *semantics* are proven separately
//! against an independent expectation
//! (`crates/da-parse/tests/native_visual_family_same_n.rs`).
//!
//! # Environment
//!
//! ```text
//! DA_GROBID_MODEL_ROOT=<directory that contains the family directories>
//! ```
//!
//! Relative to that root the harness expects GROBID's own layout:
//!
//! | family | relative path | labels | referenced window | printed ABI |
//! |---|---|---|---|---|
//! | `figure` | `figure/model.wapiti` | 7 | `0..=25` | 27 |
//! | `table` | `table/model.wapiti` | 10 | `0..=25` | 27 |
//! | `funding` | `funding-acknowledgement/model.wapiti` | 20 | `0..=16` | 18 |
//!
//! Unset environment is reported as `MISSING`, which is not a pass: it makes no
//! claim about the real models at all. No absolute path is baked into this file
//! and no model byte is copied into the tree.
//!
//! # Two lanes, two polarities
//!
//! 1. **structure** — a divergence between the artifact and the declared
//!    dictionary is a hard failure: the band may not absorb a dictionary or a
//!    window change silently, and the fix is a corrected transcription, never a
//!    relaxed check;
//! 2. **budget** — an artifact whose observation table exceeds the production
//!    reader's `MAX_CRF_OBSERVATIONS` is reported as a typed refusal instead of a
//!    panic or a truncated read (the shipped fulltext model is in exactly that
//!    state, see `native_fulltext_real_model.rs`), followed by *no* decode claim.
//!
//! # The gazetteer lane
//!
//! The funding family's two `printVector` columns are fed from lexicons D358
//! forbids bundling. When the caller's tree carries
//! `lexicon/organisations/funders.txt` and
//! `lexicon/organisations/research_infrastructures.txt`, the harness reads one
//! entry **at runtime, without copying it**, builds the caller-supplied coverage
//! the funding ABI consumes, and asserts the covered tokens print `1` while an
//! uncovered token prints `0`. When the lexicons are absent the lane is
//! `MISSING` — never a pass, because "no funder was recognised" would otherwise
//! be indistinguishable from "no funder exists".

use da_adapters::FilesystemCrfModelLoader;
use da_domain::scalar_crf::MAX_CRF_MODEL_BYTES;
use da_parse::{
    FulltextFeatureRow, FundingFeatureRow, FundingFeatureToken, FundingLineStatus,
    NATIVE_FIGURE_LABELS, NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, NATIVE_FULLTEXT_FEATURE_COLUMNS,
    NATIVE_FUNDING_FEATURE_COLUMNS, NATIVE_FUNDING_LABELS, NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
    NATIVE_FUNDING_ROW_COLUMNS, NATIVE_TABLE_LABELS, NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
    NativeFundingGazetteerCoverage, NativeFundingGazetteerLane, NativeFundingGazetteerSpan,
    NativeTableAbsence, NativeTableCells, check_figure_label_block, check_funding_label_block,
    check_table_label_block, decode_figure_sequence, decode_funding_sequence,
    decode_table_sequence, render_funding_features,
};
use da_ports::{MAX_CRF_OBSERVATIONS, validate_loaded_crf_model};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Caller-owned resource root; the same opt-in convention as the scalar CRF,
/// header, bibliography and fulltext harnesses.
const MODEL_ROOT_ENV: &str = "DA_GROBID_MODEL_ROOT";

/// The DATA file the declared facts are bound to.
const DATA: &str = "data/parse/native_visual_families.yaml";

/// The model resource budget, restated independently of the loader constant.
const DECLARED_MODEL_BYTES_LIMIT: usize = 128 * 1024 * 1024;

/// How much of the file the lexical header read may touch.
const HEADER_PROBE_BYTES: usize = 256 * 1024;

/// One real family's declared structure.
struct RealFamily {
    /// Family name, matching the DATA key and the Rust dictionary name.
    name: &'static str,
    /// Path below [`MODEL_ROOT_ENV`], in GROBID's own resource layout.
    relative_path: &'static str,
    /// The Rust dictionary the DATA file transcribes.
    dictionary: &'static [&'static str],
    /// The family's row width as printed by the native ABI.
    printed_row_width: usize,
    /// The DATA sub-key holding the family declaration.
    data_family: &'static str,
}

const FAMILIES: [RealFamily; 3] = [
    RealFamily {
        name: "figure",
        relative_path: "figure/model.wapiti",
        dictionary: &NATIVE_FIGURE_LABELS,
        printed_row_width: NATIVE_FULLTEXT_FEATURE_COLUMNS,
        data_family: "figure",
    },
    RealFamily {
        name: "table",
        relative_path: "table/model.wapiti",
        dictionary: &NATIVE_TABLE_LABELS,
        printed_row_width: NATIVE_FULLTEXT_FEATURE_COLUMNS,
        data_family: "table",
    },
    RealFamily {
        name: "funding",
        relative_path: "funding-acknowledgement/model.wapiti",
        dictionary: &NATIVE_FUNDING_LABELS,
        printed_row_width: NATIVE_FUNDING_ROW_COLUMNS,
        data_family: "funding",
    },
];

/// The structural facts a Wapiti model file declares in its header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DeclaredHeader {
    model_type: Option<u32>,
    /// `#mdl#`'s third field.
    feature_count: Option<usize>,
    /// The whole `#mdl#` line, which is what `declared_checkpoint` transcribes.
    checkpoint_line: Option<String>,
    /// The whole `#rdr#` line.
    reader_line: Option<String>,
    declared_patterns: Option<usize>,
    patterns: Vec<String>,
    declared_labels: Option<usize>,
    labels: Vec<String>,
    declared_observations: Option<usize>,
    complete: bool,
}

/// Why a family could not be driven all the way to a decode.
///
/// A refusal is a *typed* outcome: it names the limit, carries the observed
/// value, and suppresses any decode claim rather than degrading into a truncated
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FamilyVerdict {
    Decoded {
        labels: Vec<String>,
        observations: usize,
        nftr: usize,
    },
    RefusedObservationBudget {
        declared: usize,
        maximum: usize,
    },
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
            header.checkpoint_line = Some(line.to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("#rdr#") {
            header.declared_patterns = rest.split('/').next().and_then(|value| value.parse().ok());
            header.reader_line = Some(line.to_owned());
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
        .unwrap_or_else(|error| panic!("the model resource must open: {error}"));
    let mut buffer = Vec::new();
    file.take(limit as u64)
        .read_to_end(&mut buffer)
        .unwrap_or_else(|error| panic!("the model header must read: {error}"));
    buffer
}

/// Every column index the model's `%x[<offset>,<column>]` templates reference.
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

/// `#rdr#<patterns>/<max column index>/<tokens>` — the second field.
fn declared_window(reader_line: &str) -> Option<usize> {
    reader_line
        .strip_prefix("#rdr#")?
        .split('/')
        .nth(1)?
        .trim()
        .parse()
        .ok()
}

/// The digest `observed_labels_sha256` records: `sha256:` over each label
/// spelling followed by a NUL byte, in the artifact's own order.
fn label_block_digest(labels: &[String]) -> String {
    let mut hasher = Sha256::new();
    for label in labels {
        hasher.update(label.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// The path below the caller's root must be relative, parent-free and named.
fn assert_relative_model_path(relative: &str) {
    let path = Path::new(relative);
    assert!(
        !path.is_absolute(),
        "the harness must resolve a relative path below {MODEL_ROOT_ENV}: {relative}"
    );
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "the relative model path must not contain a root or parent component: {relative}"
    );
    assert!(path.ends_with("model.wapiti"), "{relative}");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root resolves")
}

fn data_document() -> serde_yaml::Value {
    let path = repo_root().join(DATA);
    serde_yaml::from_str(&fs::read_to_string(&path).expect(DATA)).expect(DATA)
}

fn data_family(family: &str) -> serde_yaml::Value {
    data_document()["families"][family].clone()
}

fn data_text(family: &str, key: &str) -> String {
    data_family(family)[key]
        .as_str()
        .unwrap_or_else(|| panic!("{family}.{key} must be a string"))
        .to_string()
}

fn data_number(family: &str, key: &str) -> usize {
    data_family(family)[key]
        .as_u64()
        .unwrap_or_else(|| panic!("{family}.{key} must be a number")) as usize
}

fn data_strings(family: &str, key: &str) -> Vec<String> {
    data_family(family)[key]
        .as_sequence()
        .unwrap_or_else(|| panic!("{family}.{key} must be a sequence"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{family}.{key} carries a non-string"))
                .to_string()
        })
        .collect()
}

/// One 27-column fulltext row whose first column carries the token text.
fn fulltext_rows(tokens: &[&str]) -> Vec<FulltextFeatureRow> {
    tokens
        .iter()
        .map(|token| FulltextFeatureRow {
            columns: std::iter::once((*token).to_string())
                .chain((1..NATIVE_FULLTEXT_FEATURE_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        })
        .collect()
}

/// One 18-column funding row per token, through the shipped ABI.
fn funding_rows(
    tokens: &[&str],
    coverage: &NativeFundingGazetteerCoverage,
) -> Vec<FundingFeatureRow> {
    let last = tokens.len().saturating_sub(1);
    let facts: Vec<FundingFeatureToken> = tokens
        .iter()
        .enumerate()
        .map(|(index, text)| FundingFeatureToken {
            text: (*text).to_string(),
            line_status: if index == 0 {
                FundingLineStatus::Linestart
            } else if index == last {
                FundingLineStatus::Lineend
            } else {
                FundingLineStatus::Linein
            },
        })
        .collect();
    render_funding_features(&facts, coverage)
        .unwrap_or_else(|error| panic!("the funding ABI refused the probe tokens: {error}"))
}

/// Drive one real family's declared ABI through its native runner.
fn drive_family(
    family: &RealFamily,
    model: &da_domain::scalar_crf::CrfModel,
    tokens: &[&str],
    coverage: &NativeFundingGazetteerCoverage,
) -> FamilyVerdict {
    let labels = match family.name {
        "figure" => {
            let rows = fulltext_rows(tokens);
            let outcome = decode_figure_sequence(&rows, model, &[]).unwrap_or_else(|error| {
                panic!("the native figure runner refused the real model: {error:?}")
            });
            match outcome {
                da_parse::NativeFigureDecodeOutcome::Decoded { labels, .. } => labels,
                da_parse::NativeFigureDecodeOutcome::NotObserved { reason } => {
                    panic!(
                        "the native figure runner reported the absence {reason:?} for a non-empty decode"
                    )
                }
            }
        }
        "table" => {
            let rows = fulltext_rows(tokens);
            let outcome = decode_table_sequence(
                &rows,
                model,
                &[],
                NativeTableCells::NotObserved {
                    reason: NativeTableAbsence::NoCellInventory,
                },
            )
            .unwrap_or_else(|error| {
                panic!("the native table runner refused the real model: {error:?}")
            });
            match outcome {
                da_parse::NativeTableDecodeOutcome::Decoded { labels, .. } => labels,
                da_parse::NativeTableDecodeOutcome::NotObserved { reason } => {
                    panic!(
                        "the native table runner reported the absence {reason:?} for a non-empty decode"
                    )
                }
            }
        }
        "funding" => {
            let rows = funding_rows(tokens, coverage);
            let outcome = decode_funding_sequence(&rows, model).unwrap_or_else(|error| {
                panic!("the native funding runner refused the real model: {error:?}")
            });
            match outcome {
                da_parse::NativeFundingDecodeOutcome::Decoded { labels, .. } => labels,
                da_parse::NativeFundingDecodeOutcome::NotObserved { reason } => {
                    panic!(
                        "the native funding runner reported the absence {reason:?} for a non-empty decode"
                    )
                }
            }
        }
        other => panic!("{other} is not a real family in this harness"),
    };
    assert_eq!(
        labels.len(),
        tokens.len(),
        "{}: one decoded label per probe row",
        family.name
    );
    let unknown: Vec<&str> = labels
        .iter()
        .map(String::as_str)
        .filter(|label| !family.dictionary.contains(label))
        .collect();
    assert!(
        unknown.is_empty(),
        "{}: the real model emitted labels outside the declared dictionary: {unknown:?}",
        family.name
    );
    FamilyVerdict::Decoded {
        labels,
        observations: model.observations.len(),
        nftr: model.nftr,
    }
}

/// A minimal probe body: enough rows for every family to produce a decode.
fn probe_tokens(family: &str) -> Vec<&'static str> {
    match family {
        "funding" => vec!["This", "work", "was", "funded", "by", "grant", "101"],
        _ => vec!["Figure", "1", ":", "overview", "of", "the", "band"],
    }
}

/// One lexicon entry read at runtime, split into ABI-legal tokens.
fn lexicon_entry_words(path: &Path, limit: usize) -> Option<Vec<String>> {
    let text = fs::read_to_string(path).ok()?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))?;
    let words: Vec<String> = line
        .split_whitespace()
        .take(limit)
        .map(str::to_string)
        .collect();
    (!words.is_empty()).then_some(words)
}

/// The organisation lexicon directory, if the caller's tree carries one.
///
/// Accepted beside the model root (`<root>/lexicon/organisations`) and one level
/// up (`<root>/../lexicon/organisations`), which is GROBID's own layout when the
/// root is `grobid-home/models`.
fn organisations_dir(root: &Path) -> Option<PathBuf> {
    let beside = root.join("lexicon/organisations");
    if beside.is_dir() {
        return Some(beside);
    }
    let above = root.parent()?.join("lexicon/organisations");
    above.is_dir().then_some(above)
}

#[test]
fn optional_real_visual_family_models_are_explicitly_missing_or_structurally_validated() {
    let Some(root) = std::env::var_os(MODEL_ROOT_ENV).filter(|value| !value.is_empty()) else {
        eprintln!(
            "MISSING: {MODEL_ROOT_ENV} is not configured; no real GROBID visual/funding model claim was made"
        );
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.is_dir(),
        "{MODEL_ROOT_ENV} must name a directory of GROBID family directories"
    );
    let repo = repo_root();

    // Three families, three distinct dictionaries. Two of them deliberately
    // share the fulltext ABI (the secondary `figure`/`table` models consume the
    // same 27-column row), while funding's is its own 18-column row — so the
    // widths are *two* values and the label counts are three.
    let mut counts: Vec<usize> = FAMILIES.iter().map(|spec| spec.dictionary.len()).collect();
    counts.sort_unstable();
    counts.dedup();
    assert_eq!(
        counts.len(),
        FAMILIES.len(),
        "the family dictionaries must differ"
    );
    let mut widths: Vec<usize> = FAMILIES.iter().map(|spec| spec.printed_row_width).collect();
    widths.sort_unstable();
    widths.dedup();
    assert_eq!(
        widths,
        vec![NATIVE_FUNDING_ROW_COLUMNS, NATIVE_FULLTEXT_FEATURE_COLUMNS],
        "figure and table share the fulltext ABI; funding does not"
    );
    let mut windows: Vec<usize> = FAMILIES
        .iter()
        .map(|spec| match spec.name {
            "funding" => NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
            _ => NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
        })
        .collect();
    windows.sort_unstable();
    windows.dedup();
    assert_eq!(
        windows.len(),
        2,
        "two referenced windows: fulltext and funding"
    );

    let mut verdicts: BTreeMap<&str, FamilyVerdict> = BTreeMap::new();
    let mut spelling_histogram: BTreeMap<String, usize> = BTreeMap::new();

    for spec in FAMILIES {
        assert_relative_model_path(spec.relative_path);

        // The declared structure, before touching any bytes: the Rust dictionary
        // and the DATA transcription must agree with each other as well.
        assert_eq!(
            data_number(spec.data_family, "declared_label_count"),
            spec.dictionary.len(),
            "{}: the DATA label count and the Rust dictionary disagree",
            spec.name
        );
        assert_eq!(
            data_strings(spec.data_family, "declared_labels"),
            spec.dictionary
                .iter()
                .map(|label| (*label).to_string())
                .collect::<Vec<_>>(),
            "{}: the DATA label block and the Rust dictionary disagree",
            spec.name
        );
        assert_eq!(
            data_text(spec.data_family, "model_file"),
            spec.relative_path,
            "{}: the DATA model path and the harness path disagree",
            spec.name
        );

        let resolved = root.join(spec.relative_path);
        assert!(
            resolved.exists(),
            "no {} below {MODEL_ROOT_ENV}",
            spec.relative_path
        );
        let model_path = resolved
            .canonicalize()
            .unwrap_or_else(|error| panic!("{} must resolve: {error}", spec.relative_path));

        // D358: the resource is read from the caller's root and never lives in
        // the tree. A root pointing back into the workspace is refused rather
        // than silently treated as a tracked fixture.
        assert!(
            !model_path.starts_with(&repo),
            "D358: the {} model resource must not resolve inside the workspace ({} resolves to {})",
            spec.name,
            MODEL_ROOT_ENV,
            model_path.display()
        );

        // The declared model budget is enforced before the bytes are read.
        let metadata = fs::metadata(&model_path).expect("the model resource is readable");
        assert!(
            metadata.len() <= DECLARED_MODEL_BYTES_LIMIT as u64,
            "the {} model resource is {} bytes, over the declared {} byte budget",
            spec.name,
            metadata.len(),
            DECLARED_MODEL_BYTES_LIMIT
        );

        // (2) Lexical read of the declared header, before the production reader.
        let declared = parse_declared_header(&read_header_prefix(&model_path, HEADER_PROBE_BYTES));
        assert!(
            declared.complete,
            "{}: the header did not declare its observation table inside the {HEADER_PROBE_BYTES} byte probe",
            spec.name
        );
        assert_eq!(declared.model_type, Some(2), "{}: scalar CRF", spec.name);
        if let Some(count) = declared.declared_patterns {
            assert_eq!(
                declared.patterns.len(),
                count,
                "{}: the declared pattern count must equal the parsed pattern lines",
                spec.name
            );
        }
        if let Some(count) = declared.declared_labels {
            assert_eq!(
                declared.labels.len(),
                count,
                "{}: the declared label count must equal the parsed label lines",
                spec.name
            );
        }
        for label in &declared.labels {
            *spelling_histogram
                .entry(base_of(label).to_string())
                .or_insert(0) += 1;
        }

        // (3) Fail closed on any divergence — the artifact may not move a
        // declared fact, and the fix is a corrected transcription.
        assert_eq!(
            declared.labels, spec.dictionary,
            "{}: the frozen model's label block (in model order) is not the declared dictionary",
            spec.name
        );
        assert_eq!(
            declared.checkpoint_line.as_deref(),
            Some(data_text(spec.data_family, "declared_checkpoint").as_str()),
            "{}: the frozen model's checkpoint line is not the declared fact",
            spec.name
        );
        assert_eq!(
            declared.reader_line.as_deref(),
            Some(data_text(spec.data_family, "declared_rdr").as_str()),
            "{}: the frozen model's `#rdr#` line is not the declared fact",
            spec.name
        );

        let window = declared_window(
            declared
                .reader_line
                .as_deref()
                .expect("the reader header was read"),
        )
        .expect("the reader header declares a max column index");
        assert_eq!(
            window + 1,
            data_number(spec.data_family, "referenced_columns"),
            "{}: the declared `#rdr#` window disagrees with referenced_columns",
            spec.name
        );
        assert_eq!(
            referenced_columns(&declared.patterns),
            (0..=window).collect::<BTreeSet<usize>>(),
            "{}: the artifact's templates must reference exactly 0..={window}",
            spec.name
        );

        // The printed ABI must cover the artifact's window *and* keep the
        // declared excluded column out of it.
        assert!(
            window < spec.printed_row_width,
            "{}: the declared window reaches column {window} but the printed ABI is only {} columns wide",
            spec.name,
            spec.printed_row_width
        );
        let excluded: Vec<u64> = data_family(spec.data_family)["template_excludes"]
            .as_sequence()
            .map(|sequence| {
                sequence
                    .iter()
                    .filter_map(serde_yaml::Value::as_u64)
                    .collect()
            })
            .unwrap_or_default();
        for column in &excluded {
            assert!(
                *column as usize > window,
                "{}: an excluded column may not be inside the referenced window",
                spec.name
            );
        }

        // (4) The declared observation budget, checked *before* the production
        // reader so an over-budget artifact is a typed refusal and not a panic
        // or a truncated read.
        let declared_observations = declared
            .declared_observations
            .expect("the header declares an observation count");
        let verdict = if declared_observations > MAX_CRF_OBSERVATIONS {
            FamilyVerdict::RefusedObservationBudget {
                declared: declared_observations,
                maximum: MAX_CRF_OBSERVATIONS,
            }
        } else {
            let loader = FilesystemCrfModelLoader::new(&model_path);
            let model = validate_loaded_crf_model(&loader).unwrap_or_else(|error| {
                panic!(
                    "the configured real {} model failed validation: {error}",
                    spec.name
                )
            });
            assert_eq!(
                model.labels, spec.dictionary,
                "{}: the loaded model's label block is not the declared dictionary",
                spec.name
            );
            assert_eq!(
                model.observations.len(),
                declared_observations,
                "{}: the loaded observation table must equal the declared count",
                spec.name
            );
            let check = match spec.name {
                "figure" => check_figure_label_block(&model).map_err(|error| error.to_string()),
                "table" => check_table_label_block(&model).map_err(|error| error.to_string()),
                _ => check_funding_label_block(&model).map_err(|error| error.to_string()),
            };
            check.unwrap_or_else(|error| {
                panic!(
                    "the native {} family refused the real model's label block: {error}",
                    spec.name
                )
            });
            drive_family(&spec, &model, &probe_tokens(spec.name), &empty_coverage())
        };
        match &verdict {
            FamilyVerdict::Decoded { labels, .. } => eprintln!(
                "PASS: real {} model validated (labels={}, observations={}, window=0..={window}, decoded={:?})",
                spec.name,
                spec.dictionary.len(),
                declared_observations,
                labels
            ),
            FamilyVerdict::RefusedObservationBudget { declared, maximum } => eprintln!(
                "REFUSED: real {} model declares {declared} observations, over the production reader's {maximum}; no decode claim was made",
                spec.name
            ),
        }
        verdicts.insert(spec.name, verdict);
    }

    // The family set is three distinct dictionaries over three distinct windows.
    assert_eq!(
        verdicts.len(),
        FAMILIES.len(),
        "every family must produce a verdict"
    );
    assert_eq!(
        NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX,
        "figure and table consume the same fulltext window"
    );
    assert_ne!(
        NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
        "the funding window is its own ABI, not the fulltext one"
    );

    // Budget polarity, on the artifact that is actually configured: a refusal
    // may only be reported for an artifact that really is over budget, and a
    // decode may only be claimed for one that is not.
    for spec in FAMILIES {
        match &verdicts[spec.name] {
            FamilyVerdict::RefusedObservationBudget { declared, maximum } => {
                assert!(declared > maximum, "a refusal must name a real excess");
            }
            FamilyVerdict::Decoded { observations, .. } => {
                assert!(
                    *observations <= MAX_CRF_OBSERVATIONS,
                    "a decode was claimed for an over-budget artifact"
                );
            }
        }
    }

    eprintln!(
        "PASS: three real visual/funding families structurally validated under {MODEL_ROOT_ENV} \
         (label spellings per base: {spelling_histogram:?})"
    );

    // The observed facts the DATA file records must be what this run observed:
    // the transcription is not allowed to be self-certifying.
    for spec in FAMILIES {
        let header = parse_declared_header(&read_header_prefix(
            &root
                .join(spec.relative_path)
                .canonicalize()
                .expect("the resource resolves"),
            HEADER_PROBE_BYTES,
        ));
        assert_eq!(
            data_text(spec.data_family, "observed_checkpoint"),
            header.checkpoint_line.expect("the checkpoint line"),
            "{}: the recorded observed checkpoint is not what was observed",
            spec.name
        );
        assert_eq!(
            data_text(spec.data_family, "observed_labels_sha256"),
            label_block_digest(&header.labels),
            "{}: the recorded observed label digest is not what was observed",
            spec.name
        );
        assert_eq!(
            data_text(spec.data_family, "observed_rdr"),
            header.reader_line.expect("the reader line"),
            "{}: the recorded observed `#rdr#` line is not what was observed",
            spec.name
        );
    }

    // The gazetteer lane: real lexicon entries, read at runtime and never copied.
    if let Some(directory) = organisations_dir(&root) {
        for (file, lane, column) in [
            (
                "funders.txt",
                NativeFundingGazetteerLane::KnownFunder,
                14usize,
            ),
            (
                "research_infrastructures.txt",
                NativeFundingGazetteerLane::KnownInfrastructure,
                15usize,
            ),
        ] {
            let path = directory.join(file);
            let Some(words) = lexicon_entry_words(&path, 8) else {
                eprintln!(
                    "MISSING: {} carries no usable entry; no funding flag claim was made",
                    path.display()
                );
                continue;
            };
            let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
            let span = NativeFundingGazetteerSpan {
                start: 0,
                end: words.len() - 1,
            };
            let coverage = match lane {
                NativeFundingGazetteerLane::KnownFunder => NativeFundingGazetteerCoverage {
                    funders: vec![span],
                    infrastructures: Vec::new(),
                },
                NativeFundingGazetteerLane::KnownInfrastructure => NativeFundingGazetteerCoverage {
                    funders: Vec::new(),
                    infrastructures: vec![span],
                },
            };
            let mut tokens = borrowed.clone();
            tokens.push("UNLISTED");
            let rows = funding_rows(&tokens, &coverage);
            assert_eq!(rows.len(), tokens.len());
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(
                    row.width(),
                    NATIVE_FUNDING_ROW_COLUMNS,
                    "the funding ABI width"
                );
                let flag = row.columns[column].as_str();
                if index < words.len() {
                    assert_eq!(
                        flag,
                        "1",
                        "{}: the covered token `{}` must print the {} flag",
                        file,
                        tokens[index],
                        lane.as_str()
                    );
                } else {
                    assert_eq!(
                        flag,
                        "0",
                        "{}: an uncovered token must print the {} flag as 0",
                        file,
                        lane.as_str()
                    );
                }
            }
            let label_slot = row_label_slot(&rows[0]);
            assert_eq!(
                label_slot, "0",
                "the inference label slot is the literal zero"
            );
            assert_eq!(
                NATIVE_FUNDING_FEATURE_COLUMNS + 1,
                NATIVE_FUNDING_ROW_COLUMNS,
                "the row is the printed features plus the label slot"
            );
            eprintln!(
                "PASS: {file} entry `{}` printed the {} flag through the funding ABI ({} tokens)",
                words.join(" "),
                lane.as_str(),
                words.len()
            );
        }
    } else {
        eprintln!(
            "MISSING: no lexicon/organisations below {MODEL_ROOT_ENV} or its parent; \
             no funding gazetteer flag claim was made"
        );
    }
}

fn row_label_slot(row: &FundingFeatureRow) -> String {
    row.label_column().unwrap_or_default().to_string()
}

fn empty_coverage() -> NativeFundingGazetteerCoverage {
    NativeFundingGazetteerCoverage {
        funders: Vec::new(),
        infrastructures: Vec::new(),
    }
}

/// The base label of one spelling (`I-` is a spelling prefix, never a base).
fn base_of(label: &str) -> &str {
    label.strip_prefix("I-").unwrap_or(label)
}

/// The declared surface is pinned without any resource, so a silent move of a
/// constant is caught even when no model is configured.
#[test]
fn declared_visual_family_surface_stays_frozen() {
    assert_eq!(NATIVE_FIGURE_LABELS.len(), 7);
    assert_eq!(NATIVE_TABLE_LABELS.len(), 10);
    assert_eq!(NATIVE_FUNDING_LABELS.len(), 20);
    assert_eq!(NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, 25);
    assert_eq!(NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, 25);
    assert_eq!(NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, 16);
    assert_eq!(NATIVE_FULLTEXT_FEATURE_COLUMNS, 27);
    assert_eq!(NATIVE_FUNDING_ROW_COLUMNS, 18);
    assert_eq!(DECLARED_MODEL_BYTES_LIMIT, MAX_CRF_MODEL_BYTES);
    assert_eq!(DECLARED_MODEL_BYTES_LIMIT, 128 * 1024 * 1024);
    for spec in FAMILIES {
        assert_relative_model_path(spec.relative_path);
        assert_eq!(
            Path::new(spec.relative_path).components().count(),
            2,
            "the family path is one directory component plus the resource name"
        );
        assert_eq!(
            data_text(spec.data_family, "model_file"),
            spec.relative_path,
            "the DATA model path is the only path this harness drives"
        );
    }
    // The DATA window and the Rust window are the same fact.
    assert_eq!(
        data_number("figure", "referenced_columns") - 1,
        NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX
    );
    assert_eq!(
        data_number("table", "referenced_columns") - 1,
        NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX
    );
    assert_eq!(
        data_number("funding", "referenced_columns") - 1,
        NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX
    );
    // The recorded observation is checkable without any model resource: it is a
    // well-formed digest and a `#mdl#2#<n>` checkpoint line.
    for spec in FAMILIES {
        let digest = data_text(spec.data_family, "observed_labels_sha256");
        assert!(
            digest.starts_with("sha256:") && digest.len() == 71,
            "{}: the recorded label digest must be one `sha256:` value",
            spec.name
        );
        let checkpoint = data_text(spec.data_family, "observed_checkpoint");
        assert!(
            checkpoint.starts_with("#mdl#2#"),
            "{}: the recorded checkpoint must be the model header line, got {checkpoint}",
            spec.name
        );
        assert_eq!(
            data_text(spec.data_family, "declared_checkpoint"),
            checkpoint,
            "{}: the declared and observed checkpoints are the same fact",
            spec.name
        );
        assert_eq!(
            data_text(spec.data_family, "declared_rdr"),
            data_text(spec.data_family, "observed_rdr"),
            "{}: the declared and observed `#rdr#` lines are the same fact",
            spec.name
        );
    }
}

/// The header reader, the window reader, the digest and the budget verdict are
/// the invariants the opt-in path enforces, so their polarity is pinned on
/// synthetic bytes with no model resource involved.
#[test]
fn header_reading_digest_and_budget_polarity_are_detected_without_model_bytes() {
    let header = b"#mdl#2#521\n#rdr#4/25/0\n\
                   12:u00:%x[-4,0],\n11:u04:%x[0,0],\n9:uN5:%x[0,25],\n1:b,\n\
                   #qrk#7\n15:I-<figure_head>,\n9:I-<label>,\n\
                   #qrk#27108\n";
    let declared = parse_declared_header(header);
    assert_eq!(declared.model_type, Some(2));
    assert_eq!(declared.feature_count, Some(521));
    assert_eq!(declared.checkpoint_line.as_deref(), Some("#mdl#2#521"));
    assert_eq!(declared.reader_line.as_deref(), Some("#rdr#4/25/0"));
    assert_eq!(declared.declared_patterns, Some(4));
    assert_eq!(declared.patterns.len(), 4);
    assert_eq!(declared.declared_labels, Some(7));
    assert_eq!(declared.labels, vec!["I-<figure_head>", "I-<label>"]);
    assert_eq!(declared.declared_observations, Some(27_108));
    assert!(declared.complete);
    assert_eq!(declared_window("#rdr#85/25/0"), Some(25));
    assert_eq!(declared_window("#rdr#100/16/0"), Some(16));
    assert_eq!(declared_window("#rdr#garbage"), None);

    // The synthetic header references two of the window's columns; the *real*
    // artifacts reference every column up to the declared window, which is
    // asserted in the opt-in path. Here the point is that the scan reports the
    // declared columns and the window reader names the maximum in one place.
    let columns = referenced_columns(&declared.patterns);
    assert_eq!(
        columns,
        [0usize, 25].into_iter().collect::<BTreeSet<usize>>()
    );
    assert_eq!(
        columns.iter().next_back().copied(),
        declared_window("#rdr#4/25/0")
    );
    let narrow: Vec<String> = vec!["u:%x[0,0]".to_owned(), "u:%x[0,24]".to_owned()];
    assert_eq!(
        referenced_columns(&narrow).iter().next_back().copied(),
        Some(24),
        "a template set whose maximum is one column short is visible before any load"
    );
    let malformed: Vec<String> = vec!["u:%x[0]".to_owned(), "u:%x[0,abc]".to_owned()];
    assert!(referenced_columns(&malformed).is_empty());

    // A header cut off before its second `#qrk#` is incomplete rather than
    // silently reported with a missing observation count.
    let truncated = b"#mdl#2#1\n#rdr#1/0/0\n1:b,\n#qrk#1\n9:<other>,\n";
    let declared = parse_declared_header(truncated);
    assert!(!declared.complete);
    assert_eq!(declared.declared_observations, None);

    // The label digest is order-sensitive: a reordered block is a different
    // dictionary even when the set is the same.
    let ordered = vec!["I-<label>".to_string(), "<label>".to_string()];
    let reordered = vec!["<label>".to_string(), "I-<label>".to_string()];
    assert_ne!(label_block_digest(&ordered), label_block_digest(&reordered));
    assert_eq!(
        label_block_digest(&ordered),
        label_block_digest(&ordered.clone())
    );
    assert!(label_block_digest(&ordered).starts_with("sha256:"));

    // Budget polarity: the verdict is a refusal exactly when the declared count
    // exceeds the production reader's budget, and never a panic.
    let over = MAX_CRF_OBSERVATIONS + 1;
    let verdict = if over > MAX_CRF_OBSERVATIONS {
        FamilyVerdict::RefusedObservationBudget {
            declared: over,
            maximum: MAX_CRF_OBSERVATIONS,
        }
    } else {
        panic!("the polarity is inverted")
    };
    match verdict {
        FamilyVerdict::RefusedObservationBudget { declared, maximum } => {
            assert_eq!(declared, 2_000_001);
            assert_eq!(maximum, MAX_CRF_OBSERVATIONS);
        }
        other => panic!("{other:?}"),
    }
    // A tautological `assert!(MAX_CRF_OBSERVATIONS <= MAX_CRF_OBSERVATIONS)` stood
    // here. It could never fail, so it protected nothing. The intended boundary
    // check (a declaration of exactly the budget is accepted) belongs with the
    // polarity case above, once the production budget check is exercised
    // directly rather than re-implemented in this test.
}

/// The runtime lexicon reader is bounded and skips comments and blanks, so a
/// lexicon that carries only headers is `MISSING` rather than a false claim.
#[test]
fn lexicon_entry_reading_is_bounded_and_skips_non_entries() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("funders.txt");
    fs::write(
        &path,
        "# a comment\n\nNational Science Foundation\nOther Funder\n",
    )
    .expect("write");
    assert_eq!(
        lexicon_entry_words(&path, 8),
        Some(vec![
            "National".to_string(),
            "Science".to_string(),
            "Foundation".to_string()
        ])
    );
    assert_eq!(
        lexicon_entry_words(&path, 2),
        Some(vec!["National".to_string(), "Science".to_string()])
    );
    fs::write(&path, "# only a comment\n\n").expect("write");
    assert_eq!(lexicon_entry_words(&path, 8), None);
    assert_eq!(lexicon_entry_words(&dir.path().join("absent.txt"), 8), None);

    // A lexicon beside the root and one level up are both accepted; neither is
    // copied anywhere.
    let root = dir.path().join("models");
    fs::create_dir_all(root.join("lexicon/organisations")).expect("mkdir");
    assert_eq!(
        organisations_dir(&root),
        Some(root.join("lexicon/organisations"))
    );
    let nested = dir.path().join("grobid-home/models");
    fs::create_dir_all(&nested).expect("mkdir");
    fs::create_dir_all(dir.path().join("grobid-home/lexicon/organisations")).expect("mkdir");
    assert_eq!(
        organisations_dir(&nested),
        Some(dir.path().join("grobid-home/lexicon/organisations"))
    );
}
