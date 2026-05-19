---
id: T02
parent: S07
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/import_boundary_rehearsal.py
  - tests/test_import_boundary_rehearsal.py
key_decisions:
  - Use aggregate-refusal expansion for negative rehearsal candidates so rejected_count can match S06 total refused count without loading raw paper files.
  - Preserve S06 missing-source caveats in the rehearsal contract as caveats rather than fatal adapter errors.
duration: 
verification_result: passed
completed_at: 2026-05-19T12:22:06.614Z
blocker_discovered: false
---

# T02: Built S07 rehearsal candidates from S06 benchmark artifacts without raw-content access or graph writes.

**Built S07 rehearsal candidates from S06 benchmark artifacts without raw-content access or graph writes.**

## What Happened

Implemented `build_import_boundary_rehearsal_from_benchmark()` to consume S06 benchmark summary and diagnostics. The adapter reads only redacted JSON/JSONL artifacts, expands method-level refusal counts into 2,471 rejected import-boundary candidates, preserves method ids and refusal reasons, includes remediation hints, carries benchmark summary totals, and keeps missing-source caveats visible. Tests assert the current artifacts produce a valid rehearsal contract with zero accepted candidates, 2,471 rejected candidates, zero import eligibility, trusted KG import excluded for every candidate, and `missing_original_pdf:16` preserved as a caveat.

## Verification

Fresh verification passed: 74 focused tests passed and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "ImportBoundaryRehearsal", direction: "upstream", repo: "daily-archive"})` | 0 | ℹ️ target not found because new module is not indexed yet; changes confined to S07 module/tests | 0ms |
| 2 | `uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py` | 0 | ✅ pass — 74 passed; ruff all checks passed | 3700ms |

## Deviations

S06 diagnostics are aggregate and redacted, not per-chunk raw records. T02 therefore expands aggregate refusal counts into synthetic redacted candidate identities by method/refusal reason, preserving count equality without inventing raw content.

## Known Issues

Candidates are redacted synthetic import-boundary identities derived from aggregate benchmark diagnostics, not source chunk payloads. This is appropriate for negative boundary proof but not sufficient for future positive import rehearsal.

## Files Created/Modified

- `src/arxiv_archive/import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`
