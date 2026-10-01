//! Pure, bounded native document segmentation for shadow evaluation.
//!
//! The lexical sidecar is invocation-local: it is never serialized, logged, or
//! attached to the provider-neutral layout schema.  This module has no IO,
//! runtime, parser, or persistence dependencies.

use crate::{
    FurnitureEvidence, FurnitureKind, LayoutDocument, LayoutLattice, LayoutToken, LayoutTokenKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const NATIVE_SEGMENTATION_SCHEMA_VERSION: &str = "da-parse.native-segmentation/v1";
pub const MAX_PAGES: usize = 10_000;
pub const MAX_TOKENS: usize = 100_000;
pub const MAX_LEXEME_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_REGIONS: usize = 5;
pub const MAX_SECTIONS: usize = 16_384;
pub const MAX_SECTION_DEPTH: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub struct LexemeRow {
    pub document_id: String,
    pub provider_id: String,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub text_digest: String,
    text: String,
}
impl LexemeRow {
    pub fn new(
        document_id: impl Into<String>,
        provider_id: impl Into<String>,
        page_ordinal: u32,
        source_ordinal: u32,
        text_digest: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            document_id: document_id.into(),
            provider_id: provider_id.into(),
            page_ordinal,
            source_ordinal,
            text_digest: text_digest.into(),
            text: text.into(),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl fmt::Debug for LexemeRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LexemeRow")
            .field("document_id", &self.document_id)
            .field("provider_id", &self.provider_id)
            .field("page_ordinal", &self.page_ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("text_digest", &self.text_digest)
            .field("text", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentationLimits {
    pub max_pages: usize,
    pub max_tokens: usize,
    pub max_lexeme_bytes: usize,
    pub max_regions: usize,
    pub max_sections: usize,
    pub max_depth: usize,
}
impl Default for SegmentationLimits {
    fn default() -> Self {
        Self {
            max_pages: MAX_PAGES,
            max_tokens: MAX_TOKENS,
            max_lexeme_bytes: MAX_LEXEME_BYTES,
            max_regions: MAX_REGIONS,
            max_sections: MAX_SECTIONS,
            max_depth: MAX_SECTION_DEPTH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SegmentationConfig {
    pub limits: SegmentationLimits,
    pub margin_fraction: f64,
    pub heading_min_font_size_milli_pt: u32,
    pub furniture_min_page_fraction: f64,
}
impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            limits: SegmentationLimits::default(),
            margin_fraction: 0.08,
            heading_min_font_size_milli_pt: 11000,
            furniture_min_page_fraction: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRegion {
    Header,
    Body,
    References,
    Annex,
    PageFurniture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAssignment {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub text_digest: String,
    pub region: DocumentRegion,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionReceipt {
    pub section_id: String,
    pub parent_region: DocumentRegion,
    pub order_ordinal: usize,
    pub depth: usize,
    pub parent_section_id: Option<String>,
    pub first_token_ordinal: u32,
    pub boundary_digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentationOutcome {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub assignments: Vec<TokenAssignment>,
    pub sections: Vec<SectionReceipt>,
    pub boundary_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentationError {
    InvalidConfig,
    LayoutInvalid,
    TooManyPages { count: usize },
    TooManyTokens { count: usize },
    TooManyLexemeBytes { count: usize },
    TooManyRegions { count: usize },
    TooManySections { count: usize },
    SectionDepthExceeded { depth: usize },
    MissingLexeme { page: u32, source: u32 },
    DuplicateLexeme { page: u32, source: u32 },
    DocumentMismatch,
    ProviderMismatch,
    DigestMismatch { page: u32, source: u32 },
    MalformedOrder { page: u32, source: u32 },
    InvalidLexeme,
}
impl fmt::Display for SegmentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native segmentation rejected input: {self:?}")
    }
}
impl std::error::Error for SegmentationError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSegmenter;

impl NativeSegmenter {
    pub const fn new() -> Self {
        Self
    }
    pub fn segment(
        &self,
        document: &LayoutDocument,
        lexemes: &[LexemeRow],
        config: SegmentationConfig,
    ) -> Result<SegmentationOutcome, SegmentationError> {
        validate_config(&config)?;
        document
            .validate()
            .map_err(|_| SegmentationError::LayoutInvalid)?;
        let token_count = document.pages.iter().map(|p| p.tokens.len()).sum::<usize>();
        if document.pages.len() > config.limits.max_pages {
            return Err(SegmentationError::TooManyPages {
                count: document.pages.len(),
            });
        }
        if token_count > config.limits.max_tokens {
            return Err(SegmentationError::TooManyTokens { count: token_count });
        }
        let bytes = lexemes.iter().map(|l| l.text.len()).sum::<usize>();
        if bytes > config.limits.max_lexeme_bytes {
            return Err(SegmentationError::TooManyLexemeBytes { count: bytes });
        }
        let provider = document
            .pages
            .iter()
            .flat_map(|p| p.tokens.iter())
            .next()
            .map(|t| t.provider.id.clone())
            .unwrap_or_default();
        validate_sidecar(document, lexemes, &provider)?;
        let furniture = furniture_keys(document, lexemes, &config);
        let mut assignments = Vec::with_capacity(token_count);
        let mut state = RegionState {
            region: DocumentRegion::Header,
        };
        let mut seen_content = false;
        let mut sections = Vec::new();
        let mut section_stack: Vec<(usize, String)> = Vec::new();
        let mut section_region = None;
        for page in &document.pages {
            for token in &page.tokens {
                let lexeme = lexemes.iter().find(|l| {
                    l.page_ordinal == token.page_ordinal && l.source_ordinal == token.source_ordinal
                });
                let text = lexeme.map(LexemeRow::text).unwrap_or("");
                let region = if furniture.contains(&(token.page_ordinal, token.source_ordinal)) {
                    DocumentRegion::PageFurniture
                } else {
                    let marker = marker(text);
                    state.advance(marker, token.page_ordinal == 0, seen_content);
                    if !matches!(token.kind, LayoutTokenKind::Whitespace) {
                        seen_content = true;
                    }
                    state.region()
                };
                assignments.push(TokenAssignment {
                    page_ordinal: token.page_ordinal,
                    source_ordinal: token.source_ordinal,
                    token_ordinal: token.token_ordinal,
                    text_digest: token.text_digest.clone(),
                    region,
                });
                if matches!(region, DocumentRegion::Body | DocumentRegion::Annex)
                    && is_heading(token, text, config.heading_min_font_size_milli_pt)
                {
                    if section_region != Some(region) {
                        section_stack.clear();
                        section_region = Some(region);
                    }
                    let depth = heading_depth(text).max(1);
                    if depth > config.limits.max_depth {
                        return Err(SegmentationError::SectionDepthExceeded { depth });
                    }
                    while section_stack.len() >= depth {
                        section_stack.pop();
                    }
                    let parent = region;
                    let parent_id = section_stack.last().map(|(_, id)| id.clone());
                    let id = format!(
                        "{}-{}-{}",
                        document.document_id,
                        region_name(parent),
                        sections.len()
                    );
                    let digest = digest_parts(&[&id, text, &token.text_digest]);
                    sections.push(SectionReceipt {
                        section_id: id.clone(),
                        parent_region: parent,
                        order_ordinal: sections.len(),
                        depth,
                        parent_section_id: parent_id,
                        first_token_ordinal: token.token_ordinal,
                        boundary_digest: digest,
                    });
                    section_stack.push((depth, id));
                    if sections.len() > config.limits.max_sections {
                        return Err(SegmentationError::TooManySections {
                            count: sections.len(),
                        });
                    }
                }
            }
        }
        if !assignments.is_empty() {
            assignments.windows(2).try_for_each(|w| {
                if w[0].page_ordinal > w[1].page_ordinal
                    || (w[0].page_ordinal == w[1].page_ordinal
                        && w[0].token_ordinal >= w[1].token_ordinal)
                {
                    Err(SegmentationError::MalformedOrder {
                        page: w[1].page_ordinal,
                        source: w[1].source_ordinal,
                    })
                } else {
                    Ok(())
                }
            })?;
        }
        let boundary_digest = digest_parts(
            &assignments
                .iter()
                .map(|a| {
                    format!(
                        "{}:{}:{}",
                        a.page_ordinal,
                        a.token_ordinal,
                        region_name(a.region)
                    )
                })
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        Ok(SegmentationOutcome {
            schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            provider_id: provider,
            assignments,
            sections,
            boundary_digest,
        })
    }
}

#[derive(Clone, Copy)]
enum Marker {
    References,
    Annex,
    BodyStart,
    None,
}
struct RegionState {
    region: DocumentRegion,
}
impl RegionState {
    fn advance(&mut self, marker: Marker, first_page: bool, _content: bool) {
        match marker {
            Marker::References => self.region = DocumentRegion::References,
            Marker::Annex => self.region = DocumentRegion::Annex,
            Marker::BodyStart if matches!(self.region, DocumentRegion::Header) => {
                self.region = DocumentRegion::Body
            }
            Marker::None if first_page && matches!(self.region, DocumentRegion::Header) => {}
            _ => {}
        }
    }
    fn region(&self) -> DocumentRegion {
        self.region
    }
}
fn marker(text: &str) -> Marker {
    let t = text.trim().to_ascii_lowercase();
    if ["references", "bibliography", "参考文献"].contains(&t.as_str()) {
        Marker::References
    } else if ["appendix", "appendices", "annex", "supplementary material"].contains(&t.as_str()) {
        Marker::Annex
    } else if heading_depth(text) > 0 {
        Marker::BodyStart
    } else {
        Marker::None
    }
}
fn is_heading(token: &LayoutToken, text: &str, min_font_size_milli_pt: u32) -> bool {
    let t = text.trim();
    if t.is_empty() || is_caption_like(t) {
        return false;
    }
    token.style.bold
        || token
            .style
            .font_size_milli_pt
            .is_some_and(|s| s >= min_font_size_milli_pt)
        || heading_depth(t) > 0
}
fn is_caption_like(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    ["figure ", "fig. ", "table ", "caption ", "formula "]
        .iter()
        .any(|p| t.starts_with(p))
}
fn heading_depth(text: &str) -> usize {
    let t = text.trim();
    let digits = t
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    if digits.chars().all(|c| c.is_ascii_digit() || c == '.')
        && digits.chars().any(|c| c.is_ascii_digit())
    {
        digits.split('.').filter(|s| !s.is_empty()).count()
    } else if t
        .chars()
        .all(|c| c.is_uppercase() || c.is_whitespace() || c.is_ascii_punctuation())
        && t.chars().any(|c| c.is_alphabetic())
    {
        1
    } else {
        0
    }
}
fn region_name(r: DocumentRegion) -> &'static str {
    match r {
        DocumentRegion::Header => "header",
        DocumentRegion::Body => "body",
        DocumentRegion::References => "references",
        DocumentRegion::Annex => "annex",
        DocumentRegion::PageFurniture => "page_furniture",
    }
}
fn text_digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn digest_parts(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0u8]);
    }
    format!("sha256:{:x}", h.finalize())
}
fn validate_config(c: &SegmentationConfig) -> Result<(), SegmentationError> {
    if c.limits.max_regions != 5
        || !(0.0..=1.0).contains(&c.margin_fraction)
        || !(0.0..=1.0).contains(&c.furniture_min_page_fraction)
    {
        Err(SegmentationError::InvalidConfig)
    } else {
        Ok(())
    }
}
fn validate_sidecar(
    doc: &LayoutDocument,
    rows: &[LexemeRow],
    provider: &str,
) -> Result<(), SegmentationError> {
    let mut keys = std::collections::BTreeSet::new();
    let mut row_keys = Vec::with_capacity(rows.len());
    for row in rows {
        if row.document_id != doc.document_id {
            return Err(SegmentationError::DocumentMismatch);
        }
        if row.provider_id != provider {
            return Err(SegmentationError::ProviderMismatch);
        }
        if !keys.insert((row.page_ordinal, row.source_ordinal)) {
            return Err(SegmentationError::DuplicateLexeme {
                page: row.page_ordinal,
                source: row.source_ordinal,
            });
        }
        if text_digest(&row.text) != row.text_digest {
            return Err(SegmentationError::DigestMismatch {
                page: row.page_ordinal,
                source: row.source_ordinal,
            });
        }
        row_keys.push((row.page_ordinal, row.source_ordinal));
    }
    let token_keys: std::collections::BTreeSet<_> = doc
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .filter(|token| {
            !matches!(
                token.kind,
                LayoutTokenKind::Formula | LayoutTokenKind::Image
            )
        })
        .map(|token| (token.page_ordinal, token.source_ordinal))
        .collect();
    let document_order = doc
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .filter(|token| {
            !matches!(
                token.kind,
                LayoutTokenKind::Formula | LayoutTokenKind::Image
            )
        })
        .map(|token| (token.page_ordinal, token.source_ordinal))
        .collect::<Vec<_>>();

    // Resolve the join by identity before validating either admitted order.
    // This keeps typed sidecar failures specific: an incomplete or extra
    // sidecar must not be reported as an ordering error merely because its
    // key sequence cannot match a complete document sequence.
    for (page, source) in &document_order {
        if !keys.contains(&(*page, *source)) {
            return Err(SegmentationError::MissingLexeme {
                page: *page,
                source: *source,
            });
        }
    }
    for row in rows {
        if !token_keys.contains(&(row.page_ordinal, row.source_ordinal)) {
            return Err(SegmentationError::InvalidLexeme);
        }
    }

    let source_order = token_keys.iter().copied().collect::<Vec<_>>();
    if row_keys != source_order && row_keys != document_order {
        let (page, source) = document_order.first().copied().unwrap_or((0, 0));
        return Err(SegmentationError::MalformedOrder { page, source });
    }
    for page in &doc.pages {
        for t in &page.tokens {
            if matches!(t.kind, LayoutTokenKind::Formula | LayoutTokenKind::Image) {
                continue;
            }
            let row = rows
                .iter()
                .find(|row| {
                    row.page_ordinal == t.page_ordinal && row.source_ordinal == t.source_ordinal
                })
                .expect("sidecar coverage checked above");
            if row.text_digest != t.text_digest {
                return Err(SegmentationError::DigestMismatch {
                    page: t.page_ordinal,
                    source: t.source_ordinal,
                });
            }
        }
    }
    Ok(())
}
fn furniture_keys(
    doc: &LayoutDocument,
    rows: &[LexemeRow],
    c: &SegmentationConfig,
) -> std::collections::BTreeSet<(u32, u32)> {
    let mut counts = std::collections::BTreeMap::<String, std::collections::BTreeSet<u32>>::new();
    for page in &doc.pages {
        for t in &page.tokens {
            if (t.bbox.y_min <= page.height * c.margin_fraction
                || t.bbox.y_max >= page.height * (1.0 - c.margin_fraction))
                && let Some(r) = rows.iter().find(|r| {
                    r.page_ordinal == t.page_ordinal && r.source_ordinal == t.source_ordinal
                })
            {
                counts
                    .entry(r.text.trim().to_ascii_lowercase())
                    .or_default()
                    .insert(page.page_ordinal);
            }
        }
    }
    let threshold = ((doc.pages.len() as f64) * c.furniture_min_page_fraction).ceil() as usize;
    let repeated: std::collections::BTreeSet<_> = counts
        .into_iter()
        .filter(|(k, pages)| !k.is_empty() && pages.len() >= threshold)
        .map(|(k, _)| k)
        .collect();
    rows.iter()
        .filter(|r| repeated.contains(&r.text.trim().to_ascii_lowercase()))
        .map(|r| (r.page_ordinal, r.source_ordinal))
        .collect()
}

pub fn segment_document(
    document: &LayoutDocument,
    lexemes: &[LexemeRow],
    config: SegmentationConfig,
) -> Result<SegmentationOutcome, SegmentationError> {
    NativeSegmenter::new().segment(document, lexemes, config)
}

/// Versioned additive companion for the nine-region, S01-lattice-bound shadow.
/// This deliberately does not alter [`NativeSegmenter`] or its v1 wire format.
pub const FULL_SEGMENTATION_SCHEMA_VERSION: &str = "da-parse.native-segmentation/v2";
pub const FULL_SEGMENTATION_REGION_COUNT: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum FullSemanticRegion {
    Header,
    Body,
    References,
    Annex,
    Acknowledgement,
    Toc,
    Funding,
    Availability,
    PageFurniture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullFurnitureSubtype {
    PageNumber,
    Cover,
    Footnote,
    Headnote,
    MarginNote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullLocation {
    pub page_ordinal: u32,
    pub block_ordinal: u32,
    pub line_ordinal: u32,
    pub token_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullTokenAssignment {
    pub location: FullLocation,
    pub source_ordinal: u32,
    pub text_digest: String,
    pub region: FullSemanticRegion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullRegionRun {
    pub region: FullSemanticRegion,
    pub start: FullLocation,
    pub end: FullLocation,
    pub boundary_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullFurnitureAnnotation {
    pub location: FullLocation,
    pub source_ordinal: u32,
    pub subtype: FullFurnitureSubtype,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullRegionAbsence {
    NoAdmittedTokens,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullRegionDisposition {
    pub region: FullSemanticRegion,
    pub absence: Option<FullRegionAbsence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullSegmentationIdentity {
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullSegmentationOutcome {
    pub schema_version: String,
    pub identity: FullSegmentationIdentity,
    pub assignments: Vec<FullTokenAssignment>,
    pub runs: Vec<FullRegionRun>,
    /// Furniture is intentionally a different ordered sequence from reading order.
    pub furniture: Vec<FullFurnitureAnnotation>,
    pub dispositions: Vec<FullRegionDisposition>,
    pub order_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullSegmentationError {
    LayoutInvalid,
    LatticeInvalid,
    IdentityMismatch(&'static str),
    UnknownFurniture,
    ConflictingFurniture,
    MissingLatticeToken { page: u32, source: u32 },
    UnexpectedLatticeToken { page: u32, source: u32 },
    LatticeTokenMismatch { page: u32, source: u32 },
    NonMonotonic,
    Serialization,
}
impl fmt::Display for FullSegmentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "full native segmentation rejected input: {self:?}")
    }
}
impl std::error::Error for FullSegmentationError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct FullLatticeSegmenter;

impl FullLatticeSegmenter {
    pub const fn new() -> Self {
        Self
    }

    pub fn segment(
        &self,
        document: &LayoutDocument,
        lattice: &LayoutLattice,
        lexemes: &[LexemeRow],
        config: SegmentationConfig,
    ) -> Result<FullSegmentationOutcome, FullSegmentationError> {
        document
            .validate()
            .map_err(|_| FullSegmentationError::LayoutInvalid)?;
        lattice
            .validate_against(document)
            .map_err(|_| FullSegmentationError::LatticeInvalid)?;
        if lattice.identity.document_id != document.document_id
            || lattice.identity.source_pdf_sha256 != document.source_pdf_sha256
        {
            return Err(FullSegmentationError::IdentityMismatch("layout_document"));
        }
        let provider = document
            .pages
            .iter()
            .flat_map(|p| p.tokens.iter())
            .next()
            .map(|t| t.provider.id.clone())
            .unwrap_or_default();
        validate_sidecar(document, lexemes, &provider).map_err(|e| {
            FullSegmentationError::IdentityMismatch(match e {
                SegmentationError::DocumentMismatch => "document_id",
                SegmentationError::ProviderMismatch => "provider_id",
                _ => "lexeme_sidecar",
            })
        })?;
        let lattice_fingerprint = lattice
            .digest()
            .map_err(|_| FullSegmentationError::Serialization)?;
        let config_fingerprint =
            digest_parts(&[&serde_json::to_string(&config)
                .map_err(|_| FullSegmentationError::Serialization)?]);
        let locations = lattice_locations(lattice, document)?;
        let furniture = furniture_annotations(lattice, &locations)?;
        let furniture_keys: std::collections::BTreeSet<_> = furniture
            .iter()
            .map(|f| (f.location.page_ordinal, f.source_ordinal))
            .collect();
        let mut assignments: Vec<FullTokenAssignment> = Vec::new();
        let mut current = None;
        let mut run_start: Option<FullLocation> = None;
        let mut runs = Vec::new();
        for page in &document.pages {
            for token in &page.tokens {
                if !token.kind.is_lexical() {
                    continue;
                }
                let location = locations
                    .get(&(token.page_ordinal, token.source_ordinal))
                    .ok_or(FullSegmentationError::MissingLatticeToken {
                        page: token.page_ordinal,
                        source: token.source_ordinal,
                    })?
                    .clone();
                let text = lexemes
                    .iter()
                    .find(|l| {
                        l.page_ordinal == token.page_ordinal
                            && l.source_ordinal == token.source_ordinal
                    })
                    .map(LexemeRow::text)
                    .unwrap_or("");
                let is_furniture =
                    furniture_keys.contains(&(token.page_ordinal, token.source_ordinal));
                let region = if is_furniture {
                    FullSemanticRegion::PageFurniture
                } else {
                    full_marker(text)
                        .unwrap_or_else(|| current.unwrap_or(FullSemanticRegion::Header))
                };
                if !is_furniture && current != Some(region) {
                    if let (Some(previous), Some(start)) = (current, run_start.take()) {
                        let end = assignments
                            .iter()
                            .rev()
                            .find(|a| a.region != FullSemanticRegion::PageFurniture)
                            .map(|a| a.location.clone())
                            .unwrap_or_else(|| start.clone());
                        runs.push(full_run(previous, start, end));
                    }
                    current = Some(region);
                    run_start = Some(location.clone());
                }
                assignments.push(FullTokenAssignment {
                    location,
                    source_ordinal: token.source_ordinal,
                    text_digest: token.text_digest.clone(),
                    region,
                });
            }
        }
        if let (Some(region), Some(start)) = (current, run_start) {
            let end = assignments
                .iter()
                .rev()
                .find(|a| a.region != FullSemanticRegion::PageFurniture)
                .map(|a| a.location.clone())
                .unwrap_or_else(|| start.clone());
            runs.push(full_run(region, start, end));
        }
        validate_assignment_order(&assignments)?;
        let dispositions = full_regions()
            .into_iter()
            .map(|region| FullRegionDisposition {
                region,
                absence: (!assignments.iter().any(|a| a.region == region))
                    .then_some(FullRegionAbsence::NoAdmittedTokens),
            })
            .collect();
        let order_digest = digest_parts(
            &assignments
                .iter()
                .map(|a| {
                    format!(
                        "{}:{}:{}:{}:{}",
                        a.location.page_ordinal,
                        a.location.block_ordinal,
                        a.location.line_ordinal,
                        a.location.token_ordinal,
                        full_region_name(a.region)
                    )
                })
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        Ok(FullSegmentationOutcome {
            schema_version: FULL_SEGMENTATION_SCHEMA_VERSION.to_owned(),
            identity: FullSegmentationIdentity {
                document_id: document.document_id.clone(),
                provider_id: provider,
                source_pdf_sha256: document.source_pdf_sha256.clone(),
                lattice_fingerprint,
                config_fingerprint,
            },
            assignments,
            runs,
            furniture,
            dispositions,
            order_digest,
        })
    }
}

pub fn segment_document_full(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    config: SegmentationConfig,
) -> Result<FullSegmentationOutcome, FullSegmentationError> {
    FullLatticeSegmenter::new().segment(document, lattice, lexemes, config)
}

fn full_regions() -> [FullSemanticRegion; FULL_SEGMENTATION_REGION_COUNT] {
    [
        FullSemanticRegion::Header,
        FullSemanticRegion::Body,
        FullSemanticRegion::References,
        FullSemanticRegion::Annex,
        FullSemanticRegion::Acknowledgement,
        FullSemanticRegion::Toc,
        FullSemanticRegion::Funding,
        FullSemanticRegion::Availability,
        FullSemanticRegion::PageFurniture,
    ]
}
fn full_region_name(r: FullSemanticRegion) -> &'static str {
    match r {
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
pub(crate) fn full_marker(text: &str) -> Option<FullSemanticRegion> {
    match text.trim().to_ascii_lowercase().as_str() {
        "acknowledgement" | "acknowledgements" | "acknowledgment" | "acknowledgments" => {
            Some(FullSemanticRegion::Acknowledgement)
        }
        "contents" | "table of contents" | "toc" => Some(FullSemanticRegion::Toc),
        "funding" | "funding information" => Some(FullSemanticRegion::Funding),
        "availability" | "data availability" | "code availability" => {
            Some(FullSemanticRegion::Availability)
        }
        "references" | "bibliography" | "参考文献" => Some(FullSemanticRegion::References),
        "appendix" | "appendices" | "annex" | "supplementary material" => {
            Some(FullSemanticRegion::Annex)
        }
        "introduction"
        | "methods"
        | "materials and methods"
        | "results"
        | "discussion"
        | "conclusion"
        | "conclusions" => Some(FullSemanticRegion::Body),
        _ => None,
    }
}
fn full_run(region: FullSemanticRegion, start: FullLocation, end: FullLocation) -> FullRegionRun {
    let boundary_digest = digest_parts(&[
        full_region_name(region),
        &format!("{:?}", start),
        &format!("{:?}", end),
    ]);
    FullRegionRun {
        region,
        start,
        end,
        boundary_digest,
    }
}
fn validate_assignment_order(
    assignments: &[FullTokenAssignment],
) -> Result<(), FullSegmentationError> {
    if assignments.windows(2).any(|w| {
        (
            w[0].location.page_ordinal,
            w[0].location.block_ordinal,
            w[0].location.line_ordinal,
            w[0].location.token_ordinal,
        ) >= (
            w[1].location.page_ordinal,
            w[1].location.block_ordinal,
            w[1].location.line_ordinal,
            w[1].location.token_ordinal,
        )
    }) {
        Err(FullSegmentationError::NonMonotonic)
    } else {
        Ok(())
    }
}
fn lattice_locations(
    lattice: &LayoutLattice,
    document: &LayoutDocument,
) -> Result<std::collections::BTreeMap<(u32, u32), FullLocation>, FullSegmentationError> {
    let mut out = std::collections::BTreeMap::new();
    for page in &lattice.pages {
        for (bi, block) in page.blocks.iter().enumerate() {
            for (li, line) in block.lines.iter().enumerate() {
                for token in &line.tokens {
                    let key = (page.page_ordinal, token.source_ordinal);
                    if out
                        .insert(
                            key,
                            FullLocation {
                                page_ordinal: page.page_ordinal,
                                block_ordinal: block.source_ordinal,
                                line_ordinal: line.source_ordinal,
                                token_ordinal: token.token_ordinal,
                            },
                        )
                        .is_some()
                    {
                        return Err(FullSegmentationError::LatticeTokenMismatch {
                            page: key.0,
                            source: key.1,
                        });
                    }
                    let source = document.pages[page.page_ordinal as usize]
                        .tokens
                        .iter()
                        .find(|t| t.source_ordinal == token.source_ordinal)
                        .ok_or(FullSegmentationError::UnexpectedLatticeToken {
                            page: key.0,
                            source: key.1,
                        })?;
                    if source.text_digest != token.text_digest
                        || source.token_ordinal != token.token_ordinal
                    {
                        return Err(FullSegmentationError::LatticeTokenMismatch {
                            page: key.0,
                            source: key.1,
                        });
                    }
                    let _ = (bi, li);
                }
            }
        }
    }
    for page in document.pages.iter() {
        for token in page.tokens.iter().filter(|t| t.kind.is_lexical()) {
            if !out.contains_key(&(page.page_ordinal, token.source_ordinal)) {
                return Err(FullSegmentationError::MissingLatticeToken {
                    page: page.page_ordinal,
                    source: token.source_ordinal,
                });
            }
        }
    }
    Ok(out)
}
fn furniture_annotations(
    lattice: &LayoutLattice,
    locations: &std::collections::BTreeMap<(u32, u32), FullLocation>,
) -> Result<Vec<FullFurnitureAnnotation>, FullSegmentationError> {
    let mut out = Vec::new();
    for page in &lattice.pages {
        for block in &page.blocks {
            let subtype = match block.furniture {
                // Honest donor evidence: Unknown means "not classified", not a
                // conflicting furniture claim. Treat it like Absent so shadow
                // lanes can still emit semantic runs. ConflictingFurniture stays
                // reserved for two classified claims on the same token.
                FurnitureEvidence::Absent | FurnitureEvidence::Unknown => continue,
                FurnitureEvidence::Classified { kind } => match kind {
                    FurnitureKind::PageNumber => FullFurnitureSubtype::PageNumber,
                    FurnitureKind::Footnote => FullFurnitureSubtype::Footnote,
                    FurnitureKind::Headnote => FullFurnitureSubtype::Headnote,
                    FurnitureKind::Marginnote => FullFurnitureSubtype::MarginNote,
                    FurnitureKind::Header | FurnitureKind::Footer => FullFurnitureSubtype::Cover,
                },
            };
            for line in &block.lines {
                for token in &line.tokens {
                    if let Some(location) =
                        locations.get(&(page.page_ordinal, token.source_ordinal))
                    {
                        out.push(FullFurnitureAnnotation {
                            location: location.clone(),
                            source_ordinal: token.source_ordinal,
                            subtype,
                            evidence: "lattice_classified".to_owned(),
                        });
                    }
                }
            }
        }
    }
    out.sort_by_key(|f| {
        (
            f.location.page_ordinal,
            f.location.block_ordinal,
            f.location.line_ordinal,
            f.location.token_ordinal,
        )
    });
    Ok(out)
}
