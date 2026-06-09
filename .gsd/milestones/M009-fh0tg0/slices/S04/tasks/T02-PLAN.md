---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T02: Added bounded top-up tests for success, shortage, duplicate exclusion, and blocker diagnostics.

Add tests for top-up planning: already full quota, underfilled with enough replacements, underfilled with max-attempt shortage, duplicate/selected candidate exclusion, and redaction/safety flags.

## Inputs

- `src/arxiv_archive/validation_batch_workflow.py`

## Expected Output

- `tests/test_validation_batch_top_up.py`

## Verification

uv run pytest tests/test_validation_batch_top_up.py tests/test_validation_batch_quota_fill.py -q && uv run ruff check tests/test_validation_batch_top_up.py src/arxiv_archive/validation_batch_workflow.py

## Observability Impact

Tests prove shortage handling instead of happy-path-only quota checks.
