---
id: T02
parent: S05
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/source_asset_manifest.py
  - tests/test_source_asset_manifest.py
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-preservation-summary.json
key_decisions:
  - Source preservation copies available PDF/Markdown files into deterministic per-paper S05 workspaces and records hashes/provenance instead of embedding contents in JSON/JSONL.
  - Missing source files are redacted diagnostics and do not cause opaque crashes during preservation dry runs.
duration: 
verification_result: passed
completed_at: 2026-05-19T09:14:41.518Z
blocker_discovered: false
---

# T02: Implemented deterministic source PDF/Markdown preservation with hash manifests and redacted missing-source diagnostics.

**Implemented deterministic source PDF/Markdown preservation with hash manifests and redacted missing-source diagnostics.**

## What Happened

Implemented deterministic source preservation for gold-corpus paper entries. The preservation path resolves normalized Markdown and PDF candidates, copies available files into stable per-paper source workspaces, computes SHA-256 hashes, records byte sizes/media types/provenance, emits per-paper manifests and JSONL diagnostics, and writes run summaries without raw file contents. The gold-corpus dry-run preserved 12 files across 10 papers with hash coverage 1.0 and redacted diagnostics for 8 missing PDFs. All no-raw/no-binary/no-embedding/no-write safety flags remain false.

## Verification

Fresh verification passed after artifact generation: source-asset, structure-aware, and import-contract tests passed; ruff passed; source-preservation summary exists; artifact guard confirmed paper_count=10, source_file_count=12, hash_coverage_rate=1.0, missing_original_pdf=8, and all safety flags false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-preservation-summary.json && uv run python - <<'PY' ... source preservation artifact guard ... PY` | 0 | ✅ pass — 50 passed; ruff all checks passed; artifact guard confirmed source_file_count=12, hash_coverage_rate=1.0, missing_original_pdf=8, safety_flags_false=true | 5900ms |

## Deviations

The dry-run found only 2 available PDFs in the current local source paths; the other 8 PDFs are recorded as redacted `missing_original_pdf` diagnostics rather than treated as fatal T02 failures. This preserves observability while leaving PDF acquisition/repair as downstream or rerun work.

## Known Issues

Eight gold-corpus PDFs were not available at the current required/fallback source paths. The run preserves all available normalized Markdown files and two PDFs with hashes; missing PDFs remain visible for future acquisition or repair.

## Files Created/Modified

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-preservation-summary.json`
