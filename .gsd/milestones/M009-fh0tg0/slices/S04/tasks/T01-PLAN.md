---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Implemented bounded quota top-up planning with explicit shortage/blocker reporting.

Add bounded top-up planning helpers that consume current batch state plus candidate inventory/readiness metadata and produce a redacted top-up report. The helper should not acquire sources; it plans deterministic replacements within max_candidates_to_consider and computes scan_allowed.

## Inputs

- `src/arxiv_archive/validation_batch_workflow.py`
- `src/arxiv_archive/validation_batch_state.py`

## Expected Output

- `src/arxiv_archive/validation_batch_workflow.py`

## Verification

uv run pytest tests/test_validation_batch_quota_fill.py tests/test_validation_batch_top_up.py -q && uv run ruff check src/arxiv_archive/validation_batch_workflow.py tests/test_validation_batch_top_up.py

## Observability Impact

Adds deterministic shortage/replacement diagnostics before scan.
