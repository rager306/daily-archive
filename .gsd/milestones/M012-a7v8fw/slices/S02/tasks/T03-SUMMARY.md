---
id: T03
parent: S02
milestone: M012-a7v8fw
key_files:
  - .gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json
  - .gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md
key_decisions:
  - MiniMax verdict: conditional go for optional bounded helper probe; no-go for orchestrator/source-of-truth/direct PDF ingestion.
  - Next safe MiniMax step: explicitly approved synthetic auth/header smoke test.
duration: 
verification_result: passed
completed_at: 2026-05-20T10:19:12.823Z
blocker_discovered: false
---

# T03: MiniMax guard written: optional helper probe possible later, orchestration/import/direct PDF blocked.

**MiniMax guard written: optional helper probe possible later, orchestration/import/direct PDF blocked.**

## What Happened

Synthesized MiniMax official-doc research and no-call payload probe into a compatibility guard. The guard allows MiniMax only as a future optional bounded helper probe. It records Anthropic-compatible text API as the primary surface, MiniMax-M2.7 as recommended model, key presence without secret value logging, no live call attempted, and blocks MiniMax as orchestrator/source of truth, direct PDF/raw paper ingestion, positive import, production writes, and unbounded repair/scaling.

## Verification

minimax-compatibility-guard.json exists and confirms production_import_attempted=false and minimax_orchestrator_allowed=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `write minimax-compatibility-guard.json and summary` | 0 | ✅ pass — live_call_attempted=false; key present; orchestrator_allowed=false | 4600ms |
| 2 | `guard verification assertions` | 0 | ✅ pass — minimax-compatibility-guard-ok | 4600ms |

## Deviations

None.

## Known Issues

MiniMax live callability is not proven. A key is present, but live external calls remain deferred until explicit approval.

## Files Created/Modified

- `.gsd/milestones/M012-a7v8fw/slices/S02/run-evidence/minimax-compatibility-guard.json`
- `.gsd/milestones/M012-a7v8fw/slices/S02/minimax-compatibility-summary.md`
