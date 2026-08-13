//! ADR-051 contract suite for ACAP physical replay evidence (M371/S07/T02).
//!
//! Proves:
//! - MutationCounters map one-to-one into domain ReplayObservation
//! - reports are self-validating after serialization (full partition preserved)
//! - NetWrites/WriteDelta alone cannot satisfy integrity
//! - legacy v1 artifacts without replay_observation deserialize and fail integrity
//! - valid zero-write replay with attempted>0 and unchanged>0 passes

use da_application::assurance::{
    MaterializationAssuranceUseCase, MaterializationReadback, MaterializationReplayObservation,
    MaterializedSectionObservation,
};
use da_domain::{
    ArticleAssuranceReport, ArticleContextState, AssuranceVerdict, ContentDigest,
    LayerAssuranceResult, MutationFamilyObservation, ReplayObservation, WriteDelta,
};
use da_ports::parser::{CitationEntry, ParsedArticle, Section};
use da_ports::{MutationCounters, MutationFamilyCounters};
use std::collections::BTreeMap;

fn parsed() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2507.19457".into(),
        title: "GEPA".into(),
        abstract_text: "ABSTRACT".into(),
        body_text: "BODY".into(),
        sections: vec![Section {
            title: "Intro".into(),
            text: "BODY".into(),
            level: 1,
            section_id: Some("sec-intro".into()),
            ..Default::default()
        }],
        citations: vec![CitationEntry {
            raw_text: "Ref".into(),
            doi: None,
            arxiv_id: None,
            title: Some("Ref".into()),
        }],
        layout_json: None,
        tei_xml: None,
        pdf_hash: "src".into(),
    }
}

fn ingest() -> da_application::ingest::IngestResult {
    da_application::ingest::IngestResult {
        paper_id: "2507.19457".into(),
        title: "GEPA".into(),
        body_chars: 4,
        section_count: 1,
        sections_written: 1,
        citation_count: 1,
        references_written: 1,
        cites_resolved: 0,
        vector_dimensions: 0,
        graph_node_id: Some(1),
        revision_vid: None,
        import_eligible: false,
        pdf_classification: None,
        pdf_triage: da_domain::PdfTriageStatus::NotConfigured,
    }
}

fn readback() -> MaterializationReadback {
    MaterializationReadback {
        abstract_text: Some("ABSTRACT".into()),
        sections: vec![MaterializedSectionObservation {
            ordinal: 0,
            text: "BODY".into(),
        }],
        paper_section_count: Some(1),
        paper_citation_count: Some(1),
        import_eligible_true_nodes: 0,
    }
}

fn valid_counters(unchanged: u64) -> MutationCounters {
    MutationCounters {
        property: MutationFamilyCounters {
            attempted: unchanged,
            applied: 0,
            unchanged,
            rejected: 0,
        },
        ..MutationCounters::default()
    }
}

#[test]
fn mutation_counters_map_one_to_one_into_domain_replay_observation() {
    let counters = MutationCounters {
        node: MutationFamilyCounters {
            attempted: 3,
            applied: 1,
            unchanged: 1,
            rejected: 1,
        },
        property: MutationFamilyCounters {
            attempted: 10,
            applied: 0,
            unchanged: 10,
            rejected: 0,
        },
        edge: MutationFamilyCounters {
            attempted: 2,
            applied: 0,
            unchanged: 2,
            rejected: 0,
        },
        vector: MutationFamilyCounters {
            attempted: 1,
            applied: 0,
            unchanged: 1,
            rejected: 0,
        },
        unobserved_operations: 4,
    };
    let obs = match MaterializationReplayObservation::from_mutations(counters) {
        MaterializationReplayObservation::Observed(obs) => obs,
        MaterializationReplayObservation::NotRun => panic!("expected Observed"),
    };
    assert_eq!(obs.node.attempted, 3);
    assert_eq!(obs.node.applied, 1);
    assert_eq!(obs.node.unchanged, 1);
    assert_eq!(obs.node.rejected, 1);
    assert_eq!(obs.property.unchanged, 10);
    assert_eq!(obs.edge.unchanged, 2);
    assert_eq!(obs.vector.unchanged, 1);
    assert_eq!(obs.unobserved_operations, 4);
    assert!(!obs.integrity_passes());
}

#[test]
fn valid_replay_report_is_self_validating_after_serde() {
    let report = MaterializationAssuranceUseCase::new()
        .evaluate(
            &parsed(),
            &ingest(),
            &readback(),
            MaterializationReplayObservation::from_mutations(valid_counters(270)),
            ContentDigest::of_text("policy"),
        )
        .unwrap();
    assert_eq!(report.verdict, AssuranceVerdict::Pass);
    let obs = report.layer_results[0]
        .replay_observation
        .expect("full observation required");
    assert!(obs.integrity_passes());
    assert_eq!(obs.property.unchanged, 270);

    // After serialization, the report must still recompute integrity without
    // any in-memory MutationCounters.
    let json = serde_json::to_vec(&report).unwrap();
    let back: ArticleAssuranceReport = serde_json::from_slice(&json).unwrap();
    let mut layer = back.layer_results[0].clone();
    layer.evaluate_hard_failures();
    assert!(
        layer.hard_failures.is_empty(),
        "failures: {:?}",
        layer.hard_failures
    );
    assert!(layer.replay_observation.unwrap().integrity_passes());
}

#[test]
fn write_delta_alone_cannot_prove_integrity() {
    // Zero WriteDelta with no full observation is incomplete.
    let mut layer = LayerAssuranceResult {
        layer_id: "materialization".into(),
        state: ArticleContextState::Materialized,
        article_count: 1,
        available_required_fields: 1,
        retained_required_fields: 1,
        conservations: vec![],
        counts: vec![],
        replay_writes: Some(WriteDelta::default()),
        replay_observation: None,
        replay_missingness: None,
        d127_unlock_count: 0,
        quality_metrics: BTreeMap::new(),
        hard_failures: vec![],
    };
    layer.evaluate_hard_failures();
    assert!(
        layer
            .hard_failures
            .contains(&"replay_observation_integrity:legacy_v1_incomplete".into())
    );
}

#[test]
fn legacy_v1_materialization_batch_artifact_deserializes_and_flags_incomplete() {
    // Real durable artifact: replay_writes present, replay_observation absent.
    let artifact = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/article_context/materialization_batch_01.json");
    let raw = std::fs::read_to_string(&artifact)
        .unwrap_or_else(|e| panic!("batch artifact {}: {e}", artifact.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let paper = &value["materialization"]["papers"][0];
    let layer_json = &paper["assurance"]["layer_results"][0];
    let mut layer: LayerAssuranceResult =
        serde_json::from_value(layer_json.clone()).expect("deserialize layer");
    assert!(
        layer.replay_observation.is_none(),
        "v1 artifact must not have replay_observation"
    );
    assert!(layer.replay_writes.is_some());
    // The embedded replay_mutations on the paper (outside assurance) still
    // has the full partition — prove we can rebuild ReplayObservation from it.
    let mutations = &paper["replay_mutations"];
    let rebuilt = ReplayObservation {
        node: family_from_json(&mutations["node"]),
        property: family_from_json(&mutations["property"]),
        edge: family_from_json(&mutations["edge"]),
        vector: family_from_json(&mutations["vector"]),
        unobserved_operations: mutations["unobserved_operations"].as_u64().unwrap(),
    };
    assert!(
        rebuilt.integrity_passes(),
        "batch paper0 replay_mutations must be a valid zero-write partition"
    );

    // Without attaching the full observation, the layer fails integrity.
    layer.evaluate_hard_failures();
    assert!(
        layer
            .hard_failures
            .contains(&"replay_observation_integrity:legacy_v1_incomplete".into())
    );

    // Attaching the rebuilt observation makes the layer self-validating.
    layer.replay_observation = Some(rebuilt);
    layer.hard_failures.clear();
    layer.evaluate_hard_failures();
    assert!(
        !layer.hard_failures.iter().any(|f| f.starts_with("replay_")),
        "failures: {:?}",
        layer.hard_failures
    );
}

fn family_from_json(v: &serde_json::Value) -> MutationFamilyObservation {
    MutationFamilyObservation {
        attempted: v["attempted"].as_u64().unwrap(),
        applied: v["applied"].as_u64().unwrap(),
        unchanged: v["unchanged"].as_u64().unwrap(),
        rejected: v["rejected"].as_u64().unwrap(),
    }
}

#[test]
fn net_writes_diagnostic_projection_excludes_rejected_and_cannot_pass_integrity() {
    let obs = ReplayObservation {
        property: MutationFamilyObservation {
            attempted: 3,
            applied: 0,
            unchanged: 0,
            rejected: 3,
        },
        ..ReplayObservation::default()
    };
    let delta = obs.applied_write_delta();
    assert_eq!(delta.total(), 0); // applied only
    assert_eq!(delta.rejected_writes, 3);
    assert!(!obs.integrity_passes());
}

#[test]
fn unobserved_operations_fail_integrity_even_with_zero_applied() {
    let mut counters = valid_counters(5);
    counters.unobserved_operations = 1;
    let report = MaterializationAssuranceUseCase::new()
        .evaluate(
            &parsed(),
            &ingest(),
            &readback(),
            MaterializationReplayObservation::from_mutations(counters),
            ContentDigest::of_text("policy"),
        )
        .unwrap();
    assert_eq!(report.verdict, AssuranceVerdict::Fail);
    assert!(
        report.layer_results[0]
            .hard_failures
            .contains(&"replay_observation_integrity:unobserved:1".into())
    );
}
