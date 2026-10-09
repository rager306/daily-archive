//! Production computations for the declared ACAP hard metrics (M516/S08/T03).
//!
//! `scripts/governor/checks/article_context_assurance.yaml` declares eleven
//! `kind: hard` metrics. Five already carry a production computation in the
//! per-layer `evaluate_hard_failures` surface; the remaining six had no
//! occurrence anywhere in `crates/*/src`, so the declaration-synchronisation
//! axis (`scripts/governor/adapters/declaration_sync_gap.py`, class
//! `hard_metric_without_computation`) counted them as gaps.
//!
//! This module closes that half of the axis by giving each of the six a `pub fn`
//! whose **name is exactly the declared metric name** — the axis resolves a
//! metric to production code by that whole-token match. Each function is pure
//! and returns a [`MetricCoverage`] numerator/denominator pair, so a `pass:
//! "== 1.0"` gate has an honest denominator and a caller can tell "100 % of zero
//! things" from "100 % of everything".
//!
//! # Fail-closed rule
//!
//! An empty input is **not** a vacuous 100 %. `MetricCoverage::ratio` returns
//! `None` and `MetricCoverage::is_complete` returns `false` whenever the
//! denominator is zero, so a stage that recorded no evidence cannot silently
//! satisfy a hard gate (ADR-055: missing evidence is `no_claim`, never a pass).
//!
//! # Honest denominators only
//!
//! Where a metric has no typed evidence that yields a real denominator it is
//! left out and downgraded with an owner and reason in the disposition registry
//! (`metric_dispositions`, T05) rather than faked. All six covered here have a
//! typed denominator:
//!
//! | metric | numerator | denominator |
//! |---|---|---|
//! | `checksum_lineage_coverage` | `StageExecutionMetrics` rows carrying both input and output digests | rows |
//! | `typed_missingness_coverage` | `MetricValue`s carrying a typed `MissingnessReason` | values |
//! | `cache_lineage_exactness` | `CacheLineageEvidence` rows whose key/payload digests and three fingerprints are present | rows |
//! | `ontology_conformance` | checked objects minus `SchemaViolation`s | checked objects |
//! | `evidence_grounding_coverage` | matched local bibliography links | local bibliography entries processed |
//! | `semantic_task_pass_rate` | required graph-task classes whose gate passed | required classes |

use serde::{Deserialize, Serialize};

use crate::graph_tasks::{ClassifiedTaskEvidence, TaskClassStatus};
use crate::pipeline_cache_lineage::CacheLineageEvidence;
use crate::pipeline_metrics::{MetricValue, StageExecutionMetrics};
use crate::schema_invariants::SchemaViolation;
use crate::scholarly_evidence::ScholarlyEvidenceReport;

/// A hard-metric coverage measurement as an explicit fraction.
///
/// The numerator and denominator are kept separate on purpose: collapsing to a
/// `f64` loses the difference between "0 of 0" (no evidence — fail closed) and
/// "0 of 5" (observed and failing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricCoverage {
    /// How many units satisfy the metric.
    pub numerator: u64,
    /// How many units the metric was evaluated over.
    pub denominator: u64,
}

impl MetricCoverage {
    /// Construct a coverage fraction. Kept explicit so callers cannot pass a
    /// ratio where a count is expected.
    pub fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// The coverage ratio, or `None` when nothing was evaluated.
    ///
    /// `None` (not `1.0`) is the whole point: an empty denominator must never
    /// read as a satisfied hard gate.
    pub fn ratio(&self) -> Option<f64> {
        if self.denominator == 0 {
            None
        } else {
            Some(self.numerator as f64 / self.denominator as f64)
        }
    }

    /// The metric is satisfied only when at least one unit was evaluated and
    /// every one of them passed.
    pub fn is_complete(&self) -> bool {
        self.denominator > 0 && self.numerator == self.denominator
    }
}

/// `checksum_lineage_coverage` = `outputs_with_source_and_output_sha256 / outputs`.
///
/// A stage-execution row is covered when it recorded both the input digest it
/// consumed and the output digest it produced; a row with either missing cannot
/// be chained into a lineage audit.
pub fn checksum_lineage_coverage(records: &[StageExecutionMetrics]) -> MetricCoverage {
    let numerator = records
        .iter()
        .filter(|record| record.input_sha256.is_some() && record.output_sha256.is_some())
        .count() as u64;
    MetricCoverage::new(numerator, records.len() as u64)
}

/// `typed_missingness_coverage` = `missing_values_with_reason / missing_values`.
///
/// A recorded metric value is covered when it carries a typed
/// [`MissingnessReason`](crate::assurance::MissingnessReason): a value that is
/// absent or refused must say *why*, never publish a silent zero (ACAP principle
/// `missingness_must_be_typed`).
pub fn typed_missingness_coverage(values: &[MetricValue]) -> MetricCoverage {
    let numerator = values
        .iter()
        .filter(|value| value.missingness.is_some())
        .count() as u64;
    MetricCoverage::new(numerator, values.len() as u64)
}

/// `cache_lineage_exactness` = `cache_records_with_exact_key_and_output_hash / cache_records`.
///
/// A cache row is exact when its key and payload digests are present *and* the
/// row is structurally valid — which, through
/// [`CacheLineageEvidence::validate`], requires the implementation,
/// configuration and output-schema fingerprints that make a cache hit
/// reproducible rather than merely plausible.
pub fn cache_lineage_exactness(records: &[CacheLineageEvidence]) -> MetricCoverage {
    let numerator = records
        .iter()
        .filter(|record| {
            !record.key_sha256.as_str().trim().is_empty()
                && !record.payload_sha256.as_str().trim().is_empty()
                && record.validate().is_ok()
        })
        .count() as u64;
    MetricCoverage::new(numerator, records.len() as u64)
}

/// `ontology_conformance` = `valid_nodes_and_edges / checked_nodes_and_edges`.
///
/// `checked` is the number of objects the schema engine evaluated;
/// `violations` are the ones it rejected. The subtraction saturates so a
/// caller that mis-reports more violations than checks records zero conformance
/// rather than wrapping around to a false pass.
pub fn ontology_conformance(checked: u64, violations: &[SchemaViolation]) -> MetricCoverage {
    let rejected = violations.len() as u64;
    MetricCoverage::new(checked.saturating_sub(rejected), checked)
}

/// `evidence_grounding_coverage` = `semantic_facts_with_evidence_path / semantic_facts`.
///
/// Evaluated over the multi-provider scholarly reconciliation report: the
/// authoritative facts are the local bibliography entries the report processed,
/// and a fact is grounded when it carries a strong identity (DOI or eprint)
/// linking it to a provider observation — recorded as a
/// [`MatchedBibliographyLink`](crate::scholarly_evidence::MatchedBibliographyLink).
pub fn evidence_grounding_coverage(report: &ScholarlyEvidenceReport) -> MetricCoverage {
    let grounded = report.matched_bibliography.len() as u64;
    let total = grounded + report.unmatched_local_bibliography as u64;
    MetricCoverage::new(grounded, total)
}

/// `semantic_task_pass_rate` = `passed_required_tasks / required_tasks`.
///
/// Evaluated over a class-qualified graph-task suite: the required classes are
/// the denominator, and a class counts as passed only when it is present in the
/// per-class evidence with a `Passed` gate. A class that is required but absent
/// from the evidence counts against the rate, so an undeclared suite cannot
/// satisfy the metric by simply running nothing.
pub fn semantic_task_pass_rate(evidence: &ClassifiedTaskEvidence) -> MetricCoverage {
    let denominator = evidence.required_classes.len() as u64;
    let numerator = evidence
        .required_classes
        .iter()
        .filter(|class| {
            evidence
                .by_class
                .get(class)
                .map(|row| row.status == TaskClassStatus::Passed)
                .unwrap_or(false)
        })
        .count() as u64;
    MetricCoverage::new(numerator, denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance::{ContentDigest, MissingnessReason, MissingnessReason as Reason};
    use crate::graph_tasks::{GraphTaskClass, TaskClassResult};
    use crate::pipeline_metrics::StageExecutionState;
    use crate::scholarly_evidence::{
        MatchedBibliographyLink, ScholarlyEvidenceReport, ScholarlyReconcileVerdict,
    };
    use std::collections::BTreeMap;

    // ── fixtures ────────────────────────────────────────────────────────────

    fn no_claim(unit: &str) -> MetricValue {
        MetricValue::no_claim(unit, Reason::NotMaterialized)
    }

    fn stage_metric(input: bool, output: bool) -> StageExecutionMetrics {
        StageExecutionMetrics {
            article_id: "x".into(),
            run_id: "r".into(),
            stage_id: "parse".into(),
            state: StageExecutionState::Complete,
            input_sha256: input.then(|| ContentDigest::of_text("in")),
            output_sha256: output.then(|| ContentDigest::of_text("out")),
            wall_time: no_claim("ms"),
            cpu_time: no_claim("ms"),
            peak_rss: no_claim("bytes"),
            io_read: no_claim("bytes"),
            io_write: no_claim("bytes"),
            queue_wait: no_claim("ms"),
            queue_depth: no_claim("items"),
            worker_busy: no_claim("basis_points"),
            input_tokens: no_claim("tokens"),
            output_tokens: no_claim("tokens"),
            model_latency: no_claim("ms"),
            estimated_cost_microunits: no_claim("microunits"),
            cache: None,
            context: None,
            backend: "none".into(),
            backend_on_hot_path: false,
            failure_class: None,
            import_eligible: false,
        }
    }

    fn cache_record(exact: bool) -> CacheLineageEvidence {
        CacheLineageEvidence {
            article_id: "x".into(),
            stage_id: "parse".into(),
            key_sha256: ContentDigest::of_text("key"),
            payload_sha256: ContentDigest::of_text("payload"),
            payload_size: 7,
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: if exact { "impl".into() } else { String::new() },
            configuration_fingerprint: "config".into(),
            prompt_fingerprint: None,
            model_fingerprint: Some("model".into()),
            output_schema_fingerprint: "schema".into(),
            cache_hit: true,
            graph_materialization_skipped: false,
            import_eligible: false,
        }
    }

    fn violation(id: &str) -> SchemaViolation {
        SchemaViolation {
            object_id: id.into(),
            pattern: "enum_membership".into(),
            kind: crate::schema_invariants::ViolationKind::ValueNotInVocabulary,
            evidence: "value not in vocabulary".into(),
        }
    }

    fn grounded_link(key: &str) -> MatchedBibliographyLink {
        MatchedBibliographyLink {
            local_entry_key: key.into(),
            local_candidate_id: ContentDigest::of_text(key),
            provider: "local_tex".into(),
            match_kind: "local_doi_present".into(),
            provider_key: format!("10.0000/{key}"),
        }
    }

    fn scholarly_report(matched: usize, unmatched: usize) -> ScholarlyEvidenceReport {
        ScholarlyEvidenceReport {
            schema_version: "scholarly-evidence-reconcile.v1".into(),
            article_id: "x".into(),
            title: None,
            providers: Vec::new(),
            tex: None,
            findings: Vec::new(),
            matched_bibliography: (0..matched)
                .map(|i| grounded_link(&format!("k{i}")))
                .collect(),
            unmatched_local_bibliography: unmatched,
            provider_reference_denominators: BTreeMap::new(),
            local_reference_denominator: None,
            verdict: ScholarlyReconcileVerdict::Converged,
            import_eligible: false,
            report_sha256: String::new(),
        }
    }

    fn class_row(class: GraphTaskClass, status: TaskClassStatus) -> TaskClassResult {
        TaskClassResult {
            class,
            status,
            task_count: 1,
            pass: usize::from(status == TaskClassStatus::Passed),
            empty: 0,
            error: 0,
        }
    }

    // ── MetricCoverage ──────────────────────────────────────────────────────

    #[test]
    fn empty_denominator_is_not_a_vacuous_pass() {
        let empty = MetricCoverage::new(0, 0);
        assert_eq!(empty.ratio(), None, "0 of 0 must not read as 100%");
        assert!(!empty.is_complete());
    }

    #[test]
    fn partial_coverage_is_never_complete() {
        let partial = MetricCoverage::new(1, 2);
        assert_eq!(partial.ratio(), Some(0.5));
        assert!(!partial.is_complete());
    }

    #[test]
    fn full_coverage_over_a_real_denominator_is_complete() {
        let full = MetricCoverage::new(3, 3);
        assert_eq!(full.ratio(), Some(1.0));
        assert!(full.is_complete());
    }

    // ── checksum_lineage_coverage ───────────────────────────────────────────

    #[test]
    fn checksum_lineage_is_complete_when_every_row_carries_both_digests() {
        let records = vec![stage_metric(true, true), stage_metric(true, true)];
        let coverage = checksum_lineage_coverage(&records);
        assert_eq!(coverage, MetricCoverage::new(2, 2));
        assert!(coverage.is_complete());
    }

    #[test]
    fn checksum_lineage_flags_a_row_missing_an_output_digest() {
        let records = vec![stage_metric(true, true), stage_metric(true, false)];
        let coverage = checksum_lineage_coverage(&records);
        assert_eq!(coverage, MetricCoverage::new(1, 2));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn checksum_lineage_of_nothing_has_no_denominator() {
        assert_eq!(checksum_lineage_coverage(&[]).ratio(), None);
    }

    // ── typed_missingness_coverage ──────────────────────────────────────────

    #[test]
    fn typed_missingness_is_complete_when_every_value_is_typed() {
        let values = vec![no_claim("ms"), no_claim("bytes")];
        let coverage = typed_missingness_coverage(&values);
        assert_eq!(coverage, MetricCoverage::new(2, 2));
        assert!(coverage.is_complete());
    }

    #[test]
    fn typed_missingness_flags_an_untyped_value() {
        let values = vec![
            no_claim("ms"),
            MetricValue::observed(0, "bytes"), // a zero with no reason
            no_claim("tokens"),
        ];
        let coverage = typed_missingness_coverage(&values);
        assert_eq!(coverage, MetricCoverage::new(2, 3));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn typed_missingness_of_nothing_has_no_denominator() {
        assert_eq!(typed_missingness_coverage(&[]).ratio(), None);
    }

    // ── cache_lineage_exactness ─────────────────────────────────────────────

    #[test]
    fn cache_lineage_is_complete_when_keys_and_fingerprints_are_present() {
        let records = vec![cache_record(true), cache_record(true)];
        let coverage = cache_lineage_exactness(&records);
        assert_eq!(coverage, MetricCoverage::new(2, 2));
        assert!(coverage.is_complete());
    }

    #[test]
    fn cache_lineage_rejects_a_row_with_an_empty_fingerprint() {
        let records = vec![cache_record(true), cache_record(false)];
        let coverage = cache_lineage_exactness(&records);
        assert_eq!(coverage, MetricCoverage::new(1, 2));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn cache_lineage_of_nothing_has_no_denominator() {
        assert_eq!(cache_lineage_exactness(&[]).ratio(), None);
    }

    // ── ontology_conformance ────────────────────────────────────────────────

    #[test]
    fn ontology_conformance_is_complete_without_violations() {
        let coverage = ontology_conformance(4, &[]);
        assert_eq!(coverage, MetricCoverage::new(4, 4));
        assert!(coverage.is_complete());
    }

    #[test]
    fn ontology_conformance_counts_violations_against_the_checked_total() {
        let coverage = ontology_conformance(4, &[violation("a"), violation("b")]);
        assert_eq!(coverage, MetricCoverage::new(2, 4));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn ontology_conformance_saturates_when_more_violations_than_checks() {
        let coverage = ontology_conformance(1, &[violation("a"), violation("b")]);
        assert_eq!(coverage, MetricCoverage::new(0, 1));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn ontology_conformance_of_nothing_has_no_denominator() {
        assert_eq!(ontology_conformance(0, &[]).ratio(), None);
    }

    // ── evidence_grounding_coverage ─────────────────────────────────────────

    #[test]
    fn evidence_grounding_is_complete_when_every_fact_is_matched() {
        let coverage = evidence_grounding_coverage(&scholarly_report(3, 0));
        assert_eq!(coverage, MetricCoverage::new(3, 3));
        assert!(coverage.is_complete());
    }

    #[test]
    fn evidence_grounding_flags_an_unmatched_local_fact() {
        let coverage = evidence_grounding_coverage(&scholarly_report(2, 1));
        assert_eq!(coverage, MetricCoverage::new(2, 3));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn evidence_grounding_of_nothing_has_no_denominator() {
        assert_eq!(
            evidence_grounding_coverage(&scholarly_report(0, 0)).ratio(),
            None
        );
    }

    // ── semantic_task_pass_rate ─────────────────────────────────────────────

    #[test]
    fn semantic_task_pass_rate_is_complete_when_required_classes_pass() {
        let mut by_class = BTreeMap::new();
        by_class.insert(
            GraphTaskClass::Structural,
            class_row(GraphTaskClass::Structural, TaskClassStatus::Passed),
        );
        by_class.insert(
            GraphTaskClass::Fact,
            class_row(GraphTaskClass::Fact, TaskClassStatus::Passed),
        );
        let evidence = ClassifiedTaskEvidence {
            overall_passed: true,
            by_class,
            required_classes: vec![GraphTaskClass::Structural, GraphTaskClass::Fact],
        };
        let coverage = semantic_task_pass_rate(&evidence);
        assert_eq!(coverage, MetricCoverage::new(2, 2));
        assert!(coverage.is_complete());
    }

    #[test]
    fn semantic_task_pass_rate_counts_an_absent_required_class_against_it() {
        let mut by_class = BTreeMap::new();
        by_class.insert(
            GraphTaskClass::Structural,
            class_row(GraphTaskClass::Structural, TaskClassStatus::Passed),
        );
        let evidence = ClassifiedTaskEvidence {
            // Suite-level boolean is true, but a required class never ran:
            // the class gate must not collapse to the overall boolean.
            overall_passed: true,
            by_class,
            required_classes: vec![GraphTaskClass::Structural, GraphTaskClass::Summary],
        };
        let coverage = semantic_task_pass_rate(&evidence);
        assert_eq!(coverage, MetricCoverage::new(1, 2));
        assert!(!coverage.is_complete());
    }

    #[test]
    fn semantic_task_pass_rate_without_required_classes_has_no_denominator() {
        let evidence = ClassifiedTaskEvidence::overall_only(true);
        assert_eq!(semantic_task_pass_rate(&evidence).ratio(), None);
    }

    // ── declaration coverage: every implemented name is present ─────────────

    #[test]
    fn every_implemented_metric_name_is_reachable_as_a_whole_token() {
        // A compile-time pin: if a declared function is renamed away from its
        // metric name the governor axis would count a new gap. Taking the
        // function pointers proves the symbols exist under these exact names.
        let _ = checksum_lineage_coverage as fn(&[StageExecutionMetrics]) -> MetricCoverage;
        let _ = typed_missingness_coverage as fn(&[MetricValue]) -> MetricCoverage;
        let _ = cache_lineage_exactness as fn(&[CacheLineageEvidence]) -> MetricCoverage;
        let _ = ontology_conformance as fn(u64, &[SchemaViolation]) -> MetricCoverage;
        let _ = evidence_grounding_coverage as fn(&ScholarlyEvidenceReport) -> MetricCoverage;
        let _ = semantic_task_pass_rate as fn(&ClassifiedTaskEvidence) -> MetricCoverage;
    }

    #[test]
    fn missingness_reason_is_used_from_the_assurance_enum() {
        // Guards the import alias so the module compiles against the real enum.
        let v = MetricValue::no_claim("ms", MissingnessReason::SourceUnavailable);
        assert!(v.missingness.is_some());
    }
}
