---
id: M058-cmjp1u
title: "M059 Pilot Cycle plotextractor v2 + Marker Iterative Expansion"
status: complete
completed_at: 2026-06-12T08:33:32.342Z
key_decisions:
  - Accept figure_similarity_v2 as a binding diagnostic supplement via ADR-012.
  - Stop Marker scale-up for M058 because S02 evidence is page-limited.
  - Defer Marker full-document scale-up and chart extraction to future scoped milestones.
  - Focus M060 on 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection.
key_files:
  - scripts/m058_build_graph_manifest.py
  - tests/test_m058_s05.py
  - artifacts/m058-pilot/combined-edges.json
  - artifacts/m058-pilot/per-layer-summary.json
  - artifacts/m058-pilot/REPORT.md
  - artifacts/m058-pilot/decision-deferred.md
  - doc/adr/ADR-012-figure-caption-v2.md
lessons_learned:
  - Iterative gates worked as designed: S02 stopped S03/S04 rather than scaling weak evidence.
  - Page-limited parser evidence is useful for smoke tests but insufficient for full-corpus scale decisions.
  - Figure-caption v2 is valuable for provenance even without increasing edge volume.
---

# M058-cmjp1u: M059 Pilot Cycle plotextractor v2 + Marker Iterative Expansion

**M058 closed as a pilot evidence milestone: figure-caption v2 accepted, Marker scale-up stopped, and M060 scope deferred to graph-readiness foundation.**

## What Happened

M058 executed the plotextractor v2 pilot and Marker stage 1, then synthesized the evidence in S05. S01 produced TeX-derived figure-caption evidence with 104 figures and 15 v2 similarity edges. S02 ran a 5-PDF page-limited Marker pilot and correctly returned a NO-GO for automatic expansion. S03 and S04 were skipped/cancelled per that gate. S05 combined M056/M057/M058 graph layers into a 9418-edge diagnostic manifest, wrote REPORT.md, accepted ADR-012 as a binding supplement to ADR-011, and documented deferred Marker/chart decisions for M060.

## Success Criteria Results

- PASS: Combined graph manifest with 4 layers exists and reports 9418 edges.
- PASS: REPORT.md exists and covers the requested closeout sections.
- PASS: ADR-012 is Accepted (binding).
- PASS: decision-deferred.md documents M060 plan plus Marker/chart deferrals.
- PASS: 7 S05 tests passed.
- PASS: M045 trajectory closeout reported on_track.
- PASS: M044 guardrail exited successfully.
- PASS: S03 and S04 are skipped/cancelled per S02 gate, leaving 3/5 executed/complete slices plus 2 skipped slices.

## Definition of Done Results

- PASS: S01 and S02 evidence preserved.
- PASS: S03/S04 not executed after S02 NO-GO.
- PASS: S05 artifacts and tests produced.
- PASS: Safety defaults remain false.
- PASS: Local commit created for S05 closeout artifacts.

## Requirement Outcomes

No new requirement IDs were introduced. The milestone preserves the existing safety and graph-readiness constraints: production import is disabled, graph writes is disabled, external network is not authorized, fact promotion is not authorized, and LLM calls is disabled.

## Deviations

S03 and S04 were skipped/cancelled per S02 gate rather than executed.

## Follow-ups

M060 should execute the narrowed graph-readiness foundation: 2-hop BFS, fd production hardening, and ADR-002 GraphDB selection. Marker full-document scale-up and chart extraction remain deferred.
