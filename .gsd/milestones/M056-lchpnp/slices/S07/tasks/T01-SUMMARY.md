---
id: T01
parent: S07
milestone: M056-lchpnp
key_files:
  - scripts/emit_m056_candidate_edges.py
  - artifacts/m056-bfs-graph/candidate-edges.json
key_decisions:
  - Preserve all extracted GROBID arXiv citation candidates while marking `in_corpus` so downstream gates can filter without losing evidence.
duration: 
verification_result: passed
completed_at: 2026-06-10T15:07:38.649Z
blocker_discovered: false
---

# T01: Emitted diagnostic M056 candidate citation graph JSON from six wave GROBID TEI packets.

**Emitted diagnostic M056 candidate citation graph JSON from six wave GROBID TEI packets.**

## What Happened

Implemented `scripts/emit_m056_candidate_edges.py` as a stdlib-only, deterministic diagnostic emitter. It reads anchor and wave GROBID TEI files, extracts arXiv IDs from `listBibl/biblStruct`, marks corpus membership, preserves all safety defaults false, and writes `artifacts/m056-bfs-graph/candidate-edges.json` without graph writes or production import.

## Verification

Ran `uv run python scripts/emit_m056_candidate_edges.py`, producing `candidate-edges.json` with 2448 nodes, 3983 citation edges, and 427 corpus-internal diagnostic edges. Subsequent pytest schema/idempotency checks passed in `tests/test_m056_final_s07.py`.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/emit_m056_candidate_edges.py && uv run python scripts/render_m056_report.py` | 0 | ✅ pass | 47900ms |
| 2 | `uv run pytest tests/test_m056_final_s07.py -q` | 0 | ✅ pass | 13800ms |

## Deviations

Included broad external citation candidates as well as corpus membership flags; the report separately distinguishes this full diagnostic graph from the narrower 7-8 target-set saturation metric.

## Known Issues

None.

## Files Created/Modified

- `scripts/emit_m056_candidate_edges.py`
- `artifacts/m056-bfs-graph/candidate-edges.json`
