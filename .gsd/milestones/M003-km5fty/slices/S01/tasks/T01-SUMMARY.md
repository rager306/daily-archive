---
id: T01
parent: S01
milestone: M003-km5fty
key_files:
  - tests/fixtures/full_text/structured_paper.md
  - tests/fixtures/full_text/plain_fallback.txt
  - tests/test_full_text_ingestion.py
key_decisions:
  - Define the public ingestion boundary as `FullTextSource` plus `ingest_full_text(source)`.
  - Treat missing, empty, and unstructured input as typed results with explicit diagnostic metadata instead of silent empty output.
duration: 
verification_result: passed
completed_at: 2026-05-17T16:24:57.030Z
blocker_discovered: false
---

# T01: Added S01 full-text ingestion contract tests and deterministic fixtures.

**Added S01 full-text ingestion contract tests and deterministic fixtures.**

## What Happened

Created deterministic local full-text fixtures for structured markdown and plain-text fallback, then added `tests/test_full_text_ingestion.py` to define the future `arxiv_archive.full_text` public boundary. The tests specify `FullTextSource` and `ingest_full_text(source)` behavior for structured markdown, plain text fallback, missing source files, empty or malformed files, unsupported source types, provenance fields, warnings, extraction modes, and fallback reasons. This is intentionally test-first: the focused test command fails because `src/arxiv_archive/full_text.py` does not exist yet, which is the implementation boundary for T02.

## Verification

Ran `uv run pytest tests/test_full_text_ingestion.py -q`; it failed as expected during collection with `ModuleNotFoundError: No module named 'arxiv_archive.full_text'`. Ran `uv run pytest tests/test_analysis.py -q`; it passed with 21 tests. Ran `uv run ruff check tests/test_full_text_ingestion.py`; it passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_full_text_ingestion.py -q` | 2 | ✅ expected red contract: collection fails on missing arxiv_archive.full_text implementation boundary | 180ms |
| 2 | `uv run pytest tests/test_analysis.py -q` | 0 | ✅ pass: existing analysis tests still run | 3500ms |
| 3 | `uv run ruff check tests/test_full_text_ingestion.py` | 0 | ✅ pass: new test file lint clean | 0ms |

## Deviations

T01 intentionally ends with a red contract test rather than a green implementation because S01 planned implementation for T02.

## Known Issues

`arxiv_archive.full_text` is not implemented yet; T02 must add `FullTextSource`, `ingest_full_text`, and result models matching the contract.

## Files Created/Modified

- `tests/fixtures/full_text/structured_paper.md`
- `tests/fixtures/full_text/plain_fallback.txt`
- `tests/test_full_text_ingestion.py`
