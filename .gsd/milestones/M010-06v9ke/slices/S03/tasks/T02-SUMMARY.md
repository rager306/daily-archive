---
id: T02
parent: S03
milestone: M010-06v9ke
key_files:
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-provenance.jsonl
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-response.json
  - .gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-run-id.txt
key_decisions:
  - Keep the stale m010-s03-scan-001 evidence as diagnostic history but use m010-s03-scan-002 as the valid run id.
  - Metadata freshness verification should cover metadata-bearing JSON outputs; raw diagnostic JSONL is not suitable for expected artifact metadata checks.
duration: 
verification_result: passed
completed_at: 2026-05-20T07:30:09.387Z
blocker_discovered: false
---

# T02: Recorded real scan provenance and verified the corrected M010 scan artifacts as fresh.

**Recorded real scan provenance and verified the corrected M010 scan artifacts as fresh.**

## What Happened

Created a real scan provenance entry and ran verify-artifacts. The first attempt correctly failed stale because metadata expectations were applied to non-JSONL/response artifacts. A corrected provenance entry m010-s03-scan-002 records the source-ready batch state and baseline inputs plus five metadata-bearing JSON outputs: scan summary, delta report, outlier report, scan manifest, and source readiness. verify-artifacts returned fresh with zero diagnostics, zero mismatches, and zero missing outputs.

## Verification

scan-freshness-report.json exists and has verdict=fresh for run_id=m010-s03-scan-002.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `build_validation_cli_provenance_entry(... run_id='m010-s03-scan-002' ...) + validation-batch verify-artifacts` | 0 | ✅ pass — verdict=fresh; diagnostics=0; mismatch_count=0; missing_count=0 | 3600ms |
| 2 | `test -s .../scan-freshness-report.json && uv run python - <<'PY' ... guard ... PY` | 0 | ✅ pass — scan-provenance-fresh-ok | 3100ms |

## Deviations

The first provenance entry included JSONL diagnostics and scan-response wrapper outputs under expected metadata checks, producing a stale verifier result. A corrected real provenance entry m010-s03-scan-002 records only metadata-bearing JSON scan artifacts, and verify-artifacts returns fresh.

## Known Issues

The provenance log contains one stale diagnostic entry followed by the valid fresh entry. S03 final guard must select m010-s03-scan-002.

## Files Created/Modified

- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-provenance.jsonl`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-report.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-freshness-response.json`
- `.gsd/milestones/M010-06v9ke/slices/S03/run-evidence/scan-run-id.txt`
