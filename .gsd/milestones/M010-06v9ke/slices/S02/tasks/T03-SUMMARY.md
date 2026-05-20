---
id: T03
parent: S02
milestone: M010-06v9ke
key_files:
  - .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/materialized-source-ready-manifest.json
  - .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json
  - .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-preflight-summary.json
  - .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json
  - .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/top-up-summary.json
  - .gsd/milestones/M010-06v9ke/slices/S02/source-readiness-report.md
key_decisions:
  - Drop the two original conversion-failed papers from the scan batch and replace them with the first two acquired deterministic replacements.
  - Treat top-up planning as insufficient until replacements are materialized into a new batch state and preflighted.
  - Use the materialized source-ready batch state as the only S03 scan input.
duration: 
verification_result: passed
completed_at: 2026-05-20T07:21:49.653Z
blocker_discovered: false
---

# T03: Materialized two acquired replacements and produced a final 10/10 source-ready M010 batch state.

**Materialized two acquired replacements and produced a final 10/10 source-ready M010 batch state.**

## What Happened

Reran final preflight after original acquisition and confirmed only 8/10 original papers were ready. The initial top-up plan had no already-ready replacements, so bounded replacement acquisition attempted the next 20 deterministic candidates and acquired 16 Markdown files. The first two acquired replacements, 2002.05505v6 and 2405.08246v1, replaced the two failed originals, 2001.00575v1 and 2001.00817v1. A new materialized batch was initialized and preflighted to 10/10 Markdown-ready. Quota-fill now reports accepted_ready_count=10, shortage_count=0, scan_allowed=true, with no production import or LadybugDB writes.

## Verification

Quota-fill summary exists and confirms target_count=10, accepted_ready_count=10, shortage_count=0, and raw_text_included=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `final preflight + bounded replacement acquisition + materialized batch init/preflight + quota/top-up reports` | 0 | ✅ pass — final_preflight_ready=10; quota_ready=10; top_up_final_ready=10; scan_allowed=true | 54500ms |
| 2 | `test -s .../quota-fill-summary.json && uv run python - <<'PY' ... guard ... PY` | 0 | ✅ pass — quota-gate-ok | 8500ms |

## Deviations

T03 expanded beyond the initial top-up plan: because the first top-up report found no preexisting ready replacements, it performed bounded replacement acquisition over the next 20 deterministic candidates, then materialized the first 2 acquired replacements into a final source-ready batch state.

## Known Issues

PDF availability remains 0/10. Source acquisition summaries still carry legacy `milestone: M006-638rza` from the reused helper, so S03 must rely on active scan lineage/provenance for scan artifacts.

## Files Created/Modified

- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/materialized-source-ready-manifest.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/top-up-summary.json`
- `.gsd/milestones/M010-06v9ke/slices/S02/source-readiness-report.md`
