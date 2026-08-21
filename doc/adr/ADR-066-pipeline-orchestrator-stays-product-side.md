# ADR-066: PipelineOrchestrator stays product-side (engine migration deferred-by-design)

- Status: accepted
- Date: 2026-08-21
- Deciders: agent (recon-driven), user (standing GSD mandate)
- Related: ADR-049 (product pipeline DSL), ADR-050 (universal kg-* crates),
  ADR-064 §5 (migration listed as possible follow-up), D280

## Context

ADR-064 §5 left one follow-up open: migrate `PipelineOrchestrator`
(da-application, ~2655 lines) onto the now-honest `kg-pipeline`
`ExecutionEngine` primitives. This ADR closes that question with evidence.

Recon (GitNexus + direct read of `pipeline.rs::run`):

- The **overlap** between orchestrator and engine is only the sequential
  step-walk with dependency checks (~30 lines).
- The orchestrator implements **durable cross-process retry**: a
  `StageExecutionError::Retryable` persists `RetryableFailure{retry_after}`
  into the pipeline cache and returns a failed report; the next invocation
  observes `RetrySuppressed` until `retry_after` passes. There is **no
  in-process retry loop** — by design, so retries survive process death and
  are auditable in the cache manifest.
- The engine implements **in-process retry**: attempts loop with backoff via
  the injected `Sleeper` (ADR-064). These are complementary, not competing
  semantics: one is crash-safe durable scheduling, the other is bounded
  in-run resilience.
- The orchestrator additionally owns four concerns that must not enter a
  universal crate (ADR-050 zero-`da-*` invariant): ACAP runtime admission
  gate (PIR-014), cache contract identity (`StageCacheKey`, canonical/legacy
  alias fallback), `StageCacheMode::GraphMaterialize` rerun-over-cache
  (MEM678), and the typed `StageRunRecord` report surface consumed by
  gold-graph waves and ACAP audits.

## Decision

**No migration.** The two execution stacks stay as they are:

- `kg-pipeline::ExecutionEngine` — universal, honest execution semantics
  (retry registry, failure policies, observable FSM, control plane) for
  consumers that orchestrate arbitrary stages in-process.
- `da-application::PipelineOrchestrator` — product composition with durable
  retry, cache identity, admission gating, and audit surfaces.

`build_extraction_pipeline()` (da-application) continues to declare topology
through `kg_pipeline::PipelineBuilder`, so the declarative shape is shared
even though execution is not.

## Consequences

- ADR-064 §5 follow-up is closed permanently (revisit only if a second
  product adopts the engine, or the orchestrator grows a second in-process
  retry consumer that would justify extracting a shared walker).
- The engine keeps its independent value: contract-tested semantics, and the
  only FSM-observable executor available to non-da consumers.
- No code changes; this ADR is the deliverable (with D280).
