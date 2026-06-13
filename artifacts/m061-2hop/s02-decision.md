# M061 S02 Decision: 5-anchor 2-hop BFS

Generated: 2026-06-13T10:31:04+00:00

## Decision

**GO to S03 synthesis.** Gate result: pass.

## Gates

| Gate | Threshold | Observed | Result |
|---|---:|---:|---|
| Cumulative real-paper throughput | >= 1 paper/min | 7.11 | pass |
| 5-layer graph validates | true | true | pass |
| HTTP 429 responses | 0 | 0 | pass |

## Per-anchor stats

| Anchor | 1-hop refs | 2-hop new arXiv IDs | Fully processed papers | M3 judge success | Throughput papers/min | arXiv requests | HTTP 429s | Fallback |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| 2605.18747 | 165 | 2491 | 30 | 100.0% | 7.26 | 64 | 0 | false |
| 2401.04016 | 166 | 2496 | 30 | 100.0% | 6.78 | 67 | 0 | true |
| 2207.05608 | 165 | 2480 | 30 | 100.0% | 7.22 | 64 | 0 | false |
| 2505.19443 | 165 | 2416 | 30 | 100.0% | 7.21 | 64 | 0 | false |
| 2510.12157 | 165 | 2479 | 30 | 100.0% | 7.12 | 64 | 0 | false |

## Combined 5-layer graph

| Layer | Nodes | Edges |
|---|---:|---:|
| citation_m056_plus_m061_2hop | 2662 | 8911 |
| table_similarity_m057 | 83 | 4934 |
| figure_similarity_m057_v1 | 1 | 15 |
| figure_similarity_m058_v2 | 16 | 15 |
| judge_scores_m3_m060g_diagnostic | 150 | 150 |

## Cumulative arXiv rate-limit metrics

- Total requests made: 323.
- HTTP 429 responses: 0.
- Minimum interval: 3.0 seconds.
- Request kinds: {'api': 21, 'eprint': 151, 'pdf': 151}.
- Total wall time by anchor sum: 1265.31s.
- S02 runner wall time: 265.79s.

## Safety posture

External network is disabled by default, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.
Scoped override: external_network_authorized=True for M064-wqfgfa S02 only, four requested anchors, 30 sample PDFs per anchor, no production import, and no graph writes.
Network host reference for local services is 127.0.0.1.

## Artifacts

- Combined summary: `artifacts/m061-2hop/combined-5-anchor-summary.json`
- Combined graph manifest: `artifacts/m061-2hop/5-anchor-5-layer-graph-manifest.json`
