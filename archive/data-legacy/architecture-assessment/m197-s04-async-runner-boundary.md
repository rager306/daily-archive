# M197 S04 Async Runner Boundary

## Verdict

**PASS: S04 should add a new async runner module without editing existing queue, rehearsal, or smoke runner semantics.**

## Module placement

New module:

- `src/research_graph/workflows/universal_kb/reactive_runner.py`

New tests:

- `tests/test_m197_reactive_runner.py`

## Additive strategy

S04 must not edit:

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`

The new module should be imported only by new tests in S04. Existing sync call paths remain unchanged.

## Public runner shape

The S04 runner should provide a minimal async primitive:

- run one async stage;
- emit `stage.started`;
- emit `stage.completed` on success;
- emit `stage.failed_terminal` on exception;
- preserve no-write defaults;
- return metadata events, not raw payloads.

## Event mapping

The runner must emit events compatible with `m197.reactive_event.v1` required fields:

- `schema_version`
- `event_type`
- `job_id`
- `stage_id`
- `correlation_id`
- `phase`
- `status`
- `attempt`
- `timestamp`
- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`
- `artifact_refs`
- `diagnostics`

## Failure behavior

For S04, exceptions become `stage.failed_terminal` events with a metadata-only `last_error_code` style diagnostic. S06 will add timeout/cancellation semantics; S04 should not overbuild them.

## Payload safety

The runner must not persist or echo raw prompts, source text payloads, chunk text payloads, embeddings, vectors, API keys, or secret values. Stage results may expose artifact refs and diagnostics only.

## Boundary statement

S04 introduces an additive runner foundation. It does not change queue dependency semantics, run production graph imports, write to graph backends, run schema migrations, or promote import eligibility.
