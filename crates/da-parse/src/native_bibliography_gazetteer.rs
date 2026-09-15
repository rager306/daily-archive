//! Pure positional gazetteer matching for the native GROBID bibliography ABI.
//!
//! S01's `FeatureAbi::extract` answers *"is this one token in the dictionary?"*
//! with an exact `binary_search` membership test. The citation model asks a
//! different question: *"which token positions are covered by a gazetteer
//! match?"* Java `CitationParser.processingLayoutTokenMultiple`
//! (`grobid-core/src/main/java/org/grobid/core/engines/CitationParser.java:171-178`)
//! computes eight position lists per citation block and
//! `FeaturesVectorCitation.addFeaturesCitation` sets `isKnown*` / `http` only
//! when `start <= n <= end` for the token index `n`. The two mechanisms agree
//! for a one-token match and diverge for every multi-token phrase, so both must
//! exist and this module owns the second one. [`span_covers`] is that
//! consumption rule, published as its own function precisely because it is the
//! thing that is *not* membership.
//!
//! ## Source of truth (format authority)
//!
//! Ported from the vendored GROBID tree (read for format only; never linked,
//! vendored, or copied):
//!
//! - `.../engines/CitationParser.java` (`processingLayoutTokenMultiple`) — the
//!   eight lanes and their order;
//! - `.../lexicon/FastMatcher.java` (`loadTerm`, `matchLayoutToken`) — the
//!   phrase walk, the delimiter skipping, and the *token-index* span shape;
//! - `.../lexicon/Lexicon.java` (`tokenPositions*`, `convertStringOffsetToTokenOffset`)
//!   — lane plumbing and the character-offset → token-offset mapping;
//! - `.../utilities/TextUtilities.java` (`delimiters`, `fullPunctuations`,
//!   `DOIPattern`, `arXivPattern`, `urlPattern1`) — the delimiter set and the
//!   pattern lanes;
//! - `.../utilities/UnicodeUtil.java` (`normaliseText`) and
//!   `.../engines/...` call sites — normalisation before the lookup;
//! - `.../features/FeaturesVectorCitation.java` (`addFeaturesCitation`) — the
//!   inclusive `start <= n <= end` consumption.
//!
//! ## What this module deliberately does not reproduce
//!
//! - **Pattern lanes are shape rules, not the source-bound GROBID pattern set.**
//!   `Crossref`-style identifier configuration lives in the local `grobid-home`
//!   `Lexicon` configuration; the real pattern inventory stays opt-in and its
//!   absence is reported as missingness, never as a pass (D358). The DOI and
//!   arXiv shapes below transcribe `TextUtilities.DOIPattern` / `arXivPattern`
//!   as bounded linear scanners; the URL shape transcribes `urlPattern1`. No
//!   lexeme, model byte, or lexicon byte is committed here.
//! - **`ISSN`/`ISBN` stay unwired.** `Lexicon.tokenPositionsIdentifierPattern`
//!   only merges DOI and arXiv: both `tokenPositionsISSNPattern` and
//!   `tokenPositionsISBNPattern` are empty stubs and their call sites are
//!   commented out upstream. Emitting them would flag identifiers Java never
//!   flags, so [`is_issn_shape`] / [`is_isbn_shape`] are published as shape
//!   predicates only and no lane consumes them.
//! - **Java returns every match; this module returns maximal spans.** For one
//!   start position the Java trie emits a match for every term-ending prefix; a
//!   span with the same start and a shorter end is fully covered by the longest
//!   one, and `addFeaturesCitation` consumes the spans only through the
//!   inclusive coverage predicate, so the reduction is coverage-equivalent while
//!   being deterministic and duplicate-free.
//! - **Phrase keys are re-sorted here.** `LexiconBundle::validate` guarantees the
//!   raw *entry* strings are ordered and unique, but phrase keys are built from
//!   normalised, delimiter-stripped tokens and therefore need not inherit that
//!   order. This module sorts its own key space before binary-searching it.
//! - **Byte offsets, not UTF-16 code units.** Java indexes `String` (UTF-16)
//!   characters; every shape here is ASCII, so byte and code-unit offsets agree
//!   on every input the shapes can match, and the token mapping is exact either
//!   way.
//! - **Bounded normalisation.** `UnicodeUtil.normaliseText` performs a wider
//!   Unicode normalisation (dash punctuation, low lines, vertical lines, ...).
//!   Only whitespace → `' '` and the dash family → `'-'` are reproduced: those
//!   are the two classes that decide token boundaries and the phrase keys.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! regex, no absolute path, no model bytes, no real lexicon bytes. Nothing here
//! is spliced into the production parse route; `import_eligible` stays false and
//! the band is candidate-only.

use crate::native_feature_facts::BIBLIOGRAPHY_FULL_PUNCTUATIONS;
use da_ports::{LexiconBundle, LexiconChannel};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema identity for the typed positional gazetteer layer.
pub const NATIVE_BIBLIOGRAPHY_GAZETTEER_SCHEMA_VERSION: &str =
    "da-parse.native-bibliography-gazetteer/v1";

/// `TextUtilities.delimiters` head; the full set appends `fullPunctuations`.
pub const GAZETTEER_DELIMITERS_HEAD: &str = "\n\r\t\u{c}\u{a0}\u{200c}";

/// Longest phrase (in significant tokens) a single gazetteer entry may occupy.
pub const GAZETTEER_MAX_PHRASE_TOKENS: usize = 24;

/// Longest input token list admitted by one gazetteer pass.
pub const GAZETTEER_MAX_TOKENS: usize = 16_384;

/// Entries admitted per lane.
///
/// Deliberately tighter than the provider cap
/// (`da_domain::MAX_LEXICON_ENTRIES_PER_CHANNEL` = 100_000) so a shadow-band
/// call cannot be driven into a multi-hundred-megabyte key table.
pub const GAZETTEER_MAX_ENTRIES_PER_LANE: usize = 65_536;

/// Spans admitted per lane.
pub const GAZETTEER_MAX_SPANS: usize = 65_536;

/// Concatenated token text admitted by a pattern-lane scan.
pub const GAZETTEER_MAX_TEXT_BYTES: usize = 1_048_576;

/// The eight gazetteer lanes, in the Java declaration order of
/// `CitationParser.processingLayoutTokenMultiple`.
///
/// The first six are dictionary lanes (`FastMatcher` over a lexicon channel);
/// the last two are pattern lanes (`TextUtilities` shape patterns).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum BibliographyGazetteerLane {
    /// `tokenPositionsJournalNames`.
    #[default]
    JournalNames,
    /// `tokenPositionsAbbrevJournalNames`.
    AbbrevJournalNames,
    /// `tokenPositionsConferenceNames`.
    ConferenceNames,
    /// `tokenPositionsPublisherNames`.
    PublisherNames,
    /// `tokenPositionsLocationNames`.
    LocationNames,
    /// `tokenPositionsCollaborationNames`.
    CollaborationNames,
    /// `tokenPositionsIdentifierPattern` (DOI + arXiv shapes).
    IdentifierPattern,
    /// `tokenPositionsUrlPattern` (`urlPattern1` shape).
    UrlPattern,
}

impl BibliographyGazetteerLane {
    /// All eight lanes in Java order.
    pub const ALL: [Self; 8] = [
        Self::JournalNames,
        Self::AbbrevJournalNames,
        Self::ConferenceNames,
        Self::PublisherNames,
        Self::LocationNames,
        Self::CollaborationNames,
        Self::IdentifierPattern,
        Self::UrlPattern,
    ];

    /// Dense index used to key per-lane storage.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The lane name as used for the lexicon channel key and span identity.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JournalNames => "JournalNames",
            Self::AbbrevJournalNames => "AbbrevJournalNames",
            Self::ConferenceNames => "ConferenceNames",
            Self::PublisherNames => "PublisherNames",
            Self::LocationNames => "LocationNames",
            Self::CollaborationNames => "CollaborationNames",
            Self::IdentifierPattern => "IdentifierPattern",
            Self::UrlPattern => "UrlPattern",
        }
    }

    /// The Java position-list field this lane corresponds to.
    pub const fn java_position_field(self) -> &'static str {
        match self {
            Self::JournalNames => "tokenPositionsJournalNames",
            Self::AbbrevJournalNames => "tokenPositionsAbbrevJournalNames",
            Self::ConferenceNames => "tokenPositionsConferenceNames",
            Self::PublisherNames => "tokenPositionsPublisherNames",
            Self::LocationNames => "tokenPositionsLocationNames",
            Self::CollaborationNames => "tokenPositionsCollaborationNames",
            Self::IdentifierPattern => "tokenPositionsIdentifierPattern",
            Self::UrlPattern => "tokenPositionsUrlPattern",
        }
    }

    /// `true` for the two lanes fed by shape rules instead of a channel.
    pub const fn is_pattern(self) -> bool {
        matches!(self, Self::IdentifierPattern | Self::UrlPattern)
    }

    /// Typed lane-name parse; alphanumeric and case insensitive, so
    /// `JournalNames`, `journal_names` and `journalnames` are one lane.
    pub fn from_name(name: &str) -> Result<Self, BibliographyGazetteerError> {
        Self::ALL
            .into_iter()
            .find(|lane| channel_key_matches(lane.as_str(), name))
            .ok_or_else(|| BibliographyGazetteerError::UnknownLane {
                name: name.to_string(),
            })
    }
}

/// A half-open-inclusive positional span over the *input token list*.
///
/// `start` and `end` are indices into the token list that was scanned — the same
/// index space `addFeaturesCitation` iterates — so the span covers `start`,
/// `end`, every token between them, and (because the Java walk skips separators
/// without resetting) any delimiter or space token in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GazetteerSpan {
    pub lane: BibliographyGazetteerLane,
    pub start: usize,
    pub end: usize,
}

impl GazetteerSpan {
    /// `CitationParser` / `addFeaturesCitation` inclusive coverage: `start <= n <= end`.
    pub const fn covers(self, n: usize) -> bool {
        self.start <= n && n <= self.end
    }
}

/// The distinct consumption rule of the positional gazetteer.
///
/// This is the whole reason the module exists: membership (`binary_search` on
/// one token) is a different predicate, and the ABI columns are set from *this*
/// one.
pub fn span_covers(spans: &[GazetteerSpan], n: usize) -> bool {
    spans.iter().any(|span| span.covers(n))
}

/// Per-lane spans produced by one gazetteer pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BibliographyGazetteerSpans {
    by_lane: [Vec<GazetteerSpan>; 8],
}

impl BibliographyGazetteerSpans {
    /// Spans for one lane, ordered by `(start, end)`.
    pub fn for_lane(&self, lane: BibliographyGazetteerLane) -> &[GazetteerSpan] {
        &self.by_lane[lane.index()]
    }

    /// `span_covers` for one lane.
    pub fn covers(&self, lane: BibliographyGazetteerLane, n: usize) -> bool {
        span_covers(self.for_lane(lane), n)
    }

    /// Span count for one lane.
    pub fn lane_len(&self, lane: BibliographyGazetteerLane) -> usize {
        self.by_lane[lane.index()].len()
    }

    /// Spans across every lane.
    pub fn len(&self) -> usize {
        self.by_lane.iter().map(Vec::len).sum()
    }

    /// `true` when no lane produced a span.
    pub fn is_empty(&self) -> bool {
        self.by_lane.iter().all(Vec::is_empty)
    }

    /// Every span, lane by lane in Java order.
    pub fn iter(&self) -> impl Iterator<Item = &GazetteerSpan> {
        self.by_lane.iter().flatten()
    }
}

/// Precomputed phrase keys for the six dictionary lanes.
///
/// Building the key table once and reusing it across citation blocks is the
/// point: the provider bundle is a validated, immutable resource, while the
/// per-block cost must stay proportional to the block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BibliographyGazetteerIndex {
    lanes: [Vec<String>; 8],
}

impl BibliographyGazetteerIndex {
    /// Build the key table from a validated lexicon bundle.
    ///
    /// The bundle is validated first because the phrase-key lookup relies on a
    /// well-formed provider surface (declared versions, ordered and unique
    /// channel keys, non-empty entries).
    pub fn from_lexicon(lexicon: &LexiconBundle) -> Result<Self, BibliographyGazetteerError> {
        lexicon
            .validate()
            .map_err(|error| BibliographyGazetteerError::InvalidLexicon(error.to_string()))?;
        let delimiters = gazetteer_delimiters();
        let mut lanes: [Vec<String>; 8] = Default::default();
        for lane in BibliographyGazetteerLane::ALL {
            if lane.is_pattern() {
                continue;
            }
            let channel = lexicon
                .channels
                .iter()
                .find(|channel| channel_key_matches(&channel.key, lane.as_str()));
            if let Some(channel) = channel {
                lanes[lane.index()] = lane_keys(channel, lane, &delimiters)?;
            }
        }
        Ok(Self { lanes })
    }

    /// Normalised, sorted, de-duplicated phrase keys of one dictionary lane.
    pub fn lane_keys(&self, lane: BibliographyGazetteerLane) -> &[String] {
        &self.lanes[lane.index()]
    }

    /// Phrase keys for a lane, encoded as the channel entries they came from.
    ///
    /// A lane with no matching channel yields no spans — typed missingness, not
    /// an error and never a fabricated flag.
    fn phrase_spans(
        &self,
        lane: BibliographyGazetteerLane,
        significant: &[SignificantToken],
    ) -> Vec<GazetteerSpan> {
        let keys = &self.lanes[lane.index()];
        if keys.is_empty() || significant.is_empty() {
            return Vec::new();
        }
        let mut candidates: Vec<(usize, usize)> = Vec::new();
        let mut phrase = String::new();
        for (offset, first) in significant.iter().enumerate() {
            let mut window = 0usize;
            for token in &significant[offset..] {
                if token.segment != first.segment {
                    break;
                }
                window += 1;
                if window == GAZETTEER_MAX_PHRASE_TOKENS {
                    break;
                }
            }
            phrase.clear();
            for length in 1..=window {
                let token = &significant[offset + length - 1];
                if length > 1 {
                    phrase.push(' ');
                }
                phrase.push_str(&token.key);
                if keys
                    .binary_search_by(|probe| probe.as_str().cmp(phrase.as_str()))
                    .is_ok()
                {
                    candidates.push((first.index, token.index));
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        // Keep the maximal span per start: the Java walk emits every
        // term-ending prefix, and all of them are covered by the longest one.
        let mut spans = Vec::new();
        let mut cursor = 0usize;
        while cursor < candidates.len() {
            let start = candidates[cursor].0;
            let mut end = candidates[cursor].1;
            let mut probe = cursor + 1;
            while probe < candidates.len() && candidates[probe].0 == start {
                end = end.max(candidates[probe].1);
                probe += 1;
            }
            spans.push(GazetteerSpan { lane, start, end });
            cursor = probe;
        }
        spans
    }

    /// All eight lanes over one token list.
    ///
    /// Token texts are read as-is; classification (transparent / breaker /
    /// significant) is lane independent and therefore computed once.
    pub fn spans<S: AsRef<str>>(
        &self,
        tokens: &[S],
    ) -> Result<BibliographyGazetteerSpans, BibliographyGazetteerError> {
        if tokens.len() > GAZETTEER_MAX_TOKENS {
            return Err(BibliographyGazetteerError::TooManyTokens {
                actual: tokens.len(),
                maximum: GAZETTEER_MAX_TOKENS,
            });
        }
        let delimiters = gazetteer_delimiters();
        let mut significant: Vec<SignificantToken> = Vec::new();
        let mut segment = 0usize;
        for (index, token) in tokens.iter().enumerate() {
            let raw = token.as_ref();
            if is_transparent(raw, &delimiters) {
                continue;
            }
            let key = normalize_gazetteer_text(raw);
            if key.is_empty() || key.chars().any(char::is_whitespace) {
                // A token that normalises to whitespace ends any open phrase in
                // Java (the trie lookup misses and the chain is dropped); a
                // separator token, by contrast, is skipped and does not.
                segment += 1;
                continue;
            }
            significant.push(SignificantToken {
                index,
                key,
                segment,
            });
        }
        let mut by_lane: [Vec<GazetteerSpan>; 8] = Default::default();
        for lane in BibliographyGazetteerLane::ALL {
            let spans = if lane.is_pattern() {
                pattern_spans(lane, tokens)?
            } else {
                self.phrase_spans(lane, &significant)
            };
            check_span_limit(lane, spans.len())?;
            by_lane[lane.index()] = spans;
        }
        Ok(BibliographyGazetteerSpans { by_lane })
    }
}

/// One gazetteer pass: build the key table, then scan.
///
/// Convenience for callers that do not reuse the bundle across blocks; callers
/// that do should hold a [`BibliographyGazetteerIndex`].
pub fn gazetteer_spans<S: AsRef<str>>(
    lexicon: &LexiconBundle,
    tokens: &[S],
) -> Result<BibliographyGazetteerSpans, BibliographyGazetteerError> {
    BibliographyGazetteerIndex::from_lexicon(lexicon)?.spans(tokens)
}

/// `true` for an `ISSN`-shaped value: `dddd-ddd` plus a digit or `X` check
/// character.
///
/// A shape rule, not a source-bound GROBID pattern: `Lexicon`'s `ISSN`
/// position list is an empty stub upstream and no lane consumes this.
pub fn is_issn_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 9
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..8].iter().all(u8::is_ascii_digit)
        && (bytes[8].is_ascii_digit() || bytes[8] == b'X' || bytes[8] == b'x')
}

/// `true` for an `ISBN`-shaped value: 10 characters (last one a digit or `X`)
/// or 13 digits, with `-` and space separators allowed.
///
/// A shape rule, not a source-bound GROBID pattern: `Lexicon`'s `ISBN`
/// position list is an empty stub upstream and no lane consumes this.
pub fn is_isbn_shape(value: &str) -> bool {
    let digits: Vec<u8> = value
        .bytes()
        .filter(|byte| *byte != b'-' && *byte != b' ')
        .collect();
    match digits.len() {
        10 => {
            digits[..9].iter().all(u8::is_ascii_digit)
                && (digits[9].is_ascii_digit() || digits[9] == b'X' || digits[9] == b'x')
        }
        13 => digits.iter().all(u8::is_ascii_digit),
        _ => false,
    }
}

/// Typed refusal vocabulary for the positional gazetteer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibliographyGazetteerError {
    /// A lane name that is not one of the eight Java lanes.
    UnknownLane { name: String },
    /// The input token list exceeds the admitted bound.
    TooManyTokens { actual: usize, maximum: usize },
    /// A lane's channel declares more entries than admitted.
    TooManyEntries {
        lane: BibliographyGazetteerLane,
        actual: usize,
        maximum: usize,
    },
    /// A channel entry expands to more significant tokens than admitted.
    PhraseTooLong {
        lane: BibliographyGazetteerLane,
        entry: usize,
        tokens: usize,
        maximum: usize,
    },
    /// A lane produced more spans than admitted.
    TooManySpans {
        lane: BibliographyGazetteerLane,
        actual: usize,
        maximum: usize,
    },
    /// The concatenated token text exceeds the admitted bound.
    TooManyTextBytes { actual: usize, maximum: usize },
    /// The provider bundle failed its own validation.
    InvalidLexicon(String),
}

impl fmt::Display for BibliographyGazetteerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLane { name } => write!(f, "unknown bibliography gazetteer lane: {name}"),
            Self::TooManyTokens { actual, maximum } => {
                write!(f, "gazetteer input has {actual} tokens; maximum {maximum}")
            }
            Self::TooManyEntries {
                lane,
                actual,
                maximum,
            } => write!(
                f,
                "gazetteer lane {} declares {actual} entries; maximum {maximum}",
                lane.as_str()
            ),
            Self::PhraseTooLong {
                lane,
                entry,
                tokens,
                maximum,
            } => write!(
                f,
                "gazetteer lane {} entry {entry} has {tokens} tokens; maximum {maximum}",
                lane.as_str()
            ),
            Self::TooManySpans {
                lane,
                actual,
                maximum,
            } => write!(
                f,
                "gazetteer lane {} produced {actual} spans; maximum {maximum}",
                lane.as_str()
            ),
            Self::TooManyTextBytes { actual, maximum } => write!(
                f,
                "gazetteer pattern input has {actual} bytes; maximum {maximum}"
            ),
            Self::InvalidLexicon(error) => write!(f, "lexicon bundle is invalid: {error}"),
        }
    }
}

impl std::error::Error for BibliographyGazetteerError {}

/// One significant (non-separator, non-emitting) token of the scanned list.
struct SignificantToken {
    /// Index in the original token list — the index space `span_covers` reads.
    index: usize,
    /// Normalised, lowercased phrase key component.
    key: String,
    /// Phrase segment; a breaker token ends the segment, a separator does not.
    segment: usize,
}

/// `TextUtilities.delimiters` = the separator head plus `fullPunctuations`.
fn gazetteer_delimiters() -> String {
    let mut delimiters = String::with_capacity(
        GAZETTEER_DELIMITERS_HEAD.len() + BIBLIOGRAPHY_FULL_PUNCTUATIONS.len(),
    );
    delimiters.push_str(GAZETTEER_DELIMITERS_HEAD);
    delimiters.push_str(BIBLIOGRAPHY_FULL_PUNCTUATIONS);
    delimiters
}

/// The Java transparency test: a space, a newline, or any delimiter substring.
///
/// `str::contains("")` is `true`, matching `String.indexOf("") == 0`, so an
/// empty token is a separator in both implementations.
fn is_transparent(raw: &str, delimiters: &str) -> bool {
    raw == " " || raw == "\n" || delimiters.contains(raw)
}

/// Bounded `UnicodeUtil.normaliseText`: whitespace → `' '`, dash family → `'-'`,
/// then lowercase.
///
/// Spaces are *kept* here, as upstream: the caller decides whether a token
/// containing whitespace can participate in a phrase (it cannot).
fn normalize_gazetteer_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars() {
        let mapped = if character.is_whitespace() {
            ' '
        } else if is_dash_punctuation(character) {
            '-'
        } else {
            character
        };
        normalized.extend(mapped.to_lowercase());
    }
    normalized
}

/// The dash characters `UnicodeUtil.DASH_PATTERN` normalises, restricted to the
/// families that can appear in bibliographic token text.
fn is_dash_punctuation(character: char) -> bool {
    matches!(
        character,
        '\u{2010}'
            ..='\u{2015}' // hyphen .. horizontal bar
            | '\u{2212}'        // minus sign
            | '\u{2e3a}' | '\u{2e3b}' // two/three-em dash
            | '\u{fe58}' | '\u{fe63}' | '\u{ff0d}' // small/vertical/fullwidth hyphen-minus
            | '\u{2043}' // hyphen bullet
            | '\u{fe31}' | '\u{fe32}' // vertical em/en dash
    )
}

/// Split a channel entry into normalised phrase-key tokens, as
/// `FastMatcher.loadTerm` does after its analyzer tokenisation: separators are
/// dropped, the rest is lowercased.
fn phrase_tokens(entry: &str, delimiters: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in entry.chars() {
        let mapped = if character.is_whitespace() {
            ' '
        } else if is_dash_punctuation(character) {
            '-'
        } else {
            character
        };
        if mapped == ' ' || delimiters.contains(mapped) {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.extend(mapped.to_lowercase());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn lane_keys(
    channel: &LexiconChannel,
    lane: BibliographyGazetteerLane,
    delimiters: &str,
) -> Result<Vec<String>, BibliographyGazetteerError> {
    if channel.entries.len() > GAZETTEER_MAX_ENTRIES_PER_LANE {
        return Err(BibliographyGazetteerError::TooManyEntries {
            lane,
            actual: channel.entries.len(),
            maximum: GAZETTEER_MAX_ENTRIES_PER_LANE,
        });
    }
    let mut keys = Vec::with_capacity(channel.entries.len());
    for (entry, text) in channel.entries.iter().enumerate() {
        let tokens = phrase_tokens(text, delimiters);
        if tokens.is_empty() {
            // `FastMatcher.loadTerm` adds nothing for an all-delimiter entry.
            continue;
        }
        if tokens.len() > GAZETTEER_MAX_PHRASE_TOKENS {
            return Err(BibliographyGazetteerError::PhraseTooLong {
                lane,
                entry,
                tokens: tokens.len(),
                maximum: GAZETTEER_MAX_PHRASE_TOKENS,
            });
        }
        keys.push(tokens.join(" "));
    }
    // The provider orders raw entries; normalisation can reorder them, so the
    // key space is ordered here before it is binary-searched.
    keys.sort_unstable();
    keys.dedup();
    Ok(keys)
}

fn channel_key_matches(actual: &str, expected: &str) -> bool {
    normalize_channel_key(actual) == normalize_channel_key(expected)
}

fn normalize_channel_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn check_span_limit(
    lane: BibliographyGazetteerLane,
    actual: usize,
) -> Result<(), BibliographyGazetteerError> {
    if actual > GAZETTEER_MAX_SPANS {
        return Err(BibliographyGazetteerError::TooManySpans {
            lane,
            actual,
            maximum: GAZETTEER_MAX_SPANS,
        });
    }
    Ok(())
}

/// Character-offset spans of a pattern lane, mapped onto token indices.
fn pattern_spans<S: AsRef<str>>(
    lane: BibliographyGazetteerLane,
    tokens: &[S],
) -> Result<Vec<GazetteerSpan>, BibliographyGazetteerError> {
    let mut text = String::new();
    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(tokens.len());
    for token in tokens {
        let start = text.len();
        text.push_str(token.as_ref());
        ranges.push((start, text.len()));
    }
    if text.len() > GAZETTEER_MAX_TEXT_BYTES {
        return Err(BibliographyGazetteerError::TooManyTextBytes {
            actual: text.len(),
            maximum: GAZETTEER_MAX_TEXT_BYTES,
        });
    }
    let mut spans = Vec::new();
    for (match_start, match_end) in scan_pattern(lane, &text) {
        let start = ranges.partition_point(|(_, end)| *end <= match_start);
        let Some(back) = ranges
            .partition_point(|(token_start, _)| *token_start < match_end)
            .checked_sub(1)
        else {
            continue;
        };
        if start <= back {
            spans.push(GazetteerSpan {
                lane,
                start,
                end: back,
            });
        }
    }
    spans.sort_unstable();
    spans.dedup();
    Ok(spans)
}

fn scan_pattern(lane: BibliographyGazetteerLane, text: &str) -> Vec<(usize, usize)> {
    match lane {
        BibliographyGazetteerLane::IdentifierPattern => {
            let mut matches = scan_doi(text);
            matches.extend(scan_arxiv(text));
            matches.sort_unstable();
            matches.dedup();
            matches
        }
        BibliographyGazetteerLane::UrlPattern => scan_url(text),
        _ => Vec::new(),
    }
}

/// `TextUtilities.DOIPattern` = `(10\.\d{4,5}\/[\S]+[^;,.\s])`, linearised.
///
/// `[\S]+` is greedy and the final class rejects `;`, `,` and `.`, so the match
/// ends at the last character of the non-space run that is not one of those.
fn scan_doi(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut matches = Vec::new();
    if bytes.len() < 3 {
        return matches;
    }
    for start in 0..=bytes.len() - 3 {
        if &bytes[start..start + 3] != b"10." {
            continue;
        }
        let digits_start = start + 3;
        let mut digits_end = digits_start;
        while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
            digits_end += 1;
        }
        let digits = digits_end - digits_start;
        if !(4..=5).contains(&digits) || digits_end >= bytes.len() || bytes[digits_end] != b'/' {
            continue;
        }
        let run_start = digits_end + 1;
        let mut run_end = run_start;
        while run_end < bytes.len() && !bytes[run_end].is_ascii_whitespace() {
            run_end += 1;
        }
        if run_end - run_start < 2 {
            continue;
        }
        let mut end = run_end;
        while end > run_start && matches!(bytes[end - 1], b';' | b',' | b'.') {
            end -= 1;
        }
        if end > run_start {
            matches.push((start, end));
        }
    }
    matches
}

/// `TextUtilities.arXivPattern`, linearised: either the `NNNN.NNNNN` form or the
/// `subject/NNNNNNN` form, both behind a literal case-sensitive `arXiv` prefix.
fn scan_arxiv(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut matches = Vec::new();
    let mut cursor = 0usize;
    while cursor + 5 <= bytes.len() {
        if &bytes[cursor..cursor + 5] != b"arXiv" {
            cursor += 1;
            continue;
        }
        let Some(end) = arxiv_form(bytes, cursor + 5) else {
            cursor += 1;
            continue;
        };
        matches.push((cursor, end));
        cursor = end;
    }
    matches
}

fn arxiv_form(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    cursor = skip_optional_space(bytes, cursor);
    if bytes[cursor..].starts_with(b".org") {
        cursor = skip_optional_space(bytes, cursor + 4);
    }
    if cursor >= bytes.len() || bytes[cursor] != b':' {
        return None;
    }
    cursor = skip_optional_space(bytes, cursor + 1);
    if let Some(end) = arxiv_dotted(bytes, cursor) {
        return Some(end);
    }
    arxiv_slashed(bytes, cursor)
}

/// `\d{4}\s?\.\s?\d{4,5}(v\d+)?`
fn arxiv_dotted(bytes: &[u8], start: usize) -> Option<usize> {
    if start + 4 > bytes.len() || !bytes[start..start + 4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut cursor = skip_optional_space(bytes, start + 4);
    if cursor >= bytes.len() || bytes[cursor] != b'.' {
        return None;
    }
    cursor = skip_optional_space(bytes, cursor + 1);
    let mut digits = 0usize;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() && digits < 5 {
        digits += 1;
        cursor += 1;
    }
    if digits < 4 {
        return None;
    }
    Some(arxiv_version(bytes, cursor))
}

/// `[ a-zA-Z\-\.]*\s?/\s?\d{7}(v\d+)?`
fn arxiv_slashed(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    while cursor < bytes.len() && is_arxiv_subject_byte(bytes[cursor]) {
        cursor += 1;
    }
    let slash = cursor;
    if slash >= bytes.len() || bytes[slash] != b'/' {
        return None;
    }
    cursor = skip_optional_space(bytes, slash + 1);
    let mut digits = 0usize;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() && digits < 7 {
        digits += 1;
        cursor += 1;
    }
    if digits != 7 {
        return None;
    }
    Some(arxiv_version(bytes, cursor))
}

fn arxiv_version(bytes: &[u8], cursor: usize) -> usize {
    if cursor < bytes.len() && bytes[cursor] == b'v' {
        let mut probe = cursor + 1;
        while probe < bytes.len() && bytes[probe].is_ascii_digit() {
            probe += 1;
        }
        if probe > cursor + 1 {
            return probe;
        }
    }
    cursor
}

fn is_arxiv_subject_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b' ' | b'-' | b'.')
}

/// `TextUtilities.urlPattern1`: `(?i)(https?|ftp)\s{0,2}:\s{0,2}//\s{0,2}<run>`
/// or `(?i)www\s{0,2}\.\s{0,2}<run>`.
fn scan_url(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut matches = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let found = scheme_url(bytes, cursor).or_else(|| www_url(bytes, cursor));
        match found {
            Some((start, end)) => {
                matches.push((start, end));
                cursor = end.max(cursor + 1);
            }
            None => cursor += 1,
        }
    }
    matches
}

fn scheme_url(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let scheme_len = ["https", "http", "ftp"]
        .into_iter()
        .find(|scheme| starts_with_ignore_ascii_case(&bytes[start..], scheme.as_bytes()))?
        .len();
    let mut cursor = skip_up_to_two_spaces(bytes, start + scheme_len);
    if cursor >= bytes.len() || bytes[cursor] != b':' {
        return None;
    }
    cursor = skip_up_to_two_spaces(bytes, cursor + 1);
    if !bytes[cursor..].starts_with(b"//") {
        return None;
    }
    cursor = skip_up_to_two_spaces(bytes, cursor + 2);
    url_run_end(bytes, cursor).map(|end| (start, end))
}

fn www_url(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if !starts_with_ignore_ascii_case(&bytes[start..], b"www") {
        return None;
    }
    let mut cursor = skip_up_to_two_spaces(bytes, start + 3);
    if cursor >= bytes.len() || bytes[cursor] != b'.' {
        return None;
    }
    cursor = skip_up_to_two_spaces(bytes, cursor + 1);
    url_run_end(bytes, cursor).map(|end| (start, end))
}

/// The trailing `[-A-Z0-9+&@#/%=~_()|]` of `urlPattern1`: a maximal run of the
/// wider set whose end is trimmed back to the last character of the narrower one.
fn url_run_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut run_end = start;
    while run_end < bytes.len() && is_url_byte(bytes[run_end]) {
        run_end += 1;
    }
    if run_end == start {
        return None;
    }
    let mut end = run_end;
    while end > start && !is_url_final_byte(bytes[end - 1]) {
        end -= 1;
    }
    if end > start { Some(end) } else { None }
}

fn is_url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'+'
                | b'&'
                | b'@'
                | b'#'
                | b'/'
                | b'%'
                | b'?'
                | b'='
                | b'~'
                | b'_'
                | b'('
                | b')'
                | b'|'
                | b'!'
                | b':'
                | b'.'
                | b';'
        )
}

fn is_url_final_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'+'
                | b'&'
                | b'@'
                | b'#'
                | b'/'
                | b'%'
                | b'='
                | b'~'
                | b'_'
                | b'('
                | b')'
                | b'|'
        )
}

/// Java `\s?` — at most one `[ \t\n\x0B\f\r]`.
fn skip_optional_space(bytes: &[u8], cursor: usize) -> usize {
    if cursor < bytes.len() && is_java_space(bytes[cursor]) {
        cursor + 1
    } else {
        cursor
    }
}

/// Java `\s{0,2}`.
fn skip_up_to_two_spaces(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    let mut skipped = 0usize;
    while cursor < bytes.len() && skipped < 2 && is_java_space(bytes[cursor]) {
        cursor += 1;
        skipped += 1;
    }
    cursor
}

fn is_java_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn starts_with_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::FeatureTemplate;

    fn lexicon(channels: &[(&str, &[&str])]) -> LexiconBundle {
        let mut built: Vec<LexiconChannel> = channels
            .iter()
            .map(|(key, entries)| {
                let mut entries: Vec<String> =
                    entries.iter().map(|entry| (*entry).to_string()).collect();
                // `LexiconBundle::validate` requires strictly ordered entries; the
                // helper sorts so call sites can list entries in reading order.
                entries.sort();
                LexiconChannel {
                    key: (*key).to_string(),
                    entries,
                }
            })
            .collect();
        built.sort_by(|left, right| left.key.cmp(&right.key));
        let channel_keys = vec![built[0].key.clone()];
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: built,
            feature_templates: vec![FeatureTemplate {
                key: "bibliography".into(),
                channel_keys,
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }

    fn index(channels: &[(&str, &[&str])]) -> BibliographyGazetteerIndex {
        BibliographyGazetteerIndex::from_lexicon(&lexicon(channels)).unwrap()
    }

    fn journal_index() -> BibliographyGazetteerIndex {
        index(&[("JournalNames", &["New York Times", "Nature"])])
    }

    #[test]
    fn lane_vocabulary_matches_the_java_order_and_spelling() {
        let names: Vec<&str> = BibliographyGazetteerLane::ALL
            .iter()
            .map(|lane| lane.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "JournalNames",
                "AbbrevJournalNames",
                "ConferenceNames",
                "PublisherNames",
                "LocationNames",
                "CollaborationNames",
                "IdentifierPattern",
                "UrlPattern",
            ]
        );
        assert_eq!(
            BibliographyGazetteerLane::JournalNames.java_position_field(),
            "tokenPositionsJournalNames"
        );
        assert_eq!(
            BibliographyGazetteerLane::UrlPattern.java_position_field(),
            "tokenPositionsUrlPattern"
        );
        for (position, lane) in BibliographyGazetteerLane::ALL.iter().enumerate() {
            assert_eq!(lane.index(), position);
        }
        assert!(BibliographyGazetteerLane::IdentifierPattern.is_pattern());
        assert!(BibliographyGazetteerLane::UrlPattern.is_pattern());
        assert!(!BibliographyGazetteerLane::JournalNames.is_pattern());
    }

    #[test]
    fn lane_names_parse_typed_and_reject_unknown() {
        assert_eq!(
            BibliographyGazetteerLane::from_name("JournalNames").unwrap(),
            BibliographyGazetteerLane::JournalNames
        );
        assert_eq!(
            BibliographyGazetteerLane::from_name("journal_names").unwrap(),
            BibliographyGazetteerLane::JournalNames
        );
        assert_eq!(
            BibliographyGazetteerLane::from_name("tokenPositionsUrlPattern").unwrap_err(),
            BibliographyGazetteerError::UnknownLane {
                name: "tokenPositionsUrlPattern".into()
            }
        );
        assert!(matches!(
            BibliographyGazetteerLane::from_name("Books"),
            Err(BibliographyGazetteerError::UnknownLane { .. })
        ));
    }

    #[test]
    fn multi_token_phrase_covers_every_position_in_its_span() {
        let spans = journal_index()
            .spans(&["New", "York", "Times", "was", "cited"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(
            journal,
            &[GazetteerSpan {
                lane: BibliographyGazetteerLane::JournalNames,
                start: 0,
                end: 2,
            }]
        );
        assert!(spans.covers(BibliographyGazetteerLane::JournalNames, 0));
        assert!(spans.covers(BibliographyGazetteerLane::JournalNames, 1));
        assert!(spans.covers(BibliographyGazetteerLane::JournalNames, 2));
        assert!(!spans.covers(BibliographyGazetteerLane::JournalNames, 3));
        assert!(!spans.covers(BibliographyGazetteerLane::JournalNames, 4));
    }

    #[test]
    fn phrase_matches_across_separator_tokens_which_membership_can_never_do() {
        // "New , York" — the delimiter token keeps its index inside the span,
        // exactly as the Java `start <= n <= end` scan would report it.
        let spans = index(&[("JournalNames", &["New York"])])
            .spans(&["New", ",", " ", "York"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(journal.len(), 1);
        assert_eq!((journal[0].start, journal[0].end), (0, 3));
        // Coverage, not membership: no single token equals "new york".
        assert!(journal[0].covers(1));
        assert!(journal[0].covers(2));
        assert!(span_covers(journal, 3));
        assert!(!span_covers(journal, 4));
    }

    #[test]
    fn span_covers_is_inclusive_on_both_ends() {
        let span = GazetteerSpan {
            lane: BibliographyGazetteerLane::PublisherNames,
            start: 4,
            end: 6,
        };
        assert!(!span.covers(3));
        assert!(span.covers(4));
        assert!(span.covers(5));
        assert!(span.covers(6));
        assert!(!span.covers(7));
        assert!(!span_covers(&[], 5));
        assert!(span_covers(&[span], 5));
    }

    #[test]
    fn non_matching_and_prefix_only_tokens_produce_no_spans() {
        let spans = journal_index()
            .spans(&["Journal", "of", "Stuff", "Nat", "York"])
            .unwrap();
        assert!(spans.is_empty());
        assert_eq!(spans.len(), 0);
        // A sub-phrase is not a membership back door either.
        assert!(!spans.covers(BibliographyGazetteerLane::JournalNames, 0));
        assert!(!spans.covers(BibliographyGazetteerLane::JournalNames, 3));
    }

    #[test]
    fn the_longest_phrase_wins_for_one_start() {
        let spans = index(&[("JournalNames", &["New York", "New York Times"])])
            .spans(&["New", "York", "Times"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(
            journal,
            &[GazetteerSpan {
                lane: BibliographyGazetteerLane::JournalNames,
                start: 0,
                end: 2,
            }],
            "the covered positions are unchanged, so only the maximal span is kept"
        );
    }

    #[test]
    fn overlapping_phrases_from_different_starts_are_both_kept() {
        let spans = index(&[("JournalNames", &["New York", "York Times"])])
            .spans(&["New", "York", "Times"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(
            journal,
            &[
                GazetteerSpan {
                    lane: BibliographyGazetteerLane::JournalNames,
                    start: 0,
                    end: 1,
                },
                GazetteerSpan {
                    lane: BibliographyGazetteerLane::JournalNames,
                    start: 1,
                    end: 2,
                },
            ]
        );
    }

    #[test]
    fn a_token_that_normalises_to_whitespace_breaks_the_phrase() {
        // U+2008 is whitespace but is not in `delimiters`, so Java normalises it
        // to " ", misses the trie lookup and drops the open chain.
        let spans = journal_index().spans(&["New", "\u{2008}", "York"]).unwrap();
        assert!(spans.is_empty(), "a breaker token must not be jumped over");
    }

    #[test]
    fn separator_only_tokens_are_transparent_and_keep_their_index() {
        let spans = index(&[("JournalNames", &["New York"])])
            .spans(&["", " ", "\n", "New", "-", "York"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(journal.len(), 1);
        assert_eq!(
            (journal[0].start, journal[0].end),
            (3, 5),
            "the hyphen keeps index 4 inside the span"
        );
        assert!(journal[0].covers(4));
    }

    #[test]
    fn dehyphenated_and_dash_normalised_entries_meet_the_same_key() {
        let spans = index(&[("JournalNames", &["Well-Known Journal"])])
            .spans(&["Well", "Known", "Journal"])
            .unwrap();
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::JournalNames),
            1,
            "the ASCII hyphen is a delimiter in the entry and in the input"
        );
        let en_dash = index(&[("JournalNames", &["Well\u{2013}Known"])])
            .spans(&["Well", "Known"])
            .unwrap();
        assert_eq!(
            en_dash.lane_len(BibliographyGazetteerLane::JournalNames),
            1,
            "the en dash normalises to the same delimiter"
        );
    }

    #[test]
    fn every_dictionary_lane_is_served_from_its_own_channel() {
        let spans = index(&[
            ("JournalNames", &["Nature"]),
            ("ConferenceNames", &["NeurIPS"]),
            ("PublisherNames", &["Elsevier"]),
            ("LocationNames", &["Z\u{fc}rich"]),
        ])
        .spans(&["Nature", "NeurIPS", "Elsevier", "Z\u{fc}rich", "nothing"])
        .unwrap();
        assert_eq!(spans.lane_len(BibliographyGazetteerLane::JournalNames), 1);
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::ConferenceNames),
            1
        );
        assert_eq!(spans.lane_len(BibliographyGazetteerLane::PublisherNames), 1);
        assert_eq!(spans.lane_len(BibliographyGazetteerLane::LocationNames), 1);
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::AbbrevJournalNames),
            0
        );
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::CollaborationNames),
            0
        );
        assert!(spans.covers(BibliographyGazetteerLane::LocationNames, 3));
        assert!(!spans.covers(BibliographyGazetteerLane::LocationNames, 4));
    }

    #[test]
    fn a_missing_channel_is_typed_absence_not_an_error() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["Publisher", "Name"])
            .unwrap();
        assert!(spans.is_empty());
        assert!(!spans.covers(BibliographyGazetteerLane::PublisherNames, 0));
    }

    #[test]
    fn spans_are_ordered_and_duplicate_free() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["Nature", "Nature", "x"])
            .unwrap();
        let journal = spans.for_lane(BibliographyGazetteerLane::JournalNames);
        assert_eq!(journal.len(), 2);
        assert_eq!((journal[0].start, journal[0].end), (0, 0));
        assert_eq!((journal[1].start, journal[1].end), (1, 1));
        assert!(journal.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn doi_shape_span_maps_onto_the_token_run() {
        // Space tokens are separators inside the concatenated text, exactly as a
        // layout stream supplies them; without one the DOI run would swallow the
        // following token, which is also what Java does.
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["doi:", " ", "10.1000/xyz123", " ", "and"])
            .unwrap();
        let identifiers = spans.for_lane(BibliographyGazetteerLane::IdentifierPattern);
        assert_eq!(
            identifiers,
            &[GazetteerSpan {
                lane: BibliographyGazetteerLane::IdentifierPattern,
                start: 2,
                end: 2,
            }]
        );
        // The DOI is one token, so the shape must not swallow the neighbours.
        assert!(!identifiers[0].covers(0));
        assert!(!identifiers[0].covers(4));
    }

    #[test]
    fn doi_shape_spans_across_a_split_token_run() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["10.1000/", "xyz123", " ", "end"])
            .unwrap();
        let identifiers = spans.for_lane(BibliographyGazetteerLane::IdentifierPattern);
        assert_eq!(identifiers.len(), 1);
        assert_eq!((identifiers[0].start, identifiers[0].end), (0, 1));
        assert!(identifiers[0].covers(1));
        assert!(!identifiers[0].covers(3));
    }

    #[test]
    fn doi_shape_trims_the_trailing_punctuation_java_trims() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["10.1000/xyz123.,", " ", "tail"])
            .unwrap();
        let identifiers = spans.for_lane(BibliographyGazetteerLane::IdentifierPattern);
        assert_eq!(identifiers.len(), 1);
        // `[\S]+[^;,.\s]` backtracks over every trailing `; , .` in the run, so
        // the match still ends inside the same token.
        assert_eq!((identifiers[0].start, identifiers[0].end), (0, 0));
    }

    #[test]
    fn doi_shape_rejects_wrong_digit_counts_and_short_runs() {
        let reject = |text: &str| {
            index(&[("JournalNames", &["Nature"])])
                .spans(&[text])
                .unwrap()
                .lane_len(BibliographyGazetteerLane::IdentifierPattern)
        };
        assert_eq!(
            reject("10.100/xyz123"),
            0,
            "three digits is not a DOI prefix"
        );
        assert_eq!(reject("10.100000/xyz"), 0, "six digits is not a DOI prefix");
        assert_eq!(reject("10.1000/x"), 0, "the run must carry two characters");
        assert_eq!(reject("11.1000/xyz"), 0);
        assert_eq!(reject("10.1000xyz"), 0);
    }

    #[test]
    fn arxiv_shape_matches_both_java_forms() {
        let dotted = index(&[("JournalNames", &["Nature"])])
            .spans(&["arXiv:2507.19457", "x"])
            .unwrap();
        assert_eq!(
            dotted.lane_len(BibliographyGazetteerLane::IdentifierPattern),
            1
        );
        let versioned = index(&[("JournalNames", &["Nature"])])
            .spans(&["arXiv", ":", "2201.12345v2"])
            .unwrap();
        let identifiers = versioned.for_lane(BibliographyGazetteerLane::IdentifierPattern);
        assert_eq!(identifiers.len(), 1);
        assert_eq!((identifiers[0].start, identifiers[0].end), (0, 2));
        let slashed = index(&[("JournalNames", &["Nature"])])
            .spans(&["arXiv:cs.CL/0701001"])
            .unwrap();
        assert_eq!(
            slashed.lane_len(BibliographyGazetteerLane::IdentifierPattern),
            1
        );
        // The Java pattern is case sensitive.
        let cased = index(&[("JournalNames", &["Nature"])])
            .spans(&["ARXIV:2507.19457"])
            .unwrap();
        assert_eq!(
            cased.lane_len(BibliographyGazetteerLane::IdentifierPattern),
            0
        );
    }

    #[test]
    fn url_shape_matches_scheme_and_www_forms() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&[
                "https://example.org/x",
                " ",
                "www.example.org",
                " ",
                "not-a-url",
                " ",
                "ftp://files.example.org",
            ])
            .unwrap();
        let urls = spans.for_lane(BibliographyGazetteerLane::UrlPattern);
        assert_eq!(urls.len(), 3);
        assert_eq!((urls[0].start, urls[0].end), (0, 0));
        assert_eq!((urls[1].start, urls[1].end), (2, 2));
        assert_eq!((urls[2].start, urls[2].end), (6, 6));
        assert!(!urls.iter().any(|span| span.covers(4)));
    }

    #[test]
    fn url_shape_tolerates_the_spaces_java_tolerates() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["http", " ", ":", " ", "//", "example.org", " ", "tail"])
            .unwrap();
        let urls = spans.for_lane(BibliographyGazetteerLane::UrlPattern);
        assert_eq!(urls.len(), 1);
        assert_eq!((urls[0].start, urls[0].end), (0, 5));
        assert!(urls[0].covers(1));
        assert!(urls[0].covers(3));
        assert!(!urls[0].covers(7));
    }

    #[test]
    fn issn_and_isbn_shape_predicates_are_bounded_shape_rules() {
        assert!(is_issn_shape("1234-567X"));
        assert!(is_issn_shape("1234-5678"));
        assert!(!is_issn_shape("1234-567"));
        assert!(!is_issn_shape("12345-5678"));
        assert!(is_isbn_shape("0-306-40615-2"));
        assert!(is_isbn_shape("9780306406157"));
        assert!(is_isbn_shape("123456789X"));
        assert!(!is_isbn_shape("12345678901"));
        assert!(!is_isbn_shape("123456789Y"));
        // Neither predicate is wired into a lane: GROBID's ISSN/ISBN position
        // lists are empty stubs upstream.
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["1234-567X"])
            .unwrap();
        assert!(spans.is_empty());
    }

    #[test]
    fn pattern_lanes_do_not_read_the_dictionary_channels() {
        let spans = index(&[("JournalNames", &["Nature"])])
            .spans(&["10.1000/xyz123"])
            .unwrap();
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::JournalNames),
            0,
            "the DOI text is not a journal name"
        );
        assert_eq!(
            spans.lane_len(BibliographyGazetteerLane::IdentifierPattern),
            1
        );
    }

    #[test]
    fn phrase_keys_are_sorted_deduplicated_and_normalised() {
        // The provider order is preserved by the bundle (`validate` requires raw
        // entries to be strictly increasing); the key space is normalised, so
        // these two distinct entries share one key.
        let built = index(&[("JournalNames", &["New  York", "New York"])]);
        assert_eq!(
            built.lane_keys(BibliographyGazetteerLane::JournalNames),
            ["new york"]
        );
        assert!(
            built
                .lane_keys(BibliographyGazetteerLane::AbbrevJournalNames)
                .is_empty()
        );
    }

    #[test]
    fn too_many_input_tokens_is_a_typed_refusal() {
        let tokens = vec!["Nature"; GAZETTEER_MAX_TOKENS + 1];
        let error = journal_index().spans(&tokens).unwrap_err();
        assert_eq!(
            error,
            BibliographyGazetteerError::TooManyTokens {
                actual: GAZETTEER_MAX_TOKENS + 1,
                maximum: GAZETTEER_MAX_TOKENS,
            }
        );
        assert!(error.to_string().contains("tokens"));
    }

    #[test]
    fn too_many_entries_is_a_typed_refusal() {
        let entries: Vec<String> = (0..=GAZETTEER_MAX_ENTRIES_PER_LANE)
            .map(|index| format!("journal-{index:07}"))
            .collect();
        let entries: Vec<&str> = entries.iter().map(String::as_str).collect();
        let error =
            BibliographyGazetteerIndex::from_lexicon(&lexicon(&[("JournalNames", &entries[..])]))
                .unwrap_err();
        assert_eq!(
            error,
            BibliographyGazetteerError::TooManyEntries {
                lane: BibliographyGazetteerLane::JournalNames,
                actual: GAZETTEER_MAX_ENTRIES_PER_LANE + 1,
                maximum: GAZETTEER_MAX_ENTRIES_PER_LANE,
            }
        );
    }

    #[test]
    fn an_over_long_entry_is_a_typed_refusal() {
        let long = vec!["word"; GAZETTEER_MAX_PHRASE_TOKENS + 1].join(" ");
        let entries = [long.as_str()];
        let error =
            BibliographyGazetteerIndex::from_lexicon(&lexicon(&[("JournalNames", &entries[..])]))
                .unwrap_err();
        assert_eq!(
            error,
            BibliographyGazetteerError::PhraseTooLong {
                lane: BibliographyGazetteerLane::JournalNames,
                entry: 0,
                tokens: GAZETTEER_MAX_PHRASE_TOKENS + 1,
                maximum: GAZETTEER_MAX_PHRASE_TOKENS,
            }
        );
    }

    #[test]
    fn span_and_text_limits_are_enforced() {
        assert!(
            check_span_limit(BibliographyGazetteerLane::UrlPattern, GAZETTEER_MAX_SPANS).is_ok()
        );
        assert_eq!(
            check_span_limit(
                BibliographyGazetteerLane::UrlPattern,
                GAZETTEER_MAX_SPANS + 1
            )
            .unwrap_err(),
            BibliographyGazetteerError::TooManySpans {
                lane: BibliographyGazetteerLane::UrlPattern,
                actual: GAZETTEER_MAX_SPANS + 1,
                maximum: GAZETTEER_MAX_SPANS,
            }
        );
        let huge = "x".repeat(GAZETTEER_MAX_TEXT_BYTES + 1);
        let error = journal_index().spans(&[huge.as_str()]).unwrap_err();
        assert_eq!(
            error,
            BibliographyGazetteerError::TooManyTextBytes {
                actual: GAZETTEER_MAX_TEXT_BYTES + 1,
                maximum: GAZETTEER_MAX_TEXT_BYTES,
            }
        );
    }

    #[test]
    fn an_invalid_bundle_is_a_typed_refusal() {
        let mut bundle = lexicon(&[("JournalNames", &["Nature"])]);
        bundle.resource_version = "0.8.0-crf".into();
        let error = BibliographyGazetteerIndex::from_lexicon(&bundle).unwrap_err();
        assert!(matches!(
            error,
            BibliographyGazetteerError::InvalidLexicon(_)
        ));
        assert!(error.to_string().contains("lexicon bundle is invalid"));
    }

    #[test]
    fn convenience_entry_point_matches_the_index_path() {
        let bundle = lexicon(&[("JournalNames", &["New York Times"])]);
        let direct = gazetteer_spans(&bundle, &["New", "York", "Times"]).unwrap();
        let staged = index(&[("JournalNames", &["New York Times"])])
            .spans(&["New", "York", "Times"])
            .unwrap();
        assert_eq!(direct, staged);
    }

    #[test]
    fn spans_serialise_with_the_snake_case_lane_name() {
        let span = GazetteerSpan {
            lane: BibliographyGazetteerLane::AbbrevJournalNames,
            start: 1,
            end: 2,
        };
        let encoded = serde_json::to_string(&span).unwrap();
        assert_eq!(
            encoded,
            r#"{"lane":"abbrev_journal_names","start":1,"end":2}"#
        );
        assert_eq!(
            serde_json::from_str::<GazetteerSpan>(&encoded).unwrap(),
            span
        );
    }

    #[test]
    fn hostile_tokens_never_panic_and_never_match() {
        let spans = journal_index()
            .spans(&["\u{0}", "\u{1f600}", "aaaaaaaa", "--", "\u{fffd}"])
            .unwrap();
        assert!(spans.is_empty());
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    fn bundle() -> LexiconBundle {
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: vec![LexiconChannel {
                key: "JournalNames".into(),
                entries: vec!["nature".into(), "new york times".into()],
            }],
            feature_templates: vec![da_ports::FeatureTemplate {
                key: "bibliography".into(),
                channel_keys: vec!["JournalNames".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }

    fn arbitrary_tokens() -> impl Strategy<Value = Vec<String>> {
        proptest::collection::vec(
            proptest::collection::vec(any::<char>(), 0..12)
                .prop_map(|characters| characters.into_iter().collect::<String>()),
            0..32,
        )
    }

    proptest! {
        #[test]
        fn spans_are_deterministic_for_arbitrary_tokens(tokens in arbitrary_tokens()) {
            let index = BibliographyGazetteerIndex::from_lexicon(&bundle()).unwrap();
            let first = index.spans(&tokens).unwrap();
            let second = index.spans(&tokens).unwrap();
            prop_assert_eq!(&first, &second);
        }

        #[test]
        fn spans_stay_inside_the_token_list(tokens in arbitrary_tokens()) {
            let index = BibliographyGazetteerIndex::from_lexicon(&bundle()).unwrap();
            let spans = index.spans(&tokens).unwrap();
            for span in spans.iter() {
                prop_assert!(span.start <= span.end);
                prop_assert!(span.end < tokens.len());
            }
        }

        #[test]
        fn spans_are_ordered_within_a_lane(tokens in arbitrary_tokens()) {
            let index = BibliographyGazetteerIndex::from_lexicon(&bundle()).unwrap();
            let spans = index.spans(&tokens).unwrap();
            for lane in BibliographyGazetteerLane::ALL {
                let lane_spans = spans.for_lane(lane);
                for pair in lane_spans.windows(2) {
                    prop_assert!(pair[0].start <= pair[1].start);
                    prop_assert!(pair[0].end <= pair[1].end);
                }
            }
        }

        #[test]
        fn coverage_agrees_with_the_inclusive_predicate(tokens in arbitrary_tokens()) {
            let index = BibliographyGazetteerIndex::from_lexicon(&bundle()).unwrap();
            let spans = index.spans(&tokens).unwrap();
            for lane in BibliographyGazetteerLane::ALL {
                let lane_spans = spans.for_lane(lane);
                for position in 0..tokens.len() {
                    let expected = lane_spans
                        .iter()
                        .any(|span| span.start <= position && position <= span.end);
                    prop_assert_eq!(spans.covers(lane, position), expected);
                }
            }
        }
    }
}
