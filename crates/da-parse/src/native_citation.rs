//! Pure, bounded, redacting citation-field parser for shadow comparison.
//!
//! This module is deliberately not connected to `TeiLifter`, `TeiParseIr`,
//! `CitationEntry`, caches, transports, or graph persistence. It recognizes
//! only explicit scholarly field evidence and preserves the exact UTF-8 byte
//! span that supplied each value.

use serde::{Deserialize, Serialize};

use crate::{
    NativeDateOutcome, NativeDateParser, NativePersonalNameOutcome, NativePersonalNameParser,
    PersonalNameContext,
};

pub const NATIVE_CITATION_PARSER_ID: &str = "da-parse.native_citation";
pub const NATIVE_CITATION_PARSER_VERSION: &str = "1.0.0";
pub const NATIVE_CITATION_OUTCOME_SCHEMA_VERSION: &str = "da-parse.native-citation.outcome/v1";
pub const NATIVE_CITATION_INPUT_BYTE_CAP: usize = 4096;
pub const NATIVE_REFERENCE_SEGMENTER_ID: &str = "da-parse.native_reference_segmenter";
pub const NATIVE_CITATION_ANCHOR_SCANNER_ID: &str = "da-parse.native_citation_anchor_scanner";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeReferenceSegmenterLimits {
    pub total_bytes: usize,
    pub max_references: usize,
    pub max_segment_bytes: usize,
}

impl Default for NativeReferenceSegmenterLimits {
    fn default() -> Self {
        Self {
            total_bytes: 256 * 1024,
            max_references: 4096,
            max_segment_bytes: 16 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeReference {
    pub ordinal: usize,
    pub source: String,
    pub span: CitationSpan,
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeReferenceListError {
    EmptySegment,
    InvalidUtf8Boundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeReferenceListOutcome {
    Recognized { references: Vec<NativeReference> },
    Empty,
    Malformed { reason: NativeReferenceListError },
    Truncated,
    Oversized,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeReferenceSegmenter {
    limits: NativeReferenceSegmenterLimits,
}

impl NativeReferenceSegmenter {
    pub const fn new() -> Self {
        Self {
            limits: NativeReferenceSegmenterLimits {
                total_bytes: 256 * 1024,
                max_references: 4096,
                max_segment_bytes: 16 * 1024,
            },
        }
    }
    pub const fn with_limits(limits: NativeReferenceSegmenterLimits) -> Self {
        Self { limits }
    }

    pub fn segment(&self, entries: &[&str]) -> NativeReferenceListOutcome {
        let total = entries.iter().map(|entry| entry.len()).sum::<usize>();
        if total > self.limits.total_bytes {
            return NativeReferenceListOutcome::Oversized;
        }
        if entries.is_empty() || entries.iter().all(|entry| entry.trim().is_empty()) {
            return NativeReferenceListOutcome::Empty;
        }
        if entries.len() > self.limits.max_references {
            return NativeReferenceListOutcome::Truncated;
        }
        let mut references = Vec::with_capacity(entries.len());
        for (ordinal, source) in entries.iter().enumerate() {
            if source.trim().is_empty() {
                return NativeReferenceListOutcome::Malformed {
                    reason: NativeReferenceListError::EmptySegment,
                };
            }
            if source.len() > self.limits.max_segment_bytes {
                return NativeReferenceListOutcome::Oversized;
            }
            references.push(NativeReference {
                ordinal,
                source: (*source).to_owned(),
                span: CitationSpan {
                    start: 0,
                    end: source.len(),
                },
                provenance: "preserved_tei_reconstruction".to_owned(),
            });
        }
        NativeReferenceListOutcome::Recognized { references }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCitationAnchorLimits {
    pub total_bytes: usize,
    pub max_anchors: usize,
    pub max_anchor_bytes: usize,
}

impl Default for NativeCitationAnchorLimits {
    fn default() -> Self {
        Self {
            total_bytes: 512 * 1024,
            max_anchors: 16_384,
            max_anchor_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCitationAnchor {
    pub ordinal: usize,
    pub source: String,
    pub span: CitationSpan,
    pub target: Option<String>,
    pub target_present: bool,
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCitationAnchorError {
    MalformedOpeningTag,
    MissingClosingTag,
    InvalidSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeCitationAnchorOutcome {
    Recognized {
        anchors: Vec<NativeCitationAnchor>,
        adjacent_pairs: Vec<(usize, usize)>,
    },
    Empty,
    Malformed {
        reason: NativeCitationAnchorError,
    },
    Truncated,
    Oversized,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCitationAnchorScanner {
    limits: NativeCitationAnchorLimits,
}

impl NativeCitationAnchorScanner {
    pub const fn new() -> Self {
        Self {
            limits: NativeCitationAnchorLimits {
                total_bytes: 512 * 1024,
                max_anchors: 16_384,
                max_anchor_bytes: 64 * 1024,
            },
        }
    }
    pub const fn with_limits(limits: NativeCitationAnchorLimits) -> Self {
        Self { limits }
    }

    pub fn scan(&self, input: &str) -> NativeCitationAnchorOutcome {
        if input.is_empty() {
            return NativeCitationAnchorOutcome::Empty;
        }
        if input.len() > self.limits.total_bytes {
            return NativeCitationAnchorOutcome::Oversized;
        }
        let mut cursor = 0;
        let mut anchors = Vec::new();
        while let Some(relative) = input[cursor..].find("<ref") {
            let start = cursor + relative;
            let Some(open_end_rel) = input[start..].find('>') else {
                return NativeCitationAnchorOutcome::Malformed {
                    reason: NativeCitationAnchorError::MalformedOpeningTag,
                };
            };
            let open_end = start + open_end_rel;
            let opening = &input[start..=open_end];
            if !is_bibr_opening(opening) {
                cursor = open_end + 1;
                continue;
            }
            let Some(close_rel) = input[open_end + 1..].find("</ref>") else {
                return NativeCitationAnchorOutcome::Malformed {
                    reason: NativeCitationAnchorError::MissingClosingTag,
                };
            };
            let end = open_end + 1 + close_rel + "</ref>".len();
            if end - start > self.limits.max_anchor_bytes {
                return NativeCitationAnchorOutcome::Oversized;
            }
            if anchors.len() >= self.limits.max_anchors {
                return NativeCitationAnchorOutcome::Truncated;
            }
            let target = attribute(opening, "target");
            let source = &input[start..end];
            if !input.is_char_boundary(start) || !input.is_char_boundary(end) {
                return NativeCitationAnchorOutcome::Malformed {
                    reason: NativeCitationAnchorError::InvalidSpan,
                };
            }
            anchors.push(NativeCitationAnchor {
                ordinal: anchors.len(),
                source: source.to_owned(),
                span: CitationSpan { start, end },
                target_present: target.is_some(),
                target,
                provenance: "preserved_tei_reconstruction".to_owned(),
            });
            cursor = end;
        }
        if anchors.is_empty() {
            return NativeCitationAnchorOutcome::Empty;
        }
        let adjacent_pairs = anchors
            .windows(2)
            .enumerate()
            .filter_map(|(index, pair)| {
                let between = &input[pair[0].span.end..pair[1].span.start];
                between
                    .chars()
                    .all(|c| c.is_whitespace() || matches!(c, ';' | ','))
                    .then_some((index, index + 1))
            })
            .collect();
        NativeCitationAnchorOutcome::Recognized {
            anchors,
            adjacent_pairs,
        }
    }
}

fn is_bibr_opening(opening: &str) -> bool {
    let body = opening
        .strip_prefix("<ref")
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or("");
    body.split_whitespace()
        .any(|token| token == "type=\"bibr\"" || token == "type='bibr'")
}

fn attribute(opening: &str, name: &str) -> Option<String> {
    for quote in ['\"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = opening.find(&needle) {
            let value_start = start + needle.len();
            let value_end = opening[value_start..].find(quote)? + value_start;
            return Some(opening[value_start..value_end].to_owned());
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationFieldRole {
    Authors,
    PublicationDate,
    ArticleTitle,
    ContainerTitle,
    Volume,
    Issue,
    Pages,
    ArticleNumber,
    Publisher,
    Place,
    Doi,
    ArxivId,
    Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationSpan {
    pub start: usize,
    pub end: usize,
}

impl CitationSpan {
    pub const fn is_valid_for(self, input_len: usize) -> bool {
        self.start < self.end && self.end <= input_len
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationField {
    pub role: CitationFieldRole,
    pub value: String,
    pub span: CitationSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCitationMalformed {
    ControlCharacter,
    NonAsciiScript,
    UnexpectedCharacter,
    InvalidSpan,
    MissingValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCitationUnsupported {
    UnsupportedPunctuation,
    UnsupportedRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCitationAmbiguous {
    MultipleDateReadings,
    MultipleUnlabelledTitles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeCitationOutcome {
    Recognized { fields: Vec<CitationField> },
    Empty,
    Malformed { reason: NativeCitationMalformed },
    Unsupported { reason: NativeCitationUnsupported },
    Ambiguous { reason: NativeCitationAmbiguous },
    Oversized,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCitationParser;

impl NativeCitationParser {
    pub const fn new() -> Self {
        Self
    }

    /// Parses one citation into explicit, independently provenance-bound fields.
    /// The scan is linear and bounded by [`NATIVE_CITATION_INPUT_BYTE_CAP`].
    pub fn parse(&self, input: &str) -> NativeCitationOutcome {
        parse(input)
    }
}

fn malformed(reason: NativeCitationMalformed) -> NativeCitationOutcome {
    NativeCitationOutcome::Malformed { reason }
}

fn trim_span(input: &str, start: usize, end: usize) -> Option<CitationSpan> {
    let bytes = input.as_bytes();
    let mut start = start.min(end).min(bytes.len());
    let mut end = end.min(bytes.len());
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    while end > start && matches!(bytes[end - 1], b'.' | b',' | b';' | b')' | b']') {
        end -= 1;
    }
    (start < end && input.is_char_boundary(start) && input.is_char_boundary(end))
        .then_some(CitationSpan { start, end })
}

fn push_field(
    fields: &mut Vec<CitationField>,
    role: CitationFieldRole,
    input: &str,
    start: usize,
    end: usize,
) {
    if let Some(span) = trim_span(input, start, end) {
        fields.push(CitationField {
            role,
            value: input[span.start..span.end].to_owned(),
            span,
        });
    }
}

fn explicit_label(input: &str, label: &str) -> Option<(usize, usize)> {
    let lower = input.as_bytes();
    let needle = label.as_bytes();
    lower
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle))
        .and_then(|at| {
            let value_start = at + needle.len();
            let value_start = input[value_start..]
                .find(|c: char| !c.is_whitespace() && !matches!(c, ':' | '='))
                .map(|n| value_start + n)?;
            let value_end = input[value_start..]
                .find(|c: char| [';', '\n'].contains(&c))
                .map_or(input.len(), |n| value_start + n);
            Some((value_start, value_end))
        })
}

fn identifier(fields: &mut Vec<CitationField>, input: &str, prefix: &str, role: CitationFieldRole) {
    let lower = input.to_ascii_lowercase();
    let mut from = 0;
    while let Some(relative) = lower[from..].find(prefix) {
        let start = from + relative + prefix.len();
        let end = input[start..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '<' | '>' | ')' | ']' | ';' | ',')
            })
            .map_or(input.len(), |n| start + n);
        push_field(fields, role, input, start, end);
        from = end.saturating_add(1);
        if from >= input.len() {
            break;
        }
    }
}

fn parse(input: &str) -> NativeCitationOutcome {
    if input.len() > NATIVE_CITATION_INPUT_BYTE_CAP {
        return NativeCitationOutcome::Oversized;
    }
    if input.is_empty() || input.chars().all(char::is_whitespace) {
        return NativeCitationOutcome::Empty;
    }
    if input.chars().any(char::is_control) {
        return malformed(NativeCitationMalformed::ControlCharacter);
    }
    if input.chars().any(|c| !c.is_ascii() && !c.is_whitespace()) {
        return malformed(NativeCitationMalformed::NonAsciiScript);
    }

    if input.to_ascii_lowercase().contains("et al") {
        return NativeCitationOutcome::Unsupported {
            reason: NativeCitationUnsupported::UnsupportedRole,
        };
    }
    let title_count = input.to_ascii_lowercase().match_indices("title:").count();
    if title_count > 1 {
        return NativeCitationOutcome::Ambiguous {
            reason: NativeCitationAmbiguous::MultipleUnlabelledTitles,
        };
    }

    let mut fields = Vec::new();
    identifier(&mut fields, input, "doi:", CitationFieldRole::Doi);
    identifier(&mut fields, input, "https://", CitationFieldRole::Url);
    identifier(&mut fields, input, "http://", CitationFieldRole::Url);
    identifier(&mut fields, input, "arxiv:", CitationFieldRole::ArxivId);
    identifier(&mut fields, input, "arXiv:", CitationFieldRole::ArxivId);

    for (label, role) in [
        ("title", CitationFieldRole::ArticleTitle),
        ("container", CitationFieldRole::ContainerTitle),
        ("journal", CitationFieldRole::ContainerTitle),
        ("volume", CitationFieldRole::Volume),
        ("issue", CitationFieldRole::Issue),
        ("pages", CitationFieldRole::Pages),
        ("publisher", CitationFieldRole::Publisher),
        ("place", CitationFieldRole::Place),
        ("article number", CitationFieldRole::ArticleNumber),
    ] {
        if let Some((start, end)) = explicit_label(input, label) {
            push_field(&mut fields, role, input, start, end);
        }
    }

    // Delegate only the observable author/date subcontracts. Their normalized
    // component results are not guessed into this wire model; provenance stays
    // the literal source span.
    if let Some(author_end) = input.find(". ").or_else(|| input.find(" (")) {
        let author_end = author_end.min(input.len());
        let author = &input[..author_end];
        if matches!(
            NativePersonalNameParser::new().parse(PersonalNameContext::Citation, author),
            NativePersonalNameOutcome::Recognized { .. }
        ) {
            push_field(
                &mut fields,
                CitationFieldRole::Authors,
                input,
                0,
                author_end,
            );
        }
    }
    for (start, end) in year_candidates(input) {
        let candidate = &input[start..end];
        if matches!(
            NativeDateParser::new().parse(candidate),
            NativeDateOutcome::Recognized { .. }
        ) {
            push_field(
                &mut fields,
                CitationFieldRole::PublicationDate,
                input,
                start,
                end,
            );
        }
    }

    fields.sort_by_key(|field| (field.span.start, field.span.end, field.role as u8));
    fields.dedup_by(|a, b| a.role == b.role && a.span == b.span);
    if fields.is_empty() {
        return malformed(NativeCitationMalformed::MissingValue);
    }
    NativeCitationOutcome::Recognized { fields }
}

fn year_candidates(input: &str) -> Vec<(usize, usize)> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i..i + 4].iter().all(u8::is_ascii_digit)
            && (i == 0 || !bytes[i - 1].is_ascii_digit())
            && (i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit())
        {
            out.push((i, i + 4));
            // The four-digit year is a bounded candidate; do not scan the same
            // bytes again as part of a regex-like search.
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}
