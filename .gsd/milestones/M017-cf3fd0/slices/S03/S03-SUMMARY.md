---
id: S03
parent: M017-cf3fd0
milestone: M017-cf3fd0
provides:
  - MiniMax forced-tool request builder
  - MiniMax local tool response validator
  - Structured helper safety guard
requires:
  []
affects:
  - S04
key_files:
  - src/arxiv_archive/minimax_structured.py
  - tests/test_minimax_structured.py
key_decisions:
  - Forced tool calls are required for structured helper output.
  - Prompt-only JSON is rejected as proof.
  - Raw corpus payloads are blocked at helper request construction.
patterns_established:
  - Prompt-only model JSON is not evidence; structured helper outputs must be returned through tool_use and locally schema-validated.
  - Raw corpus payload class should be rejected before an external model request is constructed.
observability_surfaces:
  - minimax-structured-helper-guard.json
  - sanitized validation result dicts
drill_down_paths:
  - .gsd/milestones/M017-cf3fd0/slices/S03/tasks/T01-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T06:28:32.511Z
blocker_discovered: false
---

# S03: MiniMax structured helper boundary

**S03 added a tested MiniMax structured helper boundary with forced tool-call schema validation and fail-closed safety behavior.**

## What Happened

S03 implemented the MiniMax structured helper boundary as pure, testable code. It builds Anthropic-compatible forced-tool request payloads with `tool_choice`, rejects raw-corpus payload classes and invalid temperature 0, validates returned `tool_use.input` against a local schema subset, rejects prompt-only JSON, and emits sanitized diagnostics. The helper remains non-authoritative and does not enable KG import or LadybugDB writes.

## Verification

Fresh verification passed: 3 targeted tests, ruff, LSP diagnostics, and guard assertions.

## Requirements Advanced

- R045 — Implements the S03 structured-helper portion of R045 with tests and guard evidence.

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

The local schema validator intentionally covers a bounded JSON Schema subset; future advanced schema features require new tests.

## Follow-ups

S04 should run final safety review across S02/S03 and close R045 if all guards remain green.

## Files Created/Modified

- `src/arxiv_archive/minimax_structured.py` — Dev-only MiniMax structured request and schema-validation helpers.
- `tests/test_minimax_structured.py` — Tests for forced tool-call shape, local schema validation, prompt-only JSON rejection, raw corpus blocking, and temperature fail-closed behavior.
- `.gsd/milestones/M017-cf3fd0/slices/S03/run-evidence/minimax-structured-helper-guard.json` — S03 safety/verification guard.
