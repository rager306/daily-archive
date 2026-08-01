# ADR-049: Pipeline DSL and Execution Engine

**Status:** Proposed
**Date:** 2026-07-24
**Deciders:** collaborative
**Related:** ADR-040 (Samyama store), ADR-043 (process plane use cases), ADR-048 (decisions), [Semantica pipeline module](https://github.com/semantica-agi/semantica/blob/main/semantica/pipeline/)

## Context

daily-archive currently wires pipeline stages imperatively in CLI handlers
and integration tests:

```rust
// crates/da-cli/src/main.rs — current ad-hoc pattern
let ingest = IngestUseCase::new(...);
let result = ingest.ingest_pdf(&pdf, &id).await?;

let extract = ExtractionUseCase::new(...);
let result = extract.extract_from_parsed(&parsed).await?;

let enrich = EnrichUseCase::new(...);
let result = enrich.enrich_by_arxiv_id(&id).await?;
```

This pattern has visible costs:

- No status tracking between stages (a 3-minute batch ingest gives one
  `Ok` at the end, no progress signal).
- No retry / failure recovery — one failed stage aborts the whole CLI
  invocation.
- No parallelism across independent stages (enrich Topic + Author can run
  concurrently but is sequential).
- No pipeline-level validation pre-flight — invalid inputs surface at the
  stage that touches them, not before the pipeline starts.
- Pipeline shape is implicit in CLI handler order; changing it requires
  editing Rust code, not config.
- No structured per-run provenance — "which pipeline template produced this
  graph state?" is unanswerable.

Semantica's `pipeline` module ships `PipelineBuilder`, `ExecutionEngine`,
`FailureHandler`, `ParallelismManager`, `ResourceScheduler`,
`PipelineValidator`, `PipelineTemplateManager` — a complete pipeline DSL
that we can adapt (not adopt wholesale — different language, different
runtime).

## Decision

Introduce a Rust-native Pipeline DSL in
`crates/da-application/src/pipeline.rs` that composes existing use cases
into declarative, observable, retryable pipelines. Inspired by Semantica's
shape but idiomatic Rust (async traits, enums, builders).

### Pipeline data model

```rust
pub struct Pipeline {
    pub id: String,
    pub steps: Vec<PipelineStep>,
    pub failure_policy: FailurePolicy,
    pub parallelism: Parallelism,
}

pub struct PipelineStep {
    pub name: String,
    pub stage: PipelineStage,
    pub depends_on: Vec<String>,
    pub retry: RetryPolicy,
}

pub enum PipelineStage {
    Ingest { pdf_path: PathBuf, paper_id: String },
    Extract { paper_id: String },
    Enrich { paper_id: String },
    Cluster,
    DetectConflicts { since: Option<DateTime> },
    ValidateGraph { label: Option<String> },
    Custom(Box<dyn Stage>),
}

pub enum FailurePolicy {
    Abort,
    Continue,
    RetryThenAbort { retries: u32, backoff: Duration },
}

pub enum Parallelism {
    Sequential,
    Concurrent { max_workers: usize },
}
```

### Builder DSL

```rust
let pipeline = PipelineBuilder::new("single-paper-ingest")
    .step("ingest", PipelineStage::Ingest { pdf_path, paper_id })
        .retry(RetryPolicy::new().max_attempts(3))
    .step("extract", PipelineStage::Extract { paper_id })
        .depends_on("ingest")
    .step("enrich", PipelineStage::Enrich { paper_id })
        .depends_on("ingest")
    .step("validate", PipelineStage::ValidateGraph { label: None })
        .depends_on("extract")
        .depends_on("enrich")
    .failure_policy(FailurePolicy::RetryThenAbort { retries: 3, backoff: 5s })
    .parallelism(Parallelism::Sequential)
    .build();

let result = ExecutionEngine::new().execute(pipeline).await?;
```

### ExecutionEngine

```rust
pub struct ExecutionEngine {
    graph_store: Box<dyn DirectGraphStore>,
    failure_handler: FailureHandler,
    progress_tracker: ProgressTracker,
}

impl ExecutionEngine {
    pub async fn execute(&self, pipeline: Pipeline) -> Result<ExecutionResult>;
    pub fn status(&self, pipeline_id: &str) -> PipelineStatus;
    pub fn progress(&self, pipeline_id: &str) -> PipelineProgress;
}
```

`PipelineStatus` enum: `Pending`, `Running`, `Paused`, `Completed`, `Failed`,
`Stopped` (matches Semantica's enum, plus Rust-idiomatic naming).

### PipelineValidator (pre-flight)

Before execution, `PipelineValidator::validate(&pipeline)` checks:

- Dependency graph is acyclic (topological sort succeeds).
- Every `depends_on` name exists as a step.
- Required input fields are present (e.g. Ingest requires non-empty
  paper_id).
- Stage-specific pre-conditions (e.g. Extract requires a Paper node with
  matching arxiv_id — soft check via `find_node_by_string_property`).

Validator returns a list of `ValidationIssue` (severity Error/Warning).
Errors block execution; Warnings are logged.

### FailureHandler

```rust
pub struct FailureHandler {
    policy: FailurePolicy,
    decision_recorder: DecisionRecorder, // ADR-048
}

impl FailureHandler {
    pub async fn on_stage_failure(&self, stage: &PipelineStep, error: &StageError)
        -> Result<FailureAction>;
}

pub enum FailureAction {
    Retry { after: Duration },
    Skip,
    Abort,
    Escalate(Decision), // records a Decision of category `escalation`
}
```

Ties into ADR-048 — a failed stage with exhausted retries emits a Decision
so the failure is auditable.

### Pipeline templates

Pre-built templates in `data/pipeline_templates/`:

| Template                   | Steps                                                      |
|---------------------------|------------------------------------------------------------|
| `single_paper.yaml`        | ingest → extract → enrich → validate                       |
| `batch_ingest.yaml`        | for each pdf: ingest; then batch extract + enrich          |
| `reprocess.yaml`           | extract → detect_conflicts → validate                      |
| `healing_pass.yaml`        | detect_conflicts → resolve → validate                      |
| `full_audit.yaml`          | validate_graph → detect_conflicts → report                 |

Templates are YAML, not Rust — operators can compose pipelines without
code changes. Loaded by `PipelineTemplateManager`.

### Progress and observability

`ProgressTracker` exposes:
- per-stage status (pending/running/completed/failed).
- per-stage duration.
- cumulative node/edge counts written.
- structured log events (`tracing` spans).

CLI gains `da pipeline status <id>` and `da pipeline progress <id>` for
live monitoring.

### Pipeline provenance

Every ExecutionResult carries:
- `pipeline_id`
- `template_name` (if from template)
- `started_at` / `finished_at`
- `params` (JSON)
- `decision_ids` (Decisions emitted during the run — ADR-048)

Stored as a `PipelineRun` node (future; Phase 2). Phase 1 keeps provenance
in ExecutionResult + structured logs.

### Cross-ADR alignment

- **ADR-040**: ExecutionEngine holds a `Box<dyn DirectGraphStore>` —
  Samyama stays the sole store.
- **ADR-043**: each process-plane use case becomes a PipelineStage.
- **ADR-047**: ConflictDetectionUseCase is a Stage; runs after extract.
- **ADR-048**: FailureHandler escalations emit Decisions; DecisionRecorder
  is a Stage-dependency for stages that produce decisions.
- **ADR-049 §Validator**: PipelineValidator composes with ADR-045
  SchemaValidator — pipeline pre-flight checks include schema consistency.

### Phase 1 scope

- `Pipeline`, `PipelineStep`, `PipelineStage` data types.
- `PipelineBuilder`.
- `ExecutionEngine` (sequential only — Concurrent deferred).
- `PipelineValidator` (structural checks).
- `FailureHandler` with `RetryThenAbort` policy.
- `single_paper` template (replaces current ad-hoc CLI flow).
- CLI: `da pipeline run --template single_paper --param paper_id=...`.

### Phase 2 (out of scope, future)

- `Parallelism::Concurrent` with `ParallelismManager`.
- `ResourceScheduler` (CPU/memory quotas).
- `PipelineRun` provenance node.
- `paused` state and `da pipeline resume <id>`.
- Custom stage plugins via trait object registration.

## Alternatives considered

1. **Keep ad-hoc CLI orchestration.** Rejected — no observability, no
   retry, no template composition.
2. **Adopt an existing Rust pipeline crate (e.g. nu_protocol, tokio
   pipeline patterns).** Rejected — those target stream processing or
   generic task graphs, not domain-specific KG pipelines. The DSL needs
   to know about PipelineStage variants.
3. **Embed a scripting language (rhai, deno_core) for pipeline definitions.**
   Defer — adds dependency surface for marginal benefit at current scale.

## Consequences

- New module `crates/da-application/src/pipeline.rs`.
- CLI gains `pipeline` subcommand tree.
- Existing ad-hoc CLI handlers become thin wrappers around
  `ExecutionEngine::execute(template)`.
- Pipeline definitions become data (YAML), not code — operators can
  compose new flows.
- Every pipeline run emits structured progress events; one `Ok` at the
  end is no longer the only signal.
- Failure modes become auditable Decisions (ADR-048), not log lines.

## Open questions

- Should `PipelineStage::Custom` be public API or internal escape hatch?
  Tentative: public, but documented as "use sparingly".
- How to represent fan-out (one paper → N enrichment sub-stages)?
  Tentative: Phase 1 sequential loop; Phase 2 Concurrent with stage-level
  fan-out.
- Template versioning — semver on templates? Defer to ADR-044 mechanism.
