# S03: Wave 3: refs 61-90

**Goal:** Acquire refs 61-90, continue BFS, check edge saturation.
**Demo:** 30 more PDFs (cumulative 110) with per-wave analysis.

## Must-Haves

- 30 new PDFs acquired
- 30 GROBID + 30 OpenDataLoader packets
- Wave 3 analysis with edge saturation
- Cumulative corpus: 110 PDFs (or unique count)
- 5+ tests pass
- 5 safety defaults stay false
- M045 trajectory on_track, M044 guardrail exit 0
- 1 commit in git history

## Proof Level

- This slice proves: operational

## Integration Closure

Continues BFS. Cumulative ~110 PDFs.

## Verification

- Wave 3 analysis, parser packets, edge saturation tracking.

## Tasks

- [x] **T01: Wave 3 refs 61-90 were acquired from arxiv.org with 30/30 successful PDFs.** `est:20m`
  Acquire refs 61-90 from /tmp/wave-order.json. Bounded retry. Output: artifacts/m056-bfs-graph/wave-3/acquisition-log.json.
  - Files: `artifacts/m056-bfs-graph/wave-3/acquisition-log.json`
  - Verify: test -f artifacts/m056-bfs-graph/wave-3/acquisition-log.json

- [x] **T02: Wave 3 PDFs were parsed through GROBID fulltext and OpenDataLoader.** `est:15m`
  Run GROBID /api/processFulltextDocument and OpenDataLoader on each of 30 Wave 3 PDFs. Output per-pdf JSON packets + summary.json.
  - Files: `artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json`, `artifacts/m056-bfs-graph/wave-3/opendataloader/summary.json`
  - Verify: test -f artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json

- [x] **T03: Wave 3 analysis, regression tests, trajectory check, and guardrail check passed.** `est:15m`
  scripts/analyze_m056_wave_3.py reads Wave 3 + Wave 1-2 + corpus. Emits analysis.md with edge saturation. tests/test_m056_wave_3.py with 5+ tests. Commit with feat(m056-bfs): S03 Wave 3 message.
  - Files: `artifacts/m056-bfs-graph/wave-3/analysis.md`, `tests/test_m056_wave_3.py`, `scripts/analyze_m056_wave_3.py`
  - Verify: uv run pytest tests/test_m056_wave_3.py -q

## Files Likely Touched

- artifacts/m056-bfs-graph/wave-3/acquisition-log.json
- artifacts/m056-bfs-graph/wave-3/grobid-fulltext/summary.json
- artifacts/m056-bfs-graph/wave-3/opendataloader/summary.json
- artifacts/m056-bfs-graph/wave-3/analysis.md
- tests/test_m056_wave_3.py
- scripts/analyze_m056_wave_3.py
