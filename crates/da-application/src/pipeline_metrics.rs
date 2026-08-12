//! Pure builders for complete stage-ledger metric coverage.

use da_domain::{
    assurance::{ContentDigest, MissingnessReason},
    pipeline_ledger::PipelineStageLedger,
    pipeline_metrics::{
        MetricValue, PipelineMetricsReport, StageExecutionMetrics, StageExecutionState,
    },
};

pub fn no_claim_pipeline_metrics(
    ledger: &PipelineStageLedger,
    ledger_bytes: &[u8],
    run_id: &str,
    article_id: &str,
) -> Result<PipelineMetricsReport, String> {
    ledger.validate()?;
    if run_id.trim().is_empty() || article_id.trim().is_empty() {
        return Err("metrics run/article identity is empty".into());
    }
    let stages = ledger
        .stages
        .iter()
        .map(|stage| {
            let unavailable = || MetricValue::no_claim("ms", MissingnessReason::NotMaterialized);
            StageExecutionMetrics {
                article_id: article_id.into(),
                run_id: run_id.into(),
                stage_id: stage.id.clone(),
                state: StageExecutionState::NoClaim,
                input_sha256: None,
                output_sha256: None,
                wall_time: unavailable(),
                cpu_time: unavailable(),
                peak_rss: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
                io_read: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
                io_write: MetricValue::no_claim("bytes", MissingnessReason::NotMaterialized),
                queue_wait: unavailable(),
                queue_depth: MetricValue::no_claim("items", MissingnessReason::NotMaterialized),
                worker_busy: MetricValue::no_claim(
                    "basis_points",
                    MissingnessReason::NotMaterialized,
                ),
                input_tokens: MetricValue::no_claim("tokens", MissingnessReason::NotApplicable),
                output_tokens: MetricValue::no_claim("tokens", MissingnessReason::NotApplicable),
                model_latency: MetricValue::no_claim("ms", MissingnessReason::NotApplicable),
                estimated_cost_microunits: MetricValue::no_claim(
                    "microunits",
                    MissingnessReason::NotApplicable,
                ),
                cache: None,
                context: None,
                backend: "unobserved".into(),
                backend_on_hot_path: false,
                failure_class: None,
                import_eligible: false,
            }
        })
        .collect::<Vec<_>>();
    let report = PipelineMetricsReport {
        schema_version: "pipeline-metrics.v1".into(),
        ledger_sha256: ContentDigest::of_bytes(ledger_bytes),
        run_id: run_id.into(),
        stages,
        optimizer_admission_verdict: None,
        import_eligible: false,
        report_sha256: String::new(),
    };
    let ids = ledger
        .stages
        .iter()
        .map(|x| x.id.clone())
        .collect::<Vec<_>>();
    report.validate(&ids)?;
    Ok(report)
}
