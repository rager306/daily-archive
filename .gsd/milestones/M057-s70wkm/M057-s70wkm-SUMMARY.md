---
id: M057-s70wkm
title: "Graph-Readiness Gate v1 via fd Embeddings + Marker Re-extraction"
status: complete
completed_at: 2026-06-11T09:27:07.790Z
key_decisions:
  - Accepted content graph v1 via fd as supplementary evidence to the M056 citation graph.
  - Treated OpenDataLoader table similarity as the primary content evidence layer.
  - Deferred PlotExtract chart extraction and Marker re-extraction to M059.
key_files:
  - scripts/m057_build_graph_manifest.py
  - artifacts/m057-fd-marker/combined-edges.json
  - artifacts/m057-fd-marker/per-layer-summary.json
  - artifacts/m057-fd-marker/REPORT.md
  - doc/adr/ADR-011-content-graph-via-fd.md
  - artifacts/m057-fd-marker/decision-deferred.md
  - tests/test_m057_s04.py
  - .gsd/milestones/M057-s70wkm/M057-s70wkm-VALIDATION.md
lessons_learned:
  - Citation-only graph evidence needs content layers before graph-readiness decisions.
  - OpenDataLoader tables provide the strongest content signal in the current corpus.
  - Figure similarity is low-volume but useful as an independent evidence layer.
  - Safety defaults must remain explicit in every graph-readiness artifact.
---

# M057-s70wkm: Graph-Readiness Gate v1 via fd Embeddings + Marker Re-extraction

**M057 accepted diagnostic content graph v1 via fd as supplementary evidence to the M056 citation graph.**

## What Happened

M057 validated the local fd embedding path, built table-similarity and figure-similarity content evidence, then synthesized those layers with the M056 citation graph. The final S04 output is a normalized 9403-edge diagnostic graph manifest, per-layer summary, Russian REPORT.md, binding ADR-011, deferred-decision note for M059, and a dedicated S04 test suite. The milestone keeps all five safety defaults false and does not authorize production import or graph writes.

## Success Criteria Results

- fd validation: PASS, S01 recorded 7/7 tests passing with p95 about 253 ms.
- table similarity: PASS, S02 recorded 1468 tables and 4934 edges.
- figure similarity: PASS, S03 recorded 937 figures and 15 inter-doc edges.
- S04 synthesis: PASS, combined graph has 9403 edges across citation, table_similarity, and figure_similarity.
- Safety posture: PASS, all five safety defaults remain false and production import is disabled.

## Definition of Done Results

- REPORT.md created: PASS.
- ADR-011 created and accepted as binding supplement to ADR-010: PASS.
- decision-deferred.md created for PlotExtract chart extraction and Marker re-extraction: PASS.
- S04 tests added and passed: PASS, 7 passed.
- Regression checks passed: PASS, M057 S01-S04 28 passed, M045/M044 19 passed, M044 CLI exit 0, M045 trajectory on_track.

## Requirement Outcomes

M057 validates the diagnostic graph-readiness direction by adding content evidence layers to citation evidence. Production import, graph writes, fact promotion, external network calls, and LLM calls remain disabled.

## Deviations

The command with relative root for trajectory exposed a pre-existing CLI path issue; the supported default trajectory command reported on_track. Chart extraction and Marker re-extraction were intentionally deferred to M059.

## Follow-ups

M059 should repair transformers.onnx support for Marker, rerun Marker extraction, add PlotExtract chart extraction, and evaluate 2-hop BFS/content graph connectivity.
