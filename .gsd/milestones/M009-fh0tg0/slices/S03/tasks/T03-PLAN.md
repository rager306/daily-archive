---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Run lineage regression and sample evidence

Generate S03 sample scan/freshness evidence showing active M009 lineage and a negative lineage mismatch report.

## Inputs

- `tests/test_validation_batch_scan_workflow.py`
- `tests/test_validation_batch_provenance.py`

## Expected Output

- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json`

## Verification

uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py && test -s .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json && test -s .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json

## Observability Impact

Sample evidence demonstrates both lineage pass and mismatch detection.
