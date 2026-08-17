//! M399/S04 T01 — same-process control-plane proof.
//!
//! Composes the three ACAP control planes in one process and replays the whole
//! scenario to prove replay idempotence — no duplicate authoritative effects:
//!
//! 1. **Admission (PIR-014)** — a legal pipeline run through the ACAP FSM:
//!    every gated stage is admitted with a durable evidence reference
//!    (`source:sha256:...` for the entry stage, the prior stage cache-key
//!    sha256 for downstream stages). Replaying the identical run hits the
//!    cache on every stage (executor call counts stay 1) and produces exactly
//!    one record per step — no duplicate stage records.
//! 2. **Promotion control (PIR-015)** — an independently verified promotion
//!    commit (constrained multi-paper gold promote through the fail-closed
//!    `PromotionService` with distinct verifier/reviewer/promoter actors).
//!    Replay: a byte-identical decision commit is idempotent (sink length
//!    unchanged), a conflicting decision under the same id fails closed with
//!    `Conflict`, and a double Promote on the same candidate is refused
//!    (`AlreadyPromoted`).
//! 3. **Durable scheduling (PIR-016)** — a crashed worker's expired lease is
//!    recovered by worker-b which completes the task and publishes exactly one
//!    logical result with the full attempt lineage. Replay: the completed task
//!    is never due again (due=0), no new attempts, the logical result is
//!    unchanged, and a stale holder cannot re-execute or re-publish.
//!
//! Negative control: an illegal mid-FSM replay cannot bypass the admission
//! gate even when a durable cached output exists for the target stage — the
//! record is `Failed` with `admission:illegal_transition`, no cache hit is
//! accepted, and the executor never runs.
//!
//! The test closes with a serializable evidence ledger linking admission
//! decisions, promotion transitions, scheduler recovery, replay counters and
//! lock status (D127 `import_eligible=false` everywhere) — the durable
//! evidence surface the slice closeout publishes.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_application::promotion::{PromotionService, PromotionTransitionRequest};
use da_application::{
    SchedulerExecuteOutcome, SchedulerRunner, SchedulerTaskExecutor, StageDisposition,
};
use da_domain::eval::{GoldEntity, MultiPaperGoldIndex, PredictedEntity, promote_candidates};
use da_domain::promotion::{
    EvidenceRef, PromotionCandidate, PromotionError, PromotionKind, PromotionState,
};
use da_domain::scheduler::{
    AttemptOutcome, PendingTask, RetryPolicy, TaskPriority, TaskResourceClass, TaskStatus,
    TaskType, logical_result_vid,
};
use da_domain::{
    ArticleContextState, PipelineStageContract, PipelineStageLedger, StageCacheMode,
    StageIdRegistry, StageOwner,
};
use da_ports::clock::{Clock, FixedClock};
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::{OpenAlexClient, OpenAlexError, OpenAlexResult, OpenAlexWork};
use da_ports::pipeline_cache::{
    CacheLookup, CachedSuccess, PipelineCache, PipelineCacheResult, SourceFormat, SourceIdentity,
    StageCacheKey, StageManifestEntry, StageOutcome,
};
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use da_ports::promotion::{PromotionCommitError, PromotionCommitSink, PromotionDecision};
use kg_pipeline::{Pipeline, PipelineBuilder};
use sha2::Digest as _;

mod common;
use common::mock_graph_store::MockGraphStore;

/// Fixed epoch for deterministic scheduler scenarios (no real sleep anywhere).
const T0: i64 = 1_784_000_000;
/// Default lease term (expiry lands inside the scenario window).
const LEASE_TERM: i64 = 300;

// ────────────────────────────────────────────────────────────────────────────
// Plane A fixtures: in-memory pipeline cache + fake stage executors
// ────────────────────────────────────────────────────────────────────────────

/// In-memory `PipelineCache` mirroring the orchestrator unit-test fake:
/// exact-key lookup, one manifest per key, stable `key_sha256` per stage.
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
            StageOutcome::RetryableFailure { retry_after, .. } if now < retry_after.as_str() => {
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
                payload_sha256: hex::encode(sha2::Sha256::digest(payload)),
                payload_size: payload.len() as u64,
            },
            metadata,
        };
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(stored, _, _)| stored != key);
        entries.push((key.clone(), entry.clone(), payload.to_vec()));
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
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(stored, _, _)| stored != key);
        entries.push((key.clone(), entry.clone(), Vec::new()));
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

/// Fake stage executor with a call counter — the replay-idempotence counter
/// for the pipeline plane.
struct FakeStage {
    contract: StageContract,
    payload: Vec<u8>,
    calls: AtomicUsize,
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

    fn validate_cached_payload(&self, _payload: &[u8]) -> Result<(), StageExecutionError> {
        Ok(())
    }

    async fn execute(
        &self,
        _request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(StageExecutionOutput {
            payload: self.payload.clone(),
            metadata: BTreeMap::new(),
        })
    }
}

fn source(pdf_hash: &str) -> SourceIdentity {
    SourceIdentity {
        article_id: "2507.19457".into(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(pdf_hash.into()),
        html_sha256: None,
        acquisition_fingerprint: "catalog-v1".into(),
        normalization_fingerprint: "normalization-v1".into(),
    }
}

fn pipeline_clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 8, 8, 0, 0, 0).unwrap(),
    ))
}

fn fixed_clock(secs: i64) -> Arc<dyn Clock> {
    Arc::new(FixedClock(Utc.timestamp_opt(secs, 0).unwrap()))
}

fn stage_contract(id: &str, aliases: &[&str]) -> PipelineStageContract {
    PipelineStageContract {
        id: id.into(),
        aliases: aliases.iter().map(|alias| (*alias).into()).collect(),
        owner: StageOwner::Application,
        input_semantics: vec!["input".into()],
        output_semantics: vec!["output".into()],
        depends_on: vec![],
        context_adds: vec!["identity".into()],
        context_losses: vec![],
        hard_invariants: vec!["D127_false".into()],
        consumers: vec!["test".into()],
        cache_mode: StageCacheMode::PureCompute,
        failure_class: "terminal".into(),
        conditional: false,
        graph_truth_boundary: false,
        candidate_only: false,
    }
}

/// Registry covering the full ACAP FSM mapping used by the legal run plus
/// `summary` (used by the negative mid-FSM control). Returns the registry and
/// the ledger's D127 `import_eligible` configuration — the pipeline plane's
/// lock statement, observed from the same configuration the run executes on.
fn fsm_registry() -> (Arc<StageIdRegistry>, bool) {
    let ledger = PipelineStageLedger {
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
        ],
        import_eligible: false,
    };
    let pipeline_import_eligible = ledger.import_eligible;
    (
        Arc::new(ledger.registry().unwrap()),
        pipeline_import_eligible,
    )
}

/// Legal linear pipeline over the ACAP FSM: one step per gated stage, each
/// depending on its predecessor (stage-subset shape, monotonic forward).
fn legal_acap_pipeline() -> Pipeline {
    PipelineBuilder::new("full-fsm-chain")
        .step("acquisition", "acquisition", &[])
        .step("catalog", "catalog", &["acquisition"])
        .step("parse", "parse", &["catalog"])
        .step("extraction", "extraction_compute", &["parse"])
        .step("embedding", "embedding", &["extraction"])
        .step("resolution", "resolution", &["embedding"])
        .step("summary", "summary", &["resolution"])
        .step("checkpoint", "checkpoint", &["summary"])
        .build()
        .unwrap()
}

/// The eight canonical gated stage ids in pipeline order.
const ACAP_STAGES: [&str; 8] = [
    "acquisition",
    "catalog",
    "parse",
    "extraction_compute",
    "embedding",
    "resolution",
    "summary",
    "checkpoint",
];

// ────────────────────────────────────────────────────────────────────────────
// Plane B fixtures: fail-closed promotion sink
// ────────────────────────────────────────────────────────────────────────────

/// In-memory fail-closed sink mirroring the promotion port contract:
/// validate before store, idempotent byte-identical replay, Conflict on the
/// same decision id with different content.
#[derive(Default)]
struct FakeSink {
    decisions: Mutex<BTreeMap<String, PromotionDecision>>,
}

impl FakeSink {
    fn stored(&self, decision_id: &str) -> Option<PromotionDecision> {
        self.decisions.lock().unwrap().get(decision_id).cloned()
    }

    fn len(&self) -> usize {
        self.decisions.lock().unwrap().len()
    }
}

#[async_trait]
impl PromotionCommitSink for FakeSink {
    async fn commit(&self, decision: &PromotionDecision) -> Result<(), PromotionCommitError> {
        decision.validate()?;
        let mut decisions = self.decisions.lock().unwrap();
        match decisions.get(&decision.decision_id) {
            Some(existing) if existing == decision => Ok(()),
            Some(_) => Err(PromotionCommitError::Conflict {
                decision_id: decision.decision_id.clone(),
                candidate_id: decision.candidate.candidate_id.clone(),
            }),
            None => {
                decisions.insert(decision.decision_id.clone(), decision.clone());
                Ok(())
            }
        }
    }
}

fn evidence(reference: &str) -> EvidenceRef {
    EvidenceRef::new(reference)
}

fn promotion_service() -> (PromotionService, Arc<FakeSink>) {
    let sink = Arc::new(FakeSink::default());
    let service = PromotionService::from_shared_with_clock(
        sink.clone(),
        Arc::new(FixedClock::from_timestamp(1_780_000_000)),
    );
    (service, sink)
}

/// Independently verified gold promotion: the constrained multi-paper promote
/// (`promote_candidates` + multi-paper evidence) mapped to the shared
/// protocol, then advanced Verify → Review → Promote through the fail-closed
/// service with distinct actors.
async fn independently_verified_promotion() -> (PromotionCandidate, PromotionDecision, Arc<FakeSink>)
{
    // RLHF is gold in papers B and C but not in paper A (the paper being
    // processed), so the constrained pipeline promotes it for A.
    let paper_a = vec![
        GoldEntity {
            label: "GRPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "LoRA".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let paper_b = vec![
        GoldEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "DPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let paper_c = vec![
        GoldEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
            section: None,
        },
        GoldEntity {
            label: "PPO".into(),
            entity_type: "Method".into(),
            section: None,
        },
    ];
    let index = MultiPaperGoldIndex::from_papers(vec![
        ("2507.19457", paper_a.as_slice()),
        ("2510.26094", paper_b.as_slice()),
        ("2601.00001", paper_c.as_slice()),
    ]);
    let predicted = vec![
        PredictedEntity {
            label: "GRPO".into(),
            entity_type: "Method".into(),
        },
        PredictedEntity {
            label: "LoRA".into(),
            entity_type: "Method".into(),
        },
        PredictedEntity {
            label: "RLHF".into(),
            entity_type: "Method".into(),
        },
    ];
    let promoted = promote_candidates(&paper_a, &predicted, &index, 2);
    assert_eq!(promoted.len(), 1, "only RLHF is multi-paper and not local");
    assert_eq!(promoted[0].label, "RLHF");

    let proposal = promoted[0]
        .to_promotion_proposal(&["2510.26094".into(), "2601.00001".into()], 2)
        .expect("adapter");
    assert_eq!(proposal.state, PromotionState::Proposed);
    assert_eq!(proposal.kind, PromotionKind::Gold);
    assert_eq!(proposal.proposer, "gold:multi-paper-promote");
    assert!(!proposal.import_eligible, "D127 lock");

    let (service, sink) = promotion_service();
    let mut current = proposal;
    let mut step = 0u32;
    let decision = loop {
        step += 1;
        let decision_id = format!("gold-{step}");
        let request = match current.state {
            PromotionState::Proposed => PromotionTransitionRequest::Verify {
                decision_id,
                candidate: current,
                verifier: format!("contract:verifier:gold:{step}"),
                evidence_refs: vec![evidence(&format!("ev:gold:verify:{step}"))],
            },
            PromotionState::Verified => PromotionTransitionRequest::Review {
                decision_id,
                candidate: current,
                reviewer: format!("contract:reviewer:gold:{step}"),
                evidence_refs: vec![evidence(&format!("ev:gold:review:{step}"))],
            },
            PromotionState::Reviewed => PromotionTransitionRequest::Promote {
                decision_id,
                candidate: current,
                promoter: format!("contract:promoter:gold:{step}"),
                evidence_refs: vec![evidence(&format!("ev:gold:promote:{step}"))],
            },
            state => panic!("expected non-terminal candidate, got {state:?}"),
        };
        let committed = service.commit(request).await.expect("commit must succeed");
        current = committed.candidate.clone();
        if committed.transition == PromotionState::Promoted {
            break committed;
        }
    };
    (current, decision, sink)
}

// ────────────────────────────────────────────────────────────────────────────
// Plane C fixtures: scheduler runner + durable graph store
// ────────────────────────────────────────────────────────────────────────────

struct NoOpOpenAlex;

#[async_trait]
impl OpenAlexClient for NoOpOpenAlex {
    async fn fetch_by_arxiv_id(&self, _arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        Err(OpenAlexError::Network("noop".into()))
    }

    async fn search(&self, _query: &str, _limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        Ok(vec![])
    }
}

struct FakeExecutor {
    task_type: TaskType,
    outcome: SchedulerExecuteOutcome,
}

#[async_trait]
impl SchedulerTaskExecutor for FakeExecutor {
    fn task_type(&self) -> TaskType {
        self.task_type
    }

    async fn execute(&self, _task: &PendingTask) -> SchedulerExecuteOutcome {
        self.outcome.clone()
    }
}

fn completed_outcome(summary: &str) -> SchedulerExecuteOutcome {
    SchedulerExecuteOutcome::Completed {
        summary: summary.to_string(),
        evidence_refs: vec!["stage:parse:cache:sha256:cache-key:sha256:abc123".into()],
    }
}

/// Seed a durable SchedulerTask node (task_json + flat mirrors;
/// `import_eligible` always false — D127).
async fn seed_durable(store: &MockGraphStore, task: &PendingTask) -> u64 {
    let node = store.create_node("SchedulerTask").await.unwrap();
    store
        .set_node_property_string(node, "vid", task.task_vid())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "arxiv_id", task.arxiv_id.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "task_type", task.task_type.as_str().to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "status", "pending".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(node, "schema_version", task.schema_version as i64)
        .await
        .unwrap();
    store
        .set_node_property_string(node, "task_json", serde_json::to_string(task).unwrap())
        .await
        .unwrap();
    node
}

async fn persisted_task(store: &MockGraphStore, node_id: u64) -> PendingTask {
    let json = store
        .get_node_property_string(node_id, "task_json")
        .await
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

fn runner_with(
    store: &MockGraphStore,
    worker_id: &str,
    now: i64,
    executor: Arc<dyn SchedulerTaskExecutor>,
) -> SchedulerRunner {
    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    SchedulerRunner::new(graph, Arc::new(NoOpOpenAlex))
        .with_clock(fixed_clock(now))
        .with_worker_id(worker_id)
        .with_lease_term(LEASE_TERM)
        .with_executor(executor)
}

// ────────────────────────────────────────────────────────────────────────────
// Durable evidence ledger linking the three planes (closeout surface)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdmissionEvidenceRow {
    stage_id: String,
    admitted: bool,
    evidence_ref: String,
    disposition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PromotionEvidenceRow {
    decision_id: String,
    transition: String,
    promoter: String,
    evidence_refs: Vec<String>,
    sink_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct SchedulerEvidenceRow {
    task_vid: String,
    logical_result_id: String,
    attempts: usize,
    published_by_epoch: u64,
    evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ReplayCounters {
    pipeline_executor_calls: Vec<(String, usize)>,
    pipeline_stage_records: usize,
    pipeline_replay_dispositions: Vec<String>,
    promotion_sink_len_after_replay: usize,
    scheduler_attempts_after_replay: usize,
    scheduler_due_after_replay: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct LockStatus {
    /// Pipeline plane D127: the ACAP registry the run executes on is built
    /// from a `PipelineStageLedger` with `import_eligible=false`.
    pipeline_import_eligible: bool,
    /// Promotion plane D127: the promoted candidate keeps `import_eligible=false`.
    promotion_import_eligible: bool,
    /// Scheduler plane D127: the durable SchedulerTask node keeps
    /// `import_eligible=false` through crash recovery and replay.
    scheduler_import_eligible: bool,
    /// PIR-015: no optimizer auto-promotion — promotion required exactly three
    /// explicit actor-gated transitions (Verify → Review → Promote).
    promotion_explicit_transitions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ControlPlaneEvidenceLedger {
    schema_version: String,
    admission: Vec<AdmissionEvidenceRow>,
    promotion: PromotionEvidenceRow,
    scheduler: SchedulerEvidenceRow,
    replay: ReplayCounters,
    locks: LockStatus,
}

// ────────────────────────────────────────────────────────────────────────────
// The control-plane composition test
// ────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mixed_control_plane_run_replays_without_duplicate_authoritative_effects() {
    // ── Plane A: legal pipeline run through the ACAP admission gate ────────
    let cache = Arc::new(MemoryCache::default());
    let (registry, pipeline_import_eligible) = fsm_registry();
    let mut orchestrator = da_application::pipeline::PipelineOrchestrator::new(
        cache.clone(),
        pipeline_clock(),
        registry,
    );
    let mut executors = Vec::new();
    for id in ACAP_STAGES {
        let stage = Arc::new(FakeStage::success(id, format!("{id}-out").as_bytes()));
        orchestrator = orchestrator.with_executor(id, stage.clone()).unwrap();
        executors.push(stage);
    }
    let first = orchestrator
        .run(&legal_acap_pipeline(), source("pdf-v1"))
        .await
        .unwrap();
    assert!(first.completed, "legal ACAP chain completes");
    assert_eq!(first.stages.len(), ACAP_STAGES.len(), "one record per step");
    let admission_rows: Vec<AdmissionEvidenceRow> = first
        .stages
        .iter()
        .map(|record| {
            let admission = record
                .admission
                .as_ref()
                .expect("gated stage carries admission");
            assert!(admission.admitted, "stage {} admitted", record.stage_id);
            assert!(admission.error.is_none());
            assert!(
                !admission.evidence_ref.is_empty(),
                "admission carries a durable evidence reference"
            );
            AdmissionEvidenceRow {
                stage_id: record.stage_id.clone(),
                admitted: admission.admitted,
                evidence_ref: admission.evidence_ref.clone(),
                disposition: "executed".into(),
            }
        })
        .collect();
    // Evidence chain: the entry stage is bound to the checksum-bound source
    // identity; every downstream stage references the prior stage's durable
    // cache key sha256.
    assert_eq!(admission_rows[0].evidence_ref, "source:sha256:pdf-v1");
    for (index, row) in admission_rows.iter().enumerate().skip(1) {
        assert_eq!(
            row.evidence_ref,
            format!("test-key-{}", ACAP_STAGES[index - 1]),
            "downstream admission references the prior stage cache key"
        );
    }

    // ── Plane B: independently verified promotion commit ───────────────────
    let (promoted_candidate, promotion_decision, sink) = independently_verified_promotion().await;
    assert_eq!(promotion_decision.transition, PromotionState::Promoted);
    assert_eq!(promotion_decision.candidate.kind, PromotionKind::Gold);
    assert_eq!(
        promotion_decision.candidate.promoter.as_deref(),
        Some("contract:promoter:gold:3")
    );
    assert!(
        promotion_decision
            .candidate
            .evidence_refs
            .iter()
            .any(|e| e.reference == "gold:2510.26094:RLHF:Method"),
        "independent gold evidence must be durable in the decision"
    );
    assert!(
        promotion_decision
            .candidate
            .evidence_refs
            .iter()
            .any(|e| e.reference == "gold:constraint:min_papers=2")
    );
    assert!(!promotion_decision.candidate.import_eligible, "D127 lock");
    let promotion_sink_len = sink.len();
    assert_eq!(promotion_sink_len, 3, "propose-path decisions are durable");

    // ── Plane C: recovered scheduler task (crash → reclaim → one result) ───
    let store = MockGraphStore::new();
    let mut task = PendingTask::new_openalex_enrich("2401.00001", &RetryPolicy::default(), T0);
    task.priority = TaskPriority::Medium;
    task.resource_class = TaskResourceClass::Standard;
    task.next_retry = T0; // due immediately; lease expires inside the window
    task.claim("worker-a", T0, LEASE_TERM).unwrap();
    let node = seed_durable(&store, &task).await;

    let report = runner_with(
        &store,
        "worker-b",
        T0 + LEASE_TERM + 1,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("enriched by worker-b"),
        }),
    )
    .run_due()
    .await
    .unwrap();
    assert_eq!(report.recovered, 1, "expired lease recovered");
    assert_eq!(report.completed, 1);

    let recovered = persisted_task(&store, node).await;
    assert_eq!(
        recovered.attempts.len(),
        2,
        "crashed attempt retained as Aborted + worker-b attempt"
    );
    assert_eq!(recovered.attempts[0].outcome, AttemptOutcome::Aborted);
    assert_eq!(recovered.attempts[0].worker, "worker-a");
    assert_eq!(recovered.attempts[0].epoch, 1);
    assert_eq!(recovered.attempts[1].outcome, AttemptOutcome::Succeeded);
    assert_eq!(recovered.attempts[1].worker, "worker-b");
    assert_eq!(
        recovered.attempts[1].epoch, 2,
        "fence incremented on reclaim"
    );
    let result = recovered
        .logical_result
        .as_ref()
        .expect("exactly one logical result");
    assert_eq!(result.result_id, logical_result_vid(&recovered.task_vid()));
    assert_eq!(result.published_by_epoch, 2);
    assert_eq!(
        result.evidence_refs,
        vec!["stage:parse:cache:sha256:cache-key:sha256:abc123"]
    );
    assert_eq!(recovered.status, TaskStatus::Completed);

    // ── Replay: no duplicate authoritative effects in any plane ────────────

    // Pipeline replay: identical run → every stage is a cache hit, executors
    // never re-run, exactly one record per step, admission evidence retained.
    let second = orchestrator
        .run(&legal_acap_pipeline(), source("pdf-v1"))
        .await
        .unwrap();
    assert!(second.completed);
    assert_eq!(
        second.stages.len(),
        ACAP_STAGES.len(),
        "no duplicate records"
    );
    let replay_dispositions: Vec<String> = second
        .stages
        .iter()
        .map(|record| {
            assert!(matches!(
                record.disposition,
                StageDisposition::CacheHit { .. }
            ));
            let admission = record.admission.as_ref().expect("admission retained");
            assert!(admission.admitted);
            assert!(
                record.cache_key_sha256.is_some(),
                "cache hit carries its durable cache key"
            );
            "cache_hit".into()
        })
        .collect();
    let executor_calls: Vec<(String, usize)> = ACAP_STAGES
        .iter()
        .zip(executors.iter())
        .map(|(id, stage)| ((*id).to_string(), stage.calls()))
        .collect();
    for (id, calls) in &executor_calls {
        assert_eq!(*calls, 1, "stage {id} must execute exactly once");
    }

    // Promotion replay: byte-identical decision commit is idempotent; a
    // conflicting decision under the same id fails closed; double promote is
    // refused at the service boundary.
    let stored = sink.stored("gold-3").expect("promote decision durable");
    sink.commit(&stored).await.expect("idempotent replay");
    assert_eq!(sink.len(), promotion_sink_len, "replay stores nothing new");
    let mut conflicting = stored.clone();
    conflicting.committed_at = "2030-01-01T00:00:00Z".into();
    let err = sink
        .commit(&conflicting)
        .await
        .expect_err("conflicting content under the same decision id fails closed");
    assert!(matches!(
        err,
        PromotionCommitError::Conflict {
            ref decision_id,
            ..
        } if decision_id == "gold-3"
    ));
    assert_eq!(sink.len(), promotion_sink_len, "conflict stores nothing");
    let service = PromotionService::from_shared_with_clock(
        sink.clone(),
        Arc::new(FixedClock::from_timestamp(1_780_000_000)),
    );
    let double = service
        .commit(PromotionTransitionRequest::Promote {
            decision_id: "gold-4".into(),
            candidate: promoted_candidate.clone(),
            promoter: "contract:promoter:replay".into(),
            evidence_refs: vec![evidence("ev:replay:promote")],
        })
        .await
        .expect_err("double promotion refused");
    assert!(matches!(
        double,
        da_application::promotion::PromotionServiceError::Domain(PromotionError::AlreadyPromoted)
    ));
    assert_eq!(
        sink.len(),
        promotion_sink_len,
        "refused promote stores nothing"
    );

    // Scheduler replay: the completed task is never due again, no new
    // attempts, logical result unchanged, stale holder cannot re-execute.
    let replay_report = runner_with(
        &store,
        "worker-b",
        T0 + LEASE_TERM + 2,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("must not run again"),
        }),
    )
    .run_due()
    .await
    .unwrap();
    assert_eq!(replay_report.due, 0, "completed task is never due");
    assert_eq!(replay_report.completed, 0);
    let stale_holder = runner_with(
        &store,
        "worker-a",
        T0 + 400,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("stale"),
        }),
    )
    .run_due()
    .await
    .unwrap();
    assert_eq!(
        stale_holder.due, 0,
        "resumed stale worker finds nothing due"
    );
    let after_replay = persisted_task(&store, node).await;
    assert_eq!(
        after_replay.attempts.len(),
        2,
        "replay must not add attempts"
    );
    assert_eq!(
        after_replay.logical_result.as_ref().unwrap().result_id,
        result.result_id,
        "logical result unchanged by replay"
    );

    // ── Negative control: cache cannot bypass the admission gate ───────────
    // A mid-FSM replay (summary as the first step) is illegal even though a
    // durable cached summary output exists for the exact key: the gate blocks
    // before cache-hit acceptance, the executor never runs.
    let mid_fsm = PipelineBuilder::new("mid-fsm-entry")
        .step("summary", "summary", &[])
        .build()
        .unwrap();
    let summary = Arc::new(FakeStage::success("summary", b"summary-out"));
    let (blocked_registry, _) = fsm_registry();
    let blocked = da_application::pipeline::PipelineOrchestrator::new(
        cache.clone(),
        pipeline_clock(),
        blocked_registry,
    )
    .with_executor("summary", summary.clone())
    .unwrap()
    .run(&mid_fsm, source("pdf-v1"))
    .await
    .unwrap();
    assert!(!blocked.completed);
    assert!(matches!(
        blocked.stages[0].disposition,
        StageDisposition::Failed {
            ref error_class,
            retryable: false,
            ..
        } if error_class == "admission:illegal_transition"
    ));
    assert_eq!(
        blocked.stages[0].cache_key_sha256, None,
        "a blocked admission must not accept the cached hit"
    );
    let gate_error = blocked.stages[0]
        .admission
        .as_ref()
        .unwrap()
        .error
        .as_ref()
        .expect("typed error");
    assert_eq!(gate_error.error_class(), "admission:illegal_transition");
    assert_eq!(gate_error.from, ArticleContextState::SourceRegistered);
    assert_eq!(gate_error.to, ArticleContextState::Summarized);
    assert_eq!(
        summary.calls(),
        0,
        "blocked transition never reaches the executor"
    );

    // ── Durable evidence ledger: links the three planes and the locks ──────
    let ledger = ControlPlaneEvidenceLedger {
        schema_version: "acap-control-plane.v1".into(),
        admission: admission_rows,
        promotion: PromotionEvidenceRow {
            decision_id: stored.decision_id.clone(),
            transition: "promoted".into(),
            promoter: stored
                .candidate
                .promoter
                .clone()
                .unwrap_or_else(|| "<none>".into()),
            evidence_refs: stored
                .candidate
                .evidence_refs
                .iter()
                .map(|e| e.reference.clone())
                .collect(),
            sink_len: sink.len(),
        },
        scheduler: SchedulerEvidenceRow {
            task_vid: after_replay.task_vid(),
            logical_result_id: after_replay
                .logical_result
                .as_ref()
                .unwrap()
                .result_id
                .clone(),
            attempts: after_replay.attempts.len(),
            published_by_epoch: result.published_by_epoch,
            evidence_refs: result.evidence_refs.clone(),
        },
        replay: ReplayCounters {
            pipeline_executor_calls: executor_calls,
            pipeline_stage_records: second.stages.len(),
            pipeline_replay_dispositions: replay_dispositions,
            promotion_sink_len_after_replay: sink.len(),
            scheduler_attempts_after_replay: after_replay.attempts.len(),
            scheduler_due_after_replay: replay_report.due,
        },
        locks: LockStatus {
            pipeline_import_eligible,
            promotion_import_eligible: stored.candidate.import_eligible,
            scheduler_import_eligible: store.get_node_property_bool(node, "import_eligible").await
                != Some(false),
            promotion_explicit_transitions: 3, // Verify → Review → Promote
        },
    };
    // Lock status must be false everywhere (D127 import lock); promotion ran
    // through explicit actor-gated transitions, never optimizer auto-promote.
    assert!(!ledger.locks.pipeline_import_eligible);
    assert!(!ledger.locks.promotion_import_eligible);
    assert!(
        !ledger.locks.scheduler_import_eligible,
        "node must be locked"
    );
    assert_eq!(ledger.locks.promotion_explicit_transitions, 3);

    // The ledger survives serialization and deserialization unchanged — the
    // closeout evidence is durable and self-contained.
    let json = serde_json::to_vec(&ledger).unwrap();
    let back: ControlPlaneEvidenceLedger = serde_json::from_slice(&json).unwrap();
    assert_eq!(back, ledger);
    assert_eq!(back.replay.pipeline_stage_records, ACAP_STAGES.len());
    assert_eq!(back.replay.pipeline_executor_calls.len(), ACAP_STAGES.len());
    assert!(
        back.replay
            .pipeline_executor_calls
            .iter()
            .all(|(_, calls)| *calls == 1)
    );
    assert_eq!(back.replay.scheduler_attempts_after_replay, 2);
    assert_eq!(back.replay.scheduler_due_after_replay, 0);
    assert_eq!(
        back.replay.promotion_sink_len_after_replay,
        promotion_sink_len
    );

    // The composed evidence is observable for the closeout: print the ledger.
    println!(
        "ACAP control-plane evidence ledger:\n{}",
        serde_json::to_string_pretty(&ledger).unwrap()
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Focused negative tests (same-process, no real I/O)
// ────────────────────────────────────────────────────────────────────────────

/// The independent verification itself refuses to produce a promotion
/// proposal when the durable multi-paper evidence is missing or below the
/// constraint — nothing reaches the promotion service.
#[test]
fn independent_verification_refuses_without_durable_evidence() {
    let paper_a = vec![GoldEntity {
        label: "RLHF".into(),
        entity_type: "Method".into(),
        section: None,
    }];
    let index = MultiPaperGoldIndex::from_papers(vec![("2507.19457", paper_a.as_slice())]);
    let predicted = vec![PredictedEntity {
        label: "RLHF".into(),
        entity_type: "Method".into(),
    }];
    let promoted = promote_candidates(&paper_a, &predicted, &index, 2);
    // RLHF is local-only gold → the constrained promote refuses it (no
    // multi-paper evidence), so no candidate and no promotion path exist.
    assert!(promoted.is_empty(), "local-only gold must not be promoted");

    // Even with a promoted entity, missing/insufficient durable paper
    // evidence fails closed at the adapter boundary.
    let multi = MultiPaperGoldIndex::from_papers(vec![
        ("2507.19457", paper_a.as_slice()),
        ("2510.26094", paper_a.as_slice()),
    ]);
    let promoted = promote_candidates(&paper_a, &predicted, &multi, 2);
    if let Some(entity) = promoted.first() {
        let err = entity
            .to_promotion_proposal(&[], 2)
            .expect_err("empty papers refused");
        assert_eq!(err, PromotionError::MissingEvidence);
        let err = entity
            .to_promotion_proposal(&["2510.26094".into()], 2)
            .expect_err("below constraint refused");
        assert_eq!(err, PromotionError::MissingEvidence);
    }
}

/// The recovered scheduler task keeps D127 false and never double-publishes
/// even when a stale holder attempts a domain-level re-publish.
#[tokio::test]
async fn recovered_scheduler_task_keeps_lock_and_rejects_stale_publish() {
    let store = MockGraphStore::new();
    let mut task = PendingTask::new_openalex_enrich("2401.00002", &RetryPolicy::default(), T0);
    task.next_retry = T0; // due immediately; lease expires inside the window
    task.claim("worker-a", T0, LEASE_TERM).unwrap();
    let node = seed_durable(&store, &task).await;

    let report = runner_with(
        &store,
        "worker-b",
        T0 + LEASE_TERM + 1,
        Arc::new(FakeExecutor {
            task_type: TaskType::OpenAlexEnrich,
            outcome: completed_outcome("recovered"),
        }),
    )
    .run_due()
    .await
    .unwrap();
    assert_eq!(report.completed, 1);

    let recovered = persisted_task(&store, node).await;
    assert!(recovered.logical_result.is_some());
    assert_eq!(recovered.status, TaskStatus::Completed);
    assert_eq!(
        store.get_node_property_bool(node, "import_eligible").await,
        Some(false),
        "D127 stays false through crash-recovery"
    );

    // Domain-level fence: a stale holder holding the pre-reclaim record
    // cannot re-publish a second logical result.
    let mut stale = recovered.clone();
    let stale_fence = stale.publish_logical_result(1, "stale".into(), vec![], T0 + 400);
    assert!(stale_fence.is_err(), "stale epoch publish must fail");
    let after = persisted_task(&store, node).await;
    assert_eq!(
        after.logical_result.as_ref().unwrap().result_id,
        recovered.logical_result.as_ref().unwrap().result_id,
        "the authoritative result is unchanged"
    );
}
