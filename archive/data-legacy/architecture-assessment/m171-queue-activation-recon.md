# M171 Queue Activation Recon

## Verdict

**UniversalKBQueue can be prepared for local production-style activation readiness, but M171 must not claim real production activation.**

The queue implementation explicitly describes itself as a local SQLite durable queue, not a distributed production queue. M171 should therefore build readiness gates, local soak evidence, and runbook/checklist artifacts rather than starting real production workers.

## Surfaces inspected

| Surface | Role | Notes |
|---|---|---|
| `src/research_graph/workflows/universal_kb/queue.py` | Durable local queue state machine | SQLite, WAL, busy timeout, statuses, payload safety validation, claim/complete APIs. |
| `src/research_graph/workflows/universal_kb/smoke.py` | No-write smoke command surface | Metadata-only profiles, safety flag checks, forbidden payload scans. |
| `src/research_graph/workflows/universal_kb/smoke_runner.py` | Real-corpus no-write runner | Builds continuity metadata and review assistance records with false safety flags. |
| `scripts/soak_universal_kb_queue.py` | Process-level soak harness | Configurable jobs/processes/rounds/timeouts and JSON diagnostics. |

## Current strengths

- Queue uses SQLite WAL and busy timeout for local concurrency.
- Claim/complete path already has process-level soak evidence from M170.
- Safety flags default to no graph writes, no production import, no promotion.
- Smoke surfaces scan persisted artifacts for forbidden payload terms.
- Soak harness emits structured diagnostics and non-zero exit on failed pass conditions.

## Activation gaps

1. No long-running worker supervisor is in scope.
2. No production queue deployment target is configured.
3. No external service binding or production data source is provided.
4. No production rollback mechanism exists beyond local stop conditions.
5. No environment-specific shared filesystem behavior has been tested.

## M171 readiness approach

M171 should close activation readiness by producing:

- a local activation checklist;
- an environment-specific soak profile and result;
- clear stop/rollback criteria;
- a readiness verdict that states what is ready and what remains before production.

## Non-claim boundary

M171 may claim local activation readiness evidence. M171 must not claim production workers are activated or that the queue is a distributed production queue.
