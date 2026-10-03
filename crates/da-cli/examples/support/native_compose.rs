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

use da_cli::native_class_parity::{NATIVE_COMPOSITION_REFUSED, probe_admissions};
use da_cli::native_composition_measurement::{AdmittedBand, MeasurementError, ProbeSource};
use da_parse::{
    FulltextCompositionIdentity, GROBID_HTTP_BACKEND_ID, NativeFulltextCompositionInput,
    NativeFulltextCompositionOutcome, TeiEnvelope, TeiIdentity, compose_native_fulltext,
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
