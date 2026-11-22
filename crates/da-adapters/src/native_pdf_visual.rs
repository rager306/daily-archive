//! Opt-in source observations for top-level PDF Image XObjects.
//!
//! For offline, trusted-source use only. Admission limits are not a memory or
//! time sandbox: lopdf may inflate object streams and cross-reference streams
//! inside `Document::load_mem` before this adapter can inspect them. Content and
//! operation admission checks also run after decoding and allocation.
//!
//! `content_digest` hashes the literal encoded Image stream bytes returned by
//! lopdf, not decoded pixels, rendered appearance, or the PDF as a whole. The
//! provider identity states that dialect. An admitted image is a source
//! occurrence, not a semantic figure, caption, table, formula, or callout target.
//!
//! Only zero-origin, unrotated MediaBox pages with an absent/equal CropBox and
//! axis-aligned positive q/Q/cm/Do placement are supported. Forms, inline images,
//! clipping and extended graphics state make the whole lane typed Unsupported.
//! The pinned content parser can conservatively refuse valid PDF syntax, such as
//! extra blank lines after comments. Parser traversal is not general valid-PDF
//! acceptance; source whitespace is never removed to make a prefix pass.
//! No text layout is changed, and no bytes are retained in the returned carriers.

use crate::layout_donors::{LayoutDonorError, LayoutDonorLimits};
use crate::layout_visual::{
    MAX_VISUAL_PRIMITIVES, VisualAbsenceReason, VisualObservation, VisualPrimitive,
    VisualPrimitiveKind, VisualProvider, digest_bytes,
};
use da_parse::{LayoutBBox, LayoutDocument, LayoutPage};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, content::Content};
use std::collections::BTreeSet;

const CONTENT_END_SENTINEL: &str = "DANativeVisualContentEnd";

pub const NATIVE_PDF_VISUAL_PROVIDER: &str = "native-pdf-image-xobject-encoded";
pub const NATIVE_PDF_VISUAL_VERSION: &str = "lopdf-0.41.0/v1";

/// Existing geometry and visual carriers from one PDF invocation. The document
/// contains page geometry only; it must not replace or be merged into text layout.
#[derive(Debug, Clone, PartialEq)]
pub struct NativePdfVisualExtraction {
    pub document: LayoutDocument,
    pub visual: VisualObservation,
}

/// Observe encoded image source streams without enabling a parser route.
///
/// For offline, trusted-source use only. This function is not a memory or time
/// sandbox. lopdf may inflate object and cross-reference streams before
/// `Document::load_mem` returns; page content and operations are also decoded and
/// allocated before their admission checks. Untrusted input needs an external
/// resource boundary before calling this API.
///
/// Unsupported placement returns a terminal visual absence, never a partial
/// successful image inventory. Malformed source data and exceeded admission
/// limits are errors. Input bytes are checked before loading. `max_output_bytes`
/// otherwise rejects decoded content or image streams only after allocation;
/// `max_tokens` rejects parsed operations and collected image occurrences. These
/// checks do not bound peak memory. `timeout_ms` is not enforced here.
///
/// A collision-refused end operator checks that the pinned parser traverses the
/// entire page content; it does not validate every operator's PDF semantics.
/// Unwinding panics from `Content::decode` become malformed-content errors. This
/// does not catch aborts, allocation failure, or panics from `Document::load_mem`.
pub fn extract_native_pdf_visual(
    pdf_bytes: &[u8],
    document_id: &str,
    limits: LayoutDonorLimits,
) -> Result<NativePdfVisualExtraction, LayoutDonorError> {
    check_size(pdf_bytes.len(), limits.max_output_bytes, "pdf_bytes")?;
    let pdf = Document::load_mem(pdf_bytes).map_err(|_| malformed("PDF load"))?;
    if pdf.was_encrypted() || pdf.is_encrypted() {
        return Err(LayoutDonorError::Encrypted);
    }
    let source_digest = digest_bytes(pdf_bytes);
    let empty = LayoutDocument::new(document_id, source_digest.clone(), Vec::new())?;
    let pages = pdf.get_pages();
    if pages.is_empty() {
        return Err(malformed("PDF has no pages"));
    }
    check_size(pages.len(), limits.max_pages, "pages")?;
    let mut geometry = Vec::with_capacity(pages.len());
    for (&page_number, &page_id) in &pages {
        let Some(page) = page_geometry(&pdf, page_id, page_number - 1)? else {
            return terminal(empty, VisualAbsenceReason::Unsupported);
        };
        geometry.push(page);
    }
    let document = LayoutDocument::new(document_id, source_digest, geometry)?;
    let provider = visual_provider();
    let mut primitives = Vec::new();
    let mut operation_count = 0usize;
    let mut content_bytes = 0usize;
    for ((_, &page_id), page) in pages.iter().zip(&document.pages) {
        let bytes = page_content(&pdf, page_id, limits.max_output_bytes)?;
        content_bytes = content_bytes.saturating_add(bytes.len());
        check_size(
            content_bytes,
            limits.max_output_bytes,
            "decoded_content_bytes",
        )?;
        let Some(content) = decode_complete_content(&bytes)? else {
            return terminal(document, VisualAbsenceReason::Unsupported);
        };
        operation_count = operation_count.saturating_add(content.operations.len());
        check_size(operation_count, limits.max_tokens, "content_operations")?;
        // Positive axis-aligned CTM: scale_x, scale_y, translate_x, translate_y.
        let mut ctm = [1.0, 1.0, 0.0, 0.0];
        let mut stack = Vec::new();
        for (ordinal, operation) in content.operations.iter().enumerate() {
            match operation.operator.as_str() {
                "q" if operation.operands.is_empty() => stack.push(ctm),
                "Q" if operation.operands.is_empty() => {
                    ctm = stack.pop().ok_or_else(|| malformed("unbalanced Q"))?;
                }
                "cm" => {
                    let Some(matrix) = positive_axis_matrix(&operation.operands)? else {
                        return terminal(document, VisualAbsenceReason::Unsupported);
                    };
                    ctm = [
                        ctm[0] * matrix[0],
                        ctm[1] * matrix[1],
                        ctm[0] * matrix[2] + ctm[2],
                        ctm[1] * matrix[3] + ctm[3],
                    ];
                    if !ctm.iter().all(|value| value.is_finite()) {
                        return Err(malformed("non-finite CTM"));
                    }
                }
                "Do" => {
                    let [name] = operation.operands.as_slice() else {
                        return Err(malformed("XObject invocation"));
                    };
                    let name = name.as_name().map_err(|_| malformed("XObject name"))?;
                    let stream = image_stream(&pdf, page_id, name)?;
                    let Some(stream) = stream else {
                        return terminal(document, VisualAbsenceReason::Unsupported);
                    };
                    check_size(
                        stream.content.len(),
                        limits.max_output_bytes,
                        "image_stream_bytes",
                    )?;
                    if stream.content.is_empty()
                        || stream
                            .dict
                            .get(b"Width")
                            .and_then(Object::as_i64)
                            .unwrap_or(0)
                            <= 0
                        || stream
                            .dict
                            .get(b"Height")
                            .and_then(Object::as_i64)
                            .unwrap_or(0)
                            <= 0
                    {
                        return Err(malformed("empty image stream or invalid image dimensions"));
                    }
                    let bbox = LayoutBBox::from_bottom_left(
                        ctm[2],
                        ctm[3],
                        ctm[2] + ctm[0],
                        ctm[3] + ctm[1],
                        page.height,
                    )?;
                    let source_ordinal =
                        u32::try_from(ordinal).map_err(|_| malformed("image source ordinal"))?;
                    let primitive = VisualPrimitive::observed(
                        &document,
                        provider.clone(),
                        page.page_ordinal,
                        source_ordinal,
                        VisualPrimitiveKind::Image,
                        bbox,
                        digest_bytes(&stream.content),
                    )
                    .map_err(|_| malformed("image geometry or identity"))?;
                    primitives.push(primitive);
                    check_size(
                        primitives.len(),
                        limits.max_tokens.min(MAX_VISUAL_PRIMITIVES),
                        "visual_primitives",
                    )?;
                }
                "W" | "W*" | "gs" | "BI" | "Tr" => {
                    return terminal(document, VisualAbsenceReason::Unsupported);
                }
                "q" | "Q" => return Err(malformed("graphics-state operands")),
                _ => {}
            }
        }
        if !stack.is_empty() {
            return Err(malformed("unbalanced q"));
        }
    }
    if primitives.is_empty() {
        return terminal(document, VisualAbsenceReason::NotObserved);
    }
    let visual = VisualObservation::new(&document, primitives)
        .map_err(|_| malformed("visual observation"))?;
    Ok(NativePdfVisualExtraction { document, visual })
}

fn visual_provider() -> VisualProvider {
    VisualProvider::new(NATIVE_PDF_VISUAL_PROVIDER, NATIVE_PDF_VISUAL_VERSION)
        .expect("constant native PDF visual provider")
}

fn terminal(
    document: LayoutDocument,
    reason: VisualAbsenceReason,
) -> Result<NativePdfVisualExtraction, LayoutDonorError> {
    let visual = VisualObservation::terminal_absence(&document, visual_provider(), reason)
        .map_err(|_| malformed("visual absence"))?;
    Ok(NativePdfVisualExtraction { document, visual })
}

fn malformed(detail: &str) -> LayoutDonorError {
    LayoutDonorError::Malformed {
        detail: detail.to_owned(),
    }
}

fn check_size(size: usize, limit: usize, resource: &'static str) -> Result<(), LayoutDonorError> {
    if size > limit {
        return Err(LayoutDonorError::Oversize { resource, limit });
    }
    Ok(())
}

fn inherited<'a>(
    pdf: &'a Document,
    mut page_id: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>, LayoutDonorError> {
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(page_id) {
            return Err(malformed("page inheritance cycle"));
        }
        let dictionary = pdf
            .get_dictionary(page_id)
            .map_err(|_| malformed("page dictionary"))?;
        if dictionary.has(key) {
            return dictionary
                .get_deref(key, pdf)
                .map(Some)
                .map_err(|_| malformed("page attribute"));
        }
        if !dictionary.has(b"Parent") {
            return Ok(None);
        }
        page_id = dictionary
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|_| malformed("page parent"))?;
    }
}

fn number(value: &Object) -> Result<f64, LayoutDonorError> {
    let number = match value {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => return Err(malformed("non-numeric geometry")),
    };
    if !number.is_finite() {
        return Err(malformed("non-finite geometry"));
    }
    Ok(number)
}

fn box_values(value: &Object) -> Result<[f64; 4], LayoutDonorError> {
    let values = value.as_array().map_err(|_| malformed("page box"))?;
    let [left, bottom, right, top] = values.as_slice() else {
        return Err(malformed("page box width"));
    };
    let bounds = [number(left)?, number(bottom)?, number(right)?, number(top)?];
    if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return Err(malformed("inverted page box"));
    }
    Ok(bounds)
}

fn page_geometry(
    pdf: &Document,
    page_id: ObjectId,
    ordinal: u32,
) -> Result<Option<LayoutPage>, LayoutDonorError> {
    let media =
        inherited(pdf, page_id, b"MediaBox")?.ok_or_else(|| malformed("MediaBox missing"))?;
    let bounds = box_values(media)?;
    if bounds[0] != 0.0 || bounds[1] != 0.0 {
        return Ok(None);
    }
    let crop_box = match inherited(pdf, page_id, b"CropBox")? {
        Some(crop) => {
            if box_values(crop)? != bounds {
                return Ok(None);
            }
            Some(LayoutBBox::new(0.0, 0.0, bounds[2], bounds[3])?)
        }
        None => None,
    };
    if let Some(rotation) = inherited(pdf, page_id, b"Rotate")?
        && number(rotation)? != 0.0
    {
        return Ok(None);
    }
    let page = pdf
        .get_dictionary(page_id)
        .map_err(|_| malformed("page dictionary"))?;
    if page.has(b"UserUnit")
        && number(
            page.get_deref(b"UserUnit", pdf)
                .map_err(|_| malformed("UserUnit"))?,
        )? != 1.0
    {
        return Ok(None);
    }
    Ok(Some(LayoutPage::new(
        ordinal,
        bounds[2],
        bounds[3],
        0,
        crop_box,
        Vec::new(),
    )?))
}

fn page_content(
    pdf: &Document,
    page_id: ObjectId,
    limit: usize,
) -> Result<Vec<u8>, LayoutDonorError> {
    let page = pdf
        .get_dictionary(page_id)
        .map_err(|_| malformed("page dictionary"))?;
    if !page.has(b"Contents") {
        return Ok(Vec::new());
    }
    let contents = page
        .get_deref(b"Contents", pdf)
        .map_err(|_| malformed("content reference"))?;
    let streams = match contents {
        Object::Stream(stream) => vec![stream],
        Object::Array(items) => items
            .iter()
            .map(|item| {
                pdf.dereference(item)
                    .and_then(|(_, object)| object.as_stream())
                    .map_err(|_| malformed("content stream"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(malformed("content inventory")),
    };
    let mut bytes = Vec::new();
    for stream in streams {
        let content = stream
            .decompressed_content()
            .map_err(|_| malformed("content decoding"))?;
        check_size(
            bytes.len().saturating_add(content.len()).saturating_add(1),
            limit,
            "page_content_bytes",
        )?;
        bytes.extend_from_slice(&content);
        if !matches!(bytes.last(), Some(b'\r' | b'\n')) {
            bytes.push(b'\n');
        }
    }
    Ok(bytes)
}

// Pinned lopdf discards its parser remainder. A collision-free final operator
// proves traversal to the appended boundary; operands on it expose a dangling
// source operand. This is not semantic PDF validation or an allocation sandbox.
fn decode_complete_content(bytes: &[u8]) -> Result<Option<Content>, LayoutDonorError> {
    if bytes
        .windows(CONTENT_END_SENTINEL.len())
        .any(|window| window == CONTENT_END_SENTINEL.as_bytes())
    {
        return Ok(None);
    }
    let mut terminated = Vec::with_capacity(bytes.len() + CONTENT_END_SENTINEL.len() + 2);
    terminated.extend_from_slice(bytes);
    if !matches!(bytes.last(), Some(b'\r' | b'\n')) {
        terminated.push(b'\n');
    }
    terminated.extend_from_slice(CONTENT_END_SENTINEL.as_bytes());
    terminated.push(b'\n');
    let mut content = std::panic::catch_unwind(|| Content::decode(&terminated))
        .map_err(|_| malformed("page content decoder panic"))?
        .map_err(|_| malformed("page content"))?;
    let boundary = content
        .operations
        .pop()
        .ok_or_else(|| malformed("page content boundary"))?;
    if boundary.operator != CONTENT_END_SENTINEL || !boundary.operands.is_empty() {
        return Err(malformed("page content was not fully traversed"));
    }
    Ok(Some(content))
}

fn positive_axis_matrix(operands: &[Object]) -> Result<Option<[f64; 4]>, LayoutDonorError> {
    let [a, b, c, d, e, f] = operands else {
        return Err(malformed("CTM operand count"));
    };
    let matrix = [
        number(a)?,
        number(b)?,
        number(c)?,
        number(d)?,
        number(e)?,
        number(f)?,
    ];
    if matrix[1] != 0.0 || matrix[2] != 0.0 || matrix[0] <= 0.0 || matrix[3] <= 0.0 {
        return Ok(None);
    }
    Ok(Some([matrix[0], matrix[3], matrix[4], matrix[5]]))
}

fn image_stream<'a>(
    pdf: &'a Document,
    page_id: ObjectId,
    name: &[u8],
) -> Result<Option<&'a Stream>, LayoutDonorError> {
    let resources = inherited(pdf, page_id, b"Resources")?
        .ok_or_else(|| malformed("XObject resources missing"))?
        .as_dict()
        .map_err(|_| malformed("resource dictionary"))?;
    let xobjects: &Dictionary = resources
        .get_deref(b"XObject", pdf)
        .and_then(Object::as_dict)
        .map_err(|_| malformed("XObject dictionary"))?;
    let stream = xobjects
        .get_deref(name, pdf)
        .and_then(Object::as_stream)
        .map_err(|_| malformed("Image XObject stream"))?;
    let subtype = stream
        .dict
        .get(b"Subtype")
        .and_then(Object::as_name)
        .map_err(|_| malformed("XObject subtype"))?;
    if subtype != b"Image" || stream.dict.has(b"F") {
        return Ok(None);
    }
    Ok(Some(stream))
}

#[cfg(test)]
mod coverage_smoke {
    use super::*;

    #[test]
    fn garbage_bytes_are_refused_not_parsed() {
        let error = extract_native_pdf_visual(
            b"%PDF-not-a-real-document",
            "coverage-smoke",
            LayoutDonorLimits::default(),
        )
        .expect_err("non-PDF bytes must be refused");
        assert!(matches!(error, LayoutDonorError::Malformed { .. }));
    }
}
