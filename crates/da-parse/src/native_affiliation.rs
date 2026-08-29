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
