---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Define deterministic annotation sidecars

Define annotation sidecar dataclasses and contract serialization for deterministic chunk annotations. Include annotation id, paper id, chunk id, method, annotation type, values, confidence class, warnings, and `promoted_to_fact=false`.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/structure_aware_chunking.py`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Expose annotation redaction flags and ensure annotations are serializable without raw text.
