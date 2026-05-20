---
id: T02
parent: S03
milestone: M012-a7v8fw
key_files:
  - .gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json
key_decisions:
  - Production import remains globally blocked regardless of individual DSPy/MiniMax probe readiness.
  - Next safe options are explicit, separate probes or the chunk-span provenance packet, not general activation.
duration: 
verification_result: passed
completed_at: 2026-05-20T10:23:50.801Z
blocker_discovered: false
---

# T02: Wrote integration guard blocking production activation and naming the next safe probes.

**Wrote integration guard blocking production activation and naming the next safe probes.**

## What Happened

Wrote the integration guard. It confirms DSPy production runtime is not allowed, DSPy optimizers are not allowed, MiniMax orchestration/source-of-truth behavior is not allowed, production import is not allowed, and LadybugDB writes remain false. It names three possible next safe options: DSPy optional dev dependency no-LM probe, MiniMax explicit synthetic auth smoke test, and chunk-span provenance/candidate-locator packet.

## Verification

integration-guard.json exists and confirms dspy_production_runtime_allowed=false, minimax_orchestrator_allowed=false, and production_import_allowed=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `write integration-guard.json and assert activation boundaries` | 0 | ✅ pass — integration-guard-ok | 6100ms |

## Deviations

None.

## Known Issues

None for S03. S04 must turn the three next-safe options into a final recommendation and requirement update.

## Files Created/Modified

- `.gsd/milestones/M012-a7v8fw/slices/S03/run-evidence/integration-guard.json`
