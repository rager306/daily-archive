---
id: T01
parent: S01
milestone: M001
key_files:
  - tests/test_cli_contract.py
key_decisions:
  - Use subprocess against `uv run python -m arxiv_archive` so the contract tests cover the same public entrypoint Hermes/cron agents will invoke.
duration: 
verification_result: passed
completed_at: 2026-05-16T12:19:01.011Z
blocker_discovered: false
---

# T01: Added failing CLI help contract tests for the future Typer-based Hermes/cron agent contract.

**Added failing CLI help contract tests for the future Typer-based Hermes/cron agent contract.**

## What Happened

Created `tests/test_cli_contract.py` with subprocess-based pytest coverage for the public `uv run python -m arxiv_archive --help` and `uv run python -m arxiv_archive run --help` surfaces. The assertions check semantic contract content from the milestone context: project purpose, Hermes and cron usage, durable artifact paths, `--date`, `--json`, exit code documentation, status meanings, examples, and explicit M001 non-goals. The tests intentionally exercise the real module entrypoint rather than importing CLI internals, so they will catch public contract drift after the CLI is replaced with Typer.

## Verification

Ran `uv run pytest tests/test_cli_contract.py -v`. The command exited 1 with both new tests failing, which is the expected TDD red phase for this task because the current argparse CLI does not yet document the required Hermes/cron agent contract or `run` command help. This verifies the tests are active and currently expose the missing implementation work.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_cli_contract.py -v` | 1 | ✅ expected red-phase failure: 2 contract tests fail against current argparse help | 1363ms |

## Deviations

None.

## Known Issues

The new contract tests fail until later S01 implementation tasks replace the thin argparse CLI with the planned Typer help contract.

## Files Created/Modified

- `tests/test_cli_contract.py`
