//! Pure, bounded composition of a source-bound native fulltext shadow candidate.
//!
//! This is deliberately an orchestration boundary, not a parser or runtime
//! route.  It consumes a complete Java TEI envelope and already typed leaf
//! outcomes.  Only independently admitted header and reference outcomes may
//! replace their corresponding TEI subtrees; every other family remains byte
//! accounted Java fallback.
use crate::{
    HeaderCompositionOutcome, NativeVisualHeadOutcome, ReferenceCompositionOutcome,
    SegmentationOutcome, TeiEnvelope, TeiIdentity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const NATIVE_FULLTEXT_COMPOSITION_SCHEMA_VERSION: &str =
    "da-parse.native-fulltext-composition/v1";
pub const NATIVE_FULLTEXT_MAX_JAVA_BYTES: usize = 32 * 1024 * 1024;
pub const NATIVE_FULLTEXT_MAX_COMPONENT_BYTES: usize = 4 * 1024 * 1024;
pub const NATIVE_FULLTEXT_MAX_XML_DEPTH: usize = 128;
pub const NATIVE_FULLTEXT_FALLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulltextComponent {
    Header,
    References,
    DocumentSegmentation,
    Figure,
    Table,
    Formula,
    LearningPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionDisposition {
    Passed,
    Fallback,
    Rejected,
    Unsupported,
    NotObserved,
    InvalidEvidence,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FulltextCompositionIdentity {
    pub source_sha256: String,
    pub profile_fingerprint: String,
    pub config_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FulltextAdmissionReceipt {
    pub component: FulltextComponent,
    pub producer: String,
    pub disposition: AdmissionDisposition,
    pub evidence_state: String,
    pub schema_version: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub fallback_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct S17LineageReceipt {
    pub source: String,
    pub artifact_sha256: String,
    pub live_ruvllm_receipt: bool,
    pub output_affecting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FulltextComponentManifest {
    pub component: FulltextComponent,
    pub producer: String,
    pub disposition: AdmissionDisposition,
    pub evidence_digest: String,
    pub byte_count: usize,
    pub sha256: String,
    pub truncated: bool,
    pub fallback_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFulltextManifest {
    pub schema_version: String,
    pub identity: FulltextCompositionIdentity,
    pub components: Vec<FulltextComponentManifest>,
    pub native_components: usize,
    pub fallback_components: usize,
    pub java_fallback_bytes: usize,
    pub java_fallback_sha256: String,
    pub s17_output_affecting: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeFulltextCompositionInput {
    pub java_fulltext: TeiEnvelope,
    pub identity: FulltextCompositionIdentity,
    pub header: HeaderCompositionOutcome,
    pub references: ReferenceCompositionOutcome,
    pub segmentation: Option<SegmentationOutcome>,
    pub figure: Option<NativeVisualHeadOutcome>,
    pub table: Option<NativeVisualHeadOutcome>,
    pub formula: Option<NativeVisualHeadOutcome>,
    pub admissions: Vec<FulltextAdmissionReceipt>,
    pub s17_lineage: Option<S17LineageReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFulltextCompositionError {
    InvalidIdentity(&'static str),
    WrongBackend,
    Oversized {
        component: &'static str,
        bytes: usize,
    },
    MissingAdmission(FulltextComponent),
    StaleAdmission(FulltextComponent),
    InvalidAdmission(FulltextComponent),
    InvalidSchema(FulltextComponent),
    MalformedXml,
    ForbiddenExternalEntity,
    DuplicateId,
    UnbalancedXml,
    AmbiguousSplice(String),
    /// The splice produced a document the module's own structural parser or its
    /// well-formedness checker rejects.  Composed bytes are never accepted on
    /// trust: the emitter re-reads its own output before `Accepted`, and the
    /// message names the enforced class that failed (see `validate_well_formed`).
    ComposedOutputNotWellFormed(String),
    InvalidUtf8,
    ChangedReadingOrder,
    ArtifactDigestMismatch {
        component: &'static str,
        claimed: String,
        observed: String,
    },
}
impl fmt::Display for NativeFulltextCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native fulltext composition rejected input: {self:?}")
    }
}
impl std::error::Error for NativeFulltextCompositionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFulltextCandidate {
    pub envelope: TeiEnvelope,
    pub manifest: NativeFulltextManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFulltextCompositionOutcome {
    Accepted(NativeFulltextCandidate),
    Rejected {
        fallback: TeiEnvelope,
        error: NativeFulltextCompositionError,
    },
}

pub fn compose_native_fulltext(
    input: NativeFulltextCompositionInput,
) -> NativeFulltextCompositionOutcome {
    let fallback = input.java_fulltext.clone();
    match compose_inner(input) {
        Ok(candidate) => NativeFulltextCompositionOutcome::Accepted(candidate),
        Err(error) => NativeFulltextCompositionOutcome::Rejected { fallback, error },
    }
}

fn compose_inner(
    input: NativeFulltextCompositionInput,
) -> Result<NativeFulltextCandidate, NativeFulltextCompositionError> {
    validate_identity(&input)?;
    let java = input.java_fulltext.raw_tei();
    if java.len() > NATIVE_FULLTEXT_MAX_JAVA_BYTES {
        return Err(NativeFulltextCompositionError::Oversized {
            component: "java_fulltext",
            bytes: java.len(),
        });
    }
    let text =
        std::str::from_utf8(java).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    let ranges = XmlDocument::parse(text)?;

    let mut bytes = java.to_vec();
    let mut components = Vec::with_capacity(7);
    let header_receipt = admission(&input, FulltextComponent::Header)?;
    let refs_receipt = admission(&input, FulltextComponent::References)?;
    let mut replacements = Vec::new();
    if is_admitted(&header_receipt, &input.identity) {
        let HeaderCompositionOutcome::Accepted { composition } = &input.header else {
            return Err(NativeFulltextCompositionError::InvalidAdmission(
                FulltextComponent::Header,
            ));
        };
        let range = ranges.unique("teiHeader")?;
        let existing_header = &java[range.0..range.1];
        let fragment = header_xml(composition, existing_header)?;
        replacements.push((
            range,
            fragment,
            FulltextComponent::Header,
            header_receipt.clone(),
        ));
    }
    if is_admitted(&refs_receipt, &input.identity) {
        let ReferenceCompositionOutcome::Accepted { composition } = &input.references else {
            return Err(NativeFulltextCompositionError::InvalidAdmission(
                FulltextComponent::References,
            ));
        };
        let fragment = references_xml(composition)?;
        let range = ranges.reference_range()?;
        replacements.push((
            range,
            fragment,
            FulltextComponent::References,
            refs_receipt.clone(),
        ));
    }
    replacements.sort_by_key(|(range, _, _, _)| range.0);
    for pair in replacements.windows(2) {
        if pair[0].0.1 > pair[1].0.0 {
            return Err(NativeFulltextCompositionError::AmbiguousSplice(
                "overlap".into(),
            ));
        }
    }
    if let Some(body) = ranges.optional_unique("body")? {
        for (range, _, _, _) in &replacements {
            if range.0 < body.1 && range.1 > body.0 {
                return Err(NativeFulltextCompositionError::ChangedReadingOrder);
            }
        }
    }
    for (range, fragment, component, receipt) in replacements.into_iter().rev() {
        if fragment.len() > NATIVE_FULLTEXT_MAX_COMPONENT_BYTES {
            return Err(NativeFulltextCompositionError::Oversized {
                component: "native",
                bytes: fragment.len(),
            });
        }
        let fragment_bytes = fragment.as_bytes();
        let observed = digest(fragment_bytes);
        if !receipt.artifact_sha256.is_empty()
            && receipt.artifact_sha256 != observed.strip_prefix("sha256:").unwrap_or(&observed)
        {
            return Err(NativeFulltextCompositionError::ArtifactDigestMismatch {
                component: component_name(component),
                claimed: receipt.artifact_sha256,
                observed,
            });
        }
        bytes.splice(range.0..range.1, fragment_bytes.iter().copied());
        components.push(manifest_entry(component, &receipt, fragment_bytes));
    }
    validate_emitted(&bytes)?;
    for (component, receipt, outcome) in [
        (
            FulltextComponent::Header,
            header_receipt,
            is_admitted(
                &admission(&input, FulltextComponent::Header)?,
                &input.identity,
            ),
        ),
        (
            FulltextComponent::References,
            refs_receipt,
            is_admitted(
                &admission(&input, FulltextComponent::References)?,
                &input.identity,
            ),
        ),
    ] {
        if !outcome {
            components.push(fallback_entry(component, &receipt, java));
        }
    }
    for component in [
        FulltextComponent::DocumentSegmentation,
        FulltextComponent::Figure,
        FulltextComponent::Table,
        FulltextComponent::Formula,
    ] {
        let receipt = admission(&input, component)?;
        components.push(fallback_entry(component, &receipt, java));
    }
    let learning = admission(&input, FulltextComponent::LearningPolicy)?;
    components.push(fallback_entry(
        FulltextComponent::LearningPolicy,
        &learning,
        java,
    ));
    components.sort_by_key(|m| m.component as u8);
    let native_components = components
        .iter()
        .filter(|m| matches!(m.disposition, AdmissionDisposition::Passed))
        .count();
    let fallback_components = components.len() - native_components;
    let java_fallback_sha256 = digest(java);
    let identity = input.identity;
    let candidate_identity = TeiIdentity::new(
        "native-fulltext-composition-shadow",
        identity.profile_fingerprint.clone(),
    );
    Ok(NativeFulltextCandidate {
        envelope: TeiEnvelope::new(bytes, candidate_identity),
        manifest: NativeFulltextManifest {
            schema_version: NATIVE_FULLTEXT_COMPOSITION_SCHEMA_VERSION.into(),
            identity,
            components,
            native_components,
            fallback_components,
            java_fallback_bytes: java.len(),
            java_fallback_sha256,
            s17_output_affecting: false,
        },
    })
}

fn validate_identity(
    input: &NativeFulltextCompositionInput,
) -> Result<(), NativeFulltextCompositionError> {
    let id = &input.identity;
    if !valid_sha(&id.source_sha256)
        || id.profile_fingerprint.is_empty()
        || id.config_fingerprint.is_empty()
    {
        return Err(NativeFulltextCompositionError::InvalidIdentity(
            "source/profile/config",
        ));
    }
    if input.java_fulltext.identity().backend_id() != crate::GROBID_HTTP_BACKEND_ID {
        return Err(NativeFulltextCompositionError::WrongBackend);
    }
    if input.java_fulltext.identity().profile_fingerprint() != id.profile_fingerprint {
        return Err(NativeFulltextCompositionError::InvalidIdentity("profile"));
    }
    if let Some(lineage) = &input.s17_lineage
        && (!valid_sha(&lineage.artifact_sha256) || lineage.output_affecting)
    {
        return Err(NativeFulltextCompositionError::InvalidIdentity(
            "s17 lineage",
        ));
    }
    Ok(())
}
fn admission(
    input: &NativeFulltextCompositionInput,
    component: FulltextComponent,
) -> Result<FulltextAdmissionReceipt, NativeFulltextCompositionError> {
    input
        .admissions
        .iter()
        .find(|r| r.component == component)
        .cloned()
        .ok_or(NativeFulltextCompositionError::MissingAdmission(component))
}
fn is_admitted(r: &FulltextAdmissionReceipt, id: &FulltextCompositionIdentity) -> bool {
    r.disposition == AdmissionDisposition::Passed
        && r.evidence_state == "passing"
        && r.source_sha256 == id.source_sha256
        && (r.artifact_sha256.is_empty() || valid_sha(&r.artifact_sha256))
        && r.fallback_target == NATIVE_FULLTEXT_FALLBACK_TARGET
        && r.schema_version == "native-composition.outcome/v1"
}
fn manifest_entry(
    c: FulltextComponent,
    r: &FulltextAdmissionReceipt,
    bytes: &[u8],
) -> FulltextComponentManifest {
    FulltextComponentManifest {
        component: c,
        producer: r.producer.clone(),
        disposition: AdmissionDisposition::Passed,
        evidence_digest: digest(bytes),
        byte_count: bytes.len(),
        sha256: digest(bytes),
        truncated: false,
        fallback_target: r.fallback_target.clone(),
    }
}
fn fallback_entry(
    c: FulltextComponent,
    r: &FulltextAdmissionReceipt,
    bytes: &[u8],
) -> FulltextComponentManifest {
    FulltextComponentManifest {
        component: c,
        producer: r.producer.clone(),
        disposition: if r.disposition == AdmissionDisposition::Passed {
            AdmissionDisposition::Fallback
        } else {
            r.disposition
        },
        evidence_digest: digest(bytes),
        byte_count: bytes.len(),
        sha256: digest(bytes),
        truncated: false,
        fallback_target: r.fallback_target.clone(),
    }
}
/// The typed refusal for an emission that fails an enforced well-formedness
/// constraint.  It reuses the composer's existing `ComposedOutputNotWellFormed`
/// class, so the fallback path (byte-identical Java envelope) and every consumer
/// stay unchanged while the message names the class that failed.
fn wfc(detail: impl Into<String>) -> NativeFulltextCompositionError {
    NativeFulltextCompositionError::ComposedOutputNotWellFormed(detail.into())
}

/// XML 1.0 `Char`.
fn is_legal_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&c)
        || ('\u{E000}'..='\u{FFFD}').contains(&c)
        || ('\u{10000}'..='\u{10FFFF}').contains(&c)
}

/// `NameStartChar` without `:` — the colon is the `QName` separator and is
/// checked by `qname_parts`.
fn is_name_start_char(c: char) -> bool {
    c == '_'
        || c.is_ascii_alphabetic()
        || matches!(
            c,
            '\u{C0}'..='\u{D6}'
                | '\u{D8}'..='\u{F6}'
                | '\u{F8}'..='\u{2FF}'
                | '\u{370}'..='\u{37D}'
                | '\u{37F}'..='\u{1FFF}'
                | '\u{200C}'..='\u{200D}'
                | '\u{2070}'..='\u{218F}'
                | '\u{2C00}'..='\u{2FEF}'
                | '\u{3001}'..='\u{D7FF}'
                | '\u{F900}'..='\u{FDCF}'
                | '\u{FDF0}'..='\u{FFFD}'
                | '\u{10000}'..='\u{EFFFF}'
        )
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || c.is_ascii_digit()
        || matches!(
            c,
            '-' | '.' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}'
        )
}

/// XML `Name`, which is also `NCName` here because `:` is not a `NameChar`.
fn is_legal_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if is_name_start_char(first) => {}
        _ => return false,
    }
    chars.all(is_name_char)
}

/// `QName` decomposition into `(prefix, local)`, or `None` when the name is not a
/// legal `QName` (at most one colon, legal `NCName` on both sides).
fn qname_parts(name: &str) -> Option<(Option<&str>, &str)> {
    let mut parts = name.split(':');
    let first = parts.next()?;
    let second = parts.next();
    let third = parts.next();
    match (second, third) {
        (None, None) if is_legal_name(first) => Some((None, first)),
        (Some(local), None) if is_legal_name(first) && is_legal_name(local) => {
            Some((Some(first), local))
        }
        _ => None,
    }
}

/// One open element of the well-formedness scan: the name as written plus the
/// namespace declarations its start tag carried.  A declaration is in scope for
/// the element's own name and attributes, not only for its children, so both are
/// checked against `declarations` plus the enclosing elements.
struct OpenElement {
    name: String,
    /// `("", uri)` for `xmlns="uri"`, `(prefix, uri)` for `xmlns:prefix="uri"`.
    declarations: Vec<(String, String)>,
}

/// Resolve `prefix` against the inner-most declaration in scope: the element's
/// own declarations first, then the enclosing elements from the inside out.  The
/// `xml` prefix is bound whatever the document says, so it needs no declaration.
fn resolve_prefix<'a>(
    stack: &'a [OpenElement],
    local: &'a [(String, String)],
    prefix: &str,
) -> Option<&'a str> {
    if prefix == "xml" {
        return Some("xml");
    }
    local
        .iter()
        .rev()
        .find(|(candidate, _)| candidate == prefix)
        .or_else(|| {
            stack.iter().rev().find_map(|element| {
                element
                    .declarations
                    .iter()
                    .rev()
                    .find(|(candidate, _)| candidate == prefix)
            })
        })
        .map(|(_, uri)| uri.as_str())
}

/// `&` must start a legal reference.
///
/// A bare `&` is an A-XML class measured on a real preserved TEI: the composer
/// copies preserved source bytes verbatim, so without this check an ill-formed
/// source would be republished as `Accepted`.
///
/// Only the five predefined entities may appear.  `<!DOCTYPE`/`<!ENTITY` are
/// forbidden, so no other entity can be declared in a document this module emits;
/// a named entity it cannot resolve is a rejection, never a silent pass.
fn validate_references(data: &str) -> Result<(), NativeFulltextCompositionError> {
    let mut at = 0usize;
    while let Some(relative) = data[at..].find('&') {
        let start = at + relative;
        let rest = &data[start + 1..];
        let Some(end) = rest.find(';') else {
            return Err(wfc("`&` does not start a character or entity reference"));
        };
        let name = &rest[..end];
        let legal = if let Some(hex) = name.strip_prefix("#x") {
            !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
        } else if let Some(decimal) = name.strip_prefix('#') {
            !decimal.is_empty() && decimal.chars().all(|c| c.is_ascii_digit())
        } else {
            matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
        };
        if !legal {
            return Err(wfc(format!("`&{name};` is not a legal reference")));
        }
        let code_point = name
            .strip_prefix("#x")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| {
                name.strip_prefix('#')
                    .and_then(|decimal| decimal.parse().ok())
            });
        if let Some(code_point) = code_point
            && char::from_u32(code_point).is_none_or(|c| !is_legal_xml_char(c))
        {
            return Err(wfc(format!(
                "`&{name};` is not a legal XML character reference"
            )));
        }
        at = start + 1 + end + 1;
    }
    Ok(())
}

/// `AttValue`: a raw `<` is forbidden and every `&` must start a legal reference.
fn validate_attribute_value(value: &str) -> Result<(), NativeFulltextCompositionError> {
    if value.contains('<') {
        return Err(wfc("`<` is not allowed in an attribute value"));
    }
    validate_references(value)
}

/// Character data between tags: `]]>` may not appear literally, text outside the
/// document element must be whitespace, and `&` must start a legal reference.
fn validate_char_data(
    text: &str,
    from: usize,
    to: usize,
    inside_element: bool,
) -> Result<(), NativeFulltextCompositionError> {
    let data = &text[from..to];
    if data.contains("]]>") {
        return Err(wfc("`]]>` is not allowed in character data"));
    }
    if !inside_element && !data.chars().all(char::is_whitespace) {
        return Err(wfc("character data outside the document element"));
    }
    validate_references(data)
}

/// Well-formedness classes enforced before `Accepted`, and why this is not a
/// general XML processor.
///
/// The composer used to return `Accepted` on emissions an independent parser
/// rejected (measured S01/T04, at least the classes below).  `XmlDocument::parse`
/// is the structural range/identity scanner the splice itself is built on, so it
/// cannot be the arbiter of the bytes it helped select; this checker is a
/// separate pass over the emitted document, for the constraints a splice can
/// violate while copying source bytes verbatim:
///
/// * exactly one document element, and no non-whitespace character data outside it;
/// * `Name`/`QName` legality for elements, attributes, prefixes and PI targets;
/// * attribute lists in `Name S? Eq S? AttValue` form, quoted values, and no
///   duplicate attribute name on one element;
/// * `&` starts one of the five predefined entities or a numeric character
///   reference (a raw `&` is a measured class);
/// * namespace declarations and prefix bindings: every prefixed element or
///   attribute name must resolve in scope, so a prefix whose declaration did not
///   travel with its subtree is a typed rejection, not `Accepted`;
/// * `]]>` in character data, `--` inside a comment, CDATA only inside the
///   document element, the depth limit, and legal XML characters only;
/// * `<!DOCTYPE`/`<!ENTITY` stay forbidden.
///
/// It is deliberately not a validating parser: no DTD, no entity expansion, no
/// schema (`xmlns:xml` re-binding and URI-reference validity are outside this
/// list).  The offline independent oracle — `roxmltree`, a dev-dependency used by
/// the S01 contract tests — is what judges every adversarial emission beyond the
/// list, and this function is never called proof of the stronger claim.
fn validate_well_formed(text: &str) -> Result<(), NativeFulltextCompositionError> {
    if let Some(illegal) = text.chars().find(|c| !is_legal_xml_char(*c)) {
        return Err(wfc(format!(
            "illegal XML character U+{:04X}",
            illegal as u32
        )));
    }
    let mut stack: Vec<OpenElement> = Vec::new();
    let mut root_seen = false;
    let mut pos = 0usize;
    while let Some(relative) = text[pos..].find('<') {
        let start = pos + relative;
        validate_char_data(text, pos, start, !stack.is_empty())?;
        let rest = &text[start + 1..];
        match rest.chars().next() {
            Some('?') => {
                let close = text[start..]
                    .find("?>")
                    .ok_or_else(|| wfc("unterminated processing instruction"))?;
                let content = &text[start + 2..start + close];
                let target = content.split(char::is_whitespace).next().unwrap_or("");
                if qname_parts(target).is_none() {
                    return Err(wfc(format!(
                        "illegal processing-instruction target `{target}`"
                    )));
                }
                if target == "xml" && start != 0 {
                    return Err(wfc(
                        "the `xml` processing-instruction target is reserved for the declaration",
                    ));
                }
                pos = start + close + 2;
            }
            Some('!') => {
                if rest.starts_with("!--") {
                    let close = text[start..]
                        .find("-->")
                        .ok_or_else(|| wfc("unterminated comment"))?;
                    if text[start + 4..start + close].contains("--") {
                        return Err(wfc("`--` inside a comment"));
                    }
                    pos = start + close + 3;
                } else if rest.starts_with("![CDATA[") {
                    if stack.is_empty() {
                        return Err(wfc("CDATA section outside the document element"));
                    }
                    let close = text[start..]
                        .find("]]>")
                        .ok_or_else(|| wfc("unterminated CDATA section"))?;
                    pos = start + close + 3;
                } else if rest.starts_with("!DOCTYPE") || rest.starts_with("!ENTITY") {
                    return Err(NativeFulltextCompositionError::ForbiddenExternalEntity);
                } else {
                    return Err(wfc("unsupported `<!` markup declaration"));
                }
            }
            Some('/') => {
                let end = tag_end(text, start + 1).ok_or_else(|| wfc("unterminated close tag"))?;
                let name = text[start + 2..end].trim_end();
                if name.is_empty()
                    || name.contains(char::is_whitespace)
                    || qname_parts(name).is_none()
                {
                    return Err(wfc(format!("illegal close tag `</{name}>`")));
                }
                let Some(open) = stack.pop() else {
                    return Err(wfc(format!(
                        "close tag `</{name}>` without an open element"
                    )));
                };
                if open.name != name {
                    return Err(wfc(format!(
                        "close tag `</{name}>` does not match the open element `<{}>`",
                        open.name
                    )));
                }
                pos = end + 1;
            }
            Some(_) => {
                let end = tag_end(text, start + 1).ok_or_else(|| wfc("unterminated open tag"))?;
                let body = tag_attributes(&text[start + 1..end]);
                let name_end = body.find(char::is_whitespace).unwrap_or(body.len());
                let name = &body[..name_end];
                let Some((prefix, _local)) = qname_parts(name) else {
                    return Err(wfc(format!("illegal element name `{name}`")));
                };
                if prefix == Some("xmlns") {
                    return Err(wfc(format!(
                        "the `xmlns` prefix cannot be an element prefix (`{name}`)"
                    )));
                }
                let attributes = parse_attributes(&body[name_end..])
                    .map_err(|_| wfc(format!("illegal attribute list on `<{name}>`")))?;
                // Declarations are collected first: they are in scope for this
                // element's own name and attributes whatever their order is.
                let mut local: Vec<(String, String)> = Vec::new();
                let mut seen: Vec<&str> = Vec::new();
                for (attribute, value) in &attributes {
                    if seen.contains(attribute) {
                        return Err(wfc(format!(
                            "duplicate attribute `{attribute}` on `<{name}>`"
                        )));
                    }
                    seen.push(attribute);
                    if *attribute == "xmlns" {
                        local.push((String::new(), (*value).to_owned()));
                    } else if let Some(declared) = attribute.strip_prefix("xmlns:") {
                        if !is_legal_name(declared) || declared == "xmlns" {
                            return Err(wfc(format!(
                                "illegal namespace prefix `{declared}` on `<{name}>`"
                            )));
                        }
                        if value.is_empty() {
                            return Err(wfc(format!(
                                "namespace prefix `{declared}` cannot be undeclared"
                            )));
                        }
                        local.push((declared.to_owned(), (*value).to_owned()));
                    }
                }
                for (attribute, value) in &attributes {
                    validate_attribute_value(value)?;
                    if *attribute == "xmlns" || attribute.starts_with("xmlns:") {
                        continue;
                    }
                    let Some((attribute_prefix, _)) = qname_parts(attribute) else {
                        return Err(wfc(format!("illegal attribute name `{attribute}`")));
                    };
                    if let Some(attribute_prefix) = attribute_prefix
                        && resolve_prefix(&stack, &local, attribute_prefix).is_none()
                    {
                        return Err(wfc(format!(
                            "unbound namespace prefix `{attribute_prefix}` on attribute \
                             `{attribute}` in `<{name}>`"
                        )));
                    }
                }
                if let Some(prefix) = prefix
                    && resolve_prefix(&stack, &local, prefix).is_none()
                {
                    return Err(wfc(format!(
                        "unbound namespace prefix `{prefix}` on element `{name}`"
                    )));
                }
                let empty = is_self_closing(&text[start + 1..end]);
                if !empty && stack.len() >= NATIVE_FULLTEXT_MAX_XML_DEPTH {
                    return Err(wfc("element nesting exceeded the depth limit"));
                }
                if stack.is_empty() {
                    if root_seen {
                        return Err(wfc("more than one document element"));
                    }
                    root_seen = true;
                }
                if !empty {
                    stack.push(OpenElement {
                        name: name.to_owned(),
                        declarations: local,
                    });
                }
                pos = end + 1;
            }
            None => return Err(wfc("`<` at the end of the document")),
        }
    }
    validate_char_data(text, pos, text.len(), !stack.is_empty())?;
    if let Some(unclosed) = stack.first() {
        return Err(wfc(format!("unclosed element `<{}>`", unclosed.name)));
    }
    if !root_seen {
        return Err(wfc("no document element"));
    }
    Ok(())
}

/// Re-read the emitted document before it may be accepted.
///
/// `Accepted` is a claim about the bytes the candidate carries, so the emitter
/// re-reads its own output first:
///
/// 1. `XmlDocument::parse` — the structural range/identity scanner the splice was
///    built on — covers tag shape, element balance and `xml:id`/`id` uniqueness
///    across the whole document, which is also what catches a preserved source id
///    colliding with a synthesized `author-N`/`bibl-N`;
/// 2. `validate_well_formed` — a separate pass enforcing the well-formedness
///    class list documented on it, including namespace prefix bindings.
///
/// Both are needed, and neither is the whole claim: (2) exists because (1) shares
/// the composer's own heritage and cannot be the arbiter of the bytes it helped
/// select, and (2) is itself an enumerated class list rather than a validating
/// parser, so the offline independent oracle (`roxmltree` in the S01 contract
/// tests) remains the judge of the stronger claim.  A failure of either is typed
/// and falls back to the byte-identical Java envelope instead of publishing a
/// damaged candidate.
fn validate_emitted(bytes: &[u8]) -> Result<(), NativeFulltextCompositionError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    XmlDocument::parse(text).map(|_| ()).map_err(|error| {
        NativeFulltextCompositionError::ComposedOutputNotWellFormed(format!("{error:?}"))
    })?;
    validate_well_formed(text)
}

fn component_name(component: FulltextComponent) -> &'static str {
    match component {
        FulltextComponent::Header => "header",
        FulltextComponent::References => "references",
        FulltextComponent::DocumentSegmentation => "document_segmentation",
        FulltextComponent::Figure => "figure",
        FulltextComponent::Table => "table",
        FulltextComponent::Formula => "formula",
        FulltextComponent::LearningPolicy => "learning_policy",
    }
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn header_xml(
    c: &crate::HeaderCompositionCandidate,
    existing: &[u8],
) -> Result<String, NativeFulltextCompositionError> {
    let existing =
        std::str::from_utf8(existing).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    let document = XmlDocument::parse(existing)?;
    // The source header open tag carries declarations that the preserved
    // subtrees depend on (`xmlns:*`), plus `xml:lang` and any other attribute
    // the Java envelope was produced with.  Dropping them left preserved
    // subtrees holding unbound prefixes, so the declaration is reproduced
    // verbatim.
    let attributes = document.open_tag_attributes(existing, "teiHeader")?;
    // The composer synthesizes `fileDesc` and `titleStmt`, so a namespace
    // declaration carried by the *source* wrapper would be dropped with the
    // wrapper while its children are moved into the composed one: the preserved
    // `ext:` descendants would then hold an unbound prefix and an independent
    // parser would reject the emission even though this module accepted it
    // (measured for S01/T03: `an unknown namespace prefix 'ext' at 1:202` for a
    // declaration on `fileDesc` and `1:153` for one on `titleStmt`).  Only
    // `xmlns`/`xmlns:*` are reproduced, so no other wrapper attribute — for
    // example an `xml:id` that could collide with a synthesized id — is copied.
    // Where the source wrappers are, if they exist at all.  Only a *direct*
    // child can be the wrapper the composer synthesizes: a same-named element
    // nested deeper belongs to its own parent and must travel with it.  Matching
    // by name anywhere in the document instead let the only `titleStmt` — one
    // under `fileDesc/seriesStmt` — be taken as the wrapper, so its children were
    // moved into the composed title statement *and* the enclosing `seriesStmt`
    // was carried over as a `fileDesc` child, duplicating the nested `<title>`
    // (measured S01/T05).
    let file_desc_index = document.direct_child_index_opt("teiHeader", "fileDesc")?;
    let title_stmt_index = match file_desc_index {
        Some(_) => {
            // A `titleStmt` at the header level while a `fileDesc` exists cannot
            // be placed: the composer synthesizes one title statement inside the
            // synthesized file description, and re-parenting the header-level one
            // would be a guess.
            if document
                .direct_child_index_opt("teiHeader", "titleStmt")?
                .is_some()
            {
                return Err(NativeFulltextCompositionError::AmbiguousSplice(
                    "teiHeader/titleStmt".into(),
                ));
            }
            document.direct_child_index_opt("fileDesc", "titleStmt")?
        }
        None => document.direct_child_index_opt("teiHeader", "titleStmt")?,
    };
    // The composer synthesizes `fileDesc` and `titleStmt`, so a namespace
    // declaration carried by the *source* wrapper would be dropped with the
    // wrapper while its children are moved into the composed one: the preserved
    // `ext:` descendants would then hold an unbound prefix and an independent
    // parser would reject the emission even though this module accepted it
    // (measured for S01/T03: `an unknown namespace prefix 'ext' at 1:202` for a
    // declaration on `fileDesc` and `1:153` for one on `titleStmt`).  Only
    // `xmlns`/`xmlns:*` are reproduced, so no other wrapper attribute — for
    // example an `xml:id` that could collide with a synthesized id — is copied.
    // The declaration is read from the *located* wrapper, not from a same-named
    // element nested elsewhere.
    let file_desc_declarations = document.namespace_declarations_at(existing, file_desc_index)?;
    let title_stmt_declarations = document.namespace_declarations_at(existing, title_stmt_index)?;
    // Placement is structural, never name-guessed.  The synthesized skeleton is
    // `teiHeader > fileDesc > titleStmt`, so a source subtree is re-inserted
    // under the wrapper it actually came from:
    //
    //   A = content of the source `titleStmt`  -> inside the composed titleStmt
    //   B = content of the source `fileDesc`   -> after the composed titleStmt,
    //       minus `titleStmt`                      inside the composed fileDesc
    //   C = content of the source `teiHeader`  -> after the composed fileDesc
    //       minus `fileDesc` and `titleStmt`
    //
    // A, B and C are pairwise disjoint (each element has exactly one parent) and
    // together they cover every byte of the source `teiHeader` except the two
    // wrappers the composer synthesizes, so no subtree and no non-element child
    // is duplicated and none is dropped.  Segments are byte intervals, not just
    // element spans, so direct comments, processing instructions and the
    // significant text/whitespace between children are conserved too.  A missing
    // `fileDesc`/`titleStmt` just leaves its set empty; an *ambiguous* wrapper
    // cannot be placed without guessing, so it is a typed rejection rather than a
    // silent re-parenting.
    let title_stmt = document.content_segments(title_stmt_index, &[]);
    let file_desc = document.content_segments(file_desc_index, &["titleStmt"]);
    let header = document.content_segments(
        document.element_index_opt("teiHeader")?,
        &["fileDesc", "titleStmt"],
    );
    let mut out = String::from("<teiHeader");
    out.push_str(attributes);
    out.push_str("><fileDesc");
    out.push_str(&file_desc_declarations);
    out.push_str("><titleStmt");
    out.push_str(&title_stmt_declarations);
    out.push('>');
    for a in &c.candidate.authors {
        out.push_str("<author xml:id=\"author-");
        out.push_str(&a.ordinal.to_string());
        out.push_str("\">");
        if let Some(g) = &a.name.given_name {
            out.push_str("<forename>");
            out.push_str(&esc(g));
            out.push_str("</forename>");
        }
        out.push_str("<surname>");
        out.push_str(&esc(&a.name.surname));
        out.push_str("</surname></author>");
    }
    for (start, end) in title_stmt {
        out.push_str(&existing[start..end]);
    }
    out.push_str("</titleStmt><date when=\"");
    out.push_str(&c.candidate.date.year.to_string());
    if let Some(month) = c.candidate.date.month {
        out.push_str(&format!("-{month:02}"));
    }
    if let Some(day) = c.candidate.date.day {
        out.push_str(&format!("-{day:02}"));
    }
    out.push_str("\"/>");
    for (start, end) in file_desc {
        out.push_str(&existing[start..end]);
    }
    out.push_str("</fileDesc>");
    for (start, end) in header {
        out.push_str(&existing[start..end]);
    }
    out.push_str("</teiHeader>");
    Ok(out)
}
fn references_xml(
    c: &crate::ReferenceCompositionCandidate,
) -> Result<String, NativeFulltextCompositionError> {
    for anchor in &c.candidate.anchors {
        if let Some(target) = &anchor.target {
            let target = target.strip_prefix('#').unwrap_or(target);
            let resolves = c
                .candidate
                .references
                .iter()
                .any(|reference| format!("bibl-{}", reference.ordinal) == target);
            if !resolves {
                return Err(NativeFulltextCompositionError::AmbiguousSplice(format!(
                    "unresolved bibliography target: {target}"
                )));
            }
        }
    }
    let mut out = String::from("<listBibl>");
    for r in &c.candidate.references {
        out.push_str("<biblStruct xml:id=\"bibl-");
        out.push_str(&r.ordinal.to_string());
        out.push_str("\">");
        out.push_str(&esc(&r.source));
        out.push_str("</biblStruct>");
    }
    out.push_str("</listBibl>");
    Ok(out)
}

/// Index of the `>` that closes the tag whose body starts at `from`.
///
/// A `>` inside a quoted attribute value is legal XML and must not end the tag:
/// a naive `find('>')` would move every following offset into the middle of an
/// attribute, which is exactly the class of mistake that corrupts a splice.
fn tag_end(s: &str, from: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (offset, c) in s[from..].char_indices() {
        match quote {
            Some(open) => {
                if c == open {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '>' => return Some(from + offset),
                _ => {}
            },
        }
    }
    None
}

/// Whether a tag body closes itself, ignoring a `/` inside a quoted value.
fn is_self_closing(body: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut last = None;
    for c in body.chars() {
        match quote {
            Some(open) => {
                if c == open {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                c if c.is_whitespace() => {}
                c => last = Some(c),
            },
        }
    }
    last == Some('/')
}

/// Open-tag body without the tag's own trailing `/`.
///
/// `is_self_closing` is quote-aware, so `<a b="x/"/>` loses only the tag's slash
/// and keeps the attribute value intact, while `<a b="x/">` keeps both.  A bare
/// `strip_suffix('/')` would truncate the value instead.
fn tag_attributes(body: &str) -> &str {
    let trimmed = body.trim_end();
    if is_self_closing(trimmed) {
        trimmed[..trimmed.len() - 1].trim_end()
    } else {
        trimmed
    }
}

/// Attribute `(name, value)` pairs of an open-tag attribute text.
///
/// The XML grammar is `Name S? Eq S? AttValue`: whitespace is legal on both sides
/// of `=`, and every value is quoted.  A lexer that splits on unquoted whitespace
/// cannot see `xmlns:ext = "urn:x"` — it yields `xmlns:ext`, `=`, `"urn:x"` as
/// three tokens, so neither a name nor a value survives, which is how a wrapper's
/// namespace declaration used to be dropped together with the wrapper while its
/// children moved into the composed one.
///
/// A malformed attribute list is a typed rejection rather than a guess: the
/// composer cannot reproduce a declaration it cannot read.
fn parse_attributes(body: &str) -> Result<Vec<(&str, &str)>, NativeFulltextCompositionError> {
    let bytes = body.as_bytes();
    let mut at = 0usize;
    let mut attributes = Vec::new();
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at == bytes.len() {
            break;
        }
        let name_start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'=' {
            at += 1;
        }
        let name = &body[name_start..at];
        if name.is_empty() {
            return Err(NativeFulltextCompositionError::MalformedXml);
        }
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            return Err(NativeFulltextCompositionError::MalformedXml);
        }
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let Some(&quote) = bytes.get(at) else {
            return Err(NativeFulltextCompositionError::MalformedXml);
        };
        if quote != b'"' && quote != b'\'' {
            return Err(NativeFulltextCompositionError::MalformedXml);
        }
        at += 1;
        let value_start = at;
        while at < bytes.len() && bytes[at] != quote {
            at += 1;
        }
        if at == bytes.len() {
            return Err(NativeFulltextCompositionError::MalformedXml);
        }
        let value = &body[value_start..at];
        at += 1;
        attributes.push((name, value));
    }
    Ok(attributes)
}

/// Structural view of a TEI document, used for splice ranges and for the
/// pre-acceptance validation of composed bytes.
///
/// The composer cannot work from a lexical tag list: a range that starts at a
/// closing tag silently truncates the source.  Elements are therefore paired by
/// the same open/close stack that validates balance, so every span is
/// `(start of the open tag, end of the close tag)` and can never begin with
/// `</`.  Namespace prefixes are not interpreted here — the composer preserves
/// declarations together with the subtrees they cover, and a splice only needs
/// byte-exact spans.  That is also why this scanner is not the arbiter of the
/// emission: `validate_well_formed` is a separate pass, and the offline
/// independent oracle (`roxmltree`) is the judge beyond it.
///
/// Two markup-aware readings sit here rather than in a byte scan of the whole
/// document: `<!DOCTYPE`/`<!ENTITY` are refused at the `<` that actually starts
/// the declaration (so a comment, CDATA section or attribute value that merely
/// mentions the string declares nothing), and `id`/`xml:id` are read through the
/// XML attribute grammar (so no legal spelling escapes the uniqueness gate).
#[derive(Debug)]
struct XmlDocument {
    elements: Vec<XmlElement>,
}

#[derive(Debug)]
struct XmlElement {
    name: String,
    /// Index of the `<` that opens the element.
    start: usize,
    /// Exclusive index just past the element's closing `>`.
    end: usize,
    /// Exclusive index just past the element's open tag.
    open_end: usize,
    /// Index of the `<` that opens the element's closing tag; equal to
    /// `open_end` for a self-closing element.
    ///
    /// Content bytes live in `open_end..close_start`, which is what lets the
    /// composer carry the wrapper's comments, processing instructions and text,
    /// not only its element subtrees.
    close_start: usize,
    /// Index of the enclosing element, `None` at the document root.
    parent: Option<usize>,
}

impl XmlDocument {
    fn parse(s: &str) -> Result<Self, NativeFulltextCompositionError> {
        let mut elements: Vec<XmlElement> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut seen_ids: Vec<String> = Vec::new();
        let mut pos = 0;
        while let Some(offset) = s[pos..].find('<') {
            let start = pos + offset;
            match s[start + 1..].chars().next() {
                // Markup declaration: a comment or a CDATA section.  Both may
                // legally contain `<` and `>`, so the terminator is searched
                // for instead of stopping at the first `>`.
                // `<!DOCTYPE`/`<!ENTITY` are refused here, where XML markup
                // actually starts, not by scanning the raw bytes of the whole
                // document: a document that merely *mentions* the string — in a
                // comment, in a CDATA section or inside an attribute value — is
                // not declaring anything, and the previous substring scan refused
                // the first two classes even though an independent parser accepts
                // them (measured S01/T06).  Attribute values stay covered by the
                // well-formedness pass, which rejects a raw `<` in an `AttValue`.
                Some('!') => {
                    let rest = &s[start + 1..];
                    if rest.starts_with("!DOCTYPE") || rest.starts_with("!ENTITY") {
                        return Err(NativeFulltextCompositionError::ForbiddenExternalEntity);
                    }
                    let terminator = if rest.starts_with("!--") {
                        "-->"
                    } else if rest.starts_with("![CDATA[") {
                        "]]>"
                    } else {
                        ">"
                    };
                    // `find` is relative to `start`, so the offset must be
                    // rebased before it becomes the next scan position.
                    let close = start
                        + s[start..]
                            .find(terminator)
                            .ok_or(NativeFulltextCompositionError::MalformedXml)?;
                    pos = close + terminator.len();
                    continue;
                }
                // Processing instruction, including the XML declaration.
                Some('?') => {
                    let close = start
                        + s[start..]
                            .find("?>")
                            .ok_or(NativeFulltextCompositionError::MalformedXml)?;
                    pos = close + 2;
                    continue;
                }
                _ => {}
            }
            let end = tag_end(s, start + 1).ok_or(NativeFulltextCompositionError::MalformedXml)?;
            let raw = &s[start + 1..end];
            let closing = raw.starts_with('/');
            let body = raw.trim_start_matches('/');
            let name = body
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("");
            if name.is_empty() || !name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()) {
                return Err(NativeFulltextCompositionError::MalformedXml);
            }
            let open_end = end + 1;
            if closing {
                let Some(index) = stack.pop() else {
                    return Err(NativeFulltextCompositionError::UnbalancedXml);
                };
                if elements[index].name != name {
                    return Err(NativeFulltextCompositionError::UnbalancedXml);
                }
                elements[index].end = open_end;
                elements[index].close_start = start;
            } else {
                // `id`/`xml:id` are read through the XML attribute grammar
                // (`Name S? Eq S? AttValue`), not by splitting the open tag on
                // unquoted whitespace.  `xml:id = 'author-0'` is the same
                // attribute as `xml:id="author-0"`, and the tokenizer only
                // recognised the compact double-quoted spelling, so a preserved
                // subtree written the other way slipped past the uniqueness gate
                // and an `Accepted` emission carried two elements with the same
                // `xml:id` (measured S01/T06).  A list that cannot be read is not
                // a list whose ids may be guessed at, so it is `MalformedXml`
                // here — the well-formedness pass would reject it as well, and
                // this scanner never vouches for what it cannot read.
                let tag = tag_attributes(body);
                let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
                for (attribute, value) in parse_attributes(&tag[name_end..])? {
                    if attribute == "id" || attribute == "xml:id" {
                        if seen_ids.iter().any(|seen| seen == value) {
                            return Err(NativeFulltextCompositionError::DuplicateId);
                        }
                        seen_ids.push(value.to_owned());
                    }
                }
                let empty = is_self_closing(body);
                if !empty && stack.len() >= NATIVE_FULLTEXT_MAX_XML_DEPTH {
                    return Err(NativeFulltextCompositionError::MalformedXml);
                }
                elements.push(XmlElement {
                    name: name.to_owned(),
                    start,
                    end: open_end,
                    open_end,
                    close_start: open_end,
                    parent: stack.last().copied(),
                });
                if !empty {
                    stack.push(elements.len() - 1);
                }
            }
            pos = open_end;
        }
        if !stack.is_empty() {
            return Err(NativeFulltextCompositionError::UnbalancedXml);
        }
        Ok(Self { elements })
    }

    /// Exactly one element with this name anywhere in the document.
    fn unique(&self, name: &str) -> Result<(usize, usize), NativeFulltextCompositionError> {
        self.element_index(name).map(|index| {
            let element = &self.elements[index];
            (element.start, element.end)
        })
    }

    /// The single element with this name, or `None` when the document has no
    /// such element.  An ambiguous name is still a typed failure: silently
    /// skipping a guard would be worse than falling back to Java.
    fn optional_unique(
        &self,
        name: &str,
    ) -> Result<Option<(usize, usize)>, NativeFulltextCompositionError> {
        match self.element_index_opt(name)? {
            Some(index) => {
                let element = &self.elements[index];
                Ok(Some((element.start, element.end)))
            }
            None => Ok(None),
        }
    }

    fn element_index(&self, name: &str) -> Result<usize, NativeFulltextCompositionError> {
        self.element_index_opt(name)?
            .ok_or_else(|| NativeFulltextCompositionError::AmbiguousSplice(name.into()))
    }

    fn element_index_opt(
        &self,
        name: &str,
    ) -> Result<Option<usize>, NativeFulltextCompositionError> {
        let mut found = None;
        for (index, element) in self.elements.iter().enumerate() {
            if element.name == name {
                if found.is_some() {
                    return Err(NativeFulltextCompositionError::AmbiguousSplice(name.into()));
                }
                found = Some(index);
            }
        }
        Ok(found)
    }

    /// The single *direct* child of `parent` named `name`, or `None` when that
    /// parent has no such child.
    ///
    /// Direct-child scoping is load-bearing, not a convenience.  The composer
    /// replaces the source `fileDesc`/`titleStmt` wrappers with synthesized ones,
    /// and only an element that sits at that level *is* that wrapper.  Matching by
    /// name anywhere in the document instead made a nested `titleStmt` — for
    /// example one under `fileDesc/seriesStmt` — be treated as the wrapper: its
    /// content was moved into the composed title statement *and* the enclosing
    /// `seriesStmt` travelled over as a `fileDesc` child, so the nested `<title>`
    /// appeared twice in an `Accepted` emission (measured S01/T05).
    ///
    /// Two direct children with that name cannot be placed without guessing and
    /// are a typed rejection; a missing `parent` yields `None`, because the
    /// composer synthesizes the skeleton and an absent wrapper removes nothing.
    fn direct_child_index_opt(
        &self,
        parent: &str,
        name: &str,
    ) -> Result<Option<usize>, NativeFulltextCompositionError> {
        let Some(parent_index) = self.element_index_opt(parent)? else {
            return Ok(None);
        };
        let mut found = None;
        for (index, element) in self.elements.iter().enumerate() {
            if element.parent == Some(parent_index) && element.name == name {
                if found.is_some() {
                    return Err(NativeFulltextCompositionError::AmbiguousSplice(format!(
                        "{parent}/{name}"
                    )));
                }
                found = Some(index);
            }
        }
        Ok(found)
    }

    /// Byte intervals covering the *content* of the element at `index` minus the
    /// excluded direct-child subtrees, in source order.
    ///
    /// This is the conservation primitive.  It carries the wrapper's comments,
    /// processing instructions and the significant text/whitespace between its
    /// children, not only its element subtrees: element-only copying silently
    /// dropped them, and the preserved TEI roots carry `xml:space="preserve"`
    /// with whitespace between `</fileDesc>` and `<encodingDesc>` (measured
    /// S01/T05).  Every interval is copied verbatim from the source, so the
    /// conservation stays byte-exact.
    ///
    /// A missing element (`None`) yields no intervals, so a wrapper the composer
    /// synthesizes can be absent without an error.  Excluded direct children are
    /// siblings and therefore non-overlapping, which is what keeps the intervals
    /// disjoint and unambiguous.
    fn content_segments(&self, index: Option<usize>, excluded: &[&str]) -> Vec<(usize, usize)> {
        let Some(index) = index else {
            return Vec::new();
        };
        let element = &self.elements[index];
        let mut cut_points: Vec<(usize, usize)> = self
            .children_of(index)
            .filter(|child| excluded.contains(&child.name.as_str()))
            .map(|child| (child.start, child.end))
            .collect();
        cut_points.sort_unstable();
        let mut segments = Vec::new();
        let mut cursor = element.open_end;
        for (start, end) in cut_points {
            if cursor < start {
                segments.push((cursor, start));
            }
            cursor = cursor.max(end);
        }
        if cursor < element.close_start {
            segments.push((cursor, element.close_start));
        }
        segments
    }

    fn children_of(&self, index: usize) -> impl Iterator<Item = &XmlElement> {
        self.elements
            .iter()
            .filter(move |element| element.parent == Some(index))
    }

    /// Verbatim attribute text of `name`'s open tag (`""` when it has none,
    /// no trailing `/`).
    ///
    /// Namespace declarations and `xml:lang` live here: dropping them leaves
    /// preserved subtrees with unbound prefixes or a lost language tag, so the
    /// source declaration is reproduced byte-for-byte around the synthesized
    /// element.
    fn open_tag_attributes<'a>(
        &self,
        source: &'a str,
        name: &str,
    ) -> Result<&'a str, NativeFulltextCompositionError> {
        let index = self.element_index(name)?;
        let element = &self.elements[index];
        // The parser requires the open tag to start with the element name, so
        // the name length is an exact char boundary after `<`.
        let tag = &source[element.start + 1..element.open_end - 1];
        Ok(tag_attributes(&tag[name.len()..]))
    }

    /// Namespace declarations (`xmlns`, `xmlns:*`) carried by the open tag of the
    /// element at `index`, each re-emitted as ` name="value"` (`""` when the
    /// element is absent or declares none).
    ///
    /// The composer synthesizes `fileDesc` and `titleStmt` instead of preserving
    /// the source wrappers, so their own attributes are not otherwise reproduced.
    /// Dropping a declaration there leaves the moved children holding an unbound
    /// prefix — a well-formedness failure `XmlDocument::parse` cannot see, because
    /// it deliberately does not interpret namespaces, and which only the separate
    /// `validate_well_formed` pass (and the independent oracle) can catch.
    ///
    /// Reading through the *located* element, not through a global name lookup, is
    /// what keeps a nested same-named element from donating a declaration that
    /// belongs to a subtree which travels over whole.
    ///
    /// The pair is read through `parse_attributes`, so `xmlns:ext = "urn:x"` and
    /// `xmlns:ext ="urn:x"` are carried like the compact spelling; the earlier
    /// whitespace tokenizer saw three tokens (`xmlns:ext`, `=`, `"urn:x"`) and
    /// silently dropped the declaration.  A declaration that cannot be read is a
    /// typed rejection, never a silent omission.  A missing element is not an
    /// error: the absence paths are owned by `content_segments`, and there is no
    /// declaration to reproduce for an element that is not there.
    fn namespace_declarations_at(
        &self,
        source: &str,
        index: Option<usize>,
    ) -> Result<String, NativeFulltextCompositionError> {
        let Some(index) = index else {
            return Ok(String::new());
        };
        let element = &self.elements[index];
        // The parser requires the open tag to start with the element name, so the
        // name length is an exact char boundary after `<`.
        let tag = &source[element.start + 1..element.open_end - 1];
        let body = tag_attributes(&tag[element.name.len()..]);
        let mut declarations = String::new();
        for (attribute, value) in parse_attributes(body)? {
            if attribute == "xmlns" || attribute.starts_with("xmlns:") {
                declarations.push(' ');
                declarations.push_str(attribute);
                declarations.push_str("=\"");
                declarations.push_str(value);
                declarations.push('"');
            }
        }
        Ok(declarations)
    }

    fn reference_range(&self) -> Result<(usize, usize), NativeFulltextCompositionError> {
        self.unique("listBibl")
    }
}

/// Exact element bytes from TEI, using the same unique-range parser as composition.
pub fn tei_element_bytes<'a>(
    xml: &'a [u8],
    name: &str,
) -> Result<&'a [u8], NativeFulltextCompositionError> {
    let text = std::str::from_utf8(xml).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    let ranges = XmlDocument::parse(text)?;
    let (start, end) = ranges.unique(name)?;
    Ok(&xml[start..end])
}
