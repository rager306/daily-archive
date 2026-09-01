//! Provider-neutral layout donors for the S13 bakeoff.
//!
//! This module is deliberately shadow-only: it produces the neutral layout
//! contract and never selects a parser route or writes graph/cache state.

use crate::layout_visual::{
    VisualAbsenceReason, VisualObservation, VisualPrimitive, VisualPrimitiveKind, VisualProvider,
};
use da_parse::{
    LayoutBBox, LayoutDocument, LayoutError, LayoutPage, LayoutProvider, LayoutSpan,
    LayoutStyleHints, LayoutToken, LayoutTokenKind,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const DEFAULT_CONFIG_FINGERPRINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Copy)]
pub struct LayoutDonorLimits {
    pub max_pages: usize,
    pub max_tokens: usize,
    pub max_output_bytes: usize,
    pub timeout_ms: u64,
}
impl Default for LayoutDonorLimits {
    fn default() -> Self {
        Self {
            max_pages: 512,
            max_tokens: 100_000,
            max_output_bytes: 64 * 1024 * 1024,
            timeout_ms: 30_000,
        }
    }
}

#[derive(Debug)]
pub enum LayoutDonorError {
    Unavailable {
        executable: String,
    },
    Timeout {
        timeout_ms: u64,
    },
    NonZero {
        status: Option<i32>,
    },
    Malformed {
        detail: String,
    },
    Oversize {
        resource: &'static str,
        limit: usize,
    },
    Encrypted,
    Truncated,
    InvalidLayout(LayoutError),
    Io(std::io::Error),
}
impl fmt::Display for LayoutDonorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout donor error: {self:?}")
    }
}
impl std::error::Error for LayoutDonorError {}
impl From<LayoutError> for LayoutDonorError {
    fn from(value: LayoutError) -> Self {
        Self::InvalidLayout(value)
    }
}
impl From<std::io::Error> for LayoutDonorError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// One operation, one neutral document or one typed terminal error.
pub trait LayoutDonor: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn extract(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutDocument, LayoutDonorError>;

    /// Additive invocation-local lexical observation. Existing layout-only
    /// donors keep the neutral API and reject this optional surface.
    #[cfg(feature = "pdf-layout")]
    fn extract_shadow(
        &self,
        _pdf_bytes: &[u8],
        _document_id: &str,
        _limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        Err(LayoutDonorError::Malformed {
            detail: "layout donor has no shadow lexical observation".into(),
        })
    }
}

/// Invocation-local lexical evidence paired with the unchanged neutral layout.
///
/// The text is intentionally private and is never serialized or included in
/// diagnostics. Consumers can pass this value directly to `da_parse`'s native
/// segmentation API while the shipped S13 layout schema remains unchanged.
#[cfg(feature = "pdf-layout")]
#[derive(Clone, PartialEq)]
pub struct LayoutShadowObservation {
    pub document: LayoutDocument,
    pub lexemes: Vec<da_parse::LexemeRow>,
    /// Bounded visual evidence from this invocation, kept separate from the
    /// neutral lexical layout and absent when the upstream signal is missing.
    pub visual: VisualObservation,
}

#[cfg(feature = "pdf-layout")]
impl fmt::Debug for LayoutShadowObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayoutShadowObservation")
            .field("document", &self.document)
            .field("lexeme_count", &self.lexemes.len())
            .field("visual", &self.visual)
            .finish()
    }
}

#[cfg(feature = "pdf-layout")]
fn observation_from_items(
    pdf_bytes: &[u8],
    document_id: &str,
    provider: LayoutProvider,
    items: Vec<PositionedItem>,
    page_geometry: &BTreeMap<u32, PageGeometry>,
    limits: LayoutDonorLimits,
    require_page_geometry: bool,
) -> Result<LayoutShadowObservation, LayoutDonorError> {
    let document = document_from_items(
        pdf_bytes,
        document_id,
        provider.clone(),
        items.clone(),
        page_geometry,
        limits,
        require_page_geometry,
    )?;
    let mut ordered = items;
    ordered.sort_by_key(|item| (item.page, item.source_ordinal));
    let mut lexeme_bytes = 0usize;
    let lexemes = ordered
        .into_iter()
        .map(|item| {
            lexeme_bytes = lexeme_bytes.saturating_add(item.text.len());
            da_parse::LexemeRow::new(
                document_id,
                &provider.id,
                item.page,
                item.source_ordinal,
                sha256_hex(item.text.as_bytes()),
                item.text,
            )
        })
        .collect::<Vec<_>>();
    if lexeme_bytes > limits.max_output_bytes {
        return Err(LayoutDonorError::Oversize {
            resource: "lexeme_bytes",
            limit: limits.max_output_bytes,
        });
    }
    let visual = VisualObservation::terminal_absence(
        &document,
        crate::layout_visual::VisualProvider::new(provider.id.clone(), provider.version.clone())
            .map_err(|error| LayoutDonorError::Malformed {
                detail: format!("visual provider: {error}"),
            })?,
        VisualAbsenceReason::NotObserved,
    )
    .map_err(|error| LayoutDonorError::Malformed {
        detail: format!("visual observation: {error}"),
    })?;
    Ok(LayoutShadowObservation {
        document,
        lexemes,
        visual,
    })
}

#[cfg(feature = "pdf-layout")]
fn validate_shadow_lexemes(observation: &LayoutShadowObservation) -> Result<(), LayoutDonorError> {
    let tokens = observation
        .document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .map(|token| ((token.page_ordinal, token.source_ordinal), token))
        .collect::<BTreeMap<_, _>>();
    if tokens.len()
        != observation
            .document
            .pages
            .iter()
            .map(|page| page.tokens.len())
            .sum::<usize>()
        || observation.lexemes.len() != tokens.len()
    {
        return Err(LayoutDonorError::Malformed {
            detail: "shadow lexical observation has duplicate or missing rows".into(),
        });
    }
    let mut seen = BTreeMap::new();
    for row in &observation.lexemes {
        let key = (row.page_ordinal, row.source_ordinal);
        if seen.insert(key, ()).is_some() {
            return Err(LayoutDonorError::Malformed {
                detail: "shadow lexical observation has duplicate identity".into(),
            });
        }
        let token = tokens
            .get(&key)
            .ok_or_else(|| LayoutDonorError::Malformed {
                detail: "shadow lexical observation has unmatched identity".into(),
            })?;
        if row.document_id != observation.document.document_id
            || row.provider_id != token.provider.id
            || row.text_digest != token.text_digest
        {
            return Err(LayoutDonorError::Malformed {
                detail: "shadow lexical observation digest or lineage mismatch".into(),
            });
        }
    }
    Ok(())
}

#[cfg(feature = "pdf-layout")]
pub fn reorder_shadow_with_xy_cut(
    mut observation: LayoutShadowObservation,
) -> Result<LayoutShadowObservation, LayoutDonorError> {
    validate_shadow_lexemes(&observation)?;
    observation.document = reorder_with_xy_cut(observation.document)?;
    let mut by_key = observation
        .lexemes
        .into_iter()
        .map(|row| ((row.page_ordinal, row.source_ordinal), row))
        .collect::<BTreeMap<_, _>>();
    let mut lexemes = Vec::new();
    for page in &observation.document.pages {
        for token in &page.tokens {
            let row = by_key
                .remove(&(token.page_ordinal, token.source_ordinal))
                .ok_or_else(|| LayoutDonorError::Malformed {
                    detail: "XY-Cut lexical observation lost token identity".into(),
                })?;
            lexemes.push(row);
        }
    }
    if !by_key.is_empty() {
        return Err(LayoutDonorError::Malformed {
            detail: "XY-Cut lexical observation has unmatched rows".into(),
        });
    }
    // Keep the lexical sidecar in the document's admitted order. XY-Cut
    // changes only that order while source identity remains the stable join
    // key; the native validator accepts this exact order or the source order.
    observation.lexemes = lexemes;
    observation
        .visual
        .validate(&observation.document)
        .map_err(|error| LayoutDonorError::Malformed {
            detail: format!("visual observation: {error}"),
        })?;
    Ok(observation)
}

/// Apply XY-Cut to an already positioned document without dropping or
/// rewriting token text, geometry, source ordinals, or provider lineage.
pub fn reorder_with_xy_cut(
    mut document: LayoutDocument,
) -> Result<LayoutDocument, LayoutDonorError> {
    for page in &mut document.pages {
        let original = std::mem::take(&mut page.tokens);
        let lines: Vec<da_domain::xy_cut::LineBox> = original
            .iter()
            .map(|token| {
                da_domain::xy_cut::LineBox {
                    // The algorithm treats text as opaque; ordinal is a stable
                    // carrier so the exact original token can be restored.
                    text: token.source_ordinal.to_string(),
                    x0: token.bbox.x_min as f32,
                    y0: token.bbox.y_min as f32,
                    x1: token.bbox.x_max as f32,
                    y1: token.bbox.y_max as f32,
                }
            })
            .collect();
        let ordered =
            da_domain::xy_cut::order_lines(&lines, &da_domain::xy_cut::XyCutConfig::default());
        let original_len = original.len();
        let mut by_source: BTreeMap<u32, LayoutToken> = original
            .into_iter()
            .map(|token| (token.source_ordinal, token))
            .collect();
        if by_source.len() != original_len {
            return Err(LayoutDonorError::Malformed {
                detail: "XY-Cut input has duplicate source identity".into(),
            });
        }
        let mut tokens = Vec::with_capacity(ordered.len());
        for (ordinal, line) in ordered.into_iter().enumerate() {
            let source = line
                .text
                .parse::<u32>()
                .map_err(|_| LayoutDonorError::Malformed {
                    detail: "XY-Cut identity carrier corrupted".into(),
                })?;
            let mut token =
                by_source
                    .remove(&source)
                    .ok_or_else(|| LayoutDonorError::Malformed {
                        detail: "XY-Cut dropped or duplicated token".into(),
                    })?;
            token.token_ordinal = ordinal as u32;
            tokens.push(token);
        }
        if !by_source.is_empty() {
            return Err(LayoutDonorError::Malformed {
                detail: "XY-Cut dropped token".into(),
            });
        }
        *page = LayoutPage::new(
            page.page_ordinal,
            page.width,
            page.height,
            page.rotation,
            page.crop_box,
            tokens,
        )?;
    }
    document.validate()?;
    Ok(document)
}

/// XY-Cut donor over positioned tokens produced by another lane. It keeps
/// extraction order in `source_ordinal` and records only the reordered view.
#[cfg(feature = "pdf-layout")]
pub struct XyCutLayoutDonor {
    pub base: Box<dyn LayoutDonor>,
}
#[cfg(feature = "pdf-layout")]
impl XyCutLayoutDonor {
    pub fn new(base: Box<dyn LayoutDonor>) -> Self {
        Self { base }
    }

    pub fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        let observation = self.base.extract_shadow(pdf_bytes, document_id, limits)?;
        reorder_shadow_with_xy_cut(observation)
    }
}
#[cfg(feature = "pdf-layout")]
impl LayoutDonor for XyCutLayoutDonor {
    fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        XyCutLayoutDonor::extract_shadow(self, pdf_bytes, document_id, limits)
    }

    fn id(&self) -> &'static str {
        "xy-cut-positioned"
    }
    fn version(&self) -> &'static str {
        "adr-072"
    }
    fn extract(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutDocument, LayoutDonorError> {
        reorder_with_xy_cut(self.base.extract(pdf_bytes, document_id, limits)?)
    }
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn token_kind(text: &str) -> LayoutTokenKind {
    if text.chars().all(char::is_whitespace) {
        LayoutTokenKind::Whitespace
    } else if text.chars().all(|c| c.is_ascii_punctuation()) {
        LayoutTokenKind::Punctuation
    } else if text.chars().all(|c| !c.is_alphanumeric()) {
        LayoutTokenKind::Symbol
    } else {
        LayoutTokenKind::Word
    }
}
fn provider(id: &'static str, version: &'static str) -> LayoutProvider {
    LayoutProvider::new(id, version, DEFAULT_CONFIG_FINGERPRINT.to_owned())
        .expect("constant fingerprint")
}
fn document_from_items(
    pdf_bytes: &[u8],
    document_id: &str,
    provider: LayoutProvider,
    items: impl IntoIterator<Item = PositionedItem>,
    page_geometry: &BTreeMap<u32, PageGeometry>,
    limits: LayoutDonorLimits,
    require_page_geometry: bool,
) -> Result<LayoutDocument, LayoutDonorError> {
    let mut by_page: BTreeMap<u32, Vec<PositionedItem>> = BTreeMap::new();
    for item in items {
        if !item.x.is_finite()
            || !item.y.is_finite()
            || !item.width.is_finite()
            || !item.height.is_finite()
            || item.x < 0.0
            || item.y < 0.0
            || item.width <= 0.0
            || item.height <= 0.0
        {
            return Err(LayoutDonorError::Malformed {
                detail: "non-finite or inverted item geometry".into(),
            });
        }
        by_page.entry(item.page).or_default().push(item);
    }
    let page_ids = by_page
        .keys()
        .chain(page_geometry.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if page_ids.len() > limits.max_pages {
        return Err(LayoutDonorError::Oversize {
            resource: "pages",
            limit: limits.max_pages,
        });
    }
    let mut total = 0usize;
    let mut pages = Vec::with_capacity(page_ids.len());
    for page in page_ids {
        let mut items = by_page.remove(&page).unwrap_or_default();
        items.sort_by_key(|i| i.source_ordinal);
        if require_page_geometry && !page_geometry.contains_key(&page) {
            return Err(LayoutDonorError::Malformed {
                detail: "authoritative page geometry missing".into(),
            });
        }
        let fallback_width = items.iter().map(|i| i.x + i.width).fold(1.0_f64, f64::max);
        let fallback_height = items.iter().map(|i| i.y + i.height).fold(1.0_f64, f64::max);
        let geometry = page_geometry.get(&page).copied().unwrap_or(PageGeometry {
            width: fallback_width,
            height: fallback_height,
            rotation: 0,
            crop_box: None,
        });
        let width = geometry.width;
        let height = geometry.height;
        let mut tokens = Vec::with_capacity(items.len());
        for (ordinal, item) in items.into_iter().enumerate() {
            total += 1;
            if total > limits.max_tokens {
                return Err(LayoutDonorError::Oversize {
                    resource: "tokens",
                    limit: limits.max_tokens,
                });
            }
            if item.x + item.width > width || item.y + item.height > height {
                return Err(LayoutDonorError::Malformed {
                    detail: "item geometry exceeds authoritative page bounds".into(),
                });
            }
            let bbox = match item.origin {
                CoordinateOrigin::BottomLeft => LayoutBBox::from_bottom_left(
                    item.x,
                    item.y,
                    item.x + item.width,
                    item.y + item.height,
                    height,
                )?,
                CoordinateOrigin::TopLeft => {
                    LayoutBBox::new(item.x, item.y, item.x + item.width, item.y + item.height)?
                }
            };
            tokens.push(LayoutToken::new(
                page,
                item.source_ordinal,
                ordinal as u32,
                bbox,
                sha256_hex(item.text.as_bytes()),
                LayoutSpan::absent(da_parse::LayoutSpanAbsence::NotAvailable),
                token_kind(&item.text),
                LayoutStyleHints {
                    bold: item.bold,
                    italic: item.italic,
                    monospace: false,
                    font_size_milli_pt: item.font_size.map(|v| (v * 1000.0) as u32),
                },
                provider.clone(),
            )?);
        }
        pages.push(LayoutPage::new(
            page,
            width,
            height,
            geometry.rotation,
            geometry.crop_box,
            tokens,
        )?);
    }
    Ok(LayoutDocument::new(
        document_id,
        sha256_hex(pdf_bytes),
        pages,
    )?)
}

#[derive(Debug, Clone, Copy)]
struct PageGeometry {
    width: f64,
    height: f64,
    rotation: u16,
    crop_box: Option<LayoutBBox>,
}

#[derive(Debug, Clone, Copy)]
enum CoordinateOrigin {
    BottomLeft,
    TopLeft,
}

#[derive(Debug, Clone)]
struct PositionedItem {
    page: u32,
    source_ordinal: u32,
    text: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    bold: bool,
    italic: bool,
    font_size: Option<f64>,
    origin: CoordinateOrigin,
}

#[cfg(feature = "pdf-layout")]
pub struct PdfInspectorLayoutDonor;
#[cfg(feature = "pdf-layout")]
impl Default for PdfInspectorLayoutDonor {
    fn default() -> Self {
        Self
    }
}
#[cfg(feature = "pdf-layout")]
impl PdfInspectorLayoutDonor {
    pub fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        let items = pdf_inspector::extract_text_with_positions_mem(pdf_bytes).map_err(|e| {
            LayoutDonorError::Malformed {
                detail: e.to_string(),
            }
        })?;
        let mut next = BTreeMap::<u32, u32>::new();
        let converted = items
            .into_iter()
            .filter_map(|i| {
                if i.width == 0.0 || i.height == 0.0 {
                    return None;
                }
                let n = next.entry(i.page).or_insert(0);
                let source_ordinal = *n;
                *n += 1;
                Some(PositionedItem {
                    page: i.page,
                    source_ordinal,
                    text: i.text,
                    x: i.x as f64,
                    y: i.y as f64,
                    width: i.width as f64,
                    height: i.height as f64,
                    bold: i.is_bold,
                    italic: i.is_italic,
                    font_size: Some(i.font_size as f64),
                    origin: CoordinateOrigin::BottomLeft,
                })
            })
            .collect();
        observation_from_items(
            pdf_bytes,
            document_id,
            provider(self.id(), self.version()),
            converted,
            &BTreeMap::new(),
            limits,
            false,
        )
    }
}
#[cfg(feature = "pdf-layout")]
impl LayoutDonor for PdfInspectorLayoutDonor {
    #[cfg(feature = "pdf-layout")]
    fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        PdfInspectorLayoutDonor::extract_shadow(self, pdf_bytes, document_id, limits)
    }

    fn id(&self) -> &'static str {
        "pdf-inspector-positioned"
    }
    fn version(&self) -> &'static str {
        "0.1.7"
    }
    fn extract(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutDocument, LayoutDonorError> {
        Ok(self
            .extract_shadow(pdf_bytes, document_id, limits)?
            .document)
    }
}

/// External pdfalto donor. The executable is explicit and never replaced by
/// pdftotext or another fallback.
#[cfg(feature = "pdf-layout")]
pub struct PdfaltoLayoutDonor {
    pub executable: std::path::PathBuf,
}
#[cfg(feature = "pdf-layout")]
impl PdfaltoLayoutDonor {
    pub fn new(executable: impl Into<std::path::PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Run the explicit pdfalto lane while retaining bounded lexical evidence
    /// in memory. This is deliberately separate from `LayoutDonor::extract`.
    pub fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let dir = std::env::temp_dir();
        let stem = format!(
            "daily-archive-layout-shadow-{}-{}",
            std::process::id(),
            Sha256::digest(pdf_bytes)[..8]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let input = dir.join(format!("{stem}.pdf"));
        let output = dir.join(format!("{stem}.xml"));
        let result = (|| {
            let mut file = std::fs::File::create(&input)?;
            file.write_all(pdf_bytes)?;
            file.sync_all()?;
            let mut child = Command::new(&self.executable)
                .arg(&input)
                .arg(&output)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        LayoutDonorError::Unavailable {
                            executable: self.executable.display().to_string(),
                        }
                    } else {
                        LayoutDonorError::Io(e)
                    }
                })?;
            let started = std::time::Instant::now();
            loop {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        return Err(LayoutDonorError::NonZero {
                            status: status.code(),
                        });
                    }
                    break;
                }
                if started.elapsed().as_millis() >= limits.timeout_ms as u128 {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(LayoutDonorError::Timeout {
                        timeout_ms: limits.timeout_ms,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let metadata = std::fs::metadata(&output)?;
            if metadata.len() > limits.max_output_bytes as u64 {
                return Err(LayoutDonorError::Oversize {
                    resource: "xml_output",
                    limit: limits.max_output_bytes,
                });
            }
            let xml = std::fs::read(&output)?;
            if xml.is_empty() {
                return Err(LayoutDonorError::Truncated);
            }
            parse_pdfalto_xml_shadow(&xml, pdf_bytes, document_id, limits)
        })();
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
        result
    }
}
#[cfg(feature = "pdf-layout")]
impl LayoutDonor for PdfaltoLayoutDonor {
    fn extract_shadow(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutShadowObservation, LayoutDonorError> {
        PdfaltoLayoutDonor::extract_shadow(self, pdf_bytes, document_id, limits)
    }
    fn id(&self) -> &'static str {
        "pdfalto"
    }
    fn version(&self) -> &'static str {
        "external"
    }
    fn extract(
        &self,
        pdf_bytes: &[u8],
        document_id: &str,
        limits: LayoutDonorLimits,
    ) -> Result<LayoutDocument, LayoutDonorError> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let dir = std::env::temp_dir();
        let stem = format!(
            "daily-archive-layout-{}-{}",
            std::process::id(),
            Sha256::digest(pdf_bytes)[..8]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let input = dir.join(format!("{stem}.pdf"));
        let output = dir.join(format!("{stem}.xml"));
        let result = (|| {
            let mut file = std::fs::File::create(&input)?;
            file.write_all(pdf_bytes)?;
            file.sync_all()?;
            let mut child = Command::new(&self.executable)
                .arg(&input)
                .arg(&output)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        LayoutDonorError::Unavailable {
                            executable: self.executable.display().to_string(),
                        }
                    } else {
                        LayoutDonorError::Io(e)
                    }
                })?;
            let started = std::time::Instant::now();
            loop {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        return Err(LayoutDonorError::NonZero {
                            status: status.code(),
                        });
                    }
                    break;
                }
                if started.elapsed().as_millis() >= limits.timeout_ms as u128 {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(LayoutDonorError::Timeout {
                        timeout_ms: limits.timeout_ms,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let metadata = std::fs::metadata(&output)?;
            if metadata.len() > limits.max_output_bytes as u64 {
                return Err(LayoutDonorError::Oversize {
                    resource: "xml_output",
                    limit: limits.max_output_bytes,
                });
            }
            let xml = std::fs::read(&output)?;
            if xml.is_empty() {
                return Err(LayoutDonorError::Truncated);
            }
            parse_pdfalto_xml(&xml, pdf_bytes, document_id, limits)
        })();
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
        result
    }
}

/// Parses the bounded, text-only XML emitted by a pdfalto-compatible command.
pub fn parse_pdfalto_xml(
    xml: &[u8],
    pdf_bytes: &[u8],
    document_id: &str,
    limits: LayoutDonorLimits,
) -> Result<LayoutDocument, LayoutDonorError> {
    Ok(parse_pdfalto_xml_shadow(xml, pdf_bytes, document_id, limits)?.document)
}

/// Shadow variant of [`parse_pdfalto_xml`] retaining invocation-local lexemes.
#[cfg(feature = "pdf-layout")]
pub fn parse_pdfalto_xml_shadow(
    xml: &[u8],
    pdf_bytes: &[u8],
    document_id: &str,
    limits: LayoutDonorLimits,
) -> Result<LayoutShadowObservation, LayoutDonorError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut page = 0u32;
    let mut page_items = Vec::new();
    let mut all = Vec::new();
    let mut page_geometry = BTreeMap::<u32, PageGeometry>::new();
    let mut current: Option<(f64, f64, f64, f64, String)> = None;
    let mut ordinal = BTreeMap::<u32, u32>::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if e.name().as_ref().eq_ignore_ascii_case(b"page") =>
            {
                page += 1;
                if page as usize > limits.max_pages {
                    return Err(LayoutDonorError::Oversize {
                        resource: "pages",
                        limit: limits.max_pages,
                    });
                }
                let mut width = None;
                let mut height = None;
                let mut rotation = 0u16;
                let mut crop = [None; 4];
                for attr in e.attributes() {
                    let attr = attr.map_err(|error| LayoutDonorError::Malformed {
                        detail: error.to_string(),
                    })?;
                    let key = attr.key.as_ref();
                    let recognized = key.eq_ignore_ascii_case(b"width")
                        || key.eq_ignore_ascii_case(b"height")
                        || key.eq_ignore_ascii_case(b"rotation")
                        || key.eq_ignore_ascii_case(b"crop_x")
                        || key.eq_ignore_ascii_case(b"crop_y")
                        || key.eq_ignore_ascii_case(b"crop_width")
                        || key.eq_ignore_ascii_case(b"crop_height");
                    if !recognized {
                        continue;
                    }
                    let raw = std::str::from_utf8(&attr.value).map_err(|error| {
                        LayoutDonorError::Malformed {
                            detail: error.to_string(),
                        }
                    })?;
                    if key.eq_ignore_ascii_case(b"rotation") {
                        rotation =
                            raw.parse::<u16>()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?;
                        continue;
                    }
                    let value =
                        raw.parse::<f64>()
                            .map_err(|error| LayoutDonorError::Malformed {
                                detail: error.to_string(),
                            })?;
                    if key.eq_ignore_ascii_case(b"width") {
                        width = Some(value);
                    } else if key.eq_ignore_ascii_case(b"height") {
                        height = Some(value);
                    } else if key.eq_ignore_ascii_case(b"crop_x") {
                        crop[0] = Some(value);
                    } else if key.eq_ignore_ascii_case(b"crop_y") {
                        crop[1] = Some(value);
                    } else if key.eq_ignore_ascii_case(b"crop_width") {
                        crop[2] = Some(value);
                    } else if key.eq_ignore_ascii_case(b"crop_height") {
                        crop[3] = Some(value);
                    }
                }
                let (width, height) =
                    width
                        .zip(height)
                        .ok_or_else(|| LayoutDonorError::Malformed {
                            detail: "pdfalto page width/height missing".into(),
                        })?;
                if !width.is_finite()
                    || !height.is_finite()
                    || width <= 0.0
                    || height <= 0.0
                    || width > 100_000.0
                    || height > 100_000.0
                {
                    return Err(LayoutDonorError::Malformed {
                        detail: "invalid or oversized pdfalto page geometry".into(),
                    });
                }
                if !matches!(rotation, 0 | 90 | 180 | 270) {
                    return Err(LayoutDonorError::Malformed {
                        detail: "unsupported pdfalto page rotation".into(),
                    });
                }
                let crop_box = match crop {
                    [Some(x), Some(y), Some(w), Some(h)] => {
                        if !x.is_finite()
                            || !y.is_finite()
                            || !w.is_finite()
                            || !h.is_finite()
                            || x < 0.0
                            || y < 0.0
                            || w <= 0.0
                            || h <= 0.0
                            || x + w > width
                            || y + h > height
                        {
                            return Err(LayoutDonorError::Malformed {
                                detail: "invalid or out-of-page pdfalto crop geometry".into(),
                            });
                        }
                        Some(LayoutBBox::new(x, height - (y + h), x + w, height - y)?)
                    }
                    [None, None, None, None] => None,
                    _ => {
                        return Err(LayoutDonorError::Malformed {
                            detail: "incomplete pdfalto crop geometry".into(),
                        });
                    }
                };
                page_geometry.insert(
                    page,
                    PageGeometry {
                        width,
                        height,
                        rotation,
                        crop_box,
                    },
                );
            }
            Ok(Event::Start(e))
                if e.name().as_ref().eq_ignore_ascii_case(b"text")
                    || e.name().as_ref().eq_ignore_ascii_case(b"string") =>
            {
                let mut vals = [None; 4];
                let mut content = String::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|error| LayoutDonorError::Malformed {
                        detail: error.to_string(),
                    })?;
                    let key = attr.key.as_ref();
                    if key.eq_ignore_ascii_case(b"content") {
                        content = attr
                            .unescape_value()
                            .map_err(|error| LayoutDonorError::Malformed {
                                detail: error.to_string(),
                            })?
                            .into_owned();
                        continue;
                    }
                    let slot = if key.eq_ignore_ascii_case(b"x")
                        || key.eq_ignore_ascii_case(b"x0")
                        || key.eq_ignore_ascii_case(b"hpos")
                    {
                        Some(0)
                    } else if key.eq_ignore_ascii_case(b"y")
                        || key.eq_ignore_ascii_case(b"y0")
                        || key.eq_ignore_ascii_case(b"vpos")
                    {
                        Some(1)
                    } else if key.eq_ignore_ascii_case(b"width") || key.eq_ignore_ascii_case(b"w") {
                        Some(2)
                    } else if key.eq_ignore_ascii_case(b"height") || key.eq_ignore_ascii_case(b"h")
                    {
                        Some(3)
                    } else {
                        None
                    };
                    let Some(slot) = slot else { continue };
                    let raw = std::str::from_utf8(&attr.value).map_err(|error| {
                        LayoutDonorError::Malformed {
                            detail: error.to_string(),
                        }
                    })?;
                    vals[slot] =
                        Some(
                            raw.parse::<f64>()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?,
                        );
                }
                current = Some((
                    vals[0].unwrap_or(0.0),
                    vals[1].unwrap_or(0.0),
                    vals[2].unwrap_or(0.0),
                    vals[3].unwrap_or(0.0),
                    content,
                ));
            }
            Ok(Event::Empty(e)) if e.name().as_ref().eq_ignore_ascii_case(b"string") => {
                let mut vals = [None; 4];
                let mut content = None;
                for attr in e.attributes() {
                    let attr = attr.map_err(|error| LayoutDonorError::Malformed {
                        detail: error.to_string(),
                    })?;
                    let key = attr.key.as_ref();
                    if key.eq_ignore_ascii_case(b"content") {
                        content = Some(
                            attr.unescape_value()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?
                                .into_owned(),
                        );
                        continue;
                    }
                    let slot = if key.eq_ignore_ascii_case(b"hpos") {
                        Some(0)
                    } else if key.eq_ignore_ascii_case(b"vpos") {
                        Some(1)
                    } else if key.eq_ignore_ascii_case(b"width") {
                        Some(2)
                    } else if key.eq_ignore_ascii_case(b"height") {
                        Some(3)
                    } else {
                        None
                    };
                    let Some(slot) = slot else { continue };
                    let raw = std::str::from_utf8(&attr.value).map_err(|error| {
                        LayoutDonorError::Malformed {
                            detail: error.to_string(),
                        }
                    })?;
                    vals[slot] =
                        Some(
                            raw.parse::<f64>()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?,
                        );
                }
                let [Some(x), Some(y), Some(width), Some(height)] = vals else {
                    return Err(LayoutDonorError::Malformed {
                        detail: "pdfalto String geometry missing".into(),
                    });
                };
                // pdfalto emits zero-area bookkeeping strings on some pages;
                // they are not positioned text tokens. Preserve fail-closed
                // behavior for negative/non-finite geometry below.
                if width == 0.0 || height == 0.0 {
                    continue;
                }
                let n = ordinal.entry(page).or_insert(0);
                page_items.push(PositionedItem {
                    page,
                    source_ordinal: *n,
                    text: content.unwrap_or_default(),
                    x,
                    y,
                    width,
                    height,
                    bold: false,
                    italic: false,
                    font_size: None,
                    origin: CoordinateOrigin::TopLeft,
                });
                *n += 1;
            }
            Ok(Event::Text(e)) => {
                if let Some(item) = current.as_mut() {
                    item.4.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e))
                if e.name().as_ref().eq_ignore_ascii_case(b"text")
                    || e.name().as_ref().eq_ignore_ascii_case(b"string") =>
            {
                if let Some((x, y, w, h, text)) = current.take() {
                    let n = ordinal.entry(page).or_insert(0);
                    page_items.push(PositionedItem {
                        page,
                        source_ordinal: *n,
                        text,
                        x,
                        y,
                        width: w,
                        height: h,
                        bold: false,
                        italic: false,
                        font_size: None,
                        origin: if e.name().as_ref().eq_ignore_ascii_case(b"string") {
                            CoordinateOrigin::TopLeft
                        } else {
                            CoordinateOrigin::BottomLeft
                        },
                    });
                    *n += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(LayoutDonorError::Malformed {
                    detail: e.to_string(),
                });
            }
            _ => {}
        }
        buf.clear();
    }
    all.extend(page_items);
    let observation = observation_from_items(
        pdf_bytes,
        document_id,
        provider("pdfalto", "unknown"),
        all,
        &page_geometry,
        limits,
        true,
    )?;
    let visual = parse_pdfalto_visual_observation(
        xml,
        &observation.document,
        VisualProvider::new("pdfalto", "unknown").map_err(|error| LayoutDonorError::Malformed {
            detail: format!("visual provider: {error}"),
        })?,
        limits,
    )?;
    Ok(LayoutShadowObservation {
        visual,
        ..observation
    })
}

/// Retain only explicit pdfalto-compatible visual observations. Text elements
/// are deliberately ignored: lexical content is owned by the neutral layout
/// and cannot be promoted to detector coverage. The parser accepts common
/// ALTO-like geometry aliases, but requires a source content digest (or an
/// inline content value that is hashed immediately) for observed primitives.
#[cfg(feature = "pdf-layout")]
fn parse_pdfalto_visual_observation(
    xml: &[u8],
    document: &LayoutDocument,
    provider: VisualProvider,
    limits: LayoutDonorLimits,
) -> Result<VisualObservation, LayoutDonorError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut page = 0u32;
    let mut primitives = Vec::new();
    let mut ordinals = BTreeMap::<u32, u32>::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) if event.name().as_ref().eq_ignore_ascii_case(b"page") => {
                page = page.saturating_add(1);
            }
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let name = event.name();
                let kind = match visual_kind(name.as_ref()) {
                    Some(kind) => kind,
                    None => {
                        buffer.clear();
                        continue;
                    }
                };
                let mut values = [None; 4];
                let mut digest = None;
                let mut inline_content = None;
                let mut bottom_left = false;
                for attribute in event.attributes() {
                    let attribute = attribute.map_err(|error| LayoutDonorError::Malformed {
                        detail: error.to_string(),
                    })?;
                    let key = attribute.key.as_ref();
                    if key.eq_ignore_ascii_case(b"content_digest")
                        || key.eq_ignore_ascii_case(b"digest")
                        || key.eq_ignore_ascii_case(b"data_digest")
                    {
                        digest = Some(
                            attribute
                                .unescape_value()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?
                                .into_owned(),
                        );
                        continue;
                    }
                    if key.eq_ignore_ascii_case(b"content") {
                        inline_content = Some(
                            attribute
                                .unescape_value()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?
                                .into_owned(),
                        );
                        continue;
                    }
                    if key.eq_ignore_ascii_case(b"origin") {
                        let origin = attribute.unescape_value().map_err(|error| {
                            LayoutDonorError::Malformed {
                                detail: error.to_string(),
                            }
                        })?;
                        bottom_left = origin.eq_ignore_ascii_case("bottom_left");
                        continue;
                    }
                    let slot = if key.eq_ignore_ascii_case(b"x")
                        || key.eq_ignore_ascii_case(b"x0")
                        || key.eq_ignore_ascii_case(b"hpos")
                    {
                        Some(0)
                    } else if key.eq_ignore_ascii_case(b"y")
                        || key.eq_ignore_ascii_case(b"y0")
                        || key.eq_ignore_ascii_case(b"vpos")
                    {
                        Some(1)
                    } else if key.eq_ignore_ascii_case(b"width") || key.eq_ignore_ascii_case(b"w") {
                        Some(2)
                    } else if key.eq_ignore_ascii_case(b"height") || key.eq_ignore_ascii_case(b"h")
                    {
                        Some(3)
                    } else {
                        None
                    };
                    let Some(slot) = slot else { continue };
                    let raw = std::str::from_utf8(&attribute.value).map_err(|error| {
                        LayoutDonorError::Malformed {
                            detail: error.to_string(),
                        }
                    })?;
                    values[slot] =
                        Some(
                            raw.parse::<f64>()
                                .map_err(|error| LayoutDonorError::Malformed {
                                    detail: error.to_string(),
                                })?,
                        );
                }
                let [Some(x), Some(y), Some(width), Some(height)] = values else {
                    return Err(LayoutDonorError::Malformed {
                        detail: "visual primitive geometry missing".into(),
                    });
                };
                let page_doc = document
                    .pages
                    .iter()
                    .find(|candidate| candidate.page_ordinal == page)
                    .ok_or_else(|| LayoutDonorError::Malformed {
                        detail: "visual primitive page identity missing".into(),
                    })?;
                let bbox = if bottom_left {
                    LayoutBBox::from_bottom_left(x, y, x + width, y + height, page_doc.height)
                } else {
                    LayoutBBox::new(x, y, x + width, y + height)
                }
                .map_err(LayoutDonorError::InvalidLayout)?;
                let content_digest = digest
                    .or_else(|| inline_content.map(|content| sha256_hex(content.as_bytes())))
                    .ok_or_else(|| LayoutDonorError::Malformed {
                        detail: "visual primitive content digest missing".into(),
                    })?;
                let source_ordinal = ordinals.entry(page).or_insert(0);
                let primitive = VisualPrimitive::observed(
                    document,
                    provider.clone(),
                    page,
                    *source_ordinal,
                    kind,
                    bbox,
                    content_digest,
                )
                .map_err(|error| LayoutDonorError::Malformed {
                    detail: format!("visual primitive: {error}"),
                })?;
                *source_ordinal += 1;
                primitives.push(primitive);
                if primitives.len() > crate::layout_visual::MAX_VISUAL_PRIMITIVES
                    || primitives.len() > limits.max_tokens
                {
                    return Err(LayoutDonorError::Oversize {
                        resource: "visual_primitives",
                        limit: crate::layout_visual::MAX_VISUAL_PRIMITIVES.min(limits.max_tokens),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(LayoutDonorError::Malformed {
                    detail: error.to_string(),
                });
            }
            _ => {}
        }
        buffer.clear();
    }
    if primitives.is_empty() {
        VisualObservation::terminal_absence(document, provider, VisualAbsenceReason::NotObserved)
            .map_err(|error| LayoutDonorError::Malformed {
                detail: format!("visual observation: {error}"),
            })
    } else {
        VisualObservation::new(document, primitives).map_err(|error| LayoutDonorError::Malformed {
            detail: format!("visual observation: {error}"),
        })
    }
}

#[cfg(feature = "pdf-layout")]
fn visual_kind(name: &[u8]) -> Option<VisualPrimitiveKind> {
    if name.eq_ignore_ascii_case(b"image")
        || name.eq_ignore_ascii_case(b"illustration")
        || name.eq_ignore_ascii_case(b"figure")
    {
        Some(VisualPrimitiveKind::Image)
    } else if name.eq_ignore_ascii_case(b"graphic")
        || name.eq_ignore_ascii_case(b"graphicalelement")
    {
        Some(VisualPrimitiveKind::Graphic)
    } else if name.eq_ignore_ascii_case(b"table") {
        Some(VisualPrimitiveKind::Table)
    } else if name.eq_ignore_ascii_case(b"formula") || name.eq_ignore_ascii_case(b"equation") {
        Some(VisualPrimitiveKind::Formula)
    } else if name.eq_ignore_ascii_case(b"rect") || name.eq_ignore_ascii_case(b"rectangle") {
        Some(VisualPrimitiveKind::Rect)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fake_pdfalto_is_normalized_once_and_lossless() {
        let pdf = b"%PDF-test";
        let xml = br#"<doc><page width="600" height="800"><text x="10" y="20" width="30" height="8">Hello</text><text x="50" y="20" width="20" height="8">world</text></page></doc>"#;
        let doc = parse_pdfalto_xml(xml, pdf, "paper", LayoutDonorLimits::default()).unwrap();
        assert_eq!(doc.pages[0].tokens.len(), 2);
        assert_eq!(doc.pages[0].tokens[0].bbox.y_min, 772.0);
        assert_eq!(doc.source_pdf_sha256, sha256_hex(pdf));
        assert_eq!(doc.digest().unwrap(), doc.digest().unwrap());
    }
}
