---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Generated freshness verifier pass/fail sample reports and ran focused regression.

Generate S02 sample freshness reports for a fresh and stale artifact set, then run focused CLI regression tests.

## Inputs

- `tests/test_validation_batch_cli_freshness.py`
- `tests/test_validation_batch_cli_contract.py`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json`

## Verification

uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_cli_freshness.py tests/test_validation_batch_cli_contract.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_cli_freshness.py && test -s .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json && test -s .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json

## Observability Impact

S02 evidence demonstrates both pass and fail freshness outcomes.
