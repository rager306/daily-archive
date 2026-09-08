//! Bounded executable specification for the offline cross-stage checker.
//! All receipts below are already-produced, payload-free values; no producer,
//! parser, filesystem, network, graph, or promotion path is invoked.

use da_parse::{
    AccountedKind, AccountedUnit, AccountingBucket, BucketCount, CrossStageSemanticInvariantsInput,
    DocumentRegion, JavaFallbackEvidence, JavaP2Observation, JavaP4Observation, JavaP6Observation,
    JavaP7Observation, NATIVE_BODY_SCHEMA_VERSION, NATIVE_SEGMENTATION_SCHEMA_VERSION,
    NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION, NativeBodyAccounting, NativeBodySequenceCounts,
    NativeBodySequenceIdentity, NativeBodyShadowSummary, NativeVisualHeadOutcome,
    NativeVisualJavaFallback, NativeVisualMetrics, NativeVisualSequenceIdentity,
    NativeVisualSequenceReceipt, NativeVisualSequenceStep, P5_SHADOW_JOIN_SCHEMA_VERSION,
    P5ShadowJoinIdentity, P5ShadowJoinReport, check_cross_stage_semantic_invariants,
};
use std::collections::BTreeMap;

const DOCUMENT_ID: &str = "cross-stage-example";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn join() -> P5ShadowJoinReport {
    let identity = P5ShadowJoinIdentity {
        document_id: DOCUMENT_ID.into(),
        source_sha256: DIGEST.into(),
        grobid_profile: "structural".into(),
        provider_id: "native-shadow".into(),
        segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
        accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
        boundary_digest: DIGEST.into(),
    };
    let counts = NativeBodySequenceCounts {
        admitted: 2,
        conserved: 2,
        unsupported: 0,
        loss: 0,
    };
    P5ShadowJoinReport {
        schema_version: P5_SHADOW_JOIN_SCHEMA_VERSION.into(),
        identity_match: true,
        identity: identity.clone(),
        java_p5: da_parse::JavaP5Observation {
            paper_id: DOCUMENT_ID.into(),
            grobid_profile: "structural".into(),
            production_authority: "java_http".into(),
            observe_mode: "production_stamp".into(),
            sections_len: 0,
            body_empty: true,
            empty_count_not_success: true,
            native_surface: "none".into(),
        },
        java_p2: JavaP2Observation {
            paper_id: DOCUMENT_ID.into(),
            grobid_profile: "structural".into(),
            java_section_n: 0,
        },
        native_sequence: NativeBodyShadowSummary {
            identity: NativeBodySequenceIdentity {
                document_id: DOCUMENT_ID.into(),
                source_sha256: DIGEST.into(),
                provider_id: "native-shadow".into(),
                segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
                accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
                boundary_digest: DIGEST.into(),
            },
            status: "pass".into(),
            counts: counts.clone(),
        },
        native_status: "pass".into(),
        native_counts: counts,
        java_fallback: JavaFallbackEvidence {
            disposition: "preserved_evidence_only".into(),
            target: "java_grobid".into(),
            source: "production_fallback".into(),
        },
        empty_count_not_success: true,
        redaction: BTreeMap::new(),
    }
}

fn java_observations() -> (JavaP4Observation, JavaP6Observation, JavaP7Observation) {
    let common = (DOCUMENT_ID.to_owned(), "structural".to_owned());
    (
        JavaP4Observation {
            paper_id: common.0.clone(),
            grobid_profile: common.1.clone(),
            java_citation_n: 3,
            production_authority: "java_http".into(),
            observe_mode: "shadow_receipt".into(),
            empty_count_not_success: true,
        },
        JavaP6Observation {
            paper_id: common.0.clone(),
            grobid_profile: common.1.clone(),
            java_figure_n: 1,
            java_table_n: 2,
            java_formula_n: 1,
            figure_typed_absence: false,
            table_typed_absence: false,
            formula_typed_absence: false,
            production_authority: "java_http".into(),
            observe_mode: "shadow_receipt".into(),
            empty_count_not_success: true,
        },
        JavaP7Observation {
            paper_id: common.0,
            grobid_profile: common.1,
            java_callout_n: 2,
            unresolved_n: 1,
            production_authority: "java_http".into(),
            observe_mode: "production_stamp".into(),
            native_surface: "none".into(),
            empty_count_not_success: true,
        },
    )
}

fn accounting() -> NativeBodyAccounting {
    let units = (0..2)
        .map(|n| AccountedUnit {
            page_ordinal: 0,
            source_ordinal: n,
            token_ordinal: n,
            text_digest: DIGEST.into(),
            region: DocumentRegion::Body,
            bucket: AccountingBucket::BodyBlocks,
            kind: AccountedKind::Lexical,
        })
        .collect();
    let mut buckets = BTreeMap::new();
    buckets.insert(
        AccountingBucket::BodyBlocks,
        BucketCount {
            admitted: 2,
            conserved: 2,
            unsupported: 0,
            loss: 0,
        },
    );
    NativeBodyAccounting {
        schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
        document_id: DOCUMENT_ID.into(),
        source_sha256: DIGEST.into(),
        provider_id: "native-shadow".into(),
        source_boundary_digest: DIGEST.into(),
        admitted: 2,
        conserved: 2,
        unsupported: 0,
        loss: 0,
        buckets,
        units,
        assignments: vec![],
    }
}

fn visual() -> NativeVisualSequenceReceipt {
    let metrics = |admitted| NativeVisualMetrics {
        admitted,
        terminal: 0,
        caption_conservation: admitted,
        content_conservation: admitted,
        callout_conservation: admitted,
        geometry_conservation: admitted,
    };
    let terminal = |name: &str| NativeVisualSequenceStep {
        name: name.into(),
        status: "terminal".into(),
        duration_ms: 0,
        failure_code: Some("not_observed".into()),
        outcome: Some(NativeVisualHeadOutcome::Terminal {
            reason: da_parse::NativeVisualAbsence::NotObserved,
            metrics: metrics(0),
        }),
    };
    let mut steps = vec![terminal("figure"), terminal("table"), terminal("formula")];
    steps[0].outcome = Some(NativeVisualHeadOutcome::Terminal {
        reason: da_parse::NativeVisualAbsence::NotObserved,
        metrics: metrics(1),
    });
    NativeVisualSequenceReceipt {
        schema_version: NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION.into(),
        status: "pass".into(),
        identity: NativeVisualSequenceIdentity {
            document_id: DOCUMENT_ID.into(),
            source_sha256: DIGEST.into(),
            provider_id: "native-shadow".into(),
            visual_schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
        },
        steps,
        java_fallback: NativeVisualJavaFallback {
            disposition: "preserved_evidence_only".into(),
            target: "java_grobid".into(),
            source: "production_fallback".into(),
        },
        redaction: BTreeMap::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let join = join();
    let (java_p4, java_p6, java_p7) = java_observations();
    let report = check_cross_stage_semantic_invariants(CrossStageSemanticInvariantsInput {
        join: &join,
        java_p4,
        java_p6,
        java_p7,
        native_accounting: Some(&accounting()),
        native_visual: Some(&visual()),
    })
    .map_err(|error| format!("cross-stage checker rejected synthetic receipts: {error:?}"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
