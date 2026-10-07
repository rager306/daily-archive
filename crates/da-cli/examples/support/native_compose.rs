//! Measurement-plane helper: run the quarantined fulltext composition shadow
//! candidate on one admitted band and return its emission XML.
//!
//! This module deliberately lives outside `crates/da-cli/src`. The fulltext
//! composer is a quarantined shadow candidate, and
//! `architecture_contract::native_fulltext_composition_is_pure_and_stays_outside_every_runtime_route`
//! forbids every runtime seam — including any file under `crates/da-cli/src` —
//! from naming the composer's identifiers. The one call site that must compose a
//! document to *measure* the preservation and extraction thresholds therefore
//! stays in the example/test plane, where the candidate-evidence runners live,
//! instead of in the shipped `da-cli` library. D381 records the rule: the
//! composer-marker scanner may ignore a read-only DATA reference, but a runtime
//! seam that names composer identifiers is exactly the condition for which the
//! exemption must not be broadened.
//!
//! The parent crate's runtime module `da_cli::native_class_parity` keeps the
//! pure projection (`ClassProfile`, `measure_document_class_profiles`, …); only
//! the composer invocation is kept here and shared by both example runners and
//! the integration test through `#[path]`.
//!
//! Because `#[path]`-sharing compiles this module once per including target and
//! each target measures through a different subset of it (an example runner's
//! threshold measurement uses one entry point, the semantic receipt another, a
//! class-parity reproduction test neither), a per-target dead-code warning is
//! expected rather than a defect.
#![allow(dead_code)]

use da_cli::native_class_parity::{NATIVE_COMPOSITION_REFUSED, probe_admissions};
use da_cli::native_composition_measurement::{AdmittedBand, MeasurementError, ProbeSource};
use da_parse::{
    FulltextCompositionIdentity, GROBID_HTTP_BACKEND_ID, NativeFulltextCompositionError,
    NativeFulltextCompositionInput, NativeFulltextCompositionOutcome, NativeFulltextManifest,
    TeiEnvelope, TeiIdentity, compose_native_fulltext,
};

/// Compose one document's admitted band with the quarantine shadow API this
/// module reaches, returning the emission XML.
///
/// A typed composition refusal becomes a named [`MeasurementError`]
/// ([`NATIVE_COMPOSITION_REFUSED`]), never a panic: a document the composer
/// refuses is not measured, and its refusal is carried, not swallowed.
pub fn compose_native_document(
    profile_fingerprint: &str,
    config_fingerprint: &str,
    source: &ProbeSource,
    admitted: &AdmittedBand,
    source_tei_xml: &str,
) -> Result<String, MeasurementError> {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            source_tei_xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, profile_fingerprint.to_string()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: profile_fingerprint.to_string(),
            config_fingerprint: config_fingerprint.to_string(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: probe_admissions(&source.sha256),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            String::from_utf8(candidate.envelope.raw_tei().to_vec()).map_err(|error| {
                MeasurementError::new(format!(
                    "{NATIVE_COMPOSITION_REFUSED}: emitted document is not UTF-8 ({error})"
                ))
            })
        }
        NativeFulltextCompositionOutcome::Rejected { error, .. } => Err(MeasurementError::new(
            format!("{NATIVE_COMPOSITION_REFUSED}: {error:?}"),
        )),
    }
}

/// The typed outcome of one composed document: the composer's manifest (with the
/// `reference_links` ledger), or the typed refusal class.
///
/// `compose_native_document` above collapses a refusal into an opaque
/// [`MeasurementError`] string; the semantic-correctness receipt needs the
/// composer's own typed class and its ledger, so it measures through this
/// function instead. Both share the same input construction. The manifest is
/// boxed so the accepted variant does not dwarf the refusal variant.
pub enum NativeCompositionMeasurement {
    /// The composer accepted the document; the manifest carries the ledger.
    Accepted {
        /// The composer's manifest, including `reference_links`.
        manifest: Box<NativeFulltextManifest>,
    },
    /// The composer refused the document; `class` is the typed variant token.
    Refused {
        /// The typed refusal class token (never a free-form message).
        class: &'static str,
    },
}

/// Compose one document and keep the composer's typed refusal class and manifest.
pub fn compose_native_document_typed(
    profile_fingerprint: &str,
    config_fingerprint: &str,
    source: &ProbeSource,
    admitted: &AdmittedBand,
    source_tei_xml: &str,
) -> NativeCompositionMeasurement {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            source_tei_xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, profile_fingerprint.to_string()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: profile_fingerprint.to_string(),
            config_fingerprint: config_fingerprint.to_string(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: probe_admissions(&source.sha256),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            NativeCompositionMeasurement::Accepted {
                manifest: Box::new(candidate.manifest),
            }
        }
        NativeFulltextCompositionOutcome::Rejected { error, .. } => {
            NativeCompositionMeasurement::Refused {
                class: composition_refusal_class(&error),
            }
        }
    }
}

/// The typed variant token of a composer refusal, spelled once.
fn composition_refusal_class(error: &NativeFulltextCompositionError) -> &'static str {
    use NativeFulltextCompositionError as E;
    match error {
        E::InvalidIdentity(_) => "InvalidIdentity",
        E::WrongBackend => "WrongBackend",
        E::Oversized { .. } => "Oversized",
        E::MissingAdmission(_) => "MissingAdmission",
        E::StaleAdmission(_) => "StaleAdmission",
        E::InvalidAdmission(_) => "InvalidAdmission",
        E::InvalidSchema(_) => "InvalidSchema",
        E::MalformedXml => "MalformedXml",
        E::ForbiddenExternalEntity => "ForbiddenExternalEntity",
        E::DuplicateId => "DuplicateId",
        E::UnbalancedXml => "UnbalancedXml",
        E::AmbiguousSplice(_) => "AmbiguousSplice",
        E::BibliographyDomainMismatch(_) => "BibliographyDomainMismatch",
        E::BibliographyKeyCollision(_) => "BibliographyKeyCollision",
        E::UnresolvedBibliographyTarget(_) => "UnresolvedBibliographyTarget",
        E::InconservableBibliographyChild(_) => "InconservableBibliographyChild",
        E::SemanticFieldLoss { .. } => "SemanticFieldLoss",
        E::ComposedOutputNotWellFormed(_) => "ComposedOutputNotWellFormed",
        E::InvalidUtf8 => "InvalidUtf8",
        E::ChangedReadingOrder => "ChangedReadingOrder",
        E::ArtifactDigestMismatch { .. } => "ArtifactDigestMismatch",
    }
}
