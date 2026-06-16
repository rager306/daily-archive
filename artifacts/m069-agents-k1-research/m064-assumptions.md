# M069 S03 T01 M064 Queue Assumptions

## Purpose

Collect current assumptions about the future queue foundation before deciding whether it needs replanning after M069 S01/S02 research.

## Important naming clarification

`M064-wqfgfa` already exists and is closed as **M061 2-hop BFS with M3 Judge Integration at Scale**. In that milestone and ADR-018, `M064 queue` refers to a deferred future direction, not an active GSD milestone ID to continue directly.

Therefore, any future queue work should probably receive a fresh GSD milestone ID instead of reusing `M064` as a filesystem/DB identifier.

## Source evidence

| Source | Finding |
|---|---|
| `doc/adr/ADR-017-pipeline-queue-deferred.md` | Pipeline queue and async infrastructure were deferred until M061, M062, and M063 evidence exists and async queue is needed. |
| `doc/adr/ADR-018-m061-2-hop-evidence-and-m064-trigger.md` | M061 2-hop BFS completed and queue trigger was `CONFIRM DEFER` because synchronous execution was sufficient under ADR-017. |
| `README.md` | GraphDB writes and agentic orchestration are still guarded; M068 added fd v2 env support including `REDIS_HOST` and `REDIS_PORT`. |
| M067/M068 project memory | FalkorDB selected and fd v2 config completed, satisfying prior technical prerequisites but not automatically requiring queue execution. |
| M069 S01/S02 | Schema and metrics research adds new payload requirements before queue implementation. |

## Previously implied queue assumptions

From ADR-017 and discussion context:

- Queue would support async execution.
- Queue would enable smart scheduling.
- Queue would model per-article atomic DAG work.
- Queue would support multi-worker execution.
- Queue would support lease-based claiming.
- Queue was deferred until pipeline evidence justified infrastructure.
- Queue should not be built for an unvalidated pipeline.

## Updated prerequisites now satisfied

| Prerequisite | Current status |
|---|---|
| M061 2-hop BFS evidence | satisfied |
| M062 fd hardening | satisfied by M065-vq0do4 and M068 v2 verification |
| M063 GraphDB selection | satisfied, superseded to FalkorDB by ADR-022 |
| Redis env support | satisfied by M068 (`REDIS_HOST`, `REDIS_PORT`) |
| Canonical PDF catalog | satisfied at 220 PDFs |

## New M069 prerequisites discovered

M069 S01/S02 adds requirements that were not explicit in ADR-017:

- Queue payloads must carry `schema_version`.
- Queue payloads must carry `stable_id_version` or stable ID policy reference.
- Queue payloads must carry `metric_bundle_id`.
- Queue payloads must carry `extractor_version`.
- Queue payloads must carry `prompt_program_hash` for future DSPy/MiniMax work.
- Queue payloads must carry `source_artifact_refs` and `evidence_path_refs`.
- Queue payloads must record cost and latency diagnostics.
- Queue payloads must preserve `write_eligibility=false` and `promotion_eligibility=false` unless later explicitly authorized.
- Queue should not assume only current five-layer graph edges; it must be versioned for richer schema modules.

## Assumption risk table

| Assumption | Risk after M069 | Action |
|---|---|---|
| Queue can carry arbitrary article processing jobs | low | keep |
| Queue only needs current graph edge payloads | high | adjust before execution |
| Queue does not need metric metadata | high | adjust before execution |
| Queue can ignore stable ID policy | high | adjust before execution |
| Queue can enable graph writes as part of foundation | critical | keep disabled |
| Queue milestone can reuse `M064` ID | medium | avoid; generate fresh ID |
| Redis support is needed | medium | keep as env-supported option, not necessarily dependency for research queue |
| FalkorDB is the write target | medium | true for future production graph, but queue should stay write-disabled until authorization |

## Initial conclusion

The queue foundation is still conceptually useful, but the future plan should be updated before execution. The update is not a total redesign; it is a payload and gate contract adjustment driven by M069 schema and metric findings.
