---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Run regression and sample provenance artifacts

Run regression checks to ensure new module does not alter existing validation-batch workflow behavior, then write S01 sample run-log/freshness artifacts for review.

## Inputs

- `tests/test_validation_batch_provenance.py`
- `tests/test_validation_batch_workflow.py`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl`
- `.gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json`

## Verification

uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py && test -s .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-cli-run-log.jsonl && test -s .gsd/milestones/M009-fh0tg0/slices/S01/run-evidence/sample-freshness-report.json

## Observability Impact

Sample artifacts demonstrate commit-safe provenance output shape.
