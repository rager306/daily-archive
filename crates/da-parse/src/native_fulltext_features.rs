//! Pure renderer for the GROBID 0.9.0 CRF body/fulltext feature ABI.
//!
//! One model-facing string ABI lives here and nothing else: the 27-column
//! `FeaturesVectorFulltext.printVector` row. It is whitespace separated, one row
//! per admitted layout token, and candidate-only: this module renders rows, it
//! never loads a model, never decodes a label sequence, and is never spliced
//! into the production parse route.
//!
//! ## Source of truth (format authority)
//!
//! Ported from the vendored GROBID 0.9.x tree, read for format only and never
//! linked, vendored, or copied:
//!
//! - `grobid-core/src/main/java/org/grobid/core/features/FeaturesVectorFulltext.java`
//!   (`printVector`) — the exact emitted column list and order;
//! - `grobid-core/src/main/java/org/grobid/core/engines/FullTextParser.java`
//!   (`getBodyTextFeatured`, the `features.*` assignment block) — the stateful
//!   `fontStatus` / `fontSize` lanes and the `NBBINS_POSITION = 12` call sites;
//! - `grobid-core/src/main/java/org/grobid/core/features/FeatureFactory.java`
//!   (`linearScaling(int,int,int)` and the `linearScaling(double,double,int)`
//!   overload, `isDigit` = `^\d+$`, `isPunct` = `^[,\:;?\.]+$`);
//! - `grobid-core/src/main/java/org/grobid/core/layout/LayoutToken.java`
//!   (`getFont()` returns a *font-name string*, `getFontSize()` a `double`).
//!
//! ## The 27 emitted columns, in order
//!
//! | # | Java field | source |
//! |---|---|---|
//! | 0 | `string` | token text |
//! | 1 | lowercased `string` | `String.toLowerCase` |
//! | 2-5 | `prefix(1..4)` | `TextUtilities.prefix` |
//! | 6-9 | `suffix(1..4)` | `TextUtilities.suffix` |
//! | 10 | `blockStatus` | producer |
//! | 11 | `lineStatus` | producer |
//! | 12 | `alignmentStatus` | producer (`ALIGNEDLEFT` / `LINEINDENT`) |
//! | 13 | `fontStatus` | stateful, font-name change |
//! | 14 | `fontSize` | stateful, truncated size comparison |
//! | 15 | `bold` | producer |
//! | 16 | `italic` | producer |
//! | 17 | `capitalisation` | ladder, `ALLDIGIT` overrides to `NOCAPS` |
//! | 18 | `digit` | ladder, `ALLDIGIT` overrides `CONTAINSDIGITS` |
//! | 19 | `singleChar` | length == 1 |
//! | 20 | `punctType` | punctuation ladder |
//! | 21 | `relativeDocumentPosition` | `linearScaling(nn, fulltextLength, 12)` |
//! | 22 | `relativePagePosition` | `linearScaling(lineY, pageHeight, 12)`, clamped |
//! | 23 | `bitmapAround` | producer |
//! | 24 | `calloutType` | `UNKNOWN` when unknown |
//! | 25 | `calloutKnown` | producer |
//! | 26 | `superscript` | producer |
//!
//! ## Deliberately absent (the D363 trap)
//!
//! `printVector` also carries four fields whose emission is commented out
//! upstream, plus one set-but-never-printed field. They must not be emitted,
//! because the frozen model was trained on exactly the 27 fields above:
//!
//! - `vectorAround` (both the field and its append are commented out);
//! - `spacingWithPreviousBlock` (`NBBINS_SPACE = 5`, append commented out);
//! - `characterDensity` (`NBBINS_DENSITY = 5`, append commented out);
//! - the training-label placeholder (append commented out);
//! - `relativePagePositionChar` — assigned in `FullTextParser` and never read.
//!
//! ## Emitted value, not intent
//!
//! Two lanes are inherited verbatim and are easy to "fix" by mistake:
//!
//! - the printed `capitalisation` is `NOCAPS` whenever `digit == ALLDIGIT`, so
//!   `2017` is `ALLCAP` by the ladder and `NOCAPS` as printed;
//! - `fontSize` is *not* a comparison against an average: the Java loop keeps a
//!   running `currentFontSize` (initialised to `-1`) and updates it on every
//!   change, so the lane is the sign of the change against the most recent
//!   token, not a document-level statistic.
//!
//! ## Scales
//!
//! `NBBINS_POSITION = 12` is the fulltext scale and is deliberately *not* the
//! citation-named accessor from [`crate::native_feature_facts`]: the two
//! families carry separate scales (S04/D361), so a change to one must not silently
//! move the other. Positions are supplied as raw inputs and scaled here, so the
//! renderer can never print a bin outside `[0, 12]`.
//!
//! ## Known residuals (contract-level proof, not source-bound parity)
//!
//! - Java indexes UTF-16 code units (`String.length()`, `charAt`, `prefix`,
//!   `suffix`); this module indexes Unicode scalar values. The two agree for
//!   every BMP token, which is what the frozen models were trained on.
//! - Java's `String.toLowerCase` uses the default locale; `str::to_lowercase`
//!   is locale-independent.
//! - `relativePagePositionChar` is computed by Java (the integer overload,
//!   `mm` / `pageLength`) and never printed, so it is not modelled.
//! - The `alignmentStatus` field's declared vocabulary also carries `CENTERED`,
//!   which `getBodyTextFeatured` never assigns; the value is carried so the
//!   typed input is not silently narrower than the field.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! regex, no absolute path, no model bytes. Every function is total on hostile
//! input — untrusted tokens (empty, whitespace, multibyte) yield a *typed*
//! refusal, never a panic and never a silent approximation. `import_eligible`
//! stays false and nothing here is spliced into the production parse route.

use crate::native_feature_facts::{
    FactsBlockStatus, FactsLineStatus, capitalisation, capitalisation_as_printed, digit_status,
    is_single_char, linear_scaling, punct_type, token_prefix, token_suffix,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Printed row width: the exact number of whitespace-delimited fields
/// `FeaturesVectorFulltext.printVector` emits.
pub const NATIVE_FULLTEXT_FEATURE_COLUMNS: usize = 27;

/// Schema identity of the rendered fulltext rows.
pub const NATIVE_FULLTEXT_FEATURE_SCHEMA_VERSION: &str = "da-parse.native-fulltext-features/v1";

/// Highest column *index* the printed row can satisfy (`0..=26`).
///
/// A model that declares a higher referenced column than this would be reading a
/// field this ABI refuses to print, which is a fail-closed condition rather than
/// a reason to widen the row (see [`check_fulltext_feature_columns`]).
pub const NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX: usize = NATIVE_FULLTEXT_FEATURE_COLUMNS - 1;

/// `FullTextParser.NBBINS_POSITION` — the fulltext relative-position scale.
pub const NATIVE_FULLTEXT_RELATIVE_POSITION_BINS: i32 = 12;

/// Upper bound on `render_fulltext_features` input length.
///
/// Provisional and fail-closed: the fulltext band is not yet a runtime surface,
/// so the two position accumulators (`nn`, `mm`) are still carried by hand
/// rather than by the production thread. An input beyond this bound is refused
/// instead of being truncated, so no silently partial read can be mistaken for a
/// complete one.
pub const MAX_NATIVE_FULLTEXT_FEATURE_ROWS: usize = 32_768;

/// `alignmentStatus` lane.
///
/// The fulltext producer assigns only `ALIGNEDLEFT` and `LINEINDENT`;
/// `CENTERED` belongs to the field's declared vocabulary and is carried so the
/// typed input is not narrower than the Java field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FulltextAlignment {
    Alignedleft,
    Lineindent,
    Centered,
}

impl FulltextAlignment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alignedleft => "ALIGNEDLEFT",
            Self::Lineindent => "LINEINDENT",
            Self::Centered => "CENTERED",
        }
    }
}

/// `fontStatus` lane: `NEWFONT` whenever the font *name* changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FulltextFontStatus {
    Newfont,
    Samefont,
}

impl FulltextFontStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Newfont => "NEWFONT",
            Self::Samefont => "SAMEFONT",
        }
    }
}

/// `fontSize` lane: the sign of the truncated size change against the previous
/// admitted token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FulltextFontSize {
    Higherfont,
    Samefontsize,
    Lowerfont,
}

impl FulltextFontSize {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Higherfont => "HIGHERFONT",
            Self::Samefontsize => "SAMEFONTSIZE",
            Self::Lowerfont => "LOWERFONT",
        }
    }
}

/// `calloutType` lane: how a reference callout is expressed, if known.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FulltextCalloutType {
    #[default]
    Unknown,
    Number,
    Author,
}

impl FulltextCalloutType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Number => "NUMBER",
            Self::Author => "AUTHOR",
        }
    }
}

/// Facts the layout producer must provide for one body token.
///
/// Everything the single token cannot see stays an input: the status lanes, the
/// font identity, the running document/page positions. Everything the token
/// *can* see (capitalisation, digit, punctuation, single-character, prefix,
/// suffix) is derived from [`crate::native_feature_facts`] rather than accepted
/// from the caller, so a caller cannot smuggle a divergent fact into the row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FulltextFeatureToken {
    pub text: String,
    pub block_status: FactsBlockStatus,
    pub line_status: FactsLineStatus,
    pub alignment: FulltextAlignment,
    /// Font *name* (`LayoutToken.getFont()`), the identity `fontStatus` compares.
    /// Never printed, so it may contain spaces.
    pub font: String,
    /// Raw `LayoutToken.getFontSize()`; Java truncates it with `(int)`.
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    /// Running `nn` — the token offset over the whole body.
    pub document_position: i32,
    /// `fulltextLength` — the admitted body token count.
    pub document_length: i32,
    /// `coordinateLineY` — the line ordinate within the page.
    pub page_line_position: f64,
    /// `pageHeight` — the page height in the same coordinate system.
    pub page_height: f64,
    pub bitmap_around: bool,
    pub callout_type: FulltextCalloutType,
    pub callout_known: bool,
    pub superscript: bool,
}

/// One rendered fulltext row, one whitespace-delimited field per column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FulltextFeatureRow {
    pub columns: Vec<String>,
}

impl FulltextFeatureRow {
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

/// Typed refusal of the fulltext row builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFulltextFeatureError {
    /// No tokens were supplied; an empty row list is not a valid observation.
    EmptyInput,
    /// A token text was empty.
    EmptyToken { token: usize },
    /// A token text contains whitespace, which would corrupt the row arity by
    /// splitting one field into two.
    TokenContainsWhitespace { token: usize },
    /// `font_size` was not finite.
    NonFiniteFont { token: usize },
    /// The page geometry (`page_line_position` / `page_height`) was not finite.
    NonFinitePageGeometry { token: usize },
    /// A position or length input was negative; Java ints are non-negative here.
    NegativePosition { token: usize, field: &'static str },
    /// The input exceeded [`MAX_NATIVE_FULLTEXT_FEATURE_ROWS`].
    TooManyRows { rows: usize, limit: usize },
    /// A rendered row did not carry exactly
    /// [`NATIVE_FULLTEXT_FEATURE_COLUMNS`] fields.
    WrongArity { actual: usize, expected: usize },
    /// A model declared a different number of observation columns than this ABI
    /// prints; fail closed instead of widening the row.
    DeclaredColumnMismatch { declared: usize, expected: usize },
}

impl fmt::Display for NativeFulltextFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "fulltext feature input is empty"),
            Self::EmptyToken { token } => write!(f, "fulltext feature token {token} is empty"),
            Self::TokenContainsWhitespace { token } => write!(
                f,
                "fulltext feature token {token} contains a separator or whitespace"
            ),
            Self::NonFiniteFont { token } => {
                write!(f, "fulltext feature token {token} has non-finite font size")
            }
            Self::NonFinitePageGeometry { token } => write!(
                f,
                "fulltext feature token {token} has non-finite page geometry"
            ),
            Self::NegativePosition { token, field } => {
                write!(f, "fulltext feature token {token} has negative {field}")
            }
            Self::TooManyRows { rows, limit } => write!(
                f,
                "fulltext feature input has {rows} rows; the admitted limit is {limit}"
            ),
            Self::WrongArity { actual, expected } => write!(
                f,
                "rendered fulltext row has {actual} columns; expected {expected}"
            ),
            Self::DeclaredColumnMismatch { declared, expected } => write!(
                f,
                "model declares {declared} fulltext observation columns; this ABI prints {expected}"
            ),
        }
    }
}

impl std::error::Error for NativeFulltextFeatureError {}

/// Render ordered tokens into the exact 27-column
/// `FeaturesVectorFulltext.printVector` ABI.
///
/// The two font lanes are *stateful*: rows must be rendered in document order,
/// because `fontStatus` compares each token's font name with the previous
/// token's and `fontSize` compares the truncated size with the running value.
pub fn render_fulltext_features(
    tokens: &[FulltextFeatureToken],
) -> Result<Vec<FulltextFeatureRow>, NativeFulltextFeatureError> {
    if tokens.is_empty() {
        return Err(NativeFulltextFeatureError::EmptyInput);
    }
    if tokens.len() > MAX_NATIVE_FULLTEXT_FEATURE_ROWS {
        return Err(NativeFulltextFeatureError::TooManyRows {
            rows: tokens.len(),
            limit: MAX_NATIVE_FULLTEXT_FEATURE_ROWS,
        });
    }
    let mut rows = Vec::with_capacity(tokens.len());
    let mut previous_font: Option<&str> = None;
    let mut current_font_size: Option<i32> = None;
    for (index, token) in tokens.iter().enumerate() {
        validate_token(index, token)?;
        let facts_text = token.text.as_str();
        let capitalisation =
            capitalisation_as_printed(capitalisation(facts_text), digit_status(facts_text));
        let font_status = if previous_font == Some(token.font.as_str()) {
            FulltextFontStatus::Samefont
        } else {
            FulltextFontStatus::Newfont
        };
        previous_font = Some(token.font.as_str());
        let new_font_size = token.font_size as i32;
        let font_size = match current_font_size {
            None => FulltextFontSize::Higherfont,
            Some(current) if current == new_font_size => FulltextFontSize::Samefontsize,
            Some(current) if current < new_font_size => FulltextFontSize::Higherfont,
            Some(_) => FulltextFontSize::Lowerfont,
        };
        current_font_size = Some(new_font_size);
        let columns = vec![
            token.text.clone(),
            facts_text.to_lowercase(),
            token_prefix(facts_text, 1),
            token_prefix(facts_text, 2),
            token_prefix(facts_text, 3),
            token_prefix(facts_text, 4),
            token_suffix(facts_text, 1),
            token_suffix(facts_text, 2),
            token_suffix(facts_text, 3),
            token_suffix(facts_text, 4),
            token.block_status.as_str().into(),
            token.line_status.as_str().into(),
            token.alignment.as_str().into(),
            font_status.as_str().into(),
            font_size.as_str().into(),
            bool_flag(token.bold),
            bool_flag(token.italic),
            capitalisation.as_str().into(),
            digit_status(facts_text).as_str().into(),
            bool_flag(is_single_char(facts_text)),
            punct_type(facts_text).as_str().into(),
            relative_position_fulltext_document(token.document_position, token.document_length)
                .to_string(),
            relative_position_fulltext_page(token.page_line_position, token.page_height)
                .to_string(),
            bool_flag(token.bitmap_around),
            token.callout_type.as_str().into(),
            bool_flag(token.callout_known),
            bool_flag(token.superscript),
        ];
        if columns.len() != NATIVE_FULLTEXT_FEATURE_COLUMNS {
            return Err(NativeFulltextFeatureError::WrongArity {
                actual: columns.len(),
                expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
            });
        }
        rows.push(FulltextFeatureRow { columns });
    }
    Ok(rows)
}

/// Serialize rows exactly as expected by the sequence seam's `map_input`: one
/// row per line, no trailing newline (the caller joins/terminates).
pub fn render_fulltext_feature_text(
    rows: &[FulltextFeatureRow],
) -> Result<String, NativeFulltextFeatureError> {
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        if row.width() != NATIVE_FULLTEXT_FEATURE_COLUMNS {
            return Err(NativeFulltextFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
            });
        }
        lines.push(row.as_text());
    }
    Ok(lines.join("\n"))
}

/// Fail closed when a model declares a different observation width than the
/// printed ABI.
///
/// Called with the column count a frozen model observation declares. Widening
/// the row to satisfy a model would emit fields the model was never trained on,
/// so a mismatch is a typed refusal.
pub fn check_fulltext_feature_columns(declared: usize) -> Result<(), NativeFulltextFeatureError> {
    if declared == NATIVE_FULLTEXT_FEATURE_COLUMNS {
        Ok(())
    } else {
        Err(NativeFulltextFeatureError::DeclaredColumnMismatch {
            declared,
            expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
        })
    }
}

/// Fulltext column 21: `linearScaling(nn, fulltextLength, NBBINS_POSITION)`.
///
/// Deliberately separate from the citation-named accessor in
/// [`crate::native_feature_facts`] — the two families carry separate scales
/// (S04/D361), so neither can silently move the other.
pub fn relative_position_fulltext_document(position: i32, total: i32) -> i32 {
    linear_scaling(position, total, NATIVE_FULLTEXT_RELATIVE_POSITION_BINS)
}

/// Fulltext column 22: `linearScaling(coordinateLineY, pageHeight, NBBINS_POSITION)`.
///
/// Java resolves this call site to the `double` overload, and then clamps the
/// result to `NBBINS_POSITION`. The clamp is retained even though the scale
/// already bounds the value, so the emitted bin matches the Java code path
/// exactly.
pub fn relative_position_fulltext_page(line_position: f64, page_height: f64) -> i32 {
    let scaled = linear_scaling_f64(
        line_position,
        page_height,
        NATIVE_FULLTEXT_RELATIVE_POSITION_BINS,
    );
    if scaled > NATIVE_FULLTEXT_RELATIVE_POSITION_BINS {
        NATIVE_FULLTEXT_RELATIVE_POSITION_BINS
    } else {
        scaled
    }
}

/// `FeatureFactory.linearScaling(double, double, int)`.
///
/// A distinct overload from the `f32` integer-precision primitive in
/// [`crate::native_feature_facts`]: the branch order is identical, but the
/// arithmetic is `f64`, and the Java cast truncates toward zero. `NaN` therefore
/// renders `0` exactly as Java's `(int) NaN` does.
fn linear_scaling_f64(pos: f64, total: f64, nb_bins: i32) -> i32 {
    if pos >= total {
        return nb_bins;
    }
    if pos <= 0.0 {
        return 0;
    }
    let relative = pos / total;
    let scaled = relative * nb_bins as f64;
    scaled as i32
}

fn validate_token(
    index: usize,
    token: &FulltextFeatureToken,
) -> Result<(), NativeFulltextFeatureError> {
    if token.text.is_empty() {
        return Err(NativeFulltextFeatureError::EmptyToken { token: index });
    }
    if token.text.chars().any(char::is_whitespace) {
        return Err(NativeFulltextFeatureError::TokenContainsWhitespace { token: index });
    }
    if !token.font_size.is_finite() {
        return Err(NativeFulltextFeatureError::NonFiniteFont { token: index });
    }
    if !token.page_line_position.is_finite() || !token.page_height.is_finite() {
        return Err(NativeFulltextFeatureError::NonFinitePageGeometry { token: index });
    }
    for (field, value) in [
        ("document_position", token.document_position),
        ("document_length", token.document_length),
    ] {
        if value < 0 {
            return Err(NativeFulltextFeatureError::NegativePosition {
                token: index,
                field,
            });
        }
    }
    Ok(())
}

fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str) -> FulltextFeatureToken {
        FulltextFeatureToken {
            text: text.into(),
            block_status: FactsBlockStatus::Blockstart,
            line_status: FactsLineStatus::Linestart,
            alignment: FulltextAlignment::Lineindent,
            font: "Times-Roman".into(),
            font_size: 10.0,
            bold: false,
            italic: false,
            document_position: 0,
            document_length: 10,
            page_line_position: 0.0,
            page_height: 792.0,
            bitmap_around: false,
            callout_type: FulltextCalloutType::Unknown,
            callout_known: false,
            superscript: false,
        }
    }

    #[test]
    fn renders_exact_27_columns_in_java_order() {
        let rows = render_fulltext_features(&[token("Name")]).unwrap();
        assert_eq!(rows[0].width(), NATIVE_FULLTEXT_FEATURE_COLUMNS);
        assert_eq!(rows[0].width(), 27);
        assert_eq!(rows[0].columns[0], "Name");
        assert_eq!(rows[0].columns[1], "name");
        assert_eq!(rows[0].columns[2], "N");
        assert_eq!(rows[0].columns[5], "Name");
        assert_eq!(rows[0].columns[6], "e");
        assert_eq!(rows[0].columns[9], "Name");
        assert_eq!(rows[0].columns[10], "BLOCKSTART");
        assert_eq!(rows[0].columns[11], "LINESTART");
        assert_eq!(rows[0].columns[12], "LINEINDENT");
        assert_eq!(rows[0].columns[13], "NEWFONT");
        assert_eq!(rows[0].columns[14], "HIGHERFONT");
        assert_eq!(rows[0].columns[15], "0");
        assert_eq!(rows[0].columns[16], "0");
        assert_eq!(rows[0].columns[17], "INITCAP");
        assert_eq!(rows[0].columns[18], "NODIGIT");
        assert_eq!(rows[0].columns[19], "0");
        assert_eq!(rows[0].columns[20], "NOPUNCT");
        assert_eq!(rows[0].columns[21], "0");
        assert_eq!(rows[0].columns[22], "0");
        assert_eq!(rows[0].columns[23], "0");
        assert_eq!(rows[0].columns[24], "UNKNOWN");
        assert_eq!(rows[0].columns[25], "0");
        assert_eq!(rows[0].columns[26], "0");
    }

    #[test]
    fn position_lane_zero_uses_the_document_scale_not_the_sign() {
        // `linearScaling(0, 0, 12)` hits the `pos >= total` branch first.
        assert_eq!(relative_position_fulltext_document(0, 0), 12);
        assert_eq!(relative_position_fulltext_document(0, 5), 0);
        assert_eq!(relative_position_fulltext_document(5, 5), 12);
        assert_eq!(relative_position_fulltext_document(1, 3), 4);
        // The double overload is f64, so the citation f32 rounding does not
        // apply here.
        assert_eq!(relative_position_fulltext_page(0.0, 792.0), 0);
        assert_eq!(relative_position_fulltext_page(396.0, 792.0), 6);
        assert_eq!(relative_position_fulltext_page(792.0, 792.0), 12);
        assert_eq!(relative_position_fulltext_page(1000.0, 792.0), 12);
    }

    #[test]
    fn printed_capitalisation_and_digit_lanes_take_the_alldigit_override() {
        let rows = render_fulltext_features(&[token("2017"), token("2nd"), token("Word")]).unwrap();
        assert_eq!(rows[0].columns[17], "NOCAPS");
        assert_eq!(rows[0].columns[18], "ALLDIGIT");
        assert_eq!(rows[1].columns[17], "NOCAPS");
        assert_eq!(rows[1].columns[18], "CONTAINSDIGITS");
        assert_eq!(rows[2].columns[17], "INITCAP");
        assert_eq!(rows[2].columns[18], "NODIGIT");
        // Punctuation is ALLCAP by the no-lowercase rule and is printed as such
        // because its digit lane is not ALLDIGIT.
        let dotted = render_fulltext_features(&[token(".")]).unwrap();
        assert_eq!(dotted[0].columns[17], "ALLCAP");
        assert_eq!(dotted[0].columns[20], "DOT");
    }

    #[test]
    fn font_lanes_are_stateful_over_the_admitted_sequence() {
        let mut first = token("A");
        first.font = "Times".into();
        first.font_size = 10.0;
        let mut second = token("B");
        second.font = "Times".into();
        second.font_size = 10.4;
        let mut third = token("C");
        third.font = "Helvetica".into();
        third.font_size = 8.0;
        let rows = render_fulltext_features(&[first, second, third]).unwrap();
        assert_eq!(rows[0].columns[13], "NEWFONT");
        assert_eq!(rows[1].columns[13], "SAMEFONT");
        assert_eq!(rows[2].columns[13], "NEWFONT");
        // `(int) 10.4 == 10`, so the rounded-up size is not a change.
        assert_eq!(rows[1].columns[14], "SAMEFONTSIZE");
        assert_eq!(rows[2].columns[14], "LOWERFONT");
    }

    #[test]
    fn multibyte_tokens_render_without_panicking() {
        let rows = render_fulltext_features(&[token("日本語"), token("🙂"), token("٣٤")]).unwrap();
        assert_eq!(rows[0].width(), 27);
        assert_eq!(rows[0].columns[1], "日本語");
        assert_eq!(rows[0].columns[2], "日");
        // `TextUtilities.suffix` returns the whole string when it is shorter
        // than the count, so the 4-character lane carries the 3-character token.
        assert_eq!(rows[0].columns[9], "日本語");
        // Astral-plane token: one scalar, so the single-character lane fires.
        assert_eq!(rows[1].columns[19], "1");
        assert_eq!(rows[2].columns[18], "CONTAINSDIGITS");
    }

    #[test]
    fn negatives_are_typed() {
        assert!(matches!(
            render_fulltext_features(&[]),
            Err(NativeFulltextFeatureError::EmptyInput)
        ));
        assert!(matches!(
            render_fulltext_features(&[token("")]),
            Err(NativeFulltextFeatureError::EmptyToken { token: 0 })
        ));
        // Unreachable by construction; the guard is still reachable through the
        // row-level serializer.
        assert!(matches!(
            render_fulltext_features(&[token("bad token")]),
            Err(NativeFulltextFeatureError::TokenContainsWhitespace { token: 0 })
        ));
        assert!(matches!(
            render_fulltext_features(&[token("\n")]),
            Err(NativeFulltextFeatureError::TokenContainsWhitespace { token: 0 })
        ));
        let mut bad_font = token("x");
        bad_font.font_size = f64::NAN;
        assert!(matches!(
            render_fulltext_features(&[bad_font]),
            Err(NativeFulltextFeatureError::NonFiniteFont { token: 0 })
        ));
        let mut bad_page = token("x");
        bad_page.page_height = f64::INFINITY;
        assert!(matches!(
            render_fulltext_features(&[bad_page]),
            Err(NativeFulltextFeatureError::NonFinitePageGeometry { token: 0 })
        ));
        let mut bad_position = token("x");
        bad_position.document_position = -1;
        assert!(matches!(
            render_fulltext_features(&[bad_position]),
            Err(NativeFulltextFeatureError::NegativePosition {
                token: 0,
                field: "document_position"
            })
        ));
    }

    #[test]
    fn row_serializer_refuses_a_wrong_arity_row() {
        let row = FulltextFeatureRow {
            columns: vec!["a".into(), "b".into()],
        };
        assert!(matches!(
            render_fulltext_feature_text(&[row]),
            Err(NativeFulltextFeatureError::WrongArity {
                actual: 2,
                expected: 27
            })
        ));
        let rows = render_fulltext_features(&[token("A"), token(".")]).unwrap();
        let text = render_fulltext_feature_text(&rows).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert_eq!(text.lines().next().unwrap().split(' ').count(), 27);
        assert!(!text.ends_with('\n'));
    }

    #[test]
    fn declared_column_count_must_match_the_printed_surface() {
        assert_eq!(check_fulltext_feature_columns(27), Ok(()));
        assert!(matches!(
            check_fulltext_feature_columns(26),
            Err(NativeFulltextFeatureError::DeclaredColumnMismatch {
                declared: 26,
                expected: 27
            })
        ));
        assert_eq!(NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX, 26);
        assert_eq!(NATIVE_FULLTEXT_RELATIVE_POSITION_BINS, 12);
    }

    #[test]
    fn rendering_is_deterministic() {
        let tokens = [token("Body"), token("text"), token(",")];
        let first = render_fulltext_features(&tokens).unwrap();
        let second = render_fulltext_features(&tokens).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            render_fulltext_feature_text(&first).unwrap(),
            render_fulltext_feature_text(&second).unwrap()
        );
    }
}
