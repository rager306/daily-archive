---
id: S01
parent: M003-km5fty
milestone: M003-km5fty
provides:
  - A tested `arxiv_archive.full_text` module for local markdown/text ingestion.
  - Deterministic fixtures and contract tests for downstream PageIndex construction.
  - A `full_text_source_for_paper()` helper mapping stored paper ids to local full-text source paths.
requires:
  []
affects:
  - S02
key_files:
  - src/arxiv_archive/full_text.py
  - tests/test_full_text_ingestion.py
  - tests/fixtures/full_text/structured_paper.md
  - tests/fixtures/full_text/plain_fallback.txt
key_decisions:
  - Use `FullTextSource` + `ingest_full_text(source)` as the public full-text ingestion interface.
  - Use `papers/{paper_id}/full_text.md` as the deterministic default stored paper full-text source path.
  - Represent parser quality and failure states through result fields: extraction mode, warnings, fallback reason, and provenance.
patterns_established:
  - Local-only ingestion boundaries should return typed diagnostic results rather than logging-only parser state.
  - Stored paper artifact consumers should derive deterministic paths from paper id and pass them into pure ingestion functions.
observability_surfaces:
  - `FullTextIngestionResult.extraction_mode` records structured markdown, plain text fallback, missing source, or empty source.
  - `FullTextIngestionResult.warnings` records parser quality or source-state diagnostics.
  - `FullTextIngestionResult.fallback_reason` records machine-readable fallback causes.
  - `FullTextIngestionResult.provenance` records paper id, source type, source path, extraction mode, and fallback reason when present.
drill_down_paths:
  - .gsd/milestones/M003-km5fty/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S01/tasks/T03-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S01/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-17T16:31:44.491Z
blocker_discovered: false
---

# S01: Full text ingestion contract

**S01 established a deterministic local full-text ingestion boundary with fixtures, tests, diagnostics, and PageIndex-ready artifact path mapping.**

## What Happened

S01 delivered a deterministic local full-text ingestion contract for future scientific PageIndex and evidence-path work. It started with red contract tests and fixtures, then implemented `src/arxiv_archive/full_text.py` as a local-only boundary with typed source/result models and code-readable diagnostics. The slice also added a stored-paper artifact helper so downstream consumers can map a paper id to `papers/{paper_id}/full_text.md` without touching the public daily CLI. Final verification proved the ingestion behavior, analysis regressions, and CLI contract continue to pass.

## Verification

Fresh T04 verification passed: `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` reported 29 passed; Ruff reported all checks passed; public module help smoke passed; LSP diagnostics reported no diagnostics.

## Requirements Advanced

None.

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

The current parser intentionally detects only basic markdown section structure and plain text fallback. It does not parse PDF files, fetch network content, or split text into chunks; those are downstream concerns. GitNexus has not indexed the newly added symbols until `npx gitnexus analyze` is run.

## Follow-ups

S02 should consume `FullTextIngestionResult` through `full_text_source_for_paper()` and `ingest_full_text()` when constructing PageIndex documents. Rebuild GitNexus index before relying on graph impact for the newly added full-text symbols.

## Files Created/Modified

- `src/arxiv_archive/full_text.py` — New local-only ingestion boundary with typed source/result dataclasses, diagnostics, fallback metadata, and artifact path helper.
- `tests/test_full_text_ingestion.py` — New contract and regression tests for structured markdown, plain text fallback, missing/empty sources, unsupported source types, and stored paper artifact path readiness.
- `tests/fixtures/full_text/structured_paper.md` — Structured markdown fixture used by ingestion tests.
- `tests/fixtures/full_text/plain_fallback.txt` — Plain-text fallback fixture used by ingestion tests.
