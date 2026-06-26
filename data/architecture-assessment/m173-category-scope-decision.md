# M173 Category Scope Decision

## Decision

Implement exactly three path-family categories:

1. `parser-replay-output`
2. `source-scan-output`
3. `graph-probe-output`

These categories are approved only for exact reviewed source paths.

## Approved exact scopes

| New category | Exact source path scope | Current categories affected | Why safe |
|---|---|---|---|
| `parser-replay-output` | `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | caller-owned, run-scoped | Parser replay cache, output, and summary artifacts are generated under one replay adapter path. |
| `source-scan-output` | `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py`, `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | caller-owned | Thirty-paper source scan outputs are generated under reviewed scan modules. |
| `graph-probe-output` | `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | caller-owned | R024 graph probe summary and memory profile outputs are generated under one graph probe module. |

## Rejected for this milestone

| Scope | Reason |
|---|---|
| Generic `summary_path` or `destination` matching | Would hide unrelated caller-owned outputs. |
| Generic `cache_path` matching | Could hide shared cache/state writes in other modules. |
| Generic graph module matching | Too broad; only R024 networkx probe is reviewed here. |
| Any `shared-state` record | M173 must not reduce shared-state risk signals. |

## Implementation rule

The classifier may match only the exact source paths above. It must not classify by broad target words or directory families outside the approved scope.

## Test rule

Every new category needs:

- one positive test for an exact approved source path;
- one fallback test showing similar target words outside approved source paths stay in broad/conservative categories.
