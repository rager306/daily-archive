//! M444: corpus regression diff over materialization run reports.
//!
//! The user direction: work over ALL articles and catch regressions
//! in-line. The run report already records per-paper outcomes for every
//! article; this module turns two reports into typed regression findings
//! so a corpus rebuild fails loudly when it loses papers, drops entity
//! counts, or drifts revisions — no gold corpus involved.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identity fields have zero tolerance; counts get a relative tolerance.
pub const DEFAULT_TOLERANCE: f64 = 0.10;

fn env_tolerance() -> f64 {
    std::env::var("DA_REGRESSION_TOLERANCE")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|t: &f64| *t >= 0.0)
        .unwrap_or(DEFAULT_TOLERANCE)
}

#[derive(Debug, Clone, Deserialize)]
struct ReportPaper {
    paper_id: String,
    #[serde(default)]
    state: serde_json::Value,
    #[serde(default)]
    revision: Option<u32>,
    #[serde(default)]
    entities_created: Option<usize>,
    #[serde(default)]
    mentions_edges: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RunReportFile {
    #[serde(default)]
    papers: Vec<ReportPaper>,
    #[serde(default)]
    totals: TotalsFile,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TotalsFile {
    #[serde(default)]
    completed: usize,
    #[serde(default)]
    failed: usize,
    #[serde(default)]
    replay_noop_violations: usize,
}

fn completed(p: &ReportPaper) -> bool {
    p.state.as_str().is_some_and(|s| s == "completed")
}

fn numeric_drop(before: usize, after: usize, tolerance: f64) -> bool {
    // A drop is only measurable against a positive baseline: 0 -> 0 is
    // stable and 0 -> n is an improvement, never a regression.
    if before == 0 {
        return false;
    }
    let allowed = (before as f64 * tolerance).floor() as i64;
    (after as i64) < before as i64 - allowed
}

/// One typed regression finding.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "finding")]
pub enum RegressionFinding {
    PapersRemoved {
        count: usize,
        sample: Vec<String>,
    },
    PapersAdded {
        count: usize,
        sample: Vec<String>,
    },
    RevisionDrift {
        paper_id: String,
        before: u32,
        after: u32,
    },
    StructuralDrop {
        paper_id: String,
        field: &'static str,
        before: usize,
        after: usize,
    },
    ReplayHonestyLost {
        before: usize,
        after: usize,
    },
    FailedPapers {
        before: usize,
        after: usize,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct RegressionReport {
    pub schema_version: u32,
    pub before_papers: usize,
    pub after_papers: usize,
    pub before_totals_completed: usize,
    pub after_totals_completed: usize,
    pub tolerance: f64,
    pub regressions: Vec<RegressionFinding>,
}

impl RegressionReport {
    pub fn is_clean(&self) -> bool {
        self.regressions.is_empty()
    }
}

/// Diff two run-report JSON byte slices. Identity regressions (removed
/// papers, revision drift, lost replay honesty) are always findings;
/// structural count drops use the relative tolerance.
pub fn diff_reports(before_json: &[u8], after_json: &[u8]) -> serde_json::Result<RegressionReport> {
    diff_reports_with_tolerance(before_json, after_json, env_tolerance())
}

pub fn diff_reports_with_tolerance(
    before_json: &[u8],
    after_json: &[u8],
    tolerance: f64,
) -> serde_json::Result<RegressionReport> {
    let before: RunReportFile = serde_json::from_slice(before_json)?;
    let after: RunReportFile = serde_json::from_slice(after_json)?;
    Ok(diff_parsed(&before, &after, tolerance))
}

fn diff_parsed(before: &RunReportFile, after: &RunReportFile, tolerance: f64) -> RegressionReport {
    let mut regressions = Vec::new();
    let before_map: BTreeMap<&str, &ReportPaper> = before
        .papers
        .iter()
        .filter(|p| completed(p))
        .map(|p| (p.paper_id.as_str(), p))
        .collect();
    let after_map: BTreeMap<&str, &ReportPaper> = after
        .papers
        .iter()
        .filter(|p| completed(p))
        .map(|p| (p.paper_id.as_str(), p))
        .collect();

    let removed: Vec<String> = before_map
        .keys()
        .filter(|id| !after_map.contains_key(**id))
        .map(|id| id.to_string())
        .collect();
    if !removed.is_empty() {
        let count = removed.len();
        regressions.push(RegressionFinding::PapersRemoved {
            count,
            sample: removed.into_iter().take(5).collect(),
        });
    }
    let added: Vec<String> = after_map
        .keys()
        .filter(|id| !before_map.contains_key(**id))
        .map(|id| id.to_string())
        .collect();
    if !added.is_empty() {
        let count = added.len();
        regressions.push(RegressionFinding::PapersAdded {
            count,
            sample: added.into_iter().take(5).collect(),
        });
    }

    for (id, b) in &before_map {
        let Some(a) = after_map.get(*id) else {
            continue;
        };
        match (b.revision, a.revision) {
            (Some(bv), Some(av)) if bv != av => {
                regressions.push(RegressionFinding::RevisionDrift {
                    paper_id: id.to_string(),
                    before: bv,
                    after: av,
                });
            }
            _ => {}
        }
        for (field, bv, av) in [
            ("entities_created", b.entities_created, a.entities_created),
            ("mentions_edges", b.mentions_edges, a.mentions_edges),
        ] {
            if let (Some(bv), Some(av)) = (bv, av)
                && numeric_drop(bv, av, tolerance)
            {
                regressions.push(RegressionFinding::StructuralDrop {
                    paper_id: id.to_string(),
                    field,
                    before: bv,
                    after: av,
                });
            }
        }
    }

    if after.totals.replay_noop_violations > before.totals.replay_noop_violations {
        regressions.push(RegressionFinding::ReplayHonestyLost {
            before: before.totals.replay_noop_violations,
            after: after.totals.replay_noop_violations,
        });
    }
    if after.totals.failed > 0 {
        regressions.push(RegressionFinding::FailedPapers {
            before: before.totals.failed,
            after: after.totals.failed,
        });
    }

    RegressionReport {
        schema_version: 1,
        before_papers: before_map.len(),
        after_papers: after_map.len(),
        before_totals_completed: before.totals.completed,
        after_totals_completed: after.totals.completed,
        tolerance,
        regressions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(papers: &str, totals: &str) -> Vec<u8> {
        format!(r#"{{"papers":{papers},"totals":{totals}}}"#).into_bytes()
    }

    const T: f64 = 0.0;

    #[test]
    fn identical_reports_are_clean() {
        let json = report(
            r#"[{"paper_id":"1111.1111","state":"completed","revision":2,"entities_created":9,"mentions_edges":12}]"#,
            r#"{"completed":1,"failed":0,"replay_noop_violations":0}"#,
        );
        let r = diff_reports_with_tolerance(&json, &json, T).unwrap();
        assert!(r.is_clean());
        assert_eq!(r.before_papers, 1);
    }

    #[test]
    fn removed_paper_is_a_regression() {
        let before = report(
            r#"[{"paper_id":"1111.1111","state":"completed","revision":1},{"paper_id":"2222.2222","state":"completed","revision":1}]"#,
            r#"{"completed":2}"#,
        );
        let after = report(
            r#"[{"paper_id":"1111.1111","state":"completed","revision":1}]"#,
            r#"{"completed":1}"#,
        );
        let r = diff_reports_with_tolerance(&before, &after, T).unwrap();
        assert!(matches!(
            r.regressions[0],
            RegressionFinding::PapersRemoved { count: 1, .. }
        ));
    }

    #[test]
    fn added_paper_is_reported_but_not_fatal_semantics() {
        let before = report(
            r#"[{"paper_id":"1","state":"completed"}]"#,
            r#"{"completed":1}"#,
        );
        let after = report(
            r#"[{"paper_id":"1","state":"completed"},{"paper_id":"2","state":"completed"}]"#,
            r#"{"completed":2}"#,
        );
        let r = diff_reports_with_tolerance(&before, &after, T).unwrap();
        assert!(matches!(
            r.regressions[0],
            RegressionFinding::PapersAdded { count: 1, .. }
        ));
    }

    #[test]
    fn revision_drift_is_a_regression() {
        let mk = |rev: u32| {
            let body = r#"[{"paper_id":"1111.1111","state":"completed","revision":REVISION}]"#
                .replace("REVISION", &rev.to_string());
            report(&body, r#"{"completed":1}"#)
        };
        let r = diff_reports_with_tolerance(&mk(2), &mk(3), T).unwrap();
        assert!(matches!(
            r.regressions[0],
            RegressionFinding::RevisionDrift {
                before: 2,
                after: 3,
                paper_id: _
            }
        ));
    }

    #[test]
    fn entity_drop_beyond_tolerance_is_a_regression() {
        let mk = |e: usize| {
            let body = r#"[{"paper_id":"1","state":"completed","entities_created":ENT}]"#
                .replace("ENT", &e.to_string());
            report(&body, r#"{"completed":1}"#)
        };
        // tolerance 0.5: 10 -> 4 is a drop (allowed floor 5).
        let r = diff_reports_with_tolerance(&mk(10), &mk(4), 0.5).unwrap();
        assert!(matches!(
            r.regressions[0],
            RegressionFinding::StructuralDrop {
                field: "entities_created",
                before: 10,
                after: 4,
                ..
            }
        ));
        // 10 -> 6 is within tolerance.
        let ok = diff_reports_with_tolerance(&mk(10), &mk(6), 0.5).unwrap();
        assert!(ok.is_clean());
    }

    #[test]
    fn replay_honesty_and_failed_papers_regress() {
        let before = report(
            r#"[]"#,
            r#"{"completed":0,"failed":0,"replay_noop_violations":0}"#,
        );
        let after = report(
            r#"[{"paper_id":"9","state":"failed"}]"#,
            r#"{"completed":0,"failed":1,"replay_noop_violations":2}"#,
        );
        let r = diff_reports_with_tolerance(&before, &after, T).unwrap();
        assert!(
            r.regressions
                .iter()
                .any(|f| matches!(f, RegressionFinding::ReplayHonestyLost { .. }))
        );
        assert!(
            r.regressions
                .iter()
                .any(|f| matches!(f, RegressionFinding::FailedPapers { .. }))
        );
    }
}
