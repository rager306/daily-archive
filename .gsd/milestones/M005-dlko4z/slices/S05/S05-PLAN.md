# S05: S05

**Goal:** Preserve source PDF/Markdown artifacts beside each gold-corpus paper and emit redacted multimodal/source asset manifests that link table/figure/equation/caption candidates to chunks without embedding raw binary/image/text data in machine logs or treating assets as KG facts.
**Demo:** After this slice, source PDFs, normalized Markdown, extracted figures, tables, and image assets are preserved with redacted asset manifests for future multimodal retrieval.

## Must-Haves

- Source PDFs and normalized Markdown referenced by the gold-corpus manifest are preserved in deterministic per-paper workspaces or reported with redacted missing-source diagnostics.
- Each preserved source file has stable path, sha256, byte size, media type, provenance, and safety flags in a redacted manifest.
- Table/figure/equation/caption candidates from S04 sidecars are represented as asset-link manifest records with stable ids, source spans/chunk ids, extraction state, and no raw content.
- JSON/JSONL artifacts contain no raw binary/base64, raw paper text, chunk text, embeddings, vectors, secrets, optimizer traces, or KG facts.
- Dry-run summary confirms production KG writes/imports remain false and asset annotations are not promoted to facts.
- Independent review confirms artifacts are meaningful and not count-only.

## Proof Level

- This slice proves: Automated tests for manifest schema, hashing, source copy/link behavior, redaction, and sidecar-to-asset linkage; gold-corpus dry-run artifacts; independent artifact review before slice completion.

## Integration Closure

S05 consumes S04 annotation sidecars and the S01 gold corpus. It produces per-paper source workspaces and manifest records that S06 can use for chunking-method benchmarks and S07 can inspect during import rehearsal, while keeping assets out of KG facts and preserving all no-write/no-import boundaries.

## Verification

- Adds run-level and per-paper asset-preservation summaries with file counts, hash coverage, missing-source diagnostics, asset-linkage counts, extraction states, and safety flags proving no raw binary/base64/text/embeddings are serialized in JSON/JSONL logs.

## Tasks

- [x] **T01: Defined the redacted source asset manifest contract and validator.** `est:medium`
  Define dataclasses and validators for source artifacts, preserved files, asset records, and per-paper multimodal manifests. Include stable ids, source paths, workspace-relative paths, sha256, byte size, media type, provenance, source spans/chunk ids, extraction state, and redaction/no-import flags. Add tests for redaction, required fields, hash metadata, and assets-not-KG-facts boundaries.
  - Files: `src/arxiv_archive/source_asset_manifest.py`, `tests/test_source_asset_manifest.py`
  - Verify: uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

- [x] **T02: Implemented deterministic source PDF/Markdown preservation with hash manifests and redacted missing-source diagnostics.** `est:large`
  Implement deterministic preservation of source PDFs and normalized Markdown from gold-corpus required paths into a per-paper workspace under S05 run evidence. Preserve files by copy with stable names, sha256 hashes, byte sizes, source provenance, and media type. Missing files should be recorded as redacted diagnostics instead of raising opaque errors.
  - Files: `src/arxiv_archive/source_asset_manifest.py`, `tests/test_source_asset_manifest.py`
  - Verify: uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

- [x] **T03: Linked annotation sidecar diagnostics to redacted non-fact asset records.** `est:large`
  Use S04 annotation sidecar diagnostics to create redacted asset-link records for table, figure, equation, reference, and metadata-related chunks. Link each asset candidate to paper id, chunk id, route/type/state, source span, source artifact, and extraction state. Do not create KG facts, embeddings, base64 payloads, or raw table/figure text.
  - Files: `src/arxiv_archive/source_asset_manifest.py`, `tests/test_source_asset_manifest.py`
  - Verify: uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py

- [x] **T04: Ran the S05 source asset preservation dry-run with linked multimodal asset records.** `est:medium`
  Run the source asset preservation and multimodal manifest dry-run over the 10-paper gold corpus. Write per-paper manifests, a redacted run summary, and JSONL diagnostics under S05 run evidence. Confirm all machine artifacts contain only paths/hashes/provenance/linkage/safety flags, not raw content.
  - Files: `src/arxiv_archive/source_asset_manifest.py`, `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`, `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl`
  - Verify: uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl

- [x] **T05: Reviewed S05 source asset preservation artifacts and documented S06-safe consumption boundaries.** `est:medium`
  Review the S05 manifests and diagnostics independently for semantic usefulness, source/hash coverage, missing-source clarity, redaction, and non-fact boundaries. Write a slice report that states what is preserved, what remains linked-but-not-extracted, and whether S06 benchmarking can safely consume the manifests.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S05/source-asset-preservation-report.md`, `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md`
  - Verify: uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S05/source-asset-preservation-report.md && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md

## Files Likely Touched

- src/arxiv_archive/source_asset_manifest.py
- tests/test_source_asset_manifest.py
- .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json
- .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl
- .gsd/milestones/M005-dlko4z/slices/S05/source-asset-preservation-report.md
- .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md
