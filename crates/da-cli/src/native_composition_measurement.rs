//! M515-v2mvmc / S01: the shared composition-measurement surface.
//!
//! The preservation metric (`preservation.occurrence_conservation`) is
//! `admitted / source_population` over the fixed measurement-class set, and both
//! terms are read from the frozen reference TEI, never from a composed emission.
//! The runtime probe (`external_holdout_release_probe.rs`) and the corpus
//! acceptance runner (`native_corpus_acceptance.rs`) must measure the same
//! aggregate, so the compact scanner, the source-side [`ProbeSource`], the
//! [`AdmittedBand`], and the class enumeration live here once.
//!
//! This is deliberately a library module, not an example: a Cargo example cannot
//! be imported, and the scanner was already duplicated between the probe and the
//! S05 corpus shadow.  It depends on `da_parse` (an unconditional `da-cli`
//! dependency) because the admitted band is produced by the public typed
//! parsers; it does not depend on `pdf-layout`, so it compiles in the default
//! feature set.
//!
//! The compact scanner is not an oracle: it is the same minimal,
//! non-validating tag scanner the probe carries, and its only job is to let the
//! runtime runner read the carriers the independent `roxmltree` oracle reads.

use da_parse::*;

/// Every failure message carries this prefix so an inspection never guesses.
const PROBE: &str = "[native-composition-measurement]";

// ───────────────────────── the class vocabulary ─────────────────────────────

// The class tokens are derived from the production enum, from the composer's own
// declared reference-link derivation and from its citation role mapping: none of
// these is a second literal copy.
pub const CLASS_AUTHOR_LINE: &str = "AdmittedAuthorLine";
pub const CLASS_HEADER_TITLE: &str = "SemanticFieldLoss::HeaderTitle";
pub const CLASS_HEADER_EMAIL: &str = "SemanticFieldLoss::HeaderEmail";
pub const CLASS_HEADER_IDS: &str = "SemanticFieldLoss::HeaderIds";
pub const CLASS_AUTHOR_AFFILIATION: &str = "SemanticFieldLoss::AuthorAffiliation";
pub const CLASS_REFERENCE_DOMAIN: &str = NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION;
pub const CLASS_CITATION_DOI: &str = "CitationFieldRole::doi";
pub const CLASS_CITATION_ARXIV: &str = "CitationFieldRole::arxiv_id";
pub const CLASS_CITATION_URL: &str = "CitationFieldRole::url";
pub const CLASS_CITATION_ARTICLE_TITLE: &str = "CitationFieldRole::article_title";

pub const CLASS_ORDER: [&str; 10] = [
    CLASS_AUTHOR_LINE,
    CLASS_HEADER_TITLE,
    CLASS_HEADER_EMAIL,
    CLASS_HEADER_IDS,
    CLASS_AUTHOR_AFFILIATION,
    CLASS_REFERENCE_DOMAIN,
    CLASS_CITATION_DOI,
    CLASS_CITATION_ARXIV,
    CLASS_CITATION_URL,
    CLASS_CITATION_ARTICLE_TITLE,
];

pub const PROBE_CITATION_ROLES: [(CitationFieldRole, &str); 4] = [
    (CitationFieldRole::Doi, CLASS_CITATION_DOI),
    (CitationFieldRole::ArxivId, CLASS_CITATION_ARXIV),
    (CitationFieldRole::Url, CLASS_CITATION_URL),
    (
        CitationFieldRole::ArticleTitle,
        CLASS_CITATION_ARTICLE_TITLE,
    ),
];

const CHECKSUM_IDNO_TYPES: [&str; 3] = ["MD5", "SHA1", "SHA256"];

// ────────────────────────── the typed refusals ──────────────────────────────

/// A typed measurement refusal: the frozen reference TEI could not be measured
/// (a malformed header, an unadmittable real field, or a source/admitted
/// population inconsistency).  A refusal is named, never silently dropped and
/// never a fabricated zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementError {
    reason: String,
}

impl MeasurementError {
    /// Build a refusal with a named reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// The named reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl std::fmt::Display for MeasurementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for MeasurementError {}

/// The preservation aggregate for one frozen reference document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompositionCounts {
    /// The sum of the per-class source populations.
    pub source_population: usize,
    /// The sum of the per-class admitted occurrences.
    pub admitted: usize,
}

impl CompositionCounts {
    /// `admitted / source_population`, or `None` when no class declares a
    /// carrier (`no_denominator`, never a vacuum `1.0`).
    pub fn occurrence_conservation(&self) -> Option<f64> {
        (self.source_population > 0).then(|| self.admitted as f64 / self.source_population as f64)
    }
}

// ─────────────────── the per-class admitted/source enumeration ──────────────

/// The shape and admitted/source values of one measurement class, before any
/// composed-emission comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassAdmission {
    /// Text-valued carriers (titles, emails, ids, reference text, citation roles).
    Text {
        /// The class token (`CLASS_*`).
        class: &'static str,
        /// The source document's carrier population for the class.
        source_population: usize,
        /// The admitted values.
        admitted: Vec<String>,
        /// The source document's own carriers.
        source_carriers: Vec<String>,
    },
    /// Binding-valued carriers (author line, affiliation key).
    Binding {
        /// The class token (`CLASS_*`).
        class: &'static str,
        /// The source document's carrier population for the class.
        source_population: usize,
        /// The admitted `(owner, value)` bindings.
        admitted: Vec<(String, String)>,
        /// The source document's own `(owner, value)` bindings.
        source_carriers: Vec<(String, String)>,
    },
}

impl ClassAdmission {
    /// The class token.
    pub fn class(&self) -> &'static str {
        match self {
            ClassAdmission::Text { class, .. } | ClassAdmission::Binding { class, .. } => class,
        }
    }

    /// The source carrier population for the class.
    pub fn source_population(&self) -> usize {
        match self {
            ClassAdmission::Text {
                source_population, ..
            }
            | ClassAdmission::Binding {
                source_population, ..
            } => *source_population,
        }
    }

    /// The number of admitted occurrences.
    pub fn admitted_len(&self) -> usize {
        match self {
            ClassAdmission::Text { admitted, .. } => admitted.len(),
            ClassAdmission::Binding { admitted, .. } => admitted.len(),
        }
    }
}

/// The per-class admitted/source enumeration for one frozen reference document.
///
/// This is the single source of truth the probe's `oracle_class_profiles` and
/// the acceptance runner both consume: `admitted_len()` summed over the rows is
/// the preservation numerator and `source_population()` is its denominator.
pub fn class_admissions(
    source: &ProbeSource,
    admitted: &AdmittedBand,
    source_xml: &str,
) -> Vec<ClassAdmission> {
    let mut rows = Vec::with_capacity(CLASS_ORDER.len());
    rows.push(ClassAdmission::Binding {
        class: CLASS_AUTHOR_LINE,
        source_population: source.author_name_texts.len(),
        admitted: admitted.author_lines.clone(),
        source_carriers: source_author_lines(source_xml),
    });
    rows.push(ClassAdmission::Text {
        class: CLASS_HEADER_TITLE,
        source_population: source.header_titles.len(),
        admitted: admitted.titles.clone(),
        source_carriers: source.header_titles.clone(),
    });
    rows.push(ClassAdmission::Text {
        class: CLASS_HEADER_EMAIL,
        source_population: source.header_emails.len(),
        admitted: admitted.emails.clone(),
        source_carriers: source.header_emails.clone(),
    });
    rows.push(ClassAdmission::Text {
        class: CLASS_HEADER_IDS,
        source_population: source.header_pubnum_ids.len(),
        admitted: admitted.ids.clone(),
        source_carriers: source.header_idnos.clone(),
    });
    rows.push(ClassAdmission::Binding {
        class: CLASS_AUTHOR_AFFILIATION,
        source_population: source.affiliation_occurrences.len(),
        admitted: admitted.affiliation_bindings.clone(),
        source_carriers: source_affiliation_bindings(source_xml),
    });
    rows.push(ClassAdmission::Text {
        class: CLASS_REFERENCE_DOMAIN,
        source_population: source.reference_texts.len(),
        admitted: admitted.reference_texts.clone(),
        source_carriers: source.reference_texts.clone(),
    });

    let source_citations: Vec<(String, String)> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();
    let admitted_citations: Vec<(String, String)> = admitted
        .citation_fields
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();
    for (role, class) in PROBE_CITATION_ROLES {
        let token = citation_field_role_token(role);
        let admitted_values: Vec<String> = admitted_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        let source_values: Vec<String> = source_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        rows.push(ClassAdmission::Text {
            class,
            source_population: source_values.len(),
            admitted: admitted_values,
            source_carriers: source_values,
        });
    }
    assert_eq!(
        rows.len(),
        CLASS_ORDER.len(),
        "every declared class is enumerated once"
    );
    rows
}

/// The wire token the composed emission keys its citation carriers by for a
/// citation class token, or `None` when `class` is not a citation class.
///
/// The reported class label is `CitationFieldRole::doi` while the emission
/// carrier role is the serde wire token `doi`; this is the one place the two
/// vocabularies are joined.
pub fn citation_class_wire_token(class: &str) -> Option<&'static str> {
    PROBE_CITATION_ROLES
        .iter()
        .find(|(_, candidate)| *candidate == class)
        .map(|(role, _)| citation_field_role_token(*role))
}

/// The feature-free preservation entry: `{ source_population, admitted }` for one
/// frozen reference TEI, with no composed emission and no `pdf-layout`.
///
/// `admitted` is the sum of `admitted_values.len()` over the fixed class set and
/// `source_population` is the sum of the per-class source populations, matching
/// `document_metrics` in `external_holdout_release_probe.rs`.  The aggregate is
/// emission-independent, so the preservation threshold is measurable from the
/// frozen reference alone.
pub fn measure_document_admitted_coverage(
    tei_xml: &str,
) -> Result<CompositionCounts, MeasurementError> {
    let source = measure_source("", "", tei_xml)?;
    let admitted = build_admitted_band(&source)?;
    let rows = class_admissions(&source, &admitted, tei_xml);
    let mut counts = CompositionCounts::default();
    for row in &rows {
        counts.source_population += row.source_population();
        counts.admitted += row.admitted_len();
    }
    Ok(counts)
}

/// The unique element named `name`, or a typed refusal when it is absent or
/// duplicated (the panicking [`XmlScan::unique`] stays for the emission scan).
///
/// M515/S02 widened this from a module-private helper to a public seam so the
/// class-parity projection can refuse an emission whose class-carrying anchors
/// are missing (`ClassParityClassMissing`) instead of panicking on them; the
/// scanner itself is unchanged.
pub fn find_unique(scan: &XmlScan, name: &str) -> Result<usize, MeasurementError> {
    let mut found = scan
        .elems
        .iter()
        .enumerate()
        .filter(|(_, element)| element.name == name);
    let Some((index, _)) = found.next() else {
        return Err(MeasurementError::new(format!(
            "the document declares no <{name}> element"
        )));
    };
    if found.next().is_some() {
        return Err(MeasurementError::new(format!(
            "the scanner requires a unique <{name}> element"
        )));
    }
    Ok(index)
}

// ───────────────────────── the compact XML tag scanner ──────────────────────
//
// `da-cli` has no XML parser and adding one is out of scope, so this file carries
// a minimal, non-validating scanner: it builds the element list with attributes
// and content ranges, and reconstructs an element's normalized text with XML
// entity decoding.  It is deliberately nothing more than that — it is not a
// substitute for the `roxmltree` oracle in `native_fulltext_corpus_semantics.rs`
// (the authoritative structural oracle), it does not validate well-formedness,
// and it is never used on untrusted input.  Its only job is to let the runtime
// runner read the same carriers the oracle reads.

struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
    content_start: usize,
    end: usize,
    parent: Option<usize>,
}

pub struct XmlScan {
    elems: Vec<Elem>,
    source: String,
}

/// Byte offset of the `>` that closes the tag opened at `start`, skipping `>`
/// characters inside quoted attribute values.
fn find_tag_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let byte = bytes[i];
        match quote {
            Some(q) => {
                if byte == q {
                    quote = None;
                }
            }
            None => {
                if byte == b'"' || byte == b'\'' {
                    quote = Some(byte);
                } else if byte == b'>' {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

fn find_after(source: &str, start: usize, marker: &str) -> usize {
    source[start..]
        .find(marker)
        .map(|offset| start + offset + marker.len())
        .unwrap_or(source.len())
}

fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.'
}

fn tag_name(inner: &str) -> &str {
    let start = inner
        .find(|c: char| !c.is_whitespace() && c != '/')
        .unwrap_or(inner.len());
    let rest = &inner[start..];
    let end = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
    &rest[..end]
}

/// Decode the five predefined XML entities plus numeric character references.
/// The composer re-escapes every emitted value (`esc`), so both sides of the
/// measurement must decode to compare; this is the same projection `roxmltree`
/// applies.
fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let rest = &input[index + 1..];
        let Some(semi) = rest.find(';') else {
            out.push(ch);
            continue;
        };
        if semi > 12 {
            out.push(ch);
            continue;
        }
        let entity = &rest[..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(decoded) => {
                out.push(decoded);
                for _ in 0..=semi {
                    chars.next();
                }
            }
            None => out.push(ch),
        }
    }
    out
}

fn parse_attrs(inner: &str) -> Vec<(String, String)> {
    let bytes = inner.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = inner[name_start..i].to_string();
        if name.is_empty() {
            i += 1;
            continue;
        }
        i = skip_whitespace(bytes, i);
        if i >= bytes.len() || bytes[i] != b'=' {
            attrs.push((name, String::new()));
            continue;
        }
        i = skip_whitespace(bytes, i + 1);
        if i >= bytes.len() {
            attrs.push((name, String::new()));
            break;
        }
        let quote = bytes[i];
        if quote == b'"' || quote == b'\'' {
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            attrs.push((name, decode_entities(&inner[value_start..i])));
            if i < bytes.len() {
                i += 1;
            }
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            attrs.push((name, decode_entities(&inner[value_start..i])));
        }
    }
    attrs
}

pub fn scan_xml(source: &str) -> XmlScan {
    let bytes = source.as_bytes();
    let mut elems: Vec<Elem> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &source[i..];
        if rest.starts_with("<!--") {
            i = find_after(source, i, "-->");
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = find_after(source, i, "]]>");
            continue;
        }
        if rest.starts_with("<?") {
            i = find_after(source, i, "?>");
            continue;
        }
        if rest.starts_with("<!") {
            i = find_after(source, i, ">");
            continue;
        }
        let Some(gt) = find_tag_end(source, i) else {
            break;
        };
        if rest.starts_with("</") {
            let name = tag_name(&source[i + 2..gt]).to_string();
            while let Some(top) = stack.pop() {
                elems[top].end = i;
                if elems[top].name == name {
                    break;
                }
            }
            i = gt + 1;
            continue;
        }
        let inner = &source[i + 1..gt];
        let self_closing = inner.trim_end().ends_with('/');
        let name = tag_name(inner).to_string();
        let attrs = parse_attrs(inner);
        let parent = stack.last().copied();
        let index = elems.len();
        elems.push(Elem {
            name,
            attrs,
            content_start: gt + 1,
            end: gt + 1,
            parent,
        });
        if self_closing {
            elems[index].end = gt + 1;
        } else {
            stack.push(index);
        }
        i = gt + 1;
    }
    for top in stack {
        elems[top].end = source.len();
    }
    XmlScan {
        elems,
        source: source.to_string(),
    }
}

impl XmlScan {
    /// The unique element named `name` anywhere in the document.
    pub fn unique(&self, name: &str) -> usize {
        let mut found = self
            .elems
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name == name);
        let (index, _) = found
            .next()
            .unwrap_or_else(|| panic!("the document declares no <{name}> element"));
        assert!(
            found.next().is_none(),
            "the scanner requires a unique <{name}> element"
        );
        index
    }

    fn is_descendant(&self, node: usize, root: usize) -> bool {
        let mut current = self.elems[node].parent;
        while let Some(parent) = current {
            if parent == root {
                return true;
            }
            current = self.elems[parent].parent;
        }
        false
    }

    pub fn descendants(&self, root: usize, name: &str) -> Vec<usize> {
        (0..self.elems.len())
            .filter(|&index| self.elems[index].name == name && self.is_descendant(index, root))
            .collect()
    }

    pub fn children(&self, root: usize, name: &str) -> Vec<usize> {
        (0..self.elems.len())
            .filter(|&index| {
                self.elems[index].name == name && self.elems[index].parent == Some(root)
            })
            .collect()
    }

    pub fn attr(&self, index: usize, name: &str) -> Option<&str> {
        self.elems[index]
            .attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Normalized, entity-decoded text of an element: tags become a space (the
    /// join `roxmltree`'s text-descendant join performs) and whitespace runs
    /// collapse.
    pub fn text(&self, index: usize) -> String {
        let raw = &self.source[self.elems[index].content_start..self.elems[index].end];
        let mut stripped = String::with_capacity(raw.len());
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                if let Some(gt) = find_tag_end(raw, i) {
                    stripped.push(' ');
                    i = gt + 1;
                    continue;
                }
                i += 1;
                continue;
            }
            let ch = raw[i..].chars().next().expect("in-bounds char");
            stripped.push(ch);
            i += ch.len_utf8();
        }
        decode_entities(&stripped)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn text_list(&self, indices: &[usize]) -> Vec<String> {
        indices.iter().map(|&index| self.text(index)).collect()
    }
}

// ───────────────────── the independent source-side measurement ──────────────

/// One real `author`/`affiliation key` binding, in document order.
pub struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the compact scanner reads from one frozen reference document:
/// the *source population* side of the profile.  Nothing here is produced by the
/// composer.
pub struct ProbeSource {
    pub article_id: String,
    pub sha256: String,
    pub header_titles: Vec<String>,
    pub title_stmt_titles: Vec<String>,
    pub header_emails: Vec<String>,
    pub header_idnos: Vec<String>,
    pub header_pubnum_ids: Vec<String>,
    pub author_name_texts: Vec<String>,
    pub affiliation_occurrences: Vec<AffiliationOccurrence>,
    pub date_text: String,
    pub reference_texts: Vec<String>,
    pub citation_carriers: Vec<(CitationFieldRole, String)>,
}

fn is_checksum_idno(scan: &XmlScan, index: usize) -> bool {
    scan.attr(index, "type")
        .is_some_and(|kind| CHECKSUM_IDNO_TYPES.contains(&kind))
}

pub fn measure_source(
    article_id: &str,
    declared_sha256: &str,
    xml: &str,
) -> Result<ProbeSource, MeasurementError> {
    let scan = scan_xml(xml);
    let header = find_unique(&scan, "teiHeader")?;
    let list = find_unique(&scan, "listBibl")?;

    let title_stmt = scan
        .children(header, "fileDesc")
        .first()
        .and_then(|&desc| scan.children(desc, "titleStmt").first().copied());
    let title_stmt_titles: Vec<String> = title_stmt
        .map(|stmt| scan.text_list(&scan.descendants(stmt, "title")))
        .unwrap_or_default();
    let header_titles = scan.text_list(&scan.descendants(header, "title"));
    let header_emails = scan.text_list(&scan.descendants(header, "email"));
    let header_idnos = scan.text_list(&scan.descendants(header, "idno"));
    let header_pubnum_ids: Vec<String> = scan
        .descendants(header, "idno")
        .into_iter()
        .filter(|&index| !is_checksum_idno(&scan, index))
        .map(|index| scan.text(index))
        .collect();

    let author_elements = scan.descendants(header, "author");
    let author_name_texts: Vec<String> = author_elements
        .iter()
        .map(|&author| {
            scan.children(author, "persName")
                .first()
                .map(|&pers_name| scan.text(pers_name))
                .unwrap_or_default()
        })
        .collect();

    let mut affiliation_occurrences = Vec::new();
    for (author_index, &author) in author_elements.iter().enumerate() {
        for affiliation in scan.children(author, "affiliation") {
            let Some(key) = scan.attr(affiliation, "key") else {
                continue;
            };
            affiliation_occurrences.push(AffiliationOccurrence {
                author_index,
                key: key.to_string(),
                text: scan.text(affiliation),
            });
        }
    }

    let date_text = scan
        .descendants(header, "date")
        .into_iter()
        .map(|index| scan.text(index))
        .find(|text| !text.is_empty())
        .ok_or_else(|| {
            MeasurementError::new(format!(
                "FATAL {PROBE} {article_id}: the header declares no dated element with text"
            ))
        })?;

    let reference_texts: Vec<String> = scan.text_list(&scan.children(list, "biblStruct"));

    let mut citation_carriers = Vec::new();
    for index in scan.descendants(list, "idno") {
        match scan.attr(index, "type") {
            Some("DOI") => citation_carriers.push((CitationFieldRole::Doi, scan.text(index))),
            Some("arXiv") => citation_carriers.push((CitationFieldRole::ArxivId, scan.text(index))),
            _ => {}
        }
    }
    for index in scan.descendants(list, "ptr") {
        if let Some(target) = scan.attr(index, "target")
            && target.starts_with("http")
        {
            citation_carriers.push((CitationFieldRole::Url, target.to_string()));
        }
    }
    for index in scan.descendants(list, "title") {
        if scan.attr(index, "level") == Some("a") {
            citation_carriers.push((CitationFieldRole::ArticleTitle, scan.text(index)));
        }
    }

    Ok(ProbeSource {
        article_id: article_id.to_string(),
        sha256: declared_sha256.to_string(),
        header_titles,
        title_stmt_titles,
        header_emails,
        header_idnos,
        header_pubnum_ids,
        author_name_texts,
        affiliation_occurrences,
        date_text,
        reference_texts,
        citation_carriers,
    })
}

// ────────────────────────── the admitted band (input) ───────────────────────

/// One named, typed admission narrowing: a class whose real carrier population
/// could not be admitted in full, with the reason.  `admitted < source_population`
/// is only allowed when a row of this table explains it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionGap {
    pub class: &'static str,
    pub source_population: usize,
    pub admitted: usize,
    pub reason: String,
}

/// The typed composition input plus the admitted-side values the measurement
/// compares the emission against.  All of it is derived from the real document.
pub struct AdmittedBand {
    pub header: HeaderCompositionOutcome,
    pub references: ReferenceCompositionOutcome,
    pub titles: Vec<String>,
    pub emails: Vec<String>,
    pub ids: Vec<String>,
    pub author_lines: Vec<(String, String)>,
    pub affiliation_bindings: Vec<(String, String)>,
    pub reference_texts: Vec<String>,
    pub citation_fields: Vec<(CitationFieldRole, String)>,
    pub gaps: Vec<AdmissionGap>,
}

/// The serde wire tag and reason of a rejected parser outcome, e.g.
/// `ambiguous:single_token`.
fn outcome_token<T: serde::Serialize>(outcome: &T) -> String {
    let value = serde_json::to_value(outcome).expect("a parser outcome serializes");
    let tag = value
        .get("outcome")
        .and_then(|tag| tag.as_str())
        .unwrap_or("unknown")
        .to_string();
    match value.get("reason").and_then(|reason| reason.as_str()) {
        Some(reason) => format!("{tag}:{reason}"),
        None => tag,
    }
}

/// Assemble the admitted band from the real frozen document through the public
/// typed parsers.  Every parser rejection becomes a named [`AdmissionGap`];
/// nothing is silently dropped and no value is invented.
pub fn build_admitted_band(source: &ProbeSource) -> Result<AdmittedBand, MeasurementError> {
    let mut gaps = Vec::new();

    let date = match NativeDateParser::new().parse(&source.date_text) {
        NativeDateOutcome::Recognized { date } => date,
        other => {
            return Err(MeasurementError::new(format!(
                "FATAL {PROBE} {}: the real publication date form cannot be admitted by \
                 NativeDateParser ({}); the header band cannot be composed from the frozen document",
                source.article_id,
                outcome_token(&other)
            )));
        }
    };

    let titles = source.title_stmt_titles.clone();
    if titles.len() < source.header_titles.len() {
        gaps.push(AdmissionGap {
            class: CLASS_HEADER_TITLE,
            source_population: source.header_titles.len(),
            admitted: titles.len(),
            reason: format!(
                "band definition: the admitted title field is the direct fileDesc/titleStmt/title \
                 children; the remaining {} of {} header title carriers are conserved by the \
                 source subtree and are not produced by the admitted band",
                source.header_titles.len() - titles.len(),
                source.header_titles.len()
            ),
        });
    }

    let emails = source.header_emails.clone();
    let ids = source.header_pubnum_ids.clone();

    let affiliation_parser = NativeAffiliationParser::new();
    let mut occurrence_component: Vec<Option<usize>> = Vec::new();
    let mut rejected_affiliations = Vec::new();
    let mut recognized_components = 0usize;
    for occurrence in &source.affiliation_occurrences {
        match affiliation_parser.parse(&occurrence.text) {
            NativeAffiliationOutcome::Recognized { components } => {
                if components.is_empty() {
                    return Err(MeasurementError::new(format!(
                        "{}: a recognized affiliation must carry at least one component",
                        source.article_id
                    )));
                }
                occurrence_component.push(Some(recognized_components));
                recognized_components += 1;
            }
            other => {
                occurrence_component.push(None);
                rejected_affiliations.push(outcome_token(&other));
            }
        }
    }

    let name_parser = NativePersonalNameParser::new();
    let mut authors: Vec<HeaderAuthor> = Vec::new();
    let mut author_lines: Vec<(String, String)> = Vec::new();
    let mut affiliation_bindings: Vec<(String, String)> = Vec::new();
    let mut rejected_names = Vec::new();

    for (author_index, name_text) in source.author_name_texts.iter().enumerate() {
        let name = match name_parser.parse(PersonalNameContext::Header, name_text) {
            NativePersonalNameOutcome::Recognized { name } => name,
            other => {
                rejected_names.push(outcome_token(&other));
                continue;
            }
        };
        let ordinal = authors.len();
        let owner = format!("author-{ordinal}");
        let mut links = Vec::new();
        for (occurrence_index, occurrence) in source.affiliation_occurrences.iter().enumerate() {
            if occurrence.author_index != author_index {
                continue;
            }
            // A rejected affiliation text is not admitted, and neither is an
            // affiliation carrying no key: the link is simply not in the band.
            if occurrence_component[occurrence_index].is_none() {
                continue;
            }
            let parsed = match affiliation_parser.parse(&occurrence.text) {
                NativeAffiliationOutcome::Recognized { components } => components,
                other => {
                    return Err(MeasurementError::new(format!(
                        "{PROBE} {}: an affiliation admitted once must be admitted again ({})",
                        source.article_id,
                        outcome_token(&other)
                    )));
                }
            };
            let span = parsed
                .first()
                .map(|component| component.span)
                .ok_or_else(|| {
                    MeasurementError::new(format!(
                        "{PROBE} {}: a recognized affiliation carries no component",
                        source.article_id
                    ))
                })?;
            links.push(NativeAffiliationLink {
                key: occurrence.key.clone(),
                affiliation_ordinal: occurrence_index,
                affiliation_span: span,
            });
        }
        author_lines.push((owner.clone(), name.surname.clone()));
        affiliation_bindings.extend(links.iter().map(|link| (owner.clone(), link.key.clone())));
        authors.push(HeaderAuthor {
            ordinal,
            span: CompositionSpan {
                start: ordinal,
                end: ordinal + 1,
            },
            name,
            affiliations: links,
        });
    }

    if authors.is_empty() {
        return Err(MeasurementError::new(format!(
            "FATAL {PROBE} {}: the frozen reference admitted no author line",
            source.article_id
        )));
    }
    for reason in &rejected_names {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_LINE,
            source_population: source.author_name_texts.len(),
            admitted: authors.len(),
            reason: format!("NativePersonalNameParser rejected the real persName text ({reason})"),
        });
    }
    for reason in &rejected_affiliations {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_AFFILIATION,
            source_population: source.affiliation_occurrences.len(),
            admitted: affiliation_bindings.len(),
            reason: format!(
                "NativeAffiliationParser rejected the real affiliation key carrier text ({reason})"
            ),
        });
    }

    let references: Vec<NativeReference> = source
        .reference_texts
        .iter()
        .enumerate()
        .map(|(ordinal, text)| NativeReference {
            ordinal,
            source: text.clone(),
            span: CitationSpan {
                start: 0,
                end: text.len(),
            },
            provenance: "frozen-external-holdout-reference".into(),
        })
        .collect();
    if !references
        .iter()
        .all(|reference| reference.span.start < reference.span.end)
    {
        return Err(MeasurementError::new(format!(
            "FATAL {PROBE} {}: a source biblStruct carries empty text, so no honest reference span exists",
            source.article_id
        )));
    }

    let citation_fields: Vec<CitationField> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| CitationField {
            role: *role,
            value: value.clone(),
            span: CitationSpan {
                start: 0,
                end: value.len(),
            },
        })
        .collect();
    if !citation_fields
        .iter()
        .all(|field| field.span.start < field.span.end)
    {
        return Err(MeasurementError::new(format!(
            "FATAL {PROBE} {}: a citation carrier carries an empty value",
            source.article_id
        )));
    }

    let header = HeaderCompositionOutcome::Accepted {
        composition: HeaderCompositionCandidate {
            candidate: HeaderCandidate {
                date,
                authors,
                fields: NativeHeaderFields {
                    title: titles.clone(),
                    email: emails.clone(),
                    ids: ids.clone(),
                },
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::Date,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Names,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Affiliations,
                    disposition: ComponentDisposition::Accepted,
                },
            ],
        },
    };
    let references_outcome = ReferenceCompositionOutcome::Accepted {
        composition: ReferenceCompositionCandidate {
            candidate: ReferenceCandidate {
                references,
                fields: citation_fields.clone(),
                anchors: Vec::new(),
                adjacent_pairs: Vec::new(),
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::References,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::CitationFields,
                    disposition: if citation_fields.is_empty() {
                        ComponentDisposition::Empty
                    } else {
                        ComponentDisposition::Accepted
                    },
                },
                ComponentReceipt {
                    component: CompositionComponent::Anchors,
                    disposition: ComponentDisposition::Empty,
                },
            ],
        },
    };

    if affiliation_bindings.len() > source.affiliation_occurrences.len() {
        return Err(MeasurementError::new(format!(
            "{}: the admitted band cannot carry more bindings than the source declares",
            source.article_id
        )));
    }

    Ok(AdmittedBand {
        header,
        references: references_outcome,
        titles,
        emails,
        ids,
        author_lines,
        affiliation_bindings,
        reference_texts: source.reference_texts.clone(),
        citation_fields: source.citation_carriers.clone(),
        gaps,
    })
}

// ─────────────── source-side carrier helpers (kept public) ─────────────────
pub fn source_author_lines(xml: &str) -> Vec<(String, String)> {
    let scan = scan_xml(xml);
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .map(|author| {
            let surname = scan
                .children(author, "surname")
                .first()
                .map(|&index| scan.text(index))
                .unwrap_or_default();
            (String::new(), surname)
        })
        .collect()
}

pub fn source_affiliation_bindings(xml: &str) -> Vec<(String, String)> {
    let scan = scan_xml(xml);
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .flat_map(|author| {
            scan.descendants(author, "affiliation")
                .into_iter()
                .filter_map(|index| {
                    scan.attr(index, "key")
                        .map(|key| (String::new(), key.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A git-tracked frozen reference TEI (the probe's `reference_tei[0]`).
    const REFERENCE_TEI: &str =
        include_str!("../../da-parse/tests/fixtures/holdout/1203.2295.tei.xml");

    #[test]
    fn measures_a_non_empty_preservation_denominator() {
        let counts = measure_document_admitted_coverage(REFERENCE_TEI)
            .expect("the frozen reference must be measurable");
        assert!(
            counts.source_population > 0,
            "the denominator must be non-empty"
        );
        assert!(
            counts.admitted <= counts.source_population,
            "admitted cannot exceed the source population"
        );
        assert!(counts.occurrence_conservation().is_some());
    }

    #[test]
    fn refuses_a_document_without_a_tei_header() {
        let error = measure_document_admitted_coverage("<TEI></TEI>")
            .expect_err("a document without a teiHeader must be refused");
        assert!(
            error.reason().contains("teiHeader"),
            "the refusal must name the missing element, got: {error}"
        );
    }

    #[test]
    fn refuses_an_empty_document() {
        assert!(measure_document_admitted_coverage("").is_err());
    }
}
