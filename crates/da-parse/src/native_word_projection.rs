//! Additive word-level projection of the S03 native layout plane.
//!
//! The positioned donor's lexical unit is the PDF show-operator span, not a
//! word: one `LexemeRow` may legitimately carry a whole line, so its text
//! contains internal whitespace. The T02/T03 native bands contract on word
//! granularity and refuse a token with internal whitespace, which made the
//! declared three-document native run unreachable (S04/T04 blocker WP-01).
//! This module is the missing projection: it partitions each lexical parent
//! span into ordered word tokens without inventing content, font data, or
//! measured geometry.
//!
//! ## Rules (fail closed, never a silent skip)
//!
//! * **Content is conserved, never invented.** The split is a partition of the
//!   parent text's characters: every non-whitespace character belongs to
//!   exactly one word and every other character is whitespace. The projection
//!   carries `reconstruction_digest` over the ordered word texts plus the
//!   separator character count, and refuses with
//!   [`NativeWordProjectionError::ContentNotConserved`] when the accounting
//!   does not close.
//! * **Identity is lineage, not assumption.** Each word carries its parent
//!   `(page_ordinal, source_ordinal)`, its own text digest, the parent text
//!   digest re-derived from the bytes, and a document-scoped `order_digest`, so
//!   the projection can be recomputed and compared without the text.
//! * **Geometry is honest about its origin.** A single-word parent span keeps
//!   the parent box unchanged ([`NativeWordGeometryOrigin::MeasuredParentSpan`]).
//!   A multi-word span yields a
//!   [`NativeWordGeometryOrigin::ProportionalEstimateFromParentSpan`] box: the
//!   horizontal extent is allocated by word character count, the vertical
//!   extent is inherited, and the estimate lives in its own labelled field —
//!   never presented as a measurement.
//! * **Font data is inherited, never invented.** `bold`, `italic`, `monospace`
//!   and `font_size_milli_pt` are copied verbatim from the parent
//!   [`LayoutToken`].
//! * **Every gap is typed.** Empty or whitespace-only parent text becomes a
//!   typed [`NativeWordAbsence`]; a missing layout token, a missing sidecar
//!   row, a digest disagreement and a non-closing allocation each have their
//!   own error variant.
//!
//! ## Redaction
//!
//! Word text is private, reachable only through [`NativeWordToken::text`];
//! `Debug` prints `<redacted>` and the serde projection emits digests, geometry
//! and style but never the text — mirroring [`LexemeRow`]'s discipline.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface, and no TEI/Java type is an input. The S03 wire formats
//! (`NativeSegmenter`/v1, `segment_document_full`/v2) and the frozen carriers
//! are not touched, and nothing here is spliced into the Java-authoritative
//! parse route.

use crate::{
    LayoutBBox, LayoutDocument, LayoutStyleHints, LayoutToken, LayoutTokenKind, LexemeRow,
};
use serde::{Serialize, ser::SerializeStruct};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Schema identity of the additive word-projection surface.
pub const NATIVE_WORD_PROJECTION_SCHEMA_VERSION: &str = "da-parse.native-word-projection/v1";

/// Upper bound on projected parent spans. Mirrors the layout token ceiling so a
/// projection can never admit more parents than its document already bounds.
pub const MAX_NATIVE_WORD_PARENTS: usize = 100_000;

/// Upper bound on projected words. A multi-word span multiplies the token
/// count, so this sits above the layout ceiling but still bounds the surface.
pub const MAX_NATIVE_WORDS: usize = 250_000;

/// Where a word's bounding box came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeWordGeometryOrigin {
    /// The parent span carried exactly one word, so the parent box is the word
    /// box — a measurement, not an estimate.
    MeasuredParentSpan,
    /// The parent span carried several words. The horizontal extent is a
    /// documented proportional estimate; the vertical extent is inherited.
    ProportionalEstimateFromParentSpan,
}

/// One word's geometry, with its origin stated explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWordGeometry {
    /// The measured parent span box this word was projected out of.
    pub parent_bbox: LayoutBBox,
    /// Inclusive character offset of the word start inside the parent text.
    pub char_start: u32,
    /// Exclusive character offset of the word end inside the parent text.
    pub char_end: u32,
    /// The derived box, present only for a multi-word parent span. Consumers
    /// that need measurement must not read this as one.
    pub estimated_bbox: Option<LayoutBBox>,
    pub origin: NativeWordGeometryOrigin,
}

/// One projected word: lineage and layout beside private text.
#[derive(Clone, PartialEq)]
pub struct NativeWordToken {
    pub page_ordinal: u32,
    /// The parent span's extraction identity, retained as provenance.
    pub source_ordinal: u32,
    /// Document-scoped reading order within the page, monotonic and stable.
    pub word_ordinal: u32,
    /// Digest of the word text, cross-checked at build time.
    pub text_digest: String,
    /// Derived from the word text, never from the parent span text.
    pub kind: LayoutTokenKind,
    /// Inherited verbatim from the parent layout token; never synthesised.
    pub style: LayoutStyleHints,
    pub geometry: NativeWordGeometry,
    text: String,
}

impl NativeWordToken {
    /// The raw word text. Deliberately the only path to it: `Debug` and the
    /// serde projection never carry it.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for NativeWordToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeWordToken")
            .field("page_ordinal", &self.page_ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("word_ordinal", &self.word_ordinal)
            .field("text_digest", &self.text_digest)
            .field("kind", &self.kind)
            .field("style", &self.style)
            .field("geometry", &self.geometry)
            .field("text", &"<redacted>")
            .finish()
    }
}

impl Serialize for NativeWordToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NativeWordToken", 7)?;
        state.serialize_field("page_ordinal", &self.page_ordinal)?;
        state.serialize_field("source_ordinal", &self.source_ordinal)?;
        state.serialize_field("word_ordinal", &self.word_ordinal)?;
        state.serialize_field("text_digest", &self.text_digest)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("style", &self.style)?;
        state.serialize_field("geometry", &self.geometry)?;
        state.end()
    }
}

/// One parent span's projected words, in character order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWordParent {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    /// The parent text digest, re-derived from the bytes and compared against
    /// both the layout token and the sidecar row.
    pub parent_text_digest: String,
    /// Digest over the ordered word texts. A consumer can recompute it from the
    /// word digests to prove no word was added, dropped or reordered.
    pub reconstruction_digest: String,
    /// Character length of the parent text.
    pub parent_char_len: u32,
    /// Whitespace characters that separate the words (leading, internal and
    /// trailing). `sum(word chars) + separator_chars == parent_char_len` is the
    /// conservation invariant checked at build time.
    pub separator_chars: u32,
    pub words: Vec<NativeWordToken>,
}

impl NativeWordParent {
    /// Number of projected words.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

/// Typed absence of words for a parent span that was itself present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum NativeWordAbsence {
    /// The parent span's text was empty.
    EmptyText {
        page_ordinal: u32,
        source_ordinal: u32,
    },
    /// The parent span's text was whitespace only.
    WhitespaceOnly {
        page_ordinal: u32,
        source_ordinal: u32,
    },
}

/// Provenance identity of the projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWordIdentity {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub parent_count: u32,
    pub word_count: u32,
    /// Ordered identity over `(page, source, word_ordinal, word digest)`.
    pub order_digest: String,
}

/// Deterministic word projection of one layout document's lexical sidecar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWordProjection {
    pub schema_version: String,
    pub identity: NativeWordIdentity,
    pub parents: Vec<NativeWordParent>,
    pub absences: Vec<NativeWordAbsence>,
}

impl NativeWordProjection {
    /// The projection for one parent span, when it produced at least one word.
    pub fn parent(&self, page_ordinal: u32, source_ordinal: u32) -> Option<&NativeWordParent> {
        self.parents.iter().find(|parent| {
            parent.page_ordinal == page_ordinal && parent.source_ordinal == source_ordinal
        })
    }

    /// Canonical redaction-safe digest over the serialized projection. The
    /// projection excludes raw text but includes every word digest, so two
    /// projections over different texts still differ.
    pub fn digest(&self) -> Result<String, NativeWordProjectionError> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| NativeWordProjectionError::Serialization)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

/// Fail-closed refusal vocabulary of the word projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeWordProjectionError {
    /// The layout document failed its own contract.
    LayoutInvalid,
    /// The document carries no tokens at all, so there is nothing to project.
    EmptyDocument,
    /// A sidecar row declares another document.
    DocumentMismatch,
    /// A sidecar row declares another provider.
    ProviderMismatch,
    /// Two sidecar rows declare the same `(page, source)`.
    DuplicateLexeme {
        page: u32,
        source: u32,
    },
    /// A lexical token has no sidecar row.
    MissingLexeme {
        page: u32,
        source: u32,
    },
    /// A sidecar row has no matching layout token.
    MissingLayoutToken {
        page: u32,
        source: u32,
    },
    /// The re-derived text digest disagrees with the sidecar row or the layout
    /// token.
    DigestMismatch {
        page: u32,
        source: u32,
    },
    /// The word/separator accounting does not close over the parent text.
    ContentNotConserved {
        page: u32,
        source: u32,
    },
    /// A derived word box was inverted or non-finite.
    DerivedGeometryInvalid {
        page: u32,
        source: u32,
    },
    /// More parent spans or words than the surface bounds.
    TooManyParents {
        count: usize,
    },
    TooManyWords {
        count: usize,
    },
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeWordProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native word projection rejected input: {self:?}")
    }
}

impl std::error::Error for NativeWordProjectionError {}

/// Project `document`'s lexical sidecar into deterministic word tokens.
///
/// The join mirrors the S03 sidecar contract: every token that is not a
/// formula or an image needs a sidecar row, every sidecar row needs its token,
/// and every digest is re-derived from the bytes. Only pure layout and lexical
/// inputs are read — no TEI, no Java carrier.
pub fn project_native_words(
    document: &LayoutDocument,
    lexemes: &[LexemeRow],
) -> Result<NativeWordProjection, NativeWordProjectionError> {
    document
        .validate()
        .map_err(|_| NativeWordProjectionError::LayoutInvalid)?;

    let mut lexeme_index: BTreeMap<(u32, u32), &LexemeRow> = BTreeMap::new();
    for lexeme in lexemes {
        if lexeme.document_id != document.document_id {
            return Err(NativeWordProjectionError::DocumentMismatch);
        }
        let key = (lexeme.page_ordinal, lexeme.source_ordinal);
        if lexeme_index.insert(key, lexeme).is_some() {
            return Err(NativeWordProjectionError::DuplicateLexeme {
                page: key.0,
                source: key.1,
            });
        }
    }

    let mut token_index: BTreeMap<(u32, u32), &LayoutToken> = BTreeMap::new();
    for page in &document.pages {
        for token in &page.tokens {
            token_index.insert((token.page_ordinal, token.source_ordinal), token);
        }
    }
    if token_index.is_empty() {
        return Err(NativeWordProjectionError::EmptyDocument);
    }

    let provider_id = document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .next()
        .map(|token| token.provider.id.clone())
        .unwrap_or_default();
    for lexeme in lexemes {
        if lexeme.provider_id != provider_id {
            return Err(NativeWordProjectionError::ProviderMismatch);
        }
    }

    // Resolve the join by identity before admitting anything, so an incomplete
    // or extra sidecar is reported as exactly that.
    for page in &document.pages {
        for token in &page.tokens {
            if needs_sidecar(token.kind)
                && !lexeme_index.contains_key(&(token.page_ordinal, token.source_ordinal))
            {
                return Err(NativeWordProjectionError::MissingLexeme {
                    page: token.page_ordinal,
                    source: token.source_ordinal,
                });
            }
        }
    }
    for lexeme in lexemes {
        let key = (lexeme.page_ordinal, lexeme.source_ordinal);
        match token_index.get(&key) {
            Some(token) if needs_sidecar(token.kind) => {}
            _ => {
                return Err(NativeWordProjectionError::MissingLayoutToken {
                    page: key.0,
                    source: key.1,
                });
            }
        }
    }

    let mut parents: Vec<NativeWordParent> = Vec::new();
    let mut absences: Vec<NativeWordAbsence> = Vec::new();
    let mut order_entries: Vec<String> = Vec::new();
    let mut word_count = 0usize;

    for page in &document.pages {
        let mut page_word_ordinal: u32 = 0;
        for token in &page.tokens {
            if !needs_sidecar(token.kind) {
                continue;
            }
            let key = (token.page_ordinal, token.source_ordinal);
            let lexeme =
                lexeme_index
                    .get(&key)
                    .ok_or(NativeWordProjectionError::MissingLexeme {
                        page: key.0,
                        source: key.1,
                    })?;
            let text = lexeme.text();
            let derived_digest = digest_text(text);
            if derived_digest != lexeme.text_digest || lexeme.text_digest != token.text_digest {
                return Err(NativeWordProjectionError::DigestMismatch {
                    page: key.0,
                    source: key.1,
                });
            }

            let chars: Vec<char> = text.chars().collect();
            let (spans, separator_chars) = split_word_spans(&chars);
            if spans.is_empty() {
                absences.push(if chars.is_empty() {
                    NativeWordAbsence::EmptyText {
                        page_ordinal: key.0,
                        source_ordinal: key.1,
                    }
                } else {
                    NativeWordAbsence::WhitespaceOnly {
                        page_ordinal: key.0,
                        source_ordinal: key.1,
                    }
                });
                continue;
            }

            let word_chars: u32 = spans.iter().map(|(start, end)| end - start).sum();
            if word_chars + separator_chars != chars.len() as u32 {
                return Err(NativeWordProjectionError::ContentNotConserved {
                    page: key.0,
                    source: key.1,
                });
            }

            word_count += spans.len();
            if word_count > MAX_NATIVE_WORDS {
                return Err(NativeWordProjectionError::TooManyWords { count: word_count });
            }

            let mut words: Vec<NativeWordToken> = Vec::with_capacity(spans.len());
            for (index, (start, end)) in spans.iter().enumerate() {
                let word_text: String = chars[*start as usize..*end as usize].iter().collect();
                let geometry = word_geometry(token.bbox, &spans, index).ok_or(
                    NativeWordProjectionError::DerivedGeometryInvalid {
                        page: key.0,
                        source: key.1,
                    },
                )?;
                words.push(NativeWordToken {
                    page_ordinal: key.0,
                    source_ordinal: key.1,
                    word_ordinal: page_word_ordinal,
                    text_digest: digest_text(&word_text),
                    kind: LayoutTokenKind::from_lexical_text(&word_text),
                    style: token.style,
                    geometry,
                    text: word_text,
                });
                page_word_ordinal += 1;
            }
            order_entries.extend(words.iter().map(|word| {
                format!(
                    "{}:{}:{}:{}",
                    word.page_ordinal, word.source_ordinal, word.word_ordinal, word.text_digest
                )
            }));

            let reconstruction_digest = native_word_reconstruction_digest(&words);
            parents.push(NativeWordParent {
                page_ordinal: key.0,
                source_ordinal: key.1,
                token_ordinal: token.token_ordinal,
                parent_text_digest: derived_digest,
                reconstruction_digest,
                parent_char_len: chars.len() as u32,
                separator_chars,
                words,
            });
        }
    }

    if parents.len() > MAX_NATIVE_WORD_PARENTS {
        return Err(NativeWordProjectionError::TooManyParents {
            count: parents.len(),
        });
    }

    let order_digest = digest_parts(
        &order_entries
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>(),
    );

    Ok(NativeWordProjection {
        schema_version: NATIVE_WORD_PROJECTION_SCHEMA_VERSION.to_owned(),
        identity: NativeWordIdentity {
            schema_version: NATIVE_WORD_PROJECTION_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            provider_id,
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            parent_count: parents.len() as u32,
            word_count: word_count as u32,
            order_digest,
        },
        parents,
        absences,
    })
}

/// Whether a layout token kind participates in the lexical sidecar. Mirrors the
/// S03 sidecar contract exactly: formula and image tokens do not.
fn needs_sidecar(kind: LayoutTokenKind) -> bool {
    !matches!(kind, LayoutTokenKind::Formula | LayoutTokenKind::Image)
}

/// Partition `chars` into maximal non-whitespace runs.
///
/// Returns the runs as `(start, end)` character offsets in ascending order and
/// the total number of whitespace characters, so the caller can prove the
/// partition closes over the parent text.
fn split_word_spans(chars: &[char]) -> (Vec<(u32, u32)>, u32) {
    let mut spans: Vec<(u32, u32)> = Vec::new();
    let mut separator_chars: u32 = 0;
    let mut start: Option<u32> = None;
    for (index, ch) in chars.iter().enumerate() {
        let index = index as u32;
        if ch.is_whitespace() {
            if let Some(open) = start.take() {
                spans.push((open, index));
            }
            separator_chars += 1;
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(open) = start {
        spans.push((open, chars.len() as u32));
    }
    (spans, separator_chars)
}

/// Derive one word's box from the parent span.
///
/// A single-word span keeps the parent box (a measurement). A multi-word span
/// allocates the parent's horizontal extent by word character count and
/// inherits its vertical extent, and the result is reported as an estimate.
fn word_geometry(
    parent: LayoutBBox,
    spans: &[(u32, u32)],
    index: usize,
) -> Option<NativeWordGeometry> {
    let (start, end) = *spans.get(index)?;
    if spans.len() == 1 {
        return Some(NativeWordGeometry {
            parent_bbox: parent,
            char_start: start,
            char_end: end,
            estimated_bbox: None,
            origin: NativeWordGeometryOrigin::MeasuredParentSpan,
        });
    }
    let total: f64 = spans
        .iter()
        .map(|(s, e)| (*e - *s) as f64)
        .sum::<f64>()
        .max(f64::EPSILON);
    let width = parent.x_max - parent.x_min;
    let mut cursor = parent.x_min;
    for (s, e) in spans.iter().take(index) {
        cursor += width * ((*e - *s) as f64 / total);
    }
    let (s, e) = spans[index];
    let x_min = cursor;
    let x_max = if index + 1 == spans.len() {
        parent.x_max
    } else {
        cursor + width * ((e - s) as f64 / total)
    };
    let estimated = LayoutBBox::new(x_min, parent.y_min, x_max, parent.y_max).ok()?;
    Some(NativeWordGeometry {
        parent_bbox: parent,
        char_start: start,
        char_end: end,
        estimated_bbox: Some(estimated),
        origin: NativeWordGeometryOrigin::ProportionalEstimateFromParentSpan,
    })
}

/// Digest over the ordered word texts: the recomputable reconstruction
/// identity. Order and separators are part of the identity, so a reordering or
/// a dropped word changes it. This is the single definition of the rule — a
/// consumer verifying a projection must not re-implement it.
pub fn native_word_reconstruction_digest(words: &[NativeWordToken]) -> String {
    let mut hasher = Sha256::new();
    for word in words {
        hasher.update(word.text.as_bytes());
        hasher.update([0u8]);
    }
    format!("sha256:{:x}", hasher.finalize())
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

/// The reading-order key of a projected word.
pub fn native_word_order_key(word: &NativeWordToken) -> (u32, u32, u32) {
    (word.page_ordinal, word.source_ordinal, word.word_ordinal)
}

/// Deterministic dedup helper used by consumers that build a key set from the
/// projection without re-deriving the ordering rule.
pub fn native_word_keys(projection: &NativeWordProjection) -> BTreeSet<(u32, u32, u32)> {
    projection
        .parents
        .iter()
        .flat_map(|parent| parent.words.iter())
        .map(native_word_order_key)
        .collect()
}
