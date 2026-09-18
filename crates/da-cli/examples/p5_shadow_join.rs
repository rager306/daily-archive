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
    // GAR-006: the referenced authority registry, the executed fixture and the
    // evaluated population are separate objects, so a zero-count join can never
    // be read as a corpus execution.
    reference_population: serde_json::Value,
    executed_population: serde_json::Value,
    evaluated_population: serde_json::Value,
    outcomes: serde_json::Value,
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

fn parse_report_path() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    // Every arm below either returns or terminates, so this is a single
    // dispatch rather than a loop (`clippy::never_loop`).
    if let Some(arg) = args.next() {
        match arg.as_str() {
            "--report" => return args.next().map(std::path::PathBuf::from),
            "--help" => {
                println!("p5_shadow_join [--report FILE]");
                std::process::exit(0)
            }
            other => panic!("unknown argument {other}"),
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = da_cli::logging::init_pipeline_logging("p5_shadow_join");
    let report_path = parse_report_path();
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
        reference_population: serde_json::json!({
            "identity": "p5_java_authority_split_registry",
            "role": "identity_only",
            "documents": 0,
            "pages": 0,
            "document_lane_outcomes": 0,
            "note": "authority_registry_not_a_document_corpus",
        }),
        executed_population: serde_json::json!({
            "corpus_execution": false,
            "documents": 1,
            "pages": 0,
            "objects": 0,
            "execution_evidence": "tracked_offline_join_fixture:p5-shadow-example",
        }),
        evaluated_population: serde_json::json!({
            "status": "not_evaluated",
            "documents": 0,
            "oracle": "none",
            "reason": "offline_shadow_join_without_corpus_execution",
        }),
        outcomes: serde_json::json!({
            "accounting_contour": {
                "admitted": report.native_counts.admitted,
                "conserved": report.native_counts.conserved,
                "unsupported": report.native_counts.unsupported,
                "loss": report.native_counts.loss,
                "java_p5_sections_len": report.java_p5.sections_len,
                "java_p2_section_n": report.java_p2.java_section_n,
            },
            "quality_claims": [],
        }),
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
    let serialized = serde_json::to_vec_pretty(&output)?;
    if let Some(path) = report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &serialized)?;
    }
    println!("{}", String::from_utf8(serialized)?);
    Ok(())
}
