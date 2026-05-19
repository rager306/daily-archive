---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Run negative import boundary rehearsal

Run the negative isolated import rehearsal over current M005 artifacts and write redacted run summary plus rejection diagnostics. Confirm accepted imports are zero, rejected candidates match benchmark counts, and all no-write safety flags remain false.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl`

## Verification

uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl

## Observability Impact

Run summary becomes the authoritative evidence that current import remains safely blocked with zero writes.
