//! Pure, bounded, redacting affiliation recognizer for shadow comparison.
//!
//! This module is deliberately a leaf: it accepts only affiliation text and
//! returns source spans plus closed reason codes. It does not know authors,
//! links, TEI, persistence, or the production parser.
use serde::{Deserialize, Serialize};

pub const NATIVE_AFFILIATION_PARSER_ID: &str = "da-parse.native_affiliation";
pub const NATIVE_AFFILIATION_PARSER_VERSION: &str = "1.0.0";
pub const NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION: &str =
    "da-parse.native-affiliation.outcome/v1";
pub const NATIVE_AFFILIATION_INPUT_BYTE_CAP: usize = 512;
pub const NATIVE_AFFILIATION_LINK_SCHEMA_VERSION: &str =
    "da-parse.native-affiliation-link.outcome/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliationSpan {
    pub start: usize,
    pub end: usize,
}

impl AffiliationSpan {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAffiliationNesting {
    DirectAuthorAffiliation,
}

/// An affiliation already recognized by the independent affiliation surface.
/// The linker never accepts raw text and therefore cannot invent its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAffiliationOccurrence {
    pub document_id: String,
    pub key: String,
    pub ordinal: usize,
    pub span: AffiliationSpan,
    pub nesting: NativeAffiliationNesting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAffiliationAuthorEvidence {
    pub ordinal: usize,
    pub span: AffiliationSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAffiliationLink {
    pub key: String,
    pub affiliation_ordinal: usize,
    pub affiliation_span: AffiliationSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum NativeAffiliationLinkError {
    DuplicateKey { key: String },
    MissingKey { ordinal: usize },
    UnknownKey { key: String },
    InvalidSpan { ordinal: usize },
    Reordered { previous: usize, current: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeAffiliationLinkOutcome {
    Linked {
        author: NativeAffiliationAuthorEvidence,
        links: Vec<NativeAffiliationLink>,
    },
    Error {
        error: NativeAffiliationLinkError,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAffiliationLinker;

impl NativeAffiliationLinker {
    pub const fn new() -> Self {
        Self
    }

    pub fn link(
        &self,
        _name: &crate::RecognizedPersonalName,
        document_id: &str,
        author_ordinal: usize,
        author_span: AffiliationSpan,
        occurrences: &[NativeAffiliationOccurrence],
    ) -> NativeAffiliationLinkOutcome {
        if !valid_span(author_span) {
            return NativeAffiliationLinkOutcome::Error {
                error: NativeAffiliationLinkError::InvalidSpan {
                    ordinal: author_ordinal,
                },
            };
        }

        let mut links = Vec::with_capacity(occurrences.len());
        let mut previous_ordinal = None;
        if document_id.is_empty() {
            return link_error(NativeAffiliationLinkError::UnknownKey {
                key: document_id.to_owned(),
            });
        }
        for occurrence in occurrences {
            if occurrence.document_id != document_id {
                return link_error(NativeAffiliationLinkError::UnknownKey {
                    key: occurrence.key.clone(),
                });
            }
            if occurrence.key.is_empty() {
                return link_error(NativeAffiliationLinkError::MissingKey {
                    ordinal: occurrence.ordinal,
                });
            }
            if !valid_span(occurrence.span) {
                return link_error(NativeAffiliationLinkError::InvalidSpan {
                    ordinal: occurrence.ordinal,
                });
            }
            if occurrence.nesting != NativeAffiliationNesting::DirectAuthorAffiliation {
                return link_error(NativeAffiliationLinkError::UnknownKey {
                    key: occurrence.key.clone(),
                });
            }
            if let Some(previous) = previous_ordinal
                && occurrence.ordinal <= previous
            {
                return link_error(NativeAffiliationLinkError::Reordered {
                    previous,
                    current: occurrence.ordinal,
                });
            }
            if links
                .iter()
                .any(|link: &NativeAffiliationLink| link.key == occurrence.key)
            {
                return link_error(NativeAffiliationLinkError::DuplicateKey {
                    key: occurrence.key.clone(),
                });
            }
            previous_ordinal = Some(occurrence.ordinal);
            links.push(NativeAffiliationLink {
                key: occurrence.key.clone(),
                affiliation_ordinal: occurrence.ordinal,
                affiliation_span: occurrence.span,
            });
        }

        NativeAffiliationLinkOutcome::Linked {
            author: NativeAffiliationAuthorEvidence {
                ordinal: author_ordinal,
                span: author_span,
            },
            links,
        }
    }
}

fn valid_span(span: AffiliationSpan) -> bool {
    span.start < span.end
}

fn link_error(error: NativeAffiliationLinkError) -> NativeAffiliationLinkOutcome {
    NativeAffiliationLinkOutcome::Error { error }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AffiliationComponentKind {
    Organization,
    Marker,
    Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognizedAffiliationComponent {
    pub kind: AffiliationComponentKind,
    pub span: AffiliationSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAffiliationMalformed {
    ControlCharacter,
    NonAsciiScript,
    UnexpectedCharacter,
    RepeatedSeparator,
    EmptyComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAffiliationUnsupported {
    UnsupportedPunctuation,
    NoRecognizableComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAffiliationAmbiguous {
    MultipleOrganizationCandidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeAffiliationOutcome {
    Recognized {
        components: Vec<RecognizedAffiliationComponent>,
    },
    Empty,
    Malformed {
        reason: NativeAffiliationMalformed,
    },
    Unsupported {
        reason: NativeAffiliationUnsupported,
    },
    Ambiguous {
        reason: NativeAffiliationAmbiguous,
    },
    Oversized,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAffiliationParser;

impl NativeAffiliationParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse(&self, input: &str) -> NativeAffiliationOutcome {
        parse(input)
    }
}

fn malformed(reason: NativeAffiliationMalformed) -> NativeAffiliationOutcome {
    NativeAffiliationOutcome::Malformed { reason }
}

fn parse(input: &str) -> NativeAffiliationOutcome {
    if input.len() > NATIVE_AFFILIATION_INPUT_BYTE_CAP {
        return NativeAffiliationOutcome::Oversized;
    }
    if input.is_empty() || input.chars().all(char::is_whitespace) {
        return NativeAffiliationOutcome::Empty;
    }
    if input.chars().any(char::is_control) {
        return malformed(NativeAffiliationMalformed::ControlCharacter);
    }
    if input.chars().any(is_bidi_formatting_mark) {
        return malformed(NativeAffiliationMalformed::UnexpectedCharacter);
    }
    if input.contains(';') || input.contains(':') || input.contains('(') || input.contains(')') {
        return NativeAffiliationOutcome::Unsupported {
            reason: NativeAffiliationUnsupported::UnsupportedPunctuation,
        };
    }

    let mut components = Vec::new();
    let mut organization_count = 0;
    let mut saw_empty = false;
    let mut saw_unsupported = false;
    let mut offset = 0;
    for (index, part) in input.split(',').enumerate() {
        let base = offset;
        offset += part.len() + usize::from(offset + part.len() < input.len());
        let leading = part.len() - part.trim_start().len();
        let mut value = part.trim();
        if value.is_empty() {
            saw_empty = true;
            continue;
        }
        let mut start = base + leading;
        let mut end = start + value.len();
        if index == 0
            && let Some((marker_end, marker)) = value
                .char_indices()
                .next()
                .map(|(end, marker)| (end + marker.len_utf8(), marker))
            && is_marker_char(marker)
        {
            components.push(RecognizedAffiliationComponent {
                kind: AffiliationComponentKind::Marker,
                span: AffiliationSpan::new(start, start + marker_end),
            });
            let remainder = &value[marker_end..];
            let skipped = remainder.len() - remainder.trim_start().len();
            let remainder = remainder.trim();
            if remainder.is_empty() {
                continue;
            }
            start += marker_end + skipped;
            end = start + remainder.len();
            value = remainder;
        }
        if is_marker(value) {
            components.push(RecognizedAffiliationComponent {
                kind: AffiliationComponentKind::Marker,
                span: AffiliationSpan::new(start, end),
            });
            continue;
        }
        if is_organization(value) {
            organization_count += 1;
            components.push(RecognizedAffiliationComponent {
                kind: AffiliationComponentKind::Organization,
                span: AffiliationSpan::new(start, end),
            });
        } else if is_address(value) {
            components.push(RecognizedAffiliationComponent {
                kind: AffiliationComponentKind::Address,
                span: AffiliationSpan::new(start, end),
            });
        } else {
            saw_unsupported = true;
        }
    }
    if saw_empty {
        return malformed(NativeAffiliationMalformed::RepeatedSeparator);
    }
    if components.is_empty() {
        return NativeAffiliationOutcome::Unsupported {
            reason: NativeAffiliationUnsupported::NoRecognizableComponent,
        };
    }
    if organization_count == 2
        && components.len() == 2
        && !saw_unsupported
        && input.split(',').all(|part| {
            let value = part.trim().to_ascii_lowercase();
            value == "university" || value == "institute"
        })
    {
        return NativeAffiliationOutcome::Ambiguous {
            reason: NativeAffiliationAmbiguous::MultipleOrganizationCandidates,
        };
    }
    NativeAffiliationOutcome::Recognized { components }
}

fn is_bidi_formatting_mark(c: char) -> bool {
    matches!(
        c,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'
            | '\u{202b}'
            | '\u{202c}'
            | '\u{202d}'
            | '\u{202e}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

fn is_marker(value: &str) -> bool {
    value.chars().count() == 1 && value.chars().next().is_some_and(is_marker_char)
}

fn is_marker_char(c: char) -> bool {
    matches!(c, '¹' | '²' | '³')
        || ('\u{2070}'..='\u{209f}').contains(&c)
        || c.is_ascii_digit()
        || matches!(c, '*' | '†' | '‡')
}

fn is_organization(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "university",
        "institute",
        "laboratory",
        "laboratório",
        "department",
        "faculty",
        "college",
        "centre",
        "center",
        "hospital",
        "research",
        "universität",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn is_address(value: &str) -> bool {
    let Some(first) = value.as_bytes().first() else {
        return false;
    };
    first.is_ascii_digit()
        && [
            " street",
            " road",
            " avenue",
            " boulevard",
            "straße",
            "strasse",
        ]
        .iter()
        .any(|word| value.to_ascii_lowercase().contains(word))
}
