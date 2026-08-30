//! Provider-neutral, lossless positioned-text contract for layout bakeoffs.
//!
//! This module intentionally contains no PDF, TEI, graph, filesystem, or runtime
//! policy types. Producers normalize into a top-left 72-DPI coordinate system;
//! consumers can compare donors without knowing how a token was obtained.

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const LAYOUT_SCHEMA_VERSION: u16 = 1;
const MAX_DOCUMENT_ID: usize = 256;
const MAX_PROVIDER_FIELD: usize = 128;
const MAX_TOKENS: usize = 10_000;
const MAX_PAGES: usize = 10_000;
const MAX_SPAN: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    InvalidField(&'static str),
    InvalidDigest(&'static str),
    NonFiniteGeometry,
    InvertedGeometry,
    OutOfPage,
    InvalidPageSize,
    InvalidRotation,
    InvalidSpan,
    InvalidUtf8Boundary,
    DuplicateOrdinal { page: u32, ordinal: u32 },
    NonMonotonicOrdinal { page: u32, previous: u32, next: u32 },
    TooManyPages { count: usize },
    TooManyTokens { count: usize },
    UnknownSchema { version: u16 },
    Serialization,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid layout contract: {self:?}")
    }
}
impl std::error::Error for LayoutError {}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutBBox {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}
impl LayoutBBox {
    pub fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Result<Self, LayoutError> {
        let b = Self {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        b.validate()?;
        Ok(b)
    }
    fn validate(&self) -> Result<(), LayoutError> {
        if ![self.x_min, self.y_min, self.x_max, self.y_max]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(LayoutError::NonFiniteGeometry);
        }
        if self.x_min >= self.x_max || self.y_min >= self.y_max {
            return Err(LayoutError::InvertedGeometry);
        }
        Ok(())
    }
    pub fn to_bottom_left(self, page_height: f64) -> Result<(f64, f64, f64, f64), LayoutError> {
        if !page_height.is_finite() || page_height <= 0.0 || self.y_max > page_height {
            return Err(LayoutError::OutOfPage);
        }
        Ok((
            self.x_min,
            page_height - self.y_max,
            self.x_max,
            page_height - self.y_min,
        ))
    }
    pub fn from_bottom_left(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        page_height: f64,
    ) -> Result<Self, LayoutError> {
        Self::new(x_min, page_height - y_max, x_max, page_height - y_min)
    }
    fn within(&self, width: f64, height: f64) -> bool {
        self.x_min >= 0.0 && self.y_min >= 0.0 && self.x_max <= width && self.y_max <= height
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutSpanAbsence {
    ProviderUnavailable,
    NotAvailable,
    NonText,
    Malformed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LayoutSpan {
    Present { start: u32, end: u32 },
    Absent { reason: LayoutSpanAbsence },
}
impl LayoutSpan {
    pub fn present(start: u32, end: u32) -> Result<Self, LayoutError> {
        if start >= end || end - start > MAX_SPAN {
            return Err(LayoutError::InvalidSpan);
        }
        Ok(Self::Present { start, end })
    }
    pub fn absent(reason: LayoutSpanAbsence) -> Self {
        Self::Absent { reason }
    }
    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent { .. })
    }
    pub fn validate_against(&self, source: &str) -> Result<(), LayoutError> {
        if let Self::Present { start, end } = self
            && (*end as usize > source.len()
                || !source.is_char_boundary(*start as usize)
                || !source.is_char_boundary(*end as usize))
        {
            return Err(LayoutError::InvalidUtf8Boundary);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LayoutStyleHints {
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub font_size_milli_pt: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutTokenKind {
    Word,
    Whitespace,
    Punctuation,
    Symbol,
    Formula,
    Image,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutProvider {
    pub id: String,
    pub version: String,
    pub config_fingerprint: String,
}
impl LayoutProvider {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        config_fingerprint: String,
    ) -> Result<Self, LayoutError> {
        let p = Self {
            id: id.into(),
            version: version.into(),
            config_fingerprint,
        };
        if p.id.is_empty()
            || p.id.len() > MAX_PROVIDER_FIELD
            || p.version.is_empty()
            || p.version.len() > MAX_PROVIDER_FIELD
            || !is_sha256(&p.config_fingerprint)
        {
            return Err(LayoutError::InvalidField("provider"));
        }
        Ok(p)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutToken {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub bbox: LayoutBBox,
    pub text_digest: String,
    pub source_span: LayoutSpan,
    pub kind: LayoutTokenKind,
    pub style: LayoutStyleHints,
    pub provider: LayoutProvider,
}
impl LayoutToken {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page_ordinal: u32,
        source_ordinal: u32,
        token_ordinal: u32,
        bbox: LayoutBBox,
        text_digest: String,
        source_span: LayoutSpan,
        kind: LayoutTokenKind,
        style: LayoutStyleHints,
        provider: LayoutProvider,
    ) -> Result<Self, LayoutError> {
        if !is_sha256(&text_digest) {
            return Err(LayoutError::InvalidDigest("text_digest"));
        }
        Ok(Self {
            page_ordinal,
            source_ordinal,
            token_ordinal,
            bbox,
            text_digest,
            source_span,
            kind,
            style,
            provider,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutPage {
    pub page_ordinal: u32,
    pub width: f64,
    pub height: f64,
    pub rotation: u16,
    pub crop_box: Option<LayoutBBox>,
    pub tokens: Vec<LayoutToken>,
}
impl LayoutPage {
    pub fn new(
        page_ordinal: u32,
        width: f64,
        height: f64,
        rotation: u16,
        crop_box: Option<LayoutBBox>,
        tokens: Vec<LayoutToken>,
    ) -> Result<Self, LayoutError> {
        let p = Self {
            page_ordinal,
            width,
            height,
            rotation,
            crop_box,
            tokens,
        };
        p.validate()?;
        Ok(p)
    }
    fn validate(&self) -> Result<(), LayoutError> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return Err(LayoutError::InvalidPageSize);
        }
        if !matches!(self.rotation, 0 | 90 | 180 | 270) {
            return Err(LayoutError::InvalidRotation);
        }
        if let Some(b) = self.crop_box {
            b.validate()?;
            if !b.within(self.width, self.height) {
                return Err(LayoutError::OutOfPage);
            }
        }
        let mut previous = None;
        let mut previous_source = None;
        for t in &self.tokens {
            if t.page_ordinal != self.page_ordinal || !t.bbox.within(self.width, self.height) {
                return Err(LayoutError::OutOfPage);
            }
            if let Some(prev) = previous_source
                && t.source_ordinal <= prev
            {
                return Err(if t.source_ordinal == prev {
                    LayoutError::DuplicateOrdinal {
                        page: self.page_ordinal,
                        ordinal: prev,
                    }
                } else {
                    LayoutError::NonMonotonicOrdinal {
                        page: self.page_ordinal,
                        previous: prev,
                        next: t.source_ordinal,
                    }
                });
            }
            previous_source = Some(t.source_ordinal);
            if let Some(prev) = previous
                && t.token_ordinal <= prev
            {
                return Err(if t.token_ordinal == prev {
                    LayoutError::DuplicateOrdinal {
                        page: self.page_ordinal,
                        ordinal: prev,
                    }
                } else {
                    LayoutError::NonMonotonicOrdinal {
                        page: self.page_ordinal,
                        previous: prev,
                        next: t.token_ordinal,
                    }
                });
            }
            previous = Some(t.token_ordinal);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutDocument {
    pub schema_version: u16,
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub pages: Vec<LayoutPage>,
}
impl LayoutDocument {
    pub fn new(
        document_id: impl Into<String>,
        source_pdf_sha256: String,
        pages: Vec<LayoutPage>,
    ) -> Result<Self, LayoutError> {
        let d = Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            document_id: document_id.into(),
            source_pdf_sha256,
            pages,
        };
        d.validate()?;
        Ok(d)
    }
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.schema_version != LAYOUT_SCHEMA_VERSION {
            return Err(LayoutError::UnknownSchema {
                version: self.schema_version,
            });
        }
        if self.document_id.is_empty()
            || self.document_id.len() > MAX_DOCUMENT_ID
            || !self.document_id.is_char_boundary(self.document_id.len())
        {
            return Err(LayoutError::InvalidField("document_id"));
        }
        if !is_sha256(&self.source_pdf_sha256) {
            return Err(LayoutError::InvalidDigest("source_pdf_sha256"));
        }
        if self.pages.len() > MAX_PAGES {
            return Err(LayoutError::TooManyPages {
                count: self.pages.len(),
            });
        }
        let mut prev = None;
        let mut total = 0usize;
        for p in &self.pages {
            p.validate()?;
            if let Some(x) = prev
                && p.page_ordinal <= x
            {
                return Err(LayoutError::NonMonotonicOrdinal {
                    page: p.page_ordinal,
                    previous: x,
                    next: p.page_ordinal,
                });
            }
            prev = Some(p.page_ordinal);
            total += p.tokens.len();
        }
        if total > MAX_TOKENS {
            return Err(LayoutError::TooManyTokens { count: total });
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<String, LayoutError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| LayoutError::Serialization)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl<'de> Deserialize<'de> for LayoutPage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            page_ordinal: u32,
            width: f64,
            height: f64,
            rotation: u16,
            crop_box: Option<LayoutBBox>,
            tokens: Vec<LayoutToken>,
        }
        let r = Raw::deserialize(d)?;
        Self::new(
            r.page_ordinal,
            r.width,
            r.height,
            r.rotation,
            r.crop_box,
            r.tokens,
        )
        .map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for LayoutDocument {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            schema_version: u16,
            document_id: String,
            source_pdf_sha256: String,
            pages: Vec<LayoutPage>,
        }
        let r = Raw::deserialize(d)?;
        let document = Self {
            schema_version: r.schema_version,
            document_id: r.document_id,
            source_pdf_sha256: r.source_pdf_sha256,
            pages: r.pages,
        };
        document.validate().map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}
