---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Implemented batch scan workflow helpers and redacted delta/outlier artifact generation.

Extend workflow helpers with scan orchestration around the existing thirty-paper deviation scanner. The helper should build scan inputs from batch state, call redacted scanner logic, write validation-scan artifacts, and update batch phase without importing KG facts or writing LadybugDB.

## Inputs

- `src/arxiv_archive/validation_batch_workflow.py`
- `src/arxiv_archive/thirty_paper_deviation_scan.py`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json`

## Expected Output

- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_scan_workflow.py`

## Verification

uv run pytest tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_workflow.py tests/test_thirty_paper_deviation_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_scan_workflow.py

## Observability Impact

Adds scan artifact paths and blocker diagnostics for unsafe scan/import states.
