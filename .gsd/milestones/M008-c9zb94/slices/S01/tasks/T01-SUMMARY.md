---
id: T01
parent: S01
milestone: M008-c9zb94
key_files:
  - .gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json
key_decisions:
  - Use local research/cache inventory and exclude all M006 IDs before selection.
  - Candidate inventory stores path/status metadata only, not raw paper text.
duration: 
verification_result: passed
completed_at: 2026-05-20T02:20:29.209Z
blocker_discovered: false
---

# T01: Built a redacted candidate inventory with 800 non-M006 candidates.

**Built a redacted candidate inventory with 800 non-M006 candidates.**

## What Happened

Built the candidate inventory for the first new +10 batch. The inventory excludes all 30 M006 paper IDs and records 800 local/cache candidate IDs with redacted source path availability. Only path/status metadata is included. The inventory shows 2 candidates with existing Markdown and 1 with cached PDF, which means the new +10 batch will realistically test source preflight and bounded acquisition paths.

## Verification

Candidate inventory exists, has at least 10 candidates, and raw_text_included is false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json && uv run python - <<'PY' ... guard ... PY` | 0 | ✅ pass — candidate-inventory-ok | 7500ms |

## Deviations

None.

## Known Issues

Only 2 candidates already have Markdown and 1 has a cached PDF, so S02 likely needs bounded source acquisition/repair for the selected +10.

## Files Created/Modified

- `.gsd/milestones/M008-c9zb94/slices/S01/run-evidence/new-plus-ten-candidate-inventory.json`
