# S05: Synthesis + ADR-012 + decision deferred — UAT

**Milestone:** M058-cmjp1u
**Written:** 2026-06-12T08:28:39.105Z

# S05 UAT

## Checks

- PASS: Combined graph manifest exists at `artifacts/m058-pilot/combined-edges.json` and reports 9418 edges across 4 layers.
- PASS: Per-layer summary exists at `artifacts/m058-pilot/per-layer-summary.json` with citation, table_similarity, figure_similarity_v1, and figure_similarity_v2 counts.
- PASS: `artifacts/m058-pilot/REPORT.md` exists, is over 4KB, and documents S01 success, S02 NO-GO, S03/S04 cancellation, combined graph stats, ADR-012, M060 plan, and lessons.
- PASS: `doc/adr/ADR-012-figure-caption-v2.md` is Accepted (binding) and supplements ADR-011.
- PASS: `artifacts/m058-pilot/decision-deferred.md` documents Marker/chart deferrals and M060 scope.
- PASS: `uv run pytest tests/test_m058_s05.py -q` passed with 7 tests.
- PASS: M044 guardrail passed.
- PASS: M045 trajectory closeout reported `on_track`.

