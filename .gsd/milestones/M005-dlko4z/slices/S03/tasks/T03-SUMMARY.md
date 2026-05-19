---
id: T03
parent: S03
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - tests/test_structure_aware_chunking.py
key_decisions:
  - Structure-aware chunks now receive conservative routes and states from structural element classes, but no generated chunk receives `trusted_kg_import` permission in S03/T03.
  - Administrative/navigation blocks route to `metadata_graph` with repair-required state instead of polluting claim routes.
  - Figure and equation contexts remain retrieval-only or repair-required because the S01 contract has no dedicated figure/equation extraction route yet.
duration: 
verification_result: passed
completed_at: 2026-05-19T07:01:48.585Z
blocker_discovered: false
---

# T03: Assigned conservative routes, states, chunk types, and refusal reasons to structure-aware chunks.

**Assigned conservative routes, states, chunk types, and refusal reasons to structure-aware chunks.**

## What Happened

Added deterministic route, state, chunk type, allowed/excluded use, and refusal reason assignment from structural element classes. The parser now converts parsed elements into chunks with conservative routing for claim, method, citation, table, metadata, figure, equation, and retrieval-only contexts. Package diagnostics include counts by route, state, chunk type, and refusal reason. All generated chunks remain non-importable pending later validation; this keeps the KG import boundary intact while making route distributions measurable.

## Verification

T03 verification passed with structure-aware tests plus import-contract tests and ruff clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 28 passed; ruff all checks passed | 5700ms |

## Deviations

None.

## Known Issues

The structure-aware packages still lack evidence paths and gold-corpus run artifacts. T04 must validate package outputs over the gold corpus and write redacted diagnostics.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
