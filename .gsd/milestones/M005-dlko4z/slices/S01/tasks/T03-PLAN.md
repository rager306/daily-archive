---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Implement contract validator fixtures

Add a small contract validator module and tests for package invariants. The validator should reject raw text/embedding leakage, missing stable IDs, missing source spans for graph-eligible chunks, unresolved parent/source references, and invalid import states. It should validate synthetic fixtures only in S01.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`

## Expected Output

- `src/arxiv_archive/chunk_import_contract.py`
- `tests/test_chunk_import_contract.py`

## Verification

uv run pytest tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_import_contract.py tests/test_chunk_import_contract.py

## Observability Impact

Validator returns structured diagnostics with reasons that future slices can emit into JSON artifacts.
