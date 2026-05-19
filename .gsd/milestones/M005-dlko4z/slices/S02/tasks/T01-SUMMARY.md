---
id: T01
parent: S02
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/chunk_baseline_measurement.py
  - tests/test_chunk_baseline_measurement.py
key_decisions:
  - Current PageIndex/SemanticChunk output is represented as `ok_for_retrieval_only`, not `ok_for_graph`, until S03 adds graph-grade structure/source spans/routes.
  - Missing full-text artifacts are package-level blocker diagnostics and aggregate into baseline summary refusal counts.
  - Machine diagnostics stay redacted and do not include raw chunk text even though the builder reads full text locally.
duration: 
verification_result: passed
completed_at: 2026-05-19T05:59:27.536Z
blocker_discovered: false
---

# T01: Built the S02 baseline package validator for current chunking.

**Built the S02 baseline package validator for current chunking.**

## What Happened

Added `chunk_baseline_measurement.py`, a read-only baseline measurement module that maps the current full-text ingestion, PageIndex, and SemanticChunk path into S01 import-ready package dictionaries. It validates each package with the S01 contract validator, emits redacted package diagnostics and aggregate summaries, and records missing full-text artifacts as blockers. Tests cover retrieval-only current chunks, missing full-text rejection, aggregate redaction, and JSON/JSONL output behavior.

## Verification

`uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_baseline_measurement.py tests/test_chunk_baseline_measurement.py` passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_baseline_measurement.py tests/test_chunk_baseline_measurement.py` | 0 | ✅ pass — 23 passed; ruff all checks passed | 6600ms |

## Deviations

Implemented the baseline builder with CLI support because T02 will need to run it over the gold corpus. The builder maps current chunks to retrieval-only packages rather than pretending they are graph-import-ready.

## Known Issues

The baseline builder does not yet create bounded review samples; that is S02/T03. It also does not implement improved structure-aware chunking; current chunks are deliberately retrieval-only baseline records.

## Files Created/Modified

- `src/arxiv_archive/chunk_baseline_measurement.py`
- `tests/test_chunk_baseline_measurement.py`
