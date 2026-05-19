---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Build baseline package validator

Implement a read-only baseline package builder that maps current available paper/full-text/PageIndex chunk artifacts into S01 import-ready package dictionaries and runs `validate_import_ready_package`. It should be conservative: missing source spans, unresolved parents, missing artifacts, or current chunks without graph-grade metadata should become structured diagnostics rather than guessed fixes.

## Inputs

- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/page_index.py`
- `src/arxiv_archive/evidence.py`
- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`

## Expected Output

- `src/arxiv_archive/chunk_baseline_measurement.py`
- `tests/test_chunk_baseline_measurement.py`

## Verification

uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_baseline_measurement.py tests/test_chunk_baseline_measurement.py

## Observability Impact

Builder returns validation summaries and refusal reasons without raw text/embedding output.
