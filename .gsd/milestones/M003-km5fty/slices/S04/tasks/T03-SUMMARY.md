---
id: T03
parent: S04
milestone: M003-km5fty
key_files:
  - src/arxiv_archive/scientific_extraction.py
  - tests/test_scientific_extraction_contracts.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-17T17:54:46.030Z
blocker_discovered: false
---

# T03: Added relation and extraction patch validator coverage for endpoints, unsupported types, duplicate IDs, mismatches, and evidence warnings.

**Added relation and extraction patch validator coverage for endpoints, unsupported types, duplicate IDs, mismatches, and evidence warnings.**

## What Happened

Expanded S04 relation and extraction patch validation. Added helper fixture builders in tests and new coverage for unsupported relation types, duplicate draft IDs across claims/entities/relations, invalid relation endpoints, paper mismatches, and EvidencePath validation warnings propagated through Claim, ScientificEntity, and ScientificRelation diagnostics. Updated validate_extraction_patch to detect duplicate draft IDs while preserving endpoint validation against all unique claim/entity IDs.

## Verification

`uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py -q` passed with 21 tests. `uv run ruff check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` passed with all checks clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py -q` | 0 | ✅ pass: 21 passed | 3400ms |
| 2 | `uv run ruff check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` | 0 | ✅ pass: All checks passed | 4000ms |

## Deviations

None.

## Known Issues

None for T03. Final T04 still needs full slice regression smoke and closeout limitations recorded.

## Files Created/Modified

- `src/arxiv_archive/scientific_extraction.py`
- `tests/test_scientific_extraction_contracts.py`
