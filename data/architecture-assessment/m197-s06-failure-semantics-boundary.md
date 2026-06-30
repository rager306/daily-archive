# M197 S06 Failure Semantics Boundary

## Verdict

**PASS: S06 may edit only the additive reactive runner and its tests.** Exact GitNexus impact is LOW for the runner functions and does not touch existing queue/rehearsal/smoke processes.

## GitNexus impact evidence

| Target | Risk | Impact | Affected processes |
|---|---:|---:|---|
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stage` | LOW | impacted_count=2 | none |
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stages_bounded` | LOW | impacted_count=0 | none |

`run_reactive_stage` currently has direct caller `run_one` and upstream `run_reactive_stages_bounded`; both are inside the additive runner module.

## Allowed S06 edits

- `src/research_graph/workflows/universal_kb/reactive_runner.py`
- `tests/test_m197_reactive_runner.py`
- S06 architecture assessment artifacts

## Disallowed S06 edits

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`

## Required failure semantics

- Timeout emits `stage.timeout`.
- Cancellation emits `stage.cancelled`.
- Timeout and cancellation do not emit `stage.completed` for the same stage run.
- Diagnostics store metadata codes such as `last_error_code`, not raw exception messages or payload text.
- Events keep `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.

## Boundary statement

S06 adds failure-state observability only. It does not alter queue dependency semantics, expose an operator script, contact graph backends, run schema migrations, or promote import eligibility.
