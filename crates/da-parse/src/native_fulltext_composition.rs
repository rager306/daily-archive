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
    InvalidUtf8,
    ChangedReadingOrder,
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
        let fragment = header_xml(composition)?;
        let range = ranges.unique("teiHeader")?;
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
    if let Ok(body) = ranges.unique("body") {
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
        bytes.splice(range.0..range.1, fragment.as_bytes().iter().copied());
        components.push(manifest_entry(component, &receipt, fragment.as_bytes()));
    }
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
        && valid_sha(&r.artifact_sha256)
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
        evidence_digest: r.artifact_sha256.clone(),
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
        evidence_digest: r.artifact_sha256.clone(),
        byte_count: bytes.len(),
        sha256: digest(bytes),
        truncated: false,
        fallback_target: r.fallback_target.clone(),
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
) -> Result<String, NativeFulltextCompositionError> {
    let mut out = String::from("<teiHeader><fileDesc><titleStmt>");
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
    out.push_str("</titleStmt><date when=\"");
    out.push_str(&c.candidate.date.year.to_string());
    if let Some(month) = c.candidate.date.month {
        out.push_str(&format!("-{month:02}"));
    }
    if let Some(day) = c.candidate.date.day {
        out.push_str(&format!("-{day:02}"));
    }
    out.push_str("\"/></fileDesc></teiHeader>");
    Ok(out)
}
fn references_xml(
    c: &crate::ReferenceCompositionCandidate,
) -> Result<String, NativeFulltextCompositionError> {
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

#[derive(Debug)]
struct XmlDocument {
    tags: Vec<(String, usize, usize)>,
}
impl XmlDocument {
    fn parse(s: &str) -> Result<Self, NativeFulltextCompositionError> {
        if s.contains("<!DOCTYPE") || s.contains("<!ENTITY") {
            return Err(NativeFulltextCompositionError::ForbiddenExternalEntity);
        }
        let mut stack: Vec<String> = Vec::new();
        let mut tags = Vec::new();
        let mut pos = 0;
        while let Some(start) = s[pos..].find('<') {
            let start = pos + start;
            let end = s[start..]
                .find('>')
                .ok_or(NativeFulltextCompositionError::MalformedXml)?
                + start;
            let raw = &s[start + 1..end];
            if raw.starts_with('!') || raw.starts_with('?') {
                pos = end + 1;
                continue;
            }
            let closing = raw.starts_with('/');
            let body = raw.trim_start_matches('/').trim();
            let self_close = body.ends_with('/');
            let name = body
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("");
            if name.is_empty() || !name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()) {
                return Err(NativeFulltextCompositionError::MalformedXml);
            }
            if let Some(attribute) = raw.split_whitespace().find(|attribute| {
                attribute.starts_with("id=\"") || attribute.starts_with("xml:id=\"")
            }) {
                let id = attribute
                    .split("=\"")
                    .nth(1)
                    .and_then(|x| x.split('"').next())
                    .ok_or(NativeFulltextCompositionError::MalformedXml)?;
                let id_key = format!("#id:{id}");
                if tags.iter().any(|(n, _, _)| n == &id_key) {
                    return Err(NativeFulltextCompositionError::DuplicateId);
                }
                tags.push((id_key, start, end + 1));
            }
            if closing {
                if stack.pop().as_deref() != Some(name) {
                    return Err(NativeFulltextCompositionError::UnbalancedXml);
                }
            } else if !self_close {
                if stack.len() >= NATIVE_FULLTEXT_MAX_XML_DEPTH {
                    return Err(NativeFulltextCompositionError::MalformedXml);
                }
                stack.push(name.to_owned());
            }
            tags.push((name.to_owned(), start, end + 1));
            pos = end + 1;
        }
        if !stack.is_empty() {
            return Err(NativeFulltextCompositionError::UnbalancedXml);
        }
        Ok(Self { tags })
    }
    fn unique(&self, name: &str) -> Result<(usize, usize), NativeFulltextCompositionError> {
        let matches: Vec<(usize, usize)> = self
            .tags
            .iter()
            .filter(|tag| tag.0 == name)
            .map(|tag| (tag.1, tag.2))
            .collect();
        if matches.is_empty() || matches.len() > 2 {
            return Err(NativeFulltextCompositionError::AmbiguousSplice(name.into()));
        }
        let opening = matches[0];
        Ok(self.element_range(opening, name))
    }
    fn element_range(&self, opening: (usize, usize), name: &str) -> (usize, usize) {
        let mut depth = 0;
        let mut end = opening.1;
        for (n, a, b) in &self.tags {
            if *a < opening.0 {
                continue;
            }
            if n == name {
                depth += 1;
                if depth == 2 {
                    end = *b;
                    break;
                }
            }
        }
        (opening.0, end)
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
