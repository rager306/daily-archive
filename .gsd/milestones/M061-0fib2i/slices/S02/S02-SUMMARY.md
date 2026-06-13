---
id: S02
parent: M061-0fib2i
milestone: M061-0fib2i
provides:
  - Binding graph-library selection guidance for M060b-M064+.
  - Generated 8x5 applicability matrix for future milestone planning.
  - Regression tests for S02 decision artifacts and safety defaults.
requires:
  - slice: S01
    provides: Benchmark evidence and library research for ADR-016 graph library selection.
affects:
  - M061
  - M062
  - M063
  - M064+
key_files:
  - scripts/m060c_applicability_matrix.py
  - artifacts/m060c-benchmark/applicability-matrix.json
  - artifacts/m060c-benchmark/applicability-matrix.md
  - doc/adr/ADR-016-graph-library-selection.md
  - artifacts/m060c-benchmark/m061-m065-decision.md
  - tests/test_m060c_s02.py
key_decisions:
  - NetworkX remains the primary graph representation and correctness baseline.
  - igraph is adopted as a supplementary read-only accelerator for algorithm-heavy M060b and M061 work.
  - rustworkx is adopted when available for traversal/path hot spots with parity/fallback checks.
  - GraphScope is deferred except as a future M063 GraphDB-selection candidate.
patterns_established:
  - Applicability decisions are emitted as deterministic JSON and Markdown from a script.
  - Graph accelerator adoption requires explicit read-only safety defaults and parity/fallback checks.
observability_surfaces:
  - Applicability matrix JSON records per-cell library, milestone, applicability score, use-case fit, integration cost, decision, and research reference.
  - ADR-016 LLM Reading Notes capture binding decision, non-authorizations, safe next action, and blocked conditions.
drill_down_paths:
  - .gsd/milestones/M061-0fib2i/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M061-0fib2i/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-13T05:17:57.842Z
blocker_discovered: false
---

# S02: Applicability matrix + ADR-016 (binding) + closeout

**S02 produced the 8x5 applicability matrix, binding ADR-016, decision document, tests, and closeout verification.**

## What Happened

S02 converted the M060c S01 benchmark and library research into binding decision artifacts. The generated matrix covers NetworkX, igraph, rustworkx, graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope across M060b, M061, M062, M063, and M064+. ADR-016 records the binding decision: NetworkX remains primary, igraph is adopted as a supplementary read-only accelerator for algorithm-heavy work, and rustworkx is adopted when available for traversal/path hot spots. The Russian decision document gives per-milestone library choices for M060b through M064+. The test file verifies the artifacts, five safety defaults, and regression surfaces; target pytest, M045 trajectory, and M044 guardrail all passed.

## Verification

Generated matrix via `uv run python scripts/m060c_applicability_matrix.py`. `uv run pytest tests/test_m060c_s02.py -q` passed with 8 tests. `uv run python scripts/check_project_trajectory.py --phase closeout --output-dir /tmp/m060c-s02-project-trajectory` returned `verdict=on_track`. `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py` returned `m044 sidecar architecture guardrail ok`.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The S02 plan text says 7 libraries, while the explicit execution instruction requires 8 libraries. Implemented 8 libraries.

## Known Limitations

ADR-016 authorizes only read-only supplementary accelerator use; production adoption still requires future gate approval.

## Follow-ups

M061 should add parity tests for any accelerated 2-hop BFS path. M063 should evaluate GraphDB candidates separately from in-process graph algorithm libraries.

## Files Created/Modified

- `scripts/m060c_applicability_matrix.py` — New deterministic generator for M060c S02 applicability matrix artifacts.
- `artifacts/m060c-benchmark/applicability-matrix.json` — Generated 8 libraries x 5 milestones applicability matrix.
- `artifacts/m060c-benchmark/applicability-matrix.md` — Markdown rendering of the applicability matrix and binding recommendation.
- `doc/adr/ADR-016-graph-library-selection.md` — Binding ADR for graph library selection.
- `artifacts/m060c-benchmark/m061-m065-decision.md` — Russian per-milestone graph library decision document.
- `tests/test_m060c_s02.py` — S02 artifact, safety-default, ADR, decision-doc, and regression tests.
