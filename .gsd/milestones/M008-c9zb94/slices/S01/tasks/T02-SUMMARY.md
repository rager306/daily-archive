---
id: T02
parent: S01
milestone: M008-c9zb94
key_files:
  - .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json
  - .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md
key_decisions:
  - Select the first 10 lexicographically sorted local/cache candidate IDs after excluding all M006 IDs.
  - Do not bias selection only toward Markdown-ready candidates; this is intended to test source preflight/acquisition on genuinely new papers.
duration: 
verification_result: passed
completed_at: 2026-05-20T02:21:19.292Z
blocker_discovered: false
---

# T02: Selected the deterministic first new +10 corpus manifest.

**Selected the deterministic first new +10 corpus manifest.**

## What Happened

Selected the first new +10 corpus using a deterministic lexicographic rule over the redacted candidate inventory after excluding all M006 papers. The selected IDs are `1701.00001`, `2001.00234v1`, `2001.00236v1`, `2001.00238v2`, `2001.00248v2`, `2001.00254v1`, `2001.00258v2`, `2001.00265v1`, `2001.00267v1`, and `2001.00271v1`. The manifest is compatible with validation-batch init and contains source path/status metadata only.

## Verification

Manifest exists, has exactly 10 unique paper IDs, and raw_text_included is false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json && uv run python - <<'PY' ... guard ... PY` | 0 | ✅ pass — new-plus-ten-manifest-ok | 8100ms |

## Deviations

None.

## Known Issues

Only 1/10 selected papers is already Markdown-ready in the local preview. S02 must either acquire/repair sources boundedly or block the batch before scan.

## Files Created/Modified

- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-corpus-manifest.json`
- `.gsd/milestones/M008-c9zb94/slices/S01/new-plus-ten-selection-rationale.md`
