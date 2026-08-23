# M195 S03 Queue Baseline

## Verdict

**PASS: queue lifecycle work can proceed by extending existing `UniversalKBQueue` constants and metadata validation, not by introducing a new scheduler or schema rewrite.**

## GitNexus impact

| Target | Result |
|---|---|
| `Class:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue` | MEDIUM, impactedCount=6, direct=5, processes_affected=0 |
| `Class:src/research_graph/application/orchestrator.py:DispatchProtocol` | LOW, impactedCount=0, processes_affected=0 |
| `Class:src/research_graph/application/orchestrator.py:QueueDispatch` | LOW, impactedCount=0, processes_affected=0 |

MEDIUM on `UniversalKBQueue` is expected because S03 intentionally scopes queue lifecycle behavior. It is below the HIGH or CRITICAL stop threshold.

## Current queue state

Active files:

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/application/orchestrator.py`
- `tests/test_universal_kb_queue.py`

Existing lifecycle support:

- `STATUSES` already includes `pending`, `ready`, `running`, `succeeded`, `failed_retryable`, `failed_terminal`, `blocked`, `stale`, `needs_review`, and `skipped`.
- SQLite CHECK constraint enforces these statuses.
- Existing methods include `enqueue`, `update_payload_diagnostics`, `unblock_ready_jobs`, `claim`, `heartbeat`, `complete`, `fail_retryable`, `fail_terminal`, `block`, `reclaim_expired_leases`, `mark_stale`, `inspect`, `events`, and `close`.
- Payload metadata already supports schema/evidence/cost/latency/retry/diagnostic and false write/promotion eligibility fields.

## Observed active stage values

The active tests and code use stages such as:

- `benchmark`
- `candidate_generation`
- `converted_payload_validation`
- `current_pipeline`
- `end_to_end_boundaries`
- `extract`
- `parser_readiness`
- `real_corpus_review_assistance`
- `review`
- `review_assistance`
- `sidecar_candidate`

## Minimal edit plan

Use Ponytail minimalism:

1. Keep existing SQLite schema and status machine.
2. Add central `PIPELINE_STAGES` constants for canonical M195 stages: `intake`, `acquisition`, `parsing`, `chunking`, `evidence`, `graph_candidate`, and `projection_rehearsal`.
3. Add terminal and active status group constants derived from existing status strings.
4. Validate `stage` as metadata code in `enqueue` so raw text or secrets cannot become persisted stage names.
5. Extend payload metadata with metadata-ref lists needed by S02/S10 candidate handoff: `candidate_packet_refs`, `graph_node_refs`, `graph_edge_refs`, and `provenance_refs`.
6. Do not change `DispatchProtocol`, `QueueDispatch`, database schema, lease logic, retry logic, or external scheduler behavior in S03.

## Tests to add

- Canonical `PIPELINE_STAGES` includes the six requested pipeline stages plus projection rehearsal.
- Status group constants remain subsets of `STATUSES`.
- `enqueue` rejects raw or secret-shaped stage values.
- `payload_metadata` roundtrips candidate packet, graph node, graph edge, and provenance refs while keeping write and promotion eligibility false.

## Disallowed in S03

- No new scheduler.
- No queue storage migration beyond existing columns.
- No production graph import.
- No remote worker execution.
- No graph backend adapter.
- No optimizer invocation.
