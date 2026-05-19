---
id: T02
parent: S02
milestone: M007-opaont
key_files:
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_cli_preflight.py
  - tests/test_validation_batch_cli_contract.py
key_decisions:
  - `validation-batch init` now exits 0 and writes batch-state plus selection manifest artifacts.
  - `validation-batch preflight` now exits 0 and writes updated batch-state, source-preflight summary, and diagnostics JSONL.
  - `scan`, `review`, and `resume` remain non-zero not-implemented stubs until later slices.
duration: 
verification_result: passed
completed_at: 2026-05-19T19:09:01.060Z
blocker_discovered: false
---

# T02: Wired validation-batch init and preflight CLI commands to local artifact-writing helpers.

**Wired validation-batch init and preflight CLI commands to local artifact-writing helpers.**

## What Happened

Wired the validation-batch CLI to the new workflow helpers. The `init` command accepts `--batch-id`, `--manifest-path`, and `--output-dir`, then writes `batch-state.json` and `selection-manifest.json`. The `preflight` command accepts `--state-path` and optional `--output-dir`, inspects source paths, writes updated state, summary, and diagnostics. Contract tests were updated so `review`/`scan` remain non-zero stubs while `init`/`preflight` now perform bounded local artifact writes. Existing root CLI behavior remains covered by regression tests.

## Verification

Verification passed: 35 tests covering workflow helpers, validation-batch CLI contract/preflight, and existing CLI analysis regression passed; ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "app", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — no upstream dependents | 0ms |
| 2 | `gitnexus_impact({target: "Function:src/arxiv_archive/cli.py:main", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — direct caller only src/arxiv_archive/__main__.py | 0ms |
| 3 | `uv run pytest tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_contract.py tests/test_validation_batch_cli_preflight.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_contract.py tests/test_validation_batch_cli_preflight.py` | 0 | ✅ pass — 35 passed; ruff all checks passed | 11100ms |

## Deviations

Initial verification had a ruff import-order failure in cli.py after adding workflow imports; ruff --fix corrected import formatting and verification reran successfully.

## Known Issues

S02 preflight only inspects existing source paths and writes readiness state. It does not perform source acquisition, conversion repair, scan execution, or review mutation.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
- `tests/test_validation_batch_cli_preflight.py`
- `tests/test_validation_batch_cli_contract.py`
