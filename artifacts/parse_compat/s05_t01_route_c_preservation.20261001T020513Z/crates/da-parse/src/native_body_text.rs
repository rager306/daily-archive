//! Additive native body-text assembly and the scope-bound content-alignment verdict.
//!
//! S03 named but did not resolve the token-coverage gap between the
//! `pdf_inspector_positioned` lane (hub 14 118, rlm 8 330, reca 5 430 spans) and
//! the `pdfalto` lane (40 501 / 23 700 / 27 560; aggregate ×3.29 over 202 pages,
//! `doc/review/grobid-absorption-s03-spike.md` §3.5). A *count* cannot separate
//! tokenization granularity from real body-text loss, and neither can a ratio
//! between two totals: a surplus can mask a selective loss, and equal totals can
//! hide different content. This module therefore issues no verdict from counts.
//!
//! ## What a verdict requires
//!
//! [`classify_body_coverage`] takes an explicit [`BodyScope`] on each side and an
//! independent **ordered, occurrence-conserving, token-boundary-grounded**
//! alignment over the actual native characters against the independent oracle
//! characters. Both sides must be tokenized at the declared [`BodyUnitRung`]
//! (normalization rung plus case policy); each [`BodyUnitInventory`] carries the
//! rung it was built at, and the verdict path refuses an inventory whose rung
//! does not match the rung its scope declares. A caller of
//! [`classify_body_coverage`] therefore cannot declare one rung and tokenize at
//! another. The P1 plane path has its own checking entry point,
//! [`localize_body_plane_checked`]: a plane and an oracle tokenized at different
//! rungs are refused before any localization is derived. The pure
//! [`align_body_content`] helper and the private `localize_body_plane_unchecked`
//! helper carry the same precondition and do not re-check it.
//!
//! The locator is a **two-phase greedy decomposition, not a joint optimum**. It
//! first binds exact occurrences as the optimal earliest-match monotone
//! subsequence of the side; the residual inter-anchor oracle gaps are then tiled
//! only by whole token-boundary runs. Fixing the exact anchors first can forgo a
//! tile that a joint exact-plus-tile optimum would have used, so the
//! decomposition is deliberately **conservative**: it can leave a unit
//! unresolved that a joint optimum would have tiled, and an unresolved unit is
//! uncertainty, never a fabricated tile. Concretely, oracle `x y` against side
//! `x xy` anchors `x` exactly and then cannot resolve `y`, although tiling the
//! oracle run `x y` onto the side token `xy` would resolve both; the verdict
//! stays `NotAssessable` rather than claiming `GranularityOnly`.
//!
//! Every oracle occurrence is consumed at most once and every located binding is
//! order-preserving, so each oracle occurrence lands in exactly one of six
//! classes, and the classes sum to the oracle unit total:
//!
//! 1. **exact** — the occurrence matches one side token occurrence;
//! 2. **boundary** — the occurrence is part of a full **tile**: a contiguous run
//!    of consecutive tokens *inside one fragment* on one side whose concatenation
//!    is exactly one token on the other side. A merged compound word
//!    (`average` + `case` ↔ `averagecase`) is a tile; a unit that merely appears
//!    as an arbitrary prefix or suffix of a larger token is **not** a tile and is
//!    not resolved by this class. Substring *containment* (`the` contains `he`)
//!    is not resolution at all: it is the weaker fact behind the uncertain
//!    classes below. A tile consumes the whole run on both sides, so one side
//!    token can never excuse an unbounded number of residual occurrences;
//! 3. **uncertain, present in a token** — unresolved, but the characters occur
//!    as a substring inside a single side token (`he` inside `the`). The
//!    characters exist; the token boundary does not. This is not loss and not
//!    granularity;
//! 4. **uncertain, spanning tokens** — unresolved, but the characters occur as a
//!    substring of the side's concatenated token run without fitting any single
//!    token or tile;
//! 5. **uncertain, character material** — unresolved, but the side's character
//!    material still carries every ASCII character the occurrence needs. This is
//!    where diacritic, script and math-symbol differences land: the declared rung
//!    does not fold them, so their absence is an encoding question;
//! 6. **absent** — the side's character material is short of an ASCII character
//!    the occurrence needs ([`BODY_ABSENCE_POLICY`]).
//!
//! The verdict follows from those classes, never from a length, ratio or total:
//!
//! * [`BodyCoverageVerdict::ContentLoss`] requires at least one **absent** oracle
//!   unit in the native plane (P1: the native extraction does not carry the ASCII
//!   character material at all). That is the only path to `d498_reopen_required`.
//! * An **uncertain** unit on the native plane is `NotAssessable`: the characters
//!   are present but their token boundary is not proven, so neither loss nor
//!   granularity is asserted. [`BodyGapReason::P1LocalizationUncertain`] carries
//!   the split between in-token, spanning and material localities.
//! * An absent or uncertain unit in the body while the plane carries it is a
//!   typed P2 [`BodyGapReason::P2RegionAssignmentGap`] /
//!   [`BodyGapReason::P2LocalizationUncertain`] and stays
//!   [`BodyCoverageVerdict::NotAssessable`]; it is never relabelled as content
//!   loss and never silently cleared. The P2 deficit is the occurrence-conserving
//!   count of oracle occurrences located in the plane **and** absent from the
//!   body: an occurrence absent from both is P1's, and an occurrence present but
//!   unbound is uncertainty, so each occurrence is consumed exactly once. Both
//!   axes are published on the gap (`plane_verdict`, `body_verdict`), so a P1
//!   uncertainty never disguises a P2 body miss.
//! * [`BodyCoverageVerdict::GranularityOnly`] is emitted only when every oracle
//!   occurrence is located exactly or as a tile. A surplus, an equal total, or a
//!   total word count can never produce it.
//! * An unmatched scope, a scope whose declared join policy is not the one the
//!   assembled body applied, a mismatched normalization rung, an empty oracle, or
//!   a declared population with absent documents stays `NotAssessable` with a
//!   typed reason.
//!
//! ## Rules
//!
//! * **Text is joined only through the S03 join.** The body text comes from
//!   [`build_native_region_carrier`] over the requested
//!   [`FullSemanticRegion::Body`] run: every admitted token was already joined
//!   from the [`LexemeRow`](crate::LexemeRow) sidecar by
//!   `(page_ordinal, source_ordinal)` and its `text_digest` re-derived from the
//!   bytes. This module re-derives the digest a second time before admitting a
//!   token's text, so a post-join mutation is a typed refusal rather than a
//!   silent character.
//! * **Nothing is invented.** Tokens are appended verbatim in the segmentation
//!   order the carrier preserved; no separator, filler, or dedup is synthesised.
//!   A caller that declares a word join policy passes the already-joined text to
//!   [`NativeBodyText::from_joined_text`] together with the join it used, and the
//!   scope must name the same policy; a caller that knows the genuine token
//!   boundaries passes them one fragment per token to
//!   [`NativeBodyText::from_fragments`] so a tile can never be invented
//!   across a boundary that exists.
//! * **Units are tokenized per fragment.** A tile is only recognised *inside* one
//!   fragment; concatenating two tokens across a fragment boundary must never
//!   merge the last word of one with the first word of the next — that would
//!   fabricate coverage.
//! * **Every gap is typed.** An empty `Body` region is
//!   [`NativeBodyTextError::NoBodyTokens`], a missing sidecar row is
//!   [`NativeBodyTextError::MissingLexeme`], and an assembled empty string is
//!   [`NativeBodyTextError::EmptyBodyDigest`] — never a zero-count success.
//! * **The oracle must be independent.** [`require_independent_body_oracle`]
//!   refuses an oracle that declares itself generated by the SUT or that names
//!   this module's generator id — a same-generator oracle cannot adjudicate.
//! * **ContentLoss is a live trigger, not a relabel.** [`BodyCoverageGap`] carries
//!   `d498_reopen_required`, true only for `ContentLoss`; a real body loss inside
//!   P1 reopens the engine decision (D498 / ADR-077 §7) and demands a new
//!   recorded decision instead of a renamed verdict.
//! * **Every corpus aggregate carries its population.** [`BodyAggregateInput`]
//!   declares the corpus and every outcome; [`BodyCoverageAggregate`] names the
//!   contributing and absent documents and cannot fold a partial population into
//!   a clean verdict.
//!
//! ## Redaction
//!
//! The assembled text is private, reachable only through
//! [`NativeBodyText::text`]; `Debug` prints `<redacted>` and the manual `Serialize`
//! projection emits counts, digests and the normalization ladder but never the
//! text. [`BodyUnitInventory`] holds words in memory only and is deliberately not
//! serializable — mirroring [`LexemeRow`](crate::LexemeRow)'s discipline.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no server parse type is an input. Nothing here is spliced
//! into the server-authoritative parse route; `import_eligible` stays false.

use crate::native_heading_regions::{HeadingMatch, HeadingRegionConfig, assign_heading_regions};
use crate::native_word_projection::project_native_words;
use crate::{
    FullSemanticRegion, LayoutDocument, LayoutLattice, LexemeRow,
    NATIVE_REGION_INPUT_SCHEMA_VERSION, NativeRegionAbsence, NativeRegionCarrier,
    NativeRegionError, NativeRegionRun, NativeRegionToken, build_native_region_carrier,
    build_native_word_region_carrier,
};
use serde::{Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema identity of the additive native body-text surface.
pub const NATIVE_BODY_TEXT_SCHEMA_VERSION: &str = "da-parse.native-body-text/v5";

/// Identity of the lexical unit alignment used by the verdict.
pub const BODY_ALIGNMENT_METHOD: &str = "ordered_occurrence_conserving_token_boundary_alignment_v2";

/// Upper bound on admitted body tokens. Sits at the region-carrier ceiling so a
/// body text can never admit more tokens than its S03 input already bounds.
pub const MAX_NATIVE_BODY_TOKENS: usize = 100_000;

/// Upper bound on the number of consecutive tokens one boundary tile may span.
/// This is a bounded search window, not a verdict threshold: a merge longer than
/// this stays unresolved and is therefore classified `uncertain` or `absent`
/// (fail-closed), never silently resolved.
pub const MAX_BOUNDARY_TILE_UNITS: usize = 16;

/// Upper bound on the cells one inter-anchor gap may spend on tiling. A larger
/// gap is left pending (fail-closed: uncertainty, never a fabricated tile). This
/// is a bounded search guard, not a verdict threshold.
const MAX_GAP_TILE_CELLS: usize = 65_536;

/// Upper bound on the total tiling work for one document.
const MAX_DOCUMENT_TILE_CELLS: usize = 4_000_000;

/// Generator identity of this SUT. An oracle that names it (or that declares
/// itself SUT-derived) is refused by [`require_independent_body_oracle`].
pub const NATIVE_BODY_TEXT_GENERATOR_ID: &str = "da-parse.native-body-text";

/// Threshold disposition recorded on every gap. There is no accepted threshold,
/// and no length, ratio, or total can by itself issue a verdict.
pub const BODY_GAP_THRESHOLDS: &str = "not_accepted";

/// Schema identity of the additive body-extraction/assignment surface.
pub const NATIVE_BODY_ASSIGNMENT_SCHEMA_VERSION: &str = "da-parse.native-body-assignment/v1";

/// Absence policy of the alignment. The declared normalization rung does not fold
/// diacritics or script, so a non-ASCII difference is an encoding question and
/// can never, by itself, prove that the native extraction dropped the text. An
/// occurrence is `absent` only when the side's character *material* is short of
/// an ASCII character the occurrence needs; every other unresolved occurrence is
/// localized uncertainty.
pub const BODY_ABSENCE_POLICY: &str = "non_ascii_material_is_encoding_uncertainty_not_loss";

/// Fail-closed refusal vocabulary of the native body-text assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBodyTextError {
    /// The region carrier refused the S03 plane; the variant is preserved.
    Carrier(NativeRegionError),
    /// The requested `Body` region admitted no tokens. A typed absence, never 0.
    NoBodyTokens,
    /// The joined body text is empty even though tokens were admitted.
    EmptyBodyDigest,
    /// More admitted tokens than [`MAX_NATIVE_BODY_TOKENS`].
    TooManyTokens { count: usize },
    /// The S03 join references a token absent from the lexeme sidecar.
    MissingLexeme { page: u32, source: u32 },
    /// Canonical serialization failed.
    Serialization,
    /// The oracle declared itself as, or named, the SUT generator.
    OracleNotIndependent,
    /// A scope was declared on the wrong side of the comparison.
    InvalidScope(&'static str),
    /// An inventory was tokenized at a rung other than the one its scope (or the
    /// comparison) declares; comparing it would silently apply the wrong rung.
    InventoryRungMismatch { side: &'static str },
    /// The native-plane and oracle inventories were tokenized at different
    /// rungs, so the P1 localization would silently apply the wrong tokenization.
    PlaneOracleRungMismatch,
    /// The heading/geometry route refused the plane; the variant is preserved.
    HeadingRoute(String),
    /// A `Body` boundary could not be grounded in the recorded heading entry
    /// evidence and the S03 assignment stream, so no body was adopted.
    BodyBoundaryUngrounded,
}

impl fmt::Display for NativeBodyTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native body text refused input: {self:?}")
    }
}

impl std::error::Error for NativeBodyTextError {}

/// One rung of the explicitly named normalization ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyNormalization {
    /// The assembled text exactly as joined.
    Raw,
    /// Runs of ASCII/Unicode spaces collapsed to one `' '`, both ends trimmed.
    WhitespaceCollapsed,
    /// Collapsed, then zero-width controls, soft hyphens and common ligatures
    /// normalized by hand (no dependency).
    ZeroWidthAndLigatureNormalized,
}

impl BodyNormalization {
    /// Apply one rung of the ladder.
    pub fn apply(self, text: &str) -> String {
        match self {
            BodyNormalization::Raw => text.to_owned(),
            BodyNormalization::WhitespaceCollapsed => collapse_whitespace(text),
            BodyNormalization::ZeroWidthAndLigatureNormalized => {
                normalize_zero_width_ligature(text)
            }
        }
    }
}

/// Case policy of the lexical unit inventory. Declared, never inferred: a
/// case-folded inventory cannot distinguish `Alpha` from `alpha`, so the folding
/// must be named on the scope rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyCasePolicy {
    /// Units keep their original case; `Alpha` and `alpha` are distinct units.
    CaseSensitive,
    /// Units are case-folded (Unicode lowercase) before entering the inventory.
    CaseFolded,
}

/// The declared tokenization rung: the normalization-ladder rung plus the case
/// policy an inventory was built at. Tokenization depends on both, so two
/// inventories are only comparable when their rungs are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyUnitRung {
    pub normalization: BodyNormalization,
    pub case: BodyCasePolicy,
}

impl BodyUnitRung {
    /// Construct a rung.
    pub const fn new(normalization: BodyNormalization, case: BodyCasePolicy) -> Self {
        Self {
            normalization,
            case,
        }
    }
}

/// The join rule that produced a body text. Declared, never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyJoinPolicy {
    /// Admitted token texts concatenated verbatim, in segmentation order.
    VerbatimTokenConcat,
    /// Word-granular tokens separated by exactly one space.
    SingleSpaceBetweenWordTokens,
}

impl BodyJoinPolicy {
    /// The separator this policy inserts between fragments.
    pub fn separator(self) -> &'static str {
        match self {
            BodyJoinPolicy::VerbatimTokenConcat => "",
            BodyJoinPolicy::SingleSpaceBetweenWordTokens => " ",
        }
    }
}

/// Character counts at every rung of the normalization ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBodyLengths {
    pub raw: usize,
    pub whitespace_collapsed: usize,
    pub zero_width_normalized: usize,
}

impl NativeBodyLengths {
    /// Measure `text` at every rung.
    pub fn measure(text: &str) -> Self {
        Self {
            raw: text.chars().count(),
            whitespace_collapsed: collapse_whitespace(text).chars().count(),
            zero_width_normalized: normalize_zero_width_ligature(text).chars().count(),
        }
    }

    /// The zero-count rung used to seed an aggregate.
    pub const fn zero() -> Self {
        Self {
            raw: 0,
            whitespace_collapsed: 0,
            zero_width_normalized: 0,
        }
    }

    /// Accumulate another document's lengths into this one.
    pub fn add(&mut self, other: &Self) {
        self.raw += other.raw;
        self.whitespace_collapsed += other.whitespace_collapsed;
        self.zero_width_normalized += other.zero_width_normalized;
    }
}

/// Independent lexical unit inventory. Units are maximal runs of alphanumeric
/// characters after the inventory's [`BodyUnitRung`] has been applied (the
/// declared normalization-ladder rung and case policy), so tokenization obeys
/// the declared rung instead of hardcoding one. Fragments are tokenized
/// **separately** so that concatenating two tokens can never merge the last word
/// of one with the first word of the next: the inventory answers a presence
/// question, and the join rule must not be able to fabricate coverage.
/// Deliberately not serializable — the units are words and would leak content.
#[derive(Clone, PartialEq, Eq)]
pub struct BodyUnitInventory {
    rung: BodyUnitRung,
    counts: BTreeMap<String, usize>,
    fragments: Vec<Vec<String>>,
    character_run: String,
    character_bag: BTreeMap<char, usize>,
    total: usize,
}

impl BodyUnitInventory {
    /// The empty inventory tokenized at `rung`.
    pub fn empty(rung: BodyUnitRung) -> Self {
        Self {
            rung,
            counts: BTreeMap::new(),
            fragments: Vec::new(),
            character_run: String::new(),
            character_bag: BTreeMap::new(),
            total: 0,
        }
    }

    /// Tokenize one continuous text as a single fragment at `rung`.
    pub fn from_text(text: &str, rung: BodyUnitRung) -> Self {
        let mut inventory = Self::empty(rung);
        inventory.add_fragment(text);
        inventory
    }

    /// Tokenize each fragment independently at `rung` and sum the counts.
    pub fn from_fragments<'a>(
        fragments: impl IntoIterator<Item = &'a str>,
        rung: BodyUnitRung,
    ) -> Self {
        let mut inventory = Self::empty(rung);
        for fragment in fragments {
            inventory.add_fragment(fragment);
        }
        inventory
    }

    /// The rung this inventory was tokenized at.
    pub fn rung(&self) -> BodyUnitRung {
        self.rung
    }

    /// Add one fragment's units to the multiset and its ordered unit run. The
    /// fragment is normalized and cased at this inventory's declared rung.
    pub fn add_fragment(&mut self, fragment: &str) {
        let units = tokenize_fragment(fragment, self.rung);
        if units.is_empty() {
            return;
        }
        for unit in &units {
            *self.counts.entry(unit.clone()).or_default() += 1;
            self.total += 1;
            self.character_run.push_str(unit);
            for ch in unit.chars() {
                *self.character_bag.entry(ch).or_default() += 1;
            }
        }
        self.fragments.push(units);
    }

    /// Total number of units held.
    pub fn total(&self) -> usize {
        self.total
    }

    /// How many occurrences of `unit` the inventory holds exactly.
    pub fn count_of(&self, unit: &str) -> usize {
        self.counts.get(unit).copied().unwrap_or(0)
    }

    /// True when `unit` occurs inside one token (properly or exactly). This is
    /// the conservative presence fact behind an `uncertain, present in a token`
    /// occurrence: the characters exist, the token boundary does not.
    pub fn contained_in_one_token(&self, unit: &str) -> bool {
        !unit.is_empty() && self.counts.keys().any(|token| token.contains(unit))
    }

    /// True when `unit` occurs in the concatenated token run. Concatenation is
    /// deliberately separator-free: a unit spanning two tokens is still *present*
    /// as characters, so it is uncertainty, not proven loss.
    pub fn contained_in_character_run(&self, unit: &str) -> bool {
        !unit.is_empty() && self.character_run.contains(unit)
    }

    /// True when the side's character *material* carries every ASCII character
    /// `unit` needs, order and contiguity ignored. Non-ASCII characters are
    /// deliberately exempt: the declared normalization rung does not fold
    /// diacritics or script, so their difference is an encoding question and can
    /// never prove absence on its own.
    pub fn ascii_material_present(&self, unit: &str) -> bool {
        for ch in unit.chars() {
            if !ch.is_ascii() {
                continue;
            }
            let needed = unit.chars().filter(|candidate| *candidate == ch).count();
            if self.character_bag.get(&ch).copied().unwrap_or(0) < needed {
                return false;
            }
        }
        true
    }
}

impl fmt::Debug for BodyUnitInventory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BodyUnitInventory")
            .field("rung", &self.rung)
            .field("distinct_units", &self.counts.len())
            .field("fragments", &self.fragments.len())
            .field("total", &self.total)
            .finish()
    }
}

/// Tokenize one fragment at the declared rung. The normalization-ladder rung is
/// applied first, then the declared case policy; the split into alphanumeric
/// runs happens last, so a zero-width control that the rung removes joins two
/// runs at `ZeroWidthAndLigatureNormalized` but separates them at `Raw`.
fn tokenize_fragment(fragment: &str, rung: BodyUnitRung) -> Vec<String> {
    let normalized = rung.normalization.apply(fragment);
    let normalized = match rung.case {
        BodyCasePolicy::CaseSensitive => normalized,
        BodyCasePolicy::CaseFolded => normalized.to_lowercase(),
    };
    let mut units = Vec::new();
    let mut current = String::new();
    for ch in normalized.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            units.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        units.push(current);
    }
    units
}

/// The SUT-side body text: private characters beside public counts and digests.
#[derive(Clone, PartialEq)]
pub struct NativeBodyText {
    pub schema_version: String,
    pub region: FullSemanticRegion,
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub token_count: usize,
    /// First and last admitted page ordinal, when the run spanned pages.
    pub page_span: Option<(u32, u32)>,
    /// The join policy actually applied to the fragments. The scope that
    /// declares this body must name the same policy; a scope that names another
    /// one is refused rather than believed.
    pub join: BodyJoinPolicy,
    /// Digest of the joined raw text.
    pub text_digest: String,
    pub lengths: NativeBodyLengths,
    units: BodyUnitInventory,
    text: String,
}

impl NativeBodyText {
    /// The joined body text. Deliberately the only path to it: `Debug` and the
    /// serde projection never carry it.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The body's lexical unit inventory, tokenized per admitted fragment so no
    /// join boundary can merge two words.
    pub fn units(&self) -> &BodyUnitInventory {
        &self.units
    }

    /// Characters at the requested rung.
    pub fn chars(&self, normalization: BodyNormalization) -> usize {
        match normalization {
            BodyNormalization::Raw => self.lengths.raw,
            BodyNormalization::WhitespaceCollapsed => self.lengths.whitespace_collapsed,
            BodyNormalization::ZeroWidthAndLigatureNormalized => self.lengths.zero_width_normalized,
        }
    }

    /// Assemble the body text from an S03 region carrier that requested
    /// [`FullSemanticRegion::Body`]. The fragment inventory is tokenized at the
    /// caller's declared `rung`.
    pub fn from_carrier(
        carrier: &NativeRegionCarrier,
        rung: BodyUnitRung,
    ) -> Result<Self, NativeBodyTextError> {
        if carrier.schema_version != NATIVE_REGION_INPUT_SCHEMA_VERSION {
            return Err(NativeBodyTextError::Carrier(
                NativeRegionError::IdentityMismatch("schema_version"),
            ));
        }
        if matches!(
            carrier.absence(FullSemanticRegion::Body),
            Some(NativeRegionAbsence::NoAdmittedTokens { .. })
        ) {
            return Err(NativeBodyTextError::NoBodyTokens);
        }
        let run = carrier
            .run(FullSemanticRegion::Body)
            .ok_or(NativeBodyTextError::NoBodyTokens)?;
        if run.tokens.len() > MAX_NATIVE_BODY_TOKENS {
            return Err(NativeBodyTextError::TooManyTokens {
                count: run.tokens.len(),
            });
        }
        let mut text = String::new();
        let mut units = BodyUnitInventory::empty(rung);
        let mut first_page: Option<u32> = None;
        let mut last_page: Option<u32> = None;
        for token in &run.tokens {
            if digest_text(token.text()) != token.text_digest {
                return Err(NativeBodyTextError::Carrier(
                    NativeRegionError::DigestMismatch {
                        page: token.location.page_ordinal,
                        source: token.source_ordinal,
                    },
                ));
            }
            first_page.get_or_insert(token.location.page_ordinal);
            last_page = Some(token.location.page_ordinal);
            units.add_fragment(token.text());
            text.push_str(token.text());
        }
        Self::assemble(
            carrier.identity.document_id.clone(),
            carrier.identity.source_pdf_sha256.clone(),
            run.tokens.len(),
            first_page.zip(last_page),
            BodyJoinPolicy::VerbatimTokenConcat,
            text,
            units,
        )
    }

    /// Admit an already-joined body text. The caller declares the join policy it
    /// used and must name the same policy on its [`BodyScope`]; the scope is
    /// checked against this value at classification time. This constructor only
    /// refuses an empty or oversized text.
    ///
    /// The whole text becomes **one** fragment. A caller that holds the genuine
    /// token boundaries must use [`NativeBodyText::from_fragments`] instead, so
    /// that a boundary tile is never invented across a boundary that exists.
    pub fn from_joined_text(
        document_id: impl Into<String>,
        source_pdf_sha256: impl Into<String>,
        token_count: usize,
        page_span: Option<(u32, u32)>,
        join: BodyJoinPolicy,
        text: impl Into<String>,
        rung: BodyUnitRung,
    ) -> Result<Self, NativeBodyTextError> {
        if token_count > MAX_NATIVE_BODY_TOKENS {
            return Err(NativeBodyTextError::TooManyTokens { count: token_count });
        }
        let text = text.into();
        let units = BodyUnitInventory::from_text(&text, rung);
        Self::assemble(
            document_id.into(),
            source_pdf_sha256.into(),
            token_count,
            page_span,
            join,
            text,
            units,
        )
    }

    /// Admit body text whose genuine fragment boundaries are known. Each item is
    /// one fragment (one admitted token's text); they are joined with the
    /// separator the declared [`BodyJoinPolicy`] names, and each fragment keeps
    /// its own boundary so a tile can only form *inside* a fragment.
    pub fn from_fragments<'a>(
        document_id: impl Into<String>,
        source_pdf_sha256: impl Into<String>,
        token_count: usize,
        page_span: Option<(u32, u32)>,
        join: BodyJoinPolicy,
        fragments: impl IntoIterator<Item = &'a str>,
        rung: BodyUnitRung,
    ) -> Result<Self, NativeBodyTextError> {
        if token_count > MAX_NATIVE_BODY_TOKENS {
            return Err(NativeBodyTextError::TooManyTokens { count: token_count });
        }
        let fragments: Vec<&str> = fragments.into_iter().collect();
        let text = fragments.join(join.separator());
        let units = BodyUnitInventory::from_fragments(fragments.iter().copied(), rung);
        Self::assemble(
            document_id.into(),
            source_pdf_sha256.into(),
            token_count,
            page_span,
            join,
            text,
            units,
        )
    }

    fn assemble(
        document_id: String,
        source_pdf_sha256: String,
        token_count: usize,
        page_span: Option<(u32, u32)>,
        join: BodyJoinPolicy,
        text: String,
        units: BodyUnitInventory,
    ) -> Result<Self, NativeBodyTextError> {
        if text.is_empty() {
            return Err(NativeBodyTextError::EmptyBodyDigest);
        }
        let lengths = NativeBodyLengths::measure(&text);
        Ok(Self {
            schema_version: NATIVE_BODY_TEXT_SCHEMA_VERSION.to_owned(),
            region: FullSemanticRegion::Body,
            document_id,
            source_pdf_sha256,
            token_count,
            page_span,
            join,
            text_digest: digest_text(&text),
            lengths,
            units,
            text,
        })
    }
}

impl fmt::Debug for NativeBodyText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeBodyText")
            .field("schema_version", &self.schema_version)
            .field("region", &self.region)
            .field("document_id", &self.document_id)
            .field("token_count", &self.token_count)
            .field("page_span", &self.page_span)
            .field("join", &self.join)
            .field("text_digest", &self.text_digest)
            .field("lengths", &self.lengths)
            .field("text", &"<redacted>")
            .finish()
    }
}

impl Serialize for NativeBodyText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NativeBodyText", 9)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("region", &self.region)?;
        state.serialize_field("document_id", &self.document_id)?;
        state.serialize_field("source_pdf_sha256", &self.source_pdf_sha256)?;
        state.serialize_field("token_count", &self.token_count)?;
        state.serialize_field("page_span", &self.page_span)?;
        state.serialize_field("join", &self.join)?;
        state.serialize_field("text_digest", &self.text_digest)?;
        state.serialize_field("lengths", &self.lengths)?;
        state.end()
    }
}

/// Build the native body text from the S03 layout plane, requesting only
/// [`FullSemanticRegion::Body`].
pub fn build_native_body_text(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    segmentation: &crate::FullSegmentationOutcome,
    rung: BodyUnitRung,
) -> Result<NativeBodyText, NativeBodyTextError> {
    let carrier = build_native_region_carrier(
        document,
        lattice,
        lexemes,
        segmentation,
        &[FullSemanticRegion::Body],
    )
    .map_err(map_region_error)?;
    NativeBodyText::from_carrier(&carrier, rung)
}

fn map_region_error(error: NativeRegionError) -> NativeBodyTextError {
    match error {
        NativeRegionError::MissingLexeme { page, source } => {
            NativeBodyTextError::MissingLexeme { page, source }
        }
        NativeRegionError::Serialization => NativeBodyTextError::Serialization,
        other => NativeBodyTextError::Carrier(other),
    }
}

/// Which of the two independent native routes supplied the adopted `Body` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyAssignmentRoute {
    /// The marker-based S03 segmentation (`segment_document_full`): a numbered
    /// section heading, an explicit vocabulary marker, or a references/annex
    /// marker enters the region.
    SegmentationMarker,
    /// The heading/geometry-aware walk (`assign_heading_regions`): a bold or
    /// larger-than-body-font line, or a whole-line vocabulary marker.
    HeadingGeometry,
    /// Both routes entered `Body` at different extents, so neither run is the
    /// adopted body: the adopted tokens are the word-level heading/geometry
    /// tokens whose parent `(page_ordinal, source_ordinal)` identity is carried
    /// as `Body` by both routes. Never a whole-run claim for an extent only one
    /// route carries, and never a scalar interval: `source_ordinal` may be
    /// permuted against the admitted reading order.
    BothRoutesIntersection,
}

/// The independent verification of the additive body boundary.
///
/// The two routes do not share their entry rule — one is textual/marker driven
/// and one is geometry driven — so a boundary is adopted only when both routes
/// carry **the same complete set of word-bearing parent identities** in
/// `Body`. Sharing only the first and last identity is not corroboration of a
/// full extent: the interior may differ, and `source_ordinal` may be permuted
/// against the admitted reading order, so a scalar interval over the tuple would
/// adopt tokens no route corroborates. A differing membership adopts the set
/// intersection instead, or the boundary is withheld. A boundary only one route
/// carries is never adopted, because that route's own entry evidence is not
/// independent corroboration of itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyBoundaryVerification {
    /// Both native routes carry `Body` and their word-bearing parent identity
    /// sets are equal. Complete membership agreement is the only whole-run
    /// corroboration; equal endpoints over a different interior is not.
    TwoIndependentRoutesAgree,
    /// Both routes carry `Body` but their word-bearing parent identity sets
    /// differ. The adopted run is the set intersection, in the heading route's
    /// admitted reading order, so the body is never longer than the extent both
    /// routes corroborate by identity; a shared start or a shared interval alone
    /// is never reported as whole-run agreement.
    ConservativeExtentIntersection,
    /// Exactly one route entered `Body`. That single route's own entry evidence is
    /// not independent corroboration, so no body is adopted: a named refusal, not
    /// a grounded boundary.
    SingleRouteUncorroborated,
    /// Both routes entered `Body`, but their extents do not overlap at all. The
    /// boundary is not adjudicated and no body is adopted: a named refusal, never
    /// a pick.
    RoutesDisagree,
    /// Neither route entered `Body`. A typed absence, never a zero-length body.
    NoRouteEnteredBody,
}

/// Additive Body extraction/assignment over the native plane.
///
/// Two independent native routes can assign [`FullSemanticRegion::Body`]: the
/// marker-based S03 segmentation, and the heading/geometry-aware walk. A
/// document whose section headings are not numbered (and are not vocabulary
/// entries) never enters `Body` on the marker route and is typed
/// [`NativeBodyTextError::NoBodyTokens`] — a route limitation, not proof that the
/// document has no body. This type records both routes, the boundary
/// verification, and the adopted body (when a boundary is corroborated).
///
/// The boundary is compared by the stable `(page_ordinal, source_ordinal)` token
/// identity, not by the route-local `FullLocation`: the two routes number blocks,
/// lines and tokens in their own order, so only the token identity is comparable.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBodyAssignment {
    pub schema_version: String,
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub segmentation_body_tokens: usize,
    pub heading_body_tokens: usize,
    pub segmentation_start: Option<(u32, u32)>,
    pub heading_start: Option<(u32, u32)>,
    /// Last admitted `Body` token identity on each route. Stored beside the start
    /// so a shared start with a different end can never read as full agreement.
    pub segmentation_end: Option<(u32, u32)>,
    pub heading_end: Option<(u32, u32)>,
    /// Which route supplied the adopted body, when one was adopted.
    pub adopted_route: Option<BodyAssignmentRoute>,
    /// The heading entry evidence that grounds a heading-supplied boundary.
    pub entry_match: Option<HeadingMatch>,
    pub verification: BodyBoundaryVerification,
    /// The first and last adopted token's parent identity in the heading route's
    /// admitted reading order, when a body was adopted. `None` whenever the
    /// boundary was withheld or refused. These are the ends of the adopted
    /// stream, not a scalar interval: the adopted identities are a set, and the
    /// pair may be non-monotonic under a permuted `source_ordinal`.
    pub adopted_bounds: Option<((u32, u32), (u32, u32))>,
    /// The adopted body, or `None` when no boundary was corroborated.
    pub body: Option<NativeBodyText>,
}

/// Build the additive Body assignment from the S03 layout plane.
///
/// The marker route is [`build_native_region_carrier`] over the S03 receipt; the
/// geometry route is `project_native_words` -> `assign_heading_regions` ->
/// `build_native_word_region_carrier`. Neither route reads TEI, Java, or a
/// server parse type. When the marker route never entered `Body`, the heading
/// boundary is grounded by requiring (a) the recorded [`HeadingMatch`] entry
/// evidence and (b) the boundary token to exist in the S03 assignment stream and
/// not be page furniture; a boundary that fails either check is a typed refusal,
/// not an adopted body.
#[allow(clippy::too_many_arguments)]
pub fn build_native_body_assignment(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    segmentation: &crate::FullSegmentationOutcome,
    join: BodyJoinPolicy,
    rung: BodyUnitRung,
) -> Result<NativeBodyAssignment, NativeBodyTextError> {
    let segmentation_carrier = build_native_region_carrier(
        document,
        lattice,
        lexemes,
        segmentation,
        &[FullSemanticRegion::Body],
    )
    .map_err(map_region_error)?;
    let segmentation_run = segmentation_carrier.run(FullSemanticRegion::Body);

    let projection = project_native_words(document, lexemes)
        .map_err(|error| NativeBodyTextError::HeadingRoute(format!("{error:?}")))?;
    let heading = assign_heading_regions(
        document,
        lattice,
        &projection,
        HeadingRegionConfig::default(),
    )
    .map_err(|error| NativeBodyTextError::HeadingRoute(format!("{error:?}")))?;
    let heading_carrier = build_native_word_region_carrier(
        document,
        lattice,
        &projection,
        &heading,
        &[FullSemanticRegion::Body],
    )
    .map_err(|error| NativeBodyTextError::HeadingRoute(format!("{error:?}")))?;
    let heading_run = heading_carrier.run(FullSemanticRegion::Body);

    let segmentation_bounds = segmentation_run.and_then(region_run_bounds);
    let heading_bounds = heading_run.and_then(region_run_bounds);
    let segmentation_start = segmentation_bounds.map(|(start, _)| start);
    let segmentation_end = segmentation_bounds.map(|(_, end)| end);
    let heading_start = heading_bounds.map(|(start, _)| start);
    let heading_end = heading_bounds.map(|(_, end)| end);
    let segmentation_body_tokens = segmentation_run.map_or(0, |run| run.tokens.len());
    let heading_body_tokens = heading_run.map_or(0, |run| run.tokens.len());

    let entry_match = heading
        .evidence(FullSemanticRegion::Body)
        .map(|evidence| evidence.matched_by);

    // Corroboration is parent-identity membership, never a scalar
    // `(page_ordinal, source_ordinal)` interval. `source_ordinal` is the donor's
    // extraction identity and may be permuted against the admitted reading
    // order, and one parent span may carry several word tokens, so a tuple
    // interval is neither the set a route carries nor ordered the way the route
    // admitted it. Only a parent that produces at least one word can appear on
    // the word-level heading route, so the marker route's word-bearing
    // identities are the comparison universe.
    let word_bearing_parents: BTreeSet<(u32, u32)> = projection
        .parents
        .iter()
        .map(|parent| (parent.page_ordinal, parent.source_ordinal))
        .collect();
    let segmentation_body_ids: BTreeSet<(u32, u32)> = segmentation_run
        .map(|run| run.tokens.iter().map(parent_identity_of).collect())
        .unwrap_or_default();
    let segmentation_word_body_ids: BTreeSet<(u32, u32)> = segmentation_body_ids
        .intersection(&word_bearing_parents)
        .copied()
        .collect();
    let heading_body_ids: BTreeSet<(u32, u32)> = heading_run
        .map(|run| run.tokens.iter().map(parent_identity_of).collect())
        .unwrap_or_default();

    // Boundary arbitration. A body is adopted only when BOTH native routes
    // corroborate the same complete set of word-bearing parent identities:
    // membership equality adopts the full heading run; a differing membership
    // adopts only the set intersection, in the heading route's admitted reading
    // order; a boundary only one route enters is a named refusal, because that
    // route's own entry evidence is not independent corroboration of itself.
    let (verification, adopted_route, adopted_parent_ids) = match (segmentation_run, heading_run) {
        (None, None) => (
            BodyBoundaryVerification::NoRouteEnteredBody,
            None,
            BTreeSet::new(),
        ),
        (Some(_), None) | (None, Some(_)) => (
            BodyBoundaryVerification::SingleRouteUncorroborated,
            None,
            BTreeSet::new(),
        ),
        (Some(_), Some(_)) if segmentation_word_body_ids == heading_body_ids => (
            BodyBoundaryVerification::TwoIndependentRoutesAgree,
            Some(BodyAssignmentRoute::HeadingGeometry),
            heading_body_ids.clone(),
        ),
        (Some(_), Some(_)) => {
            let intersection: BTreeSet<(u32, u32)> = segmentation_word_body_ids
                .intersection(&heading_body_ids)
                .copied()
                .collect();
            if intersection.is_empty() {
                (
                    BodyBoundaryVerification::RoutesDisagree,
                    None,
                    BTreeSet::new(),
                )
            } else {
                // Name the route whose set the intersection equals, when it
                // equals one of them; otherwise the adopted identities
                // belong to both routes.
                let route = if intersection == segmentation_word_body_ids {
                    BodyAssignmentRoute::SegmentationMarker
                } else if intersection == heading_body_ids {
                    BodyAssignmentRoute::HeadingGeometry
                } else {
                    BodyAssignmentRoute::BothRoutesIntersection
                };
                (
                    BodyBoundaryVerification::ConservativeExtentIntersection,
                    Some(route),
                    intersection,
                )
            }
        }
    };

    // Every adopted word token is admitted only when its parent identity is
    // corroborated `Body` by the S03 assignment stream. Checking only the two
    // ends let a `Header` parent inside the extent through; the per-token
    // membership check below closes that, and the adopted stream keeps the
    // heading route's admitted reading order.
    let adopted_tokens: Vec<&NativeRegionToken> = match heading_run {
        Some(run) if !adopted_parent_ids.is_empty() => {
            if entry_match.is_none() {
                return Err(NativeBodyTextError::BodyBoundaryUngrounded);
            }
            let mut adopted = Vec::new();
            for token in &run.tokens {
                let identity = parent_identity_of(token);
                if !adopted_parent_ids.contains(&identity) {
                    continue;
                }
                if !segmentation_body_ids.contains(&identity) {
                    return Err(NativeBodyTextError::BodyBoundaryUngrounded);
                }
                adopted.push(token);
            }
            if adopted.is_empty() {
                return Err(NativeBodyTextError::BodyBoundaryUngrounded);
            }
            adopted
        }
        _ => Vec::new(),
    };

    let adopted_bounds = adopted_tokens
        .first()
        .zip(adopted_tokens.last())
        .map(|(first, last)| (parent_identity_of(first), parent_identity_of(last)));

    let body = if adopted_tokens.is_empty() {
        None
    } else {
        let fragments: Vec<&str> = adopted_tokens.iter().map(|token| token.text()).collect();
        let page_span = adopted_tokens
            .first()
            .zip(adopted_tokens.last())
            .map(|(first, last)| (first.location.page_ordinal, last.location.page_ordinal));
        Some(NativeBodyText::from_fragments(
            document.document_id.clone(),
            document.source_pdf_sha256.clone(),
            fragments.len(),
            page_span,
            join,
            fragments,
            rung,
        )?)
    };

    Ok(NativeBodyAssignment {
        schema_version: NATIVE_BODY_ASSIGNMENT_SCHEMA_VERSION.to_owned(),
        document_id: document.document_id.clone(),
        source_pdf_sha256: document.source_pdf_sha256.clone(),
        segmentation_body_tokens,
        heading_body_tokens,
        segmentation_start,
        heading_start,
        segmentation_end,
        heading_end,
        adopted_route,
        entry_match,
        verification,
        adopted_bounds,
        body,
    })
}

/// The stable `(page_ordinal, source_ordinal)` **parent** identity of one region
/// token.
///
/// On the marker route the token is the parent span itself. On the word-level
/// heading route `source_ordinal` is the word's parent span extraction identity
/// and `location.token_ordinal` is the word's page-scoped reading position, so
/// the parent identity is `(location.page_ordinal, source_ordinal)` — never the
/// word ordinal, and never compared by scalar tuple order.
fn parent_identity_of(token: &NativeRegionToken) -> (u32, u32) {
    (token.location.page_ordinal, token.source_ordinal)
}

/// First and last admitted token identity of a region run, in that run's order.
fn region_run_bounds(run: &NativeRegionRun) -> Option<((u32, u32), (u32, u32))> {
    let first = run.tokens.first()?;
    let last = run.tokens.last()?;
    Some((parent_identity_of(first), parent_identity_of(last)))
}

/// Where an oracle came from. A same-generator oracle cannot adjudicate the SUT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOracleIndependence {
    /// Built structurally outside the SUT.
    Independent,
    /// Built by, or with, the SUT's own body-text code.
    SameGeneratorAsSut,
}

/// Provenance of the character oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOracleProvenance {
    pub generator_id: String,
    pub independence: BodyOracleIndependence,
}

/// Refuse an oracle that is not independent of the SUT.
pub fn require_independent_body_oracle(
    provenance: &BodyOracleProvenance,
) -> Result<(), NativeBodyTextError> {
    match provenance.independence {
        BodyOracleIndependence::SameGeneratorAsSut => {
            Err(NativeBodyTextError::OracleNotIndependent)
        }
        BodyOracleIndependence::Independent => {
            if provenance.generator_id == NATIVE_BODY_TEXT_GENERATOR_ID {
                Err(NativeBodyTextError::OracleNotIndependent)
            } else {
                Ok(())
            }
        }
    }
}

/// The oracle's character counts beside its generator identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOracleCounts {
    pub generator_id: String,
    pub lengths: NativeBodyLengths,
}

/// Closed scope vocabulary. A class is either in scope on a side or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyScopeClass {
    /// Running body prose.
    BodyProse,
    /// Figure/table/formula captions and their inline evidence.
    VisualEvidence,
    /// Bibliography and citation apparatus.
    References,
    /// Annex, acknowledgement, funding, availability and other back matter.
    BackMatter,
    /// Page numbers, running heads, cover, footnotes, marginalia.
    PageFurniture,
    /// Header metadata: title, authors, affiliations, abstract keys.
    HeaderMetadata,
}

/// Which side of the comparison a scope describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyScopeSide {
    /// Every character the native lane extracted, before any region split.
    NativePlane,
    /// The native `Body` region run alone.
    NativeBodyRegion,
    /// The independent preserved body-prose oracle.
    PreservedBodyProse,
}

/// What a side includes, how it was joined, and at which tokenization rung
/// (normalization ladder + case policy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyScope {
    pub side: BodyScopeSide,
    pub included_classes: Vec<BodyScopeClass>,
    pub join_policy: BodyJoinPolicy,
    pub normalization: BodyNormalization,
    /// Case policy this side was tokenized at. Declared, never inferred.
    pub case: BodyCasePolicy,
}

impl BodyScope {
    /// Construct a scope.
    pub fn new(
        side: BodyScopeSide,
        included_classes: Vec<BodyScopeClass>,
        join_policy: BodyJoinPolicy,
        normalization: BodyNormalization,
        case: BodyCasePolicy,
    ) -> Self {
        Self {
            side,
            included_classes,
            join_policy,
            normalization,
            case,
        }
    }

    /// The tokenization rung this scope declares.
    pub fn rung(&self) -> BodyUnitRung {
        BodyUnitRung::new(self.normalization, self.case)
    }
}

/// Whether every class the oracle includes is also in scope on the native side.
/// A native superset proves presence; a narrower native side does not.
pub fn scope_covers(native: &BodyScope, oracle: &BodyScope) -> bool {
    oracle
        .included_classes
        .iter()
        .all(|class| native.included_classes.contains(class))
}

/// Per-side occurrence counts of the ordered alignment. Private: the public
/// surface is [`BodyContentAlignment`] and [`BodyPlaneLocalization`].
///
/// `uncertain_present_in_token` is deliberately **not stored**: it is the sum of
/// the two sub-classes beside it, so the split can never drift from the bucket
/// it refines. The sub-classes are the ordered-alignment fact behind the bucket:
/// an occurrence that *equals* one side token (present, but not bound by the
/// monotone anchors) versus an occurrence that is only a *proper substring* of
/// one. Equality is tested first, so a unit that is both equal to one token and
/// a proper substring of another lands in `equal`, never in `proper`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SideLocalization {
    exact: usize,
    boundary: usize,
    uncertain_equal_in_token: usize,
    uncertain_proper_substring_in_token: usize,
    uncertain_spanning_tokens: usize,
    uncertain_character_material: usize,
    absent: usize,
}

impl SideLocalization {
    /// The in-token bucket this split refines. It is defined as the sum, so
    /// `equal + proper == bucket` is an invariant by construction, not a
    /// separately maintained counter that could disagree.
    fn uncertain_present_in_token(&self) -> usize {
        self.uncertain_equal_in_token + self.uncertain_proper_substring_in_token
    }
}

/// Independent lexical unit alignment between the native body, the whole native
/// plane, and the oracle. Every oracle occurrence is classified exactly and the
/// classes sum to `oracle_units`; the verdict reads those classes, never a total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyContentAlignment {
    pub method: String,
    pub oracle_units: usize,
    /// Oracle occurrences matched token-for-token in the native plane.
    pub in_native_plane: usize,
    /// Oracle occurrences matched token-for-token in the native body.
    pub in_native_body: usize,
    /// Oracle occurrences resolved by a full token-boundary tile in the plane.
    pub boundary_variant_from_plane: usize,
    /// Oracle occurrences resolved by a full token-boundary tile in the body.
    pub boundary_variant_from_body: usize,
    /// Unresolved plane occurrences whose characters sit inside one token.
    pub uncertain_present_in_token_from_plane: usize,
    /// Unresolved plane occurrences whose characters span token boundaries.
    pub uncertain_spanning_tokens_from_plane: usize,
    /// Unresolved plane occurrences whose characters exist only as material.
    pub uncertain_character_material_from_plane: usize,
    /// Unresolved body occurrences whose characters sit inside one token.
    pub uncertain_present_in_token_from_body: usize,
    /// Unresolved body occurrences whose characters span token boundaries.
    pub uncertain_spanning_tokens_from_body: usize,
    /// Unresolved body occurrences whose characters exist only as material.
    pub uncertain_character_material_from_body: usize,
    /// Plane occurrences with no character material at all: P1 loss input.
    pub unresolved_from_plane: usize,
    /// Body occurrences with no character material at all.
    pub unresolved_from_body: usize,
    /// Oracle occurrences located in the plane and absent from the body: the
    /// explicit occurrence-conserving P2 deficit. An occurrence absent from both
    /// sides is P1's, and an occurrence present but unbound is uncertainty, so
    /// neither is counted here. Joined per occurrence, never derived from two
    /// marginals, so the deficit is consumed exactly once.
    pub in_plane_not_in_body: usize,
}

impl BodyContentAlignment {
    /// Total plane occurrences that are present but not localized.
    pub fn uncertain_from_plane(&self) -> usize {
        self.uncertain_present_in_token_from_plane
            + self.uncertain_spanning_tokens_from_plane
            + self.uncertain_character_material_from_plane
    }

    /// Total body occurrences that are present but not localized.
    pub fn uncertain_from_body(&self) -> usize {
        self.uncertain_present_in_token_from_body
            + self.uncertain_spanning_tokens_from_body
            + self.uncertain_character_material_from_body
    }

    /// The zero alignment used to seed an aggregate.
    pub fn empty() -> Self {
        Self {
            method: BODY_ALIGNMENT_METHOD.to_owned(),
            oracle_units: 0,
            in_native_plane: 0,
            in_native_body: 0,
            boundary_variant_from_plane: 0,
            boundary_variant_from_body: 0,
            uncertain_present_in_token_from_plane: 0,
            uncertain_spanning_tokens_from_plane: 0,
            uncertain_character_material_from_plane: 0,
            uncertain_present_in_token_from_body: 0,
            uncertain_spanning_tokens_from_body: 0,
            uncertain_character_material_from_body: 0,
            unresolved_from_plane: 0,
            unresolved_from_body: 0,
            in_plane_not_in_body: 0,
        }
    }
}

impl Default for BodyContentAlignment {
    fn default() -> Self {
        Self::empty()
    }
}

/// Plane-only localization of the oracle content. This is the P1 evidence shape:
/// it carries the plane classes and deliberately **no** body/P2 field, so a
/// caller that never assembled a body cannot record a fabricated assignment gap.
/// It is a localization, not a verdict: pair it with [`classify_body_plane`] for
/// the typed P1 verdict, reason and D498 trigger. Both inventories must have been
/// tokenized at the same rung; [`localize_body_plane_checked`] is the enforced P1
/// path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPlaneLocalization {
    pub method: String,
    pub oracle_units: usize,
    /// Plane occurrences matched token-for-token.
    pub exact_in_plane: usize,
    /// Plane occurrences resolved by a full token-boundary tile.
    pub boundary_variant_from_plane: usize,
    /// Plane occurrences whose characters sit inside one token.
    pub uncertain_present_in_token_from_plane: usize,
    /// Plane occurrences whose characters span token boundaries.
    pub uncertain_spanning_tokens_from_plane: usize,
    /// Plane occurrences whose characters exist only as material.
    pub uncertain_character_material_from_plane: usize,
    /// Plane occurrences with no character material at all.
    pub absent_from_plane: usize,
}

impl BodyPlaneLocalization {
    /// The zero localization used to seed an aggregate.
    pub fn empty() -> Self {
        Self {
            method: BODY_ALIGNMENT_METHOD.to_owned(),
            oracle_units: 0,
            exact_in_plane: 0,
            boundary_variant_from_plane: 0,
            uncertain_present_in_token_from_plane: 0,
            uncertain_spanning_tokens_from_plane: 0,
            uncertain_character_material_from_plane: 0,
            absent_from_plane: 0,
        }
    }

    /// Total plane occurrences that are present but not localized.
    pub fn uncertain_from_plane(&self) -> usize {
        self.uncertain_present_in_token_from_plane
            + self.uncertain_spanning_tokens_from_plane
            + self.uncertain_character_material_from_plane
    }

    /// Accumulate another document's plane localization into this one.
    pub fn add(&mut self, other: &Self) {
        self.oracle_units += other.oracle_units;
        self.exact_in_plane += other.exact_in_plane;
        self.boundary_variant_from_plane += other.boundary_variant_from_plane;
        self.uncertain_present_in_token_from_plane += other.uncertain_present_in_token_from_plane;
        self.uncertain_spanning_tokens_from_plane += other.uncertain_spanning_tokens_from_plane;
        self.uncertain_character_material_from_plane +=
            other.uncertain_character_material_from_plane;
        self.absent_from_plane += other.absent_from_plane;
    }
}

/// Compute the ordered, occurrence-conserving plane localization (the P1 shape)
/// after refusing two inventories tokenized at different rungs.
///
/// This is the checking P1 entry point. A native plane and an independent oracle
/// tokenized at different normalization-ladder rungs or case policies are not
/// comparable; localizing across them would silently apply the wrong
/// tokenization, so that pair is a typed
/// [`NativeBodyTextError::PlaneOracleRungMismatch`] refusal instead.
pub fn localize_body_plane_checked(
    native_plane_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> Result<BodyPlaneLocalization, NativeBodyTextError> {
    if native_plane_units.rung() != oracle_units.rung() {
        return Err(NativeBodyTextError::PlaneOracleRungMismatch);
    }
    Ok(localize_body_plane_unchecked(
        native_plane_units,
        oracle_units,
    ))
}

/// Compute the ordered, occurrence-conserving plane localization (the P1 shape).
///
/// Private and unchecked: both inventories must already be known to sit at the
/// same rung, and the equality is enforced by [`localize_body_plane_checked`].
fn localize_body_plane_unchecked(
    native_plane_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> BodyPlaneLocalization {
    let located = ordered_side_localization(native_plane_units, oracle_units);
    BodyPlaneLocalization {
        method: BODY_ALIGNMENT_METHOD.to_owned(),
        oracle_units: oracle_units.total,
        exact_in_plane: located.exact,
        boundary_variant_from_plane: located.boundary,
        uncertain_present_in_token_from_plane: located.uncertain_present_in_token(),
        uncertain_spanning_tokens_from_plane: located.uncertain_spanning_tokens,
        uncertain_character_material_from_plane: located.uncertain_character_material,
        absent_from_plane: located.absent,
    }
}

/// The additive split of one side's `uncertain_present_in_token` bucket.
///
/// Every occurrence the ordered alignment could not localize at a token
/// boundary but whose characters sit inside one side token is classified by
/// **equality first**:
///
/// * `equal_in_token` — the occurrence *equals* one side token exactly (e.g. a
///   repeated oracle token whose single side occurrence the anchors already
///   consumed). The characters are present as a whole token, but this
///   occurrence was not bound in order: an anchoring fact, not a loss.
/// * `proper_substring_in_token` — the occurrence is only a *proper substring*
///   of one side token (e.g. `he` inside `the`). The characters are present, but
///   no token boundary exists: a tokenization fact, not a loss.
///
/// This is a **fact count, not a verdict**. `equal + proper` equals the
/// `uncertain_present_in_token` bucket of the same alignment by construction,
/// and neither sub-class proves preservation, granularity or loss, raises or
/// lowers D498, or accepts a threshold. A nonzero `equal_in_token` is not a
/// clearance, and the sub-classes are never rescaled into a matching ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyUncertaintySplit {
    /// Occurrences that equal one side token exactly but were not bound in order.
    pub equal_in_token: usize,
    /// Occurrences that are only a proper substring of one side token.
    pub proper_substring_in_token: usize,
}

impl BodyUncertaintySplit {
    /// The in-token bucket these two counters refine.
    pub fn total(&self) -> usize {
        self.equal_in_token + self.proper_substring_in_token
    }

    /// Fold another document's split into this one.
    pub fn add(&mut self, other: &Self) {
        self.equal_in_token += other.equal_in_token;
        self.proper_substring_in_token += other.proper_substring_in_token;
    }
}

/// Shared body of the two split entry points: the rung is checked exactly as
/// the localization paths check it, so a split can never be computed across two
/// tokenizations either.
fn split_side_uncertainty_checked(
    side: &BodyUnitInventory,
    oracle: &BodyUnitInventory,
    side_label: &'static str,
) -> Result<BodyUncertaintySplit, NativeBodyTextError> {
    if side.rung() != oracle.rung() {
        return Err(NativeBodyTextError::InventoryRungMismatch { side: side_label });
    }
    if oracle.total == 0 {
        return Ok(BodyUncertaintySplit::default());
    }
    let located = ordered_side_localization(side, oracle);
    Ok(BodyUncertaintySplit {
        equal_in_token: located.uncertain_equal_in_token,
        proper_substring_in_token: located.uncertain_proper_substring_in_token,
    })
}

/// Split the P1 plane's in-token uncertainty into equal-token and
/// proper-substring facts. Additive sibling of [`localize_body_plane_checked`]:
/// same refusal on a rung mismatch, same ordered alignment, one finer reading
/// of the same bucket. The returned `total()` equals
/// [`BodyPlaneLocalization::uncertain_present_in_token_from_plane`] for the same
/// pair of inventories.
pub fn split_plane_uncertainty(
    native_plane_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> Result<BodyUncertaintySplit, NativeBodyTextError> {
    split_side_uncertainty_checked(native_plane_units, oracle_units, "native plane")
}

/// Split the native body's in-token uncertainty into equal-token and
/// proper-substring facts. Additive sibling of the body side of
/// [`align_body_content`]; pass the same [`NativeBodyText::units`] the body
/// classification used, so the returned `total()` equals
/// [`BodyContentAlignment::uncertain_present_in_token_from_body`].
pub fn split_body_uncertainty(
    native_body_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> Result<BodyUncertaintySplit, NativeBodyTextError> {
    split_side_uncertainty_checked(native_body_units, oracle_units, "native body")
}

/// Typed P1 verdict derived from a plane-only localization, with the same
/// discipline as [`BodyCoverageGap`]: it carries a verdict, the measured reason,
/// and the D498 reopen trigger.
///
/// A zero `absent_from_plane` count is **never** reported as preservation on its
/// own. The plane can only be `GranularityOnly` when every oracle occurrence is
/// located exactly or as a tile; when nothing is `absent` but occurrences are
/// `uncertain`, the verdict is `NotAssessable` and D498 stays untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPlaneVerdict {
    pub verdict: BodyCoverageVerdict,
    pub reason: BodyGapReason,
    pub d498_reopen_required: bool,
}

/// Classify a plane-only P1 localization into a typed verdict.
///
/// * no oracle units -> `NotAssessable` / `OracleEmpty`;
/// * any `absent_from_plane` -> `ContentLoss` / `P1NativeTextAbsent` and
///   `d498_reopen_required = true` (genuine proven absence stays loss);
/// * otherwise any uncertainty -> `NotAssessable` / `P1LocalizationUncertain`
///   (presence is proven, localization is not);
/// * only a fully located plane -> `GranularityOnly` / `Matched`.
///
/// A caller that folds several documents must additionally withhold
/// `GranularityOnly` when its declared population is incomplete; see
/// [`BodyGapReason::PopulationIncomplete`].
pub fn classify_body_plane(localization: &BodyPlaneLocalization) -> BodyPlaneVerdict {
    let (verdict, reason) = if localization.oracle_units == 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::OracleEmpty,
        )
    } else if localization.absent_from_plane > 0 {
        (
            BodyCoverageVerdict::ContentLoss,
            BodyGapReason::P1NativeTextAbsent {
                missing_units: localization.absent_from_plane,
            },
        )
    } else if localization.uncertain_from_plane() > 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::P1LocalizationUncertain {
                present_in_token_units: localization.uncertain_present_in_token_from_plane,
                spanning_token_units: localization.uncertain_spanning_tokens_from_plane,
                character_material_units: localization.uncertain_character_material_from_plane,
            },
        )
    } else {
        (BodyCoverageVerdict::GranularityOnly, BodyGapReason::Matched)
    };
    BodyPlaneVerdict {
        verdict,
        reason,
        d498_reopen_required: verdict == BodyCoverageVerdict::ContentLoss,
    }
}

/// Classify a folded plane localization while carrying the declared population.
///
/// A clean `GranularityOnly` reading covers **every declared document**: when one
/// or more declared documents did not contribute, the clean reading is withheld
/// and the typed [`BodyGapReason::PopulationIncomplete`] reason names how many are
/// missing. A `ContentLoss` or `NotAssessable` reading is returned unchanged,
/// because an incomplete population can never make a loss or an uncertainty look
/// better than it is.
pub fn plane_verdict_for_population(
    localization: &BodyPlaneLocalization,
    documents_declared: usize,
    documents_measured: usize,
) -> BodyPlaneVerdict {
    let verdict = classify_body_plane(localization);
    if verdict.verdict == BodyCoverageVerdict::GranularityOnly
        && documents_declared > 0
        && documents_measured < documents_declared
    {
        return BodyPlaneVerdict {
            verdict: BodyCoverageVerdict::NotAssessable,
            reason: BodyGapReason::PopulationIncomplete {
                documents_absent: documents_declared - documents_measured,
            },
            d498_reopen_required: false,
        };
    }
    verdict
}

/// Compute the independent content alignment. Pure, no clock, no IO. Each side is
/// already tokenized; the alignment never re-splits a concatenation. All three
/// inventories must be tokenized at the same rung; this pure helper does not
/// re-check that, so [`classify_body_coverage`] is the checking entry point.
pub fn align_body_content(
    native_body_units: &BodyUnitInventory,
    native_plane_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> BodyContentAlignment {
    // Same oracle token stream on both sides, so the two outcome vectors are
    // index-aligned and a joint count is exact rather than a marginal guess.
    let plane_outcomes = occurrence_outcomes(native_plane_units, oracle_units);
    let body_outcomes = occurrence_outcomes(native_body_units, oracle_units);
    let plane = fold_occurrence_outcomes(&plane_outcomes);
    let body = fold_occurrence_outcomes(&body_outcomes);
    // The explicit occurrence-conserving P2 deficit: this oracle occurrence is
    // located in the plane but absent in the body, so it was consumed once as a
    // body-region miss and never re-counted. An occurrence absent from the plane
    // as well belongs to P1; an occurrence present but unbound is uncertainty,
    // not a deficit. A global side token count is not used here.
    let in_plane_not_in_body = plane_outcomes
        .iter()
        .zip(body_outcomes.iter())
        .filter(|(plane_outcome, body_outcome)| {
            plane_outcome.is_present_in_side() && !body_outcome.is_present_in_side()
        })
        .count();
    BodyContentAlignment {
        method: BODY_ALIGNMENT_METHOD.to_owned(),
        oracle_units: oracle_units.total,
        in_native_plane: plane.exact,
        in_native_body: body.exact,
        boundary_variant_from_plane: plane.boundary,
        boundary_variant_from_body: body.boundary,
        uncertain_present_in_token_from_plane: plane.uncertain_present_in_token(),
        uncertain_spanning_tokens_from_plane: plane.uncertain_spanning_tokens,
        uncertain_character_material_from_plane: plane.uncertain_character_material,
        uncertain_present_in_token_from_body: body.uncertain_present_in_token(),
        uncertain_spanning_tokens_from_body: body.uncertain_spanning_tokens,
        uncertain_character_material_from_body: body.uncertain_character_material,
        unresolved_from_plane: plane.absent,
        unresolved_from_body: body.absent,
        in_plane_not_in_body,
    }
}

/// Flatten fragments into an ordered token run carrying each token's fragment.
fn flatten(fragments: &[Vec<String>]) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        for token in fragment {
            out.push((fragment_index, token.as_str()));
        }
    }
    out
}

/// Which class one oracle occurrence landed in for a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OccurrenceClass {
    Pending,
    Exact,
    Boundary,
}

/// The reconstructed decision of one gap-DP cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GapChoice {
    SkipOracle,
    SkipSide,
    Exact,
    /// A run of `k >= 2` consecutive oracle tokens tiles exactly one side token.
    OracleRunTilesOne(usize),
    /// Exactly one oracle token tiles a run of `m >= 2` consecutive side tokens.
    OneTilesSideRun(usize),
}

/// 1:1 monotone anchors: walk the oracle front to back and bind each occurrence
/// to the earliest still-unused side occurrence of the same token. This is the
/// optimal earliest-match monotone subsequence, so it maximises the exact
/// occurrences while keeping every binding order-preserving and
/// occurrence-conserving — optimal **for the exact phase alone**. It is not a
/// joint exact-plus-tile optimum: an anchor fixed here can consume a side token
/// that a later tile would have used more profitably (see `tile_gap` and the
/// module-level `x y` / `x xy` counterexample), and that asymmetry is resolved in
/// favour of the conservative reading. The side tokens it skips are exactly the
/// native insertions.
fn exact_anchors(
    oracle_tokens: &[(usize, &str)],
    side_tokens: &[(usize, &str)],
    class: &mut [OccurrenceClass],
    anchor_side: &mut [Option<usize>],
) {
    let mut positions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, (_, token)) in side_tokens.iter().enumerate() {
        positions.entry(token).or_default().push(index);
    }
    let mut cursor = 0usize;
    for (index, (_, token)) in oracle_tokens.iter().enumerate() {
        let Some(list) = positions.get(token) else {
            continue;
        };
        let offset = list.partition_point(|&position| position < cursor);
        if let Some(&position) = list.get(offset) {
            class[index] = OccurrenceClass::Exact;
            anchor_side[index] = Some(position);
            cursor = position + 1;
        }
    }
}

/// Tile every inter-anchor oracle gap against the free side tokens between the
/// two surrounding anchors. A gap is left pending when it is larger than the
/// bounded search window, so the guard can only lose a tile, never invent one.
fn tile_gaps(
    oracle_tokens: &[(usize, &str)],
    side_tokens: &[(usize, &str)],
    class: &mut [OccurrenceClass],
    anchor_side: &[Option<usize>],
) {
    let mut budget = MAX_DOCUMENT_TILE_CELLS;
    let side_len = side_tokens.len();
    let mut index = 0usize;
    let mut lower = 0usize;
    while index < oracle_tokens.len() {
        if let Some(anchor) = anchor_side[index] {
            lower = anchor + 1;
            index += 1;
            continue;
        }
        let start = index;
        while index < oracle_tokens.len() && anchor_side[index].is_none() {
            index += 1;
        }
        let upper = if index < oracle_tokens.len() {
            anchor_side[index].unwrap_or(side_len)
        } else {
            side_len
        };
        if upper > lower {
            tile_gap(
                oracle_tokens,
                side_tokens,
                start,
                index,
                lower,
                upper,
                class,
                &mut budget,
            );
        }
    }
}

/// Bounded DP over one gap. Transitions: skip an oracle occurrence, skip a side
/// token, bind an equal 1:1 pair, or consume a full token-boundary tile. Only
/// whole runs whose concatenation is exactly one token on the other side count
/// as tiles; the DP maximises located oracle occurrences, so it is the ordered,
/// occurrence-conserving optimum **inside this gap given the anchors already
/// fixed** — a conditional optimum, not a joint exact-plus-tile optimum over the
/// whole document. The anchors are deliberately immovable here so a tile can
/// never displace an exact binding.
#[allow(clippy::too_many_arguments)]
fn tile_gap(
    oracle_tokens: &[(usize, &str)],
    side_tokens: &[(usize, &str)],
    oracle_start: usize,
    oracle_end: usize,
    side_start: usize,
    side_end: usize,
    class: &mut [OccurrenceClass],
    budget: &mut usize,
) {
    let rows = oracle_end - oracle_start;
    let cols = side_end - side_start;
    if rows == 0 || cols == 0 {
        return;
    }
    let cells = rows.saturating_mul(cols);
    if cells > MAX_GAP_TILE_CELLS || cells > *budget {
        return;
    }
    *budget -= cells;
    let width = cols + 1;
    let mut score = vec![0usize; (rows + 1) * width];
    let mut choice = vec![GapChoice::SkipOracle; (rows + 1) * width];
    for i in 1..=rows {
        for j in 1..=cols {
            let oracle_token = oracle_tokens[oracle_start + i - 1].1;
            let side_token = side_tokens[side_start + j - 1].1;
            let mut best = score[(i - 1) * width + j];
            let mut picked = GapChoice::SkipOracle;
            let from_side = score[i * width + (j - 1)];
            if from_side > best {
                best = from_side;
                picked = GapChoice::SkipSide;
            }
            if oracle_token == side_token {
                let candidate = score[(i - 1) * width + (j - 1)] + 1;
                if candidate >= best {
                    best = candidate;
                    picked = GapChoice::Exact;
                }
            }
            let oracle_fragment = oracle_tokens[oracle_start + i - 1].0;
            let mut parts: Vec<&str> = vec![oracle_token];
            let mut k = 1usize;
            while k < MAX_BOUNDARY_TILE_UNITS && k < i {
                let candidate_token = oracle_tokens[oracle_start + i - 1 - k];
                if candidate_token.0 != oracle_fragment {
                    break;
                }
                parts.push(candidate_token.1);
                k += 1;
                let accumulated: String = parts.iter().rev().copied().collect();
                if accumulated.len() >= side_token.len() {
                    if accumulated == side_token {
                        let candidate = score[(i - k) * width + (j - 1)] + k;
                        if candidate >= best {
                            best = candidate;
                            picked = GapChoice::OracleRunTilesOne(k);
                        }
                    }
                    break;
                }
            }
            let side_fragment = side_tokens[side_start + j - 1].0;
            let mut side_parts: Vec<&str> = vec![side_token];
            let mut m = 1usize;
            while m < MAX_BOUNDARY_TILE_UNITS && m < j {
                let candidate_token = side_tokens[side_start + j - 1 - m];
                if candidate_token.0 != side_fragment {
                    break;
                }
                side_parts.push(candidate_token.1);
                m += 1;
                let accumulated: String = side_parts.iter().rev().copied().collect();
                if accumulated.len() >= oracle_token.len() {
                    if accumulated == oracle_token {
                        let candidate = score[(i - 1) * width + (j - m)] + 1;
                        if candidate >= best {
                            best = candidate;
                            picked = GapChoice::OneTilesSideRun(m);
                        }
                    }
                    break;
                }
            }
            score[i * width + j] = best;
            choice[i * width + j] = picked;
        }
    }
    let mut i = rows;
    let mut j = cols;
    while i > 0 || j > 0 {
        match choice[i * width + j] {
            GapChoice::Exact => {
                if i == 0 || j == 0 {
                    break;
                }
                class[oracle_start + i - 1] = OccurrenceClass::Exact;
                i -= 1;
                j -= 1;
            }
            GapChoice::SkipOracle => {
                if i == 0 {
                    break;
                }
                i -= 1;
            }
            GapChoice::SkipSide => {
                if j == 0 {
                    break;
                }
                j -= 1;
            }
            GapChoice::OracleRunTilesOne(k) => {
                if k > i || j == 0 {
                    break;
                }
                for index in (i - k)..i {
                    class[oracle_start + index] = OccurrenceClass::Boundary;
                }
                i -= k;
                j -= 1;
            }
            GapChoice::OneTilesSideRun(m) => {
                if i == 0 || m > j {
                    break;
                }
                class[oracle_start + i - 1] = OccurrenceClass::Boundary;
                i -= 1;
                j -= m;
            }
        }
    }
}

/// One oracle occurrence's class against one side, before folding into counts.
/// Equality and proper substring stay distinct so the uncertainty split never
/// drifts from the in-token bucket it refines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OccurrenceOutcome {
    Exact,
    Boundary,
    UncertainEqualInToken,
    UncertainProperSubstringInToken,
    UncertainSpanningTokens,
    UncertainCharacterMaterial,
    Absent,
}

impl OccurrenceOutcome {
    /// Character material for this occurrence exists on the side, at a token
    /// boundary or inside one. `false` is a proven absence on that side.
    fn is_present_in_side(self) -> bool {
        !matches!(self, OccurrenceOutcome::Absent)
    }
}

/// Fold per-occurrence outcomes into the side counts. `equal + proper` equals
/// the in-token bucket by construction, because both come from the same fold.
fn fold_occurrence_outcomes(outcomes: &[OccurrenceOutcome]) -> SideLocalization {
    let mut localization = SideLocalization::default();
    for outcome in outcomes {
        match outcome {
            OccurrenceOutcome::Exact => localization.exact += 1,
            OccurrenceOutcome::Boundary => localization.boundary += 1,
            OccurrenceOutcome::UncertainEqualInToken => localization.uncertain_equal_in_token += 1,
            OccurrenceOutcome::UncertainProperSubstringInToken => {
                localization.uncertain_proper_substring_in_token += 1;
            }
            OccurrenceOutcome::UncertainSpanningTokens => {
                localization.uncertain_spanning_tokens += 1;
            }
            OccurrenceOutcome::UncertainCharacterMaterial => {
                localization.uncertain_character_material += 1;
            }
            OccurrenceOutcome::Absent => localization.absent += 1,
        }
    }
    localization
}

/// The ordered, occurrence-conserving localization of every oracle occurrence
/// against one side.
fn ordered_side_localization(
    side: &BodyUnitInventory,
    oracle: &BodyUnitInventory,
) -> SideLocalization {
    fold_occurrence_outcomes(&occurrence_outcomes(side, oracle))
}

/// One outcome per oracle occurrence, index-aligned with the oracle token stream.
/// Exact occurrences are the optimal earliest-match monotone subsequence of the
/// side; the residual gaps are tiled only by whole token-boundary runs. The exact
/// phase runs to completion before any tile is considered, so the locator is a
/// **two-phase greedy decomposition, not a joint optimum**: oracle `x y` against
/// side `x xy` anchors `x` and then leaves `y` unresolved even though tiling the
/// oracle run `x y` onto the side token `xy` would resolve both. The conservative
/// reading (uncertainty, never a fabricated tile) is the intended outcome. Every
/// oracle occurrence is classified exactly once and every side occurrence is
/// consumed at most once, so a single side token can never excuse unbounded
/// residual occurrences.
fn occurrence_outcomes(
    side: &BodyUnitInventory,
    oracle: &BodyUnitInventory,
) -> Vec<OccurrenceOutcome> {
    let oracle_tokens = flatten(&oracle.fragments);
    if oracle_tokens.is_empty() {
        return Vec::new();
    }
    let side_tokens = flatten(&side.fragments);
    let mut class = vec![OccurrenceClass::Pending; oracle_tokens.len()];
    if !side_tokens.is_empty() {
        let mut anchor_side = vec![None; oracle_tokens.len()];
        exact_anchors(&oracle_tokens, &side_tokens, &mut class, &mut anchor_side);
        tile_gaps(&oracle_tokens, &side_tokens, &mut class, &anchor_side);
    }
    let outcomes: Vec<OccurrenceOutcome> = oracle_tokens
        .iter()
        .enumerate()
        .map(|(index, (_, token))| match class[index] {
            OccurrenceClass::Exact => OccurrenceOutcome::Exact,
            OccurrenceClass::Boundary => OccurrenceOutcome::Boundary,
            OccurrenceClass::Pending => classify_occurrence(side, token),
        })
        .collect();
    debug_assert_eq!(
        outcomes.len(),
        oracle.total,
        "every oracle occurrence must land in exactly one class"
    );
    outcomes
}

/// Classify one unresolved occurrence by what the side still proves: characters
/// inside a token, characters spanning tokens, characters present as material,
/// or no character material at all.
fn classify_occurrence(side: &BodyUnitInventory, unit: &str) -> OccurrenceOutcome {
    // Equality first, then proper substring: a unit that equals one side token
    // is the stronger fact even when it is also a proper substring of another.
    // `count_of > 0` implies `contained_in_one_token`, so the two branches below
    // partition the bucket exactly. A global count is presence in the side, never
    // preservation of this occurrence, and an unbound occurrence is uncertainty,
    // never a proven loss.
    if side.count_of(unit) > 0 {
        OccurrenceOutcome::UncertainEqualInToken
    } else if side.contained_in_one_token(unit) {
        OccurrenceOutcome::UncertainProperSubstringInToken
    } else if side.contained_in_character_run(unit) {
        OccurrenceOutcome::UncertainSpanningTokens
    } else if side.ascii_material_present(unit) {
        OccurrenceOutcome::UncertainCharacterMaterial
    } else {
        OccurrenceOutcome::Absent
    }
}

/// Typed reason for a gap disposition. Carries the measured locating numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyGapReason {
    /// Every oracle occurrence was resolved in the native body: the token gap is
    /// granularity, not loss.
    Matched,
    /// Oracle occurrences are absent from the whole native plane: real P1 loss.
    P1NativeTextAbsent { missing_units: usize },
    /// Plane occurrences are present as characters but not localized at a token
    /// boundary: neither loss nor granularity is proven.
    P1LocalizationUncertain {
        present_in_token_units: usize,
        spanning_token_units: usize,
        character_material_units: usize,
    },
    /// The unresolvable oracle occurrences are located in the native plane but
    /// absent from the body region: a P2 region-assignment gap, not text loss.
    /// Consumed once: an occurrence absent from the plane too is P1's, not this.
    P2RegionAssignmentGap { units_in_plane_not_in_body: usize },
    /// Body occurrences are present as characters but not localized there.
    P2LocalizationUncertain {
        present_in_token_units: usize,
        spanning_token_units: usize,
        character_material_units: usize,
    },
    /// The native scope does not cover the oracle scope; no claim is possible.
    ScopeMismatch,
    /// The native scope names a join policy other than the one the assembled body
    /// actually applied, so the declared scope is not the body it describes.
    ScopeJoinMismatch,
    /// The two sides were asked at different tokenization rungs (normalization
    /// ladder or case policy), so their tokens are not comparable.
    NormalizationMismatch,
    /// The oracle carries no lexical unit; a count contradiction is refused.
    OracleEmpty,
    /// The declared population is incomplete: at least one declared document did
    /// not contribute, so a clean corpus reading is withheld.
    PopulationIncomplete { documents_absent: usize },
}

/// Typed verdict of the character comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyCoverageVerdict {
    /// Every oracle occurrence is resolved in the native body; the S03 gap is
    /// tokenization granularity, not text loss.
    GranularityOnly,
    /// Oracle content is absent from the whole native plane: real P1 loss.
    ContentLoss,
    /// Scope, normalization, population or localization leaves the question open.
    NotAssessable,
}

/// The P2 axis: the body-region verdict, computed independently of the P1 plane
/// verdict and published beside it, so a P1 uncertainty can never hide a body
/// miss. It reads the occurrence-conserving alignment, never a total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyRegionVerdict {
    pub verdict: BodyCoverageVerdict,
    pub reason: BodyGapReason,
}

/// Classify the body-region axis from the alignment.
///
/// * no oracle units -> `NotAssessable` / `OracleEmpty`;
/// * any occurrence located in the plane and absent from the body ->
///   `NotAssessable` / `P2RegionAssignmentGap` with the occurrence-conserving
///   deficit;
/// * otherwise any body-side uncertainty -> `NotAssessable` /
///   `P2LocalizationUncertain`;
/// * only a body that carries every plane occurrence -> `GranularityOnly`.
pub fn classify_body_region(alignment: &BodyContentAlignment) -> BodyRegionVerdict {
    let (verdict, reason) = if alignment.oracle_units == 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::OracleEmpty,
        )
    } else if alignment.in_plane_not_in_body > 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::P2RegionAssignmentGap {
                units_in_plane_not_in_body: alignment.in_plane_not_in_body,
            },
        )
    } else if alignment.uncertain_from_body() > 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::P2LocalizationUncertain {
                present_in_token_units: alignment.uncertain_present_in_token_from_body,
                spanning_token_units: alignment.uncertain_spanning_tokens_from_body,
                character_material_units: alignment.uncertain_character_material_from_body,
            },
        )
    } else {
        (BodyCoverageVerdict::GranularityOnly, BodyGapReason::Matched)
    };
    BodyRegionVerdict { verdict, reason }
}

/// Project the plane half of an alignment into the standalone P1 evidence shape,
/// so the plane verdict is derived by the same function everywhere.
fn plane_localization_from_alignment(alignment: &BodyContentAlignment) -> BodyPlaneLocalization {
    BodyPlaneLocalization {
        method: alignment.method.clone(),
        oracle_units: alignment.oracle_units,
        exact_in_plane: alignment.in_native_plane,
        boundary_variant_from_plane: alignment.boundary_variant_from_plane,
        uncertain_present_in_token_from_plane: alignment.uncertain_present_in_token_from_plane,
        uncertain_spanning_tokens_from_plane: alignment.uncertain_spanning_tokens_from_plane,
        uncertain_character_material_from_plane: alignment.uncertain_character_material_from_plane,
        absent_from_plane: alignment.unresolved_from_plane,
    }
}

/// One document's scope-bound native-vs-oracle alignment and typed verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyCoverageGap {
    pub document_id: String,
    pub native: NativeBodyLengths,
    pub oracle: NativeBodyLengths,
    pub alignment: BodyContentAlignment,
    /// The P1 plane axis, published beside the body axis.
    pub plane_verdict: BodyPlaneVerdict,
    /// The P2 body-region axis, published beside the plane axis.
    pub body_verdict: BodyRegionVerdict,
    /// Descriptive share of oracle units found in the native body, in per-mille.
    /// It is recorded for inspection only and decides nothing.
    pub aligned_body_share_milli: Option<u32>,
    pub verdict: BodyCoverageVerdict,
    pub reason: BodyGapReason,
    pub d498_reopen_required: bool,
    pub oracle_generator_id: String,
    pub native_scope: BodyScope,
    pub oracle_scope: BodyScope,
    pub thresholds: String,
}

/// Classify one document's native body text against an independent oracle.
///
/// `native_plane_units` is every lexical unit the lane extracted; `oracle_units`
/// is the independent oracle's units. The verdict is derived from the occurrence
/// location, never from a length, ratio, or total:
///
/// * unmatched scope, a mismatched normalization rung, an empty oracle, or
///   occurrences not resolved in the body -> `NotAssessable` with a typed reason;
/// * occurrences absent from the whole plane -> `ContentLoss` (and
///   `d498_reopen_required`);
/// * occurrences present only as characters -> `NotAssessable`;
/// * every occurrence resolved in the body -> `GranularityOnly`.
#[allow(clippy::too_many_arguments)]
pub fn classify_body_coverage(
    document_id: impl Into<String>,
    native: &NativeBodyText,
    native_plane_units: &BodyUnitInventory,
    oracle: &BodyOracleCounts,
    oracle_units: &BodyUnitInventory,
    provenance: &BodyOracleProvenance,
    native_scope: &BodyScope,
    oracle_scope: &BodyScope,
) -> Result<BodyCoverageGap, NativeBodyTextError> {
    require_independent_body_oracle(provenance)?;
    if provenance.generator_id != oracle.generator_id {
        return Err(NativeBodyTextError::OracleNotIndependent);
    }
    if oracle_scope.side != BodyScopeSide::PreservedBodyProse {
        return Err(NativeBodyTextError::InvalidScope(
            "oracle scope must be PreservedBodyProse",
        ));
    }
    if native_scope.side == BodyScopeSide::PreservedBodyProse {
        return Err(NativeBodyTextError::InvalidScope(
            "native scope must be a native side",
        ));
    }
    // Join policy is declared on each scope and the assembled body records the
    // policy it actually applied. A scope that names another policy describes a
    // different text (`VerbatimTokenConcat` over a body joined with
    // `SingleSpaceBetweenWordTokens`), so it is refused below instead of
    // believed. The tokenization rung (normalization ladder + case policy) is
    // not optional either: tokenization depends on it, and every inventory
    // carries the rung it was built at, so an inventory tokenized at a rung
    // other than the one its scope declares is refused instead of being silently
    // compared at the wrong rung.
    if oracle_units.rung() != oracle_scope.rung() {
        return Err(NativeBodyTextError::InventoryRungMismatch { side: "oracle" });
    }
    if native.units().rung() != native_scope.rung() {
        return Err(NativeBodyTextError::InventoryRungMismatch {
            side: "native body",
        });
    }
    if native_plane_units.rung() != native_scope.rung() {
        return Err(NativeBodyTextError::InventoryRungMismatch {
            side: "native plane",
        });
    }
    let alignment = align_body_content(native.units(), native_plane_units, oracle_units);
    // Both axes are derived and published independently. The combined verdict is
    // the plane axis whenever it is not clean, because a loss or an uncertainty
    // there cannot be improved by the body axis; the body axis is still carried
    // in `body_verdict`, so a P1 uncertainty never disguises a P2 body miss.
    let plane_verdict = classify_body_plane(&plane_localization_from_alignment(&alignment));
    let body_verdict = classify_body_region(&alignment);
    let (verdict, reason) = if !scope_covers(native_scope, oracle_scope) {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::ScopeMismatch,
        )
    } else if native_scope.rung() != oracle_scope.rung() {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::NormalizationMismatch,
        )
    } else if native.join != native_scope.join_policy {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::ScopeJoinMismatch,
        )
    } else if plane_verdict.verdict != BodyCoverageVerdict::GranularityOnly {
        (plane_verdict.verdict, plane_verdict.reason.clone())
    } else {
        (body_verdict.verdict, body_verdict.reason.clone())
    };
    let aligned_body_share_milli = if alignment.oracle_units == 0 {
        None
    } else {
        Some(
            ((alignment.in_native_body as u64 * 1000) / alignment.oracle_units as u64)
                .min(u32::MAX as u64) as u32,
        )
    };
    Ok(BodyCoverageGap {
        document_id: document_id.into(),
        native: native.lengths,
        oracle: oracle.lengths,
        alignment,
        plane_verdict,
        body_verdict,
        aligned_body_share_milli,
        verdict,
        reason,
        d498_reopen_required: verdict == BodyCoverageVerdict::ContentLoss,
        oracle_generator_id: oracle.generator_id.clone(),
        native_scope: native_scope.clone(),
        oracle_scope: oracle_scope.clone(),
        thresholds: BODY_GAP_THRESHOLDS.to_owned(),
    })
}

/// One document's contribution to a corpus fold: measured or absent.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyDocumentOutcome {
    /// A body band was produced and classified.
    Measured(BodyCoverageGap),
    /// No body band could be produced; the typed reason is preserved.
    Absent { document_id: String, reason: String },
}

/// A declared document the fold could not contribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyAbsentDocument {
    pub document_id: String,
    pub reason: String,
}

/// The declared corpus and every outcome. Nothing is implicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyAggregateInput {
    pub corpus_id: String,
    pub declared_documents: Vec<String>,
    pub outcomes: Vec<BodyDocumentOutcome>,
}

/// Corpus-level fold carrying the complete declared population.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyCoverageAggregate {
    pub corpus_id: String,
    pub documents_declared: usize,
    pub documents_contributing: usize,
    pub documents_absent: usize,
    pub absent: Vec<BodyAbsentDocument>,
    pub native: NativeBodyLengths,
    pub oracle: NativeBodyLengths,
    pub alignment: BodyContentAlignment,
    /// The P1 plane axis for the folded population, published beside the body
    /// axis. A clean reading is withheld when the population is incomplete.
    pub plane_verdict: BodyPlaneVerdict,
    /// The P2 body-region axis for the folded population, published beside the
    /// plane axis. A clean reading is withheld when the population is incomplete.
    pub body_verdict: BodyRegionVerdict,
    pub aligned_body_share_milli: Option<u32>,
    pub verdict: BodyCoverageVerdict,
    pub d498_reopen_required: bool,
    /// True only when every declared document contributed.
    pub population_complete: bool,
    pub thresholds: String,
}

/// Fold per-document outcomes into one corpus aggregate.
///
/// A declared document with no outcome is recorded as `not_reported`; an
/// outcome outside the declared corpus is recorded as `undeclared_document`.
/// Neither is dropped. An incomplete population cannot produce
/// `GranularityOnly`: only a real `ContentLoss` survives it.
pub fn aggregate_body_coverage(input: &BodyAggregateInput) -> BodyCoverageAggregate {
    let mut native = NativeBodyLengths::zero();
    let mut oracle = NativeBodyLengths::zero();
    let mut alignment = BodyContentAlignment::empty();
    let mut absent: Vec<BodyAbsentDocument> = Vec::new();
    let mut measured: Vec<&BodyCoverageGap> = Vec::new();
    let mut matched = vec![false; input.declared_documents.len()];
    for outcome in &input.outcomes {
        match outcome {
            BodyDocumentOutcome::Measured(gap) => {
                if let Some(position) = input
                    .declared_documents
                    .iter()
                    .position(|id| id == &gap.document_id)
                {
                    matched[position] = true;
                    measured.push(gap);
                    native.add(&gap.native);
                    oracle.add(&gap.oracle);
                    alignment.oracle_units += gap.alignment.oracle_units;
                    alignment.in_native_plane += gap.alignment.in_native_plane;
                    alignment.in_native_body += gap.alignment.in_native_body;
                    alignment.boundary_variant_from_plane +=
                        gap.alignment.boundary_variant_from_plane;
                    alignment.boundary_variant_from_body +=
                        gap.alignment.boundary_variant_from_body;
                    alignment.uncertain_present_in_token_from_plane +=
                        gap.alignment.uncertain_present_in_token_from_plane;
                    alignment.uncertain_spanning_tokens_from_plane +=
                        gap.alignment.uncertain_spanning_tokens_from_plane;
                    alignment.uncertain_character_material_from_plane +=
                        gap.alignment.uncertain_character_material_from_plane;
                    alignment.uncertain_present_in_token_from_body +=
                        gap.alignment.uncertain_present_in_token_from_body;
                    alignment.uncertain_spanning_tokens_from_body +=
                        gap.alignment.uncertain_spanning_tokens_from_body;
                    alignment.uncertain_character_material_from_body +=
                        gap.alignment.uncertain_character_material_from_body;
                    alignment.unresolved_from_plane += gap.alignment.unresolved_from_plane;
                    alignment.unresolved_from_body += gap.alignment.unresolved_from_body;
                    alignment.in_plane_not_in_body += gap.alignment.in_plane_not_in_body;
                } else {
                    absent.push(BodyAbsentDocument {
                        document_id: gap.document_id.clone(),
                        reason: "undeclared_document".to_owned(),
                    });
                }
            }
            BodyDocumentOutcome::Absent {
                document_id,
                reason,
            } => {
                if let Some(position) = input
                    .declared_documents
                    .iter()
                    .position(|id| id == document_id)
                {
                    matched[position] = true;
                }
                absent.push(BodyAbsentDocument {
                    document_id: document_id.clone(),
                    reason: reason.clone(),
                });
            }
        }
    }
    for (position, document_id) in input.declared_documents.iter().enumerate() {
        if !matched[position] {
            absent.push(BodyAbsentDocument {
                document_id: document_id.clone(),
                reason: "not_reported".to_owned(),
            });
        }
    }
    let worst = measured
        .iter()
        .fold(None, |current, gap| fold_worst(current, gap.verdict));
    let documents_declared = input.declared_documents.len();
    let documents_absent = absent.len();
    let population_complete = documents_declared > 0 && documents_absent == 0;
    let verdict = match worst {
        None => BodyCoverageVerdict::NotAssessable,
        Some(BodyCoverageVerdict::GranularityOnly) if !population_complete => {
            BodyCoverageVerdict::NotAssessable
        }
        Some(verdict) => verdict,
    };
    let aligned_body_share_milli = if alignment.oracle_units == 0 {
        None
    } else {
        Some(
            ((alignment.in_native_body as u64 * 1000) / alignment.oracle_units as u64)
                .min(u32::MAX as u64) as u32,
        )
    };
    let plane_verdict = {
        let raw = classify_body_plane(&plane_localization_from_alignment(&alignment));
        let (verdict, reason) = withhold_clean_verdict(
            raw.verdict,
            raw.reason,
            population_complete,
            documents_absent,
        );
        BodyPlaneVerdict {
            verdict,
            reason,
            d498_reopen_required: verdict == BodyCoverageVerdict::ContentLoss,
        }
    };
    let body_verdict = {
        let raw = classify_body_region(&alignment);
        let (verdict, reason) = withhold_clean_verdict(
            raw.verdict,
            raw.reason,
            population_complete,
            documents_absent,
        );
        BodyRegionVerdict { verdict, reason }
    };
    BodyCoverageAggregate {
        corpus_id: input.corpus_id.clone(),
        documents_declared,
        documents_contributing: measured.len(),
        documents_absent,
        absent,
        native,
        oracle,
        alignment,
        plane_verdict,
        body_verdict,
        aligned_body_share_milli,
        verdict,
        d498_reopen_required: verdict == BodyCoverageVerdict::ContentLoss,
        population_complete,
        thresholds: BODY_GAP_THRESHOLDS.to_owned(),
    }
}

/// Withhold a clean axis reading when the declared population is incomplete, so
/// a partial corpus can never look like a pass on either axis.
fn withhold_clean_verdict(
    verdict: BodyCoverageVerdict,
    reason: BodyGapReason,
    population_complete: bool,
    documents_absent: usize,
) -> (BodyCoverageVerdict, BodyGapReason) {
    if verdict == BodyCoverageVerdict::GranularityOnly && !population_complete {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::PopulationIncomplete { documents_absent },
        )
    } else {
        (verdict, reason)
    }
}

fn fold_worst(
    current: Option<BodyCoverageVerdict>,
    next: BodyCoverageVerdict,
) -> Option<BodyCoverageVerdict> {
    Some(match current {
        None => next,
        Some(current) => worst_verdict(current, next),
    })
}

fn worst_verdict(left: BodyCoverageVerdict, right: BodyCoverageVerdict) -> BodyCoverageVerdict {
    match (left, right) {
        (BodyCoverageVerdict::ContentLoss, _) | (_, BodyCoverageVerdict::ContentLoss) => {
            BodyCoverageVerdict::ContentLoss
        }
        (BodyCoverageVerdict::NotAssessable, _) | (_, BodyCoverageVerdict::NotAssessable) => {
            BodyCoverageVerdict::NotAssessable
        }
        _ => BodyCoverageVerdict::GranularityOnly,
    }
}

fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn is_collapsible_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{0b}' | '\u{0c}')
        || matches!(
            ch,
            '\u{00a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
        )
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if is_collapsible_space(ch) {
            if !out.is_empty() {
                pending_space = true;
            }
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

fn normalize_zero_width_ligature(text: &str) -> String {
    let collapsed = collapse_whitespace(text);
    let mut out = String::with_capacity(collapsed.len());
    for ch in collapsed.chars() {
        match ch {
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{00ad}' => {}
            '\u{fb00}' => out.push_str("ff"),
            '\u{fb01}' => out.push_str("fi"),
            '\u{fb02}' => out.push_str("fl"),
            '\u{fb03}' => out.push_str("ffi"),
            '\u{fb04}' => out.push_str("ffl"),
            '\u{fb05}' | '\u{fb06}' => out.push_str("st"),
            other => out.push(other),
        }
    }
    out
}
