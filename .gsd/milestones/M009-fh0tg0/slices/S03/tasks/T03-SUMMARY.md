---
id: T03
parent: S03
milestone: M009-fh0tg0
key_files:
  - .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json
  - .gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json
key_decisions:
  - S03 sample evidence includes a pure lineage mismatch where the artifact hash matches the recorded provenance but metadata fails expected active milestone lineage.
duration: 
verification_result: passed
completed_at: 2026-05-20T05:14:31.352Z
blocker_discovered: false
---

# T03: Generated lineage pass/mismatch sample reports and ran focused regression.

**Generated lineage pass/mismatch sample reports and ran focused regression.**

## What Happened

Generated S03 sample lineage reports. The pass report verifies a sample output with `milestone_id=M009-fh0tg0` and matching batch id as fresh. The mismatch report records an M006-style milestone value and fails with `artifact_metadata_mismatch`, demonstrating that the verifier can catch stale lineage even when hashes match the recorded output. Focused tests and ruff passed.

## Verification

Sample lineage reports exist. Provenance, scan workflow, and CLI scan tests passed; ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `generate lineage-pass-report.json and lineage-mismatch-report.json; uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_scan_workflow.py tests/test_validation_batch_cli_scan.py -q; uv run ruff check ...` | 0 | ✅ pass — pass=fresh; mismatch=stale/artifact_metadata_mismatch; 19 tests passed; ruff passed | 6500ms |

## Deviations

None.

## Known Issues

Sample evidence is synthetic. Real CLI scan provenance emission still needs future integration so verifier reports can be generated directly from real scan command logs.

## Files Created/Modified

- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json`
