//! Accountant-bound semantic block sequence for the native shadow plane.
//!
//! This module is deliberately downstream of [`NativeBodyAccountant`].  It does
//! not read PDF/TEI/filesystem/network input and it does not segment regions.
//! The accountant receipt is the conservation authority; this module only
//! emits bounded, digest-only semantic roles for already admitted body units.

use crate::{
    FullSegmentationOutcome, FullSemanticRegion, LayoutDocument, LayoutLattice, LayoutTokenKind,
    LexemeRow, NativeBodyAccountant, NativeBodyError, NativeBodyLimits,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION: &str = "da-parse.native-fulltext-blocks/v1";
pub const NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION: &str =
    "da-parse.native-full-region-accounting/v1";
pub const MAX_FULLTEXT_BLOCKS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulltextLeafRole {
    Paragraph,
    Section,
    ListItem,
    Note,
    Marker,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerSubtype {
    Citation,
    Figure,
    Table,
    Equation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FulltextRoleHint {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub role: FulltextLeafRole,
    pub marker_subtype: Option<MarkerSubtype>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteDisposition {
    SourceNotEstablished,
    ReviewedAbsent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFulltextBlocksConfig {
    pub schema_version: String,
    pub config_fingerprint: String,
    pub role_hints: Vec<FulltextRoleHint>,
    pub note_disposition: NoteDisposition,
    pub max_blocks: usize,
}
impl Default for NativeFulltextBlocksConfig {
    fn default() -> Self {
        Self {
            schema_version: "fulltext-block-semantics.v1".to_owned(),
            config_fingerprint: "sha256:unconfigured".to_owned(),
            role_hints: Vec::new(),
            note_disposition: NoteDisposition::SourceNotEstablished,
            max_blocks: MAX_FULLTEXT_BLOCKS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullRegionAccountingUnit {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub text_digest: String,
    pub kind: FullRegionUnitKind,
    pub region: Option<FullSemanticRegion>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullRegionUnitKind {
    Lexical,
    Formula,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFullRegionAccounting {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub segmentation_config_fingerprint: String,
    pub admitted: usize,
    pub conserved: usize,
    pub loss: usize,
    pub visual_evidence: Vec<FullRegionAccountingUnit>,
    pub lexical_units: Vec<FullRegionAccountingUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnexParentRun {
    pub start_page: u32,
    pub start_block: u32,
    pub start_line: u32,
    pub start_token: u32,
    pub end_page: u32,
    pub end_block: u32,
    pub end_line: u32,
    pub end_token: u32,
    pub boundary_digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FulltextLeafAssignment {
    pub page_ordinal: u32,
    pub block_ordinal: u32,
    pub line_ordinal: u32,
    pub token_ordinal: u32,
    pub source_ordinal: u32,
    pub text_digest: String,
    pub role: FulltextLeafRole,
    pub marker_subtype: Option<MarkerSubtype>,
    pub parent_annex: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFulltextBlocksOutcome {
    pub schema_version: String,
    pub document_id: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
    pub annex_runs: Vec<AnnexParentRun>,
    pub leaves: Vec<FulltextLeafAssignment>,
    pub unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum NativeFulltextBlocksError {
    AccountantRejected,
    InvalidAccountantReceipt,
    IdentityMismatch(&'static str),
    InvalidConfig(&'static str),
    DuplicateHint { page: u32, source: u32 },
    UnknownHint { page: u32, source: u32 },
    MarkerSubtypeMissing,
    MarkerSubtypeForbidden,
    NotePositiveInvention,
    UnsupportedRole,
    Oversize { count: usize, max: usize },
    NonMonotonic,
    Serialization,
}
impl fmt::Display for NativeFulltextBlocksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native fulltext blocks rejected input: {self:?}")
    }
}
impl std::error::Error for NativeFulltextBlocksError {}

/// Prove conservation for S02's full-region outcome before any semantic work.
pub fn account_full_regions(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    segmentation: &FullSegmentationOutcome,
    limits: NativeBodyLimits,
) -> Result<NativeFullRegionAccounting, NativeFulltextBlocksError> {
    if segmentation.schema_version != crate::FULL_SEGMENTATION_SCHEMA_VERSION {
        return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
    }
    lattice
        .validate_against(document)
        .map_err(|_| NativeFulltextBlocksError::InvalidAccountantReceipt)?;
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeFulltextBlocksError::Serialization)?;
    if segmentation.identity.document_id != document.document_id
        || segmentation.identity.source_pdf_sha256 != document.source_pdf_sha256
        || segmentation.identity.lattice_fingerprint != lattice_fingerprint
    {
        return Err(NativeFulltextBlocksError::IdentityMismatch(
            "full_segmentation",
        ));
    }
    let lexical = segmentation
        .assignments
        .iter()
        .map(|a| (a.location.page_ordinal, a.source_ordinal))
        .collect::<BTreeSet<_>>();
    let mut lexical_units = Vec::new();
    let mut visual_evidence = Vec::new();
    let mut all = BTreeSet::new();
    for page in &document.pages {
        for token in &page.tokens {
            let key = (token.page_ordinal, token.source_ordinal);
            if !all.insert(key) {
                return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
            }
            let (kind, region) = if token.kind.is_visual() {
                (
                    if token.kind == LayoutTokenKind::Image {
                        FullRegionUnitKind::Image
                    } else {
                        FullRegionUnitKind::Formula
                    },
                    None,
                )
            } else {
                let assignment = segmentation
                    .assignments
                    .iter()
                    .find(|a| (a.location.page_ordinal, a.source_ordinal) == key)
                    .ok_or(NativeFulltextBlocksError::InvalidAccountantReceipt)?;
                (FullRegionUnitKind::Lexical, Some(assignment.region))
            };
            let unit = FullRegionAccountingUnit {
                page_ordinal: key.0,
                source_ordinal: key.1,
                token_ordinal: token.token_ordinal,
                text_digest: token.text_digest.clone(),
                kind,
                region,
            };
            if kind == FullRegionUnitKind::Lexical {
                lexical_units.push(unit);
            } else {
                visual_evidence.push(unit);
            }
        }
    }
    if lexical.len() != lexical_units.len() || segmentation.assignments.len() != lexical_units.len()
    {
        return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
    }
    // Keep the existing accountant as the only conservation implementation;
    // this call validates source identity and bounded assignment mechanics.
    let legacy = crate::SegmentationOutcome {
        schema_version: crate::NATIVE_SEGMENTATION_SCHEMA_VERSION.to_owned(),
        document_id: document.document_id.clone(),
        provider_id: segmentation.identity.provider_id.clone(),
        assignments: document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .map(|token| {
                let region = segmentation
                    .assignments
                    .iter()
                    .find(|a| {
                        a.location.page_ordinal == token.page_ordinal
                            && a.source_ordinal == token.source_ordinal
                    })
                    .map(|a| a.region)
                    .map(|r| match r {
                        FullSemanticRegion::Annex => crate::DocumentRegion::Annex,
                        FullSemanticRegion::References => crate::DocumentRegion::References,
                        FullSemanticRegion::PageFurniture => crate::DocumentRegion::PageFurniture,
                        _ => crate::DocumentRegion::Body,
                    })
                    .unwrap_or(crate::DocumentRegion::Body);
                crate::TokenAssignment {
                    page_ordinal: token.page_ordinal,
                    source_ordinal: token.source_ordinal,
                    token_ordinal: token.token_ordinal,
                    text_digest: token.text_digest.clone(),
                    region,
                }
            })
            .collect(),
        sections: Vec::new(),
        boundary_digest: segmentation.order_digest.clone(),
    };
    let accounting = NativeBodyAccountant::new()
        .account_with_limits(document, &legacy, limits)
        .map_err(|_: NativeBodyError| NativeFulltextBlocksError::AccountantRejected)?;
    if accounting.admitted != all.len() || accounting.conserved != all.len() || accounting.loss != 0
    {
        return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
    }
    Ok(NativeFullRegionAccounting {
        schema_version: NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION.to_owned(),
        document_id: document.document_id.clone(),
        provider_id: segmentation.identity.provider_id.clone(),
        source_pdf_sha256: document.source_pdf_sha256.clone(),
        lattice_fingerprint,
        segmentation_config_fingerprint: segmentation.identity.config_fingerprint.clone(),
        admitted: all.len(),
        conserved: all.len(),
        loss: 0,
        visual_evidence,
        lexical_units,
    })
}

pub fn run_native_fulltext_blocks(
    receipt: &NativeFullRegionAccounting,
    lattice: &LayoutLattice,
    lexemes: &[LexemeRow],
    config: &NativeFulltextBlocksConfig,
) -> Result<NativeFulltextBlocksOutcome, NativeFulltextBlocksError> {
    if receipt.schema_version != NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION
        || receipt.loss != 0
        || receipt.admitted != receipt.conserved
    {
        return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
    }
    let digest = lattice
        .digest()
        .map_err(|_| NativeFulltextBlocksError::Serialization)?;
    if digest != receipt.lattice_fingerprint {
        return Err(NativeFulltextBlocksError::IdentityMismatch("lattice"));
    }
    if config.max_blocks == 0 || config.max_blocks > MAX_FULLTEXT_BLOCKS {
        return Err(NativeFulltextBlocksError::InvalidConfig("max_blocks"));
    }
    let mut hints = BTreeMap::new();
    for hint in &config.role_hints {
        if hints
            .insert((hint.page_ordinal, hint.source_ordinal), hint)
            .is_some()
        {
            return Err(NativeFulltextBlocksError::DuplicateHint {
                page: hint.page_ordinal,
                source: hint.source_ordinal,
            });
        }
        match (hint.role, hint.marker_subtype) {
            (FulltextLeafRole::Marker, None) => {
                return Err(NativeFulltextBlocksError::MarkerSubtypeMissing);
            }
            (FulltextLeafRole::Marker, Some(_)) => {}
            (_, Some(_)) => return Err(NativeFulltextBlocksError::MarkerSubtypeForbidden),
            (FulltextLeafRole::Note, _) => {
                return Err(NativeFulltextBlocksError::NotePositiveInvention);
            }
            _ => {}
        }
    }
    let lexeme_keys: BTreeSet<_> = lexemes
        .iter()
        .map(|l| (l.page_ordinal, l.source_ordinal))
        .collect();
    let mut annex_runs = Vec::new();
    let mut leaves = Vec::new();
    let mut current_annex: Option<AnnexParentRun> = None;
    let mut parent_index = None;
    let mut previous = None;
    for unit in &receipt.lexical_units {
        if !lexeme_keys.contains(&(unit.page_ordinal, unit.source_ordinal)) {
            return Err(NativeFulltextBlocksError::InvalidAccountantReceipt);
        }
        let Some(region) = unit.region else { continue };
        if !matches!(region, FullSemanticRegion::Body | FullSemanticRegion::Annex) {
            continue;
        }
        let location = lattice
            .pages
            .iter()
            .find(|p| p.page_ordinal == unit.page_ordinal)
            .and_then(|p| {
                p.blocks.iter().find_map(|b| {
                    b.lines.iter().find_map(|l| {
                        l.tokens
                            .iter()
                            .find(|t| t.source_ordinal == unit.source_ordinal)
                            .map(|t| (b.source_ordinal, l.source_ordinal, t.token_ordinal))
                    })
                })
            })
            .ok_or(NativeFulltextBlocksError::InvalidAccountantReceipt)?;
        let key = (unit.page_ordinal, location.0, location.1, location.2);
        if previous.is_some_and(|p| key <= p) {
            return Err(NativeFulltextBlocksError::NonMonotonic);
        }
        previous = Some(key);
        if region == FullSemanticRegion::Annex {
            let start_new = current_annex.is_none();
            if start_new {
                parent_index = Some(annex_runs.len());
            }
            let run = current_annex.get_or_insert(AnnexParentRun {
                start_page: key.0,
                start_block: key.1,
                start_line: key.2,
                start_token: key.3,
                end_page: key.0,
                end_block: key.1,
                end_line: key.2,
                end_token: key.3,
                boundary_digest: String::new(),
            });
            run.end_page = key.0;
            run.end_block = key.1;
            run.end_line = key.2;
            run.end_token = key.3;
        } else if let Some(mut run) = current_annex.take() {
            run.boundary_digest = boundary(&run);
            annex_runs.push(run);
            parent_index = None;
        }
        let hint = hints.get(&(unit.page_ordinal, unit.source_ordinal));
        let (role, subtype) = hint
            .map(|h| (h.role, h.marker_subtype))
            .unwrap_or((FulltextLeafRole::Unsupported, None));
        leaves.push(FulltextLeafAssignment {
            page_ordinal: key.0,
            block_ordinal: key.1,
            line_ordinal: key.2,
            token_ordinal: key.3,
            source_ordinal: unit.source_ordinal,
            text_digest: unit.text_digest.clone(),
            role,
            marker_subtype: subtype,
            parent_annex: if region == FullSemanticRegion::Annex {
                parent_index
            } else {
                None
            },
        });
        if leaves.len() > config.max_blocks {
            return Err(NativeFulltextBlocksError::Oversize {
                count: leaves.len(),
                max: config.max_blocks,
            });
        }
    }
    if let Some(mut run) = current_annex.take() {
        run.boundary_digest = boundary(&run);
        annex_runs.push(run);
    }
    Ok(NativeFulltextBlocksOutcome {
        schema_version: NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION.to_owned(),
        document_id: receipt.document_id.clone(),
        lattice_fingerprint: receipt.lattice_fingerprint.clone(),
        config_fingerprint: config.config_fingerprint.clone(),
        unsupported: leaves
            .iter()
            .filter(|l| l.role == FulltextLeafRole::Unsupported)
            .count(),
        annex_runs,
        leaves,
    })
}

fn boundary(run: &AnnexParentRun) -> String {
    let mut h = Sha256::new();
    h.update(format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        run.start_page,
        run.start_block,
        run.start_line,
        run.start_token,
        run.end_page,
        run.end_block,
        run.end_line,
        run.end_token
    ));
    format!("sha256:{:x}", h.finalize())
}
