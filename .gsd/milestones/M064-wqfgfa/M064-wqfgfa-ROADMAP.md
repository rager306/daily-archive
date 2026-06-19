# M064-wqfgfa: M061 2-hop BFS with M3 Judge Integration at Scale

**Vision:** Execute 2-hop BFS from 5 anchors per M056 pattern, integrating M3 multimodal judge on extracted figures. Sync execution per ADR-017. Goal: validate pipeline at scale, generate evidence for future queue decision (per ADR-017 trigger conditions), produce 5-layer diagnostic graph with judge layer. Estimated: ~3000 new papers, 30k new figures, ~30-40h wall time with 4-8 concurrent workers.

## Slices

- [x] **S04: S04** `risk:medium` `depends:[]`
  > After this:

- [x] **S01: 1-anchor pilot: 1-hop + 2-hop BFS + 8 stages + M3 judge e2e** `risk:medium` `depends:[]`
  > After this: 1 anchor (2605.18747) full pipeline: 1-hop validation, 2-hop real acquisition, 8 stages, M3 judge on figures, manifest validation, graph layer. Decision: continue to 4 more anchors or stop

- [x] **S02: 4 more anchors (full 5-anchor 2-hop BFS) + manifest + graph layer** `risk:medium` `depends:[S01]`
  > After this: 5 anchors full pipeline, 5-layer graph (citation + table + figure v1 + figure v2 + judge_scores), total ~3000 papers processed, decision: continue to S03 or adjust

- [x] **S03: Synthesis + ADR-018 (2-hop BFS evidence + M064 trigger) + close M061** `risk:low` `depends:[S01,S02]`
  > After this: REPORT.md (Russian) with full evidence, ADR-018 emitted, M061 closes, future M064 decision evidence captured

## Boundary Map

Not provided.
