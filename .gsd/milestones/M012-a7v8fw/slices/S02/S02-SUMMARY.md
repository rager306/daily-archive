---
id: S02
parent: M012-a7v8fw
milestone: M012-a7v8fw
provides:
  - MiniMax compatibility guard
  - MiniMax preconditions and blockers
requires:
  []
affects:
  - S03
key_files:
  - .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json
  - .gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md
key_decisions:
  - MiniMax is conditionally compatible for future optional bounded helper probe only.
  - MiniMax must not be orchestrator, source of truth, direct PDF parser, or production writer.
  - Next safe step is explicitly approved synthetic auth/header smoke test.
patterns_established:
  - MiniMax live calls require explicit approval even if a key is present.
  - MiniMax helper output must be locally validated and remain review_required.
observability_surfaces:
  - research report
  - no-call payload probe
  - compatibility guard
  - summary
drill_down_paths:
  - .gsd/milestones/M012-a7v8fw/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M012-a7v8fw/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M012-a7v8fw/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T10:21:02.762Z
blocker_discovered: false
---

# S02: MiniMax compatibility spike

**S02 completed MiniMax compatibility spike: optional helper probe possible later, orchestration/import/direct PDF blocked.**

## What Happened

S02 completed MiniMax compatibility research from official API docs and a no-call synthetic payload dry run. The research identifies Anthropic-compatible text API as primary, OpenAI-compatible as fallback, and MiniMax-M2.7 as initial model candidate. The no-call probe built a redacted synthetic request and recorded key presence without logging values. The guard permits only a future optional helper probe and blocks MiniMax as orchestrator/source of truth, direct PDF/raw paper ingestion, positive KG import, production LadybugDB writes, and unbounded repair/scaling.

## Verification

Fresh S01/S02 check passed: MiniMax optional_helper=true, live_call_attempted=false, orchestrator_allowed=false, production_import_attempted=false.

## Requirements Advanced

- R039 — S02 provides MiniMax side of the parallel compatibility evidence.
- R040 — S02 follows R040 by researching/probing infrastructure before activation.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

A MINIMAX_API_KEY is present, but no live call was attempted because external API use/cost requires explicit approval. The probe is intentionally a no-call dry run.

## Known Limitations

Live MiniMax callability, auth/header behavior, and structured JSON reliability are not yet proven. Direct PDF/document ingestion is not supported by the consulted text API surfaces.

## Follow-ups

S03 should include MiniMax as conditionally go for optional helper probe only, with no live call yet and no-go for orchestration/source-of-truth/direct PDF ingestion.

## Files Created/Modified

- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-research-report.md` — MiniMax research report.
- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-probe.json` — MiniMax no-call probe artifact.
- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json` — MiniMax compatibility guard.
- `.gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md` — MiniMax compatibility summary.
