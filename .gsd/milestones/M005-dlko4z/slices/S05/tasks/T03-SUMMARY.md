---
id: T03
parent: S05
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/source_asset_manifest.py
  - tests/test_source_asset_manifest.py
key_decisions:
  - Asset-link records are generated from S04 annotation diagnostics and S03 source spans, but remain `linked_not_extracted` and non-factual.
  - Only table, figure, equation, reference/citation, and metadata/admin chunk types/routes create asset records; claim/method prose remains excluded from asset linkage.
duration: 
verification_result: passed
completed_at: 2026-05-19T10:17:44.230Z
blocker_discovered: false
---

# T03: Linked annotation sidecar diagnostics to redacted non-fact asset records.

**Linked annotation sidecar diagnostics to redacted non-fact asset records.**

## What Happened

Added source-asset linkage from S04 annotation sidecar diagnostics. The new `attach_annotation_asset_links()` path reads per-chunk annotation coverage, optionally merges S03 source spans, maps table/figure/equation/reference/metadata chunks to `AssetRecord` entries, binds them to preserved normalized Markdown when available, and preserves `promoted_to_fact=false` plus import/write/embedding exclusions. Tests cover table/figure linkage with spans, reference/equation/metadata counts, missing source spans, missing preserved source files, redaction, and non-import boundaries. A real guard over the current S04/S03/S05 artifacts produced 283 valid linked asset records across 10 manifests without promoting facts.

## Verification

Fresh verification passed: source-asset, structure-aware, and import-contract tests passed; ruff reported all checks passed; a real-diagnostics guard produced 283 valid linked asset records with zero promoted facts.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py` | 0 | ✅ pass — 52 passed; ruff all checks passed | 4500ms |
| 2 | `uv run python - <<'PY' ... attach_annotation_asset_links guard over S04/S03/S05 artifacts ... PY` | 0 | ✅ pass — manifest_count=10, asset_count=283, asset_counts_by_type={equation:146, figure:86, metadata:2, reference:11, table:38}, valid_manifests=true, promoted_to_fact_count=0 | 0ms |

## Deviations

T03 did not persist new run artifacts; it added linkage behavior and tests, plus a real-diagnostics guard. The planned persisted S05 dry-run artifacts remain for T04.

## Known Issues

The linkage currently creates redacted candidate records only; it does not extract figure/table image files or table contents. T04 will persist the linked manifests/dry-run artifacts, and T05 will review them.

## Files Created/Modified

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`
