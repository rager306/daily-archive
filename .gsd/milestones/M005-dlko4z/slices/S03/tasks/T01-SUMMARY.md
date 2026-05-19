---
id: T01
parent: S03
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - tests/test_structure_aware_chunking.py
key_decisions:
  - S03 starts with a deterministic dataclass/API skeleton and redacted package builder before adding parsing or route inference.
  - The initial structure-aware package is valid but not import-ready; this preserves the no-overclaim boundary from S02.
duration: 
verification_result: passed
completed_at: 2026-05-19T06:47:27.396Z
blocker_discovered: false
---

# T01: Defined the S03 structure-aware chunking model skeleton and redacted contract package output.

**Defined the S03 structure-aware chunking model skeleton and redacted contract package output.**

## What Happened

Created the new `structure_aware_chunking` module with source-span, structural-element, route-eligibility, chunk, and package dataclasses. The package skeleton serializes to the S01 import-ready contract shape without raw text, embeddings, vectors, secrets, production import attempts, or LadybugDB writes. Added tests for normalized-Markdown spans, redacted route eligibility, valid-but-not-import-ready package output, and hierarchy serialization.

## Verification

Task verification passed after fixing the import-time default_factory and correcting redaction tests to allow required redaction flag names.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 4 passed; ruff all checks passed | 6100ms |

## Deviations

The initial test run exposed a dataclass import-time bug caused by using `_now_iso` as a default_factory before definition. It was fixed with a lazy lambda. The first redaction tests also over-blocked required redaction flag names; tests now assert absence of raw content values while allowing required `raw_text_included`/`chunk_text_included` flags.

## Known Issues

T01 does not yet parse Markdown into real structural chunks. T02 must implement canonical Markdown structure parsing and span coverage.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
