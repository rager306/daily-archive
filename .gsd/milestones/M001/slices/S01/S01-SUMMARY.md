# S01 Summary

**Title:** S01: Агентный CLI контракт
**One-liner:** Replaced the thin argparse CLI with a Typer-based agent contract, exposing project purpose, Hermes/cron usage, artifact paths, exit codes, and M001 non-goals through both top-level and run command help.
**Verification:** passed
**Blockers:** none

## What Happened

S01 replaced the thin `argparse` CLI with a full `Typer`-based command structure. The primary goal was to expose a machine-readable help contract so that Hermes/cron agents can operate without reading source files.

### Key Changes

- **Typer app** with `run` subcommand replaces the previous `argparse` entrypoint
- **Top-level help** shows project purpose, Hermes/cron usage, artifact paths, exit codes, examples, and explicit non-goals
- **Run command help** exposes `--date` (YYYY-MM-DD) and `--json` (future) options with clear semantics
- **Portable exit-code vocabulary**: 0 = success/help, 1 = runtime failure, 2 = validation error
- **setuptools src-layout packaging** added so `uv run python -m arxiv_archive` is importable outside pytest's configured pythonpath

### Integration Notes

- S02 consumed the S01 Typer command shape and `run_analysis()` return type
- S03 consumed the documented JSON output behavior via `--json` flag
- S05 consumed the exit-code vocabulary for cron-safe verification

## Key Decisions

1. Expose the agent contract in both Typer top-level help and `run --help` so Hermes/cron operators can discover the same surfaces from either entrypoint.
2. Keep `--json` documented but non-persistent for this slice, emitting a stderr notice when the option is used.
3. Use setuptools src-layout packaging metadata so the public `uv run python -m arxiv_archive` entrypoint is importable outside pytest's configured pythonpath.
4. Use subprocess against `uv run python -m arxiv_archive` in CLI contract tests to cover the same public entrypoint Hermes/cron agents will invoke.

## Patterns Established

- **TDD red-green on CLI contract**: write failing tests first, then implement to pass.
- **Typer CLI with subcommands** replaces thin argparse boundary without changing downstream pipeline logic.
- **Public entrypoint** (`uv run python -m arxiv_archive`) must work identically from both cron and direct shell invocation.

## Key Files
- `src/arxiv_archive/cli.py` — Typer app and command definitions
- `src/arxiv_archive/__main__.py` — public module entrypoint
- `tests/test_cli_contract.py` — offline subprocess tests for CLI contracts
- `pyproject.toml` — setuptools src-layout packaging metadata
- `uv.lock` — lockfile refresh after packaging metadata changes

## Deviations

- Added/updated `uv.lock` as a necessary lockfile refresh after packaging metadata changes.
- Used string-based `--date` parsing instead of `datetime.date` because the installed Typer version rejects `datetime.date` at command construction time — date validation still happens and will surface clear errors for malformed input.

## Known Limitations

- GitNexus impact/detect-changes checks are degraded in this worktree due to gitfile worktree not being recognized by the index.
- Plain `uv run pytest` in this environment resolves an external Python 3.12 pytest without project dev dependencies — `uv run --extra dev pytest` must be used for the full suite.

## Follow-ups

- `--json` is documented but non-persistent in this slice. S03 should implement actual JSON persistence.
- The `run` command currently accepts `--date YYYY-MM-DD` as a string; S05 should consider whether date validation errors should map to exit code 2 (validation error).
