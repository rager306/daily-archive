---
id: S02
parent: M017-cf3fd0
milestone: M017-cf3fd0
provides:
  - MiniMax usage/remains request builder
  - MiniMax usage/remains sanitized parser
  - MiniMax key alias resolver
requires:
  []
affects:
  - S03
  - S04
key_files:
  - src/arxiv_archive/minimax_usage.py
  - tests/test_minimax_usage.py
key_decisions:
  - MiniMax usage helper is pure and testable without live calls.
  - Only sanitized quota percentages/metadata are exposed by helper summaries.
  - Key alias diagnostics never log secret values.
patterns_established:
  - External API helpers should expose sanitized diagnostic summaries rather than raw provider payloads.
  - SDK env aliases are separate transport names for one MiniMax key value, not separate credentials by default.
observability_surfaces:
  - minimax-usage-helper-guard.json
  - sanitized usage summary dicts
drill_down_paths:
  - .gsd/milestones/M017-cf3fd0/slices/S02/tasks/T01-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T06:12:39.514Z
blocker_discovered: false
---

# S02: MiniMax usage limit helper

**S02 added a tested dev-only MiniMax usage/remains helper with safe key aliasing and 9router parsing semantics.**

## What Happened

S02 implemented the MiniMax usage limit helper. It follows the M016 9router endpoint sequence, sends the canonical MiniMax key as Bearer for usage/remains requests, requires provider `base_resp.status_code == 0`, parses `model_remains`, supports snake_case/camelCase fields, and applies endpoint-family-specific count semantics. Tests ensure exact raw quota values and secrets do not appear in sanitized outputs.

## Verification

Fresh verification passed: 5 targeted tests, ruff, LSP diagnostics, and guard assertions.

## Requirements Advanced

- R045 — Implements the S02 limit-helper portion of R045 with tests and guard evidence.

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

No production integration or CLI command was added. Exact quota values are intentionally unavailable from sanitized summaries.

## Follow-ups

S03 should implement structured-output helper boundary using Anthropic-compatible forced tool-call schema validation. S04 should run final safety review across S02/S03.

## Files Created/Modified

- `src/arxiv_archive/minimax_usage.py` — Dev-only MiniMax usage/remains helper primitives.
- `tests/test_minimax_usage.py` — Sanitized fixture tests for endpoint order, key aliases, provider success, and count semantics.
- `.gsd/milestones/M017-cf3fd0/slices/S02/run-evidence/minimax-usage-helper-guard.json` — S02 safety/verification guard.
