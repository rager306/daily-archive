//! Pure row builders for the GROBID 0.9.1 bibliography feature ABI.
//!
//! This module owns exactly two model-facing string ABIs and nothing else: the
//! 30-column citation row (`FeaturesVectorCitation.printVector`) and the
//! 28-column reference-segmenter row
//! (`FeaturesVectorReferenceSegmenter.printVector`). Both are whitespace
//! separated, one row per admitted token, and both are candidate-only: this
//! module renders rows, it never loads a model, never decodes a label sequence
//! and is never spliced into the production parse route.
//!
//! # Inputs (the `da-ports` seam)
//!
//! A row is a function of three typed inputs, never of a free string:
//!
//! * [`BibliographyTokenFacts`] — the per-token lanes of
//!   [`crate::native_feature_facts`] (capitalisation, digit, punctuation,
//!   single-character, year/month, line/block status, scaling helpers);
//! * [`BibliographyGazetteerSpans`] — the positional spans of
//!   [`crate::native_bibliography_gazetteer`], consumed through
//!   [`BibliographyGazetteerSpans::covers`] (`start <= n <= end`), which is the
//!   predicate `addFeaturesCitation` actually uses for its `isKnown*` lanes.
//!   Single-token membership is a *different* predicate and is not used here;
//! * [`LexiconBundle`] — the validated provider bundle behind the lexical
//!   lanes, reached through the `da-ports` seam so no `da-domain` type is named.
//!
//! The row builders compute the lexical lanes from the bundle they are given
//! instead of reading [`BibliographyTokenFacts::name_flags`]. A caller that
//! attached the flags from the same bundle gets identical output, while a caller
//! that forgot to attach them cannot silently emit an all-zero lexical block.
//!
//! # Emitted value, not intent
//!
//! Three traps are inherited verbatim from the Java `printVector`s and are easy
//! to "fix" by mistake:
//!
//! * the printed `capitalisation` is `NOCAPS` whenever `digit == ALLDIGIT`, so
//!   `2017` is `ALLCAP` by the ladder and `NOCAPS` as printed;
//! * the reference segmenter's `firstName` and `locationName` lanes carry
//!   `/* TODO: to review, never set! */` upstream and are never assigned, so the
//!   emitted value is `0` even where a lexicon lookup would say otherwise;
//! * the citation `http` column (21) is filled from the gazetteer *url* lane,
//!   not from the `FeatureFactory.http` pattern, while the reference segmenter
//!   has no gazetteer pass and does use that pattern. The two `http` lanes are
//!   different mechanisms.
//!
//! `email` is commented out upstream, so the reference-segmenter ABI has 28
//! columns and not 29.
//!
//! # Label tables
//!
//! Both tables are in *model order*: a label id indexes the model's weight
//! blocks (`uoff[o] + y`, `boff[o] + yp*Y + y`), so a re-sorted table is
//! silently wrong rather than merely untidy. The `name/citation` and `date`
//! tables are deliberately absent — they belong to the tasks that decode those
//! models.
//!
//! # Known residuals
//!
//! * **Position axis.** Java's reference segmenter accumulates `nn` over *all*
//!   layout tokens, advancing it by the token's own `String.length()` plus one
//!   extra unit per space/tab token, and emits the row *after* advancing. The
//!   citation model indexes `n` over all tokens, including the space tokens it
//!   skips. This ABI refuses whitespace tokens (a smuggled separator would
//!   corrupt the row arity), so positions accumulate over the admitted tokens
//!   only. A real layout producer must therefore hand over a token list whose
//!   whitespace is already accounted for, or columns 23/28 carry a bounded
//!   residual.
//! * **Row terminator.** `printVector` appends `"\n"` to every row; the
//!   renderers join with `"\n"` and leave the trailing newline to the caller,
//!   exactly as the header renderer does.
use crate::native_bibliography_gazetteer::{BibliographyGazetteerLane, BibliographyGazetteerSpans};
use crate::native_feature_facts::{
    BibliographyTokenFacts, REFERENCE_SEGMENTER_LINE_SCALE, linear_scaling, name_flags,
    punctuation_profile, punctuation_profile_field, relative_position_citation,
    relative_position_reference_segmenter,
};
use da_ports::LexiconBundle;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema version of the rendered bibliography rows.
pub const NATIVE_BIBLIOGRAPHY_FEATURES_SCHEMA_VERSION: &str =
    "da-parse.native-bibliography-features/v1";

/// Highest column *index* the citation model references.
///
/// From the model header `#rdr#81/28/0`; the printed row is
/// [`NATIVE_CITATION_FEATURE_COLUMNS`] fields wide because `printVector` also
/// emits the unreferenced training-label placeholder.
pub const NATIVE_CITATION_MODEL_MAX_COLUMN_INDEX: usize = 28;
/// Printed citation row width: 29 referenced columns plus the label placeholder.
pub const NATIVE_CITATION_FEATURE_COLUMNS: usize = 30;
/// Distinct citation roles (`author`, `editor`, …, `other`).
pub const NATIVE_CITATION_ROLE_COUNT: usize = 19;
/// Labels declared by `citation/model.wapiti` (19 roles × {begin, inside}).
pub const NATIVE_CITATION_LABEL_COUNT: usize = 38;

/// Highest column *index* the reference-segmenter model references.
///
/// From the model header `#rdr#70/25/0`.
pub const NATIVE_REFERENCE_SEGMENTER_MODEL_MAX_COLUMN_INDEX: usize = 25;
/// Printed reference-segmenter row width (the `email` lane is commented out).
pub const NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS: usize = 28;
/// Labels declared by `reference-segmenter/model.wapiti` (boundary family).
pub const NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT: usize = 6;

/// The training-label placeholder both `printVector`s emit for an unlabelled row.
pub const NATIVE_TRAINING_LABEL_PLACEHOLDER: &str = "0";

/// Label ids of `reference-segmenter/model.wapiti`, in model order.
///
/// A bare tag begins a span and `I-` continues it, but GROBID groups runs by
/// base name rather than by the BIO prefix, so the two spellings of one base are
/// adjacent in the table without implying a strict alternation.
pub const NATIVE_REFERENCE_SEGMENTER_LABELS: [&str; NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT] = [
    "I-<other>",
    "I-<label>",
    "<label>",
    "I-<reference>",
    "<reference>",
    "<other>",
];

/// Label ids of `citation/model.wapiti`, in model order.
///
/// The order is a verbatim transcription of the model's label block: it is *not*
/// grouped by role, not sorted, and not paired as `<x>, I-<x>`. Label ids index
/// the weight blocks, so this list is the only admissible order.
pub const NATIVE_CITATION_LABELS: [&str; NATIVE_CITATION_LABEL_COUNT] = [
    "I-<booktitle>",
    "<booktitle>",
    "I-<date>",
    "I-<other>",
    "I-<pubnum>",
    "<pubnum>",
    "<other>",
    "I-<note>",
    "<date>",
    "I-<author>",
    "<author>",
    "I-<title>",
    "<title>",
    "I-<publisher>",
    "<publisher>",
    "I-<journal>",
    "<journal>",
    "I-<volume>",
    "I-<pages>",
    "I-<collaboration>",
    "<collaboration>",
    "<pages>",
    "I-<issue>",
    "I-<location>",
    "<location>",
    "I-<editor>",
    "<editor>",
    "I-<series>",
    "<series>",
    "I-<institution>",
    "<institution>",
    "<issue>",
    "I-<tech>",
    "<tech>",
    "<note>",
    "I-<web>",
    "<web>",
    "<volume>",
];

/// One rendered citation row ([`NATIVE_CITATION_FEATURE_COLUMNS`] fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CitationFeatureRow {
    pub columns: Vec<String>,
}

impl CitationFeatureRow {
    /// Space-joined row, the exact payload `map_input` consumes per token.
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    /// Field count; always [`NATIVE_CITATION_FEATURE_COLUMNS`] for a built row.
    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

/// One rendered reference-segmenter row
/// ([`NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS`] fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSegmenterFeatureRow {
    pub columns: Vec<String>,
}

impl ReferenceSegmenterFeatureRow {
    /// Space-joined row, the exact payload `map_input` consumes per token.
    pub fn as_text(&self) -> String {
        self.columns.join(" ")
    }

    /// Field count; always [`NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS`].
    pub fn width(&self) -> usize {
        self.columns.len()
    }
}

/// Typed refusal vocabulary for the bibliography row builders.
///
/// Arity and row integrity are invariants, not hopes: a row the model would
/// silently mis-read is refused here instead of being emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBibliographyFeatureError {
    /// The token list is empty, so there is no block to render.
    EmptyInput,
    /// A token text is empty; `printVector` would return no row at all.
    EmptyToken { token: usize },
    /// A token text contains whitespace, which would corrupt the row arity.
    TokenContainsWhitespace { token: usize },
    /// A line's tokens disagree about the line's raw length.
    InconsistentLineLength {
        token: usize,
        actual: usize,
        expected: usize,
    },
    /// The block maximum is below the line it is supposed to bound.
    InvalidMaxLineLength {
        line_length: usize,
        max_line_length: usize,
    },
    /// The provider bundle behind the lexical lanes failed validation.
    InvalidLexicon(String),
    /// The rendered row is not the width the model declares.
    WrongArity { actual: usize, expected: usize },
}

impl fmt::Display for NativeBibliographyFeatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "bibliography feature input is empty"),
            Self::EmptyToken { token } => {
                write!(formatter, "bibliography feature token {token} is empty")
            }
            Self::TokenContainsWhitespace { token } => write!(
                formatter,
                "bibliography feature token {token} contains whitespace"
            ),
            Self::InconsistentLineLength {
                token,
                actual,
                expected,
            } => write!(
                formatter,
                "bibliography line length at token {token} is {actual}; expected {expected}"
            ),
            Self::InvalidMaxLineLength {
                line_length,
                max_line_length,
            } => write!(
                formatter,
                "bibliography block maximum line length {max_line_length} is below line length \
                 {line_length}"
            ),
            Self::InvalidLexicon(error) => {
                write!(formatter, "lexicon bundle is invalid: {error}")
            }
            Self::WrongArity { actual, expected } => write!(
                formatter,
                "rendered bibliography row has {actual} columns; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for NativeBibliographyFeatureError {}

/// Build the exact 30-column citation rows for one citation block.
///
/// Returns one row per admitted token, in input order. `spans` must have been
/// computed over the *same* token texts, in the same order: the gazetteer lanes
/// are positional and a misaligned list would silently flag the wrong tokens.
pub fn citation_row(
    tokens: &[BibliographyTokenFacts],
    spans: &BibliographyGazetteerSpans,
    lexicon: &LexiconBundle,
) -> Result<Vec<CitationFeatureRow>, NativeBibliographyFeatureError> {
    if tokens.is_empty() {
        return Err(NativeBibliographyFeatureError::EmptyInput);
    }
    lexicon
        .validate()
        .map_err(|error| NativeBibliographyFeatureError::InvalidLexicon(error.to_string()))?;
    let sentence_length = tokens.len();
    let mut rows = Vec::with_capacity(sentence_length);
    for (position, token) in tokens.iter().enumerate() {
        validate_token(position, token)?;
        let flags = name_flags(&token.text, lexicon);
        let columns = vec![
            // 0: normalised token text (normalisation is the producer's job).
            token.text.clone(),
            // 1: lowercase string.
            token.lowercase.clone(),
            // 2-5: prefix 1..4.
            token.prefix[0].clone(),
            token.prefix[1].clone(),
            token.prefix[2].clone(),
            token.prefix[3].clone(),
            // 6-9: suffix 1..4.
            token.suffix[0].clone(),
            token.suffix[1].clone(),
            token.suffix[2].clone(),
            token.suffix[3].clone(),
            // 10: lineStatus.
            token.line_status.as_str().to_string(),
            // 11: capitalisation as printed (`NOCAPS` when `digit == ALLDIGIT`).
            token.printed_capitalisation().as_str().to_string(),
            // 12: digit.
            token.digit.as_str().to_string(),
            // 13: singleChar.
            bool_flag(token.single_char),
            // 14-17: lexical lanes fed by the provider bundle.
            bool_flag(flags.proper_name),
            bool_flag(flags.common_name),
            bool_flag(flags.first_name),
            bool_flag(flags.last_name),
            // 18: isKnownLocation — gazetteer position list, not membership.
            bool_flag(spans.covers(BibliographyGazetteerLane::LocationNames, position)),
            // 19: year.
            bool_flag(token.year),
            // 20: month.
            bool_flag(token.month),
            // 21: http — the gazetteer url lane, never the `http` fact.
            bool_flag(spans.covers(BibliographyGazetteerLane::UrlPattern, position)),
            // 22: isKnownCollaboration.
            bool_flag(spans.covers(BibliographyGazetteerLane::CollaborationNames, position)),
            // 23: journal or abbreviated journal.
            bool_flag(
                spans.covers(BibliographyGazetteerLane::JournalNames, position)
                    || spans.covers(BibliographyGazetteerLane::AbbrevJournalNames, position),
            ),
            // 24: isKnownConferenceTitle.
            bool_flag(spans.covers(BibliographyGazetteerLane::ConferenceNames, position)),
            // 25: isKnownPublisher.
            bool_flag(spans.covers(BibliographyGazetteerLane::PublisherNames, position)),
            // 26: isKnownIdentifier.
            bool_flag(spans.covers(BibliographyGazetteerLane::IdentifierPattern, position)),
            // 27: punctType.
            token.punct_type.as_str().to_string(),
            // 28: relativePosition, scaled against the block length with 12 bins.
            relative_position_citation(position, sentence_length).to_string(),
            // 29: training label placeholder.
            NATIVE_TRAINING_LABEL_PLACEHOLDER.to_string(),
        ];
        let row = CitationFeatureRow { columns };
        if row.width() != NATIVE_CITATION_FEATURE_COLUMNS {
            return Err(NativeBibliographyFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_CITATION_FEATURE_COLUMNS,
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Build the exact 28-column reference-segmenter rows for one *line*.
///
/// The reference segmenter decodes line by line: `alignmentStatus` applies to
/// the whole line, `relativePosition` is the token's position in the line, and
/// the punctuation profile is the profile of the line. `max_line_length` is the
/// block-level maximum the line is scaled against, so a caller that batches
/// lines must pass the same value for every line of the block.
pub fn reference_segmenter_row(
    tokens: &[BibliographyTokenFacts],
    lexicon: &LexiconBundle,
    max_line_length: usize,
) -> Result<Vec<ReferenceSegmenterFeatureRow>, NativeBibliographyFeatureError> {
    if tokens.is_empty() {
        return Err(NativeBibliographyFeatureError::EmptyInput);
    }
    lexicon
        .validate()
        .map_err(|error| NativeBibliographyFeatureError::InvalidLexicon(error.to_string()))?;
    for (position, token) in tokens.iter().enumerate() {
        validate_token(position, token)?;
    }
    let line_length = tokens[0].line_length;
    if let Some((index, token)) = tokens
        .iter()
        .enumerate()
        .find(|(_, token)| token.line_length != line_length)
    {
        return Err(NativeBibliographyFeatureError::InconsistentLineLength {
            token: index,
            actual: token.line_length,
            expected: line_length,
        });
    }
    if max_line_length < line_length {
        return Err(NativeBibliographyFeatureError::InvalidMaxLineLength {
            line_length,
            max_line_length,
        });
    }
    // Java accumulates the line text from its raw tokens; spaces are their own
    // tokens there and are skipped by `punctuationProfile` anyway, so joining the
    // admitted tokens reproduces the same profile.
    let line_text: String = tokens.iter().map(|token| token.text.as_str()).collect();
    let profile = punctuation_profile_field(&punctuation_profile(&line_text));
    let line_length_column = linear_scaling(
        scaled_i32(line_length),
        scaled_i32(max_line_length),
        REFERENCE_SEGMENTER_LINE_SCALE,
    )
    .to_string();
    let mut rows = Vec::with_capacity(tokens.len());
    let mut position = 0usize;
    for token in tokens.iter() {
        // Java advances `nn` by the token's own length *before* emitting the row,
        // so the printed position is the token's end offset in the line. The
        // width is a UTF-16 code-unit count because that is what `String.length()`
        // measures.
        position += token.text.encode_utf16().count();
        let flags = name_flags(&token.text, lexicon);
        let columns = vec![
            // 0: normalised token text.
            token.text.clone(),
            // 1: lowercase string.
            token.lowercase.clone(),
            // 2-5: prefix 1..4.
            token.prefix[0].clone(),
            token.prefix[1].clone(),
            token.prefix[2].clone(),
            token.prefix[3].clone(),
            // 6-9: suffix 1..4.
            token.suffix[0].clone(),
            token.suffix[1].clone(),
            token.suffix[2].clone(),
            token.suffix[3].clone(),
            // 10: lineStatus.
            token.line_status.as_str().to_string(),
            // 11: alignmentStatus, applied to the whole line.
            token.alignment_status.as_str().to_string(),
            // 12: capitalisation as printed.
            token.printed_capitalisation().as_str().to_string(),
            // 13: digit.
            token.digit.as_str().to_string(),
            // 14: singleChar.
            bool_flag(token.single_char),
            // 15-16: lexical lanes fed by the provider bundle.
            bool_flag(flags.proper_name),
            bool_flag(flags.common_name),
            // 17-18: `firstName` and `locationName` are marked
            // `/* TODO: to review, never set! */` upstream and are never
            // assigned, so the emitted value is `0` whatever a lexicon says.
            bool_flag(false),
            bool_flag(false),
            // 19: year.
            bool_flag(token.year),
            // 20: month.
            bool_flag(token.month),
            // 21: http — the `FeatureFactory.http` pattern; the reference
            // segmenter has no gazetteer url lane.
            bool_flag(token.http),
            // 22: punctType.
            token.punct_type.as_str().to_string(),
            // 23: relativePosition, scaled against the line with LINESCALE.
            relative_position_reference_segmenter(position, line_length).to_string(),
            // 24: lineLength, the line scaled against the block maximum.
            line_length_column.clone(),
            // 25: blockStatus.
            token.block_status.as_str().to_string(),
            // 26: punctuationProfile (`no` when the line has no punctuation).
            profile.clone(),
            // 27: training label placeholder.
            NATIVE_TRAINING_LABEL_PLACEHOLDER.to_string(),
        ];
        let row = ReferenceSegmenterFeatureRow { columns };
        if row.width() != NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS {
            return Err(NativeBibliographyFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// Serialize citation rows exactly as `map_input` expects: one row per line.
pub fn render_citation_feature_text(
    rows: &[CitationFeatureRow],
) -> Result<String, NativeBibliographyFeatureError> {
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        if row.width() != NATIVE_CITATION_FEATURE_COLUMNS {
            return Err(NativeBibliographyFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_CITATION_FEATURE_COLUMNS,
            });
        }
        lines.push(row.as_text());
    }
    Ok(lines.join("\n"))
}

/// Serialize reference-segmenter rows exactly as `map_input` expects.
pub fn render_reference_segmenter_feature_text(
    rows: &[ReferenceSegmenterFeatureRow],
) -> Result<String, NativeBibliographyFeatureError> {
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        if row.width() != NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS {
            return Err(NativeBibliographyFeatureError::WrongArity {
                actual: row.width(),
                expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
            });
        }
        lines.push(row.as_text());
    }
    Ok(lines.join("\n"))
}

fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.into()
}

/// A token the row format cannot carry is a typed refusal, never a silent skip:
/// an empty text would emit no row at all (`printVector` returns `null`) and a
/// whitespace character would shift every following field.
fn validate_token(
    index: usize,
    token: &BibliographyTokenFacts,
) -> Result<(), NativeBibliographyFeatureError> {
    if token.text.is_empty() {
        return Err(NativeBibliographyFeatureError::EmptyToken { token: index });
    }
    if token.text.chars().any(char::is_whitespace) {
        return Err(NativeBibliographyFeatureError::TokenContainsWhitespace { token: index });
    }
    Ok(())
}

/// Widen a length to the `i32` arithmetic of `FeatureFactory.linearScaling`,
/// saturating rather than wrapping.
fn scaled_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_bibliography_gazetteer::BibliographyGazetteerIndex;
    use crate::native_feature_facts::{
        BibliographyAlignment, BibliographyTokenInput, FactsBlockStatus, FactsLineStatus,
    };
    use da_ports::{FeatureTemplate, LexiconChannel};

    /// A valid provider bundle: channel keys must be ordered and unique, and the
    /// declared versions must match the port's expectations.
    fn fixture_lexicon() -> LexiconBundle {
        LexiconBundle {
            schema_version: "grobid-lexicon.v1".into(),
            resource_version: "0.9.0-crf".into(),
            channels: vec![
                LexiconChannel {
                    key: "JournalNames".into(),
                    entries: vec!["nature".into()],
                },
                LexiconChannel {
                    key: "LocationNames".into(),
                    entries: vec!["google".into()],
                },
                LexiconChannel {
                    key: "commonName".into(),
                    entries: vec!["google".into()],
                },
                LexiconChannel {
                    key: "firstName".into(),
                    entries: vec!["ashish".into()],
                },
                LexiconChannel {
                    key: "lastName".into(),
                    entries: vec!["vaswani".into()],
                },
            ],
            feature_templates: vec![FeatureTemplate {
                key: "citation".into(),
                channel_keys: vec!["commonName".into()],
                feature_inputs: vec!["token.text".into()],
            }],
        }
    }

    fn facts(
        text: &str,
        line_status: FactsLineStatus,
        block_status: FactsBlockStatus,
        line_length: usize,
    ) -> BibliographyTokenFacts {
        BibliographyTokenFacts::from_token(&BibliographyTokenInput {
            text: text.to_string(),
            block_status,
            line_status,
            alignment: BibliographyAlignment::Alignedleft,
            line_length,
            superscript: false,
        })
    }

    fn block(line_length: usize) -> Vec<BibliographyTokenFacts> {
        vec![
            facts(
                "Ashish",
                FactsLineStatus::Linestart,
                FactsBlockStatus::Blockstart,
                line_length,
            ),
            facts(
                "Google",
                FactsLineStatus::Linein,
                FactsBlockStatus::Blockin,
                line_length,
            ),
            facts(
                "2017",
                FactsLineStatus::Lineend,
                FactsBlockStatus::Blockend,
                line_length,
            ),
        ]
    }

    fn spans_for(lexicon: &LexiconBundle, texts: [&str; 3]) -> BibliographyGazetteerSpans {
        BibliographyGazetteerIndex::from_lexicon(lexicon)
            .expect("fixture lexicon is valid")
            .spans(&texts)
            .expect("three tokens are within the gazetteer bounds")
    }

    fn fields(columns: &[String]) -> Vec<&str> {
        columns.iter().map(String::as_str).collect()
    }

    #[test]
    fn citation_row_emits_the_exact_thirty_column_abi() {
        let lexicon = fixture_lexicon();
        let tokens = block(16);
        let spans = spans_for(&lexicon, ["Ashish", "Google", "2017"]);
        let rows = citation_row(&tokens, &spans, &lexicon).expect("fixture renders");
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .all(|row| row.width() == NATIVE_CITATION_FEATURE_COLUMNS)
        );

        let first = fields(&rows[0].columns);
        assert_eq!(
            first[..11],
            [
                "Ashish",
                "ashish",
                "A",
                "As",
                "Ash",
                "Ashi",
                "h",
                "sh",
                "ish",
                "hish",
                "LINESTART"
            ]
        );
        // Columns 14-17 are the lexical lanes, and they come from the bundle the
        // builder was handed (the facts never had flags attached).
        assert_eq!(first[14..18], ["1", "0", "1", "0"]);
        assert_eq!(first[12], "NODIGIT");
        assert_eq!(first[19], "0");
        assert_eq!(first[27], "NOPUNCT");

        let second = fields(&rows[1].columns);
        assert_eq!(second[14..18], ["0", "1", "0", "0"]);
        // Column 18 is a gazetteer span (`google` is in the location lane).
        assert_eq!(second[18], "1");
        assert_eq!(second[27], "NOPUNCT");

        let third = fields(&rows[2].columns);
        // `2017` is ALLCAP by the ladder and NOCAPS as printed.
        assert_eq!(third[11], "NOCAPS");
        assert_eq!(third[12], "ALLDIGIT");
        assert_eq!(third[19], "1");
        assert_eq!(third[20], "0");

        // Column 28 scales the token position against the block length: 0/3, 1/3
        // and 2/3 mapped onto 12 bins.
        assert_eq!(first[28], "0");
        assert_eq!(second[28], "4");
        assert_eq!(third[28], "8");
        assert!(rows.iter().all(|row| row.columns[29] == "0"));

        let text = render_citation_feature_text(&rows).expect("rows are the declared width");
        assert_eq!(text.lines().count(), 3);
        assert!(text.lines().all(|line| line.split(' ').count() == 30));
    }

    #[test]
    fn citation_block_lanes_use_positions_not_membership() {
        let lexicon = fixture_lexicon();
        let tokens = block(16);
        let spans = spans_for(&lexicon, ["Ashish", "Google", "2017"]);
        let rows = citation_row(&tokens, &spans, &lexicon).expect("fixture renders");
        // No lane covers `Ashish` or `2017`, so every gazetteer column is zero
        // there even though the lexical name lanes are not.
        for row in [&rows[0], &rows[2]] {
            let columns = fields(&row.columns);
            assert_eq!(
                columns[18], "0",
                "location lane must not fire on a non-location token"
            );
            assert_eq!(columns[23], "0");
            assert_eq!(columns[26], "0");
        }
        assert!(spans.covers(BibliographyGazetteerLane::LocationNames, 1));
        assert!(!spans.covers(BibliographyGazetteerLane::LocationNames, 0));
    }

    #[test]
    fn reference_segmenter_row_emits_28_columns_without_the_email_lane() {
        let lexicon = fixture_lexicon();
        let tokens = block(16);
        let rows = reference_segmenter_row(&tokens, &lexicon, 16).expect("fixture renders");
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .all(|row| row.width() == NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS),
            "the commented-out `email` lane must not reappear"
        );

        let first = fields(&rows[0].columns);
        assert_eq!(first[10], "LINESTART");
        assert_eq!(first[11], "ALIGNEDLEFT");
        assert_eq!(first[14], "0");
        // Proper name from the bundle; `firstName` (17) and `locationName` (18)
        // are never set upstream and stay `0` even though `ashish` is in the
        // first-name channel and fires the citation lane.
        assert_eq!(first[15], "1");
        assert_eq!(first[16], "0");
        assert_eq!(first[17], "0");
        assert_eq!(first[18], "0");
        assert_eq!(first[22], "NOPUNCT");
        assert_eq!(first[25], "BLOCKSTART");
        assert_eq!(first[26], "no");
        assert_eq!(first[27], "0");

        let second = fields(&rows[1].columns);
        assert_eq!(second[15], "0");
        assert_eq!(second[16], "1");
        assert_eq!(second[25], "BLOCKIN");

        let third = fields(&rows[2].columns);
        assert_eq!(third[12], "NOCAPS");
        assert_eq!(third[13], "ALLDIGIT");
        assert_eq!(third[19], "1");
        assert_eq!(third[25], "BLOCKEND");

        // Positions are end offsets within the line (6, 12, 16 of 16) scaled by
        // LINESCALE, and the line is scaled against the block maximum.
        assert_eq!(first[23], "3");
        assert_eq!(second[23], "7");
        assert_eq!(third[23], "10");
        assert!(rows.iter().all(|row| row.columns[24] == "10"));

        let text =
            render_reference_segmenter_feature_text(&rows).expect("rows are the declared width");
        assert_eq!(text.lines().count(), 3);
        assert!(text.lines().all(|line| line.split(' ').count() == 28));
    }

    #[test]
    fn reference_segmenter_profile_and_punctuation_are_line_scoped() {
        let lexicon = fixture_lexicon();
        let tokens = vec![
            facts(
                "Smith",
                FactsLineStatus::Linestart,
                FactsBlockStatus::Blockstart,
                11,
            ),
            facts(",", FactsLineStatus::Linein, FactsBlockStatus::Blockin, 11),
            facts(
                "2020",
                FactsLineStatus::Linein,
                FactsBlockStatus::Blockin,
                11,
            ),
            facts(
                ".",
                FactsLineStatus::Lineend,
                FactsBlockStatus::Blockend,
                11,
            ),
        ];
        let rows = reference_segmenter_row(&tokens, &lexicon, 22).expect("fixture renders");
        let columns: Vec<&str> = fields(&rows[1].columns);
        assert_eq!(columns[22], "COMMA");
        assert_eq!(fields(&rows[3].columns)[22], "DOT");
        assert_eq!(fields(&rows[3].columns)[14], "1");
        // Profile of `Smith,2020.` is `,.`, i.e. length 2.
        assert!(rows.iter().all(|row| row.columns[26] == "2"));
        // 11 of a 22-character block maximum, on a 10-bin scale.
        assert!(rows.iter().all(|row| row.columns[24] == "5"));
        assert_eq!(columns[23], "5");
        assert_eq!(fields(&rows[3].columns)[23], "10");
    }

    #[test]
    fn http_lane_is_pattern_backed_for_references_and_gazetteer_backed_for_citations() {
        let lexicon = fixture_lexicon();
        let tokens = vec![facts(
            "nothttp",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            7,
        )];
        assert!(
            tokens[0].http,
            "`Pattern(\"http(s)?\")` is unanchored, so `nothttp` satisfies the fact"
        );
        let spans = BibliographyGazetteerIndex::from_lexicon(&lexicon)
            .expect("fixture lexicon is valid")
            .spans(&["nothttp"])
            .expect("one token is within the gazetteer bounds");
        let citation = citation_row(&tokens, &spans, &lexicon).expect("fixture renders");
        assert_eq!(
            fields(&citation[0].columns)[21],
            "0",
            "the citation http column reads the gazetteer url lane, not the http fact"
        );
        let reference = reference_segmenter_row(&tokens, &lexicon, 7).expect("fixture renders");
        assert_eq!(
            fields(&reference[0].columns)[21],
            "1",
            "the reference segmenter has no gazetteer pass and prints the http fact"
        );
    }

    #[test]
    fn row_builders_refuse_malformed_inputs() {
        let lexicon = fixture_lexicon();
        let spans = spans_for(&lexicon, ["Ashish", "Google", "2017"]);
        assert_eq!(
            citation_row(&[], &spans, &lexicon),
            Err(NativeBibliographyFeatureError::EmptyInput)
        );
        assert_eq!(
            reference_segmenter_row(&[], &lexicon, 16),
            Err(NativeBibliographyFeatureError::EmptyInput)
        );

        let empty = vec![facts(
            "",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            0,
        )];
        let empty_spans = BibliographyGazetteerIndex::from_lexicon(&lexicon)
            .expect("fixture lexicon is valid")
            .spans(&[""])
            .expect("one token is within the gazetteer bounds");
        assert_eq!(
            citation_row(&empty, &empty_spans, &lexicon),
            Err(NativeBibliographyFeatureError::EmptyToken { token: 0 })
        );
        assert_eq!(
            reference_segmenter_row(&empty, &lexicon, 0),
            Err(NativeBibliographyFeatureError::EmptyToken { token: 0 })
        );

        let spaced = vec![facts(
            "bad token",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            9,
        )];
        let spaced_spans = BibliographyGazetteerIndex::from_lexicon(&lexicon)
            .expect("fixture lexicon is valid")
            .spans(&["bad token"])
            .expect("one token is within the gazetteer bounds");
        assert_eq!(
            citation_row(&spaced, &spaced_spans, &lexicon),
            Err(NativeBibliographyFeatureError::TokenContainsWhitespace { token: 0 })
        );
        assert_eq!(
            reference_segmenter_row(&spaced, &lexicon, 9),
            Err(NativeBibliographyFeatureError::TokenContainsWhitespace { token: 0 })
        );
        // U+00A0 is whitespace to the row reader as well, so it is refused rather
        // than smuggled into a field.
        let nbsp = vec![facts(
            "a\u{a0}b",
            FactsLineStatus::Linestart,
            FactsBlockStatus::Blockstart,
            3,
        )];
        assert!(matches!(
            reference_segmenter_row(&nbsp, &lexicon, 3),
            Err(NativeBibliographyFeatureError::TokenContainsWhitespace { token: 0 })
        ));
    }

    #[test]
    fn reference_segmenter_row_refuses_cross_line_and_impossible_geometry() {
        let lexicon = fixture_lexicon();
        let mut tokens = block(16);
        tokens[1].line_length = 17;
        assert_eq!(
            reference_segmenter_row(&tokens, &lexicon, 17),
            Err(NativeBibliographyFeatureError::InconsistentLineLength {
                token: 1,
                actual: 17,
                expected: 16,
            })
        );

        let tokens = block(16);
        assert_eq!(
            reference_segmenter_row(&tokens, &lexicon, 15),
            Err(NativeBibliographyFeatureError::InvalidMaxLineLength {
                line_length: 16,
                max_line_length: 15,
            })
        );
    }

    #[test]
    fn row_builders_refuse_an_invalid_lexicon() {
        let mut lexicon = fixture_lexicon();
        lexicon.schema_version = "grobid-lexicon.v2".into();
        let tokens = block(16);
        let spans = BibliographyGazetteerSpans::default();
        assert!(matches!(
            citation_row(&tokens, &spans, &lexicon),
            Err(NativeBibliographyFeatureError::InvalidLexicon(_))
        ));
        assert!(matches!(
            reference_segmenter_row(&tokens, &lexicon, 16),
            Err(NativeBibliographyFeatureError::InvalidLexicon(_))
        ));
    }

    #[test]
    fn renderers_refuse_a_row_of_the_wrong_width() {
        let citation = CitationFeatureRow {
            columns: vec!["0".into()],
        };
        assert_eq!(
            render_citation_feature_text(&[citation]),
            Err(NativeBibliographyFeatureError::WrongArity {
                actual: 1,
                expected: NATIVE_CITATION_FEATURE_COLUMNS,
            })
        );
        let reference = ReferenceSegmenterFeatureRow {
            columns: vec!["0".into(); NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS + 1],
        };
        assert_eq!(
            render_reference_segmenter_feature_text(&[reference]),
            Err(NativeBibliographyFeatureError::WrongArity {
                actual: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS + 1,
                expected: NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS,
            })
        );
    }

    #[test]
    fn label_tables_keep_model_order_and_arity() {
        assert_eq!(
            NATIVE_REFERENCE_SEGMENTER_LABELS,
            [
                "I-<other>",
                "I-<label>",
                "<label>",
                "I-<reference>",
                "<reference>",
                "<other>",
            ]
        );
        assert_eq!(
            NATIVE_REFERENCE_SEGMENTER_LABELS.len(),
            NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT
        );
        assert_eq!(NATIVE_CITATION_LABELS.len(), NATIVE_CITATION_LABEL_COUNT);
        assert_eq!(
            NATIVE_CITATION_LABELS[..4],
            ["I-<booktitle>", "<booktitle>", "I-<date>", "I-<other>"]
        );
        assert_eq!(NATIVE_CITATION_LABELS[36..], ["<web>", "<volume>"]);

        // 19 roles, each with exactly a begin and an inside spelling.
        let mut bases: Vec<&str> = NATIVE_CITATION_LABELS
            .iter()
            .map(|label| label.strip_prefix("I-").unwrap_or(label))
            .collect();
        bases.sort_unstable();
        bases.dedup();
        assert_eq!(bases.len(), NATIVE_CITATION_ROLE_COUNT);
        for base in bases {
            // `base` already carries the angle brackets (`<author>`), so the two
            // admissible spellings are the base itself and its `I-` form.
            let begin = base.to_string();
            let inside = format!("I-{base}");
            let count = NATIVE_CITATION_LABELS
                .iter()
                .filter(|label| **label == begin || **label == inside)
                .count();
            assert_eq!(count, 2, "role {base} must have one begin and one inside");
        }
    }

    #[test]
    fn declared_sizes_match_the_model_headers() {
        assert_eq!(NATIVE_CITATION_MODEL_MAX_COLUMN_INDEX, 28);
        assert_eq!(NATIVE_CITATION_FEATURE_COLUMNS, 30);
        assert_eq!(NATIVE_REFERENCE_SEGMENTER_MODEL_MAX_COLUMN_INDEX, 25);
        assert_eq!(NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS, 28);
        assert_eq!(NATIVE_TRAINING_LABEL_PLACEHOLDER, "0");
        assert_eq!(
            NATIVE_BIBLIOGRAPHY_FEATURES_SCHEMA_VERSION,
            "da-parse.native-bibliography-features/v1"
        );
    }
}
