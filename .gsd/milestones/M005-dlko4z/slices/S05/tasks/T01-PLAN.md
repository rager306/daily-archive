---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Define source asset manifest contract

Define dataclasses and validators for source artifacts, preserved files, asset records, and per-paper multimodal manifests. Include stable ids, source paths, workspace-relative paths, sha256, byte size, media type, provenance, source spans/chunk ids, extraction state, and redaction/no-import flags. Add tests for redaction, required fields, hash metadata, and assets-not-KG-facts boundaries.

## Inputs

- `.gsd/REQUIREMENTS.md`
- `.gsd/DECISIONS.md`
- `.gsd/milestones/M005-dlko4z/slices/S04/S04-SUMMARY.md`

## Expected Output

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`

## Verification

uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

## Observability Impact

Validator diagnostics should name missing fields and unsafe flags without logging raw values or file contents.
