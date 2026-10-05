//! Pure, bounded native TEI emission from the native shadow plane.
//!
//! This module is deliberately an *emission* boundary, not a parser and not a
//! runtime route.  It assembles a complete TEI document
//! (`<?xml …?><TEI xmlns="http://www.tei-c.org/ns/1.0">…</TEI>`) from the typed
//! native components the shadow plane already produces — the native header
//! [`HeaderCompositionOutcome`], the native [`ReferenceCompositionOutcome`] and
//! (S02/T02) a native body input — and never from preserved Java TEI.  The
//! existing fulltext composer ([`crate::compose_native_fulltext`]) is the
//! opposite object: it refuses anything whose base envelope is not the Java
//! GROBID HTTP backend and splices native bands into preserved Java bytes.  This
//! module has **no Java byte source at all** and consumes no `TeiEnvelope`; it
//! only produces one.
//!
//! Serialization is shared verbatim with the composer through
//! [`render_header_fragment`] / [`render_bibliography_fragment`] and the
//! post-serialization gates reuse [`validate_emitted`] and
//! [`audit_semantic_fields`], so the two emitters can never drift into two
//! serializations of the same admitted semantics.
//!
//! Family scope is explicit and closed.  Every [`NativeTeiFamily`] carries
//! exactly one disposition in the manifest: `native_emitted` when this emission
//! produced it from the native plane, `typed_absence` otherwise (with a closed
//! [`NativeTeiAbsenceReason`]).  A family this emission produced may never be
//! republished as an absence, and a typed absence contributes **zero** bytes:
//! there is no Java fallback to fall back to.
//!
//! Locks (D127/ADR-076 §14): `import_eligible=false`, `auto_promote=forbidden`,
//! `deployed=false`, `graph_write=forbidden`, `production_route=forbidden`.
//! The emitter is a shadow/diagnostic surface, not a runtime route.

use crate::native_fulltext_composition::{
    AdmittedSemantics, HeaderFragmentSource, NativeFulltextCompositionError, audit_semantic_fields,
    citation_semantics, digest, esc, header_semantics, render_bibliography_fragment,
    render_header_fragment, representable_text, valid_sha, validate_emitted,
};
use crate::{
    CompositionError, FulltextCompositionIdentity, HeaderCompositionOutcome, NativeReference,
    ReferenceCompositionOutcome, ReferenceLinkLedger, SemanticFieldLedger, TeiEnvelope,
    TeiIdentity,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const NATIVE_TEI_EMISSION_SCHEMA_VERSION: &str = "da-parse.native-tei-emission/v1";
/// Backend identity of an envelope produced by this module.  Deliberately not
/// [`crate::GROBID_HTTP_BACKEND_ID`]: the emission has no Java byte source.
pub const NATIVE_TEI_EMISSION_BACKEND_ID: &str = "native-tei-emission";
pub const NATIVE_TEI_EMISSION_TEI_NAMESPACE: &str = "http://www.tei-c.org/ns/1.0";
pub const NATIVE_TEI_EMISSION_DEFAULT_MAX_ENVELOPE_BYTES: usize = 32 * 1024 * 1024;
pub const NATIVE_TEI_EMISSION_DEFAULT_MAX_FRAGMENT_BYTES: usize = 8 * 1024 * 1024;
pub const NATIVE_TEI_EMISSION_DEFAULT_MAX_ABSENCES: usize = 64;

/// The closed set of families this emission accounts for.
///
/// Granularity is emission-level (one serialized band per family), not the
/// route-measurement granularity of
/// [`crate::NativeRouteFamily`] (which splits header and bibliography into
/// separate leaf families).  Every family receives exactly one disposition in
/// the manifest; the set is closed and never abbreviated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTeiFamily {
    Header,
    References,
    Body,
    Figure,
    Table,
    Formula,
    Visual,
    Callout,
}

impl NativeTeiFamily {
    /// Every family, in manifest order.
    pub const ALL: [NativeTeiFamily; 8] = [
        NativeTeiFamily::Header,
        NativeTeiFamily::References,
        NativeTeiFamily::Body,
        NativeTeiFamily::Figure,
        NativeTeiFamily::Table,
        NativeTeiFamily::Formula,
        NativeTeiFamily::Visual,
        NativeTeiFamily::Callout,
    ];

    /// Stable token, closed by the exhaustive match.
    pub const fn token(self) -> &'static str {
        match self {
            NativeTeiFamily::Header => "header",
            NativeTeiFamily::References => "references",
            NativeTeiFamily::Body => "body",
            NativeTeiFamily::Figure => "figure",
            NativeTeiFamily::Table => "table",
            NativeTeiFamily::Formula => "formula",
            NativeTeiFamily::Visual => "visual",
            NativeTeiFamily::Callout => "callout",
        }
    }
}

impl fmt::Display for NativeTeiFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Why a non-emitted family is a typed absence.  Closed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTeiAbsenceReason {
    /// The slice does not emit this family yet.
    NotEmittedBySlice,
    /// The route produced no native input for this family.
    NoNativeInput,
    /// The native plane has no implementation for this family.
    NativeFamilyUnavailable,
    /// The native plane ran but admitted no token for this family.
    NoAdmittedTokens,
}

impl NativeTeiAbsenceReason {
    /// Stable token, closed by the exhaustive match.
    pub const fn token(self) -> &'static str {
        match self {
            NativeTeiAbsenceReason::NotEmittedBySlice => "not_emitted_by_slice",
            NativeTeiAbsenceReason::NoNativeInput => "no_native_input",
            NativeTeiAbsenceReason::NativeFamilyUnavailable => "native_family_unavailable",
            NativeTeiAbsenceReason::NoAdmittedTokens => "no_admitted_tokens",
        }
    }
}

/// A typed declaration that a family is deliberately not emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiAbsence {
    pub family: NativeTeiFamily,
    pub reason: NativeTeiAbsenceReason,
}

/// The two dispositions a family may carry in an emission manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTeiDisposition {
    NativeEmitted,
    TypedAbsence,
}

/// One family's disposition in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiFamilyDisposition {
    pub family: NativeTeiFamily,
    pub disposition: NativeTeiDisposition,
    /// Present iff `disposition == TypedAbsence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<NativeTeiAbsenceReason>,
}

/// Byte accounting for the emission.
///
/// `native_emitted_bytes` is the sum of the native fragment lengths — the only
/// bytes that partition the emission's content.  `typed_absence_bytes` is always
/// `0`: a typed absence emits nothing and has no Java fallback.
/// `java_fallback_bytes` is always `0`: this emission has no Java byte source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiByteAccounting {
    pub header_bytes: usize,
    pub references_bytes: usize,
    pub body_bytes: usize,
    pub native_emitted_bytes: usize,
    pub typed_absence_bytes: usize,
    pub envelope_bytes: usize,
    pub java_fallback_bytes: usize,
}

/// Native body material.
///
/// S02/T01 reserves this input slot and serializes the joined native body text
/// as XML character data; S02/T02 extends the same module with the section /
/// paragraph reading-order serialization and the per-leaf typed refusals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiBodyInput {
    /// The joined native body text.
    pub text: String,
}

/// Bounded emission limits, resolved by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiEmissionLimits {
    pub max_envelope_bytes: usize,
    pub max_fragment_bytes: usize,
    pub max_absences: usize,
}

impl Default for NativeTeiEmissionLimits {
    fn default() -> Self {
        Self {
            max_envelope_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_ENVELOPE_BYTES,
            max_fragment_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_FRAGMENT_BYTES,
            max_absences: NATIVE_TEI_EMISSION_DEFAULT_MAX_ABSENCES,
        }
    }
}

impl NativeTeiEmissionLimits {
    pub const fn defaults() -> Self {
        Self {
            max_envelope_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_ENVELOPE_BYTES,
            max_fragment_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_FRAGMENT_BYTES,
            max_absences: NATIVE_TEI_EMISSION_DEFAULT_MAX_ABSENCES,
        }
    }
}

/// The complete typed input of a native emission.  No Java bytes anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiEmissionInput {
    pub identity: FulltextCompositionIdentity,
    pub header: HeaderCompositionOutcome,
    pub references: ReferenceCompositionOutcome,
    /// Native body material; `None` records `Body` as a typed absence.
    pub body: Option<NativeTeiBodyInput>,
    /// Declared typed absences for non-emitted families.
    pub absences: Vec<NativeTeiAbsence>,
    pub limits: NativeTeiEmissionLimits,
}

/// Why a native emission could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTeiEmissionError {
    InvalidIdentity(&'static str),
    /// A lane that must be admitted arrived rejected.
    InvalidComposition {
        lane: NativeTeiFamily,
        error: CompositionError,
    },
    /// A family this emission produced was repeated in `absences`.
    AdmittedFamilyListedAsAbsence(NativeTeiFamily),
    /// The same non-emitted family was declared more than once.
    DuplicateAbsence(NativeTeiFamily),
    TooManyAbsences {
        count: usize,
        max: usize,
    },
    /// An admitted value cannot be carried into XML.
    UnrepresentableInput {
        family: NativeTeiFamily,
        detail: String,
    },
    /// A native reference does not sit at its own ordinal, so the positional
    /// `b{ordinal}` key domain would not be the published order.
    NonSequentialReferenceOrdinal {
        ordinal: usize,
        position: usize,
    },
    Oversized {
        part: &'static str,
        bytes: usize,
        max: usize,
    },
    /// A shared serialization / semantic gate refused the fragment.
    Fragment(NativeFulltextCompositionError),
    /// The composed bytes failed the pre-acceptance structural gate.
    EmissionNotWellFormed(String),
}

impl fmt::Display for NativeTeiEmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native TEI emission rejected input: {self:?}")
    }
}

impl std::error::Error for NativeTeiEmissionError {}

impl From<NativeFulltextCompositionError> for NativeTeiEmissionError {
    fn from(error: NativeFulltextCompositionError) -> Self {
        NativeTeiEmissionError::Fragment(error)
    }
}

/// The observable record of one native emission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiEmissionManifest {
    pub schema_version: String,
    pub identity: FulltextCompositionIdentity,
    /// `sha256:…` of the emitted envelope bytes.
    pub emission_sha256: String,
    /// One disposition per [`NativeTeiFamily`], in [`NativeTeiFamily::ALL`] order.
    pub families: Vec<NativeTeiFamilyDisposition>,
    pub byte_accounting: NativeTeiByteAccounting,
    /// `ReferenceLinkLedger` reused verbatim; the native bibliography publishes
    /// the positional `b{ordinal}` key domain.
    pub reference_links: ReferenceLinkLedger,
    /// `SemanticFieldLedger` reused verbatim; recomputed from the emitted bytes.
    pub semantic_fields: Box<SemanticFieldLedger>,
}

/// A pure native emission outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTeiEmissionOutcome {
    Accepted {
        envelope: TeiEnvelope,
        manifest: Box<NativeTeiEmissionManifest>,
    },
    Rejected {
        error: NativeTeiEmissionError,
    },
}

/// Emit a complete TEI document from the native plane alone.
pub fn emit_native_tei(input: NativeTeiEmissionInput) -> NativeTeiEmissionOutcome {
    match emit_inner(input) {
        Ok((envelope, manifest)) => NativeTeiEmissionOutcome::Accepted {
            envelope,
            manifest: Box::new(manifest),
        },
        Err(error) => NativeTeiEmissionOutcome::Rejected { error },
    }
}

fn emit_inner(
    input: NativeTeiEmissionInput,
) -> Result<(TeiEnvelope, NativeTeiEmissionManifest), NativeTeiEmissionError> {
    let identity = input.identity.clone();
    validate_identity(&identity)?;
    if input.absences.len() > input.limits.max_absences {
        return Err(NativeTeiEmissionError::TooManyAbsences {
            count: input.absences.len(),
            max: input.limits.max_absences,
        });
    }

    // Which families this emission produces natively.
    let mut emitted: BTreeSet<NativeTeiFamily> = BTreeSet::new();
    emitted.insert(NativeTeiFamily::Header);
    emitted.insert(NativeTeiFamily::References);
    if input.body.is_some() {
        emitted.insert(NativeTeiFamily::Body);
    }

    // Closed absence vocabulary: one entry per non-emitted family, duplicates
    // refused, and a family this emission produced may never be named absent.
    let mut absence_reasons: BTreeMap<NativeTeiFamily, NativeTeiAbsenceReason> = BTreeMap::new();
    for absence in &input.absences {
        if emitted.contains(&absence.family) {
            return Err(NativeTeiEmissionError::AdmittedFamilyListedAsAbsence(
                absence.family,
            ));
        }
        if absence_reasons
            .insert(absence.family, absence.reason)
            .is_some()
        {
            return Err(NativeTeiEmissionError::DuplicateAbsence(absence.family));
        }
    }

    // Both admitted lanes must carry an Accepted composition.  A rejected lane
    // is a typed refusal, never a fallback to Java.
    let header = match &input.header {
        HeaderCompositionOutcome::Accepted { composition } => composition,
        HeaderCompositionOutcome::Rejected { error } => {
            return Err(NativeTeiEmissionError::InvalidComposition {
                lane: NativeTeiFamily::Header,
                error: error.clone(),
            });
        }
    };
    let references = match &input.references {
        ReferenceCompositionOutcome::Accepted { composition } => composition,
        ReferenceCompositionOutcome::Rejected { error } => {
            return Err(NativeTeiEmissionError::InvalidComposition {
                lane: NativeTeiFamily::References,
                error: error.clone(),
            });
        }
    };

    // Validate the admitted semantics before serializing anything (reuses the
    // composer's own validators; a non-representable value is a typed refusal).
    let admitted = AdmittedSemantics {
        header: header_semantics(&header.candidate)?,
        citations: citation_semantics(&references.candidate)?,
    };

    // The native bibliography publishes GROBID's positional fallback key domain:
    // no Java source exists to declare `xml:id` keys, and this is the same form
    // the composer's `source_published_keys` publishes for an undeclared entry.
    let keys = positional_reference_keys(&references.candidate.references)?;

    // Native body text (S02/T02 extends this into section structure).
    let body_text = match &input.body {
        Some(body) => {
            if !representable_text(&body.text) {
                return Err(NativeTeiEmissionError::UnrepresentableInput {
                    family: NativeTeiFamily::Body,
                    detail: "native body text cannot be carried into XML character data".into(),
                });
            }
            body.text.as_str()
        }
        None => "",
    };

    // Serialize each band through the shared builders, so this emitter and the
    // composer can never disagree on one serialization of the same semantics.
    let header_fragment = render_header_fragment(header, HeaderFragmentSource::native());
    let bibliography_fragment = render_bibliography_fragment(
        &keys,
        &references.candidate.references,
        &references.candidate.fields,
    );
    let body_escaped = esc(body_text);

    for (part, bytes) in [
        ("header_fragment", header_fragment.len()),
        ("references_fragment", bibliography_fragment.len()),
        ("body_fragment", body_escaped.len()),
    ] {
        if bytes > input.limits.max_fragment_bytes {
            return Err(NativeTeiEmissionError::Oversized {
                part,
                bytes,
                max: input.limits.max_fragment_bytes,
            });
        }
    }

    // Assemble the base document from the native plane.  There is no preserved
    // Java envelope anywhere in this path and no Java fallback.
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    bytes.extend_from_slice(b"<TEI xmlns=\"");
    bytes.extend_from_slice(NATIVE_TEI_EMISSION_TEI_NAMESPACE.as_bytes());
    bytes.extend_from_slice(b"\">");
    bytes.extend_from_slice(header_fragment.as_bytes());
    bytes.extend_from_slice(b"<text><body>");
    bytes.extend_from_slice(body_escaped.as_bytes());
    bytes.extend_from_slice(b"</body><back><div type=\"references\">");
    bytes.extend_from_slice(bibliography_fragment.as_bytes());
    bytes.extend_from_slice(b"</div></back></text></TEI>");

    if bytes.len() > input.limits.max_envelope_bytes {
        return Err(NativeTeiEmissionError::Oversized {
            part: "envelope",
            bytes: bytes.len(),
            max: input.limits.max_envelope_bytes,
        });
    }

    // `Accepted` is a claim about the bytes returned: re-read them first through
    // the shared structural + well-formedness gate, then through the shared
    // semantic-conservation gate.
    let document = validate_emitted(&bytes)?;
    let emitted_text = std::str::from_utf8(&bytes).map_err(|_| {
        NativeTeiEmissionError::EmissionNotWellFormed("emission is not valid UTF-8".into())
    })?;
    let semantic_fields = audit_semantic_fields(&document, emitted_text, &admitted)?;

    let reference_links = ReferenceLinkLedger {
        source_entries: keys.len(),
        admitted_references: references.candidate.references.len(),
        // The section/paragraph citation refs are S02/T02's surface; T01 emits
        // no body `ref` elements, so the demand is empty and fully resolved.
        target_occurrences: 0,
        required_targets: 0,
        resolved_targets: 0,
        id_space: document.xml_id_count(),
    };

    let families = family_dispositions(&emitted, &absence_reasons);
    let byte_accounting = NativeTeiByteAccounting {
        header_bytes: header_fragment.len(),
        references_bytes: bibliography_fragment.len(),
        body_bytes: body_escaped.len(),
        native_emitted_bytes: header_fragment.len()
            + bibliography_fragment.len()
            + body_escaped.len(),
        typed_absence_bytes: 0,
        envelope_bytes: bytes.len(),
        java_fallback_bytes: 0,
    };

    let envelope = TeiEnvelope::new(
        bytes.clone(),
        TeiIdentity::new(
            NATIVE_TEI_EMISSION_BACKEND_ID,
            identity.profile_fingerprint.clone(),
        ),
    );
    let manifest = NativeTeiEmissionManifest {
        schema_version: NATIVE_TEI_EMISSION_SCHEMA_VERSION.into(),
        identity,
        emission_sha256: digest(&bytes),
        families,
        byte_accounting,
        reference_links,
        semantic_fields: Box::new(semantic_fields),
    };
    Ok((envelope, manifest))
}

fn validate_identity(identity: &FulltextCompositionIdentity) -> Result<(), NativeTeiEmissionError> {
    if !valid_sha(&identity.source_sha256)
        || identity.profile_fingerprint.is_empty()
        || identity.config_fingerprint.is_empty()
    {
        return Err(NativeTeiEmissionError::InvalidIdentity(
            "source/profile/config",
        ));
    }
    Ok(())
}

/// The native bibliography's own published key domain: one positional
/// `b{ordinal}` key per reference, in reference order.
///
/// This is the same fallback text
/// [`crate::NATIVE_FULLTEXT_REFERENCE_LINK_POSITIONAL_FALLBACK`] declares and the
/// composer publishes for an undeclared entry.  A reference that does not sit at
/// its own ordinal is refused: the positional domain would then not be the
/// published order.
fn positional_reference_keys(
    references: &[NativeReference],
) -> Result<Vec<String>, NativeTeiEmissionError> {
    let mut keys = Vec::with_capacity(references.len());
    for (position, reference) in references.iter().enumerate() {
        if reference.ordinal != position {
            return Err(NativeTeiEmissionError::NonSequentialReferenceOrdinal {
                ordinal: reference.ordinal,
                position,
            });
        }
        keys.push(format!("b{position}"));
    }
    Ok(keys)
}

/// One disposition per family, in [`NativeTeiFamily::ALL`] order.
fn family_dispositions(
    emitted: &BTreeSet<NativeTeiFamily>,
    absence_reasons: &BTreeMap<NativeTeiFamily, NativeTeiAbsenceReason>,
) -> Vec<NativeTeiFamilyDisposition> {
    NativeTeiFamily::ALL
        .iter()
        .map(|family| {
            if emitted.contains(family) {
                NativeTeiFamilyDisposition {
                    family: *family,
                    disposition: NativeTeiDisposition::NativeEmitted,
                    reason: None,
                }
            } else {
                NativeTeiFamilyDisposition {
                    family: *family,
                    disposition: NativeTeiDisposition::TypedAbsence,
                    reason: Some(
                        absence_reasons
                            .get(family)
                            .copied()
                            .unwrap_or(NativeTeiAbsenceReason::NotEmittedBySlice),
                    ),
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffiliationSpan, CitationField, CitationFieldRole, CitationSpan, ComponentDisposition,
        ComponentReceipt, CompositionComponent, CompositionSpan, HeaderAuthor, HeaderCandidate,
        HeaderCompositionCandidate, NativeAffiliationLink, NativeHeaderFields, RecognizedDate,
        RecognizedPersonalName, ReferenceCandidate, ReferenceCompositionCandidate,
    };

    fn identity() -> FulltextCompositionIdentity {
        FulltextCompositionIdentity {
            source_sha256: "a".repeat(64),
            profile_fingerprint: "profile=structural;consolidateHeader=0".into(),
            config_fingerprint: "config=native-tei-emission-test".into(),
        }
    }

    fn header_outcome() -> HeaderCompositionOutcome {
        let date = RecognizedDate::new(2024, Some(5), Some(1)).expect("valid date");
        let name = RecognizedPersonalName {
            given_name: Some("Ada".into()),
            middle_names: vec![],
            surname: "Lovelace".into(),
            recomposed_surname: "Lovelace".into(),
            surname_particles: vec![],
            suffix: None,
            initials: vec![],
        };
        let author = HeaderAuthor {
            ordinal: 0,
            span: CompositionSpan { start: 0, end: 11 },
            name,
            affiliations: vec![NativeAffiliationLink {
                key: "aff-0".into(),
                affiliation_ordinal: 0,
                affiliation_span: AffiliationSpan { start: 0, end: 12 },
            }],
        };
        HeaderCompositionOutcome::Accepted {
            composition: HeaderCompositionCandidate {
                candidate: HeaderCandidate {
                    date,
                    authors: vec![author],
                    fields: NativeHeaderFields {
                        title: vec!["A Native Title".into()],
                        email: vec!["ada@example.org".into()],
                        ids: vec!["arXiv:2507.19457".into()],
                    },
                },
                receipts: vec![ComponentReceipt {
                    component: CompositionComponent::Names,
                    disposition: ComponentDisposition::Accepted,
                }],
            },
        }
    }

    fn reference_outcome() -> ReferenceCompositionOutcome {
        let references = vec![NativeReference {
            ordinal: 0,
            source: "Lovelace, A. (1843). Notes on the Engine.".into(),
            span: CitationSpan {
                start: 0,
                end: "Lovelace, A. (1843). Notes on the Engine.".len(),
            },
            provenance: "unit-test".into(),
        }];
        let fields = vec![CitationField {
            role: CitationFieldRole::ArticleTitle,
            value: "Notes on the Engine".into(),
            span: CitationSpan { start: 0, end: 19 },
        }];
        ReferenceCompositionOutcome::Accepted {
            composition: ReferenceCompositionCandidate {
                candidate: ReferenceCandidate {
                    references,
                    fields,
                    anchors: vec![],
                    adjacent_pairs: vec![],
                },
                receipts: vec![],
            },
        }
    }

    fn base_input() -> NativeTeiEmissionInput {
        NativeTeiEmissionInput {
            identity: identity(),
            header: header_outcome(),
            references: reference_outcome(),
            body: None,
            absences: vec![],
            limits: NativeTeiEmissionLimits::default(),
        }
    }

    fn accepted(input: NativeTeiEmissionInput) -> (TeiEnvelope, NativeTeiEmissionManifest) {
        match emit_native_tei(input) {
            NativeTeiEmissionOutcome::Accepted { envelope, manifest } => (envelope, *manifest),
            other => panic!("expected an accepted emission, got {other:?}"),
        }
    }

    fn family(
        manifest: &NativeTeiEmissionManifest,
        family: NativeTeiFamily,
    ) -> &NativeTeiFamilyDisposition {
        manifest
            .families
            .iter()
            .find(|disposition| disposition.family == family)
            .expect("family disposition present")
    }

    #[test]
    fn emits_well_formed_tei_with_native_header_and_bibliography() {
        let (envelope, manifest) = accepted(base_input());
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(text.contains("<TEI xmlns=\"http://www.tei-c.org/ns/1.0\">"));
        assert!(text.contains("<teiHeader>"));
        assert!(text.contains("<title>A Native Title</title>"));
        assert!(text.contains("<email>ada@example.org</email>"));
        assert!(text.contains("<idno type=\"pubnum\">arXiv:2507.19457</idno>"));
        assert!(text.contains("<author xml:id=\"author-0\">"));
        assert!(text.contains("<affiliation key=\"aff-0\"/>"));
        assert!(text.contains("<date when=\"2024-05-01\"/>"));
        assert!(text.contains("<div type=\"references\">"));
        assert!(text.contains("<listBibl>"));
        assert!(text.contains("<biblStruct xml:id=\"b0\">"));
        assert!(text.contains("Lovelace, A. (1843). Notes on the Engine."));
        assert!(text.contains("<note role=\"article_title\">Notes on the Engine</note>"));
        assert!(text.ends_with("</TEI>"));

        assert_eq!(manifest.schema_version, NATIVE_TEI_EMISSION_SCHEMA_VERSION);
        assert_eq!(manifest.emission_sha256, digest(envelope.raw_tei()));
        assert_eq!(manifest.identity, identity());
        assert_eq!(manifest.families.len(), NativeTeiFamily::ALL.len());
        assert_eq!(manifest.semantic_fields.admitted_header_titles, 1);
        assert_eq!(manifest.semantic_fields.conserved_header_titles, 1);
        assert_eq!(manifest.semantic_fields.admitted_header_emails, 1);
        assert_eq!(manifest.semantic_fields.conserved_header_emails, 1);
        assert_eq!(manifest.semantic_fields.admitted_author_affiliations, 1);
        assert_eq!(manifest.semantic_fields.conserved_author_affiliations, 1);
        assert_eq!(manifest.semantic_fields.admitted_citation_fields, 1);
        assert_eq!(manifest.semantic_fields.conserved_citation_fields, 1);
        assert_eq!(manifest.reference_links.source_entries, 1);
        assert_eq!(manifest.reference_links.admitted_references, 1);
        assert_eq!(manifest.reference_links.id_space, 2);

        // Both admitted lanes are native-emitted; the untouched families are
        // typed absence, and a typed absence contributes no bytes.
        assert_eq!(
            family(&manifest, NativeTeiFamily::Header).disposition,
            NativeTeiDisposition::NativeEmitted
        );
        assert_eq!(
            family(&manifest, NativeTeiFamily::References).disposition,
            NativeTeiDisposition::NativeEmitted
        );
        assert_eq!(
            family(&manifest, NativeTeiFamily::Body).disposition,
            NativeTeiDisposition::TypedAbsence
        );
        assert_eq!(
            family(&manifest, NativeTeiFamily::Figure).disposition,
            NativeTeiDisposition::TypedAbsence
        );
        assert_eq!(manifest.byte_accounting.typed_absence_bytes, 0);
        assert_eq!(manifest.byte_accounting.java_fallback_bytes, 0);
        assert_eq!(
            manifest.byte_accounting.native_emitted_bytes,
            manifest.byte_accounting.header_bytes
                + manifest.byte_accounting.references_bytes
                + manifest.byte_accounting.body_bytes
        );
        assert_eq!(
            manifest.byte_accounting.envelope_bytes,
            envelope.raw_tei().len()
        );
    }

    #[test]
    fn emission_carries_no_java_backend_or_preserved_oracle_bytes() {
        let (envelope, _) = accepted(base_input());
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(!text.contains(crate::GROBID_HTTP_BACKEND_ID));
        assert!(!text.contains("java-grobid-crf"));
        assert!(!text.contains("xml:space=\"preserve\""));
        assert_eq!(
            envelope.identity().backend_id(),
            NATIVE_TEI_EMISSION_BACKEND_ID
        );
    }

    #[test]
    fn admitted_family_listed_as_absence_is_refused() {
        let mut input = base_input();
        input.absences = vec![NativeTeiAbsence {
            family: NativeTeiFamily::Header,
            reason: NativeTeiAbsenceReason::NoNativeInput,
        }];
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::AdmittedFamilyListedAsAbsence(
                    NativeTeiFamily::Header
                )
            }
        ));
    }

    #[test]
    fn duplicate_absence_is_refused() {
        let mut input = base_input();
        input.absences = vec![
            NativeTeiAbsence {
                family: NativeTeiFamily::Figure,
                reason: NativeTeiAbsenceReason::NativeFamilyUnavailable,
            },
            NativeTeiAbsence {
                family: NativeTeiFamily::Figure,
                reason: NativeTeiAbsenceReason::NoNativeInput,
            },
        ];
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::DuplicateAbsence(NativeTeiFamily::Figure)
            }
        ));
    }

    #[test]
    fn declared_absence_reason_is_recorded() {
        let mut input = base_input();
        input.absences = vec![NativeTeiAbsence {
            family: NativeTeiFamily::Figure,
            reason: NativeTeiAbsenceReason::NativeFamilyUnavailable,
        }];
        let (_, manifest) = accepted(input);
        let figure = family(&manifest, NativeTeiFamily::Figure);
        assert_eq!(figure.disposition, NativeTeiDisposition::TypedAbsence);
        assert_eq!(
            figure.reason,
            Some(NativeTeiAbsenceReason::NativeFamilyUnavailable)
        );
    }

    #[test]
    fn body_input_is_native_emitted_and_escaped() {
        let mut input = base_input();
        input.body = Some(NativeTeiBodyInput {
            text: "Body <text> & more".into(),
        });
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<body>Body &lt;text&gt; &amp; more</body>"));
        assert_eq!(
            family(&manifest, NativeTeiFamily::Body).disposition,
            NativeTeiDisposition::NativeEmitted
        );
        assert_eq!(manifest.byte_accounting.java_fallback_bytes, 0);
    }

    #[test]
    fn rejected_header_lane_is_refused() {
        let mut input = base_input();
        input.header = HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Date,
            },
        };
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::InvalidComposition {
                    lane: NativeTeiFamily::Header,
                    ..
                }
            }
        ));
    }

    #[test]
    fn non_sequential_reference_ordinal_is_refused() {
        let mut input = base_input();
        let ReferenceCompositionOutcome::Accepted { composition } = &mut input.references else {
            panic!("reference lane is accepted");
        };
        composition.candidate.references[0].ordinal = 7;
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::NonSequentialReferenceOrdinal { .. }
            }
        ));
    }

    #[test]
    fn unrepresentable_body_text_is_refused() {
        let mut input = base_input();
        input.body = Some(NativeTeiBodyInput {
            text: "carriage\rreturn".into(),
        });
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::UnrepresentableInput {
                    family: NativeTeiFamily::Body,
                    ..
                }
            }
        ));
    }

    #[test]
    fn unrepresentable_header_field_is_refused() {
        let mut input = base_input();
        let HeaderCompositionOutcome::Accepted { composition } = &mut input.header else {
            panic!("header lane is accepted");
        };
        composition.candidate.fields.title = vec!["bad\u{0}value".into()];
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::Fragment(_)
            }
        ));
    }

    #[test]
    fn invalid_identity_is_refused() {
        let mut input = base_input();
        input.identity.source_sha256 = "not-a-sha".into();
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::InvalidIdentity(_)
            }
        ));
    }

    #[test]
    fn family_tokens_are_closed_and_unique() {
        let mut tokens: Vec<&str> = NativeTeiFamily::ALL.iter().map(|f| f.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), NativeTeiFamily::ALL.len());
        let mut reasons: Vec<&str> = [
            NativeTeiAbsenceReason::NotEmittedBySlice,
            NativeTeiAbsenceReason::NoNativeInput,
            NativeTeiAbsenceReason::NativeFamilyUnavailable,
            NativeTeiAbsenceReason::NoAdmittedTokens,
        ]
        .iter()
        .map(|r| r.token())
        .collect();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), 4);
    }
}
