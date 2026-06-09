---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Built S07 rehearsal candidates from S06 benchmark artifacts without raw-content access or graph writes.

Implement adapters that read current S06 benchmark diagnostics and S05/S04/S03 package artifacts to create isolated import rehearsal candidates. The adapter should preserve method/package identity and refusal context but never load raw source files or attempt graph writes.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl`

## Expected Output

- `src/arxiv_archive/import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`

## Verification

uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py

## Observability Impact

Adapters should make no-write decisions explicit per method and preserve missing-source caveats as refusal context.
