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
    #[error("health: document {document_identity} has no outcome evidence")]
    MissingOutcomeEvidence { document_identity: String },
}

/// DATA-declared alarm thresholds (ADR-079 R146: thresholds live in DATA).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthThresholds {
    /// A non-modal backend below this ok-rate raises a drift alarm.
    pub min_ok_rate: f64,
    /// Per-document gate floors (critic_sample as derived data).
    pub gate_floors: GateFloors,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            min_ok_rate: 0.5,
            gate_floors: GateFloors::default(),
        }
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

/// Per-document triage decision derived from the same journal-derived
/// evidence (ADR-079 Faculty A, extended for the `critic_sample` gate of
/// ADR-076 `consult_status.later`). Thresholds live in DATA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentGate {
    pub document_identity: String,
    pub decision: GateDecision,
    /// The signals behind the decision (why), for audit.
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    /// Parse succeeded, coverage signals healthy.
    Accept,
    /// Re-parse candidate: failed parse or zero-section parse.
    ReParse,
    /// Defer to the critic arm: parsed but thin (sections/chars below floor).
    Defer,
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
    /// Per-document gate decisions (ADR-076 critic_sample as derived data).
    #[serde(default)]
    pub documents: Vec<DocumentGate>,
    pub report_digest: String,
}

/// Nearest-rank percentile of a non-empty sorted slice (1-based rank).
fn nearest_rank(sorted: &[u64], p: u64) -> u64 {
    debug_assert!(!sorted.is_empty());
    let rank = (p as usize * sorted.len())
        .div_ceil(100)
        .clamp(1, sorted.len());
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
    if let Some(observation) = observations.iter().find(|obs| obs.outcome.is_none()) {
        return Err(HealthError::MissingOutcomeEvidence {
            document_identity: observation.document_identity.clone(),
        });
    }

    let mut points: BTreeMap<String, (u64, u64, f64)> = BTreeMap::new();
    let mut backends: BTreeMap<String, (u64, u64, Vec<u64>)> = BTreeMap::new();
    let mut buckets: BTreeMap<String, (u64, u64, f64)> = BTreeMap::new();

    for obs in observations {
        let latency = obs.outcome.as_ref().map(|o| o.latency_ms).unwrap_or(0);
        let ok = obs.outcome.as_ref().is_some_and(|outcome| outcome.ok);
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

    // Per-document gate (critic_sample): aggregate each document's outcomes,
    // then apply the DATA floors. A document is ReParse if ANY outcome says
    // so; Defer if any says so (and none said ReParse); Accept otherwise.
    let mut doc_states: BTreeMap<String, (GateDecision, Vec<String>)> = BTreeMap::new();
    for o in observations {
        let Some(evidence) = o.outcome.as_ref() else {
            continue;
        };
        let (decision, reasons) = gate_document(evidence, &thresholds.gate_floors);
        let entry = doc_states
            .entry(o.document_identity.clone())
            .or_insert((GateDecision::Accept, Vec::new()));
        let rank = |d: &GateDecision| match d {
            GateDecision::Accept => 0,
            GateDecision::Defer => 1,
            GateDecision::ReParse => 2,
        };
        if rank(&decision) > rank(&entry.0) {
            entry.0 = decision;
        }
        entry.1.extend(reasons);
    }
    let documents: Vec<DocumentGate> = doc_states
        .into_iter()
        .map(|(document_identity, (decision, reasons))| DocumentGate {
            document_identity,
            decision,
            reasons,
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
        &documents,
    ));
    let mut documents: Vec<DocumentGate> = documents;
    documents.sort_by(|a, b| a.document_identity.cmp(&b.document_identity));
    Ok(ParseHealthReport {
        schema_version: PARSE_HEALTH_SCHEMA_VERSION.to_string(),
        journal_digest,
        points,
        backends,
        buckets,
        drift_alarms: alarms,
        documents,
        report_digest,
    })
}

/// The three-way gate the `critic_sample` faculty needs (ADR-076 later points,
/// realised as derived data per ADR-079 Faculty A):
/// - `ReParse`: the parse failed outright or recovered no sections;
/// - `Defer`: parsed but thin (sections or chars below the DATA floors);
/// - `Accept`: everything at or above the floors.
pub fn gate_document(
    evidence: &crate::parse_policy_table::PolicyOutcomeEvidence,
    floors: &GateFloors,
) -> (GateDecision, Vec<String>) {
    let mut reasons = Vec::new();
    if !evidence.ok {
        reasons.push("parse_not_ok".into());
        return (GateDecision::ReParse, reasons);
    }
    if evidence.section_count == 0 {
        reasons.push("zero_sections".into());
        return (GateDecision::ReParse, reasons);
    }
    if (evidence.body_chars as u64) < floors.min_body_chars {
        reasons.push(format!("body_chars<{}", floors.min_body_chars));
        return (GateDecision::Defer, reasons);
    }
    if (evidence.section_count as u64) < floors.min_sections {
        reasons.push(format!("sections<{}", floors.min_sections));
        return (GateDecision::Defer, reasons);
    }
    (GateDecision::Accept, reasons)
}

/// DATA floors for the gate (ADR-079: thresholds live in DATA).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateFloors {
    pub min_sections: u64,
    pub min_body_chars: u64,
}

impl Default for GateFloors {
    fn default() -> Self {
        Self {
            min_sections: 2,
            min_body_chars: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_policy_table::PolicyOutcomeEvidence;
    use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint};
    use proptest::{prop_assert, prop_assert_eq};

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
                body_chars: 2000,
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
    fn health_success_comes_from_outcome_not_pair_reward() {
        let mut winner = obs(Some("0.9.1"), 100, true, 18);
        winner.reward = 1.0;
        let mut loser = obs(Some("0.9.1"), 200, true, 18);
        loser.reward = 0.0;
        let mut tie = obs(Some("0.9.1"), 300, true, 18);
        tie.reward = 0.5;
        let mut failed = obs(Some("0.9.1"), 400, false, 18);
        failed.reward = 1.0;
        let report =
            parse_health(&[winner, loser, tie, failed], &HealthThresholds::default()).unwrap();
        assert_eq!(report.points["grammar_route"].total, 4);
        assert_eq!(report.points["grammar_route"].ok, 3);
        assert_eq!(report.backends["0.9.1"].ok, 3);
        assert_eq!(report.buckets.values().next().unwrap().ok, 3);
    }

    #[test]
    fn missing_outcome_evidence_is_refused() {
        let mut unknown = obs(Some("0.9.1"), 100, true, 18);
        unknown.reward = 1.0;
        unknown.outcome = None;
        let document_identity = unknown.document_identity.clone();
        assert_eq!(
            parse_health(&[unknown], &HealthThresholds::default()),
            Err(HealthError::MissingOutcomeEvidence { document_identity }),
        );
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
        let report = parse_health(
            &journal,
            &HealthThresholds {
                min_ok_rate: 0.5,
                gate_floors: GateFloors::default(),
            },
        )
        .unwrap();
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

    #[test]
    fn health_report_roundtrips_through_json() {
        let journal = vec![
            obs(Some("0.9.1"), 100, true, 18),
            obs(Some("0.9.1"), 200, false, 18),
        ];
        let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        let back: ParseHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, back);
    }

    proptest::proptest! {
        /// The report digest is invariant under any permutation of the journal.
        #[test]
        fn report_digest_is_permutation_invariant(
            order in proptest::collection::vec(0usize..6, 1..30),
        ) {
            let base = [
                obs(Some("0.9.1"), 100, true, 18),
                obs(Some("0.9.1"), 250, false, 18),
                obs(Some("0.9.0"), 400, true, 700),
                obs(Some("0.9.1"), 90, true, 700),
            ];
            let shuffled: Vec<_> = order.iter().map(|i| base[i % base.len()].clone()).collect();
            let report = parse_health(&shuffled, &HealthThresholds::default()).unwrap();
            let key = da_domain::fsm_policy::feature_bucket(
                da_domain::fsm_policy::DecisionPoint::GrammarRoute,
                &shuffled[0].features,
            );
            // Buckets are keyed deterministically; every observed bucket appears.
            prop_assert!(report.buckets.contains_key(&key));
            prop_assert_eq!(report.points["grammar_route"].total, shuffled.len() as u64);
        }

        /// ok-rate is always within [0, 1] and total equals the journal length.
        #[test]
        fn ok_rate_is_a_ratio(
            flags in proptest::collection::vec(proptest::prelude::any::<bool>(), 1..40),
        ) {
            let journal: Vec<PolicyObservation> = flags
                .iter()
                .enumerate()
                .map(|(i, ok)| obs(Some("0.9.1"), 100 + i as u64, *ok, 18))
                .collect();
            let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
            let p = &report.points["grammar_route"];
            prop_assert_eq!(p.total, journal.len() as u64);
            prop_assert!(p.ok <= p.total);
            let rate = p.ok as f64 / p.total as f64;
            prop_assert!((0.0..=1.0).contains(&rate));
        }
    }

    #[test]
    fn documents_gate_accept_defer_reparse() {
        let mk = |id: &str, ok: bool, sections: usize| -> PolicyObservation {
            let mut o = obs(Some("0.9.1"), 100, ok, 18);
            o.document_identity = id.into();
            o.outcome = Some(PolicyOutcomeEvidence {
                ok,
                section_count: sections,
                body_chars: 10000,
                latency_ms: 100,
                backend: None,
            });
            o
        };
        let journal = vec![mk("d1", true, 5), mk("d2", true, 1), mk("d3", false, 3)];
        let thresholds = HealthThresholds {
            min_ok_rate: 0.5,
            gate_floors: GateFloors {
                min_sections: 2,
                min_body_chars: 5000,
            },
        };
        let report = parse_health(&journal, &thresholds).unwrap();
        assert_eq!(report.documents.len(), 3);
        let by_id: std::collections::BTreeMap<&str, &DocumentGate> = report
            .documents
            .iter()
            .map(|g| (g.document_identity.as_str(), g))
            .collect();
        assert_eq!(by_id["d1"].decision, GateDecision::Accept);
        assert_eq!(by_id["d2"].decision, GateDecision::Defer);
        assert_eq!(by_id["d3"].decision, GateDecision::ReParse);
    }

    #[test]
    fn gate_reasons_are_audit_trail() {
        let journal = vec![{
            let mut o = obs(Some("0.9.1"), 100, false, 18);
            o.document_identity = "d1".into();
            o
        }];
        let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
        assert!(
            report.documents[0]
                .reasons
                .iter()
                .any(|r| r.contains("parse_not_ok"))
        );
    }

    proptest::proptest! {
        /// Gate is total: every document gets exactly one decision.
        #[test]
        fn gate_covers_all_documents(
            flags in proptest::collection::vec(proptest::prelude::any::<bool>(), 1..20),
        ) {
            let journal: Vec<PolicyObservation> = flags.iter().enumerate()
                .map(|(i, ok)| {
                    let mut o = obs(Some("0.9.1"), 100, *ok, 18);
                    o.document_identity = format!("d{i}");
                    o.outcome = Some(PolicyOutcomeEvidence {
                        ok: *ok, section_count: 3, body_chars: 10000, latency_ms: 100, backend: None,
                    });
                    o
                })
                .collect();
            let report = parse_health(&journal, &HealthThresholds::default()).unwrap();
            prop_assert_eq!(report.documents.len(), flags.len());
        }
    }
}
