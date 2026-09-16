//! (R131) Cross-page reading-order contract for the native GROBID body band.
//!
//! This file owns the *reading order* section of the S05 body band: the
//! document order derived from `(page, block, within_block)` layout facts, the
//! page spans and their per-page accumulator reset, the typed `ColumnShift` fact,
//! the cross-page continuation of a T02 run, and the reading identity digest.
//!
//! Expectations are hand-derived from the vendored GROBID source
//! (`FullTextParser.getBodyTextFeatured`: the page-transition reset of `mm` /
//! `lowestPos` / `spacingPreviousBlock`, the `linearScaling` relative positions,
//! and the `lowestPos > block.getY()` column test), never from the module under
//! test. The synthetic CRF is built in-file from the printed ABI column 0 the
//! row builder produces, so the decode consumes the same rows the glue emits.
//! The independent same-n oracle and the opt-in real-model harness are separate
//! artifacts (S05/T05).
//!
//! Boundary statement: every symbol exercised here is a pure function. No
//! filesystem, environment, clock, model bytes or production route is touched,
//! and no atom of this band is spliced into the Java-authoritative parse path.

use da_parse::*;
use da_ports::scalar_crf::{CrfModel, CrfPattern};
use std::collections::BTreeMap;

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
        width: 8.0,
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

/// A scalar CRF over the closed 24-label dictionary whose row `i` observation is
/// `u:{row_text[i]}` and whose weight forces `labels[i]`.
///
/// Every row text must be distinct, because the synthetic model identifies the
/// position through the observation index (`u_offsets[observation] + label`),
/// exactly as the hand-authored T02 fixture does. No `b:` observation is
/// declared, so every transition weight is zero and the decoded sequence is the
/// requested one rather than a tie-breaking artefact.
fn model_for(row_texts: &[&str], labels: &[&str]) -> CrfModel {
    assert_eq!(row_texts.len(), labels.len());
    let vocabulary: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    let label_count = vocabulary.len();
    let observations: Vec<String> = row_texts.iter().map(|text| format!("u:{text}")).collect();
    let mut u_offsets = Vec::with_capacity(observations.len());
    let mut weights = BTreeMap::new();
    for (position, label) in labels.iter().enumerate() {
        let offset = position * label_count;
        u_offsets.push(Some(offset));
        let target = NATIVE_FULLTEXT_LABELS
            .iter()
            .position(|known| known == label)
            .expect("the requested label is part of the closed dictionary");
        weights.insert(offset + target, 3.0);
    }
    CrfModel {
        model_type: 2,
        labels: vocabulary,
        observations,
        patterns: vec![CrfPattern {
            source: "u:%x[0,0]".into(),
            kind: 'u',
        }],
        u_offsets,
        b_offsets: vec![None; labels.len()],
        weights,
        nftr: labels.len() * label_count,
    }
}

fn decode(order: &NativeFulltextReadingOrder, model: &CrfModel) -> NativeFulltextDecodeCandidate {
    match decode_fulltext_sequence(&order.rows, model) {
        NativeFulltextDecodeOutcome::Accepted { decode } => decode,
        NativeFulltextDecodeOutcome::Rejected { error } => {
            panic!("ordered rows must decode, got {error}")
        }
    }
}

/// Two pages of a body split inside one paragraph: page 1 carries the opening
/// tokens, page 2 the continuation.
fn two_page_split() -> Vec<NativeFulltextPageToken> {
    vec![
        word(1, 0, 0, 3, "We"),
        word(1, 0, 1, 3, "study"),
        word(1, 0, 2, 3, "graphs"),
        word(2, 0, 0, 2, "across"),
        word(2, 0, 1, 2, "pages"),
    ]
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

#[test]
fn reading_order_surface_is_bounded_and_versioned() {
    assert_eq!(
        NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION,
        "da-parse.native-fulltext-reading-order/v1"
    );
    assert_eq!(MAX_FULLTEXT_TOKENS, MAX_NATIVE_FULLTEXT_FEATURE_ROWS);
    assert_eq!(MAX_FULLTEXT_TOKENS, 32_768);
    assert_eq!(MAX_FULLTEXT_PAGES, 4_096);
}

/// The glue yields document order across the page break, and the per-page
/// accumulator restarts on the new page.
#[test]
fn two_pages_glue_into_document_order_with_page_scoped_positions() {
    let order = glue_fulltext_reading_order(&two_page_split()).expect("the body glues");

    assert_eq!(
        order.schema_version,
        NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION
    );
    assert_eq!(order.row_count(), 5);
    assert_eq!(order.tokens.len(), 5, "one row per admitted word token");
    assert!(
        order.rows.iter().all(|row| row.width() == 27),
        "every ordered row carries the printed 27-column ABI"
    );
    assert_eq!(
        order.rows[0].columns[0], "We",
        "document order starts on page 1"
    );
    assert_eq!(order.rows[3].columns[0], "across", "page 2 continues it");

    assert_eq!(order.pages.len(), 2);
    assert_eq!(order.pages[0].page, 1);
    assert!(order.pages[0].page_break, "the first page is a break");
    assert_eq!(order.pages[0].first_row, 0);
    assert_eq!(order.pages[0].row_count, 3);
    assert_eq!(order.pages[0].intra_page_length, 13, "We + study + graphs");
    assert_eq!(order.pages[1].page, 2);
    assert!(order.pages[1].page_break, "the page number changed");
    assert_eq!(order.pages[1].first_row, 3);
    assert_eq!(order.pages[1].row_count, 2);
    assert_eq!(
        order.pages[1].intra_page_length, 11,
        "the counter reset: across + pages, not the document total"
    );
    assert_eq!(order.document_length, 24, "the whole body length");

    assert_eq!(order.page_of_row(0), Some(1));
    assert_eq!(order.page_of_row(2), Some(1));
    assert_eq!(order.page_of_row(3), Some(2));
    assert_eq!(order.page_of_row(4), Some(2));
    assert_eq!(order.page_of_row(5), None);

    // Parity with the printed ABI: the first row's relative document position is
    // `linearScaling(nn=0, fulltextLength=24, 12)` and the last admitted row
    // carries `nn` accumulated over the four preceding tokens (19 of 24 → 9),
    // while page 2 restarts `mm` at zero.
    assert_eq!(order.rows[0].columns[21], "0");
    assert_eq!(order.rows[4].columns[21], "9", "linearScaling(19, 24, 12)");
}

/// A paragraph split by the page break stays *one* run: continuation is a
/// consequence of gluing, not a second model.
#[test]
fn a_paragraph_split_by_the_page_break_is_one_continued_run() {
    let order = glue_fulltext_reading_order(&two_page_split()).expect("the body glues");
    let model = model_for(
        &["We", "study", "graphs", "across", "pages"],
        &[
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
        ],
    );
    let decoded = decode(&order, &model);
    assert_eq!(
        decoded.runs,
        [FulltextLabelRun {
            label: "<paragraph>".into(),
            start: 0,
            end: 5,
        }],
        "the label run is not cut at the page boundary"
    );
    let runs = cross_page_runs(&order, &decoded).expect("the pages are known");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].first_page, 1);
    assert_eq!(runs[0].last_page, 2);
    assert!(runs[0].continued, "the run spans two pages");
    assert_eq!(order.cross_page_runs(&decoded), Ok(runs));
}

/// `continued` is not trivially true: a run fully inside one page is not
/// continued.
#[test]
fn a_run_inside_one_page_is_not_continued() {
    let tokens = vec![
        word(1, 0, 0, 3, "Alpha"),
        word(1, 0, 1, 3, "beta"),
        word(1, 0, 2, 3, "Gamma"),
    ];
    let order = glue_fulltext_reading_order(&tokens).expect("one page glues");
    let model = model_for(
        &["Alpha", "beta", "Gamma"],
        &["<paragraph>", "<paragraph>", "<section>"],
    );
    let decoded = decode(&order, &model);
    let runs = cross_page_runs(&order, &decoded).expect("the page is known");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].label, "<paragraph>");
    assert_eq!(runs[0].first_page, 1);
    assert_eq!(runs[0].last_page, 1);
    assert!(!runs[0].continued);
    assert_eq!(runs[1].label, "<section>");
    assert!(!runs[1].continued);
}

/// Permuting the pages is a different reading: the label bag is unchanged while
/// the identity digest and the run structure are not.
#[test]
fn a_page_permutation_changes_identity_but_not_the_label_bag() {
    let straight = glue_fulltext_reading_order(&two_page_split()).expect("the body glues");
    let mut permuted_tokens = two_page_split();
    for token in &mut permuted_tokens {
        token.page = if token.page == 1 { 2 } else { 1 };
    }
    let permuted = glue_fulltext_reading_order(&permuted_tokens).expect("the swapped body glues");
    assert_eq!(permuted.row_count(), straight.row_count());

    let model = model_for(
        &["We", "study", "graphs", "across", "pages"],
        &[
            "<section>",
            "<section>",
            "<section>",
            "<paragraph>",
            "<paragraph>",
        ],
    );
    // The observation the model keys on is the row text, which the permutation
    // reorders; the multiset of decoded labels stays equal.
    let straight_labels = {
        let decoded = decode(&straight, &model);
        let mut labels = decoded.labels.clone();
        labels.sort();
        labels
    };
    let permuted_labels = {
        // The permuted reading puts page 2 first, so the model's per-row
        // observations no longer line up with the printed order; keying the same
        // weights to the swapped texts is what makes this a label-set
        // comparison rather than a decode of a foreign sequence.
        let swapped_texts = ["across", "pages", "We", "study", "graphs"];
        let swapped_model = model_for(
            &swapped_texts,
            &[
                "<paragraph>",
                "<paragraph>",
                "<section>",
                "<section>",
                "<section>",
            ],
        );
        let decoded = decode(&permuted, &swapped_model);
        let mut labels = decoded.labels.clone();
        labels.sort();
        labels
    };
    assert_eq!(
        straight_labels, permuted_labels,
        "the label bag is equal under the permutation"
    );
    assert_ne!(
        straight.digest, permuted.digest,
        "identity is the reading order, not the bag"
    );
    assert_ne!(
        straight
            .rows
            .iter()
            .map(FulltextFeatureRow::as_text)
            .collect::<Vec<_>>(),
        permuted
            .rows
            .iter()
            .map(FulltextFeatureRow::as_text)
            .collect::<Vec<_>>()
    );
}

/// Document order is derived from the declared facts, so the caller's list order
/// cannot change the reading.
#[test]
fn shuffling_the_input_does_not_change_the_reading() {
    let straight = glue_fulltext_reading_order(&two_page_split()).expect("the body glues");
    let mut shuffled = two_page_split();
    shuffled.reverse();
    shuffled.swap(0, 2);
    let reordered = glue_fulltext_reading_order(&shuffled).expect("the shuffled body glues");
    assert_eq!(straight, reordered);
    assert_eq!(straight.digest, reordered.digest);
}

/// A block that starts above the lowest position reached on its page is a typed
/// column shift, never a printed weight.
#[test]
fn a_column_change_is_a_typed_fact_not_a_printed_column() {
    let mut first = word(1, 0, 0, 1, "Alpha");
    first.block_y = 600.0;
    first.block_height = 20.0;
    let mut second = word(1, 1, 0, 1, "beta");
    second.block_y = 120.0;
    second.block_height = 12.0;
    let order = glue_fulltext_reading_order(&[first, second]).expect("the shift glues");
    assert_eq!(order.column_shifts.len(), 2);
    assert!(!order.column_shifts[0].column_change);
    assert!(order.column_shifts[1].column_change);
    assert_eq!(
        order.column_shifts[1].vertical_delta, -500.0,
        "block.getY() - lowestPos"
    );
    assert_eq!(
        order.column_shifts[1].spacing, None,
        "Java's substituted document statistic is not modelled"
    );
    assert!(
        order.rows.iter().all(|row| row.width() == 27),
        "the shift is a fact, not an extra column"
    );
}

/// Boundaries: nothing admitted is refused, one page is a legal document, and a
/// body of line breaks alone has no rows.
#[test]
fn empty_single_page_and_line_break_only_boundaries() {
    assert_eq!(
        glue_fulltext_reading_order(&[]),
        Err(NativeFulltextReadingOrderError::EmptyInput)
    );

    let order = glue_fulltext_reading_order(&[word(1, 0, 0, 1, "Alpha")]).expect("one page glues");
    assert_eq!(order.pages.len(), 1);
    assert!(order.pages[0].page_break);
    assert_eq!(order.pages[0].row_count, 1);
    assert_eq!(order.pages[0].intra_page_length, 5);

    let mut line_break = word(1, 0, 0, 2, "Alpha");
    line_break.text = NativeFulltextTokenText::LineBreak;
    let mut tail = word(1, 0, 1, 2, "beta");
    tail.text = NativeFulltextTokenText::LineBreak;
    let order = glue_fulltext_reading_order(&[line_break, tail]).expect("breaks glue");
    assert!(order.rows.is_empty(), "no word token, no row");
    assert_eq!(order.document_length, 2);
    assert_eq!(order.pages.len(), 1);
    assert_eq!(order.pages[0].row_count, 0);
}

/// The caps refuse instead of truncating: a reading is never silently partial.
#[test]
fn caps_refuse_rather_than_truncate() {
    let token = word(1, 0, 0, 1, "Alpha");
    let tokens = vec![token; MAX_FULLTEXT_TOKENS + 1];
    assert!(matches!(
        glue_fulltext_reading_order(&tokens),
        Err(NativeFulltextReadingOrderError::TooManyTokens {
            actual,
            maximum: MAX_FULLTEXT_TOKENS
        }) if actual == MAX_FULLTEXT_TOKENS + 1
    ));

    let pages: Vec<NativeFulltextPageToken> = (1..=MAX_FULLTEXT_PAGES as u32 + 1)
        .map(|page| word(page, 0, 0, 1, "Alpha"))
        .collect();
    assert!(matches!(
        glue_fulltext_reading_order(&pages),
        Err(NativeFulltextReadingOrderError::TooManyPages {
            actual,
            maximum: MAX_FULLTEXT_PAGES
        }) if actual == MAX_FULLTEXT_PAGES + 1
    ));
}

/// The glue is a pure function: equal input, equal reading and digest.
#[test]
fn gluing_is_deterministic() {
    let first = glue_fulltext_reading_order(&two_page_split()).unwrap();
    let second = glue_fulltext_reading_order(&two_page_split()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.digest, second.digest);
}

/// The ordered surface round-trips through serde.
#[test]
fn the_ordered_surface_round_trips() {
    let order = glue_fulltext_reading_order(&two_page_split()).unwrap();
    roundtrip(&order);
    roundtrip(&order.pages);
    roundtrip(&order.column_shifts);
    roundtrip(&order.tokens);
    roundtrip(&two_page_split());
    let mut permuted = two_page_split();
    for token in &mut permuted {
        token.page = if token.page == 1 { 2 } else { 1 };
    }
    let permuted = glue_fulltext_reading_order(&permuted).unwrap();
    let model = model_for(
        &["across", "pages", "We", "study", "graphs"],
        &[
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
            "<paragraph>",
        ],
    );
    let runs = cross_page_runs(&permuted, &decode(&permuted, &model)).unwrap();
    roundtrip(&runs);
}

/// A decode produced over a different row count is a typed refusal, not a
/// silently mismatched annotation.
#[test]
fn a_decode_over_a_different_row_count_is_refused() {
    let order = glue_fulltext_reading_order(&two_page_split()).unwrap();
    let model = model_for(&["We", "study"], &["<paragraph>", "<paragraph>"]);
    let mut short_rows = order.rows.clone();
    short_rows.truncate(2);
    let short_order = NativeFulltextReadingOrder {
        rows: short_rows,
        ..order.clone()
    };
    let decoded = decode(&short_order, &model);
    assert_eq!(
        cross_page_runs(&order, &decoded),
        Err(NativeFulltextReadingOrderError::DecodeRowCountMismatch { rows: 5, labels: 2 })
    );
}

/// A run that does not tile the ordered rows is refused instead of being
/// annotated with a page span it does not have. Both a truncated run range and a
/// zero-width run are refused.
#[test]
fn a_run_outside_the_ordered_rows_is_refused() {
    let order = glue_fulltext_reading_order(&two_page_split()).unwrap();
    let beyond = NativeFulltextDecodeCandidate {
        labels: vec!["<paragraph>".to_owned(); 5],
        runs: vec![FulltextLabelRun {
            label: "<paragraph>".into(),
            start: 3,
            end: 7,
        }],
    };
    assert_eq!(
        cross_page_runs(&order, &beyond),
        Err(NativeFulltextReadingOrderError::RunOutOfBounds {
            start: 3,
            end: 7,
            rows: 5,
        })
    );
    let empty_run = NativeFulltextDecodeCandidate {
        labels: vec!["<paragraph>".to_owned(); 5],
        runs: vec![FulltextLabelRun {
            label: "<paragraph>".into(),
            start: 2,
            end: 2,
        }],
    };
    assert_eq!(
        cross_page_runs(&order, &empty_run),
        Err(NativeFulltextReadingOrderError::RunOutOfBounds {
            start: 2,
            end: 2,
            rows: 5,
        })
    );
}
