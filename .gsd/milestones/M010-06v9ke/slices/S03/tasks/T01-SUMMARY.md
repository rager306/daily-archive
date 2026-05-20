---
id: T01
parent: S03
milestone: M010-06v9ke
key_files:
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-diagnostics.jsonl
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/delta-report.json
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/outlier-report.json
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-response.json
key_decisions:
  - Scan the materialized S02 batch state only, not the original underfilled batch.
  - Use `--milestone-id M010-06v9ke` so scan artifacts carry active lineage.
duration: 
verification_result: passed
completed_at: 2026-05-20T07:25:38.502Z
blocker_discovered: false
---

# T01: Ran the active-lineage M010 scan: 10 papers, 1,477 chunks, 7 outliers, zero import-eligible chunks.

**Ran the active-lineage M010 scan: 10 papers, 1,477 chunks, 7 outliers, zero import-eligible chunks.**

## What Happened

Ran validation-batch scan over the materialized M010 source-ready batch state with active milestone lineage. The scan processed 10 papers, produced 1,477 chunks, 7 outliers, zero import-eligible chunks, structure-aware delta -354, and mixed benchmark delta -994. Scan artifacts include milestone_id=M010-06v9ke, milestone=M010-06v9ke, and batch_id=m010-next-plus-ten-materialized. No production import or LadybugDB writes occurred.

## Verification

Scan summary exists and confirms paper_count=10, milestone_id=M010-06v9ke, production_import_attempted=false, and ladybugdb_written=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `validation-batch scan --state-path .../source-ready-batch-state.json --milestone-id M010-06v9ke --json` | 0 | ✅ pass — chunk_count=1477; outlier_count=7; import_eligible_chunk_count=0; active lineage set | 6300ms |
| 2 | `test -s .../validation-scan-summary.json && uv run python - <<'PY' ... guard ... PY` | 0 | ✅ pass — active | 6700ms |

## Deviations

None.

## Known Issues

Scan produced 7 outliers and 0 import-eligible chunks. This remains operational scan evidence, not semantic KG readiness.

## Files Created/Modified

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-response.json`
