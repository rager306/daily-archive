---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Generate sidecars from chunk metadata

Generate deterministic sidecar annotations from structural chunk metadata, including section role, route hint, structural type, table/figure/equation/reference flags, and review blockers. Do not inspect or persist raw chunk text.

## Inputs

- `src/arxiv_archive/structure_aware_chunking.py`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Diagnostics should summarize annotations by type, route, confidence class, and warning code.
