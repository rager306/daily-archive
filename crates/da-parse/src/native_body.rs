//! Pure, bounded accounting of source-bound P2 body assignments.
//!
//! This is a shadow-plane accountant: it consumes an already produced
//! [`SegmentationOutcome`] and never derives regions, reads payload text, or
//! writes parser/graph state.  Visual tokens remain opaque and are counted as
//! evidence, not folded into prose.

use crate::{
    DocumentRegion, LayoutDocument, LayoutTokenKind, SegmentationOutcome, TokenAssignment,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

pub const NATIVE_BODY_SCHEMA_VERSION: &str = "da-parse.native-body-accounting/v1";
pub const MAX_ACCOUNTED_ASSIGNMENTS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountingBucket {
    BodyBlocks,
    BodyMarkers,
    VisualContent,
    Annex,
    References,
    PageFurniture,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountedKind {
    Lexical,
    Formula,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountedUnit {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub text_digest: String,
    pub region: DocumentRegion,
    pub bucket: AccountingBucket,
    pub kind: AccountedKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BucketCount {
    pub admitted: usize,
    pub conserved: usize,
    pub unsupported: usize,
    pub loss: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBodyAccounting {
    pub schema_version: String,
    pub document_id: String,
    pub source_sha256: String,
    pub provider_id: String,
    pub source_boundary_digest: String,
    pub admitted: usize,
    pub conserved: usize,
    pub unsupported: usize,
    pub loss: usize,
    pub buckets: BTreeMap<AccountingBucket, BucketCount>,
    pub units: Vec<AccountedUnit>,
    pub assignments: Vec<TokenAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum NativeBodyError {
    EmptyInput,
    TooManyAssignments { count: usize },
    DocumentMismatch,
    ProviderMismatch,
    SourceDigestMismatch,
    AssignmentCountMismatch { expected: usize, actual: usize },
    UnknownAssignment { page: u32, source: u32 },
    AssignmentDigestMismatch { page: u32, source: u32 },
    DuplicateAssignment { page: u32, source: u32 },
    MalformedOrder { page: u32, token: u32 },
}

impl fmt::Display for NativeBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native body accounting rejected input: {self:?}")
    }
}
impl std::error::Error for NativeBodyError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeBodyAccountant;

impl NativeBodyAccountant {
    pub const fn new() -> Self {
        Self
    }

    pub fn account(
        &self,
        document: &LayoutDocument,
        segmentation: &SegmentationOutcome,
    ) -> Result<NativeBodyAccounting, NativeBodyError> {
        let tokens = document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .collect::<Vec<_>>();
        if tokens.is_empty() || segmentation.assignments.is_empty() {
            return Err(NativeBodyError::EmptyInput);
        }
        if segmentation.assignments.len() > MAX_ACCOUNTED_ASSIGNMENTS {
            return Err(NativeBodyError::TooManyAssignments {
                count: segmentation.assignments.len(),
            });
        }
        if segmentation.document_id != document.document_id {
            return Err(NativeBodyError::DocumentMismatch);
        }
        let provider_id = tokens
            .first()
            .map(|t| t.provider.id.clone())
            .unwrap_or_default();
        if segmentation.provider_id != provider_id {
            return Err(NativeBodyError::ProviderMismatch);
        }
        if segmentation.assignments.len() != tokens.len() {
            return Err(NativeBodyError::AssignmentCountMismatch {
                expected: tokens.len(),
                actual: segmentation.assignments.len(),
            });
        }
        let source_sha256 = document.source_pdf_sha256.clone();
        if !is_sha256(&source_sha256) {
            return Err(NativeBodyError::SourceDigestMismatch);
        }

        let mut token_by_key = BTreeMap::new();
        for token in tokens {
            token_by_key.insert((token.page_ordinal, token.source_ordinal), token);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut units = Vec::with_capacity(segmentation.assignments.len());
        let mut buckets = BTreeMap::<AccountingBucket, BucketCount>::new();
        let mut previous = None;
        for assignment in &segmentation.assignments {
            let key = (assignment.page_ordinal, assignment.source_ordinal);
            if !seen.insert(key) {
                return Err(NativeBodyError::DuplicateAssignment {
                    page: key.0,
                    source: key.1,
                });
            }
            if let Some((page, ordinal)) = previous
                && (assignment.page_ordinal < page
                    || (assignment.page_ordinal == page && assignment.token_ordinal <= ordinal))
            {
                return Err(NativeBodyError::MalformedOrder {
                    page: assignment.page_ordinal,
                    token: assignment.token_ordinal,
                });
            }
            previous = Some((assignment.page_ordinal, assignment.token_ordinal));
            let token = token_by_key
                .get(&key)
                .ok_or(NativeBodyError::UnknownAssignment {
                    page: key.0,
                    source: key.1,
                })?;
            if token.token_ordinal != assignment.token_ordinal {
                return Err(NativeBodyError::UnknownAssignment {
                    page: key.0,
                    source: key.1,
                });
            }
            if token.text_digest != assignment.text_digest {
                return Err(NativeBodyError::AssignmentDigestMismatch {
                    page: key.0,
                    source: key.1,
                });
            }
            let kind = match token.kind {
                LayoutTokenKind::Formula => AccountedKind::Formula,
                LayoutTokenKind::Image => AccountedKind::Image,
                _ => AccountedKind::Lexical,
            };
            let bucket = bucket_for(assignment.region, kind);
            let count = buckets.entry(bucket).or_default();
            count.admitted += 1;
            count.conserved += 1;
            if bucket == AccountingBucket::Unsupported {
                count.unsupported += 1;
            }
            units.push(AccountedUnit {
                page_ordinal: assignment.page_ordinal,
                source_ordinal: assignment.source_ordinal,
                token_ordinal: assignment.token_ordinal,
                text_digest: assignment.text_digest.clone(),
                region: assignment.region,
                bucket,
                kind,
            });
        }
        let unsupported = buckets.values().map(|c| c.unsupported).sum();
        Ok(NativeBodyAccounting {
            schema_version: NATIVE_BODY_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            source_sha256,
            provider_id,
            source_boundary_digest: segmentation.boundary_digest.clone(),
            admitted: units.len(),
            conserved: units.len(),
            unsupported,
            loss: 0,
            buckets,
            units,
            assignments: segmentation.assignments.clone(),
        })
    }
}

fn bucket_for(region: DocumentRegion, kind: AccountedKind) -> AccountingBucket {
    if matches!(kind, AccountedKind::Formula | AccountedKind::Image)
        && region == DocumentRegion::Body
    {
        return AccountingBucket::VisualContent;
    }
    match region {
        DocumentRegion::Body => AccountingBucket::BodyBlocks,
        DocumentRegion::Annex => AccountingBucket::Annex,
        DocumentRegion::References => AccountingBucket::References,
        DocumentRegion::PageFurniture => AccountingBucket::PageFurniture,
        DocumentRegion::Header => AccountingBucket::Unsupported,
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn source_digest(document: &LayoutDocument) -> String {
    let mut hasher = Sha256::new();
    hasher.update(document.document_id.as_bytes());
    hasher.update(document.source_pdf_sha256.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}
