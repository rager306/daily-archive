---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Ran the redacted chunking benchmark dry-run across three bounded methods.

Run the benchmark over the 10-paper gold corpus and write redacted aggregate summary plus per-paper/method diagnostics. Confirm all import/no-write flags remain false and no raw text/chunk text/embeddings are serialized.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`

## Verification

uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl

## Observability Impact

Run summary becomes the health surface for benchmark readiness and must include method counts, caveats, no-write flags, and recommendation status.
