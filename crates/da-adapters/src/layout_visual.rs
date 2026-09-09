//! Invocation-local, provider-neutral visual observations for layout donors.
//!
//! This is an additive shadow seam. It never serializes pixels, PDF bytes,
//! extracted text, captions, or filesystem paths. A donor that has no visual
//! signal must return a typed terminal absence rather than an empty-success
//! observation.

use da_parse::{LayoutBBox, LayoutDocument, LayoutError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const VISUAL_OBSERVATION_SCHEMA_VERSION: u16 = 1;
pub const MAX_VISUAL_PRIMITIVES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualPrimitiveKind {
    Image,
    Graphic,
    Table,
    Formula,
    Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAbsenceReason {
    Unsupported,
    NotObserved,
    ProviderUnavailable,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualProvider {
    pub id: String,
    pub version: String,
}

impl VisualProvider {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Result<Self, VisualError> {
        let provider = Self {
            id: id.into(),
            version: version.into(),
        };
        if provider.id.is_empty()
            || provider.id.len() > 128
            || provider.version.is_empty()
            || provider.version.len() > 128
        {
            return Err(VisualError::InvalidIdentity("provider"));
        }
        Ok(provider)
    }
}

/// The state of one source-bound visual item. Absence is data, not success.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VisualPrimitiveState {
    Observed { content_digest: String },
    Absent { reason: VisualAbsenceReason },
}

impl fmt::Debug for VisualPrimitiveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observed { .. } => f
                .debug_struct("Observed")
                .field("content_digest", &"[redacted]")
                .finish(),
            Self::Absent { reason } => f.debug_struct("Absent").field("reason", reason).finish(),
        }
    }
}

/// One bounded primitive or an explicit typed absence. No payload is retained.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualPrimitive {
    pub document_id: String,
    pub provider: VisualProvider,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub kind: VisualPrimitiveKind,
    pub bbox: Option<LayoutBBox>,
    pub state: VisualPrimitiveState,
}

impl fmt::Debug for VisualPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisualPrimitive")
            .field("document_id", &self.document_id)
            .field("provider", &self.provider)
            .field("page_ordinal", &self.page_ordinal)
            .field("source_ordinal", &self.source_ordinal)
            .field("kind", &self.kind)
            .field("bbox", &self.bbox)
            .field("state", &self.state)
            .finish()
    }
}

impl VisualPrimitive {
    #[allow(clippy::too_many_arguments)]
    pub fn observed(
        document: &LayoutDocument,
        provider: VisualProvider,
        page_ordinal: u32,
        source_ordinal: u32,
        kind: VisualPrimitiveKind,
        bbox: LayoutBBox,
        content_digest: impl Into<String>,
    ) -> Result<Self, VisualError> {
        let primitive = Self {
            document_id: document.document_id.clone(),
            provider,
            page_ordinal,
            source_ordinal,
            kind,
            bbox: Some(bbox),
            state: VisualPrimitiveState::Observed {
                content_digest: content_digest.into(),
            },
        };
        primitive.validate(document)?;
        Ok(primitive)
    }

    pub fn absent(
        document: &LayoutDocument,
        provider: VisualProvider,
        page_ordinal: u32,
        source_ordinal: u32,
        kind: VisualPrimitiveKind,
        reason: VisualAbsenceReason,
    ) -> Result<Self, VisualError> {
        let primitive = Self {
            document_id: document.document_id.clone(),
            provider,
            page_ordinal,
            source_ordinal,
            kind,
            bbox: None,
            state: VisualPrimitiveState::Absent { reason },
        };
        primitive.validate(document)?;
        Ok(primitive)
    }

    pub fn validate(&self, document: &LayoutDocument) -> Result<(), VisualError> {
        if self.document_id != document.document_id {
            return Err(VisualError::InvalidIdentity("document_id"));
        }
        // Keep the pre-lattice direct visual helper compatible with its
        // historical one-based caller while all donor receipts remain
        // canonical zero-based. Ambiguity is avoided whenever an exact page
        // exists; the alias is only a fallback for legacy callers.
        let canonical_page = if document
            .pages
            .iter()
            .any(|page| page.page_ordinal == self.page_ordinal)
        {
            self.page_ordinal
        } else if self.page_ordinal > 0
            && document
                .pages
                .iter()
                .any(|page| page.page_ordinal == self.page_ordinal - 1)
        {
            self.page_ordinal - 1
        } else {
            return Err(VisualError::InvalidIdentity("page_ordinal"));
        };
        match (&self.bbox, &self.state) {
            (Some(bbox), VisualPrimitiveState::Observed { content_digest }) => {
                let page = document
                    .pages
                    .iter()
                    .find(|p| p.page_ordinal == canonical_page)
                    .ok_or(VisualError::InvalidIdentity("page_ordinal"))?;
                if bbox.x_min < 0.0
                    || bbox.y_min < 0.0
                    || bbox.x_max > page.width
                    || bbox.y_max > page.height
                {
                    return Err(VisualError::Layout(LayoutError::OutOfPage));
                }
                if !is_sha256(content_digest) {
                    return Err(VisualError::InvalidDigest);
                }
            }
            (None, VisualPrimitiveState::Absent { .. }) => {}
            _ => return Err(VisualError::InvalidIdentity("bbox/state")),
        }
        Ok(())
    }
}

/// Bounded visual sidecar paired with the unchanged neutral LayoutDocument.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualObservation {
    pub schema_version: u16,
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub provider: VisualProvider,
    pub primitives: Vec<VisualPrimitive>,
    /// Terminal lane-level absence when no visual upstream signal exists.
    pub absence: Option<VisualAbsenceReason>,
}

impl fmt::Debug for VisualObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisualObservation")
            .field("schema_version", &self.schema_version)
            .field("document_id", &self.document_id)
            .field("source_pdf_sha256", &"[redacted]")
            .field("provider", &self.provider)
            .field("primitive_count", &self.primitives.len())
            .field("absence", &self.absence)
            .finish()
    }
}

impl VisualObservation {
    pub fn new(
        document: &LayoutDocument,
        primitives: Vec<VisualPrimitive>,
    ) -> Result<Self, VisualError> {
        if primitives.len() > MAX_VISUAL_PRIMITIVES {
            return Err(VisualError::Oversize {
                limit: MAX_VISUAL_PRIMITIVES,
            });
        }
        let provider = primitives
            .first()
            .ok_or(VisualError::InvalidIdentity("provider"))?
            .provider
            .clone();
        let observation = Self {
            schema_version: VISUAL_OBSERVATION_SCHEMA_VERSION,
            document_id: document.document_id.clone(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            provider,
            primitives,
            absence: None,
        };
        observation.validate(document)?;
        Ok(observation)
    }

    pub fn terminal_absence(
        document: &LayoutDocument,
        provider: VisualProvider,
        reason: VisualAbsenceReason,
    ) -> Result<Self, VisualError> {
        if document.document_id.is_empty() {
            return Err(VisualError::InvalidIdentity("document_id"));
        }
        Ok(Self {
            schema_version: VISUAL_OBSERVATION_SCHEMA_VERSION,
            document_id: document.document_id.clone(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            provider,
            primitives: Vec::new(),
            absence: Some(reason),
        })
    }

    pub fn validate(&self, document: &LayoutDocument) -> Result<(), VisualError> {
        if self.schema_version != VISUAL_OBSERVATION_SCHEMA_VERSION
            || self.document_id != document.document_id
            || self.source_pdf_sha256 != document.source_pdf_sha256
            || (self.absence.is_some() && !self.primitives.is_empty())
            || self
                .primitives
                .iter()
                .any(|primitive| primitive.provider != self.provider)
        {
            return Err(VisualError::InvalidIdentity("observation"));
        }
        let mut keys = std::collections::BTreeSet::new();
        for primitive in &self.primitives {
            primitive.validate(document)?;
            if !keys.insert((
                primitive.page_ordinal,
                primitive.source_ordinal,
                primitive.kind as u8,
            )) {
                return Err(VisualError::DuplicateIdentity);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualError {
    InvalidIdentity(&'static str),
    InvalidDigest,
    DuplicateIdentity,
    Oversize { limit: usize },
    Layout(LayoutError),
}
impl fmt::Display for VisualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid visual observation: {self:?}")
    }
}
impl std::error::Error for VisualError {}
impl From<LayoutError> for VisualError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

pub fn digest_bytes(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
