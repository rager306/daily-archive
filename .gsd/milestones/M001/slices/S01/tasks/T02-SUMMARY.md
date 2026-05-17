---
id: T02
parent: S01
milestone: M001
key_files:
  - src/arxiv_archive/cli.py
  - src/arxiv_archive/__main__.py
  - pyproject.toml
  - uv.lock
key_decisions:
  - Expose the agent contract in both Typer top-level help and `run --help` so Hermes/cron operators can discover the same operational surfaces from either entrypoint.
  - Keep `--json` documented but non-persistent for this slice, emitting a stderr notice only when the option is used.
  - Use setuptools src-layout packaging metadata so the public `uv run python -m arxiv_archive` entrypoint is importable outside pytest's configured pythonpath.
duration: 
verification_result: passed
completed_at: 2026-05-16T12:25:08.857Z
blocker_discovered: false
---

# T02: Replaced the argparse CLI boundary with a Typer app that exposes the Hermes/cron help contract through `python -m arxiv_archive`.

**Replaced the argparse CLI boundary with a Typer app that exposes the Hermes/cron help contract through `python -m arxiv_archive`.**

## What Happened

Implemented a Typer-based CLI in `src/arxiv_archive/cli.py` with top-level and `run` command help documenting project purpose, Hermes/cron usage, artifact paths, status meanings, exit codes, examples, and M001 non-goals. Preserved the existing pipeline behind `run --date`, added a documented `--json` option without implementing JSON persistence, and changed date handling to parse a `YYYY-MM-DD` string inside the command because this Typer version does not support `datetime.date` options directly. Updated `src/arxiv_archive/__main__.py` to guard the module entrypoint and updated packaging metadata so `uv run python -m arxiv_archive` can import the src-layout package. Refreshed `uv.lock` after adding Typer/project packaging metadata.

## Verification

Ran the task verification command `uv run pytest tests/test_cli_contract.py -v`; both top-level and `run` help contract tests passed. Also attempted GitNexus impact/detect checks; impact could not identify this worktree's Python symbols in the current index and detect-changes failed because the CLI's internal git diff did not recognize the gitfile worktree as a repository.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_cli_contract.py -v` | 0 | ✅ pass (2 passed, 1 warning) | 1610ms |

## Deviations

Added/updated `uv.lock` as a necessary lockfile refresh after dependency/packaging metadata changes. Used string parsing for `--date` instead of a direct `datetime.date` Typer option because the installed Typer stack rejected `datetime.date` at command construction time.

## Known Issues

GitNexus checks are degraded in this worktree: `run_pipeline` was not found in the index, `main` was ambiguous across indexed repos, and `detect-changes` failed with an internal `git diff` repository recognition error. Pytest still emits an unrelated warning about unknown config option `asyncio_mode` because pytest-asyncio is not installed in the base pytest process used by this environment.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
- `src/arxiv_archive/__main__.py`
- `pyproject.toml`
- `uv.lock`
