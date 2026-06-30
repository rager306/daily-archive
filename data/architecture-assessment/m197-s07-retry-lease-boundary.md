# M197 S07 Retry Heartbeat Lease Boundary

## Verdict

**PASS: S07 may add retry/heartbeat/lease metadata only inside the additive reactive runner. Queue dependency semantics remain HIGH risk and out of scope.**

## GitNexus impact evidence

| Target | Risk | Impact | Affected processes |
|---|---:|---:|---|
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stage` | LOW | impacted_count=2 | none |
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stages_bounded` | LOW | impacted_count=0 | none |
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | HIGH | impacted_count=5 | `run_universal_kb_no_write_rehearsal`, `run_article`, `smoke.py main` |

## Allowed S07 edits

- `src/research_graph/workflows/universal_kb/reactive_runner.py`
- `tests/test_m197_reactive_runner.py`
- S07 architecture assessment artifacts

## Disallowed S07 edits

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`

## Required semantics

- Retryable exceptions emit `stage.failed_retryable` rather than `stage.failed_terminal`.
- Retry events expose `retry_after_ms` as metadata only.
- Events may expose `heartbeat_at` and `lease_expires_at` metadata.
- Retry/heartbeat/lease metadata does not imply queue dependency satisfaction.
- All events keep `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.

## Boundary statement

S07 adds metadata observability only. It does not change queue leases, queue dependency resolution, unblock behavior, production graph imports, backend writes, schema migrations, or import eligibility.
