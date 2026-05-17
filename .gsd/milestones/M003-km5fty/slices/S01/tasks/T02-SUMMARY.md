---
id: T02
parent: S01
milestone: M003-km5fty
key_files:
  - src/arxiv_archive/full_text.py
  - tests/test_full_text_ingestion.py
key_decisions:
  - Keep full-text ingestion local-only: no PDF fetching, arXiv calls, or network access.
  - Return diagnostic fields in `FullTextIngestionResult` instead of logging-only parser state.
duration: 
verification_result: passed
completed_at: 2026-05-17T16:27:05.745Z
blocker_discovered: false
---

# T02: Implemented the local full-text ingestion boundary and made the S01 contract tests pass.

**Implemented the local full-text ingestion boundary and made the S01 contract tests pass.**

## What Happened

Implemented `src/arxiv_archive/full_text.py` with frozen dataclasses for `FullTextSource` and `FullTextIngestionResult`, plus a local-only `ingest_full_text(source)` function. The implementation validates source type before parsing, reads UTF-8 local files, returns structured markdown results when markdown headings are present, returns explicit plain-text fallback metadata for unstructured inputs, and returns typed diagnostic results for missing or empty sources. It preserves source path, source type, extraction mode, warnings, fallback reason, and provenance in code-readable fields for downstream PageIndex construction.

## Verification

Ran `uv run pytest tests/test_full_text_ingestion.py -q`; all 5 tests passed. Ran `uv run ruff check src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py`; all checks passed. LSP diagnostics reported no diagnostics for `src/arxiv_archive/full_text.py`. GitNexus impact for the new symbols returned target-not-found/zero indexed callers as expected for new code, and change detection reported no indexed changed symbols.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_full_text_ingestion.py -q` | 0 | ✅ pass: 5 full-text ingestion tests passed | 110ms |
| 2 | `uv run ruff check src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py` | 0 | ✅ pass: Ruff clean | 0ms |
| 3 | `lsp diagnostics src/arxiv_archive/full_text.py` | 0 | ✅ pass: no diagnostics | 0ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`
