---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: Generate benchmark review samples

Generate bounded redacted benchmark review samples that let an independent reviewer inspect method differences without exposing raw paper text. Include representative per-paper/method rows, route/type/refusal/asset-linkage deltas, missing-source caveats, and recommendation rationale.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-index.json`

## Verification

uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-index.json

## Observability Impact

Review samples should make count-only or schema-only false confidence visible to reviewers.
