---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Wire validation batch scan CLI

Wire `validation-batch scan` to the scan workflow helper. It should require a source-ready state path, write scan/delta/outlier artifacts, and keep review/resume as non-zero stubs.

## Inputs

- `src/arxiv_archive/cli.py`
- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_cli_preflight.py`

## Expected Output

- `src/arxiv_archive/cli.py`
- `tests/test_validation_batch_cli_scan.py`

## Verification

uv run pytest tests/test_validation_batch_cli_scan.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_cli_contract.py tests/test_analysis.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_cli_scan.py

## Observability Impact

Makes scan execution discoverable and reviewable from CLI with JSON response paths.
