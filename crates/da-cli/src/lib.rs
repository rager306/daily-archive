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
#[cfg(feature = "pdf-layout")]
pub mod layout_runtime;
pub mod load;
pub mod load_journey;
pub mod logging;
pub mod materialization_progress;
pub mod parse_batch;
pub mod parse_config;
pub mod parse_job;
pub mod parse_profile;
pub mod production;
pub mod snapshot_identity;
pub mod snapshot_lineage;
pub mod source_acquire;
pub mod source_cli;
pub mod summarize_revisions;

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
