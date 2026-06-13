# M061 S01 Decision: 1-anchor pilot (2605.18747)

Generated: 2026-06-13T08:43:42+00:00

## Decision

**STOP before S02.** The 1-anchor pilot did not meet all quantitative gates with safety defaults enabled.

## Gates

| Gate | Threshold | Observed | Result |
|---|---:|---:|---|
| New 2-hop papers | >= 100 | 2491 | pass |
| M3 judge success rate | >= 80% | 100.0% | pass |
| Real-paper throughput | >= 1 paper/min | 0.00 | fail |

## Safety posture

External network is disabled, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.
Stage 7 uses a diagnostic-only M3 override by reusing M060g evidence; no new live LLM call is made by this S01 pilot.

## Rationale

- 1-hop validation matched M056 with 165 references.
- 2-hop BFS produced 2491 new arXiv IDs from available TEI files.
- 30 papers were audited through stage records; 0 were fully processed as real acquired papers.
- M3 diagnostic evidence covered 30 figures with 100.0% success.
- Because live arXiv acquisition is disabled by default, this pilot should not be treated as proof that network acquisition capacity is production-ready.
