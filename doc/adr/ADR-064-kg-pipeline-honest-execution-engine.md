# ADR-064: kg-pipeline honest execution engine (FSM + policies)

- Status: accepted
- Date: 2026-08-21
- Deciders: user (standing GSD mandate), agent (implementation)
- Related: ADR-049 (Pipeline DSL — product orchestrator), ADR-050 (universal
  kg-* crates), ADR-051 (testing contours; determinism infrastructure),
  M436-q9cuc1

## Context

`kg-pipeline` (ADR-050 universal crate) declared an execution engine whose
doc-comment promised "retry, failure policy, and status tracking", and
exported `FailurePolicy` / `FailureAction` / `RetryPolicy` types. Inspection
(M436 recon) showed the contract was fictitious:

- `ExecutionEngine::execute` was a linear abort-on-first-error loop;
  `Step.retry` (`RetryPolicyRef`) was never read.
- `FailurePolicy::{Abort, Continue, RetryThenAbort}` and `FailureAction`
  branches could never execute — no code consumed them.
- 4 of 6 `PipelineStatus` variants (`Pending`, `Running`, `Paused`,
  `Stopped`) were unreachable; the "FSM" was cosmetic.

The product path never noticed: `da-cli`/`da-application` execute through
`PipelineOrchestrator` (ADR-049, da-application) and use kg-pipeline only for
topology building/validation. A fake contract in a universal crate is still a
defect of the class this project hunts (STUB-AUDIT).

## Decision

1. **The engine owns execution semantics** in kg-pipeline, and they are now
   real and tested:
   - Per-step retry from a `HashMap<String, RetryPolicy>` registry keyed by
     the step's `retry` reference; absent reference ⇒ single attempt.
   - Pipeline-level `FailurePolicy`: `Abort` (stop after the step's
     retries), `Continue` (mark the step skipped; keep independent steps;
     dependents of a skipped step are skipped transitively — the dependency
     constraint holds), `RetryThenAbort` (registry retry, then abort).
   - `ExecutionResult` gained `attempts_per_step` and `skipped_steps`.
2. **Determinism infrastructure (ADR-051 contour 1)**: backoff goes through
   an injected `Sleeper` port (`TokioSleeper` in production, counting
   `NoopSleeper` in tests). No real sleeps in tests.
3. **The FSM is observable**: `with_observer` delivers `StepEvent`s
   (`Started/Retried/Failed/Skipped/Completed` with step name + attempt
   number) and `StatusChanged` transitions
   (`Pending→Running→{Completed,Failed,Paused,Stopped}`). All six statuses
   are reachable and asserted in tests.
4. **Cooperative control**: `EngineControl` (`pause/resume/stop`) checked
   between steps; stop wins over pause. Pause returns a `Paused` result;
   the caller re-invokes to continue.
5. **Division of responsibility stands**: kg-pipeline remains the universal
   engine-primitives crate (no cache, no ledger, no stage registry, zero
   da-* dependencies); `PipelineOrchestrator` (da-application) remains the
   product composition. Migrating the orchestrator onto the engine is a
   possible follow-up, deliberately out of scope here.
6. `execute()`/`execute_with_policies()` keep their historical single-attempt
   abort behavior — existing consumers see no semantic change.

## Consequences

- Red-first/mutation-checked tests cover: retry success on attempt N,
  attempts recording, sleeper invocation count, all three failure policies,
  transitive skip, pause/stop, and every FSM transition.
- `FailureAction` remains exported but unconsumed; it describes per-failure
  decisions an embedding policy engine could make. Left as-is (documented)
  rather than removed to avoid a breaking export removal; revisit if no
  consumer appears.
- No hardcode added: policies are data passed by the caller; defaults live
  in `Default` impls of the policy types.
