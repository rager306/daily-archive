//! S06/T07 independent same-n oracle for the four native visual/funding families.
//!
//! # Why this file exists
//!
//! The wave's success criteria forbid a Java-derived self oracle: an expectation
//! that the SUT (or the Java code it was transcribed from) produced would only
//! prove the transcription is self-consistent. So the expectation side of this
//! file is a **second implementation** of the same written rules — GROBID's
//! `TaggingTokenClusteror` boundary, `FullTextParser.processEquations`,
//! `TableParser`'s head-after-body split and `Table.validateTable` revert, and
//! `FundingAcknowledgementParser`'s role table with its two predicates — written
//! from the rule text over the reviewed fixture facts, and it never calls the
//! runners it is compared against.
//!
//! The two lanes are separated by literal source markers and the separation is
//! asserted by `expected_builder_stays_free_of_the_sut_lane`, which scans the
//! region between them for the SUT symbols by name. Everything the oracle needs
//! from the SUT arrives below the end marker.
//!
//! # Inputs, and what each one is allowed to prove
//!
//! * `native_visual_family_oracle.yaml` — reviewed token facts plus reviewed
//!   decoded label sequences, per family and per document. It carries **no
//!   expectation**: the expectation is assembled here, and the fixture carries
//!   only the `expectation_sha256` that anchors this file's understanding so a
//!   silent change of it is visible. The file is digest-only: no raw text, PDF,
//!   TEI, image or asset material, no physical path (asserted).
//! * `native_figure_synthetic.wapiti`, `native_table_synthetic.wapiti`,
//!   `native_funding_synthetic.wapiti` — hand-authored Wapiti text models whose
//!   label block is the DATA dictionary and whose one unigram pattern keys on the
//!   printed token text, so a decode is deterministic and cannot be an artefact of
//!   tie-breaking. They are synthetic, not GROBID-derived: the real bytes are not
//!   redistributable (D358) and are probed only opt-in
//!   (`crates/da-adapters/tests/native_visual_family_real_model.rs`).
//! * `data/parse/native_visual_families.yaml` — the declared dictionaries, the
//!   declared `#rdr#` window and the shadow locks. The synthetic models must
//!   agree with it, so a transcription drift on either side fails here.
//!
//! # What this oracle does *not* claim
//!
//! A synthetic model is not the frozen model: this file proves the *family
//! semantics* (grouping, role assembly, split/revert, join, lift) against an
//! independent expectation, and it proves the declared dictionaries and widths
//! are mutually consistent. It proves nothing about the real model's weights —
//! that is the opt-in harness's structural claim, and it is explicitly `MISSING`
//! without a caller-supplied resource root.
//!
//! The real corpus rows are typed absence for these families
//! (`native_visual_oracle.yaml`), so the positive cases here are synthetic. The
//! absence cases are taken from the reviewed rows' polarity: an absence is a
//! typed absence and never a zero count (H12-N1).

use da_parse::{
    FigureSpan, FulltextFeatureRow, FundingFeatureRow, FundingFeatureToken, FundingLineStatus,
    NATIVE_FIGURE_BASE_LABELS, NATIVE_FIGURE_LABELS, NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
    NATIVE_FULLTEXT_FEATURE_COLUMNS, NATIVE_FUNDING_BASE_LABELS, NATIVE_FUNDING_LABEL_COLUMN_DUMMY,
    NATIVE_FUNDING_LABEL_COLUMN_INDEX, NATIVE_FUNDING_LABELS,
    NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_ROW_COLUMNS, NATIVE_TABLE_BASE_LABELS,
    NATIVE_TABLE_LABELS, NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, NativeCaptionLinkAbsence,
    NativeCaptionLinks, NativeCaptionMarker, NativeFigureAbsence, NativeFigureDecodeOutcome,
    NativeFormulaAbsence, NativeFormulaJoinOutcome, NativeFundingAbsence,
    NativeFundingDecodeOutcome, NativeFundingEntry, NativeFundingGazetteerCoverage,
    NativeTableAbsence, NativeTableCell, NativeTableCells, NativeTableDecodeOutcome,
    NativeTableSpan, decode_figure_sequence, decode_funding_sequence, decode_table_sequence,
    figure_spans_from_decoded, funding_entries_from_decoded, join_native_equation_labels,
    render_funding_features, table_spans_from_decoded,
};
use da_ports::scalar_crf::CrfModel;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_family_oracle.yaml";
const FIGURE_MODEL: &str = "crates/da-parse/tests/fixtures/native_figure_synthetic.wapiti";
const TABLE_MODEL: &str = "crates/da-parse/tests/fixtures/native_table_synthetic.wapiti";
const FUNDING_MODEL: &str = "crates/da-parse/tests/fixtures/native_funding_synthetic.wapiti";
const DATA: &str = "data/parse/native_visual_families.yaml";

const SCHEMA: &str = "native-visual-family-oracle.v1";
const FAMILIES: [&str; 4] = ["figure", "table", "formula", "funding"];

/// Payload field names the digest-only fixture may never carry, anywhere.
const FORBIDDEN_FIELDS: [&str; 10] = [
    "raw_text",
    "raw_pdf",
    "pdf_bytes",
    "tei_xml",
    "image",
    "page_image",
    "assets",
    "physical_path",
    "hostname",
    "executable",
];

// ---- independent expectation builder (no SUT lane below this marker) ----

/// The reviewed fixture, parsed once and never mutated.
fn oracle_document() -> Option<serde_yaml::Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(ORACLE);
    serde_yaml::from_str(&fs::read_to_string(path).expect(ORACLE)).ok()
}

/// One maximal cluster of a decoded label sequence, under GROBID's
/// `TaggingTokenClusteror.isBeginning` rule: a label that starts with `I-` opens
/// a cluster whatever preceded it, and inside a segment consecutive *identical*
/// spellings merge. `<x>,<x>` is one cluster; `I-<x>,<x>` is two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependentCluster {
    pub base: String,
    pub start: usize,
    pub end: usize,
    pub opened_by_prefix: bool,
}

/// The base name of a spelling (the `I-` prefix is a spelling, never a name).
fn independent_base(spelling: &str) -> &str {
    spelling.strip_prefix("I-").unwrap_or(spelling)
}

/// Cluster a decoded label sequence by the clusteror rule, independently.
pub fn independent_clusters(labels: &[String]) -> Vec<IndependentCluster> {
    let mut clusters = Vec::new();
    let mut start = 0usize;
    while start < labels.len() {
        let spelling = labels[start].as_str();
        let opened_by_prefix = spelling.starts_with("I-");
        let mut end = start + 1;
        while end < labels.len()
            && labels[end].as_str() == spelling
            && !labels[end].starts_with("I-")
        {
            end += 1;
        }
        clusters.push(IndependentCluster {
            base: independent_base(spelling).to_string(),
            start,
            end,
            opened_by_prefix,
        });
        start = end;
    }
    clusters
}

/// The printed text of one cluster: its tokens joined by a single space.
fn cluster_text(tokens: &[String], cluster: &IndependentCluster) -> String {
    tokens[cluster.start..cluster.end].join(" ")
}

/// Append one cluster's text to a role slot, joining parts with a single space
/// and never storing the empty string (an empty cluster observed nothing).
fn append_role(slot: &mut Option<String>, text: &str) {
    if text.is_empty() {
        return;
    }
    match slot {
        Some(existing) => {
            existing.push(' ');
            existing.push_str(text);
        }
        None => *slot = Some(text.to_string()),
    }
}

/// The caption-link inventory, as the fixture declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedLinks {
    NotObserved,
    Provided(Vec<(String, usize)>),
}

/// The cell inventory, as the fixture declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedCells {
    NotObserved,
    Provided(Vec<(u32, u32, String)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedFigure {
    pub label: Option<String>,
    pub head: Option<String>,
    pub caption: Option<String>,
    pub content: Option<String>,
    pub links: ExpectedLinks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedTable {
    pub label: Option<String>,
    pub head: Option<String>,
    pub caption: Option<String>,
    pub content: Option<String>,
    pub note: Option<String>,
    pub other: Option<String>,
    /// The first revert reason, in the fixed order the rules are applied.
    pub reverted: Option<String>,
    pub cells: ExpectedCells,
    pub links: ExpectedLinks,
    pub cluster_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedEquation {
    pub content_digest: String,
    pub label_digest: Option<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpectedFundingEntry {
    pub funder_name: Option<String>,
    pub grant_name: Option<String>,
    pub grant_number: Option<String>,
    pub program_name: Option<String>,
    pub project_name: Option<String>,
    pub persons: Vec<String>,
    pub affiliations: Vec<String>,
    pub institutions: Vec<String>,
    pub infrastructures: Vec<String>,
}

/// The assembled expectation of one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// A typed absence, never a zero count.
    Absent,
    Figure(ExpectedFigure),
    Tables(Vec<ExpectedTable>),
    Equations {
        equations: Vec<ExpectedEquation>,
        markers: usize,
    },
    Funding {
        entries: Vec<ExpectedFundingEntry>,
        other_count: usize,
    },
}

/// Identity of a sequence of decoded label spellings: `sha256:` over each
/// spelling followed by a NUL byte. Re-derived here from the rule text so the
/// comparison is between two implementations and not one.
fn spelling_digest(spellings: &[String]) -> String {
    let mut hasher = Sha256::new();
    for spelling in spellings {
        hasher.update(spelling.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

/// One slot of the canonical expectation form: presence is tagged, so `None` can
/// never be confused with `Some("")`.
fn canonical_slot(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("\u{1f}+{text}"),
        None => "\u{1f}-".to_string(),
    }
}

/// One list of the canonical form; an empty list is its own marker.
fn canonical_list(values: &[String]) -> String {
    if values.is_empty() {
        return "\u{1f}\u{1e}".to_string();
    }
    values
        .iter()
        .map(|value| format!("\u{1e}{value}"))
        .collect()
}

fn canonical_links(links: &ExpectedLinks) -> String {
    match links {
        ExpectedLinks::NotObserved => "\u{1f}\u{21}".to_string(),
        ExpectedLinks::Provided(items) => items
            .iter()
            .map(|(digest, cluster)| format!("\u{1e}{digest}@{cluster}"))
            .collect(),
    }
}

fn canonical_cells(cells: &ExpectedCells) -> String {
    match cells {
        ExpectedCells::NotObserved => "\u{1f}\u{21}".to_string(),
        ExpectedCells::Provided(items) => items
            .iter()
            .map(|(row, column, digest)| format!("\u{1e}{row}/{column}/{digest}"))
            .collect(),
    }
}

/// The canonical text of one assembled expectation, and the input of its pin.
pub fn canonical_expectation(family: &str, id: &str, expected: &Expected) -> String {
    let mut out = format!("{family}\u{1d}{id}\u{1d}");
    match expected {
        Expected::Absent => out.push_str("absent"),
        Expected::Figure(figure) => {
            out.push_str("figure");
            out.push_str(&canonical_slot(figure.label.as_deref()));
            out.push_str(&canonical_slot(figure.head.as_deref()));
            out.push_str(&canonical_slot(figure.caption.as_deref()));
            out.push_str(&canonical_slot(figure.content.as_deref()));
            out.push_str(&canonical_links(&figure.links));
        }
        Expected::Tables(tables) => {
            out.push_str("tables");
            for table in tables {
                out.push('\u{1c}');
                out.push_str(&canonical_slot(table.label.as_deref()));
                out.push_str(&canonical_slot(table.head.as_deref()));
                out.push_str(&canonical_slot(table.caption.as_deref()));
                out.push_str(&canonical_slot(table.content.as_deref()));
                out.push_str(&canonical_slot(table.note.as_deref()));
                out.push_str(&canonical_slot(table.other.as_deref()));
                out.push_str(&canonical_slot(table.reverted.as_deref()));
                out.push_str(&canonical_cells(&table.cells));
                out.push_str(&canonical_links(&table.links));
                out.push_str(&format!("\u{1e}{}", table.cluster_index));
            }
        }
        Expected::Equations { equations, markers } => {
            out.push_str("equations");
            for equation in equations {
                out.push('\u{1c}');
                out.push_str(&equation.content_digest);
                out.push_str(&canonical_slot(equation.label_digest.as_deref()));
                out.push_str(&format!("\u{1e}{}:{}", equation.start, equation.end));
            }
            out.push_str(&format!("\u{1b}{markers}"));
        }
        Expected::Funding {
            entries,
            other_count,
        } => {
            out.push_str("funding");
            for entry in entries {
                out.push('\u{1c}');
                out.push_str(&canonical_slot(entry.funder_name.as_deref()));
                out.push_str(&canonical_slot(entry.grant_name.as_deref()));
                out.push_str(&canonical_slot(entry.grant_number.as_deref()));
                out.push_str(&canonical_slot(entry.program_name.as_deref()));
                out.push_str(&canonical_slot(entry.project_name.as_deref()));
                out.push_str(&canonical_list(&entry.persons));
                out.push_str(&canonical_list(&entry.affiliations));
                out.push_str(&canonical_list(&entry.institutions));
                out.push_str(&canonical_list(&entry.infrastructures));
            }
            out.push_str(&format!("\u{1b}{other_count}"));
        }
    }
    out
}

/// The digest a document's `expectation_sha256` pins.
pub fn expectation_digest(family: &str, id: &str, expected: &Expected) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(canonical_expectation(family, id, expected).as_bytes())
    )
}

/// Java's blank guard, applied to one field: trimmed and non-empty.
fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

/// Java's `validateTable`, transcribed as the independent revert verdict.
///
/// The order is the SUT's documented order — blank guard, then the integer
/// label, then the `tab` prefix, then a non-blank content — so the verdict is a
/// reason and not a bitmask.
fn independent_revert(
    label: Option<&str>,
    header: Option<&str>,
    content: Option<&str>,
) -> Option<String> {
    let label = match non_blank(label) {
        Some(label) => label,
        None => return Some("label_not_integer".to_string()),
    };
    let header = match non_blank(header) {
        Some(header) => header,
        None => return Some("head_not_tab_prefixed".to_string()),
    };
    if label.parse::<i32>().is_err() {
        return Some("label_not_integer".to_string());
    }
    if !header.to_lowercase().starts_with("tab") {
        return Some("head_not_tab_prefixed".to_string());
    }
    if non_blank(content).is_none() {
        return Some("empty_content".to_string());
    }
    None
}

/// The previous base labels that make a repeated head cluster start a new table.
const INDEPENDENT_SPLIT_PREVIOUS: [&str; 3] = ["<content>", "<note>", "<figDesc>"];

/// Assemble the expected figure span of one document.
///
/// A figure decode yields exactly one span, so every marker the fixture declares
/// addresses a cluster of that span: the inventory is attributed whole and never
/// distributed.
fn assemble_figure(tokens: &[String], labels: &[String], doc: &serde_yaml::Value) -> Expected {
    if labels.is_empty() {
        return Expected::Absent;
    }
    let mut label = None;
    let mut head = None;
    let mut caption = None;
    let mut content = None;
    for cluster in independent_clusters(labels) {
        let text = cluster_text(tokens, &cluster);
        match cluster.base.as_str() {
            "<label>" => append_role(&mut label, &text),
            "<figure_head>" => append_role(&mut head, &text),
            "<figDesc>" => append_role(&mut caption, &text),
            "<content>" => append_role(&mut content, &text),
            _ => {}
        }
    }
    Expected::Figure(ExpectedFigure {
        label,
        head,
        caption,
        content,
        links: match expected_markers(doc) {
            Some(items) => ExpectedLinks::Provided(items),
            None => ExpectedLinks::NotObserved,
        },
    })
}

/// Assemble the expected tables of one document, split and reverted by the rules
/// rather than by the implementation under test.
fn assemble_tables(tokens: &[String], labels: &[String], doc: &serde_yaml::Value) -> Expected {
    if labels.is_empty() {
        return Expected::Absent;
    }
    let clusters = independent_clusters(labels);
    let markers = expected_markers(doc);
    let cells = expected_cells(doc);

    let mut tables: Vec<ExpectedTable> = Vec::new();
    let mut current = ExpectedTable {
        label: None,
        head: None,
        caption: None,
        content: None,
        note: None,
        other: None,
        reverted: None,
        cells: ExpectedCells::NotObserved,
        links: ExpectedLinks::NotObserved,
        cluster_index: 0,
    };
    let mut header = String::new();
    let mut previous: Option<String> = None;
    let mut start = 0usize;

    let close = |table: ExpectedTable,
                 header: &str,
                 start: usize,
                 end: usize,
                 index: usize,
                 tables: &mut Vec<ExpectedTable>| {
        let mut table = table;
        table.cells = if index == 0 {
            match &cells {
                Some(provided) => ExpectedCells::Provided(provided.clone()),
                None => ExpectedCells::NotObserved,
            }
        } else {
            ExpectedCells::NotObserved
        };
        let in_range: Vec<(String, usize)> = match &markers {
            Some(items) => items
                .iter()
                .filter(|(_, cluster)| *cluster >= start && *cluster < end)
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        table.links = if in_range.is_empty() {
            ExpectedLinks::NotObserved
        } else {
            ExpectedLinks::Provided(in_range)
        };
        table.reverted = independent_revert(
            table.label.as_deref(),
            Some(header),
            table.content.as_deref(),
        );
        tables.push(table);
    };

    for (index, cluster) in clusters.iter().enumerate() {
        let opens_new_table = index > 0
            && cluster.base == "<figure_head>"
            && !header.is_empty()
            && previous
                .as_deref()
                .is_some_and(|previous| INDEPENDENT_SPLIT_PREVIOUS.contains(&previous));
        if opens_new_table {
            let closed = std::mem::replace(
                &mut current,
                ExpectedTable {
                    label: None,
                    head: None,
                    caption: None,
                    content: None,
                    note: None,
                    other: None,
                    reverted: None,
                    cells: ExpectedCells::NotObserved,
                    links: ExpectedLinks::NotObserved,
                    cluster_index: index,
                },
            );
            close(closed, &header, start, index, tables.len(), &mut tables);
            header.clear();
            start = index;
        }
        let text = cluster_text(tokens, cluster);
        match cluster.base.as_str() {
            "<label>" => {
                append_role(&mut current.label, &text);
                header.push(' ');
                header.push_str(&text);
                header.push(' ');
            }
            "<figure_head>" => {
                append_role(&mut current.head, &text);
                header.push_str(&text);
            }
            "<figDesc>" => append_role(&mut current.caption, &text),
            "<content>" => append_role(&mut current.content, &text),
            "<note>" => append_role(&mut current.note, &text),
            "<other>" => append_role(&mut current.other, &text),
            _ => {}
        }
        previous = Some(cluster.base.clone());
    }
    let last = std::mem::replace(
        &mut current,
        ExpectedTable {
            label: None,
            head: None,
            caption: None,
            content: None,
            note: None,
            other: None,
            reverted: None,
            cells: ExpectedCells::NotObserved,
            links: ExpectedLinks::NotObserved,
            cluster_index: 0,
        },
    );
    close(
        last,
        &header,
        start,
        clusters.len(),
        tables.len(),
        &mut tables,
    );
    Expected::Tables(tables)
}

/// One equation being accumulated by the independent join.
struct IndependentEquation {
    start: usize,
    end: usize,
    rows: Vec<String>,
    label_rows: Vec<String>,
}

/// Assemble the expected equations of one document by the join rules:
/// `<equation>` always opens, `<equation_label>` attaches to the open equation or
/// opens a label-only span, and any other base closes the open one.
fn assemble_equations(labels: &[String]) -> Expected {
    if labels.is_empty() {
        return Expected::Absent;
    }
    let clusters = independent_clusters(labels);
    let mut equations: Vec<IndependentEquation> = Vec::new();
    let mut open: Option<IndependentEquation> = None;

    for cluster in &clusters {
        let rows: Vec<String> = labels[cluster.start..cluster.end].to_vec();
        if cluster.base == "<equation>" {
            if let Some(previous) = open.take() {
                equations.push(previous);
            }
            open = Some(IndependentEquation {
                start: cluster.start,
                end: cluster.end,
                rows,
                label_rows: Vec::new(),
            });
        } else if cluster.base == "<equation_label>" {
            match open.as_mut() {
                Some(equation) => {
                    equation.end = cluster.end;
                    equation.label_rows.extend(rows.iter().cloned());
                    equation.rows.extend(rows);
                }
                None => {
                    open = Some(IndependentEquation {
                        start: cluster.start,
                        end: cluster.end,
                        rows: rows.clone(),
                        label_rows: rows,
                    })
                }
            }
        } else if let Some(previous) = open.take() {
            equations.push(previous);
        }
    }
    if let Some(equation) = open.take() {
        equations.push(equation);
    }
    if equations.is_empty() {
        return Expected::Absent;
    }
    let markers = clusters
        .iter()
        .filter(|cluster| cluster.base == "<equation_marker>")
        .count();
    Expected::Equations {
        equations: equations
            .into_iter()
            .map(|equation| ExpectedEquation {
                content_digest: spelling_digest(&equation.rows),
                label_digest: (!equation.label_rows.is_empty())
                    .then(|| spelling_digest(&equation.label_rows)),
                start: equation.start,
                end: equation.end,
            })
            .collect(),
        markers,
    }
}

/// Java's `Funding.isValid()`: does this carry a *funding record* field?
fn independent_is_valid(entry: &ExpectedFundingEntry) -> bool {
    entry.funder_name.is_some()
        || entry.grant_name.is_some()
        || entry.grant_number.is_some()
        || entry.program_name.is_some()
        || entry.project_name.is_some()
}

/// The emission predicate: the conservative superset of [`independent_is_valid`].
fn independent_carries_observation(entry: &ExpectedFundingEntry) -> bool {
    independent_is_valid(entry)
        || !entry.persons.is_empty()
        || !entry.affiliations.is_empty()
        || !entry.institutions.is_empty()
        || !entry.infrastructures.is_empty()
}

/// The five scalar roles, by base label.
fn independent_scalar_slot(entry: &ExpectedFundingEntry, base: &str) -> bool {
    match base {
        "<funderName>" => entry.funder_name.is_some(),
        "<grantName>" => entry.grant_name.is_some(),
        "<grantNumber>" => entry.grant_number.is_some(),
        "<programName>" => entry.program_name.is_some(),
        "<projectName>" => entry.project_name.is_some(),
        _ => false,
    }
}

fn independent_set_scalar(entry: &mut ExpectedFundingEntry, base: &str, text: String) -> bool {
    match base {
        "<funderName>" => entry.funder_name = Some(text),
        "<grantName>" => entry.grant_name = Some(text),
        "<grantNumber>" => entry.grant_number = Some(text),
        "<programName>" => entry.program_name = Some(text),
        "<projectName>" => entry.project_name = Some(text),
        _ => return false,
    }
    true
}

/// Assemble the expected funding entries by the role table and the two
/// predicates: the split uses validity, the emission uses the superset.
fn assemble_funding(tokens: &[String], labels: &[String]) -> Expected {
    if labels.is_empty() {
        return Expected::Absent;
    }
    let mut entries: Vec<ExpectedFundingEntry> = Vec::new();
    let mut entry = ExpectedFundingEntry::default();
    let mut other_count = 0usize;

    for cluster in independent_clusters(labels) {
        let text = cluster_text(tokens, &cluster);
        match cluster.base.as_str() {
            "<other>" => {
                if !text.is_empty() {
                    other_count += 1;
                }
            }
            "<person>" => {
                if !text.is_empty() {
                    entry.persons.push(text);
                }
            }
            "<affiliation>" => {
                if !text.is_empty() {
                    entry.affiliations.push(text);
                }
            }
            "<institution>" => {
                if !text.is_empty() {
                    entry.institutions.push(text);
                }
            }
            "<infrastructure>" => {
                if !text.is_empty() {
                    entry.institutions.push(text.clone());
                    entry.infrastructures.push(text);
                }
            }
            scalar => {
                if text.is_empty() {
                    continue;
                }
                if independent_scalar_slot(&entry, scalar) && independent_is_valid(&entry) {
                    entries.push(std::mem::take(&mut entry));
                }
                assert!(
                    independent_set_scalar(&mut entry, scalar, text),
                    "the fixture must only use the ten declared funding bases"
                );
            }
        }
    }
    if independent_carries_observation(&entry) {
        entries.push(entry);
    }
    if entries.is_empty() && other_count == 0 {
        return Expected::Absent;
    }
    Expected::Funding {
        entries,
        other_count,
    }
}

/// The `caption_markers` inventory of one document, or `None` when the fixture
/// declares none. A declared-but-empty list and an absent key both mean
/// "no inventory was observed".
fn expected_markers(doc: &serde_yaml::Value) -> Option<Vec<(String, usize)>> {
    let markers = doc.get("caption_markers")?.as_sequence()?;
    if markers.is_empty() {
        return None;
    }
    Some(
        markers
            .iter()
            .map(|marker| {
                (
                    marker["digest"].as_str().expect("digest").to_string(),
                    marker["cluster_index"].as_u64().expect("cluster_index") as usize,
                )
            })
            .collect(),
    )
}

/// The `cells` inventory of one document, or `None` when the fixture declares
/// none.
fn expected_cells(doc: &serde_yaml::Value) -> Option<Vec<(u32, u32, String)>> {
    let cells = doc.get("cells")?.as_sequence()?;
    if cells.is_empty() {
        return None;
    }
    Some(
        cells
            .iter()
            .map(|cell| {
                (
                    cell["row"].as_u64().expect("row") as u32,
                    cell["column"].as_u64().expect("column") as u32,
                    cell["digest"].as_str().expect("digest").to_string(),
                )
            })
            .collect(),
    )
}

/// One fixture document, with its independently assembled expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCase {
    pub id: String,
    pub outcome: String,
    pub tokens: Vec<String>,
    pub labels: Vec<String>,
    pub pinned: String,
    pub expected: Expected,
}

/// Assemble every document of one family, in fixture order.
pub fn assemble_family(family: &str) -> Vec<OracleCase> {
    let document = oracle_document().expect("the reviewed oracle fixture must parse as YAML");
    let documents = document["families"][family]["documents"]
        .as_sequence()
        .unwrap_or_else(|| panic!("the {family} family must declare documents"))
        .clone();
    documents
        .iter()
        .map(|doc| {
            let id = doc["id"].as_str().expect("id").to_string();
            let outcome = doc["outcome"].as_str().expect("outcome").to_string();
            let tokens: Vec<String> = doc["tokens"]
                .as_sequence()
                .expect("tokens")
                .iter()
                .map(|token| token.as_str().unwrap_or_default().to_string())
                .collect();
            let labels: Vec<String> = doc["labels"]
                .as_sequence()
                .expect("labels")
                .iter()
                .map(|label| label.as_str().unwrap_or_default().to_string())
                .collect();
            let pinned = doc["expectation_sha256"]
                .as_str()
                .expect("expectation_sha256")
                .to_string();
            let expected = match family {
                "figure" => assemble_figure(&tokens, &labels, doc),
                "table" => assemble_tables(&tokens, &labels, doc),
                "formula" => assemble_equations(&labels),
                "funding" => assemble_funding(&tokens, &labels),
                other => panic!("{other} is not one of the four families"),
            };
            assert_eq!(
                expected == Expected::Absent,
                outcome == "not_observed",
                "{family}/{id}: the fixture outcome and the assembled expectation disagree"
            );
            OracleCase {
                id,
                outcome,
                tokens,
                labels,
                pinned,
                expected,
            }
        })
        .collect()
}

/// Every forbidden payload field a digest-only fixture may not carry, anywhere.
pub fn forbidden_fields_present(value: &serde_yaml::Value, field: &str) -> bool {
    match value {
        serde_yaml::Value::Mapping(map) => map.iter().any(|(key, child)| {
            key.as_str() == Some(field) || forbidden_fields_present(child, field)
        }),
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .any(|item| forbidden_fields_present(item, field)),
        _ => false,
    }
}

// ---- end independent expectation builder ----

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> Vec<u8> {
    fs::read(root().join(relative))
        .unwrap_or_else(|error| panic!("FATAL {relative}: fixture missing/unreadable ({error})"))
}

/// Read a fixture and parse it with the production model reader. A missing or
/// malformed fixture is FATAL: the oracle must never silently shrink.
fn load_model(relative: &str) -> CrfModel {
    CrfModel::parse(&read(relative))
        .unwrap_or_else(|error| panic!("FATAL {relative}: fixture does not parse ({error})"))
}

fn oracle() -> serde_yaml::Value {
    serde_yaml::from_slice(&read(ORACLE)).expect(ORACLE)
}

fn data() -> serde_yaml::Value {
    serde_yaml::from_slice(&read(DATA)).expect(DATA)
}

fn data_number(path: &[&str]) -> u64 {
    let value = data();
    let mut current = &value;
    for key in path {
        current = current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"));
    }
    current.as_u64().unwrap_or_else(|| panic!("{path:?}"))
}

/// Every token fact across the whole family, so a repeat can be detected.
fn family_tokens(cases: &[OracleCase]) -> Vec<String> {
    cases.iter().flat_map(|case| case.tokens.clone()).collect()
}

/// One 27-column fulltext row whose first column carries the token text.
fn fulltext_rows(tokens: &[String]) -> Vec<FulltextFeatureRow> {
    tokens
        .iter()
        .map(|token| FulltextFeatureRow {
            columns: std::iter::once(token.clone())
                .chain((1..NATIVE_FULLTEXT_FEATURE_COLUMNS).map(|_| "0".to_string()))
                .collect(),
        })
        .collect()
}

/// One 18-column funding row per token, rendered through the shipped ABI.
fn funding_rows(tokens: &[String]) -> Vec<FundingFeatureRow> {
    let last = tokens.len().saturating_sub(1);
    let facts: Vec<FundingFeatureToken> = tokens
        .iter()
        .enumerate()
        .map(|(index, text)| FundingFeatureToken {
            text: text.clone(),
            line_status: if index == 0 {
                FundingLineStatus::Linestart
            } else if index == last {
                FundingLineStatus::Lineend
            } else {
                FundingLineStatus::Linein
            },
        })
        .collect();
    render_funding_features(
        &facts,
        &NativeFundingGazetteerCoverage {
            funders: Vec::new(),
            infrastructures: Vec::new(),
        },
    )
    .unwrap_or_else(|error| panic!("the funding ABI refused the oracle tokens: {error}"))
}

fn markers_of(case: &OracleCase) -> Vec<NativeCaptionMarker> {
    match &case.expected {
        Expected::Figure(figure) => marker_values(&figure.links),
        Expected::Tables(tables) => tables
            .iter()
            .flat_map(|table| marker_values(&table.links))
            .collect(),
        _ => Vec::new(),
    }
}

fn marker_values(links: &ExpectedLinks) -> Vec<NativeCaptionMarker> {
    match links {
        ExpectedLinks::NotObserved => Vec::new(),
        ExpectedLinks::Provided(items) => items
            .iter()
            .map(|(digest, cluster_index)| NativeCaptionMarker {
                digest: digest.clone(),
                cluster_index: *cluster_index,
            })
            .collect(),
    }
}

fn cells_of(case: &OracleCase) -> NativeTableCells {
    match &case.expected {
        Expected::Tables(tables) => match &tables[0].cells {
            ExpectedCells::NotObserved => NativeTableCells::NotObserved {
                reason: NativeTableAbsence::NoCellInventory,
            },
            ExpectedCells::Provided(items) => NativeTableCells::Provided {
                cells: items
                    .iter()
                    .map(|(row, column, digest)| NativeTableCell {
                        row: *row,
                        column: *column,
                        content_digest: digest.clone(),
                    })
                    .collect(),
            },
        },
        _ => NativeTableCells::NotObserved {
            reason: NativeTableAbsence::NoCellInventory,
        },
    }
}

fn links_match(
    expected: &ExpectedLinks,
    actual: &NativeCaptionLinks,
    who: &str,
) -> Result<(), String> {
    match expected {
        ExpectedLinks::NotObserved => {
            if actual.absence != Some(NativeCaptionLinkAbsence::NotObserved) {
                return Err(format!(
                    "{who}: an absent inventory must report `not_observed`, got {:?}",
                    actual.absence
                ));
            }
            if !actual.markers.is_empty() {
                return Err(format!(
                    "{who}: an absent inventory must carry no marker, got {}",
                    actual.markers.len()
                ));
            }
        }
        ExpectedLinks::Provided(items) => {
            if actual.absence.is_some() {
                return Err(format!(
                    "{who}: a provided inventory must not report an absence, got {:?}",
                    actual.absence
                ));
            }
            let actual: Vec<(String, usize)> = actual
                .markers
                .iter()
                .map(|marker| (marker.digest.clone(), marker.cluster_index))
                .collect();
            if actual != *items {
                return Err(format!("{who}: expected markers {items:?}, got {actual:?}"));
            }
        }
    }
    Ok(())
}

fn cells_match(
    expected: &ExpectedCells,
    actual: &NativeTableCells,
    who: &str,
) -> Result<(), String> {
    match expected {
        ExpectedCells::NotObserved => match actual {
            NativeTableCells::NotObserved { .. } => Ok(()),
            NativeTableCells::Provided { cells } => Err(format!(
                "{who}: expected a typed cell absence, got {} cells",
                cells.len()
            )),
        },
        ExpectedCells::Provided(items) => match actual {
            NativeTableCells::NotObserved { reason } => Err(format!(
                "{who}: expected {} cells, got the typed absence {reason:?}",
                items.len()
            )),
            NativeTableCells::Provided { cells } => {
                let actual: Vec<(u32, u32, String)> = cells
                    .iter()
                    .map(|cell| (cell.row, cell.column, cell.content_digest.clone()))
                    .collect();
                if actual != *items {
                    return Err(format!("{who}: expected cells {items:?}, got {actual:?}"));
                }
                Ok(())
            }
        },
    }
}

fn slot_matches(
    expected: &Option<String>,
    actual: &Option<String>,
    field: &str,
    who: &str,
) -> Result<(), String> {
    if expected != actual {
        return Err(format!(
            "{who}: expected {field} {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn figure_matches(expected: &ExpectedFigure, span: &FigureSpan, who: &str) -> Result<(), String> {
    slot_matches(&expected.label, &span.label, "label", who)?;
    slot_matches(&expected.head, &span.head, "head", who)?;
    slot_matches(&expected.caption, &span.caption, "caption", who)?;
    slot_matches(&expected.content, &span.content, "content", who)?;
    links_match(&expected.links, &span.caption_links, who)
}

fn table_matches(
    expected: &ExpectedTable,
    span: &NativeTableSpan,
    who: &str,
) -> Result<(), String> {
    slot_matches(&expected.label, &span.label, "label", who)?;
    slot_matches(&expected.head, &span.head, "head", who)?;
    slot_matches(&expected.caption, &span.caption, "caption", who)?;
    slot_matches(&expected.content, &span.content, "content", who)?;
    slot_matches(&expected.note, &span.note, "note", who)?;
    slot_matches(&expected.other, &span.other, "other", who)?;
    let reverted = span.reverted.map(|reason| {
        serde_json::to_value(reason)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    });
    if reverted != expected.reverted {
        return Err(format!(
            "{who}: expected reverted {:?}, got {reverted:?}",
            expected.reverted
        ));
    }
    if span.cluster_index != expected.cluster_index {
        return Err(format!(
            "{who}: expected cluster_index {}, got {}",
            expected.cluster_index, span.cluster_index
        ));
    }
    cells_match(&expected.cells, &span.cells, who)?;
    links_match(&expected.links, &span.caption_links, who)
}

fn funding_entry_matches(
    expected: &ExpectedFundingEntry,
    actual: &NativeFundingEntry,
    who: &str,
) -> Result<(), String> {
    slot_matches(
        &expected.funder_name,
        &actual.funder_name,
        "funder_name",
        who,
    )?;
    slot_matches(&expected.grant_name, &actual.grant_name, "grant_name", who)?;
    slot_matches(
        &expected.grant_number,
        &actual.grant_number,
        "grant_number",
        who,
    )?;
    slot_matches(
        &expected.program_name,
        &actual.program_name,
        "program_name",
        who,
    )?;
    slot_matches(
        &expected.project_name,
        &actual.project_name,
        "project_name",
        who,
    )?;
    if expected.persons != actual.persons {
        return Err(format!(
            "{who}: expected persons {:?}, got {:?}",
            expected.persons, actual.persons
        ));
    }
    if expected.affiliations != actual.affiliations {
        return Err(format!(
            "{who}: expected affiliations {:?}, got {:?}",
            expected.affiliations, actual.affiliations
        ));
    }
    if expected.institutions != actual.institutions {
        return Err(format!(
            "{who}: expected institutions {:?}, got {:?}",
            expected.institutions, actual.institutions
        ));
    }
    if expected.infrastructures != actual.infrastructures {
        return Err(format!(
            "{who}: expected infrastructures {:?}, got {:?}",
            expected.infrastructures, actual.infrastructures
        ));
    }
    Ok(())
}

fn pinned_digest_matches(case: &OracleCase, family: &str) {
    let computed = expectation_digest(family, &case.id, &case.expected);
    assert_eq!(
        computed,
        case.pinned,
        "{family}/{}: the assembled expectation drifted from its reviewed pin\n  canonical: {}\n",
        case.id,
        canonical_expectation(family, &case.id, &case.expected).escape_debug()
    );
}

fn pin_verdict(bytes: &[u8], pin: &str) -> Result<(), String> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest == pin {
        Ok(())
    } else {
        Err(format!("fixture drift: {digest} is not the pinned {pin}"))
    }
}

#[test]
fn oracle_metadata_is_strict_and_carries_no_payload_field() {
    let value = oracle();
    assert_eq!(value["schema_version"].as_str(), Some(SCHEMA));
    assert_eq!(
        value["review_status"].as_str(),
        Some("independently_reviewed")
    );
    assert_eq!(value["representation"].as_str(), Some("digest_only"));
    assert_eq!(
        value["expected_rows_not_from_sut_helpers"].as_bool(),
        Some(true)
    );
    assert_eq!(
        value["provenance"]["label_dictionaries"].as_str(),
        Some(DATA),
        "the dictionaries this oracle transcribes must stay named"
    );

    let declared: Vec<&str> = value["forbidden_fields"]
        .as_sequence()
        .expect("forbidden_fields must be a sequence")
        .iter()
        .filter_map(|field| field.as_str())
        .collect();
    for field in FORBIDDEN_FIELDS {
        assert!(
            declared.contains(&field),
            "the digest-only contract must declare `{field}` forbidden"
        );
        assert!(
            !forbidden_fields_present(&value, field),
            "the fixture must not carry the payload field `{field}`"
        );
    }

    let families: BTreeSet<String> = value["families"]
        .as_mapping()
        .expect("families must be a mapping")
        .keys()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        families,
        FAMILIES.iter().map(|name| (*name).to_string()).collect(),
        "the family set is closed at four"
    );

    // Every document declares the four facts the builder needs, and a pin.
    let mut total = 0usize;
    for family in FAMILIES {
        for case in assemble_family(family) {
            assert!(
                matches!(case.outcome.as_str(), "decoded" | "not_observed"),
                "{family}/{}: unknown outcome {}",
                case.id,
                case.outcome
            );
            assert!(
                !case.tokens.is_empty(),
                "{family}/{}: no token facts",
                case.id
            );
            assert!(
                case.pinned.len() == 71 && case.pinned.starts_with("sha256:"),
                "{family}/{}: the expectation pin must be one `sha256:` digest",
                case.id
            );
            if case.outcome == "decoded" {
                assert_eq!(
                    case.tokens.len(),
                    case.labels.len(),
                    "{family}/{}: one reviewed label per token row",
                    case.id
                );
            }
            total += 1;
        }
    }
    assert_eq!(total, 14, "the oracle must keep every reviewed document");
}

#[test]
fn fixture_sha256_pins_are_stable_and_drift_is_detected() {
    // The pins are recorded here rather than derived, so an edit to any fixture
    // is a deliberate act: the failing assertion names the file.
    let pins: [(&str, &str); 4] = [
        (ORACLE, ORACLE_SHA256),
        (FIGURE_MODEL, FIGURE_MODEL_SHA256),
        (TABLE_MODEL, TABLE_MODEL_SHA256),
        (FUNDING_MODEL, FUNDING_MODEL_SHA256),
    ];
    for (path, pin) in pins {
        let bytes = read(path);
        pin_verdict(&bytes, pin).unwrap_or_else(|error| panic!("{path}: {error}"));
    }

    // The polarity of the check, proven on the same bytes: a single flipped byte
    // is a detected drift, not a silent pass.
    let mut drifted = read(ORACLE);
    drifted[0] ^= 0x01;
    assert!(
        pin_verdict(&drifted, ORACLE_SHA256).is_err(),
        "a mutated fixture must fail its pin"
    );
    assert!(pin_verdict(&drifted, "0".repeat(64).as_str()).is_err());
    assert!(pin_verdict(&read(TABLE_MODEL), FIGURE_MODEL_SHA256).is_err());
}

#[test]
fn every_synthetic_model_declares_the_data_dictionary_and_window() {
    let declarations: [(&str, &str, &[&str], u64, usize); 3] = [
        (FIGURE_MODEL, "figure", &NATIVE_FIGURE_LABELS, 25, 7),
        (TABLE_MODEL, "table", &NATIVE_TABLE_LABELS, 25, 10),
        (FUNDING_MODEL, "funding", &NATIVE_FUNDING_LABELS, 16, 20),
    ];
    for (path, family, labels, window, count) in declarations {
        let model = load_model(path);
        assert_eq!(model.model_type, 2, "{path} must be a scalar CRF model");
        assert_eq!(model.labels.len(), count, "{path} label count");
        assert_eq!(labels.len(), count, "{family} dictionary count");
        for label in labels {
            assert!(
                model.labels.iter().any(|declared| declared == label),
                "{path} must declare the reviewed label `{label}`"
            );
        }
        // The declared `#rdr#` window of the DATA file is the highest referenced
        // column index, and the synthetic model must reference exactly that
        // column: the ABI it consumes is the family's declared ABI.
        let declared_window = data_number(&["families", family, "referenced_columns"]) - 1;
        assert_eq!(declared_window, window, "{family} DATA window");
        let raw = String::from_utf8(read(path)).expect("the fixture is UTF-8");
        let rdr = raw
            .lines()
            .find_map(|line| line.strip_prefix("#rdr#"))
            .expect("the fixture declares a reader header");
        let max_column: usize = rdr
            .split('/')
            .nth(1)
            .and_then(|field| field.parse().ok())
            .expect("the reader header declares a max column index");
        assert_eq!(
            max_column as u64, declared_window,
            "{family} declared window"
        );
    }
    // The Rust constants mirror the same width, so a silent move is caught.
    assert_eq!(NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX, 25);
    assert_eq!(NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, 25);
    assert_eq!(NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, 16);
    assert_eq!(NATIVE_FIGURE_BASE_LABELS.len(), 4);
    assert_eq!(NATIVE_TABLE_BASE_LABELS.len(), 6);
    assert_eq!(NATIVE_FUNDING_BASE_LABELS.len(), 10);
}

#[test]
fn tokens_are_unique_within_a_family_so_a_model_can_key_on_them() {
    for family in ["figure", "table", "funding"] {
        let cases = assemble_family(family);
        let tokens = family_tokens(&cases);
        let unique: BTreeSet<&String> = tokens.iter().collect();
        assert_eq!(
            unique.len(),
            tokens.len(),
            "{family}: a repeated token fact would make the synthetic model's decode ambiguous"
        );
    }
    // The check has teeth: a duplicated fact is visible here rather than silently
    // decoding the wrong label.
    let repeated = ["Alpha".to_string(), "Alpha".to_string()];
    assert_eq!(family_tokens(&[]).len(), 0);
    assert_ne!(
        repeated.iter().collect::<BTreeSet<_>>().len(),
        repeated.len()
    );
}

/// The independent clusteror tiles every reviewed sequence: the clusters cover
/// the rows exactly once, and each cluster's opener is the spelling at its first
/// row. This is the invariant the `I-` boundary rests on, checked on the reviewed
/// facts and not on a runner.
#[test]
fn the_independent_clusteror_tiles_the_reviewed_sequences() {
    let mut checked = 0usize;
    for family in FAMILIES {
        for case in assemble_family(family) {
            if case.labels.is_empty() {
                assert!(independent_clusters(&case.labels).is_empty());
                continue;
            }
            let clusters = independent_clusters(&case.labels);
            assert_eq!(clusters.first().map(|cluster| cluster.start), Some(0));
            assert_eq!(
                clusters.last().map(|cluster| cluster.end),
                Some(case.labels.len())
            );
            let mut cursor = 0usize;
            for cluster in &clusters {
                assert_eq!(cluster.start, cursor, "{family}/{}: gap", case.id);
                assert!(cluster.start < cluster.end, "{family}/{}: empty", case.id);
                assert_eq!(
                    cluster.opened_by_prefix,
                    case.labels[cluster.start].starts_with("I-"),
                    "{family}/{}: the opener is the first row's spelling",
                    case.id
                );
                assert_eq!(
                    cluster.base,
                    independent_base(&case.labels[cluster.start]),
                    "{family}/{}",
                    case.id
                );
                cursor = cluster.end;
            }
            assert_eq!(cursor, case.labels.len());
            checked += 1;
        }
    }
    assert_eq!(checked, 11, "every non-empty reviewed document is tiled");

    // The polarity of the rule, stated as its own witness: the base-name rule
    // would merge these two rows, the clusteror rule must not, and identical
    // bare spellings must merge.
    assert_eq!(independent_clusters(&["I-<content>".into()]).len(), 1);
    assert_eq!(
        independent_clusters(&["I-<content>".into(), "<content>".into()]).len(),
        2
    );
    assert_eq!(
        independent_clusters(&["<content>".into(), "<content>".into()]).len(),
        1
    );
}

/// Build the rows and markers the family's runner consumes for one document.
fn figure_rows(case: &OracleCase) -> (Vec<FulltextFeatureRow>, Vec<NativeCaptionMarker>) {
    (fulltext_rows(&case.tokens), markers_of(case))
}

fn decoded_figure(case: &OracleCase, model: &CrfModel) -> NativeFigureDecodeOutcome {
    let (rows, markers) = figure_rows(case);
    if case.labels.is_empty() {
        // An empty decode is the only way this family reports absence, so the
        // seam is driven directly; the runner refuses an empty *row* list.
        return figure_spans_from_decoded(&[], &rows, &markers)
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
    }
    decode_figure_sequence(&rows, model, &markers)
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
}

#[test]
fn figure_decodes_match_the_independently_assembled_expectation() {
    let model = load_model(FIGURE_MODEL);
    let mut decoded = 0;
    for case in assemble_family("figure") {
        pinned_digest_matches(&case, "figure");
        let outcome = decoded_figure(&case, &model);
        match (&case.expected, &outcome) {
            (Expected::Figure(expected), NativeFigureDecodeOutcome::Decoded { labels, spans }) => {
                assert_eq!(
                    labels, &case.labels,
                    "{}: the model decoded other labels",
                    case.id
                );
                assert_eq!(spans.len(), 1, "one figure cluster yields exactly one span");
                figure_matches(expected, &spans[0], &case.id).unwrap();
                decoded += 1;
            }
            (Expected::Absent, NativeFigureDecodeOutcome::NotObserved { reason }) => {
                assert_eq!(*reason, NativeFigureAbsence::NoFigureClusters);
            }
            other => panic!("{}: expectation and outcome disagree: {other:?}", case.id),
        }
    }
    assert_eq!(decoded, 2, "every positive figure document is evaluated");
}

#[test]
fn table_decodes_match_the_independently_assembled_expectation() {
    let model = load_model(TABLE_MODEL);
    let mut decoded = 0;
    for case in assemble_family("table") {
        pinned_digest_matches(&case, "table");
        let rows = fulltext_rows(&case.tokens);
        let markers = markers_of(&case);
        let cells = cells_of(&case);
        let outcome = if case.labels.is_empty() {
            table_spans_from_decoded(&[], &rows, &markers, cells)
                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
        } else {
            decode_table_sequence(&rows, &model, &markers, cells)
                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
        };
        match (&case.expected, &outcome) {
            (Expected::Tables(expected), NativeTableDecodeOutcome::Decoded { labels, tables }) => {
                assert_eq!(
                    labels, &case.labels,
                    "{}: the model decoded other labels",
                    case.id
                );
                assert_eq!(
                    tables.len(),
                    expected.len(),
                    "{}: a different number of tables was assembled",
                    case.id
                );
                for (index, (expected, actual)) in expected.iter().zip(tables.iter()).enumerate() {
                    table_matches(expected, actual, &format!("{}/table{index}", case.id)).unwrap();
                }
                decoded += 1;
            }
            (Expected::Absent, NativeTableDecodeOutcome::NotObserved { reason }) => {
                assert_eq!(*reason, NativeTableAbsence::NoTableClusters);
            }
            other => panic!("{}: expectation and outcome disagree: {other:?}", case.id),
        }
    }
    assert_eq!(decoded, 3, "every positive table document is evaluated");
}

#[test]
fn formula_joins_match_the_independently_assembled_expectation() {
    let mut decoded = 0;
    for case in assemble_family("formula") {
        pinned_digest_matches(&case, "formula");
        let outcome = join_native_equation_labels(&case.labels)
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        match (&case.expected, &outcome) {
            (
                Expected::Equations { equations, markers },
                NativeFormulaJoinOutcome::Joined {
                    equations: actual,
                    markers: actual_markers,
                },
            ) => {
                assert_eq!(markers, actual_markers, "{}: marker count", case.id);
                assert_eq!(equations.len(), actual.len(), "{}: equation count", case.id);
                for (expected, actual) in equations.iter().zip(actual.iter()) {
                    assert_eq!(
                        expected.content_digest, actual.content_digest,
                        "{}",
                        case.id
                    );
                    assert_eq!(expected.label_digest, actual.label_digest, "{}", case.id);
                    assert_eq!(
                        (expected.start, expected.end),
                        (actual.start, actual.end),
                        "{}",
                        case.id
                    );
                    assert_eq!(
                        actual.page_span, None,
                        "the carrier carries no page ordinal"
                    );
                }
                decoded += 1;
            }
            (Expected::Absent, NativeFormulaJoinOutcome::NotObserved { reason }) => {
                assert_eq!(*reason, NativeFormulaAbsence::NoEquationLabels);
            }
            other => panic!("{}: expectation and outcome disagree: {other:?}", case.id),
        }
    }
    assert_eq!(decoded, 2, "every positive formula document is evaluated");
}

#[test]
fn funding_decodes_match_the_independently_assembled_expectation() {
    let model = load_model(FUNDING_MODEL);
    let mut decoded = 0;
    for case in assemble_family("funding") {
        pinned_digest_matches(&case, "funding");
        let rows = funding_rows(&case.tokens);
        assert!(
            rows.iter()
                .all(|row| row.width() == NATIVE_FUNDING_ROW_COLUMNS),
            "{}: the funding ABI must print {NATIVE_FUNDING_ROW_COLUMNS} columns",
            case.id
        );
        let outcome = if case.labels.is_empty() {
            funding_entries_from_decoded(&[], &rows)
                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
        } else {
            decode_funding_sequence(&rows, &model)
                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
        };
        match (&case.expected, &outcome) {
            (
                Expected::Funding {
                    entries,
                    other_count,
                },
                NativeFundingDecodeOutcome::Decoded {
                    labels,
                    entries: actual,
                    other_count: actual_other,
                },
            ) => {
                assert_eq!(
                    labels, &case.labels,
                    "{}: the model decoded other labels",
                    case.id
                );
                assert_eq!(other_count, actual_other, "{}: `<other>` count", case.id);
                assert_eq!(entries.len(), actual.len(), "{}: entry count", case.id);
                for (index, (expected, actual)) in entries.iter().zip(actual.iter()).enumerate() {
                    funding_entry_matches(expected, actual, &format!("{}/entry{index}", case.id))
                        .unwrap();
                }
                decoded += 1;
            }
            (Expected::Absent, NativeFundingDecodeOutcome::NotObserved { reason }) => {
                assert_eq!(*reason, NativeFundingAbsence::NoFundingLabels);
            }
            other => panic!("{}: expectation and outcome disagree: {other:?}", case.id),
        }
    }
    assert_eq!(decoded, 3, "every positive funding document is evaluated");
}

/// H12-N1 for all four families: an absence is a typed absence and never a zero
/// count, and the two are distinguishable in the serialized shape.
#[test]
fn absence_is_a_typed_absence_and_never_a_zero_count() {
    let rows = fulltext_rows(&["Absent".to_string()]);

    let figure =
        figure_spans_from_decoded(&[], &rows, &[]).expect("the empty decode is the absence");
    match &figure {
        NativeFigureDecodeOutcome::NotObserved { reason } => {
            assert_eq!(*reason, NativeFigureAbsence::NoFigureClusters)
        }
        other => panic!("an empty figure decode became a figure: {other:?}"),
    }
    let encoded = serde_json::to_value(&figure).expect("the outcome serializes");
    assert_eq!(encoded["outcome"], "not_observed");
    assert!(
        encoded.get("spans").is_none(),
        "the absence must not carry a spans field a consumer could read as zero"
    );

    let table = table_spans_from_decoded(&[], &rows, &[], cells_of(&assemble_family("table")[3]))
        .expect("the empty decode is the absence");
    match &table {
        NativeTableDecodeOutcome::NotObserved { reason } => {
            assert_eq!(*reason, NativeTableAbsence::NoTableClusters)
        }
        other => panic!("an empty table decode became a table: {other:?}"),
    }
    let encoded = serde_json::to_value(&table).expect("the outcome serializes");
    assert_eq!(encoded["outcome"], "not_observed");
    assert!(encoded.get("tables").is_none());

    let nil_rows = funding_rows(&["Nil".to_string()]);
    let funding = funding_entries_from_decoded(&[], &nil_rows).expect("the empty decode");
    match &funding {
        NativeFundingDecodeOutcome::NotObserved { reason } => {
            assert_eq!(*reason, NativeFundingAbsence::NoFundingLabels)
        }
        other => panic!("an empty funding decode became a record: {other:?}"),
    }
    let encoded = serde_json::to_value(&funding).expect("the outcome serializes");
    assert_eq!(encoded["outcome"], "not_observed");
    assert!(encoded.get("entries").is_none());
    assert!(encoded.get("other_count").is_none());

    let markers_only =
        join_native_equation_labels(&["<equation_marker>".to_string(), "<paragraph>".to_string()])
            .expect("markers alone are an absence");
    match &markers_only {
        NativeFormulaJoinOutcome::NotObserved { reason } => {
            assert_eq!(*reason, NativeFormulaAbsence::NoEquationLabels)
        }
        other => panic!("markers alone became equations: {other:?}"),
    }
    let encoded = serde_json::to_value(&markers_only).expect("the outcome serializes");
    assert_eq!(encoded["outcome"], "not_observed");
    assert!(
        encoded.get("equations").is_none() && encoded.get("markers").is_none(),
        "the absence must not carry a zero-count markers field"
    );

    // The counter-example that keeps the check from being vacuous: a decode whose
    // only label is `<other>` *was* observed, so it is decoded with zero entries
    // and `other_count` 1 rather than reported as absent.
    let other_only = funding_entries_from_decoded(
        &["<other>".to_string()],
        &funding_rows(&["Noted".to_string()]),
    )
    .expect("an `<other>`-only decode");
    match other_only {
        NativeFundingDecodeOutcome::Decoded {
            entries,
            other_count,
            ..
        } => {
            assert!(entries.is_empty());
            assert_eq!(other_count, 1, "counted, never dropped");
        }
        other => panic!("an observed `<other>` became an absence: {other:?}"),
    }
}

/// The caption and cell lanes report `not_observed` for an empty inventory and
/// never zero; the reviewed fixture's absent lanes are exactly that polarity.
#[test]
fn caption_links_and_cells_report_not_observed_rather_than_zero() {
    let rows = fulltext_rows(&["Fig.".to_string()]);
    let decoded = figure_spans_from_decoded(&["I-<label>".to_string()], &rows, &[])
        .expect("one label decodes");
    match decoded {
        NativeFigureDecodeOutcome::Decoded { spans, .. } => {
            assert_eq!(spans.len(), 1);
            assert_eq!(
                spans[0].caption_links.absence,
                Some(NativeCaptionLinkAbsence::NotObserved)
            );
            assert!(spans[0].caption_links.markers.is_empty());
        }
        other => panic!("{other:?}"),
    }

    let empty = table_spans_from_decoded(
        &["I-<content>".to_string()],
        &rows,
        &[],
        NativeTableCells::Provided { cells: vec![] },
    )
    .expect("a provided-but-empty inventory is a typed absence");
    match empty {
        NativeTableDecodeOutcome::Decoded { tables, .. } => {
            assert_eq!(tables.len(), 1);
            match &tables[0].cells {
                NativeTableCells::NotObserved { reason } => {
                    assert_eq!(*reason, NativeTableAbsence::NoCellInventory)
                }
                other => panic!("an empty inventory became a cell set: {other:?}"),
            }
            assert_eq!(
                tables[0].caption_links.absence,
                Some(NativeCaptionLinkAbsence::NotObserved)
            );
        }
        other => panic!("{other:?}"),
    }

    // The fixture's own absent lane agrees with the polarity above.
    let absent = assemble_family("figure")
        .into_iter()
        .find(|case| case.id == "figure-caption-absent")
        .expect("the reviewed absent-caption document");
    match absent.expected {
        Expected::Figure(figure) => assert_eq!(figure.links, ExpectedLinks::NotObserved),
        other => panic!("{other:?}"),
    }
}

/// A model whose label block is not the declared dictionary is refused, not
/// decoded with a narrower dictionary.
#[test]
fn a_foreign_label_block_is_refused_rather_than_decoded() {
    let rows = fulltext_rows(&["Fig.".to_string()]);
    let mut foreign = load_model(FIGURE_MODEL);
    foreign.labels.pop();
    let error = decode_figure_sequence(&rows, &foreign, &[]).expect_err("a short block is refused");
    assert!(
        matches!(
            error,
            da_parse::NativeFigureSequenceError::ModelLabelCount { .. }
        ),
        "expected a label-count refusal, got {error:?}"
    );

    let mut swapped = load_model(FIGURE_MODEL);
    swapped.labels[0] = "<not-a-figure-label>".to_string();
    let error =
        decode_figure_sequence(&rows, &swapped, &[]).expect_err("a foreign block is refused");
    match error {
        da_parse::NativeFigureSequenceError::ModelLabelMismatch {
            missing,
            unexpected,
        } => {
            assert!(
                missing.contains(&"I-<figure_head>".to_string()),
                "{missing:?}"
            );
            assert_eq!(unexpected, vec!["<not-a-figure-label>".to_string()]);
        }
        other => panic!("expected a vocabulary refusal, got {other:?}"),
    }

    let mut wrong_window = load_model(FIGURE_MODEL);
    wrong_window.patterns[1].source = "u:w24:%x[0,24]".to_string();
    let error =
        decode_figure_sequence(&rows, &wrong_window, &[]).expect_err("a narrow window is refused");
    match error {
        da_parse::NativeFigureSequenceError::DeclaredColumnMismatch { declared, expected } => {
            assert_eq!(declared, Some(24));
            assert_eq!(expected, NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX);
        }
        other => panic!("expected a window refusal, got {other:?}"),
    }

    let mut funding = load_model(FUNDING_MODEL);
    funding.labels[0] = "<other>".to_string();
    let error = decode_funding_sequence(&funding_rows(&["Noted".to_string()]), &funding)
        .expect_err("a duplicated spelling is a foreign block");
    assert!(
        matches!(
            error,
            da_parse::NativeFundingSequenceError::ModelLabelMismatch { .. }
        ),
        "{error:?}"
    );
    assert_eq!(
        NATIVE_FUNDING_LABEL_COLUMN_DUMMY, "0",
        "the inference label slot is the literal zero"
    );
    assert_eq!(NATIVE_FUNDING_LABEL_COLUMN_INDEX, 17);
}

#[test]
fn expected_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_visual_family_same_n.rs");
    let start_marker =
        "// ---- independent expectation builder (no SUT lane below this marker) ----";
    let end_marker = "// ---- end independent expectation builder ----";
    let start = source
        .find(start_marker)
        .expect("the expectation builder start marker must exist");
    let body = &source[start + start_marker.len()..];
    let end = body
        .find(end_marker)
        .expect("the expectation builder end marker must exist");
    let region = &body[..end];
    assert!(
        region.len() > 12_000,
        "the expectation builder must be substantive, got {} bytes",
        region.len()
    );

    let forbidden = [
        "cluster_family_label_runs",
        "decode_figure_sequence",
        "decode_table_sequence",
        "join_native_equations",
        "render_funding_features",
        "decode_funding_sequence",
        "caption_links_from_markers",
        "NATIVE_FIGURE_",
        "NATIVE_FUNDING_",
        "NativeFigureSpan",
        "NativeTableSpan",
        "NativeFundingEntry",
    ];
    for name in forbidden {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }

    // The expectation types are the builder's own, so the SUT shapes cannot leak
    // into the expectation by construction.
    for name in ["FigureSpan", "NativeTableSpan", "NativeFundingEntry"] {
        assert!(
            !region.contains(name),
            "the expectation must not reuse the SUT shape `{name}`"
        );
    }

    // The markers themselves are load-bearing: the region between them is what
    // was scanned above, and the SUT lane is below it.
    let sut_lane = &source[start + start_marker.len() + end + end_marker.len()..];
    assert!(
        sut_lane.contains("decode_figure_sequence") && sut_lane.contains("decode_funding_sequence"),
        "the SUT lane must sit below the end marker"
    );
}

/// The fixture pins, recorded as literals so an edit to any of the four files is
/// a deliberate act.
const ORACLE_SHA256: &str = "6d84f36624604fc6e4d86434d9b527536ef491b6e92df18cc65ea96406102494";
const FIGURE_MODEL_SHA256: &str =
    "9ce129692e003fdc0edb9653d4b1781f7e3b217440ff8f5d574aa93f7ec67095";
const TABLE_MODEL_SHA256: &str = "4406b3fc2ecacc3192808872abee0c511add91eff097c9b80414c699f431f686";
const FUNDING_MODEL_SHA256: &str =
    "94f5a0e9232b09667c671ac2a4fe9b751c412b44763071e69119a40986bddbdf";

#[test]
fn the_model_reader_refuses_a_truncated_or_foreign_synthetic_model() {
    // The synthetic models are fixtures, and a fixture the reader refuses must
    // fail loudly rather than be silently skipped.
    let bytes = read(FIGURE_MODEL);
    let truncated = &bytes[..bytes.len() / 2];
    assert!(CrfModel::parse(truncated).is_err());
    let mut no_labels = bytes.clone();
    let at = no_labels
        .windows(7)
        .position(|window| window == b"#qrk#7\n")
        .expect("the label block header");
    no_labels[at + 5] = b'0';
    assert!(CrfModel::parse(&no_labels).is_err());
}
