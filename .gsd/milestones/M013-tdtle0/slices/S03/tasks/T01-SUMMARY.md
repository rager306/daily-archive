---
id: T01
parent: S03
milestone: M013-tdtle0
key_files:
  - .gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json
key_decisions:
  - Use OpenAI-compatible MiniMax endpoint for the smoke test.
  - Treat HTTP 200 as callability evidence only, not structured-output reliability or production readiness.
duration: 
verification_result: passed
completed_at: 2026-05-20T10:45:43.556Z
blocker_discovered: false
---

# T01: MiniMax synthetic smoke test succeeded with HTTP 200 using synthetic-only input.

**MiniMax synthetic smoke test succeeded with HTTP 200 using synthetic-only input.**

## What Happened

Ran a MiniMax synthetic smoke test using the OpenAI-compatible chat completions endpoint and model MiniMax-M2.7. The request contained only synthetic compatibility text and no raw paper/chunk content. The call succeeded with HTTP 200. The artifact records payload and response hashes/status but not any secret value. No production import, LadybugDB write, trusted fact creation, or orchestration behavior occurred.

## Verification

minimax-smoke-test.json exists and records live_call_attempted=true, http_status=200, secrets_logged=false, minimax_orchestrator_allowed=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `POST https://api.minimax.io/v1/chat/completions with synthetic prompt and MINIMAX_API_KEY from env` | 0 | ✅ pass — http_status=200; live_call_exit=success; no raw project text | 7200ms |

## Deviations

A live synthetic call was run because the user explicitly requested progressing with MiniMax and a key was present. The call used only synthetic text and did not include raw project/paper/chunk content.

## Known Issues

Structured JSON reliability is only weakly indicated by one synthetic response; future helper probes still need local schema validation over expected output shapes.

## Files Created/Modified

- `.gsd/milestones/M013-tdtle0/slices/S03/run-evidence/minimax-smoke-test.json`
