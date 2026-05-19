---
id: T02
parent: S04
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - tests/test_structure_aware_chunking.py
key_decisions:
  - Sidecars are generated solely from existing chunk metadata, section paths, routes, types, states, and warning/refusal codes; no raw chunk text is inspected or persisted.
  - Table and figure chunks receive `asset_link_hint` sidecars that require the future S05 asset manifest before multimodal use.
duration: 
verification_result: passed
completed_at: 2026-05-19T08:18:10.773Z
blocker_discovered: false
---

# T02: Generated deterministic annotation sidecars from chunk metadata and added annotation diagnostics.

**Generated deterministic annotation sidecars from chunk metadata and added annotation diagnostics.**

## What Happened

Implemented deterministic annotation generation from structure-aware chunk metadata. Each chunk receives section-role, route-hint, and structural-type sidecars; chunks with refusal/review requirements receive review-blocker sidecars; table and figure chunks receive asset-link hints for the future source-asset manifest. Package diagnostics and run summaries now include annotation counts by type, confidence class, and warning code. Tests confirm sidecars are generated from metadata, remain redacted, keep `promoted_to_fact=false`, and appear in summary/diagnostic artifacts without raw text.

## Verification

Focused structure-aware tests plus import-contract tests passed; ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 34 passed; ruff all checks passed | 8000ms |

## Deviations

None.

## Known Issues

S04/T02 generates sidecars, but T03 still needs explicit negative tests for annotation contract boundaries such as unresolved chunks, promoted facts, and raw-text leakage.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
