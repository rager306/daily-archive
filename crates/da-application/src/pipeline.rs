//! Cache-aware article pipeline orchestration (M371 S01 T02).
//!
//! `kg-pipeline` owns topology validation. This application service owns
//! article-specific effective-input keys and stage outcomes. Concrete parser,
//! LLM, JAR and filesystem adapters are injected through ports.

use da_domain::{AdmissionDecision, ArticleContextState, StageIdRegistry, admit_stage_transition};
use da_ports::clock::Clock;
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, SourceIdentity, StageCacheKey, StageOutcome,
};
use da_ports::pipeline_stage::{PipelineStageExecutor, StageExecutionError, StageExecutionRequest};
use kg_pipeline::{Pipeline, PipelineBuilder};
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
    /// Canonical pipeline stage ID when a registry is configured.
    pub stage_id: String,
    /// Raw executor or pipeline alias resolved to `stage_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_from: Option<String>,
    /// Exact cache contract identity. Required for durable payload replay when
    /// multiple historical contracts produced identical output bytes.
    #[serde(default)]
    pub cache_key_sha256: Option<String>,
    /// ACAP runtime admission decision (PIR-014) computed from the previous
    /// stage state and the canonical stage id. `None` when the stage is outside
    /// the ACAP FSM mapping (gate not applicable — the report surface exposes
    /// these as `skipped` transitions). Blocked decisions carry the typed
    /// reason and evidence reference in the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<AdmissionDecision>,
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
    #[error("pipeline stage identity invalid: {0}")]
    StageIdentity(String),
    #[error("pipeline executor registration duplicated: '{registration}'")]
    ExecutorCollision { registration: String },
    #[error(transparent)]
    StagePayload(#[from] StageExecutionError),
    #[error(transparent)]
    Cache(#[from] PipelineCacheError),
}

pub struct PipelineOrchestrator {
    cache: Arc<dyn PipelineCache>,
    clock: Arc<dyn Clock>,
    stage_registry: Arc<StageIdRegistry>,
    executors: HashMap<String, Arc<dyn PipelineStageExecutor>>,
}

impl PipelineOrchestrator {
    pub fn new(
        cache: Arc<dyn PipelineCache>,
        clock: Arc<dyn Clock>,
        stage_registry: Arc<StageIdRegistry>,
    ) -> Self {
        Self {
            cache,
            clock,
            stage_registry,
            executors: HashMap::new(),
        }
    }

    fn canonical_stage_id(&self, raw: &str) -> Result<String, PipelineOrchestratorError> {
        self.stage_registry
            .resolve(raw)
            .map(|id| id.as_str().to_owned())
            .map_err(PipelineOrchestratorError::StageIdentity)
    }

    pub fn with_executor(
        mut self,
        stage_id: impl Into<String>,
        executor: Arc<dyn PipelineStageExecutor>,
    ) -> Result<Self, PipelineOrchestratorError> {
        let stage_id = stage_id.into();
        let contract = executor.contract();
        let canonical_registry_key = self.canonical_stage_id(&stage_id)?;
        let canonical_contract_stage = self.canonical_stage_id(&contract.stage_id)?;
        if canonical_contract_stage != canonical_registry_key {
            return Err(PipelineOrchestratorError::ContractMismatch {
                registry_key: stage_id,
                contract_stage: contract.stage_id,
            });
        }
        if self.executors.contains_key(&stage_id) {
            return Err(PipelineOrchestratorError::ExecutorCollision {
                registration: stage_id,
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
        // ACAP runtime admission state (PIR-014): the ACAP state established by
        // the previous gated stage plus the durable evidence (cache key sha256)
        // that establishes it. `None` until the first gated stage is admitted.
        let mut prior_state: Option<ArticleContextState> = None;
        let mut prior_evidence: Option<String> = None;

        for step in &pipeline.steps {
            let canonical_stage_id = self.canonical_stage_id(&step.stage)?;
            let resolved_from = (canonical_stage_id != step.stage).then(|| step.stage.clone());
            let executor = self.executors.get(&step.stage).ok_or_else(|| {
                PipelineOrchestratorError::MissingExecutor {
                    stage_id: step.stage.clone(),
                }
            })?;
            let contract = executor.contract();
            if self.canonical_stage_id(&contract.stage_id)? != canonical_stage_id {
                return Err(PipelineOrchestratorError::ContractMismatch {
                    registry_key: step.stage.clone(),
                    contract_stage: contract.stage_id,
                });
            }

            // ── ACAP admission gate (PIR-014) ────────────────────────────────
            // Computed once per step, before cache-hit acceptance and before
            // executor execution, from the prior stage state and the DATA
            // `acap_layer` declared on the canonical stage. Both cache paths
            // (canonical hit and legacy alias fallback) and execution sit
            // behind the same decision; a blocked transition fails closed
            // with a typed reason and a durable evidence reference, and the
            // executor never runs. Stages with no declared layer are not
            // gated (report surface: skipped).
            let target_state = self
                .stage_registry
                .acap_layer(&canonical_stage_id)
                .map_err(PipelineOrchestratorError::StageIdentity)?;
            let admission = target_state.map(|target| {
                let evidence_ref = prior_evidence
                    .clone()
                    .unwrap_or_else(|| source_entry_evidence(&source).unwrap_or_default());
                admit_stage_transition(prior_state, target, &evidence_ref)
            });
            if let Some(decision) = &admission {
                if !decision.admitted {
                    let error = decision
                        .error
                        .as_ref()
                        .expect("blocked admission decision carries a typed error");
                    tracing::warn!(
                        article_id = %source.article_id,
                        pipeline_id = %pipeline.id,
                        step = %step.name,
                        stage = %canonical_stage_id,
                        resolved_from = resolved_from.as_deref().unwrap_or(""),
                        error_class = %error.error_class(),
                        reason = %error.reason,
                        evidence_ref = %error.evidence_ref,
                        event = "pipeline_stage_admission_blocked"
                    );
                    records.push(StageRunRecord {
                        step_name: step.name.clone(),
                        stage_id: canonical_stage_id.clone(),
                        resolved_from: resolved_from.clone(),
                        cache_key_sha256: None,
                        admission: Some(decision.clone()),
                        disposition: StageDisposition::Failed {
                            error_class: error.error_class(),
                            message: error.to_string(),
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
                tracing::info!(
                    article_id = %source.article_id,
                    pipeline_id = %pipeline.id,
                    step = %step.name,
                    stage = %canonical_stage_id,
                    resolved_from = resolved_from.as_deref().unwrap_or(""),
                    evidence_ref = %decision.evidence_ref,
                    event = "pipeline_stage_admitted"
                );
                prior_state = target_state;
            } else {
                tracing::debug!(
                    article_id = %source.article_id,
                    pipeline_id = %pipeline.id,
                    step = %step.name,
                    stage = %canonical_stage_id,
                    reason = "stage outside ACAP FSM mapping",
                    event = "pipeline_stage_admission_skipped"
                );
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
                stage_id: canonical_stage_id.clone(),
                dependency_output_sha256,
                implementation_fingerprint: contract.implementation_fingerprint,
                configuration_fingerprint: contract.configuration_fingerprint,
                prompt_fingerprint: contract.prompt_fingerprint,
                model_fingerprint: contract.model_fingerprint,
                output_schema_fingerprint: contract.output_schema_fingerprint,
            };

            let (lookup, cache_resolved_from) = match self.cache.lookup(&key, &now)? {
                CacheLookup::Miss if step.name != canonical_stage_id => {
                    let mut legacy_key = key.clone();
                    legacy_key.stage_id = step.name.clone();
                    let legacy = self.cache.lookup(&legacy_key, &now)?;
                    let resolved =
                        (!matches!(legacy, CacheLookup::Miss)).then(|| step.name.clone());
                    (legacy, resolved)
                }
                lookup => (lookup, None),
            };

            match lookup {
                CacheLookup::Hit(hit) => {
                    executor.validate_cached_payload(&hit.payload)?;
                    tracing::info!(
                        article_id = %source.article_id,
                        pipeline_id = %pipeline.id,
                        step = %step.name,
                        stage = %canonical_stage_id,
                        resolved_from = resolved_from.as_deref().unwrap_or(""),
                        cache_resolved_from = cache_resolved_from.as_deref().unwrap_or(""),
                        event = "pipeline_stage_cache_hit"
                    );
                    let output_sha256 = payload_sha256(&hit.payload);
                    let cache_key_sha256 = hit.manifest.key_sha256.clone();
                    outputs.insert(step.name.clone(), hit.payload);
                    output_hashes.insert(step.name.clone(), output_sha256.clone());
                    prior_evidence = Some(cache_key_sha256.clone());
                    records.push(StageRunRecord {
                        step_name: step.name.clone(),
                        stage_id: canonical_stage_id.clone(),
                        resolved_from: resolved_from.clone(),
                        cache_key_sha256: Some(cache_key_sha256),
                        admission: admission.clone(),
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
                        stage = %canonical_stage_id,
                        resolved_from = resolved_from.as_deref().unwrap_or(""),
                        cache_resolved_from = cache_resolved_from.as_deref().unwrap_or(""),
                        retry_after = %retry_after,
                        event = "pipeline_stage_retry_suppressed"
                    );
                    records.push(StageRunRecord {
                        step_name: step.name.clone(),
                        stage_id: canonical_stage_id.clone(),
                        resolved_from: resolved_from.clone(),
                        cache_key_sha256: Some(manifest.key_sha256),
                        admission: admission.clone(),
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
                        stage = %canonical_stage_id,
                        resolved_from = resolved_from.as_deref().unwrap_or(""),
                        cache_resolved_from = cache_resolved_from.as_deref().unwrap_or(""),
                        event = "pipeline_stage_cache_miss"
                    );
                    let request = StageExecutionRequest {
                        article_id: source.article_id.clone(),
                        dependency_payloads,
                    };
                    match executor.execute(request).await {
                        Ok(output) => {
                            let output_sha256 = payload_sha256(&output.payload);
                            let manifest = self.cache.store_success(
                                &key,
                                &output.payload,
                                output.metadata,
                                &now,
                            )?;
                            outputs.insert(step.name.clone(), output.payload);
                            output_hashes.insert(step.name.clone(), output_sha256.clone());
                            prior_evidence = Some(manifest.key_sha256.clone());
                            tracing::info!(
                                article_id = %source.article_id,
                                pipeline_id = %pipeline.id,
                                step = %step.name,
                                stage = %canonical_stage_id,
                                resolved_from = resolved_from.as_deref().unwrap_or(""),
                                output_sha256 = %output_sha256,
                                event = "pipeline_stage_executed"
                            );
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: canonical_stage_id.clone(),
                                resolved_from: resolved_from.clone(),
                                cache_key_sha256: Some(manifest.key_sha256),
                                admission: admission.clone(),
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
                                stage = %canonical_stage_id,
                                resolved_from = resolved_from.as_deref().unwrap_or(""),
                                error_class = %error_class,
                                retry_after = %retry_after,
                                event = "pipeline_stage_retryable_failure"
                            );
                            let manifest = self.cache.store_retryable_failure(
                                &key,
                                &error_class,
                                &message,
                                &retry_after,
                                &now,
                            )?;
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: canonical_stage_id.clone(),
                                resolved_from: resolved_from.clone(),
                                cache_key_sha256: Some(manifest.key_sha256),
                                admission: admission.clone(),
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
                                stage = %canonical_stage_id,
                                resolved_from = resolved_from.as_deref().unwrap_or(""),
                                error_class = %error_class,
                                event = "pipeline_stage_terminal_failure"
                            );
                            records.push(StageRunRecord {
                                step_name: step.name.clone(),
                                stage_id: canonical_stage_id.clone(),
                                resolved_from: resolved_from.clone(),
                                cache_key_sha256: None,
                                admission: admission.clone(),
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

/// Durable entry evidence for the first gated stage, derived from the
/// checksum-bound source identity (never a caller-supplied free-form string).
/// `None` when the run carries no source checksum — the gate then fails closed
/// with `missing_entry_state` on the first gated stage.
fn source_entry_evidence(source: &SourceIdentity) -> Option<String> {
    if let Some(hash) = source
        .pdf_sha256
        .as_deref()
        .or(source.html_sha256.as_deref())
    {
        return Some(format!("source:sha256:{hash}"));
    }
    let fingerprint = source.acquisition_fingerprint.trim();
    if !fingerprint.is_empty() {
        return Some(format!("acquisition:{fingerprint}"));
    }
    None
}

/// Canonical multimodal production topology (M400 S04 T02).
///
/// Wires the five canonical multimodal stages (figure_context, figure_analysis,
/// table_fidelity, equation_extraction, candidate_materialization) plus their
/// parse prerequisites into one DAG. Step names follow the working example
/// topology (`pipeline_figures.rs`): `parse` and `figures` (figure_detect — a
/// parse-family alias in the DATA contract) are separate steps, figure_context
/// consumes both, and candidate materialization converges on analysis + fidelity
/// + equations. Every stage id is validated against the DATA registry so alias
///   smuggling cannot inject a non-canonical stage into the composition.
#[derive(Debug)]
pub struct MultimodalComposition {
    pub pipeline_id: String,
    pub topology: Pipeline,
    /// Unique canonical stage ids in topology order (deduplicated).
    pub canonical_stages: Vec<String>,
}

impl MultimodalComposition {
    /// Canonical multimodal topology validated against the DATA registry.
    pub fn canonical(
        stage_registry: &StageIdRegistry,
        pipeline_id: &str,
    ) -> Result<Self, PipelineOrchestratorError> {
        let required = [
            "parse",
            "figure_context",
            "figure_analysis",
            "table_fidelity",
            "equation_extraction",
            "candidate_materialization",
        ];
        for id in required {
            stage_registry
                .parse_canonical(id)
                .map_err(PipelineOrchestratorError::StageIdentity)?;
        }
        // Executor-key aliases used by the topology (figure_detect is a
        // parse-family alias per the DATA contract).
        for alias in ["grobid_parse", "figure_detect"] {
            stage_registry
                .resolve(alias)
                .map_err(PipelineOrchestratorError::StageIdentity)?;
        }
        let topology = PipelineBuilder::new(pipeline_id)
            .step("parse", "grobid_parse", &[])
            .step("figures", "figure_detect", &[])
            .step("context", "figure_context", &["parse", "figures"])
            .step("analysis", "figure_analysis", &["context"])
            .step("fidelity", "table_fidelity", &["parse"])
            .step("equations", "equation_extraction", &["parse"])
            .step(
                "materialize",
                "candidate_materialization",
                &["analysis", "fidelity", "equations"],
            )
            .build()
            .map_err(PipelineOrchestratorError::StageIdentity)?;
        let mut seen = std::collections::BTreeSet::new();
        let mut canonical_stages = Vec::new();
        for step in &topology.steps {
            let canonical = stage_registry
                .resolve(&step.stage)
                .map_err(PipelineOrchestratorError::StageIdentity)?
                .as_str()
                .to_owned();
            if seen.insert(canonical.clone()) {
                canonical_stages.push(canonical);
            }
        }
        Ok(Self {
            pipeline_id: pipeline_id.to_owned(),
            topology,
            canonical_stages,
        })
    }
}

/// Exact cache-identity fingerprint binding for a multimodal production
/// composition (M400 S04 T02). Every field is derived from live stage
/// contracts and the env/config surfaces that produced them — never a
/// hardcoded runtime value. The summary makes the effective-input contract of
/// a production run observable: source, GROBID profile, detector, schema
/// registry, model route, and output schema identity all rotate the cache key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MultimodalFingerprintSummary {
    /// Checksum-bound source identity (article id + acquisition fingerprint).
    pub source: String,
    /// GROBID consolidation policy configuration fingerprint.
    pub grobid_profile: String,
    /// Figure detector fingerprint (model_fingerprint of figure_detect).
    pub detector: String,
    /// Figure schema registry version + per-schema identities.
    pub schema_registry: String,
    /// Model route binding (route id | protocol | model | fallback).
    pub model_route: String,
    /// Per canonical stage output schema fingerprints.
    pub output_schemas: BTreeMap<String, String>,
    /// Candidate-only graph truth boundary (D127): never true for candidates.
    pub import_eligible: bool,
}

impl MultimodalFingerprintSummary {
    /// Bind fingerprints from the live stage contracts of a composed run.
    pub fn bind(
        source: &SourceIdentity,
        grobid_profile: &str,
        contracts: &BTreeMap<String, da_ports::pipeline_stage::StageContract>,
    ) -> Self {
        let detector = contracts
            .get("figure_detect")
            .and_then(|contract| contract.model_fingerprint.clone())
            .unwrap_or_else(|| "detector:unbound".into());
        let schema_registry = contracts
            .get("figure_analysis")
            .map(|contract| contract.configuration_fingerprint.clone())
            .unwrap_or_else(|| "schema_registry:unbound".into());
        let model_route = contracts
            .get("figure_analysis")
            .and_then(|contract| contract.model_fingerprint.clone())
            .unwrap_or_else(|| "model_route:unbound".into());
        let output_schemas = contracts
            .iter()
            .map(|(stage, contract)| (stage.clone(), contract.output_schema_fingerprint.clone()))
            .collect::<BTreeMap<_, _>>();
        Self {
            source: format!(
                "article={};acq={}",
                source.article_id, source.acquisition_fingerprint
            ),
            grobid_profile: grobid_profile.to_owned(),
            detector,
            schema_registry,
            model_route,
            output_schemas,
            import_eligible: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use da_domain::TransitionAdmissionCode;
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
        enforce_extraction_schema: bool,
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
                enforce_extraction_schema: false,
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

        fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
            if !self.enforce_extraction_schema {
                return Ok(());
            }
            da_ports::cache_payload::decode_extracted_entities(payload)
                .map(|_| ())
                .map_err(|error| StageExecutionError::Terminal {
                    error_class: "incompatible_cache_payload".into(),
                    message: error.to_string(),
                })
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

    fn test_acap_layer(id: &str) -> Option<da_domain::ArticleContextState> {
        use da_domain::ArticleContextState::*;
        match id {
            "acquisition" => Some(SourceRegistered),
            "catalog" => Some(SourceVerified),
            "parse" => Some(Parsed),
            "figure_context" => Some(Structured),
            "figure_analysis" => Some(Extracted),
            "table_fidelity" => Some(EvidenceLinked),
            "equation_extraction" => Some(Embedded),
            "revision_materialization" => Some(Structured),
            "extraction_compute" => Some(Extracted),
            "extraction_materialization" => Some(EvidenceLinked),
            "embedding" => Some(Embedded),
            "summary" => Some(Summarized),
            "resolution" => Some(Resolved),
            "checkpoint" => Some(AcceptedCheckpoint),
            _ => None,
        }
    }

    fn stage_contract(id: &str, aliases: &[&str]) -> da_domain::PipelineStageContract {
        da_domain::PipelineStageContract {
            id: id.into(),
            aliases: aliases.iter().map(|alias| (*alias).into()).collect(),
            owner: da_domain::StageOwner::Application,
            input_semantics: vec!["input".into()],
            output_semantics: vec!["output".into()],
            depends_on: vec![],
            context_adds: vec!["identity".into()],
            context_losses: vec![],
            hard_invariants: vec!["D127_false".into()],
            consumers: vec!["test".into()],
            cache_mode: da_domain::StageCacheMode::PureCompute,
            failure_class: "terminal".into(),
            conditional: false,
            graph_truth_boundary: false,
            candidate_only: false,
            acap_layer: test_acap_layer(id),
        }
    }

    fn registry() -> Arc<StageIdRegistry> {
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage_contract("parse", &["grobid_parse", "html_parse"]),
                stage_contract("summary", &[]),
            ],
            import_eligible: false,
        };
        Arc::new(ledger.registry().unwrap())
    }

    /// Registry covering the full ACAP FSM mapping plus one stage (`readiness`)
    /// outside the FSM — used by the T03 fail-closed admission tests.
    fn fsm_registry() -> Arc<StageIdRegistry> {
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage_contract("acquisition", &[]),
                stage_contract("catalog", &[]),
                stage_contract("parse", &["grobid_parse", "html_parse"]),
                stage_contract("extraction_compute", &[]),
                stage_contract("embedding", &[]),
                stage_contract("resolution", &[]),
                stage_contract("summary", &[]),
                stage_contract("checkpoint", &[]),
                stage_contract("readiness", &[]),
            ],
            import_eligible: false,
        };
        Arc::new(ledger.registry().unwrap())
    }

    /// Registry covering the canonical multimodal stage identities (M400 S04):
    /// the four gated compute stages plus the ungated candidate materialization
    /// helper.
    fn multimodal_registry() -> Arc<StageIdRegistry> {
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage_contract("parse", &["grobid_parse", "figure_detect"]),
                stage_contract("figure_context", &["figure_context_artifact"]),
                stage_contract("figure_analysis", &[]),
                stage_contract("table_fidelity", &[]),
                stage_contract("equation_extraction", &[]),
                stage_contract("candidate_materialization", &[]),
            ],
            import_eligible: false,
        };
        Arc::new(ledger.registry().unwrap())
    }

    /// Registry for the linear article materialization spine (M402 S02):
    /// parse establishes Parsed, revision_materialization Structured,
    /// extraction_compute Extracted, extraction_materialization EvidenceLinked.
    fn spine_registry() -> Arc<StageIdRegistry> {
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage_contract("parse", &[]),
                stage_contract("revision_materialization", &[]),
                stage_contract("extraction_compute", &[]),
                stage_contract("extraction_materialization", &[]),
                stage_contract("embedding", &[]),
                stage_contract("summary", &[]),
                stage_contract("checkpoint", &[]),
            ],
            import_eligible: false,
        };
        Arc::new(ledger.registry().unwrap())
    }

    /// Build a cache key for a canonical stage with the given dependency output
    /// hashes and the executor's exact contract fingerprints.
    fn cache_key(
        source: &SourceIdentity,
        stage_id: &str,
        deps: BTreeMap<String, String>,
        contract: &StageContract,
    ) -> StageCacheKey {
        StageCacheKey {
            source: source.clone(),
            stage_id: stage_id.into(),
            dependency_output_sha256: deps,
            implementation_fingerprint: contract.implementation_fingerprint.clone(),
            configuration_fingerprint: contract.configuration_fingerprint.clone(),
            prompt_fingerprint: contract.prompt_fingerprint.clone(),
            model_fingerprint: contract.model_fingerprint.clone(),
            output_schema_fingerprint: contract.output_schema_fingerprint.clone(),
        }
    }

    #[tokio::test]
    async fn registry_canonicalizes_executor_alias_cache_key_and_report() {
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![stage_contract("parse", &["grobid_parse"])],
            import_eligible: false,
        };
        let registry = Arc::new(ledger.registry().unwrap());
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("grobid_parse", b"parsed"));
        let pipeline = PipelineBuilder::new("alias-parse")
            .step("parse-step", "grobid_parse", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache.clone(), clock(), registry)
            .with_executor("grobid_parse", parse)
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert_eq!(report.stages[0].stage_id, "parse");
        assert_eq!(
            report.stages[0].resolved_from.as_deref(),
            Some("grobid_parse")
        );
        assert_eq!(cache.entries.lock().unwrap()[0].0.stage_id, "parse");
    }

    #[test]
    fn executor_registry_rejects_duplicate_raw_registration() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let first = Arc::new(FakeStage::success("grobid_parse", b"first"));
        let second = Arc::new(FakeStage::success("grobid_parse", b"second"));
        let error = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("grobid_parse", first)
            .unwrap()
            .with_executor("grobid_parse", second)
            .err()
            .expect("duplicate raw registration must fail");
        assert!(matches!(
            error,
            PipelineOrchestratorError::ExecutorCollision { ref registration }
                if registration == "grobid_parse"
        ));
    }

    #[test]
    fn executor_registry_rejects_contract_mismatch() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let error = PipelineOrchestrator::new(cache, clock(), registry())
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
        let error = PipelineOrchestrator::new(cache, clock(), registry())
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
    async fn orchestrator_rejects_invalid_same_key_cached_payload_before_hit() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let mut stage = FakeStage::success("parse", b"[]");
        stage.enforce_extraction_schema = true;
        let contract = stage.contract();
        let invalid_payload = serde_json::to_vec(&serde_json::json!([{
            "label": "GEPA",
            "entity_type": "Method",
            "section_title": "Methods",
            "char_start": 12,
            "char_end": 16,
            "surface": "GEPA"
        }]))
        .unwrap();
        let key = StageCacheKey {
            source: source("pdf-v1"),
            stage_id: "parse".into(),
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: contract.implementation_fingerprint,
            configuration_fingerprint: contract.configuration_fingerprint,
            prompt_fingerprint: contract.prompt_fingerprint,
            model_fingerprint: contract.model_fingerprint,
            output_schema_fingerprint: contract.output_schema_fingerprint,
        };
        cache
            .store_success(
                &key,
                &invalid_payload,
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let one_stage = PipelineBuilder::new("schema-fence")
            .step("parse", "parse", &[])
            .build()
            .unwrap();
        let stage = Arc::new(stage);
        let error = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", stage.clone())
            .unwrap()
            .run(&one_stage, source("pdf-v1"))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PipelineOrchestratorError::StagePayload(StageExecutionError::Terminal {
                ref error_class,
                ..
            }) if error_class == "incompatible_cache_payload"
        ));
        assert_eq!(
            stage.calls(),
            0,
            "invalid cached payload must not execute stage"
        );
    }

    #[tokio::test]
    async fn legacy_step_retryable_failure_suppresses_canonical_execution() {
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("grobid_parse", b"must-not-run"));
        let contract = parse.contract();
        let legacy_key = StageCacheKey {
            source: source("pdf-v1"),
            stage_id: "legacy-parse-cache".into(),
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: contract.implementation_fingerprint,
            configuration_fingerprint: contract.configuration_fingerprint,
            prompt_fingerprint: contract.prompt_fingerprint,
            model_fingerprint: contract.model_fingerprint,
            output_schema_fingerprint: contract.output_schema_fingerprint,
        };
        cache
            .store_retryable_failure(
                &legacy_key,
                "grobid_unavailable",
                "retry later",
                "2026-08-08T01:00:00Z",
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let pipeline = PipelineBuilder::new("legacy-negative-cache")
            .step("legacy-parse-cache", "grobid_parse", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("grobid_parse", parse.clone())
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::RetrySuppressed { .. }
        ));
        assert_eq!(report.stages[0].stage_id, "parse");
        assert_eq!(
            parse.calls(),
            0,
            "legacy negative cache must suppress execution"
        );
    }

    #[tokio::test]
    async fn first_run_executes_and_unchanged_second_run_hits_cache() {
        let cache: Arc<dyn PipelineCache> = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed-gepa"));
        let summary = Arc::new(FakeStage::success(
            "summary",
            b"GEPA improves 6% with 35x fewer rollouts",
        ));
        let orchestrator = PipelineOrchestrator::new(cache, clock(), registry())
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
        let first = PipelineOrchestrator::new(cache.clone(), clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary_v1.clone())
            .unwrap();
        first.run(&pipeline(), source("pdf-v1")).await.unwrap();

        let mut changed = FakeStage::success("summary", b"summary-v2");
        changed.contract.prompt_fingerprint = Some("prompt-v2".into());
        let summary_v2 = Arc::new(changed);
        let second = PipelineOrchestrator::new(cache, clock(), registry())
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
        let orchestrator = PipelineOrchestrator::new(cache, clock(), registry())
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
        let orchestrator = PipelineOrchestrator::new(cache, clock(), registry())
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

    // ── PIR-014 fail-closed admission tests (T03) ──

    #[tokio::test]
    async fn mid_fsm_entry_fails_closed_before_executor_runs() {
        // A run that starts mid-FSM (first step = summary) must fail closed on
        // the entry transition: no default-admit, executor call count 0, typed
        // reason + durable evidence ref in the Failed disposition.
        let cache = Arc::new(MemoryCache::default());
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let mid_fsm = PipelineBuilder::new("mid-fsm-entry")
            .step("summary", "summary", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&mid_fsm, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert_eq!(report.stages.len(), 1);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::Failed {
                ref error_class,
                retryable: false,
                ..
            } if error_class == "admission:illegal_transition"
        ));
        let admission = report.stages[0]
            .admission
            .as_ref()
            .expect("blocked record carries admission");
        assert!(!admission.admitted);
        let error = admission.error.as_ref().expect("typed error");
        assert_eq!(error.code, TransitionAdmissionCode::IllegalTransition);
        assert_eq!(error.from, ArticleContextState::SourceRegistered);
        assert_eq!(error.to, ArticleContextState::Summarized);
        assert_eq!(error.evidence_ref, "source:sha256:pdf-v1");
        assert_eq!(
            summary.calls(),
            0,
            "blocked transition must never reach the executor"
        );
    }

    #[tokio::test]
    async fn fsm_regression_fails_closed_without_running_regressed_stage() {
        // parse -> summary -> parse regresses the ACAP FSM: the third step is
        // blocked even though its executor is registered and its dependencies
        // are present. The regressed stage must not execute.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let regressed = PipelineBuilder::new("fsm-regression")
            .step("parse", "parse", &[])
            .step("summary", "summary", &["parse"])
            .step("parse-again", "parse", &["summary"])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&regressed, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert_eq!(
            report.stages.len(),
            3,
            "blocked step still records one Failed row"
        );
        assert!(matches!(
            report.stages[2].disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        let error = report.stages[2]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.from, ArticleContextState::Summarized);
        assert_eq!(error.to, ArticleContextState::Parsed);
        assert_eq!(
            parse.calls(),
            1,
            "parse ran for step 1 only; the regressed step 3 must not run"
        );
        assert_eq!(summary.calls(), 1);
    }

    #[tokio::test]
    async fn terminal_failure_does_not_chain_to_next_stage() {
        // A non-FailedVisible failure (terminal executor error) must fail the
        // run closed: the stage after the failed disposition never executes.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let mut failing = FakeStage::success("summary", b"");
        failing.failure = Some(StageExecutionError::Terminal {
            error_class: "llm_unavailable".into(),
            message: "LLM provider rejected request".into(),
        });
        let summary = Arc::new(failing);
        let chain = PipelineBuilder::new("terminal-fail-chain")
            .step("parse", "parse", &[])
            .step("summary", "summary", &["parse"])
            .step("summary-again", "summary", &["summary"])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&chain, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert_eq!(report.stages.len(), 2, "run must stop at the failed stage");
        assert!(matches!(
            report.stages[1].disposition,
            StageDisposition::Failed {
                ref error_class,
                retryable: false,
                ..
            } if error_class == "llm_unavailable"
        ));
        assert_eq!(
            summary.calls(),
            1,
            "failed disposition must not chain to the next stage"
        );
    }

    #[tokio::test]
    async fn retryable_failure_does_not_chain_to_next_stage() {
        // Same fail-closed property for a retryable (non-FailedVisible)
        // failure: the run returns completed=false and the next stage never
        // executes.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let mut failing = FakeStage::success("summary", b"");
        failing.failure = Some(StageExecutionError::Retryable {
            error_class: "grobid_unavailable".into(),
            message: "GROBID returned 503".into(),
            retry_after: "2026-08-08T01:00:00Z".into(),
        });
        let summary = Arc::new(failing);
        let chain = PipelineBuilder::new("retryable-fail-chain")
            .step("parse", "parse", &[])
            .step("summary", "summary", &["parse"])
            .step("summary-again", "summary", &["summary"])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&chain, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert_eq!(report.stages.len(), 2, "run must stop at the failed stage");
        assert!(matches!(
            report.stages[1].disposition,
            StageDisposition::Failed {
                ref error_class,
                retryable: true,
                ..
            } if error_class == "grobid_unavailable"
        ));
        assert_eq!(
            summary.calls(),
            1,
            "retryable failure must not chain to the next stage"
        );
    }

    #[tokio::test]
    async fn legacy_linear_fsm_chain_passes_with_admission_evidence() {
        // The legacy linear pipeline shape (full ACAP chain, skipping layers) is
        // monotonic forward: every stage is admitted with a durable evidence
        // reference and the run completes with exactly one record per step.
        let cache = Arc::new(MemoryCache::default());
        let ids = [
            "acquisition",
            "catalog",
            "parse",
            "extraction_compute",
            "embedding",
            "resolution",
            "summary",
            "checkpoint",
        ];
        let mut orchestrator = PipelineOrchestrator::new(cache, clock(), fsm_registry());
        let mut executors = Vec::new();
        for id in ids {
            let stage = Arc::new(FakeStage::success(id, format!("{id}-out").as_bytes()));
            orchestrator = orchestrator.with_executor(id, stage.clone()).unwrap();
            executors.push(stage);
        }
        let chain = PipelineBuilder::new("full-fsm-chain")
            .step("acquisition", "acquisition", &[])
            .step("catalog", "catalog", &["acquisition"])
            .step("parse", "parse", &["catalog"])
            .step("extraction", "extraction_compute", &["parse"])
            .step("embedding", "embedding", &["extraction"])
            .step("resolution", "resolution", &["embedding"])
            .step("summary", "summary", &["resolution"])
            .step("checkpoint", "checkpoint", &["summary"])
            .build()
            .unwrap();
        let report = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(report.completed);
        assert_eq!(report.stages.len(), 8, "one record per step, no duplicates");
        for (index, record) in report.stages.iter().enumerate() {
            assert!(matches!(
                record.disposition,
                StageDisposition::Executed { .. }
            ));
            let admission = record
                .admission
                .as_ref()
                .expect("gated stage carries admission");
            assert!(
                admission.admitted,
                "stage {} must be admitted",
                record.stage_id
            );
            assert!(admission.error.is_none());
            assert!(
                !admission.evidence_ref.is_empty(),
                "admission must carry a durable evidence reference"
            );
            if index == 0 {
                assert_eq!(admission.evidence_ref, "source:sha256:pdf-v1");
            } else {
                assert!(
                    admission.evidence_ref.starts_with("test-key-"),
                    "downstream evidence references the prior stage cache key"
                );
            }
        }
    }

    #[tokio::test]
    async fn canonical_multimodal_chain_admits_gated_stages_and_skips_materialization() {
        // The canonical multimodal run (M400 S04): parse -> figure_context ->
        // figure_analysis -> table_fidelity -> equation_extraction ->
        // candidate_materialization. The four compute stages are gated and
        // admitted monotonically with durable evidence; candidate
        // materialization is a cross-cutting graph helper (candidate-only
        // truth boundary, D127) and is deliberately not gated.
        let cache = Arc::new(MemoryCache::default());
        let ids = [
            "parse",
            "figure_context",
            "figure_analysis",
            "table_fidelity",
            "equation_extraction",
            "candidate_materialization",
        ];
        let mut orchestrator = PipelineOrchestrator::new(cache, clock(), multimodal_registry());
        for id in ids {
            let stage = Arc::new(FakeStage::success(id, format!("{id}-out").as_bytes()));
            orchestrator = orchestrator.with_executor(id, stage).unwrap();
        }
        let chain = PipelineBuilder::new("canonical-multimodal")
            .step("parse", "parse", &[])
            .step("context", "figure_context", &["parse"])
            .step("analysis", "figure_analysis", &["context"])
            .step("fidelity", "table_fidelity", &["parse"])
            .step("equations", "equation_extraction", &["parse"])
            .step(
                "materialize",
                "candidate_materialization",
                &["analysis", "fidelity", "equations"],
            )
            .build()
            .unwrap();
        let report = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(report.completed);
        assert_eq!(report.stages.len(), 6, "one record per step");
        for (index, record) in report.stages.iter().enumerate() {
            assert!(matches!(
                record.disposition,
                StageDisposition::Executed { .. }
            ));
            if index == 5 {
                assert!(
                    record.admission.is_none(),
                    "candidate materialization is a cross-cutting helper, not gated"
                );
            } else {
                let admission = record
                    .admission
                    .as_ref()
                    .expect("gated multimodal stage carries admission");
                assert!(
                    admission.admitted,
                    "stage {} must be admitted",
                    record.stage_id
                );
                assert!(admission.error.is_none());
                assert!(
                    !admission.evidence_ref.is_empty(),
                    "admission carries durable evidence"
                );
            }
        }
    }

    #[tokio::test]
    async fn multimodal_regression_fails_closed_without_executing_stage() {
        // figure_analysis establishes Extracted; a later figure_context step
        // targeting Structured regresses the FSM and must fail closed with a
        // typed admission error before the executor runs.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let analysis = Arc::new(FakeStage::success("figure_analysis", b"analysis"));
        let context = Arc::new(FakeStage::success("figure_context", b"context"));
        let pipeline = PipelineBuilder::new("multimodal-regression")
            .step("parse", "parse", &[])
            .step("analysis", "figure_analysis", &["parse"])
            .step("context", "figure_context", &["parse"])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), multimodal_registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("figure_analysis", analysis.clone())
            .unwrap()
            .with_executor("figure_context", context.clone())
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        let blocked = &report.stages[2];
        assert!(matches!(
            blocked.disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        let error = blocked
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.from, ArticleContextState::Extracted);
        assert_eq!(error.to, ArticleContextState::Structured);
        assert_eq!(context.calls(), 0, "blocked stage must not execute");
    }

    #[tokio::test]
    async fn multimodal_mid_fsm_entry_fails_closed() {
        // A run cannot start at figure_analysis: its ACAP target (Extracted)
        // is above the Parsed entry layer, so the entry gate blocks before
        // the executor runs even with durable source evidence present.
        let cache = Arc::new(MemoryCache::default());
        let analysis = Arc::new(FakeStage::success("figure_analysis", b"analysis"));
        let pipeline = PipelineBuilder::new("multimodal-mid-entry")
            .step("analysis", "figure_analysis", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), multimodal_registry())
            .with_executor("figure_analysis", analysis.clone())
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        let error = report.stages[0]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.from, ArticleContextState::SourceRegistered);
        assert_eq!(error.to, ArticleContextState::Extracted);
        assert_eq!(analysis.calls(), 0, "blocked entry must not execute");
    }

    #[tokio::test]
    async fn canonical_multimodal_replay_hits_cache_without_duplicate_records() {
        // Identical replay of the canonical multimodal run: every stage is a
        // cache hit, executors never re-run, exactly one record per step.
        let cache = Arc::new(MemoryCache::default());
        let ids = [
            "parse",
            "figure_context",
            "figure_analysis",
            "table_fidelity",
            "equation_extraction",
            "candidate_materialization",
        ];
        let mut orchestrator = PipelineOrchestrator::new(cache, clock(), multimodal_registry());
        let mut executors = Vec::new();
        for id in ids {
            let stage = Arc::new(FakeStage::success(id, format!("{id}-out").as_bytes()));
            orchestrator = orchestrator.with_executor(id, stage.clone()).unwrap();
            executors.push(stage);
        }
        let chain = PipelineBuilder::new("canonical-multimodal-replay")
            .step("parse", "parse", &[])
            .step("context", "figure_context", &["parse"])
            .step("analysis", "figure_analysis", &["context"])
            .step("fidelity", "table_fidelity", &["parse"])
            .step("equations", "equation_extraction", &["parse"])
            .step(
                "materialize",
                "candidate_materialization",
                &["analysis", "fidelity", "equations"],
            )
            .build()
            .unwrap();
        let first = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(first.completed);
        let second = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(second.completed);
        assert_eq!(second.stages.len(), 6, "no duplicate stage records");
        for record in &second.stages {
            assert!(matches!(
                record.disposition,
                StageDisposition::CacheHit { .. }
            ));
            assert!(
                record.cache_key_sha256.is_some(),
                "replay cache hit carries its durable cache key"
            );
        }
        for executor in &executors {
            assert_eq!(
                executor.calls(),
                1,
                "replay must not re-execute stage executors"
            );
        }
    }

    #[tokio::test]
    async fn linear_article_spine_admits_materialization_and_replays_from_cache() {
        // Linear article spine (M402 S02): parse -> revision_materialization ->
        // extraction_compute -> extraction_materialization -> embedding ->
        // summary -> checkpoint. Every stage is gated from DATA; first run
        // admits monotonically with durable evidence; identical replay is
        // CacheHit, one record per step, executors not rerun.
        let cache = Arc::new(MemoryCache::default());
        let ids = [
            "parse",
            "revision_materialization",
            "extraction_compute",
            "extraction_materialization",
            "embedding",
            "summary",
            "checkpoint",
        ];
        let mut orchestrator = PipelineOrchestrator::new(cache, clock(), spine_registry());
        let mut executors = Vec::new();
        for id in ids {
            let stage = Arc::new(FakeStage::success(id, format!("{id}-out").as_bytes()));
            orchestrator = orchestrator.with_executor(id, stage.clone()).unwrap();
            executors.push(stage);
        }
        let chain = PipelineBuilder::new("linear-article-spine")
            .step("parse", "parse", &[])
            .step("revision", "revision_materialization", &["parse"])
            .step("extract", "extraction_compute", &["parse"])
            .step(
                "materialize",
                "extraction_materialization",
                &["revision", "extract"],
            )
            .step("embed", "embedding", &["parse"])
            .step("summary", "summary", &["revision"])
            .step("checkpoint", "checkpoint", &["summary"])
            .build()
            .unwrap();
        let first = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(first.completed, "spine must complete: {first:?}");
        assert_eq!(first.stages.len(), 7, "one record per spine step");
        for (index, record) in first.stages.iter().enumerate() {
            assert!(
                matches!(record.disposition, StageDisposition::Executed { .. }),
                "stage {} must execute on first run",
                record.stage_id
            );
            let admission = record
                .admission
                .as_ref()
                .expect("spine stage is gated from DATA");
            assert!(
                admission.admitted,
                "stage {} must be admitted",
                record.stage_id
            );
            assert!(admission.error.is_none());
            if index == 0 {
                assert_eq!(admission.evidence_ref, "source:sha256:pdf-v1");
            } else {
                assert!(
                    admission.evidence_ref.starts_with("test-key-"),
                    "downstream evidence references the prior stage cache key"
                );
            }
        }
        assert_eq!(first.stages[1].stage_id, "revision_materialization");
        assert_eq!(first.stages[3].stage_id, "extraction_materialization");
        let second = orchestrator.run(&chain, source("pdf-v1")).await.unwrap();
        assert!(second.completed);
        assert_eq!(second.stages.len(), 7, "replay keeps one record per step");
        for record in &second.stages {
            assert!(
                matches!(record.disposition, StageDisposition::CacheHit { .. }),
                "spine replay must be cache hit for {}",
                record.stage_id
            );
        }
        for executor in &executors {
            assert_eq!(
                executor.calls(),
                1,
                "spine replay must not re-execute stage executors"
            );
        }
    }

    #[tokio::test]
    async fn cached_output_cannot_bypass_mid_fsm_entry_gate() {
        // Replay attack via cache: a cached summary output exists for the exact
        // canonical key, but the run starts mid-FSM. The gate must block BEFORE
        // cache-hit acceptance — the record is Failed, not CacheHit, and the
        // executor never runs.
        let cache = Arc::new(MemoryCache::default());
        let summary = Arc::new(FakeStage::success("summary", b"cached-summary"));
        let key = cache_key(
            &source("pdf-v1"),
            "summary",
            BTreeMap::new(),
            &summary.contract(),
        );
        cache
            .store_success(
                &key,
                b"cached-summary",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let mid_fsm = PipelineBuilder::new("mid-fsm-entry")
            .step("summary", "summary", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&mid_fsm, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        assert_eq!(
            report.stages[0].cache_key_sha256, None,
            "a blocked admission must not accept the cached hit"
        );
        let error = report.stages[0]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(summary.calls(), 0);
    }

    #[tokio::test]
    async fn cache_hit_does_not_relax_gate_for_downstream_regression() {
        // Even when a cache hit is *available* for a regressed stage (the
        // replay-attack variant: parse with a summary dependency output), the
        // gate must reject the transition before the hit is accepted.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let orchestrator = PipelineOrchestrator::new(cache.clone(), clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap();
        // Prime the cache with a legal parse -> summary run.
        orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        // Seed a cache hit for the regressed parse step (parse keyed on a
        // summary dependency output).
        let regressed_key = cache_key(
            &source("pdf-v1"),
            "parse",
            BTreeMap::from([("summary".into(), payload_sha256(b"summary"))]),
            &parse.contract(),
        );
        cache
            .store_success(
                &regressed_key,
                b"parsed-from-summary",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let regressed = PipelineBuilder::new("cache-replay-attack")
            .step("parse", "parse", &[])
            .step("summary", "summary", &["parse"])
            .step("parse-again", "parse", &["summary"])
            .build()
            .unwrap();
        let report = orchestrator
            .run(&regressed, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert_eq!(report.stages.len(), 3);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::CacheHit { .. }
        ));
        assert!(matches!(
            report.stages[1].disposition,
            StageDisposition::CacheHit { .. }
        ));
        assert!(matches!(
            report.stages[2].disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        let error = report.stages[2]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.from, ArticleContextState::Summarized);
        assert_eq!(error.to, ArticleContextState::Parsed);
        assert_eq!(parse.calls(), 1, "only the priming run executed parse");
        assert_eq!(summary.calls(), 1);
    }

    #[tokio::test]
    async fn legacy_alias_cache_hit_cannot_bypass_entry_gate() {
        // The legacy alias fallback lookup also sits behind the gate: a cached
        // entry keyed on the raw step name cannot admit a mid-FSM entry.
        let cache = Arc::new(MemoryCache::default());
        let summary = Arc::new(FakeStage::success("summary", b"cached-summary"));
        let legacy_key = cache_key(
            &source("pdf-v1"),
            "legacy-summary-cache",
            BTreeMap::new(),
            &summary.contract(),
        );
        cache
            .store_success(
                &legacy_key,
                b"legacy-summary",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let mid_fsm = PipelineBuilder::new("legacy-mid-fsm")
            .step("legacy-summary-cache", "summary", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&mid_fsm, source("pdf-v1"))
            .await
            .unwrap();
        assert!(!report.completed);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        let error = report.stages[0]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:illegal_transition");
        assert_eq!(error.evidence_ref, "source:sha256:pdf-v1");
        assert_eq!(summary.calls(), 0);
    }

    #[tokio::test]
    async fn legal_legacy_alias_hit_keeps_gate_evidence() {
        // The legacy alias cache-hit path is not broken by the gate: a legal
        // entry (parse via its alias) admits, accepts the legacy hit, and the
        // record carries the admission decision with durable source evidence.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("grobid_parse", b"parsed"));
        let legacy_key = cache_key(
            &source("pdf-v1"),
            "legacy-parse-cache",
            BTreeMap::new(),
            &parse.contract(),
        );
        cache
            .store_success(
                &legacy_key,
                b"parsed-legacy",
                BTreeMap::new(),
                "2026-08-08T00:00:00Z",
            )
            .unwrap();
        let pipeline = PipelineBuilder::new("legacy-parse-hit")
            .step("legacy-parse-cache", "grobid_parse", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("grobid_parse", parse.clone())
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert!(report.completed);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::CacheHit { .. }
        ));
        assert_eq!(report.stages[0].stage_id, "parse");
        let admission = report.stages[0]
            .admission
            .as_ref()
            .expect("gated stage carries admission");
        assert!(admission.admitted);
        assert_eq!(admission.evidence_ref, "source:sha256:pdf-v1");
        assert_eq!(parse.calls(), 0);
    }

    #[tokio::test]
    async fn cache_hit_replay_keeps_one_record_per_step_with_admission_evidence() {
        // Replay via cache must not duplicate stage records and must keep the
        // admission decision (with durable evidence) on every gated record.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let orchestrator = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap();
        let first = orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        assert!(first.completed);
        assert_eq!(first.stages.len(), 2, "one record per step");
        let second = orchestrator
            .run(&pipeline(), source("pdf-v1"))
            .await
            .unwrap();
        assert!(second.completed);
        assert_eq!(
            second.stages.len(),
            2,
            "replay must not duplicate stage records"
        );
        assert!(
            second
                .stages
                .iter()
                .all(|record| matches!(record.disposition, StageDisposition::CacheHit { .. }))
        );
        for (index, record) in second.stages.iter().enumerate() {
            let admission = record
                .admission
                .as_ref()
                .expect("gated stage carries admission");
            assert!(admission.admitted, "cache-hit replay keeps the admission");
            assert!(admission.error.is_none());
            assert!(
                !admission.evidence_ref.is_empty(),
                "evidence ref must be durable"
            );
            if index == 0 {
                assert_eq!(admission.evidence_ref, "source:sha256:pdf-v1");
            } else {
                assert_eq!(
                    admission.evidence_ref, "test-key-parse",
                    "downstream evidence is the prior stage cache key"
                );
            }
        }
        assert_eq!(parse.calls(), 1);
        assert_eq!(summary.calls(), 1);
    }

    #[tokio::test]
    async fn run_without_source_evidence_fails_closed_with_missing_entry_state() {
        // A run whose source identity carries no durable checksum or fingerprint
        // must fail closed on the first gated stage (missing_entry_state): the
        // gate never default-admits.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let mut unbound = source("pdf-v1");
        unbound.pdf_sha256 = None;
        unbound.html_sha256 = None;
        unbound.acquisition_fingerprint = "   ".into();
        let one_stage = PipelineBuilder::new("no-entry-evidence")
            .step("parse", "parse", &[])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .run(&one_stage, unbound)
            .await
            .unwrap();
        assert!(!report.completed);
        assert!(matches!(
            report.stages[0].disposition,
            StageDisposition::Failed {
                retryable: false,
                ..
            }
        ));
        let error = report.stages[0]
            .admission
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .expect("typed error");
        assert_eq!(error.error_class(), "admission:missing_entry_state");
        assert_eq!(error.code, TransitionAdmissionCode::MissingEntryState);
        assert_eq!(parse.calls(), 0);
    }

    #[tokio::test]
    async fn skipped_stage_outside_fsm_does_not_advance_or_bypass_gate() {
        // Stages outside the ACAP FSM mapping surface as `skipped` (admission
        // None), still execute, and do not advance or corrupt the run's FSM
        // state: summary after readiness is still admitted from Parsed.
        let cache = Arc::new(MemoryCache::default());
        let parse = Arc::new(FakeStage::success("parse", b"parsed"));
        let readiness = Arc::new(FakeStage::success("readiness", b"ready"));
        let summary = Arc::new(FakeStage::success("summary", b"summary"));
        let pipeline = PipelineBuilder::new("with-skipped-stage")
            .step("parse", "parse", &[])
            .step("readiness", "readiness", &["parse"])
            .step("summary", "summary", &["readiness"])
            .build()
            .unwrap();
        let report = PipelineOrchestrator::new(cache, clock(), fsm_registry())
            .with_executor("parse", parse.clone())
            .unwrap()
            .with_executor("readiness", readiness.clone())
            .unwrap()
            .with_executor("summary", summary.clone())
            .unwrap()
            .run(&pipeline, source("pdf-v1"))
            .await
            .unwrap();
        assert!(report.completed);
        assert_eq!(report.stages.len(), 3);
        assert!(report.stages[1].admission.is_none(), "readiness is skipped");
        assert!(matches!(
            report.stages[1].disposition,
            StageDisposition::Executed { .. }
        ));
        let parse_admission = report.stages[0].admission.as_ref().unwrap();
        assert!(parse_admission.admitted);
        assert_eq!(parse_admission.evidence_ref, "source:sha256:pdf-v1");
        let summary_admission = report.stages[2].admission.as_ref().unwrap();
        assert!(summary_admission.admitted);
        // The skipped stage does not advance the FSM state (prior stays Parsed,
        // so summary is admitted), but `prior_evidence` is refreshed by every
        // executed stage: the evidence reference names the last durable record
        // (readiness's cache key), which is still an orchestrator-produced
        // durable record.
        assert_eq!(
            summary_admission.evidence_ref, "test-key-readiness",
            "evidence ref is the last durable record; FSM state is unaffected by skipped stages"
        );
    }

    #[test]
    fn canonical_multimodal_topology_validates_and_deduplicates_stages() {
        let registry = multimodal_registry();
        let composition = MultimodalComposition::canonical(&registry, "canonical-multimodal")
            .expect("canonical multimodal topology");
        assert_eq!(composition.topology.steps.len(), 7);
        assert_eq!(
            composition.canonical_stages,
            vec![
                "parse",
                "figure_context",
                "figure_analysis",
                "table_fidelity",
                "equation_extraction",
                "candidate_materialization",
            ]
        );
        // figure_detect is a parse-family alias: its step resolves to parse.
        let figures = composition
            .topology
            .steps
            .iter()
            .find(|step| step.name == "figures")
            .expect("figures step");
        assert_eq!(registry.resolve(&figures.stage).unwrap().as_str(), "parse");
        // materialize converges on analysis + fidelity + equations.
        let materialize = composition
            .topology
            .steps
            .iter()
            .find(|step| step.name == "materialize")
            .expect("materialize step");
        assert_eq!(
            materialize.depends_on,
            vec!["analysis", "fidelity", "equations"]
        );
    }

    #[test]
    fn canonical_multimodal_topology_rejects_unknown_stage() {
        // A registry missing one required canonical stage must fail closed:
        // the composition can never silently drop a multimodal stage.
        let ledger = da_domain::PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage_contract("parse", &["grobid_parse", "figure_detect"]),
                stage_contract("figure_context", &[]),
                stage_contract("figure_analysis", &[]),
                stage_contract("table_fidelity", &[]),
                stage_contract("equation_extraction", &[]),
            ],
            import_eligible: false,
        };
        let registry = Arc::new(ledger.registry().unwrap());
        let error = MultimodalComposition::canonical(&registry, "bad").unwrap_err();
        assert!(matches!(error, PipelineOrchestratorError::StageIdentity(_)));
    }

    #[test]
    fn multimodal_fingerprint_summary_binds_exact_contract_identity() {
        let source = source("pdf-v1");
        let contracts = BTreeMap::from([
            (
                "figure_detect".to_string(),
                StageContract {
                    stage_id: "figure_detect".into(),
                    implementation_fingerprint: "detector-impl".into(),
                    configuration_fingerprint: "render_dpi=150".into(),
                    prompt_fingerprint: None,
                    model_fingerprint: Some("pdffigures2-0.0.12-SNAPSHOT".into()),
                    output_schema_fingerprint: "DetectedVisualArtifact[]@serde-v1".into(),
                },
            ),
            (
                "figure_analysis".to_string(),
                StageContract {
                    stage_id: "figure_analysis".into(),
                    implementation_fingerprint: "FigureAnalysisUseCase@v1".into(),
                    configuration_fingerprint: "schema_registry=figure-schemas.v1".into(),
                    prompt_fingerprint: Some("prompt-sha".into()),
                    model_fingerprint: Some(
                        "route|anthropic_messages|vision|fallback=false".into(),
                    ),
                    output_schema_fingerprint: "FigureAnalysis@serde-v1".into(),
                },
            ),
        ]);
        let summary = MultimodalFingerprintSummary::bind(
            &source,
            "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula",
            &contracts,
        );
        assert_eq!(summary.source, "article=2507.19457;acq=catalog-v1");
        assert_eq!(summary.detector, "pdffigures2-0.0.12-SNAPSHOT");
        assert!(summary.schema_registry.contains("figure-schemas.v1"));
        assert!(summary.model_route.contains("anthropic_messages"));
        assert_eq!(
            summary.output_schemas["figure_analysis"],
            "FigureAnalysis@serde-v1"
        );
        assert!(!summary.import_eligible, "D127 stays false");
        // Unbound stages degrade to explicit markers, never silent empty.
        let empty =
            MultimodalFingerprintSummary::bind(&source, "profile=structural", &BTreeMap::new());
        assert_eq!(empty.detector, "detector:unbound");
        assert_eq!(empty.model_route, "model_route:unbound");
        assert!(empty.output_schemas.is_empty());
    }
}
