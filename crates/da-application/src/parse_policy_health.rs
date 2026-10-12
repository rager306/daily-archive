//! Faculty A of ADR-079: health report over the policy observation journal.
//!
//! The queen's health faculty is derived data: every field aggregates
//! `PolicyObservation` records the pipeline already writes. Nothing here
//! probes a sidecar, collects new signals, or mutates anything. Percentiles
//! are nearest-rank on sorted latencies — deterministic, no interpolation —
//! and every map is ordered so the report digest is replayable from the same
//! journal bytes.

use crate::parse_policy_table::PolicyObservation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const PARSE_HEALTH_SCHEMA_VERSION: &str = "parse-health-report.v1";

#[derive(Debug, Error, PartialEq)]
pub enum HealthError {
    #[error("health: the journal slice is empty — nothing to report")]
    EmptyJournal,
}

/// DATA-declared alarm thresholds (ADR-079 R146: thresholds live in DATA).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthThresholds {
    /// A non-modal backend below this ok-rate raises a drift alarm.
    pub min_ok_rate: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self { min_ok_rate: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointHealth {
    pub total: u64,
    pub ok: u64,
    pub mean_latency_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendHealth {
    pub total: u64,
    pub ok: u64,
    pub latency_mean_ms: f64,
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BucketHealth {
    pub total: u64,
    pub ok: u64,
    pub mean_latency_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftAlarm {
    /// Closed alarm vocabulary: `multiple_backends` | `backend_ok_rate`.
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParseHealthReport {
    pub schema_version: String,
    /// Digest of the canonical JSON of the observations the report read.
    pub journal_digest: String,
    pub points: BTreeMap<String, PointHealth>,
    pub backends: BTreeMap<String, BackendHealth>,
    pub buckets: BTreeMap<String, BucketHealth>,
    pub drift_alarms: Vec<DriftAlarm>,
    pub report_digest: String,
}

/// Nearest-rank percentile of a non-empty sorted slice (1-based rank).
fn nearest_rank(sorted: &[u64], p: u64) -> u64 {
    debug_assert!(!sorted.is_empty());
    let rank = ((p as usize * sorted.len() + 99) / 100).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn content_digest(value: &impl Serialize) -> String {
    use sha2::Digest;
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

/// Aggregate a journal slice into a sealed health report. Alarms name their
/// cause; they never suppress, reclassify, or gate anything.
pub fn parse_health(
    observations: &[PolicyObservation],
    thresholds: &HealthThresholds,
) -> Result<ParseHealthReport, HealthError> {
    if observations.is_empty() {
        return Err(HealthError::EmptyJournal);
    }

    let mut points: BTreeMap<String, (u64, u64, f64)> = BTreeMap::new();
    let mut backends: BTreeMap<String, (u64, u64, Vec<u64>)> = BTreeMap::new();
    let mut buckets: BTreeMap<String, (u64, u64, f64)> = BTreeMap::new();

    for obs in observations {
        let latency = obs.outcome.as_ref().map(|o| o.latency_ms).unwrap_or(0);
        let ok = obs.reward >= 1.0;
        let ok_u = u64::from(ok);

        let p = points
            .entry(obs.decision_point.as_str().to_string())
            .or_insert((0, 0, 0.0));
        p.0 += 1;
        p.1 += ok_u;
        p.2 += latency as f64;

        if let Some(backend) = obs.outcome.as_ref().and_then(|o| o.backend.as_deref()) {
            let b = backends.entry(backend.to_string()).or_default();
            b.0 += 1;
            b.1 += ok_u;
            b.2.push(latency);
        }

        let bucket = da_domain::fsm_policy::feature_bucket(obs.decision_point, &obs.features);
        let bu = buckets.entry(bucket).or_insert((0, 0, 0.0));
        bu.0 += 1;
        bu.1 += ok_u;
        bu.2 += latency as f64;
    }

    let points: BTreeMap<String, PointHealth> = points
        .into_iter()
        .map(|(k, (total, ok, lat_sum))| {
            (
                k,
                PointHealth {
                    total,
                    ok,
                    mean_latency_ms: lat_sum / total as f64,
                },
            )
        })
        .collect();

    let mut alarms: Vec<DriftAlarm> = Vec::new();
    if backends.len() > 1 {
        alarms.push(DriftAlarm {
            kind: "multiple_backends".into(),
            detail: format!(
                "backends observed: {} — a version change is a retrain trigger (ADR-079 D504)",
                backends.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        });
    }

    let backends: BTreeMap<String, BackendHealth> = backends
        .into_iter()
        .map(|(k, (total, ok, mut latencies))| {
            latencies.sort_unstable();
            let mean = latencies.iter().sum::<u64>() as f64 / total as f64;
            (
                k.clone(),
                BackendHealth {
                    total,
                    ok,
                    latency_mean_ms: mean,
                    latency_p50_ms: nearest_rank(&latencies, 50),
                    latency_p95_ms: nearest_rank(&latencies, 95),
                },
            )
        })
        .collect();

    // Non-modal backend below the ok-rate floor: alarm per backend.
    if let Some(modal) = backends
        .iter()
        .max_by_key(|(_, b)| b.total)
        .map(|(k, _)| k.clone())
    {
        for (name, health) in &backends {
            if name != &modal {
                let ok_rate = health.ok as f64 / health.total as f64;
                if ok_rate < thresholds.min_ok_rate {
                    alarms.push(DriftAlarm {
                        kind: "backend_ok_rate".into(),
                        detail: format!(
                            "backend {name}: ok-rate {ok_rate:.2} < {:.2} (owner: gsd-control-plane class retrain trigger)",
                            thresholds.min_ok_rate
                        ),
                    });
                }
            }
        }
    }

    let buckets: BTreeMap<String, BucketHealth> = buckets
        .into_iter()
        .map(|(k, (total, ok, lat_sum))| {
            (
                k,
                BucketHealth {
                    total,
                    ok,
                    mean_latency_ms: lat_sum / total as f64,
                },
            )
        })
        .collect();

    // The journal digest preserves read order (provenance of what was read);
    // the report digest covers the aggregates only, so the same multiset of
    // observations yields the same report identity regardless of order.
    let journal_digest = content_digest(&observations);
    let report_digest = content_digest(&(
        PARSE_HEALTH_SCHEMA_VERSION,
        &points,
        &backends,
        &buckets,
        &alarms,
    ));
    Ok(ParseHealthReport {
        schema_version: PARSE_HEALTH_SCHEMA_VERSION.to_string(),
        journal_digest,
        points,
        backends,
        buckets,
        drift_alarms: alarms,
        report_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_policy_table::PolicyOutcomeEvidence;
    use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint};

    fn obs(backend: Option<&str>, latency: u64, ok: bool, pages: u32) -> PolicyObservation {
        PolicyObservation {
            document_identity: format!("d{pages}-{latency}"),
            decision_point: DecisionPoint::GrammarRoute,
            features: ArticleFeatures {
                page_count: pages,
                toc_bookmarks: 0,
                text_layer_quality: 0.85,
                domain: String::new(),
            },
            decision_code: "xy_cut".into(),
            reward: f64::from(ok),
            outcome: Some(PolicyOutcomeEvidence {
                ok,
                section_count: 3,
                body_chars: 100,
                latency_ms: latency,
                backend: backend.map(str::to_string),
            }),
        }
    }

    #[test]
    fn aggregates_points_backends_buckets() {
        let journal = vec![
            obs(Some("0.9.1"), 100, true, 18),
            obs(Some("0.9.1"), 300, true, 18),
            obs(Some("0.9.1"), 200, false, 700),
        ];
        let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
        let p = &report.points["grammar_route"];
        assert_eq!(p.total, 3);
        assert_eq!(p.ok, 2);
        assert!((p.mean_latency_ms - 200.0).abs() < 1e-9);
        let b = &report.backends["0.9.1"];
        assert_eq!(b.latency_p50_ms, 200);
        assert_eq!(b.latency_p95_ms, 300);
        assert_eq!(
            report.buckets.len(),
            2,
            "18p and 700p land in different buckets"
        );
        assert!(report.drift_alarms.is_empty(), "single backend, no alarm");
    }

    #[test]
    fn drift_alarm_on_multiple_backends() {
        let journal = vec![
            obs(Some("0.9.0"), 100, true, 18),
            obs(Some("0.9.0"), 100, true, 18),
            obs(Some("0.9.1"), 100, true, 18),
        ];
        let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
        assert!(
            report
                .drift_alarms
                .iter()
                .any(|a| a.kind == "multiple_backends")
        );
    }

    #[test]
    fn drift_alarm_on_degraded_non_modal_backend() {
        let journal = vec![
            obs(Some("0.9.1"), 100, true, 18),
            obs(Some("0.9.1"), 100, true, 18),
            obs(Some("0.9.0"), 100, false, 18),
            obs(Some("0.9.0"), 100, false, 18),
        ];
        let report = parse_health(&journal, &HealthThresholds { min_ok_rate: 0.5 }).unwrap();
        assert!(
            report
                .drift_alarms
                .iter()
                .any(|a| a.kind == "backend_ok_rate")
        );
    }

    #[test]
    fn deterministic_across_permutations() {
        let a = vec![
            obs(Some("0.9.1"), 100, true, 18),
            obs(Some("0.9.1"), 250, false, 700),
        ];
        let b: Vec<_> = a.iter().cloned().rev().collect();
        let ra = parse_health(&a, &HealthThresholds::default()).unwrap();
        let rb = parse_health(&b, &HealthThresholds::default()).unwrap();
        // Same multiset of observations, same report identity.
        assert_eq!(ra.report_digest, rb.report_digest);
    }

    #[test]
    fn empty_journal_is_an_honest_error() {
        assert_eq!(
            parse_health(&[], &HealthThresholds::default()),
            Err(HealthError::EmptyJournal)
        );
    }

    #[test]
    fn nearest_rank_matches_reference_values() {
        let sorted = [10u64, 20, 30, 40];
        assert_eq!(nearest_rank(&sorted, 50), 20);
        assert_eq!(nearest_rank(&sorted, 95), 40);
        assert_eq!(nearest_rank(&sorted, 100), 40);
    }

    /// R149 latency budget: aggregation over 10k observations stays under 1s.
    #[test]
    fn health_budget_10k_under_one_second() {
        let journal: Vec<PolicyObservation> = (0..10_000)
            .map(|i| obs(Some("0.9.1"), 100 + (i % 900) as u64, i % 5 != 0, 18))
            .collect();
        let t = std::time::Instant::now();
        let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
        let elapsed = t.elapsed();
        assert_eq!(report.points["grammar_route"].total, 10_000);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "R149 budget violated: {elapsed:?}"
        );
    }
}
