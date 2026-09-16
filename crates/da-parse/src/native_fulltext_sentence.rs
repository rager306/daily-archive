//! The two body steps that are **not** CRF families, plus the body version surface.
//!
//! ## Why this module states a gap instead of porting a detector
//!
//! The S05 sketch asks for sentence and language coverage, but GROBID 0.9.x has
//! no sentence CRF and no language CRF in the body pipeline:
//!
//! - `SentenceUtilities.runSentenceDetection` is applied by the **TEI
//!   formatter**, not by `FullTextParser`: the body CRF labels paragraphs,
//!   items, sections and markers, and sentence splitting happens later, only
//!   for the `segmentSentences` request switch (the V1 wire dialect carries it
//!   as `FulltextDialectV1::segment_sentences`);
//! - `LanguageUtilities` is not a CRF family and is barely called from
//!   `FullTextParser`.
//!
//! Porting "as GROBID" without such a resource would be a *silent
//! approximation* — exactly what this wave's success criteria forbid. So these
//! two steps are modelled as typed **outcomes** with no success variant at all:
//! a step can report that it was not asked, that no resource is configured, or
//! that this band has no native producer for it. It can never report that it
//! produced sentences or languages, because it does not.
//!
//! ## The three outcomes, and why the refusal is structural
//!
//! | Requested | Resource | Outcome |
//! |---|---|---|
//! | no | any | [`NativeBodyStepOutcome::NotRequested`] |
//! | yes | absent (or blank) | [`NativeBodyStepOutcome::MissingResource`] |
//! | yes | configured | [`NativeBodyStepOutcome::Unsupported`] |
//!
//! [`NativeBodyStepOutcome::is_productive`] is `false` for every variant, so a
//! consumer cannot mistake a gap for output. Adding a real detector later means
//! adding an outcome variant **and** bumping
//! [`NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION`]; it can never be done by
//! widening an existing variant.
//!
//! ## Body versioning surface
//!
//! [`native_body_version_surface`] publishes the schema identities of the body
//! band's own surfaces (printed features, decoded sequence, reading order, and
//! this non-CRF step surface). Two adjacent concerns are deliberately **not**
//! listed as body surfaces:
//!
//! - **Abstract is a header label.** `<abstract>` / `I-<abstract>` live in the
//!   S03 header dictionary ([`crate::NATIVE_HEADER_LABELS`]); the body
//!   dictionary ([`crate::NATIVE_FULLTEXT_BASE_LABELS`]) has no abstract label;
//! - **PDF version vs TEI body is an envelope concern.** Which bytes reach the
//!   band is decided by the request dialect and cache identity, not by a CRF
//!   family, so it belongs to the parse envelope and not to this version
//!   ledger.
//!
//! Both are pinned as data in [`NATIVE_BODY_ADJACENT_SURFACES`] so the
//! classification is checked by a test rather than asserted only in prose.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no model bytes,
//! no logging, no absolute path. Every function is total — a missing resource, a
//! blank resource id and an unrequested step all return a typed outcome and
//! never panic. `import_eligible` stays false and nothing here is spliced into
//! the Java-authoritative parse route.

use serde::{Deserialize, Serialize};

/// Schema identity of the non-CRF body step surface.
///
/// The sentence and language steps share one identity on purpose: neither is a
/// CRF family, both are stated as named gaps, and a real producer for either
/// must bump this constant.
pub const NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION: &str = "da-parse.native-fulltext-sentence/v1";

/// Resource a native sentence step would have to be configured with.
///
/// GROBID owns no sentence *model file* — sentence detection is a formatter
/// utility — so this names the capability, not a path.
pub const NATIVE_SENTENCE_RESOURCE_NAME: &str = "sentence-detection";

/// Resource a native language step would have to be configured with.
pub const NATIVE_LANGUAGE_RESOURCE_NAME: &str = "language-identification";

/// Stable diagnostic for the sentence step's named gap.
pub const NATIVE_SENTENCE_GAP_REASON: &str = "GROBID applies sentence detection in the TEI formatter (SentenceUtilities.runSentenceDetection), not in a body CRF family; this band has no native sentence detector, so a configured resource is refused rather than guessed at";

/// Stable diagnostic for the language step's named gap.
pub const NATIVE_LANGUAGE_GAP_REASON: &str = "GROBID's LanguageUtilities is not a CRF family and FullTextParser barely calls it; this band has no native language identifier, so a configured resource is refused rather than guessed at";

/// Surfaces that are adjacent to the body band but are not body CRF families,
/// as `(surface, owner)`.
///
/// Data, not prose: the contract test asserts that `<abstract>` is a header
/// label and absent from the body dictionary, and that the PDF-version / TEI
/// body difference is owned by the parse envelope.
pub const NATIVE_BODY_ADJACENT_SURFACES: [(&str, &str); 2] = [
    ("abstract", "header-label"),
    ("pdf_version_vs_tei_body", "parse-envelope"),
];

/// Which of the two non-CRF body steps a receipt or outcome belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeBodyStep {
    Sentence,
    Language,
}

impl NativeBodyStep {
    /// Stable wire/diagnostic name of the step.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sentence => "sentence",
            Self::Language => "language",
        }
    }

    /// Resource this step would have to be configured with.
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::Sentence => NATIVE_SENTENCE_RESOURCE_NAME,
            Self::Language => NATIVE_LANGUAGE_RESOURCE_NAME,
        }
    }

    /// Stable named-gap diagnostic of this step.
    pub const fn gap_reason(self) -> &'static str {
        match self {
            Self::Sentence => NATIVE_SENTENCE_GAP_REASON,
            Self::Language => NATIVE_LANGUAGE_GAP_REASON,
        }
    }
}

/// Typed outcome of a non-CRF body step.
///
/// There is deliberately **no success variant**: this band implements neither a
/// sentence detector nor a language identifier, so the only honest reports are
/// "not asked", "no resource", and "no native producer". See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeBodyStepOutcome {
    /// The caller did not request the step.
    NotRequested,
    /// The step was requested but no resource is configured; `resource` names
    /// what would have to be configured. An absent *and* a blank resource id
    /// both land here, so a misconfigured id cannot masquerade as a resource.
    MissingResource {
        /// Required resource name (never the blank id the caller supplied).
        resource: String,
    },
    /// The step was requested with a resource, but this band has no native
    /// producer for it: refuse with the configured id and the named gap
    /// instead of substituting a heuristic.
    Unsupported {
        /// The configured resource id that was refused.
        resource: String,
        /// Stable named-gap diagnostic ([`NativeBodyStep::gap_reason`]).
        reason: String,
    },
}

impl NativeBodyStepOutcome {
    /// Whether this outcome carries produced sentences/languages.
    ///
    /// Always `false` today: no variant is productive. The method exists so a
    /// caller can branch on productivity instead of pattern-matching, and so a
    /// future productive variant cannot be added without this method becoming
    /// a deliberate lie (the contract test pins it).
    pub const fn is_productive(&self) -> bool {
        false
    }
}

/// One non-CRF step's self-describing receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBodyStepReceipt {
    pub schema_version: String,
    pub step: NativeBodyStep,
    pub outcome: NativeBodyStepOutcome,
}

impl NativeBodyStepReceipt {
    /// Whether the step produced sentences/languages (delegates to
    /// [`NativeBodyStepOutcome::is_productive`]).
    pub const fn is_productive(&self) -> bool {
        self.outcome.is_productive()
    }
}

/// The sentence-segmentation step as the caller declares it.
///
/// `requested` mirrors the `segmentSentences` switch of the V1 fulltext request
/// dialect; `resource` is the optional native-side producer id. The step is a
/// plain data carrier: it holds no connection and reads no file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSentenceStep {
    pub requested: bool,
    pub resource: Option<String>,
}

impl NativeSentenceStep {
    /// The step was not requested at all.
    pub const fn not_requested() -> Self {
        Self {
            requested: false,
            resource: None,
        }
    }

    /// The step was requested without any native resource configured.
    pub const fn requested_without_resource() -> Self {
        Self {
            requested: true,
            resource: None,
        }
    }

    /// The step was requested with a native resource id.
    pub fn requested_with_resource(resource: impl Into<String>) -> Self {
        Self {
            requested: true,
            resource: Some(resource.into()),
        }
    }

    /// Evaluate the step. Total: never panics, never returns a success.
    pub fn run(&self) -> NativeBodyStepOutcome {
        evaluate_body_step(
            NativeBodyStep::Sentence,
            self.requested,
            self.resource.as_deref(),
        )
    }

    /// Evaluate the step into a self-describing receipt.
    pub fn receipt(&self) -> NativeBodyStepReceipt {
        NativeBodyStepReceipt {
            schema_version: NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION.into(),
            step: NativeBodyStep::Sentence,
            outcome: self.run(),
        }
    }
}

/// The language-identification step as the caller declares it.
///
/// The V1 dialect has no language switch, so `requested` is caller intent
/// rather than a mirrored wire field; the step is nonetheless explicit so an
/// unrequested language step is distinguishable from a refused one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLanguageStep {
    pub requested: bool,
    pub resource: Option<String>,
}

impl NativeLanguageStep {
    /// The step was not requested at all.
    pub const fn not_requested() -> Self {
        Self {
            requested: false,
            resource: None,
        }
    }

    /// The step was requested without any native resource configured.
    pub const fn requested_without_resource() -> Self {
        Self {
            requested: true,
            resource: None,
        }
    }

    /// The step was requested with a native resource id.
    pub fn requested_with_resource(resource: impl Into<String>) -> Self {
        Self {
            requested: true,
            resource: Some(resource.into()),
        }
    }

    /// Evaluate the step. Total: never panics, never returns a success.
    pub fn run(&self) -> NativeBodyStepOutcome {
        evaluate_body_step(
            NativeBodyStep::Language,
            self.requested,
            self.resource.as_deref(),
        )
    }

    /// Evaluate the step into a self-describing receipt.
    pub fn receipt(&self) -> NativeBodyStepReceipt {
        NativeBodyStepReceipt {
            schema_version: NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION.into(),
            step: NativeBodyStep::Language,
            outcome: self.run(),
        }
    }
}

/// Shared evaluation of a non-CRF body step (one rule for both steps).
fn evaluate_body_step(
    step: NativeBodyStep,
    requested: bool,
    resource: Option<&str>,
) -> NativeBodyStepOutcome {
    if !requested {
        return NativeBodyStepOutcome::NotRequested;
    }
    match resource.map(str::trim) {
        None | Some("") => NativeBodyStepOutcome::MissingResource {
            resource: step.resource_name().into(),
        },
        Some(id) => NativeBodyStepOutcome::Unsupported {
            resource: id.into(),
            reason: step.gap_reason().into(),
        },
    }
}

/// What proof obligation a versioned body surface carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeBodySurfaceKind {
    /// The printed model-facing feature ABI.
    Features,
    /// The decoded CRF label sequence.
    Sequence,
    /// The cross-page reading-order glue.
    ReadingOrder,
    /// A step that is not a CRF family and has no native producer here.
    NonCrfStep,
}

impl NativeBodySurfaceKind {
    /// Stable diagnostic name of the kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Features => "features",
            Self::Sequence => "sequence",
            Self::ReadingOrder => "reading_order",
            Self::NonCrfStep => "non_crf_step",
        }
    }
}

/// One versioned surface of the native body band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBodyVersionSurface {
    pub surface: String,
    pub schema_version: String,
    pub kind: NativeBodySurfaceKind,
}

/// The body band's version ledger, in a stable order.
///
/// Only the band's own surfaces are listed. Adjacent concerns — the abstract
/// label (header, S03) and the PDF-version / TEI-body difference (parse
/// envelope) — are classified in [`NATIVE_BODY_ADJACENT_SURFACES`] instead, so
/// the ledger cannot be read as if the body band owned them.
pub fn native_body_version_surface() -> Vec<NativeBodyVersionSurface> {
    vec![
        version_surface(
            "features",
            crate::NATIVE_FULLTEXT_FEATURE_SCHEMA_VERSION,
            NativeBodySurfaceKind::Features,
        ),
        version_surface(
            "sequence",
            crate::NATIVE_FULLTEXT_SEQUENCE_SCHEMA_VERSION,
            NativeBodySurfaceKind::Sequence,
        ),
        version_surface(
            "reading_order",
            crate::NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION,
            NativeBodySurfaceKind::ReadingOrder,
        ),
        version_surface(
            "sentence_language",
            NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION,
            NativeBodySurfaceKind::NonCrfStep,
        ),
    ]
}

fn version_surface(
    surface: &str,
    schema_version: &str,
    kind: NativeBodySurfaceKind,
) -> NativeBodyVersionSurface {
    NativeBodyVersionSurface {
        surface: surface.into(),
        schema_version: schema_version.into(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrequested_steps_report_not_requested_even_with_a_resource() {
        let sentence = NativeSentenceStep::not_requested();
        assert_eq!(sentence.run(), NativeBodyStepOutcome::NotRequested);
        let mut declared = NativeSentenceStep::requested_with_resource("lexicon/x");
        declared.requested = false;
        assert_eq!(declared.run(), NativeBodyStepOutcome::NotRequested);
        assert_eq!(
            NativeLanguageStep::not_requested().run(),
            NativeBodyStepOutcome::NotRequested
        );
    }

    #[test]
    fn requested_without_a_resource_reports_the_required_resource() {
        assert_eq!(
            NativeSentenceStep::requested_without_resource().run(),
            NativeBodyStepOutcome::MissingResource {
                resource: NATIVE_SENTENCE_RESOURCE_NAME.into()
            }
        );
        assert_eq!(
            NativeLanguageStep::requested_without_resource().run(),
            NativeBodyStepOutcome::MissingResource {
                resource: NATIVE_LANGUAGE_RESOURCE_NAME.into()
            }
        );
    }

    #[test]
    fn a_blank_resource_id_is_not_a_configured_resource() {
        for blank in ["", "   ", "\t"] {
            assert!(matches!(
                NativeSentenceStep::requested_with_resource(blank).run(),
                NativeBodyStepOutcome::MissingResource { .. }
            ));
        }
        assert_eq!(
            NativeLanguageStep::requested_with_resource(" ").run(),
            NativeBodyStepOutcome::MissingResource {
                resource: NATIVE_LANGUAGE_RESOURCE_NAME.into()
            }
        );
    }

    #[test]
    fn a_configured_resource_is_refused_with_its_named_gap() {
        assert_eq!(
            NativeSentenceStep::requested_with_resource("lexicon/sentence").run(),
            NativeBodyStepOutcome::Unsupported {
                resource: "lexicon/sentence".into(),
                reason: NATIVE_SENTENCE_GAP_REASON.into()
            }
        );
        assert_eq!(
            NativeLanguageStep::requested_with_resource("models/lang").run(),
            NativeBodyStepOutcome::Unsupported {
                resource: "models/lang".into(),
                reason: NATIVE_LANGUAGE_GAP_REASON.into()
            }
        );
    }

    #[test]
    fn no_outcome_is_productive() {
        let outcomes = [
            NativeSentenceStep::not_requested().run(),
            NativeSentenceStep::requested_without_resource().run(),
            NativeSentenceStep::requested_with_resource("x").run(),
        ];
        for outcome in &outcomes {
            assert!(!outcome.is_productive(), "{outcome:?} claimed output");
        }
        assert!(
            !NativeSentenceStep::requested_with_resource("x")
                .receipt()
                .is_productive()
        );
    }

    #[test]
    fn receipts_carry_the_step_identity_and_schema_version() {
        let receipt = NativeLanguageStep::requested_without_resource().receipt();
        assert_eq!(
            receipt.schema_version,
            NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION
        );
        assert_eq!(receipt.step, NativeBodyStep::Language);
        assert_eq!(receipt.step.as_str(), "language");
        assert_eq!(receipt.step.resource_name(), NATIVE_LANGUAGE_RESOURCE_NAME);
    }

    #[test]
    fn version_surface_lists_the_band_and_classifies_the_adjacent_concerns() {
        let surface = native_body_version_surface();
        let names: Vec<&str> = surface.iter().map(|entry| entry.surface.as_str()).collect();
        assert_eq!(
            names,
            ["features", "sequence", "reading_order", "sentence_language"]
        );
        assert!(surface.iter().all(|entry| {
            entry
                .schema_version
                .starts_with("da-parse.native-fulltext-")
        }));
        assert_eq!(surface[3].kind, NativeBodySurfaceKind::NonCrfStep);
        assert_eq!(surface[3].kind.as_str(), "non_crf_step");
        assert!(
            native_body_version_surface().contains(&NativeBodyVersionSurface {
                surface: "reading_order".into(),
                schema_version: crate::NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION.into(),
                kind: NativeBodySurfaceKind::ReadingOrder,
            })
        );
        assert_eq!(
            NATIVE_BODY_ADJACENT_SURFACES,
            [
                ("abstract", "header-label"),
                ("pdf_version_vs_tei_body", "parse-envelope")
            ]
        );
    }

    #[test]
    fn evaluating_a_step_is_deterministic_and_panic_free() {
        let step = NativeSentenceStep::requested_with_resource("日本語");
        assert_eq!(step.run(), step.run());
        // Multibyte, empty and absurdly long ids are all just data.
        assert!(
            !NativeLanguageStep::requested_with_resource("x".repeat(4096))
                .run()
                .is_productive()
        );
    }
}
