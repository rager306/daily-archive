---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Defined the negative import rehearsal contract and validator for S07.

Define an isolated import rehearsal contract and validator for negative import boundary evidence. Include accepted/rejected counts, refusal reasons, package/method ids, no-write flags, redaction flags, and remediation hints. Add tests showing import-ineligible chunks/assets are rejected and raw/embedding/write leakage is blocked.

## Inputs

- `src/arxiv_archive/chunk_import_contract.py`
- `src/arxiv_archive/chunking_benchmark.py`
- `.gsd/milestones/M005-dlko4z/slices/S06/S06-SUMMARY.md`

## Expected Output

- `src/arxiv_archive/import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`

## Verification

uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py

## Observability Impact

Boundary diagnostics should expose refusal reasons and write-prevention state without raw payloads.
