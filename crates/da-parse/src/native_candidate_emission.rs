//! Diagnostic-only TEI from independently available native scholarly candidates.
//! No Java input, graph writes, deployment authority, or invented body structure.
use crate::{
    CalloutLinkTarget, CalloutMarkerClass, CitationRole, NativeHeaderCandidate,
    NativeReferenceCandidate, NativeTeiEmissionLimits, TeiEnvelope, TeiIdentity,
    link_native_reference_markers,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const NATIVE_CANDIDATE_EMISSION_SCHEMA_VERSION: &str = "da-parse.native-candidate-emission/v1";
pub const NATIVE_CANDIDATE_EMISSION_BACKEND_ID: &str = "native-scholarly-candidate";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCandidateEmissionManifest {
    pub schema_version: String,
    pub source_sha256: String,
    pub config_fingerprint: String,
    pub diagnostic_only: bool,
    pub import_eligible: bool,
    pub deployed: bool,
    pub auto_promote: String,
    pub graph_write: String,
    pub production_route: String,
    pub body_structure: String,
    pub header_present: bool,
    pub reference_records: usize,
    pub resolved_markers: usize,
    /// `max_absences` bounds summary entries, not line occurrences. Raw line
    /// evidence and full dispositions remain individually byte-bounded TEI notes.
    pub limitations: Vec<String>,
    pub limitation_count_semantics: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCandidateEmission {
    pub envelope: TeiEnvelope,
    pub manifest: NativeCandidateEmissionManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCandidateEmissionError {
    Identity,
    UnrepresentableText,
    DuplicateReferenceKey,
    LimitsExceeded(&'static str),
    InvalidLinks,
}
impl fmt::Display for NativeCandidateEmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native candidate emission refused: {self:?}")
    }
}
impl std::error::Error for NativeCandidateEmissionError {}

type Result<T> = std::result::Result<T, NativeCandidateEmissionError>;

pub fn emit_native_candidate(
    header: Option<&NativeHeaderCandidate>,
    references: Option<&NativeReferenceCandidate>,
    body: &str,
    source_sha256: &str,
    config_fingerprint: &str,
) -> Result<NativeCandidateEmission> {
    emit_native_candidate_with_limits(
        header,
        references,
        body,
        source_sha256,
        config_fingerprint,
        NativeTeiEmissionLimits::default(),
    )
}

pub fn emit_native_candidate_with_limits(
    header: Option<&NativeHeaderCandidate>,
    references: Option<&NativeReferenceCandidate>,
    body: &str,
    source_sha256: &str,
    config_fingerprint: &str,
    limits: NativeTeiEmissionLimits,
) -> Result<NativeCandidateEmission> {
    if source_sha256.len() != 64
        || !source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || !representable_attribute(config_fingerprint)
        || header.is_some_and(|h| h.identity.source_pdf_sha256 != source_sha256)
        || references.is_some_and(|r| r.identity.source_pdf_sha256 != source_sha256)
        || matches!((header, references), (Some(h), Some(r)) if h.identity != r.identity)
    {
        return Err(NativeCandidateEmissionError::Identity);
    }
    if body.len() > limits.max_fragment_bytes {
        return Err(NativeCandidateEmissionError::LimitsExceeded("body"));
    }
    if !body.is_empty() && !representable_text(body) {
        return Err(NativeCandidateEmissionError::UnrepresentableText);
    }
    if limits.max_body_sections == 0 || limits.max_body_leaves == 0 {
        return Err(NativeCandidateEmissionError::LimitsExceeded(
            "single_body_paragraph",
        ));
    }
    let mut limitations = vec![
        "body_structure_unsupported: exact supplied text in one paragraph".to_owned(),
        "author_lists_and_affiliation_linkage_unsupported".to_owned(),
        "captions_tables_formulas_and_visual_structure_not_emitted".to_owned(),
        "citation_role_notes_are_candidate_evidence_not_graph_truth".to_owned(),
        "unprojected_header_roles_not_emitted".to_owned(),
        "frozen_lifter_does_not_decode_xml_entities: emitted_xml_is_authoritative".to_owned(),
        "existing_callout_scanner_requires_standalone_bracket_tokens: punctuation_forms_may_remain_unlinked".to_owned(),
    ];
    let mut head = String::from("<teiHeader><fileDesc><titleStmt>");
    // The existing lifter needs a paired title to keep absent header titles
    // from consuming a later bibliography title as the article title.
    if header.is_none_or(|h| h.fields.title.is_empty()) {
        head.push_str("<title></title>");
    }
    if let Some(h) = header {
        for title in &h.fields.title {
            element(&mut head, "title", "", title)?;
        }
    }
    head.push_str("</titleStmt><publicationStmt>");
    if let Some(h) = header {
        if h.recognized_date().is_some() {
            element(
                &mut head,
                "date",
                " type=\"published\"",
                &h.dates[0].source_text,
            )?;
        }
        for email in &h.fields.email {
            element(&mut head, "email", "", email)?;
        }
        for id in &h.fields.ids {
            element(&mut head, "idno", " type=\"pubnum\"", id)?;
        }
    }
    head.push_str("</publicationStmt>");
    if let Some(h) = header {
        head.push_str("<notesStmt>");
        for author in &h.authors {
            element(
                &mut head,
                "note",
                " type=\"native_header_author\"",
                &author.source_text,
            )?;
            element(
                &mut head,
                "note",
                " type=\"native_header_author_leaf_outcome\"",
                &format!("{:?}", author.outcome),
            )?;
        }
        for affiliation in &h.affiliations {
            element(
                &mut head,
                "note",
                " type=\"native_header_affiliation\"",
                &affiliation.source_text,
            )?;
        }
        if h.recognized_date().is_none() {
            for date in &h.dates {
                element(
                    &mut head,
                    "note",
                    " type=\"native_header_date\"",
                    &date.source_text,
                )?;
            }
        }
        head.push_str("</notesStmt>");
    }
    head.push_str(
        "<sourceDesc><p>Native candidate evidence</p></sourceDesc></fileDesc></teiHeader>",
    );
    let mut bibliography = String::from("<back><listBibl>");
    let mut keys = BTreeSet::new();
    if let Some(references) = references {
        for record in &references.records {
            let key = record.published_key();
            if !keys.insert(key.clone()) {
                return Err(NativeCandidateEmissionError::DuplicateReferenceKey);
            }
            bibliography.push_str(&format!("<biblStruct xml:id=\"{key}\"><analytic>"));
            for field in &record.citation.roles {
                if field.role == CitationRole::Title {
                    element(&mut bibliography, "title", " level=\"a\"", &field.text)?;
                }
            }
            bibliography.push_str("</analytic><monogr>");
            for field in &record.citation.roles {
                match field.role {
                    CitationRole::Journal => {
                        element(&mut bibliography, "title", " level=\"j\"", &field.text)?
                    }
                    CitationRole::Booktitle => {
                        element(&mut bibliography, "title", " level=\"m\"", &field.text)?
                    }
                    _ => {}
                }
            }
            bibliography.push_str("<imprint>");
            for field in &record.citation.roles {
                match field.role {
                    CitationRole::Date => element(
                        &mut bibliography,
                        "date",
                        " type=\"published\"",
                        &field.text,
                    )?,
                    CitationRole::Publisher => {
                        element(&mut bibliography, "publisher", "", &field.text)?
                    }
                    CitationRole::Location => {
                        element(&mut bibliography, "pubPlace", "", &field.text)?
                    }
                    CitationRole::Volume | CitationRole::Issue | CitationRole::Pages => {
                        element(
                            &mut bibliography,
                            "biblScope",
                            &format!(" unit=\"{}\"", field.role.as_str()),
                            &field.text,
                        )?;
                    }
                    _ => {}
                }
            }
            bibliography.push_str("</imprint></monogr>");
            element(
                &mut bibliography,
                "note",
                " type=\"raw_reference\"",
                &record.source_text,
            )?;
            element(
                &mut bibliography,
                "note",
                " type=\"printed_label\"",
                &record.printed_label.text,
            )?;
            // Every role is retained per record, including roles with no safe TEI mapping.
            for field in &record.citation.roles {
                element(
                    &mut bibliography,
                    "note",
                    &format!(
                        " type=\"native_citation_role\" subtype=\"{}\"",
                        field.role.as_str()
                    ),
                    &field.text,
                )?;
                if field.role == CitationRole::Web
                    && (field.text.starts_with("https://") || field.text.starts_with("http://"))
                    && representable_attribute(&field.text)
                    && !field.text.chars().any(char::is_whitespace)
                {
                    bibliography.push_str(&format!(
                        "<ptr type=\"web\" target=\"{}\"/>",
                        esc(&field.text)
                    ));
                }
            }
            bibliography.push_str("</biblStruct>");
        }
        // Closed typed categories, never source text/digests as summary keys.
        // This new emitter counts summary entries under max_absences; each line
        // still contributes its raw evidence and full disposition to byte caps.
        let mut line_counts = BTreeMap::<&str, usize>::new();
        for line in &references.lines {
            let category = match &line.disposition {
                crate::NativeReferenceLineDisposition::Unsupported { reason } => {
                    Some(match reason {
                        crate::NativeReferenceLineUnsupported::NoPrintedLabel => {
                            "unsupported_reference_line:no_printed_label"
                        }
                        crate::NativeReferenceLineUnsupported::MultiplePrintedLabels => {
                            "unsupported_reference_line:multiple_printed_labels"
                        }
                        crate::NativeReferenceLineUnsupported::NonLeadingLabel => {
                            "unsupported_reference_line:non_leading_label"
                        }
                        crate::NativeReferenceLineUnsupported::NoReferenceText => {
                            "unsupported_reference_line:no_reference_text"
                        }
                        crate::NativeReferenceLineUnsupported::OtherMaterial => {
                            "unsupported_reference_line:other_material"
                        }
                    })
                }
                crate::NativeReferenceLineDisposition::SectionHeading { matched_by, .. } => {
                    Some(match matched_by {
                        crate::HeadingMatch::PreambleDefault => {
                            "reference_section_heading:preamble_default"
                        }
                        crate::HeadingMatch::NormalizedLine => {
                            "reference_section_heading:normalized_line"
                        }
                        crate::HeadingMatch::OrdinalStrippedLine => {
                            "reference_section_heading:ordinal_stripped_line"
                        }
                        crate::HeadingMatch::LeadingSpanLine => {
                            "reference_section_heading:leading_span_line"
                        }
                        crate::HeadingMatch::ProminentSectionHeading => {
                            "reference_section_heading:prominent_section_heading"
                        }
                    })
                }
                crate::NativeReferenceLineDisposition::LabelledLine { .. }
                | crate::NativeReferenceLineDisposition::Empty => None,
            };
            if let Some(category) = category {
                *line_counts.entry(category).or_default() += 1;
                let note_type = if matches!(
                    line.disposition,
                    crate::NativeReferenceLineDisposition::SectionHeading { .. }
                ) {
                    "reference_section_heading"
                } else {
                    "unsupported_reference_line"
                };
                if let Some(decoded) = &line.decoded {
                    element(
                        &mut bibliography,
                        "note",
                        &format!(" type=\"{note_type}\""),
                        &decoded.line_text,
                    )?;
                }
                element(
                    &mut bibliography,
                    "note",
                    " type=\"reference_line_disposition\"",
                    &format!("{:?}", line.disposition),
                )?;
            }
        }
        limitations.extend(
            line_counts
                .into_iter()
                .map(|(category, count)| format!("{category}:count={count}")),
        );
    }
    bibliography.push_str("</listBibl></back>");
    let mut paragraph = String::from("<div type=\"native_candidate_body\"><p>");
    let mut resolved_markers = 0;
    let mut cursor = 0;
    if let Some(references) = references {
        match link_native_reference_markers(body, references) {
            Ok(linked) => {
                for marker in &linked.report.markers {
                    if marker.class != CalloutMarkerClass::Bibliography {
                        continue;
                    }
                    let links = linked
                        .report
                        .links
                        .iter()
                        .filter(|l| l.occurrence_index == marker.index)
                        .collect::<Vec<_>>();
                    let mut targets = Vec::new();
                    for link in &links {
                        if let CalloutLinkTarget::Resolved { target_id, .. } = &link.target {
                            if !keys.contains(target_id) {
                                return Err(NativeCandidateEmissionError::InvalidLinks);
                            }
                            targets.push(format!("#{target_id}"));
                        }
                    }
                    if targets.is_empty() || targets.len() != links.len() {
                        continue;
                    }
                    if marker.span.start < cursor {
                        return Err(NativeCandidateEmissionError::InvalidLinks);
                    }
                    let prefix = body
                        .get(cursor..marker.span.start)
                        .ok_or(NativeCandidateEmissionError::InvalidLinks)?;
                    let text = body
                        .get(marker.span.clone())
                        .ok_or(NativeCandidateEmissionError::InvalidLinks)?;
                    paragraph.push_str(&esc(prefix));
                    element(
                        &mut paragraph,
                        "ref",
                        &format!(" type=\"bibr\" target=\"{}\"", targets.join(" ")),
                        text,
                    )?;
                    cursor = marker.span.end;
                    resolved_markers += 1;
                }
            }
            Err(error) => limitations.push(format!("inline_links_not_emitted:{error}")),
        }
    }
    paragraph.push_str(&esc(&body[cursor..]));
    paragraph.push_str("</p></div>");
    for fragment in [&head, &bibliography, &paragraph] {
        if fragment.len() > limits.max_fragment_bytes {
            return Err(NativeCandidateEmissionError::LimitsExceeded("fragment"));
        }
    }
    if limitations.len() > limits.max_absences {
        return Err(NativeCandidateEmissionError::LimitsExceeded("limitations"));
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><TEI xmlns=\"http://www.tei-c.org/ns/1.0\">{head}<text><body>{paragraph}</body>{bibliography}</text></TEI>"
    );
    if xml.len() > limits.max_envelope_bytes {
        return Err(NativeCandidateEmissionError::LimitsExceeded("envelope"));
    }
    // Markup and names are closed constants; source values are escaped text.
    // IDs are generated integers and every inline target was checked above.
    Ok(NativeCandidateEmission {
        envelope: TeiEnvelope::new(
            xml.into_bytes(),
            TeiIdentity::new(NATIVE_CANDIDATE_EMISSION_BACKEND_ID, config_fingerprint),
        ),
        manifest: NativeCandidateEmissionManifest {
            schema_version: NATIVE_CANDIDATE_EMISSION_SCHEMA_VERSION.to_owned(),
            source_sha256: source_sha256.to_owned(),
            config_fingerprint: config_fingerprint.to_owned(),
            diagnostic_only: true,
            import_eligible: false,
            deployed: false,
            auto_promote: "forbidden".into(),
            graph_write: "forbidden".into(),
            production_route: "forbidden".into(),
            body_structure: "single_supplied_text_paragraph".into(),
            header_present: header.is_some(),
            reference_records: references.map_or(0, |r| r.records.len()),
            resolved_markers,
            limitations,
            limitation_count_semantics: "bounded_typed_summary_entries_not_line_occurrences".into(),
        },
    })
}

fn element(out: &mut String, tag: &str, attributes: &str, text: &str) -> Result<()> {
    if !representable_text(text) {
        return Err(NativeCandidateEmissionError::UnrepresentableText);
    }
    out.push_str(&format!("<{tag}{attributes}>{}</{tag}>", esc(text)));
    Ok(())
}

fn representable_text(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| is_legal_xml_char(c) && c != '\r')
}

fn representable_attribute(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| is_legal_xml_char(c) && !matches!(c, '\t' | '\n' | '\r'))
}

fn is_legal_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&c)
        || ('\u{E000}'..='\u{FFFD}').contains(&c)
        || ('\u{10000}'..='\u{10FFFF}').contains(&c)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod coverage_smoke {
    use super::*;

    #[test]
    fn empty_body_with_no_candidates_emits_a_diagnostic_envelope() {
        let emission = emit_native_candidate(None, None, "", &"a".repeat(64), "coverage-smoke")
            .expect("an empty body with no candidates must still emit");
        assert!(emission.manifest.diagnostic_only);
        assert!(!emission.manifest.import_eligible);
        assert_eq!(
            emission.manifest.schema_version,
            NATIVE_CANDIDATE_EMISSION_SCHEMA_VERSION
        );
    }
}
