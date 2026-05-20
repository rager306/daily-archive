---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Verify artifact lineage metadata

Extend provenance freshness verification to optionally check expected artifact metadata fields, then add tests proving mismatched milestone/batch metadata fails.

## Inputs

- `src/arxiv_archive/validation_batch_provenance.py`
- `tests/test_validation_batch_provenance.py`

## Expected Output

- `src/arxiv_archive/validation_batch_provenance.py`
- `tests/test_validation_batch_provenance.py`

## Verification

uv run pytest tests/test_validation_batch_provenance.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py

## Observability Impact

Verifier catches stale lineage metadata instead of only byte freshness.
