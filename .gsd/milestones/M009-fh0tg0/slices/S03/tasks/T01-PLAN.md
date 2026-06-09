---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Added active milestone/batch lineage metadata to validation-batch scan artifacts.

Add active lineage metadata support to validation-batch scan artifact production. Keep compatibility with existing scanner outputs, but ensure validation-batch summary/delta/outlier artifacts expose active milestone_id and batch_id.

## Inputs

- `src/arxiv_archive/validation_batch_workflow.py`
- `src/arxiv_archive/validation_batch_state.py`

## Expected Output

- `src/arxiv_archive/validation_batch_workflow.py`

## Verification

uv run pytest tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py

## Observability Impact

Scan artifacts become traceable to active batch/milestone context.
