//! Native in-text callout-marker classification and body linkage (S04/T06).
//!
//! This is the text-plane half of the slice demo — *"callout links point into
//! the body"*. It answers two questions with one grammar:
//!
//! 1. **Which markers does the printed text name?** Figures, tables, equations,
//!    sections and bibliography entries are named by a keyword plus a label
//!    (`Fig. 3`, `Table 2`, `Eq. (4)`, `Section 2.1`, `Ref. 12`, `[12]`).
//! 2. **Which body element does each marker name?** A marker resolves to
//!    exactly the body element whose *printed* label it names, or it is
//!    explicitly unresolved. No target is ever invented: a marker naming a
//!    label the body does not declare stays visible and unresolved.
//!
//! ## Relationship to the neighbouring planes (kept separate on purpose)
//!
//! * [`crate::native_callout_classification`] classifies the *token* shape of a
//!   marker against Java `CalloutAnalyzer` (`BRACKET_NUMBER`, `ROMAN`, …) and
//!   maps a `[12]` marker through the reference segmenter's `<label>` ordinal
//!   map. It is the format-fidelity plane.
//! * [`crate::native_callout_matching`] is the accountant-bound digest-only
//!   matcher over finalised region receipts.
//! * this module is the **text plane**: it owns the marker grammar (which
//!   keyword names which class, which label shapes are addressable) and the
//!   resolution of a marker into a body element. It does not touch either
//!   sibling, and neither sibling is modified by it.
//!
//! ## Grammar (closed, deterministic, no regex dependency)
//!
//! * **Keyword**: one printed keyword token, per class — `Figure`/`Fig.`/`Figs.`,
//!   `Table`/`Tab.`, `Equation`/`Eq.`, `Section`/`Sec.`/`§`,
//!   `Reference`/`Ref.`, plus the Russian spellings the corpus prints
//!   (`Рис.`, `Табл.`, `Уравнение`, `Раздел`). Longest match wins, and the
//!   keyword must be *followed* by an addressable label: `Figure of speech`
//!   names nothing and is therefore not a marker.
//! * **Label kinds**: arabic (`3`, `2.1`), suffixed (`3b`, `1a`), roman (`IV`,
//!   uppercase only — lowercase `i`/`v` are ordinary words), alpha (`B`),
//!   ranges (`2-4`), and parenthesised/bracketed arabic (`(4)`, `[12]`).
//! * **Attached forms** (`Fig.4`, `§2`, `Table1`) are read from the keyword
//!   token's remainder, but only as arabic-family labels: a possessive such as
//!   `Figure's` must never produce a marker from the trailing `s`.
//! * **Bracketed numerals** (`[12]`) classify as bibliography markers without a
//!   keyword. That is the idiomatic in-text citation shape (`<ref
//!   type="bibr">`); a bare `12` is not a marker.
//! * **Compound enumerations** (`Fig. 1 and Table 2`) are two markers: the scan
//!   never consumes tokens past its own label.
//!
//! ## Resolution (never fabricates a target)
//!
//! [`CalloutBodyInventory`] holds one [`CalloutBodyTarget`] per body element and
//! only the labels that element *declares*: the printed `<label>`/caption text
//! for figures, tables, formulas and sections, and the 1-based bibliography
//! ordinal for citations (the ordinal convention
//! [`crate::native_callout_classification::CalloutLabelOrdinalMap`] already
//! publishes for `[12] → bibl #12`). Each target records its
//! [`CalloutLabelBasis`], so a resolved link states whether it matched printed
//! text or the ordinal convention. A label claimed by two body elements is
//! `Ambiguous`, never first-wins.
//!
//! ## Limits (the only amplification vector is a range)
//!
//! [`classify_callout_markers`] refuses text over [`MAX_CALLOUT_TEXT_BYTES`] and
//! marker sets over [`MAX_CALLOUT_MARKERS`]; a range wider than
//! [`MAX_CALLOUT_RANGE_SPAN`] is *not* expanded and *not* silently narrowed — it
//! stays in the marker set with an empty label list and resolves as
//! [`CalloutLinkUnresolved::RangeTooWide`].
//!
//! ## Determinism
//!
//! No I/O, no clock, no RNG, no environment, no allocation-order dependence:
//! equal input text yields an equal marker vector, and the inventory index is a
//! `BTreeMap`, so equal targets yield equal resolutions. Candidate evidence only:
//! nothing here is spliced into a production route.

use crate::ir::TeiParseIr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

/// Wire identity of this module's outcome types.
pub const CALLOUT_MARKER_SCHEMA_VERSION: &str = "da-parse.callout-markers/v1";

/// Admission bound on the scanned text. Mirrors the sibling planners' byte
/// caps: the scan is linear, but nothing may be handed an unbounded string.
pub const MAX_CALLOUT_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Admission bound on the number of marker occurrences in one text.
pub const MAX_CALLOUT_MARKERS: usize = 100_000;

/// Widest range a `2-4`-shaped marker may expand to. A wider range is not
/// expanded (and not narrowed): it resolves as
/// [`CalloutLinkUnresolved::RangeTooWide`].
pub const MAX_CALLOUT_RANGE_SPAN: u32 = 64;

/// The five body families an in-text marker can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutMarkerClass {
    Figure,
    Table,
    Equation,
    Section,
    Bibliography,
}

impl CalloutMarkerClass {
    /// Every class, in the order resolutions are reported per marker set.
    pub const ALL: [CalloutMarkerClass; 5] = [
        Self::Figure,
        Self::Table,
        Self::Equation,
        Self::Section,
        Self::Bibliography,
    ];

    /// Wire spelling, and the key prefix of every resolved label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Figure => "figure",
            Self::Table => "table",
            Self::Equation => "equation",
            Self::Section => "section",
            Self::Bibliography => "bibliography",
        }
    }
}

/// The printed shape of a marker's label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLabelKind {
    /// `3`, `2.1` — plain arabic, dotted numbering allowed.
    Arabic,
    /// `3b`, `1a` — arabic base plus one sub-element letter.
    Suffixed,
    /// `IV` — uppercase roman numerals only.
    Roman,
    /// `B` — a single letter that is not a roman numeral.
    Alpha,
    /// `2-4` — an arabic range, expanded into one label per element.
    Range,
}

/// Where a target's label comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLabelBasis {
    /// The label is printed by the body element itself (`<label>`, caption,
    /// section number).
    Printed,
    /// The label is the element's 1-based document ordinal — the published
    /// bibliography convention (`[12]` names the 12th entry).
    Ordinal,
}

/// Why a marker did not resolve to a body element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLinkUnresolved {
    /// The body declares no element of this class at all.
    NoTarget,
    /// The class has elements, but none declares this label.
    TargetNotInInventory,
    /// Several elements declare this label; guessing one would be fabrication.
    Ambiguous,
    /// The marker is a range wider than [`MAX_CALLOUT_RANGE_SPAN`], so it was
    /// never expanded.
    RangeTooWide,
}

/// One body element a marker can name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutBodyTarget {
    pub class: CalloutMarkerClass,
    /// The element's own identity (`figure_id`, `table_id`, `formula_id`,
    /// section `path_id`, or `bibliography:{ordinal}`).
    pub target_id: String,
    /// The labels the body declares for this element, canonical, deduplicated,
    /// in printed order. Empty = the element prints no addressable label and is
    /// therefore never matched (never invented).
    pub labels: Vec<String>,
    /// Where [`CalloutBodyTarget::labels`] come from.
    pub basis: CalloutLabelBasis,
    /// 1-based document ordinal within the class.
    pub ordinal: usize,
}

/// Every body element of one document, indexed by `(class, label)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalloutBodyInventory {
    targets: Vec<CalloutBodyTarget>,
    index: BTreeMap<(CalloutMarkerClass, String), Vec<usize>>,
}

impl CalloutBodyInventory {
    /// Index a target list. Labels claimed by several targets are kept as
    /// several candidates — resolution reports them as
    /// [`CalloutLinkUnresolved::Ambiguous`] instead of silently picking one.
    pub fn from_targets(targets: Vec<CalloutBodyTarget>) -> Self {
        let mut index: BTreeMap<(CalloutMarkerClass, String), Vec<usize>> = BTreeMap::new();
        for (position, target) in targets.iter().enumerate() {
            for label in &target.labels {
                index
                    .entry((target.class, label.clone()))
                    .or_default()
                    .push(position);
            }
        }
        Self { targets, index }
    }

    /// Derive the inventory from the lifted IR.
    ///
    /// Only labels the body declares are read: `ExtractedFigure::label`, the
    /// figure/table caption head (`Figure 4 :`, `Table 1 :`), and
    /// `ExtractedFormula::label` / `Section::path_id` are parsed with the same
    /// grammar the scanner uses. Elements that print no label enter the
    /// inventory with an empty label list — they keep the class non-empty (so a
    /// miss reports `TargetNotInInventory`, not `NoTarget`) and are never
    /// matched by a guessed ordinal.
    ///
    /// Citations carry no printed label in the IR, so they use the published
    /// ordinal convention with [`CalloutLabelBasis::Ordinal`].
    pub fn from_ir(ir: &TeiParseIr) -> Self {
        let mut targets = Vec::new();
        for (position, figure) in ir.figures.iter().enumerate() {
            let mut labels = Vec::new();
            if let Some(label) = figure.label.as_deref() {
                labels.extend(pull_printed_labels(label));
            }
            if let Some(caption) = figure.caption.as_deref() {
                labels.extend(pull_printed_labels(caption));
            }
            targets.push(CalloutBodyTarget {
                class: CalloutMarkerClass::Figure,
                target_id: figure.figure_id.clone(),
                labels: dedup(labels),
                basis: CalloutLabelBasis::Printed,
                ordinal: position + 1,
            });
        }
        for (position, table) in ir.tables.iter().enumerate() {
            targets.push(CalloutBodyTarget {
                class: CalloutMarkerClass::Table,
                target_id: table.table_id.clone(),
                labels: dedup(pull_printed_labels(&table.caption)),
                basis: CalloutLabelBasis::Printed,
                ordinal: position + 1,
            });
        }
        for (position, formula) in ir.formulas.iter().enumerate() {
            let labels = formula
                .label
                .as_deref()
                .map(pull_printed_labels)
                .unwrap_or_default();
            targets.push(CalloutBodyTarget {
                class: CalloutMarkerClass::Equation,
                target_id: formula.formula_id.clone(),
                labels: dedup(labels),
                basis: CalloutLabelBasis::Printed,
                ordinal: position + 1,
            });
        }
        for (position, section) in ir.sections.iter().enumerate() {
            let mut labels = Vec::new();
            if let Some(path_id) = section.path_id.as_deref() {
                labels.extend(pull_printed_labels(path_id));
            }
            labels.extend(pull_printed_labels(&section.title));
            targets.push(CalloutBodyTarget {
                class: CalloutMarkerClass::Section,
                target_id: section
                    .path_id
                    .clone()
                    .unwrap_or_else(|| format!("section-{}", position + 1)),
                labels: dedup(labels),
                basis: CalloutLabelBasis::Printed,
                ordinal: position + 1,
            });
        }
        for position in 0..ir.citations.len() {
            targets.push(CalloutBodyTarget {
                class: CalloutMarkerClass::Bibliography,
                target_id: format!("bibliography:{}", position + 1),
                labels: vec![(position + 1).to_string()],
                basis: CalloutLabelBasis::Ordinal,
                ordinal: position + 1,
            });
        }
        Self::from_targets(targets)
    }

    /// Every target, in document order.
    pub fn targets(&self) -> &[CalloutBodyTarget] {
        &self.targets
    }

    /// How many elements of `class` the body declares (labelled or not).
    pub fn target_count(&self, class: CalloutMarkerClass) -> usize {
        self.targets.iter().filter(|t| t.class == class).count()
    }

    /// Every addressable key, sorted — the audit surface for "was this target
    /// ever in the inventory?".
    pub fn keys(&self) -> Vec<String> {
        self.index
            .keys()
            .map(|(class, label)| format!("{}:{}", class.as_str(), label))
            .collect()
    }

    /// Whether any element of `class` declares `label`.
    pub fn contains(&self, class: CalloutMarkerClass, label: &str) -> bool {
        self.index.contains_key(&(class, label.to_string()))
    }

    /// Number of indexed targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether the body declares no element at all.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    fn resolve(&self, class: CalloutMarkerClass, label: &str) -> CalloutLinkTarget {
        match self.index.get(&(class, label.to_string())) {
            Some(candidates) if candidates.len() == 1 => {
                let target = &self.targets[candidates[0]];
                CalloutLinkTarget::Resolved {
                    target_id: target.target_id.clone(),
                    key: format!("{}:{}", class.as_str(), label),
                    basis: target.basis,
                }
            }
            Some(_) => CalloutLinkTarget::Unresolved {
                reason: CalloutLinkUnresolved::Ambiguous,
            },
            None if self.target_count(class) == 0 => CalloutLinkTarget::Unresolved {
                reason: CalloutLinkUnresolved::NoTarget,
            },
            None => CalloutLinkTarget::Unresolved {
                reason: CalloutLinkUnresolved::TargetNotInInventory,
            },
        }
    }
}

/// One classified marker occurrence, in document order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutMarkerOccurrence {
    /// 0-based position; equals this occurrence's index in the returned vector.
    pub index: usize,
    pub class: CalloutMarkerClass,
    /// The printed keyword (`Fig.`, `Table`, `§`, …). `None` for the
    /// keywordless bracketed citation form (`[12]`).
    pub keyword: Option<String>,
    /// The label exactly as printed, without surrounding punctuation
    /// (`2-4`, `(4)`, `3b`, `IV`, `12`).
    pub printed_label: String,
    pub label_kind: CalloutLabelKind,
    /// Canonical labels this marker names, in order. A range names one label
    /// per element in the closed range; empty only when the range exceeded
    /// [`MAX_CALLOUT_RANGE_SPAN`].
    pub labels: Vec<String>,
    /// Byte span of the whole marker (keyword through label) in the scanned
    /// text.
    pub span: Range<usize>,
}

impl CalloutMarkerOccurrence {
    /// Whether this marker names a sub-element (`Fig. 3b`).
    pub const fn is_specific(&self) -> bool {
        matches!(self.label_kind, CalloutLabelKind::Suffixed)
    }

    /// Whether this marker printed a range (`Figs. 2-4`).
    pub const fn is_range(&self) -> bool {
        matches!(self.label_kind, CalloutLabelKind::Range)
    }
}

/// One marker's resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLinkTarget {
    /// Exactly one body element declares this label.
    Resolved {
        target_id: String,
        /// `"<class>:<label>"`.
        key: String,
        basis: CalloutLabelBasis,
    },
    Unresolved {
        reason: CalloutLinkUnresolved,
    },
}

impl CalloutLinkTarget {
    /// Whether this link found a body element.
    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }

    /// The typed reason of an unresolved link.
    pub const fn reason(&self) -> Option<CalloutLinkUnresolved> {
        match self {
            Self::Resolved { .. } => None,
            Self::Unresolved { reason } => Some(*reason),
        }
    }
}

/// One `(marker, named label)` pair and its resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutLink {
    /// Index into [`CalloutLinkReport::markers`].
    pub occurrence_index: usize,
    pub class: CalloutMarkerClass,
    /// The named label (the printed range expression when a range was too wide
    /// to expand).
    pub label: String,
    /// `"<class>:<label>"` — the identity that was looked up.
    pub key: String,
    pub target: CalloutLinkTarget,
}

impl CalloutLink {
    /// Whether this link found a body element.
    pub const fn is_resolved(&self) -> bool {
        self.target.is_resolved()
    }
}

/// Marker classification plus one link per named label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutLinkReport {
    pub schema_version: String,
    pub markers: Vec<CalloutMarkerOccurrence>,
    pub links: Vec<CalloutLink>,
    /// How many links resolved.
    pub resolved: usize,
    /// How many links did not.
    pub unresolved: usize,
}

impl CalloutLinkReport {
    /// Whether every named label resolved and at least one was named.
    pub fn is_complete(&self) -> bool {
        !self.links.is_empty() && self.unresolved == 0
    }

    /// Re-derive the counters and the per-link identity from the stored parts.
    /// A report that does not reproduce its own counts is inadmissible.
    pub fn validate(&self) -> Result<(), CalloutMarkerError> {
        let resolved = self.links.iter().filter(|link| link.is_resolved()).count();
        if resolved != self.resolved {
            return Err(CalloutMarkerError::InconsistentReport("resolved_count"));
        }
        if self.links.len() - resolved != self.unresolved {
            return Err(CalloutMarkerError::InconsistentReport("unresolved_count"));
        }
        let expected: usize = self.markers.iter().map(|m| m.labels.len().max(1)).sum();
        if expected != self.links.len() {
            return Err(CalloutMarkerError::InconsistentReport("link_count"));
        }
        for (position, marker) in self.markers.iter().enumerate() {
            if marker.index != position {
                return Err(CalloutMarkerError::InconsistentReport("marker_index"));
            }
        }
        for link in &self.links {
            let Some(marker) = self.markers.get(link.occurrence_index) else {
                return Err(CalloutMarkerError::InconsistentReport("occurrence_index"));
            };
            if marker.class != link.class {
                return Err(CalloutMarkerError::InconsistentReport("link_class"));
            }
            if link.key != format!("{}:{}", link.class.as_str(), link.label) {
                return Err(CalloutMarkerError::InconsistentReport("link_key"));
            }
        }
        Ok(())
    }
}

/// Typed refusal vocabulary for scanning and reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalloutMarkerError {
    /// The scanned text exceeded [`MAX_CALLOUT_TEXT_BYTES`].
    TextTooLong { bytes: usize, max: usize },
    /// The text named more markers than [`MAX_CALLOUT_MARKERS`].
    TooManyMarkers { count: usize, max: usize },
    /// A report did not reproduce its own identity (see
    /// [`CalloutLinkReport::validate`]).
    InconsistentReport(&'static str),
}

impl fmt::Display for CalloutMarkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextTooLong { bytes, max } => {
                write!(f, "callout scan refused {bytes} bytes (max {max})")
            }
            Self::TooManyMarkers { count, max } => {
                write!(f, "callout scan found {count} markers (max {max})")
            }
            Self::InconsistentReport(what) => {
                write!(f, "callout link report is inconsistent: {what}")
            }
        }
    }
}

impl std::error::Error for CalloutMarkerError {}

// ───────────────────────────── grammar ─────────────────────────────

/// Printed keywords, lowercased. Longest match wins, so the order here carries
/// no meaning; the boundary rule (below) keeps `figures` from matching `fig`.
const KEYWORDS: &[(&str, CalloutMarkerClass)] = &[
    ("figures", CalloutMarkerClass::Figure),
    ("figure", CalloutMarkerClass::Figure),
    ("figs", CalloutMarkerClass::Figure),
    ("fig", CalloutMarkerClass::Figure),
    ("рисунок", CalloutMarkerClass::Figure),
    ("рисунка", CalloutMarkerClass::Figure),
    ("рис", CalloutMarkerClass::Figure),
    ("tables", CalloutMarkerClass::Table),
    ("table", CalloutMarkerClass::Table),
    ("tabs", CalloutMarkerClass::Table),
    ("tab", CalloutMarkerClass::Table),
    ("таблицы", CalloutMarkerClass::Table),
    ("таблица", CalloutMarkerClass::Table),
    ("табл", CalloutMarkerClass::Table),
    ("equations", CalloutMarkerClass::Equation),
    ("equation", CalloutMarkerClass::Equation),
    ("eqs", CalloutMarkerClass::Equation),
    ("eq", CalloutMarkerClass::Equation),
    ("уравнения", CalloutMarkerClass::Equation),
    ("уравнение", CalloutMarkerClass::Equation),
    ("sections", CalloutMarkerClass::Section),
    ("section", CalloutMarkerClass::Section),
    ("secs", CalloutMarkerClass::Section),
    ("sec", CalloutMarkerClass::Section),
    ("раздела", CalloutMarkerClass::Section),
    ("раздел", CalloutMarkerClass::Section),
    ("§", CalloutMarkerClass::Section),
    ("references", CalloutMarkerClass::Bibliography),
    ("reference", CalloutMarkerClass::Bibliography),
    ("refs", CalloutMarkerClass::Bibliography),
    ("ref", CalloutMarkerClass::Bibliography),
    ("citations", CalloutMarkerClass::Bibliography),
    ("citation", CalloutMarkerClass::Bibliography),
];

/// Separators a label may print immediately after its keyword (`Fig. 4`,
/// `§2`, `Eq. (4)`).
const LEADING_SEPARATORS: [char; 7] = ['.', ':', ';', ',', '(', '[', '\u{00a7}'];

/// One whitespace-separated token with its byte span in the scanned text.
struct Token<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn split_tokens(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (offset, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(begin) = start.take() {
                tokens.push(Token {
                    text: &text[begin..offset],
                    start: begin,
                    end: offset,
                });
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }
    if let Some(begin) = start {
        tokens.push(Token {
            text: &text[begin..],
            start: begin,
            end: text.len(),
        });
    }
    tokens
}

/// The longest keyword `token` starts with, together with the token remainder
/// after it. `None` when no keyword matches or the match is inside a word.
fn match_keyword(token: &str) -> Option<(CalloutMarkerClass, String, &str)> {
    let lowered = token.to_lowercase();
    let mut best: Option<(usize, CalloutMarkerClass, String, &str)> = None;
    for (keyword, class) in KEYWORDS {
        let Some(remainder) = lowered.strip_prefix(keyword) else {
            continue;
        };
        // The keyword must end at a *letter* edge: a following letter breaks it
        // (`figures` must not be read as `fig` + `ures`, `Tablespoons` must not
        // be read as `table` + `spoons`), while a following digit or separator
        // is the attached form (`Table1`, `Fig.4`, `§2`). A keyword that ends
        // non-alphabetically (`§`) is a boundary on its own.
        let boundary_ok = remainder.is_empty()
            || !remainder.chars().next().is_some_and(char::is_alphabetic)
            || !keyword.chars().last().is_some_and(char::is_alphabetic);
        if !boundary_ok {
            continue;
        }
        // `to_lowercase` preserves byte length for every keyword above, so the
        // remainder can be sliced out of the original token; when it cannot
        // (exotic case folding), this keyword is skipped rather than mis-sliced.
        let Some(original_remainder) = token.get(keyword.len()..) else {
            continue;
        };
        let candidate = (
            keyword.chars().count(),
            *class,
            (*keyword).to_string(),
            original_remainder,
        );
        if best.as_ref().is_none_or(|current| candidate.0 > current.0) {
            best = Some(candidate);
        }
    }
    best.map(|(_, class, keyword, remainder)| (class, keyword, remainder))
}

/// Canonical form of one printed label token.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedLabel {
    kind: CalloutLabelKind,
    printed: String,
    labels: Vec<String>,
}

fn is_roman(text: &str) -> bool {
    !text.is_empty() && text.len() <= 7 && text.chars().all(|c| "IVXLCDM".contains(c))
}

fn is_arabic_base(text: &str) -> bool {
    !text.is_empty()
        && text
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// Parse one printed label token (`3`, `2.1`, `3b`, `IV`, `B`, `2-4`, `(4)`,
/// `[12]`). `None` means "not an addressable label", which is what keeps noise
/// such as `Figure of speech` out of the marker set.
fn parse_printed_label(raw: &str) -> Option<ParsedLabel> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Bracketed / parenthesised arabic (and ranges inside them).
    let (inner, bracketed) = match trimmed
        .strip_prefix(['(', '['])
        .and_then(|rest| rest.strip_suffix([')', ']']))
    {
        Some(inner) => (inner, true),
        None => (trimmed, false),
    };
    if let Some(parsed) = parse_arabic_family(inner) {
        return Some(parsed);
    }
    if bracketed {
        return None;
    }
    if is_roman(trimmed) {
        return Some(ParsedLabel {
            kind: CalloutLabelKind::Roman,
            printed: trimmed.to_string(),
            labels: vec![trimmed.to_string()],
        });
    }
    if trimmed.chars().count() == 1
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        let letter = trimmed.to_ascii_uppercase();
        return Some(ParsedLabel {
            kind: CalloutLabelKind::Alpha,
            printed: trimmed.to_string(),
            labels: vec![letter],
        });
    }
    None
}

/// Arabic base, dotted arabic, arabic with suffix, and arabic ranges.
fn parse_arabic_family(text: &str) -> Option<ParsedLabel> {
    if text.is_empty() {
        return None;
    }
    if let Some((start, end)) = text.split_once(['-', '\u{2013}']) {
        let (Some(start_parsed), Some(end_parsed)) = (plain_arabic(start), plain_arabic(end))
        else {
            return None;
        };
        let first = start_parsed.labels[0].parse::<u32>().ok()?;
        let last = end_parsed.labels[0].parse::<u32>().ok()?;
        let mut labels = Vec::new();
        // Only a sane, bounded, ascending range is expanded. A wider range is
        // kept visible with no labels so it resolves as `RangeTooWide` rather
        // than being silently narrowed to its first element.
        if last >= first && last - first < MAX_CALLOUT_RANGE_SPAN {
            for value in first..=last {
                labels.push(value.to_string());
            }
        }
        return Some(ParsedLabel {
            kind: CalloutLabelKind::Range,
            printed: text.to_string(),
            labels,
        });
    }
    if let Some(base) = plain_arabic(text) {
        return Some(base);
    }
    // Suffixed: `<arabic base><one letter>` (`3b`, `1a`, `2.1c`). The suffix
    // is taken char-wise, never by byte offset: a multi-byte trailing character
    // must be a clean refusal, not a slice panic.
    let suffix_char = text.chars().next_back()?;
    if !suffix_char.is_ascii_alphabetic() {
        return None;
    }
    let base = &text[..text.len() - suffix_char.len_utf8()];
    if !is_arabic_base(base) {
        return None;
    }
    let canonical = format!("{}{}", base, suffix_char.to_ascii_lowercase());
    Some(ParsedLabel {
        kind: CalloutLabelKind::Suffixed,
        printed: text.to_string(),
        labels: vec![canonical],
    })
}

/// Plain arabic or dotted arabic (`3`, `2.1`).
fn plain_arabic(text: &str) -> Option<ParsedLabel> {
    if !is_arabic_base(text) {
        return None;
    }
    Some(ParsedLabel {
        kind: CalloutLabelKind::Arabic,
        printed: text.to_string(),
        labels: vec![text.to_string()],
    })
}

/// Drop separator characters a label prints against its keyword or sentence
/// punctuation (`Fig. 4.` → `4`, `Eq. (4)` → `(4)`).
fn trim_label_edges(raw: &str) -> &str {
    raw.trim_matches(|c: char| LEADING_SEPARATORS.contains(&c) || c == ')' || c == ']' || c == '?')
}

/// Byte offset just past the printed label of `token`.
///
/// Sentence punctuation that merely follows the label is *not* part of the
/// marker (`Table 2.` ends after the `2`), while a bracketed label keeps its own
/// closer (`Eq. (4)` ends at the `)`, which is not sentence punctuation).
fn label_end(token: &Token<'_>) -> usize {
    let printed = token
        .text
        .trim_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | '?'));
    token.start + printed.len()
}

/// Every label a printed string declares, canonical and deduplicated.
///
/// Used to read the body's own labels: a caption head (`Figure 4 :`) is parsed
/// with the marker grammar, and a bare label field (`4`, `(1)`, `2.1`) with the
/// label grammar. A string that declares neither contributes nothing — never a
/// guessed ordinal.
fn pull_printed_labels(text: &str) -> Vec<String> {
    let mut labels = Vec::new();
    if let Ok(markers) = classify_callout_markers(text) {
        for marker in markers {
            labels.extend(marker.labels);
        }
    }
    for token in split_tokens(text) {
        if let Some(parsed) = parse_printed_label(trim_label_edges(token.text)) {
            labels.extend(parsed.labels);
        }
    }
    dedup(labels)
}

fn dedup(labels: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for label in labels {
        if seen.insert(label.clone()) {
            out.push(label);
        }
    }
    out
}

// ───────────────────────────── entry points ─────────────────────────────

/// Classify every in-text callout marker in `text`, in document order.
///
/// A keyword only becomes a marker when it is followed by an addressable label,
/// so prose *about* markers (`Figure of speech`, `the table of contents`) emits
/// nothing. Bracketed numerals (`[12]`) are bibliography markers on their own.
pub fn classify_callout_markers(
    text: &str,
) -> Result<Vec<CalloutMarkerOccurrence>, CalloutMarkerError> {
    if text.len() > MAX_CALLOUT_TEXT_BYTES {
        return Err(CalloutMarkerError::TextTooLong {
            bytes: text.len(),
            max: MAX_CALLOUT_TEXT_BYTES,
        });
    }
    let tokens = split_tokens(text);
    let mut markers: Vec<CalloutMarkerOccurrence> = Vec::new();
    let mut position = 0;
    while position < tokens.len() {
        let token = &tokens[position];
        // 1) keyword form: `Fig. 3`, `Table 2`, `Eq. (4)`, `Figs. 2-4`, `§2`.
        if let Some((class, keyword, remainder)) = match_keyword(token.text) {
            let remainder_trimmed =
                remainder.trim_start_matches(|c: char| LEADING_SEPARATORS.contains(&c));
            // The keyword must be followed by a label: either attached to the
            // same token, or the immediately following token.
            let (parsed, span_end) = if remainder_trimmed.is_empty() {
                match tokens.get(position + 1) {
                    Some(next) => {
                        let parsed = parse_printed_label(trim_label_edges(next.text));
                        // Alpha/roman labels are only read as standalone tokens,
                        // never glued to the keyword.
                        (parsed, label_end(next))
                    }
                    None => (None, token.end),
                }
            } else {
                let parsed =
                    parse_printed_label(trim_label_edges(remainder_trimmed)).filter(|parsed| {
                        !matches!(
                            parsed.kind,
                            CalloutLabelKind::Roman | CalloutLabelKind::Alpha
                        )
                    });
                (parsed, label_end(token))
            };
            if let Some(parsed) = parsed {
                let consumed_next = remainder_trimmed.is_empty();
                markers.push(CalloutMarkerOccurrence {
                    index: markers.len(),
                    class,
                    keyword: Some(keyword),
                    printed_label: parsed.printed,
                    label_kind: parsed.kind,
                    labels: parsed.labels,
                    span: token.start..span_end,
                });
                position += if consumed_next { 2 } else { 1 };
                continue;
            }
        }
        // 2) keywordless bracketed citation: `[12]`, `[12-14]`.
        if token.text.starts_with('[')
            && token.text.ends_with(']')
            && let Some(parsed) = parse_printed_label(token.text)
        {
            markers.push(CalloutMarkerOccurrence {
                index: markers.len(),
                class: CalloutMarkerClass::Bibliography,
                keyword: None,
                printed_label: parsed.printed,
                label_kind: parsed.kind,
                labels: parsed.labels,
                span: token.start..token.end,
            });
        }
        position += 1;
    }
    if markers.len() > MAX_CALLOUT_MARKERS {
        return Err(CalloutMarkerError::TooManyMarkers {
            count: markers.len(),
            max: MAX_CALLOUT_MARKERS,
        });
    }
    Ok(markers)
}

/// Resolve every named label against the body inventory.
///
/// One link per named label, in marker order. An unexpanded over-wide range
/// yields exactly one link carrying [`CalloutLinkUnresolved::RangeTooWide`], so
/// the marker is never lost.
pub fn link_callout_markers(
    markers: &[CalloutMarkerOccurrence],
    inventory: &CalloutBodyInventory,
) -> Vec<CalloutLink> {
    let mut links = Vec::new();
    for marker in markers {
        if marker.labels.is_empty() {
            let key = format!("{}:{}", marker.class.as_str(), marker.printed_label);
            links.push(CalloutLink {
                occurrence_index: marker.index,
                class: marker.class,
                label: marker.printed_label.clone(),
                key,
                target: CalloutLinkTarget::Unresolved {
                    reason: CalloutLinkUnresolved::RangeTooWide,
                },
            });
            continue;
        }
        for label in &marker.labels {
            links.push(CalloutLink {
                occurrence_index: marker.index,
                class: marker.class,
                label: label.clone(),
                key: format!("{}:{}", marker.class.as_str(), label),
                target: inventory.resolve(marker.class, label),
            });
        }
    }
    links
}

/// Classify the IR's body text and resolve every marker against the IR's own
/// elements.
///
/// The scanned text is [`TeiParseIr::body_text`] — the stripped `<body>` region,
/// so a figure's own caption head is a *mention* of that figure, exactly like a
/// prose mention. Marker intent is not claimed here; link truth is.
pub fn classify_and_link_callout_markers(
    ir: &TeiParseIr,
) -> Result<CalloutLinkReport, CalloutMarkerError> {
    let markers = classify_callout_markers(&ir.body_text)?;
    let inventory = CalloutBodyInventory::from_ir(ir);
    let links = link_callout_markers(&markers, &inventory);
    let resolved = links.iter().filter(|link| link.is_resolved()).count();
    let report = CalloutLinkReport {
        schema_version: CALLOUT_MARKER_SCHEMA_VERSION.to_string(),
        resolved,
        unresolved: links.len() - resolved,
        markers,
        links,
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn figure(label: &str, id: &str) -> CalloutBodyTarget {
        CalloutBodyTarget {
            class: CalloutMarkerClass::Figure,
            target_id: id.to_string(),
            labels: vec![label.to_string()],
            basis: CalloutLabelBasis::Printed,
            ordinal: 1,
        }
    }

    fn inventory() -> CalloutBodyInventory {
        CalloutBodyInventory::from_targets(vec![
            figure("1", "fig_0"),
            figure("2", "fig_1"),
            figure("3", "fig_2"),
            CalloutBodyTarget {
                class: CalloutMarkerClass::Table,
                target_id: "tab_0".to_string(),
                labels: vec!["2".to_string()],
                basis: CalloutLabelBasis::Printed,
                ordinal: 1,
            },
        ])
    }

    fn classes_and_labels(text: &str) -> Vec<(CalloutMarkerClass, Vec<String>)> {
        classify_callout_markers(text)
            .expect("scan")
            .into_iter()
            .map(|marker| (marker.class, marker.labels))
            .collect()
    }

    #[test]
    fn callout_keywords_classify_every_body_family() {
        assert_eq!(
            classes_and_labels("Fig. 3 and Figure 12 and FIGS. 2"),
            vec![
                (CalloutMarkerClass::Figure, vec!["3".to_string()]),
                (CalloutMarkerClass::Figure, vec!["12".to_string()]),
                (CalloutMarkerClass::Figure, vec!["2".to_string()]),
            ]
        );
        assert_eq!(
            classes_and_labels("Table 3, Табл. 4, Eq. (4), Section 2.1, Ref. 12"),
            vec![
                (CalloutMarkerClass::Table, vec!["3".to_string()]),
                (CalloutMarkerClass::Table, vec!["4".to_string()]),
                (CalloutMarkerClass::Equation, vec!["4".to_string()]),
                (CalloutMarkerClass::Section, vec!["2.1".to_string()]),
                (CalloutMarkerClass::Bibliography, vec!["12".to_string()]),
            ]
        );
        assert_eq!(
            classes_and_labels("Рис. 5 и Табл. 6 и Уравнение 7"),
            vec![
                (CalloutMarkerClass::Figure, vec!["5".to_string()]),
                (CalloutMarkerClass::Table, vec!["6".to_string()]),
                (CalloutMarkerClass::Equation, vec!["7".to_string()]),
            ]
        );
    }

    #[test]
    fn callout_label_kinds_cover_arabic_suffixed_roman_alpha_and_ranges() {
        let kinds: Vec<(CalloutLabelKind, Vec<String>)> =
            classify_callout_markers("Fig. 2.1, Fig. 3b, Fig. IV, Fig. B, Figs. 2-4")
                .expect("scan")
                .into_iter()
                .map(|marker| (marker.label_kind, marker.labels))
                .collect();
        assert_eq!(
            kinds,
            vec![
                (CalloutLabelKind::Arabic, vec!["2.1".to_string()]),
                (CalloutLabelKind::Suffixed, vec!["3b".to_string()]),
                (CalloutLabelKind::Roman, vec!["IV".to_string()]),
                (CalloutLabelKind::Alpha, vec!["B".to_string()]),
                (
                    CalloutLabelKind::Range,
                    vec!["2".to_string(), "3".to_string(), "4".to_string()]
                ),
            ]
        );
        let specific = classify_callout_markers("Fig. 3b").expect("scan");
        assert!(specific[0].is_specific());
        assert!(!specific[0].is_range());
        let range = classify_callout_markers("Figs. 2-4").expect("scan");
        assert!(range[0].is_range());
    }

    #[test]
    fn callout_attached_and_compound_forms_are_read_without_overreach() {
        // Attached forms are arabic-family only.
        assert_eq!(
            classes_and_labels("Fig.4 and §2 and Figure1"),
            vec![
                (CalloutMarkerClass::Figure, vec!["4".to_string()]),
                (CalloutMarkerClass::Section, vec!["2".to_string()]),
                (CalloutMarkerClass::Figure, vec!["1".to_string()]),
            ]
        );
        // A possessive must not leak its trailing letter as an alpha label.
        assert!(
            classify_callout_markers("Figure's caption")
                .expect("scan")
                .is_empty()
        );
        // Compound enumerations are one marker per keyword, in document order.
        let markers = classify_callout_markers("Fig. 1 and Table 2").expect("scan");
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].class, CalloutMarkerClass::Figure);
        assert_eq!(markers[1].class, CalloutMarkerClass::Table);
        assert!(markers[0].span.end < markers[1].span.start);
    }

    #[test]
    fn callout_bracketed_numeral_is_a_bibliography_marker() {
        let markers = classify_callout_markers("see [12] and [14-15] here").expect("scan");
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].class, CalloutMarkerClass::Bibliography);
        assert_eq!(markers[0].keyword, None);
        assert_eq!(markers[0].labels, vec!["12".to_string()]);
        assert_eq!(markers[1].labels, vec!["14".to_string(), "15".to_string()]);
        // A bare numeral is not a marker.
        assert!(
            classify_callout_markers("see 12 here")
                .expect("scan")
                .is_empty()
        );
    }

    #[test]
    fn callout_noise_without_an_addressable_label_is_not_a_marker() {
        for noise in [
            "Figure of speech",
            "the table of contents",
            "Section headed here",
            "Figs are great",
            "equation free prose",
            "In Figure the axis is blue",
        ] {
            assert!(
                classify_callout_markers(noise).expect("scan").is_empty(),
                "noise classified as a marker: {noise}"
            );
        }
        let markers = classify_callout_markers("Figure 1 shows Table 2").expect("scan");
        assert_eq!(markers.len(), 2);
    }

    #[test]
    fn callout_spans_cover_the_printed_marker() {
        let text = "see Fig. 12-14 and Table 2.";
        let markers = classify_callout_markers(text).expect("scan");
        assert_eq!(&text[markers[0].span.clone()], "Fig. 12-14");
        assert_eq!(&text[markers[1].span.clone()], "Table 2");
        assert_eq!(markers[0].printed_label, "12-14");
    }

    #[test]
    fn callout_unresolved_is_explicit_and_never_fabricated() {
        let inventory = inventory();
        let markers =
            classify_callout_markers("Fig. 1 and Fig. 9 and Table 2 and Eq. (1)").expect("scan");
        let links = link_callout_markers(&markers, &inventory);
        assert_eq!(links.len(), 4);
        assert_eq!(
            links[0].target,
            CalloutLinkTarget::Resolved {
                target_id: "fig_0".to_string(),
                key: "figure:1".to_string(),
                basis: CalloutLabelBasis::Printed,
            }
        );
        assert_eq!(
            links[1].target.reason(),
            Some(CalloutLinkUnresolved::TargetNotInInventory)
        );
        assert_eq!(
            links[2].target,
            CalloutLinkTarget::Resolved {
                target_id: "tab_0".to_string(),
                key: "table:2".to_string(),
                basis: CalloutLabelBasis::Printed,
            }
        );
        // The body declares no equation at all: a different, typed reason.
        assert_eq!(
            links[3].target.reason(),
            Some(CalloutLinkUnresolved::NoTarget)
        );
        assert!(links[3].key == "equation:1");
    }

    #[test]
    fn callout_label_claimed_twice_is_ambiguous_not_first_wins() {
        let inventory =
            CalloutBodyInventory::from_targets(vec![figure("4", "fig_0"), figure("4", "fig_1")]);
        let markers = classify_callout_markers("Fig. 4").expect("scan");
        let links = link_callout_markers(&markers, &inventory);
        assert_eq!(
            links[0].target.reason(),
            Some(CalloutLinkUnresolved::Ambiguous)
        );
        // Positive control: a unique label still resolves.
        let unique = CalloutBodyInventory::from_targets(vec![figure("4", "fig_0")]);
        let resolved = link_callout_markers(&markers, &unique);
        assert!(resolved[0].is_resolved());
    }

    #[test]
    fn callout_range_beyond_cap_is_visible_and_unresolved() {
        let markers = classify_callout_markers("Figs. 1-99999").expect("scan");
        assert_eq!(markers.len(), 1);
        assert!(markers[0].labels.is_empty());
        assert_eq!(markers[0].label_kind, CalloutLabelKind::Range);
        let links = link_callout_markers(&markers, &inventory());
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].target.reason(),
            Some(CalloutLinkUnresolved::RangeTooWide)
        );
        assert_eq!(links[0].key, "figure:1-99999");
    }

    #[test]
    fn callout_links_are_deterministic_and_reproduce_their_report() {
        let inventory = inventory();
        let markers = classify_callout_markers("Fig. 2, Table 2, Fig. 9").expect("scan");
        let first = link_callout_markers(&markers, &inventory);
        let second = link_callout_markers(&markers, &inventory);
        assert_eq!(first, second);
        let resolved = first.iter().filter(|link| link.is_resolved()).count();
        let report = CalloutLinkReport {
            schema_version: CALLOUT_MARKER_SCHEMA_VERSION.to_string(),
            unresolved: first.len() - resolved,
            resolved,
            markers,
            links: first,
        };
        report.validate().expect("a self-consistent report");
        assert!(!report.is_complete());
        assert_eq!(report.resolved, 2);
        assert_eq!(report.unresolved, 1);
    }

    #[test]
    fn callout_inconsistent_report_is_refused() {
        let inventory = inventory();
        let markers = classify_callout_markers("Fig. 1").expect("scan");
        let links = link_callout_markers(&markers, &inventory);
        let report = CalloutLinkReport {
            schema_version: CALLOUT_MARKER_SCHEMA_VERSION.to_string(),
            markers,
            links,
            resolved: 0,
            unresolved: 1,
        };
        assert_eq!(
            report.validate(),
            Err(CalloutMarkerError::InconsistentReport("resolved_count"))
        );
    }

    #[test]
    fn callout_oversized_text_and_marker_flood_are_refused() {
        let oversized = "a".repeat(MAX_CALLOUT_TEXT_BYTES + 1);
        assert_eq!(
            classify_callout_markers(&oversized),
            Err(CalloutMarkerError::TextTooLong {
                bytes: MAX_CALLOUT_TEXT_BYTES + 1,
                max: MAX_CALLOUT_TEXT_BYTES,
            })
        );
        let flood = "Fig. 1 ".repeat(MAX_CALLOUT_MARKERS + 1);
        assert!(matches!(
            classify_callout_markers(&flood),
            Err(CalloutMarkerError::TooManyMarkers { .. })
        ));
    }

    #[test]
    fn callout_inventory_from_targets_indexes_every_declared_label() {
        let inventory = inventory();
        assert_eq!(inventory.len(), 4);
        assert!(!inventory.is_empty());
        assert_eq!(inventory.target_count(CalloutMarkerClass::Figure), 3);
        assert_eq!(inventory.target_count(CalloutMarkerClass::Equation), 0);
        assert!(inventory.contains(CalloutMarkerClass::Table, "2"));
        assert!(!inventory.contains(CalloutMarkerClass::Table, "3"));
        assert_eq!(
            inventory.keys(),
            vec![
                "figure:1".to_string(),
                "figure:2".to_string(),
                "figure:3".to_string(),
                "table:2".to_string(),
            ]
        );
        // An unlabelled element keeps the class non-empty but is unmatched.
        let unlabelled = CalloutBodyInventory::from_targets(vec![CalloutBodyTarget {
            class: CalloutMarkerClass::Figure,
            target_id: "fig_0".to_string(),
            labels: Vec::new(),
            basis: CalloutLabelBasis::Printed,
            ordinal: 1,
        }]);
        let markers = classify_callout_markers("Fig. 7").expect("scan");
        let links = link_callout_markers(&markers, &unlabelled);
        assert_eq!(
            links[0].target.reason(),
            Some(CalloutLinkUnresolved::TargetNotInInventory)
        );
        assert!(CalloutBodyInventory::default().is_empty());
    }
}
