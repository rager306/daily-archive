---
id: S01
parent: M015-ktorc7
milestone: M015-ktorc7
provides:
  - Corrected Token Plan access verdict
requires:
  []
affects:
  - S03
key_files:
  - .gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json
key_decisions:
  - HTTP 200 is not enough for remains success; base_resp.status_code must be 0 and useful remains fields must exist.
  - Current reliable method to check limits is Billing > Token Plan UI.
  - Programmatic remains checking is not verified with available key material.
patterns_established:
  - Classify MiniMax endpoint success by base_resp/status and useful fields, not HTTP status alone.
  - Compare key hashes to detect when secure-collected key is not actually distinct.
observability_surfaces:
  - access matrix
  - access guard
  - remediation report
drill_down_paths:
  - .gsd/milestones/M015-ktorc7/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M015-ktorc7/slices/S01/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-20T12:17:16.562Z
blocker_discovered: false
---

# S01: Token Plan remains access remediation

**S01 corrected Token Plan limits debugging: UI path is reliable; API remains is not verified with available key material.**

## What Happened

S01 reran Token Plan limit debugging properly. It collected a Token Plan key securely, discovered it matched the existing API key, and ran a 32-row matrix across keys, hosts, endpoint paths, methods, and header modes. No true remains success was found; all apparent HTTP 200 JSON successes were base_resp-only with non-zero MiniMax status codes. The reliable current answer is Billing > Token Plan UI; API remains requires a distinct authorized Token Plan Key or session-supported endpoint and is not verified here.

## Verification

token-plan-access-matrix-ok and token-plan-access-guard-ok passed.

## Requirements Advanced

- R043 — S01 covers the limit-check remediation portion of R043, with a precise limitation rather than false acceptance.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The secure-collected MINIMAX_TOKEN_PLAN_KEY was not distinct from MINIMAX_API_KEY, so S01 could not prove behavior for a separate Token Plan Key. The matrix also corrected an initial false-positive classification of HTTP 200 base_resp-only responses.

## Known Limitations

Programmatic current quota/remains is still not obtained. This is now a precise limitation, not an under-debugged single-call result.

## Follow-ups

If exact API remains is still required, collect a distinct Token Plan Key from Billing > Token Plan and rerun the matrix; if still failing, treat remains as UI/session-only and use vendor support/docs.

## Files Created/Modified

- `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-matrix.json` — Access matrix.
- `.gsd/milestones/M015-ktorc7/slices/S01/run-evidence/token-plan-access-guard.json` — Access guard.
- `.gsd/milestones/M015-ktorc7/slices/S01/token-plan-access-remediation.md` — Remediation report.
