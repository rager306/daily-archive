---
id: T02
parent: S04
milestone: M001
key_files:
  - src/arxiv_archive/cli.py
key_decisions:
  - Use `Counter` with `sorted(counter.items(), key=lambda item: (-item[1], item[0]))` for deterministic aggregate ordering.
  - Compute score breakdown statistics only over papers that actually expose each component, avoiding division by zero for empty days.
  - Reuse existing `_serialize_paper` and `_serialize_scored_paper` helpers for both daily and per-paper artifact shapes.
duration: 
verification_result: passed
completed_at: 2026-05-16T15:45:19.550Z
blocker_discovered: false
---

# T02: Persisted reusable per-paper JSON artifacts and populated daily overview aggregates for S04 analysis output.

**Persisted reusable per-paper JSON artifacts and populated daily overview aggregates for S04 analysis output.**

## What Happened

Added `PAPERS_DIR` beside the existing analysis/session roots and implemented `write_paper_artifacts(scored)` to idempotently create `~/.research/papers/{arxiv-id}/paper.json` and `scored.json` through the existing serializer helpers. Added `build_overview_payload(analysis)` to preserve the S03 overview metadata while populating deterministic category counts, keyword counts, top-paper payloads, and per-component score breakdown statistics. Updated `write_daily_artifacts()` to call the per-paper writer for every scored paper and write the populated overview while preserving the existing `papers.json` and `scored.json` daily artifacts. Empty-day behavior naturally emits empty aggregate arrays and `{}` score breakdown because the aggregation counters and breakdown map remain empty.

## Verification

Focused S04 contract tests now pass, the full `tests/test_analysis.py` regression suite passes, and production Ruff is clean for `src/arxiv_archive/cli.py`. A broader `ruff check src/arxiv_archive/cli.py tests/test_analysis.py` was also attempted and exposed pre-existing test-only forward-reference annotation lint errors in `tests/test_analysis.py`; production source lint remains clean. GitNexus impact analysis was attempted via CLI fallback because the GitNexus MCP tools were not exposed in this session; the CLI reported `Target 'write_daily_artifacts' not found` and likewise could not resolve the new helper symbols. GitNexus change detection was attempted with `--repo root`, but the CLI failed to run `git diff HEAD` in this isolated worktree, reporting it was not a git repository. No HIGH/CRITICAL GitNexus risk result was returned; risk remained UNKNOWN due unresolved symbols/tooling limitations.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_analysis.py -v -k s04` | 0 | ✅ pass | 1152ms |
| 2 | `uv run pytest tests/test_analysis.py -v` | 0 | ✅ pass | 1795ms |
| 3 | `uv run ruff check src/arxiv_archive/cli.py` | 0 | ✅ pass | 143ms |
| 4 | `npx gitnexus impact write_daily_artifacts --repo root --direction upstream` | 0 | ⚠️ diagnostic: target not found, risk UNKNOWN | 2695ms |
| 5 | `npx gitnexus detect-changes --repo root --scope all` | 0 | ⚠️ diagnostic: CLI failed internally because worktree was not recognized as a git repository | 2695ms |

## Deviations

GitNexus MCP impact/detect tools were not available in the callable tool set. CLI fallback attempts were recorded but could not resolve the symbols or run change detection in the worktree. No test changes were needed because T01 had already added the S04 contracts.

## Known Issues

`uv run ruff check src/arxiv_archive/cli.py tests/test_analysis.py` fails on existing `tests/test_analysis.py` quoted `DailyAnalysis` annotations (UP037/F821); this task verified production Ruff clean on the modified source file instead. GitNexus index/CLI did not resolve `write_daily_artifacts` or new helper symbols for impact analysis.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
