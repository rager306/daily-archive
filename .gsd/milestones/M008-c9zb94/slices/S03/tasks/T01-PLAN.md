---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Implement quota fill gate helpers

Implement quota-fill helper functions and tests. The helper should classify source-ready selected papers as accepted, mark unready papers as rejected/needs_replacement, and compute shortage_count. It should support deterministic future replacement metadata but not perform unbounded acquisition in this task.

## Inputs

- `src/arxiv_archive/validation_batch_workflow.py`
- `src/arxiv_archive/validation_batch_state.py`

## Expected Output

- `src/arxiv_archive/validation_batch_workflow.py`
- `tests/test_validation_batch_quota_fill.py`

## Verification

uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_workflow.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_quota_fill.py

## Observability Impact

Adds structured quota-fill diagnostics reusable by future +10 batches.
