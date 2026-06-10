---
id: S05
parent: M055-kyxuqm
milestone: M055-kyxuqm
provides:
  - 20-PDF routing evidence for S06 report and ADR amendment.
requires:
  []
affects:
  - S06
key_files:
  - scripts/benchmark_m055deep_hybrid_routing_20.py
  - tests/test_m055deep_hybrid_routing_20.py
  - artifacts/m055deep-parser-benchmark/hybrid-routing-20/summary.json
  - artifacts/m055deep-parser-benchmark/hybrid-routing-20/per-pdf/
key_decisions:
  - Hybrid remains the default when OpenDataLoader body markdown is successful and non-low-quality.
  - GROBID fulltext is the fallback single parser when OpenDataLoader body evidence is low-quality or unavailable.
patterns_established:
  - Fulltext-aware routing summary includes per-dimension winners, length-bucket patterns, and fulltext-versus-header delta.
observability_surfaces:
  - Per-PDF comparison packets include residual_gaps and explicit recommended_route rationale.
drill_down_paths:
  - .gsd/milestones/M055-kyxuqm/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M055-kyxuqm/slices/S05/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-10T12:20:21.450Z
blocker_discovered: false
---

# S05: Hybrid routing on 20 PDFs with GROBID fulltext comparison

**S05 produced fulltext-aware 20-PDF hybrid routing with 95% hybrid and one GROBID fallback.**

## What Happened

Implemented and ran the M055deep S05 routing comparator over 20 PDFs. The aggregate recommendation is hybrid_with_fulltext_grobid_fallback: 19 PDFs route to GROBID fulltext plus OpenDataLoader body, while 2605.28617v1 routes to GROBID fulltext only because the OpenDataLoader packet is low-quality. GROBID wins metadata, citations, layout, processing-time plurality, and quality; OpenDataLoader wins body_content in aggregate.

## Verification

uv run pytest tests/test_m055deep_hybrid_routing_20.py -q passed with 6 tests. The M050/M052/M053/M055/M055deep regression subset passed with 159 tests. M045 trajectory and M044 guardrail tests passed with 19 tests.

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

Expected outcome allowed 100% or close hybrid; actual data produced 95% hybrid due one low-quality OpenDataLoader packet.

## Known Limitations

Routing remains diagnostic only and is not authorized for graph writes or production import.

## Follow-ups

S06 REPORT and ADR amendment should codify fulltext-aware fallback.

## Files Created/Modified

- `scripts/benchmark_m055deep_hybrid_routing_20.py` — Added 20-PDF fulltext-aware routing comparator.
- `tests/test_m055deep_hybrid_routing_20.py` — Added S05 routing tests.
- `artifacts/m055deep-parser-benchmark/hybrid-routing-20/` — Added S05 routing outputs.
