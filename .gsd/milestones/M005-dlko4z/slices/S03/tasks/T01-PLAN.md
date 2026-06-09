---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Defined the S03 structure-aware chunking model skeleton and redacted contract package output.

Define the S03 structure-aware chunking module interface and core dataclasses for structural elements, chunks, source spans, hierarchy links, route eligibility, and package output. Keep the API deterministic and independent of LLM calls or production KG writes.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/chunk_baseline_measurement.py`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`

## Verification

uv run pytest tests/test_structure_aware_chunking.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py

## Observability Impact

Expose explicit safety flags and diagnostic fields in package summaries from the first API skeleton.
