---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implemented deterministic source PDF/Markdown preservation with hash manifests and redacted missing-source diagnostics.

Implement deterministic preservation of source PDFs and normalized Markdown from gold-corpus required paths into a per-paper workspace under S05 run evidence. Preserve files by copy with stable names, sha256 hashes, byte sizes, source provenance, and media type. Missing files should be recorded as redacted diagnostics instead of raising opaque errors.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`

## Expected Output

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-preservation-summary.json`

## Verification

uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

## Observability Impact

Run summary should expose copied/missing counts, hash coverage, and safety flags without serializing file contents.
