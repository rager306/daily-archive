---
id: T03
parent: S01
milestone: M003-km5fty
key_files:
  - src/arxiv_archive/full_text.py
  - tests/test_full_text_ingestion.py
key_decisions:
  - Use `papers/{paper_id}/full_text.md` as the deterministic default local full-text source path for stored paper artifacts.
  - Keep the artifact-to-ingestion helper independent of `arxiv_archive.cli` to avoid changing the public daily CLI contract.
duration: 
verification_result: passed
completed_at: 2026-05-17T16:29:55.265Z
blocker_discovered: false
---

# T03: Verified the stored paper artifact path can feed the full-text ingestion boundary for future PageIndex consumers.

**Verified the stored paper artifact path can feed the full-text ingestion boundary for future PageIndex consumers.**

## What Happened

Added `full_text_source_for_paper(paper_id, papers_dir, source_type='markdown', filename='full_text.md')` to derive a deterministic local full-text source from the stored paper artifact layout without importing or modifying the CLI. Added a PageIndex-readiness contract test proving a stored paper id under a `papers/{paper_id}/full_text.md` path produces a non-empty structured ingestion result with preserved paper id, source path, extraction mode, warnings, fallback reason, and provenance. Existing T01/T02 behavior remains covered.

## Verification

Ran `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py -q`; 27 tests passed. Ran `uv run ruff check src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py`; all checks passed. LSP diagnostics reported no diagnostics for the touched files. GitNexus change detection reported low risk with no indexed changed symbols.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py -q` | 0 | ✅ pass: 27 tests passed | 3600ms |
| 2 | `uv run ruff check src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py` | 0 | ✅ pass: Ruff clean | 0ms |
| 3 | `lsp diagnostics src/arxiv_archive/full_text.py and tests/test_full_text_ingestion.py` | 0 | ✅ pass: no diagnostics | 0ms |

## Deviations

None.

## Known Issues

GitNexus has not indexed the new symbols yet, so impact/context calls return target-not-found until analysis is rebuilt.

## Files Created/Modified

- `src/arxiv_archive/full_text.py`
- `tests/test_full_text_ingestion.py`
