# M197 Operator Handoff: Reactive No-Write Dry Run

## Reader and action

Reader: a future operator or agent landing cold on M197.

After reading this handoff, the reader should be able to run the reactive dry-run pilot, inspect its JSONL lifecycle events, and avoid claiming graph/import readiness.

## Command

Run the dry-run pilot with an explicit event path:

```bash
uv run python scripts/run_m197_reactive_dry_run.py \
  --events artifacts/m197-reactive-dry-run/events.jsonl
```

Expected stdout contains:

```text
m197_reactive_events=4
events_path=artifacts/m197-reactive-dry-run/events.jsonl
```

## Expected JSONL shape

The command writes four events for two deterministic dry-run stages:

1. `stage.started` for `dry_run.schema_gate`
2. `stage.completed` for `dry_run.schema_gate`
3. `stage.started` for `dry_run.projection_safety`
4. `stage.completed` for `dry_run.projection_safety`

Every event must include the `m197.reactive_event.v1` identity and state fields:

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

Completed events also carry lineage metadata:

- `parent_artifact_refs`
- `child_artifact_refs`
- `checksum_sha256`

## Safety invariants

These values are not optional. If any event violates them, stop and treat the run as failed evidence:

- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`

The pilot is metadata-only. It must not persist raw prompts, source text, chunk text, embeddings, vectors, API keys, secrets, or production payloads.

## Evidence map

Use these evidence surfaces when deciding whether the pilot is safe enough for the next validation step:

| Evidence | What it proves |
|---|---|
| `tests/test_m197_reactive_dry_run.py` | The dry-run command writes contract-shaped JSONL events. |
| `tests/test_m197_queue_compatibility.py` | Reactive dry-run output does not alter queue/rehearsal safety surfaces. |
| `tests/test_m197_realistic_no_write_rehearsal.py` | Multiple dry-run jobs remain no-write and payload-safe beside sync rehearsal. |
| `tests/test_m197_governance_ratchets.py` | S09-S12 evidence and no-write/import-blocked constraints are ratcheted. |
| `data/architecture-assessment/m197-s09-scope-verification.md` | Script dry-run integration scope. |
| `data/architecture-assessment/m197-s10-scope-verification.md` | Queue compatibility scope. |
| `data/architecture-assessment/m197-s11-scope-verification.md` | Realistic rehearsal scope. |
| `data/architecture-assessment/m197-s12-scope-verification.md` | Governance ratchet scope. |

## Troubleshooting

### Invalid concurrency

If the command is run with `--max-concurrency 0`, it must fail with `max_concurrency must be >= 1`. That is expected and protects bounded execution semantics.

### Missing events file

If the events file is missing, do not infer success from stdout alone. Rerun the command and inspect the path printed as `events_path`.

### Payload-shaped terms

If event JSON contains forbidden payload-shaped terms such as `api_key`, `secret_value`, `embedding_payload`, `vector_payload`, or `raw_prompt_payload`, the run is not valid evidence.

### Readiness claims

The dry-run command does not prove production graph readiness. It proves only that the reactive pilot can emit metadata-only lifecycle events under no-write/import-blocked constraints.

## Explicit non-goals

This handoff does not authorize:

- production graph import;
- schema migration;
- queue dependency semantic changes;
- smoke or no-write rehearsal semantic changes;
- direct graph backend writes;
- treating `import_eligible=true` as acceptable evidence.

## Next safe step

Run the final compatibility sweep before validation readiness:

```bash
uv run pytest \
  tests/test_m197_operator_handoff.py \
  tests/test_m197_governance_ratchets.py \
  tests/test_m197_realistic_no_write_rehearsal.py \
  tests/test_m197_queue_compatibility.py \
  tests/test_m197_reactive_dry_run.py \
  tests/test_m197_reactive_runner.py \
  tests/test_m197_reactive_event_contract.py \
  tests/test_m197_sync_baseline.py \
  tests/test_m196_queue_resilience.py \
  tests/test_m196_run_artifact_observability.py \
  tests/test_m196_governance_ratchets.py \
  tests/test_m195_governance_ratchets.py \
  -q
```
