---
id: T01
parent: S05
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/source_asset_manifest.py
  - tests/test_source_asset_manifest.py
key_decisions:
  - Source/asset manifests serialize path, hash, provenance, span, linkage, extraction-state, and safety metadata only; raw content and binary payloads are forbidden.
  - Assets are explicitly excluded from trusted KG import and embedding generation and remain non-facts via `promoted_to_fact=false`.
duration: 
verification_result: passed
completed_at: 2026-05-19T09:01:02.200Z
blocker_discovered: false
---

# T01: Defined the redacted source asset manifest contract and validator.

**Defined the redacted source asset manifest contract and validator.**

## What Happened

Created the S05 source asset manifest contract with dataclasses for source spans, preserved source files, asset records, and per-paper source asset manifests. Added a validator that checks required fields, SHA-256 metadata, source-file references, source spans, non-fact/import-exclusion boundaries, safety flags, and recursive forbidden-field leakage without echoing raw values. Tests cover redaction, hash metadata, required fields, unresolved references, promoted assets, import-allowed assets, nested raw/base64/embedding leakage, and unsafe diagnostics.

## Verification

Fresh verification after the final edit passed: source-asset manifest tests plus structure-aware/import-contract tests passed, and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/source_asset_manifest.py tests/test_source_asset_manifest.py` | 0 | ✅ pass — 47 passed; ruff all checks passed | 5900ms |

## Deviations

None.

## Known Issues

T01 defines and validates the contract only. Actual source file preservation, sidecar-to-asset linkage, and gold-corpus dry-run artifacts remain for T02-T04.

## Files Created/Modified

- `src/arxiv_archive/source_asset_manifest.py`
- `tests/test_source_asset_manifest.py`
