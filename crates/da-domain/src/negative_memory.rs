//! Negative semantic knowledge (Vasiliev 2026 / M455).
//!
//! A critic Reject that exhausted repair is not merely a log line.
//! Reuse of the same claim (paper + stage + pattern) must fail closed.
//! Pure domain: no IO. Persistence lives behind a port.

use serde::{Deserialize, Serialize};

/// Durable "do not reuse this claim as current" record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegativeMemoryRecord {
    pub id: String,
    pub paper_id: String,
    pub stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Link to the critic/repair source event (trajectory or CLI pair id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_event_id: Option<String>,
    pub recorded_at: String,
    /// Only exhausted rejects block. Resolved repairs are experience, not bans.
    pub exhausted: bool,
}

/// Lookup key for admit (must not be paper_id alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitKey<'a> {
    pub paper_id: &'a str,
    pub stage: &'a str,
    pub pattern: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum AdmitDecision {
    Allowed { reason: String },
    BlockedExhausted { record_id: String, reason: String },
}

impl AdmitDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

fn pattern_matches(record: &NegativeMemoryRecord, key: &AdmitKey<'_>) -> bool {
    match (record.pattern.as_deref(), key.pattern) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(rp), Some(kp)) => rp == kp || kp.contains(rp) || rp.contains(kp),
    }
}

/// Fail-closed for matching exhausted negatives; otherwise allow.
pub fn admit_claim(key: AdmitKey<'_>, records: &[NegativeMemoryRecord]) -> AdmitDecision {
    for rec in records {
        if !rec.exhausted {
            continue;
        }
        if rec.paper_id != key.paper_id {
            continue;
        }
        if rec.stage != key.stage {
            continue;
        }
        if !pattern_matches(rec, &key) {
            continue;
        }
        return AdmitDecision::BlockedExhausted {
            record_id: rec.id.clone(),
            reason: format!(
                "blocked_exhausted: paper={} stage={} pattern={:?}",
                rec.paper_id, rec.stage, rec.pattern
            ),
        };
    }
    AdmitDecision::Allowed {
        reason: "no exhausted negative for this paper/stage/pattern".into(),
    }
}

/// Build a blocking record from an exhausted repair (caller stamps id/time).
pub fn record_from_exhausted(
    id: String,
    paper_id: String,
    stage: String,
    pattern: Option<String>,
    source_event_id: Option<String>,
    recorded_at: String,
) -> NegativeMemoryRecord {
    NegativeMemoryRecord {
        id,
        paper_id,
        stage,
        pattern,
        source_event_id,
        recorded_at,
        exhausted: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(
        id: &str,
        paper: &str,
        stage: &str,
        pattern: Option<&str>,
        exhausted: bool,
    ) -> NegativeMemoryRecord {
        NegativeMemoryRecord {
            id: id.into(),
            paper_id: paper.into(),
            stage: stage.into(),
            pattern: pattern.map(str::to_string),
            source_event_id: Some("evt-1".into()),
            recorded_at: "2026-08-25T00:00:00Z".into(),
            exhausted,
        }
    }

    #[test]
    fn exhausted_reject_is_denied() {
        let records = [rec("n1", "2507.19457", "figure_analysis", Some("schema violation"), true)];
        let d = admit_claim(
            AdmitKey {
                paper_id: "2507.19457",
                stage: "figure_analysis",
                pattern: Some("schema violation"),
            },
            &records,
        );
        assert!(!d.is_allowed());
        match d {
            AdmitDecision::BlockedExhausted { record_id, .. } => assert_eq!(record_id, "n1"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn different_paper_is_allowed() {
        let records = [rec("n1", "2507.19457", "figure_analysis", Some("schema violation"), true)];
        let d = admit_claim(
            AdmitKey {
                paper_id: "2608.14905",
                stage: "figure_analysis",
                pattern: Some("schema violation"),
            },
            &records,
        );
        assert!(d.is_allowed());
    }

    #[test]
    fn different_stage_is_allowed() {
        let records = [rec("n1", "2507.19457", "figure_analysis", Some("schema violation"), true)];
        let d = admit_claim(
            AdmitKey {
                paper_id: "2507.19457",
                stage: "summaries",
                pattern: Some("schema violation"),
            },
            &records,
        );
        assert!(d.is_allowed());
    }

    #[test]
    fn resolved_repair_does_not_block() {
        let records = [rec("n1", "2507.19457", "figure_analysis", Some("schema violation"), false)];
        let d = admit_claim(
            AdmitKey {
                paper_id: "2507.19457",
                stage: "figure_analysis",
                pattern: Some("schema violation"),
            },
            &records,
        );
        assert!(d.is_allowed());
    }

    #[test]
    fn serde_roundtrip() {
        let r = rec("n1", "p", "figure_analysis", Some("x"), true);
        let json = serde_json::to_string(&r).unwrap();
        let back: NegativeMemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn record_from_exhausted_sets_flag() {
        let r = record_from_exhausted(
            "id".into(),
            "p".into(),
            "figure_analysis".into(),
            Some("schema violation".into()),
            Some("fig-1".into()),
            "t".into(),
        );
        assert!(r.exhausted);
        let d = admit_claim(
            AdmitKey {
                paper_id: "p",
                stage: "figure_analysis",
                pattern: Some("schema violation"),
            },
            &[r],
        );
        assert!(!d.is_allowed());
    }
}
