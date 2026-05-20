---
id: T02
parent: S02
milestone: M014-65dlgp
key_files:
  - .gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json
key_decisions:
  - Allow next helper integration probe only with local JSON schema validation and bounded retry controls.
  - Do not allow unattended batch use or source-of-truth use despite successful real calls.
duration: 
verification_result: passed
completed_at: 2026-05-20T11:19:38.226Z
blocker_discovered: false
---

# T02: Wrote MiniMax real-test guard: real helper probe can continue only with schema validation and retry controls.

**Wrote MiniMax real-test guard: real helper probe can continue only with schema validation and retry controls.**

## What Happened

Synthesized the four real MiniMax calls into a real-test guard. The guard records live_call_count=4, successful_http_count=4, json_parse_success_count=2, redacted_helper_success_count=1, edge_behavior_recorded_count=1, strict JSON passed, helper retry passed, initial helper truncated, and length edge failed closed. It recommends only a next helper integration probe with local schema validation and bounded retry, while blocking unattended batch use, source-of-truth use, production import, LadybugDB writes, and orchestration.

## Verification

minimax-real-test-guard-ok confirmed live_call_count>=3, raw_response_persisted=false, raw_model_content_persisted=false, minimax_orchestrator_allowed=false, production_import_allowed=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `write minimax-real-test-guard.json and assert invariants` | 0 | ✅ pass — minimax-real-test-guard-ok | 6100ms |

## Deviations

None.

## Known Issues

Schema reliability requires controls: one helper attempt truncated; only retry with larger budget produced a parseable helper result.

## Files Created/Modified

- `.gsd/milestones/M014-65dlgp/slices/S02/run-evidence/minimax-real-test-guard.json`
