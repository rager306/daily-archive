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
    /// The splice produced a document the module's own structural parser
    /// rejects.  Composed bytes are never accepted on trust: the emitter
    /// re-reads its own output before `Accepted`.
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
/// Re-read the emitted document before it may be accepted.
///
/// `Accepted` is a claim about the bytes the candidate carries, so the emitter
/// runs the same structural validation over its own output that it ran over its
/// input: tag shape, element balance, and `xml:id`/`id` uniqueness across the
/// whole document — which is also what catches a preserved source id colliding
/// with a synthesized `author-N`/`bibl-N`.  A failure is typed and falls back to
/// the byte-identical Java envelope instead of publishing a damaged candidate.
///
/// Scope note: this is well-formedness and identity hygiene, not TEI schema
/// validation.  The module imports typed leaves, hashing, serde and formatting
/// only; a schema validator would be a runtime dependency, so the stronger
/// schema claim belongs to the offline independent oracle instead.
fn validate_emitted(bytes: &[u8]) -> Result<(), NativeFulltextCompositionError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    XmlDocument::parse(text).map(|_| ()).map_err(|error| {
        NativeFulltextCompositionError::ComposedOutputNotWellFormed(format!("{error:?}"))
    })
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
    let file_desc_declarations = document.namespace_declarations(existing, "fileDesc")?;
    let title_stmt_declarations = document.namespace_declarations(existing, "titleStmt")?;
    // Placement is structural, never name-guessed.  The synthesized skeleton is
    // `teiHeader > fileDesc > titleStmt`, so a source subtree is re-inserted
    // under the wrapper it actually came from:
    //
    //   A = children of the source `titleStmt`   -> inside the composed titleStmt
    //   B = children of the source `fileDesc`    -> siblings of the composed
    //       minus `titleStmt`                       titleStmt, inside the composed
    //                                               fileDesc
    //   C = children of the source `teiHeader`   -> siblings of the composed
    //       minus `fileDesc` and `titleStmt`        fileDesc
    //
    // A, B and C are pairwise disjoint (each element has exactly one parent) and
    // together they cover every descendant of the source `teiHeader` except the
    // two wrappers the composer synthesizes, so no element subtree is duplicated
    // and none is dropped.  A missing `fileDesc`/`titleStmt` just leaves its set
    // empty; an *ambiguous* wrapper cannot be placed without guessing, so it is a
    // typed rejection rather than a silent re-parenting.
    let title_stmt = document.child_spans("titleStmt")?;
    let file_desc = document.child_spans_excluding("fileDesc", &["titleStmt"])?;
    let header = document.child_spans_excluding("teiHeader", &["fileDesc", "titleStmt"])?;
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

/// Attribute substrings of an open tag body, honouring quoted values.
///
/// A value may contain whitespace (`note="a b"`) or the other quote character
/// (`note="it's"`), so a token ends only at unquoted whitespace.  The body is the
/// text after the element name and before a trailing `/`, so a self-closing tag
/// yields no tokens.
fn attribute_tokens(body: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut quote: Option<char> = None;
    let mut start: Option<usize> = None;
    for (offset, c) in body.char_indices() {
        match quote {
            Some(open) => {
                if c == open {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                c if c.is_whitespace() => {
                    if let Some(from) = start.take() {
                        tokens.push(&body[from..offset]);
                    }
                }
                _ => {
                    if start.is_none() {
                        start = Some(offset);
                    }
                }
            },
        }
    }
    if let Some(from) = start {
        tokens.push(&body[from..]);
    }
    tokens
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
/// byte-exact spans.
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
    /// Index of the enclosing element, `None` at the document root.
    parent: Option<usize>,
}

impl XmlDocument {
    fn parse(s: &str) -> Result<Self, NativeFulltextCompositionError> {
        if s.contains("<!DOCTYPE") || s.contains("<!ENTITY") {
            return Err(NativeFulltextCompositionError::ForbiddenExternalEntity);
        }
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
                Some('!') => {
                    let rest = &s[start + 1..];
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
            } else {
                if let Some(attribute) = body.split_whitespace().find(|attribute| {
                    attribute.starts_with("id=\"") || attribute.starts_with("xml:id=\"")
                }) {
                    let id = attribute
                        .split("=\"")
                        .nth(1)
                        .and_then(|x| x.split('"').next())
                        .ok_or(NativeFulltextCompositionError::MalformedXml)?;
                    if seen_ids.iter().any(|seen| seen == id) {
                        return Err(NativeFulltextCompositionError::DuplicateId);
                    }
                    seen_ids.push(id.to_owned());
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

    /// Whole-subtree spans of the direct children of the single element named
    /// `parent`, in source order.
    ///
    /// A missing `parent` yields no children instead of an error: the composer
    /// synthesizes `fileDesc` and `titleStmt` itself, so their absence removes no
    /// source subtree.  An *ambiguous* `parent` is a typed failure, because the
    /// composer cannot place the source content without guessing.
    fn child_spans(
        &self,
        parent: &str,
    ) -> Result<Vec<(usize, usize)>, NativeFulltextCompositionError> {
        let Some(index) = self.element_index_opt(parent)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .children_of(index)
            .map(|element| (element.start, element.end))
            .collect())
    }

    /// Direct child spans of `parent` minus children whose name is excluded.
    /// Every span is a whole subtree, so preserved children can never overlap.
    fn child_spans_excluding(
        &self,
        parent: &str,
        excluded: &[&str],
    ) -> Result<Vec<(usize, usize)>, NativeFulltextCompositionError> {
        let Some(index) = self.element_index_opt(parent)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .children_of(index)
            .filter(|element| !excluded.contains(&element.name.as_str()))
            .map(|element| (element.start, element.end))
            .collect())
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
        let attributes = tag[name.len()..].trim_end();
        Ok(attributes
            .strip_suffix('/')
            .unwrap_or(attributes)
            .trim_end())
    }

    /// Namespace declarations (`xmlns`, `xmlns:*`) carried by `name`'s open tag,
    /// each rendered with a leading space (`""` when the element is absent or
    /// declares none).
    ///
    /// The composer synthesizes `fileDesc` and `titleStmt` instead of preserving
    /// the source wrappers, so their own attributes are not otherwise reproduced.
    /// Dropping a declaration there leaves the moved children holding an unbound
    /// prefix — a well-formedness failure this module's own parser cannot see,
    /// because it deliberately does not interpret namespaces.  A missing element
    /// is not an error: the absence paths are owned by `child_spans*`, and there
    /// is no declaration to reproduce for an element that is not there.
    fn namespace_declarations(
        &self,
        source: &str,
        name: &str,
    ) -> Result<String, NativeFulltextCompositionError> {
        let Some(index) = self.element_index_opt(name)? else {
            return Ok(String::new());
        };
        let element = &self.elements[index];
        // The parser requires the open tag to start with the element name, so the
        // name length is an exact char boundary after `<`.
        let tag = &source[element.start + 1..element.open_end - 1];
        let body = tag[name.len()..].trim_end();
        let body = body.strip_suffix('/').unwrap_or(body);
        let mut declarations = String::new();
        for attribute in attribute_tokens(body) {
            let declaration = attribute == "xmlns"
                || attribute.starts_with("xmlns=")
                || attribute.starts_with("xmlns:");
            if declaration && attribute.contains('=') {
                declarations.push(' ');
                declarations.push_str(attribute);
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
