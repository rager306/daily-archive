//! Corpus ACAP aggregate composition (M371/S07/T03).
//!
//! Pure application aggregator over already-validated per-article reports and
//! optional physical replay upgrades. No filesystem, no graph mutation, no
//! D127 unlock. CLI composition roots supply observations and emit the durable
//! aggregate artifact.

use std::collections::{BTreeMap, BTreeSet};

use da_domain::{
    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ACAP_POLICY_ID, ArticleAssuranceReport, AssuranceError,
    AssuranceVerdict, ContentDigest, CorpusAggregateMetrics, CorpusAssuranceReport,
    CorpusLayerResult, CorpusLayerStatus, MissingnessReason, ReplayObservation,
    corpus_report_digest,
};
use serde::{Deserialize, Serialize};

/// Policy layer declaration needed for aggregate coverage (injected, no YAML).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusPolicyLayer {
    pub id: String,
    pub enforcement: String,
}

/// One article contribution: normalized per-article report plus optional
/// physical replay upgrade from paper-level mutation counters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusArticleContribution {
    pub report: ArticleAssuranceReport,
    /// When the embedded layer lacks `replay_observation`, attach this upgrade
    /// (typically rebuilt from paper-level `replay_mutations`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_upgrade: Option<ReplayObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusAssuranceInput {
    pub policy_sha256: ContentDigest,
    pub pipeline_revision: String,
    pub source_artifact_sha256: ContentDigest,
    pub policy_layers: Vec<CorpusPolicyLayer>,
    pub articles: Vec<CorpusArticleContribution>,
    pub import_eligible: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CorpusAssuranceError {
    #[error(transparent)]
    Domain(#[from] AssuranceError),
    #[error("invalid corpus assurance input: {0}")]
    InvalidInput(String),
}

#[derive(Default)]
struct LayerAcc {
    enforcement: String,
    article_count: u64,
    available_required_fields: u64,
    retained_required_fields: u64,
    hard_failures: BTreeSet<String>,
    hard_failure_articles: u64,
    pass_articles: u64,
    fail_articles: u64,
    no_claim_articles: u64,
    d127_unlock_count: u64,
    replay_net_writes: u64,
    replay_integrity_pass_articles: u64,
    observed: bool,
}

pub struct CorpusAssuranceUseCase;

impl CorpusAssuranceUseCase {
    pub fn aggregate(
        input: CorpusAssuranceInput,
    ) -> Result<CorpusAssuranceReport, CorpusAssuranceError> {
        validate_input(&input)?;

        let article_count = input.articles.len() as u64;
        let policy_layer_order = input.policy_layers.clone();
        let policy_layer_ids: BTreeSet<String> = policy_layer_order
            .iter()
            .map(|layer| layer.id.clone())
            .collect();

        let mut by_layer: BTreeMap<String, LayerAcc> = BTreeMap::new();
        for layer in &policy_layer_order {
            by_layer.insert(
                layer.id.clone(),
                LayerAcc {
                    enforcement: layer.enforcement.clone(),
                    ..LayerAcc::default()
                },
            );
        }

        let mut fsm_results: BTreeMap<String, u64> = BTreeMap::new();
        let mut corpus_hard_failures: BTreeSet<String> = BTreeSet::new();
        let mut corpus_d127 = 0_u64;
        let mut corpus_replay_net = 0_u64;
        let mut articles_with_hard = 0_u64;
        let mut quality_sum = 0.0_f64;
        let mut quality_n = 0_u64;

        for contrib in &input.articles {
            let mut report = contrib.report.clone().normalize()?;
            if let Some(upgrade) = contrib.replay_upgrade {
                for layer in &mut report.layer_results {
                    if layer.replay_observation.is_none() {
                        layer.replay_observation = Some(upgrade);
                        layer.replay_writes = Some(upgrade.applied_write_delta());
                    }
                    layer.evaluate_hard_failures();
                }
                report = report.normalize()?;
            }

            let state_key = serde_json::to_value(report.current_state)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{:?}", report.current_state).to_ascii_lowercase());
            *fsm_results.entry(state_key).or_insert(0) += 1;

            let mut article_has_hard = false;
            let observed_ids: BTreeSet<String> = report
                .layer_results
                .iter()
                .map(|layer| layer.layer_id.clone())
                .collect();

            for layer in &report.layer_results {
                if !policy_layer_ids.contains(&layer.layer_id) {
                    corpus_hard_failures.insert(format!("unknown_report_layer:{}", layer.layer_id));
                }
                let acc = by_layer
                    .entry(layer.layer_id.clone())
                    .or_insert_with(|| LayerAcc {
                        enforcement: "unknown".into(),
                        ..LayerAcc::default()
                    });
                acc.observed = true;
                acc.article_count += 1;
                acc.available_required_fields += layer.available_required_fields;
                acc.retained_required_fields += layer.retained_required_fields;
                acc.d127_unlock_count = acc.d127_unlock_count.max(layer.d127_unlock_count);
                corpus_d127 = corpus_d127.max(layer.d127_unlock_count);

                let replay_net = layer
                    .replay_observation
                    .map(|obs| obs.applied())
                    .or_else(|| layer.replay_writes.map(|writes| writes.total()))
                    .unwrap_or(0);
                acc.replay_net_writes += replay_net;
                corpus_replay_net += replay_net;

                if layer
                    .replay_observation
                    .map(|obs| obs.integrity_passes())
                    .unwrap_or(false)
                {
                    acc.replay_integrity_pass_articles += 1;
                }

                if layer.hard_failures.is_empty() {
                    acc.pass_articles += 1;
                } else {
                    acc.fail_articles += 1;
                    acc.hard_failure_articles += 1;
                    article_has_hard = true;
                    for code in &layer.hard_failures {
                        acc.hard_failures.insert(code.clone());
                        corpus_hard_failures.insert(code.clone());
                    }
                }

                if let Some(score) = layer.quality_metrics.get("coverage") {
                    quality_sum += *score;
                    quality_n += 1;
                }
            }

            for layer in &policy_layer_order {
                if !observed_ids.contains(&layer.id) {
                    by_layer
                        .get_mut(&layer.id)
                        .expect("policy layer seeded")
                        .no_claim_articles += 1;
                }
            }

            if article_has_hard {
                articles_with_hard += 1;
            }
        }

        let mut layer_results = Vec::new();
        let mut seen = BTreeSet::new();
        for layer in &policy_layer_order {
            seen.insert(layer.id.clone());
            let acc = by_layer.get(&layer.id).expect("policy layer seeded");
            layer_results.push(to_row(&layer.id, acc));
        }
        for (id, acc) in &by_layer {
            if !seen.contains(id) {
                layer_results.push(to_row(id, acc));
            }
        }

        // Required layers absent from the corpus artifact are typed no_claim on
        // the layer row (empty hard_failures, missingness set). Aggregate-level
        // hard codes keep the corpus Fail so closeout cannot invent PASS.
        for layer in &layer_results {
            if layer.enforcement == "required" && layer.status == CorpusLayerStatus::NoClaim {
                corpus_hard_failures.insert(format!("required_layer_absent:{}", layer.layer_id));
            }
        }

        let observed_required = layer_results
            .iter()
            .filter(|layer| {
                layer.enforcement == "required" && layer.status == CorpusLayerStatus::Observed
            })
            .count() as f64;
        let required_total = layer_results
            .iter()
            .filter(|layer| layer.enforcement == "required")
            .count()
            .max(1) as f64;
        let weighted_layer_coverage = observed_required / required_total;
        let hard_pass_rate = if article_count == 0 {
            0.0
        } else {
            (article_count - articles_with_hard) as f64 / article_count as f64
        };
        let weighted_quality_score = if quality_n == 0 {
            0.0
        } else {
            quality_sum / quality_n as f64
        };

        let hard_failures: Vec<String> = corpus_hard_failures.into_iter().collect();
        let verdict = if article_count == 0 {
            AssuranceVerdict::NoClaim
        } else if hard_failures.is_empty() && corpus_d127 == 0 && corpus_replay_net == 0 {
            AssuranceVerdict::Pass
        } else {
            AssuranceVerdict::Fail
        };

        let mut report = CorpusAssuranceReport {
            schema_version: ACAP_AGGREGATE_REPORT_SCHEMA_VERSION.into(),
            policy_id: ACAP_POLICY_ID.into(),
            policy_sha256: input.policy_sha256,
            article_count,
            pipeline_revision: input.pipeline_revision,
            layer_results,
            fsm_results,
            cache_results: BTreeMap::from([(
                "status".into(),
                "not_observed_in_corpus_artifact".into(),
            )]),
            prompt_results: BTreeMap::from([(
                "status".into(),
                "not_observed_in_corpus_artifact".into(),
            )]),
            optimizer_results: BTreeMap::from([
                ("candidate_only".into(), "true".into()),
                ("auto_promote".into(), "false".into()),
                ("status".into(), "not_observed_in_corpus_artifact".into()),
            ]),
            aggregate: CorpusAggregateMetrics {
                hard_failures,
                hard_pass_rate,
                weighted_layer_coverage,
                weighted_quality_score,
                replay_net_writes: corpus_replay_net,
                d127_unlock_count: corpus_d127,
                total_cost: None,
                latency_p95_ms: None,
                cost_missingness: Some(MissingnessReason::NotApplicable),
                latency_missingness: Some(MissingnessReason::NotApplicable),
            },
            import_eligible: false,
            source_artifact_sha256: input.source_artifact_sha256,
            verdict,
            report_sha256: String::new(),
        };
        report.report_sha256 = corpus_report_digest(&report);
        report.validate()?;
        Ok(report)
    }
}

fn to_row(id: &str, acc: &LayerAcc) -> CorpusLayerResult {
    let status = if acc.observed {
        CorpusLayerStatus::Observed
    } else {
        CorpusLayerStatus::NoClaim
    };
    CorpusLayerResult {
        layer_id: id.to_string(),
        enforcement: acc.enforcement.clone(),
        status,
        article_count: acc.article_count,
        available_required_fields: acc.available_required_fields,
        retained_required_fields: acc.retained_required_fields,
        hard_failures: acc.hard_failures.iter().cloned().collect(),
        hard_failure_articles: acc.hard_failure_articles,
        pass_articles: acc.pass_articles,
        fail_articles: acc.fail_articles,
        no_claim_articles: acc.no_claim_articles,
        d127_unlock_count: acc.d127_unlock_count,
        replay_net_writes: acc.replay_net_writes,
        replay_integrity_pass_articles: acc.replay_integrity_pass_articles,
        missingness: (!acc.observed).then_some(MissingnessReason::NotApplicable),
    }
}

fn validate_input(input: &CorpusAssuranceInput) -> Result<(), CorpusAssuranceError> {
    if input.import_eligible {
        return Err(CorpusAssuranceError::InvalidInput(
            "D127 import_eligible must be false".into(),
        ));
    }
    if input.pipeline_revision.trim().is_empty() {
        return Err(CorpusAssuranceError::InvalidInput(
            "pipeline_revision must not be empty".into(),
        ));
    }
    if input.policy_layers.is_empty() {
        return Err(CorpusAssuranceError::InvalidInput(
            "policy must declare at least one layer".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for layer in &input.policy_layers {
        if layer.id.trim().is_empty() {
            return Err(CorpusAssuranceError::InvalidInput(
                "layer id must not be empty".into(),
            ));
        }
        if layer.enforcement != "required" && layer.enforcement != "bootstrap" {
            return Err(CorpusAssuranceError::InvalidInput(format!(
                "layer '{}' enforcement must be required|bootstrap",
                layer.id
            )));
        }
        if !seen.insert(layer.id.clone()) {
            return Err(CorpusAssuranceError::InvalidInput(format!(
                "duplicate policy layer '{}'",
                layer.id
            )));
        }
    }
    for contrib in &input.articles {
        if contrib.report.article_id.trim().is_empty() {
            return Err(CorpusAssuranceError::InvalidInput(
                "article_id must not be empty".into(),
            ));
        }
    }
    Ok(())
}
