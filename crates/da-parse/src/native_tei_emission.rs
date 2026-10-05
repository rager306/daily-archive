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
    AdmittedHeaderSemantics, AdmittedSemantics, HeaderFragmentSource,
    NativeFulltextCompositionError, audit_semantic_fields, citation_semantics, digest, esc,
    header_semantics, render_bibliography_fragment, render_header_fragment,
    representable_attribute, representable_text, valid_sha, validate_emitted,
};
use crate::{
    CompositionError, FulltextCompositionIdentity, FulltextLeafRole, HeaderCompositionOutcome,
    MAX_NATIVE_SECTION_DEPTH, MarkerSubtype, NativeBodyText, NativeReference, NoteDisposition,
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
///
/// S02/T02 pairs every band's byte count with a digest computed from the very
/// bytes that were serialized (`*_sha256`), so the accounting is derived from
/// the components rather than asserted about them.  A typed absence carries no
/// content bytes, so its digest is taken over the declared absence declaration
/// instead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiByteAccounting {
    pub header_bytes: usize,
    pub references_bytes: usize,
    pub body_bytes: usize,
    pub native_emitted_bytes: usize,
    pub typed_absence_bytes: usize,
    pub envelope_bytes: usize,
    pub java_fallback_bytes: usize,
    /// `sha256:…` of the serialized header fragment.
    #[serde(default)]
    pub header_sha256: String,
    /// `sha256:…` of the serialized bibliography fragment.
    #[serde(default)]
    pub references_sha256: String,
    /// `sha256:…` of the serialized body fragment.
    #[serde(default)]
    pub body_sha256: String,
    /// `sha256:…` of the declared typed-absence declaration (family/reason
    /// pairs in [`NativeTeiFamily::ALL`] order).  No content bytes, so the
    /// digest stands in for the band.
    #[serde(default)]
    pub typed_absence_sha256: String,
}

/// One native body leaf in reading order.
///
/// The role vocabulary is [`FulltextLeafRole`] and the marker subtype is
/// [`MarkerSubtype`] — the same closed vocabularies the block plane publishes —
/// so this input surface does not invent a second leaf taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiBodyLeaf {
    /// The leaf's admitted text.  A prose leaf must carry text; an empty prose
    /// leaf is a typed refusal, never a silent empty `<p>`.  A marker may be
    /// empty (an empty `<ref>` or a typed absence).
    pub text: String,
    pub role: FulltextLeafRole,
    /// Present iff `role == Marker`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_subtype: Option<MarkerSubtype>,
    /// The published bibliography key a citation marker targets.  Present only
    /// when the route resolved it against the native bibliography's published
    /// `b{ordinal}` domain; a citation marker without a key is plain text, and a
    /// key is never synthesized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_key: Option<String>,
}

/// One native section in reading order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiBodySection {
    /// The section id from the native section plane (`section_id`), when the
    /// plane named one.  Emitted as `xml:id` only when present and representable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// The section depth from the heading's leading ordinal.
    pub depth: u32,
    /// The parent section id, when the section is nested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_section_id: Option<String>,
    /// The ordered leaves of this section.
    pub leaves: Vec<NativeTeiBodyLeaf>,
}

/// A body-plane typed absence: a leaf the native plane cannot emit, recorded by
/// name instead of silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiBodyAbsence {
    pub role: FulltextLeafRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_subtype: Option<MarkerSubtype>,
    pub reason: NativeTeiAbsenceReason,
}

/// Native body material: ordered sections, each with ordered leaves.
///
/// S02/T01 serialized one joined body text as XML character data.  S02/T02
/// replaces that with the structured reading order the route plane already
/// produces: a `<div type="section">` per native section, a `<p>` per prose
/// leaf, a `<head>` per heading leaf and a `<ref type="bibr">` per resolved
/// citation marker.  The joined text is reached through
/// [`NativeTeiBodyInput::from_body_text`] (`NativeBodyText::text()`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiBodyInput {
    pub sections: Vec<NativeTeiBodySection>,
    /// The native note disposition from the block plane
    /// ([`NoteDisposition`]).  Notes are not emitted by this slice: a note leaf
    /// that carries text is refused, an empty one is a typed absence.
    #[serde(default = "default_note_disposition")]
    pub note_disposition: NoteDisposition,
}

fn default_note_disposition() -> NoteDisposition {
    NoteDisposition::SourceNotEstablished
}

impl NativeTeiBodyInput {
    /// A body with no section reading order: one unnamed section holding one
    /// paragraph.  `text` is meant to be `NativeBodyText::text()`.
    pub fn single_paragraph(text: impl Into<String>) -> Self {
        Self {
            sections: vec![NativeTeiBodySection {
                section_id: None,
                depth: 1,
                parent_section_id: None,
                leaves: vec![NativeTeiBodyLeaf {
                    text: text.into(),
                    role: FulltextLeafRole::Paragraph,
                    marker_subtype: None,
                    published_key: None,
                }],
            }],
            note_disposition: NoteDisposition::SourceNotEstablished,
        }
    }

    /// The joined native body text, emitted unchanged as one paragraph.  The
    /// native body plane exposes its text only through `text()`; taking the
    /// borrow here is what keeps the emission's body content the admitted one.
    pub fn from_body_text(body: &NativeBodyText) -> Self {
        Self::single_paragraph(body.text())
    }
}

/// Bounded emission limits, resolved by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTeiEmissionLimits {
    pub max_envelope_bytes: usize,
    pub max_fragment_bytes: usize,
    pub max_absences: usize,
    #[serde(default = "default_max_body_sections")]
    pub max_body_sections: usize,
    #[serde(default = "default_max_body_leaves")]
    pub max_body_leaves: usize,
}

/// Mirrors the native section plane's own [`MAX_NATIVE_SECTION_RUNS`].
pub const NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_SECTIONS: usize = 16_384;
/// Mirrors the block plane's own `MAX_FULLTEXT_BLOCKS`.
pub const NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_LEAVES: usize = 100_000;

fn default_max_body_sections() -> usize {
    NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_SECTIONS
}

fn default_max_body_leaves() -> usize {
    NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_LEAVES
}

impl Default for NativeTeiEmissionLimits {
    fn default() -> Self {
        Self {
            max_envelope_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_ENVELOPE_BYTES,
            max_fragment_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_FRAGMENT_BYTES,
            max_absences: NATIVE_TEI_EMISSION_DEFAULT_MAX_ABSENCES,
            max_body_sections: NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_SECTIONS,
            max_body_leaves: NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_LEAVES,
        }
    }
}

impl NativeTeiEmissionLimits {
    pub const fn defaults() -> Self {
        Self {
            max_envelope_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_ENVELOPE_BYTES,
            max_fragment_bytes: NATIVE_TEI_EMISSION_DEFAULT_MAX_FRAGMENT_BYTES,
            max_absences: NATIVE_TEI_EMISSION_DEFAULT_MAX_ABSENCES,
            max_body_sections: NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_SECTIONS,
            max_body_leaves: NATIVE_TEI_EMISSION_DEFAULT_MAX_BODY_LEAVES,
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
    /// The body input carries no section: there is no content to serialize, so
    /// the caller should have passed `body: None` (a typed absence) instead.
    EmptyBodyInput,
    TooManyBodySections {
        count: usize,
        max: usize,
    },
    TooManyBodyLeaves {
        count: usize,
        max: usize,
    },
    TooManyBodyAbsences {
        count: usize,
        max: usize,
    },
    BodySectionDepthExceeded {
        depth: u32,
        max: u32,
    },
    /// A section id cannot be carried verbatim into an XML attribute.
    UnrepresentableSectionId {
        section_id: String,
    },
    DuplicateSectionId {
        section_id: String,
    },
    /// A prose leaf carries no text: an empty paragraph would be a silent empty
    /// element, so it is a typed refusal.
    EmptyBodyLeaf {
        role: FulltextLeafRole,
    },
    /// A leaf's text cannot be carried into XML character data (an illegal XML
    /// character or a carriage return).
    UnrepresentableBodyLeaf {
        role: FulltextLeafRole,
        detail: String,
    },
    /// A family the native plane cannot emit carries text: dropping it would be
    /// silent loss, and splicing Java bytes is forbidden.
    AbsentFamilyCarriesText {
        family: NativeTeiFamily,
    },
    /// A leaf role this slice cannot serialize carries text.
    BodyLeafNotEmittable {
        role: FulltextLeafRole,
        marker_subtype: Option<MarkerSubtype>,
    },
    /// A citation marker names a bibliography key the emission does not publish.
    UnresolvedCitationTarget {
        key: String,
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
    /// Body-plane typed absences: leaves this slice cannot emit, recorded by
    /// role instead of silently dropped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body_absences: Vec<NativeTeiBodyAbsence>,
    /// The native note disposition carried by the body input, when a body was
    /// supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_note_disposition: Option<NoteDisposition>,
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

    // Which families this emission produces natively.  A lane the native plane
    // did not admit is emitted only as a declared typed absence, so the manifest
    // can state exactly what was and was not exercised (S02).
    let header_admitted = matches!(&input.header, HeaderCompositionOutcome::Accepted { .. });
    let references_admitted = matches!(
        &input.references,
        ReferenceCompositionOutcome::Accepted { .. }
    );
    let mut emitted: BTreeSet<NativeTeiFamily> = BTreeSet::new();
    if header_admitted {
        emitted.insert(NativeTeiFamily::Header);
    }
    if references_admitted {
        emitted.insert(NativeTeiFamily::References);
    }
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

    // A rejected lane is a declared typed absence when the caller named it in
    // `absences`; otherwise it is a typed refusal, never a fallback to Java.
    let header = match &input.header {
        HeaderCompositionOutcome::Accepted { composition } => Some(composition),
        HeaderCompositionOutcome::Rejected { error } => {
            if !absence_reasons.contains_key(&NativeTeiFamily::Header) {
                return Err(NativeTeiEmissionError::InvalidComposition {
                    lane: NativeTeiFamily::Header,
                    error: error.clone(),
                });
            }
            None
        }
    };
    let references = match &input.references {
        ReferenceCompositionOutcome::Accepted { composition } => Some(composition),
        ReferenceCompositionOutcome::Rejected { error } => {
            if !absence_reasons.contains_key(&NativeTeiFamily::References) {
                return Err(NativeTeiEmissionError::InvalidComposition {
                    lane: NativeTeiFamily::References,
                    error: error.clone(),
                });
            }
            None
        }
    };

    // Validate the admitted semantics before serializing anything (reuses the
    // composer's own validators; a non-representable value is a typed refusal).
    let admitted = AdmittedSemantics {
        header: match header {
            Some(header) => header_semantics(&header.candidate)?,
            None => AdmittedHeaderSemantics {
                titles: Vec::new(),
                emails: Vec::new(),
                ids: Vec::new(),
                author_affiliations: Vec::new(),
            },
        },
        citations: match references {
            Some(references) => citation_semantics(&references.candidate)?,
            None => Vec::new(),
        },
    };

    // The native bibliography publishes GROBID's positional fallback key domain:
    // no Java source exists to declare `xml:id` keys, and this is the same form
    // the composer's `source_published_keys` publishes for an undeclared entry.
    let keys = match references {
        Some(references) => positional_reference_keys(&references.candidate.references)?,
        None => Vec::new(),
    };

    // Native body text (S02/T02 extends this into section structure).
    let key_set: BTreeSet<String> = keys.iter().cloned().collect();

    // Serialize each band through the shared builders, so this emitter and the
    // composer can never disagree on one serialization of the same semantics.
    let header_fragment = match header {
        Some(header) => render_header_fragment(header, HeaderFragmentSource::native()),
        // The envelope stays structurally TEI; a header this emission did not
        // produce is an empty element, never Java bytes.
        None => String::from("<teiHeader/>"),
    };
    let bibliography_fragment = match references {
        Some(references) => render_bibliography_fragment(
            &keys,
            &references.candidate.references,
            &references.candidate.fields,
        ),
        None => String::from("<listBibl/>"),
    };

    // Body: one `<div type="section">` per native section in reading order, one
    // `<p>`/`<head>`/`<ref>` per leaf.  A family the native plane cannot emit is
    // a typed absence, never a Java fallback; a leaf carrying text for such a
    // family is a typed refusal (silent loss is forbidden).
    let rendered_body = match &input.body {
        Some(body) => render_body_fragment(body, &key_set, &input.limits, &mut absence_reasons)?,
        None => RenderedBody::default(),
    };
    if rendered_body.body_absences.len() > input.limits.max_absences {
        return Err(NativeTeiEmissionError::TooManyBodyAbsences {
            count: rendered_body.body_absences.len(),
            max: input.limits.max_absences,
        });
    }
    let body_fragment = rendered_body.fragment.as_str();

    for (part, bytes) in [
        ("header_fragment", header_fragment.len()),
        ("references_fragment", bibliography_fragment.len()),
        ("body_fragment", body_fragment.len()),
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
    bytes.extend_from_slice(body_fragment.as_bytes());
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
        admitted_references: references
            .map(|references| references.candidate.references.len())
            .unwrap_or(0),
        // Body citation refs this emission serialized, and how many of their
        // distinct targets resolve in the emitted id space.
        target_occurrences: rendered_body.target_occurrences,
        required_targets: rendered_body.targets.len(),
        resolved_targets: rendered_body
            .targets
            .iter()
            .filter(|target| document.has_xml_id(target))
            .count(),
        id_space: document.xml_id_count(),
    };

    let families = family_dispositions(&emitted, &absence_reasons);
    let mut absence_canon = String::new();
    for family in NativeTeiFamily::ALL {
        if let Some(reason) = absence_reasons.get(&family) {
            absence_canon.push_str(family.token());
            absence_canon.push(':');
            absence_canon.push_str(reason.token());
            absence_canon.push('\n');
        }
    }
    let header_bytes = if header.is_some() {
        header_fragment.len()
    } else {
        0
    };
    let references_bytes = if references.is_some() {
        bibliography_fragment.len()
    } else {
        0
    };
    let byte_accounting = NativeTeiByteAccounting {
        header_bytes,
        references_bytes,
        body_bytes: body_fragment.len(),
        native_emitted_bytes: header_bytes + references_bytes + body_fragment.len(),
        typed_absence_bytes: 0,
        envelope_bytes: bytes.len(),
        java_fallback_bytes: 0,
        header_sha256: digest(header_fragment.as_bytes()),
        references_sha256: digest(bibliography_fragment.as_bytes()),
        body_sha256: digest(body_fragment.as_bytes()),
        typed_absence_sha256: digest(absence_canon.as_bytes()),
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
        body_absences: rendered_body.body_absences,
        body_note_disposition: input
            .body
            .as_ref()
            .map(|body| body.note_disposition.clone()),
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

/// The rendered body band and the typed-absence facts it produced.
#[derive(Debug, Default)]
struct RenderedBody {
    /// The serialized inner `<body>` content (one `<div type="section">` per
    /// native section).
    fragment: String,
    /// Body-plane typed absences (leaves this slice cannot emit).
    body_absences: Vec<NativeTeiBodyAbsence>,
    /// `ref type="bibr"` elements serialized.
    target_occurrences: usize,
    /// Distinct published keys those refs demand.
    targets: BTreeSet<String>,
}

/// Which [`NativeTeiFamily`] a body leaf belongs to, when the native plane has
/// no emission for it.  `None` means the leaf is either emittable prose or a
/// family-less leaf (a note, an unsupported token, a keyless marker).
fn leaf_family(role: FulltextLeafRole, subtype: Option<MarkerSubtype>) -> Option<NativeTeiFamily> {
    match (role, subtype) {
        (FulltextLeafRole::Marker, Some(MarkerSubtype::Figure)) => Some(NativeTeiFamily::Figure),
        (FulltextLeafRole::Marker, Some(MarkerSubtype::Table)) => Some(NativeTeiFamily::Table),
        (FulltextLeafRole::Marker, Some(MarkerSubtype::Equation)) => Some(NativeTeiFamily::Formula),
        _ => None,
    }
}

/// Serialize the native body reading order.
///
/// One `<div type="section">` per native section, in the input's reading order;
/// one `<p>` per prose leaf, `<head>` per heading leaf, `<ref type="bibr">` per
/// resolved citation marker.  A family the native plane cannot emit becomes a
/// typed absence (merged into `absence_reasons`); a leaf carrying text for such
/// a family is a typed refusal, never silent loss and never Java bytes.
fn render_body_fragment(
    body: &NativeTeiBodyInput,
    keys: &BTreeSet<String>,
    limits: &NativeTeiEmissionLimits,
    absence_reasons: &mut BTreeMap<NativeTeiFamily, NativeTeiAbsenceReason>,
) -> Result<RenderedBody, NativeTeiEmissionError> {
    if body.sections.is_empty() {
        return Err(NativeTeiEmissionError::EmptyBodyInput);
    }
    if body.sections.len() > limits.max_body_sections {
        return Err(NativeTeiEmissionError::TooManyBodySections {
            count: body.sections.len(),
            max: limits.max_body_sections,
        });
    }
    let mut rendered = RenderedBody::default();
    let mut seen_section_ids: BTreeSet<String> = BTreeSet::new();
    let mut leaf_count = 0usize;
    for section in &body.sections {
        if section.depth == 0 || section.depth > MAX_NATIVE_SECTION_DEPTH {
            return Err(NativeTeiEmissionError::BodySectionDepthExceeded {
                depth: section.depth,
                max: MAX_NATIVE_SECTION_DEPTH,
            });
        }
        rendered.fragment.push_str("<div type=\"section\"");
        if let Some(section_id) = &section.section_id {
            if !representable_attribute(section_id) {
                return Err(NativeTeiEmissionError::UnrepresentableSectionId {
                    section_id: section_id.clone(),
                });
            }
            if !seen_section_ids.insert(section_id.clone()) {
                return Err(NativeTeiEmissionError::DuplicateSectionId {
                    section_id: section_id.clone(),
                });
            }
            rendered.fragment.push_str(" xml:id=\"");
            rendered.fragment.push_str(&esc(section_id));
            rendered.fragment.push('"');
        }
        rendered.fragment.push('>');
        for leaf in &section.leaves {
            leaf_count += 1;
            if leaf_count > limits.max_body_leaves {
                return Err(NativeTeiEmissionError::TooManyBodyLeaves {
                    count: leaf_count,
                    max: limits.max_body_leaves,
                });
            }
            render_leaf(&mut rendered, leaf, keys, absence_reasons)?;
        }
        rendered.fragment.push_str("</div>");
    }
    Ok(rendered)
}

fn render_leaf(
    rendered: &mut RenderedBody,
    leaf: &NativeTeiBodyLeaf,
    keys: &BTreeSet<String>,
    absence_reasons: &mut BTreeMap<NativeTeiFamily, NativeTeiAbsenceReason>,
) -> Result<(), NativeTeiEmissionError> {
    match leaf.role {
        FulltextLeafRole::Paragraph | FulltextLeafRole::ListItem => {
            require_non_empty_text(leaf)?;
            rendered.fragment.push_str("<p>");
            rendered.fragment.push_str(&esc(&leaf.text));
            rendered.fragment.push_str("</p>");
        }
        FulltextLeafRole::Section => {
            require_non_empty_text(leaf)?;
            rendered.fragment.push_str("<head>");
            rendered.fragment.push_str(&esc(&leaf.text));
            rendered.fragment.push_str("</head>");
        }
        FulltextLeafRole::Marker => match leaf.marker_subtype {
            Some(MarkerSubtype::Citation) => match &leaf.published_key {
                Some(key) => {
                    if !representable_attribute(key) {
                        return Err(NativeTeiEmissionError::UnrepresentableBodyLeaf {
                            role: leaf.role,
                            detail: "citation key cannot be carried into an XML attribute".into(),
                        });
                    }
                    if !keys.contains(key) {
                        return Err(NativeTeiEmissionError::UnresolvedCitationTarget {
                            key: key.clone(),
                        });
                    }
                    rendered.fragment.push_str("<ref type=\"bibr\" target=\"#");
                    rendered.fragment.push_str(&esc(key));
                    rendered.fragment.push('"');
                    if leaf.text.is_empty() {
                        rendered.fragment.push_str("/>");
                    } else {
                        require_representable_text(leaf)?;
                        rendered.fragment.push('>');
                        rendered.fragment.push_str(&esc(&leaf.text));
                        rendered.fragment.push_str("</ref>");
                    }
                    rendered.target_occurrences += 1;
                    rendered.targets.insert(key.clone());
                }
                None => {
                    if leaf.text.is_empty() {
                        rendered.body_absences.push(NativeTeiBodyAbsence {
                            role: leaf.role,
                            marker_subtype: leaf.marker_subtype,
                            reason: NativeTeiAbsenceReason::NoAdmittedTokens,
                        });
                    } else {
                        require_representable_text(leaf)?;
                        rendered.fragment.push_str("<p>");
                        rendered.fragment.push_str(&esc(&leaf.text));
                        rendered.fragment.push_str("</p>");
                    }
                }
            },
            Some(subtype) => {
                let family = leaf_family(leaf.role, Some(subtype))
                    .expect("a non-citation marker subtype maps to a family");
                if !leaf.text.is_empty() {
                    return Err(NativeTeiEmissionError::AbsentFamilyCarriesText { family });
                }
                absence_reasons
                    .entry(family)
                    .or_insert(NativeTeiAbsenceReason::NoAdmittedTokens);
                rendered.body_absences.push(NativeTeiBodyAbsence {
                    role: leaf.role,
                    marker_subtype: Some(subtype),
                    reason: NativeTeiAbsenceReason::NoAdmittedTokens,
                });
            }
            None => {
                if !leaf.text.is_empty() {
                    return Err(NativeTeiEmissionError::BodyLeafNotEmittable {
                        role: leaf.role,
                        marker_subtype: leaf.marker_subtype,
                    });
                }
                rendered.body_absences.push(NativeTeiBodyAbsence {
                    role: leaf.role,
                    marker_subtype: leaf.marker_subtype,
                    reason: NativeTeiAbsenceReason::NoAdmittedTokens,
                });
            }
        },
        FulltextLeafRole::Note | FulltextLeafRole::Unsupported => {
            if !leaf.text.is_empty() {
                return Err(NativeTeiEmissionError::BodyLeafNotEmittable {
                    role: leaf.role,
                    marker_subtype: leaf.marker_subtype,
                });
            }
            rendered.body_absences.push(NativeTeiBodyAbsence {
                role: leaf.role,
                marker_subtype: leaf.marker_subtype,
                reason: NativeTeiAbsenceReason::NoAdmittedTokens,
            });
        }
    }
    Ok(())
}

/// A prose leaf must carry text: an empty paragraph is a typed refusal rather
/// than a silent empty element.
fn require_non_empty_text(leaf: &NativeTeiBodyLeaf) -> Result<(), NativeTeiEmissionError> {
    if leaf.text.is_empty() {
        return Err(NativeTeiEmissionError::EmptyBodyLeaf { role: leaf.role });
    }
    require_representable_text(leaf)
}

fn require_representable_text(leaf: &NativeTeiBodyLeaf) -> Result<(), NativeTeiEmissionError> {
    if !representable_text(&leaf.text) {
        return Err(NativeTeiEmissionError::UnrepresentableBodyLeaf {
            role: leaf.role,
            detail: "body leaf text cannot be carried into XML character data".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffiliationSpan, BodyCasePolicy, BodyJoinPolicy, BodyNormalization, BodyUnitRung,
        CitationField, CitationFieldRole, CitationSpan, ComponentDisposition, ComponentReceipt,
        CompositionComponent, CompositionSpan, HeaderAuthor, HeaderCandidate,
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
        input.body = Some(NativeTeiBodyInput::single_paragraph("Body <text> & more"));
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains(
            "<text><body><div type=\"section\"><p>Body &lt;text&gt; &amp; more</p></div></body>"
        ));
        assert_eq!(
            manifest.body_note_disposition,
            Some(NoteDisposition::SourceNotEstablished)
        );
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
    fn rejected_header_lane_with_declared_absence_is_typed_absence() {
        let mut input = base_input();
        input.header = HeaderCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::Date,
            },
        };
        input.absences = vec![NativeTeiAbsence {
            family: NativeTeiFamily::Header,
            reason: NativeTeiAbsenceReason::NoAdmittedTokens,
        }];
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<teiHeader/>"));
        assert_eq!(
            family(&manifest, NativeTeiFamily::Header).disposition,
            NativeTeiDisposition::TypedAbsence
        );
        assert_eq!(manifest.byte_accounting.header_bytes, 0);
    }

    #[test]
    fn rejected_references_lane_with_declared_absence_is_typed_absence() {
        let mut input = base_input();
        input.references = ReferenceCompositionOutcome::Rejected {
            error: CompositionError::EmptyComponent {
                component: CompositionComponent::References,
            },
        };
        input.absences = vec![NativeTeiAbsence {
            family: NativeTeiFamily::References,
            reason: NativeTeiAbsenceReason::NoAdmittedTokens,
        }];
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<listBibl/>"));
        assert_eq!(
            family(&manifest, NativeTeiFamily::References).disposition,
            NativeTeiDisposition::TypedAbsence
        );
        assert_eq!(manifest.byte_accounting.references_bytes, 0);
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
        input.body = Some(NativeTeiBodyInput::single_paragraph("carriage\rreturn"));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::UnrepresentableBodyLeaf {
                    role: FulltextLeafRole::Paragraph,
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

    fn leaf(text: &str, role: FulltextLeafRole) -> NativeTeiBodyLeaf {
        NativeTeiBodyLeaf {
            text: text.into(),
            role,
            marker_subtype: None,
            published_key: None,
        }
    }

    fn section(id: &str, leaves: Vec<NativeTeiBodyLeaf>) -> NativeTeiBodySection {
        NativeTeiBodySection {
            section_id: Some(id.into()),
            depth: 1,
            parent_section_id: None,
            leaves,
        }
    }

    fn body_with(sections: Vec<NativeTeiBodySection>) -> NativeTeiBodyInput {
        NativeTeiBodyInput {
            sections,
            note_disposition: NoteDisposition::SourceNotEstablished,
        }
    }

    #[test]
    fn synthetic_body_emits_div_and_p_in_reading_order() {
        let mut input = base_input();
        input.body = Some(body_with(vec![
            section(
                "body-0-1",
                vec![
                    NativeTeiBodyLeaf {
                        text: "Introduction".into(),
                        role: FulltextLeafRole::Section,
                        marker_subtype: None,
                        published_key: None,
                    },
                    leaf("First paragraph.", FulltextLeafRole::Paragraph),
                ],
            ),
            section(
                "body-1-2",
                vec![leaf("Second paragraph.", FulltextLeafRole::Paragraph)],
            ),
        ]));
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        let div0 = text
            .find("<div type=\"section\" xml:id=\"body-0-1\">")
            .expect("first section div");
        let head = text.find("<head>Introduction</head>").expect("heading");
        let p0 = text.find("<p>First paragraph.</p>").expect("first p");
        let div1 = text
            .find("<div type=\"section\" xml:id=\"body-1-2\">")
            .expect("second section div");
        let p1 = text.find("<p>Second paragraph.</p>").expect("second p");
        assert!(div0 < head && head < p0 && p0 < div1 && div1 < p1);
        assert!(manifest.body_absences.is_empty());
    }

    #[test]
    fn empty_prose_leaf_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![leaf("", FulltextLeafRole::Paragraph)],
        )]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::EmptyBodyLeaf {
                    role: FulltextLeafRole::Paragraph
                }
            }
        ));
    }

    #[test]
    fn illegal_xml_char_in_body_leaf_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![leaf("bad\u{0}value", FulltextLeafRole::Paragraph)],
        )]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::UnrepresentableBodyLeaf { .. }
            }
        ));
    }

    #[test]
    fn absent_family_carries_text_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![NativeTeiBodyLeaf {
                text: "Figure 1".into(),
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Figure),
                published_key: None,
            }],
        )]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::AbsentFamilyCarriesText {
                    family: NativeTeiFamily::Figure
                }
            }
        ));
    }

    #[test]
    fn citation_marker_emits_ref_with_published_key() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![NativeTeiBodyLeaf {
                text: "[1]".into(),
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Citation),
                published_key: Some("b0".into()),
            }],
        )]));
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<ref type=\"bibr\" target=\"#b0\">[1]</ref>"));
        assert_eq!(manifest.reference_links.target_occurrences, 1);
        assert_eq!(manifest.reference_links.required_targets, 1);
        assert_eq!(manifest.reference_links.resolved_targets, 1);
    }

    #[test]
    fn citation_marker_without_key_is_plain_text() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![NativeTeiBodyLeaf {
                text: "[1]".into(),
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Citation),
                published_key: None,
            }],
        )]));
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<p>[1]</p>"));
        assert!(!text.contains("target=\"#"));
        assert_eq!(manifest.reference_links.target_occurrences, 0);
    }

    #[test]
    fn unresolved_citation_target_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![NativeTeiBodyLeaf {
                text: "[9]".into(),
                role: FulltextLeafRole::Marker,
                marker_subtype: Some(MarkerSubtype::Citation),
                published_key: Some("b7".into()),
            }],
        )]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::UnresolvedCitationTarget { key }
            } if key == "b7"
        ));
    }

    #[test]
    fn absent_families_are_typed_absence_in_manifest() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![
                leaf("A paragraph.", FulltextLeafRole::Paragraph),
                NativeTeiBodyLeaf {
                    text: String::new(),
                    role: FulltextLeafRole::Marker,
                    marker_subtype: Some(MarkerSubtype::Figure),
                    published_key: None,
                },
                NativeTeiBodyLeaf {
                    text: String::new(),
                    role: FulltextLeafRole::Marker,
                    marker_subtype: Some(MarkerSubtype::Table),
                    published_key: None,
                },
            ],
        )]));
        input.absences = vec![
            NativeTeiAbsence {
                family: NativeTeiFamily::Visual,
                reason: NativeTeiAbsenceReason::NativeFamilyUnavailable,
            },
            NativeTeiAbsence {
                family: NativeTeiFamily::Callout,
                reason: NativeTeiAbsenceReason::NativeFamilyUnavailable,
            },
        ];
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        assert!(text.contains("<p>A paragraph.</p>"));
        for absent in [
            NativeTeiFamily::Figure,
            NativeTeiFamily::Table,
            NativeTeiFamily::Visual,
            NativeTeiFamily::Callout,
        ] {
            let disposition = family(&manifest, absent);
            assert_eq!(disposition.disposition, NativeTeiDisposition::TypedAbsence);
        }
        assert_eq!(manifest.body_absences.len(), 2);
        assert_eq!(manifest.byte_accounting.typed_absence_bytes, 0);
        assert!(!manifest.byte_accounting.typed_absence_sha256.is_empty());
    }

    #[test]
    fn byte_accounting_re_derives_from_components() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![leaf("A paragraph.", FulltextLeafRole::Paragraph)],
        )]));
        let (envelope, manifest) = accepted(input);
        let text = std::str::from_utf8(envelope.raw_tei()).expect("utf-8 emission");
        let body_start = text.find("<text><body>").expect("body open") + "<text><body>".len();
        let body_end = text.find("</body>").expect("body close");
        let body_fragment = &text[body_start..body_end];
        assert_eq!(manifest.byte_accounting.body_bytes, body_fragment.len());
        assert_eq!(
            manifest.byte_accounting.body_sha256,
            digest(body_fragment.as_bytes())
        );
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
        assert!(!manifest.byte_accounting.header_sha256.is_empty());
        assert!(!manifest.byte_accounting.references_sha256.is_empty());
    }

    #[test]
    fn duplicate_section_id_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![
            section("body-0-1", vec![leaf("One.", FulltextLeafRole::Paragraph)]),
            section("body-0-1", vec![leaf("Two.", FulltextLeafRole::Paragraph)]),
        ]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::DuplicateSectionId { .. }
            }
        ));
    }

    #[test]
    fn non_emittable_leaf_with_text_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![section(
            "body-0-1",
            vec![leaf("a footnote", FulltextLeafRole::Note)],
        )]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::BodyLeafNotEmittable {
                    role: FulltextLeafRole::Note,
                    ..
                }
            }
        ));
    }

    #[test]
    fn empty_body_input_is_refused() {
        let mut input = base_input();
        input.body = Some(body_with(vec![]));
        assert!(matches!(
            emit_native_tei(input),
            NativeTeiEmissionOutcome::Rejected {
                error: NativeTeiEmissionError::EmptyBodyInput
            }
        ));
    }

    #[test]
    fn body_input_from_native_body_text_uses_joined_text() {
        let text = NativeBodyText::from_joined_text(
            "doc-1",
            "a".repeat(64),
            3,
            Some((1, 1)),
            BodyJoinPolicy::VerbatimTokenConcat,
            "abc def ghi",
            BodyUnitRung::new(BodyNormalization::Raw, BodyCasePolicy::CaseSensitive),
        )
        .expect("a valid native body text");
        let body = NativeTeiBodyInput::from_body_text(&text);
        assert_eq!(body.sections.len(), 1);
        assert_eq!(body.sections[0].leaves.len(), 1);
        assert_eq!(body.sections[0].leaves[0].text, "abc def ghi");
        assert_eq!(body.sections[0].leaves[0].role, FulltextLeafRole::Paragraph);
    }
}
