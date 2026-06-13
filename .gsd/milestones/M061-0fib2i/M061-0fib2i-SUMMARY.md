---
id: M061-0fib2i
title: "M060c Graph Library Alternatives Research and Applicability"
status: complete
completed_at: 2026-06-13T05:18:45.407Z
key_decisions:
  - NetworkX remains the primary graph representation and correctness baseline.
  - igraph is adopted as a supplementary read-only accelerator for algorithm-heavy M060b and M061 work.
  - rustworkx is adopted when available for traversal/path hot spots with parity/fallback checks.
  - graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope remain deferred except GraphScope as a future M063 GraphDB-selection candidate.
key_files:
  - scripts/m060c_applicability_matrix.py
  - tests/test_m060c_s02.py
  - artifacts/m060c-benchmark/applicability-matrix.json
  - artifacts/m060c-benchmark/applicability-matrix.md
  - doc/adr/ADR-016-graph-library-selection.md
  - artifacts/m060c-benchmark/m061-m065-decision.md
  - .gsd/milestones/M061-0fib2i/M061-0fib2i-VALIDATION.md
lessons_learned:
  - The roadmap text said 7 libraries but the execution instruction required 8; future plans should keep library counts synchronized with explicit library lists.
  - Trajectory checks can be run to a temporary output directory during closeout to avoid dirtying project artifacts.
---

# M061-0fib2i: M060c Graph Library Alternatives Research and Applicability

**M060c benchmarked graph-library alternatives and bound NetworkX, igraph, and rustworkx usage for M060b-M064+.**

## What Happened

The milestone produced benchmark evidence for NetworkX, igraph, and rustworkx, library research for deferred alternatives, and binding S02 decision artifacts. S01 established benchmark data showing igraph and rustworkx materially improve selected heavy graph operations while preserving NetworkX as the baseline. S02 generated an 8-library by 5-milestone applicability matrix, authored ADR-016 as a binding graph-library selection decision, and wrote a Russian M061-M065 decision document. The final decision keeps NetworkX primary, adopts igraph as a supplementary read-only accelerator for algorithm-heavy work, adopts rustworkx when available for traversal/path hot spots, and defers other libraries unless future milestones explicitly justify them.

## Success Criteria Results

- S01 benchmark pip-installable alternatives: met via `artifacts/m060c-benchmark/benchmark.json` and S01 tests.
- S01 research deferred alternatives: met via `artifacts/m060c-benchmark/library-research/*.md`.
- S02 applicability matrix: met via `artifacts/m060c-benchmark/applicability-matrix.json` and `.md`.
- S02 ADR-016 binding decision: met via `doc/adr/ADR-016-graph-library-selection.md`.
- S02 decision doc: met via `artifacts/m060c-benchmark/m061-m065-decision.md`.
- 5+ tests: met via `uv run pytest tests/test_m060c_s02.py -q` -> 8 passed.
- 5 safety defaults false: met via tests and artifacts.
- M045 on_track: met via `uv run python scripts/check_project_trajectory.py --phase closeout --output-dir /tmp/m060c-s02-project-trajectory`.
- M044 ok: met via `uv run python scripts/verify_m044_sidecar_architecture_guardrail.py`.

## Definition of Done Results

- Code/artifacts generated deterministically: done.
- Tests pass: done, 8 passed in S02 target test file.
- Guardrails pass: done, M045 on_track and M044 ok.
- GSD slice and milestone artifacts recorded: done.
- Local commit prepared without remote push: pending until git commit step immediately after DB checkpoint/staging.

## Requirement Outcomes

No project requirement IDs were created or transitioned in this milestone. The milestone advances graph-library selection evidence and keeps all safety defaults false.

## Deviations

S02 delivered 8 libraries instead of the roadmap's 7-library wording because the explicit execution task required 8 libraries.

## Follow-ups

M061 should add parity tests for any accelerated 2-hop BFS path. M063 should evaluate GraphDB candidates separately from in-process graph algorithm libraries. M064+ must require explicit production authorization before accelerator production adoption.
