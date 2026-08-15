//! GROBID parser adapter — implements ParserPort.
//!
//! ADR-037 §4.1: GROBID HTTP service for scholarly PDF parsing.
//! GROBID runs at http://127.0.0.1:8070 (Java service, no API key).

use async_trait::async_trait;
use da_ports::parser::{ParseResult, ParsedArticle, ParserError, ParserPort};
use std::collections::{HashMap, HashSet};

const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrobidProfile {
    pub consolidate_citations: bool,
    pub consolidate_header: bool,
}

impl GrobidProfile {
    /// Coordinate groups always requested by this adapter.
    pub const TEI_COORDINATES: &'static str = "figure,table,note";

    pub const fn rich() -> Self {
        Self {
            consolidate_citations: true,
            consolidate_header: true,
        }
    }

    pub const fn structural() -> Self {
        Self {
            consolidate_citations: false,
            consolidate_header: false,
        }
    }

    /// Stable semantic name for the effective consolidation policy.
    /// Mixed policies are intentionally reported as `custom`; they remain
    /// valid but must never reuse a named profile's cache identity.
    pub fn name(self) -> &'static str {
        match (self.consolidate_header, self.consolidate_citations) {
            (true, true) => "rich",
            (false, false) => "structural",
            _ => "custom",
        }
    }

    /// Exact effective-input identity for cache keys. HTTP timeout is
    /// deliberately excluded: it does not alter successful TEI semantics.
    pub fn configuration_fingerprint(self) -> String {
        format!(
            "profile={};consolidateHeader={};consolidateCitations={};teiCoordinates={}",
            self.name(),
            u8::from(self.consolidate_header),
            u8::from(self.consolidate_citations),
            Self::TEI_COORDINATES
        )
    }
}

/// GROBID HTTP adapter for PDF → scholarly structure (TEI XML).
#[derive(Clone)]
pub struct GrobidParser {
    url: String,
    client: reqwest::Client,
    profile: GrobidProfile,
}

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

impl GrobidParser {
    pub fn new(url: Option<&str>) -> Self {
        Self {
            url: url.unwrap_or(DEFAULT_GROBID_URL).to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("reqwest client"),
            profile: GrobidProfile::rich(),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("GROBID_URL").unwrap_or_else(|_| DEFAULT_GROBID_URL.to_string());
        Self::new(Some(&url))
    }

    /// Override the bounded HTTP request timeout for batch profiles.
    ///
    /// Full citation/header consolidation can legitimately exceed the 120s
    /// interactive default while Crossref applies rate-limit backoff. The
    /// caller should record this operational policy in its run report. It does
    /// not alter successful TEI semantics and therefore is not a content-key input.
    pub fn with_profile(mut self, profile: GrobidProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn profile(&self) -> GrobidProfile {
        self.profile
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client");
        self
    }

    /// Check if GROBID is alive.
    pub async fn is_alive(&self) -> bool {
        let url = format!("{}/api/isalive", self.url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Process PDF via GROBID full-text endpoint → TEI XML.
    ///
    /// Sends consolidation + coordinate parameters for richer extraction:
    /// - `consolidateCitations=1`: enrich references via Crossref/biblio-glutton
    ///   (adds DOI, venue, publisher, normalized author names). Critical for
    ///   ADR-053 Layer F cross-paper edges (predecessor_of, superseded_by).
    /// - `consolidateHeader=1`: enrich paper header (title, authors, DOI, abstract)
    ///   via Crossref. Improves author disambiguation (Layer G) and DOI coverage.
    /// - `teiCoordinates=["figure","table","note"]`: add PDF coordinates for
    ///   figures, tables, and footnotes. Enables Layer I figure image cropping
    ///   and provenance tracking.
    ///
    /// These parameters are GROBID 0.9.0 REST API options (see GROBID docs).
    /// They add ~2-5s per paper for Crossref lookups; acceptable for batch ingest.
    async fn process_fulltext(&self, pdf_bytes: &[u8]) -> ParseResult<String> {
        let url = format!("{}/api/processFulltextDocument", self.url);
        let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name("paper.pdf")
            .mime_str("application/pdf")
            .map_err(|e| ParserError::ParseFailed(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("input", part)
            .text(
                "consolidateCitations",
                if self.profile.consolidate_citations {
                    "1"
                } else {
                    "0"
                },
            )
            .text(
                "consolidateHeader",
                if self.profile.consolidate_header {
                    "1"
                } else {
                    "0"
                },
            )
            .text("teiCoordinates", "[\"figure\",\"table\",\"note\"]");

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(
                    api = "grobid",
                    endpoint = %url,
                    error = %e,
                    "GROBID processFulltextDocument network error"
                );
                ParserError::Unavailable(e.to_string())
            })?;

        let status = resp.status();
        if !status.is_success() {
            tracing::warn!(
                api = "grobid",
                endpoint = %url,
                status = %status,
                "GROBID processFulltextDocument non-success HTTP status"
            );
            return Err(ParserError::ParseFailed(format!("GROBID HTTP {status}")));
        }

        let tei_xml = resp.text().await.map_err(|e| {
            tracing::warn!(
                api = "grobid",
                error = %e,
                "GROBID processFulltextDocument: failed to read response body"
            );
            ParserError::ParseFailed(e.to_string())
        })?;

        tracing::debug!(
            api = "grobid",
            endpoint = %url,
            tei_size = tei_xml.len(),
            "GROBID processFulltextDocument: TEI XML received"
        );

        Ok(tei_xml)
    }

    /// Extract title from TEI XML.
    /// Handles both `<title>Text</title>` and `<title level="a" type="main">Text</title>`.
    fn extract_title(tei: &str) -> String {
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
    fn extract_abstract(tei: &str) -> String {
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
    fn extract_body(tei: &str) -> String {
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

    /// Extract sections from TEI body.
    /// GROBID structures body as nested `<div><head n="1">Title</head><p>...</p></div>`.
    /// Level is derived from the `n` attribute on `<head>` (1, 2, 3...).
    fn extract_sections(tei: &str) -> Vec<da_ports::parser::Section> {
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
    fn extract_head_level(open_tag: &str) -> u32 {
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
    fn extract_citations(tei: &str) -> Vec<da_ports::parser::CitationEntry> {
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
    fn strip_xml_tags(s: &str) -> String {
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
    fn pdf_hash(pdf_bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pdf_bytes);
        hex::encode(hasher.finalize())
    }
}

#[async_trait]
impl ParserPort for GrobidParser {
    #[tracing::instrument(skip(self, pdf_path), fields(paper_id = %paper_id), name = "grobid_parse")]
    async fn parse_pdf(&self, pdf_path: &str, paper_id: &str) -> ParseResult<ParsedArticle> {
        tracing::info!(paper_id, pdf_path, "GROBID parsing");

        let pdf_bytes = tokio::fs::read(pdf_path)
            .await
            .map_err(|e| ParserError::ParseFailed(format!("read PDF: {e}")))?;

        let pdf_size = pdf_bytes.len();
        let pdf_hash = Self::pdf_hash(&pdf_bytes);
        tracing::debug!(
            paper_id,
            pdf_size,
            pdf_hash = %pdf_hash,
            "PDF read, sending to GROBID"
        );

        let request_start = std::time::Instant::now();
        let tei_xml = self.process_fulltext(&pdf_bytes).await?;
        let grobid_elapsed_ms = request_start.elapsed().as_millis() as u64;
        let tei_size = tei_xml.len();
        tracing::info!(
            paper_id,
            api = "grobid",
            elapsed_ms = grobid_elapsed_ms,
            pdf_size,
            tei_size,
            "GROBID processFulltextDocument response received"
        );

        let title = Self::extract_title(&tei_xml);
        let abstract_text = Self::extract_abstract(&tei_xml);
        let body_text = Self::extract_body(&tei_xml);
        let sections = Self::extract_sections(&tei_xml);
        let citations = Self::extract_citations(&tei_xml);

        let sections_with_path_id = sections.iter().filter(|s| s.path_id.is_some()).count();
        let max_depth = sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .map(|p| p.split('.').count())
            .max()
            .unwrap_or(0);

        tracing::info!(
            paper_id,
            title = %title,
            body_chars = body_text.len(),
            abstract_chars = abstract_text.len(),
            sections = sections.len(),
            sections_with_path_id,
            max_tree_depth = max_depth,
            citations = citations.len(),
            pdf_hash = %pdf_hash,
            "GROBID parsed: extraction complete"
        );

        if sections_with_path_id == 0 && !sections.is_empty() {
            tracing::warn!(
                paper_id,
                "GROBID TEI: no sections have path_id — flat structure without <head n=...> attributes"
            );
        }

        Ok(ParsedArticle {
            paper_id: paper_id.to_string(),
            title,
            abstract_text,
            body_text,
            sections,
            citations,
            layout_json: None, // ODL provides this separately
            tei_xml: Some(tei_xml),
            pdf_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_profile_disables_remote_consolidation() {
        let parser = GrobidParser::new(None).with_profile(GrobidProfile::structural());
        assert_eq!(parser.profile(), GrobidProfile::structural());
        assert!(!parser.profile().consolidate_header);
        assert!(!parser.profile().consolidate_citations);
    }

    #[test]
    fn profile_fingerprints_bind_semantics_and_exclude_timeout() {
        assert_eq!(GrobidProfile::rich().name(), "rich");
        assert_eq!(
            GrobidProfile::rich().configuration_fingerprint(),
            "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note"
        );
        assert_eq!(
            GrobidProfile::structural().configuration_fingerprint(),
            "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note"
        );
        assert_ne!(
            GrobidProfile::rich().configuration_fingerprint(),
            GrobidProfile::structural().configuration_fingerprint()
        );
        let mut custom = GrobidProfile::structural();
        custom.consolidate_header = true;
        assert_eq!(custom.name(), "custom");
        assert!(
            custom
                .configuration_fingerprint()
                .contains("profile=custom")
        );
        let with_timeout = GrobidParser::new(None)
            .with_timeout(std::time::Duration::from_secs(1))
            .profile();
        assert_eq!(
            with_timeout.configuration_fingerprint(),
            GrobidProfile::rich().configuration_fingerprint(),
            "timeout is operational policy, not a content-key input"
        );
    }

    #[test]
    fn test_extract_title() {
        let tei = r#"<TEI><teiHeader><fileDesc><titleStmt><title>Seq2Seq Models</title></titleStmt></fileDesc></teiHeader></TEI>"#;
        assert_eq!(GrobidParser::extract_title(tei), "Seq2Seq Models");
    }

    #[test]
    fn test_extract_title_with_attributes() {
        // GROBID TEI uses attributes: <title level="a" type="main">
        let tei =
            r#"<title level="a" type="main">A Joint Model of Language and Perception</title>"#;
        assert_eq!(
            GrobidParser::extract_title(tei),
            "A Joint Model of Language and Perception"
        );
    }

    #[test]
    fn test_strip_xml_tags() {
        let s = "<p>Hello <ref>world</ref></p>";
        assert_eq!(GrobidParser::strip_xml_tags(s), "Hello world");
    }

    #[test]
    fn test_pdf_hash() {
        let bytes = b"test pdf content";
        let hash = GrobidParser::pdf_hash(bytes);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_extract_sections() {
        let tei = r#"<TEI><text><body>
            <div><head n="1">INTRODUCTION</head><p>Intro text here.</p></div>
            <div><head n="2">Methods</head><p>Method details.</p></div>
        </body></text></TEI>"#;
        let sections = GrobidParser::extract_sections(tei);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "INTRODUCTION");
        assert_eq!(sections[0].level, 1);
        assert!(sections[0].text.contains("Intro text"));
        assert_eq!(sections[1].title, "Methods");
        assert_eq!(sections[1].level, 2);
    }

    #[test]
    fn test_extract_sections_no_body() {
        let tei = r#"<TEI><teiHeader><title>No body</title></teiHeader></TEI>"#;
        let sections = GrobidParser::extract_sections(tei);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_extract_citations() {
        let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct><analytic><title level="a" type="main">First Citation</title></analytic>
            <idno type="DOI">10.1000/cite.1</idno></biblStruct>
            <biblStruct><analytic><title level="a" type="main">Second Citation</title></analytic>
            <idno type="arXiv">arXiv:2001.00002v1</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].title.as_deref(), Some("First Citation"));
        assert_eq!(citations[0].doi.as_deref(), Some("10.1000/cite.1"));
        assert_eq!(citations[1].title.as_deref(), Some("Second Citation"));
        assert_eq!(citations[1].arxiv_id.as_deref(), Some("2001.00002"));
    }

    #[test]
    fn test_extract_citations_no_back() {
        let tei = r#"<TEI><text><body><p>no refs</p></body></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert!(citations.is_empty());
    }

    #[test]
    fn test_extract_citations_with_attributes() {
        // Real GROBID output: <biblStruct status="extracted" xml:id="b0">
        let tei = r#"<TEI><text><back><div type="references"><listBibl>
            <biblStruct status="extracted" xml:id="b0"><analytic>
            <title level="a" type="main">Cited With Attrs</title></analytic>
            <idno type="DOI">10.2000/x</idno></biblStruct>
        </listBibl></div></back></text></TEI>"#;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].title.as_deref(), Some("Cited With Attrs"));
        assert_eq!(citations[0].doi.as_deref(), Some("10.2000/x"));
        // No in-text refs in this TEI → anchors stay empty.
        assert!(citations[0].anchors.is_empty());
    }

    // ─── T03 (M398-cz4pzx/S03): in-text citation anchors ───

    /// Flat TEI: `<head n="1">` window contains a `<ref type="bibr">`
    /// targeting a back-matter biblStruct xml:id. The anchor must carry the
    /// resolved section path "1" and the char offset of the quote in the
    /// section's stripped text ("Alpha [1] Beta" → offset 6).
    #[test]
    fn test_extract_citations_in_text_anchors_flat() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>Alpha <ref type="bibr" target="#b0">[1]</ref> Beta</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic><idno type="DOI">10.1000/cite.1</idno></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 1, "one in-text anchor attached");
        let anchor = &citations[0].anchors[0];
        assert_eq!(anchor.ref_id, "b0");
        assert_eq!(anchor.section_path_id.as_deref(), Some("1"));
        assert_eq!(anchor.quote, "[1]");
        assert_eq!(
            anchor.offset,
            Some(6),
            "offset of '[1]' in 'Alpha [1] Beta'"
        );
    }

    /// Nested TEI: the same citation referenced in two sections keeps two
    /// anchors with distinct section paths and offsets (multi-span, D249).
    #[test]
    fn test_extract_citations_anchors_nested_div_multi_span() {
        let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha <ref type="bibr" target="#b1">(Jones, 2020)</ref></p><div n="1.1"><head>Setup</head><p>See <ref type="bibr" target="#b1">[1]</ref></p></div></div></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
<biblStruct xml:id="b1"><analytic><title level="a" type="main">Jones 2020</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 2);
        assert!(citations[0].anchors.is_empty(), "b0 has no in-text refs");
        let anchors = &citations[1].anchors;
        assert_eq!(anchors.len(), 2, "Jones cited in sections 1 and 1.1");
        assert_eq!(anchors[0].ref_id, "b1");
        assert_eq!(anchors[0].section_path_id.as_deref(), Some("1"));
        assert_eq!(anchors[0].quote, "(Jones, 2020)");
        assert_eq!(anchors[0].offset, Some(6));
        assert_eq!(anchors[1].section_path_id.as_deref(), Some("1.1"));
        assert_eq!(anchors[1].quote, "[1]");
        assert_eq!(anchors[1].offset, Some(4), "offset of '[1]' in 'See [1]'");
    }

    /// No in-text refs in the body → every citation stays anchor-less
    /// (ingest then records typed missingness no_in_text_anchor).
    #[test]
    fn test_extract_citations_no_in_text_refs_anchors_empty() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>plain text</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert!(citations[0].anchors.is_empty());
    }

    /// A `<ref type="bibr">` in the body before the first `<head>` is not
    /// inside any section text window → anchor attaches with section_path_id
    /// None (typed missingness anchor_section_path_unresolved at ingest).
    #[test]
    fn test_extract_citations_ref_outside_section_path_unresolved() {
        let tei = r##"<TEI><text><body><p>Preamble <ref type="bibr" target="#b0">[1]</ref></p><head n="1">Intro</head><p>Body</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 1);
        let anchor = &citations[0].anchors[0];
        assert_eq!(anchor.ref_id, "b0");
        assert!(
            anchor.section_path_id.is_none(),
            "no section window → None path"
        );
        assert!(anchor.offset.is_none(), "no section text → no offset");
    }

    /// Refs without a `target` attribute (GROBID emits them for merged ref
    /// groups) cannot be mapped to a bibliography entry and are dropped;
    /// targeted refs in the same window still attach.
    #[test]
    fn test_extract_citations_ref_without_target_skipped() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>A <ref type="bibr">(no target)</ref> B <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(
            citations[0].anchors.len(),
            1,
            "only the targeted ref attaches"
        );
        assert_eq!(citations[0].anchors[0].quote, "[1]");
    }

    /// Non-bibliographic refs (`type="note"`) are not citation anchors.
    #[test]
    fn test_extract_citations_ignores_non_bibr_refs() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="note" target="#n1">footnote</ref> and <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 1);
        assert_eq!(citations[0].anchors[0].quote, "[1]");
    }

    /// biblStruct without xml:id falls back to its ordinal key ("b{index}")
    /// so synthetic/hand-written TEI with ordinal targets still resolves.
    #[test]
    fn test_extract_citations_ordinal_fallback_without_xml_id() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="bibr" target="#b0">[1]</ref></p></body><back><div type="references"><listBibl>
<biblStruct><analytic><title level="a" type="main">NoId</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 1);
        assert_eq!(citations[0].anchors[0].ref_id, "b0");
    }

    /// Flat TEI with a numbering restart (two `<head n="1">`, e.g. an
    /// appendix restarting section numbers): anchors from the second window
    /// must resolve to the same uniquified path the section pass produces
    /// ("1~2"), never the first window's "1" (regression: a global per-path
    /// cache collapsed both windows onto the first window's path, so the
    /// appendix anchors pointed into the INTRODUCTION section text).
    #[test]
    fn test_extract_citations_flat_numbering_restart_paths() {
        let tei = r##"<TEI><text><body><head n="1">Intro</head><p>See <ref type="bibr" target="#b0">[1]</ref></p><head n="2">Methods</head><p>X</p><head n="1">Appendix</head><p>See <ref type="bibr" target="#b0">[1]</ref> again</p></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let sections = GrobidParser::extract_sections(tei);
        let citations = GrobidParser::extract_citations(tei);
        // Section paths: "1", "2", then the appendix restart becomes "1~2".
        let paths: Vec<&str> = sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .collect();
        assert_eq!(paths, vec!["1", "2", "1~2"]);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 2);
        let p0 = citations[0].anchors[0].section_path_id.clone();
        let p1 = citations[0].anchors[1].section_path_id.clone();
        assert_eq!(p0.as_deref(), Some("1"), "first window keeps path 1");
        assert_eq!(
            p1.as_deref(),
            Some("1~2"),
            "restarted window resolves to the section's uniquified path"
        );
        // Offsets must point at the quote inside each section's own text.
        let s1 = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1"))
            .expect("section 1");
        let sa = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1~2"))
            .expect("section 1~2");
        let a0 = &citations[0].anchors[0];
        let a1 = &citations[0].anchors[1];
        let off0 = a0.offset.expect("anchor 0 offset");
        let off1 = a1.offset.expect("anchor 1 offset");
        assert_eq!(
            &s1.text[off0 as usize..off0 as usize + a0.quote.len()],
            a0.quote,
            "anchor 0 offset must land on its quote in section 1"
        );
        assert_eq!(
            &sa.text[off1 as usize..off1 as usize + a1.quote.len()],
            a1.quote,
            "anchor 1 offset must land on its quote in section 1~2"
        );
    }

    /// Anchors in a top-level gap (between sibling divs) of a NESTED body are
    /// attached with section_path_id None rather than being dropped: the gap
    /// belongs to no section text window.
    #[test]
    fn test_extract_citations_anchor_in_top_level_gap_path_unresolved() {
        // div n="1" contains a nested child so the body counts as nested; the
        // ref sits between div 1's close and div 2's open — outside any window.
        let tei = r##"<TEI><text><body><div n="1"><head>Intro</head><p>Alpha</p><div n="1.1"><head>Sub</head><p>x</p></div></div><p>Between <ref type="bibr" target="#b0">[1]</ref></p><div n="2"><head>Methods</head><p>Beta</p></div></body><back><div type="references"><listBibl>
<biblStruct xml:id="b0"><analytic><title level="a" type="main">First</title></analytic></biblStruct>
</listBibl></div></back></text></TEI>"##;
        let citations = GrobidParser::extract_citations(tei);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].anchors.len(), 1);
        assert!(
            citations[0].anchors[0].section_path_id.is_none(),
            "inter-div gap belongs to no section → None path"
        );
        assert!(citations[0].anchors[0].offset.is_none());
    }

    #[test]
    fn test_extract_head_level() {
        assert_eq!(GrobidParser::extract_head_level(r#"<head n="3">"#), 3);
        assert_eq!(GrobidParser::extract_head_level("<head>"), 1);
        assert_eq!(GrobidParser::extract_head_level(r#"<head n="abc">"#), 1);
    }

    // ─── ADR-053 Layer A T02: recursive TEI div walk for path_id ───

    /// Fixture: minimal TEI body with nested `<div>` elements matching GROBID
    /// output structure. `<div n="N">` gives the section number; nesting gives
    /// the tree. Text content is in `<p>` children of each div.
    const NESTED_DIV_TEI: &str = r#"<body>
      <div n="1"><head>Introduction</head><p>Intro text.</p>
        <div n="1.1"><head>Background</head><p>Background text.</p></div>
        <div n="1.2"><head>Motivation</head><p>Motivation text.</p></div>
      </div>
      <div n="2"><head>Methods</head><p>Methods overview.</p>
        <div n="2.1"><head>Model</head><p>Model text.</p>
          <div n="2.1.1"><head>Encoder</head><p>Encoder text.</p></div>
        </div>
        <div n="2.2"><head>Training</head><p>Training text.</p></div>
      </div>
      <div n="3"><head>Results</head><p>Results text.</p></div>
    </body>"#;

    #[test]
    fn test_extract_sections_nested_div_path_id_populated() {
        let sections = GrobidParser::extract_sections(NESTED_DIV_TEI);
        let path_ids: Vec<&str> = sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .collect();
        assert!(
            path_ids.contains(&"1"),
            "missing root 1, got: {:?}",
            path_ids
        );
        assert!(path_ids.contains(&"1.1"));
        assert!(path_ids.contains(&"1.2"));
        assert!(path_ids.contains(&"2"));
        assert!(path_ids.contains(&"2.1"));
        assert!(path_ids.contains(&"2.1.1"));
        assert!(path_ids.contains(&"2.2"));
        assert!(path_ids.contains(&"3"));
    }

    #[test]
    fn test_duplicate_div_numbers_receive_deterministic_unique_paths() {
        let tei = r#"<body>
          <div n="1"><head>Introduction</head><p>Intro.</p></div>
          <div n="1"><head>Appendix A</head><p>First appendix.</p>
            <div n="1.1"><head>Details</head><p>Details.</p></div>
          </div>
          <div n="1"><head>Appendix B</head><p>Second appendix.</p></div>
        </body>"#;
        let sections = GrobidParser::extract_sections(tei);
        let paths: Vec<_> = sections
            .iter()
            .map(|section| section.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "1~2", "1~2.1", "1~3"]);
        let unique: HashSet<_> = paths.iter().copied().collect();
        assert_eq!(unique.len(), paths.len());
        assert_eq!(sections[2].parent_section_id.as_deref(), Some("sec-1~2"));
    }

    #[test]
    fn test_extract_sections_nested_div_parent_section_id() {
        let sections = GrobidParser::extract_sections(NESTED_DIV_TEI);
        // Section 2.1.1 parent should be section 2.1
        let encoder = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("2.1.1"))
            .expect("section 2.1.1 present");
        assert_eq!(
            encoder.parent_section_id.as_deref(),
            Some("sec-2.1"),
            "parent_section_id must point at the parent div's section_id"
        );
        // Root sections have no parent.
        let intro = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1"))
            .expect("section 1 present");
        assert!(
            intro.parent_section_id.is_none(),
            "root section has no parent"
        );
    }

    #[test]
    fn test_extract_sections_nested_div_order_in_parent() {
        let sections = GrobidParser::extract_sections(NESTED_DIV_TEI);
        // Section 1.1 has order 0, section 1.2 has order 1 in parent 1.
        let bg = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1.1"))
            .expect("section 1.1 present");
        assert_eq!(bg.order_in_parent, Some(0));
        let motiv = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1.2"))
            .expect("section 1.2 present");
        assert_eq!(motiv.order_in_parent, Some(1));
        // Training is the second child of Methods (after Model).
        let training = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("2.2"))
            .expect("section 2.2 present");
        assert_eq!(training.order_in_parent, Some(1));
    }

    #[test]
    fn test_extract_sections_nested_div_section_id_stable() {
        let sections = GrobidParser::extract_sections(NESTED_DIV_TEI);
        // Every section must have a section_id of form "sec-<path_id>".
        for s in &sections {
            let sid = s
                .section_id
                .as_ref()
                .expect("section_id populated for nested TEI");
            let pid = s
                .path_id
                .as_ref()
                .expect("path_id populated for nested TEI");
            assert!(sid.starts_with("sec-"), "section_id should start with sec-");
            assert!(sid.ends_with(pid), "section_id should end with path_id");
        }
    }

    #[test]
    fn test_extract_sections_nested_div_depth_matches_level() {
        let sections = GrobidParser::extract_sections(NESTED_DIV_TEI);
        // depth (0-based) for section 2.1.1 is 2; level (1-based) is 3.
        let encoder = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("2.1.1"))
            .expect("section 2.1.1 present");
        // level field is populated from existing logic — recursive walk must set it.
        assert_eq!(
            encoder.level, 3,
            "level = path_id component count (1-based)"
        );
        // Root section has level 1.
        let root = sections
            .iter()
            .find(|s| s.path_id.as_deref() == Some("1"))
            .expect("section 1 present");
        assert_eq!(root.level, 1);
    }

    #[test]
    fn test_flat_numbering_restart_preserves_unique_tree_identity() {
        let tei = r#"<body>
          <head n="1">Introduction</head><p>Intro.</p>
          <head n="2">Methods</head><p>Methods.</p>
          <head n="1">Appendix</head><p>Appendix root.</p>
          <head n="1.1">Appendix Detail</head><p>Detail.</p>
          <head n="1.1">Appendix Detail Two</head><p>Detail two.</p>
        </body>"#;
        let sections = GrobidParser::extract_sections(tei);
        let paths: Vec<_> = sections
            .iter()
            .map(|section| section.path_id.as_deref().unwrap())
            .collect();
        assert_eq!(paths, vec!["1", "2", "1~2", "1~2.1", "1~2.1~2"]);
        let unique: HashSet<_> = paths.iter().copied().collect();
        assert_eq!(unique.len(), sections.len());
        assert_eq!(sections[3].parent_section_id.as_deref(), Some("sec-1~2"));
    }

    #[test]
    fn test_extract_sections_flat_legacy_tei_still_works() {
        // Legacy TEI with no nested divs — flat <head n="N"> scan.
        // After MEM654 fix: path_id IS populated from <head n="N"> attribute
        // (GROBID encodes section hierarchy lexically, not via XML nesting).
        let tei = r#"<body>
          <head n="1">Intro</head><p>Intro text.</p>
          <head n="2">Methods</head><p>Methods text.</p>
        </body>"#;
        let sections = GrobidParser::extract_sections(tei);
        assert_eq!(sections.len(), 2, "flat TEI yields 2 sections");
        // path_id is now populated from n attribute (MEM654).
        assert_eq!(sections[0].path_id.as_deref(), Some("1"));
        assert_eq!(sections[1].path_id.as_deref(), Some("2"));
    }
}

#[cfg(test)]
mod real_tei_tests {
    use super::*;

    /// Hub paper for large bibliography regression: arXiv **2507.19457** (GEPA).
    /// Real GROBID TEI has ~80 `<biblStruct>` in `<back>` (largest in m033 probe).
    /// Changelog historically called this out as "80 citations with DOI/arXiv id".
    /// Previous pin was only `citations.len() > 5` — too weak to catch bibliography drops.
    #[test]
    fn test_real_grobid_tei_gepa_hub_bibliography_2507_19457() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/grobid.tei.xml"
        );
        let tei = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("skipping real TEI test (file not found): {path}");
                return;
            }
        };
        let sections = GrobidParser::extract_sections(&tei);
        let citations = GrobidParser::extract_citations(&tei);
        println!(
            "GEPA hub TEI 2507.19457: {} sections, {} citations",
            sections.len(),
            citations.len()
        );
        assert!(
            sections.len() >= 80,
            "GEPA hub expected ≥80 sections, got {}",
            sections.len()
        );
        // Pin the large bibliography: must stay at full hub scale (not toy >5).
        assert!(
            citations.len() >= 80,
            "GEPA hub expected ≥80 bibliography citations, got {} — bibliography extract regressed",
            citations.len()
        );
        // First section should be INTRODUCTION
        assert_eq!(sections[0].title, "INTRODUCTION");
        // At least some resolvable identifiers in the hub bibliography.
        let with_arxiv = citations.iter().filter(|c| c.arxiv_id.is_some()).count();
        let with_doi = citations.iter().filter(|c| c.doi.is_some()).count();
        let with_title = citations.iter().filter(|c| c.title.is_some()).count();
        println!("  arxiv_id={with_arxiv} doi={with_doi} title={with_title}");
        // Real GEPA TEI: ~23 arXiv + ~6 DOI (many refs are monographs/venues without idno).
        // Pin floor against total extract collapse, not against optimistic id coverage.
        assert!(
            with_arxiv >= 20,
            "expected ≥20 arXiv ids in hub bibliography, got {with_arxiv}"
        );
        assert!(
            with_arxiv + with_doi >= 25,
            "expected ≥25 DOI/arXiv ids combined, arxiv={with_arxiv} doi={with_doi}"
        );
        assert!(
            with_title >= 15,
            "expected ≥15 citation titles (analytic title present), got {with_title}"
        );
        // raw_text must be non-empty for Reference-node path honesty
        assert!(
            citations.iter().all(|c| !c.raw_text.trim().is_empty()),
            "every citation must carry non-empty raw_text for Reference nodes"
        );
        // T03: in-text `<ref type="bibr">` anchors parsed from the real hub
        // TEI. Every targetable ref resolves against back-matter xml:id; a
        // minority sits in top-level gaps (no section path) and must surface
        // as path-unresolved anchors instead of being dropped.
        let total_anchors: usize = citations.iter().map(|c| c.anchors.len()).sum();
        let resolved_paths = citations
            .iter()
            .flat_map(|c| c.anchors.iter())
            .filter(|a| a.section_path_id.is_some())
            .count();
        let unresolved_paths = total_anchors - resolved_paths;
        println!(
            "  anchors total={total_anchors} resolved_paths={resolved_paths} unresolved_paths={unresolved_paths}"
        );
        assert!(
            total_anchors >= 60,
            "expected ≥60 in-text anchors in hub TEI, got {total_anchors}"
        );
        assert!(
            resolved_paths >= 50,
            "expected ≥50 anchors with a resolved section path, got {resolved_paths}"
        );
        // This fixed hub TEI has every targetable ref inside a section text
        // window (footnote refs included) — none may be dropped or left
        // path-unresolved. The unresolved path is exercised by unit tests
        // (test_extract_citations_ref_outside_section_path_unresolved,
        // test_extract_citations_anchor_in_top_level_gap_path_unresolved).
        assert_eq!(
            unresolved_paths, 0,
            "hub TEI must resolve every anchor to a section path, got {unresolved_paths} unresolved"
        );
        // Anchor offsets must be stable within the section's stored text and
        // every resolved path must exist among the section extractor's paths.
        let section_paths: std::collections::HashSet<&str> = sections
            .iter()
            .filter_map(|s| s.path_id.as_deref())
            .collect();
        for c in &citations {
            for a in &c.anchors {
                if let Some(p) = a.section_path_id.as_deref() {
                    assert!(
                        section_paths.contains(p),
                        "anchor path '{p}' must exist among section paths (anchor {} on '{}')",
                        a.ref_id,
                        c.raw_text.chars().take(40).collect::<String>()
                    );
                    if let Some(offset) = a.offset {
                        // Offset must point at the quote inside the section text.
                        let sec = sections
                            .iter()
                            .find(|s| s.path_id.as_deref() == Some(p))
                            .expect("path found");
                        let chars: Vec<char> = sec.text.chars().collect();
                        let start = offset as usize;
                        let end = (start + a.quote.chars().count()).min(chars.len());
                        assert_eq!(
                            chars.get(start..end).map(|s| s.iter().collect::<String>()),
                            Some(a.quote.clone()),
                            "offset {offset} must point at quote '{}' in section '{p}'",
                            a.quote
                        );
                    }
                }
            }
        }
    }

    /// Second large-bibliography pin (m033 probe): 2605.26525v1 ~78 back biblStruct.
    #[test]
    fn test_real_grobid_tei_large_bibliography_2605_26525() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/article_corpora/m033-grobid-probe-v1/per-paper/2605.26525v1/grobid.tei.xml"
        );
        let tei = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("skipping real TEI test (file not found): {path}");
                return;
            }
        };
        let citations = GrobidParser::extract_citations(&tei);
        println!("2605.26525v1 TEI: {} citations", citations.len());
        assert!(
            citations.len() >= 70,
            "expected large bibliography ≥70 citations, got {}",
            citations.len()
        );
    }

    /// Backward-compatible alias of the hub pin (historical test name).
    #[test]
    fn test_real_grobid_tei_sections_and_citations() {
        test_real_grobid_tei_gepa_hub_bibliography_2507_19457();
    }
}
