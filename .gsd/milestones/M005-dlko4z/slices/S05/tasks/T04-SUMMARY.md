---
id: T04
parent: S05
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/source_asset_manifest.py
  - tests/test_source_asset_manifest.py
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests
key_decisions:
  - The S05 dry-run persists asset records as `linked_not_extracted`; it does not extract or serialize figures/tables/equations/references as raw payloads.
  - Source asset manifests now combine preserved source files with annotation-derived asset links in the same per-paper contract.
duration: 
verification_result: passed
completed_at: 2026-05-19T10:22:08.410Z
blocker_discovered: false
---

# T04: Ran the S05 source asset preservation dry-run with linked multimodal asset records.

**Ran the S05 source asset preservation dry-run with linked multimodal asset records.**

## What Happened

Ran the full source asset preservation and multimodal manifest dry-run over the 10-paper gold corpus. The dry-run preserves available normalized Markdown/PDF files with hashes and provenance, attaches S04 annotation-derived asset records using S03 source spans, writes per-paper manifests, writes run-level summaries, and writes package diagnostics. Final artifacts include 12 preserved source files, 283 linked asset records, asset counts by type, extraction-state counts, missing PDF diagnostics, and no-write/no-raw/no-embedding safety flags. All assets remain `linked_not_extracted`, `promoted_to_fact=false`, excluded from trusted KG import, and excluded from embedding generation.

## Verification

Fresh verification passed after artifact generation: source-asset, structure-aware, and import-contract tests passed; artifact files are non-empty; ruff passed; artifact guard confirmed 10 papers, 12 source files, 283 linked assets, expected asset counts, zero promoted facts, and all safety flags false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py && uv run python - <<'PY' ... source asset artifact guard ... PY` | 0 | ✅ pass — 53 passed; ruff all checks passed; artifact guard confirmed source_file_count=12, asset_count=283, promoted_to_fact_count=0, safety_flags_false=true | 5700ms |

## Deviations

T04 added a small reusable `build_source_asset_run()` helper so the dry-run is reproducible instead of relying on a one-off script. The dry-run still only writes redacted paths/hashes/provenance/linkage metadata; no asset binary extraction is performed.

## Known Issues

Eight original PDFs remain missing in current source paths. All 283 asset records are linked-not-extracted candidates; no figure/table/image extraction is claimed yet.

## Files Created/Modified

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests`
