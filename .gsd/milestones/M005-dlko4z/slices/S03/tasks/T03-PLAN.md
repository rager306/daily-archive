---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Assign routes states and refusal reasons

Assign deterministic chunk types, routes, quality states, allowed/excluded uses, and refusal reasons from structural element classes. Ensure references, administrative/front-matter, tables, figures, equations, method sections, and retrieval-only prose are routed conservatively.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
- `.gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Diagnostics should include counts by route, state, type, and refusal reason for each package.
