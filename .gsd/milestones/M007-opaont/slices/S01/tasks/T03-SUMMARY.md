---
id: T03
parent: S01
milestone: M007-opaont
key_files:
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_cli_contract.py
key_decisions:
  - Expose `validation-batch` as a nested Typer app to avoid overloading the existing daily `run` command.
  - Make `contract` exit 0 and workflow stubs exit 1 so automation cannot mistake stubs for completed work.
  - Preserve legacy root `uv run python -m arxiv_archive --date ...` behavior via callback.
duration: 
verification_result: passed
completed_at: 2026-05-19T18:57:13.813Z
blocker_discovered: false
---

# T03: Exposed the validation-batch CLI contract stub while preserving legacy CLI behavior.

**Exposed the validation-batch CLI contract stub while preserving legacy CLI behavior.**

## What Happened

Added the `validation-batch` CLI namespace with `contract`, `init`, `preflight`, `scan`, `review`, and `resume` commands. The `contract --json` command exits successfully and prints the safety contract. Workflow commands print safe `not_implemented` JSON and exit non-zero. Tests verify command discoverability, contract JSON safety, non-zero stub behavior, and no work claims. A regression in the legacy root `--date` shorthand was caught and fixed with a root callback that delegates to the existing daily run command when no subcommand is invoked.

## Verification

Verification passed: 34 tests covering validation batch state, CLI contract, and existing CLI analysis behavior passed; 19 additional focused M006-adjacent tests passed; ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "app", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — no upstream dependents | 0ms |
| 2 | `gitnexus_impact({target: "Function:src/arxiv_archive/cli.py:main", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — direct caller only src/arxiv_archive/__main__.py | 0ms |
| 3 | `uv run pytest tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py` | 0 | ✅ pass — 34 passed; ruff all checks passed | 8500ms |
| 4 | `uv run pytest tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py tests/test_thirty_paper_source_scan.py tests/test_thirty_paper_deviation_scan.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_state.py tests/test_validation_batch_cli_contract.py` | 0 | ✅ pass — 19 passed; ruff all checks passed | 9500ms |

## Deviations

Initial CLI regression failed because adding a nested Typer app removed the legacy root `--date` shorthand. Fixed by adding a root callback that delegates to the existing `run` command when no subcommand is invoked.

## Known Issues

Workflow commands are intentionally not implemented beyond safe contract responses. S02 must replace or extend stubs with real batch initialization/source preflight behavior while preserving non-implemented commands as unsafe to treat as success.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
- `tests/test_validation_batch_cli_contract.py`
