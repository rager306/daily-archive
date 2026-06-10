# S02: Wave 2: refs 31-60

**Goal:** Acquire refs 31-60 from /tmp/wave-order.json, run GROBID + OpenDataLoader, per-wave analysis with edge saturation check.
**Demo:** 30 more PDFs (cumulative 80) with per-wave analysis.

## Must-Haves

- 30 new PDFs acquired
- 30 GROBID + 30 OpenDataLoader packets
- Wave 2 analysis with edge saturation check
- Cumulative corpus: 80 PDFs
- 5+ tests pass
- 5 safety defaults stay false
- M045 trajectory on_track, M044 guardrail exit 0
- 1 commit in git history

## Proof Level

- This slice proves: operational

## Integration Closure

Continues Wave 1 BFS. Cumulative 80 PDFs.

## Verification

- Wave 2 acquisition log, parser packets, analysis report.

## Tasks

- [x] **T01: Wave 2 acquisition collected 30/30 requested arXiv PDFs for refs 31-60.** `est:20m`
  Reuse scripts/acquire_m056_wave.py with --wave-number 2 (or hardcoded IDs 31-60 from /tmp/wave-order.json). Skip self. Acquire 30 PDFs with bounded retry. Output: artifacts/m056-bfs-graph/wave-2/acquisition-log.json. Accept 25/30 minimum.
  - Files: `artifacts/m056-bfs-graph/wave-2/acquisition-log.json`
  - Verify: test -f artifacts/m056-bfs-graph/wave-2/acquisition-log.json

- [x] **T02: Wave 2 parser runs produced 30 GROBID packets and 30 OpenDataLoader packets.** `est:15m`
  Run GROBID /api/processFulltextDocument and OpenDataLoader on each of 30 Wave 2 PDFs. Output per-pdf JSON packets + summary.json. Use existing scripts/benchmark_m055deep_grobid_fulltext.py and scripts/benchmark_m055_opendataloader_only.py.
  - Files: `artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json`, `artifacts/m056-bfs-graph/wave-2/opendataloader/summary.json`
  - Verify: test -f artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json

- [x] **T03: Wave 2 analysis, tests, regression checks, trajectory check, and guardrail check all passed.** `est:15m`
  scripts/analyze_m056_wave_2.py reads:
  - Wave 2 acquisition log
  - 30 GROBID + 30 OpenDataLoader Wave 2 packets
  - Wave 1 packets (cumulative)
  - 20-PDF existing corpus
  - 2605.18747 anchor TEI
  Emits artifacts/m056-bfs-graph/wave-2/analysis.md with:
  - Connectivity gain delta vs Wave 1 (saturation check)
  - New edges added this wave
  - Cumulative edges
  - Per-wave edge rate (decreasing = saturation)
  - Self-citation cluster
  - Category/length distribution
  - Parser quality
  - Files: `artifacts/m056-bfs-graph/wave-2/analysis.md`, `tests/test_m056_wave_2.py`, `scripts/analyze_m056_wave_2.py`
  - Verify: uv run pytest tests/test_m056_wave_2.py -q

## Files Likely Touched

- artifacts/m056-bfs-graph/wave-2/acquisition-log.json
- artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json
- artifacts/m056-bfs-graph/wave-2/opendataloader/summary.json
- artifacts/m056-bfs-graph/wave-2/analysis.md
- tests/test_m056_wave_2.py
- scripts/analyze_m056_wave_2.py
