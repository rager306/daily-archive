//! Library surface for pure CLI helpers (tests without binary entry).

pub mod acquire_arxiv;
pub mod agent_run;
pub mod audit_fields;
pub mod background_tasks;
pub mod batch_ids;
pub mod catalog_drift;
pub mod catalog_materialize;
pub mod catalog_regression;
pub mod catalog_sweep;
pub mod gold_corpus;
pub mod gold_graph;
pub mod graph_query;
pub mod graph_tasks;
pub mod health_report;
// S08/T01: layout_runtime holds the qualified CPU characterization primitive
// (`characterize_cpu_kernel`, `gate_simd_candidate`) that the native core
// profile consumes. It is pure std/serde/sha2, so it stays available in the
// default feature set: the profile module, its example, and its contract test
// must compile without `pdf-layout`.
pub mod layout_runtime;
pub mod load;
pub mod load_journey;
pub mod logging;
pub mod materialization_progress;
/// S08/T01 native CRF core profile and closed SIMD/PGO disposition.
pub mod native_core_profile;
// S08/T02: bounded SMP qualification of the same native CRF core over the
// worker matrix `1..=physical_core_cap`. Pure std/serde/sha2 + layout_runtime,
// so the module, its example and its contract test compile in the default
// feature set (no `pdf-layout`).
pub mod native_core_smp;
// S08/T03: measured Java work (per-model CRF invocations) projected from a
// verified GROBID stage tape. Pure serde/sha2 over `da_parse::StageTape`, so
// the module, its example and its contract test compile in the default feature
// set (no `pdf-layout`).
pub mod native_java_work;
// S08/T04: route-recovery ledger. Joins the closed P0..P9 cascade skeleton, the
// component admission matrix and the S08/T03 measured Java-work receipt into
// per-stage recovery verdicts. Pure serde/serde_yaml/sha2, so the module, its
// example and its contract test compile in the default feature set (no
// `pdf-layout`), and nothing here is a production route.
pub mod native_route_recovery;
pub mod parse_batch;
pub mod parse_config;
pub mod parse_job;
pub mod parse_profile;
/// SUT-free reference producer for the closed parse-policy ablation (S07/T04).
pub mod policy_ablation_reference;
pub mod production;
pub mod snapshot_identity;
pub mod snapshot_lineage;
pub mod source_acquire;
pub mod source_cli;
pub mod summarize_revisions;
// S02/T01: donor visual sidecar → native visual observation converter. Every
// type it consumes (`da_adapters::VisualObservation`,
// `da_adapters::LayoutShadowObservation`) exists only with `pdf-layout`, and
// `default = []`, so this module must stay out of the default feature set. It
// is deliberately gated (unlike `layout_runtime`, which M507/S08 ungated): the
// gate keeps pdf-inspector out of the default `cargo check -p da-cli`, while
// the existing governor step `"da-cli feature-gated example contracts"`
// (`cargo test -p da-cli --features pdf-layout --tests`) still executes the
// converted unit tests through the `unittests src/lib.rs` target.
#[cfg(feature = "pdf-layout")]
pub mod visual_adapter;

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
#[cfg(test)]
pub(crate) fn roundtrip_json<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json1).expect("deserialize failed");
    let json2 = serde_json::to_string(&back).expect("re-serialize failed");
    assert_eq!(json1, json2, "roundtrip JSON mismatch");
}

#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;
    use crate::graph_tasks::{GraphTask, GraphTaskSuite, TaskVerdict};
    use da_domain::graph_tasks::GraphTaskClass;

    #[test]
    fn roundtrip_graph_task_basic() {
        roundtrip_json(&GraphTask {
            id: "paper_count".into(),
            title: "Paper count | Scale".into(),
            insight: "Scale check".into(),
            cypher: "MATCH (n:Paper) RETURN count(n)".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: None,
            expect_min_value: None,
            expect_value: None,
            class: GraphTaskClass::default_for_legacy(),
        });
    }

    #[test]
    fn roundtrip_graph_task_with_bounds() {
        roundtrip_json(&GraphTask {
            id: "paper_count".into(),
            title: "Paper count | Scale".into(),
            insight: "Scale check".into(),
            cypher: "MATCH (n:Paper) RETURN count(n)".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: Some(1),
            expect_min_value: Some(24.0),
            expect_value: None,
            class: GraphTaskClass::Structural,
        });
    }

    #[test]
    fn roundtrip_graph_task_suite() {
        roundtrip_json(&GraphTaskSuite {
            tasks: vec![
                GraphTask {
                    id: "t1".into(),
                    title: "T1 | x".into(),
                    insight: "".into(),
                    cypher: "MATCH (n) RETURN n LIMIT 1".into(),
                    min_rows: 1,
                    allow_empty: false,
                    expect_rows: None,
                    expect_min_value: None,
                    expect_value: None,
                    class: GraphTaskClass::Structural,
                },
                GraphTask {
                    id: "t2".into(),
                    title: "T2 | y".into(),
                    insight: "".into(),
                    cypher: "MATCH (n) RETURN count(n)".into(),
                    min_rows: 1,
                    allow_empty: true,
                    expect_rows: Some(1),
                    expect_min_value: None,
                    expect_value: None,
                    class: GraphTaskClass::Fact,
                },
            ],
        });
    }

    #[test]
    fn roundtrip_task_verdict() {
        for v in [TaskVerdict::Pass, TaskVerdict::Empty, TaskVerdict::Error] {
            roundtrip_json(&v);
        }
    }
}
