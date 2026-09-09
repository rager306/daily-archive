//! Accountant-bound specialized metadata companion for the offline shadow plane.
//!
//! This module is deliberately observation-only. It accepts validated receipts and
//! injected digest-only evidence; it performs no IO, parsing, segmentation, network
//! access, graph write, or production route selection.

use crate::{NativeFullRegionAccounting, NativeFulltextBlocksOutcome};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const NATIVE_CONSOLIDATION_SPECIALIZED_SCHEMA_VERSION: &str =
    "da-parse.native-consolidation-specialized/v1";
pub const MAX_SPECIALIZED_DIGESTS: usize = 10_000;
const FAMILIES: [SpecializedFamily; 6] = [
    SpecializedFamily::Funding,
    SpecializedFamily::Acknowledgement,
    SpecializedFamily::Licence,
    SpecializedFamily::Copyright,
    SpecializedFamily::Software,
    SpecializedFamily::Consolidation,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecializedFamily {
    Funding,
    Acknowledgement,
    Licence,
    Copyright,
    Software,
    Consolidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecializedDisposition {
    SourceNotEstablished,
    Unsupported,
    EnrichmentNotRequired,
    PropagatedTerminal,
    StructuralDefault,
    RichOptIn,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecializedUpstreamStatus {
    Pass,
    InvalidEvidence,
    TerminalFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestOnlyEnrichmentReceipt {
    pub profile: String,
    pub receipt_digest: String,
    pub evidence_digests: Vec<String>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecializedFamilyOutcome {
    pub family: SpecializedFamily,
    pub disposition: SpecializedDisposition,
    pub count: usize,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeConsolidationSpecializedOutcome {
    pub schema_version: String,
    pub document_id: String,
    pub lattice_fingerprint: String,
    pub source_pdf_sha256: String,
    pub upstream_status: SpecializedUpstreamStatus,
    pub enrichment_digest: Option<String>,
    pub families: Vec<SpecializedFamilyOutcome>,
    pub admitted: usize,
    pub conserved: usize,
    pub loss: usize,
    pub outcome_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeConsolidationSpecializedError {
    InvalidAccountantReceipt,
    IdentityMismatch(&'static str),
    InvalidEnrichment(&'static str),
    DuplicateDigest,
    Oversize { count: usize, max: usize },
}
impl fmt::Display for NativeConsolidationSpecializedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "native specialized consolidation rejected input: {self:?}"
        )
    }
}
impl std::error::Error for NativeConsolidationSpecializedError {}

fn digest(parts: impl IntoIterator<Item = String>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn validate_enrichment(
    enrichment: Option<&DigestOnlyEnrichmentReceipt>,
) -> Result<Option<String>, NativeConsolidationSpecializedError> {
    let Some(receipt) = enrichment else {
        return Ok(None);
    };
    if receipt.profile != "rich" {
        return Err(NativeConsolidationSpecializedError::InvalidEnrichment(
            "profile",
        ));
    }
    if receipt.receipt_digest.len() != 71 || !receipt.receipt_digest.starts_with("sha256:") {
        return Err(NativeConsolidationSpecializedError::InvalidEnrichment(
            "receipt_digest",
        ));
    }
    if receipt.evidence_digests.len() > MAX_SPECIALIZED_DIGESTS {
        return Err(NativeConsolidationSpecializedError::Oversize {
            count: receipt.evidence_digests.len(),
            max: MAX_SPECIALIZED_DIGESTS,
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for value in &receipt.evidence_digests {
        if value.len() != 71 || !value.starts_with("sha256:") || !seen.insert(value) {
            return Err(if value.len() == 71 && value.starts_with("sha256:") {
                NativeConsolidationSpecializedError::DuplicateDigest
            } else {
                NativeConsolidationSpecializedError::InvalidEnrichment("evidence_digest")
            });
        }
    }
    let expected = digest(receipt.evidence_digests.clone());
    if !receipt.timed_out && receipt.receipt_digest != expected {
        return Err(NativeConsolidationSpecializedError::InvalidEnrichment(
            "receipt_digest_mismatch",
        ));
    }
    Ok(Some(receipt.receipt_digest.clone()))
}

/// Evaluate specialized metadata over validated, identity-matched receipts only.
///
/// The function is intentionally bounded and deterministic: positive specialized
/// semantics are never invented, and terminal/invalid upstream evidence propagates
/// to every family instead of becoming an empty or successful result.
pub fn evaluate_native_consolidation_specialized(
    accounting: &NativeFullRegionAccounting,
    blocks: &NativeFulltextBlocksOutcome,
    upstream: SpecializedUpstreamStatus,
    enrichment: Option<&DigestOnlyEnrichmentReceipt>,
) -> Result<NativeConsolidationSpecializedOutcome, NativeConsolidationSpecializedError> {
    if accounting.schema_version != crate::NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION
        || accounting.loss != 0
        || accounting.admitted != accounting.conserved
    {
        return Err(NativeConsolidationSpecializedError::InvalidAccountantReceipt);
    }
    if accounting.document_id != blocks.document_id {
        return Err(NativeConsolidationSpecializedError::IdentityMismatch(
            "document_id",
        ));
    }
    if accounting.lattice_fingerprint != blocks.lattice_fingerprint
        || accounting.lattice_fingerprint.is_empty()
    {
        return Err(NativeConsolidationSpecializedError::IdentityMismatch(
            "lattice_fingerprint",
        ));
    }
    let enrichment_digest = validate_enrichment(enrichment)?;
    let propagation = matches!(
        upstream,
        SpecializedUpstreamStatus::InvalidEvidence | SpecializedUpstreamStatus::TerminalFailure
    );
    let mut families = Vec::with_capacity(FAMILIES.len());
    for family in FAMILIES {
        let disposition = if propagation {
            SpecializedDisposition::PropagatedTerminal
        } else {
            match family {
                SpecializedFamily::Consolidation
                    if enrichment.is_some_and(|receipt| receipt.timed_out) =>
                {
                    SpecializedDisposition::Unavailable
                }
                SpecializedFamily::Consolidation if enrichment.is_some() => {
                    SpecializedDisposition::RichOptIn
                }
                SpecializedFamily::Consolidation => SpecializedDisposition::StructuralDefault,
                SpecializedFamily::Funding | SpecializedFamily::Acknowledgement => {
                    SpecializedDisposition::SourceNotEstablished
                }
                _ => SpecializedDisposition::Unsupported,
            }
        };
        let count = 0;
        families.push(SpecializedFamilyOutcome {
            family,
            disposition,
            count,
            digest: digest([
                format!("{family:?}"),
                format!("{disposition:?}"),
                count.to_string(),
            ]),
        });
    }
    let outcome_digest = digest(families.iter().map(|item| {
        format!(
            "{:?}:{:?}:{}:{}",
            item.family, item.disposition, item.count, item.digest
        )
    }));
    Ok(NativeConsolidationSpecializedOutcome {
        schema_version: NATIVE_CONSOLIDATION_SPECIALIZED_SCHEMA_VERSION.to_owned(),
        document_id: accounting.document_id.clone(),
        lattice_fingerprint: accounting.lattice_fingerprint.clone(),
        source_pdf_sha256: accounting.source_pdf_sha256.clone(),
        upstream_status: upstream,
        enrichment_digest,
        families,
        admitted: accounting.admitted,
        conserved: accounting.conserved,
        loss: accounting.loss,
        outcome_digest,
    })
}
