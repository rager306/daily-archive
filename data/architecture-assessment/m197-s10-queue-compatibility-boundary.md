# M197 S10 Queue Compatibility Boundary

## Verdict

**PASS with guard: S10 may add compatibility tests and artifacts, but must not edit queue dependency semantics.**

## GitNexus impact evidence

| Target | Risk | Result |
|---|---:|---|
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | HIGH | impacted_count=5; affected processes include `run_universal_kb_no_write_rehearsal`, `run_article`, and `smoke.py main` |
| `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal` | LOW | impacted_count=0; no affected processes when used as a read-only compatibility baseline |

## HIGH-risk warning

`UniversalKBQueue._dependencies_satisfied` feeds no-write rehearsal, smoke runner, and smoke main. S10 must not edit it. Any future queue dependency behavior change requires a separate plan, explicit warning, and queue + rehearsal + smoke compatibility verification.

## Allowed S10 edits

- `tests/test_m197_queue_compatibility.py`
- S10 architecture assessment artifacts

## Disallowed S10 edits

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`
- `scripts/run_m197_reactive_dry_run.py` unless a blocker proves the S09 contract wrong
- Production graph backend code
- Schema migration code

## Required compatibility checks

- Run the S09 dry-run command in a temporary output directory.
- Run the sync no-write rehearsal in a separate temporary output directory.
- Confirm reactive events keep `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Confirm sync queue artifacts retain current keys such as `job_id`, `graphdb_written`, `ladybugdb_written`, `production_import_attempted`, `graph_import_allowed`, and `import_eligible`.
- Confirm the sync baseline still does not emit standalone `queue_events.json`.
- Confirm the reactive dry-run does not create or mutate `queue.sqlite`.

## Downstream dependency map

- S11 consumes S10 as proof that realistic no-write rehearsal can run alongside the reactive dry-run command.
- S12 consumes S10 as governance evidence for preserving queue semantics.
