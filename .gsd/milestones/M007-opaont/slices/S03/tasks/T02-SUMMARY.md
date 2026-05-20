---
id: T02
parent: S03
milestone: M007-opaont
key_files:
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_cli_scan.py
  - tests/test_validation_batch_cli_contract.py
key_decisions:
  - `validation-batch scan` now requires a source-ready state path and output directory.
  - Scan CLI writes scan, diagnostics, delta, outlier, and updated state artifacts through workflow helpers.
  - `review` and `resume` remain non-zero stubs until S04/future slices.
duration: 
verification_result: passed
completed_at: 2026-05-20T01:52:50.350Z
blocker_discovered: false
---

# T02: Wired validation-batch scan CLI to redacted scan/delta/outlier artifact generation.

**Wired validation-batch scan CLI to redacted scan/delta/outlier artifact generation.**

## What Happened

Wired `validation-batch scan` to the scan workflow helper. The command reads a source-ready batch state, writes scan artifacts under an output directory, accepts optional M005/S03 and M005/S06 baseline paths, and prints a JSON response with artifact paths. Tests cover successful scan artifact generation, source-blocked rejection, continued contract command behavior, preflight compatibility, and existing daily CLI regression behavior.

## Verification

Verification passed: 30 CLI scan/preflight/contract/analysis tests passed and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "validation_batch_scan", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — no upstream dependents | 0ms |
| 2 | `gitnexus_impact({target: "Function:src/arxiv_archive/cli.py:main", direction: "upstream", repo: "daily-archive"})` | 0 | ✅ low risk — direct caller only src/arxiv_archive/__main__.py | 0ms |
| 3 | `uv run pytest tests/test_validation_batch_cli_scan.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_cli_contract.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_cli_scan.py` | 0 | ✅ pass — 30 passed; ruff all checks passed | 12900ms |

## Deviations

The first verification failed because an older contract test still expected `validation-batch scan` to be a non-zero stub. S03 intentionally makes scan real, so the test was updated to keep `resume` as the non-zero stub check.

## Known Issues

The CLI currently surfaces unready state errors via Typer stderr/exit non-zero rather than a structured JSON error payload. This is acceptable for S03 but could be improved later for automation ergonomics.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
- `tests/test_validation_batch_cli_scan.py`
- `tests/test_validation_batch_cli_contract.py`
