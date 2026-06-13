---
estimated_steps: 18
estimated_files: 5
skills_used: []
---

# T01: Built the M060c S02 applicability matrix, ADR-016, and M061-M065 decision document.

Step 1: scripts/m060c_applicability_matrix.py:
- Build applicability matrix: 7 libraries x 5 future milestones
  - 7 libraries: NetworkX, igraph, rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, GraphScope
  - 5 milestones: M060b (intermediate layer), M061 (2-hop BFS), M062 (fd hardening), M063 (GraphDB selection), M064+ (production)
  - Per-cell: applicability_score (0-3) + use_case_fit + integration_cost + decision
- Aggregate: count of cells with score >= 2 per library (higher = more applicable)
- 5 safety defaults explicit
- 127.0.0.1 NOT localhost
- Output: artifacts/m060c-benchmark/applicability-matrix.{json,md}

Step 2: doc/adr/ADR-016-graph-library-selection.md (M034 template, 14 sections, LLM Reading Notes):
- 0. One-line: igraph + rustworkx adopted as supplementary libraries, NetworkX remains primary
- 1. Context: graph layer for diagnostic + algorithm
- 2. Decision: ADOPT igraph (5-10x speedup) + rustworkx (when available) for heavy algorithm ops; NetworkX remains primary for simple ops + read-only manifests
- 3-13. Per M034 template
- 14. LLM Reading Notes

Step 3: artifacts/m060c-benchmark/m061-m065-decision.md (Russian):
- For each of M060b, M061, M062, M063, M064+ — which library to use
- 5+ tests, 5 safety defaults, M045 on_track

## Inputs

- `artifacts/m060c-benchmark/library-research/`
- `artifacts/m060c-benchmark/benchmark.json`

## Expected Output

- `scripts/m060c_applicability_matrix.py`
- `artifacts/m060c-benchmark/applicability-matrix.json`
- `artifacts/m060c-benchmark/applicability-matrix.md`
- `artifacts/m060c-benchmark/m061-m065-decision.md`
- `doc/adr/ADR-016-graph-library-selection.md`

## Verification

test -f artifacts/m060c-benchmark/applicability-matrix.json

## Observability Impact

Applicability matrix + ADR-016 + decision doc.
