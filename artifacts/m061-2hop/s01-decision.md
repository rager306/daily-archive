# M061 S01 Decision: 1-anchor pilot (2605.18747)

Generated: 2026-06-13T09:48:57+00:00

## Decision

**GO to S02.** All quantitative gates passed with scoped real acquisition.

## Gates

| Gate | Threshold | Observed | Result |
|---|---:|---:|---|
| New 2-hop papers | >= 100 | 2491 | pass |
| M3 judge success rate | >= 80% | 100.0% | pass |
| Real-paper throughput | >= 1 paper/min | 7.26 | pass |

## Real acquisition and rate-limit metrics

- Real arXiv acquisition time: 189.06s.
- Requests made: 64 total ({'api': 4, 'pdf': 30, 'eprint': 30}).
- HTTP 429 rate: 0.0% (0 responses).
- Average pacing delay: 2.86s with minimum interval 3.0s.
- M3 judge time: 0.00s (diagnostic evidence reuse).

## Safety posture

External network is disabled by default, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.
Scoped override: external_network_authorized=True for M064-wqfgfa S01 only, 30 sample PDFs, no production import, no graph writes.
Stage 7 uses a diagnostic-only M3 override by reusing M060g evidence; no new live LLM call is made by this S01 pilot.

## Rationale

- 1-hop validation matched M056 with 165 references.
- 2-hop BFS produced 2491 new arXiv IDs from available TEI files.
- 30 papers were audited through stage records; 30 were fully processed as real acquired papers.
- M3 diagnostic evidence covered 30 figures with 100.0% success.
- Graph layer node counts: {'citation_m056_plus_m061_2hop': 2662, 'table_similarity_m057': 83, 'figure_similarity_m057_v1': 1, 'figure_similarity_m058_v2': 16, 'judge_scores_m3_m060g_diagnostic': 30}.
