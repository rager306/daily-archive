---
id: T01
parent: S07
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/import_boundary_rehearsal.py
  - tests/test_import_boundary_rehearsal.py
key_decisions:
  - The S07 import rehearsal contract is explicitly negative: it proves current candidates are rejected safely rather than authorizing positive KG import.
  - Redaction validation runs once at the rehearsal artifact level to avoid duplicate nested leakage diagnostics while still validating per-candidate safety and import semantics.
duration: 
verification_result: passed
completed_at: 2026-05-19T12:17:38.574Z
blocker_discovered: false
---

# T01: Defined the negative import rehearsal contract and validator for S07.

**Defined the negative import rehearsal contract and validator for S07.**

## What Happened

Defined the M005/S07 negative import-boundary rehearsal contract in a new module. The contract serializes redacted candidate identities, package/method ids, accepted/rejected counts, refusal counts, no-write/no-import safety flags, remediation hints, and caveats. The validator enforces required fields, count consistency, refusal-count consistency, schema version, nested forbidden-field detection, unsafe write flags, and trusted-import exclusion. Tests cover valid negative rehearsal artifacts, count mismatches, unsafe positive-import settings, unsafe write flags, nested raw/embedding/vector/secret/optimizer leakage, and rejected candidates without refusal reasons.

## Verification

Fresh verification passed: 72 focused tests passed and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "_validate_candidate", direction: "upstream", repo: "daily-archive"})` | 0 | ℹ️ target not found because helper is new/unindexed; change confined to new module/tests | 0ms |
| 2 | `uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py` | 0 | ✅ pass — 72 passed; ruff all checks passed | 3400ms |

## Deviations

None.

## Known Issues

The contract currently validates hand-built rehearsal candidates only. T02 still needs adapters from S06/S05/S04/S03 artifacts.

## Files Created/Modified

- `src/arxiv_archive/import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`
