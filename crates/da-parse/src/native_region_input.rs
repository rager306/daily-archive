//! Additive native region carrier: the join that turns the S03 layout plane
//! into ordered, redaction-safe region token runs.
//!
//! S03 (`segment_document_full`) publishes *where* each layout token belongs
//! (`FullTokenAssignment.region`) and `LexemeRow` publishes the invocation-local
//! text beside that geometry, but nothing in production connects the two: the
//! header and bibliography atoms are fed TEI-derived copies. This module is the
//! missing input band. It reads only the provider-neutral layout plane —
//! [`LayoutDocument`], [`LayoutLattice`], [`LexemeRow`], and S03's
//! [`FullSegmentationOutcome`] — and emits one deterministic, digest-bearing
//! [`NativeRegionCarrier`] per requested semantic region.
//!
//! ## Join rules (fail closed, never a silent skip)
//!
//! * **Region and order come from segmentation.** A token is admitted to a run
//!   only through its `FullTokenAssignment.region`; the run's tokens keep the
//!   segmentation order and the carrier carries the segmentation `order_digest`
//!   as provenance. The caller's region list can only *select*; it cannot
//!   reorder (runs are ordered by their first token location).
//! * **Text comes only from the lexeme sidecar.** The joined [`LexemeRow`] is
//!   looked up by `(page_ordinal, source_ordinal)` and its `text_digest` is
//!   re-derived from the bytes and cross-checked against the assignment, the
//!   lexeme row, and both layout carriers. A mismatch is a typed
//!   [`NativeRegionError::DigestMismatch`], never a dropped token.
//! * **Geometry and style come only from the layout token.** `kind`, `style`
//!   and `bbox` are copied from the `LayoutToken` under the same key; the
//!   lattice token is consulted so the lattice fingerprint and token identity
//!   are validated rather than assumed.
//! * **Every failure is typed.** Missing lexeme, missing lattice token, digest
//!   mismatch, identity/document/provider drift, duplicate or non-monotonic
//!   assignments, and an empty region request each have their own variant. An
//!   absent region is a typed [`NativeRegionAbsence::NoAdmittedTokens`] rather
//!   than a zero, so "no tokens" and "not looked at" stay distinguishable.
//!
//! ## Redaction
//!
//! Raw text is private to [`NativeRegionToken`], reachable only through
//! [`NativeRegionToken::text`]. `Debug` prints `<redacted>`, and the manual
//! `Serialize` projection emits the digest, geometry, and location but never the
//! text — mirroring [`LexemeRow`]'s discipline. Receipts built from a carrier can
//! therefore be hashed and compared without carrying the article text.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no TEI/Java type is an input. Nothing here is spliced
//! into the Java-authoritative parse route; `import_eligible` stays false. The
//! S03 wire formats (`NativeSegmenter`/v1, `segment_document_full`/v2) are not
//! touched.

use crate::native_heading_regions::{
    NATIVE_HEADING_REGIONS_SCHEMA_VERSION, NativeHeadingAssignment, NativeHeadingRegionOutcome,
};
use crate::native_word_projection::{
    NATIVE_WORD_PROJECTION_SCHEMA_VERSION, NativeWordProjection, native_word_reconstruction_digest,
};
use crate::{
    FULL_SEGMENTATION_SCHEMA_VERSION, FullLocation, FullSegmentationOutcome, FullSemanticRegion,
    LayoutBBox, LayoutDocument, LayoutLattice, LayoutStyleHints, LayoutToken, LayoutTokenKind,
    LexemeRow,
};
use serde::{Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Schema identity of the additive region-carrier surface.
pub const NATIVE_REGION_INPUT_SCHEMA_VERSION: &str = "da-parse.native-region-input/v1";

/// Upper bound on admitted tokens across all requested regions. Sits at the
/// layout/segmentation token ceiling so a carrier can never admit more tokens
/// than its S03 inputs already bound.
pub const MAX_NATIVE_REGION_TOKENS: usize = 100_000;

/// Provenance identity of one carrier, carried from the S03 segmentation
/// identity plus the segmentation's order digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRegionIdentity {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
    /// The S03 `FullSegmentationOutcome::order_digest`: the ordered region
    /// assignment identity the carrier was derived from.
    pub order_digest: String,
}

/// Typed absence of a requested region. A region with no admitted tokens is
/// reported as an absence, never silently as an empty run or a zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRegionAbsence {
    /// The requested region has no admitted tokens in the segmentation.
    NoAdmittedTokens { region: FullSemanticRegion },
}

/// One admitted token: layout geometry/style beside private lexeme text.
#[derive(Clone, PartialEq)]
pub struct NativeRegionToken {
    /// Lattice location from the S03 assignment (`page/block/line/token`).
    pub location: FullLocation,
    pub source_ordinal: u32,
    /// Digest of the joined text, cross-checked at build time.
    pub text_digest: String,
    /// Layout token kind (`Word`/`Whitespace`/`Punctuation`/`Symbol`).
    pub kind: LayoutTokenKind,
    /// Layout style hints (`bold`/`italic`/`monospace`/`font_size_milli_pt`).
    pub style: LayoutStyleHints,
    /// Layout token bounding box in the page coordinate system.
    pub bbox: LayoutBBox,
    text: String,
}

impl NativeRegionToken {
    /// The raw joined text. Deliberately the only path to it: `Debug` and the
    /// serde projection never carry it.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for NativeRegionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeRegionToken")
            .field("location", &self.location)
            .field("source_ordinal", &self.source_ordinal)
            .field("text_digest", &self.text_digest)
            .field("kind", &self.kind)
            .field("style", &self.style)
            .field("bbox", &self.bbox)
            .field("text", &"<redacted>")
            .finish()
    }
}

impl Serialize for NativeRegionToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NativeRegionToken", 6)?;
        state.serialize_field("location", &self.location)?;
        state.serialize_field("source_ordinal", &self.source_ordinal)?;
        state.serialize_field("text_digest", &self.text_digest)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("style", &self.style)?;
        state.serialize_field("bbox", &self.bbox)?;
        state.end()
    }
}

/// One requested region's admitted tokens, in S03 segmentation order.
///
/// `start`/`end` are the first and last admitted locations; `boundary_digest`
/// folds the region name and both endpoints so a run is comparable without its
/// text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRegionRun {
    pub region: FullSemanticRegion,
    pub start: FullLocation,
    pub end: FullLocation,
    pub boundary_digest: String,
    pub tokens: Vec<NativeRegionToken>,
}

/// Deterministic carrier for the requested regions.
///
/// Exactly one run per requested region that admitted tokens; exactly one
/// absence per requested region that did not. Runs are ordered by first token
/// location, so the carrier is independent of the caller's region ordering.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRegionCarrier {
    pub schema_version: String,
    pub identity: NativeRegionIdentity,
    pub runs: Vec<NativeRegionRun>,
    pub absences: Vec<NativeRegionAbsence>,
}

impl NativeRegionCarrier {
    /// The run for `region`, when the region admitted tokens.
    pub fn run(&self, region: FullSemanticRegion) -> Option<&NativeRegionRun> {
        self.runs.iter().find(|run| run.region == region)
    }

    /// The typed absence for `region`, when the region was requested but empty.
    pub fn absence(&self, region: FullSemanticRegion) -> Option<&NativeRegionAbsence> {
        self.absences
            .iter()
            .find(|absence| matches!(absence, NativeRegionAbsence::NoAdmittedTokens { region: r } if *r == region))
    }

    /// Canonical redaction-safe digest over the serialized carrier. The
    /// projection excludes raw text but includes every `text_digest`, so two
    /// carriers over different texts still differ.
    pub fn digest(&self) -> Result<String, NativeRegionError> {
        let bytes = serde_json::to_vec(self).map_err(|_| NativeRegionError::Serialization)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

/// Fail-closed refusal vocabulary of the region-carrier build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRegionError {
    /// The layout document failed its own contract.
    LayoutInvalid,
    /// The lattice does not validate against the layout document.
    LatticeInvalid,
    /// A segmentation identity field drifted (`document_id`,
    /// `source_pdf_sha256`, `lattice_fingerprint`, `schema_version`).
    IdentityMismatch(&'static str),
    /// A lexeme row declares another document.
    DocumentMismatch,
    /// A lexeme row (or the segmentation identity) declares another provider.
    ProviderMismatch,
    /// Two lexeme rows declare the same `(page, source)`.
    DuplicateLexeme { page: u32, source: u32 },
    /// Two assignments declare the same `(page, source)`.
    DuplicateAssignment { page: u32, source: u32 },
    /// The assignments are not strictly monotonic in reading order.
    MalformedOrder { page: u32, source: u32 },
    /// The segmentation references a token absent from the lexeme sidecar.
    MissingLexeme { page: u32, source: u32 },
    /// The segmentation references a token absent from the lattice (or the
    /// lattice token's layout document counterpart).
    MissingLatticeToken { page: u32, source: u32 },
    /// The re-derived text digest disagrees with the assignment or a carrier.
    DigestMismatch { page: u32, source: u32 },
    /// More requested tokens than [`MAX_NATIVE_REGION_TOKENS`].
    TooManyTokens { count: usize },
    /// No region was requested; an empty carrier has no meaning.
    EmptyRegionRequest,
    /// The word projection was built for another document, provider, source or
    /// schema, or disagrees with itself.
    WordProjectionMismatch(&'static str),
    /// The heading-region outcome does not belong to this lattice, this
    /// projection, or this document.
    HeadingRegionMismatch(&'static str),
    /// A parent span's recorded digest disagrees with the layout token it names.
    ParentDigestMismatch { page: u32, source: u32 },
    /// A word's digest or its parent's reconstruction digest does not
    /// recompute.
    WordLineageMismatch { page: u32, source: u32, word: u32 },
    /// The projection names the same word identity twice.
    DuplicateWordKey { page: u32, source: u32, word: u32 },
    /// The heading-region outcome assigns one word identity twice.
    DuplicateWordRegion { page: u32, source: u32, word: u32 },
    /// A projected word carries no region assignment.
    UncoveredWord { page: u32, source: u32, word: u32 },
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native region carrier rejected input: {self:?}")
    }
}

impl std::error::Error for NativeRegionError {}

/// Build the additive region carrier for `regions` from the S03 layout plane.
///
/// Regions are de-duplicated (first occurrence wins) and used only for
/// selection; run order is derived from the segmentation. The carrier is a pure
/// function of its inputs: two calls over equal inputs are byte-identical, and
/// the caller's region ordering never changes the result.
pub fn build_native_region_carrier(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    segmentation: &FullSegmentationOutcome,
    regions: &[FullSemanticRegion],
) -> Result<NativeRegionCarrier, NativeRegionError> {
    if regions.is_empty() {
        return Err(NativeRegionError::EmptyRegionRequest);
    }
    document
        .validate()
        .map_err(|_| NativeRegionError::LayoutInvalid)?;
    lattice
        .validate_against(document)
        .map_err(|_| NativeRegionError::LatticeInvalid)?;
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeRegionError::Serialization)?;

    if segmentation.schema_version != FULL_SEGMENTATION_SCHEMA_VERSION {
        return Err(NativeRegionError::IdentityMismatch("schema_version"));
    }
    if segmentation.identity.document_id != document.document_id {
        return Err(NativeRegionError::IdentityMismatch("document_id"));
    }
    if segmentation.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeRegionError::IdentityMismatch("source_pdf_sha256"));
    }
    if segmentation.identity.lattice_fingerprint != lattice_fingerprint {
        return Err(NativeRegionError::IdentityMismatch("lattice_fingerprint"));
    }
    let provider_id = document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .next()
        .map(|token| token.provider.id.as_str())
        .unwrap_or("");
    if segmentation.identity.provider_id != provider_id {
        return Err(NativeRegionError::ProviderMismatch);
    }

    // Selection only: keep first occurrences so the caller's ordering cannot
    // leak into the carrier.
    let mut requested: Vec<FullSemanticRegion> = Vec::with_capacity(regions.len());
    for region in regions {
        if !requested.contains(region) {
            requested.push(*region);
        }
    }

    let mut lexeme_index: BTreeMap<(u32, u32), &LexemeRow> = BTreeMap::new();
    for lexeme in lexemes {
        if lexeme.document_id != document.document_id {
            return Err(NativeRegionError::DocumentMismatch);
        }
        if lexeme.provider_id != provider_id {
            return Err(NativeRegionError::ProviderMismatch);
        }
        let key = (lexeme.page_ordinal, lexeme.source_ordinal);
        if lexeme_index.insert(key, lexeme).is_some() {
            return Err(NativeRegionError::DuplicateLexeme {
                page: key.0,
                source: key.1,
            });
        }
    }

    let mut lattice_index: BTreeMap<(u32, u32), &crate::LatticeToken> = BTreeMap::new();
    for page in &lattice.pages {
        for block in &page.blocks {
            for line in &block.lines {
                for token in &line.tokens {
                    lattice_index.insert((page.page_ordinal, token.source_ordinal), token);
                }
            }
        }
    }

    let mut document_index: BTreeMap<(u32, u32), &LayoutToken> = BTreeMap::new();
    for page in &document.pages {
        for token in &page.tokens {
            document_index.insert((token.page_ordinal, token.source_ordinal), token);
        }
    }

    let requested_tokens = segmentation
        .assignments
        .iter()
        .filter(|assignment| requested.contains(&assignment.region))
        .count();
    if requested_tokens > MAX_NATIVE_REGION_TOKENS {
        return Err(NativeRegionError::TooManyTokens {
            count: requested_tokens,
        });
    }

    // Validate the assignment stream before admitting anything: uniqueness and
    // strict reading-order monotonicity are properties of the whole receipt, not
    // of the requested subset.
    let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut previous: Option<(u32, u32, u32, u32)> = None;
    for assignment in &segmentation.assignments {
        let key = (assignment.location.page_ordinal, assignment.source_ordinal);
        if !seen.insert(key) {
            return Err(NativeRegionError::DuplicateAssignment {
                page: key.0,
                source: key.1,
            });
        }
        let order = location_key(&assignment.location);
        if previous.is_some_and(|prior| order <= prior) {
            return Err(NativeRegionError::MalformedOrder {
                page: key.0,
                source: key.1,
            });
        }
        previous = Some(order);
    }

    let mut grouped: BTreeMap<FullSemanticRegion, Vec<NativeRegionToken>> = BTreeMap::new();
    for assignment in &segmentation.assignments {
        if !requested.contains(&assignment.region) {
            continue;
        }
        let key = (assignment.location.page_ordinal, assignment.source_ordinal);
        let lattice_token =
            lattice_index
                .get(&key)
                .ok_or(NativeRegionError::MissingLatticeToken {
                    page: key.0,
                    source: key.1,
                })?;
        let layout_token =
            document_index
                .get(&key)
                .ok_or(NativeRegionError::MissingLatticeToken {
                    page: key.0,
                    source: key.1,
                })?;
        let lexeme = lexeme_index
            .get(&key)
            .ok_or(NativeRegionError::MissingLexeme {
                page: key.0,
                source: key.1,
            })?;
        if digest_text(lexeme.text()) != assignment.text_digest
            || lexeme.text_digest != assignment.text_digest
            || layout_token.text_digest != assignment.text_digest
            || lattice_token.text_digest != assignment.text_digest
        {
            return Err(NativeRegionError::DigestMismatch {
                page: key.0,
                source: key.1,
            });
        }
        grouped
            .entry(assignment.region)
            .or_default()
            .push(NativeRegionToken {
                location: assignment.location.clone(),
                source_ordinal: assignment.source_ordinal,
                text_digest: assignment.text_digest.clone(),
                kind: layout_token.kind,
                style: layout_token.style,
                bbox: layout_token.bbox,
                text: lexeme.text().to_owned(),
            });
    }

    let mut runs: Vec<NativeRegionRun> = grouped
        .into_iter()
        .map(|(region, tokens)| {
            let start = tokens
                .first()
                .map(|token| token.location.clone())
                .expect("a grouped region always has at least one token");
            let end = tokens
                .last()
                .map(|token| token.location.clone())
                .expect("a grouped region always has at least one token");
            let boundary_digest = digest_parts(&[
                region_name(region),
                &location_text(&start),
                &location_text(&end),
            ]);
            NativeRegionRun {
                region,
                start,
                end,
                boundary_digest,
                tokens,
            }
        })
        .collect();
    runs.sort_by_key(|run| location_key(&run.start));

    let run_regions: BTreeSet<FullSemanticRegion> = runs.iter().map(|run| run.region).collect();
    let mut absences: Vec<NativeRegionAbsence> = requested
        .iter()
        .filter(|region| !run_regions.contains(region))
        .map(|region| NativeRegionAbsence::NoAdmittedTokens { region: *region })
        .collect();
    absences.sort_by_key(|absence| match absence {
        NativeRegionAbsence::NoAdmittedTokens { region } => *region,
    });

    Ok(NativeRegionCarrier {
        schema_version: NATIVE_REGION_INPUT_SCHEMA_VERSION.to_owned(),
        identity: NativeRegionIdentity {
            schema_version: NATIVE_REGION_INPUT_SCHEMA_VERSION.to_owned(),
            document_id: segmentation.identity.document_id.clone(),
            provider_id: segmentation.identity.provider_id.clone(),
            source_pdf_sha256: segmentation.identity.source_pdf_sha256.clone(),
            lattice_fingerprint,
            config_fingerprint: segmentation.identity.config_fingerprint.clone(),
            order_digest: segmentation.order_digest.clone(),
        },
        runs,
        absences,
    })
}

fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn digest_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn location_key(location: &FullLocation) -> (u32, u32, u32, u32) {
    (
        location.page_ordinal,
        location.block_ordinal,
        location.line_ordinal,
        location.token_ordinal,
    )
}

fn location_text(location: &FullLocation) -> String {
    let (page, block, line, token) = location_key(location);
    format!("{page}:{block}:{line}:{token}")
}

fn region_name(region: FullSemanticRegion) -> &'static str {
    match region {
        FullSemanticRegion::Header => "header",
        FullSemanticRegion::Body => "body",
        FullSemanticRegion::References => "references",
        FullSemanticRegion::Annex => "annex",
        FullSemanticRegion::Acknowledgement => "acknowledgement",
        FullSemanticRegion::Toc => "toc",
        FullSemanticRegion::Funding => "funding",
        FullSemanticRegion::Availability => "availability",
        FullSemanticRegion::PageFurniture => "page_furniture",
    }
}

// ───────────── additive word-level carrier (S04/T04 repair) ─────────────

/// Build the additive region carrier from the word projection and its
/// heading-aware region assignment.
///
/// This is the bridge that lets the T02/T03 native bands run on the real
/// positioned donor: their contract is word-granular, while the donor's lexical
/// unit is the show-operator span. The parent-token builder above is unchanged;
/// this entry point keeps the same carrier types so no band has to learn a
/// second vocabulary.
///
/// The same fail-closed discipline applies, with the lineage chain spelled out:
/// the parent span digest must equal the layout token digest, the reconstruction
/// digest must recompute from the word digests, every word digest must recompute
/// from its own bytes, and every word must carry exactly one region assignment.
/// A word's layout location is derived: the block ordinal comes from the
/// lattice, the line ordinal is the derived visual line, and the token ordinal
/// is the word's page-scoped reading order — the lattice carries no line
/// structure for this donor, and pretending otherwise would be the dishonest
/// part.
pub fn build_native_word_region_carrier(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    projection: &NativeWordProjection,
    outcome: &NativeHeadingRegionOutcome,
    regions: &[FullSemanticRegion],
) -> Result<NativeRegionCarrier, NativeRegionError> {
    if regions.is_empty() {
        return Err(NativeRegionError::EmptyRegionRequest);
    }
    document
        .validate()
        .map_err(|_| NativeRegionError::LayoutInvalid)?;
    lattice
        .validate_against(document)
        .map_err(|_| NativeRegionError::LatticeInvalid)?;

    if projection.schema_version != NATIVE_WORD_PROJECTION_SCHEMA_VERSION {
        return Err(NativeRegionError::WordProjectionMismatch("schema_version"));
    }
    if projection.identity.document_id != document.document_id {
        return Err(NativeRegionError::WordProjectionMismatch("document_id"));
    }
    if projection.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeRegionError::WordProjectionMismatch(
            "source_pdf_sha256",
        ));
    }
    if projection.identity.parent_count as usize != projection.parents.len()
        || projection.identity.word_count as usize
            != projection
                .parents
                .iter()
                .map(|parent| parent.words.len())
                .sum::<usize>()
    {
        return Err(NativeRegionError::WordProjectionMismatch("identity_counts"));
    }

    if outcome.schema_version != NATIVE_HEADING_REGIONS_SCHEMA_VERSION {
        return Err(NativeRegionError::HeadingRegionMismatch("schema_version"));
    }
    if outcome.identity.document_id != document.document_id {
        return Err(NativeRegionError::HeadingRegionMismatch("document_id"));
    }
    if outcome.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeRegionError::HeadingRegionMismatch(
            "source_pdf_sha256",
        ));
    }
    if outcome.identity.word_projection_order_digest != projection.identity.order_digest {
        return Err(NativeRegionError::HeadingRegionMismatch(
            "word_projection_order_digest",
        ));
    }
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeRegionError::Serialization)?;
    if outcome.identity.lattice_fingerprint != lattice_fingerprint {
        return Err(NativeRegionError::HeadingRegionMismatch(
            "lattice_fingerprint",
        ));
    }

    let provider_id = document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .next()
        .map(|token| token.provider.id.as_str())
        .unwrap_or("");
    if projection.identity.provider_id != provider_id || outcome.identity.provider_id != provider_id
    {
        return Err(NativeRegionError::ProviderMismatch);
    }

    let mut requested: Vec<FullSemanticRegion> = Vec::with_capacity(regions.len());
    for region in regions {
        if !requested.contains(region) {
            requested.push(*region);
        }
    }

    let mut region_index: BTreeMap<(u32, u32, u32), &NativeHeadingAssignment> = BTreeMap::new();
    for assignment in &outcome.assignments {
        let key = (
            assignment.page_ordinal,
            assignment.source_ordinal,
            assignment.word_ordinal,
        );
        if region_index.insert(key, assignment).is_some() {
            return Err(NativeRegionError::DuplicateWordRegion {
                page: key.0,
                source: key.1,
                word: key.2,
            });
        }
    }

    let mut document_index: BTreeMap<(u32, u32), &LayoutToken> = BTreeMap::new();
    for page in &document.pages {
        for token in &page.tokens {
            document_index.insert((token.page_ordinal, token.source_ordinal), token);
        }
    }
    let mut lattice_index: BTreeMap<(u32, u32), (&crate::LatticeToken, u32)> = BTreeMap::new();
    for page in &lattice.pages {
        for block in &page.blocks {
            for line in &block.lines {
                for token in &line.tokens {
                    lattice_index.insert(
                        (page.page_ordinal, token.source_ordinal),
                        (token, block.source_ordinal),
                    );
                }
            }
        }
    }

    let mut seen: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
    let mut grouped: BTreeMap<FullSemanticRegion, Vec<NativeRegionToken>> = BTreeMap::new();
    for parent in &projection.parents {
        let key = (parent.page_ordinal, parent.source_ordinal);
        let token = document_index
            .get(&key)
            .ok_or(NativeRegionError::MissingLatticeToken {
                page: key.0,
                source: key.1,
            })?;
        let (lattice_token, block_ordinal) =
            lattice_index
                .get(&key)
                .ok_or(NativeRegionError::MissingLatticeToken {
                    page: key.0,
                    source: key.1,
                })?;
        if parent.parent_text_digest != token.text_digest
            || parent.parent_text_digest != lattice_token.text_digest
        {
            return Err(NativeRegionError::ParentDigestMismatch {
                page: key.0,
                source: key.1,
            });
        }
        if parent.reconstruction_digest != native_word_reconstruction_digest(&parent.words) {
            return Err(NativeRegionError::WordLineageMismatch {
                page: key.0,
                source: key.1,
                word: 0,
            });
        }
        for word in &parent.words {
            let word_key = (word.page_ordinal, word.source_ordinal, word.word_ordinal);
            if !seen.insert(word_key) {
                return Err(NativeRegionError::DuplicateWordKey {
                    page: word_key.0,
                    source: word_key.1,
                    word: word_key.2,
                });
            }
            if digest_text(word.text()) != word.text_digest {
                return Err(NativeRegionError::WordLineageMismatch {
                    page: word_key.0,
                    source: word_key.1,
                    word: word_key.2,
                });
            }
            let assignment =
                region_index
                    .get(&word_key)
                    .ok_or(NativeRegionError::UncoveredWord {
                        page: word_key.0,
                        source: word_key.1,
                        word: word_key.2,
                    })?;
            if !requested.contains(&assignment.region) {
                continue;
            }
            grouped
                .entry(assignment.region)
                .or_default()
                .push(NativeRegionToken {
                    location: FullLocation {
                        page_ordinal: word.page_ordinal,
                        block_ordinal: *block_ordinal,
                        line_ordinal: assignment.line_ordinal,
                        token_ordinal: word.word_ordinal,
                    },
                    source_ordinal: word.source_ordinal,
                    text_digest: word.text_digest.clone(),
                    kind: word.kind,
                    style: word.style,
                    bbox: word
                        .geometry
                        .estimated_bbox
                        .unwrap_or(word.geometry.parent_bbox),
                    text: word.text().to_owned(),
                });
        }
    }

    let mut runs: Vec<NativeRegionRun> = grouped
        .into_iter()
        .map(|(region, tokens)| {
            let start = tokens
                .first()
                .map(|token| token.location.clone())
                .expect("a grouped region always has at least one token");
            let end = tokens
                .last()
                .map(|token| token.location.clone())
                .expect("a grouped region always has at least one token");
            let boundary_digest = digest_parts(&[
                region_name(region),
                &location_text(&start),
                &location_text(&end),
            ]);
            NativeRegionRun {
                region,
                start,
                end,
                boundary_digest,
                tokens,
            }
        })
        .collect();
    runs.sort_by_key(|run| location_key(&run.start));

    let run_regions: BTreeSet<FullSemanticRegion> = runs.iter().map(|run| run.region).collect();
    let mut absences: Vec<NativeRegionAbsence> = requested
        .iter()
        .filter(|region| !run_regions.contains(region))
        .map(|region| NativeRegionAbsence::NoAdmittedTokens { region: *region })
        .collect();
    absences.sort_by_key(|absence| match absence {
        NativeRegionAbsence::NoAdmittedTokens { region } => *region,
    });

    Ok(NativeRegionCarrier {
        schema_version: NATIVE_REGION_INPUT_SCHEMA_VERSION.to_owned(),
        identity: NativeRegionIdentity {
            schema_version: NATIVE_REGION_INPUT_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            provider_id: provider_id.to_owned(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            lattice_fingerprint,
            config_fingerprint: outcome.identity.config_fingerprint.clone(),
            order_digest: outcome.order_digest.clone(),
        },
        runs,
        absences,
    })
}
