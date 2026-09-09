//! Additive page/block/line lattice companion for [`LayoutDocument`] v1.
//!
//! The lattice is an observation contract, not a parser route. It retains
//! identities and redacted geometry/order metadata while leaving lexical text
//! and visual evidence in their existing, separate surfaces.

use crate::{LayoutBBox, LayoutDocument, LayoutError, LayoutSpan, LayoutTokenKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

pub const LAYOUT_LATTICE_SCHEMA_VERSION: &str = "da-parse.layout-lattice/v1";
pub const MAX_LATTICE_PAGES: usize = 10_000;
pub const MAX_LATTICE_BLOCKS: usize = 100_000;
pub const MAX_LATTICE_LINES: usize = 500_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateDialect {
    AltoTopLeft72Dpi,
    BottomLeft72Dpi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingOrder {
    XmlSource,
    XyCutDerived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FurnitureKind {
    Header,
    Footer,
    Footnote,
    Headnote,
    Marginnote,
    PageNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum FurnitureEvidence {
    Classified { kind: FurnitureKind },
    Absent,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatticeIdentity {
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub layout_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatticeToken {
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub bbox: LayoutBBox,
    pub text_digest: String,
    pub source_span: LayoutSpan,
    pub kind: LayoutTokenKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatticeLine {
    pub source_ordinal: u32,
    pub source_span: LayoutSpan,
    pub bbox: LayoutBBox,
    pub tokens: Vec<LatticeToken>,
    pub newline_boundary_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatticeBlock {
    pub source_ordinal: u32,
    pub source_span: LayoutSpan,
    pub bbox: LayoutBBox,
    pub lines: Vec<LatticeLine>,
    pub source_order: u32,
    pub derived_order: Option<u32>,
    pub furniture: FurnitureEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatticePage {
    /// Rust's zero-based page ordinal, matching `LayoutPage::page_ordinal`.
    pub page_ordinal: u32,
    /// Java's one-based page number, retained explicitly for auditability.
    pub java_page_number: u32,
    pub width: f64,
    pub height: f64,
    pub rotation: u16,
    pub crop_box: Option<LayoutBBox>,
    pub blocks: Vec<LatticeBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutLattice {
    pub schema_version: String,
    pub identity: LatticeIdentity,
    pub coordinate_dialect: CoordinateDialect,
    pub source_order: ReadingOrder,
    pub derived_order: Option<ReadingOrder>,
    pub pages: Vec<LatticePage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutLatticeError {
    InvalidSchema,
    InvalidIdentity(&'static str),
    InvalidJavaPage(u32),
    PageMappingMismatch {
        java: u32,
        rust: u32,
    },
    PageMismatch(u32),
    PageGeometry(u32),
    InvalidRotation(u32),
    InvalidSpan {
        page: u32,
        kind: &'static str,
        ordinal: u32,
    },
    SpanOverlap {
        page: u32,
        kind: &'static str,
        ordinal: u32,
    },
    NonMonotonic {
        page: u32,
        kind: &'static str,
    },
    DuplicateToken {
        page: u32,
        source: u32,
    },
    MissingToken {
        page: u32,
        source: u32,
    },
    UnexpectedToken {
        page: u32,
        source: u32,
    },
    VisualTokenInLexicalLattice {
        page: u32,
        source: u32,
    },
    TooMany(&'static str, usize),
    Serialization,
}

impl fmt::Display for LayoutLatticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid layout lattice: {self:?}")
    }
}
impl std::error::Error for LayoutLatticeError {}

impl LayoutLattice {
    pub fn new(
        document: &LayoutDocument,
        coordinate_dialect: CoordinateDialect,
        source_order: ReadingOrder,
        derived_order: Option<ReadingOrder>,
        pages: Vec<LatticePage>,
    ) -> Result<Self, LayoutLatticeError> {
        let lattice = Self {
            schema_version: LAYOUT_LATTICE_SCHEMA_VERSION.to_owned(),
            identity: LatticeIdentity {
                document_id: document.document_id.clone(),
                source_pdf_sha256: document.source_pdf_sha256.clone(),
                layout_digest: document
                    .digest()
                    .map_err(|_| LayoutLatticeError::Serialization)?,
            },
            coordinate_dialect,
            source_order,
            derived_order,
            pages,
        };
        lattice.validate_against(document)?;
        Ok(lattice)
    }

    pub fn java_page_to_rust(java_page_number: u32) -> Result<u32, LayoutLatticeError> {
        java_page_number
            .checked_sub(1)
            .ok_or(LayoutLatticeError::InvalidJavaPage(java_page_number))
    }

    pub fn validate_against(&self, document: &LayoutDocument) -> Result<(), LayoutLatticeError> {
        document
            .validate()
            .map_err(|_| LayoutLatticeError::Serialization)?;
        if self.schema_version != LAYOUT_LATTICE_SCHEMA_VERSION {
            return Err(LayoutLatticeError::InvalidSchema);
        }
        if self.identity.document_id != document.document_id
            || self.identity.source_pdf_sha256 != document.source_pdf_sha256
            || self.identity.layout_digest
                != document
                    .digest()
                    .map_err(|_| LayoutLatticeError::Serialization)?
        {
            return Err(LayoutLatticeError::InvalidIdentity("layout_document"));
        }
        if self.pages.len() > MAX_LATTICE_PAGES {
            return Err(LayoutLatticeError::TooMany("pages", self.pages.len()));
        }
        let mut admitted = BTreeSet::new();
        let mut block_count = 0;
        let mut line_count = 0;
        if self.pages.len() != document.pages.len() {
            return Err(LayoutLatticeError::PageMismatch(self.pages.len() as u32));
        }
        let mut previous_page = None;
        for (page_index, page) in self.pages.iter().enumerate() {
            let expected = Self::java_page_to_rust(page.java_page_number)?;
            if expected != page.page_ordinal || page.page_ordinal as usize != page_index {
                return Err(LayoutLatticeError::PageMappingMismatch {
                    java: page.java_page_number,
                    rust: page.page_ordinal,
                });
            }
            if previous_page.is_some_and(|p| page.page_ordinal <= p)
                || page.page_ordinal as usize >= document.pages.len()
            {
                return Err(LayoutLatticeError::PageMismatch(page.page_ordinal));
            }
            previous_page = Some(page.page_ordinal);
            let source_page = &document.pages[page.page_ordinal as usize];
            if page.width != source_page.width
                || page.height != source_page.height
                || page.rotation != source_page.rotation
                || page.crop_box != source_page.crop_box
            {
                return Err(LayoutLatticeError::PageGeometry(page.page_ordinal));
            }
            if !matches!(page.rotation, 0 | 90 | 180 | 270) {
                return Err(LayoutLatticeError::InvalidRotation(page.page_ordinal));
            }
            let mut previous_block = None;
            let mut block_spans = Vec::new();
            for block in &page.blocks {
                block_count += 1;
                if block_count > MAX_LATTICE_BLOCKS {
                    return Err(LayoutLatticeError::TooMany("blocks", block_count));
                }
                monotonic(
                    page.page_ordinal,
                    "block",
                    previous_block,
                    block.source_ordinal,
                )?;
                previous_block = Some(block.source_ordinal);
                validate_span(
                    &block.source_span,
                    page.page_ordinal,
                    "block",
                    block.source_ordinal,
                )?;
                ensure_no_overlap(
                    &block.source_span,
                    &block_spans,
                    page.page_ordinal,
                    "block",
                    block.source_ordinal,
                )?;
                block_spans.push(block.source_span.clone());
                if !block.bbox.within_for_contract(page.width, page.height) {
                    return Err(LayoutLatticeError::PageGeometry(page.page_ordinal));
                }
                let mut previous_line = None;
                let mut line_spans = Vec::new();
                for line in &block.lines {
                    line_count += 1;
                    if line_count > MAX_LATTICE_LINES {
                        return Err(LayoutLatticeError::TooMany("lines", line_count));
                    }
                    monotonic(
                        page.page_ordinal,
                        "line",
                        previous_line,
                        line.source_ordinal,
                    )?;
                    previous_line = Some(line.source_ordinal);
                    validate_span(
                        &line.source_span,
                        page.page_ordinal,
                        "line",
                        line.source_ordinal,
                    )?;
                    ensure_no_overlap(
                        &line.source_span,
                        &line_spans,
                        page.page_ordinal,
                        "line",
                        line.source_ordinal,
                    )?;
                    line_spans.push(line.source_span.clone());
                    if !line.bbox.within_for_contract(page.width, page.height) {
                        return Err(LayoutLatticeError::PageGeometry(page.page_ordinal));
                    }
                    let mut previous_token = None;
                    let mut token_spans = Vec::new();
                    for token in &line.tokens {
                        if !token.kind.is_lexical() {
                            return Err(LayoutLatticeError::VisualTokenInLexicalLattice {
                                page: page.page_ordinal,
                                source: token.source_ordinal,
                            });
                        }
                        monotonic(
                            page.page_ordinal,
                            "token",
                            previous_token,
                            token.source_ordinal,
                        )?;
                        previous_token = Some(token.source_ordinal);
                        validate_span(
                            &token.source_span,
                            page.page_ordinal,
                            "token",
                            token.source_ordinal,
                        )?;
                        ensure_no_overlap(
                            &token.source_span,
                            &token_spans,
                            page.page_ordinal,
                            "token",
                            token.source_ordinal,
                        )?;
                        token_spans.push(token.source_span.clone());
                        if !token.bbox.within_for_contract(page.width, page.height) {
                            return Err(LayoutLatticeError::PageGeometry(page.page_ordinal));
                        }
                        if !admitted.insert((page.page_ordinal, token.source_ordinal)) {
                            return Err(LayoutLatticeError::DuplicateToken {
                                page: page.page_ordinal,
                                source: token.source_ordinal,
                            });
                        }
                    }
                }
            }
        }
        for page in &document.pages {
            let expected: BTreeSet<_> = page
                .tokens
                .iter()
                .filter(|t| t.kind.is_lexical())
                .map(|t| (page.page_ordinal, t.source_ordinal))
                .collect();
            for &(admitted_page, admitted_source) in
                admitted.iter().filter(|(p, _)| *p == page.page_ordinal)
            {
                if !expected.contains(&(admitted_page, admitted_source)) {
                    return Err(LayoutLatticeError::UnexpectedToken {
                        page: admitted_page,
                        source: admitted_source,
                    });
                }
            }
            for token in page.tokens.iter().filter(|t| t.kind.is_lexical()) {
                if !admitted.contains(&(page.page_ordinal, token.source_ordinal)) {
                    return Err(LayoutLatticeError::MissingToken {
                        page: page.page_ordinal,
                        source: token.source_ordinal,
                    });
                }
            }
            if page.tokens.iter().any(|t| t.kind.is_visual())
                && admitted.iter().any(|(p, _)| *p == page.page_ordinal)
            {
                // Visual tokens are allowed in LayoutDocument but never admitted
                // into this lexical lattice; this branch is intentionally a no-op.
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, LayoutLatticeError> {
        let bytes = serde_json::to_vec(self).map_err(|_| LayoutLatticeError::Serialization)?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

fn monotonic(
    page: u32,
    kind: &'static str,
    previous: Option<u32>,
    next: u32,
) -> Result<(), LayoutLatticeError> {
    if previous.is_some_and(|p| next <= p) {
        Err(LayoutLatticeError::NonMonotonic { page, kind })
    } else {
        Ok(())
    }
}

fn validate_span(
    span: &LayoutSpan,
    page: u32,
    kind: &'static str,
    ordinal: u32,
) -> Result<(), LayoutLatticeError> {
    if matches!(span, LayoutSpan::Present { start, end } if start >= end) {
        Err(LayoutLatticeError::InvalidSpan {
            page,
            kind,
            ordinal,
        })
    } else {
        Ok(())
    }
}

fn ensure_no_overlap(
    span: &LayoutSpan,
    prior: &[LayoutSpan],
    page: u32,
    kind: &'static str,
    ordinal: u32,
) -> Result<(), LayoutLatticeError> {
    let Some((start, end)) = present_bounds(span) else {
        return Ok(());
    };
    if prior
        .iter()
        .filter_map(present_bounds)
        .any(|(a, b)| start < b && a < end)
    {
        Err(LayoutLatticeError::SpanOverlap {
            page,
            kind,
            ordinal,
        })
    } else {
        Ok(())
    }
}

fn present_bounds(span: &LayoutSpan) -> Option<(u32, u32)> {
    match span {
        LayoutSpan::Present { start, end } => Some((*start, *end)),
        LayoutSpan::Absent { .. } => None,
    }
}

// LayoutBBox's bounds check is intentionally private in the v1 module. Keep
// this companion's check local so LayoutDocument remains unchanged.
trait BoundedBBox {
    fn within_for_contract(&self, width: f64, height: f64) -> bool;
}
impl BoundedBBox for LayoutBBox {
    fn within_for_contract(&self, width: f64, height: f64) -> bool {
        self.x_min.is_finite()
            && self.y_min.is_finite()
            && self.x_max.is_finite()
            && self.y_max.is_finite()
            && self.x_min >= 0.0
            && self.y_min >= 0.0
            && self.x_max <= width
            && self.y_max <= height
            && self.x_min < self.x_max
            && self.y_min < self.y_max
    }
}

impl From<LayoutError> for LayoutLatticeError {
    fn from(_: LayoutError) -> Self {
        Self::Serialization
    }
}
