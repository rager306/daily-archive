# M174 Repair Category Scope Decision

## Decision

Implement exactly one path-family category:

1. `repair-benchmark-output`

The category is approved only for exact reviewed source paths and must preserve the existing `caller-owned-index` exception.

## Approved exact scope

| New category | Exact source path scope | Current categories affected | Why safe |
|---|---|---|---|
| `repair-benchmark-output` | `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py`, `src/research_graph/infrastructure/repair/chunking_benchmark.py` | append-log, run-scoped, caller-owned | These are repair benchmark diagnostics, summaries, and review outputs under two reviewed benchmark modules. |

## Preserved exception

| Existing category | Exact record | Reason |
|---|---|---|
| `caller-owned-index` | `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` + `index_path` | Existing reviewed paired review index output; must not move to repair-benchmark-output. |

## Rejected for this milestone

| Scope | Reason |
|---|---|
| Generic `diagnostics`, `summary`, `review`, or `benchmark` target matching | Would hide unrelated write paths. |
| All `src/research_graph/infrastructure/repair/` writes | Too broad; only two benchmark modules are reviewed here. |
| Any `shared-state` record | M174 must not reduce shared-state risk signals. |

## Implementation rule

The classifier may match only the exact source paths above and must test `index_path` preservation before assigning `repair-benchmark-output`.

## Test rule

M174 needs:

- one positive test for an exact repair benchmark output;
- one test preserving `caller-owned-index` for `index_path`;
- one fallback test for similar unapproved repair-like source paths.
