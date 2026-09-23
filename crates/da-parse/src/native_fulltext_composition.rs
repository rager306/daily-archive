//! Pure, bounded composition of a source-bound native fulltext shadow candidate.
//!
//! This is deliberately an orchestration boundary, not a parser or runtime
//! route.  It consumes a complete Java TEI envelope and already typed leaf
//! outcomes.  Only independently admitted header and reference outcomes may
//! replace their corresponding TEI subtrees; every other family remains byte
//! accounted Java fallback.
use crate::{
    CitationFieldRole, HeaderCandidate, HeaderCompositionOutcome, NativeVisualHeadOutcome,
    ReferenceCandidate, ReferenceCompositionOutcome, SegmentationOutcome, TeiEnvelope, TeiIdentity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

pub const NATIVE_FULLTEXT_COMPOSITION_SCHEMA_VERSION: &str =
    "da-parse.native-fulltext-composition/v1";
pub const NATIVE_FULLTEXT_MAX_JAVA_BYTES: usize = 32 * 1024 * 1024;
pub const NATIVE_FULLTEXT_MAX_COMPONENT_BYTES: usize = 4 * 1024 * 1024;
pub const NATIVE_FULLTEXT_MAX_XML_DEPTH: usize = 128;
pub const NATIVE_FULLTEXT_FALLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";

/// The closed vocabulary of the reference-link integrity contract owned by the
/// composition policy DATA (`reference_link_integrity` block).
///
/// S02/T04: the rule that an accepted emission's local links resolve is
/// machine-checkable DATA, not prose. These constants are the code side of that
/// contract and the DATA contract test refuses drift in either direction:
///
/// * `ID_SPACE` — the emitted document's `#fragment` address space is the
///   *source's* published keys, never a synthesized `bibl-{ordinal}` domain;
/// * `DERIVATION` — those keys are the source `listBibl`'s direct children;
/// * `POSITIONAL_FALLBACK` — an undeclared entry publishes the same text
///   `da_ports::citation_link::positional_entry_key` renders (`b{ordinal}`);
/// * `REQUIRED_TARGETS` — the obligations read from the source are
///   `ref type="bibr"` elements with a local `#`-fragment `target`;
/// * `RESOLVE_GATE` — the gate runs *after* the splice, so it sees the ids the
///   emission actually carries (the pre-splice guard was vacuous on an empty
///   supplied anchor stream, which is how the dangling links survived).
///
/// Behaviour is exercised by `native_fulltext_composition_reference_integrity.rs`.
pub const NATIVE_FULLTEXT_REFERENCE_LINK_ID_SPACE: &str = "source_published_keys";
pub const NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION: &str = "source_listbibl_direct_children";
pub const NATIVE_FULLTEXT_REFERENCE_LINK_POSITIONAL_FALLBACK: &str = "b{ordinal}";
pub const NATIVE_FULLTEXT_REFERENCE_LINK_REQUIRED_TARGETS: &str =
    "source_ref_type_bibr_with_hash_target";
pub const NATIVE_FULLTEXT_REFERENCE_LINK_RESOLVE_GATE: &str = "post_splice";

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

/// What the composed document did to the source's citation links.
///
/// S02/T02: the composer used to rename every bibliography entry into a
/// synthesized `bibl-{ordinal}` domain while copying the source body verbatim,
/// so a kept `target="#b0"` was dangling in an emission still reported as
/// `Accepted` (measured 217 occurrences / 120 unique dangling targets across the
/// three sha256-pinned preserved TEI).  The ledger is the observable record of
/// the repair: the source's own published keys are republished and every
/// mandatory local target is resolved before acceptance.
///
/// All counts are computed from the source and emitted documents, never from the
/// caller's `anchors`: with an empty supplied anchor stream the old pre-splice
/// guard had nothing to check, which is exactly how the dangling links survived.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceLinkLedger {
    /// Direct `biblStruct` children of the source's unique `listBibl` whose keys
    /// the admitted band republishes.  `0` when the band falls back to Java.
    pub source_entries: usize,
    /// Admitted native reference entries the band spliced in.
    pub admitted_references: usize,
    /// `ref type="bibr"` elements carrying a local `#`-fragment `target`, by the
    /// XML attribute grammar (so `target = '#b0'` counts like the compact
    /// spelling).  Repeats are occurrences, not identities.
    pub target_occurrences: usize,
    /// Distinct fragments those occurrences demand.
    pub required_targets: usize,
    /// Mandatory targets that resolve in the emitted document's id space.
    pub resolved_targets: usize,
    /// Distinct `xml:id` values in the emitted document — the address space a
    /// `#fragment` target can resolve against.  `xml:id` is the only attribute
    /// whose ID-ness is defined without a DTD or schema, which is why a plain
    /// `id` is not part of this space.
    pub id_space: usize,
}

/// What the admitted header and reference bands carried semantically and what
/// the emitted document conserved.
///
/// S03/T02: the emitters used to carry only `authors`/`date` and each
/// reference's `source` text, so an `Accepted` emission was a claim about
/// structure and links and never about the fields the admitted bands had already
/// produced (`NativeHeaderFields{title,email,ids}`, `HeaderAuthor.affiliations`,
/// `ReferenceCandidate.fields`).  The audit (A-FIELDS/H16) measured that loss.
/// This ledger is the observable record of the repair: the admitted side is
/// counted from the admitted candidate the composer was handed, the conserved
/// side is counted from the *emitted bytes*, and the post-serialization gate
/// refuses whenever a conserved count is below its admitted count.
///
/// All counts are recomputed from the admitted values and the emission, never
/// trusted as an input: the gate reads the bytes it just produced, so a ledger
/// can never vouch for a field the emission does not carry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFieldLedger {
    /// Admitted header `<title>` values (`NativeHeaderFields::title`).
    pub admitted_header_titles: usize,
    /// Admitted header `<email>` values (`NativeHeaderFields::email`).
    pub admitted_header_emails: usize,
    /// Admitted header id values (`NativeHeaderFields::ids`, GROBID `<pubnum>`).
    pub admitted_header_ids: usize,
    /// Admitted `author -> affiliation` bindings (`HeaderAuthor.affiliations`).
    pub admitted_author_affiliations: usize,
    /// Admitted bibliography fields (`ReferenceCandidate.fields`).
    pub admitted_citation_fields: usize,
    /// Admitted header titles the emitted document carries.
    pub conserved_header_titles: usize,
    /// Admitted header emails the emitted document carries.
    pub conserved_header_emails: usize,
    /// Admitted header ids the emitted document carries.
    pub conserved_header_ids: usize,
    /// Admitted author-affiliation bindings the emitted document carries.
    pub conserved_author_affiliations: usize,
    /// Admitted bibliography fields the emitted document carries with their role.
    pub conserved_citation_fields: usize,
}
impl SemanticFieldLedger {
    /// Human-readable list of every admitted class the emission did not conserve,
    /// used as the typed-refusal detail.  Empty means the emission carried every
    /// admitted semantic field.
    fn losses(&self) -> Vec<String> {
        let mut losses = Vec::new();
        let mut class = |class: SemanticFieldLoss, admitted: usize, conserved: usize| {
            if conserved < admitted {
                losses.push(format!(
                    "{}: {conserved}/{admitted} admitted fields conserved in the emission",
                    class.token()
                ));
            }
        };
        class(
            SemanticFieldLoss::HeaderTitle,
            self.admitted_header_titles,
            self.conserved_header_titles,
        );
        class(
            SemanticFieldLoss::HeaderEmail,
            self.admitted_header_emails,
            self.conserved_header_emails,
        );
        class(
            SemanticFieldLoss::HeaderIds,
            self.admitted_header_ids,
            self.conserved_header_ids,
        );
        class(
            SemanticFieldLoss::AuthorAffiliation,
            self.admitted_author_affiliations,
            self.conserved_author_affiliations,
        );
        class(
            SemanticFieldLoss::CitationRole,
            self.admitted_citation_fields,
            self.conserved_citation_fields,
        );
        losses
    }

    /// The first admitted class the emission failed to conserve, in report order.
    fn first_loss(&self) -> Option<SemanticFieldLoss> {
        if self.conserved_header_titles < self.admitted_header_titles {
            return Some(SemanticFieldLoss::HeaderTitle);
        }
        if self.conserved_header_emails < self.admitted_header_emails {
            return Some(SemanticFieldLoss::HeaderEmail);
        }
        if self.conserved_header_ids < self.admitted_header_ids {
            return Some(SemanticFieldLoss::HeaderIds);
        }
        if self.conserved_author_affiliations < self.admitted_author_affiliations {
            return Some(SemanticFieldLoss::AuthorAffiliation);
        }
        if self.conserved_citation_fields < self.admitted_citation_fields {
            return Some(SemanticFieldLoss::CitationRole);
        }
        None
    }
}

/// The closed class vocabulary for an admitted semantic field the emitter cannot
/// conserve in the emitted bytes.
///
/// S03/T02: the same tokens are reported by the independent post-serialization
/// oracle (`native_fulltext_composition_semantics.rs`), so the production refusal
/// and the measurement cannot drift into near-miss spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFieldLoss {
    HeaderTitle,
    HeaderEmail,
    HeaderIds,
    AuthorAffiliation,
    CitationRole,
}
impl SemanticFieldLoss {
    /// The class token shared with the independent oracle's `CLASS_*` constants.
    pub const fn token(self) -> &'static str {
        match self {
            Self::HeaderTitle => "SemanticFieldLoss::HeaderTitle",
            Self::HeaderEmail => "SemanticFieldLoss::HeaderEmail",
            Self::HeaderIds => "SemanticFieldLoss::HeaderIds",
            Self::AuthorAffiliation => "SemanticFieldLoss::AuthorAffiliation",
            Self::CitationRole => "SemanticFieldLoss::CitationRole",
        }
    }
}
impl fmt::Display for SemanticFieldLoss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeFulltextManifest {
    pub schema_version: String,
    pub identity: FulltextCompositionIdentity,
    pub components: Vec<FulltextComponentManifest>,
    /// `#[serde(default)]`: manifests serialized before S02/T02 stay decodable
    /// (R085-style backward compatibility), and an absent ledger degrades to the
    /// all-zero record rather than an error.
    #[serde(default)]
    pub reference_links: ReferenceLinkLedger,
    /// `#[serde(default)]`: manifests serialized before S03/T02 stay decodable
    /// (all-zero ledger), the same backward-compatibility device
    /// `reference_links` uses.  Boxed because the ledger's ten counters would
    /// otherwise grow `NativeFulltextCompositionOutcome::Accepted` past clippy's
    /// `large_enum_variant` bound against the byte-identical Java fallback.
    #[serde(default)]
    pub semantic_fields: Box<SemanticFieldLedger>,
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
    /// The admitted reference band cannot be republished under the source's own
    /// published citation keys: the number of admitted entries differs from the
    /// number of direct `biblStruct` children of the source `listBibl`, or their
    /// ordinals do not follow the source order.  The composer never renumbers a
    /// bibliography into a different domain, because the kept document's own
    /// `target="#b0"` links would then point at entries that no longer exist.
    BibliographyDomainMismatch(String),
    /// Two source bibliography children publish the same key.  One key is one
    /// identity: publishing both would make a `#fragment` target ambiguous, so
    /// this is a typed refusal rather than a silent first-wins choice.
    BibliographyKeyCollision(String),
    /// A mandatory local reference of the kept document does not resolve in the
    /// id space of the emitted document.  `Accepted` is a claim about the bytes
    /// returned, so a dangling `target="#b0"` may never be republished as
    /// accepted; the typed refusal keeps the byte-identical Java envelope.
    UnresolvedBibliographyTarget(String),
    /// A direct `listBibl` child cannot be conserved under its own key: it is not
    /// a `biblStruct`, or its declared `xml:id` is not a legal `NCName` and can
    /// therefore not be re-emitted as an attribute value.  Dropping such a child
    /// would silently lose a bibliography entry, so it is named here instead.
    InconservableBibliographyChild(String),
    /// An admitted semantic field cannot be conserved in the emitted bytes.
    ///
    /// S03/T02: the emitters used to carry only `authors`/`date` and each
    /// reference's `source` text, so an `Accepted` emission was a claim about
    /// structure and links and never about the fields the admitted bands already
    /// produced (`NativeHeaderFields{title,email,ids}`,
    /// `HeaderAuthor.affiliations`, `ReferenceCandidate.fields`).  A field whose
    /// value or role cannot be represented, an affiliation link whose component
    /// or span is not admitted, and a field the post-serialization gate cannot
    /// find in the emission are all this class — never a silent omission.
    SemanticFieldLoss {
        class: SemanticFieldLoss,
        detail: String,
    },
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
    // The obligations the kept document carries are read from the kept document,
    // not from the caller's anchor stream: with empty supplied anchors the old
    // pre-splice guard was vacuous and a dangling `#b0` survived into `Accepted`.
    let (target_occurrences, required_targets) = mandatory_targets(&ranges, text)?;

    let mut bytes = java.to_vec();
    let mut components = Vec::with_capacity(7);
    let header_receipt = admission(&input, FulltextComponent::Header)?;
    let refs_receipt = admission(&input, FulltextComponent::References)?;
    let header_admitted = is_admitted(&header_receipt, &input.identity);
    let refs_admitted = is_admitted(&refs_receipt, &input.identity);
    // S03/T02: the admitted semantic fields are validated before anything is
    // spliced, so an unrepresentable value or an invalid affiliation link is a
    // typed refusal instead of a silently omitted element.
    let admitted = admitted_semantics(&input, header_admitted, refs_admitted)?;
    let mut reference_links = ReferenceLinkLedger::default();
    let mut replacements = Vec::new();
    if header_admitted {
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
    if refs_admitted {
        let ReferenceCompositionOutcome::Accepted { composition } = &input.references else {
            return Err(NativeFulltextCompositionError::InvalidAdmission(
                FulltextComponent::References,
            ));
        };
        let (fragment, published) = references_xml(composition, &ranges, text)?;
        reference_links = published;
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
    // Post-splice resolve gate (S02/T02).  `Accepted` is a claim about the bytes
    // returned, so before this module may publish them it asks the question the
    // kept document actually poses: does every local `target="#fragment"` in the
    // source resolve in the emitted document's own id space?  The composer used
    // to rename the bibliography into `bibl-{ordinal}` and never ask, so all 120
    // mandatory targets of the preserved fixtures resolved nowhere while the
    // emission was reported as accepted.  A dangling link is now a typed refusal
    // and the byte-identical Java envelope is returned instead.
    let emitted = validate_emitted(&bytes)?;
    let emitted_ids: BTreeSet<&str> = emitted.xml_ids.iter().map(String::as_str).collect();
    let unresolved: Vec<&str> = required_targets
        .iter()
        .filter(|target| !emitted_ids.contains(target.as_str()))
        .map(String::as_str)
        .collect();
    if !unresolved.is_empty() {
        return Err(
            NativeFulltextCompositionError::UnresolvedBibliographyTarget(format!(
                "{} of {} mandatory source targets do not resolve in the emitted document (head {:?})",
                unresolved.len(),
                required_targets.len(),
                &unresolved[..unresolved.len().min(8)]
            )),
        );
    }
    reference_links.target_occurrences = target_occurrences;
    reference_links.required_targets = required_targets.len();
    reference_links.resolved_targets = required_targets.len();
    reference_links.id_space = emitted_ids.len();
    // Post-serialization semantic gate (S03/T02).  `Accepted` is a claim about
    // the bytes returned, so the emitter re-reads its own emission and checks
    // that every admitted semantic field actually reached it.  The A-FIELDS
    // audit measured that header title/email/ids, author->affiliation bindings
    // and bibliography citation roles never did; a class the emission does not
    // conserve is a typed refusal with the byte-identical Java envelope.
    let emitted_text =
        std::str::from_utf8(&bytes).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    let semantic_fields = audit_semantic_fields(&emitted, emitted_text, &admitted)?;
    for (component, receipt, outcome) in [
        (FulltextComponent::Header, header_receipt, header_admitted),
        (FulltextComponent::References, refs_receipt, refs_admitted),
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
            reference_links,
            semantic_fields: Box::new(semantic_fields),
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

/// The typed refusal for an admitted semantic field the emitter cannot conserve.
fn semantic_loss(
    class: SemanticFieldLoss,
    detail: impl Into<String>,
) -> NativeFulltextCompositionError {
    NativeFulltextCompositionError::SemanticFieldLoss {
        class,
        detail: detail.into(),
    }
}

/// Whether a value survives a byte-exact round trip through XML character data.
///
/// `esc` escapes `&<>"`, so every legal XML character is representable; an empty
/// value and an illegal character are not, and a literal `\r` would be
/// normalized to `\n` by the XML end-of-line rules.  Such a value is a typed
/// semantic loss, never a silently omitted element.
fn representable_text(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| is_legal_xml_char(c) && c != '\r')
}

/// Whether a value survives a byte-exact round trip through an XML attribute.
///
/// Attribute values additionally normalize tab, newline and carriage return to a
/// single space, so a key carrying any of them cannot be re-emitted verbatim.
fn representable_attribute(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| is_legal_xml_char(c) && !matches!(c, '\t' | '\n' | '\r'))
}

/// The XML attribute token of a citation role.
///
/// Mirrors `CitationFieldRole`'s `#[serde(rename_all = "snake_case")]` wire form
/// (the same token the independent oracle derives through serde); a `match`
/// without a wildcard keeps a newly added variant a compile error instead of a
/// silently unserializable role.  The contract test
/// `native_fulltext_composition_semantics` asserts every arm against the serde
/// token.
pub const fn citation_field_role_token(role: CitationFieldRole) -> &'static str {
    match role {
        CitationFieldRole::Authors => "authors",
        CitationFieldRole::PublicationDate => "publication_date",
        CitationFieldRole::ArticleTitle => "article_title",
        CitationFieldRole::ContainerTitle => "container_title",
        CitationFieldRole::Volume => "volume",
        CitationFieldRole::Issue => "issue",
        CitationFieldRole::Pages => "pages",
        CitationFieldRole::ArticleNumber => "article_number",
        CitationFieldRole::Publisher => "publisher",
        CitationFieldRole::Place => "place",
        CitationFieldRole::Doi => "doi",
        CitationFieldRole::ArxivId => "arxiv_id",
        CitationFieldRole::Url => "url",
    }
}

/// The admitted header semantics one admitted header band carries.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct AdmittedHeaderSemantics {
    titles: Vec<String>,
    emails: Vec<String>,
    ids: Vec<String>,
    /// `(author ordinal, affiliation key)` bindings, in admitted order.
    author_affiliations: Vec<(usize, String)>,
}

/// One admitted bibliography field: its role wire token and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmittedCitationSemantic {
    role: &'static str,
    value: String,
}

/// Every admitted semantic field the emission is obligated to conserve.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct AdmittedSemantics {
    header: AdmittedHeaderSemantics,
    citations: Vec<AdmittedCitationSemantic>,
}

/// Validate and extract the semantic fields of an admitted header band.
///
/// Values are taken verbatim and in candidate order — never deduplicated, merged
/// or reordered — and an admitted value that cannot be carried into the emission
/// is a typed [`SemanticFieldLoss`] instead of a silently dropped element.
///
/// The affiliation component *text* is not reachable here: the fulltext input
/// retains only the typed `HeaderCompositionOutcome`, and
/// `RecognizedAffiliationComponent` carries a span, not its text.  Component
/// existence and span legality are therefore owned upstream by `compose_header`
/// (which refuses a non-`Recognized` component or an invalid component span
/// before a header outcome exists at all); this layer conserves and validates the
/// binding the outcome actually carries — a representable key and a valid link
/// span — and a missing component can never be silently replaced by an empty
/// string because no string is invented here.
fn header_semantics(
    candidate: &HeaderCandidate,
) -> Result<AdmittedHeaderSemantics, NativeFulltextCompositionError> {
    for (class, label, values) in [
        (
            SemanticFieldLoss::HeaderTitle,
            "title",
            candidate.fields.title.as_slice(),
        ),
        (
            SemanticFieldLoss::HeaderEmail,
            "email",
            candidate.fields.email.as_slice(),
        ),
        (
            SemanticFieldLoss::HeaderIds,
            "id",
            candidate.fields.ids.as_slice(),
        ),
    ] {
        for value in values {
            if !representable_text(value) {
                return Err(semantic_loss(
                    class,
                    format!(
                        "admitted header {label} value ({} chars) cannot be carried into XML character data",
                        value.chars().count()
                    ),
                ));
            }
        }
    }
    let mut semantics = AdmittedHeaderSemantics {
        titles: candidate.fields.title.clone(),
        emails: candidate.fields.email.clone(),
        ids: candidate.fields.ids.clone(),
        author_affiliations: Vec::new(),
    };
    for author in &candidate.authors {
        for link in &author.affiliations {
            if !representable_attribute(&link.key) {
                return Err(semantic_loss(
                    SemanticFieldLoss::AuthorAffiliation,
                    format!(
                        "author {} affiliation key cannot be carried into an XML attribute",
                        author.ordinal
                    ),
                ));
            }
            if link.affiliation_span.start >= link.affiliation_span.end {
                return Err(semantic_loss(
                    SemanticFieldLoss::AuthorAffiliation,
                    format!(
                        "author {} affiliation link span {}..{} is invalid",
                        author.ordinal, link.affiliation_span.start, link.affiliation_span.end
                    ),
                ));
            }
            semantics
                .author_affiliations
                .push((author.ordinal, link.key.clone()));
        }
    }
    Ok(semantics)
}

/// Validate and extract the bibliography fields of an admitted reference band.
///
/// The candidate's field pool is flat (no record ordinal), so every admitted
/// field must be conserved at the bibliography level; a field whose span is
/// invalid or whose value cannot be represented is a typed refusal rather than a
/// silently dropped role.
fn citation_semantics(
    candidate: &ReferenceCandidate,
) -> Result<Vec<AdmittedCitationSemantic>, NativeFulltextCompositionError> {
    let mut semantics = Vec::with_capacity(candidate.fields.len());
    for field in &candidate.fields {
        let role = citation_field_role_token(field.role);
        if field.span.start >= field.span.end {
            return Err(semantic_loss(
                SemanticFieldLoss::CitationRole,
                format!(
                    "admitted bibliography field ({role}) span {}..{} is invalid",
                    field.span.start, field.span.end
                ),
            ));
        }
        if !representable_text(&field.value) {
            return Err(semantic_loss(
                SemanticFieldLoss::CitationRole,
                format!(
                    "admitted bibliography field ({role}) value cannot be carried into XML character data"
                ),
            ));
        }
        semantics.push(AdmittedCitationSemantic {
            role,
            value: field.value.clone(),
        });
    }
    Ok(semantics)
}

/// Extract the admitted semantics of every admitted lane, validating that each
/// field is representable before anything is spliced.
fn admitted_semantics(
    input: &NativeFulltextCompositionInput,
    header_admitted: bool,
    references_admitted: bool,
) -> Result<AdmittedSemantics, NativeFulltextCompositionError> {
    let mut semantics = AdmittedSemantics::default();
    if header_admitted {
        let HeaderCompositionOutcome::Accepted { composition } = &input.header else {
            return Err(NativeFulltextCompositionError::InvalidAdmission(
                FulltextComponent::Header,
            ));
        };
        semantics.header = header_semantics(&composition.candidate)?;
    }
    if references_admitted {
        let ReferenceCompositionOutcome::Accepted { composition } = &input.references else {
            return Err(NativeFulltextCompositionError::InvalidAdmission(
                FulltextComponent::References,
            ));
        };
        semantics.citations = citation_semantics(&composition.candidate)?;
    }
    Ok(semantics)
}

/// Raw open-to-close content of the element at `index`, markup included.
///
/// The semantic gate compares this against an escaped *single* value, so a
/// nested subtree (whose content contains markup) can never masquerade as a
/// serialized field.
fn element_content<'a>(source: &'a str, document: &XmlDocument, index: usize) -> &'a str {
    let element = &document.elements[index];
    &source[element.open_end..element.close_start]
}

/// Whether `index` is `ancestor` or one of its descendants, by parent links.
fn is_within(document: &XmlDocument, index: usize, ancestor: usize) -> bool {
    let mut current = Some(index);
    while let Some(node) = current {
        if node == ancestor {
            return true;
        }
        current = document.elements[node].parent;
    }
    false
}

/// Whether the emitted document carries an element named `names` whose whole
/// escaped content is exactly `value` inside `ancestor`.
///
/// Raw-content equality, not element-kind presence: the emission conserves the
/// preserved source's own `<title>`/`<email>`/`<idno>` subtrees, so a
/// kind-presence check could be satisfied by source text the admitted band never
/// produced.  Comparing the escaped content also refuses a value that only
/// appears inside an attribute.
fn emission_has_element_text(
    document: &XmlDocument,
    source: &str,
    ancestor: usize,
    names: &[&str],
    value: &str,
) -> bool {
    let escaped = esc(value);
    document
        .elements
        .iter()
        .enumerate()
        .any(|(index, element)| {
            is_within(document, index, ancestor)
                && names.contains(&element.name.as_str())
                && element_content(source, document, index).trim() == escaped
        })
}

/// Whether the emitted document binds `author-{ordinal}` to `key` inside the
/// author element — the same attribute forms the independent oracle reads
/// (`key`/`ref`/`target`/`href`).
fn emission_has_author_affiliation(
    document: &XmlDocument,
    source: &str,
    ordinal: usize,
    key: &str,
) -> bool {
    let author_id = format!("author-{ordinal}");
    let escaped = esc(key);
    document
        .elements
        .iter()
        .enumerate()
        .any(|(index, element)| {
            if element.name != "author"
                || document.xml_id_at(source, index).ok().flatten() != Some(author_id.as_str())
            {
                return false;
            }
            document.elements.iter().enumerate().any(|(child, _)| {
                is_within(document, child, index)
                    && document
                        .attributes_at(source, child)
                        .map(|attributes| {
                            attributes.iter().any(|(name, value)| {
                                matches!(*name, "key" | "ref" | "target" | "href")
                                    && *value == escaped
                            })
                        })
                        .unwrap_or(false)
            })
        })
}

/// Whether the emitted bibliography carries `field`'s value with its role bound:
/// the element whose whole escaped content equals the value, or one of its three
/// nearest ancestors, declares an attribute whose name or value is the role's
/// wire token.  This is the independent oracle's rule, evaluated in production so
/// the gate judges the bytes rather than the fragment it meant to write.
fn emission_has_citation_role(
    document: &XmlDocument,
    source: &str,
    list: usize,
    field: &AdmittedCitationSemantic,
) -> bool {
    let escaped = esc(&field.value);
    document.elements.iter().enumerate().any(|(index, _)| {
        if index == list || !is_within(document, index, list) {
            return false;
        }
        if element_content(source, document, index).trim() != escaped {
            return false;
        }
        let mut current = Some(index);
        for _ in 0..4 {
            let Some(node) = current else {
                return false;
            };
            let bound = document
                .attributes_at(source, node)
                .map(|attributes| {
                    attributes
                        .iter()
                        .any(|(name, value)| *name == field.role || *value == field.role)
                })
                .unwrap_or(false);
            if bound {
                return true;
            }
            current = document.elements[node].parent;
        }
        false
    })
}

/// Re-read the emitted document and verify that every admitted semantic field is
/// present in its bytes.
///
/// S03/T02: this is the third post-serialization gate, beside S01's
/// well-formedness pass and S02's post-splice reference resolve gate.  `Accepted`
/// is a claim about the bytes returned, so the emitter asks the bytes — not its
/// own intention — whether each admitted header title/email/id, each
/// author -> affiliation binding, and each bibliography field with its role
/// reached the emission.  A missing class is a typed refusal and the composer
/// returns the byte-identical Java envelope.  The counts are recomputed from the
/// admitted values and the re-parsed emission; the returned ledger is the
/// observable record, never an input.
fn audit_semantic_fields(
    document: &XmlDocument,
    source: &str,
    admitted: &AdmittedSemantics,
) -> Result<SemanticFieldLedger, NativeFulltextCompositionError> {
    let mut ledger = SemanticFieldLedger {
        admitted_header_titles: admitted.header.titles.len(),
        admitted_header_emails: admitted.header.emails.len(),
        admitted_header_ids: admitted.header.ids.len(),
        admitted_author_affiliations: admitted.header.author_affiliations.len(),
        admitted_citation_fields: admitted.citations.len(),
        ..SemanticFieldLedger::default()
    };
    if ledger.admitted_header_titles > 0
        || ledger.admitted_header_emails > 0
        || ledger.admitted_header_ids > 0
        || ledger.admitted_author_affiliations > 0
    {
        let header = document.element_index("teiHeader")?;
        ledger.conserved_header_titles = admitted
            .header
            .titles
            .iter()
            .filter(|value| emission_has_element_text(document, source, header, &["title"], value))
            .count();
        ledger.conserved_header_emails = admitted
            .header
            .emails
            .iter()
            .filter(|value| emission_has_element_text(document, source, header, &["email"], value))
            .count();
        ledger.conserved_header_ids = admitted
            .header
            .ids
            .iter()
            .filter(|value| {
                emission_has_element_text(document, source, header, &["idno", "pubnum"], value)
            })
            .count();
        ledger.conserved_author_affiliations = admitted
            .header
            .author_affiliations
            .iter()
            .filter(|(ordinal, key)| {
                emission_has_author_affiliation(document, source, *ordinal, key)
            })
            .count();
    }
    if ledger.admitted_citation_fields > 0 {
        let list = document.element_index("listBibl")?;
        ledger.conserved_citation_fields = admitted
            .citations
            .iter()
            .filter(|field| emission_has_citation_role(document, source, list, field))
            .count();
    }
    if let Some(class) = ledger.first_loss() {
        return Err(semantic_loss(
            class,
            format!(
                "post-serialization semantic gate: {}",
                ledger.losses().join("; ")
            ),
        ));
    }
    Ok(ledger)
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
///
/// It returns the structural view it parsed, so the post-splice reference-resolve
/// gate in `compose_inner` reads the emitted document exactly once instead of
/// parsing it a second time to ask the same question.
fn validate_emitted(bytes: &[u8]) -> Result<XmlDocument, NativeFulltextCompositionError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| NativeFulltextCompositionError::InvalidUtf8)?;
    let document = XmlDocument::parse(text).map_err(|error| {
        NativeFulltextCompositionError::ComposedOutputNotWellFormed(format!("{error:?}"))
    })?;
    validate_well_formed(text)?;
    Ok(document)
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
        out.push_str("</surname>");
        // S03/T02: the admitted author->affiliation binding is carried, not
        // just the author line.  The link's key/ordinal/span is all the
        // composer's typed input holds (the affiliation component text is not
        // retained by `RecognizedAffiliationComponent`), and the key is the
        // binding the independent oracle reads.  `header_semantics` already
        // validated the key's representability, the span and the component.
        for link in &a.affiliations {
            out.push_str("<affiliation key=\"");
            out.push_str(&esc(&link.key));
            out.push_str("\"/>");
        }
        out.push_str("</author>");
    }
    // S03/T02: admitted header fields are serialized verbatim, in candidate
    // order, one element per value — never deduplicated, merged or reordered.
    // The A-FIELDS audit measured that these were the fields `header_xml` never
    // carried at all: an `Accepted` emission claimed structure and links, not
    // the semantics the admitted band produced.
    for title in &c.candidate.fields.title {
        out.push_str("<title>");
        out.push_str(&esc(title));
        out.push_str("</title>");
    }
    for email in &c.candidate.fields.email {
        out.push_str("<email>");
        out.push_str(&esc(email));
        out.push_str("</email>");
    }
    for id in &c.candidate.fields.ids {
        out.push_str("<idno type=\"pubnum\">");
        out.push_str(&esc(id));
        out.push_str("</idno>");
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
/// The source bibliography's own published keys, one per direct `listBibl`
/// child, in document order.
///
/// S02/T02: the composer used to publish a synthesized `bibl-{ordinal}` domain
/// while keeping the source body byte-for-byte, so every kept `target="#b0"`
/// became dangling in an emission still reported as `Accepted` (measured on the
/// three sha256-pinned preserved TEI: 217 occurrences, 120 unique mandatory
/// targets, all dangling).  The keys are therefore read from the document the
/// composer is keeping, never invented:
///
/// * a `biblStruct` that declares `xml:id` publishes that identifier;
/// * otherwise it publishes GROBID's positional fallback `b{ordinal}`, the same
///   text `da_ports::citation_link::positional_entry_key` publishes;
/// * a direct child that is not a `biblStruct` cannot be carried under its own
///   key, so it is a typed `InconservableBibliographyChild` instead of a silent
///   drop;
/// * a declared `xml:id` that is not a legal `NCName` cannot be re-emitted as an
///   attribute value and is refused the same way;
/// * two children publishing one key are a typed `BibliographyKeyCollision`: one
///   key is one identity, and publishing both would make a `#fragment` target
///   ambiguous.
fn source_published_keys(
    document: &XmlDocument,
    source: &str,
) -> Result<Vec<String>, NativeFulltextCompositionError> {
    let list = document.element_index("listBibl")?;
    let mut keys: Vec<String> = Vec::new();
    let mut published: BTreeSet<String> = BTreeSet::new();
    for (ordinal, child) in document.element_children_of(list).into_iter().enumerate() {
        if document.elements[child].name != "biblStruct" {
            return Err(
                NativeFulltextCompositionError::InconservableBibliographyChild(format!(
                    "listBibl/{}",
                    document.elements[child].name
                )),
            );
        }
        let key = match document.xml_id_at(source, child)? {
            Some(declared) => {
                if !is_legal_name(declared) {
                    return Err(
                        NativeFulltextCompositionError::InconservableBibliographyChild(format!(
                            "listBibl/biblStruct[{ordinal}] xml:id={declared:?} is not a legal NCName"
                        )),
                    );
                }
                declared.to_owned()
            }
            None => format!("b{ordinal}"),
        };
        if !published.insert(key.clone()) {
            return Err(NativeFulltextCompositionError::BibliographyKeyCollision(
                format!("listBibl/biblStruct[{ordinal}] republishes the key {key:?}"),
            ));
        }
        keys.push(key);
    }
    Ok(keys)
}

/// The obligations the *source* document carries: every `ref` with
/// `type="bibr"` whose `target` is a non-empty local `#fragment`, counted as
/// occurrences plus the distinct fragments they demand.
///
/// This is deliberately independent of `candidate.anchors`.  The pre-splice
/// guard only ever saw the *supplied* anchor stream, so the audit configuration
/// (an empty stream beside a live body link) left it vacuous and a dangling
/// `#b0` travelled into an accepted emission.  The document the composer keeps
/// is the authority on what its own ids must satisfy.
///
/// Both attributes are read through the XML attribute grammar, so
/// `type = 'bibr'` and `target = "#b0"` count exactly like the compact
/// spellings; refs inside comments or CDATA sections are invisible here because
/// the structural scanner never creates elements for them.
fn mandatory_targets(
    document: &XmlDocument,
    source: &str,
) -> Result<(usize, BTreeSet<String>), NativeFulltextCompositionError> {
    let mut occurrences = 0usize;
    let mut targets = BTreeSet::new();
    for index in 0..document.elements.len() {
        if document.elements[index].name != "ref" {
            continue;
        }
        let attributes = document.attributes_at(source, index)?;
        let value = |name: &str| {
            attributes
                .iter()
                .find(|(attribute, _)| *attribute == name)
                .map(|(_, value)| *value)
        };
        if value("type") != Some("bibr") {
            continue;
        }
        let Some(target) = value("target") else {
            continue;
        };
        let Some(fragment) = target.strip_prefix('#') else {
            continue;
        };
        if fragment.is_empty() {
            continue;
        }
        occurrences += 1;
        targets.insert(fragment.to_owned());
    }
    Ok((occurrences, targets))
}

/// The `<listBibl>` fragment the reference band contributes, under the source's
/// own published keys, plus the ledger of what it republished and demanded.
///
/// The domain is checked before anything is emitted: the admitted entries must
/// be exactly the source's direct `biblStruct` children, in source order,
/// because renumbering or reordering them is what makes the kept body's targets
/// dangle.  Supplied anchors must resolve in that same published domain — a
/// target that reaches into a different document is not a link this band can
/// honour.
fn references_xml(
    c: &crate::ReferenceCompositionCandidate,
    document: &XmlDocument,
    source: &str,
) -> Result<(String, ReferenceLinkLedger), NativeFulltextCompositionError> {
    let keys = source_published_keys(document, source)?;
    let references = &c.candidate.references;
    if references.len() != keys.len() {
        return Err(NativeFulltextCompositionError::BibliographyDomainMismatch(
            format!(
                "admitted {} references, the source listBibl declares {} biblStruct children",
                references.len(),
                keys.len()
            ),
        ));
    }
    for (index, reference) in references.iter().enumerate() {
        if reference.ordinal != index {
            return Err(NativeFulltextCompositionError::BibliographyDomainMismatch(
                format!(
                    "admitted reference ordinal {} sits at position {index}; the source order is the key domain",
                    reference.ordinal
                ),
            ));
        }
    }
    let published: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    for anchor in &c.candidate.anchors {
        if let Some(target) = &anchor.target {
            let target = target.strip_prefix('#').unwrap_or(target);
            if !published.contains(target) {
                return Err(
                    NativeFulltextCompositionError::UnresolvedBibliographyTarget(format!(
                        "supplied anchor target #{target} is not published by the source listBibl"
                    )),
                );
            }
        }
    }
    let mut out = String::from("<listBibl>");
    for (key, reference) in keys.iter().zip(references) {
        out.push_str("<biblStruct xml:id=\"");
        out.push_str(key);
        out.push_str("\">");
        out.push_str(&esc(&reference.source));
        out.push_str("</biblStruct>");
    }
    // S03/T02: admitted bibliography fields (role + value) are serialized so the
    // emission carries the citation roles the admitted band produced, not only
    // the reference text.  `ReferenceCandidate.fields` is a flat pool without a
    // record ordinal, so a field cannot be attributed to one `biblStruct`; the
    // fields are conserved at the bibliography level and the post-serialization
    // gate checks each one independently.  `citation_semantics` already refused
    // an unrepresentable value or an invalid span.
    if !c.candidate.fields.is_empty() {
        out.push_str("<noteGrp>");
        for field in &c.candidate.fields {
            out.push_str("<note role=\"");
            out.push_str(citation_field_role_token(field.role));
            out.push_str("\">");
            out.push_str(&esc(&field.value));
            out.push_str("</note>");
        }
        out.push_str("</noteGrp>");
    }
    out.push_str("</listBibl>");
    Ok((
        out,
        ReferenceLinkLedger {
            source_entries: keys.len(),
            admitted_references: references.len(),
            ..ReferenceLinkLedger::default()
        },
    ))
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
    /// Every `xml:id` value in document order — the document's link address
    /// space.  `xml:id` alone, not `id`: without a DTD or schema only `xml:id` is
    /// defined to be of type ID, so a plain `id="b0"` is not something a
    /// `target="#b0"` can address through this module's contract (the same
    /// declared-id form `da_ports::citation_link` publishes).  The uniqueness
    /// gate keeps reading both spellings, because two elements sharing an
    /// identifier is a well-formedness problem whatever the attribute is called.
    xml_ids: Vec<String>,
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
        let mut xml_ids: Vec<String> = Vec::new();
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
                        if attribute == "xml:id" {
                            xml_ids.push(value.to_owned());
                        }
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
        Ok(Self { elements, xml_ids })
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

    /// `(name, value)` pairs of the open tag of the element at `index`, read
    /// through the XML attribute grammar (`Name S? Eq S? AttValue`).
    ///
    /// S02/T02 reads the *source document's own* reference obligations through
    /// this accessor — declared `xml:id`, `type="bibr"`, `target="#b0"` — so a
    /// legal spelling with whitespace around `=` or single quotes is the same
    /// attribute as the compact double-quoted one.  The values are the raw
    /// source bytes; a value that cannot be re-emitted verbatim (an escaped
    /// entity, an illegal `NCName`) is refused by the caller rather than
    /// guessed at.
    ///
    /// The parser requires the open tag to start with the element name, so
    /// `element.name.len()` is an exact char boundary after `<`.
    fn attributes_at<'a>(
        &self,
        source: &'a str,
        index: usize,
    ) -> Result<Vec<(&'a str, &'a str)>, NativeFulltextCompositionError> {
        let element = &self.elements[index];
        let tag = &source[element.start + 1..element.open_end - 1];
        let body = tag_attributes(&tag[element.name.len()..]);
        parse_attributes(body)
    }

    /// Declared `xml:id` of the element at `index`, raw as written, or `None`.
    ///
    /// Only the namespaced spelling counts: it is the declared-id form the
    /// published link contract and the independent oracle both read, and the
    /// only attribute whose ID-ness a DTD-less document defines.
    fn xml_id_at<'a>(
        &self,
        source: &'a str,
        index: usize,
    ) -> Result<Option<&'a str>, NativeFulltextCompositionError> {
        Ok(self
            .attributes_at(source, index)?
            .into_iter()
            .find(|(name, _)| *name == "xml:id")
            .map(|(_, value)| value))
    }

    /// Indexes of the direct element children of the element at `index` in
    /// document order.  Interval children (text, comments, processing
    /// instructions) have no index and are therefore not "children" here, which
    /// is what the bibliography-key domain means by a direct child.
    fn element_children_of(&self, index: usize) -> Vec<usize> {
        (0..self.elements.len())
            .filter(|candidate| self.elements[*candidate].parent == Some(index))
            .collect()
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
