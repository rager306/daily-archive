---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Validate annotation contract boundaries

Validate that annotation sidecars satisfy the S01 contract: all chunk references resolve, redaction holds, `promoted_to_fact=false`, and no annotation creates import eligibility. Include negative tests for unresolved chunks, promoted facts, and raw text leakage.

## Inputs

- `src/arxiv_archive/chunk_import_contract.py`
- `tests/test_chunk_import_contract.py`

## Expected Output

- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Failure diagnostics should clearly identify annotation contract violations without logging raw values.
