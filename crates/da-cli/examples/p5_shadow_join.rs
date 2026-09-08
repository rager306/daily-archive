//! Bounded offline P5 shadow join demonstration.
//! This is observability only: it never parses a PDF, calls Java, writes graph
//! state, or invokes the native accountant.

use da_parse::{
    JavaFallbackEvidence, JavaP2Observation, JavaP5Observation, NATIVE_BODY_SCHEMA_VERSION,
    NATIVE_BODY_SEQUENCE_SCHEMA_VERSION, NATIVE_SEGMENTATION_SCHEMA_VERSION,
    NativeBodySequenceCounts, NativeBodySequenceIdentity, NativeBodySequenceReceipt,
    P5ShadowJoinIdentity, P5ShadowJoinInput, join_p5_shadow,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct DigestOnlyOutput<'a> {
    schema_version: &'a str,
    identity_match: bool,
    identity: &'a P5ShadowJoinIdentity,
    java_p5: JavaP5Output,
    java_p2: JavaP2Output,
    native: NativeOutput<'a>,
    empty_count_not_success: bool,
    java_fallback: &'a JavaFallbackEvidence,
    redaction: &'a BTreeMap<String, bool>,
}

#[derive(Serialize)]
struct JavaP5Output {
    sections_len: usize,
    body_empty: bool,
}

#[derive(Serialize)]
struct JavaP2Output {
    java_section_n: usize,
}

#[derive(Serialize)]
struct NativeOutput<'a> {
    status: &'a str,
    counts: &'a NativeBodySequenceCounts,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = da_cli::logging::init_pipeline_logging("p5_shadow_join");
    let identity = P5ShadowJoinIdentity {
        document_id: "shadow-example-document".into(),
        source_sha256: "a".repeat(64),
        grobid_profile: "structural".into(),
        provider_id: "provider-shadow-example".into(),
        segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
        accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
        boundary_digest: "b".repeat(64),
    };
    let receipt = NativeBodySequenceReceipt {
        schema_version: NATIVE_BODY_SEQUENCE_SCHEMA_VERSION.into(),
        status: "pass".into(),
        identity: NativeBodySequenceIdentity {
            document_id: identity.document_id.clone(),
            source_sha256: identity.source_sha256.clone(),
            provider_id: identity.provider_id.clone(),
            segmentation_schema_version: identity.segmentation_schema_version.clone(),
            accounting_schema_version: identity.accounting_schema_version.clone(),
            boundary_digest: identity.boundary_digest.clone(),
        },
        steps: Vec::new(),
        counts: NativeBodySequenceCounts {
            admitted: 0,
            conserved: 0,
            unsupported: 0,
            loss: 0,
        },
        java_fallback: JavaFallbackEvidence {
            disposition: "preserved_evidence_only".into(),
            target: "java_grobid".into(),
            source: "production_fallback".into(),
        },
        redaction: BTreeMap::new(),
    };
    let report = join_p5_shadow(P5ShadowJoinInput {
        identity,
        java_p5: JavaP5Observation {
            paper_id: "shadow-example-document".into(),
            grobid_profile: "structural".into(),
            production_authority: "java_http".into(),
            observe_mode: "production_stamp".into(),
            sections_len: 0,
            body_empty: true,
            empty_count_not_success: true,
            native_surface: "none".into(),
        },
        java_p2: Some(JavaP2Observation {
            paper_id: "shadow-example-document".into(),
            grobid_profile: "structural".into(),
            java_section_n: 0,
        }),
        native_body: Some(&receipt),
    })
    .map_err(|error| format!("offline shadow join rejected: {error:?}"))?;

    let output = DigestOnlyOutput {
        schema_version: &report.schema_version,
        identity_match: report.identity_match,
        identity: &report.identity,
        java_p5: JavaP5Output {
            sections_len: report.java_p5.sections_len,
            body_empty: report.java_p5.body_empty,
        },
        java_p2: JavaP2Output {
            java_section_n: report.java_p2.java_section_n,
        },
        native: NativeOutput {
            status: &report.native_status,
            counts: &report.native_counts,
        },
        empty_count_not_success: report.empty_count_not_success,
        java_fallback: &report.java_fallback,
        redaction: &report.redaction,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
