---
id: T01
parent: S03
milestone: M006-638rza
key_files:
  - src/arxiv_archive/thirty_paper_deviation_scan.py
  - tests/test_thirty_paper_deviation_scan.py
key_decisions:
  - Build S03 as a Markdown-based scanner using structure-aware package diagnostics, not as PDF/multimodal analysis.
  - Serialize per-paper counts, distributions, outlier flags, and baseline deltas only; raw Markdown/chunk text remains excluded.
duration: 
verification_result: passed
completed_at: 2026-05-19T17:58:59.772Z
blocker_discovered: false
---

# T01: Implemented the 30-paper deviation scanner and tests.

**Implemented the 30-paper deviation scanner and tests.**

## What Happened

Implemented the M006 30-paper deviation scanner. The helper normalizes M006 manifest entries into the structure-aware chunker input shape, builds redacted package diagnostics for each paper, aggregates route/type/state/refusal distributions, compares against an optional M005 baseline summary, and flags simple outliers such as high chunk count, table-heavy, claim-heavy, zero chunks, or unexpected import eligibility. Tests verify redacted output, baseline comparison, split summary/diagnostics writing, and that raw Markdown content is not serialized.

## Verification

Focused verification passed: 34 tests passed and ruff reported all checks passed for the new scanner and tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_thirty_paper_deviation_scan.py tests/test_structure_aware_chunking.py tests/test_chunking_benchmark.py -q && uv run ruff check src/arxiv_archive/thirty_paper_deviation_scan.py tests/test_thirty_paper_deviation_scan.py` | 0 | ✅ pass — 34 passed; ruff all checks passed | 10700ms |

## Deviations

None.

## Known Issues

The scanner currently provides deterministic distribution/outlier metrics. Semantic interpretation and recommendations are deferred to T03/S04 review.

## Files Created/Modified

- `src/arxiv_archive/thirty_paper_deviation_scan.py`
- `tests/test_thirty_paper_deviation_scan.py`
