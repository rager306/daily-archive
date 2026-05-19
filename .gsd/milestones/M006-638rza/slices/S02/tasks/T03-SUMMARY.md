---
id: T03
parent: S02
milestone: M006-638rza
key_files:
  - src/arxiv_archive/thirty_paper_source_scan.py
  - tests/test_thirty_paper_source_scan.py
  - .gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json
  - .gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl
key_decisions:
  - Use fast arxiv2md-only batch acquisition for the 30-paper scan to avoid unbounded Docling/PDF fallback in bulk.
  - Record preexisting Markdown readiness among originally missing papers so partial acquisition effects are visible instead of hidden.
duration: 
verification_result: passed
completed_at: 2026-05-19T17:01:12.364Z
blocker_discovered: false
---

# T03: Ran bounded source acquisition and improved the corpus to 29/30 Markdown-ready papers.

**Ran bounded source acquisition and improved the corpus to 29/30 Markdown-ready papers.**

## What Happened

Ran bounded source acquisition for the M006 30-paper corpus. After adding fast-only mode to avoid long PDF/Docling batch hangs, the run records the S01 baseline of 20 originally missing Markdown papers, 19 originally missing papers now Markdown-ready, one active conversion attempt, and one remaining missing-Markdown blocker. Current readiness is 29/30 papers, with eight cached PDFs available. The remaining blocked paper is 2001.00186v1, where arxiv2md returned low-quality empty output. All diagnostics remain redacted and all no-import/no-write safety flags are false.

## Verification

Fresh verification passed: source acquisition summary has paper_count=30, originally_missing_markdown_count=20, ready_for_markdown_scan_count=29, still_missing_markdown_count=1, redacted diagnostics non-empty, all safety flags false; helper tests pass; ruff passes.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cancel_job bg_fdf17d4a` | 0 | ✅ cancelled long-running unbounded acquisition attempt before changing helper | 0ms |
| 2 | `uv run pytest tests/test_thirty_paper_source_scan.py -q && uv run ruff check src/arxiv_archive/thirty_paper_source_scan.py tests/test_thirty_paper_source_scan.py` | 0 | ✅ pass — 3 tests passed; ruff all checks passed after fast-only summary update | 8000ms |
| 3 | `uv run python - <<'PY' ... acquire_sources_for_manifest_sync(... fast_only=True) ... PY` | 0 | ✅ pass — paper_count=30; originally_missing=20; ready_for_markdown_scan=29; still_missing=1; safety flags false | 7700ms |
| 4 | `uv run python - <<'PY' ... summary guard ... PY && uv run pytest tests/test_thirty_paper_source_scan.py -q && uv run ruff check ...` | 0 | ✅ pass — summary guard, 3 tests, and ruff passed | 5200ms |

## Deviations

The first unbounded attempt was cancelled after running too long, but it had already produced Markdown for some expansion papers. The helper was updated to record original S01 missing count versus run-time attempts, then a bounded fast-only rerun produced authoritative S02 evidence.

## Known Issues

One expansion paper remains blocked: 2001.00186v1 failed fast arxiv2md with low-quality/empty source. It may require PDF/Docling targeted repair if full 30/30 coverage is mandatory.

## Files Created/Modified

- `src/arxiv_archive/thirty_paper_source_scan.py`
- `tests/test_thirty_paper_source_scan.py`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`
