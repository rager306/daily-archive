---
id: T03
parent: S07
milestone: M056-lchpnp
key_files:
  - doc/adr/ADR-010-bfs-scale-167-pdf.md
  - doc/adr/ADR-INDEX.md
  - .gsd/DECISIONS.md
key_decisions:
  - D084: 1-hop saturation observed; 2-hop BFS recommended for M058 graph-readiness.
duration: 
verification_result: passed
completed_at: 2026-06-10T15:07:58.686Z
blocker_discovered: false
---

# T03: Added ADR-010 and recorded GSD decision D084 for BFS scale evidence.

**Added ADR-010 and recorded GSD decision D084 for BFS scale evidence.**

## What Happened

Drafted `doc/adr/ADR-010-bfs-scale-167-pdf.md` as an Accepted binding supplement to ADR-009, documenting M056's 149-PDF 1-hop BFS saturation evidence and recommending 2-hop BFS or an alternative anchor for M058. Updated `doc/adr/ADR-INDEX.md` and saved the corresponding GSD decision as D084.

## Verification

`tests/test_m056_final_s07.py` verified ADR-010 exists, references M056, includes the 2605.18747 anchor, 149 unique PDFs, 7-8 target-set edges, M058 recommendation, and ADR index reference. `gsd_decision_save` returned D084.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_m056_final_s07.py -q` | 0 | ✅ pass | 13800ms |
| 2 | `gsd_decision_save(scope='parser-benchmark / graph-readiness', decision='BFS scale evidence for M056 1-hop expansion from 2605.18747', choice='1-hop saturation observed; 2-hop BFS recommended for M058 graph-readiness')` | 0 | ✅ pass | 0ms |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `doc/adr/ADR-010-bfs-scale-167-pdf.md`
- `doc/adr/ADR-INDEX.md`
- `.gsd/DECISIONS.md`
