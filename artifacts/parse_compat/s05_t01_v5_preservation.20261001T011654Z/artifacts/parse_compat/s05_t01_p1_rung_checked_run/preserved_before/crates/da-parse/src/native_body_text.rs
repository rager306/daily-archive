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
//! another. The pure [`align_body_content`] and [`localize_body_plane`] helpers
//! carry the same precondition and do not re-check it.
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
//!   loss and never silently cleared.
//! * [`BodyCoverageVerdict::GranularityOnly`] is emitted only when every oracle
//!   occurrence is located exactly or as a tile. A surplus, an equal total, or a
//!   total word count can never produce it.
//! * An unmatched scope, a mismatched normalization rung, an empty oracle, or a
//!   declared population with absent documents stays `NotAssessable` with a typed
//!   reason.
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
//!   [`NativeBodyText::from_joined_text`] and records the policy on its scope; a
//!   caller that knows the genuine token boundaries passes them one fragment per
//!   token to [`NativeBodyText::from_fragments`] so a tile can never be invented
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

use crate::{
    FullSemanticRegion, LayoutDocument, LayoutLattice, LexemeRow,
    NATIVE_REGION_INPUT_SCHEMA_VERSION, NativeRegionAbsence, NativeRegionCarrier,
    NativeRegionError, build_native_region_carrier,
};
use serde::{Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
            text,
            units,
        )
    }

    /// Admit an already-joined body text whose join policy the caller declares
    /// on its [`BodyScope`]. The caller owns the join rule; this constructor
    /// only refuses an empty or oversized text.
    ///
    /// The whole text becomes **one** fragment. A caller that holds the genuine
    /// token boundaries must use [`NativeBodyText::from_fragments`] instead, so
    /// that a boundary tile is never invented across a boundary that exists.
    pub fn from_joined_text(
        document_id: impl Into<String>,
        source_pdf_sha256: impl Into<String>,
        token_count: usize,
        page_span: Option<(u32, u32)>,
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
            text,
            units,
        )
    }

    fn assemble(
        document_id: String,
        source_pdf_sha256: String,
        token_count: usize,
        page_span: Option<(u32, u32)>,
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
        let mut state = serializer.serialize_struct("NativeBodyText", 8)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("region", &self.region)?;
        state.serialize_field("document_id", &self.document_id)?;
        state.serialize_field("source_pdf_sha256", &self.source_pdf_sha256)?;
        state.serialize_field("token_count", &self.token_count)?;
        state.serialize_field("page_span", &self.page_span)?;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SideLocalization {
    exact: usize,
    boundary: usize,
    uncertain_present_in_token: usize,
    uncertain_spanning_tokens: usize,
    uncertain_character_material: usize,
    absent: usize,
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
    /// Oracle occurrences not resolved (exactly or by tile) in the body.
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
/// tokenized at the same rung; [`classify_body_coverage`] is the enforced path.
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

/// Compute the ordered, occurrence-conserving plane localization (the P1 shape).
/// Both inventories must be tokenized at the same rung; this pure helper does
/// not re-check that, so [`classify_body_coverage`] is the checking entry point.
pub fn localize_body_plane(
    native_plane_units: &BodyUnitInventory,
    oracle_units: &BodyUnitInventory,
) -> BodyPlaneLocalization {
    let located = ordered_side_localization(native_plane_units, oracle_units);
    BodyPlaneLocalization {
        method: BODY_ALIGNMENT_METHOD.to_owned(),
        oracle_units: oracle_units.total,
        exact_in_plane: located.exact,
        boundary_variant_from_plane: located.boundary,
        uncertain_present_in_token_from_plane: located.uncertain_present_in_token,
        uncertain_spanning_tokens_from_plane: located.uncertain_spanning_tokens,
        uncertain_character_material_from_plane: located.uncertain_character_material,
        absent_from_plane: located.absent,
    }
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
    let plane = ordered_side_localization(native_plane_units, oracle_units);
    let body = ordered_side_localization(native_body_units, oracle_units);
    BodyContentAlignment {
        method: BODY_ALIGNMENT_METHOD.to_owned(),
        oracle_units: oracle_units.total,
        in_native_plane: plane.exact,
        in_native_body: body.exact,
        boundary_variant_from_plane: plane.boundary,
        boundary_variant_from_body: body.boundary,
        uncertain_present_in_token_from_plane: plane.uncertain_present_in_token,
        uncertain_spanning_tokens_from_plane: plane.uncertain_spanning_tokens,
        uncertain_character_material_from_plane: plane.uncertain_character_material,
        uncertain_present_in_token_from_body: body.uncertain_present_in_token,
        uncertain_spanning_tokens_from_body: body.uncertain_spanning_tokens,
        uncertain_character_material_from_body: body.uncertain_character_material,
        unresolved_from_plane: plane.absent,
        unresolved_from_body: body.absent,
        in_plane_not_in_body: oracle_units
            .total
            .saturating_sub(body.exact)
            .saturating_sub(body.boundary),
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

/// The ordered, occurrence-conserving localization of every oracle occurrence
/// against one side. Exact occurrences are the optimal earliest-match monotone
/// subsequence of the side; the residual gaps are tiled only by whole
/// token-boundary runs. The exact phase runs to completion before any tile is
/// considered, so the locator is a **two-phase greedy decomposition, not a joint
/// optimum**: oracle `x y` against side `x xy` anchors `x` and then leaves `y`
/// unresolved even though tiling the oracle run `x y` onto the side token `xy`
/// would resolve both. The conservative reading (uncertainty, never a fabricated
/// tile) is the intended outcome. Every oracle occurrence is classified exactly
/// once and every side occurrence is consumed at most once, so a single side
/// token can never excuse unbounded residual occurrences.
fn ordered_side_localization(
    side: &BodyUnitInventory,
    oracle: &BodyUnitInventory,
) -> SideLocalization {
    let mut localization = SideLocalization::default();
    if oracle.total == 0 {
        return localization;
    }
    let oracle_tokens = flatten(&oracle.fragments);
    let side_tokens = flatten(&side.fragments);
    let mut class = vec![OccurrenceClass::Pending; oracle_tokens.len()];
    if !side_tokens.is_empty() {
        let mut anchor_side = vec![None; oracle_tokens.len()];
        exact_anchors(&oracle_tokens, &side_tokens, &mut class, &mut anchor_side);
        tile_gaps(&oracle_tokens, &side_tokens, &mut class, &anchor_side);
    }
    for (index, (_, token)) in oracle_tokens.iter().enumerate() {
        match class[index] {
            OccurrenceClass::Exact => localization.exact += 1,
            OccurrenceClass::Boundary => localization.boundary += 1,
            OccurrenceClass::Pending => classify_unresolved(side, token, &mut localization),
        }
    }
    debug_assert_eq!(
        localization.exact
            + localization.boundary
            + localization.uncertain_present_in_token
            + localization.uncertain_spanning_tokens
            + localization.uncertain_character_material
            + localization.absent,
        oracle.total,
        "every oracle occurrence must land in exactly one class"
    );
    localization
}

/// Classify one unresolved occurrence by what the side still proves: characters
/// inside a token, characters spanning tokens, characters present as material,
/// or no character material at all.
fn classify_unresolved(side: &BodyUnitInventory, unit: &str, localization: &mut SideLocalization) {
    if side.contained_in_one_token(unit) {
        localization.uncertain_present_in_token += 1;
    } else if side.contained_in_character_run(unit) {
        localization.uncertain_spanning_tokens += 1;
    } else if side.ascii_material_present(unit) {
        localization.uncertain_character_material += 1;
    } else {
        localization.absent += 1;
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
    /// The unresolvable oracle occurrences are in the native plane but not in the
    /// body region: a P2 region-assignment gap, not text loss.
    P2RegionAssignmentGap { units_in_plane_not_in_body: usize },
    /// Body occurrences are present as characters but not localized there.
    P2LocalizationUncertain {
        present_in_token_units: usize,
        spanning_token_units: usize,
        character_material_units: usize,
    },
    /// The native scope does not cover the oracle scope; no claim is possible.
    ScopeMismatch,
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

/// One document's scope-bound native-vs-oracle alignment and typed verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BodyCoverageGap {
    pub document_id: String,
    pub native: NativeBodyLengths,
    pub oracle: NativeBodyLengths,
    pub alignment: BodyContentAlignment,
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
    // Join policy is recorded on each scope; the unit alignment is itself
    // join-policy independent, so a differing policy is not fatal. The
    // tokenization rung (normalization ladder + case policy) is not optional:
    // tokenization depends on it, and every inventory carries the rung it was
    // built at, so an inventory tokenized at a rung other than the one its scope
    // declares is refused instead of being silently compared at the wrong rung.
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
    } else if alignment.oracle_units == 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::OracleEmpty,
        )
    } else if alignment.unresolved_from_plane > 0 {
        (
            BodyCoverageVerdict::ContentLoss,
            BodyGapReason::P1NativeTextAbsent {
                missing_units: alignment.unresolved_from_plane,
            },
        )
    } else if alignment.uncertain_from_plane() > 0 {
        (
            BodyCoverageVerdict::NotAssessable,
            BodyGapReason::P1LocalizationUncertain {
                present_in_token_units: alignment.uncertain_present_in_token_from_plane,
                spanning_token_units: alignment.uncertain_spanning_tokens_from_plane,
                character_material_units: alignment.uncertain_character_material_from_plane,
            },
        )
    } else if alignment.unresolved_from_body > 0 {
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
    BodyCoverageAggregate {
        corpus_id: input.corpus_id.clone(),
        documents_declared,
        documents_contributing: measured.len(),
        documents_absent,
        absent,
        native,
        oracle,
        alignment,
        aligned_body_share_milli,
        verdict,
        d498_reopen_required: verdict == BodyCoverageVerdict::ContentLoss,
        population_complete,
        thresholds: BODY_GAP_THRESHOLDS.to_owned(),
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
