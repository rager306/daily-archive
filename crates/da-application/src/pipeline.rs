//! Cache-aware article pipeline orchestration (M371 S01 T02).
//!
//! `kg-pipeline` owns topology validation. This application service owns
//! article-specific effective-input keys and stage outcomes. Concrete parser,
//! LLM, JAR and filesystem adapters are injected through ports.

use da_ports::clock::Clock;
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, SourceIdentity, StageCacheKey, StageOutcome,
};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageExecutionError, StageExecutionRequest};
use kg_pipeline::Pipeline;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StageDisposition {
    Executed {
        output_sha256: String,
    },
    CacheHit {
        output_sha256: String,
    },
    RetrySuppressed {
        retry_after: String,
    },
    Failed {
        error_class: String,
        message: String,
        retryable: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StageRunRecord {
    pub step_name: String,
    pub stage_id: String,
    pub disposition: StageDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PipelineRunReport {
    pub pipeline_id: String,
    pub article_id: String,
    pub completed: bool,
    pub stages: Vec<StageRunRecord>,
}

#[derive(Debug, Error)]
pub enum PipelineOrchestratorError {
    #[error("pipeline stage '{stage_id}' has no injected executor")]
    MissingExecutor { stage_id: String },
    #[error("pipeline step '{step_name}' depends on unavailable output '{dependency}'")]
    MissingDependencyOutput {
        step_name: String,
        dependency: String,
    },
    #[error(
        "executor contract mismatch: registry key '{registry_key}', contract stage '{contract_stage}'"
    )]
    ContractMismatch {
        registry_key: String,
        contract_stage: String,
    },
    #[error(transparent)]
    Cache(#[from] PipelineCacheError),
}

pub struct PipelineOrchestrator {
    cache: Arc<dyn PipelineCache>,
    clock: Arc<dyn Clock>,
    executors: HashMap<String, Arc<dyn PipelineStageExecutor>>,
}

impl PipelineOrchestrator {
    pub fn new(cache: Arc<dyn PipelineCache>, clock: Arc<dyn Clock>) -> Self {
        Self {
            cache,
            clock,
            executors: HashMap::new(),
        }
    }

    pub fn with_executor(
        mut self,
        stage_id: impl Into<String>,
        executor: Arc<dyn PipelineStageExecutor>,
    ) -> Result<Self, PipelineOrchestratorError> {
        let stage_id = stage_id.into();
        let contract = executor.contract();
        if contract.stage_id != stage_id {
            return Err(PipelineOrchestratorError::ContractMismatch {
                registry_key: stage_id,
                contract_stage: contract.stage_id,
            });
        }
        self.executors.insert(stage_id, executor);
        Ok(self)
    }

    pub async fn run(
        &self,
        pipeline: &Pipeline,
        source: SourceIdentity,
    ) -> Result<PipelineRunReport, PipelineOrchestratorError> {
        let now = self.clock.now().to_rfc3339();
        let mut records = Vec::with_capacity(pipeline.steps.len());
        let mut outputs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut output_hashes: BTreeMap<String, String> = BTreeMap::new();

        for step in &pipeline.steps {
            let executor = self.executors.get(&step.stage).ok_or_else(|| {
                PipelineOrchestratorError::MissingExecutor {
                    stage_id: step.stage.clone(),
                }
            })?;
            let contract = executor.contract();
            if contract.stage_id != step.stage {
                return Err(PipelineOrchestratorError::ContractMismatch {
                    registry_key: step.stage.clone(),
                    contract_stage: contract.stage_id,
                });
            }

            let mut dependency_payloads = BTreeMap::new();
            let mut dependency_output_sha256 = BTreeMap::new();
            for dependency in &step.depends_on {
                let payload = outputs.get(dependency).ok_or_else(|| {
                    PipelineOrchestratorError::MissingDependencyOutput {
                        step_name: step.name.clone(),
                        dependency: dependency.clone(),
                    }
                })?;
                let hash = output_hashes.get(dependency).ok_or_else(|| {
                    PipelineOrchestratorError::MissingDependencyOutput {
                        step_name: step.name.clone(),
                        dependency: dependency.clone(),
                    }
                })?;
                dependency_payloads.insert(dependency.clone(), payload.clone());
                dependency_output_sha256.insert(dependency.clone(), hash.clone());
            }

            let key = StageCacheKey {
                source: source.clone(),
                stage_id: step.name.clone(),
                dependency_output_sha256,
                implementation_fingerprint: contract.implementation_fingerprint,
                configuration_fingerprint: contract.configuration_fingerprint,
                prompt_fingerprint: contract.prompt_fingerprint,
                model_fingerprint: contract.model_fingerprint,
                output_schema_fingerprint: contract.output_schema_fingerprint,
            };

            match self.cache.lookup(&key, &now)? {
                CacheLookup::Hit(hit) => {
                    tracing::info!(
                        article_id = %source.article_id,
                        pipeline_id = %pipeline.id,
                        step = %step.name,
                        stage = %step.stage,
                        event = "pipeline_stage_cache_hit"
                    );
                    let output_sha256 = payload_sha256(&hit.payload);
                    outputs.insert(step.name.clone(), hit.payload);
                    output_hashes.insert(step.name.clone(), output_sha256.clone());
                    records.push(StageRunRecord {
                        step_name: step.name.clone(),
                        stage_id: step.stage.clone(),
                        disposition: StageDisposition::CacheHit { output_sha256 },
                    });
                }
                CacheLookup::RetryableFailure(manifest) => {
                    let StageOutcome::RetryableFailure { retry_after, .. } = manifest.outcome
                    else {
                        unreachable!("cache lookup classified retryable failure")
                    };
                    tracing::warn!(
                        article_id = %source.article_id,
                        pipeline_id = %pipeline.id,
                        step = %step.name,
                        stage = %step.stage,
                        retry_after = %retry_after,
                        event = "pipeline_stage_retry_suppressed"
                    );
                    records.push(StageRunRecord {
                        step_name: step.name.clone(),
                        stage_id: step.stage.clone(),
                        disposition: StageDisposition::RetrySuppressed { retry_after },
                    });
                    return Ok(PipelineRunReport {
                        pipeline_id: pipeline.id.clone(),
                        article_id: source.article_id,
                        completed: false,
                        stages: records,
                    });
                }
                CacheLookup::Miss => {
                    tracing::info!(
                        article_id = %source.article_id,
                        pipeline_id = %pipeline.id,
                        step = %step.name,
                        stage = %step.stage,
                        event = "pipeline_stage_cache_miss"
                    );
                    let request = StageExecutionRequest {
                        article_id: source.article_id.clone(),
                        dependency_payloads,
                    };
                    match executor.execute(request).await {
                        Ok(output) => {
                            let output_sha256 = payload_sha256(&output.payload);
                            self.cache.store_success(
                                &key,
                                &output.payload,
                                output.metadata,
                                &now,
                            )?;
                            outputs.insert(step.name.clone(), output.payload);
                            output_hashes.insert(step.name.clone(), output_sha256.clone());
                            tracing::info!(
                                article_id = %source.article_id,
                                pipeline_id = %pipeline.id,
                                step = %step.name,
                                stage = %step.stage,
                                output_sha256 = %output_sha256,
                                event = "pipeline_stage_executed"
                            );
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: step.stage.clone(),
                                disposition: StageDisposition::Executed { output_sha256 },
                            });
                        }
                        Err(StageExecutionError::Retryable {
                            error_class,
                            message,
                            retry_after,
                        }) => {
                            tracing::warn!(
                                article_id = %source.article_id,
                                pipeline_id = %pipeline.id,
                                step = %step.name,
                                stage = %step.stage,
                                error_class = %error_class,
                                retry_after = %retry_after,
                                event = "pipeline_stage_retryable_failure"
                            );
                            self.cache.store_retryable_failure(
                                &key,
                                &error_class,
                                &message,
                                &retry_after,
                                &now,
                            )?;
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: step.stage.clone(),
                                disposition: StageDisposition::Failed {
                                    error_class,
                                    message,
                                    retryable: true,
                                },
                            });
                            return Ok(PipelineRunReport {
                                pipeline_id: pipeline.id.clone(),
                                article_id: source.article_id,
                                completed: false,
                                stages: records,
                            });
                        }
                        Err(StageExecutionError::Terminal {
                            error_class,
                            message,
                        }) => {
                            tracing::error!(
                                article_id = %source.article_id,
                                pipeline_id = %pipeline.id,
                                step = %step.name,
                                stage = %step.stage,
                                error_class = %error_class,
                                event = "pipeline_stage_terminal_failure"
                            );
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: step.stage.clone(),
                                disposition: StageDisposition::Failed {
                                    error_class,
                                    message,
                                    retryable: false,
                                },
                            });
                            return Ok(PipelineRunReport {
                                pipeline_id: pipeline.id.clone(),
                                article_id: source.article_id,
                                completed: false,
                                stages: records,
                            });
                        }
                    }
                }
            }
        }

        Ok(PipelineRunReport {
            pipeline_id: pipeline.id.clone(),
            article_id: source.article_id,
            completed: true,
            stages: records,
        })
    }
}

fn payload_sha256(payload: &[u8]) -> String {
    hex::encode(Sha256::digest(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use da_ports::clock::FixedClock;
    use da_ports::pipeline_cache::{
        CacheLookup, CachedSuccess, PipelineCacheResult, StageManifestEntry,
    };
    use da_ports::pipeline_stage::{StageContract, StageExecutionOutput};
    use kg_pipeline::PipelineBuilder;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct MemoryCache {
        entries: Mutex<Vec<(StageCacheKey, StageManifestEntry, Vec<u8>)>>,
    }

    impl PipelineCache for MemoryCache {
        fn lookup(&self, key: &StageCacheKey, now: &str) -> PipelineCacheResult<CacheLookup> {
            let entries = self.entries.lock().unwrap();
            let Some((_, manifest, payload)) = entries.iter().find(|(stored, _, _)| stored == key)
            else {
                return Ok(CacheLookup::Miss);
            };
            match &manifest.outcome {
                StageOutcome::Success { .. } => Ok(CacheLookup::Hit(CachedSuccess {
                    manifest: manifest.clone(),
                    payload: payload.clone(),
                })),
                StageOutcome::RetryableFailure { retry_after, .. }
                    if now < retry_after.as_str() =>
                {
                    Ok(CacheLookup::RetryableFailure(manifest.clone()))
                }
                StageOutcome::RetryableFailure { .. } => Ok(CacheLookup::Miss),
            }
        }

        fn store_success(
            &self,
            key: &StageCacheKey,
            payload: &[u8],
            metadata: BTreeMap<String, String>,
            recorded_at: &str,
        ) -> PipelineCacheResult<StageManifestEntry> {
            let entry = StageManifestEntry {
                key: key.clone(),
                key_sha256: format!("test-key-{}", key.stage_id),
                recorded_at: recorded_at.into(),
                outcome: StageOutcome::Success {
                    payload_sha256: payload_sha256(payload),
                    payload_size: payload.len() as u64,
                },
                metadata,
            };
            self.entries
                .lock()
                .unwrap()
                .retain(|(stored, _, _)| stored != key);
            self.entries
                .lock()
                .unwrap()
                .push((key.clone(), entry.clone(), payload.to_vec()));
            Ok(entry)
        }

        fn store_retryable_failure(
            &self,
            key: &StageCacheKey,
            error_class: &str,
            message: &str,
            retry_after: &str,
            recorded_at: &str,
        ) -> PipelineCacheResult<StageManifestEntry> {
            let entry = StageManifestEntry {
                key: key.clone(),
                key_sha256: format!("test-key-{}", key.stage_id),
                recorded_at: recorded_at.into(),
                outcome: StageOutcome::RetryableFailure {
                    error_class: error_class.into(),
                    message: message.into(),
                    retry_after: retry_after.into(),
                },
                metadata: BTreeMap::new(),
            };
            self.entries
                .lock()
                .unwrap()
                .retain(|(stored, _, _)| stored != key);
            self.entries
                .lock()
                .unwrap()
                .push((key.clone(), entry.clone(), Vec::new()));
            Ok(entry)
        }

        fn manifest(&self, article_id: &str) -> PipelineCacheResult<Vec<StageManifestEntry>> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .iter()
                .filter(|(key, _, _)| key.source.article_id == article_id)
                .map(|(_, entry, _)| entry.clone())
                .collect())
        }
    }

    struct FakeStage {
        contract: StageContract,
        payload: Vec<u8>,
        calls: AtomicUsize,
        failure: Option<StageExecutionError>,
    }

    impl FakeStage {
        fn success(stage_id: &str, payload: &[u8]) -> Self {
            Self {
                contract: StageContract {
                    stage_id: stage_id.into(),
                    implementation_fingerprint: format!("{stage_id}-impl-v1"),
                    configuration_fingerprint: format!("{stage_id}-config-v1"),
                    prompt_fingerprint: None,
                    model_fingerprint: None,
                    output_schema_fingerprint: format!("{stage_id}-schema-v1"),
                },
                payload: payload.to_vec(),
                calls: AtomicUsize::new(0),
                failure: None,
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl PipelineStageExecutor for FakeStage {
        fn contract(&self) -> StageContract {
            self.contract.clone()
        }

        async fn execute(
            &self,
            _request: StageExecutionRequest,
        ) -> Result<StageExecutionOutput, StageExecutionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.failure {
                return Err(error.clone());
            }
            Ok(StageExecutionOutput {
                payload: self.payload.clone(),
                metadata: BTreeMap::new(),
            })
        }
    }

    fn source(pdf_hash: &str) -> SourceIdentity {
        SourceIdentity {
            article_id: "2507.19457".into(),
            selected_format: da_ports::pipeline_cache::SourceFormat::Pdf,
            pdf_sha256: Some(pdf_hash.into()),
            html_sha256: None,
            acquisition_fingerprint: "catalog-v1".into(),
            normalization_fingerprint: "normalization-v1".into(),
        }
    }

    fn pipeline() -> Pipeline {
        PipelineBuilder::new("article-thin")
            .step("parse", "parse", &[])
            .step("summary", "summary", &["parse"])
            .build()
            .unwrap()
    }

    fn clock() -> Arc<dyn Clock> {
        Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 8, 8, 0, 0, 0).unwrap(),
        ))
    }

    #[test]
    fn executor_registry_rejects_contract_mismatch() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let error = PipelineOrchestrator::new(cache, clock())
            .with_executor("parse", summary)
            .err()
            .expect("mismatched executor contract must fail");
        assert!(matches!(
            error,
            PipelineOrchestratorError::ContractMismatch { .. }
        ));
    }

    #[tokio::test]
    async fn run_rejects_missing_executor_instead_of_using_a_stub() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let error = PipelineOrchestrator::new(cache, clock())
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PipelineOrchestratorError::MissingExecutor { ref stage_id }
                if stage_id == "parse"
        ));
    }

    #[tokio::test]
    async fn first_run_executes_and_unchanged_second_run_hits_cache() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed-gepa"));
        let summary = Arc::new(FakeStage::success(
            "summary",
            b"GEPA improves 6% with 35x fewer rollouts",
        ));
        let orchestrator = PipelineOrchestrator::new(cache, clock())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap();

        let first = orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        assert!(first.completed);
        assert!(
            first
                .stages
                .iter()
                .all(|record| matches!(record.disposition, StageDisposition::Executed { .. }))
        );

        let second = orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        assert!(second.completed);
        assert!(
            second
                .stages
                .iter()
                .all(|record| matches!(record.disposition, StageDisposition::CacheHit { .. }))
        );
        assert_eq!(parse.calls(), 1, "parse/GROBID-equivalent must not rerun");
        assert_eq!(summary.calls(), 1, "summary/LLM-equivalent must not rerun");
    }

    #[tokio::test]
    async fn prompt_change_reruns_summary_but_not_parse() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed-gepa"));
        let summary_v1 = Arc::new(FakeStage::success("summary", b"summary-v1"));
        let first = PipelineOrchestrator::new(cache.clone(), clock())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary_v1.clone())
            .unwrap();
        first.run(&pipeline(), source("pdf-v1")).await.unwrap();

        let mut changed = FakeStage::success("summary", b"summary-v2");
        changed.contract.prompt_fingerprint = Some("prompt-v2".into());
        let summary_v2 = Arc::new(changed);
        let second = PipelineOrchestrator::new(cache, clock())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary_v2.clone())
            .unwrap()
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();

        assert!(matches!(
            second.stages[0].disposition,
            StageDisposition::CacheHit { .. }
        ));
        assert!(matches!(
            second.stages[1].disposition,
            StageDisposition::Executed { .. }
        ));
        assert_eq!(parse.calls(), 1);
        assert_eq!(summary_v1.calls(), 1);
        assert_eq!(summary_v2.calls(), 1);
    }

    #[tokio::test]
    async fn source_change_reruns_source_dependent_descendants() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let orchestrator = PipelineOrchestrator::new(cache, clock())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap();
        orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        orchestrator
            .run(&pipeline(), source("pdf-v2"))
            .await
            .unwrap();
        assert_eq!(parse.calls(), 2);
        assert_eq!(summary.calls(), 2);
    }

    #[tokio::test]
    async fn retryable_failure_is_visible_and_suppresses_early_retry() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let mut failing = FakeStage::success("parse", b"");
        failing.failure = Some(StageExecutionError::Retryable {
            error_class: "grobid_unavailable".into(),
            message: "GROBID returned 503".into(),
            retry_after: "2026-08-08T01:00:00Z".into(),
        });
        let parse = Arc::new(failing);
        let orchestrator = PipelineOrchestrator::new(cache, clock())
            .with_executor("parse", parse.clone())
            .unwrap();
        let one_stage = PipelineBuilder::new("parse-only")
            .step("parse", "parse", &[])
            .build()
            .unwrap();

        let first = orchestrator
            .run(&one_stage, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!first.completed);
        assert!(matches!(
            first.stages[0].disposition,
            StageDisposition::Failed {
                retryable: true,
                ..
            }
        ));
        let second = orchestrator
            .run(&one_stage, source("pdf-v1"))
            .await
            .unwrap();
        assert!(matches!(
            second.stages[0].disposition,
            StageDisposition::RetrySuppressed { .. }
        ));
        assert_eq!(parse.calls(), 1, "negative cache must suppress early retry");
    }
}
