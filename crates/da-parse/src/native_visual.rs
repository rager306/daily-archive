//! Pure, bounded native visual heads for shadow evaluation.
//!
//! The module deliberately owns only digest and geometry receipts. It has no
//! filesystem, network, process, clock, tracing, adapter, or fulltext
//! dependencies. Each head is callable independently and reports its own
//! conservation counters.

use crate::LayoutBBox;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

pub const NATIVE_VISUAL_SCHEMA_VERSION: &str = "da-parse.native-visual/v1";
pub const NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-visual-sequence/v1";
pub const MAX_PRIMITIVES: usize = 16_384;
pub const MAX_RECEIPTS: usize = 4_096;
pub const MAX_DIGEST_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualKind {
    Image,
    Graphic,
    Table,
    Formula,
    Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualAbsence {
    Unsupported,
    NotObserved,
    MissingIdentity,
    ChecksumMismatch,
    MalformedBbox,
    InvalidDigest,
    DuplicateReceipt,
    MissingReceipt,
    Oversize,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualProvider {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeVisualPrimitive {
    pub document_id: String,
    pub source_sha256: String,
    pub provider: NativeVisualProvider,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub kind: NativeVisualKind,
    pub bbox: Option<LayoutBBox>,
    pub content_digest: Option<String>,
    pub absence: Option<NativeVisualAbsence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualDigestReceipt {
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub digest: Option<String>,
    pub absence: Option<NativeVisualAbsence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTableCellReceipt {
    pub row: u16,
    pub column: u16,
    pub digest: Option<String>,
    pub absence: Option<NativeVisualAbsence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeVisualObservation {
    pub schema_version: String,
    pub document_id: String,
    pub source_sha256: String,
    pub provider: NativeVisualProvider,
    pub primitives: Vec<NativeVisualPrimitive>,
    pub figure_captions: Vec<NativeVisualDigestReceipt>,
    pub figure_callouts: Vec<NativeVisualDigestReceipt>,
    pub table_captions: Vec<NativeVisualDigestReceipt>,
    pub table_callouts: Vec<NativeVisualDigestReceipt>,
    pub table_cells: Vec<NativeTableCellReceipt>,
    pub formula_labels: Vec<NativeVisualDigestReceipt>,
    pub formula_callouts: Vec<NativeVisualDigestReceipt>,
    pub lane_absence: Option<NativeVisualAbsence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualConfig {
    pub max_primitives: usize,
    pub max_receipts: usize,
    pub max_heads_per_page: usize,
}
impl NativeVisualConfig {
    pub const fn new(
        max_primitives: usize,
        max_receipts: usize,
        max_heads_per_page: usize,
    ) -> Self {
        Self {
            max_primitives,
            max_receipts,
            max_heads_per_page,
        }
    }
}
impl Default for NativeVisualConfig {
    fn default() -> Self {
        Self::new(MAX_PRIMITIVES, MAX_RECEIPTS, 128)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualHead {
    Figure,
    Table,
    Formula,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualMetrics {
    pub admitted: u32,
    pub terminal: u32,
    pub caption_conservation: u32,
    pub content_conservation: u32,
    pub callout_conservation: u32,
    pub geometry_conservation: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureHeadReceipt {
    pub head_ordinal: u32,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub bbox: LayoutBBox,
    pub content_digest: String,
    pub caption_digest: Option<String>,
    pub callout_digest: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableHeadReceipt {
    pub head_ordinal: u32,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub bbox: LayoutBBox,
    pub content_digest: String,
    pub caption_digest: Option<String>,
    pub callout_digest: Option<String>,
    pub cells: Vec<NativeTableCellReceipt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormulaHeadReceipt {
    pub head_ordinal: u32,
    pub page_ordinal: u32,
    pub source_ordinal: u32,
    pub bbox: LayoutBBox,
    pub content_digest: String,
    pub equation_label_digest: Option<String>,
    pub callout_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NativeVisualHeadOutcome {
    Figure {
        receipts: Vec<FigureHeadReceipt>,
        metrics: NativeVisualMetrics,
    },
    Table {
        receipts: Vec<TableHeadReceipt>,
        metrics: NativeVisualMetrics,
    },
    Formula {
        receipts: Vec<FormulaHeadReceipt>,
        metrics: NativeVisualMetrics,
    },
    Terminal {
        reason: NativeVisualAbsence,
        metrics: NativeVisualMetrics,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualSequenceIdentity {
    pub document_id: String,
    pub source_sha256: String,
    pub provider_id: String,
    pub visual_schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeVisualSequenceStep {
    pub name: String,
    pub status: String,
    pub duration_ms: u64,
    pub failure_code: Option<String>,
    pub outcome: Option<NativeVisualHeadOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeVisualSequenceReceipt {
    pub schema_version: String,
    pub status: String,
    pub identity: NativeVisualSequenceIdentity,
    pub steps: Vec<NativeVisualSequenceStep>,
    pub java_fallback: NativeVisualJavaFallback,
    pub redaction: std::collections::BTreeMap<String, bool>,
}

/// Fixed Java evidence retained while native visual heads remain shadow-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualJavaFallback {
    pub disposition: String,
    pub target: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeVisualError {
    InvalidIdentity(&'static str),
    InvalidDigest,
    InvalidGeometry,
    DuplicatePrimitive,
    DuplicateReceipt,
    Oversize(&'static str),
    InvalidSchema,
    HeadLimit,
    ReceiptMismatch,
}
impl fmt::Display for NativeVisualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid native visual input: {self:?}")
    }
}
impl std::error::Error for NativeVisualError {}

impl NativeVisualObservation {
    pub fn validate(&self, config: NativeVisualConfig) -> Result<(), NativeVisualError> {
        if self.schema_version != NATIVE_VISUAL_SCHEMA_VERSION {
            return Err(NativeVisualError::InvalidSchema);
        }
        if self.document_id.is_empty()
            || self.provider.id.is_empty()
            || self.provider.version.is_empty()
        {
            return Err(NativeVisualError::InvalidIdentity("identity"));
        }
        if !sha256(&self.source_sha256) {
            return Err(NativeVisualError::InvalidDigest);
        }
        if self.primitives.len() > config.max_primitives {
            return Err(NativeVisualError::Oversize("primitives"));
        }
        if self.lane_absence.is_some() && !self.primitives.is_empty() {
            return Err(NativeVisualError::InvalidIdentity("lane_absence"));
        }
        let mut ids = BTreeSet::new();
        for p in &self.primitives {
            if p.document_id != self.document_id
                || p.source_sha256 != self.source_sha256
                || p.provider != self.provider
            {
                return Err(NativeVisualError::InvalidIdentity("primitive lineage"));
            }
            if !ids.insert((p.page_ordinal, p.source_ordinal, p.kind as u8)) {
                return Err(NativeVisualError::DuplicatePrimitive);
            }
            match (p.bbox, p.content_digest.as_deref(), p.absence) {
                (Some(b), Some(d), None) if valid_bbox(b) && sha256(d) => {}
                (None, None, Some(_)) => {}
                _ => return Err(NativeVisualError::InvalidGeometry),
            }
        }
        let lists = [
            &self.figure_captions,
            &self.figure_callouts,
            &self.table_captions,
            &self.table_callouts,
            &self.formula_labels,
            &self.formula_callouts,
        ];
        if lists.iter().map(|x| x.len()).sum::<usize>() + self.table_cells.len()
            > config.max_receipts
        {
            return Err(NativeVisualError::Oversize("receipts"));
        }
        for list in lists {
            validate_digest_receipts(list)?;
        }
        let mut cell_ids = BTreeSet::new();
        for c in &self.table_cells {
            if !cell_ids.insert((c.row, c.column)) {
                return Err(NativeVisualError::DuplicateReceipt);
            }
            if !c.digest.as_deref().is_some_and(sha256) && c.absence.is_none() {
                return Err(NativeVisualError::InvalidDigest);
            }
        }
        Ok(())
    }
}

fn validate_digest_receipts(items: &[NativeVisualDigestReceipt]) -> Result<(), NativeVisualError> {
    let mut ids = BTreeSet::new();
    for item in items {
        if !ids.insert((item.page_ordinal, item.source_ordinal)) {
            return Err(NativeVisualError::DuplicateReceipt);
        }
        if let Some(d) = &item.digest
            && !sha256(d)
        {
            return Err(NativeVisualError::InvalidDigest);
        }
        if item.digest.is_none() && item.absence.is_none() {
            return Err(NativeVisualError::ReceiptMismatch);
        }
    }
    Ok(())
}
fn sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_bbox(b: LayoutBBox) -> bool {
    b.x_min.is_finite()
        && b.y_min.is_finite()
        && b.x_max.is_finite()
        && b.y_max.is_finite()
        && b.x_min >= 0.0
        && b.y_min >= 0.0
        && b.x_min < b.x_max
        && b.y_min < b.y_max
}

fn head_items<'a>(
    input: &'a NativeVisualObservation,
    kinds: &[NativeVisualKind],
    config: NativeVisualConfig,
) -> Result<Vec<&'a NativeVisualPrimitive>, NativeVisualError> {
    input.validate(config)?;
    if input.lane_absence.is_some() {
        return Ok(Vec::new());
    }
    let mut per_page = std::collections::BTreeMap::new();
    let mut result = input
        .primitives
        .iter()
        .filter(|p| kinds.contains(&p.kind))
        .collect::<Vec<_>>();
    result.sort_by_key(|p| (p.page_ordinal, p.source_ordinal));
    for p in &result {
        let count = per_page.entry(p.page_ordinal).or_insert(0usize);
        *count += 1;
        if *count > config.max_heads_per_page {
            return Err(NativeVisualError::HeadLimit);
        }
    }
    Ok(result)
}
fn digest_at(items: &[NativeVisualDigestReceipt], p: &NativeVisualPrimitive) -> Option<String> {
    items
        .iter()
        .find(|r| r.page_ordinal == p.page_ordinal && r.source_ordinal == p.source_ordinal)
        .and_then(|r| r.digest.clone())
}
fn not_observed() -> NativeVisualHeadOutcome {
    NativeVisualHeadOutcome::Terminal {
        reason: NativeVisualAbsence::NotObserved,
        metrics: metrics(0, 1, 0, 0, 0),
    }
}
fn terminal_if_absent(
    input: &NativeVisualObservation,
    no_items: bool,
) -> Option<NativeVisualHeadOutcome> {
    if let Some(reason) = input.lane_absence {
        return Some(NativeVisualHeadOutcome::Terminal {
            reason,
            metrics: metrics(0, 1, 0, 0, 0),
        });
    }
    no_items.then(not_observed)
}
fn metrics(
    admitted: usize,
    terminal: usize,
    captions: usize,
    content: usize,
    callouts: usize,
) -> NativeVisualMetrics {
    NativeVisualMetrics {
        admitted: admitted as u32,
        terminal: terminal as u32,
        caption_conservation: captions as u32,
        content_conservation: content as u32,
        callout_conservation: callouts as u32,
        geometry_conservation: admitted as u32,
    }
}

pub fn detect_figure_heads(
    input: &NativeVisualObservation,
    config: NativeVisualConfig,
) -> Result<NativeVisualHeadOutcome, NativeVisualError> {
    let items = head_items(
        input,
        &[NativeVisualKind::Image, NativeVisualKind::Graphic],
        config,
    )?;
    if let Some(outcome) = terminal_if_absent(input, items.is_empty()) {
        return Ok(outcome);
    }
    let mut receipts = Vec::with_capacity(items.len());
    for (n, p) in items.iter().enumerate() {
        let (Some(bbox), Some(content_digest)) = (p.bbox, p.content_digest.clone()) else {
            continue;
        };
        receipts.push(FigureHeadReceipt {
            head_ordinal: n as u32,
            page_ordinal: p.page_ordinal,
            source_ordinal: p.source_ordinal,
            bbox,
            content_digest,
            caption_digest: digest_at(&input.figure_captions, p),
            callout_digest: digest_at(&input.figure_callouts, p),
        });
    }
    if receipts.is_empty() {
        return Ok(not_observed());
    }
    let captions = receipts
        .iter()
        .filter(|r| r.caption_digest.is_some())
        .count();
    let callouts = receipts
        .iter()
        .filter(|r| r.callout_digest.is_some())
        .count();
    Ok(NativeVisualHeadOutcome::Figure {
        metrics: metrics(
            receipts.len(),
            items.len() - receipts.len(),
            captions,
            receipts.len(),
            callouts,
        ),
        receipts,
    })
}
pub fn detect_table_heads(
    input: &NativeVisualObservation,
    config: NativeVisualConfig,
) -> Result<NativeVisualHeadOutcome, NativeVisualError> {
    let items = head_items(
        input,
        &[NativeVisualKind::Table, NativeVisualKind::Rect],
        config,
    )?;
    if let Some(outcome) = terminal_if_absent(input, items.is_empty()) {
        return Ok(outcome);
    }
    let mut receipts = Vec::with_capacity(items.len());
    for (n, p) in items.iter().enumerate() {
        let (Some(bbox), Some(content_digest)) = (p.bbox, p.content_digest.clone()) else {
            continue;
        };
        receipts.push(TableHeadReceipt {
            head_ordinal: n as u32,
            page_ordinal: p.page_ordinal,
            source_ordinal: p.source_ordinal,
            bbox,
            content_digest,
            caption_digest: digest_at(&input.table_captions, p),
            callout_digest: digest_at(&input.table_callouts, p),
            // NativeTableCellReceipt has no source identity. Keep the
            // sibling population observation-level and serialize it once on
            // the first stable table receipt rather than guessing ownership
            // or multiplying logical cells across heads.
            cells: if n == 0 {
                input.table_cells.clone()
            } else {
                Default::default()
            },
        });
    }
    if receipts.is_empty() {
        return Ok(not_observed());
    }
    let captions = receipts
        .iter()
        .filter(|r| r.caption_digest.is_some())
        .count();
    let callouts = receipts
        .iter()
        .filter(|r| r.callout_digest.is_some())
        .count();
    Ok(NativeVisualHeadOutcome::Table {
        metrics: metrics(
            receipts.len(),
            items.len() - receipts.len(),
            captions,
            receipts.len(),
            callouts,
        ),
        receipts,
    })
}
pub fn detect_formula_heads(
    input: &NativeVisualObservation,
    config: NativeVisualConfig,
) -> Result<NativeVisualHeadOutcome, NativeVisualError> {
    let items = head_items(input, &[NativeVisualKind::Formula], config)?;
    if let Some(outcome) = terminal_if_absent(input, items.is_empty()) {
        return Ok(outcome);
    }
    let mut receipts = Vec::with_capacity(items.len());
    for (n, p) in items.iter().enumerate() {
        let (Some(bbox), Some(content_digest)) = (p.bbox, p.content_digest.clone()) else {
            continue;
        };
        receipts.push(FormulaHeadReceipt {
            head_ordinal: n as u32,
            page_ordinal: p.page_ordinal,
            source_ordinal: p.source_ordinal,
            bbox,
            content_digest,
            equation_label_digest: digest_at(&input.formula_labels, p),
            callout_digest: digest_at(&input.formula_callouts, p),
        });
    }
    if receipts.is_empty() {
        return Ok(not_observed());
    }
    let labels = receipts
        .iter()
        .filter(|r| r.equation_label_digest.is_some())
        .count();
    let callouts = receipts
        .iter()
        .filter(|r| r.callout_digest.is_some())
        .count();
    Ok(NativeVisualHeadOutcome::Formula {
        metrics: metrics(
            receipts.len(),
            items.len() - receipts.len(),
            labels,
            receipts.len(),
            callouts,
        ),
        receipts,
    })
}

fn native_visual_error_code(error: &NativeVisualError) -> &'static str {
    match error {
        NativeVisualError::InvalidIdentity(_) => "invalid_identity",
        NativeVisualError::InvalidDigest => "invalid_digest",
        NativeVisualError::InvalidGeometry => "invalid_geometry",
        NativeVisualError::DuplicatePrimitive => "duplicate_primitive",
        NativeVisualError::DuplicateReceipt => "duplicate_receipt",
        NativeVisualError::Oversize(_) => "oversize",
        NativeVisualError::InvalidSchema => "invalid_schema",
        NativeVisualError::HeadLimit => "head_limit",
        NativeVisualError::ReceiptMismatch => "receipt_mismatch",
    }
}

fn visual_redaction() -> std::collections::BTreeMap<String, bool> {
    [
        ("raw_text", false),
        ("raw_pdf", false),
        ("physical_paths", false),
        ("hostname", false),
        ("executable", false),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
    .collect()
}

fn sequence_step(
    name: &str,
    status: &str,
    failure_code: Option<&str>,
    outcome: Option<NativeVisualHeadOutcome>,
) -> NativeVisualSequenceStep {
    NativeVisualSequenceStep {
        name: name.to_owned(),
        status: status.to_owned(),
        duration_ms: 0,
        failure_code: failure_code.map(str::to_owned),
        outcome,
    }
}

/// Run the three independent visual heads in a deterministic shadow sequence.
///
/// Validation is the sole admission gate. Typed head absence is a completed
/// sibling step; only a `NativeVisualError` stops later heads.
pub fn run_visual_heads_sequence(
    input: &NativeVisualObservation,
    config: &NativeVisualConfig,
) -> NativeVisualSequenceReceipt {
    let identity = NativeVisualSequenceIdentity {
        document_id: input.document_id.clone(),
        source_sha256: input.source_sha256.clone(),
        provider_id: input.provider.id.clone(),
        visual_schema_version: input.schema_version.clone(),
    };
    let mut steps = Vec::with_capacity(4);
    let status = if let Err(error) = input.validate(*config) {
        let code = native_visual_error_code(&error);
        steps.push(sequence_step(
            "validate_observation",
            "terminal",
            Some(code),
            None,
        ));
        for name in ["figure", "table", "formula"] {
            steps.push(sequence_step(
                name,
                "not_admitted",
                Some("validate_observation_terminal_failure"),
                None,
            ));
        }
        "terminal_failure"
    } else {
        steps.push(sequence_step("validate_observation", "pass", None, None));
        let mut sequence_status = "pass";
        let mut stop_code = None;
        for (name, detector) in [
            (
                "figure",
                detect_figure_heads as fn(&NativeVisualObservation, NativeVisualConfig) -> _,
            ),
            ("table", detect_table_heads),
            ("formula", detect_formula_heads),
        ] {
            if let Some(code) = stop_code {
                steps.push(sequence_step(name, "not_admitted", Some(code), None));
                continue;
            }
            match detector(input, *config) {
                Ok(outcome @ NativeVisualHeadOutcome::Terminal { .. }) => {
                    steps.push(sequence_step(name, "terminal", None, Some(outcome)));
                }
                Ok(outcome) => steps.push(sequence_step(name, "pass", None, Some(outcome))),
                Err(error) => {
                    let code = native_visual_error_code(&error);
                    steps.push(sequence_step(name, "terminal", Some(code), None));
                    sequence_status = "terminal_failure";
                    stop_code = Some(match name {
                        "figure" => "figure_terminal_failure",
                        "table" => "table_terminal_failure",
                        _ => "formula_terminal_failure",
                    });
                }
            }
        }
        sequence_status
    };
    NativeVisualSequenceReceipt {
        schema_version: NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION.to_owned(),
        status: status.to_owned(),
        identity,
        steps,
        java_fallback: NativeVisualJavaFallback {
            disposition: "preserved_evidence_only".to_owned(),
            target: "java_grobid".to_owned(),
            source: "production_fallback".to_owned(),
        },
        redaction: visual_redaction(),
    }
}
