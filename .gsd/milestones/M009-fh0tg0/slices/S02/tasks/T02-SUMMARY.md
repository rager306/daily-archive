---
id: T02
parent: S02
milestone: M009-fh0tg0
key_files:
  - tests/test_validation_batch_cli_freshness.py
key_decisions:
  - CLI verifier tests assert nonzero exit for stale/missing input/output mutations.
  - Sentinel assertions ensure raw fixture content and redacted token values are absent from CLI stdout.
duration: 
verification_result: passed
completed_at: 2026-05-20T04:45:23.106Z
blocker_discovered: false
---

# T02: Added freshness verifier CLI tests for fresh, stale, missing, input-mutation, and redaction cases.

**Added freshness verifier CLI tests for fresh, stale, missing, input-mutation, and redaction cases.**

## What Happened

Added CLI tests for `validation-batch verify-artifacts`. The tests create provenance fixtures, verify fresh outputs, write a report path, fail after output mutation, fail after output deletion, fail after input mutation, and confirm invalid selection responses stay redacted and keep no-write/no-import flags false. Focused tests and ruff pass.

## Verification

`tests/test_validation_batch_cli_freshness.py` passed and ruff passed for CLI/test changes.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_cli_freshness.py -q && uv run ruff check src/arxiv_archive/cli.py tests/test_validation_batch_cli_freshness.py` | 0 | ✅ pass — 6 tests passed; ruff passed | 4400ms |

## Deviations

Tests cover command selection, fresh pass, report writing, output mutation, output deletion, input mutation, and invalid selection redaction. They do not yet cover real init/preflight/scan provenance emission because that wiring is not part of S02.

## Known Issues

The verifier consumes provenance logs created by library helpers; validation-batch init/preflight/scan still need optional provenance emission in a later slice.

## Files Created/Modified

- `tests/test_validation_batch_cli_freshness.py`
