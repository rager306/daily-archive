//! Native callout-marker classification and body linkage (S04/T06).
//!
//! This is the second half of the slice demo — "callout links point into the
//! body". It is the *layout-token* plane counterpart of
//! [`crate::native_callout_matching`], and the two are deliberately separate:
//!
//! * `native_callout_matching` is an **accountant-bound, digest-only matcher**
//!   over `NativeFullRegionAccounting` / `NativeFulltextBlocks` receipts. It
//!   never sees token text, never classifies a marker, and is **not touched by
//!   this module** (it has its own contract, data-contract, property and oracle
//!   suites).
//! * this module reads typed callout tokens (text + superscript flag + document
//!   position), classifies each marker, picks the document's majority callout
//!   style, maps a marker through the reference-segmenter's `<label>` ordinal
//!   map, and counts **every** occurrence into two separate, checksum-bound
//!   denominators.
//!
//! ## Format authority (read, never linked, never vendored)
//!
//! The classifier reproduces `CalloutAnalyzer.getCalloutType(List<LayoutToken>)`
//! from `grobid-core/src/main/java/org/grobid/core/engines/citations/CalloutAnalyzer.java`
//! (109 lines, Apache-2.0). Its exact Java semantics, all of which are load
//! bearing, are:
//!
//! 1. `LayoutTokensUtil.toText(callout)` concatenates the token texts with **no
//!    separator** (`Joiner.on("").join(Token::t)`).
//! 2. `null` or `calloutString.trim().length() == 0` → `UNKNOWN`. Java's
//!    `String.trim()` removes characters `<= U+0020` (so a non-breaking space is
//!    *not* blank), and it is applied before step 3.
//! 3. `calloutString.replace(" ", "")` removes **ASCII spaces only** — not tabs,
//!    not newlines. Every pattern below matches the space-stripped string with
//!    `Matcher.find()`, i.e. an unanchored substring search.
//! 4. The superscript flag is `true` iff every non-blank token
//!    (`getText().trim().length() != 0`) is a superscript token; blank tokens are
//!    skipped and never clear the flag.
//! 5. The checks run in a fixed order and the first hit wins:
//!    `NUMBER` **only if superscript** → `BRACKET_NUMBER` → `PARENTHESIS_NUMBER`
//!    → `BRACKET_TEXT` → `PARENTHESIS_TEXT` → `NUMBER` → `ROMAN`.
//!    Note the first check is unanchored, so a *superscript* `[12]` is
//!    `SUPERSCRIPT_NUMBER`, not `BRACKET_NUMBER`.
//! 6. `ROMAN_PATTERN = (IX|IV|V?I{0,3})`: the third alternative is entirely
//!    optional and therefore matches the empty string, so `find()` succeeds at
//!    position 0 of **any** non-empty string. The Roman check is a catch-all and
//!    `UNKNOWN` is reachable only for a blank marker. That is reproduced
//!    faithfully (see [`find_roman_marker`]) rather than "fixed": the type is an
//!    input to majority-style voting, not a final verdict.
//!
//! ## Why this module owns a token DTO
//!
//! The layout plane has no superscript flag. `LayoutToken`/`LayoutStyleHints`
//! (`crate::layout`) and `LatticeToken` (`crate::layout_lattice`) carry
//! `text_digest`, `bold`, `italic`, `monospace` and `font_size_milli_pt` only, so
//! the flag the Java classifier reads cannot be recovered from them. The
//! provider hands the flag over in [`CalloutToken`]; this module does not infer
//! it and does not depend on layout geometry.
//!
//! ## Denominators are separate, checksum-bound and never pooled
//!
//! MEM992: the Java lifter *skips targetless bibr refs*, so it cannot oracle
//! fragmented callouts. The frozen rich-TEI corpus therefore carries
//! [`MEM992_FROZEN_TOTAL_CALLOUTS`] occurrences
//! ([`MEM992_FROZEN_TARGETED_CALLOUTS`] targeted +
//! [`MEM992_FROZEN_TARGETLESS_CALLOUTS`] targetless) and a native inventory must
//! keep them as **two** denominators. [`CalloutDenominator`] carries its own
//! count *and* a checksum over the ordered digests of exactly its own members,
//! computed in a per-label digest domain, so:
//!
//! * eroding an occurrence changes the checksum of its denominator;
//! * pooling the two denominators cannot reproduce either checksum (different
//!   domain tags), enforced structurally by [`CalloutInventory::validate`];
//! * these counters are **never** merged with `figure_n` (P6) or `citation_n`
//!   (MEM1061 / MEM986) — [`NATIVE_CALLOUT_INVENTORY_LANE`] names the lane they
//!   belong to and is part of every denominator checksum.
//!
//! ## Properties
//!
//! No I/O, no clock, no environment, no RNG, no model: every function is a pure
//! function of its arguments, and every collection is ordered, so equal inputs
//! always produce equal output. Nothing here is spliced into a production route:
//! `deployed = false`, candidate evidence only.

use crate::{
    AdjacencyDisposition, CalloutPosition, CalloutTargetClass, CalloutUnresolvedReason,
    MajorityStyleOutcome, ObservedCalloutStyle, ReferenceSegmentKind, ReferenceSegmentSequence,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

/// Wire identity of this module's outcome types.
pub const NATIVE_CALLOUT_CLASSIFICATION_SCHEMA_VERSION: &str =
    "da-parse.native-callout-classification/v1";

/// Admission bound on the number of callout markers in one inventory.
pub const MAX_NATIVE_CALLOUT_MARKERS: usize = 100_000;

/// Admission bound on the number of tokens one marker may carry.
pub const MAX_NATIVE_CALLOUT_MARKER_TOKENS: usize = 1_024;

/// Admission bound on the number of decoded reference lines fed to the map.
pub const MAX_NATIVE_CALLOUT_REFERENCE_LINES: usize = 100_000;

/// The lane this inventory counts. Part of every denominator checksum: a
/// callout denominator can never be read as a `figure_n` or `citation_n`
/// denominator, and never pooled with one.
pub const NATIVE_CALLOUT_INVENTORY_LANE: &str = "callout-classification/in-body-markers";

/// Label of the targeted denominator (markers that resolved to a target).
pub const NATIVE_CALLOUT_TARGETED_DENOMINATOR: &str = "targeted";

/// Label of the targetless denominator (markers that did not resolve).
pub const NATIVE_CALLOUT_TARGETLESS_DENOMINATOR: &str = "targetless";

/// Digest domain of one occurrence. Distinguishes an occurrence digest from any
/// other digest in the crate.
pub const CALLOUT_OBSERVATION_DIGEST_DOMAIN: &str =
    "da-parse.native-callout-classification/observation/v1";

/// Digest domain of one denominator. The label is appended, so targeted and
/// targetless checksums live in disjoint domains by construction.
pub const CALLOUT_DENOMINATOR_DIGEST_DOMAIN: &str =
    "da-parse.native-callout-classification/denominator/v1";

/// MEM992: targeted callouts on the frozen rich-TEI corpus. Declared, not
/// measured here — the point is that it is a *separate* denominator.
pub const MEM992_FROZEN_TARGETED_CALLOUTS: usize = 217;

/// MEM992: targetless callouts on the frozen rich-TEI corpus. The Java lifter
/// skipped these, which is exactly why a native inventory must count them.
pub const MEM992_FROZEN_TARGETLESS_CALLOUTS: usize = 163;

/// MEM992: every callout occurrence on the frozen rich-TEI corpus. Equal to the
/// sum of the two denominators above; never a pooled denominator of its own.
pub const MEM992_FROZEN_TOTAL_CALLOUTS: usize = 380;

/// The eight marker types, in the Java `CalloutAnalyzer.MarkerType` declaration
/// order (that order is the model's label order, so it is preserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutMarkerType {
    Unknown,
    BracketText,
    BracketNumber,
    ParenthesisText,
    ParenthesisNumber,
    SuperscriptNumber,
    Number,
    Roman,
}

impl CalloutMarkerType {
    /// Every marker type, in Java declaration order.
    pub const ALL: [CalloutMarkerType; 8] = [
        Self::Unknown,
        Self::BracketText,
        Self::BracketNumber,
        Self::ParenthesisText,
        Self::ParenthesisNumber,
        Self::SuperscriptNumber,
        Self::Number,
        Self::Roman,
    ];

    /// The decision order of [`classify_callout_marker`], which is *not* the
    /// declaration order: superscript wins over brackets, brackets over
    /// parentheses, and `Unknown` is the blank-text terminal. Exposed so a
    /// reviewer can check the precedence without reading the classifier.
    pub const CHECK_ORDER: [CalloutMarkerType; 8] = [
        Self::SuperscriptNumber,
        Self::BracketNumber,
        Self::ParenthesisNumber,
        Self::BracketText,
        Self::ParenthesisText,
        Self::Number,
        Self::Roman,
        Self::Unknown,
    ];

    /// Wire spelling, identical to the Java enum constant lowercased.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::BracketText => "bracket_text",
            Self::BracketNumber => "bracket_number",
            Self::ParenthesisText => "parenthesis_text",
            Self::ParenthesisNumber => "parenthesis_number",
            Self::SuperscriptNumber => "superscript_number",
            Self::Number => "number",
            Self::Roman => "roman",
        }
    }

    /// The [`ObservedCalloutStyle`] this marker type votes for.
    ///
    /// The four numeric spellings vote `Numeric`, the two brackets/parentheses
    /// with text vote `AuthorYear` (the document printed names and years, not
    /// numbers), `Roman` votes `Roman`, and a blank marker votes `Unknown`.
    /// This is the bridge to the vocabulary [`crate::native_callout_matching`]
    /// already publishes; the vocabulary is reused, not re-declared.
    pub const fn style(self) -> ObservedCalloutStyle {
        match self {
            Self::Unknown => ObservedCalloutStyle::Unknown,
            Self::BracketText | Self::ParenthesisText => ObservedCalloutStyle::AuthorYear,
            Self::Roman => ObservedCalloutStyle::Roman,
            Self::BracketNumber
            | Self::ParenthesisNumber
            | Self::SuperscriptNumber
            | Self::Number => ObservedCalloutStyle::Numeric,
        }
    }

    /// Whether the marker can carry an ordinal (`[12] → bibl #12`).
    pub const fn is_numeric(self) -> bool {
        matches!(
            self.style(),
            ObservedCalloutStyle::Numeric | ObservedCalloutStyle::Roman
        )
    }
}

/// One callout token: the text, the superscript flag the layout plane does not
/// carry, and where it sits in the document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutToken {
    /// Token text exactly as the provider hands it over.
    pub text: String,
    /// Java `LayoutToken.isSuperscript()`.
    pub superscript: bool,
    /// Document position of this token (`CalloutPosition` is reused from the
    /// digest plane so both planes order tokens identically).
    pub position: CalloutPosition,
}

/// The classifier's result for one marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutMarkerClassification {
    /// The Java `MarkerType`.
    pub marker_type: CalloutMarkerType,
    /// [`CalloutMarkerType::style`] — carried so consumers never re-derive it
    /// and never disagree with the majority vote.
    pub style: ObservedCalloutStyle,
    /// `LayoutTokensUtil.toText(tokens)` — the token texts concatenated with no
    /// separator.
    pub text: String,
    /// `text` with ASCII spaces removed — the string every Java pattern is
    /// actually matched against.
    pub compact_text: String,
    /// Whether every non-blank token is a superscript token.
    pub is_superscript: bool,
    /// Contiguity of the marker's own tokens (see [`marker_adjacency`]).
    pub adjacency: AdjacencyDisposition,
    /// Number of admitted tokens, including blank ones.
    pub token_count: usize,
}

/// One majority-style vote tally, in [`ObservedCalloutStyle`] order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutStyleCount {
    pub style: ObservedCalloutStyle,
    pub count: usize,
}

/// The document's majority callout style, over every admitted marker —
/// including `Unknown` ones, because dropping an occurrence is exactly the
/// erosion MEM992 forbids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutStyleMajority {
    /// `Selected` (one winner), `Tie` (several), or `Empty` (no markers).
    /// `SourceNotEstablished` is part of the reused vocabulary but is not
    /// produced here: this plane always has its marker set in hand.
    pub outcome: MajorityStyleOutcome,
    /// The winning style, only when `outcome == Selected`.
    pub style: Option<ObservedCalloutStyle>,
    /// Every observed style with its count, in `ObservedCalloutStyle` order.
    pub counts: Vec<CalloutStyleCount>,
    /// How many markers voted.
    pub candidate_count: usize,
}

/// One reference line's `<label>` as the ordinal source for `[12] → bibl #12`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutLabelOrdinalEntry {
    /// 1-based reference ordinal — the line's position in the bibliography.
    pub ordinal: usize,
    /// The printed label exactly as the reference segmenter decoded it.
    pub label_text: String,
    /// [`normalize_callout_label`] of `label_text` — the lookup key.
    pub label_key: String,
    /// 0-based index of the decoded line this label came from.
    pub line_index: usize,
    /// Half-open byte range of the `<label>` span inside the line's text.
    pub byte_start: usize,
    pub byte_end: usize,
}

/// A normalized key that two reference lines claim with different ordinals.
///
/// Resolving such a key is refused with [`CalloutUnresolvedReason::Ambiguous`]
/// rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutLabelKeyConflict {
    pub key: String,
    /// The competing ordinals, ascending.
    pub ordinals: Vec<usize>,
}

/// `<label>` → ordinal map built from the reference segmenter's output (T04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutLabelOrdinalMap {
    /// One entry per labelled reference line, in reading order.
    pub entries: Vec<CalloutLabelOrdinalEntry>,
    /// Lines that carried no usable `<label>` span (none at all, or one whose
    /// text has no alphanumeric character). Recorded rather than dropped, so the
    /// ordinal axis stays auditable.
    pub unlabeled_lines: Vec<usize>,
    /// Keys claimed by more than one ordinal.
    pub conflicts: Vec<CalloutLabelKeyConflict>,
}

/// The outcome of resolving one marker text against the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLabelResolution {
    /// Exactly one reference line claims the key.
    Resolved { ordinal: usize, key: String },
    /// Several reference lines claim the key.
    Ambiguous { key: String, ordinals: Vec<usize> },
    /// No reference line claims the key.
    Unmapped { key: String },
}

/// One marker: a class plus the ordered tokens that spell it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutMarkerInput {
    pub target_class: CalloutTargetClass,
    pub tokens: Vec<CalloutToken>,
}

/// Typed resolution of one marker against the body's target inventory.
///
/// `CalloutUnresolvedReason` is reused verbatim from
/// [`crate::native_callout_matching`] — this plane adds no second reason
/// vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCalloutResolution {
    /// The marker named a reference the bibliography inventory knows.
    Targeted {
        ordinal: usize,
        label_key: String,
    },
    Unresolved {
        reason: CalloutUnresolvedReason,
    },
}

impl NativeCalloutResolution {
    /// Whether this occurrence counts into the targeted denominator.
    pub const fn is_targeted(&self) -> bool {
        matches!(self, Self::Targeted { .. })
    }

    /// The typed reason of an unresolved occurrence.
    pub const fn reason(&self) -> Option<CalloutUnresolvedReason> {
        match self {
            Self::Targeted { .. } => None,
            Self::Unresolved { reason } => Some(*reason),
        }
    }
}

/// One counted marker occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutMarkerObservation {
    /// 0-based position in document order; must equal the vector index.
    pub index: usize,
    pub target_class: CalloutTargetClass,
    /// Position of the marker's first token.
    pub position: CalloutPosition,
    pub marker: CalloutMarkerClassification,
    pub resolution: NativeCalloutResolution,
    /// Checksum of this occurrence, bound to the document identity when the
    /// denominator checksums are built.
    pub digest: String,
}

/// One checksum-bound denominator. `count` and `checksum` are recomputed by
/// [`CalloutInventory::validate`], so eroding or pooling occurrences is
/// detectable rather than silent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutDenominator {
    /// `targeted` or `targetless`; never a figure/citation counter label.
    pub label: String,
    /// Number of occurrences in this denominator.
    pub count: usize,
    /// Digest over the ordered member digests, in this denominator's own
    /// digest domain (label + lane + document + source identity included).
    pub checksum: String,
}

/// What the inventory can honestly claim about body linkage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLinkageVerdict {
    /// No occurrence was admitted: nothing can be claimed as success.
    Empty,
    /// Every admitted occurrence is targetless.
    Targetless,
    /// Some occurrences resolved, some did not.
    Partial,
    /// Every admitted occurrence resolved.
    Complete,
}

/// Every admitted callout occurrence, counted once, in two denominators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalloutInventory {
    pub schema_version: String,
    pub document_id: String,
    /// Source identity the occurrences were observed on.
    pub source_sha256: String,
    /// [`NATIVE_CALLOUT_INVENTORY_LANE`] — the anti-pooling namespace.
    pub lane: String,
    pub observations: Vec<CalloutMarkerObservation>,
    pub targeted: CalloutDenominator,
    pub targetless: CalloutDenominator,
    pub majority: CalloutStyleMajority,
}

/// Typed refusal vocabulary for classification, mapping and inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCalloutClassificationError {
    /// A marker carried no token, so it is not an occurrence.
    EmptyMarker { index: usize },
    /// No marker was admitted, so no inventory can be claimed.
    EmptyInventory,
    /// An admission bound was exceeded.
    LimitsExceeded {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    /// A required identity field is empty.
    MissingIdentity { field: &'static str },
    /// Observations are not in document order.
    NonMonotonic { index: usize },
    /// `observation.index` disagrees with its vector position.
    ObservationIndexMismatch { index: usize, actual: usize },
    /// An occurrence digest does not match its content.
    ObservationDigestMismatch { index: usize },
    /// A carried style disagrees with the marker type's own style.
    StyleMismatch { index: usize },
    /// The two denominators do not partition the observations.
    DenominatorCountMismatch {
        targeted: usize,
        targetless: usize,
        total: usize,
    },
    /// A denominator checksum does not match its members.
    DenominatorChecksumMismatch { label: String },
    /// A denominator carries the wrong label.
    DenominatorLabelMismatch { found: String },
    /// The two denominators were pooled into one (identical checksums).
    PooledDenominator,
    /// The carried majority tally disagrees with the observations.
    MajorityMismatch,
}

impl fmt::Display for NativeCalloutClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMarker { index } => {
                write!(formatter, "callout marker {index} carried no tokens")
            }
            Self::EmptyInventory => write!(formatter, "callout inventory has no occurrences"),
            Self::LimitsExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "callout {resource} count {actual} exceeds the maximum {maximum}"
            ),
            Self::MissingIdentity { field } => {
                write!(
                    formatter,
                    "callout inventory identity field {field} is empty"
                )
            }
            Self::NonMonotonic { index } => write!(
                formatter,
                "callout observation {index} is out of document order"
            ),
            Self::ObservationIndexMismatch { index, actual } => write!(
                formatter,
                "callout observation at position {index} carries index {actual}"
            ),
            Self::ObservationDigestMismatch { index } => write!(
                formatter,
                "callout observation {index} digest does not match its content"
            ),
            Self::StyleMismatch { index } => write!(
                formatter,
                "callout observation {index} carries a style its marker type does not vote for"
            ),
            Self::DenominatorCountMismatch {
                targeted,
                targetless,
                total,
            } => write!(
                formatter,
                "callout denominators {targeted} + {targetless} do not partition {total} occurrences"
            ),
            Self::DenominatorChecksumMismatch { label } => write!(
                formatter,
                "callout {label} denominator checksum does not match its members"
            ),
            Self::DenominatorLabelMismatch { found } => write!(
                formatter,
                "callout denominator label {found:?} is not the targeted or targetless label"
            ),
            Self::PooledDenominator => write!(
                formatter,
                "callout denominators were pooled: targeted and targetless checksums are identical"
            ),
            Self::MajorityMismatch => write!(
                formatter,
                "callout majority tally disagrees with the admitted observations"
            ),
        }
    }
}

impl std::error::Error for NativeCalloutClassificationError {}

/// Classify one callout marker exactly as `CalloutAnalyzer.getCalloutType` does.
///
/// Total by construction: a marker with no tokens (or only blank ones) is
/// [`CalloutMarkerType::Unknown`], which is precisely what the Java method
/// returns for `null`/blank input. Refusing a zero-token marker is the
/// *inventory builder's* job ([`NativeCalloutClassificationError::EmptyMarker`]),
/// because "no tokens" means "not an occurrence", which is a different claim
/// from "an unclassifiable marker".
pub fn classify_callout_marker(tokens: &[CalloutToken]) -> CalloutMarkerClassification {
    let text: String = tokens.iter().map(|token| token.text.as_str()).collect();
    let is_superscript = callout_is_superscript(tokens);
    let compact: Vec<char> = text.chars().filter(|character| *character != ' ').collect();

    let marker_type = if java_blank(&text) {
        CalloutMarkerType::Unknown
    } else if find_number_marker(&compact) && is_superscript {
        CalloutMarkerType::SuperscriptNumber
    } else if find_atomic_number_list(&compact, '[', ']') {
        CalloutMarkerType::BracketNumber
    } else if find_atomic_number_list(&compact, '(', ')') {
        CalloutMarkerType::ParenthesisNumber
    } else if find_text_span(&compact, '[', ']') {
        CalloutMarkerType::BracketText
    } else if find_text_span(&compact, '(', ')') {
        CalloutMarkerType::ParenthesisText
    } else if find_number_marker(&compact) {
        CalloutMarkerType::Number
    } else if find_roman_marker(&compact) {
        CalloutMarkerType::Roman
    } else {
        // Unreachable: `find_roman_marker` is a catch-all for non-blank text.
        // Total anyway, so no input can panic the classifier.
        CalloutMarkerType::Unknown
    };

    CalloutMarkerClassification {
        marker_type,
        style: marker_type.style(),
        text,
        compact_text: compact.into_iter().collect(),
        is_superscript,
        adjacency: marker_adjacency(tokens),
        token_count: tokens.len(),
    }
}

/// Contiguity of one marker's own tokens, reusing [`AdjacencyDisposition`].
///
/// `Adjacent` iff, in reading order, every consecutive pair shares page, block,
/// line and annex parent **and** has `token_ordinal + 1` as the next ordinal.
///
/// Boundary crossings dominate: a marker whose tokens cross a page/block/line/
/// annex boundary anywhere is [`AdjacencyDisposition::IllegalBoundaryCrossing`],
/// even if an earlier pair was merely non-consecutive. That is the fail-closed
/// reading — a marker that crosses a boundary is not one marker, so it must
/// never be allowed to resolve. Only when no pair crosses a boundary does a
/// non-consecutive pair report [`AdjacencyDisposition::NonAdjacent`]. A
/// single-token marker is trivially `Adjacent`; an empty marker has no source
/// and reports [`AdjacencyDisposition::SourceNotEstablished`].
pub fn marker_adjacency(tokens: &[CalloutToken]) -> AdjacencyDisposition {
    let Some(first) = tokens.first() else {
        return AdjacencyDisposition::SourceNotEstablished;
    };
    let mut previous = first.position;
    let mut non_contiguous = false;
    for token in &tokens[1..] {
        let current = token.position;
        if boundary_differs(previous, current) {
            return AdjacencyDisposition::IllegalBoundaryCrossing;
        }
        if previous.token_ordinal.checked_add(1) != Some(current.token_ordinal) {
            non_contiguous = true;
        }
        previous = current;
    }
    if non_contiguous {
        AdjacencyDisposition::NonAdjacent
    } else {
        AdjacencyDisposition::Adjacent
    }
}

/// The document's majority callout style over **every** admitted marker.
///
/// `Empty` when nothing voted, `Tie` when several styles share the maximum,
/// `Selected` when exactly one does — the same rule (and the same
/// [`MajorityStyleOutcome`] vocabulary) as the digest plane's majority tally.
pub fn majority_callout_style(
    classifications: &[CalloutMarkerClassification],
) -> CalloutStyleMajority {
    let styles: Vec<ObservedCalloutStyle> = classifications
        .iter()
        .map(|classification| classification.style)
        .collect();
    majority_of_styles(&styles)
}

/// Normalize a printed label into its lookup key.
///
/// Keeps ASCII alphanumerics (lowercased) and drops everything else, so the
/// four ways a document can print the same ordinal — `12`, `12.`, `[12]`,
/// `(12)` — share one key. Returns an empty key when the label carries no
/// alphanumeric character at all; [`CalloutLabelOrdinalMap::resolve_label`]
/// reports an empty key as `Unmapped`, never as a match.
pub fn normalize_callout_label(label: &str) -> String {
    label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// Build the `<label>` → ordinal map from decoded reference-segmenter lines.
///
/// One entry per labelled line: the ordinal is the line's 1-based position in
/// the bibliography, which is exactly the ordinal `[12] → bibl #12` names. A
/// line's first `<label>` span is its label; further `<label>` runs in the same
/// line are not extra ordinals (ordinals are per reference, not per label run).
/// Lines with no usable label are recorded in
/// [`CalloutLabelOrdinalMap::unlabeled_lines`] instead of being dropped.
pub fn build_callout_label_ordinal_map(
    lines: &[ReferenceSegmentSequence],
) -> Result<CalloutLabelOrdinalMap, NativeCalloutClassificationError> {
    if lines.len() > MAX_NATIVE_CALLOUT_REFERENCE_LINES {
        return Err(NativeCalloutClassificationError::LimitsExceeded {
            resource: "reference_line",
            actual: lines.len(),
            maximum: MAX_NATIVE_CALLOUT_REFERENCE_LINES,
        });
    }
    let mut entries = Vec::new();
    let mut unlabeled_lines = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let Some(span) = line
            .segments
            .iter()
            .find(|segment| segment.kind == ReferenceSegmentKind::Label)
        else {
            unlabeled_lines.push(line_index);
            continue;
        };
        let label_key = normalize_callout_label(&span.text);
        if label_key.is_empty() {
            unlabeled_lines.push(line_index);
            continue;
        }
        entries.push(CalloutLabelOrdinalEntry {
            ordinal: line_index + 1,
            label_text: span.text.clone(),
            label_key,
            line_index,
            byte_start: span.byte_start,
            byte_end: span.byte_end,
        });
    }

    let mut ordinals_by_key: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for entry in &entries {
        ordinals_by_key
            .entry(entry.label_key.as_str())
            .or_default()
            .push(entry.ordinal);
    }
    let conflicts = ordinals_by_key
        .into_iter()
        .filter(|(_, ordinals)| ordinals.len() > 1)
        .map(|(key, ordinals)| CalloutLabelKeyConflict {
            key: key.to_string(),
            ordinals,
        })
        .collect();

    Ok(CalloutLabelOrdinalMap {
        entries,
        unlabeled_lines,
        conflicts,
    })
}

impl CalloutLabelOrdinalMap {
    /// Whether the map knows no labelled reference at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many labelled references the map knows.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Resolve one marker text to a reference ordinal.
    ///
    /// Ambiguity is never guessed: a key claimed by several ordinals comes back
    /// as [`CalloutLabelResolution::Ambiguous`] and becomes
    /// [`CalloutUnresolvedReason::Ambiguous`] in the inventory.
    pub fn resolve_label(&self, marker_text: &str) -> CalloutLabelResolution {
        let key = normalize_callout_label(marker_text);
        let mut ordinals: Vec<usize> = self
            .entries
            .iter()
            .filter(|entry| entry.label_key == key)
            .map(|entry| entry.ordinal)
            .collect();
        ordinals.dedup();
        match ordinals.len() {
            0 => CalloutLabelResolution::Unmapped { key },
            1 => CalloutLabelResolution::Resolved {
                ordinal: ordinals[0],
                key,
            },
            _ => CalloutLabelResolution::Ambiguous { key, ordinals },
        }
    }
}

/// Build the occurrence inventory for one document.
///
/// Every marker is admitted once and lands in exactly one denominator, in
/// document order. The resolution precedence is fixed and documented, so a
/// marker can never be counted as targeted through two different rules:
///
/// 1. a zero-token marker is refused
///    ([`NativeCalloutClassificationError::EmptyMarker`]);
/// 2. a marker whose tokens cross a page/block/line/annex boundary is
///    `IllegalAdjacency` — it is not one marker;
/// 3. a blank (unclassifiable) marker is `SourceNotEstablished`;
/// 4. a `Citation` marker resolves through the label map: `Targeted`,
///    `Ambiguous`, `TargetNotInInventory`, or `NoTarget` when the bibliography
///    inventory is empty;
/// 5. an `Equation` marker is `UnsupportedEquation`;
/// 6. a `Figure`/`Table` marker is `SourceNotEstablished` — this lane
///    establishes the bibliography inventory only.
pub fn build_callout_inventory(
    document_id: &str,
    source_sha256: &str,
    markers: &[CalloutMarkerInput],
    labels: &CalloutLabelOrdinalMap,
) -> Result<CalloutInventory, NativeCalloutClassificationError> {
    if document_id.is_empty() {
        return Err(NativeCalloutClassificationError::MissingIdentity {
            field: "document_id",
        });
    }
    if source_sha256.is_empty() {
        return Err(NativeCalloutClassificationError::MissingIdentity {
            field: "source_sha256",
        });
    }
    if markers.len() > MAX_NATIVE_CALLOUT_MARKERS {
        return Err(NativeCalloutClassificationError::LimitsExceeded {
            resource: "marker",
            actual: markers.len(),
            maximum: MAX_NATIVE_CALLOUT_MARKERS,
        });
    }
    if markers.is_empty() {
        return Err(NativeCalloutClassificationError::EmptyInventory);
    }

    let mut observations = Vec::with_capacity(markers.len());
    for (index, marker) in markers.iter().enumerate() {
        if marker.tokens.is_empty() {
            return Err(NativeCalloutClassificationError::EmptyMarker { index });
        }
        if marker.tokens.len() > MAX_NATIVE_CALLOUT_MARKER_TOKENS {
            return Err(NativeCalloutClassificationError::LimitsExceeded {
                resource: "marker_token",
                actual: marker.tokens.len(),
                maximum: MAX_NATIVE_CALLOUT_MARKER_TOKENS,
            });
        }
        let classification = classify_callout_marker(&marker.tokens);
        let resolution = resolve_marker(&classification, marker.target_class, labels);
        let position = marker.tokens[0].position;
        if let Some(previous) = observations
            .last()
            .map(|observation: &CalloutMarkerObservation| observation.position)
            && position < previous
        {
            return Err(NativeCalloutClassificationError::NonMonotonic { index });
        }
        let digest = observation_digest(
            index,
            marker.target_class,
            position,
            &classification,
            &resolution,
        );
        observations.push(CalloutMarkerObservation {
            index,
            target_class: marker.target_class,
            position,
            marker: classification,
            resolution,
            digest,
        });
    }

    let targeted_digests = member_digests(&observations, true);
    let targetless_digests = member_digests(&observations, false);
    let inventory = CalloutInventory {
        schema_version: NATIVE_CALLOUT_CLASSIFICATION_SCHEMA_VERSION.to_string(),
        document_id: document_id.to_string(),
        source_sha256: source_sha256.to_string(),
        lane: NATIVE_CALLOUT_INVENTORY_LANE.to_string(),
        targeted: CalloutDenominator {
            label: NATIVE_CALLOUT_TARGETED_DENOMINATOR.to_string(),
            count: targeted_digests.len(),
            checksum: denominator_checksum(
                NATIVE_CALLOUT_TARGETED_DENOMINATOR,
                NATIVE_CALLOUT_INVENTORY_LANE,
                document_id,
                source_sha256,
                &targeted_digests,
            ),
        },
        targetless: CalloutDenominator {
            label: NATIVE_CALLOUT_TARGETLESS_DENOMINATOR.to_string(),
            count: targetless_digests.len(),
            checksum: denominator_checksum(
                NATIVE_CALLOUT_TARGETLESS_DENOMINATOR,
                NATIVE_CALLOUT_INVENTORY_LANE,
                document_id,
                source_sha256,
                &targetless_digests,
            ),
        },
        majority: majority_of_styles(
            &observations
                .iter()
                .map(|observation| observation.marker.style)
                .collect::<Vec<_>>(),
        ),
        observations,
    };
    inventory.validate()?;
    Ok(inventory)
}

impl CalloutInventory {
    /// Targeted occurrences (markers that named a reference in the inventory).
    pub fn targeted_count(&self) -> usize {
        self.targeted.count
    }

    /// Targetless occurrences (every other admitted marker). A denominator of
    /// its own — never folded into [`Self::targeted_count`].
    pub fn targetless_count(&self) -> usize {
        self.targetless.count
    }

    /// Every admitted occurrence.
    pub fn total_count(&self) -> usize {
        self.observations.len()
    }

    /// The honest linkage verdict for this inventory.
    pub fn verdict(&self) -> CalloutLinkageVerdict {
        callout_linkage_verdict(self.targeted_count(), self.targetless_count())
    }

    /// Whether every admitted occurrence resolved. A targetless-heavy document
    /// is **not** complete however large `total_count()` is.
    pub fn is_complete(&self) -> bool {
        matches!(self.verdict(), CalloutLinkageVerdict::Complete)
    }

    /// Recheck identity, order, occurrence digests, both denominators and the
    /// majority tally.
    ///
    /// This is what makes the denominators checksum-bound rather than
    /// decorative: erasing an occurrence, moving one across denominators, or
    /// pooling the two (`targeted.checksum == targetless.checksum`) is refused.
    pub fn validate(&self) -> Result<(), NativeCalloutClassificationError> {
        if self.document_id.is_empty() {
            return Err(NativeCalloutClassificationError::MissingIdentity {
                field: "document_id",
            });
        }
        if self.source_sha256.is_empty() {
            return Err(NativeCalloutClassificationError::MissingIdentity {
                field: "source_sha256",
            });
        }
        if self.observations.is_empty() {
            return Err(NativeCalloutClassificationError::EmptyInventory);
        }
        if self.observations.len() > MAX_NATIVE_CALLOUT_MARKERS {
            return Err(NativeCalloutClassificationError::LimitsExceeded {
                resource: "marker",
                actual: self.observations.len(),
                maximum: MAX_NATIVE_CALLOUT_MARKERS,
            });
        }
        if self.targeted.label != NATIVE_CALLOUT_TARGETED_DENOMINATOR {
            return Err(NativeCalloutClassificationError::DenominatorLabelMismatch {
                found: self.targeted.label.clone(),
            });
        }
        if self.targetless.label != NATIVE_CALLOUT_TARGETLESS_DENOMINATOR {
            return Err(NativeCalloutClassificationError::DenominatorLabelMismatch {
                found: self.targetless.label.clone(),
            });
        }
        if self.targeted.checksum == self.targetless.checksum {
            return Err(NativeCalloutClassificationError::PooledDenominator);
        }

        let mut previous = None;
        for (index, observation) in self.observations.iter().enumerate() {
            if observation.index != index {
                return Err(NativeCalloutClassificationError::ObservationIndexMismatch {
                    index,
                    actual: observation.index,
                });
            }
            if observation.marker.style != observation.marker.marker_type.style() {
                return Err(NativeCalloutClassificationError::StyleMismatch { index });
            }
            if observation.digest
                != observation_digest(
                    index,
                    observation.target_class,
                    observation.position,
                    &observation.marker,
                    &observation.resolution,
                )
            {
                return Err(NativeCalloutClassificationError::ObservationDigestMismatch { index });
            }
            if let Some(previous) = previous
                && observation.position < previous
            {
                return Err(NativeCalloutClassificationError::NonMonotonic { index });
            }
            previous = Some(observation.position);
        }

        let targeted_digests = member_digests(&self.observations, true);
        let targetless_digests = member_digests(&self.observations, false);
        if targeted_digests.len() != self.targeted.count
            || targetless_digests.len() != self.targetless.count
            || self.targeted.count + self.targetless.count != self.observations.len()
        {
            return Err(NativeCalloutClassificationError::DenominatorCountMismatch {
                targeted: self.targeted.count,
                targetless: self.targetless.count,
                total: self.observations.len(),
            });
        }
        if self.targeted.checksum
            != denominator_checksum(
                NATIVE_CALLOUT_TARGETED_DENOMINATOR,
                &self.lane,
                &self.document_id,
                &self.source_sha256,
                &targeted_digests,
            )
        {
            return Err(
                NativeCalloutClassificationError::DenominatorChecksumMismatch {
                    label: NATIVE_CALLOUT_TARGETED_DENOMINATOR.to_string(),
                },
            );
        }
        if self.targetless.checksum
            != denominator_checksum(
                NATIVE_CALLOUT_TARGETLESS_DENOMINATOR,
                &self.lane,
                &self.document_id,
                &self.source_sha256,
                &targetless_digests,
            )
        {
            return Err(
                NativeCalloutClassificationError::DenominatorChecksumMismatch {
                    label: NATIVE_CALLOUT_TARGETLESS_DENOMINATOR.to_string(),
                },
            );
        }
        if self.majority
            != majority_of_styles(
                &self
                    .observations
                    .iter()
                    .map(|observation| observation.marker.style)
                    .collect::<Vec<_>>(),
            )
        {
            return Err(NativeCalloutClassificationError::MajorityMismatch);
        }
        Ok(())
    }
}

/// The verdict rule, exposed so a caller can state it without an inventory.
///
/// Zero occurrences is `Empty`, not `Complete`: a claim over an empty
/// denominator cannot be a success.
pub fn callout_linkage_verdict(targeted: usize, targetless: usize) -> CalloutLinkageVerdict {
    if targeted + targetless == 0 {
        CalloutLinkageVerdict::Empty
    } else if targetless == 0 {
        CalloutLinkageVerdict::Complete
    } else if targeted == 0 {
        CalloutLinkageVerdict::Targetless
    } else {
        CalloutLinkageVerdict::Partial
    }
}

/// Resolve one classified marker against the body's target inventory.
fn resolve_marker(
    classification: &CalloutMarkerClassification,
    target_class: CalloutTargetClass,
    labels: &CalloutLabelOrdinalMap,
) -> NativeCalloutResolution {
    if classification.adjacency == AdjacencyDisposition::IllegalBoundaryCrossing {
        return unresolved(CalloutUnresolvedReason::IllegalAdjacency);
    }
    if classification.marker_type == CalloutMarkerType::Unknown {
        return unresolved(CalloutUnresolvedReason::SourceNotEstablished);
    }
    match target_class {
        CalloutTargetClass::Citation => match labels.resolve_label(&classification.text) {
            CalloutLabelResolution::Resolved { ordinal, key } => {
                NativeCalloutResolution::Targeted {
                    ordinal,
                    label_key: key,
                }
            }
            CalloutLabelResolution::Ambiguous { .. } => {
                unresolved(CalloutUnresolvedReason::Ambiguous)
            }
            CalloutLabelResolution::Unmapped { .. } => unresolved(if labels.is_empty() {
                CalloutUnresolvedReason::NoTarget
            } else {
                CalloutUnresolvedReason::TargetNotInInventory
            }),
        },
        CalloutTargetClass::Equation => unresolved(CalloutUnresolvedReason::UnsupportedEquation),
        CalloutTargetClass::Figure | CalloutTargetClass::Table => {
            unresolved(CalloutUnresolvedReason::SourceNotEstablished)
        }
    }
}

fn unresolved(reason: CalloutUnresolvedReason) -> NativeCalloutResolution {
    NativeCalloutResolution::Unresolved { reason }
}

/// Majority tally over already-extracted styles (the one implementation both
/// [`majority_callout_style`] and [`CalloutInventory::validate`] use).
fn majority_of_styles(styles: &[ObservedCalloutStyle]) -> CalloutStyleMajority {
    let mut counts: BTreeMap<ObservedCalloutStyle, usize> = BTreeMap::new();
    for style in styles {
        *counts.entry(*style).or_insert(0) += 1;
    }
    if counts.is_empty() {
        return CalloutStyleMajority {
            outcome: MajorityStyleOutcome::Empty,
            style: None,
            counts: Vec::new(),
            candidate_count: 0,
        };
    }
    let maximum = *counts.values().max().unwrap_or(&0);
    let winners: Vec<ObservedCalloutStyle> = counts
        .iter()
        .filter(|(_, count)| **count == maximum)
        .map(|(style, _)| *style)
        .collect();
    let selected = (winners.len() == 1).then_some(winners[0]);
    CalloutStyleMajority {
        outcome: if selected.is_some() {
            MajorityStyleOutcome::Selected
        } else {
            MajorityStyleOutcome::Tie
        },
        style: selected,
        counts: counts
            .into_iter()
            .map(|(style, count)| CalloutStyleCount { style, count })
            .collect(),
        candidate_count: styles.len(),
    }
}

/// Occurrence digests of one denominator, in document order.
fn member_digests(observations: &[CalloutMarkerObservation], targeted: bool) -> Vec<String> {
    observations
        .iter()
        .filter(|observation| observation.resolution.is_targeted() == targeted)
        .map(|observation| observation.digest.clone())
        .collect()
}

/// Digest of one occurrence, bound to its own content and document slot.
fn observation_digest(
    index: usize,
    target_class: CalloutTargetClass,
    position: CalloutPosition,
    classification: &CalloutMarkerClassification,
    resolution: &NativeCalloutResolution,
) -> String {
    let tag = match resolution {
        NativeCalloutResolution::Targeted { ordinal, label_key } => {
            format!("targeted/{ordinal}/{label_key}")
        }
        NativeCalloutResolution::Unresolved { reason } => format!("unresolved/{reason:?}"),
    };
    let mut hasher = Sha256::new();
    hasher.update(CALLOUT_OBSERVATION_DIGEST_DOMAIN.as_bytes());
    hasher.update(
        format!(
            "|{index}|{:?}|{:?}|{}|{}|{}|{tag}",
            target_class,
            position,
            classification.marker_type.as_str(),
            classification.compact_text,
            classification.adjacency as u8,
        )
        .as_bytes(),
    );
    format!("sha256:{:x}", hasher.finalize())
}

/// Checksum of one denominator: its own label, the lane, the document identity
/// and the ordered member digests. The label and lane are inside the hashed
/// bytes, so targeted and targetless checksums can never coincide (pooling is
/// structurally impossible, not merely discouraged).
fn denominator_checksum(
    label: &str,
    lane: &str,
    document_id: &str,
    source_sha256: &str,
    member_digests: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CALLOUT_DENOMINATOR_DIGEST_DOMAIN.as_bytes());
    hasher.update(format!("|{label}|{lane}|{document_id}|{source_sha256}|").as_bytes());
    for digest in member_digests {
        hasher.update(digest.as_bytes());
        hasher.update(b";");
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn boundary_differs(previous: CalloutPosition, current: CalloutPosition) -> bool {
    previous.page_ordinal != current.page_ordinal
        || previous.block_ordinal != current.block_ordinal
        || previous.line_ordinal != current.line_ordinal
        || previous.annex_parent != current.annex_parent
}

/// Java `String.trim().length() == 0`: every character is `<= U+0020`.
///
/// Rust's `str::trim` is deliberately *not* used — it also strips non-breaking
/// spaces, which Java's `trim()` keeps, so a `\u{00A0}` token is a non-blank
/// token for the superscript rule.
fn java_blank(text: &str) -> bool {
    text.chars().all(|character| character <= ' ')
}

/// Java's superscript rule: every non-blank token is superscript. Blank tokens
/// are skipped and never clear the flag, so an all-blank marker stays "true" —
/// which is unreachable in practice because such a marker is already `UNKNOWN`.
fn callout_is_superscript(tokens: &[CalloutToken]) -> bool {
    tokens
        .iter()
        .filter(|token| !java_blank(&token.text))
        .all(|token| token.superscript)
}

/// `NUMBER_PATTERN = (?>\d+)[a-f]?` under `find()`.
///
/// The optional `[a-f]` suffix cannot change *whether* a match exists: the
/// atomic greedy digit run already matches on its own. So the predicate is
/// exactly "the text contains an ASCII digit".
fn find_number_marker(text: &[char]) -> bool {
    text.iter().any(|character| character.is_ascii_digit())
}

/// `ROMAN_PATTERN = (IX|IV|V?I{0,3})` under `find()`.
///
/// The third alternative is `V?I{0,3}`, every part of which is optional, so it
/// matches the empty string at position 0 of any string. `find()` therefore
/// succeeds on any non-empty input and this Java check is a **catch-all**: a
/// non-blank marker falls back to `ROMAN` whenever no earlier pattern matched.
/// Reproduced rather than corrected, because the marker *type* is only an input
/// to majority-style voting.
fn find_roman_marker(text: &[char]) -> bool {
    !text.is_empty()
}

/// `\[(?>[0-9]{1,4}[a-f]?[\-;•,]?((and)|&|(et))?)+\]` under `find()`, for any
/// bracket pair.
///
/// The atomic group makes one repetition a deterministic greedy parse; the `+`
/// outside it is greedy with backtracking, which for *existence* is the same as
/// "some prefix of the deterministic repetition parse is followed by the
/// closing bracket". That is exactly what this loop checks.
fn find_atomic_number_list(text: &[char], open: char, close: char) -> bool {
    for start in 0..text.len() {
        if text[start] != open {
            continue;
        }
        let mut cursor = start + 1;
        while let Some(next) = parse_list_repetition(text, cursor) {
            cursor = next;
            if text.get(cursor) == Some(&close) {
                return true;
            }
        }
    }
    false
}

/// One atomic `[0-9]{1,4}[a-f]?[\-;•,]?((and)|&|(et))?` repetition, or `None`
/// when no repetition can start here. `•` is the same U+2022 bullet the Java
/// character class carries.
fn parse_list_repetition(text: &[char], start: usize) -> Option<usize> {
    let mut cursor = start;
    let mut digits = 0;
    while digits < 4 && text.get(cursor).is_some_and(|c| c.is_ascii_digit()) {
        cursor += 1;
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    if text.get(cursor).is_some_and(|c| matches!(c, 'a'..='f')) {
        cursor += 1;
    }
    if text
        .get(cursor)
        .is_some_and(|c| matches!(c, '-' | ';' | '\u{2022}' | ','))
    {
        cursor += 1;
    }
    if starts_with_at(text, cursor, "and") {
        cursor += 3;
    } else if text.get(cursor) == Some(&'&') {
        cursor += 1;
    } else if starts_with_at(text, cursor, "et") {
        cursor += 2;
    }
    Some(cursor)
}

/// `\[(.)+\]` / `\((.)+\)` under `find()`, where `.` is Java's dot: any single
/// character **except** a line terminator.
fn find_text_span(text: &[char], open: char, close: char) -> bool {
    for start in 0..text.len() {
        if text[start] != open {
            continue;
        }
        let mut cursor = start + 1;
        while let Some(&character) = text.get(cursor) {
            if is_java_line_terminator(character) {
                break;
            }
            if character == close && cursor > start + 1 {
                return true;
            }
            cursor += 1;
        }
    }
    false
}

/// Java `Pattern`'s default line terminators; `.` matches none of them unless
/// `DOTALL` is set, and `CalloutAnalyzer` does not set it.
const fn is_java_line_terminator(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

fn starts_with_at(text: &[char], start: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(offset, expected)| text.get(start + offset) == Some(&expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(token: u32) -> CalloutPosition {
        CalloutPosition {
            page_ordinal: 0,
            block_ordinal: 0,
            line_ordinal: 0,
            token_ordinal: token,
            annex_parent: None,
        }
    }

    fn token(text: &str, superscript: bool, ordinal: u32) -> CalloutToken {
        CalloutToken {
            text: text.to_string(),
            superscript,
            position: position(ordinal),
        }
    }

    fn plain(texts: &[&str]) -> Vec<CalloutToken> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| token(text, false, index as u32))
            .collect()
    }

    fn classify(texts: &[&str]) -> CalloutMarkerType {
        classify_callout_marker(&plain(texts)).marker_type
    }

    /// The eight Java `MarkerType` constants, each reached by one hand-derived
    /// input. The expectations come from the Java source quoted in the module
    /// docs, not from this implementation.
    #[test]
    fn eight_java_marker_types_are_reachable() {
        assert_eq!(classify(&["12"]), CalloutMarkerType::Number);
        assert_eq!(
            classify(&["[", "12", "]"]),
            CalloutMarkerType::BracketNumber
        );
        assert_eq!(
            classify(&["(", "12", ")"]),
            CalloutMarkerType::ParenthesisNumber
        );
        assert_eq!(
            classify(&["[", "et", " ", "al", "]"]),
            CalloutMarkerType::BracketText
        );
        assert_eq!(
            classify(&["(", "Smith", " ", "2019", ")"]),
            CalloutMarkerType::ParenthesisText
        );
        let superscript = vec![token("12", true, 0)];
        assert_eq!(
            classify_callout_marker(&superscript).marker_type,
            CalloutMarkerType::SuperscriptNumber
        );
        assert_eq!(classify(&["IV"]), CalloutMarkerType::Roman);
        assert_eq!(classify(&[" "]), CalloutMarkerType::Unknown);
    }

    /// The decision order is a published API, and it is not the declaration
    /// order.
    #[test]
    fn check_order_is_published_and_covers_every_type() {
        assert_eq!(CalloutMarkerType::CHECK_ORDER.len(), 8);
        assert_eq!(CalloutMarkerType::ALL.len(), 8);
        for marker_type in CalloutMarkerType::ALL {
            assert!(CalloutMarkerType::CHECK_ORDER.contains(&marker_type));
        }
        assert_eq!(
            CalloutMarkerType::CHECK_ORDER[0],
            CalloutMarkerType::SuperscriptNumber
        );
        assert_eq!(
            CalloutMarkerType::CHECK_ORDER[7],
            CalloutMarkerType::Unknown
        );
    }

    /// Java's first check is unanchored, so a superscript bracket is a
    /// superscript number, not a bracket number.
    #[test]
    fn superscript_wins_over_brackets_exactly_as_in_java() {
        let tokens = vec![
            CalloutToken {
                text: "[".into(),
                superscript: true,
                position: position(0),
            },
            CalloutToken {
                text: "12".into(),
                superscript: true,
                position: position(1),
            },
            CalloutToken {
                text: "]".into(),
                superscript: true,
                position: position(2),
            },
        ];
        let classification = classify_callout_marker(&tokens);
        assert_eq!(
            classification.marker_type,
            CalloutMarkerType::SuperscriptNumber
        );
        assert!(classification.is_superscript);
        assert_eq!(classification.compact_text, "[12]");
        assert_eq!(classification.style, ObservedCalloutStyle::Numeric);
    }

    /// A blank token never clears the superscript flag (Java skips it).
    #[test]
    fn blank_tokens_do_not_clear_the_superscript_flag() {
        let tokens = vec![token("12", true, 0), token(" ", false, 1)];
        let classification = classify_callout_marker(&tokens);
        assert!(classification.is_superscript);
        assert_eq!(
            classification.marker_type,
            CalloutMarkerType::SuperscriptNumber
        );
    }

    /// Roman is Java's catch-all: any non-blank text that no earlier pattern
    /// matched is `ROMAN`, so `UNKNOWN` is only ever blank text.
    #[test]
    fn roman_is_the_java_catch_all_and_unknown_is_only_blank() {
        assert_eq!(classify(&["xyz"]), CalloutMarkerType::Roman);
        assert_eq!(classify(&["Figure"]), CalloutMarkerType::Roman);
        assert_eq!(classify(&[]), CalloutMarkerType::Unknown);
        assert_eq!(classify(&[""]), CalloutMarkerType::Unknown);
        assert_eq!(classify(&["\t"]), CalloutMarkerType::Unknown);
        assert!(CalloutMarkerType::Roman.is_numeric());
        assert!(!CalloutMarkerType::Unknown.is_numeric());
    }

    /// The bracket-number list accepts a `,`-separated list and lower-case
    /// `[a-f]` suffixes, and it is checked before bracket *text*.
    #[test]
    fn atomic_number_lists_accept_java_shapes_and_precede_text() {
        assert_eq!(
            classify(&["[", "12", ",", "13", "]"]),
            CalloutMarkerType::BracketNumber
        );
        assert_eq!(classify(&["[12", "a]"]), CalloutMarkerType::BracketNumber);
        assert_eq!(
            classify(&["(", "1", ";", "2", ")"]),
            CalloutMarkerType::ParenthesisNumber
        );
        // `[Fig1]` has a digit but not directly after the bracket.
        assert_eq!(
            classify(&["[", "Fig", "1", "]"]),
            CalloutMarkerType::BracketText
        );
        // `[]` has an empty group, which `(.)+` refuses.
        assert_eq!(classify(&["[", "]"]), CalloutMarkerType::Roman);
    }

    /// Tabs are not removed and spaces are: the compact text is the Java
    /// `replace(" ", "")` string.
    #[test]
    fn only_ascii_spaces_are_removed() {
        let classification = classify_callout_marker(&plain(&["[", "1", " ", "2", "]"]));
        assert_eq!(classification.text, "[1 2]");
        assert_eq!(classification.compact_text, "[12]");
        assert_eq!(classification.marker_type, CalloutMarkerType::BracketNumber);
    }

    #[test]
    fn classification_is_deterministic_and_carries_its_token_count() {
        let tokens = plain(&["[", "12", "]"]);
        let first = classify_callout_marker(&tokens);
        let second = classify_callout_marker(&tokens);
        assert_eq!(first, second);
        assert_eq!(first.token_count, 3);
        assert_eq!(first.adjacency, AdjacencyDisposition::Adjacent);
    }

    #[test]
    fn label_normalization_folds_the_four_printed_spellings() {
        assert_eq!(normalize_callout_label("12"), "12");
        assert_eq!(normalize_callout_label("12."), "12");
        assert_eq!(normalize_callout_label("[12]"), "12");
        assert_eq!(normalize_callout_label("(12)"), "12");
        assert_eq!(normalize_callout_label("[12A]"), "12a");
        assert_eq!(normalize_callout_label("[]"), "");
        assert_eq!(normalize_callout_label("—"), "");
    }

    #[test]
    fn verdict_never_calls_an_empty_or_targetless_inventory_complete() {
        assert_eq!(callout_linkage_verdict(0, 0), CalloutLinkageVerdict::Empty);
        assert_eq!(
            callout_linkage_verdict(0, MEM992_FROZEN_TARGETLESS_CALLOUTS),
            CalloutLinkageVerdict::Targetless
        );
        assert_eq!(
            callout_linkage_verdict(
                MEM992_FROZEN_TARGETED_CALLOUTS,
                MEM992_FROZEN_TARGETLESS_CALLOUTS
            ),
            CalloutLinkageVerdict::Partial
        );
        assert_eq!(
            callout_linkage_verdict(MEM992_FROZEN_TARGETED_CALLOUTS, 0),
            CalloutLinkageVerdict::Complete
        );
    }

    /// MEM992's frozen split is a declared pair of separate denominators, and
    /// the total is their sum — never a third denominator of its own.
    #[test]
    fn mem992_split_is_declared_as_two_denominators() {
        assert_eq!(
            MEM992_FROZEN_TARGETED_CALLOUTS + MEM992_FROZEN_TARGETLESS_CALLOUTS,
            MEM992_FROZEN_TOTAL_CALLOUTS
        );
        assert_ne!(
            NATIVE_CALLOUT_TARGETED_DENOMINATOR,
            NATIVE_CALLOUT_TARGETLESS_DENOMINATOR
        );
    }
}
