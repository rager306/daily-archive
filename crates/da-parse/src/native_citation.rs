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
