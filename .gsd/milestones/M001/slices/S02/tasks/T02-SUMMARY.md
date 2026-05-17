---
id: T02
parent: S02
milestone: M001
key_files:
  - src/arxiv_archive/cli.py
key_decisions:
  - Keep run_pipeline() as a compatibility wrapper around run_analysis() plus save_session() instead of removing it.
  - Use a pure DailyAnalysis boundary with no persistence side effects so S03 can serialize it later.
  - Allow arXiv/httpx/keyword/scoring failures and malformed fetched-paper data to propagate rather than converting them to empty status.
duration: 
verification_result: passed
completed_at: 2026-05-16T13:52:25.416Z
blocker_discovered: false
---

# T02: Implemented the normalized in-memory DailyAnalysis boundary and wired the CLI run command to it.

**Implemented the normalized in-memory DailyAnalysis boundary and wired the CLI run command to it.**

## What Happened

Added a frozen DailyAnalysis dataclass plus DailyAnalysisStatus type in src/arxiv_archive/cli.py. Extracted the existing fetch, keyword extraction, scoring, score-desc sorting, and top-10 selection flow into run_analysis(run_date), which returns done when papers are fetched and empty when no papers are fetched. The new boundary intentionally performs no save_session or JSON/session artifact writes, and dependency failures are allowed to propagate. Retained run_pipeline() as a compatibility wrapper that calls run_analysis() and then save_session(), preserving legacy persistence for any direct callers. Updated the Typer run command to keep the existing --date parsing and --json warning, call run_analysis(), and print one concise status/count line for done or empty results.

## Verification

Ran the required task verification command `uv run --extra dev pytest tests/test_analysis.py tests/test_pipeline.py -v`; all 13 tests passed. Reran the exact previously failing gate command `uv run --extra dev pytest tests/test_analysis.py -v`; all 7 tests passed. A symbol search found run_pipeline only referenced in cli.py, run as the Typer command/subprocess surface, and run_analysis/DailyAnalysis used by the new S02 contract tests; the dedicated GitNexus impact tool was not exposed in this harness namespace, so repository symbol search was used as the available fallback.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run --extra dev pytest tests/test_analysis.py tests/test_pipeline.py -v` | 0 | ✅ pass (13 passed) | 2436ms |
| 2 | `uv run --extra dev pytest tests/test_analysis.py -v` | 0 | ✅ pass (7 passed) | 1763ms |

## Deviations

The dedicated GitNexus impact/detect_changes tools were listed in project guidance but not available as callable tools in this harness namespace, so I used repository symbol search for direct-caller impact context instead. No JSON/session persistence was added.

## Known Issues

None.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
