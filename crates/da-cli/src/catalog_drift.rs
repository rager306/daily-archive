//! Deterministic drift statistics for catalog materialization run reports.
//!
//! The report is intentionally derived from the per-paper `mentions_edges`
//! counters rather than from graph state. This keeps the alarm reproducible
//! and makes malformed or incomplete evidence fail closed.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Deserialize)]
struct RunReport {
    totals: Totals,
    papers: Vec<Paper>,
}

#[derive(Debug, Deserialize)]
struct Totals {
    completed: usize,
}

#[derive(Debug, Deserialize)]
struct Paper {
    paper_id: String,
    state: String,
    mentions_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogDriftStats {
    pub schema_version: u32,
    pub completed_papers: usize,
    pub total_mentions: usize,
    pub average_mentions: f64,
    pub zero_mention_papers: usize,
    pub zero_mention_alarms: Vec<String>,
}

/// Derive drift counters from a materialization run report.
///
/// Required fields are not defaulted: accepting a missing counter would turn
/// incomplete evidence into a false zero-mention pass. Paper IDs are unique
/// across all states, and the report's completed total must match its rows.
pub fn derive_stats(json: &[u8]) -> Result<CatalogDriftStats, serde_json::Error> {
    let report: RunReport = serde_json::from_slice(json)?;
    let mut ids = BTreeSet::new();
    for paper in &report.papers {
        if paper.paper_id.trim().is_empty() || !ids.insert(paper.paper_id.clone()) {
            return Err(serde::de::Error::custom(
                "paper_id must be non-empty and unique",
            ));
        }
    }

    let completed: Vec<&Paper> = report
        .papers
        .iter()
        .filter(|paper| paper.state == "completed")
        .collect();
    if report.totals.completed != completed.len() {
        return Err(serde::de::Error::custom(
            "totals.completed does not match completed paper rows",
        ));
    }
    if completed.is_empty() {
        return Err(serde::de::Error::custom("report has no completed papers"));
    }

    let total_mentions = completed.iter().map(|paper| paper.mentions_edges).sum();
    let mut zero_mention_alarms: Vec<String> = completed
        .iter()
        .filter(|paper| paper.mentions_edges == 0)
        .map(|paper| paper.paper_id.clone())
        .collect();
    zero_mention_alarms.sort();

    Ok(CatalogDriftStats {
        schema_version: 1,
        completed_papers: completed.len(),
        total_mentions,
        average_mentions: total_mentions as f64 / completed.len() as f64,
        zero_mention_papers: zero_mention_alarms.len(),
        zero_mention_alarms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(totals: &str, papers: &str) -> Vec<u8> {
        format!(r#"{{"totals":{totals},"papers":{papers}}}"#).into_bytes()
    }

    #[test]
    fn derives_sorted_positive_and_zero_mentions() {
        let input = report(
            r#"{"completed":3}"#,
            r#"[
                {"paper_id":"b","state":"completed","mentions_edges":4},
                {"paper_id":"a","state":"completed","mentions_edges":0},
                {"paper_id":"c","state":"skipped","mentions_edges":99},
                {"paper_id":"z","state":"completed","mentions_edges":0}
            ]"#,
        );
        let stats = derive_stats(&input).unwrap();
        assert_eq!(stats.completed_papers, 3);
        assert_eq!(stats.total_mentions, 4);
        assert_eq!(stats.average_mentions, 4.0 / 3.0);
        assert_eq!(stats.zero_mention_alarms, ["a", "z"]);
    }

    #[test]
    fn malformed_and_empty_reports_fail_closed() {
        assert!(derive_stats(b"not json").is_err());
        assert!(derive_stats(&report(r#"{"completed":0}"#, "[]")).is_err());
        assert!(
            derive_stats(&report(
                r#"{"completed":1}"#,
                r#"[{"paper_id":"a","state":"completed"}]"#,
            ))
            .is_err()
        );
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let input = report(
            r#"{"completed":2}"#,
            r#"[
                {"paper_id":"a","state":"completed","mentions_edges":1},
                {"paper_id":"a","state":"completed","mentions_edges":2}
            ]"#,
        );
        assert!(derive_stats(&input).is_err());
    }
}
