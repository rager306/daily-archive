//! Opt-in scholarly stages over the existing native region carrier.
//!
//! Models and lexicons are caller-owned inputs. This candidate does not load
//! resources, choose a production route, emit TEI, or change the legacy
//! date-required composition. Reference discovery is deliberately limited to one
//! explicitly labelled reference per physical line; unsupported lines remain
//! visible and prevent a claim of a complete citation-target inventory.

use crate::{
    BibliographyGazetteerError, BibliographyGazetteerIndex, BibliographyTokenFacts,
    CALLOUT_MARKER_SCHEMA_VERSION, CalloutBodyInventory, CalloutBodyTarget, CalloutLabelBasis,
    CalloutLinkReport, CalloutMarkerClass, CalloutMarkerError, CitationClusterSequence,
    FullLocation, FullSemanticRegion, HeaderFieldSpan, HeadingMatch, LayoutTokenKind,
    MAX_NATIVE_REGION_TOKENS, NATIVE_COMPOSITION_MAX_REFERENCES,
    NATIVE_HEADING_REGIONS_SCHEMA_VERSION, NATIVE_REGION_INPUT_SCHEMA_VERSION,
    NativeAffiliationOutcome, NativeAffiliationParser, NativeBibliographyFeatureError,
    NativeBibliographyInputError, NativeBibliographySequenceError, NativeDateOutcome,
    NativeDateParser, NativeHeaderDecodeCandidate, NativeHeaderDecodeError,
    NativeHeaderDecodeOutcome, NativeHeaderFeatureError, NativeHeaderFields,
    NativeHeaderFieldsError, NativeHeaderInputError, NativeHeadingRegionOutcome,
    NativePersonalNameOutcome, NativePersonalNameParser, NativeRegionCarrier, NativeRegionRun,
    PersonalNameContext, RecognizedDate, ReferenceSegmentKind, ReferenceSegmentSequence,
    build_bibliography_line_facts, build_header_feature_tokens, citation_row,
    classify_callout_markers, decode_citation_sequence_prepared, decode_header_sequence_prepared,
    decode_reference_sequence_prepared, derive_header_font_statistics, link_callout_markers,
    project_native_header_fields, reference_segmenter_row, render_citation_feature_text,
    render_header_feature_text, render_header_features, render_reference_segmenter_feature_text,
};
use da_ports::{CrfModel, LexiconBundle, PreparedCrfModel};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt, ops::Range};

pub const NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION: &str =
    "da-parse.native-scholarly-candidate/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCandidateIdentity {
    pub document_id: String,
    pub source_pdf_sha256: String,
    pub carrier_digest: String,
}

/// One ABI word's identity in its region, not an inferred PDF byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCandidateSourceRow {
    pub row: usize,
    pub location: FullLocation,
    pub source_ordinal: u32,
    pub text_digest: String,
}

/// The leaf consumes this entire decoded family span without guessing splits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeHeaderLeaf<T> {
    pub span: HeaderFieldSpan,
    pub source_rows: Vec<NativeCandidateSourceRow>,
    pub source_text: String,
    pub outcome: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeHeaderCandidate {
    pub schema_version: &'static str,
    pub identity: NativeCandidateIdentity,
    pub feature_digest: String,
    pub rows: Vec<NativeCandidateSourceRow>,
    pub decoded: NativeHeaderDecodeCandidate,
    pub fields: NativeHeaderFields,
    pub dates: Vec<NativeHeaderLeaf<NativeDateOutcome>>,
    /// One leaf outcome per CRF run, not a claim that a run contains one person.
    pub authors: Vec<NativeHeaderLeaf<NativePersonalNameOutcome>>,
    pub affiliations: Vec<NativeHeaderLeaf<NativeAffiliationOutcome>>,
}

impl NativeHeaderCandidate {
    /// Multiple or unrecognized date runs are not silently resolved.
    pub fn recognized_date(&self) -> Option<RecognizedDate> {
        match self.dates.as_slice() {
            [date] => date.outcome.recognized(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePrintedReferenceLabel {
    pub text: String,
    /// Half-open ABI row range in this record's physical reference line.
    pub rows: Range<usize>,
    /// Only explicit Arabic labels are addressable in this initial contract.
    /// Brackets and a final numbering dot are syntax, never an ordinal guess.
    pub arabic_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReferenceRecord {
    pub ordinal: usize,
    pub printed_label: NativePrintedReferenceLabel,
    /// Source rows of the entire labelled line, including the label.
    pub source_rows: Vec<NativeCandidateSourceRow>,
    pub source_text: String,
    /// Source rows corresponding exactly to `citation.labels` and role ranges.
    pub citation_source_rows: Vec<NativeCandidateSourceRow>,
    pub citation: CitationClusterSequence,
}

impl NativeReferenceRecord {
    pub fn published_key(&self) -> String {
        format!("b{}", self.ordinal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeReferenceLineUnsupported {
    NoPrintedLabel,
    MultiplePrintedLabels,
    NonLeadingLabel,
    NoReferenceText,
    OtherMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeReferenceLineDisposition {
    LabelledLine {
        ordinal: usize,
    },
    /// Exact whole-line heading evidence, retained rather than decoded as a record.
    SectionHeading {
        matched_by: HeadingMatch,
        text_digest: String,
    },
    Empty,
    Unsupported {
        reason: NativeReferenceLineUnsupported,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReferenceLine {
    pub source_rows: Vec<NativeCandidateSourceRow>,
    pub decoded: Option<ReferenceSegmentSequence>,
    pub disposition: NativeReferenceLineDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReferenceCandidate {
    pub schema_version: &'static str,
    pub identity: NativeCandidateIdentity,
    pub reference_feature_digest: String,
    pub citation_feature_digest: String,
    /// Per-reference ownership is retained; fields are never pooled.
    pub records: Vec<NativeReferenceRecord>,
    pub lines: Vec<NativeReferenceLine>,
}

impl NativeReferenceCandidate {
    /// Complete only within the declared one-labelled-physical-line envelope.
    pub fn has_complete_labelled_line_inventory(&self) -> bool {
        !self.records.is_empty()
            && self.lines.iter().all(|line| {
                !matches!(
                    line.disposition,
                    NativeReferenceLineDisposition::Unsupported { .. }
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReferenceLinks {
    pub body_sha256: String,
    pub report: CalloutLinkReport,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeScholarlyModels<'a> {
    pub header: &'a CrfModel,
    pub reference_segmenter: &'a CrfModel,
    pub citation: &'a CrfModel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeScholarlyCandidate {
    pub header: Result<NativeHeaderCandidate, NativeScholarlyCandidateError>,
    pub references: Result<NativeReferenceCandidate, NativeScholarlyCandidateError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeScholarlyCandidateError {
    RegionAbsent {
        region: FullSemanticRegion,
        requested: bool,
    },
    LimitsExceeded {
        resource: &'static str,
        count: usize,
        maximum: usize,
    },
    SourceRowMismatch {
        region: FullSemanticRegion,
    },
    HeadingEvidenceMismatch {
        field: &'static str,
    },
    Identity,
    HeaderInput(NativeHeaderInputError),
    HeaderFeatures(NativeHeaderFeatureError),
    HeaderDecode(NativeHeaderDecodeError),
    HeaderFields(NativeHeaderFieldsError),
    BibliographyInput(NativeBibliographyInputError),
    BibliographyFeatures(NativeBibliographyFeatureError),
    BibliographyDecode(NativeBibliographySequenceError),
    Gazetteer(BibliographyGazetteerError),
    Callout(CalloutMarkerError),
    ReferenceInventoryIncomplete,
    UnaddressablePrintedLabel {
        ordinal: usize,
    },
}

impl fmt::Display for NativeScholarlyCandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native scholarly candidate refused: {self:?}")
    }
}

impl std::error::Error for NativeScholarlyCandidateError {}

pub fn build_native_scholarly_candidate(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
    models: NativeScholarlyModels<'_>,
) -> NativeScholarlyCandidate {
    NativeScholarlyCandidate {
        header: build_native_header_candidate(carrier, lexicon, models.header),
        references: build_native_reference_candidate(
            carrier,
            lexicon,
            models.reference_segmenter,
            models.citation,
        ),
    }
}

pub fn build_native_header_candidate(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
    model: &CrfModel,
) -> Result<NativeHeaderCandidate, NativeScholarlyCandidateError> {
    let run = region_run(carrier, FullSemanticRegion::Header)?;
    let source_rows = source_rows(run)?;
    let tokens =
        build_header_feature_tokens(run).map_err(NativeScholarlyCandidateError::HeaderInput)?;
    let fonts = derive_header_font_statistics(&tokens)
        .map_err(NativeScholarlyCandidateError::HeaderInput)?;
    let rows = render_header_features(&tokens, fonts, lexicon)
        .map_err(NativeScholarlyCandidateError::HeaderFeatures)?;
    if source_rows.len() != rows.len() {
        return Err(NativeScholarlyCandidateError::SourceRowMismatch {
            region: FullSemanticRegion::Header,
        });
    }
    let feature_text =
        render_header_feature_text(&rows).map_err(NativeScholarlyCandidateError::HeaderFeatures)?;
    let prepared = PreparedCrfModel::new(model);
    let decoded = match decode_header_sequence_prepared(&rows, &prepared) {
        NativeHeaderDecodeOutcome::Accepted { decode } => decode,
        NativeHeaderDecodeOutcome::Rejected { error } => {
            return Err(NativeScholarlyCandidateError::HeaderDecode(error));
        }
    };
    let fields = project_native_header_fields(&rows, &decoded.fields)
        .map_err(NativeScholarlyCandidateError::HeaderFields)?;
    let mut dates = Vec::new();
    let mut authors = Vec::new();
    let mut affiliations = Vec::new();
    for span in &decoded.fields {
        let text = rows[span.start..span.end]
            .iter()
            .map(|row| row.columns[0].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let owned_rows = source_rows[span.start..span.end].to_vec();
        match span.label.as_str() {
            "<date>" => dates.push(NativeHeaderLeaf {
                span: span.clone(),
                source_rows: owned_rows,
                outcome: NativeDateParser::new().parse(&text),
                source_text: text,
            }),
            "<author>" => authors.push(NativeHeaderLeaf {
                span: span.clone(),
                source_rows: owned_rows,
                outcome: NativePersonalNameParser::new().parse(PersonalNameContext::Header, &text),
                source_text: text,
            }),
            "<affiliation>" => affiliations.push(NativeHeaderLeaf {
                span: span.clone(),
                source_rows: owned_rows,
                outcome: NativeAffiliationParser::new().parse(&text),
                source_text: text,
            }),
            _ => {}
        }
    }
    Ok(NativeHeaderCandidate {
        schema_version: NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION,
        identity: candidate_identity(carrier)?,
        feature_digest: digest(feature_text.as_bytes()),
        rows: source_rows,
        decoded,
        fields,
        dates,
        authors,
        affiliations,
    })
}

pub fn build_native_reference_candidate(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
    reference_model: &CrfModel,
    citation_model: &CrfModel,
) -> Result<NativeReferenceCandidate, NativeScholarlyCandidateError> {
    build_native_reference_candidate_inner(carrier, lexicon, reference_model, citation_model, None)
}

/// The original heading receipt distinguishes whole section headings from
/// reference material. Leading-span matches may include the first entry and
/// are deliberately not exempted from the labelled-line contract.
///
/// `headings` must be the unmodified in-process result of `assign_heading_regions`
/// used to build this word carrier. Identity and source-row checks reject a
/// mismatched receipt; they do not authenticate caller-mutated heading metadata.
pub fn build_native_reference_candidate_with_headings(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
    reference_model: &CrfModel,
    citation_model: &CrfModel,
    headings: &NativeHeadingRegionOutcome,
) -> Result<NativeReferenceCandidate, NativeScholarlyCandidateError> {
    build_native_reference_candidate_inner(
        carrier,
        lexicon,
        reference_model,
        citation_model,
        Some(headings),
    )
}

fn build_native_reference_candidate_inner(
    carrier: &NativeRegionCarrier,
    lexicon: &LexiconBundle,
    reference_model: &CrfModel,
    citation_model: &CrfModel,
    headings: Option<&NativeHeadingRegionOutcome>,
) -> Result<NativeReferenceCandidate, NativeScholarlyCandidateError> {
    let run = region_run(carrier, FullSemanticRegion::References)?;
    let source_rows = source_rows(run)?;
    let section_headings = match headings {
        Some(headings) => reference_section_headings(carrier, &source_rows, headings)?,
        None => BTreeMap::new(),
    };
    let reference_prepared = PreparedCrfModel::new(reference_model);
    let citation_prepared = PreparedCrfModel::new(citation_model);
    let gazetteer = BibliographyGazetteerIndex::from_lexicon(lexicon)
        .map_err(NativeScholarlyCandidateError::Gazetteer)?;
    let facts = build_bibliography_line_facts(run)
        .map_err(NativeScholarlyCandidateError::BibliographyInput)?;
    let mut block_maxima: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for line in &facts {
        let maximum = block_maxima
            .entry((line.page_ordinal, line.block_ordinal))
            .or_default();
        *maximum = (*maximum).max(line.facts.len());
    }
    let mut lines = Vec::with_capacity(facts.len());
    let mut records = Vec::new();
    let mut reference_hash = Sha256::new();
    let mut citation_hash = Sha256::new();
    let mut cursor = 0usize;
    for line in facts {
        let end = cursor + line.facts.len();
        let owned_rows = source_rows
            .get(cursor..end)
            .ok_or(NativeScholarlyCandidateError::SourceRowMismatch {
                region: FullSemanticRegion::References,
            })?
            .to_vec();
        cursor = end;
        if line.facts.is_empty() {
            lines.push(NativeReferenceLine {
                source_rows: owned_rows,
                decoded: None,
                disposition: NativeReferenceLineDisposition::Empty,
            });
            continue;
        }
        let maximum = block_maxima[&(line.page_ordinal, line.block_ordinal)];
        let rows = reference_segmenter_row(&line.facts, lexicon, maximum)
            .map_err(NativeScholarlyCandidateError::BibliographyFeatures)?;
        let feature_text = render_reference_segmenter_feature_text(&rows)
            .map_err(NativeScholarlyCandidateError::BibliographyFeatures)?;
        reference_hash.update(feature_text.as_bytes());
        reference_hash.update(
            b"
",
        );
        let decoded = decode_reference_sequence_prepared(&rows, &reference_prepared)
            .map_err(NativeScholarlyCandidateError::BibliographyDecode)?;
        if let Some((matched_by, text_digest)) =
            section_headings.get(&(line.page_ordinal, line.line_ordinal))
        {
            lines.push(NativeReferenceLine {
                source_rows: owned_rows,
                decoded: Some(decoded),
                disposition: NativeReferenceLineDisposition::SectionHeading {
                    matched_by: *matched_by,
                    text_digest: text_digest.clone(),
                },
            });
            continue;
        }
        let disposition = match labelled_line_ranges(&decoded) {
            Ok((label_range, reference_range)) => {
                if records.len() >= NATIVE_COMPOSITION_MAX_REFERENCES {
                    return Err(NativeScholarlyCandidateError::LimitsExceeded {
                        resource: "reference",
                        count: records.len() + 1,
                        maximum: NATIVE_COMPOSITION_MAX_REFERENCES,
                    });
                }
                let label_text = decoded.segments[0].text.clone();
                let citation_facts: &[BibliographyTokenFacts] =
                    &line.facts[reference_range.clone()];
                let texts = citation_facts
                    .iter()
                    .map(|fact| fact.text.clone())
                    .collect::<Vec<_>>();
                let spans = gazetteer
                    .spans(&texts)
                    .map_err(NativeScholarlyCandidateError::Gazetteer)?;
                let rows = citation_row(citation_facts, &spans, lexicon)
                    .map_err(NativeScholarlyCandidateError::BibliographyFeatures)?;
                let feature_text = render_citation_feature_text(&rows)
                    .map_err(NativeScholarlyCandidateError::BibliographyFeatures)?;
                citation_hash.update(feature_text.as_bytes());
                citation_hash.update(
                    b"
",
                );
                let citation = decode_citation_sequence_prepared(&rows, &citation_prepared)
                    .map_err(NativeScholarlyCandidateError::BibliographyDecode)?;
                let ordinal = records.len();
                records.push(NativeReferenceRecord {
                    ordinal,
                    printed_label: NativePrintedReferenceLabel {
                        arabic_key: printed_arabic_label(&label_text),
                        text: label_text,
                        rows: label_range,
                    },
                    citation_source_rows: owned_rows[reference_range].to_vec(),
                    source_rows: owned_rows.clone(),
                    source_text: decoded.line_text.clone(),
                    citation,
                });
                NativeReferenceLineDisposition::LabelledLine { ordinal }
            }
            Err(reason) => NativeReferenceLineDisposition::Unsupported { reason },
        };
        lines.push(NativeReferenceLine {
            source_rows: owned_rows,
            decoded: Some(decoded),
            disposition,
        });
    }
    if cursor != source_rows.len() {
        return Err(NativeScholarlyCandidateError::SourceRowMismatch {
            region: FullSemanticRegion::References,
        });
    }
    Ok(NativeReferenceCandidate {
        schema_version: NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION,
        identity: candidate_identity(carrier)?,
        reference_feature_digest: format!("sha256:{:x}", reference_hash.finalize()),
        citation_feature_digest: format!("sha256:{:x}", citation_hash.finalize()),
        records,
        lines,
    })
}

/// Printed Arabic labels, not reference position, establish this inventory.
/// Unsupported reference material prevents linking against an incomplete domain.
pub fn link_native_reference_markers(
    body_text: &str,
    references: &NativeReferenceCandidate,
) -> Result<NativeReferenceLinks, NativeScholarlyCandidateError> {
    if !references.has_complete_labelled_line_inventory() {
        return Err(NativeScholarlyCandidateError::ReferenceInventoryIncomplete);
    }
    let mut targets = Vec::with_capacity(references.records.len());
    for record in &references.records {
        let key = record.printed_label.arabic_key.clone().ok_or(
            NativeScholarlyCandidateError::UnaddressablePrintedLabel {
                ordinal: record.ordinal,
            },
        )?;
        targets.push(CalloutBodyTarget {
            class: CalloutMarkerClass::Bibliography,
            target_id: record.published_key(),
            labels: vec![key],
            basis: CalloutLabelBasis::Printed,
            ordinal: record.ordinal + 1,
        });
    }
    let markers =
        classify_callout_markers(body_text).map_err(NativeScholarlyCandidateError::Callout)?;
    let inventory = CalloutBodyInventory::from_targets(targets);
    let links = link_callout_markers(&markers, &inventory);
    let resolved = links.iter().filter(|link| link.is_resolved()).count();
    let report = CalloutLinkReport {
        schema_version: CALLOUT_MARKER_SCHEMA_VERSION.to_owned(),
        resolved,
        unresolved: links.len() - resolved,
        markers,
        links,
    };
    report
        .validate()
        .map_err(NativeScholarlyCandidateError::Callout)?;
    Ok(NativeReferenceLinks {
        body_sha256: digest(body_text.as_bytes()),
        report,
    })
}

fn region_run(
    carrier: &NativeRegionCarrier,
    region: FullSemanticRegion,
) -> Result<&NativeRegionRun, NativeScholarlyCandidateError> {
    carrier
        .run(region)
        .ok_or(NativeScholarlyCandidateError::RegionAbsent {
            region,
            requested: carrier.absence(region).is_some(),
        })
}

fn source_rows(
    run: &NativeRegionRun,
) -> Result<Vec<NativeCandidateSourceRow>, NativeScholarlyCandidateError> {
    if run.tokens.len() > MAX_NATIVE_REGION_TOKENS {
        return Err(NativeScholarlyCandidateError::LimitsExceeded {
            resource: "region_token",
            count: run.tokens.len(),
            maximum: MAX_NATIVE_REGION_TOKENS,
        });
    }
    Ok(run
        .tokens
        .iter()
        .filter(|token| token.kind != LayoutTokenKind::Whitespace && !token.text().is_empty())
        .enumerate()
        .map(|(row, token)| NativeCandidateSourceRow {
            row,
            location: token.location.clone(),
            source_ordinal: token.source_ordinal,
            text_digest: token.text_digest.clone(),
        })
        .collect())
}

fn candidate_identity(
    carrier: &NativeRegionCarrier,
) -> Result<NativeCandidateIdentity, NativeScholarlyCandidateError> {
    Ok(NativeCandidateIdentity {
        document_id: carrier.identity.document_id.clone(),
        source_pdf_sha256: carrier.identity.source_pdf_sha256.clone(),
        carrier_digest: carrier
            .digest()
            .map_err(|_| NativeScholarlyCandidateError::Identity)?,
    })
}

type ReferenceSectionHeadings = BTreeMap<(u32, u32), (HeadingMatch, String)>;

/// Whole-line heading metadata is admitted only for the same word carrier.
fn reference_section_headings(
    carrier: &NativeRegionCarrier,
    rows: &[NativeCandidateSourceRow],
    headings: &NativeHeadingRegionOutcome,
) -> Result<ReferenceSectionHeadings, NativeScholarlyCandidateError> {
    let mismatch = |field| NativeScholarlyCandidateError::HeadingEvidenceMismatch { field };
    if carrier.schema_version != NATIVE_REGION_INPUT_SCHEMA_VERSION
        || carrier.identity.schema_version != NATIVE_REGION_INPUT_SCHEMA_VERSION
        || headings.schema_version != NATIVE_HEADING_REGIONS_SCHEMA_VERSION
        || headings.identity.schema_version != NATIVE_HEADING_REGIONS_SCHEMA_VERSION
    {
        return Err(mismatch("schema_version"));
    }
    for (field, carrier_value, heading_value) in [
        (
            "document_id",
            carrier.identity.document_id.as_str(),
            headings.identity.document_id.as_str(),
        ),
        (
            "provider_id",
            carrier.identity.provider_id.as_str(),
            headings.identity.provider_id.as_str(),
        ),
        (
            "source_pdf_sha256",
            carrier.identity.source_pdf_sha256.as_str(),
            headings.identity.source_pdf_sha256.as_str(),
        ),
        (
            "lattice_fingerprint",
            carrier.identity.lattice_fingerprint.as_str(),
            headings.identity.lattice_fingerprint.as_str(),
        ),
        (
            "config_fingerprint",
            carrier.identity.config_fingerprint.as_str(),
            headings.identity.config_fingerprint.as_str(),
        ),
        (
            "order_digest",
            carrier.identity.order_digest.as_str(),
            headings.order_digest.as_str(),
        ),
    ] {
        if carrier_value != heading_value {
            return Err(mismatch(field));
        }
    }
    let mut assignments = BTreeMap::new();
    for assignment in headings
        .assignments
        .iter()
        .filter(|assignment| assignment.region == FullSemanticRegion::References)
    {
        let key = (
            assignment.page_ordinal,
            assignment.source_ordinal,
            assignment.word_ordinal,
        );
        if assignments.insert(key, assignment.line_ordinal).is_some() {
            return Err(mismatch("duplicate_assignment"));
        }
    }
    if assignments.len() != rows.len() {
        return Err(mismatch("assignment_count"));
    }
    let mut line_counts: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for row in rows {
        let key = (
            row.location.page_ordinal,
            row.source_ordinal,
            row.location.token_ordinal,
        );
        if assignments.remove(&key) != Some(row.location.line_ordinal) {
            return Err(mismatch("source_row"));
        }
        *line_counts
            .entry((row.location.page_ordinal, row.location.line_ordinal))
            .or_default() += 1;
    }
    let mut result = BTreeMap::new();
    for line in &headings.lines {
        if line.marker != Some(FullSemanticRegion::References) {
            continue;
        }
        let Some(matched_by @ (HeadingMatch::NormalizedLine | HeadingMatch::OrdinalStrippedLine)) =
            line.matched_by
        else {
            continue;
        };
        let key = (line.page_ordinal, line.line_ordinal);
        if line_counts.get(&key).copied() != Some(line.word_count as usize) {
            return Err(mismatch("heading_word_count"));
        }
        if result
            .insert(key, (matched_by, line.text_digest.clone()))
            .is_some()
        {
            return Err(mismatch("duplicate_heading_line"));
        }
    }
    Ok(result)
}

fn labelled_line_ranges(
    decoded: &ReferenceSegmentSequence,
) -> Result<(Range<usize>, Range<usize>), NativeReferenceLineUnsupported> {
    let label_count = decoded
        .segments
        .iter()
        .filter(|span| span.kind == ReferenceSegmentKind::Label)
        .count();
    if label_count == 0 {
        return Err(NativeReferenceLineUnsupported::NoPrintedLabel);
    }
    if label_count > 1 {
        return Err(NativeReferenceLineUnsupported::MultiplePrintedLabels);
    }
    let label = &decoded.segments[0];
    if label.kind != ReferenceSegmentKind::Label || label.start != 0 {
        return Err(NativeReferenceLineUnsupported::NonLeadingLabel);
    }
    let reference = &decoded.segments[1..];
    if reference.is_empty() {
        return Err(NativeReferenceLineUnsupported::NoReferenceText);
    }
    if reference
        .iter()
        .any(|span| span.kind != ReferenceSegmentKind::Reference)
    {
        return Err(NativeReferenceLineUnsupported::OtherMaterial);
    }
    Ok((label.start..label.end, label.end..decoded.labels.len()))
}

fn printed_arabic_label(text: &str) -> Option<String> {
    let text = text.trim();
    let inner = if let Some(inner) = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        inner
    } else if let Some(inner) = text
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        inner
    } else {
        text.strip_suffix('.').unwrap_or(text)
    };
    (!inner.is_empty() && inner.bytes().all(|byte| byte.is_ascii_digit())).then(|| inner.to_owned())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod coverage_smoke {
    use super::*;

    #[test]
    fn empty_reference_inventory_refuses_linking() {
        let candidate = NativeReferenceCandidate {
            schema_version: NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION,
            identity: NativeCandidateIdentity {
                document_id: "coverage-smoke".to_string(),
                source_pdf_sha256: "a".repeat(64),
                carrier_digest: "coverage-smoke".to_string(),
            },
            reference_feature_digest: String::new(),
            citation_feature_digest: String::new(),
            records: Vec::new(),
            lines: Vec::new(),
        };
        assert!(!candidate.has_complete_labelled_line_inventory());
        assert!(matches!(
            link_native_reference_markers("plain body without markers", &candidate),
            Err(NativeScholarlyCandidateError::ReferenceInventoryIncomplete)
        ));
    }
}
