# networkx-temporal library research

## Architecture summary

GitNexus context found `TemporalGraph` at `src/networkx_temporal/classes/graph.py:7-63`. The package extends NetworkX concepts across temporal snapshots and temporal graph containers rather than replacing NetworkX with a faster backend.

For our M060b intermediate graph layer, this is conceptually adjacent but premature: the current 4-layer manifest has typed evidence edges, not time-sliced graph state. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Inherited/No direct surface | Local vendored search found no direct BFS hits; algorithms would come through contained NetworkX graphs. |
| PageRank | Inherited/No direct surface | Local vendored search found no direct PageRank hits; likely delegated to NetworkX snapshots. |
| shortest_path | Inherited/No direct surface | Local vendored search found no direct shortest-path hits. |
| community | Partial | Local vendored hits include SBM/community-oriented temporal generators/tests. |

## Our use case fit

Neutral-to-poor near-term fit: it may be valuable if article/paper states become temporal snapshots, but it does not address the current NetworkX performance question.

## Decision

**DEFER**. Rationale: keep as a future option for temporal modeling, not as an M060b/M061 graph acceleration dependency.
