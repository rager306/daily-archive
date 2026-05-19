---
id: S01
parent: M007-opaont
milestone: M007-opaont
provides:
  - validation batch contract
  - state schema helpers
  - source contradiction diagnostics
  - safe CLI namespace for S02 implementation
requires:
  - slice: M006/S04
    provides: Reviewed M006 final recommendation and safety boundaries.
affects:
  - S02
  - S03
  - S04
key_files:
  - .gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md
  - src/arxiv_archive/validation_batch_state.py
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_state.py
  - tests/test_validation_batch_cli_contract.py
key_decisions:
  - Use pure dataclasses for state schema.
  - Keep contract-only workflow stubs non-zero.
  - Preserve legacy root CLI compatibility with a Typer callback.
  - Keep safety flags local in validation_batch_state until reuse pressure justifies centralization.
patterns_established:
  - Contract command exits zero; workflow stubs exit non-zero until implemented.
  - Markdown-scan readiness is modeled separately from PDF/source/KG readiness.
  - Contradictions are explicit diagnostics rather than silent success.
  - Legacy CLI entrypoints must be regression-tested when adding Typer subcommands.
observability_surfaces:
  - Structured diagnostics with severity/code/paper_id/message/recommended_action.
  - Contract JSON response with explicit no-work and no-write flags.
  - Batch state JSON schema for later persisted automation.
drill_down_paths:
  - .gsd/milestones/M007-opaont/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M007-opaont/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T18:58:15.667Z
blocker_discovered: false
---

# S01: CLI contract and batch state model

**S01 defined the validation batch contract, state schema, diagnostics, and safe CLI stubs.**

## What Happened

S01 established the first layer of M007 automation. The contract document defines command names, artifact layout, phases, state schema, diagnostics, and safety boundaries. The new `validation_batch_state` module provides pure dataclass state helpers, JSON serialization, read/write helpers, safety flag validation, source contradiction detection, aggregate diagnostics, and safe contract responses. The CLI now exposes `validation-batch contract/init/preflight/scan/review/resume`; only `contract` succeeds, while workflow stubs return safe `not_implemented` responses and exit non-zero. Regression tests confirmed existing root `--date` CLI behavior still works.

## Verification

Fresh slice verification passed: contract artifact guard passed, 40 tests passed across validation state, validation CLI contract, existing CLI analysis regression, source scan, and deviation scan; ruff passed.

## Requirements Advanced

- R033 — S01 implements the first contract/state layer for deterministic resumable validation batches.
- R032 — S01 translates M006's +10 automation recommendation into concrete CLI and state surfaces.

## Requirements Validated

None.

## New Requirements Surfaced

- S02 must implement actual batch initialization/source preflight against this contract and must include contradiction diagnostics in persisted artifacts.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The first CLI regression run failed because adding a nested Typer app changed root option parsing and broke the legacy `uv run python -m arxiv_archive --date ...` contract. S01 fixed this with a root callback that preserves the legacy entrypoint while allowing subcommands.

## Known Limitations

S01 does not implement actual batch selection, source preflight, acquisition, scan execution, or review mutation. It only defines the safe contract and discoverable command surface.

## Follow-ups

S02 should implement real deterministic batch initialization and source preflight using the S01 state schema. It must preserve non-zero behavior for unimplemented commands and keep source acquisition bounded.

## Files Created/Modified

- `.gsd/milestones/M007-opaont/slices/S01/validation-cli-contract.md` — Validation batch CLI/state contract document.
- `src/arxiv_archive/validation_batch_state.py` — Pure validation batch state module with safety flags and contradiction diagnostics.
- `src/arxiv_archive/cli.py` — Validation batch CLI namespace and contract-only stubs.
- `tests/test_validation_batch_state.py` — State helper tests.
- `tests/test_validation_batch_cli_contract.py` — CLI contract tests.
