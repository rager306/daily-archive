---
id: S04
parent: M057-s70wkm
milestone: M057-s70wkm
provides:
  - Diagnostic content graph v1 with 9403 edges.
  - Binding ADR-011 supplement to ADR-010.
  - Deferred-decision handoff for M059 chart and Marker work.
requires:
  []
affects:
  []
key_files:
  - scripts/m057_build_graph_manifest.py
  - artifacts/m057-fd-marker/combined-edges.json
  - artifacts/m057-fd-marker/per-layer-summary.json
  - artifacts/m057-fd-marker/REPORT.md
  - doc/adr/ADR-011-content-graph-via-fd.md
  - artifacts/m057-fd-marker/decision-deferred.md
  - tests/test_m057_s04.py
key_decisions:
  - Content graph v1 via fd accepted as supplementary evidence to the M056 citation graph.
  - OpenDataLoader tables are the primary content evidence layer for graph-readiness gate v1.
  - PlotExtract chart extraction and Marker re-extraction are deferred to M059.
patterns_established:
  - Three evidence layers normalized into one diagnostic graph edge schema.
  - Artifact-based regression tests preserve S01-S03 results without requiring a live fd service.
observability_surfaces:
  - combined-edges.json exposes every normalized edge with evidence_layer and evidence_id.
  - per-layer-summary.json exposes count, mean_similarity, distinct_source_papers, and distinct_target_papers by layer.
  - REPORT.md records graph-readiness gate v1 rationale and safety posture.
drill_down_paths:
  - .gsd/milestones/M057-s70wkm/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M057-s70wkm/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M057-s70wkm/slices/S04/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-11T09:26:23.059Z
blocker_discovered: false
---

# S04: Graph-readiness gate v1 synthesis

**Synthesized M057 into a combined content graph manifest, REPORT.md, ADR-011, deferred-decision note, and verification suite.**

## What Happened

S04 combined M056 citation edges with M057 table-similarity and figure-similarity edges into a normalized diagnostic graph manifest. It then documented the graph-readiness gate v1 result in REPORT.md, accepted content graph v1 through ADR-011, deferred chart extraction and Marker re-extraction to M059, and added tests/test_m057_s04.py to lock the expected graph stats, safety defaults, documents, and regression evidence.

## Verification

uv run pytest tests/test_m057_s04.py -q passed 7 tests. uv run pytest tests/test_m057_s01.py tests/test_m057_s02.py tests/test_m057_s03.py tests/test_m057_s04.py -q passed 28 tests. uv run pytest tests/test_m045_project_trajectory.py tests/test_m044_sidecar_architecture_guardrail.py -q passed 19 tests. uv run python scripts/verify_m044_sidecar_architecture_guardrail.py exited 0. uv run python scripts/check_project_trajectory.py reported verdict=on_track.

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

The direct command `uv run python scripts/check_project_trajectory.py --root .` exposed a relative-path CLI issue; the supported default invocation returned verdict=on_track. No S04 source was changed for this pre-existing issue.

## Known Limitations

Marker re-extraction and PlotExtract chart extraction are intentionally deferred to M059. Citation mean_similarity is citation_count based and can exceed 1.0; this is documented in REPORT.md.

## Follow-ups

M059 should repair transformers.onnx environment support, rerun Marker extraction, add PlotExtract chart extraction, and evaluate 2-hop BFS/content graph connectivity.

## Files Created/Modified

- `scripts/m057_build_graph_manifest.py` — New graph manifest builder for citation, table_similarity, and figure_similarity layers.
- `artifacts/m057-fd-marker/combined-edges.json` — Generated normalized 9403-edge graph manifest.
- `artifacts/m057-fd-marker/per-layer-summary.json` — Generated per-layer edge statistics.
- `artifacts/m057-fd-marker/REPORT.md` — Russian M057 synthesis report.
- `doc/adr/ADR-011-content-graph-via-fd.md` — Binding ADR accepting content graph v1 as supplementary evidence.
- `artifacts/m057-fd-marker/decision-deferred.md` — Deferred chart extraction and Marker re-extraction decisions for M059.
- `tests/test_m057_s04.py` — S04 test suite covering manifest, docs, safety defaults, and regression artifacts.
