//! Closed-vocabulary structural feature shadow for the Java tagger surface.
//!
//! This is an additive, pure companion. It consumes only already validated
//! lattice, segmentation, and conservation receipts. It never reads payload
//! sources, performs lexical lookup, constructs an engine, or owns persistence.

use crate::{
    FullRegionUnitKind, FullSegmentationOutcome, LayoutLattice, NativeFullRegionAccounting,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

pub const NATIVE_FEATURE_SCHEMA_VERSION: &str = "da-parse.native-feature-tagger/v1";
pub const NATIVE_FEATURE_COLUMNS: &[&str; 11] = &[
    "token",
    "line_status",
    "block_status",
    "font",
    "font_size",
    "relative_position",
    "capitalisation",
    "digitisation",
    "punctuation",
    "lexicon_flags",
    "label",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureDisposition {
    StructuralPass,
    UnsupportedLexiconBacked,
    PropagatedTerminal { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralFeatureRow {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub token_ordinal: u32,
    pub token: String,
    pub line_status: String,
    pub block_status: String,
    pub font: Option<String>,
    pub font_size: Option<String>,
    pub relative_position: String,
    pub capitalisation: FeatureDisposition,
    pub digitisation: FeatureDisposition,
    pub punctuation: FeatureDisposition,
    pub lexicon_flags: FeatureDisposition,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFeatureOutput {
    pub schema_version: String,
    pub document_id: String,
    pub lattice_fingerprint: String,
    pub rows: Vec<StructuralFeatureRow>,
    pub disposition: FeatureDisposition,
    pub admitted: usize,
    pub conserved: usize,
    pub loss: usize,
}
impl NativeFeatureOutput {
    pub const fn columns(&self) -> &'static [&'static str; 11] {
        NATIVE_FEATURE_COLUMNS
    }
}

#[derive(Debug, Clone)]
pub struct NativeFeatureInput<'a> {
    pub lattice: &'a LayoutLattice,
    pub segmentation: &'a FullSegmentationOutcome,
    pub accounting: &'a NativeFullRegionAccounting,
    pub upstream: FeatureUpstreamStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureUpstreamStatus {
    Pass,
    Terminal { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFeatureError {
    SchemaMismatch,
    IdentityMismatch(&'static str),
    AccountingMismatch,
    UnsupportedVisualAdmission,
    NonMonotonic,
    Serialization,
}
impl fmt::Display for NativeFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "native structural feature shadow rejected input: {self:?}"
        )
    }
}
impl std::error::Error for NativeFeatureError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFeatureTagger;

impl NativeFeatureTagger {
    pub const fn new() -> Self {
        Self
    }

    pub fn render(
        input: NativeFeatureInput<'_>,
    ) -> Result<NativeFeatureOutput, NativeFeatureError> {
        if input.segmentation.schema_version != crate::FULL_SEGMENTATION_SCHEMA_VERSION
            || input.accounting.schema_version
                != crate::NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION
        {
            return Err(NativeFeatureError::SchemaMismatch);
        }
        let lattice_fingerprint = input
            .lattice
            .digest()
            .map_err(|_| NativeFeatureError::Serialization)?;
        if input.segmentation.identity.lattice_fingerprint != lattice_fingerprint
            || input.accounting.lattice_fingerprint != lattice_fingerprint
        {
            return Err(NativeFeatureError::IdentityMismatch("lattice"));
        }
        if input.segmentation.identity.document_id != input.accounting.document_id
            || input.segmentation.identity.document_id != input.lattice.identity.document_id
            || input.segmentation.identity.source_pdf_sha256 != input.accounting.source_pdf_sha256
        {
            return Err(NativeFeatureError::IdentityMismatch("document"));
        }
        if input.accounting.loss != 0 || input.accounting.admitted != input.accounting.conserved {
            return Err(NativeFeatureError::AccountingMismatch);
        }
        validate_accounting_shape(&input)?;

        if let FeatureUpstreamStatus::Terminal { code } = &input.upstream {
            return Ok(NativeFeatureOutput {
                schema_version: NATIVE_FEATURE_SCHEMA_VERSION.into(),
                document_id: input.accounting.document_id.clone(),
                lattice_fingerprint,
                rows: Vec::new(),
                disposition: FeatureDisposition::PropagatedTerminal { code: code.clone() },
                admitted: 0,
                conserved: 0,
                loss: input.accounting.admitted,
            });
        }

        let mut rows = Vec::with_capacity(input.accounting.lexical_units.len());
        let mut previous = None;
        for unit in &input.accounting.lexical_units {
            if unit.kind != FullRegionUnitKind::Lexical {
                return Err(NativeFeatureError::UnsupportedVisualAdmission);
            }
            let location = lattice_location(input.lattice, unit.page_ordinal, unit.source_ordinal)
                .ok_or(NativeFeatureError::AccountingMismatch)?;
            let key = (location.0, location.1, location.2, location.3);
            if previous.is_some_and(|prior| key <= prior) {
                return Err(NativeFeatureError::NonMonotonic);
            }
            previous = Some(key);
            let assignment = input
                .segmentation
                .assignments
                .iter()
                .find(|a| {
                    a.location.page_ordinal == unit.page_ordinal
                        && a.source_ordinal == unit.source_ordinal
                })
                .ok_or(NativeFeatureError::AccountingMismatch)?;
            if assignment.text_digest != unit.text_digest {
                return Err(NativeFeatureError::AccountingMismatch);
            }
            rows.push(StructuralFeatureRow {
                page_ordinal: unit.page_ordinal,
                source_ordinal: unit.source_ordinal,
                token_ordinal: unit.token_ordinal,
                token: unit.text_digest.clone(),
                line_status: line_status(input.lattice, unit.page_ordinal, unit.source_ordinal),
                block_status: format_region(assignment.region),
                font: None,
                font_size: None,
                relative_position: relative_position(
                    input.lattice,
                    unit.page_ordinal,
                    unit.source_ordinal,
                ),
                capitalisation: FeatureDisposition::UnsupportedLexiconBacked,
                digitisation: FeatureDisposition::UnsupportedLexiconBacked,
                punctuation: FeatureDisposition::UnsupportedLexiconBacked,
                lexicon_flags: FeatureDisposition::UnsupportedLexiconBacked,
                label: None,
            });
        }
        Ok(NativeFeatureOutput {
            schema_version: NATIVE_FEATURE_SCHEMA_VERSION.into(),
            document_id: input.accounting.document_id.clone(),
            lattice_fingerprint,
            admitted: rows.len(),
            conserved: rows.len(),
            loss: 0,
            disposition: FeatureDisposition::StructuralPass,
            rows,
        })
    }
}

fn validate_accounting_shape(input: &NativeFeatureInput<'_>) -> Result<(), NativeFeatureError> {
    let lexical = input
        .accounting
        .lexical_units
        .iter()
        .filter(|u| u.kind == FullRegionUnitKind::Lexical)
        .count();
    let visual = input.accounting.visual_evidence.len();
    if lexical + visual != input.accounting.admitted
        || visual
            != input
                .lattice
                .pages
                .iter()
                .flat_map(|p| p.blocks.iter())
                .flat_map(|b| b.lines.iter())
                .flat_map(|l| l.tokens.iter())
                .filter(|t| t.kind.is_visual())
                .count()
    {
        return Err(NativeFeatureError::AccountingMismatch);
    }
    let keys: BTreeSet<_> = input
        .accounting
        .lexical_units
        .iter()
        .map(|u| (u.page_ordinal, u.source_ordinal))
        .collect();
    if keys.len() != lexical {
        return Err(NativeFeatureError::AccountingMismatch);
    }
    Ok(())
}

fn lattice_location(
    lattice: &LayoutLattice,
    page: u32,
    source: u32,
) -> Option<(u32, u32, u32, u32)> {
    lattice
        .pages
        .iter()
        .find(|p| p.page_ordinal == page)
        .and_then(|p| {
            p.blocks.iter().find_map(|b| {
                b.lines.iter().find_map(|l| {
                    l.tokens
                        .iter()
                        .find(|t| t.source_ordinal == source)
                        .map(|t| {
                            (
                                p.page_ordinal,
                                b.source_ordinal,
                                l.source_ordinal,
                                t.token_ordinal,
                            )
                        })
                })
            })
        })
}
fn line_status(lattice: &LayoutLattice, page: u32, source: u32) -> String {
    lattice
        .pages
        .iter()
        .find(|p| p.page_ordinal == page)
        .and_then(|p| {
            p.blocks.iter().find_map(|b| {
                b.lines
                    .iter()
                    .find(|l| l.tokens.iter().any(|t| t.source_ordinal == source))
                    .map(|l| {
                        if l.newline_boundary_count > 0 {
                            "line_boundary"
                        } else {
                            "line_continuation"
                        }
                    })
            })
        })
        .unwrap_or("unknown")
        .into()
}
fn relative_position(lattice: &LayoutLattice, page: u32, source: u32) -> String {
    lattice_location(lattice, page, source)
        .map(|(_, b, l, t)| format!("{b}:{l}:{t}"))
        .unwrap_or_else(|| "unknown".into())
}
fn format_region(region: crate::FullSemanticRegion) -> String {
    format!("{region:?}").to_ascii_lowercase()
}
