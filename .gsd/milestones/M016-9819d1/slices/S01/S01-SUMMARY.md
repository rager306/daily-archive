---
id: S01
parent: M016-9819d1
milestone: M016-9819d1
provides:
  - 9router MiniMax endpoint order
  - 9router parsing semantics
requires:
  []
affects:
  - S02
key_files:
  - .gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json
key_decisions:
  - 9router implementation is the source of truth for M016 probing.
  - M015 missed the global `api.minimax.io` coding_plan fallback endpoint.
  - MiniMax usage success requires provider base_resp success and model_remains quota rows, not HTTP 200 alone.
patterns_established:
  - Use known working vendor implementation before guessing endpoints.
  - Persist algorithm summaries as machine-readable evidence for live probes.
observability_surfaces:
  - algorithm report
  - summary JSON
drill_down_paths:
  - .gsd/milestones/M016-9819d1/slices/S01/tasks/T01-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T12:38:28.200Z
blocker_discovered: false
---

# S01: 9router MiniMax usage algorithm

**S01 extracted 9router’s exact MiniMax limits algorithm and corrected the missed M015 fallback.**

## What Happened

S01 used the GitNexus-indexed 9router source to extract MiniMax usage behavior. 9router's provider `minimax` checks `www.minimax.io/v1/token_plan/remains` then `api.minimax.io/v1/api/openplatform/coding_plan/remains`; provider `minimax-cn` checks `www.minimaxi.com/v1/api/openplatform/coding_plan/remains` then `api.minimaxi.com/v1/api/openplatform/coding_plan/remains`. It uses GET with Authorization Bearer, parses `base_resp`, requires status 0 and model_remains, and interprets token_plan counts as used but coding_plan counts as remaining.

## Verification

9router-minimax-usage-summary-ok passed.

## Requirements Advanced

- R044 — S01 documents the 9router algorithm required by R044.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

No live call was made in S01; it is source extraction only.

## Follow-ups

S02 must rerun live limit probe using exactly the 9router endpoint order and success criteria.

## Files Created/Modified

- `.gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md` — Source-backed 9router algorithm report.
- `.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json` — Machine-readable 9router summary.
