---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Run top up regression and sample evidence

Generate S04 sample evidence for a successful top-up plan and a blocked shortage plan, then run focused regression.

## Inputs

- `tests/test_validation_batch_top_up.py`
- `tests/test_validation_batch_quota_fill.py`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json`
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json`

## Verification

uv run pytest tests/test_validation_batch_top_up.py tests/test_validation_batch_quota_fill.py tests/test_validation_batch_scan_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py && test -s .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json && test -s .gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json

## Observability Impact

Sample evidence demonstrates both replacement success and explicit blocker behavior.
