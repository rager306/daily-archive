---
id: T03
parent: S05
milestone: M001
key_files:
  - tests/test_analysis.py
key_decisions:
  - Use a temporary child-process `sitecustomize.py` to stub analysis without changing production code, preserving the real CLI parser/stdout/writer path under subprocess tests.
  - Use isolated temporary HOME directories for all S05 subprocess tests so queue/session/artifact writes cannot touch real user or project state.
duration: 
verification_result: passed
completed_at: 2026-05-16T16:42:14.422Z
blocker_discovered: false
---

# T03: Added offline subprocess contract tests for the cron-safe daily arXiv CLI behavior.

**Added offline subprocess contract tests for the cron-safe daily arXiv CLI behavior.**

## What Happened

Added S05 subprocess helpers to `tests/test_analysis.py` that create a temporary `sitecustomize.py` stub for child processes, set an isolated temporary HOME, and run the local CLI through `uv run python -m arxiv_archive --date 2026-05-14 --json`. The child-process stub replaces only `run_analysis`, so Typer parsing, stdout formatting, JSON session/artifact writers, per-paper artifact writers, and queue state writer all remain in the exercised public path. Added subprocess coverage for successful JSON output, empty-day output and empty aggregates, failed queue-state persistence with error text, and same-date rerun overwrite behavior with a single date-named queue file.

## Verification

Verified the requested S05 test selection, ruff on the modified test file, and the full `tests/test_analysis.py` suite. The S05 subprocess tests passed offline with isolated HOME directories and deterministic child-process stubs. Ruff passed with no findings. The full test file passed with 21 tests.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_analysis.py -k s05 -q` | 0 | ✅ pass | 3706ms |
| 2 | `uv run ruff check tests/test_analysis.py` | 0 | ✅ pass | 197ms |
| 3 | `uv run pytest tests/test_analysis.py -q` | 0 | ✅ pass | 4228ms |

## Deviations

The current Typer app exposes the command as a flattened single-command entrypoint (`python -m arxiv_archive --date ...`) while the help text examples include a `run` token that is currently rejected as an unexpected extra argument. The subprocess tests therefore use the existing working public invocation already used by prior tests, rather than making production CLI changes in this test-only task.

## Known Issues

The CLI help examples still document `uv run python -m arxiv_archive run --date ...`, but the current Typer configuration accepts `uv run python -m arxiv_archive --date ...` instead.

## Files Created/Modified

- `tests/test_analysis.py`
