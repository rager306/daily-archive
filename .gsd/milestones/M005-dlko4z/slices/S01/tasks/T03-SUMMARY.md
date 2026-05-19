---
id: T03
parent: S01
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/chunk_import_contract.py
  - tests/test_chunk_import_contract.py
key_decisions:
  - The validator distinguishes structural package validity from import eligibility; retrieval-only packages can be valid while having zero import-eligible chunks.
  - Contract diagnostics use stable refusal reason names from the T01 contract.
  - Annotations with `promoted_to_fact=true` are rejected in M005.
duration: 
verification_result: passed
completed_at: 2026-05-19T05:09:55.061Z
blocker_discovered: false
---

# T03: Implemented executable import-ready chunk contract validation fixtures.

**Implemented executable import-ready chunk contract validation fixtures.**

## What Happened

Added `chunk_import_contract.py` with structured diagnostics and validation for the M005 import-ready chunk package. The validator checks schema/contract versions, redaction leakage, graph-ready source spans, evidence path resolution, parent element resolution, invalid import states, route pollution, annotation fact promotion, and serialization safety. Tests cover a valid package, missing IDs/spans/references, retrieval-only non-importability, raw text/embedding/vector leakage, annotation promotion, claim-route pollution, and a valid retrieval-only package with zero import-eligible chunks.

## Verification

`uv run pytest tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_import_contract.py tests/test_chunk_import_contract.py` passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_import_contract.py tests/test_chunk_import_contract.py` | 0 | ✅ pass — 11 passed; ruff all checks passed | 4000ms |

## Deviations

Implemented the S01 validator as dict-based contract validation rather than dataclasses so S02/S03 exporters can validate JSON-like artifacts before the final model shape stabilizes.

## Known Issues

The validator currently validates synthetic fixtures and contract invariants only. It does not yet export or measure real paper chunks; that is S02/S03 scope.

## Files Created/Modified

- `src/arxiv_archive/chunk_import_contract.py`
- `tests/test_chunk_import_contract.py`
