# M197 S06 Failure Semantics Audit

## Verdict

**PASS: timeout and cancellation semantics are observable, metadata-only, and compatible with no-write baselines and governance ratchets.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| Timeout/cancellation focused tests | PASS: 14 passed | `gsd_exec[fea5a451-e9ea-4738-8ce0-10c8576064cc]` |
| Failure semantics compatibility suite | PASS: 29 passed | `gsd_exec[fd478ee7-160a-46c2-8327-3c9c5495d0d6]` |

## What changed

- `run_reactive_stage` accepts optional `timeout_ms`.
- Timeout emits `stage.timeout` and status `timeout`.
- Cancellation emits `stage.cancelled` and status `cancelled`.
- Bounded execution forwards per-stage `timeout_ms`.
- Failure diagnostics store metadata-only `last_error_code` values.
- Timeout/cancelled stages do not emit `stage.completed`.

## Compatibility coverage

The suite covered:

- M197 reactive runner tests.
- M197 event contract tests.
- M197 sync no-write baseline tests.
- M196 run artifact observability tests.
- M196 governance ratchets.
- M195 governance ratchets.

## Safety findings

- Exception/cancellation messages are not copied into emitted events.
- Raw prompt/source/chunk/vector payload terms remain absent from tested events.
- All emitted events keep `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Existing queue, rehearsal, smoke runner, and smoke wrapper files were not edited.

## Boundary statement

S06 adds timeout and cancellation observability only. It does not alter queue dependency semantics, expose an operator script, contact graph backends, run schema migrations, or promote import eligibility.
