//! Native header band: the join that runs the header atoms on the S03 native
//! carrier instead of on TEI-derived copies.
//!
//! S04/T01 published [`NativeRegionCarrier`]: ordered, redaction-safe token
//! runs per semantic region, joined from the S03 layout plane
//! ([`LayoutDocument`], [`LayoutLattice`], [`LexemeRow`],
//! [`FullSegmentationOutcome`]). This module is the next additive band. It
//! consumes the **Header** run and:
//!
//! 1. reconstructs the header's ordered text lines from the carrier (line
//!    grouping comes from `FullLocation`, the text from the joined
//!    [`NativeRegionToken`]s — never from TEI);
//! 2. builds the GROBID header CRF feature ABI ([`HeaderFeatureToken`]) from
//!    the carrier's own geometry and style (`block_status`/`line_status` from
//!    `FullLocation`, `alignment` from `bbox`, `font_size`/`bold`/`italic`
//!    from `LayoutStyleHints`) and renders it with the pure
//!    [`render_header_features`] — this closes the previously unlinked
//!    "native facts → `HeaderFeatureToken`" bridge that until now existed only
//!    inside a test helper;
//! 3. runs the existing pure header atoms — [`NativeDateParser`],
//!    [`NativePersonalNameParser`], [`NativeAffiliationParser`] and
//!    [`NativeAffiliationLinker`] — over the native lines and reports one
//!    typed disposition per atom.
//!
//! ## CRF decode is a typed out-of-scope, not a silent gap
//!
//! The real GROBID 0.9.0 header model bytes are not redistributable and are
//! not in the tree (D358). [`decode_header_sequence`] therefore cannot be
//! invoked here, and the outcome carries a typed
//! [`NativeHeaderCrfScope::OutOfScope`] with reason
//! [`NativeHeaderOutOfScope::ModelNotInTree`] rather than a fabricated label
//! sequence. The feature ABI is still built and rendered in full, so the
//! bridge is exercised; only the decode step is deferred.
//!
//! ## Dispositions, not values
//!
//! Because no field decoder exists in this cut, the band cannot and does not
//! claim field assignment. It reports, per atom, a closed
//! [`HeaderAtomDisposition`] and the number of native lines the atom
//! recognized. The `evidence_digest` is the sha256 of the **normalized source
//! text** of the recognized lines (joined by U+001F); the source text itself is
//! never carried. This mirrors the carrier's redaction discipline: a caller can
//! compare digests and counts without the outcome leaking article text.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no TEI/Java type is an input. Nothing here is spliced
//! into the Java-authoritative parse route; `import_eligible` stays false. The
//! S03 wire formats (`NativeSegmenter`/v1, `segment_document_full`/v2) and the
//! T01 carrier are not touched.

use crate::{
    AffiliationSpan, BlockStatus, FullSemanticRegion, HeaderAlignment, HeaderFeatureRow,
    HeaderFeatureToken, HeaderFontStatistics, LayoutTokenKind, LineStatus,
    NativeAffiliationLinkOutcome, NativeAffiliationLinker, NativeAffiliationNesting,
    NativeAffiliationOccurrence, NativeAffiliationOutcome, NativeAffiliationParser,
    NativeDateOutcome, NativeDateParser, NativeHeaderFeatureError, NativePersonalNameOutcome,
    NativePersonalNameParser, NativeRegionCarrier, NativeRegionRun, PersonalNameContext,
    render_header_feature_text, render_header_features,
};
use da_ports::LexiconBundle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

/// Schema identity of the additive native header-input surface.
pub const NATIVE_HEADER_INPUT_SCHEMA_VERSION: &str = "da-parse.native-header-native-input/v1";

/// Generator identity of this band. An independent oracle must not declare it
/// (see [`require_independent_header_oracle`]).
pub const NATIVE_HEADER_INPUT_GENERATOR_ID: &str = "da-parse.native-header-native-input";

/// Additional horizontal indent (in points) a line's first token must carry
/// before it is reported as `LINEINDENT` rather than `ALIGNEDLEFT`. GROBID's
/// own feature uses glyph-relative character width; the carrier exposes only
/// the bounding box, so this documented, deterministic threshold stands in for
/// it and is below typical text-rendering jitter.
pub const NATIVE_HEADER_INDENT_EPSILON_PT: f64 = 0.5;

/// Separator used when folding several recognized lines into one evidence
/// digest. U+001F (unit separator) keeps joined values collision-free.
const EVIDENCE_SEPARATOR: char = '\u{1f}';

/// Closed atom vocabulary executed on the native carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum HeaderAtom {
    Date,
    PersonalName,
    Affiliation,
    AffiliationLink,
}

/// Closed per-atom disposition. Mirrors the atom outcome vocabularies; no
/// source text is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderAtomDisposition {
    Recognized,
    Empty,
    Malformed,
    Unsupported,
    Ambiguous,
    Oversized,
}

/// Why the CRF decode step is not executed in this cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHeaderOutOfScope {
    /// The GROBID 0.9.0 header model bytes are not redistributable and are not
    /// in the tree (D358).
    ModelNotInTree,
}

/// Typed scope marker for the decode step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHeaderCrfScope {
    OutOfScope { reason: NativeHeaderOutOfScope },
}

/// Provenance identity of the native carrier the outcome was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeaderInputIdentity {
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub order_digest: String,
    /// Digest of the redaction-safe region carrier.
    pub carrier_digest: String,
}

/// One atom's aggregate outcome over the native header lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeaderAtomOutcome {
    pub atom: HeaderAtom,
    pub disposition: HeaderAtomDisposition,
    /// Number of native lines the atom recognized.
    pub recognized_count: usize,
    /// sha256 of the normalized recognized-line texts joined by U+001F.
    pub evidence_digest: String,
}

/// Redaction-safe result of running the header atoms on the native carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHeaderFromInputOutcome {
    pub schema_version: String,
    pub identity: NativeHeaderInputIdentity,
    /// Always [`FullSemanticRegion::Header`]; carried so the outcome is
    /// self-describing.
    pub region: FullSemanticRegion,
    /// Admitted tokens in the Header run.
    pub token_count: usize,
    /// Distinct native text lines reconstructed from the carrier.
    pub line_count: usize,
    /// Distinct layout blocks the Header run spans.
    pub block_count: usize,
    /// Non-whitespace tokens promoted into the CRF feature ABI.
    pub feature_row_count: usize,
    /// sha256 over the rendered feature text (the ABI input the model would
    /// consume). The feature text itself is never carried.
    pub feature_digest: String,
    pub atoms: Vec<NativeHeaderAtomOutcome>,
    pub crf_scope: NativeHeaderCrfScope,
}

/// Fail-closed refusal vocabulary of the native header band.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeHeaderInputError {
    /// The Header region was requested but admitted no tokens in the
    /// segmentation. A typed absence, never a zero.
    NoAdmittedTokens { region: FullSemanticRegion },
    /// The Header region was not part of the carrier at all.
    RegionNotRequested { region: FullSemanticRegion },
    /// The run admitted tokens, but none survived the whitespace projection
    /// into the CRF feature ABI.
    NoFeatureTokens,
    /// A layout token carried no `font_size_milli_pt`, so a CRF font column
    /// cannot be derived without inventing one.
    MissingFontSize { page: u32, source: u32 },
    /// A layout token declared a zero font size, which cannot be a valid
    /// feature font.
    InvalidFontSize {
        page: u32,
        source: u32,
        milli_pt: u32,
    },
    /// A lexeme carried internal whitespace; the CRF ABI is word-level and
    /// never splits a token.
    TokenContainsWhitespace { page: u32, source: u32 },
    /// The pure feature renderer refused the derived tokens.
    FeatureRender(NativeHeaderFeatureError),
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeHeaderInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdmittedTokens { region } => {
                write!(
                    formatter,
                    "native header region {region:?} admitted no tokens"
                )
            }
            Self::RegionNotRequested { region } => {
                write!(
                    formatter,
                    "native header region {region:?} was not requested"
                )
            }
            Self::NoFeatureTokens => {
                write!(formatter, "native header run has no non-whitespace tokens")
            }
            Self::MissingFontSize { page, source } => write!(
                formatter,
                "native header token {page}:{source} carries no font size"
            ),
            Self::InvalidFontSize {
                page,
                source,
                milli_pt,
            } => write!(
                formatter,
                "native header token {page}:{source} declares font size {milli_pt} milli-pt"
            ),
            Self::TokenContainsWhitespace { page, source } => write!(
                formatter,
                "native header token {page}:{source} contains whitespace"
            ),
            Self::FeatureRender(error) => {
                write!(formatter, "header feature render failed: {error}")
            }
            Self::Serialization => write!(formatter, "native header outcome serialization failed"),
        }
    }
}

impl std::error::Error for NativeHeaderInputError {}

/// Typed verdict of a character-level oracle comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderCoverageVerdict {
    /// The native side covers every oracle character; differences are
    /// granularity (tokenization/spacing), not lost content.
    GranularityOnly,
    /// The native side is missing characters the oracle carried: real content
    /// loss.
    ContentLoss,
    /// The oracle is empty, so nothing can be assessed. Not a pass.
    NotAssessable,
}

/// Provenance identity of an independent oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderOracleProvenance {
    /// Identity of the code that produced the oracle. Must differ from
    /// [`NATIVE_HEADER_INPUT_GENERATOR_ID`].
    pub generator_id: String,
    /// The producing backend, e.g. `java-grobid-crf`.
    pub backend: String,
    /// sha256 pin of the oracle source bytes.
    pub sha256: String,
}

/// Refusal vocabulary of the oracle independence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderOracleIndependenceError {
    MissingGenerator,
    MissingBackend,
    MissingPin,
    /// The oracle was produced by the same code as the SUT.
    NotIndependent {
        generator_id: String,
    },
}

impl fmt::Display for HeaderOracleIndependenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGenerator => write!(formatter, "oracle provenance has no generator id"),
            Self::MissingBackend => write!(formatter, "oracle provenance has no backend"),
            Self::MissingPin => write!(formatter, "oracle provenance has no sha256 pin"),
            Self::NotIndependent { generator_id } => write!(
                formatter,
                "oracle generator `{generator_id}` is the SUT generator, not independent"
            ),
        }
    }
}

impl std::error::Error for HeaderOracleIndependenceError {}

/// Normalize header text for digest comparison: collapse whitespace and fold
/// case. The canonicalization is shared by the SUT and the oracle so a digest
/// equality compares the same normalized form on both sides.
pub fn normalize_header_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The set of normalized alphanumeric characters in `text`. Used as the
/// character-level coverage unit: whitespace and punctuation are granularity,
/// not content.
pub fn header_char_set(text: &str) -> BTreeSet<char> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Classify oracle-vs-native character coverage.
///
/// * an empty oracle is [`HeaderCoverageVerdict::NotAssessable`] — there is no
///   expectation to measure, and that is not a pass;
/// * a native set that is a superset of the oracle set is
///   [`HeaderCoverageVerdict::GranularityOnly`];
/// * anything else is [`HeaderCoverageVerdict::ContentLoss`] (including an
///   empty native set against a non-empty oracle).
pub fn classify_header_coverage(
    oracle: &BTreeSet<char>,
    native: &BTreeSet<char>,
) -> HeaderCoverageVerdict {
    if oracle.is_empty() {
        return HeaderCoverageVerdict::NotAssessable;
    }
    if oracle.is_subset(native) {
        HeaderCoverageVerdict::GranularityOnly
    } else {
        HeaderCoverageVerdict::ContentLoss
    }
}

/// Require an oracle to have a distinct generator, a backend, and a sha256 pin.
///
/// An oracle built by the SUT's own generator is refused as
/// [`HeaderOracleIndependenceError::NotIndependent`]: a same-generator oracle
/// can only reproduce the SUT's own errors.
pub fn require_independent_header_oracle(
    provenance: &HeaderOracleProvenance,
) -> Result<(), HeaderOracleIndependenceError> {
    if provenance.generator_id.trim().is_empty() {
        return Err(HeaderOracleIndependenceError::MissingGenerator);
    }
    if provenance.generator_id == NATIVE_HEADER_INPUT_GENERATOR_ID {
        return Err(HeaderOracleIndependenceError::NotIndependent {
            generator_id: provenance.generator_id.clone(),
        });
    }
    if provenance.backend.trim().is_empty() {
        return Err(HeaderOracleIndependenceError::MissingBackend);
    }
    if !is_sha256_hex(&provenance.sha256) {
        return Err(HeaderOracleIndependenceError::MissingPin);
    }
    Ok(())
}

/// Build the CRF feature ABI tokens for one header run.
///
/// `block_status`/`line_status` come from the run's `FullLocation` grouping
/// (first token of a block/line starts, last ends); `alignment` comes from the
/// first token's `bbox.x_min` relative to the most-left line start;
/// `font_size`/`bold`/`italic` come from `LayoutStyleHints`. Whitespace-kind
/// tokens are not CRF words and are omitted; a non-whitespace token with
/// internal whitespace is refused rather than split.
pub fn build_header_feature_tokens(
    run: &NativeRegionRun,
) -> Result<Vec<HeaderFeatureToken>, NativeHeaderInputError> {
    let lines = group_run_lines(run);
    let mut staged: Vec<StagedToken> = Vec::new();
    for line in &lines {
        // A line's alignment anchor is its first *lexical* token; a leading
        // whitespace token would otherwise mask the indentation.
        let first_index = line
            .indices
            .iter()
            .copied()
            .find(|&index| !matches!(run.tokens[index].kind, LayoutTokenKind::Whitespace));
        for &index in &line.indices {
            let token = &run.tokens[index];
            if matches!(token.kind, LayoutTokenKind::Whitespace) {
                continue;
            }
            let text = token.text();
            if text.is_empty() {
                continue;
            }
            if text.chars().any(char::is_whitespace) {
                return Err(NativeHeaderInputError::TokenContainsWhitespace {
                    page: token.location.page_ordinal,
                    source: token.source_ordinal,
                });
            }
            let milli_pt =
                token
                    .style
                    .font_size_milli_pt
                    .ok_or(NativeHeaderInputError::MissingFontSize {
                        page: token.location.page_ordinal,
                        source: token.source_ordinal,
                    })?;
            if milli_pt == 0 {
                return Err(NativeHeaderInputError::InvalidFontSize {
                    page: token.location.page_ordinal,
                    source: token.source_ordinal,
                    milli_pt,
                });
            }
            staged.push(StagedToken {
                block_ordinal: token.location.block_ordinal,
                line_ordinal: token.location.line_ordinal,
                line_first: first_index == Some(index),
                x_min: token.bbox.x_min,
                text: text.to_owned(),
                font_size: milli_pt as f32 / 1000.0,
                bold: token.style.bold,
                italic: token.style.italic,
            });
        }
    }
    if staged.is_empty() {
        return Err(NativeHeaderInputError::NoFeatureTokens);
    }
    let leftmost_line = staged
        .iter()
        .filter(|token| token.line_first)
        .map(|token| token.x_min)
        .fold(f64::INFINITY, f64::min);
    let leftmost_line = if leftmost_line.is_finite() {
        leftmost_line
    } else {
        0.0
    };
    let total = staged.len();
    let mut tokens = Vec::with_capacity(total);
    for (index, token) in staged.iter().enumerate() {
        let block_status = if index == 0 || staged[index - 1].block_ordinal != token.block_ordinal {
            BlockStatus::Blockstart
        } else if index + 1 == total || staged[index + 1].block_ordinal != token.block_ordinal {
            BlockStatus::Blockend
        } else {
            BlockStatus::Blockin
        };
        let line_status = if index == 0 || staged[index - 1].line_ordinal != token.line_ordinal {
            LineStatus::Linestart
        } else if index + 1 == total || staged[index + 1].line_ordinal != token.line_ordinal {
            LineStatus::Lineend
        } else {
            LineStatus::Linein
        };
        let alignment =
            if token.line_first && token.x_min > leftmost_line + NATIVE_HEADER_INDENT_EPSILON_PT {
                HeaderAlignment::LineIndent
            } else {
                HeaderAlignment::AlignedLeft
            };
        tokens.push(HeaderFeatureToken {
            text: token.text.clone(),
            block_status,
            line_status,
            alignment,
            font_size: token.font_size,
            bold: token.bold,
            italic: token.italic,
        });
    }
    Ok(tokens)
}

/// Aggregate font facts over the derived feature tokens.
///
/// `smallest <= average <= largest` holds by construction; every value is
/// strictly positive because zero fonts are refused upstream.
pub fn derive_header_font_statistics(
    tokens: &[HeaderFeatureToken],
) -> Result<HeaderFontStatistics, NativeHeaderInputError> {
    if tokens.is_empty() {
        return Err(NativeHeaderInputError::NoFeatureTokens);
    }
    let mut smallest = f32::INFINITY;
    let mut largest = f32::NEG_INFINITY;
    let mut sum = 0.0f32;
    for token in tokens {
        if !token.font_size.is_finite() || token.font_size <= 0.0 {
            return Err(NativeHeaderInputError::InvalidFontSize {
                page: 0,
                source: 0,
                milli_pt: 0,
            });
        }
        smallest = smallest.min(token.font_size);
        largest = largest.max(token.font_size);
        sum += token.font_size;
    }
    Ok(HeaderFontStatistics {
        largest_font: largest,
        average_font: sum / tokens.len() as f32,
        smallest_font: smallest,
    })
}

/// Run the header atoms on the native Header run and report typed dispositions.
///
/// The outcome carries counts, closed dispositions, sha256 digests and the
/// carrier identity; it never carries the article text.
pub fn build_native_header_input(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
) -> Result<NativeHeaderFromInputOutcome, NativeHeaderInputError> {
    let run = match carrier.run(FullSemanticRegion::Header) {
        Some(run) => run,
        None if carrier.absence(FullSemanticRegion::Header).is_some() => {
            return Err(NativeHeaderInputError::NoAdmittedTokens {
                region: FullSemanticRegion::Header,
            });
        }
        None => {
            return Err(NativeHeaderInputError::RegionNotRequested {
                region: FullSemanticRegion::Header,
            });
        }
    };
    let carrier_digest = carrier
        .digest()
        .map_err(|_| NativeHeaderInputError::Serialization)?;

    let feature_tokens = build_header_feature_tokens(run)?;
    let fonts = derive_header_font_statistics(&feature_tokens)?;
    let rows: Vec<HeaderFeatureRow> = render_header_features(&feature_tokens, fonts, lexicon)
        .map_err(NativeHeaderInputError::FeatureRender)?;
    let feature_text =
        render_header_feature_text(&rows).map_err(NativeHeaderInputError::FeatureRender)?;
    let feature_digest = format!("sha256:{:x}", Sha256::digest(feature_text.as_bytes()));

    let lines = group_run_lines(run);
    let line_texts: Vec<String> = lines
        .iter()
        .map(|line| join_line_tokens(run, &line.indices))
        .collect();
    let block_count = distinct_ordinals(run, |token| token.location.block_ordinal);
    let line_count = distinct_ordinals(run, |token| token.location.line_ordinal);

    let date_lines: Vec<(HeaderAtomDisposition, Option<String>)> = line_texts
        .iter()
        .map(|text| evidence_pair(date_disposition(&NativeDateParser::new().parse(text)), text))
        .collect();
    let name_lines: Vec<(HeaderAtomDisposition, Option<String>)> = line_texts
        .iter()
        .map(|text| {
            evidence_pair(
                name_disposition(
                    &NativePersonalNameParser::new().parse(PersonalNameContext::Header, text),
                ),
                text,
            )
        })
        .collect();
    let affiliation_lines: Vec<(HeaderAtomDisposition, Option<String>)> = line_texts
        .iter()
        .map(|text| {
            evidence_pair(
                affiliation_disposition(&NativeAffiliationParser::new().parse(text)),
                text,
            )
        })
        .collect();

    let atoms = vec![
        aggregate_atom(HeaderAtom::Date, date_lines),
        aggregate_atom(HeaderAtom::PersonalName, name_lines.clone()),
        aggregate_atom(HeaderAtom::Affiliation, affiliation_lines.clone()),
        link_atom(&carrier.identity.document_id, name_lines, affiliation_lines),
    ];

    Ok(NativeHeaderFromInputOutcome {
        schema_version: NATIVE_HEADER_INPUT_SCHEMA_VERSION.to_owned(),
        identity: NativeHeaderInputIdentity {
            document_id: carrier.identity.document_id.clone(),
            provider_id: carrier.identity.provider_id.clone(),
            source_pdf_sha256: carrier.identity.source_pdf_sha256.clone(),
            order_digest: carrier.identity.order_digest.clone(),
            carrier_digest,
        },
        region: FullSemanticRegion::Header,
        token_count: run.tokens.len(),
        line_count,
        block_count,
        feature_row_count: rows.len(),
        feature_digest,
        atoms,
        crf_scope: NativeHeaderCrfScope::OutOfScope {
            reason: NativeHeaderOutOfScope::ModelNotInTree,
        },
    })
}

struct StagedToken {
    block_ordinal: u32,
    line_ordinal: u32,
    line_first: bool,
    x_min: f64,
    text: String,
    font_size: f32,
    bold: bool,
    italic: bool,
}

struct LineGroup {
    indices: Vec<usize>,
}

/// Group the run's tokens into ordered text lines by `(block, line)` ordinal.
fn group_run_lines(run: &NativeRegionRun) -> Vec<LineGroup> {
    let mut lines: Vec<LineGroup> = Vec::new();
    let mut current: Option<(u32, u32)> = None;
    for (index, token) in run.tokens.iter().enumerate() {
        let key = (token.location.block_ordinal, token.location.line_ordinal);
        if current != Some(key) {
            lines.push(LineGroup {
                indices: Vec::new(),
            });
            current = Some(key);
        }
        lines
            .last_mut()
            .expect("a line was just pushed")
            .indices
            .push(index);
    }
    lines
}

fn distinct_ordinals(
    run: &NativeRegionRun,
    pick: impl Fn(&crate::NativeRegionToken) -> u32,
) -> usize {
    run.tokens.iter().map(pick).collect::<BTreeSet<_>>().len()
}

/// Reconstruct one line's text from its tokens.
///
/// Whitespace tokens contribute a single separator; lexical tokens are
/// concatenated with a single space unless the next token is punctuation or a
/// symbol. The result is whitespace-collapsed. This is a documented lexical
/// reconstruction, not the PDF's original micro-spacing (which the ABI does
/// not carry).
fn join_line_tokens(run: &NativeRegionRun, indices: &[usize]) -> String {
    let mut out = String::new();
    for &index in indices {
        let token = &run.tokens[index];
        let text = token.text();
        if text.is_empty() {
            continue;
        }
        if text.chars().all(char::is_whitespace) {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        let needs_space = !out.is_empty()
            && !out.ends_with(' ')
            && !matches!(
                token.kind,
                LayoutTokenKind::Punctuation | LayoutTokenKind::Symbol
            );
        if needs_space {
            out.push(' ');
        }
        out.push_str(text);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pair a disposition with the source line to fold into the evidence digest,
/// which only a recognized line contributes.
fn evidence_pair(
    disposition: HeaderAtomDisposition,
    text: &str,
) -> (HeaderAtomDisposition, Option<String>) {
    let evidence = (disposition == HeaderAtomDisposition::Recognized).then(|| text.to_owned());
    (disposition, evidence)
}

fn date_disposition(outcome: &NativeDateOutcome) -> HeaderAtomDisposition {
    match outcome {
        NativeDateOutcome::Recognized { .. } => HeaderAtomDisposition::Recognized,
        NativeDateOutcome::Empty => HeaderAtomDisposition::Empty,
        NativeDateOutcome::Malformed { .. } => HeaderAtomDisposition::Malformed,
        NativeDateOutcome::Unsupported { .. } => HeaderAtomDisposition::Unsupported,
        NativeDateOutcome::Ambiguous { .. } => HeaderAtomDisposition::Ambiguous,
        NativeDateOutcome::Oversized => HeaderAtomDisposition::Oversized,
    }
}

fn name_disposition(outcome: &NativePersonalNameOutcome) -> HeaderAtomDisposition {
    match outcome {
        NativePersonalNameOutcome::Recognized { .. } => HeaderAtomDisposition::Recognized,
        NativePersonalNameOutcome::Empty => HeaderAtomDisposition::Empty,
        NativePersonalNameOutcome::Malformed { .. } => HeaderAtomDisposition::Malformed,
        NativePersonalNameOutcome::Unsupported { .. } => HeaderAtomDisposition::Unsupported,
        NativePersonalNameOutcome::Ambiguous { .. } => HeaderAtomDisposition::Ambiguous,
        NativePersonalNameOutcome::Oversized => HeaderAtomDisposition::Oversized,
    }
}

fn affiliation_disposition(outcome: &NativeAffiliationOutcome) -> HeaderAtomDisposition {
    match outcome {
        NativeAffiliationOutcome::Recognized { .. } => HeaderAtomDisposition::Recognized,
        NativeAffiliationOutcome::Empty => HeaderAtomDisposition::Empty,
        NativeAffiliationOutcome::Malformed { .. } => HeaderAtomDisposition::Malformed,
        NativeAffiliationOutcome::Unsupported { .. } => HeaderAtomDisposition::Unsupported,
        NativeAffiliationOutcome::Ambiguous { .. } => HeaderAtomDisposition::Ambiguous,
        NativeAffiliationOutcome::Oversized => HeaderAtomDisposition::Oversized,
    }
}

/// Aggregate per-line dispositions for one atom.
///
/// Recognized dominates: if any line was recognized the atom is `Recognized`
/// with the recognized count, and the evidence digest folds the recognized
/// lines' normalized text. Otherwise the first non-empty typed refusal is
/// reported, and an all-empty lane is `Empty`.
fn aggregate_atom(
    atom: HeaderAtom,
    lines: Vec<(HeaderAtomDisposition, Option<String>)>,
) -> NativeHeaderAtomOutcome {
    let mut recognized = Vec::new();
    let mut fallback: Option<HeaderAtomDisposition> = None;
    for (disposition, evidence) in lines {
        match disposition {
            HeaderAtomDisposition::Recognized => {
                recognized.push(normalize_header_text(evidence.as_deref().unwrap_or("")));
            }
            HeaderAtomDisposition::Empty => {}
            other => {
                if fallback.is_none() {
                    fallback = Some(other);
                }
            }
        }
    }
    let recognized_count = recognized.len();
    let disposition = if recognized_count > 0 {
        HeaderAtomDisposition::Recognized
    } else {
        fallback.unwrap_or(HeaderAtomDisposition::Empty)
    };
    let joined = recognized.join(&EVIDENCE_SEPARATOR.to_string());
    NativeHeaderAtomOutcome {
        atom,
        disposition,
        recognized_count,
        evidence_digest: format!("sha256:{:x}", Sha256::digest(joined.as_bytes())),
    }
}

/// Link recognized names to recognized affiliations via the existing linker.
///
/// Occurrences are built from the recognized affiliation lines (the affiliation
/// atom reports whole lines, so the span is the line itself); recognized names
/// are linked in line order. The atom is `Recognized` when at least one name
/// links to at least one affiliation, and the evidence digest folds the linked
/// names' normalized text.
fn link_atom(
    document_id: &str,
    names: Vec<(HeaderAtomDisposition, Option<String>)>,
    affiliations: Vec<(HeaderAtomDisposition, Option<String>)>,
) -> NativeHeaderAtomOutcome {
    let occurrences: Vec<NativeAffiliationOccurrence> = affiliations
        .iter()
        .enumerate()
        .filter(|(_, (disposition, _))| *disposition == HeaderAtomDisposition::Recognized)
        .map(|(ordinal, (_, evidence))| NativeAffiliationOccurrence {
            document_id: document_id.to_owned(),
            key: format!("native-line-{ordinal}"),
            ordinal,
            span: AffiliationSpan {
                start: 0,
                end: evidence.as_deref().unwrap_or("").len().max(1),
            },
            nesting: NativeAffiliationNesting::DirectAuthorAffiliation,
        })
        .collect();

    let mut linked = Vec::new();
    let mut saw_error = false;
    for (ordinal, (disposition, evidence)) in names.iter().enumerate() {
        if *disposition != HeaderAtomDisposition::Recognized {
            continue;
        }
        let text = evidence.as_deref().unwrap_or("");
        let name = match NativePersonalNameParser::new().parse(PersonalNameContext::Header, text) {
            NativePersonalNameOutcome::Recognized { name } => name,
            _ => continue,
        };
        let span = AffiliationSpan {
            start: 0,
            end: text.len().max(1),
        };
        match NativeAffiliationLinker::new().link(&name, document_id, ordinal, span, &occurrences) {
            NativeAffiliationLinkOutcome::Linked { links, .. } if !links.is_empty() => {
                linked.push(normalize_header_text(text));
            }
            NativeAffiliationLinkOutcome::Linked { .. } => {}
            NativeAffiliationLinkOutcome::Error { .. } => saw_error = true,
        }
    }

    let recognized_count = linked.len();
    let disposition = if recognized_count > 0 {
        HeaderAtomDisposition::Recognized
    } else if saw_error {
        HeaderAtomDisposition::Malformed
    } else {
        HeaderAtomDisposition::Empty
    };
    let joined = linked.join(&EVIDENCE_SEPARATOR.to_string());
    NativeHeaderAtomOutcome {
        atom: HeaderAtom::AffiliationLink,
        disposition,
        recognized_count,
        evidence_digest: format!("sha256:{:x}", Sha256::digest(joined.as_bytes())),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
