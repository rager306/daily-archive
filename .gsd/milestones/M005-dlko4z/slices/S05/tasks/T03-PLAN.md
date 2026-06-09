---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Linked annotation sidecar diagnostics to redacted non-fact asset records.

Use S04 annotation sidecar diagnostics to create redacted asset-link records for table, figure, equation, reference, and metadata-related chunks. Link each asset candidate to paper id, chunk id, route/type/state, source span, source artifact, and extraction state. Do not create KG facts, embeddings, base64 payloads, or raw table/figure text.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json`

## Expected Output

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`

## Verification

uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

## Observability Impact

Diagnostics should distinguish preserved source files from linked-but-not-extracted asset candidates and count extraction states by type.
