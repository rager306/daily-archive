//! Pure renderer for the GROBID 0.9.0 CRF header feature ABI.
//!
//! This is deliberately separate from the generic `FeatureAbi`: the header
//! model consumes ordered, whitespace-delimited string rows, not floats.
use da_ports::LexiconBundle;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NATIVE_HEADER_FEATURE_COLUMNS: usize = 29;
pub const NATIVE_HEADER_FEATURE_SCHEMA_VERSION: &str = "da-parse.native-header-features/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BlockStatus {
    Blockstart,
    Blockin,
    Blockend,
}
impl BlockStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Blockstart => "BLOCKSTART",
            Self::Blockin => "BLOCKIN",
            Self::Blockend => "BLOCKEND",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LineStatus {
    Linestart,
    Linein,
    Lineend,
}
impl LineStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Linestart => "LINESTART",
            Self::Linein => "LINEIN",
            Self::Lineend => "LINEEND",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderAlignment {
    LineIndent,
    AlignedLeft,
}
impl HeaderAlignment {
    fn as_str(self) -> &'static str {
        match self {
            Self::LineIndent => "LINEINDENT",
            Self::AlignedLeft => "ALIGNEDLEFT",
        }
    }
}

/// Facts that the PDF/layout producer must provide for one header token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderFeatureToken {
    pub text: String,
    pub block_status: BlockStatus,
    pub line_status: LineStatus,
    pub alignment: HeaderAlignment,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
}

/// Aggregate font facts for the current header piece.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderFontStatistics {
    pub largest_font: f32,
    pub average_font: f32,
    pub smallest_font: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderFeatureRow {
    pub columns: Vec<String>,
}
impl HeaderFeatureRow {
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }
    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeHeaderFeatureError {
    EmptyInput,
    EmptyToken { token: usize },
    TokenContainsWhitespace { token: usize },
    NonFiniteFont { token: usize },
    NonFiniteStatistic { name: &'static str },
    InvalidStatistic { name: &'static str, value: f32 },
    InvalidLexicon(String),
    WrongArity { actual: usize, expected: usize },
}

impl fmt::Display for NativeHeaderFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "header feature input is empty"),
            Self::EmptyToken { token } => write!(f, "header feature token {token} is empty"),
            Self::TokenContainsWhitespace { token } => {
                write!(f, "header feature token {token} contains whitespace")
            }
            Self::NonFiniteFont { token } => {
                write!(f, "header feature token {token} has non-finite font size")
            }
            Self::NonFiniteStatistic { name } => {
                write!(f, "header font statistic {name} is non-finite")
            }
            Self::InvalidStatistic { name, value } => {
                write!(f, "header font statistic {name} is invalid: {value}")
            }
            Self::InvalidLexicon(error) => write!(f, "lexicon bundle is invalid: {error}"),
            Self::WrongArity { actual, expected } => write!(
                f,
                "rendered header row has {actual} columns; expected {expected}"
            ),
        }
    }
}
impl std::error::Error for NativeHeaderFeatureError {}

/// Render ordered tokens into the exact 29-column `FeaturesVectorHeader` ABI.
pub fn render_header_features(
    tokens: &[HeaderFeatureToken],
    fonts: HeaderFontStatistics,
    lexicon: &LexiconBundle,
) -> Result<Vec<HeaderFeatureRow>, NativeHeaderFeatureError> {
    if tokens.is_empty() {
        return Err(NativeHeaderFeatureError::EmptyInput);
    }
    validate_fonts(fonts)?;
    lexicon
        .validate()
        .map_err(|e| NativeHeaderFeatureError::InvalidLexicon(e.to_string()))?;
    let mut rows = Vec::with_capacity(tokens.len());
    let mut previous_font: Option<f32> = None;
    for (index, token) in tokens.iter().enumerate() {
        validate_token(index, token)?;
        let lower = token.text.to_lowercase();
        let digit = digit_status(&token.text);
        let capitalisation = if digit == "ALLDIGIT" {
            "NOCAPS"
        } else {
            capitalisation(&token.text)
        };
        let font_status = match previous_font {
            None => "NEWFONT",
            Some(previous) if previous.to_bits() == token.font_size.to_bits() => "SAMEFONT",
            Some(_) => "NEWFONT",
        };
        let font_size = if token.font_size > fonts.average_font {
            "HIGHERFONT"
        } else if token.font_size < fonts.average_font {
            "LOWERFONT"
        } else {
            "SAMEFONTSIZE"
        };
        previous_font = Some(token.font_size);
        let columns = vec![
            token.text.clone(),
            lower.clone(),
            prefix(&token.text, 1),
            prefix(&token.text, 2),
            prefix(&token.text, 3),
            prefix(&token.text, 4),
            suffix(&token.text, 1),
            suffix(&token.text, 2),
            suffix(&token.text, 3),
            suffix(&token.text, 4),
            token.block_status.as_str().into(),
            token.line_status.as_str().into(),
            token.alignment.as_str().into(),
            font_status.into(),
            font_size.into(),
            bool_flag(token.bold),
            bool_flag(token.italic),
            capitalisation.into(),
            digit.into(),
            bool_flag(token.text.chars().count() == 1),
            lexicon_flag(lexicon, "properName", &token.text, &lower),
            lexicon_flag(lexicon, "commonName", &token.text, &lower),
            lexicon_flag(lexicon, "year", &token.text, &lower),
            lexicon_flag(lexicon, "month", &token.text, &lower),
            lexicon_flag(lexicon, "locationName", &token.text, &lower),
            lexicon_flag(lexicon, "email", &token.text, &lower),
            lexicon_flag(lexicon, "http", &token.text, &lower),
            punctuation(&token.text).into(),
            bool_flag(token.font_size.to_bits() == fonts.largest_font.to_bits()),
        ];
        if columns.len() != NATIVE_HEADER_FEATURE_COLUMNS {
            return Err(NativeHeaderFeatureError::WrongArity {
                actual: columns.len(),
                expected: NATIVE_HEADER_FEATURE_COLUMNS,
            });
        }
        rows.push(HeaderFeatureRow { columns });
    }
    Ok(rows)
}

/// Serialize rows exactly as expected by `map_input`: one row per line.
pub fn render_header_feature_text(
    rows: &[HeaderFeatureRow],
) -> Result<String, NativeHeaderFeatureError> {
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        if row.width() != NATIVE_HEADER_FEATURE_COLUMNS {
            return Err(NativeHeaderFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_HEADER_FEATURE_COLUMNS,
            });
        }
        lines.push(row.as_text());
    }
    Ok(lines.join("\n"))
}

fn validate_token(
    index: usize,
    token: &HeaderFeatureToken,
) -> Result<(), NativeHeaderFeatureError> {
    if token.text.is_empty() {
        return Err(NativeHeaderFeatureError::EmptyToken { token: index });
    }
    if token.text.chars().any(char::is_whitespace) {
        return Err(NativeHeaderFeatureError::TokenContainsWhitespace { token: index });
    }
    if !token.font_size.is_finite() {
        return Err(NativeHeaderFeatureError::NonFiniteFont { token: index });
    }
    Ok(())
}
fn validate_fonts(fonts: HeaderFontStatistics) -> Result<(), NativeHeaderFeatureError> {
    for (name, value) in [
        ("largest_font", fonts.largest_font),
        ("average_font", fonts.average_font),
        ("smallest_font", fonts.smallest_font),
    ] {
        if !value.is_finite() {
            return Err(NativeHeaderFeatureError::NonFiniteStatistic { name });
        }
        if value <= 0.0 {
            return Err(NativeHeaderFeatureError::InvalidStatistic { name, value });
        }
    }
    if fonts.smallest_font > fonts.average_font || fonts.average_font > fonts.largest_font {
        return Err(NativeHeaderFeatureError::InvalidStatistic {
            name: "font_order",
            value: fonts.average_font,
        });
    }
    Ok(())
}
fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.into()
}
fn prefix(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}
fn suffix(value: &str, count: usize) -> String {
    let chars: Vec<_> = value.chars().collect();
    chars[chars.len().saturating_sub(count)..].iter().collect()
}
fn capitalisation(value: &str) -> &'static str {
    let letters: Vec<_> = value.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        "NOCAPS"
    } else if letters.iter().all(|c| c.is_uppercase()) {
        "ALLCAP"
    } else if letters.first().is_some_and(|c| c.is_uppercase()) {
        "INITCAP"
    } else {
        "NOCAPS"
    }
}
fn digit_status(value: &str) -> &'static str {
    if value.chars().all(|c| c.is_ascii_digit()) {
        "ALLDIGIT"
    } else if value.chars().any(|c| c.is_ascii_digit()) {
        "CONTAINSDIGITS"
    } else {
        "NODIGIT"
    }
}
fn punctuation(value: &str) -> &'static str {
    match value {
        "(" | "[" | "{" => "OPENBRACKET",
        ")" | "]" | "}" => "ENDBRACKET",
        "." => "DOT",
        "," => "COMMA",
        "-" | "–" => "HYPHEN",
        "\"" | "'" => "QUOTE",
        _ if value.chars().any(|c| c.is_ascii_punctuation()) => "PUNCT",
        _ => "NOPUNCT",
    }
}
fn lexicon_flag(bundle: &LexiconBundle, key: &str, text: &str, lower: &str) -> String {
    let found = bundle
        .channels
        .iter()
        .find(|channel| channel_matches(&channel.key, key))
        .is_some_and(|channel| {
            channel
                .entries
                .iter()
                .any(|entry| entry == text || entry == lower)
        });
    bool_flag(found)
}
fn channel_matches(actual: &str, expected: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
    };
    normalize(actual) == normalize(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::{FeatureTemplate, LexiconChannel};
    fn bundle() -> LexiconBundle {
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: ([
                ("commonName", "attention"),
                ("email", "a@b.org"),
                ("http", "https://x"),
                ("locationName", "Google"),
                ("month", "january"),
                ("properName", "Ashish"),
                ("year", "2017"),
            ])
            .into_iter()
            .map(|(key, entry)| LexiconChannel {
                key: key.into(),
                entries: vec![entry.into()],
            })
            .collect(),
            feature_templates: vec![FeatureTemplate {
                key: "header".into(),
                channel_keys: vec!["commonName".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }
    fn token(text: &str, font_size: f32) -> HeaderFeatureToken {
        HeaderFeatureToken {
            text: text.into(),
            block_status: BlockStatus::Blockstart,
            line_status: LineStatus::Linestart,
            alignment: HeaderAlignment::LineIndent,
            font_size,
            bold: true,
            italic: false,
        }
    }
    fn fonts() -> HeaderFontStatistics {
        HeaderFontStatistics {
            largest_font: 14.0,
            average_font: 12.0,
            smallest_font: 10.0,
        }
    }
    #[test]
    fn renders_exact_29_columns_and_text() {
        let rows =
            render_header_features(&[token("A1", 14.0), token(".", 14.0)], fonts(), &bundle())
                .unwrap();
        assert_eq!(rows[0].width(), 29);
        assert_eq!(rows[0].columns[12], "LINEINDENT");
        assert_eq!(rows[0].columns[18], "CONTAINSDIGITS");
        assert_eq!(rows[1].columns[27], "DOT");
        assert_eq!(
            render_header_feature_text(&rows).unwrap().lines().count(),
            2
        );
    }
    #[test]
    fn preserves_singular_vocabulary_traps() {
        let rows = render_header_features(
            &[token("2017", 12.0), token("Name", 12.0)],
            fonts(),
            &bundle(),
        )
        .unwrap();
        assert_eq!(rows[0].columns[17], "NOCAPS");
        assert_eq!(rows[0].columns[18], "ALLDIGIT");
        assert_eq!(rows[1].columns[13], "SAMEFONT");
        assert_eq!(rows[1].columns[14], "SAMEFONTSIZE");
    }
    #[test]
    fn uses_lexicon_and_rejects_bad_tokens() {
        let rows = render_header_features(&[token("attention", 12.0)], fonts(), &bundle()).unwrap();
        assert_eq!(rows[0].columns[21], "1");
        assert!(matches!(
            render_header_features(&[token("bad token", 12.0)], fonts(), &bundle()),
            Err(NativeHeaderFeatureError::TokenContainsWhitespace { .. })
        ));
    }
}
