# S02: Visualization + 2-hop BFS preview algorithm + close M060b

**Goal:** Build visualization script (NetworkX + matplotlib PNG) + 2-hop BFS preview algorithm + close M060b.
**Demo:** PNG visualization of 4-layer graph, 2-hop BFS preview report (estimated scale for M061), M060b closes, next-gate for M061 documented

## Must-Haves

- Visualization PNG emitted (artifacts/m060b-graph/graph-viz.png)
- 2-hop BFS preview emitted (artifacts/m060b-graph/two-hop-preview.json)
- REPORT.md (Russian) with 4 sections
- M060b closeout artifacts (SUMMARY + VALIDATION)
- 5+ tests pass
- 5 safety defaults stay false
- M045 on_track, M044 ok
- 1 commit in git history
- code-memory synced

## Proof Level

- This slice proves: operational

## Integration Closure

Closes M060b with full graph layer established. Provides 2-hop BFS scale estimation for M061 planning.

## Verification

- M060b closeout + next-gate for M061.

## Tasks

- [x] **T01: Built M060b graph visualization PNG and directed 2-hop BFS preview artifacts.** `est:60m`
  Step 1: scripts/m060b_graph_visualize.py:
  - Usage: uv run python scripts/m060b_graph_visualize.py --manifest=X --output=PNG
  - Load M058 4-layer manifest
  - Build NetworkX DiGraph
  - Render with matplotlib (spring_layout):
    - Color by layer: citation=blue, table_similarity=green, figure_similarity_v1=orange, figure_similarity_v2=red
    - Node size by degree
    - Edge alpha by similarity (or 0.3 default)
    - Subsample to 200 nodes max for readability (top-degree nodes)
  - Output: artifacts/m060b-graph/graph-viz.png
  - 5 safety defaults explicit
  - 127.0.0.1 NOT localhost
  - Files: `scripts/m060b_graph_visualize.py`, `scripts/m060b_two_hop_preview.py`, `artifacts/m060b-graph/graph-viz.png`, `artifacts/m060b-graph/two-hop-preview.json`, `artifacts/m060b-graph/REPORT.md`, `tests/test_m060b_s02.py`
  - Verify: test -f artifacts/m060b-graph/graph-viz.png

- [x] **T02: Added S02 pytest coverage, Russian REPORT.md, M045/M044 verification, and code-memory mirror sync.** `est:15m`
  tests/test_m060b_s02.py with 5+ tests:
  1. test_visualize_runs
  2. test_png_file_exists
  3. test_two_hop_preview_runs
  4. test_two_hop_anchor_2605.18747
  5. test_5_safety_defaults
  6. M050-M063-S01 regression
  - Files: `tests/test_m060b_s02.py`
  - Verify: uv run pytest tests/test_m060b_s02.py -q

## Files Likely Touched

- scripts/m060b_graph_visualize.py
- scripts/m060b_two_hop_preview.py
- artifacts/m060b-graph/graph-viz.png
- artifacts/m060b-graph/two-hop-preview.json
- artifacts/m060b-graph/REPORT.md
- tests/test_m060b_s02.py
