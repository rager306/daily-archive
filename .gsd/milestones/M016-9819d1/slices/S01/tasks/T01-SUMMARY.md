---
id: T01
parent: S01
milestone: M016-9819d1
key_files:
  - .gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md
  - .gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json
key_decisions:
  - Use 9router endpoint order as authoritative correction for the next probe.
  - Do not use X-Api-Key for MiniMax usage path; 9router uses Authorization Bearer only.
  - Treat token_plan counts as used counts and coding_plan counts as remaining counts.
duration: 
verification_result: passed
completed_at: 2026-05-20T12:37:54.027Z
blocker_discovered: false
---

# T01: Documented 9router’s MiniMax usage algorithm and found the exact M015 endpoint omission.

**Documented 9router’s MiniMax usage algorithm and found the exact M015 endpoint omission.**

## What Happened

Cloned 9router into `/root/vendor-source/9router`, indexed it as GitNexus repo `9router`, and extracted the MiniMax usage implementation from `open-sse/services/usage.js` plus unit-test behavior from `tests/unit/minimax-usage.test.js`. The report identifies the missed M015 endpoint: `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`, the GET Bearer header pattern, base_resp status handling, model_remains parsing, and token_plan vs coding_plan count semantics.

## Verification

9router-minimax-usage-summary-ok passed and asserts the GitNexus repo, missed endpoint, and base_resp success requirement.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus analyze /root/vendor-source/9router --name 9router` | 0 | ✅ pass — 18,040 nodes / 28,116 edges / 300 flows | 16200ms |
| 2 | `uv run python JSON assertions for 9router summary` | 0 | ✅ pass — 9router-minimax-usage-summary-ok | 7900ms |

## Deviations

None.

## Known Issues

This slice documents the algorithm; it does not yet prove the corrected endpoint order works with the available key.

## Files Created/Modified

- `.gsd/milestones/M016-9819d1/slices/S01/9router-minimax-usage-report.md`
- `.gsd/milestones/M016-9819d1/slices/S01/run-evidence/9router-minimax-usage-summary.json`
