# M063-8d01zz: M060b NetworkX Graph Validation Intermediate Layer

**Vision:** Build NetworkX-based intermediate graph layer: statistics, validation, visualization, 2-hop BFS preview algorithm on 4-layer graph (9418 edges). Per amended ADR-016: NetworkX is primary for read-only ops + manifest validation + simple algorithms; igraph is supplementary for heavy algorithm ops. Establishes M060b as the operational graph layer before M061 2-hop BFS scale.

## Slices

- [x] **S01: Build graph statistics and validation tool with NetworkX** `risk:low` `depends:[]`
  > After this: NetworkX-based statistics + validation tool, 4-layer graph analyzed, statistics report emitted, ADR-016 pattern enforced

- [x] **S02: Visualization + 2-hop BFS preview algorithm + close M060b** `risk:low` `depends:[S01]`
  > After this: PNG visualization of 4-layer graph, 2-hop BFS preview report (estimated scale for M061), M060b closes, next-gate for M061 documented

## Boundary Map

Not provided.
