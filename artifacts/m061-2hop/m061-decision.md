# M061 Decision: 2-hop BFS evidence and M064 trigger

Generated: 2026-06-13T10:51:29Z

## Decision

**CONFIRM DEFER M064.** M061 completed 5-anchor 2-hop BFS at scale and did not trigger the ADR-017 condition for queue infrastructure.

## Evidence

| Gate | Threshold | Observed | Result |
|---|---:|---:|---|
| Anchors completed | 5 | 5 | pass |
| Citation edges | >= 8911 | 8911 | pass |
| HTTP 429 responses | 0 | 0 | pass |
| Cumulative throughput | >= 1 paper/min | 7.11 | pass |
| M3 judge success | >= 80% | 100.0% | pass |
| Graph validates | true | true | pass |

## Safety posture

External network is disabled by default, LLM calls are disabled by default, graph writes is not authorized, production import is not authorized, and fact promotion is not authorized. M061 used scoped acquisition and diagnostic-only overrides documented in S01/S02 artifacts.

## Trigger evaluation

M064 remains deferred per ADR-017 because sync execution is sufficient. No async queue, lease, multi-worker, or smart scheduler evidence is required before M062/M063 complete.
