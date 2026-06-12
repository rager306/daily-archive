# M058 Deferred Decision for M060

## Decision

Defer Marker scale-up and chart extraction from M058. Carry forward the accepted M058 S01 figure-caption v2 evidence into M060, but do not proceed with Marker S03/S04 expansion from the page-limited S02 result.

## Rationale

M058 produced enough evidence to accept `figure_similarity_v2` as a diagnostic content-graph layer. It did not produce enough evidence to decide Marker full-document scale-up. S02 processed 5 PDFs with `page_range=0`, and that page-limited signal is not actionable for 15/45/166-PDF expansion.

The requested S02 input `2305.14314` was not available locally; the executable fifth PDF was `1804.02767`. This was acceptable for a smoke pilot, but it reinforces that M060 should start from local corpus readiness rather than automatic parser expansion.

## M060 Proposed Scope

M060 should focus on graph-readiness foundation:

1. Run or design a 2-hop BFS cite-graph expansion to test whether graph connectivity improves beyond M056 1-hop evidence.
2. Harden fd usage as a local production-style diagnostic service: health checks, deterministic batch contracts, bounded retries, and durable failure artifacts.
3. Resolve ADR-002 GraphDB selection for the diagnostic graph runtime.
4. Preserve the four-layer M058 combined graph as input evidence: citation, table_similarity, figure_similarity_v1, figure_similarity_v2.

## Explicit Deferrals

- Marker full-document scale-up is deferred until a future milestone defines local input readiness, full-document cost budget, comparison criteria, and stop/go thresholds.
- Chart extraction is deferred until it has a separate pilot question and is not coupled to Marker scale-up.
- Production import is disabled.
- Graph writes is disabled.
- External network is not authorized.
- Fact promotion is not authorized.
- LLM calls is disabled.

## Evidence Links

- `artifacts/m058-plotextractor/summary.json`
- `artifacts/m058-plotextractor/v2-vs-m057.json`
- `artifacts/m058-plotextractor/s01-decision.md`
- `artifacts/m058-marker/pilot-5/summary.json`
- `artifacts/m058-marker/pilot-5/decision.md`
- `artifacts/m058-pilot/combined-edges.json`
- `artifacts/m058-pilot/per-layer-summary.json`
- `doc/adr/ADR-012-figure-caption-v2.md`

## Closeout Interpretation

M058 closes with S01 and S02 executed, S03 and S04 cancelled per S02 gate, and S05 synthesis complete. The correct downstream state is not “Marker failed”; it is “Marker scale decision deferred because page-limited evidence is insufficient.”
