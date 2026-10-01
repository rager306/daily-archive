//! Additive native visual-family band (S05/T03): the visual heads run on the S03
//! native plane instead of on a donor observation.
//!
//! S03 publishes the positioned layout plane ([`LayoutDocument`],
//! [`LayoutLattice`]) and the semantic segmentation
//! ([`FullSegmentationOutcome`]); the pure head detectors of
//! [`crate::native_visual`] already conserve figure/table/formula receipts, but
//! nothing in production *feeds* them from the native plane. Until now the only
//! feed was the adapter donor (`da_adapters::VisualObservation`, an out-of-crate
//! surface) and the committed digest-only oracle was all-absence,
//! so evaluating against it was vacuous: absence trivially matched absence.
//!
//! This module is the missing input band plus the non-vacuous comparison.
//!
//! ## What it builds
//!
//! [`build_native_visual_observation`] turns the S03 plane into a
//! [`NativeVisualObservation`] using **only** the plane's own facts:
//!
//! * geometry is the `LayoutToken`/`LatticeToken` bounding box;
//! * the visual kind is the `LayoutTokenKind` itself (`Image` → figure,
//!   `Formula` → formula). No lexical string can manufacture a visual kind, so
//!   the donor's word-only vocabulary cannot fabricate a head;
//! * a table is a *documented, conservative* grid heuristic over lattice
//!   blocks: at least [`NATIVE_VISUAL_TABLE_MIN_ROWS`] lines, each carrying at
//!   least [`NATIVE_VISUAL_TABLE_MIN_COLUMNS`] cells separated by
//!   [`NATIVE_VISUAL_TABLE_CELL_GAP_PT`] points, aligned within
//!   [`NATIVE_VISUAL_TABLE_ANCHOR_TOLERANCE_PT`] points across rows. Ordinary
//!   prose lines form a single cell and are never a table.
//!
//! No `da_adapters::VisualObservation`, no TEI and no Java type is an input
//! anywhere in this file.
//!
//! ## Honest dispositions, never a quality claim
//!
//! [`evaluate_visual_family_rows`] compares the native band against an
//! independent oracle that must carry **positive** per-family rows. Each
//! document × family gets exactly one closed disposition — `measured`,
//! `unmet` (with a typed reason) or `not_observed` — and `out_of_scope` is
//! *structurally absent* from the vocabulary: an unsupported required family is
//! a typed `unmet`, never a silent scope reduction.
//!
//! The oracle itself is guarded: a same-generator oracle is refused as
//! `NotIndependent`, and an oracle without positive rows for a declared family
//! is refused as `OracleVacuous` (it can only agree with absence by
//! construction).
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, environment, clock, network, logging, or
//! process surface. Nothing here is spliced into the Java-authoritative parse
//! route; `import_eligible` stays false, no threshold is accepted and no
//! promotion happens.

use crate::native_heading_regions::MAX_NATIVE_VISUAL_LINES;
use crate::native_visual::{
    NATIVE_VISUAL_SCHEMA_VERSION, NativeVisualAbsence, NativeVisualConfig, NativeVisualError,
    NativeVisualHeadOutcome, NativeVisualKind, NativeVisualObservation, NativeVisualPrimitive,
    NativeVisualProvider, detect_figure_heads, detect_formula_heads, detect_table_heads,
};
use crate::{
    FULL_SEGMENTATION_SCHEMA_VERSION, FullSegmentationOutcome, LayoutDocument, LayoutLattice,
    LayoutTokenKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the additive native visual-input surface.
pub const NATIVE_VISUAL_NATIVE_INPUT_SCHEMA_VERSION: &str =
    "da-parse.native-visual-native-input/v1";

/// Generator identity of this band. An independent oracle must not declare it
/// (see [`require_independent_visual_oracle`]).
pub const NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID: &str = "da-parse.native-visual-native-input";

/// The closed visual-family vocabulary. Mirrors the head labels of
/// `data/parse/native_visual_heads.yaml#heads.closed_labels`.
pub const NATIVE_VISUAL_FAMILY_VOCABULARY: [&str; 3] = ["figure", "table", "formula"];

/// Closed disposition vocabulary of one required family. `out_of_scope` is
/// deliberately not a member: an unsupported required family is `unmet`.
pub const NATIVE_VISUAL_FAMILY_DISPOSITION_VOCABULARY: [&str; 3] =
    ["measured", "unmet", "not_observed"];

/// Minimum number of stacked lines before a lattice block can be a table grid.
pub const NATIVE_VISUAL_TABLE_MIN_ROWS: usize = 2;
/// Minimum number of horizontally separated cells in a table row.
pub const NATIVE_VISUAL_TABLE_MIN_COLUMNS: usize = 2;
/// Minimum horizontal gap (points) between two cells of the same row.
pub const NATIVE_VISUAL_TABLE_CELL_GAP_PT: f64 = 12.0;
/// Maximum anchor drift (points) tolerated between the cells of two rows.
pub const NATIVE_VISUAL_TABLE_ANCHOR_TOLERANCE_PT: f64 = 6.0;

/// The closed visual-family vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualFamily {
    Figure,
    Table,
    Formula,
}

impl NativeVisualFamily {
    /// Every family, in declaration order.
    pub const ALL: [Self; 3] = [Self::Figure, Self::Table, Self::Formula];

    /// The stable `snake_case` token this family serializes to.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Figure => "figure",
            Self::Table => "table",
            Self::Formula => "formula",
        }
    }
}

/// Configuration of the native visual-input build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualFromInputConfig {
    /// Upper bound on lattice lines scanned; mirrors the heading walk's visual
    /// line ceiling.
    pub max_lines: usize,
    /// Detector configuration (primitive/receipt/per-page ceilings).
    pub visual: NativeVisualConfig,
}

impl NativeVisualFromInputConfig {
    pub const fn new(max_lines: usize, visual: NativeVisualConfig) -> Self {
        Self { max_lines, visual }
    }
}

impl Default for NativeVisualFromInputConfig {
    fn default() -> Self {
        Self::new(MAX_NATIVE_VISUAL_LINES, NativeVisualConfig::default())
    }
}

/// Provenance identity of the native plane the outcome was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualInputIdentity {
    pub schema_version: String,
    pub document_id: String,
    pub provider_id: String,
    pub source_pdf_sha256: String,
    pub lattice_fingerprint: String,
    pub config_fingerprint: String,
    /// The S03 `FullSegmentationOutcome::order_digest`.
    pub order_digest: String,
}

/// One family's head outcome on the native plane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualHeadObservation {
    pub family: NativeVisualFamily,
    pub outcome: NativeVisualHeadOutcome,
}

/// Redaction-safe result of running the three visual heads on the native plane.
///
/// `observation` is `None` exactly when the document carries no tokens at all:
/// a typed absence, never a zero-length observation. `heads` is then empty and
/// every family is an absence, not a measured zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualFromInputOutcome {
    pub schema_version: String,
    pub identity: NativeVisualInputIdentity,
    pub observation: Option<NativeVisualObservation>,
    /// Typed absence of the whole lane when `observation` is `None`.
    pub absence: Option<NativeVisualAbsence>,
    pub heads: Vec<NativeVisualHeadObservation>,
}

impl NativeVisualFromInputOutcome {
    /// The head observation of one family, when the plane was admitted.
    pub fn head(&self, family: NativeVisualFamily) -> Option<&NativeVisualHeadObservation> {
        self.heads.iter().find(|head| head.family == family)
    }

    /// Admitted head count of one family; `0` for an absent plane or a terminal.
    pub fn admitted_heads(&self, family: NativeVisualFamily) -> usize {
        match self.head(family).map(|head| &head.outcome) {
            Some(NativeVisualHeadOutcome::Figure { metrics, .. }) => metrics.admitted as usize,
            Some(NativeVisualHeadOutcome::Table { metrics, .. }) => metrics.admitted as usize,
            Some(NativeVisualHeadOutcome::Formula { metrics, .. }) => metrics.admitted as usize,
            Some(NativeVisualHeadOutcome::Terminal { .. }) | None => 0,
        }
    }
}

/// Fail-closed refusal vocabulary of the native visual-input build.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeVisualInputError {
    /// The layout document failed its own contract.
    LayoutInvalid,
    /// The lattice does not validate against the layout document.
    LatticeInvalid,
    /// A segmentation identity field drifted (`schema_version`,
    /// `document_id`, `source_pdf_sha256`, `lattice_fingerprint`,
    /// `provider_id`).
    IdentityMismatch(&'static str),
    /// More lattice lines than the configured budget.
    LineLimit { count: usize },
    /// A visual layout token's text digest is not a sha256.
    InvalidDigest { page: u32, source: u32 },
    /// A table block's derived content digest could not be formed.
    InvalidTableDigest { page: u32, block: u32 },
    /// Two primitives declared the same `(page, source, kind)` identity.
    DuplicatePrimitive { page: u32, source: u32 },
    /// The head detector refused the built observation.
    NativeVisual(NativeVisualError),
    /// Canonical serialization failed.
    Serialization,
}

impl fmt::Display for NativeVisualInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native visual input rejected the plane: {self:?}"
        )
    }
}

impl std::error::Error for NativeVisualInputError {}

/// Build the native visual observation from the S03 plane.
///
/// Only `LayoutDocument` + `LayoutLattice` + `FullSegmentationOutcome` are
/// inputs. Geometry comes from the token/block bounding boxes; the visual kind
/// comes from the layout token kind plus the documented table grid heuristic.
pub fn build_native_visual_observation(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    segmentation: &FullSegmentationOutcome,
    config: NativeVisualFromInputConfig,
) -> Result<NativeVisualObservation, NativeVisualInputError> {
    let (provider, _) = validate_plane(document, lattice, segmentation)?;
    let line_count = lattice
        .pages
        .iter()
        .flat_map(|page| page.blocks.iter())
        .map(|block| block.lines.len())
        .sum::<usize>();
    if line_count > config.max_lines {
        return Err(NativeVisualInputError::LineLimit { count: line_count });
    }

    let furniture: std::collections::BTreeSet<(u32, u32)> = segmentation
        .furniture
        .iter()
        .map(|annotation| (annotation.location.page_ordinal, annotation.source_ordinal))
        .collect();

    let mut primitives = Vec::new();

    // Visual-kind layout tokens: image → figure, formula → formula.
    for page in &document.pages {
        for token in &page.tokens {
            let kind = match token.kind {
                LayoutTokenKind::Image => NativeVisualKind::Image,
                LayoutTokenKind::Formula => NativeVisualKind::Formula,
                _ => continue,
            };
            if furniture.contains(&(token.page_ordinal, token.source_ordinal)) {
                continue;
            }
            if !is_sha256(&token.text_digest) {
                return Err(NativeVisualInputError::InvalidDigest {
                    page: token.page_ordinal,
                    source: token.source_ordinal,
                });
            }
            primitives.push(NativeVisualPrimitive {
                document_id: document.document_id.clone(),
                source_sha256: document.source_pdf_sha256.clone(),
                provider: provider.clone(),
                page_ordinal: token.page_ordinal,
                source_ordinal: token.source_ordinal,
                kind,
                bbox: Some(token.bbox),
                content_digest: Some(token.text_digest.clone()),
                absence: None,
            });
        }
    }

    // Table grid heuristic over lattice blocks.
    for page in &lattice.pages {
        for block in &page.blocks {
            if matches!(block.furniture, crate::FurnitureEvidence::Classified { .. }) {
                continue;
            }
            if !block_is_table_grid(block) {
                continue;
            }
            let digest =
                table_content_digest(block).ok_or(NativeVisualInputError::InvalidTableDigest {
                    page: page.page_ordinal,
                    block: block.source_ordinal,
                })?;
            primitives.push(NativeVisualPrimitive {
                document_id: document.document_id.clone(),
                source_sha256: document.source_pdf_sha256.clone(),
                provider: provider.clone(),
                page_ordinal: page.page_ordinal,
                source_ordinal: block.source_ordinal,
                kind: NativeVisualKind::Table,
                bbox: Some(block.bbox),
                content_digest: Some(digest),
                absence: None,
            });
        }
    }

    primitives.sort_by_key(|primitive| {
        (
            primitive.page_ordinal,
            primitive.source_ordinal,
            primitive.kind as u8,
        )
    });
    let mut seen = std::collections::BTreeSet::new();
    for primitive in &primitives {
        if !seen.insert((
            primitive.page_ordinal,
            primitive.source_ordinal,
            primitive.kind as u8,
        )) {
            return Err(NativeVisualInputError::DuplicatePrimitive {
                page: primitive.page_ordinal,
                source: primitive.source_ordinal,
            });
        }
    }

    let observation = NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.to_owned(),
        document_id: document.document_id.clone(),
        source_sha256: document.source_pdf_sha256.clone(),
        provider,
        primitives,
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence: None,
    };
    observation
        .validate(config.visual)
        .map_err(NativeVisualInputError::NativeVisual)?;
    Ok(observation)
}

/// Run the three existing visual heads over a native observation.
pub fn run_native_visual_heads(
    observation: &NativeVisualObservation,
    config: NativeVisualConfig,
) -> Result<Vec<NativeVisualHeadObservation>, NativeVisualInputError> {
    let figure =
        detect_figure_heads(observation, config).map_err(NativeVisualInputError::NativeVisual)?;
    let table =
        detect_table_heads(observation, config).map_err(NativeVisualInputError::NativeVisual)?;
    let formula =
        detect_formula_heads(observation, config).map_err(NativeVisualInputError::NativeVisual)?;
    Ok(vec![
        NativeVisualHeadObservation {
            family: NativeVisualFamily::Figure,
            outcome: figure,
        },
        NativeVisualHeadObservation {
            family: NativeVisualFamily::Table,
            outcome: table,
        },
        NativeVisualHeadObservation {
            family: NativeVisualFamily::Formula,
            outcome: formula,
        },
    ])
}

/// Build the full native visual outcome: observation (or typed absence) plus
/// the three head observations.
pub fn build_native_visual_input(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    segmentation: &FullSegmentationOutcome,
    config: NativeVisualFromInputConfig,
) -> Result<NativeVisualFromInputOutcome, NativeVisualInputError> {
    let (_, identity) = validate_plane(document, lattice, segmentation)?;
    let token_count = document
        .pages
        .iter()
        .map(|page| page.tokens.len())
        .sum::<usize>();
    if token_count == 0 {
        return Ok(NativeVisualFromInputOutcome {
            schema_version: NATIVE_VISUAL_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
            identity,
            observation: None,
            absence: Some(NativeVisualAbsence::NotObserved),
            heads: Vec::new(),
        });
    }
    let observation = build_native_visual_observation(document, lattice, segmentation, config)?;
    let heads = run_native_visual_heads(&observation, config.visual)?;
    Ok(NativeVisualFromInputOutcome {
        schema_version: NATIVE_VISUAL_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
        identity,
        observation: Some(observation),
        absence: None,
        heads,
    })
}

/// Validate the plane and derive its provider plus provenance identity.
fn validate_plane(
    document: &LayoutDocument,
    lattice: &LayoutLattice,
    segmentation: &FullSegmentationOutcome,
) -> Result<(NativeVisualProvider, NativeVisualInputIdentity), NativeVisualInputError> {
    document
        .validate()
        .map_err(|_| NativeVisualInputError::LayoutInvalid)?;
    lattice
        .validate_against(document)
        .map_err(|_| NativeVisualInputError::LatticeInvalid)?;
    let lattice_fingerprint = lattice
        .digest()
        .map_err(|_| NativeVisualInputError::Serialization)?;
    if segmentation.schema_version != FULL_SEGMENTATION_SCHEMA_VERSION {
        return Err(NativeVisualInputError::IdentityMismatch("schema_version"));
    }
    if segmentation.identity.document_id != document.document_id {
        return Err(NativeVisualInputError::IdentityMismatch("document_id"));
    }
    if segmentation.identity.source_pdf_sha256 != document.source_pdf_sha256 {
        return Err(NativeVisualInputError::IdentityMismatch(
            "source_pdf_sha256",
        ));
    }
    if segmentation.identity.lattice_fingerprint != lattice_fingerprint {
        return Err(NativeVisualInputError::IdentityMismatch(
            "lattice_fingerprint",
        ));
    }
    let first = document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .next();
    let provider_id = first
        .map(|token| token.provider.id.clone())
        .unwrap_or_default();
    if segmentation.identity.provider_id != provider_id {
        return Err(NativeVisualInputError::IdentityMismatch("provider_id"));
    }
    if let Some(token) = first
        && (token.provider.id.is_empty() || token.provider.version.is_empty())
    {
        return Err(NativeVisualInputError::IdentityMismatch("provider_id"));
    }
    Ok((
        NativeVisualProvider {
            id: provider_id,
            version: first
                .map(|token| token.provider.version.clone())
                .unwrap_or_else(|| "absent".to_owned()),
        },
        NativeVisualInputIdentity {
            schema_version: NATIVE_VISUAL_NATIVE_INPUT_SCHEMA_VERSION.to_owned(),
            document_id: document.document_id.clone(),
            provider_id: segmentation.identity.provider_id.clone(),
            source_pdf_sha256: document.source_pdf_sha256.clone(),
            lattice_fingerprint,
            config_fingerprint: segmentation.identity.config_fingerprint.clone(),
            order_digest: segmentation.order_digest.clone(),
        },
    ))
}

/// A block is a table grid when at least [`NATIVE_VISUAL_TABLE_MIN_ROWS`] of its
/// lines carry at least [`NATIVE_VISUAL_TABLE_MIN_COLUMNS`] horizontally
/// separated cells, and every row's cell anchors align across rows.
fn block_is_table_grid(block: &crate::LatticeBlock) -> bool {
    if block.lines.len() < NATIVE_VISUAL_TABLE_MIN_ROWS {
        return false;
    }
    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(block.lines.len());
    for line in &block.lines {
        let anchors = cell_anchors(&line.tokens);
        if anchors.len() < NATIVE_VISUAL_TABLE_MIN_COLUMNS {
            return false;
        }
        rows.push(anchors);
    }
    let first = &rows[0];
    rows.iter()
        .all(|row| row.len() == first.len() && anchors_align(row, first))
}

/// Cell anchors of one line: the left edge of each run of tokens separated by at
/// least [`NATIVE_VISUAL_TABLE_CELL_GAP_PT`] points of horizontal white space.
fn cell_anchors(tokens: &[crate::LatticeToken]) -> Vec<f64> {
    let mut lexical: Vec<&crate::LatticeToken> = tokens
        .iter()
        .filter(|token| {
            token.kind.is_lexical() && !matches!(token.kind, LayoutTokenKind::Whitespace)
        })
        .collect();
    lexical.sort_by(|a, b| a.bbox.x_min.total_cmp(&b.bbox.x_min));
    let mut anchors = Vec::new();
    let mut cell_max = f64::NEG_INFINITY;
    for token in lexical {
        if anchors.is_empty() || token.bbox.x_min - cell_max >= NATIVE_VISUAL_TABLE_CELL_GAP_PT {
            anchors.push(token.bbox.x_min);
            cell_max = token.bbox.x_max;
        } else {
            cell_max = cell_max.max(token.bbox.x_max);
        }
    }
    anchors
}

/// Whether two rows' cell anchors align within
/// [`NATIVE_VISUAL_TABLE_ANCHOR_TOLERANCE_PT`] points.
fn anchors_align(row: &[f64], first: &[f64]) -> bool {
    row.iter()
        .zip(first.iter())
        .all(|(a, b)| (a - b).abs() <= NATIVE_VISUAL_TABLE_ANCHOR_TOLERANCE_PT)
}

/// Redaction-safe content digest of a table block: sha256 over the ordered
/// `(source_ordinal, token_ordinal, text_digest)` triples. Never carries text.
fn table_content_digest(block: &crate::LatticeBlock) -> Option<String> {
    let mut hasher = Sha256::new();
    for line in &block.lines {
        for token in &line.tokens {
            hasher.update(token.source_ordinal.to_le_bytes());
            hasher.update(token.token_ordinal.to_le_bytes());
            hasher.update(token.text_digest.as_bytes());
            hasher.update([0u8]);
        }
    }
    let digest = format!("{:x}", hasher.finalize());
    is_sha256(&digest).then_some(digest)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

// ───────────────────────── independent-oracle comparison ─────────────────────────

/// Provenance identity of an independent visual oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualOracleProvenance {
    /// Identity of the code that produced the oracle. Must differ from
    /// [`NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID`].
    pub generator_id: String,
    /// The producing backend, e.g. `java-grobid-crf`.
    pub backend: String,
    /// sha256 pin of the oracle source bytes.
    pub sha256: String,
}

/// Refusal vocabulary of the oracle independence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeVisualOracleIndependenceError {
    MissingGenerator,
    MissingBackend,
    MissingPin,
    /// The oracle was produced by the same code as the SUT.
    NotIndependent {
        generator_id: String,
    },
}

impl fmt::Display for NativeVisualOracleIndependenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGenerator => write!(formatter, "oracle provenance has no generator id"),
            Self::MissingBackend => write!(formatter, "oracle provenance has no backend"),
            Self::MissingPin => write!(formatter, "oracle provenance has no sha256 pin"),
            Self::NotIndependent { generator_id } => write!(
                formatter,
                "oracle generator `{generator_id}` is the SUT generator, not independent"
            ),
        }
    }
}

impl std::error::Error for NativeVisualOracleIndependenceError {}

/// Require an oracle to have a distinct generator, a backend, and a sha256 pin.
pub fn require_independent_visual_oracle(
    provenance: &NativeVisualOracleProvenance,
) -> Result<(), NativeVisualOracleIndependenceError> {
    if provenance.generator_id.trim().is_empty() {
        return Err(NativeVisualOracleIndependenceError::MissingGenerator);
    }
    if provenance.generator_id == NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID {
        return Err(NativeVisualOracleIndependenceError::NotIndependent {
            generator_id: provenance.generator_id.clone(),
        });
    }
    if provenance.backend.trim().is_empty() {
        return Err(NativeVisualOracleIndependenceError::MissingBackend);
    }
    if !is_sha256(&provenance.sha256) {
        return Err(NativeVisualOracleIndependenceError::MissingPin);
    }
    Ok(())
}

/// One positive per-family row read from preserved bytes by an independent
/// scanner. `page_ordinal`/`coords` are typed-optional because a document may
/// carry the family without a coordinate attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualOracleRow {
    pub document_id: String,
    pub family: NativeVisualFamily,
    /// Zero-based ordinal of the row within its document and family.
    pub ordinal: u32,
    /// Zero-based page derived from the coordinate attribute when present.
    pub page_ordinal: Option<u32>,
    /// The coordinate attribute verbatim when present.
    pub coords: Option<String>,
    /// sha256 of the caption bytes, when the family carries a caption.
    pub caption_digest: Option<String>,
    /// sha256 of the content bytes.
    pub content_digest: String,
}

/// A document pin of the oracle source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualOraclePin {
    pub document_id: String,
    pub sha256: String,
}

/// The independent positive oracle for the visual families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualOracle {
    pub schema_version: String,
    pub generator_id: String,
    pub backend: String,
    pub pins: Vec<NativeVisualOraclePin>,
    pub rows: Vec<NativeVisualOracleRow>,
}

/// Schema identity of the independent visual oracle.
pub const NATIVE_VISUAL_ORACLE_SCHEMA_VERSION: &str = "da-parse.native-visual-oracle/v1";

/// Refusal vocabulary of the oracle guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeVisualOracleError {
    InvalidSchema,
    /// A document pin is missing or not a sha256.
    InvalidPin {
        document_id: String,
    },
    /// The oracle carries no positive row for a declared family, so agreement
    /// with absence would be vacuous. It cannot give a pass.
    OracleVacuous {
        family: NativeVisualFamily,
    },
    /// The oracle is not independent of the SUT.
    NotIndependent(NativeVisualOracleIndependenceError),
}

impl fmt::Display for NativeVisualOracleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema => write!(formatter, "visual oracle schema is not recognised"),
            Self::InvalidPin { document_id } => {
                write!(
                    formatter,
                    "visual oracle pin `{document_id}` is not a sha256"
                )
            }
            Self::OracleVacuous { family } => write!(
                formatter,
                "visual oracle carries no positive `{}` row: vacuous, cannot pass",
                family.token()
            ),
            Self::NotIndependent(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NativeVisualOracleError {}

/// Positive rows of one family, across the whole oracle or one document.
pub fn visual_oracle_positive_rows(
    oracle: &NativeVisualOracle,
    document_id: Option<&str>,
    family: NativeVisualFamily,
) -> usize {
    oracle
        .rows
        .iter()
        .filter(|row| row.family == family)
        .filter(|row| document_id.is_none_or(|id| row.document_id == id))
        .count()
}

/// Require the oracle to be independent and to carry at least one positive row
/// for every declared family.
///
/// An absent family makes the whole oracle vacuous for that family: comparing
/// native absence against oracle absence is agreement by construction.
pub fn require_non_vacuous_visual_oracle(
    oracle: &NativeVisualOracle,
) -> Result<(), NativeVisualOracleError> {
    if oracle.schema_version != NATIVE_VISUAL_ORACLE_SCHEMA_VERSION {
        return Err(NativeVisualOracleError::InvalidSchema);
    }
    require_independent_visual_oracle(&NativeVisualOracleProvenance {
        generator_id: oracle.generator_id.clone(),
        backend: oracle.backend.clone(),
        sha256: oracle
            .pins
            .first()
            .map(|pin| pin.sha256.clone())
            .unwrap_or_default(),
    })
    .map_err(NativeVisualOracleError::NotIndependent)?;
    if oracle.pins.is_empty() {
        return Err(NativeVisualOracleError::InvalidPin {
            document_id: String::new(),
        });
    }
    for pin in &oracle.pins {
        if !is_sha256(&pin.sha256) {
            return Err(NativeVisualOracleError::InvalidPin {
                document_id: pin.document_id.clone(),
            });
        }
    }
    for family in NativeVisualFamily::ALL {
        if visual_oracle_positive_rows(oracle, None, family) == 0 {
            return Err(NativeVisualOracleError::OracleVacuous { family });
        }
    }
    Ok(())
}

/// Why a required family was not met by the native lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualUnmetReason {
    /// The native plane carries no visual-kind token and no table grid for the
    /// family, so its candidate population is empty.
    NativeKindUnreachable,
    /// The native plane declared a lane-level absence for the document.
    LaneAbsent,
    /// The head ran on a non-empty declared population but admitted nothing.
    HeadTerminal,
}

/// The closed disposition of one document × family. `out_of_scope` is
/// structurally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualFamilyDisposition {
    /// The native lane admitted at least one head for the family.
    Measured { native_heads: usize },
    /// The oracle requires the family but the native lane admitted nothing.
    Unmet { reason: NativeVisualUnmetReason },
    /// The oracle carries no positive row for this document × family, so there
    /// is nothing to meet.
    NotObserved,
}

/// One document × family disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVisualFamilyEvaluation {
    pub document_id: String,
    pub family: NativeVisualFamily,
    pub disposition: NativeVisualFamilyDisposition,
}

/// Compare one document's native band against the independent oracle.
///
/// Every family gets exactly one disposition. A family the native lane cannot
/// observe is `unmet` with a typed reason when the oracle requires it, and
/// `not_observed` when the oracle does not — never `out_of_scope`.
pub fn evaluate_visual_family_rows(
    outcome: &NativeVisualFromInputOutcome,
    oracle: &NativeVisualOracle,
) -> Result<Vec<NativeVisualFamilyEvaluation>, NativeVisualOracleError> {
    if oracle.schema_version != NATIVE_VISUAL_ORACLE_SCHEMA_VERSION {
        return Err(NativeVisualOracleError::InvalidSchema);
    }
    let document_id = outcome.identity.document_id.as_str();
    let mut evaluations = Vec::with_capacity(NativeVisualFamily::ALL.len());
    for family in NativeVisualFamily::ALL {
        let native_heads = outcome.admitted_heads(family);
        let required = visual_oracle_positive_rows(oracle, Some(document_id), family);
        let disposition = if native_heads > 0 {
            NativeVisualFamilyDisposition::Measured { native_heads }
        } else if required == 0 {
            NativeVisualFamilyDisposition::NotObserved
        } else {
            NativeVisualFamilyDisposition::Unmet {
                reason: unmet_reason(outcome, family),
            }
        };
        evaluations.push(NativeVisualFamilyEvaluation {
            document_id: document_id.to_owned(),
            family,
            disposition,
        });
    }
    Ok(evaluations)
}

fn unmet_reason(
    outcome: &NativeVisualFromInputOutcome,
    family: NativeVisualFamily,
) -> NativeVisualUnmetReason {
    if outcome.observation.is_none() {
        return NativeVisualUnmetReason::LaneAbsent;
    }
    match outcome.head(family).map(|head| &head.outcome) {
        Some(NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            ..
        }) => NativeVisualUnmetReason::NativeKindUnreachable,
        Some(NativeVisualHeadOutcome::Terminal { .. }) => NativeVisualUnmetReason::HeadTerminal,
        Some(_) | None => NativeVisualUnmetReason::NativeKindUnreachable,
    }
}
