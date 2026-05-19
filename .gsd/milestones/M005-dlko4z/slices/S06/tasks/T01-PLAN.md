---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Define chunking benchmark contract

Define a benchmark result contract for chunking methods. Include method id, input corpus, per-paper metrics, aggregate metrics, route/type/state/refusal counts, source-span coverage, parent/reference coverage, annotation coverage, asset-linkage coverage, import eligibility counts, missing-source caveats, and redaction/no-write flags. Add tests for metric aggregation and redaction boundaries.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`

## Expected Output

- `src/arxiv_archive/chunking_benchmark.py`
- `tests/test_chunking_benchmark.py`

## Verification

uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunking_benchmark.py tests/test_chunking_benchmark.py

## Observability Impact

Metric diagnostics should identify missing fields/count mismatches without raw text.
