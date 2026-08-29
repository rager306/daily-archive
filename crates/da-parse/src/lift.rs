//! TEI lift: title/body, sections, citations, tables, formulas, plus the
//! S07 P0 families lifted in document order (declared arXiv identity and
//! header authors since S07/T02; figures/callouts follow in T03).
//!
//! Since M464-z84in2 S02/T02 the single lift algorithm is owned by the
//! provider-neutral [`TeiLifter`]: a [`TeiEnvelope`] (verbatim raw TEI bytes
//! plus exact backend/profile identity) plus logical `paper_id`/`pdf_hash`
//! in, this-cut [`TeiParseIr`](crate::ir::TeiParseIr) out. It needs no HTTP,
//! no filesystem and no runtime infrastructure. The
//! [`GrobidParser::lift_tei`](crate::GrobidParser) compatibility stage is a
//! thin delegate onto the same code — the algorithm is not duplicated.

use super::envelope::TeiEnvelope;
use da_ports::parser::{
    CharSpan, DeclaredArxivIdentity, DeclaredIdMissingReason, ExtractedFormula, ExtractedTable,
    FormulaMissingReason, FormulaRepresentation, HeaderAuthor, TableCell, TableMissingReason,
};
use std::collections::{HashMap, HashSet};

/// Internal span for a direct-child `<div>` during recursive TEI walk (T02).
/// `content_start..content_end` is the inner content range (exclusive of the
/// enclosing `<div ...>` and `</div>`). `n_attr` is the path component.
struct DivSpan {
    content_start: usize,
    content_end: usize,
    n_attr: Option<String>,
}

/// Raw in-text `<ref type="bibr">` occurrence captured before it is mapped
/// onto a back-matter bibliography entry (T03). `target` is the `xml:id` of
/// the cited `<biblStruct>` with the leading `#` stripped; refs without a
/// target cannot be attached and are dropped at mapping time.
struct RawRefAnchor {
    target: Option<String>,
    quote: String,
    section_path_id: Option<String>,
    offset: Option<u32>,
    /// Flat-pass window ordinal: anchors emitted from one `<head>` window
    /// share an ordinal so path collision resolution can group them, while a
    /// later window that repeats the same raw path resolves independently.
    /// Nested-pass anchors all use 0 (paths are uniquified on the fly).
    window_ordinal: usize,
}

/// Borrowed TEI section-parse context shared by every table and formula in
/// one document: the body slice, its offset within the full TEI, the section
/// text windows (with resolved `path_id`), and the ordinal of each section
/// path (M400 S02 T01 tables; M400 S03 T01 formulas reuse the same windows so
/// section identity stays consistent across both extractors).
struct SectionParseContext<'a> {
    body: Option<&'a str>,
    body_start: usize,
    windows: &'a [(usize, usize, Option<String>)],
    ordinal_by_path: &'a HashMap<String, usize>,
}

/// Provider-neutral pure TEI lifter (R105): `TeiEnvelope` in, this-cut
/// `TeiParseIr` out.
///
/// Owns the single `lift_tei` algorithm and every extract helper
/// (sections, citations, tables, formulas). Stateless: identical input
/// bytes always produce an identical IR, so offline cached-TEI replay is
/// byte-equal across replays. Empty and malformed XML stay total and
/// non-panicking (typed missingness instead of errors) exactly as on the
/// compatibility path — there is exactly one code path behind both
/// entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeiLifter;

impl TeiLifter {
    /// Build the stateless lifter.
    pub fn new() -> Self {
        Self
    }

    /// Lift the verbatim raw TEI carried by `envelope` into this-cut IR.
    ///
    /// Pure: no HTTP, no filesystem, no runtime infrastructure. Bytes are
    /// decoded lossily only when they are not valid UTF-8 (total,
    /// non-panicking); every real GROBID TEI payload and every preserved
    /// fixture is valid UTF-8 and decodes byte-identically to the
    /// compatibility path. Envelope identity (backend id / profile
    /// fingerprint) is provenance metadata and never changes lift output.
    pub fn lift(
        &self,
        envelope: &TeiEnvelope,
        paper_id: impl Into<String>,
        pdf_hash: impl Into<String>,
    ) -> crate::ir::TeiParseIr {
        let tei_xml = String::from_utf8_lossy(envelope.raw_tei()).into_owned();
        Self::lift_tei(tei_xml, paper_id, pdf_hash)
    }

    /// Lift raw TEI into this-cut IR. Does not call Java.
    ///
    /// Single algorithm owner since M464-z84in2 S02/T02: the
    /// `parse.stage.lift_tei` compatibility stage
    /// (`GrobidParser::lift_tei`, recorded in
    /// data/parse/api_compatibility_ledger.yaml) delegates here, as does the
    /// envelope entry [`TeiLifter::lift`]. Semantics, determinism and the
    /// intentional-absence list are unchanged (compatibility obligation
    /// stays in force).
    pub fn lift_tei(
        tei_xml: String,
        paper_id: impl Into<String>,
        pdf_hash: impl Into<String>,
    ) -> crate::ir::TeiParseIr {
        let title = Self::extract_title(&tei_xml);
        let abstract_text = Self::extract_abstract(&tei_xml);
        let body_text = Self::extract_body(&tei_xml);
        let sections = Self::extract_sections(&tei_xml);
        let citations = Self::extract_citations(&tei_xml);
        let (tables, table_missingness) = Self::extract_tables(&tei_xml);
        let (formulas, formula_missingness) = Self::extract_formulas(&tei_xml);
        // S07/T02 (M464-z84in2): declared arXiv identity and document-ordered
        // header authors lift through this single pure algorithm, header-scoped
        // so bibliography persName entries never become authors.
        let (declared_arxiv_id, declared_id_missingness) =
            Self::extract_declared_arxiv_id(&tei_xml);
        let header_authors = Self::extract_header_authors(&tei_xml);
        crate::ir::TeiParseIr {
            tei_xml,
            paper_id: paper_id.into(),
            title,
            abstract_text,
            body_text,
            sections,
            citations,
            tables,
            table_missingness,
            formulas,
            formula_missingness,
            // S07 P0 families (M464-z84in2): declared identity + header
            // authors since T02; figures and visual callouts stay typed-empty
            // until T03 adds their families — never invented here.
            declared_arxiv_id,
            declared_id_missingness,
            header_authors,
            figures: Vec::new(),
            visual_callouts: Vec::new(),
            pdf_hash: pdf_hash.into(),
        }
    }

    /// Extract title from TEI XML.
    /// Handles both `<title>Text</title>` and `<title level="a" type="main">Text</title>`.
    pub(crate) fn extract_title(tei: &str) -> String {
        if let Some(start) = tei.find("<title") {
            // Find the closing > of the opening tag (may have attributes)
            if let Some(tag_end) = tei[start..].find('>') {
                let after = &tei[start + tag_end + 1..];
                if let Some(end) = after.find("</title>") {
                    return Self::strip_xml_tags(&after[..end]);
                }
            }
        }
        String::new()
    }

    /// Extract abstract from TEI XML.
    pub(crate) fn extract_abstract(tei: &str) -> String {
        if let Some(start) = tei.find("<abstract")
            && let Some(content_start) = tei[start..].find('>')
        {
            let after = &tei[start + content_start + 1..];
            if let Some(end) = after.find("</abstract>") {
                return Self::strip_xml_tags(&after[..end]);
            }
        }
        String::new()
    }

    /// Extract body text from TEI XML.
    pub(crate) fn extract_body(tei: &str) -> String {
        if let Some(start) = tei.find("<body")
            && let Some(content_start) = tei[start..].find('>')
        {
            let after = &tei[start + content_start + 1..];
            if let Some(end) = after.find("</body>") {
                return Self::strip_xml_tags(&after[..end]);
            }
        }
        String::new()
    }

    // ─── S07/T02 (M464-z84in2): declared arXiv identity + header authors ───

    /// Borrowed TEI header region: `<teiHeader` up to (not including)
    /// `</teiHeader>`, tolerant of namespace declarations and arbitrary
    /// attributes on the opening tag. `None` when the TEI carries no header
    /// at all (total, never panicking).
    fn tei_header_region(tei: &str) -> Option<&str> {
        let start = tei.find("<teiHeader")?;
        let after = &tei[start..];
        let end_rel = after.find("</teiHeader>")?;
        Some(&after[..end_rel])
    }

    /// Extract the declared arXiv identity from the TEI header
    /// (M464-z84in2 S07/T02).
    ///
    /// Declared-only semantics: the identity exists when the header carries
    /// a usable `<idno type="arXiv">` value (GROBID emits the article's own
    /// biblStruct idno, e.g. `arXiv:2507.19457v2[cs.CL]`). The first arXiv
    /// idno in header document order wins; attribute order and extra
    /// namespace declarations are tolerated. `raw` keeps the full declared
    /// value verbatim; the revision suffix is carried ONLY when the value
    /// explicitly declares one — never guessed, never defaulted to `v1`.
    /// Every unusable shape surfaces as typed missingness (`no_idno` /
    /// `malformed`) instead of a repaired value: missing or eroded identity
    /// is never invented (R087).
    pub(crate) fn extract_declared_arxiv_id(
        tei: &str,
    ) -> (
        Option<DeclaredArxivIdentity>,
        Option<DeclaredIdMissingReason>,
    ) {
        let Some(header) = Self::tei_header_region(tei) else {
            return (None, Some(DeclaredIdMissingReason::NoIdno));
        };
        let mut search = 0usize;
        while let Some(rel) = header[search..].find("<idno") {
            let tag_start = search + rel;
            let Some(tag_end_rel) = header[tag_start..].find('>') else {
                break;
            };
            let open_tag = &header[tag_start..=tag_start + tag_end_rel];
            let next = tag_start + tag_end_rel + 1;
            if Self::extract_attr(open_tag, "type").as_deref() == Some("arXiv") {
                let value_region = &header[next..];
                let Some(value_end) = value_region.find("</idno>") else {
                    return (
                        None,
                        Some(DeclaredIdMissingReason::Malformed(
                            "declared arXiv idno element is unterminated".to_string(),
                        )),
                    );
                };
                let raw = &value_region[..value_end];
                if raw.trim().is_empty() {
                    return (
                        None,
                        Some(DeclaredIdMissingReason::Malformed(
                            "declared arXiv idno is empty".to_string(),
                        )),
                    );
                }
                return match Self::parse_declared_arxiv_value(raw) {
                    Some((bare_id, revision)) => (
                        Some(DeclaredArxivIdentity {
                            raw: raw.to_string(),
                            bare_id,
                            revision,
                            source_digest: Self::source_digest(raw.as_bytes()),
                        }),
                        None,
                    ),
                    None => (
                        None,
                        Some(DeclaredIdMissingReason::Malformed(
                            "declared arXiv idno carries no parseable arXiv id".to_string(),
                        )),
                    ),
                };
            }
            search = next;
        }
        (None, Some(DeclaredIdMissingReason::NoIdno))
    }

    /// Parse the first modern arXiv id (`\d{4}\.\d{4,5}`) out of a declared
    /// idno value, plus the explicitly declared revision suffix (`v\d+`)
    /// when one directly follows (e.g. `arXiv:2507.19457v2[cs.CL]` →
    /// `("2507.19457", Some("v2"))`). Bounded and deterministic: the first
    /// match in value order wins, the fraction is greedy (4-or-5 digits) and
    /// a candidate never starts inside a longer number. No match → `None`
    /// (the caller emits typed `Malformed`; nothing is guessed).
    fn parse_declared_arxiv_value(raw: &str) -> Option<(String, Option<String>)> {
        let bytes = raw.as_bytes();
        let n = bytes.len();
        let mut i = 0usize;
        while i < n {
            let starts_number = bytes[i].is_ascii_digit()
                && (i == 0 || !(bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.'));
            let head_ok = i + 4 < n
                && bytes[i + 1].is_ascii_digit()
                && bytes[i + 2].is_ascii_digit()
                && bytes[i + 3].is_ascii_digit()
                && bytes[i + 4] == b'.';
            if starts_number && head_ok {
                // Fraction: greedy 5 digits, then 4.
                for frac in (4..=5usize).rev() {
                    let frac_end = i + 5 + frac;
                    if frac_end > n {
                        continue;
                    }
                    let fraction_digits = (0..frac).all(|k| bytes[i + 5 + k].is_ascii_digit());
                    if !fraction_digits {
                        continue;
                    }
                    // A digit continuing the number rejects this split.
                    if frac_end < n && bytes[frac_end].is_ascii_digit() {
                        continue;
                    }
                    // Optional declared revision: `v` + digits, directly
                    // adjacent to the id — never inferred from context.
                    let revision = if frac_end + 1 < n
                        && bytes[frac_end] == b'v'
                        && bytes[frac_end + 1].is_ascii_digit()
                    {
                        let mut rev_end = frac_end + 2;
                        while rev_end < n && bytes[rev_end].is_ascii_digit() {
                            rev_end += 1;
                        }
                        Some(raw[frac_end..rev_end].to_string())
                    } else {
                        None
                    };
                    return Some((raw[i..frac_end].to_string(), revision));
                }
            }
            i += 1;
        }
        None
    }

    /// Extract document-ordered header authors (M464-z84in2 S07/T02).
    ///
    /// Built ONLY from `<persName>` elements inside the TEI header — the
    /// hundreds of bibliography `persName` entries in the back matter are
    /// citations and never become authors. Order is source document order
    /// (no sorting). Unicode and incomplete names survive verbatim (a
    /// surname-only author stays surname-only); an empty persName stays an
    /// empty-name entry — count honesty beats dropping. Decomposition is
    /// taken only from declared `<forename>`/`<surname>` children, never
    /// guessed from the display name.
    pub(crate) fn extract_header_authors(tei: &str) -> Vec<HeaderAuthor> {
        let Some(header) = Self::tei_header_region(tei) else {
            return Vec::new();
        };
        let mut authors = Vec::new();
        let mut search = 0usize;
        while let Some(rel) = header[search..].find("<persName") {
            let tag_start = search + rel;
            let Some(tag_end_rel) = header[tag_start..].find('>') else {
                break;
            };
            let open_end = tag_start + tag_end_rel;
            let self_closing = header[..open_end].ends_with('/');
            // Content runs to the closing tag; an unterminated persName
            // (malformed TEI) stays total by extending to the header end —
            // the entry is kept, never silently dropped.
            let content_end = header[open_end + 1..]
                .find("</persName>")
                .map_or(header.len(), |end| open_end + 1 + end);
            let content = if self_closing {
                ""
            } else {
                &header[open_end + 1..content_end]
            };
            let name = Self::normalize_author_display_name(content);
            authors.push(HeaderAuthor {
                forename: Self::joined_declared_child_text(content, "forename"),
                surname: Self::joined_declared_child_text(content, "surname"),
                source_digest: Self::source_digest(name.as_bytes()),
                name,
            });
            search = open_end + 1;
        }
        authors
    }

    /// Author display name: the persName text content with tags stripped,
    /// single spaces inserted at element boundaries (GROBID emits child
    /// elements without whitespace, e.g. `<forename>…</forename><surname>…
    /// </surname>`), and internal whitespace runs normalized. Otherwise
    /// verbatim — Unicode, diacritics and incomplete names are preserved,
    /// never reordered or dropped.
    fn normalize_author_display_name(content: &str) -> String {
        Self::strip_xml_tags(&content.replace("><", "> <"))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Joined text of every declared `<child>` element inside a persName
    /// content (document order, single-space separated). `None` when the
    /// content declares no such child at all; `Some("")` when the child is
    /// declared but empty — decomposition is only ever the declared one,
    /// never guessed from the display name.
    fn joined_declared_child_text(content: &str, child: &str) -> Option<String> {
        let open_needle = format!("<{child}");
        let close_needle = format!("</{child}>");
        let mut parts: Vec<String> = Vec::new();
        let mut found_any = false;
        let mut search = 0usize;
        while let Some(rel) = content[search..].find(&open_needle) {
            let tag_start = search + rel;
            // Element-name boundary: `<forename>` / `<forename type=...>`,
            // never a longer element that merely starts with the same text.
            let next_char = content[tag_start + open_needle.len()..].chars().next();
            if !matches!(
                next_char,
                Some('>') | Some('/') | Some(' ') | Some('\t') | Some('\n') | Some('\r')
            ) {
                search = tag_start + open_needle.len();
                continue;
            }
            found_any = true;
            let Some(tag_end_rel) = content[tag_start..].find('>') else {
                break;
            };
            let open_end = tag_start + tag_end_rel;
            if content[..open_end].ends_with('/') {
                // Declared-empty self-closing child.
                search = open_end + 1;
                continue;
            }
            let child_end = content[open_end + 1..]
                .find(&close_needle)
                .map_or(content.len(), |end| open_end + 1 + end);
            let text = Self::strip_xml_tags(&content[open_end + 1..child_end]);
            if !text.is_empty() {
                parts.push(text);
            }
            search = open_end + 1;
        }
        if !found_any {
            return None;
        }
        if parts.is_empty() {
            return Some(String::new());
        }
        Some(parts.join(" "))
    }

    /// Deterministic SHA-256 hex digest over source content bytes — the
    /// shared source-digest evidence convention (R087) used by tables,
    /// formulas and the S07 P0 families.
    fn source_digest(content: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Extract sections from TEI body.
    /// GROBID structures body as nested `<div><head n="1">Title</head><p>...</p></div>`.
    /// Level is derived from the `n` attribute on `<head>` (1, 2, 3...).
    pub(crate) fn extract_sections(tei: &str) -> Vec<da_ports::parser::Section> {
        // Try nested <div> walk first (GROBID-native TEI structure).
        // Falls back to flat <head> scan for legacy parsers that do not emit
        // nested divs (backward compat — path_id stays None in that path).
        let body = match tei.find("<body") {
            Some(start) => {
                let after = &tei[start..];
                match after.find('>') {
                    Some(tag_end) => {
                        let rest = &after[tag_end + 1..];
                        match rest.find("</body>") {
                            Some(end) => &rest[..end],
                            None => return Vec::new(),
                        }
                    }
                    None => return Vec::new(),
                }
            }
            None => return Vec::new(),
        };

        // Detect nested div structure: <div> containing child <div>.
        // Detect nested div structure via find_child_divs (reuses the same logic
        // as the recursive walker — consistent results).
        if Self::has_nested_divs(body) {
            Self::extract_sections_recursive(body)
        } else {
            Self::extract_sections_flat(body)
        }
    }

    /// Detect whether the TEI body contains nested `<div>` elements.
    /// Heuristic: any `<div>` whose matching `</div>` contains another `<div>`
    /// before it closes. Sibling divs at the same level do NOT count as nested.
    fn has_nested_divs(body: &str) -> bool {
        let child_divs = Self::find_child_divs(body);
        for span in &child_divs {
            let content = &body[span.content_start..span.content_end];
            if content.contains("<div") {
                return true;
            }
        }
        false
    }

    /// Recursive TEI `<div>` walker (ADR-053 Layer A T02).
    ///
    /// For each `<div n="N">` with a `<head>` child, produces a `Section` with:
    /// - `path_id` = parent path + "." + div n attribute (or auto-increment)
    /// - `parent_section_id` = `sec-<parent_path>` (None for roots)
    /// - `order_in_parent` = 0-based sibling index
    /// - `section_id` = `sec-<path_id>` (stable hash for edge wiring)
    /// - `level` = path component count (1-based depth)
    fn extract_sections_recursive(body: &str) -> Vec<da_ports::parser::Section> {
        let mut out = Vec::new();
        let mut used_paths = HashSet::new();
        Self::walk_divs(body, "", &mut used_paths, &mut out);
        out
    }

    /// Walk `<div>` children at the given nesting level.
    ///
    /// `parent_path` is the path_id of the parent div (empty for top-level).
    /// Processes the direct children of `body` (not nested grandchildren —
    /// those are handled by recursion on each child's content).
    fn walk_divs(
        body: &str,
        parent_path: &str,
        used_paths: &mut HashSet<String>,
        out: &mut Vec<da_ports::parser::Section>,
    ) {
        let div_spans = Self::find_child_divs(body);
        for (local_sibling, div_span) in div_spans.iter().enumerate() {
            let div_content = &body[div_span.content_start..div_span.content_end];
            // The path component is the LAST segment of the n attribute (e.g.
            // "1.1" → "1". This is how GROBID encodes nested divs: n="1.1"
            // means section 1, subsection 1.
            let path_component = div_span
                .n_attr
                .as_deref()
                .and_then(|n| n.split('.').next_back())
                .unwrap_or("")
                .to_string();
            let path_component = if path_component.is_empty() {
                (local_sibling as u32 + 1).to_string()
            } else {
                path_component
            };
            let base_path = if parent_path.is_empty() {
                path_component.to_string()
            } else {
                format!("{}.{}", parent_path, path_component)
            };
            // Real GROBID TEI can repeat `n` among sibling divs. Preserve
            // normal paths and deterministically suffix only collisions.
            let mut path_id = base_path.clone();
            let mut collision = 2u32;
            while used_paths.contains(&path_id) {
                path_id = format!("{base_path}~{collision}");
                collision += 1;
            }
            used_paths.insert(path_id.clone());
            let section_id = format!("sec-{}", path_id);
            let parent_section_id = if parent_path.is_empty() {
                None
            } else {
                Some(format!("sec-{}", parent_path))
            };
            let order_in_parent = local_sibling as u32;

            // Extract <head> title from this div's direct children (before any
            // nested <div>). Fall back to empty title if missing.
            let (title, text) = Self::extract_head_and_text(div_content);
            let level = path_id.split('.').count() as u32;

            if !title.is_empty() || !text.is_empty() {
                use da_ports::parser::Section;
                out.push(Section {
                    title,
                    text,
                    level,
                    path_id: Some(path_id.clone()),
                    parent_section_id: parent_section_id.clone(),
                    order_in_parent: Some(order_in_parent),
                    section_id: Some(section_id.clone()),
                });
            }

            // Recurse using the resolved parent path so duplicate roots cannot
            // produce duplicate descendant identities.
            Self::walk_divs(div_content, &path_id, used_paths, out);
        }
    }

    /// Find direct child `<div>` elements at the top level of `body` (not nested
    /// inside other divs). Returns spans with their `n` attribute and content
    /// range (between open tag end and matching `</div>`).
    ///
    /// Uses a depth counter: only divs at depth 1 (i.e. direct children of the
    /// fragment) are included. Deeper nesting is handled by recursion on each
    /// span's content.
    fn find_child_divs(body: &str) -> Vec<DivSpan> {
        let mut spans = Vec::new();
        let mut pos = 0;
        while let Some(tag_pos) = body[pos..].find("<div") {
            let abs = pos + tag_pos;
            // Skip closing tags.
            if body[abs..].starts_with("</div") {
                pos = abs + 1;
                continue;
            }
            // Opening tag: find end of tag (the `>`).
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &body[abs..=tag_end];
            let n_attr = Self::extract_n_attr(open_tag);
            let content_start = tag_end + 1;
            let content_end = match Self::find_matching_close_div(body, content_start) {
                Some(e) => e,
                None => break,
            };
            spans.push(DivSpan {
                content_start,
                content_end,
                n_attr,
            });
            // Continue scanning AFTER this div's close (siblings, not children).
            pos = content_end + "</div>".len();
        }
        spans
    }

    /// Find the position of the `</div>` that closes the `<div>` opened at or
    /// after `start`. Handles nesting by counting open/close div tags.
    fn find_matching_close_div(body: &str, start: usize) -> Option<usize> {
        let mut depth: i32 = 1;
        let mut pos = start;
        while depth > 0 {
            let rest = &body[pos..];
            let next_open = rest.find("<div").map(|i| (pos + i, false));
            let next_close = rest.find("</div>").map(|i| (pos + i, true));
            match (next_open, next_close) {
                (Some((o, _)), Some((c, _))) if o < c => {
                    // Opening <div ...> encountered.
                    if !body[o..].starts_with("</div") {
                        depth += 1;
                    }
                    pos = o + 1;
                }
                (_, Some((c, _))) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(c);
                    }
                    pos = c + 1;
                }
                _ => return None,
            }
        }
        None
    }

    /// Extract the `n="..."` attribute value from a `<div n="1.2">` opening tag.
    fn extract_n_attr(open_tag: &str) -> Option<String> {
        if let Some(n_pos) = open_tag.find("n=\"") {
            let after = &open_tag[n_pos + 3..];
            if let Some(end) = after.find('"') {
                let v = &after[..end];
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    /// Extract `<head>` title and surrounding text from a div's direct content
    /// (ignoring nested `<div>` children — those are handled by recursion).
    fn extract_head_and_text(div_content: &str) -> (String, String) {
        // Find the first <head>...</head> at this level.
        let (title, after_head_pos) = if let Some(hs) = div_content.find("<head") {
            let tag_end = div_content[hs..].find('>').map(|e| hs + e);
            let content_start = match tag_end {
                Some(e) => e + 1,
                None => return (String::new(), String::new()),
            };
            let head_close = div_content[content_start..].find("</head>");
            match head_close {
                Some(e) => {
                    let title =
                        Self::strip_xml_tags(&div_content[content_start..content_start + e]);
                    (title, content_start + e + "</head>".len())
                }
                None => (String::new(), 0),
            }
        } else {
            (String::new(), 0)
        };
        // Text is everything after </head> up to the first nested <div> (if any).
        let text_end = div_content[after_head_pos..]
            .find("<div")
            .map(|e| after_head_pos + e)
            .unwrap_or(div_content.len());
        let text = Self::strip_xml_tags(&div_content[after_head_pos..text_end]);
        (title, text)
    }

    /// Legacy flat-structure TEI scanner — used when the body has no nested
    /// `<div>` elements. Produces sections with `path_id = None` to preserve
    /// backward compat with parsers that never populated navigation fields.
    fn extract_sections_flat(body: &str) -> Vec<da_ports::parser::Section> {
        use da_ports::parser::Section;
        let mut sections = Vec::new();
        let mut pos = 0;
        while let Some(head_start) = body[pos..].find("<head") {
            let abs = pos + head_start;
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &body[abs..=tag_end];
            // Extract path_id from <head n="N.M">. GROBID encodes the section
            // number lexically (e.g. n="3.1") regardless of XML nesting
            // (which is flat in real GROBID output — see MEM654).
            let path_component = Self::extract_head_n_string(open_tag);
            // level = number of path components (1 for "1", 2 for "3.1").
            // For backward compat with the old extract_head_level behavior,
            // when n is a single integer, level = that integer (not 1).
            // This matches the old test expectation: <head n="2"> -> level 2.
            let level = if path_component.contains('.') {
                path_component.split('.').count() as u32
            } else {
                path_component.parse::<u32>().unwrap_or(1)
            };
            let content_start = tag_end + 1;
            let head_close = match body[content_start..].find("</head>") {
                Some(e) => content_start + e,
                None => break,
            };
            let title = Self::strip_xml_tags(&body[content_start..head_close]);
            let after_head = head_close + "</head>".len();
            let next_head = body[after_head..].find("<head").map(|e| after_head + e);
            let text_end = next_head.unwrap_or(body.len());
            let text = Self::strip_xml_tags(&body[after_head..text_end]);
            if !title.is_empty() || !text.is_empty() {
                sections.push(Section {
                    title,
                    text,
                    level,
                    // ADR-053 Layer A: populate path_id + section_id from
                    // <head n="N.M"> even in flat TEI (MEM654: GROBID output
                    // is structurally flat; section hierarchy is lexical).
                    path_id: Some(path_component.clone()),
                    parent_section_id: None,
                    order_in_parent: None,
                    section_id: Some(format!("sec-{}", path_component)),
                });
            }
            pos = text_end;
            // Guard against infinite loop when text_end == abs (e.g. empty
            // section text where the next <head> is adjacent). Advance past
            // the current <head> tag.
            if pos <= tag_end {
                pos = tag_end + 1;
            }
        }

        // Real GROBID output can restart lexical `n` numbering for appendices
        // or unnumbered blocks. Resolve those collisions before wiring parents.
        Self::uniquify_flat_paths(&mut sections);
        // Post-processing pass: compute parent_section_id and order_in_parent
        // from the now-unique path_id tree.
        Self::populate_parent_and_order(&mut sections);
        sections
    }

    /// Extract the full `n` attribute value from `<head n="3.1">` as a string.
    /// Returns "1" when n is missing or unparseable (default to level-1 root).
    fn extract_head_n_string(open_tag: &str) -> String {
        if let Some(n_pos) = open_tag.find("n=\"") {
            let after = &open_tag[n_pos + 3..];
            if let Some(end) = after.find('"') {
                let v = &after[..end];
                // Validate: non-empty and consists of digits and dots only.
                if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    return v.to_string();
                }
            }
        }
        "1".to_string()
    }

    /// Make flat lexical paths unique while preserving the latest resolved
    /// parent for descendants after a numbering restart.
    fn uniquify_flat_paths(sections: &mut [da_ports::parser::Section]) {
        use std::collections::HashMap;
        let mut used = HashSet::new();
        let mut latest_resolved: HashMap<String, String> = HashMap::new();
        for section in sections.iter_mut() {
            let Some(raw_path) = section.path_id.clone() else {
                continue;
            };
            let candidate = match raw_path.rsplit_once('.') {
                Some((raw_parent, component)) => latest_resolved
                    .get(raw_parent)
                    .map(|parent| format!("{parent}.{component}"))
                    .unwrap_or_else(|| raw_path.clone()),
                None => raw_path.clone(),
            };
            let mut resolved = candidate.clone();
            let mut collision = 2u32;
            while used.contains(&resolved) {
                resolved = format!("{candidate}~{collision}");
                collision += 1;
            }
            used.insert(resolved.clone());
            latest_resolved.insert(raw_path, resolved.clone());
            section.path_id = Some(resolved.clone());
            section.section_id = Some(format!("sec-{resolved}"));
        }
    }

    /// Populate `parent_section_id` and `order_in_parent` for each Section from
    /// the path_id tree. Mutates `sections` in place. No-op when path_id is None.
    fn populate_parent_and_order(sections: &mut [da_ports::parser::Section]) {
        use std::collections::HashMap;
        // Track sibling counters per parent path.
        let mut sibling_counters: HashMap<String, u32> = HashMap::new();
        for section in sections.iter_mut() {
            let (Some(path_id), Some(section_id)) =
                (section.path_id.as_deref(), section.section_id.as_deref())
            else {
                continue;
            };
            // parent_path = everything before the last '.'.
            let parent_path = path_id.rfind('.').map(|pos| &path_id[..pos]);
            // order_in_parent: increment per-parent counter.
            let counter_key = parent_path.unwrap_or("").to_string();
            let order = sibling_counters.entry(counter_key).or_insert(0);
            section.order_in_parent = Some(*order);
            *order += 1;
            // parent_section_id: sec-<parent_path> when parent exists.
            if let Some(pp) = parent_path {
                section.parent_section_id = Some(format!("sec-{}", pp));
            }
            // section_id already populated — ensure it matches sec-<path_id>.
            let expected = format!("sec-{}", path_id);
            if section_id != expected {
                section.section_id = Some(expected);
            }
        }
    }

    /// Extract the `n` attribute value from a `<head n="1">` tag.
    #[allow(dead_code)]
    pub(crate) fn extract_head_level(open_tag: &str) -> u32 {
        if let Some(n_pos) = open_tag.find("n=\"") {
            let after = &open_tag[n_pos + 3..];
            if let Some(end) = after.find('"') {
                return after[..end].parse().unwrap_or(1);
            }
        }
        1
    }

    /// Extract citations from TEI `<back>` references.
    /// GROBID places references in `<div type="references"><listBibl><biblStruct>`.
    /// T03 (M398-cz4pzx/S03): also scans in-text `<ref type="bibr" target="#bN">`
    /// anchors in the body and attaches them to the matching bibliography
    /// entry (resolved via biblStruct `xml:id`), so a revision-owned CITES/HAS_PART
    /// edge can later carry exact source spans (D247/D249).
    pub(crate) fn extract_citations(tei: &str) -> Vec<da_ports::parser::CitationEntry> {
        use da_ports::parser::CitationEntry;
        let back = match tei.find("<back>") {
            Some(s) => match tei[s..].find("</back>") {
                Some(e) => &tei[s..s + e],
                None => return Vec::new(),
            },
            None => return Vec::new(),
        };

        let mut citations = Vec::new();
        // GROBID assigns xml:id="bN" to back-matter biblStruct entries;
        // in-text `<ref type="bibr" target="#bN">` anchors resolve through
        // this map. biblStruct without xml:id falls back to its ordinal key
        // ("b{index}") for synthetic/hand-written TEI.
        let mut id_to_index: HashMap<String, usize> = HashMap::new();
        let mut pos = 0;
        while let Some(bibl_start) = back[pos..].find("<biblStruct") {
            let abs = pos + bibl_start;
            // Find end of opening <biblStruct ...> tag (may have attributes)
            let tag_end = match back[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let bibl_end = match back[tag_end..].find("</biblStruct>") {
                Some(e) => tag_end + e + "</biblStruct>".len(),
                None => break,
            };
            let entry = &back[tag_end..bibl_end];

            // Title: first <title level="a" type="main"> inside <analytic>
            let title = Self::extract_citation_title(entry);
            // DOI: <idno type="DOI">
            let doi = Self::extract_idno(entry, "DOI");
            // arxiv: <idno type="arXiv">
            let arxiv_id = Self::extract_idno(entry, "arXiv").map(|s| {
                // strip "arXiv:" prefix and version suffix
                s.strip_prefix("arXiv:")
                    .unwrap_or(&s)
                    .split('v')
                    .next()
                    .unwrap_or(&s)
                    .to_string()
            });
            let raw_text = Self::strip_xml_tags(entry);

            let xml_id = Self::extract_bibl_xml_id(&back[abs..=tag_end]);
            let key = xml_id.unwrap_or_else(|| format!("b{}", citations.len()));
            id_to_index.insert(key, citations.len());

            citations.push(CitationEntry {
                raw_text,
                doi,
                arxiv_id,
                title,
                anchors: Vec::new(),
            });
            pos = bibl_end;
        }

        // T03: attach in-text `<ref type="bibr">` anchors resolved against
        // back-matter biblStruct xml:id targets. Refs without a target or with
        // an unresolvable target are dropped here (the citation then stays
        // anchor-less and ingest records typed missingness no_in_text_anchor).
        for anchor in Self::extract_ref_anchors(tei) {
            let Some(target) = anchor.target.as_deref() else {
                continue;
            };
            let Some(&idx) = id_to_index.get(target) else {
                continue;
            };
            if let Some(entry) = citations.get_mut(idx) {
                entry.anchors.push(da_ports::parser::CitationAnchor {
                    ref_id: target.to_string(),
                    section_path_id: anchor.section_path_id,
                    quote: anchor.quote,
                    offset: anchor.offset,
                });
            }
        }
        citations
    }

    /// Extract `xml:id` from a `<biblStruct ...>` opening tag. GROBID emits
    /// `xml:id="b0"` on back-matter bibliography entries; in-text refs point
    /// at those ids via `target="#bN"`.
    fn extract_bibl_xml_id(open_tag: &str) -> Option<String> {
        for attr in ["xml:id=\"", "xml:id='"] {
            if let Some(pos) = open_tag.find(attr) {
                let after = &open_tag[pos + attr.len()..];
                let end = after.find(['"', '\''])?;
                let value = after[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extract the `target` attribute of a `<ref>` opening tag, stripping any
    /// leading `#` (GROBID emits `target="#b12"` pointing at a biblStruct
    /// `xml:id`). Returns None when the ref has no target.
    fn extract_ref_target(open_tag: &str) -> Option<String> {
        for attr in ["target=\"", "target='"] {
            if let Some(pos) = open_tag.find(attr) {
                let after = &open_tag[pos + attr.len()..];
                let end = after.find(['"', '\''])?;
                let value = after[..end].trim();
                if !value.is_empty() {
                    return Some(value.trim_start_matches('#').to_string());
                }
            }
        }
        None
    }

    /// Scan TEI body for in-text `<ref type="bibr">` citation anchors (T03).
    /// Mirrors `extract_sections` so each anchor resolves to the same section
    /// `path_id` and text window the section extractor produces: nested div
    /// TEI walks divs recursively; flat TEI scans `<head>` windows and then
    /// applies the same flat-path collision resolution as the section pass.
    fn extract_ref_anchors(tei: &str) -> Vec<RawRefAnchor> {
        let Some(body) = Self::tei_body(tei) else {
            return Vec::new();
        };
        if Self::has_nested_divs(body) {
            let mut out = Vec::new();
            // The pre-first-div region belongs to no section (walk_divs only
            // creates sections for divs); anchors there stay path-unresolved.
            // Inter-sibling and tail gaps are handled inside walk_div_anchors
            // so each frame's coordinate system stays consistent.
            if let Some(first) = Self::find_child_divs(body).first()
                && first.content_start > 0
            {
                Self::scan_refs_in_window(body, 0, first.content_start, None, &mut out, 0);
            }
            let mut used_paths = HashSet::new();
            Self::walk_div_anchors(body, "", &mut used_paths, &mut out);
            out
        } else {
            let mut out = Vec::new();
            // Pre-first-head region belongs to no section (extract_sections_flat
            // starts sections at the first `<head>`), so anchors there stay
            // path-unresolved rather than being dropped silently.
            if let Some(first_head) = body.find("<head") {
                Self::scan_refs_in_window(body, 0, first_head, None, &mut out, 0);
            }
            Self::scan_flat_anchors(body, &mut out);
            // Resolve anchor paths against every head window (including
            // windows without anchors) using the same collision algorithm as
            // the section pass, so a later window repeating a raw path (e.g.
            // an appendix restarting at n="1") lands on the exact section path
            // (1~k) that extract_sections_flat produced.
            let windows = Self::flat_window_paths(body);
            Self::uniquify_flat_anchor_paths(&mut out, &windows);
            out
        }
    }

    /// Extract the inner content of the `<body>...</body>` element, or None.
    fn tei_body(tei: &str) -> Option<&str> {
        let start = tei.find("<body")?;
        let after = &tei[start..];
        let tag_end = after.find('>')?;
        let rest = &after[tag_end + 1..];
        let end = rest.find("</body>")?;
        Some(&rest[..end])
    }

    /// Recursive `<div>` walk for in-text anchors (nested TEI). Computes the
    /// same `path_id` as `walk_divs` (same collision handling) and scans the
    /// div's direct text window — everything after `</head>` up to the first
    /// nested `<div>` — so anchor offsets match `Section.text`. Inter-sibling
    /// gaps (after each div's close, before the next sibling or frame end)
    /// belong to no section and are scanned with a None path. All indices are
    /// relative to `body` (the current frame), so recursion into `div_content`
    /// keeps each level's coordinate system consistent.
    fn walk_div_anchors(
        body: &str,
        parent_path: &str,
        used_paths: &mut HashSet<String>,
        out: &mut Vec<RawRefAnchor>,
    ) {
        let div_spans = Self::find_child_divs(body);
        for (local_sibling, div_span) in div_spans.iter().enumerate() {
            let cs = div_span.content_start;
            let ce = div_span.content_end;
            let div_content = &body[cs..ce];
            let path_component = div_span
                .n_attr
                .as_deref()
                .and_then(|n| n.split('.').next_back())
                .unwrap_or("")
                .to_string();
            let path_component = if path_component.is_empty() {
                (local_sibling as u32 + 1).to_string()
            } else {
                path_component
            };
            let base_path = if parent_path.is_empty() {
                path_component.to_string()
            } else {
                format!("{}.{}", parent_path, path_component)
            };
            let mut path_id = base_path.clone();
            let mut collision = 2u32;
            while used_paths.contains(&path_id) {
                path_id = format!("{base_path}~{collision}");
                collision += 1;
            }
            used_paths.insert(path_id.clone());

            // This div's own text window, mapped to frame coordinates.
            let (w_start, w_end) = Self::div_text_window(div_content);
            let window_start = cs + w_start;
            let window_end = cs + w_end;
            if window_start < window_end {
                Self::scan_refs_in_window(
                    body,
                    window_start,
                    window_end,
                    Some(path_id.clone()),
                    out,
                    0,
                );
            }

            // Gap between this div's close and the next sibling (or frame
            // end): belongs to no section → anchors stay path-unresolved so
            // ingest records typed missingness anchor_section_path_unresolved.
            let next_start = div_spans
                .get(local_sibling + 1)
                .map(|s| s.content_start)
                .unwrap_or(body.len());
            let gap_start = ce + "</div>".len();
            if next_start > gap_start {
                Self::scan_refs_in_window(body, gap_start, next_start, None, out, 0);
            }

            Self::walk_div_anchors(div_content, &path_id, used_paths, out);
        }
    }

    /// Raw text window (start, end) of a div's direct content, mirroring
    /// `extract_head_and_text`: everything after `</head>` up to the first
    /// nested `<div>` (or the div end). Without a `<head>` the window starts
    /// at the div content start, exactly like the section extractor.
    fn div_text_window(div_content: &str) -> (usize, usize) {
        let after_head = Self::find_head_text_start(div_content).unwrap_or_default();
        let text_end = div_content[after_head..]
            .find("<div")
            .map(|e| after_head + e)
            .unwrap_or(div_content.len());
        (after_head, text_end)
    }

    /// Position just after the first `</head>` at this level, or None.
    fn find_head_text_start(div_content: &str) -> Option<usize> {
        let hs = div_content.find("<head")?;
        let tag_end = div_content[hs..].find('>')? + hs;
        let content_start = tag_end + 1;
        let head_close = div_content[content_start..].find("</head>")?;
        Some(content_start + head_close + "</head>".len())
    }

    /// Flat `<head>`-window scan for in-text anchors (flat TEI, MEM654),
    /// mirroring `extract_sections_flat`: section text is everything after
    /// `</head>` up to the next `<head>` (or body end).
    fn scan_flat_anchors(body: &str, out: &mut Vec<RawRefAnchor>) {
        let mut pos = 0;
        // Window ordinal 0 is the pre-first-head region (path-unresolved);
        // each <head> window gets 1, 2, ... so path-collision resolution can
        // tell a later window repeating the same raw path (e.g. an appendix
        // restarting at n="1") apart from the earlier one.
        let mut window_ordinal = 1usize;
        while let Some(head_start) = body[pos..].find("<head") {
            let abs = pos + head_start;
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &body[abs..=tag_end];
            let path_component = Self::extract_head_n_string(open_tag);
            let content_start = tag_end + 1;
            let head_close = match body[content_start..].find("</head>") {
                Some(e) => content_start + e,
                None => break,
            };
            let after_head = head_close + "</head>".len();
            let next_head = body[after_head..].find("<head").map(|e| after_head + e);
            let text_end = next_head.unwrap_or(body.len());
            Self::scan_refs_in_window(
                body,
                after_head,
                text_end,
                Some(path_component),
                out,
                window_ordinal,
            );
            pos = text_end;
            window_ordinal += 1;
            // Guard against infinite loop when text_end == abs (adjacent heads).
            if pos <= tag_end {
                pos = tag_end + 1;
            }
        }
    }

    /// Raw `n`-derived path of every flat `<head>` window in body order, plus
    /// whether the window would create a Section (`!title.is_empty() ||
    /// !text.is_empty()`, matching `extract_sections_flat`). Windows without
    /// anchors are included so collision resolution consumes the same path
    /// slots the section pass consumes.
    fn flat_window_paths(body: &str) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while let Some(head_start) = body[pos..].find("<head") {
            let abs = pos + head_start;
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &body[abs..=tag_end];
            let path_component = Self::extract_head_n_string(open_tag);
            let content_start = tag_end + 1;
            let head_close = match body[content_start..].find("</head>") {
                Some(e) => content_start + e,
                None => break,
            };
            let title = Self::strip_xml_tags(&body[content_start..head_close]);
            let after_head = head_close + "</head>".len();
            let next_head = body[after_head..].find("<head").map(|e| after_head + e);
            let text_end = next_head.unwrap_or(body.len());
            let text = Self::strip_xml_tags(&body[after_head..text_end]);
            out.push((path_component, !title.is_empty() || !text.is_empty()));
            pos = text_end;
            // Guard against infinite loop when text_end == abs (adjacent heads).
            if pos <= tag_end {
                pos = tag_end + 1;
            }
        }
        out
    }

    /// Resolve raw flat paths to the same uniquified paths the section pass
    /// produces (`uniquify_flat_paths`): a numbering restart makes "3" appear
    /// twice, so the second occurrence becomes "3~2". Anchors of one `<head>`
    /// window share its resolved path (they all carry the same `window_ordinal`),
    /// while a later window that repeats the same raw path (e.g. an appendix
    /// restarting at n="1") must re-resolve and pick up the collision suffix.
    /// Resolution iterates every window (not just windows with anchors) in
    /// body order using the same collision algorithm and content filter as the
    /// section pass, so anchor paths land exactly on the Section paths.
    fn uniquify_flat_anchor_paths(anchors: &mut [RawRefAnchor], windows: &[(String, bool)]) {
        let mut used = HashSet::new();
        let mut latest_resolved: HashMap<String, String> = HashMap::new();
        let mut resolved_by_ordinal: HashMap<usize, String> = HashMap::new();
        for (idx, (raw_path, has_content)) in windows.iter().enumerate() {
            if !has_content {
                continue;
            }
            let ordinal = idx + 1; // windows are 1-based; ordinal 0 = pre-head
            let candidate = match raw_path.rsplit_once('.') {
                Some((raw_parent, component)) => latest_resolved
                    .get(raw_parent)
                    .map(|parent| format!("{parent}.{component}"))
                    .unwrap_or_else(|| raw_path.clone()),
                None => raw_path.clone(),
            };
            let mut resolved = candidate.clone();
            let mut collision = 2u32;
            while used.contains(&resolved) {
                resolved = format!("{candidate}~{collision}");
                collision += 1;
            }
            used.insert(resolved.clone());
            latest_resolved.insert(raw_path.clone(), resolved.clone());
            resolved_by_ordinal.insert(ordinal, resolved);
        }
        for anchor in anchors.iter_mut() {
            if anchor.window_ordinal == 0 {
                // Pre-first-head region: belongs to no section, stays
                // path-unresolved (typed missingness at ingest).
                continue;
            }
            if let Some(resolved) = resolved_by_ordinal.get(&anchor.window_ordinal) {
                anchor.section_path_id = Some(resolved.clone());
            }
        }
    }

    /// Scan `raw[start..end]` for `<ref type="bibr">` anchors and push a
    /// `RawRefAnchor` per occurrence. The offset is the char position of the
    /// quote in the window's stripped-and-trimmed text (identical to
    /// `Section.text` construction), so consumers can join anchors to the
    /// stored section text directly.
    fn scan_refs_in_window(
        raw: &str,
        start: usize,
        end: usize,
        section_path_id: Option<String>,
        out: &mut Vec<RawRefAnchor>,
        window_ordinal: usize,
    ) {
        let mut pos = start;
        while let Some(rel) = raw[pos..end].find("<ref") {
            let abs = pos + rel;
            let tag_end = match raw[abs..end].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &raw[abs..=tag_end];
            // Only bibl refs are citation anchors (ignores <ref type="note">
            // and other non-bibliographic markers).
            if !(open_tag.contains("type=\"bibr\"") || open_tag.contains("type='bibr'")) {
                pos = abs + 1;
                continue;
            }
            let quote_start = tag_end + 1;
            let close = match raw[quote_start..end].find("</ref>") {
                Some(e) => quote_start + e,
                None => break,
            };
            let quote = Self::strip_xml_tags(&raw[quote_start..close]);
            let target = Self::extract_ref_target(open_tag);
            // Offset only makes sense against a real section text window.
            let offset = section_path_id
                .as_ref()
                .map(|_| Self::stripped_text_offset(raw, start, quote_start, end));
            out.push(RawRefAnchor {
                target,
                quote,
                section_path_id: section_path_id.clone(),
                offset,
                window_ordinal,
            });
            pos = close + "</ref>".len();
        }
    }

    /// Char offset of `pos` (absolute within `raw`) in the text produced by
    /// `strip_xml_tags(raw[start..end])` — the exact `Section.text`
    /// construction (tags stripped, then trimmed). Counts non-tag chars before
    /// `pos`, then subtracts the leading whitespace that trim removes.
    fn stripped_text_offset(raw: &str, start: usize, pos: usize, end: usize) -> u32 {
        let full: Vec<char> = Self::strip_xml_tags(&raw[start..end]).chars().collect();
        let leading = full.iter().take_while(|c| c.is_whitespace()).count();
        let mut count = 0usize;
        let mut in_tag = false;
        for ch in raw[start..pos].chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => count += 1,
                _ => {}
            }
        }
        count.saturating_sub(leading) as u32
    }

    // ─── M400 S02 T01: deterministic TEI table extraction ───

    /// Extract deterministic TEI table structures (M400 S02 T01).
    ///
    /// GROBID emits tables as `<figure type="table" xml:id="tab_0">
    /// <head>…</head><label>…</label><figDesc>…</figDesc>
    /// <table><row><cell …>…</cell></row></table></figure>` inside the
    /// `<body>` (coordinates requested via `teiCoordinates=["figure",
    /// "table","note"]`). Section identity mirrors `extract_sections`
    /// exactly — the same nested-div / flat-`<head>` window algorithm with
    /// the same path collision resolution — so a table maps to the same
    /// `path_id` the section extractor produced for its containing text
    /// window. Tables outside every section window (top-level gaps / trailing
    /// region) get `section_path_id: None` — typed missingness, never
    /// guessed into a neighbour.
    ///
    /// Returns the extracted tables plus the article-level typed missingness:
    /// `Some(NoTableElements)` when the TEI contains no `<table>` element at
    /// all (R087 — an empty Vec alone cannot distinguish that from a parser
    /// that did not populate tables).
    pub(crate) fn extract_tables(tei: &str) -> (Vec<ExtractedTable>, Option<TableMissingReason>) {
        let body = Self::tei_body(tei);
        let (windows, body_start) = match body {
            Some(b) => {
                let body_start = b.as_ptr() as usize - tei.as_ptr() as usize;
                (Self::section_windows(b), body_start)
            }
            None => (Vec::new(), 0),
        };

        // 0-based ordinal of each section path (section titles repeat, so
        // tables need the ordinal to disambiguate — mirrors
        // ExtractedEntity.section_ordinal, MEM685).
        let sections = Self::extract_sections(tei);
        let mut ordinal_by_path: HashMap<String, usize> = HashMap::new();
        for (idx, section) in sections.iter().enumerate() {
            if let Some(path) = section.path_id.as_deref() {
                ordinal_by_path.entry(path.to_string()).or_insert(idx);
            }
        }

        let mut tables = Vec::new();
        let mut table_count = 0usize;
        let mut pos = 0usize;
        while let Some(rel) = tei[pos..].find("<table") {
            let abs = pos + rel;
            // Skip closing tags and non-element matches.
            if tei[abs..].starts_with("</table") {
                pos = abs + 1;
                continue;
            }
            let tag_end = match tei[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let close = match tei[tag_end..].find("</table>") {
                Some(e) => tag_end + e,
                None => break,
            };
            table_count += 1;
            let open_tag = &tei[abs..=tag_end];
            let content = &tei[tag_end + 1..close];
            let table = Self::build_table(
                tei,
                open_tag,
                content,
                abs,
                close,
                &SectionParseContext {
                    body,
                    body_start,
                    windows: &windows,
                    ordinal_by_path: &ordinal_by_path,
                },
                table_count,
            );
            tables.push(table);
            pos = close + "</table>".len();
        }

        let missingness = if table_count == 0 {
            Some(TableMissingReason::NoTableElements)
        } else {
            None
        };
        (tables, missingness)
    }

    /// Section text windows with their resolved `path_id`, mirroring
    /// `extract_sections` exactly (nested div walk or flat `<head>` scan with
    /// the same path collision resolution). Windows cover the body with no
    /// gaps: regions outside any section (pre-first-div/head region,
    /// inter-sibling gaps, trailing region) carry `None` so a table or
    /// formula there is honestly reported as not-section-resolved instead of
    /// being guessed. Shared by table extraction (M400 S02 T01) and formula
    /// extraction (M400 S03 T01) so both land on the exact section paths the
    /// section pass produced.
    fn section_windows(body: &str) -> Vec<(usize, usize, Option<String>)> {
        let mut out = Vec::new();
        if Self::has_nested_divs(body) {
            let divs = Self::find_child_divs(body);
            if let Some(first) = divs.first()
                && first.content_start > 0
            {
                out.push((0, first.content_start, None));
            }
            let mut used = HashSet::new();
            Self::walk_section_windows(body, "", &mut used, &mut out);
        } else {
            Self::flat_section_windows(body, &mut out);
        }
        out
    }

    /// Recursive `<div>` walk producing section text windows (mirrors
    /// `walk_divs` path computation and `walk_div_anchors` window/gap
    /// coverage). Each div's direct text window (after `</head>` up to the
    /// first nested `<div>`) carries the div's resolved path; inter-sibling
    /// gaps and the trailing region belong to no section (`None`).
    fn walk_section_windows(
        body: &str,
        parent_path: &str,
        used_paths: &mut HashSet<String>,
        out: &mut Vec<(usize, usize, Option<String>)>,
    ) {
        let div_spans = Self::find_child_divs(body);
        for (local_sibling, div_span) in div_spans.iter().enumerate() {
            let cs = div_span.content_start;
            let ce = div_span.content_end;
            let div_content = &body[cs..ce];
            let path_component = div_span
                .n_attr
                .as_deref()
                .and_then(|n| n.split('.').next_back())
                .unwrap_or("")
                .to_string();
            let path_component = if path_component.is_empty() {
                (local_sibling as u32 + 1).to_string()
            } else {
                path_component
            };
            let base_path = if parent_path.is_empty() {
                path_component.to_string()
            } else {
                format!("{}.{}", parent_path, path_component)
            };
            let mut path_id = base_path.clone();
            let mut collision = 2u32;
            while used_paths.contains(&path_id) {
                path_id = format!("{base_path}~{collision}");
                collision += 1;
            }
            used_paths.insert(path_id.clone());

            let (w_start, w_end) = Self::div_text_window(div_content);
            let window_start = cs + w_start;
            let window_end = cs + w_end;
            if window_start < window_end {
                out.push((window_start, window_end, Some(path_id.clone())));
            }

            // Gap between this div's close and the next sibling (or frame
            // end): belongs to no section → None window.
            let next_start = div_spans
                .get(local_sibling + 1)
                .map(|s| s.content_start)
                .unwrap_or(body.len());
            let gap_start = ce + "</div>".len();
            if next_start > gap_start {
                out.push((gap_start, next_start, None));
            }

            Self::walk_section_windows(div_content, &path_id, used_paths, out);
        }
    }

    /// Flat `<head>`-window enumeration (mirrors `extract_sections_flat`):
    /// every `<head>` opens a window to the next `<head>` (or body end); the
    /// pre-first-head region belongs to no section. Raw paths are then
    /// resolved with the same collision algorithm as `uniquify_flat_paths` —
    /// windows without content (empty title AND text) are skipped exactly
    /// like the section pass.
    fn flat_section_windows(body: &str, out: &mut Vec<(usize, usize, Option<String>)>) {
        let mut raw: Vec<(usize, usize, String, bool)> = Vec::new();
        let mut pos = 0usize;
        while let Some(head_start) = body[pos..].find("<head") {
            let abs = pos + head_start;
            let tag_end = match body[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let open_tag = &body[abs..=tag_end];
            let path_component = Self::extract_head_n_string(open_tag);
            let content_start = tag_end + 1;
            let head_close = match body[content_start..].find("</head>") {
                Some(e) => content_start + e,
                None => break,
            };
            let title = Self::strip_xml_tags(&body[content_start..head_close]);
            let after_head = head_close + "</head>".len();
            let next_head = body[after_head..].find("<head").map(|e| after_head + e);
            let text_end = next_head.unwrap_or(body.len());
            let text = Self::strip_xml_tags(&body[after_head..text_end]);
            raw.push((
                after_head,
                text_end,
                path_component,
                !title.is_empty() || !text.is_empty(),
            ));
            pos = text_end;
            // Guard against infinite loop when text_end == abs (adjacent heads).
            if pos <= tag_end {
                pos = tag_end + 1;
            }
        }
        // Pre-first-head region belongs to no section.
        if let Some(first_head) = body.find("<head")
            && first_head > 0
        {
            out.push((0, first_head, None));
        }
        let resolved = Self::resolve_flat_window_paths(&raw);
        for ((start, end, _, _), path) in raw.into_iter().zip(resolved) {
            if start < end {
                out.push((start, end, path));
            }
        }
    }

    /// Resolve flat raw window paths with the same algorithm as
    /// `uniquify_flat_paths` (same iteration order, same content filter), so
    /// tables land on the exact section paths the section pass produced.
    fn resolve_flat_window_paths(raw: &[(usize, usize, String, bool)]) -> Vec<Option<String>> {
        let mut used = HashSet::new();
        let mut latest_resolved: HashMap<String, String> = HashMap::new();
        let mut out = Vec::with_capacity(raw.len());
        for (_, _, raw_path, has_content) in raw {
            if !has_content {
                // The section pass never creates a Section for this window,
                // so it consumes no path slot and tables inside it stay
                // not-section-resolved.
                out.push(None);
                continue;
            }
            let candidate = match raw_path.rsplit_once('.') {
                Some((raw_parent, component)) => latest_resolved
                    .get(raw_parent)
                    .map(|parent| format!("{parent}.{component}"))
                    .unwrap_or_else(|| raw_path.clone()),
                None => raw_path.clone(),
            };
            let mut resolved = candidate.clone();
            let mut collision = 2u32;
            while used.contains(&resolved) {
                resolved = format!("{candidate}~{collision}");
                collision += 1;
            }
            used.insert(resolved.clone());
            latest_resolved.insert(raw_path.clone(), resolved.clone());
            out.push(Some(resolved));
        }
        out
    }

    /// Build one `ExtractedTable` from a `<table ...>` element (TEI-absolute
    /// `abs_start`/`abs_end`). Section identity comes from the containing
    /// window (`windows` are body-relative); per-cell source spans are
    /// computed against the containing section's stripped text.
    fn build_table(
        tei: &str,
        open_tag: &str,
        content: &str,
        abs_start: usize,
        abs_end: usize,
        context: &SectionParseContext<'_>,
        index: usize,
    ) -> ExtractedTable {
        // Enclosing <figure type="table"> context: caption head + xml:id + coords.
        let figure_ctx = Self::enclosing_table_figure(tei, abs_start, abs_end);

        let table_id = figure_ctx
            .as_ref()
            .and_then(|(tag, _)| Self::extract_attr(tag, "xml:id"))
            .or_else(|| Self::extract_attr(open_tag, "xml:id"))
            .unwrap_or_else(|| format!("table-{index}"));

        let caption = figure_ctx
            .as_ref()
            .and_then(|(_, caption)| caption.clone())
            .unwrap_or_default();

        let coordinates = figure_ctx
            .as_ref()
            .and_then(|(tag, _)| Self::extract_attr(tag, "coords"))
            .or_else(|| Self::extract_attr(open_tag, "coords"));

        // Section window containing this table (body-relative coordinates).
        let table_rel = abs_start.saturating_sub(context.body_start);
        let window = context
            .windows
            .iter()
            .find(|(s, e, _)| *s <= table_rel && table_rel < *e);
        let section_path = window.and_then(|(_, _, path)| path.clone());
        let section_ordinal = section_path
            .as_ref()
            .and_then(|path| context.ordinal_by_path.get(path).copied());
        // Spans are only computable against a real section text window; a
        // None window (gap) has no section text to anchor against.
        let span_window = window.and_then(
            |(s, e, path)| {
                if path.is_some() { Some((*s, *e)) } else { None }
            },
        );

        let table_content_abs = abs_start + open_tag.len();
        let (cells, missingness) = Self::build_cells(
            content,
            table_content_abs,
            context.body,
            context.body_start,
            span_window,
        );
        let source_digest = Self::table_digest(&table_id, &caption, coordinates.as_deref(), &cells);

        ExtractedTable {
            table_id,
            section_path_id: section_path,
            section_ordinal,
            caption,
            coordinates,
            cells,
            source_digest,
            missingness,
        }
    }

    /// Parse `<row>`/`<cell>` structure of a `<table>` content fragment into
    /// deterministic `TableCell`s. Empty `<cell></cell>` entries are
    /// preserved (grid alignment for fidelity), `cols`/`rows` attributes
    /// become colspan/rowspan, and the column index is the cumulative grid
    /// position accounting for colspans of preceding cells. Returns typed
    /// missingness when the structure is unusable (fail-closed, never
    /// invented cells).
    fn build_cells(
        content: &str,
        table_content_tei_abs: usize,
        body: Option<&str>,
        body_start: usize,
        span_window: Option<(usize, usize)>,
    ) -> (Vec<TableCell>, Option<TableMissingReason>) {
        let mut cells = Vec::new();
        let mut pos = 0usize;
        let mut row = 0u32;
        let mut any_row = false;
        while let Some(rel) = content[pos..].find("<row") {
            let abs = pos + rel;
            if content[abs..].starts_with("</row") {
                pos = abs + 1;
                continue;
            }
            let tag_end = match content[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let close = match content[tag_end..].find("</row>") {
                Some(e) => tag_end + e,
                None => break,
            };
            any_row = true;
            let row_content = &content[tag_end + 1..close];
            let row_content_start_rel = tag_end + 1; // relative to `content`
            let mut col = 0u32;
            let mut cpos = 0usize;
            while let Some(crel) = row_content[cpos..].find("<cell") {
                let cabs = cpos + crel;
                if row_content[cabs..].starts_with("</cell") {
                    cpos = cabs + 1;
                    continue;
                }
                let ctag_end = match row_content[cabs..].find('>') {
                    Some(e) => cabs + e,
                    None => break,
                };
                let cclose = match row_content[ctag_end..].find("</cell>") {
                    Some(e) => ctag_end + e,
                    None => break,
                };
                let ctag = &row_content[cabs..=ctag_end];
                let cell_content = &row_content[ctag_end + 1..cclose];
                let colspan = Self::parse_span_attr(ctag, "cols").unwrap_or(1).max(1);
                let rowspan = Self::parse_span_attr(ctag, "rows").unwrap_or(1).max(1);
                let text = Self::strip_xml_tags(cell_content);
                let coords = Self::extract_attr(ctag, "coords");

                // Source span of the cell's visible content inside the
                // containing section's stripped text.
                let span = match span_window {
                    Some((w_start, w_end)) => body.and_then(|body| {
                        let cell_content_abs =
                            table_content_tei_abs + row_content_start_rel + (ctag_end + 1);
                        let cell_rel = cell_content_abs.saturating_sub(body_start);
                        if cell_rel < w_start || cell_rel >= w_end {
                            return None;
                        }
                        let start = Self::stripped_text_offset(body, w_start, cell_rel, w_end);
                        let end = start + Self::visible_char_count(cell_content) as u32;
                        Some(CharSpan { start, end })
                    }),
                    None => None,
                };

                let digest =
                    Self::cell_digest(row, col, colspan, rowspan, &text, coords.as_deref());
                cells.push(TableCell {
                    row,
                    column: col,
                    colspan,
                    rowspan,
                    text,
                    coordinates: coords,
                    span,
                    digest,
                });
                col += colspan;
                cpos = cclose + "</cell>".len();
            }
            row += 1;
            pos = close + "</row>".len();
        }

        let missingness = if cells.is_empty() {
            let reason = if any_row {
                "rows found but zero cells".to_string()
            } else {
                "zero rows".to_string()
            };
            Some(TableMissingReason::Malformed(reason))
        } else {
            None
        };
        (cells, missingness)
    }

    /// Find the enclosing `<figure type="table">` for a `<table>` element:
    /// returns `(figure_open_tag, Option<caption>)` where the caption is the
    /// stripped text of the figure's first `<head>` (e.g. "Table 1 :").
    fn enclosing_table_figure(
        tei: &str,
        abs_start: usize,
        abs_end: usize,
    ) -> Option<(String, Option<String>)> {
        let figure_start = tei[..abs_start].rfind("<figure")?;
        let tag_end = tei[figure_start..].find('>')? + figure_start;
        let open_tag = &tei[figure_start..=tag_end];
        if !(open_tag.contains("type=\"table\"") || open_tag.contains("type='table'")) {
            return None;
        }
        let fig_close = tei[tag_end..].find("</figure>")? + tag_end;
        if fig_close < abs_end {
            // The table is not inside this figure (closes after it).
            return None;
        }
        let fig_content = &tei[tag_end + 1..fig_close];
        let caption = if let Some(hs) = fig_content.find("<head") {
            let htag_end = fig_content[hs..].find('>').map(|e| hs + e)?;
            let content_start = htag_end + 1;
            if let Some(hc) = fig_content[content_start..].find("</head>") {
                let caption = Self::strip_xml_tags(&fig_content[content_start..content_start + hc]);
                if !caption.is_empty() {
                    Some(caption)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        Some((open_tag.to_string(), caption))
    }

    /// Extract an XML attribute `name="value"` (single or double quotes)
    /// from an opening tag. Matches on attribute boundaries so `id=` never
    /// matches inside `xml:id=` and `grid=` never matches inside `id=`.
    fn extract_attr(open_tag: &str, name: &str) -> Option<String> {
        for quote in ['"', '\''] {
            let needle = format!("{name}={quote}");
            let mut search_from = 0usize;
            while let Some(pos) = open_tag[search_from..].find(&needle) {
                let abs = search_from + pos;
                let boundary_ok = open_tag[..abs]
                    .chars()
                    .next_back()
                    .map(|c| c == ' ' || c == '\t' || c == '\n' || c == '>')
                    .unwrap_or(true);
                if boundary_ok {
                    let after = &open_tag[abs + needle.len()..];
                    if let Some(end) = after.find(quote) {
                        let value = after[..end].trim();
                        if !value.is_empty() {
                            return Some(value.to_string());
                        }
                    }
                }
                search_from = abs + 1;
            }
        }
        None
    }

    /// Parse a positive integer TEI span attribute (`cols`/`rows`); None
    /// when missing or unparseable (caller defaults to 1).
    fn parse_span_attr(open_tag: &str, name: &str) -> Option<u32> {
        Self::extract_attr(open_tag, name)?.trim().parse().ok()
    }

    /// Count visible (non-tag) chars of a raw TEI fragment, mirroring
    /// `strip_xml_tags` semantics (chars inside `<...>` are not counted).
    fn visible_char_count(s: &str) -> usize {
        let mut count = 0usize;
        let mut in_tag = false;
        for ch in s.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => count += 1,
                _ => {}
            }
        }
        count
    }

    /// Deterministic SHA-256 cell digest over (row, column, colspan,
    /// rowspan, text, coordinates). Location (span/section) is intentionally
    /// excluded so the digest is pure source-content evidence;
    /// materialization binds revision + section into the graph VID.
    fn cell_digest(
        row: u32,
        column: u32,
        colspan: u32,
        rowspan: u32,
        text: &str,
        coords: Option<&str>,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(row.to_le_bytes());
        hasher.update(column.to_le_bytes());
        hasher.update(colspan.to_le_bytes());
        hasher.update(rowspan.to_le_bytes());
        hasher.update(text.as_bytes());
        hasher.update(coords.unwrap_or("").as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Deterministic SHA-256 table digest over table_id + caption +
    /// coordinates + every cell digest (document order).
    fn table_digest(
        table_id: &str,
        caption: &str,
        coords: Option<&str>,
        cells: &[TableCell],
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(table_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(caption.as_bytes());
        hasher.update(b"\n");
        hasher.update(coords.unwrap_or("").as_bytes());
        hasher.update(b"\n");
        for cell in cells {
            hasher.update(cell.digest.as_bytes());
            hasher.update(b"\n");
        }
        hex::encode(hasher.finalize())
    }

    // ─── M400 S03 T01: deterministic TEI formula extraction ───

    /// Extract deterministic TEI formula structures (M400 S03 T01).
    ///
    /// GROBID emits formulas as `<formula xml:id="formula_0">…<label>(1)</label>
    /// </formula>` inline inside `<p>` elements within the `<body>` (coordinates
    /// requested via `teiCoordinates=["figure","table","note","formula"]`).
    /// Section identity mirrors `extract_sections` exactly — the same section
    /// windows shared with table extraction (`section_windows`), so a formula
    /// maps to the same `path_id` the section extractor produced for its
    /// containing text window. Formulas outside every section window (top-level
    /// gaps / trailing region) get `section_path_id: None` — typed missingness,
    /// never guessed into a neighbour.
    ///
    /// Returns the extracted formulas plus the article-level typed missingness:
    /// `Some(NoFormulaElements)` when the TEI contains no `<formula>` element at
    /// all (R087 — an empty Vec alone cannot distinguish that from a parser
    /// that did not populate formulas).
    pub(crate) fn extract_formulas(
        tei: &str,
    ) -> (Vec<ExtractedFormula>, Option<FormulaMissingReason>) {
        let body = Self::tei_body(tei);
        let (windows, body_start) = match body {
            Some(b) => {
                let body_start = b.as_ptr() as usize - tei.as_ptr() as usize;
                (Self::section_windows(b), body_start)
            }
            None => (Vec::new(), 0),
        };

        // 0-based ordinal of each section path (section titles repeat, so
        // formulas need the ordinal to disambiguate — mirrors
        // ExtractedEntity.section_ordinal, MEM685).
        let sections = Self::extract_sections(tei);
        let mut ordinal_by_path: HashMap<String, usize> = HashMap::new();
        for (idx, section) in sections.iter().enumerate() {
            if let Some(path) = section.path_id.as_deref() {
                ordinal_by_path.entry(path.to_string()).or_insert(idx);
            }
        }

        let mut formulas = Vec::new();
        let mut formula_count = 0usize;
        let mut pos = 0usize;
        while let Some(rel) = tei[pos..].find("<formula") {
            let abs = pos + rel;
            // Skip closing tags and non-element matches.
            if tei[abs..].starts_with("</formula") {
                pos = abs + 1;
                continue;
            }
            let tag_end = match tei[abs..].find('>') {
                Some(e) => abs + e,
                None => break,
            };
            let close = match tei[tag_end..].find("</formula>") {
                Some(e) => tag_end + e,
                None => break,
            };
            formula_count += 1;
            let open_tag = &tei[abs..=tag_end];
            let content = &tei[tag_end + 1..close];
            let formula = Self::build_formula(
                body,
                body_start,
                &windows,
                &ordinal_by_path,
                open_tag,
                content,
                abs,
                formula_count,
            );
            formulas.push(formula);
            pos = close + "</formula>".len();
        }

        let missingness = if formula_count == 0 {
            Some(FormulaMissingReason::NoFormulaElements)
        } else {
            None
        };
        (formulas, missingness)
    }

    /// Build one `ExtractedFormula` from a `<formula ...>…</formula>` element
    /// (TEI-absolute `abs_start`). Section identity comes from the containing
    /// window (`windows` are body-relative); the source span is computed
    /// against the containing section's stripped text.
    #[allow(clippy::too_many_arguments)]
    fn build_formula(
        body: Option<&str>,
        body_start: usize,
        windows: &[(usize, usize, Option<String>)],
        ordinal_by_path: &HashMap<String, usize>,
        open_tag: &str,
        content: &str,
        abs_start: usize,
        index: usize,
    ) -> ExtractedFormula {
        let formula_id =
            Self::extract_attr(open_tag, "xml:id").unwrap_or_else(|| format!("formula-{index}"));

        let label = Self::extract_formula_label(content);

        let coordinates = Self::extract_attr(open_tag, "coords");

        let (representation, missingness) = Self::formula_representation(content);

        // Section window containing this formula (body-relative coordinates).
        let formula_rel = abs_start.saturating_sub(body_start);
        let window = windows
            .iter()
            .find(|(s, e, _)| *s <= formula_rel && formula_rel < *e);
        let section_path = window.and_then(|(_, _, path)| path.clone());
        let section_ordinal = section_path
            .as_ref()
            .and_then(|path| ordinal_by_path.get(path).copied());
        // Spans are only computable against a real section text window; a
        // None window (gap) has no section text to anchor against.
        let span_window = window.and_then(
            |(s, e, path)| {
                if path.is_some() { Some((*s, *e)) } else { None }
            },
        );

        let span = match span_window {
            Some((w_start, w_end)) => body.and_then(|body| {
                let content_abs = abs_start + open_tag.len();
                let content_rel = content_abs.saturating_sub(body_start);
                if content_rel < w_start || content_rel >= w_end {
                    return None;
                }
                let start = Self::stripped_text_offset(body, w_start, content_rel, w_end);
                let end = start + Self::visible_char_count(content) as u32;
                Some(CharSpan { start, end })
            }),
            None => None,
        };

        let source_digest = Self::formula_digest(
            &formula_id,
            label.as_deref(),
            coordinates.as_deref(),
            &representation,
        );

        ExtractedFormula {
            formula_id,
            section_path_id: section_path,
            section_ordinal,
            label,
            coordinates,
            representation,
            span,
            source_digest,
            missingness,
        }
    }

    /// Extract the equation number/label from the formula's `<label>` child
    /// (e.g. "(1)"), if any. GROBID emits the label after the formula's text
    /// content; other TEI producers may place it before.
    fn extract_formula_label(content: &str) -> Option<String> {
        let ls = content.find("<label")?;
        let tag_end = content[ls..].find('>').map(|e| ls + e)?;
        let inner_start = tag_end + 1;
        let close_rel = content[inner_start..].find("</label>")?;
        let label = Self::strip_xml_tags(&content[inner_start..inner_start + close_rel]);
        if label.is_empty() { None } else { Some(label) }
    }

    /// Determine the formula's representation (M400 S03 T01): the raw MathML
    /// subtree when a `<math>` element is present, the LaTeX source from a
    /// `<texMath>` element (other TEI producers), else the stripped visible
    /// text (GROBID's PDF-layout approximation when math parsing is
    /// unavailable or disabled). Exactly one variant is populated. Fail-closed:
    /// a `<formula>` whose content yields no representation produces
    /// `PlainText("")` plus typed missingness `Malformed` — never an invented
    /// representation.
    fn formula_representation(
        content: &str,
    ) -> (FormulaRepresentation, Option<FormulaMissingReason>) {
        // Raw MathML subtree (tags included), e.g.
        // `<math xmlns="…"><mi>x</mi></math>`.
        if let Some(math_start) = content.find("<math") {
            let after = &content[math_start..];
            return match after.find("</math>") {
                Some(close_rel) => (
                    FormulaRepresentation::MathML(
                        content[math_start..math_start + close_rel + "</math>".len()].to_string(),
                    ),
                    None,
                ),
                None => (
                    FormulaRepresentation::PlainText(String::new()),
                    Some(FormulaMissingReason::Malformed(
                        "math element without closing tag".to_string(),
                    )),
                ),
            };
        }
        // `<texMath>…</texMath>` (other TEI producers): the LaTeX source.
        if let Some(tex_start) = content.find("<texMath") {
            let after = &content[tex_start..];
            if let Some(close_rel) = after.find("</texMath>") {
                let raw = &content[tex_start..tex_start + close_rel];
                let source = Self::strip_xml_tags(raw);
                if !source.is_empty() {
                    return (FormulaRepresentation::TeXMath(source), None);
                }
                return (
                    FormulaRepresentation::PlainText(String::new()),
                    Some(FormulaMissingReason::Malformed(
                        "empty texMath content".to_string(),
                    )),
                );
            }
            return (
                FormulaRepresentation::PlainText(String::new()),
                Some(FormulaMissingReason::Malformed(
                    "texMath element without closing tag".to_string(),
                )),
            );
        }
        // GROBID PDF-layout approximation (no math extraction) or plain text.
        let text = Self::strip_xml_tags(content);
        if text.is_empty() {
            (
                FormulaRepresentation::PlainText(String::new()),
                Some(FormulaMissingReason::Malformed(
                    "empty formula content".to_string(),
                )),
            )
        } else {
            (FormulaRepresentation::PlainText(text), None)
        }
    }

    /// Deterministic SHA-256 formula digest over formula_id + label +
    /// coordinates + representation kind + representation content. Location
    /// (span/section) is intentionally excluded so the digest is pure
    /// source-content evidence; materialization binds revision + section into
    /// the graph VID (R087).
    fn formula_digest(
        formula_id: &str,
        label: Option<&str>,
        coords: Option<&str>,
        representation: &FormulaRepresentation,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(formula_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(label.unwrap_or("").as_bytes());
        hasher.update(b"\n");
        hasher.update(coords.unwrap_or("").as_bytes());
        hasher.update(b"\n");
        let kind = match representation {
            FormulaRepresentation::MathML(_) => "math_ml",
            FormulaRepresentation::TeXMath(_) => "tex_math",
            FormulaRepresentation::PlainText(_) => "plain_text",
        };
        hasher.update(kind.as_bytes());
        hasher.update(b"\n");
        hasher.update(representation.as_str().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Extract the analytic title from a biblStruct entry.
    fn extract_citation_title(entry: &str) -> Option<String> {
        let analytic = entry
            .find("<analytic>")
            .and_then(|s| entry[s..].find("</analytic>").map(|e| &entry[s..s + e]))?;
        if let Some(t_start) = analytic.find("<title")
            && let Some(tag_end) = analytic[t_start..].find('>')
        {
            let content = &analytic[t_start + tag_end + 1..];
            if let Some(end) = content.find("</title>") {
                let title = Self::strip_xml_tags(&content[..end]);
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
        None
    }

    /// Extract an <idno type="...">value</idno> from a TEI fragment.
    fn extract_idno(entry: &str, idno_type: &str) -> Option<String> {
        let needle = format!("type=\"{}\"", idno_type);
        let type_pos = entry.find(&needle)?;
        // Find the <idno ...> opening tag containing this attribute
        let tag_start = entry[..type_pos].rfind("<idno")?;
        let tag_end = entry[tag_start..].find('>')? + tag_start;
        let content = &entry[tag_end + 1..];
        let close = content.find("</idno>")?;
        let value = Self::strip_xml_tags(&content[..close]);
        if value.is_empty() { None } else { Some(value) }
    }

    /// Strip XML tags from text.
    pub(crate) fn strip_xml_tags(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_tag = false;
        for ch in s.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }
        result.trim().to_string()
    }

    /// Extract SHA256 hash of PDF bytes.
    pub(crate) fn pdf_hash(pdf_bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pdf_bytes);
        hex::encode(hasher.finalize())
    }
}
