//! Offline, source-bound join of Java P5/P2 observations with a native body receipt.
//!
//! This module is deliberately not part of the parser route. It consumes already
//! produced, payload-free observations and fails closed on every identity drift.

use crate::{
    JavaFallbackEvidence, NATIVE_BODY_SCHEMA_VERSION, NATIVE_BODY_SEQUENCE_SCHEMA_VERSION,
    NATIVE_SEGMENTATION_SCHEMA_VERSION, NativeBodySequenceCounts, NativeBodySequenceReceipt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const P5_SHADOW_JOIN_SCHEMA_VERSION: &str = "da-parse.p5-shadow-join/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct P5ShadowJoinIdentity {
    pub document_id: String,
    pub source_sha256: String,
    pub grobid_profile: String,
    pub provider_id: String,
    pub segmentation_schema_version: String,
    pub accounting_schema_version: String,
    pub boundary_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaP5Observation {
    pub paper_id: String,
    pub grobid_profile: String,
    pub production_authority: String,
    pub observe_mode: String,
    pub sections_len: usize,
    pub body_empty: bool,
    pub empty_count_not_success: bool,
    pub native_surface: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaP2Observation {
    pub paper_id: String,
    pub grobid_profile: String,
    pub java_section_n: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBodyShadowSummary {
    pub identity: crate::NativeBodySequenceIdentity,
    pub status: String,
    pub counts: NativeBodySequenceCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct P5ShadowJoinReport {
    pub schema_version: String,
    pub identity_match: bool,
    pub identity: P5ShadowJoinIdentity,
    pub java_p5: JavaP5Observation,
    pub java_p2: JavaP2Observation,
    pub native_sequence: NativeBodyShadowSummary,
    pub native_status: String,
    pub native_counts: NativeBodySequenceCounts,
    pub java_fallback: JavaFallbackEvidence,
    pub empty_count_not_success: bool,
    pub redaction: BTreeMap<String, bool>,
}

pub struct P5ShadowJoinInput<'a> {
    pub identity: P5ShadowJoinIdentity,
    pub java_p5: JavaP5Observation,
    pub java_p2: Option<JavaP2Observation>,
    pub native_body: Option<&'a NativeBodySequenceReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P5ShadowJoinError {
    MissingIdentity { field: &'static str },
    MalformedIdentity { field: &'static str },
    DocumentMismatch,
    SourceDigestMismatch,
    ProfileMismatch,
    ProviderMismatch,
    SchemaMismatch { field: &'static str },
    BoundaryDigestMismatch,
    AuthorityDrift,
    ObserveModeDrift,
    EmptyCountTreatedAsSuccess,
    NativeSurfaceLive,
    MissingRequiredLeg { leg: &'static str },
    NativeReceiptNotPassing,
}

fn non_empty(value: &str, field: &'static str) -> Result<(), P5ShadowJoinError> {
    if value.trim().is_empty() {
        Err(P5ShadowJoinError::MissingIdentity { field })
    } else {
        Ok(())
    }
}

fn digest(value: &str, field: &'static str) -> Result<(), P5ShadowJoinError> {
    non_empty(value, field)?;
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(P5ShadowJoinError::MalformedIdentity { field });
    }
    Ok(())
}

/// Join existing observations without parsing, I/O, graph writes, or accounting.
/// The returned report is a redacted copy and `identity_match` is not a success
/// claim for the Java body (including when its typed observation is empty).
pub fn join_p5_shadow(
    input: P5ShadowJoinInput<'_>,
) -> Result<P5ShadowJoinReport, P5ShadowJoinError> {
    let id = &input.identity;
    non_empty(&id.document_id, "document_id")?;
    digest(&id.source_sha256, "source_sha256")?;
    non_empty(&id.grobid_profile, "grobid_profile")?;
    non_empty(&id.provider_id, "provider_id")?;
    non_empty(
        &id.segmentation_schema_version,
        "segmentation_schema_version",
    )?;
    non_empty(&id.accounting_schema_version, "accounting_schema_version")?;
    digest(&id.boundary_digest, "boundary_digest")?;
    non_empty(&input.java_p5.paper_id, "p5.paper_id")?;
    non_empty(&input.java_p5.grobid_profile, "p5.grobid_profile")?;
    if input.java_p5.production_authority != "java_http" {
        return Err(P5ShadowJoinError::AuthorityDrift);
    }
    if input.java_p5.observe_mode != "production_stamp" {
        return Err(P5ShadowJoinError::ObserveModeDrift);
    }
    if !input.java_p5.empty_count_not_success {
        return Err(P5ShadowJoinError::EmptyCountTreatedAsSuccess);
    }
    if input.java_p5.native_surface != "none" {
        return Err(P5ShadowJoinError::NativeSurfaceLive);
    }
    if input.java_p5.paper_id != id.document_id {
        return Err(P5ShadowJoinError::DocumentMismatch);
    }
    if input.java_p5.grobid_profile != id.grobid_profile {
        return Err(P5ShadowJoinError::ProfileMismatch);
    }

    let java_p2 = input
        .java_p2
        .ok_or(P5ShadowJoinError::MissingRequiredLeg { leg: "java_p2" })?;
    non_empty(&java_p2.paper_id, "p2.paper_id")?;
    non_empty(&java_p2.grobid_profile, "p2.grobid_profile")?;
    if java_p2.paper_id != id.document_id {
        return Err(P5ShadowJoinError::DocumentMismatch);
    }
    if java_p2.grobid_profile != id.grobid_profile {
        return Err(P5ShadowJoinError::ProfileMismatch);
    }

    let native = input
        .native_body
        .ok_or(P5ShadowJoinError::MissingRequiredLeg {
            leg: "native_body_sequence",
        })?;
    if native.schema_version != NATIVE_BODY_SEQUENCE_SCHEMA_VERSION {
        return Err(P5ShadowJoinError::SchemaMismatch {
            field: "native_sequence.schema_version",
        });
    }
    let native_id = &native.identity;
    non_empty(&native_id.document_id, "native.document_id")?;
    digest(&native_id.source_sha256, "native.source_sha256")?;
    non_empty(&native_id.provider_id, "native.provider_id")?;
    non_empty(
        &native_id.segmentation_schema_version,
        "native.segmentation_schema_version",
    )?;
    non_empty(
        &native_id.accounting_schema_version,
        "native.accounting_schema_version",
    )?;
    digest(&native_id.boundary_digest, "native.boundary_digest")?;
    if native_id.document_id != id.document_id {
        return Err(P5ShadowJoinError::DocumentMismatch);
    }
    if native_id.source_sha256 != id.source_sha256 {
        return Err(P5ShadowJoinError::SourceDigestMismatch);
    }
    if native_id.provider_id != id.provider_id {
        return Err(P5ShadowJoinError::ProviderMismatch);
    }
    if native_id.segmentation_schema_version != NATIVE_SEGMENTATION_SCHEMA_VERSION
        || native_id.segmentation_schema_version != id.segmentation_schema_version
    {
        return Err(P5ShadowJoinError::SchemaMismatch {
            field: "segmentation_schema_version",
        });
    }
    if native_id.accounting_schema_version != NATIVE_BODY_SCHEMA_VERSION
        || native_id.accounting_schema_version != id.accounting_schema_version
    {
        return Err(P5ShadowJoinError::SchemaMismatch {
            field: "accounting_schema_version",
        });
    }
    if native_id.boundary_digest != id.boundary_digest {
        return Err(P5ShadowJoinError::BoundaryDigestMismatch);
    }
    if native.status.is_empty() {
        return Err(P5ShadowJoinError::NativeReceiptNotPassing);
    }

    let native_sequence = NativeBodyShadowSummary {
        identity: native.identity.clone(),
        status: native.status.clone(),
        counts: native.counts.clone(),
    };
    Ok(P5ShadowJoinReport {
        schema_version: P5_SHADOW_JOIN_SCHEMA_VERSION.into(),
        identity_match: true,
        identity: input.identity,
        java_p5: input.java_p5,
        java_p2,
        native_status: native_sequence.status.clone(),
        native_counts: native_sequence.counts.clone(),
        native_sequence,
        java_fallback: native.java_fallback.clone(),
        empty_count_not_success: true,
        redaction: [
            ("body_text", false),
            ("tei_xml", false),
            ("pdf_bytes", false),
            ("physical_paths", false),
            ("hostname", false),
            ("executable", false),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect(),
    })
}
