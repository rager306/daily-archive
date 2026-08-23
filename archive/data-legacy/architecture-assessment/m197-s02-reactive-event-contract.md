# M197 S02 Reactive Event Contract

## Verdict

**PASS: M197 has a versioned reactive event contract for a no-write async pilot.** The contract is planning-only until tested and consumed by later slices.

## Contract artifact

- JSON: `data/architecture-assessment/m197-reactive-event-contract.json`
- Schema version: `m197.reactive_event.v1`
- Scope: `no_write_async_reactive_pilot`

## Required event fields

- `schema_version`
- `event_type`
- `job_id`
- `stage_id`
- `correlation_id`
- `phase`
- `status`
- `attempt`
- `timestamp`
- `graph_writes_allowed`
- `schema_migration_allowed`
- `import_eligible`
- `artifact_refs`
- `diagnostics`

## Event types

The contract covers job lifecycle events, heartbeat/retry events, stage lifecycle events, artifact registration, timeout, and cancellation.

## Safety invariants

All M197 events must keep:

- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`
- `production_graph_import=false`
- `ladybugdb_write=false`
- `falkordb_write=false`

## Payload safety

Forbidden payload-shaped terms include:

- `raw_prompt_payload`
- `source_text_payload`
- `paper_text_payload`
- `chunk_text_payload`
- `embedding_payload`
- `vector_payload`
- `api_key`
- `secret_value`

Allowed metadata includes checksums, sizes, redaction status, source refs, and artifact refs.

## Ordering assumptions

- `job.created` precedes `job.claimed`.
- `stage.started` precedes terminal stage outcomes.
- `stage.artifact_registered` requires at least one artifact ref.
- Terminal statuses are not followed by success for the same stage.
- `import_eligible` remains false for all M197 events.

## Boundary statement

S02 defines the event surface only. It does not implement async execution, change queue dependency semantics, enable graph writes, run schema migrations, or promote import eligibility.
