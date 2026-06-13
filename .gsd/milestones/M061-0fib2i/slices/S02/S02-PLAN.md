# S02: Applicability matrix + ADR-016 (binding) + closeout

**Goal:** Build applicability matrix (7 libraries x 5 future milestones) + ADR-016 (binding decision) + decision doc for M061+ integration.
**Demo:** Applicability matrix for 7 libraries, ADR-016 binding decision, decision doc for M061-M065

## Must-Haves

- Applicability matrix: 7 libraries x 5 future milestones
- ADR-016 emitted (M034 template, 14 sections, LLM Reading Notes)
- Decision doc for M061-M065 emitted
- 5+ tests pass
- 5 safety defaults stay false
- M045 on_track, M044 ok
- 1 commit in git history
- M060c closes after S02

## Proof Level

- This slice proves: operational

## Integration Closure

Provides decision evidence for M061-M065 library selection. Establishes research methodology.

## Verification

- Applicability matrix + ADR-016 + decision doc.

## Tasks

- [x] **T01: Built the M060c S02 applicability matrix, ADR-016, and M061-M065 decision document.** `est:60m`
  Step 1: scripts/m060c_applicability_matrix.py:
  - Build applicability matrix: 7 libraries x 5 future milestones
    - 7 libraries: NetworkX, igraph, rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, GraphScope
    - 5 milestones: M060b (intermediate layer), M061 (2-hop BFS), M062 (fd hardening), M063 (GraphDB selection), M064+ (production)
    - Per-cell: applicability_score (0-3) + use_case_fit + integration_cost + decision
  - Aggregate: count of cells with score >= 2 per library (higher = more applicable)
  - 5 safety defaults explicit
  - 127.0.0.1 NOT localhost
  - Output: artifacts/m060c-benchmark/applicability-matrix.{json,md}
  - Files: `artifacts/m060c-benchmark/applicability-matrix.json`, `artifacts/m060c-benchmark/applicability-matrix.md`, `artifacts/m060c-benchmark/m061-m065-decision.md`, `doc/adr/ADR-016-graph-library-selection.md`, `scripts/m060c_applicability_matrix.py`
  - Verify: test -f artifacts/m060c-benchmark/applicability-matrix.json

- [x] **T02: Added S02 tests and verified pytest, M045 trajectory, and M044 guardrail before closeout.** `est:15m`
  tests/test_m060c_s02.py with 5+ tests:
  1. test_applicability_matrix_emitted
  2. test_applicability_matrix_7_libraries
  3. test_applicability_matrix_5_milestones
  4. test_adr_016_binding (M034 template)
  5. test_m061_decision_doc
  6. test_5_safety_defaults
  7. M050-M060g-S01 regression
  - Files: `tests/test_m060c_s02.py`
  - Verify: uv run pytest tests/test_m060c_s02.py -q

## Files Likely Touched

- artifacts/m060c-benchmark/applicability-matrix.json
- artifacts/m060c-benchmark/applicability-matrix.md
- artifacts/m060c-benchmark/m061-m065-decision.md
- doc/adr/ADR-016-graph-library-selection.md
- scripts/m060c_applicability_matrix.py
- tests/test_m060c_s02.py
