//! Native bibliography band: the join that runs the bibliography atoms on the
//! S03 native carrier instead of on TEI-derived copies.
//!
//! S04/T01 published [`NativeRegionCarrier`]: ordered, redaction-safe token
//! runs per semantic region, joined from the S03 layout plane
//! ([`LayoutDocument`], [`LayoutLattice`], [`LexemeRow`],
//! [`FullSegmentationOutcome`]). This module is the bibliography twin of the
//! header band (S04/T02). It consumes the **References** run and:
//!
//! 1. reconstructs the ordered native reference entries from the carrier
//!    (entries are maximal runs of consecutive lines sharing
//!    `(page_ordinal, block_ordinal)`; line text comes from the joined
//!    [`NativeRegionToken`]s — never from TEI);
//! 2. closes the previously unlinked "native facts → [`BibliographyTokenFacts`]"
//!    bridge: `text` / `line_length` / `prefix` / `suffix` / `line_status` /
//!    `block_status` / `alignment` are derived from the carrier's own lexeme
//!    text, `FullLocation` geometry and `LayoutStyleHints` — no TEI input;
//! 3. renders both bibliography ABIs through the real row builders
//!    ([`citation_row`], [`reference_segmenter_row`]) with the positional
//!    gazetteer lanes;
//! 4. runs the existing pure bibliography atoms — [`NativeCitationParser`]
//!    (citation fields), [`NativeReferenceSegmenter`] (reference segments) and
//!    [`NativeCitationAnchorScanner`] (citation anchors) — over the native
//!    entries and reports one typed disposition per atom.
//!
//! ## Two typed out-of-scopes, not silent gaps
//!
//! * The real GROBID 0.9.0 bibliography model bytes are not redistributable and
//!   are not in the tree (D358), so [`decode_reference_sequence`] cannot be
//!   invoked. The outcome carries a typed
//!   [`NativeBibliographyCrfScope::OutOfScope`] with reason
//!   [`NativeBibliographyOutOfScope::ModelNotInTree`] rather than a fabricated
//!   label sequence. The feature ABIs are still built and rendered in full, so
//!   the row bridge is exercised; only the decode step is deferred.
//! * In-text `<ref type="bibr">` citation anchors are GROBID TEI *markup*. The
//!   native layout plane carries the anchor's printed glyphs but not the
//!   markup, so the anchor scanner has no native markup input. The anchors atom
//!   therefore runs the scanner over the reconstructed native entry text and
//!   reports the typed
//!   [`NativeBibliographyAnchorScope::OutOfScope`] with reason
//!   [`NativeBibliographyOutOfScope::AnchorMarkupNotInNativePlane`]. If the
//!   native text *does* walk to a recognized anchor, the input is refused
//!   ([`NativeBibliographyInputError::AnchorScanNotNative`]) rather than
//!   promoted: PDF text is not markup.
//!
//! ## R128 separation is structural
//!
//! `citation_names` (the Java-side citation *name* population, 2198 over the
//! three checksum-bound fixtures) is not `biblStruct` (207). The outcome carries
//! `citation_names_pooled_with_biblstruct: false` and no field derived from a
//! name lane, so a caller cannot read a bibliography *reference* count as a
//! citation *name* count without an explicit, visible substitution.
//!
//! ## Redaction and quarantine
//!
//! The outcome is redaction-safe: counts, closed disposition/enum tokens,
//! sha256 digests and the carrier identity — never article text. Raw text is
//! reachable only through the bridge ([`BibliographyLineFacts::text`] and the
//! `BibliographyTokenFacts` it carries), which is the ABI surface the model
//! consumes, exactly as `build_header_feature_tokens` is for the header band.
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no TEI/Java type is an input. Nothing here is spliced
//! into the Java-authoritative parse route; `import_eligible` stays false. The
//! S03 wire formats and the T01 carrier are untouched.

use crate::{
    BibliographyAlignment, BibliographyGazetteerError, BibliographyTokenFacts,
    BibliographyTokenInput, CitationFieldRole, FactsBlockStatus, FactsLineStatus,
    FullSemanticRegion, LayoutTokenKind, NativeBibliographyFeatureError, NativeCitationAmbiguous,
    NativeCitationAnchorOutcome, NativeCitationAnchorScanner, NativeCitationMalformed,
    NativeCitationOutcome, NativeCitationParser, NativeCitationUnsupported,
    NativeReferenceListError, NativeReferenceListOutcome, NativeReferenceSegmenter,
    NativeRegionCarrier, NativeRegionRun, citation_row, gazetteer_spans, reference_segmenter_row,
    render_citation_feature_text, render_reference_segmenter_feature_text,
};
use da_ports::LexiconBundle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

/// Schema identity of the additive native bibliography-input surface.
pub const NATIVE_BIBLIOGRAPHY_INPUT_SCHEMA_VERSION: &str =
    "da-parse.native-bibliography-native-input/v1";

/// Generator identity of this band. An independent oracle must not declare it
/// (see [`require_independent_bibliography_oracle`]).
pub const NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID: &str =
    "da-parse.native-bibliography-native-input";

/// Additional horizontal indent (in points) a line's first token must carry
/// before it is reported as `LINEINDENT` rather than `ALIGNEDLEFT`. The carrier
/// exposes only the bounding box, so this documented, deterministic threshold
/// stands in for GROBID's glyph-relative character width.
pub const NATIVE_BIBLIOGRAPHY_INDENT_EPSILON_PT: f64 = 0.5;

/// Separator used when folding several recognized values into one evidence
/// digest. U+001F (unit separator) keeps joined values collision-free.
const EVIDENCE_SEPARATOR: char = '\u{1f}';

/// Closed bibliography atom vocabulary executed on the native carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum BibliographyAtom {
    CitationFields,
    ReferenceSegments,
    CitationAnchors,
}

/// Closed per-atom disposition. Mirrors the atom outcome vocabularies; no
/// source text is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BibliographyAtomDisposition {
    Recognized,
    Empty,
    Malformed,
    Unsupported,
    Ambiguous,
    Oversized,
    Truncated,
    /// The atom has no native input class in this cut; see
    /// [`NativeBibliographyOutOfScope`].
    OutOfScope,
}

/// Why an atom is not executed (or not measurable) in this cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeBibliographyOutOfScope {
    /// The GROBID 0.9.0 bibliography model bytes are not redistributable and
    /// are not in the tree (D358), so the CRF reference-sequence decode is not
    /// executed.
    ModelNotInTree,
    /// In-text `<ref type="bibr">` anchors are TEI markup; the native layout
    /// plane carries the anchor's glyphs but not the markup.
    AnchorMarkupNotInNativePlane,
}

/// Typed scope marker for the reference-sequence decode step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeBibliographyCrfScope {
    OutOfScope {
        reason: NativeBibliographyOutOfScope,
    },
}

/// Typed scope marker for the citation-anchor atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeBibliographyAnchorScope {
    OutOfScope {
        reason: NativeBibliographyOutOfScope,
    },
}

/// Provenance identity of the native carrier the outcome was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBibliographyInputIdentity {
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub order_digest: String,
    /// Digest of the redaction-safe region carrier.
    pub carrier_digest: String,
}

/// One citation-field role's count over the native entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBibliographyFieldCount {
    pub role: CitationFieldRole,
    pub count: usize,
}

/// One atom's aggregate outcome over the native reference entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBibliographyAtomOutcome {
    pub atom: BibliographyAtom,
    pub disposition: BibliographyAtomDisposition,
    /// Entries the atom accepted (fields: entries with at least one field;
    /// segments: reconstructed references; anchors: always `0`).
    pub recognized_count: usize,
    /// Items the atom counted (fields: field count; segments: segment count;
    /// anchors: always `0`).
    pub item_count: usize,
    /// sha256 of the normalized accepted values joined by U+001F. The source
    /// text itself is never carried.
    pub evidence_digest: String,
}

/// Redaction-safe result of running the bibliography atoms on the native
/// carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBibliographyFromInputOutcome {
    pub schema_version: String,
    pub identity: NativeBibliographyInputIdentity,
    /// Always [`FullSemanticRegion::References`]; carried so the outcome is
    /// self-describing.
    pub region: FullSemanticRegion,
    /// Admitted (non-whitespace) tokens in the References run.
    pub token_count: usize,
    /// Distinct native text lines reconstructed from the carrier.
    pub line_count: usize,
    /// Distinct layout blocks the References run spans.
    pub block_count: usize,
    /// Reference entries: maximal runs of consecutive lines sharing
    /// `(page_ordinal, block_ordinal)`.
    pub entry_count: usize,
    /// Whitespace-only lines skipped by the ABI projection. Visible, never a
    /// silent drop.
    pub skipped_empty_lines: usize,
    /// Non-whitespace tokens promoted into the citation ABI.
    pub citation_row_count: usize,
    /// Non-whitespace tokens promoted into the reference-segmenter ABI.
    pub reference_row_count: usize,
    /// sha256 over the rendered citation feature text.
    pub citation_feature_digest: String,
    /// sha256 over the rendered reference-segmenter feature text.
    pub reference_feature_digest: String,
    /// Per-entry citation-field disposition, in carrier order.
    pub citation_field_entries: Vec<BibliographyAtomDisposition>,
    /// Citation field counts by role, sorted by role index.
    pub citation_field_counts: Vec<NativeBibliographyFieldCount>,
    pub atoms: Vec<NativeBibliographyAtomOutcome>,
    pub crf_scope: NativeBibliographyCrfScope,
    pub anchor_scope: NativeBibliographyAnchorScope,
    /// Always `false`: the Java-side `citation_names` population is a distinct
    /// denominator and is never reconstructed by pooling this band's lanes
    /// (R128).
    pub citation_names_pooled_with_biblstruct: bool,
}

/// Fail-closed refusal vocabulary of the native bibliography band.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeBibliographyInputError {
    /// The References region was requested but admitted no tokens in the
    /// segmentation. A typed absence, never a zero.
    NoAdmittedTokens { region: FullSemanticRegion },
    /// The References region was not part of the carrier at all.
    RegionNotRequested { region: FullSemanticRegion },
    /// The run admitted tokens, but none survived the whitespace projection
    /// into the bibliography ABI.
    NoFeatureTokens,
    /// A lexeme carried internal whitespace; the ABI is word-level and never
    /// splits a token.
    TokenContainsWhitespace { page: u32, source: u32 },
    /// The anchor scanner did not walk the native text to `Empty`: native PDF
    /// text is not TEI markup, so any other walk is refused rather than
    /// promoted into an anchor claim.
    AnchorScanNotNative {
        outcome: &'static str,
        anchors: usize,
    },
    /// The pure bibliography row builder refused the derived tokens.
    Feature(NativeBibliographyFeatureError),
    /// The positional gazetteer pass refused the derived tokens.
    Gazetteer(BibliographyGazetteerError),
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeBibliographyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdmittedTokens { region } => {
                write!(
                    formatter,
                    "native bibliography region {region:?} admitted no tokens"
                )
            }
            Self::RegionNotRequested { region } => {
                write!(
                    formatter,
                    "native bibliography region {region:?} was not requested"
                )
            }
            Self::NoFeatureTokens => {
                write!(
                    formatter,
                    "native bibliography run has no non-whitespace tokens"
                )
            }
            Self::TokenContainsWhitespace { page, source } => write!(
                formatter,
                "native bibliography token {page}:{source} contains whitespace"
            ),
            Self::AnchorScanNotNative { outcome, anchors } => write!(
                formatter,
                "native bibliography anchor scan walked to `{outcome}` ({anchors} anchors); \
                 native text is not TEI markup"
            ),
            Self::Feature(error) => write!(formatter, "bibliography feature build failed: {error}"),
            Self::Gazetteer(error) => {
                write!(formatter, "bibliography gazetteer pass failed: {error}")
            }
            Self::Serialization => {
                write!(
                    formatter,
                    "native bibliography outcome serialization failed"
                )
            }
        }
    }
}

impl std::error::Error for NativeBibliographyInputError {}

/// Typed verdict of a character-level oracle comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BibliographyCoverageVerdict {
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
pub struct BibliographyOracleProvenance {
    /// Identity of the code that produced the oracle. Must differ from
    /// [`NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID`].
    pub generator_id: String,
    /// The producing backend, e.g. `java-grobid-crf`.
    pub backend: String,
    /// sha256 pin of the oracle source bytes.
    pub sha256: String,
}

/// Refusal vocabulary of the oracle independence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibliographyOracleIndependenceError {
    MissingGenerator,
    MissingBackend,
    MissingPin,
    /// The oracle was produced by the same code as the SUT.
    NotIndependent {
        generator_id: String,
    },
}

impl fmt::Display for BibliographyOracleIndependenceError {
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

impl std::error::Error for BibliographyOracleIndependenceError {}

/// One reconstructed native bibliography line: its lattice location, the
/// joined text, and the row-ABI facts of its non-whitespace tokens.
///
/// This is the ABI surface (it carries text, like `BibliographyTokenFacts`
/// itself); the [`NativeBibliographyFromInputOutcome`] built from it is
/// redaction-safe.
#[derive(Debug, Clone, PartialEq)]
pub struct BibliographyLineFacts {
    pub page_ordinal: u32,
    pub block_ordinal: u32,
    pub line_ordinal: u32,
    /// The line's text, joined from the carrier tokens by the documented
    /// lexical rule.
    pub text: String,
    /// The row-ABI facts of the line's non-whitespace tokens, in order.
    pub facts: Vec<BibliographyTokenFacts>,
}

struct LineGroup {
    page: u32,
    block: u32,
    line: u32,
    indices: Vec<usize>,
}

/// Normalize bibliography text for digest comparison: collapse whitespace and
/// fold case. Shared by the SUT and the oracle so a digest equality compares
/// the same normalized form on both sides.
pub fn normalize_bibliography_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The set of normalized alphanumeric characters in `text`. Used as the
/// character-level coverage unit: whitespace and punctuation are granularity,
/// not content.
pub fn bibliography_char_set(text: &str) -> BTreeSet<char> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Classify oracle-vs-native character coverage.
///
/// * an empty oracle is [`BibliographyCoverageVerdict::NotAssessable`] — there
///   is no expectation to measure, and that is not a pass;
/// * a native set that is a superset of the oracle set is
///   [`BibliographyCoverageVerdict::GranularityOnly`];
/// * anything else is [`BibliographyCoverageVerdict::ContentLoss`] (including
///   an empty native set against a non-empty oracle).
pub fn classify_bibliography_coverage(
    oracle: &BTreeSet<char>,
    native: &BTreeSet<char>,
) -> BibliographyCoverageVerdict {
    if oracle.is_empty() {
        return BibliographyCoverageVerdict::NotAssessable;
    }
    if oracle.is_subset(native) {
        BibliographyCoverageVerdict::GranularityOnly
    } else {
        BibliographyCoverageVerdict::ContentLoss
    }
}

/// Require an oracle to have a distinct generator, a backend, and a sha256 pin.
///
/// An oracle built by the SUT's own generator is refused as
/// [`BibliographyOracleIndependenceError::NotIndependent`]: a same-generator
/// oracle can only reproduce the SUT's own errors.
pub fn require_independent_bibliography_oracle(
    provenance: &BibliographyOracleProvenance,
) -> Result<(), BibliographyOracleIndependenceError> {
    if provenance.generator_id.trim().is_empty() {
        return Err(BibliographyOracleIndependenceError::MissingGenerator);
    }
    if provenance.generator_id == NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID {
        return Err(BibliographyOracleIndependenceError::NotIndependent {
            generator_id: provenance.generator_id.clone(),
        });
    }
    if provenance.backend.trim().is_empty() {
        return Err(BibliographyOracleIndependenceError::MissingBackend);
    }
    if !is_sha256_hex(&provenance.sha256) {
        return Err(BibliographyOracleIndependenceError::MissingPin);
    }
    Ok(())
}

/// Group the References run into ordered native lines and build the row-ABI
/// facts for each line — the previously unlinked bridge.
///
/// * line grouping comes from `FullLocation` `(page, block, line)`;
/// * `text` comes only from the joined carrier tokens (whitespace tokens are
///   separators; a single space is inserted between lexical tokens unless the
///   next is punctuation or a symbol);
/// * `block_status` / `line_status` come from first/last grouping;
/// * `alignment` comes from the line's first token `bbox.x_min` relative to the
///   leftmost line start;
/// * `line_length` is the line's admitted token count;
/// * `superscript` is not carried by the native plane and is therefore `false`;
/// * a token with internal whitespace is refused rather than split;
/// * a whitespace-only line yields an empty `facts` vector and is skipped by
///   the caller (reported as `skipped_empty_lines`).
pub fn build_bibliography_line_facts(
    run: &NativeRegionRun,
) -> Result<Vec<BibliographyLineFacts>, NativeBibliographyInputError> {
    let lines = group_run_lines(run);
    let leftmost_line = lines
        .iter()
        .filter_map(|line| first_admitted(run, &line.indices))
        .map(|index| run.tokens[index].bbox.x_min)
        .fold(f64::INFINITY, f64::min);
    let leftmost_line = if leftmost_line.is_finite() {
        leftmost_line
    } else {
        0.0
    };

    let mut out = Vec::with_capacity(lines.len());
    for line in &lines {
        let admitted: Vec<usize> = line
            .indices
            .iter()
            .copied()
            .filter(|&index| !matches!(run.tokens[index].kind, LayoutTokenKind::Whitespace))
            .filter(|&index| !run.tokens[index].text().is_empty())
            .collect();
        let facts = facts_for_line(run, &admitted, leftmost_line)?;
        out.push(BibliographyLineFacts {
            page_ordinal: line.page,
            block_ordinal: line.block,
            line_ordinal: line.line,
            text: join_line_tokens(run, &line.indices),
            facts,
        });
    }
    Ok(out)
}

/// Run the bibliography atoms on the native References run and report typed
/// dispositions.
///
/// The outcome carries counts, closed dispositions, sha256 digests and the
/// carrier identity; it never carries the article text.
pub fn build_native_bibliography_input(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
) -> Result<NativeBibliographyFromInputOutcome, NativeBibliographyInputError> {
    let run = match carrier.run(FullSemanticRegion::References) {
        Some(run) => run,
        None if carrier.absence(FullSemanticRegion::References).is_some() => {
            return Err(NativeBibliographyInputError::NoAdmittedTokens {
                region: FullSemanticRegion::References,
            });
        }
        None => {
            return Err(NativeBibliographyInputError::RegionNotRequested {
                region: FullSemanticRegion::References,
            });
        }
    };
    let carrier_digest = carrier
        .digest()
        .map_err(|_| NativeBibliographyInputError::Serialization)?;

    let lines = build_bibliography_line_facts(run)?;
    let non_empty: Vec<&BibliographyLineFacts> =
        lines.iter().filter(|line| !line.facts.is_empty()).collect();
    let skipped_empty_lines = lines.len() - non_empty.len();
    if non_empty.is_empty() {
        return Err(NativeBibliographyInputError::NoFeatureTokens);
    }

    // One entry per maximal run of consecutive lines sharing
    // `(page_ordinal, block_ordinal)`.
    let mut entries: Vec<Vec<&BibliographyLineFacts>> = Vec::new();
    let mut current_block: Option<(u32, u32)> = None;
    for line in &non_empty {
        let key = (line.page_ordinal, line.block_ordinal);
        if current_block != Some(key) {
            entries.push(Vec::new());
            current_block = Some(key);
        }
        entries
            .last_mut()
            .expect("an entry was just pushed")
            .push(line);
    }

    let mut token_count = 0usize;
    let mut citation_row_count = 0usize;
    let mut reference_row_count = 0usize;
    let mut citation_feature = String::new();
    let mut reference_feature = String::new();
    let mut citation_field_entries: Vec<BibliographyAtomDisposition> =
        Vec::with_capacity(entries.len());
    let mut citation_field_total = 0usize;
    let mut citation_field_role_counts = [0usize; CITATION_FIELD_ROLES.len()];
    let mut citation_field_evidence: Vec<String> = Vec::new();
    let mut citation_entry_texts: Vec<String> = Vec::with_capacity(entries.len());

    for entry in &entries {
        let entry_text = entry
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let facts: Vec<BibliographyTokenFacts> = entry
            .iter()
            .flat_map(|line| line.facts.iter().cloned())
            .collect();
        token_count += facts.len();
        let texts: Vec<String> = facts.iter().map(|fact| fact.text.clone()).collect();
        let spans =
            gazetteer_spans(lexicon, &texts).map_err(NativeBibliographyInputError::Gazetteer)?;
        let rows =
            citation_row(&facts, &spans, lexicon).map_err(NativeBibliographyInputError::Feature)?;
        citation_row_count += rows.len();
        citation_feature.push_str(
            &render_citation_feature_text(&rows).map_err(NativeBibliographyInputError::Feature)?,
        );
        citation_feature.push('\n');

        let max_line_length = entry.iter().map(|line| line.facts.len()).max().unwrap_or(1);
        for line in entry {
            let rows = reference_segmenter_row(&line.facts, lexicon, max_line_length)
                .map_err(NativeBibliographyInputError::Feature)?;
            reference_row_count += rows.len();
            reference_feature.push_str(
                &render_reference_segmenter_feature_text(&rows)
                    .map_err(NativeBibliographyInputError::Feature)?,
            );
            reference_feature.push('\n');
        }

        match NativeCitationParser::new().parse(&entry_text) {
            NativeCitationOutcome::Recognized { fields } => {
                citation_field_entries.push(BibliographyAtomDisposition::Recognized);
                for field in &fields {
                    citation_field_total += 1;
                    citation_field_role_counts[role_index(field.role)] += 1;
                    citation_field_evidence.push(normalize_bibliography_text(&field.value));
                }
            }
            outcome => {
                citation_field_entries.push(citation_disposition(&outcome));
            }
        }
        citation_entry_texts.push(entry_text);
    }

    let citation_feature_digest =
        format!("sha256:{:x}", Sha256::digest(citation_feature.as_bytes()));
    let reference_feature_digest =
        format!("sha256:{:x}", Sha256::digest(reference_feature.as_bytes()));

    let segments = NativeReferenceSegmenter::new().segment(
        &citation_entry_texts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    let segments_atom = segments_outcome(&segments);

    let region_text = citation_entry_texts.join("\n");
    let anchor_scope = match NativeCitationAnchorScanner::new().scan(&region_text) {
        NativeCitationAnchorOutcome::Empty => NativeBibliographyAnchorScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::AnchorMarkupNotInNativePlane,
        },
        other => {
            return Err(NativeBibliographyInputError::AnchorScanNotNative {
                outcome: anchor_outcome_kind(&other),
                anchors: anchor_count(&other),
            });
        }
    };

    let block_count = run
        .tokens
        .iter()
        .map(|token| (token.location.page_ordinal, token.location.block_ordinal))
        .collect::<BTreeSet<_>>()
        .len();
    let line_count = run
        .tokens
        .iter()
        .map(|token| {
            (
                token.location.page_ordinal,
                token.location.block_ordinal,
                token.location.line_ordinal,
            )
        })
        .collect::<BTreeSet<_>>()
        .len();

    let citation_fields_atom = NativeBibliographyAtomOutcome {
        atom: BibliographyAtom::CitationFields,
        disposition: aggregate_citation_disposition(&citation_field_entries),
        recognized_count: citation_field_entries
            .iter()
            .filter(|d| **d == BibliographyAtomDisposition::Recognized)
            .count(),
        item_count: citation_field_total,
        evidence_digest: format!(
            "sha256:{:x}",
            Sha256::digest(
                citation_field_evidence
                    .join(&EVIDENCE_SEPARATOR.to_string())
                    .as_bytes()
            )
        ),
    };

    Ok(NativeBibliographyFromInputOutcome {
        schema_version: NATIVE_BIBLIOGRAPHY_INPUT_SCHEMA_VERSION.to_owned(),
        identity: NativeBibliographyInputIdentity {
            document_id: carrier.identity.document_id.clone(),
            provider_id: carrier.identity.provider_id.clone(),
            source_pdf_sha256: carrier.identity.source_pdf_sha256.clone(),
            order_digest: carrier.identity.order_digest.clone(),
            carrier_digest,
        },
        region: FullSemanticRegion::References,
        token_count,
        line_count,
        block_count,
        entry_count: entries.len(),
        skipped_empty_lines,
        citation_row_count,
        reference_row_count,
        citation_feature_digest,
        reference_feature_digest,
        citation_field_entries,
        citation_field_counts: CITATION_FIELD_ROLES
            .iter()
            .enumerate()
            .filter(|(index, _)| citation_field_role_counts[*index] > 0)
            .map(|(index, role)| NativeBibliographyFieldCount {
                role: *role,
                count: citation_field_role_counts[index],
            })
            .collect(),
        atoms: vec![
            citation_fields_atom,
            segments_atom,
            NativeBibliographyAtomOutcome {
                atom: BibliographyAtom::CitationAnchors,
                disposition: BibliographyAtomDisposition::OutOfScope,
                recognized_count: 0,
                item_count: 0,
                evidence_digest: format!("sha256:{:x}", Sha256::digest(b"")),
            },
        ],
        crf_scope: NativeBibliographyCrfScope::OutOfScope {
            reason: NativeBibliographyOutOfScope::ModelNotInTree,
        },
        anchor_scope,
        citation_names_pooled_with_biblstruct: false,
    })
}

fn facts_for_line(
    run: &NativeRegionRun,
    admitted: &[usize],
    leftmost_line: f64,
) -> Result<Vec<BibliographyTokenFacts>, NativeBibliographyInputError> {
    let line_length = admitted.len();
    let first = admitted.first().copied();
    let mut facts = Vec::with_capacity(admitted.len());
    for &index in admitted {
        let token = &run.tokens[index];
        let text = token.text();
        if text.chars().any(char::is_whitespace) {
            return Err(NativeBibliographyInputError::TokenContainsWhitespace {
                page: token.location.page_ordinal,
                source: token.source_ordinal,
            });
        }
        let alignment = if first == Some(index)
            && token.bbox.x_min > leftmost_line + NATIVE_BIBLIOGRAPHY_INDENT_EPSILON_PT
        {
            BibliographyAlignment::Lineindent
        } else {
            BibliographyAlignment::Alignedleft
        };
        facts.push(BibliographyTokenFacts::from_token(
            &BibliographyTokenInput {
                text: text.to_owned(),
                block_status: block_status(run, index),
                line_status: line_status(run, index),
                alignment,
                line_length,
                superscript: false,
            },
        ));
    }
    Ok(facts)
}

/// `BLOCKSTART` for a block's first token, `BLOCKEND` for its last, else
/// `BLOCKIN`. Grouping is by `(page, block)` from `FullLocation`.
fn block_status(run: &NativeRegionRun, index: usize) -> FactsBlockStatus {
    let token = &run.tokens[index];
    let key = (token.location.page_ordinal, token.location.block_ordinal);
    let mut first = index;
    while first > 0
        && (
            run.tokens[first - 1].location.page_ordinal,
            run.tokens[first - 1].location.block_ordinal,
        ) == key
    {
        first -= 1;
    }
    let mut last = index;
    while last + 1 < run.tokens.len()
        && (
            run.tokens[last + 1].location.page_ordinal,
            run.tokens[last + 1].location.block_ordinal,
        ) == key
    {
        last += 1;
    }
    if index == first {
        FactsBlockStatus::Blockstart
    } else if index == last {
        FactsBlockStatus::Blockend
    } else {
        FactsBlockStatus::Blockin
    }
}

/// `LINESTART` for a line's first token, `LINEEND` for its last, else `LINEIN`.
/// Grouping is by `(page, block, line)` from `FullLocation`.
fn line_status(run: &NativeRegionRun, index: usize) -> FactsLineStatus {
    let token = &run.tokens[index];
    let key = (
        token.location.page_ordinal,
        token.location.block_ordinal,
        token.location.line_ordinal,
    );
    let mut first = index;
    while first > 0
        && (
            run.tokens[first - 1].location.page_ordinal,
            run.tokens[first - 1].location.block_ordinal,
            run.tokens[first - 1].location.line_ordinal,
        ) == key
    {
        first -= 1;
    }
    let mut last = index;
    while last + 1 < run.tokens.len()
        && (
            run.tokens[last + 1].location.page_ordinal,
            run.tokens[last + 1].location.block_ordinal,
            run.tokens[last + 1].location.line_ordinal,
        ) == key
    {
        last += 1;
    }
    if index == first {
        FactsLineStatus::Linestart
    } else if index == last {
        FactsLineStatus::Lineend
    } else {
        FactsLineStatus::Linein
    }
}

/// Group the run's tokens into ordered text lines by `(page, block, line)`.
fn group_run_lines(run: &NativeRegionRun) -> Vec<LineGroup> {
    let mut lines: Vec<LineGroup> = Vec::new();
    let mut current: Option<(u32, u32, u32)> = None;
    for (index, token) in run.tokens.iter().enumerate() {
        let key = (
            token.location.page_ordinal,
            token.location.block_ordinal,
            token.location.line_ordinal,
        );
        if current != Some(key) {
            lines.push(LineGroup {
                page: key.0,
                block: key.1,
                line: key.2,
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

/// The first non-whitespace token index of a line, for the alignment anchor.
fn first_admitted(run: &NativeRegionRun, indices: &[usize]) -> Option<usize> {
    indices
        .iter()
        .copied()
        .find(|&index| !matches!(run.tokens[index].kind, LayoutTokenKind::Whitespace))
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

fn citation_disposition(outcome: &NativeCitationOutcome) -> BibliographyAtomDisposition {
    match outcome {
        NativeCitationOutcome::Recognized { .. } => BibliographyAtomDisposition::Recognized,
        NativeCitationOutcome::Empty => BibliographyAtomDisposition::Empty,
        NativeCitationOutcome::Malformed { reason } => match reason {
            NativeCitationMalformed::ControlCharacter
            | NativeCitationMalformed::NonAsciiScript
            | NativeCitationMalformed::UnexpectedCharacter
            | NativeCitationMalformed::InvalidSpan
            | NativeCitationMalformed::MissingValue => BibliographyAtomDisposition::Malformed,
        },
        NativeCitationOutcome::Unsupported { reason } => match reason {
            NativeCitationUnsupported::UnsupportedPunctuation
            | NativeCitationUnsupported::UnsupportedRole => {
                BibliographyAtomDisposition::Unsupported
            }
        },
        NativeCitationOutcome::Ambiguous { reason } => match reason {
            NativeCitationAmbiguous::MultipleDateReadings
            | NativeCitationAmbiguous::MultipleUnlabelledTitles => {
                BibliographyAtomDisposition::Ambiguous
            }
        },
        NativeCitationOutcome::Oversized => BibliographyAtomDisposition::Oversized,
    }
}

/// Recognized dominates; otherwise the first non-empty typed refusal is
/// reported, and an all-empty lane is `Empty`.
fn aggregate_citation_disposition(
    entries: &[BibliographyAtomDisposition],
) -> BibliographyAtomDisposition {
    if entries.contains(&BibliographyAtomDisposition::Recognized) {
        return BibliographyAtomDisposition::Recognized;
    }
    for disposition in entries {
        if *disposition != BibliographyAtomDisposition::Empty {
            return *disposition;
        }
    }
    BibliographyAtomDisposition::Empty
}

fn role_index(role: CitationFieldRole) -> usize {
    CITATION_FIELD_ROLES
        .iter()
        .position(|candidate| *candidate == role)
        .expect("the citation role table covers every role")
}

/// The citation-field roles in declaration order; the counts are sorted by this
/// index so the receipt is independent of encounter order.
const CITATION_FIELD_ROLES: [CitationFieldRole; 13] = [
    CitationFieldRole::Authors,
    CitationFieldRole::PublicationDate,
    CitationFieldRole::ArticleTitle,
    CitationFieldRole::ContainerTitle,
    CitationFieldRole::Volume,
    CitationFieldRole::Issue,
    CitationFieldRole::Pages,
    CitationFieldRole::ArticleNumber,
    CitationFieldRole::Publisher,
    CitationFieldRole::Place,
    CitationFieldRole::Doi,
    CitationFieldRole::ArxivId,
    CitationFieldRole::Url,
];

fn segments_outcome(outcome: &NativeReferenceListOutcome) -> NativeBibliographyAtomOutcome {
    let (disposition, recognized_count, item_count, sources) = match outcome {
        NativeReferenceListOutcome::Recognized { references } => (
            BibliographyAtomDisposition::Recognized,
            references.len(),
            references.len(),
            references
                .iter()
                .map(|reference| reference.source.as_str())
                .collect::<Vec<_>>(),
        ),
        NativeReferenceListOutcome::Empty => (BibliographyAtomDisposition::Empty, 0, 0, Vec::new()),
        NativeReferenceListOutcome::Malformed { reason } => match reason {
            NativeReferenceListError::EmptySegment
            | NativeReferenceListError::InvalidUtf8Boundary => {
                (BibliographyAtomDisposition::Malformed, 0, 0, Vec::new())
            }
        },
        NativeReferenceListOutcome::Truncated => {
            (BibliographyAtomDisposition::Truncated, 0, 0, Vec::new())
        }
        NativeReferenceListOutcome::Oversized => {
            (BibliographyAtomDisposition::Oversized, 0, 0, Vec::new())
        }
    };
    let joined = sources
        .iter()
        .map(|source| normalize_bibliography_text(source))
        .collect::<Vec<_>>()
        .join(&EVIDENCE_SEPARATOR.to_string());
    NativeBibliographyAtomOutcome {
        atom: BibliographyAtom::ReferenceSegments,
        disposition,
        recognized_count,
        item_count,
        evidence_digest: format!("sha256:{:x}", Sha256::digest(joined.as_bytes())),
    }
}

fn anchor_outcome_kind(outcome: &NativeCitationAnchorOutcome) -> &'static str {
    match outcome {
        NativeCitationAnchorOutcome::Recognized { .. } => "recognized",
        NativeCitationAnchorOutcome::Empty => "empty",
        NativeCitationAnchorOutcome::Malformed { .. } => "malformed",
        NativeCitationAnchorOutcome::Truncated => "truncated",
        NativeCitationAnchorOutcome::Oversized => "oversized",
    }
}

fn anchor_count(outcome: &NativeCitationAnchorOutcome) -> usize {
    match outcome {
        NativeCitationAnchorOutcome::Recognized { anchors, .. } => anchors.len(),
        _ => 0,
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
