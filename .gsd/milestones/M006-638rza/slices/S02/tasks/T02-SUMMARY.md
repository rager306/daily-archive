---
id: T02
parent: S02
milestone: M006-638rza
key_files:
  - src/arxiv_archive/thirty_paper_source_scan.py
  - tests/test_thirty_paper_source_scan.py
key_decisions:
  - Use an injectable converter protocol so tests do not need live network/PDF conversion.
  - Write converted Markdown to paper workspaces only when `assess_full_text_quality()` returns OK.
  - Keep summaries and diagnostics redacted: paths, hashes, sizes, quality counts, methods, outcomes, and errors only.
duration: 
verification_result: passed
completed_at: 2026-05-19T16:47:24.058Z
blocker_discovered: false
---

# T02: Implemented the bounded source acquisition helper and tests.

**Implemented the bounded source acquisition helper and tests.**

## What Happened

Implemented a bounded source acquisition helper for M006. The helper reads the 30-paper manifest, identifies papers missing Markdown, optionally uses an injected converter, writes successful high-quality Markdown to the local research workspace, and emits redacted summary/diagnostics artifacts. Tests cover manifest missing-Markdown detection, successful conversion, failed conversion, low-quality Markdown rejection, summary counts, diagnostics line counts, and no raw converted body text in JSONL diagnostics.

## Verification

Focused verification passed: 3 new tests passed and ruff reported all checks passed for the helper and tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "MDConverter", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — no upstream callers/processes reported; new helper uses existing converter without modifying it | 0ms |
| 2 | `uv run pytest tests/test_thirty_paper_source_scan.py -q && uv run ruff check src/arxiv_archive/thirty_paper_source_scan.py tests/test_thirty_paper_source_scan.py` | 0 | ✅ pass — 3 passed; ruff all checks passed | 5200ms |

## Deviations

None.

## Known Issues

The helper is verified with fake converters. Real network/PDF conversion success rate is still unknown until T03.

## Files Created/Modified

- `src/arxiv_archive/thirty_paper_source_scan.py`
- `tests/test_thirty_paper_source_scan.py`
