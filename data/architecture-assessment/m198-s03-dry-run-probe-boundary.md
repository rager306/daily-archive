# M198 S03 Dry Run Probe Boundary

## Verdict

**PASS: S03 may add a new dry-run probe script and tests.** The existing M197 dry-run command remains an unedited read-only input.

## GitNexus evidence

| Target | Result |
|---|---|
| `Function:scripts/run_m197_reactive_dry_run.py:main` | LOW, impacted_count=1, affected_processes=[] |

## Allowed S03 edits

- `scripts/run_m198_dry_run_probe.py`
- `tests/test_m198_dry_run_probe.py`
- S03 architecture assessment artifacts

## Disallowed S03 edits

- `scripts/run_m197_reactive_dry_run.py`
- `src/research_graph/workflows/universal_kb/reactive_runner.py`
- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`
- production graph backend code
- schema migration code

## Required probe behavior

- Reads existing M197 dry-run JSONL events.
- Writes one M198 readiness evidence JSON file.
- Uses `source_kind=reactive_dry_run`.
- Preserves `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Records event count, event file ref, checksums, diagnostics, and non-goals.
- Rejects missing event files, bad readiness flags, and forbidden payload-shaped terms.
- Does not create `queue.sqlite`.

## Downstream dependency map

- S07 consumes probe output for drift classification.
- S08 consumes probe output for evidence indexing.
