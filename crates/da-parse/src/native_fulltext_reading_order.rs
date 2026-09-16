//! Cross-page reading order for the native GROBID fulltext (body) band.
//!
//! GROBID does not let the fulltext CRF see a page: `FullTextParser
//! .getBodyTextFeatured` walks the admitted body pieces as one token rope across
//! pages, derives the per-token block/line/alignment facts, and only then hands
//! the printed rows to the model. Reading order and cross-page continuation are
//! therefore consequences of *this* glue layer, not of the CRF — which is why
//! this module exists separately from [`crate::native_fulltext_features`] (the
//! printed ABI) and [`crate::native_fulltext_sequence`] (the decode).
//!
//! ## What this module reproduces from Java
//!
//! * **Page transition.** In `getBodyTextFeatured`, `localPage != currentPage`
//!   sets `newPage` and resets the per-page state: `mm` (the intra-page
//!   accumulated token length), `lowestPos` and `spacingPreviousBlock`. This
//!   module records that transition as [`PageReadingSpan::page_break`] and the
//!   final `mm` of each page as [`PageReadingSpan::intra_page_length`], so the
//!   reset is an observable fact rather than an implicit side effect.
//! * **Relative positions.** `relativeDocumentPosition` is
//!   `linearScaling(nn, fulltextLength, NBBINS_POSITION)` with `nn` accumulating
//!   over the whole body and `fulltextLength` the whole-body length;
//!   `relativePagePosition` is `linearScaling(coordinateLineY, pageHeight,
//!   NBBINS_POSITION)` clamped to the top bin. Both lanes are filled here and
//!   printed by [`crate::native_fulltext_features`].
//! * **Column shift.** A block that starts *above* the lowest position reached
//!   so far on the page (`lowestPos > block.getY()`) is a new column or a layout
//!   jump. Java substitutes `doc.getMaxBlockSpacing() / 5.0` for the spacing
//!   weight in that case; that weight is commented out in
//!   `FeaturesVectorFulltext.printVector`, so this module records the *fact*
//!   ([`ColumnShift`]) and the raw vertical delta instead of inventing a
//!   document statistic it does not have.
//! * **Block/line/alignment status.** The status chain (LINESTART, LINEIN,
//!   LINEEND, BLOCKSTART, BLOCKIN, BLOCKEND, LINEINDENT/ALIGNEDLEFT) and its two
//!   post-corrections of the *previous* token are assigned in the same order as
//!   the Java loop; the token is therefore emitted one step late, exactly as
//!   Java emits `previousFeatures`.
//!
//! ## Document order is derived, never trusted
//!
//! The caller hands over *layout facts* — page number, block ordinal,
//! within-block ordinal — and this module derives document order from them
//! (`(page, block, within_block)`, ascending). Shuffling the input list cannot
//! change the reading, and two inputs that declare the same facts have the same
//! identity. Declaring *different* facts — including swapping the page numbers
//! of two pages' contents — is a different reading with a different
//! [`NativeFulltextReadingOrder::digest`], even when the multiset of decoded
//! labels is unchanged. Identity depends on order, not on the label bag.
//!
//! ## Named gaps (honest limits, not silent approximations)
//!
//! * **Both spellings of `pageLength`.** Java declares `pageLength = 0` and
//!   never maintains it, so `relativePagePositionChar` (commented out of
//!   `printVector`) always resolves to the top bin. This module reproduces the
//!   *live* consequence — the per-page accumulator reset, exposed as
//!   `intra_page_length` — and does not fabricate a page length Java never
//!   computed.
//! * **DocumentPointer trimming.** Java admits partial first/last blocks by
//!   `DocumentPointer` token offsets and clamps desynchronised pointers. This
//!   band's typed input is whole blocks (see [`NativeFulltextReadingOrderError`]);
//!   piece-level trimming stays with the segmentation layer.
//! * **Font lanes are stateful and live in the row builder.** `fontStatus` and
//!   `fontSize` compare each token with the previous one, and
//!   [`render_fulltext_features`] already owns that state, so this module passes
//!   the raw font identity through untouched.
//! * **Zero-row body.** A body made only of line-break tokens yields no rows.
//!   That is legal here (Java's caller tests `isNotBlank`), so the order carries
//!   an empty row vector rather than the row builder's `EmptyInput` refusal.
//!
//! Nothing here reads a file, an environment variable, or a clock, and no atom
//! of this band is spliced into the Java-authoritative parse route:
//! `import_eligible` stays false.

use crate::native_feature_facts::{FactsBlockStatus, FactsLineStatus};
use crate::native_fulltext_features::{
    FulltextAlignment, FulltextCalloutType, FulltextFeatureRow, FulltextFeatureToken,
    NATIVE_FULLTEXT_FEATURE_COLUMNS, render_fulltext_features,
};
use crate::native_fulltext_sequence::NativeFulltextDecodeCandidate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the glued reading-order surface.
pub const NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION: &str =
    "da-parse.native-fulltext-reading-order/v1";

/// Upper bound on distinct pages admitted into one glue.
pub const MAX_FULLTEXT_PAGES: usize = 4_096;

/// Upper bound on admitted layout tokens (line-break tokens included).
///
/// Sits at [`crate::MAX_NATIVE_FULLTEXT_FEATURE_ROWS`]: the glue may admit at
/// most as many tokens as the row builder prints rows, so no ordering can ever
/// push more rows into the decode than the sequence runner accepts.
pub const MAX_FULLTEXT_TOKENS: usize = crate::MAX_NATIVE_FULLTEXT_FEATURE_ROWS;

/// Java `TextUtilities.filterLine`: empty text, or a pseudo-token carrying
/// `@IMAGE`/`@PAGE`, is dropped before the token loop and emits no row.
fn is_filtered_line(text: &str) -> bool {
    text.is_empty() || text.contains("@IMAGE") || text.contains("@PAGE")
}

/// One token's payload as the layout producer declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeFulltextTokenText {
    /// A word token. Java's `LayoutToken.getText()`; never empty and never
    /// whitespace (the typed input must be sanitized, and Java's own
    /// space-stripping path is a named gap).
    Word { text: String },
    /// Java's line-break token `"\n"`: it emits no row, sets the newline flag
    /// for the next word token, and advances both position accumulators by one.
    LineBreak,
}

impl NativeFulltextTokenText {
    /// Java's contribution of this token to `nn`/`mm` (and to
    /// `getFulltextLength`): `text.length()` after sanitization, one for `"\n"`.
    pub fn length(&self) -> i32 {
        match self {
            Self::Word { text } => text.chars().count() as i32,
            Self::LineBreak => 1,
        }
    }
}

/// One layout token as the fulltext glue needs it.
///
/// Every field the Java loop reads from a `LayoutToken` is an input, and every
/// field the loop *derives* (block/line status, alignment, positions) is not.
/// `block_y`, `block_height` and `page_height` are block/page scoped in Java but
/// carried per token here so the input stays a flat rope; all tokens of one
/// block must declare the same values, which is validated fail-closed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFulltextPageToken {
    /// `block.getPage().getNumber()`.
    pub page: u32,
    /// `block.getPage().getHeight()`.
    pub page_height: f64,
    /// Block ordinal inside the page/document order (`blockPtr`).
    pub block: u32,
    /// `block.getY()`.
    pub block_y: f64,
    /// `block.getHeight()`.
    pub block_height: f64,
    /// Absolute token index inside the block (Java's `n`).
    pub within_block: u32,
    /// `block.getTokens().size()` — the block's full token count, so the
    /// last-token test is Java's `n == tokens.size() - 1`.
    pub block_token_count: u32,
    /// `token.getX()` — the line-start abscissa the indentation test compares.
    pub x: f64,
    /// `token.width` — the width the indentation test divides by the token
    /// length.
    pub width: f64,
    /// `token.getY()` — the line ordinate `relativePagePosition` scales.
    pub y: f64,
    /// `token.getFont()` — the font *name* `fontStatus` compares.
    pub font: String,
    /// `token.getFontSize()`, truncated by the row builder exactly as Java does.
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    /// Block-scoped graphic proximity (`bitmapAround`).
    pub bitmap_around: bool,
    pub callout_type: FulltextCalloutType,
    pub callout_known: bool,
    pub superscript: bool,
    pub text: NativeFulltextTokenText,
}

/// Page-scoped reading facts: where the page starts in the row sequence, and
/// what the per-page accumulator held when the page ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageReadingSpan {
    pub page: u32,
    /// Java's `newPage`: the page number differs from the previously admitted
    /// page (true for the document's first page). This transition is exactly
    /// where `mm`, `lowestPos` and `spacingPreviousBlock` reset.
    pub page_break: bool,
    /// Index of this page's first row in document order.
    pub first_row: usize,
    /// Number of rows emitted on this page.
    pub row_count: usize,
    /// Java's `mm` at the end of this page: the sanitized token length
    /// accumulated *on this page*, reset to zero at every page break.
    pub intra_page_length: i32,
}

/// A block that starts above the lowest position already reached on its page.
///
/// Java treats `lowestPos > block.getY()` as a vertical shift (a column change
/// or an unusual layout) and substitutes `doc.getMaxBlockSpacing() / 5.0` for
/// the spacing weight. That weight is not printed (`printVector` comments the
/// spacing lane out), so the native band records the typed fact and the raw
/// delta instead of a document statistic it does not model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnShift {
    pub page: u32,
    pub block: u32,
    /// `lowestPos > block.getY()`.
    pub column_change: bool,
    /// `block.getY() - lowestPos`; negative exactly when `column_change` is
    /// true.
    pub vertical_delta: f64,
    /// The weight Java substitutes in the shift case, which this band does not
    /// model (there is no document spacing statistic here). `None` for a
    /// column change, `Some(vertical_delta)` otherwise.
    pub spacing: Option<f64>,
}

/// One maximal T02 run annotated with the page span it covers.
///
/// The run itself is unchanged — a paragraph that starts on one page and ends on
/// the next stays *one* run, because the glue produced one token rope, not one
/// rope per page. `continued` is the typed statement of that cross-page span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossPageRun {
    pub label: String,
    pub start: usize,
    pub end: usize,
    pub first_page: u32,
    pub last_page: u32,
    /// `first_page != last_page`.
    pub continued: bool,
}

/// Glued reading order: the document-ordered tokens, their printed rows, the
/// page spans, the block shifts, and the reading identity digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFulltextReadingOrder {
    pub schema_version: String,
    /// `fulltextLength`: the admitted whole-body length in characters.
    pub document_length: i32,
    pub pages: Vec<PageReadingSpan>,
    pub column_shifts: Vec<ColumnShift>,
    /// Document-ordered, row-ready tokens (positions filled, font identity raw).
    pub tokens: Vec<FulltextFeatureToken>,
    /// The same tokens through the printed 27-column ABI, in the same order.
    pub rows: Vec<FulltextFeatureRow>,
    /// Deterministic reading identity over the ordered rows *and* their layout
    /// facts. Two readings with an equal label bag but a different order have
    /// different digests.
    pub digest: String,
}

impl NativeFulltextReadingOrder {
    /// Page of the row at `index`, or `None` when the index is out of range.
    pub fn page_of_row(&self, index: usize) -> Option<u32> {
        self.pages
            .iter()
            .find(|span| index >= span.first_row && index < span.first_row + span.row_count)
            .map(|span| span.page)
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Annotate T02's runs with the page span they cover.
    pub fn cross_page_runs(
        &self,
        decode: &NativeFulltextDecodeCandidate,
    ) -> Result<Vec<CrossPageRun>, NativeFulltextReadingOrderError> {
        cross_page_runs(self, decode)
    }
}

/// Typed refusal vocabulary of the fulltext reading-order glue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFulltextReadingOrderError {
    /// No layout tokens were supplied; an empty body has no reading order.
    EmptyInput,
    /// More tokens than [`MAX_FULLTEXT_TOKENS`].
    TooManyTokens { actual: usize, maximum: usize },
    /// More distinct pages than [`MAX_FULLTEXT_PAGES`].
    TooManyPages { actual: usize, maximum: usize },
    /// Two tokens declare the same `(page, block, within_block)`.
    DuplicateOrdinal {
        page: u32,
        block: u32,
        within_block: u32,
    },
    /// A token's within-block ordinal is outside its declared block size.
    OrdinalOutOfRange {
        page: u32,
        block: u32,
        within_block: u32,
        block_token_count: u32,
    },
    /// A block's admitted tokens are not the contiguous run
    /// `0..block_token_count - 1` Java's last-token test assumes.
    IncompleteBlock {
        page: u32,
        block: u32,
        expected: u32,
        actual: u32,
    },
    /// Two tokens of one block disagree on a block-scoped fact.
    BlockFactMismatch {
        page: u32,
        block: u32,
        field: &'static str,
    },
    /// A geometry input was not finite.
    NonFiniteGeometry {
        page: u32,
        block: u32,
        field: &'static str,
    },
    /// A word token was empty; Java would have dropped it before the loop.
    EmptyTokenText {
        page: u32,
        block: u32,
        within_block: u32,
    },
    /// A word token carried whitespace, which would corrupt the printed arity.
    TokenContainsWhitespace {
        page: u32,
        block: u32,
        within_block: u32,
    },
    /// The position accumulators exceeded Java's `int` range.
    PositionOverflow { field: &'static str },
    /// The printed row builder refused the glued tokens.
    Feature { message: String },
    /// A printed row did not carry the 27-column ABI.
    RowArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    /// The decode was produced over a different number of rows.
    DecodeRowCountMismatch { rows: usize, labels: usize },
    /// A decoded run lies outside the row range.
    RunOutOfBounds {
        start: usize,
        end: usize,
        rows: usize,
    },
}

impl fmt::Display for NativeFulltextReadingOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "native fulltext reading order input is empty"),
            Self::TooManyTokens { actual, maximum } => write!(
                formatter,
                "native fulltext reading order has {actual} tokens; maximum is {maximum}"
            ),
            Self::TooManyPages { actual, maximum } => write!(
                formatter,
                "native fulltext reading order has {actual} pages; maximum is {maximum}"
            ),
            Self::DuplicateOrdinal {
                page,
                block,
                within_block,
            } => write!(
                formatter,
                "native fulltext token ({page}, {block}, {within_block}) is declared twice"
            ),
            Self::OrdinalOutOfRange {
                page,
                block,
                within_block,
                block_token_count,
            } => write!(
                formatter,
                "native fulltext token ({page}, {block}, {within_block}) is outside its \
                 declared block size {block_token_count}"
            ),
            Self::IncompleteBlock {
                page,
                block,
                expected,
                actual,
            } => write!(
                formatter,
                "native fulltext block ({page}, {block}) admits {actual} of {expected} tokens"
            ),
            Self::BlockFactMismatch { page, block, field } => write!(
                formatter,
                "native fulltext block ({page}, {block}) disagrees on {field}"
            ),
            Self::NonFiniteGeometry { page, block, field } => write!(
                formatter,
                "native fulltext block ({page}, {block}) has a non-finite {field}"
            ),
            Self::EmptyTokenText {
                page,
                block,
                within_block,
            } => write!(
                formatter,
                "native fulltext token ({page}, {block}, {within_block}) is empty"
            ),
            Self::TokenContainsWhitespace {
                page,
                block,
                within_block,
            } => write!(
                formatter,
                "native fulltext token ({page}, {block}, {within_block}) contains whitespace"
            ),
            Self::PositionOverflow { field } => {
                write!(formatter, "native fulltext {field} exceeds the int range")
            }
            Self::Feature { message } => {
                write!(formatter, "native fulltext row rendering failed: {message}")
            }
            Self::RowArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "native fulltext row {row} has {actual} columns; expected {expected}"
            ),
            Self::DecodeRowCountMismatch { rows, labels } => write!(
                formatter,
                "native fulltext decode has {labels} labels for {rows} ordered rows"
            ),
            Self::RunOutOfBounds { start, end, rows } => write!(
                formatter,
                "native fulltext run {start}..{end} does not fit {rows} ordered rows"
            ),
        }
    }
}

impl std::error::Error for NativeFulltextReadingOrderError {}

/// Glue layout tokens into the document-ordered fulltext reading.
///
/// The caller's list order is irrelevant: the reading is derived from the
/// declared `(page, block, within_block)` facts, validated fail-closed against
/// duplicates, partial blocks, and non-finite geometry. Every emitted token
/// carries the status and position lanes the Java loop assigns, and the rows are
/// the printed 27-column ABI.
pub fn glue_fulltext_reading_order(
    tokens: &[NativeFulltextPageToken],
) -> Result<NativeFulltextReadingOrder, NativeFulltextReadingOrderError> {
    if tokens.is_empty() {
        return Err(NativeFulltextReadingOrderError::EmptyInput);
    }
    if tokens.len() > MAX_FULLTEXT_TOKENS {
        return Err(NativeFulltextReadingOrderError::TooManyTokens {
            actual: tokens.len(),
            maximum: MAX_FULLTEXT_TOKENS,
        });
    }
    for token in tokens {
        validate_token(token)?;
    }

    let mut ordered: Vec<&NativeFulltextPageToken> = tokens.iter().collect();
    ordered.sort_by_key(|token| (token.page, token.block, token.within_block));

    let page_count = ordered
        .iter()
        .map(|token| token.page)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if page_count > MAX_FULLTEXT_PAGES {
        return Err(NativeFulltextReadingOrderError::TooManyPages {
            actual: page_count,
            maximum: MAX_FULLTEXT_PAGES,
        });
    }

    let document_length = i32::try_from(
        ordered
            .iter()
            .map(|token| token.text.length() as i64)
            .sum::<i64>(),
    )
    .map_err(|_| NativeFulltextReadingOrderError::PositionOverflow {
        field: "document_length",
    })?;

    let blocks = group_blocks(&ordered)?;

    let mut state = GlueState::default();
    let mut page_events: Vec<PageEvent> = Vec::new();
    let mut column_shifts: Vec<ColumnShift> = Vec::new();
    let mut tokens_out: Vec<FulltextFeatureToken> = Vec::new();
    let mut row_places: Vec<(u32, u32)> = Vec::new();
    for block in &blocks {
        // Page transition: Java sets `newPage` and resets `mm`, `lowestPos` and
        // `spacingPreviousBlock` when the page number changes.
        let page_break = state.current_page != Some(block.page);
        if page_break {
            if let Some(mut open) = state.open_page.take() {
                open.intra_page_length = state.mm;
                page_events.push(open);
            }
            state.mm = 0;
            state.lowest_pos = 0.0;
            state.current_page = Some(block.page);
            state.open_page = Some(PageEvent {
                page: block.page,
                page_break: true,
                intra_page_length: 0,
            });
        }

        // Column shift: computed per visited block, before its tokens.
        let column_change = state.lowest_pos > block.block_y;
        let vertical_delta = block.block_y - state.lowest_pos;
        column_shifts.push(ColumnShift {
            page: block.page,
            block: block.block,
            column_change,
            vertical_delta,
            spacing: if column_change {
                None
            } else {
                Some(vertical_delta)
            },
        });

        let mut is_first_block_token = true;
        let block_len = block.tokens.len();
        for (index, token) in block.tokens.iter().enumerate() {
            match &token.text {
                NativeFulltextTokenText::LineBreak => {
                    // Java: `n++; mm++; nn++; continue;`
                    state.previous_newline = true;
                    state.mm += 1;
                    state.nn += 1;
                    continue;
                }
                NativeFulltextTokenText::Word { text } => {
                    if is_filtered_line(text) {
                        // `TextUtilities.filterLine`: dropped before the loop
                        // and before any position advance.
                        continue;
                    }

                    let mut newline = false;
                    if state.previous_newline {
                        newline = true;
                        state.previous_newline = false;
                        // Indentation is only updated once a previous row exists
                        // (Java guards on `previousFeatures != null`).
                        if state.pending.is_some() && !state.line_start_x.is_nan() {
                            let previous_line_start_x = state.line_start_x;
                            state.line_start_x = token.x;
                            let character_width = token.width / text.chars().count() as f64;
                            if previous_line_start_x - state.line_start_x > character_width {
                                state.indented = false;
                            } else if state.line_start_x - previous_line_start_x > character_width {
                                state.indented = true;
                            }
                        }
                    }

                    let alignment = if state.indented {
                        FulltextAlignment::Lineindent
                    } else {
                        FulltextAlignment::Alignedleft
                    };

                    let mut line_status = None;

                    if newline {
                        line_status = Some(FactsLineStatus::Linestart);
                        state.line_start_x = token.x;
                        state.close_previous_line_if_open();
                    }

                    let block_status = if is_first_block_token {
                        line_status = Some(FactsLineStatus::Linestart);
                        state.close_previous_line_if_open();
                        state.line_start_x = token.x;
                        FactsBlockStatus::Blockstart
                    } else if index == block_len - 1 {
                        line_status = Some(FactsLineStatus::Lineend);
                        state.previous_newline = true;
                        FactsBlockStatus::Blockend
                    } else {
                        let (endline, endblock) = look_ahead(block, index, text);
                        if !endline && !newline {
                            line_status = Some(FactsLineStatus::Linein);
                        } else if !newline {
                            line_status = Some(FactsLineStatus::Lineend);
                            state.previous_newline = true;
                        }
                        if endblock {
                            FactsBlockStatus::Blockend
                        } else {
                            FactsBlockStatus::Blockin
                        }
                    };

                    let token_out = FulltextFeatureToken {
                        text: text.clone(),
                        block_status,
                        line_status: line_status.expect("every branch assigns a line status"),
                        alignment,
                        font: token.font.clone(),
                        font_size: token.font_size,
                        bold: token.bold,
                        italic: token.italic,
                        document_position: state.nn,
                        document_length,
                        page_line_position: token.y,
                        page_height: token.page_height,
                        bitmap_around: token.bitmap_around,
                        callout_type: token.callout_type,
                        callout_known: token.callout_known,
                        superscript: token.superscript,
                    };

                    // Java's post-correction: a block start right behind a
                    // still-open block closes the previous token.
                    let closest = state.pending.as_mut().filter(|previous| {
                        token_out.block_status == FactsBlockStatus::Blockstart
                            && previous.token.block_status == FactsBlockStatus::Blockin
                    });
                    if let Some(previous) = closest {
                        previous.token.block_status = FactsBlockStatus::Blockend;
                        previous.token.line_status = FactsLineStatus::Lineend;
                    }

                    // Java emits `previousFeatures` one step late, which is what
                    // makes that correction observable.
                    if let Some(previous) = state.pending.take() {
                        row_places.push((previous.page, previous.block));
                        tokens_out.push(previous.token);
                    }
                    state.pending = Some(PendingRow {
                        page: block.page,
                        block: block.block,
                        token: token_out,
                    });

                    state.mm += token.text.length();
                    state.nn += token.text.length();
                    is_first_block_token = false;
                }
            }
        }

        // Java updates the block's lowest position after the token loop.
        state.lowest_pos = block.block_y + block.block_height;
    }

    if let Some(previous) = state.pending.take() {
        row_places.push((previous.page, previous.block));
        tokens_out.push(previous.token);
    }

    if let Some(mut open) = state.open_page.take() {
        open.intra_page_length = state.mm;
        page_events.push(open);
    }

    // Rows are emitted one step late, so a page's row range is only known once
    // every row has been flushed. Deriving the spans from the final row sequence
    // keeps the attribution exact and still reports a page whose only tokens are
    // line breaks with a zero row count.
    let mut pages: Vec<PageReadingSpan> = Vec::with_capacity(page_events.len());
    let mut cursor = 0usize;
    for event in &page_events {
        let first_row = cursor;
        while cursor < row_places.len() && row_places[cursor].0 == event.page {
            cursor += 1;
        }
        pages.push(PageReadingSpan {
            page: event.page,
            page_break: event.page_break,
            first_row,
            row_count: cursor - first_row,
            intra_page_length: event.intra_page_length,
        });
    }

    let rows = if tokens_out.is_empty() {
        Vec::new()
    } else {
        render_fulltext_features(&tokens_out).map_err(|error| {
            NativeFulltextReadingOrderError::Feature {
                message: error.to_string(),
            }
        })?
    };
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_FULLTEXT_FEATURE_COLUMNS {
            return Err(NativeFulltextReadingOrderError::RowArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FULLTEXT_FEATURE_COLUMNS,
            });
        }
    }

    Ok(NativeFulltextReadingOrder {
        schema_version: NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION.to_owned(),
        document_length,
        pages,
        column_shifts,
        digest: reading_identity(&row_places, &rows, document_length),
        tokens: tokens_out,
        rows,
    })
}

/// Annotate T02's runs with the page span they cover.
///
/// The run structure is T02's, not a second model: a run is only annotated, so a
/// paragraph that continues on the next page stays one run and gains
/// `continued = true`.
pub fn cross_page_runs(
    order: &NativeFulltextReadingOrder,
    decode: &NativeFulltextDecodeCandidate,
) -> Result<Vec<CrossPageRun>, NativeFulltextReadingOrderError> {
    if decode.labels.len() != order.rows.len() {
        return Err(NativeFulltextReadingOrderError::DecodeRowCountMismatch {
            rows: order.rows.len(),
            labels: decode.labels.len(),
        });
    }
    let mut runs = Vec::with_capacity(decode.runs.len());
    for run in &decode.runs {
        if run.start >= run.end || run.end > order.rows.len() {
            return Err(NativeFulltextReadingOrderError::RunOutOfBounds {
                start: run.start,
                end: run.end,
                rows: order.rows.len(),
            });
        }
        let first_page = order
            .page_of_row(run.start)
            .expect("a validated run start has a page");
        let last_page = order
            .page_of_row(run.end - 1)
            .expect("a validated run end has a page");
        runs.push(CrossPageRun {
            label: run.label.clone(),
            start: run.start,
            end: run.end,
            first_page,
            last_page,
            continued: first_page != last_page,
        });
    }
    Ok(runs)
}

/// One page's visit, in document order: the break flag and the per-page
/// accumulator at the moment the page was left.
struct PageEvent {
    page: u32,
    page_break: bool,
    intra_page_length: i32,
}

/// Per-document state the Java loop carries across blocks and pages.
struct GlueState {
    current_page: Option<u32>,
    open_page: Option<PageEvent>,
    /// `nn` — the running document position.
    nn: i32,
    /// `mm` — the running intra-page position, reset at every page break.
    mm: i32,
    /// `lowestPos` — updated to `block.getY() + block.getHeight()` per block.
    lowest_pos: f64,
    /// `lineStartX` — `Double.NaN` until the first line is seen.
    line_start_x: f64,
    indented: bool,
    previous_newline: bool,
    /// Java's `previousFeatures`: emitted one step late, together with the
    /// block/page facts of the token it belongs to (the correction that
    /// motivates the delay is applied from the *next* block).
    pending: Option<PendingRow>,
}

struct PendingRow {
    page: u32,
    block: u32,
    token: FulltextFeatureToken,
}

impl GlueState {
    fn close_previous_line_if_open(&mut self) {
        let Some(previous) = self.pending.as_mut() else {
            return;
        };
        if previous.token.line_status != FactsLineStatus::Linestart {
            previous.token.line_status = FactsLineStatus::Lineend;
        }
    }
}

impl Default for GlueState {
    fn default() -> Self {
        Self {
            current_page: None,
            open_page: None,
            nn: 0,
            mm: 0,
            lowest_pos: 0.0,
            line_start_x: f64::NAN,
            indented: false,
            previous_newline: false,
            pending: None,
        }
    }
}

/// One block's ordered tokens plus the block-scoped facts they must agree on.
struct BlockGroup<'a> {
    page: u32,
    block: u32,
    block_y: f64,
    block_height: f64,
    page_height: f64,
    tokens: Vec<&'a NativeFulltextPageToken>,
}

fn validate_token(token: &NativeFulltextPageToken) -> Result<(), NativeFulltextReadingOrderError> {
    let place = (token.page, token.block, token.within_block);
    for (field, value) in [
        ("page_height", token.page_height),
        ("block_y", token.block_y),
        ("block_height", token.block_height),
        ("x", token.x),
        ("width", token.width),
        ("y", token.y),
        ("font_size", token.font_size),
    ] {
        if !value.is_finite() {
            return Err(NativeFulltextReadingOrderError::NonFiniteGeometry {
                page: place.0,
                block: place.1,
                field,
            });
        }
    }
    if token.block_token_count == 0 || token.within_block >= token.block_token_count {
        return Err(NativeFulltextReadingOrderError::OrdinalOutOfRange {
            page: place.0,
            block: place.1,
            within_block: place.2,
            block_token_count: token.block_token_count,
        });
    }
    if let NativeFulltextTokenText::Word { text } = &token.text {
        if text.is_empty() {
            return Err(NativeFulltextReadingOrderError::EmptyTokenText {
                page: place.0,
                block: place.1,
                within_block: place.2,
            });
        }
        if text.chars().any(char::is_whitespace) {
            return Err(NativeFulltextReadingOrderError::TokenContainsWhitespace {
                page: place.0,
                block: place.1,
                within_block: place.2,
            });
        }
    }
    Ok(())
}

/// Group the sorted tokens into complete, contiguous blocks.
fn group_blocks<'a>(
    ordered: &[&'a NativeFulltextPageToken],
) -> Result<Vec<BlockGroup<'a>>, NativeFulltextReadingOrderError> {
    let mut blocks: Vec<BlockGroup<'a>> = Vec::new();
    for token in ordered {
        let starts_block = blocks
            .last()
            .map(|block| block.page != token.page || block.block != token.block)
            .unwrap_or(true);
        if starts_block {
            blocks.push(BlockGroup {
                page: token.page,
                block: token.block,
                block_y: token.block_y,
                block_height: token.block_height,
                page_height: token.page_height,
                tokens: Vec::new(),
            });
        }
        let block = blocks
            .last_mut()
            .expect("a block was just opened or already existed");
        for (field, previous, current) in [
            ("block_y", block.block_y, token.block_y),
            ("block_height", block.block_height, token.block_height),
            ("page_height", block.page_height, token.page_height),
        ] {
            if previous != current {
                return Err(NativeFulltextReadingOrderError::BlockFactMismatch {
                    page: token.page,
                    block: token.block,
                    field,
                });
            }
        }
        if block.tokens.len() as u32 != token.within_block {
            // Sorted by ordinal, so an ordinal behind the cursor is a duplicate
            // and an ordinal ahead of it is a gap in the declared block.
            return Err(if (token.within_block as usize) < block.tokens.len() {
                NativeFulltextReadingOrderError::DuplicateOrdinal {
                    page: token.page,
                    block: token.block,
                    within_block: token.within_block,
                }
            } else {
                NativeFulltextReadingOrderError::IncompleteBlock {
                    page: token.page,
                    block: token.block,
                    expected: token.block_token_count,
                    actual: token.within_block,
                }
            });
        }
        block.tokens.push(token);
    }
    for block in &blocks {
        let expected = block
            .tokens
            .first()
            .map(|token| token.block_token_count)
            .unwrap_or(0);
        if expected == 0 || block.tokens.len() as u32 != expected {
            return Err(NativeFulltextReadingOrderError::IncompleteBlock {
                page: block.page,
                block: block.block,
                expected,
                actual: block.tokens.len() as u32,
            });
        }
    }
    Ok(blocks)
}

/// Java's line-status look-ahead inside one block.
///
/// Scans forward for the next real token or line break, and reports whether the
/// block ends at that point. The `".pbm"`-family test reads the *current* token's
/// text in Java (not the look-ahead token's), and that quirk is reproduced
/// verbatim so the lanes cannot drift from the Java assignment.
fn look_ahead(block: &BlockGroup<'_>, index: usize, text: &str) -> (bool, bool) {
    let mut endline = false;
    let mut endblock = false;
    let mut ii = 1usize;
    let mut endloop = false;
    while index + ii < block.tokens.len() && !endloop {
        let next = &block.tokens[index + ii];
        match &next.text {
            NativeFulltextTokenText::LineBreak => {
                endline = true;
                endloop = true;
            }
            NativeFulltextTokenText::Word { text: next_text } => {
                if !next_text.is_empty()
                    && !next_text.starts_with("@IMAGE")
                    && !next_text.starts_with("@PAGE")
                    && !text.contains(".pbm")
                    && !text.contains(".svg")
                    && !text.contains(".png")
                    && !text.contains(".jpg")
                {
                    endloop = true;
                }
            }
        }
        if index + ii == block.tokens.len() - 1 {
            endblock = true;
            endline = true;
        }
        ii += 1;
    }
    (endline, endblock)
}

/// Reading identity: the ordered layout facts and the printed rows, folded into
/// one digest.
///
/// The identity covers the ordered layout facts *and* the printed row text, so
/// two readings that differ only in page order digest differently even when the
/// multiset of rows (and therefore of decoded labels) is equal. `row_places` is
/// parallel to `rows` by construction (both are pushed together), and the row
/// count is hashed too, so a truncated pair cannot alias a shorter reading.
fn reading_identity(
    row_places: &[(u32, u32)],
    rows: &[FulltextFeatureRow],
    document_length: i32,
) -> String {
    let mut identity = Sha256::new();
    identity.update(NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION.as_bytes());
    identity.update(format!("\n{document_length}\t{}", rows.len()).as_bytes());
    for (row, (page, block)) in rows.iter().zip(row_places) {
        identity.update(format!("\n{page}\t{block}\t{}", row.as_text()).as_bytes());
    }
    format!("sha256:{:x}", identity.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(page: u32, block: u32, within: u32, count: u32, text: &str) -> NativeFulltextPageToken {
        NativeFulltextPageToken {
            page,
            page_height: 792.0,
            block,
            block_y: 100.0,
            block_height: 12.0 * count as f64,
            within_block: within,
            block_token_count: count,
            x: 72.0,
            width: 10.0,
            y: 100.0 + within as f64 * 12.0,
            font: "Times-Roman".into(),
            font_size: 10.0,
            bold: false,
            italic: false,
            bitmap_around: false,
            callout_type: FulltextCalloutType::Unknown,
            callout_known: false,
            superscript: false,
            text: NativeFulltextTokenText::Word { text: text.into() },
        }
    }

    fn break_token(page: u32, block: u32, within: u32, count: u32) -> NativeFulltextPageToken {
        let mut token = word(page, block, within, count, "placeholder");
        token.text = NativeFulltextTokenText::LineBreak;
        token
    }

    fn statuses(order: &NativeFulltextReadingOrder) -> Vec<(String, String)> {
        order
            .tokens
            .iter()
            .map(|token| {
                (
                    token.block_status.as_str().to_owned(),
                    token.line_status.as_str().to_owned(),
                )
            })
            .collect()
    }

    /// A single block of four words: BLOCKSTART/LINESTART opens it, only the
    /// token whose look-ahead reaches a non-final word is BLOCKIN/LINEIN, and
    /// the tail is BLOCKEND/LINEEND. (In a three-token block the middle token's
    /// look-ahead already reaches the final token, so Java marks it BLOCKEND —
    /// the visible statuses are not `start/in/end` one per row.)
    #[test]
    fn a_single_block_walks_start_inside_end() {
        let tokens = vec![
            word(1, 0, 0, 4, "Alpha"),
            word(1, 0, 1, 4, "beta"),
            word(1, 0, 2, 4, "gamma"),
            word(1, 0, 3, 4, "delta"),
        ];
        let order = glue_fulltext_reading_order(&tokens).expect("the block glues");
        assert_eq!(
            statuses(&order),
            [
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
                ("BLOCKIN".to_owned(), "LINEIN".to_owned()),
                ("BLOCKEND".to_owned(), "LINEEND".to_owned()),
                ("BLOCKEND".to_owned(), "LINEEND".to_owned()),
            ]
        );
    }

    /// Java's post-correction: a block start right behind a still-open block
    /// closes the previous row.
    ///
    /// The repaired row is BLOCKIN only when the look-ahead stopped on a
    /// non-final line break: a single trailing break *is* the block's final
    /// token, so the look-ahead sets `endblock` and the row is BLOCKEND already.
    /// The correction exists for a block whose tail is EOL material, and two
    /// trailing breaks reach exactly that state.
    #[test]
    fn a_block_start_closes_the_previous_open_block() {
        let tokens = vec![
            word(1, 0, 0, 4, "Alpha"),
            word(1, 0, 1, 4, "beta"),
            break_token(1, 0, 2, 4),
            break_token(1, 0, 3, 4),
            word(1, 1, 0, 1, "Gamma"),
        ];
        let order = glue_fulltext_reading_order(&tokens).expect("two blocks glue");
        assert_eq!(
            statuses(&order),
            [
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
                ("BLOCKEND".to_owned(), "LINEEND".to_owned()),
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
            ]
        );
    }

    /// Without a trailing line break the first block already closes on its own
    /// last token, so the correction has nothing to repair.
    #[test]
    fn a_self_closing_block_needs_no_correction() {
        let tokens = vec![
            word(1, 0, 0, 2, "Alpha"),
            word(1, 0, 1, 2, "beta"),
            word(1, 1, 0, 1, "Gamma"),
        ];
        let order = glue_fulltext_reading_order(&tokens).expect("two blocks glue");
        assert_eq!(
            statuses(&order),
            [
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
                ("BLOCKEND".to_owned(), "LINEEND".to_owned()),
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
            ]
        );
    }

    /// A line-break token emits no row and the closing word ends the block.
    #[test]
    fn a_line_break_closes_the_line_and_the_next_word_starts_one() {
        let tokens = vec![
            word(1, 0, 0, 3, "Alpha"),
            break_token(1, 0, 1, 3),
            word(1, 0, 2, 3, "beta"),
        ];
        let order = glue_fulltext_reading_order(&tokens).expect("line break glues");
        assert_eq!(order.rows.len(), 2, "the break token emits no row");
        assert_eq!(
            statuses(&order),
            [
                ("BLOCKSTART".to_owned(), "LINESTART".to_owned()),
                ("BLOCKEND".to_owned(), "LINEEND".to_owned()),
            ]
        );
    }

    /// The per-page accumulator resets: the second page's `intra_page_length` is
    /// its own length, never the document total.
    #[test]
    fn a_page_break_resets_the_intra_page_accumulator() {
        let tokens = vec![word(1, 0, 0, 1, "Alpha"), word(2, 0, 0, 1, "Beta")];
        let order = glue_fulltext_reading_order(&tokens).expect("two pages glue");
        assert_eq!(order.pages.len(), 2);
        assert!(order.pages[0].page_break, "the first page is a break");
        assert!(order.pages[1].page_break, "the page number changed");
        assert_eq!(order.pages[0].intra_page_length, 5);
        assert_eq!(order.pages[1].intra_page_length, 4);
        assert_eq!(order.document_length, 9);
        assert_eq!(order.page_of_row(0), Some(1));
        assert_eq!(order.page_of_row(1), Some(2));
        assert_eq!(order.page_of_row(2), None);
    }

    /// A block that starts above the lowest position so far is a typed column
    /// shift, not a printed spacing weight.
    #[test]
    fn a_block_above_the_lowest_position_is_a_column_shift() {
        let mut first = word(1, 0, 0, 1, "Alpha");
        first.block_y = 500.0;
        first.block_height = 40.0;
        let mut second = word(1, 1, 0, 1, "Beta");
        second.block_y = 100.0;
        second.block_height = 12.0;
        let order = glue_fulltext_reading_order(&[first, second]).expect("the shift glues");
        assert_eq!(order.column_shifts.len(), 2);
        assert!(!order.column_shifts[0].column_change);
        assert!(order.column_shifts[1].column_change);
        assert_eq!(order.column_shifts[1].spacing, None);
        assert!(order.column_shifts[1].vertical_delta < 0.0);
        // The shift is a fact, not a column: the printed row keeps 27 fields.
        assert!(order.rows.iter().all(|row| row.width() == 27));
    }

    /// Identity is derived: shuffling the caller's list cannot change the
    /// reading, because the facts declare the order.
    #[test]
    fn the_reading_is_independent_of_the_caller_list_order() {
        let mut first = word(1, 0, 0, 1, "Alpha");
        first.block_y = 500.0;
        let mut second = word(2, 0, 0, 1, "Beta");
        second.block_y = 100.0;
        let sorted = glue_fulltext_reading_order(&[first.clone(), second.clone()]).unwrap();
        let shuffled = glue_fulltext_reading_order(&[second, first]).unwrap();
        assert_eq!(sorted.digest, shuffled.digest);
        assert_eq!(sorted.rows, shuffled.rows);
    }

    /// Permuting the pages changes the reading, even though the multiset of rows
    /// (and therefore of decodable labels) is equal.
    #[test]
    fn a_page_permutation_changes_the_reading_identity() {
        let mut first = word(1, 0, 0, 1, "Alpha");
        first.block_y = 500.0;
        let mut second = word(2, 0, 0, 1, "Beta");
        second.block_y = 100.0;
        let straight = glue_fulltext_reading_order(&[first.clone(), second.clone()]).unwrap();
        // Same rows, same label bag — the pages are swapped.
        let mut swapped_first = first;
        swapped_first.page = 2;
        let mut swapped_second = second;
        swapped_second.page = 1;
        let swapped = glue_fulltext_reading_order(&[swapped_first, swapped_second]).unwrap();
        assert_eq!(straight.rows.len(), swapped.rows.len());
        // The row *bag* is not comparable: the font lanes are stateful, so the
        // first row of each reading is always NEWFONT/HIGHERFONT. Compare the
        // admitted token texts instead — the multiset is what the permutation
        // preserves.
        let mut straight_text: Vec<&str> = straight
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect();
        let mut swapped_text: Vec<&str> = swapped
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect();
        straight_text.sort_unstable();
        swapped_text.sort_unstable();
        assert_eq!(straight_text, swapped_text, "the token bag is unchanged");
        assert_ne!(
            straight.digest, swapped.digest,
            "identity is the order, not the bag"
        );
    }

    #[test]
    fn refuses_an_empty_input() {
        assert_eq!(
            glue_fulltext_reading_order(&[]),
            Err(NativeFulltextReadingOrderError::EmptyInput)
        );
    }

    /// A body made only of line-break tokens has no rows, and that is legal
    /// (Java's caller tests `isNotBlank`).
    #[test]
    fn a_line_break_only_body_glues_to_zero_rows() {
        let order = glue_fulltext_reading_order(&[break_token(1, 0, 0, 1)]).unwrap();
        assert!(order.rows.is_empty());
        assert_eq!(order.document_length, 1);
        assert_eq!(order.pages.len(), 1);
        assert_eq!(order.pages[0].row_count, 0);
    }

    #[test]
    fn refuses_a_duplicate_ordinal() {
        let tokens = vec![word(1, 0, 0, 1, "Alpha"), word(1, 0, 0, 1, "Alpha")];
        assert_eq!(
            glue_fulltext_reading_order(&tokens),
            Err(NativeFulltextReadingOrderError::DuplicateOrdinal {
                page: 1,
                block: 0,
                within_block: 0,
            })
        );
    }

    #[test]
    fn refuses_an_ordinal_outside_the_declared_block() {
        let tokens = vec![word(1, 0, 3, 2, "Alpha")];
        assert_eq!(
            glue_fulltext_reading_order(&tokens),
            Err(NativeFulltextReadingOrderError::OrdinalOutOfRange {
                page: 1,
                block: 0,
                within_block: 3,
                block_token_count: 2,
            })
        );
    }

    #[test]
    fn refuses_a_partial_block() {
        let tokens = vec![word(1, 0, 0, 2, "Alpha")];
        assert_eq!(
            glue_fulltext_reading_order(&tokens),
            Err(NativeFulltextReadingOrderError::IncompleteBlock {
                page: 1,
                block: 0,
                expected: 2,
                actual: 1,
            })
        );
    }

    #[test]
    fn refuses_a_block_fact_disagreement() {
        let mut first = word(1, 0, 0, 2, "Alpha");
        first.block_y = 100.0;
        let mut second = word(1, 0, 1, 2, "beta");
        second.block_y = 300.0;
        assert_eq!(
            glue_fulltext_reading_order(&[first, second]),
            Err(NativeFulltextReadingOrderError::BlockFactMismatch {
                page: 1,
                block: 0,
                field: "block_y",
            })
        );
    }

    #[test]
    fn refuses_non_finite_geometry() {
        let mut token = word(1, 0, 0, 1, "Alpha");
        token.y = f64::NAN;
        assert!(matches!(
            glue_fulltext_reading_order(&[token]),
            Err(NativeFulltextReadingOrderError::NonFiniteGeometry { field: "y", .. })
        ));
    }

    #[test]
    fn refuses_whitespace_and_empty_word_text() {
        let mut spaced = word(1, 0, 0, 1, "Alpha beta");
        spaced.text = NativeFulltextTokenText::Word {
            text: "Alpha beta".into(),
        };
        assert!(matches!(
            glue_fulltext_reading_order(&[spaced]),
            Err(NativeFulltextReadingOrderError::TokenContainsWhitespace { .. })
        ));
    }

    #[test]
    fn refuses_more_tokens_than_the_bound() {
        let token = word(1, 0, 0, 1, "Alpha");
        let tokens = vec![token; MAX_FULLTEXT_TOKENS + 1];
        assert!(matches!(
            glue_fulltext_reading_order(&tokens),
            Err(NativeFulltextReadingOrderError::TooManyTokens { .. })
        ));
    }

    #[test]
    fn refuses_more_pages_than_the_bound() {
        let mut tokens = Vec::new();
        for page in 1..=(MAX_FULLTEXT_PAGES as u32 + 1) {
            tokens.push(word(page, 0, 0, 1, "Alpha"));
        }
        assert!(matches!(
            glue_fulltext_reading_order(&tokens),
            Err(NativeFulltextReadingOrderError::TooManyPages { .. })
        ));
    }

    /// Filtered pseudo-tokens emit no row and do not advance the accumulators.
    #[test]
    fn a_filtered_pseudo_token_emits_no_row_and_does_not_advance() {
        let tokens = vec![word(1, 0, 0, 2, "Alpha"), word(1, 0, 1, 2, "@PAGE")];
        let order = glue_fulltext_reading_order(&tokens).unwrap();
        assert_eq!(order.rows.len(), 1);
        assert_eq!(
            order.document_length, 10,
            "@PAGE is counted as a body token"
        );
    }
}
