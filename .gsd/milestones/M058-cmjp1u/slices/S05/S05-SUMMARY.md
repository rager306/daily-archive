---
id: S05
parent: M058-cmjp1u
milestone: M058-cmjp1u
provides:
  - M058 combined graph manifest for downstream graph-readiness work.
  - ADR-012 binding supplement for figure-caption v2 provenance.
  - M060 deferred-decision scope.
requires:
  []
affects:
  []
key_files:
  - scripts/m058_build_graph_manifest.py
  - tests/test_m058_s05.py
  - artifacts/m058-pilot/combined-edges.json
  - artifacts/m058-pilot/per-layer-summary.json
  - artifacts/m058-pilot/REPORT.md
  - artifacts/m058-pilot/decision-deferred.md
  - doc/adr/ADR-012-figure-caption-v2.md
key_decisions:
  - ADR-012 accepts figure_similarity_v2 as a binding diagnostic supplement to ADR-011.
  - Marker scale-up and chart extraction are deferred because S02 evidence is page-limited.
  - M060 should narrow to 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection.
patterns_established:
  - Four-layer diagnostic graph manifests keep edge normalization separate from production graph writes.
  - Page-limited parser evidence can validate tooling but must not authorize scale-up.
observability_surfaces:
  - per-layer-summary.json exposes layer counts, distinct source/target papers, mean similarity, and safety defaults.
  - tests/test_m058_s05.py protects S05 artifacts and M058 S01/S02 regression assumptions.
drill_down_paths:
  - .gsd/milestones/M058-cmjp1u/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M058-cmjp1u/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M058-cmjp1u/slices/S05/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-12T08:28:39.105Z
blocker_discovered: false
---

# S05: Synthesis + ADR-012 + decision deferred

**S05 synthesized M058 pilot evidence into a combined graph manifest, REPORT.md, ADR-012, and M060 deferred-decision plan.**

## What Happened

S05 combined the M056 citation layer, M057 table-similarity layer, M057 figure-similarity v1 layer, and M058 plotextractor figure-similarity v2 layer into a 9418-edge diagnostic graph manifest. It documented the M058 outcome: S01 plotextractor v2 succeeded, S02 Marker stage 1 produced a page-limited NO-GO for scale-up, and S03/S04 remain cancelled/skipped by that gate. ADR-012 accepts figure_similarity_v2 as a binding diagnostic supplement while keeping production import, graph writes, fact promotion, external network calls, and LLM calls disabled. The deferred decision artifact narrows M060 to 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection.

## Verification

uv run python scripts/m058_build_graph_manifest.py passed and generated 9418 edges across 4 layers. uv run pytest tests/test_m058_s05.py -q passed with 7 tests. uv run python scripts/verify_m044_sidecar_architecture_guardrail.py passed. uv run python scripts/check_project_trajectory.py --phase closeout reported verdict=on_track.

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

None from S05 plan. S03 and S04 are skipped/cancelled per the previously recorded S02 gate.

## Known Limitations

Marker full-document readiness remains unproven because S02 was page-limited. Citation mean_similarity is null because citation edges are relations, not numeric similarity scores.

## Follow-ups

M060 should focus on 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection. Marker full-document scale-up and chart extraction remain deferred.

## Files Created/Modified

- `scripts/m058_build_graph_manifest.py` — New idempotent manifest builder for M058 S05.
- `tests/test_m058_s05.py` — New regression tests for S05 manifest, report, ADR, safety defaults, and S01/S02 assumptions.
- `artifacts/m058-pilot/combined-edges.json` — Generated normalized 9418-edge combined manifest.
- `artifacts/m058-pilot/per-layer-summary.json` — Generated per-layer statistics for 4 graph evidence layers.
- `artifacts/m058-pilot/REPORT.md` — M058 synthesis report.
- `artifacts/m058-pilot/decision-deferred.md` — M060 deferred-decision plan.
- `doc/adr/ADR-012-figure-caption-v2.md` — New binding ADR supplement accepting figure-caption v2 diagnostic evidence.
