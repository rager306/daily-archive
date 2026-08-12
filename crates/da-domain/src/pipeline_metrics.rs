//! Whole-pipeline execution measurements for optimizer-ready evidence.

use crate::{
    assurance::{ContentDigest, MissingnessReason},
    context_conservation::StageContextSnapshot,
    pipeline_cache_lineage::CacheLineageEvidence,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageExecutionState {
    Scheduled,
    WaitingDependency,
    Running,
    CacheHit,
    Materializing,
    Complete,
    FailedVisible,
    NoClaim,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricValue {
    pub value: u64,
    pub unit: String,
    pub missingness: Option<MissingnessReason>,
}
impl MetricValue {
    pub fn observed(value: u64, unit: &str) -> Self {
        Self {
            value,
            unit: unit.into(),
            missingness: None,
        }
    }
    pub fn no_claim(unit: &str, reason: MissingnessReason) -> Self {
        Self {
            value: 0,
            unit: unit.into(),
            missingness: Some(reason),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.unit.trim().is_empty() {
            return Err("metric unit is empty".into());
        }
        if self.missingness.is_some() && self.value != 0 {
            return Err("no_claim metric cannot publish a value".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageExecutionMetrics {
    pub article_id: String,
    pub run_id: String,
    pub stage_id: String,
    pub state: StageExecutionState,
    pub input_sha256: Option<ContentDigest>,
    pub output_sha256: Option<ContentDigest>,
    pub wall_time: MetricValue,
    pub cpu_time: MetricValue,
    pub peak_rss: MetricValue,
    pub io_read: MetricValue,
    pub io_write: MetricValue,
    pub queue_wait: MetricValue,
    pub queue_depth: MetricValue,
    pub worker_busy: MetricValue,
    pub input_tokens: MetricValue,
    pub output_tokens: MetricValue,
    pub model_latency: MetricValue,
    pub estimated_cost_microunits: MetricValue,
    pub cache: Option<CacheLineageEvidence>,
    pub context: Option<StageContextSnapshot>,
    pub backend: String,
    pub backend_on_hot_path: bool,
    pub failure_class: Option<String>,
    pub import_eligible: bool,
}
impl StageExecutionMetrics {
    pub fn validate(&self) -> Result<(), String> {
        if self.article_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.stage_id.trim().is_empty()
            || self.backend.trim().is_empty()
        {
            return Err("stage execution identity is incomplete".into());
        }
        if self.import_eligible {
            return Err("pipeline metrics cannot authorize import".into());
        }
        for metric in [
            &self.wall_time,
            &self.cpu_time,
            &self.peak_rss,
            &self.io_read,
            &self.io_write,
            &self.queue_wait,
            &self.queue_depth,
            &self.worker_busy,
            &self.input_tokens,
            &self.output_tokens,
            &self.model_latency,
            &self.estimated_cost_microunits,
        ] {
            metric.validate()?;
        }
        if matches!(
            self.state,
            StageExecutionState::Complete | StageExecutionState::CacheHit
        ) && self.output_sha256.is_none()
        {
            return Err("completed stage requires output identity".into());
        }
        if self.state == StageExecutionState::FailedVisible
            && self
                .failure_class
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err("failed stage requires failure_class".into());
        }
        if let Some(cache) = &self.cache {
            cache.validate()?;
        }
        if let Some(context) = &self.context {
            context.validate()?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineMetricsReport {
    pub schema_version: String,
    pub ledger_sha256: ContentDigest,
    pub run_id: String,
    pub stages: Vec<StageExecutionMetrics>,
    pub optimizer_admission_verdict: Option<String>,
    pub import_eligible: bool,
    pub report_sha256: String,
}
impl PipelineMetricsReport {
    pub fn validate(&self, expected_stage_ids: &[String]) -> Result<(), String> {
        if self.schema_version != "pipeline-metrics.v1"
            || self.import_eligible
            || self.optimizer_admission_verdict.is_some()
        {
            return Err("metrics schema/D127/optimizer admission invalid".into());
        }
        let ids = self
            .stages
            .iter()
            .map(|x| x.stage_id.clone())
            .collect::<Vec<_>>();
        if ids != expected_stage_ids {
            return Err("metrics rows must exactly follow stage ledger order".into());
        }
        for stage in &self.stages {
            stage.validate()?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn metric(stage: &str, state: StageExecutionState) -> StageExecutionMetrics {
        let nc = || MetricValue::no_claim("ms", MissingnessReason::NotMaterialized);
        StageExecutionMetrics {
            article_id: "x".into(),
            run_id: "r".into(),
            stage_id: stage.into(),
            state,
            input_sha256: None,
            output_sha256: if matches!(state, StageExecutionState::Complete) {
                Some(ContentDigest::of_text(stage))
            } else {
                None
            },
            wall_time: nc(),
            cpu_time: nc(),
            peak_rss: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
            io_read: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
            io_write: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
            queue_wait: nc(),
            queue_depth: MetricValue::no_claim("items", MissingnessReason::NotMaterialized),
            worker_busy: MetricValue::no_claim("basis_points", MissingnessReason::NotMaterialized),
            input_tokens: MetricValue::no_claim("tokens", MissingnessReason::NotApplicable),
            output_tokens: MetricValue::no_claim("tokens", MissingnessReason::NotApplicable),
            model_latency: nc(),
            estimated_cost_microunits: MetricValue::no_claim(
                "microunits",
                MissingnessReason::NotApplicable,
            ),
            cache: None,
            context: None,
            backend: "none".into(),
            backend_on_hot_path: false,
            failure_class: None,
            import_eligible: false,
        }
    }
    #[test]
    fn report_requires_exact_ledger_rows() {
        let ids = vec!["parse".into(), "query".into()];
        let r = PipelineMetricsReport {
            schema_version: "pipeline-metrics.v1".into(),
            ledger_sha256: ContentDigest::of_text("ledger"),
            run_id: "r".into(),
            stages: vec![
                metric("parse", StageExecutionState::Complete),
                metric("query", StageExecutionState::NoClaim),
            ],
            optimizer_admission_verdict: None,
            import_eligible: false,
            report_sha256: "x".into(),
        };
        r.validate(&ids).unwrap();
        let mut bad = r.clone();
        bad.stages.reverse();
        assert!(bad.validate(&ids).is_err());
    }
    #[test]
    fn no_claim_cannot_invent_measurement() {
        let mut x = MetricValue::no_claim("ms", MissingnessReason::NotMaterialized);
        x.value = 1;
        assert!(x.validate().is_err());
    }
}
